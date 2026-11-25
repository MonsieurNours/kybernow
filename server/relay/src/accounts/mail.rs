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
    /// Un appareil ouvre pour la première fois un ordinateur du compte
    /// (garde-fou 4, « Avis à chaque nouvel appareil ») ; `account` : la page du compte.
    /// Lot 4 : `person`, l'appareil d'une personne ajoutée (son nom), pas du propriétaire.
    NewDevice { device: &'a str, computer: &'a str, person: Option<&'a str>, account: &'a str },
    /// Lot 4 : quelqu'un vous ouvre un ordinateur ; `rights` : les droits en mots ;
    /// `link` : la console (avec un compte) ou l'inscription (sans compte).
    ComputerShared { by: &'a str, computer: &'a str, rights: &'a str, until: Option<&'a str>, has_account: bool, link: &'a str },
    /// Lot 5 : invitation dans une organisation ; `role` : le rôle en mots.
    OrgInvite { by: &'a str, org: &'a str, role: &'a str, has_account: bool, link: &'a str },
    /// Une passkey a été ajoutée (lot 1b).
    PasskeyAdded { name: &'a str, account: &'a str },
    /// Le compte a été supprimé (lot 1b).
    AccountDeleted,
    /// Admin d'un ordinateur ouvert à distance (« Admin depuis KyberNow Share »).
    AdminOpened { by: &'a str, computer: &'a str, date: &'a str },
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
        Message::AdminOpened { by, computer, date } => (
            match lang {
                Lang::Fr => t(lang, &format!("Admin de {computer} ouvert à distance"), ""),
                Lang::En => format!("{computer}'s Admin opened remotely"),
            },
            format!(
                "{hello}\n\n{what}\n\n{check}\n\n{sign}\n",
                hello = tr("Bonjour,", "Hello,"),
                what = match lang {
                    Lang::Fr => t(lang, &format!("{} a ouvert Admin de {computer} le {date}, par KyberNow Share.", if by.is_empty() { "Quelqu'un" } else { by }), ""),
                    Lang::En => format!("{} opened {computer}'s Admin on {date}, through KyberNow Share.", if by.is_empty() { "Someone" } else { by }),
                },
                check = tr(
                    "Ce n'est pas normal ? Coupez « Admin à distance » dans Admin, sur l'ordinateur (Hors de chez vous).",
                    "Not expected? Turn off \u{201c}Remote Admin\u{201d} in Admin, on the computer (Away from home).",
                ),
            ),
        ),
        Message::NewDevice { device, computer, person, account } => {
            let device = if device.is_empty() { tr("Un appareil", "A device") } else { device.to_string() };
            let whose = match (person, lang) {
                (None, Lang::Fr) => t(lang, "avec votre compte KyberNow", ""),
                (None, Lang::En) => "with your KyberNow account".to_string(),
                (Some(p), Lang::Fr) => t(lang, &format!("avec le compte KyberNow de {p}"), ""),
                (Some(p), Lang::En) => format!("with {p}'s KyberNow account"),
            };
            (
                match lang {
                    Lang::Fr => t(lang, &format!("Nouvel appareil sur {computer}"), ""),
                    Lang::En => format!("New device on {computer}"),
                },
                format!(
                    "{hello}\n\n{what}\n\n{check}\n{account}\n\n{sign}\n",
                    hello = tr("Bonjour,", "Hello,"),
                    what = match lang {
                        Lang::Fr => t(lang, &format!("{device} vient d'ouvrir {computer} pour la première fois, {whose}."), ""),
                        Lang::En => format!("{device} just opened {computer} for the first time, {whose}."),
                    },
                    check = if person.is_none() {
                        tr(
                            "Ce n'était pas vous ? Déconnectez cet appareil et changez votre mot de passe ; dans Admin, « Compte KyberNow » permet aussi de le bloquer :",
                            "Wasn't you? Disconnect this device and change your password; in Admin, \"KyberNow account\" can also block it:",
                        )
                    } else {
                        tr(
                            "Quelque chose ne va pas ? Retirez cette personne de l'ordinateur dans la console ; dans Admin, « Compte KyberNow » permet aussi de bloquer l'appareil :",
                            "Something wrong? Remove this person from the computer in the console; in Admin, \"KyberNow account\" can also block the device:",
                        )
                    },
                ),
            )
        }
        Message::ComputerShared { by, computer, rights, until, has_account, link } => (
            match lang {
                Lang::Fr => t(lang, &format!("{by} vous ouvre {computer}"), ""),
                Lang::En => format!("{by} opened {computer} for you"),
            },
            format!(
                "{hello}\n\n{what}\n\n    {rights}{until}\n\n{next}\n{link}\n\n{sign}\n",
                hello = tr("Bonjour,", "Hello,"),
                what = match lang {
                    Lang::Fr => t(lang, &format!("{by} vous ajoute à {computer}, avec ces droits :"), ""),
                    Lang::En => format!("{by} added you to {computer}, with these rights:"),
                },
                until = match (until, lang) {
                    (None, _) => String::new(),
                    (Some(d), Lang::Fr) => t(lang, &format!(", jusqu'au {d}"), ""),
                    (Some(d), Lang::En) => format!(", until {d}"),
                },
                next = if has_account {
                    tr(
                        "L'ordinateur apparaît déjà dans vos apps KyberNow, connectées à votre compte. La console :",
                        "The computer already shows up in your KyberNow apps, signed in to your account. The console:",
                    )
                } else {
                    tr(
                        "Créez votre compte KyberNow avec cette adresse : l'ordinateur y sera dès l'adresse vérifiée.",
                        "Create your KyberNow account with this address: the computer will be there once the address is verified.",
                    )
                },
            ),
        ),
        Message::OrgInvite { by, org, role, has_account, link } => (
            match lang {
                Lang::Fr => t(lang, &format!("{by} vous invite dans {org}"), ""),
                Lang::En => format!("{by} invited you to {org}"),
            },
            format!(
                "{hello}\n\n{what}\n\n{next}\n{link}\n\n{sign}\n",
                hello = tr("Bonjour,", "Hello,"),
                what = match lang {
                    Lang::Fr => t(lang, &format!("{by} vous ajoute à l'organisation {org} sur KyberNow, comme {role}."), ""),
                    Lang::En => format!("{by} added you to the organization {org} on KyberNow, as {role}."),
                },
                next = if has_account {
                    tr("Ses ordinateurs ouverts à vous apparaissent dans vos apps. La console :", "Its computers opened to you show up in your apps. The console:")
                } else {
                    tr(
                        "Créez votre compte KyberNow avec cette adresse : vous serez membre dès l'adresse vérifiée.",
                        "Create your KyberNow account with this address: you'll be a member once the address is verified.",
                    )
                },
            ),
        ),
        Message::PasskeyAdded { name, account } => (
            tr("Passkey ajoutée à votre compte KyberNow", "Passkey added to your KyberNow account"),
            format!(
                "{hello}\n\n{what}\n\n    {name}\n\n{check}\n{account}\n\n{sign}\n",
                hello = tr("Bonjour,", "Hello,"),
                what = tr(
                    "Une passkey vient d'être ajoutée à votre compte KyberNow ; elle ouvre le compte sans mot de passe :",
                    "A passkey was just added to your KyberNow account; it opens the account without a password:",
                ),
                check = tr(
                    "Ce n'était pas vous ? Retirez-la, changez votre mot de passe et vérifiez les sessions ouvertes :",
                    "Wasn't you? Remove it, change your password and check the open sessions:",
                ),
            ),
        ),
        Message::AccountDeleted => (
            tr("Votre compte KyberNow est supprimé", "Your KyberNow account is deleted"),
            format!(
                "{hello}\n\n{what}\n\n{after}\n\n{sign}\n",
                hello = tr("Bonjour,", "Hello,"),
                what = tr(
                    "Votre compte KyberNow vient d'être supprimé, avec ses passkeys, ses sessions, ses appareils connectés et son journal ; ses ordinateurs sont détachés.",
                    "Your KyberNow account was just deleted, along with its passkeys, sessions, connected devices and log; its computers are detached.",
                ),
                after = tr(
                    "Les sauvegardes du relais l'oublient sous 30 jours. Les logiciels KyberNow restent utilisables sans compte.",
                    "The relay's backups forget it within 30 days. The KyberNow software stays usable without an account.",
                ),
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
            Message::NewDevice { device: "iPad de Léa", computer: "PC du salon", person: None, account: "c" },
            Message::NewDevice { device: "", computer: "PC du salon", person: Some("Léa"), account: "c" },
            Message::ComputerShared { by: "Ana", computer: "PC du salon", rights: "Session, Partage", until: Some("31/12/2026"), has_account: true, link: "c" },
            Message::ComputerShared { by: "Ana", computer: "PC du salon", rights: "Prêt", until: None, has_account: false, link: "c" },
            Message::OrgInvite { by: "Ana", org: "Les Lignes Claires", role: "Membre", has_account: false, link: "c" },
            Message::PasskeyAdded { name: "Chrome, Mac", account: "c" },
            Message::AccountDeleted,
            Message::AdminOpened { by: "Camille", computer: "PC du salon", date: "25/11/2026" },
        ] {
            for lang in [Lang::Fr, Lang::En] {
                let mail = compose("a@exemple.org", lang, m);
                assert!(!mail.subject.is_empty() && mail.text.ends_with("KyberNow\n"));
                assert!(!mail.text.contains('\u{2014}'), "tiret cadratin");
            }
        }
    }
}
