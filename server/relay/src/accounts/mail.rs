//! Les e-mails des comptes (docs/comptes.md, « E-mails ») : en français ou en
//! anglais, en texte simple. L'envoi passe par SMTP (`[accounts.smtp]`), dans
//! un fil à part : une page n'attend jamais le serveur d'e-mails, et le temps
//! de réponse ne dit pas si un e-mail est parti. Les journaux ne contiennent
//! ni adresse ni jeton.

use std::path::PathBuf;
use std::sync::Mutex;
use std::sync::mpsc;

use anyhow::{Context, Result};
use lettre::message::Mailbox;
use lettre::message::header::ContentType;
use lettre::transport::smtp::authentication::Credentials;
use lettre::{SmtpTransport, Transport};

use crate::config::SmtpConfig;
use crate::pages::{Lang, t};

/// Un e-mail à envoyer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Mail {
    pub to: String,
    pub subject: String,
    pub text: String,
}

/// Qui envoie les e-mails : SMTP, ou une boîte en mémoire pour les essais.
pub trait Mailer: Send + Sync {
    /// Met l'e-mail dans la file ; ne bloque pas.
    fn send(&self, mail: Mail);
}

/// Les e-mails gardés en mémoire (essais).
#[derive(Debug, Default)]
pub struct Outbox(pub Mutex<Vec<Mail>>);

impl Mailer for Outbox {
    fn send(&self, mail: Mail) {
        self.0.lock().unwrap().push(mail);
    }
}

impl Outbox {
    /// Le dernier e-mail envoyé à cette adresse.
    pub fn last_to(&self, to: &str) -> Option<Mail> {
        self.0.lock().unwrap().iter().rev().find(|m| m.to == to).cloned()
    }

    pub fn count_to(&self, to: &str) -> usize {
        self.0.lock().unwrap().iter().filter(|m| m.to == to).count()
    }
}

/// L'envoi par SMTP, dans un fil qui garde la connexion.
pub struct Smtp {
    tx: Mutex<mpsc::Sender<Mail>>,
}

impl Smtp {
    pub fn start(cfg: &SmtpConfig) -> Result<Smtp> {
        let from: Mailbox = cfg.from.parse().with_context(|| "[accounts.smtp] from : adresse illisible".to_string())?;
        let mut builder = if cfg.starttls { SmtpTransport::starttls_relay(&cfg.host) } else { SmtpTransport::relay(&cfg.host) }
            .with_context(|| format!("[accounts.smtp] host : {}", cfg.host))?
            .port(cfg.port);
        if let Some(user) = &cfg.user {
            let password = match &cfg.password_file {
                Some(p) => read_secret(p)?,
                None => String::new(),
            };
            builder = builder.credentials(Credentials::new(user.clone(), password));
        }
        let transport = builder.build();
        let (tx, rx) = mpsc::channel::<Mail>();
        std::thread::Builder::new().name("comptes-smtp".into()).spawn(move || {
            for mail in rx {
                let Ok(to) = mail.to.parse::<Mailbox>() else {
                    tracing::warn!("e-mail non envoyé : adresse illisible");
                    continue;
                };
                let message = lettre::Message::builder()
                    .from(from.clone())
                    .to(to)
                    .subject(mail.subject)
                    .header(ContentType::TEXT_PLAIN)
                    .body(mail.text);
                let message = match message {
                    Ok(m) => m,
                    Err(_) => {
                        tracing::warn!("e-mail non envoyé : message invalide");
                        continue;
                    }
                };
                if let Err(e) = transport.send(&message) {
                    // Ni l'adresse ni la réponse du serveur (elle peut citer l'adresse) : la sorte d'erreur seulement.
                    let kind = if e.is_permanent() {
                        "refus définitif"
                    } else if e.is_transient() {
                        "refus passager"
                    } else {
                        "connexion"
                    };
                    let code = e.status().map(|c| c.to_string()).unwrap_or_default();
                    tracing::warn!(erreur = kind, code = %code, "e-mail non envoyé");
                }
            }
        })?;
        Ok(Smtp { tx: Mutex::new(tx) })
    }
}

impl Mailer for Smtp {
    fn send(&self, mail: Mail) {
        let _ = self.tx.lock().unwrap().send(mail);
    }
}

fn read_secret(path: &PathBuf) -> Result<String> {
    let text = std::fs::read_to_string(path).with_context(|| format!("[accounts.smtp] password_file : {}", path.display()))?;
    Ok(text.trim_end_matches(['\r', '\n']).to_string())
}

/// Ce que dit un e-mail.
#[derive(Debug, Clone, Copy)]
pub enum Message<'a> {
    /// Vérifier l'adresse : le code et le lien (24 h).
    Verify { name: &'a str, code: &'a str, link: &'a str },
    /// Mot de passe oublié : le lien (1 h).
    Reset { link: &'a str },
    /// Quelqu'un a voulu créer un compte avec une adresse déjà prise.
    SignupTaken { sign_in: &'a str, forgot: &'a str },
    /// La double authentification a été activée (`true`) ou coupée.
    TotpChanged { on: bool, account: &'a str },
    /// Le mot de passe a changé.
    PasswordChanged { forgot: &'a str },
}

/// Le code « 123 456 », plus facile à lire.
pub fn spaced_code(code: &str) -> String {
    if code.len() == 6 { format!("{} {}", &code[..3], &code[3..]) } else { code.to_string() }
}

/// Compose un e-mail dans la langue du compte.
pub fn compose(to: &str, lang: Lang, message: Message<'_>) -> Mail {
    let tr = |f: &str, e: &str| t(lang, f, e);
    let sign = tr("KyberNow", "KyberNow");
    let (subject, text) = match message {
        Message::Verify { name, code, link } => (
            format!("{} {}", tr("Votre code KyberNow :", "Your KyberNow code:"), spaced_code(code)),
            format!(
                "{hello}\n\n{intro}\n\n    {code}\n\n{or}\n{link}\n\n{ignore}\n\n{sign}\n",
                hello = if name.is_empty() { tr("Bonjour,", "Hello,") } else { format!("{} {name},", tr("Bonjour", "Hello")) },
                intro = tr("Voici le code pour vérifier votre adresse e-mail :", "Here is the code to verify your email address:"),
                code = spaced_code(code),
                or = tr("Ou ouvrez ce lien, valable 24 heures :", "Or open this link, valid for 24 hours:"),
                ignore = tr(
                    "Vous n'avez pas créé de compte KyberNow ? Ignorez cet e-mail : sans vérification, le compte ne sert à rien.",
                    "You didn't create a KyberNow account? Ignore this email: without verification, the account is of no use.",
                ),
            ),
        ),
        Message::Reset { link } => (
            tr("Nouveau mot de passe KyberNow", "New KyberNow password"),
            format!(
                "{hello}\n\n{intro}\n{link}\n\n{ignore}\n\n{sign}\n",
                hello = tr("Bonjour,", "Hello,"),
                intro = tr(
                    "Pour choisir un nouveau mot de passe, ouvrez ce lien, valable une heure :",
                    "To choose a new password, open this link, valid for one hour:",
                ),
                ignore = tr(
                    "Vous n'avez rien demandé ? Ignorez cet e-mail : votre mot de passe ne change pas.",
                    "You didn't ask for anything? Ignore this email: your password stays the same.",
                ),
            ),
        ),
        Message::SignupTaken { sign_in, forgot } => (
            tr("Votre compte KyberNow existe déjà", "Your KyberNow account already exists"),
            format!(
                "{hello}\n\n{intro}\n\n{login}\n{sign_in}\n\n{lost}\n{forgot}\n\n{ignore}\n\n{sign}\n",
                hello = tr("Bonjour,", "Hello,"),
                intro = tr(
                    "Quelqu'un vient de vouloir créer un compte KyberNow avec cette adresse, qui en a déjà un.",
                    "Someone just tried to create a KyberNow account with this address, which already has one.",
                ),
                login = tr("C'était vous ? Connectez-vous :", "Was it you? Sign in:"),
                lost = tr("Mot de passe oublié :", "Forgot your password:"),
                ignore = tr("Ce n'était pas vous ? Rien n'a changé, ignorez cet e-mail.", "Wasn't you? Nothing has changed, ignore this email."),
            ),
        ),
        Message::TotpChanged { on, account } => {
            let (subject, what) = if on {
                (
                    tr("Double authentification activée", "Two-factor authentication turned on"),
                    tr(
                        "La double authentification vient d'être activée sur votre compte KyberNow.",
                        "Two-factor authentication was just turned on for your KyberNow account.",
                    ),
                )
            } else {
                (
                    tr("Double authentification coupée", "Two-factor authentication turned off"),
                    tr(
                        "La double authentification vient d'être coupée sur votre compte KyberNow.",
                        "Two-factor authentication was just turned off for your KyberNow account.",
                    ),
                )
            };
            (
                subject,
                format!(
                    "{hello}\n\n{what}\n\n{check}\n{account}\n\n{sign}\n",
                    hello = tr("Bonjour,", "Hello,"),
                    check = tr(
                        "Ce n'était pas vous ? Changez votre mot de passe et vérifiez les sessions ouvertes :",
                        "Wasn't you? Change your password and check the open sessions:",
                    ),
                ),
            )
        }
        Message::PasswordChanged { forgot } => (
            tr("Mot de passe KyberNow changé", "KyberNow password changed"),
            format!(
                "{hello}\n\n{what}\n\n{check}\n{forgot}\n\n{sign}\n",
                hello = tr("Bonjour,", "Hello,"),
                what = tr(
                    "Le mot de passe de votre compte KyberNow vient de changer ; les autres sessions sont fermées.",
                    "The password of your KyberNow account was just changed; the other sessions are closed.",
                ),
                check = tr("Ce n'était pas vous ? Choisissez-en un autre tout de suite :", "Wasn't you? Choose another one right away:"),
            ),
        ),
    };
    Mail { to: to.to_string(), subject, text }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn e_mails_dans_les_deux_langues() {
        let fr = compose("a@exemple.org", Lang::Fr, Message::Verify { name: "Ana", code: "123456", link: "https://share.test/compte/verifier?jeton=x" });
        assert_eq!(fr.subject, "Votre code KyberNow\u{202F}: 123 456");
        assert!(fr.text.starts_with("Bonjour Ana,"));
        assert!(fr.text.contains("    123 456\n"));
        assert!(fr.text.contains("https://share.test/compte/verifier?jeton=x"));
        assert!(!fr.text.contains(" :") && !fr.text.contains(" ?"), "typographie française");
        let en = compose("a@exemple.org", Lang::En, Message::Reset { link: "https://share.test/x" });
        assert_eq!(en.subject, "New KyberNow password");
        assert!(en.text.contains("valid for one hour"));
        for m in [
            Message::SignupTaken { sign_in: "s", forgot: "f" },
            Message::TotpChanged { on: true, account: "c" },
            Message::TotpChanged { on: false, account: "c" },
            Message::PasswordChanged { forgot: "f" },
        ] {
            for lang in [Lang::Fr, Lang::En] {
                let mail = compose("a@exemple.org", lang, m);
                assert!(!mail.subject.is_empty() && mail.text.ends_with("KyberNow\n"));
                assert!(!mail.text.contains('\u{2014}'), "tiret cadratin");
            }
        }
    }
}
