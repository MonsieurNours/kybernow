//! Les e-mails des comptes (docs/comptes.md, « E-mails ») : en français ou en
//! anglais, en HTML avec une version texte (le même contenu, rendu deux fois).
//! L'envoi passe par SMTP (`[accounts.smtp]`), dans
//! un fil à part : une page n'attend jamais le serveur d'e-mails, et le temps
//! de réponse ne dit pas si un e-mail est parti. Les journaux ne contiennent
//! ni adresse ni jeton.

use std::path::PathBuf;
use std::sync::Mutex;
use std::sync::mpsc;

use anyhow::{Context, Result};
use lettre::message::header::{Header, HeaderName, HeaderValue};
use lettre::message::{Mailbox, MultiPart};
use lettre::transport::smtp::authentication::Credentials;
use lettre::{SmtpTransport, Transport};

use crate::config::SmtpConfig;
use crate::accounts::pages::esc;
use crate::pages::{Lang, t};

/// Un e-mail à envoyer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Mail {
    pub to: String,
    pub subject: String,
    /// La version texte (lue par les essais, pour les liens et les codes).
    pub text: String,
    /// La version HTML, envoyée avec la version texte.
    pub html: String,
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
        // SendGrid réécrit les liens et ajoute un pixel pour suivre les e-mails : coupé pour chacun.
        let sendgrid = cfg.host.ends_with("sendgrid.net");
        let (tx, rx) = mpsc::channel::<Mail>();
        std::thread::Builder::new().name("comptes-smtp".into()).spawn(move || {
            for mail in rx {
                let Ok(to) = mail.to.parse::<Mailbox>() else {
                    tracing::warn!("e-mail non envoyé : adresse illisible");
                    continue;
                };
                let mut builder = lettre::Message::builder().from(from.clone()).to(to).subject(mail.subject);
                if sendgrid {
                    builder = builder.header(NoTracking);
                }
                let message = builder.multipart(MultiPart::alternative_plain_html(mail.text, mail.html));
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

/// `X-SMTPAPI` : pas de suivi des clics ni des ouvertures chez SendGrid.
#[derive(Clone)]
struct NoTracking;

const NO_TRACKING: &str = r#"{"filters":{"clicktrack":{"settings":{"enable":0,"enable_text":false}},"opentrack":{"settings":{"enable":0}}}}"#;

impl Header for NoTracking {
    fn name() -> HeaderName {
        HeaderName::new_from_ascii_str("X-SMTPAPI")
    }

    fn parse(_: &str) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        Ok(NoTracking)
    }

    fn display(&self) -> HeaderValue {
        HeaderValue::new(Self::name(), NO_TRACKING.to_string())
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

/// Un morceau du corps d'un e-mail, rendu en HTML et en texte.
enum Block {
    /// Un paragraphe.
    P(String),
    /// Le détail en encadré : une étiquette, la ligne principale, une ligne de plus.
    Detail { label: String, main: String, sub: Option<String> },
    /// Le code à six chiffres, en grand.
    Code(String),
    /// Le bouton de l'action ; en texte, l'étiquette puis le lien.
    Button { label: String, url: String },
    /// Un lien secondaire, sous le bouton.
    Link { label: String, url: String },
    /// La note séparée : « Ce n'était pas vous ? » en gras, puis la suite.
    Note { lead: String, text: String },
}

/// Ce que dit un e-mail, avant sa mise en forme.
struct Body {
    heading: String,
    hello: String,
    blocks: Vec<Block>,
    /// Pourquoi cet e-mail arrive (pied de l'e-mail).
    reason: String,
}

/// Compose un e-mail dans la langue du compte ; `site` : l'adresse publique du relais
/// (logo et liens du pied de l'e-mail).
pub fn compose(site: &str, to: &str, lang: Lang, message: Message<'_>) -> Mail {
    let (subject, body) = content(lang, message);
    let text = text_of(lang, &body);
    let html = html_of(site, lang, &subject, &body);
    Mail { to: to.to_string(), subject, text, html }
}

fn content(lang: Lang, message: Message<'_>) -> (String, Body) {
    let tr = |f: &str, e: &str| t(lang, f, e);
    let fe = |f: String, e: String| match lang {
        Lang::Fr => crate::pages::fr(&f),
        Lang::En => e,
    };
    let hello = tr("Bonjour,", "Hello,");
    let security = tr(
        "Vous recevez cet e-mail parce qu'il concerne la sécurité de votre compte KyberNow.",
        "You're receiving this email because it concerns the security of your KyberNow account.",
    );
    let not_you = tr("Ce n'était pas vous ?", "Wasn't you?");
    match message {
        Message::Verify { name, code, link } => (
            format!("{} {}", tr("Votre code KyberNow :", "Your KyberNow code:"), spaced_code(code)),
            Body {
                heading: if name.is_empty() { tr("Vérifiez votre adresse", "Verify your address") } else { fe(format!("Bienvenue, {name}"), format!("Welcome, {name}")) },
                hello: if name.is_empty() { hello } else { format!("{} {name},", tr("Bonjour", "Hello")) },
                blocks: vec![
                    Block::P(tr("Voici le code pour vérifier votre adresse e-mail :", "Here is the code to verify your email address:")),
                    Block::Code(spaced_code(code)),
                    Block::P(tr("Ou vérifiez-la d'un clic ; le lien reste valable 24 heures.", "Or verify it in one click; the link stays valid for 24 hours.")),
                    Block::Button { label: tr("Vérifier mon adresse", "Verify my address"), url: link.to_string() },
                    Block::Note {
                        lead: tr("Vous n'avez pas créé de compte KyberNow ?", "You didn't create a KyberNow account?"),
                        text: tr("Ignorez cet e-mail : sans vérification, le compte ne sert à rien.", "Ignore this email: without verification, the account is of no use."),
                    },
                ],
                reason: tr(
                    "Vous recevez cet e-mail parce qu'un compte KyberNow vient d'être créé avec cette adresse.",
                    "You're receiving this email because a KyberNow account was just created with this address.",
                ),
            },
        ),
        Message::Reset { link } => (
            tr("Nouveau mot de passe KyberNow", "New KyberNow password"),
            Body {
                heading: tr("Nouveau mot de passe", "New password"),
                hello,
                blocks: vec![
                    Block::P(tr(
                        "Pour choisir un nouveau mot de passe, ouvrez ce lien. Il reste valable une heure.",
                        "To choose a new password, open this link. It stays valid for one hour.",
                    )),
                    Block::Button { label: tr("Choisir un mot de passe", "Choose a password"), url: link.to_string() },
                    Block::Note {
                        lead: tr("Vous n'avez rien demandé ?", "You didn't ask for anything?"),
                        text: tr("Ignorez cet e-mail : votre mot de passe ne change pas.", "Ignore this email: your password stays the same."),
                    },
                ],
                reason: tr(
                    "Vous recevez cet e-mail parce qu'un nouveau mot de passe a été demandé pour votre compte KyberNow.",
                    "You're receiving this email because a new password was requested for your KyberNow account.",
                ),
            },
        ),
        Message::SignupTaken { sign_in, forgot } => (
            tr("Votre compte KyberNow existe déjà", "Your KyberNow account already exists"),
            Body {
                heading: tr("Vous avez déjà un compte", "You already have an account"),
                hello,
                blocks: vec![
                    Block::P(tr(
                        "Quelqu'un vient de vouloir créer un compte KyberNow avec cette adresse, qui en a déjà un. C'était vous ? Connectez-vous.",
                        "Someone just tried to create a KyberNow account with this address, which already has one. Was it you? Sign in.",
                    )),
                    Block::Button { label: tr("Se connecter", "Sign in"), url: sign_in.to_string() },
                    Block::Link { label: tr("Mot de passe oublié", "Forgot your password"), url: forgot.to_string() },
                    Block::Note { lead: not_you, text: tr("Rien n'a changé, ignorez cet e-mail.", "Nothing has changed, ignore this email.") },
                ],
                reason: security,
            },
        ),
        Message::TotpChanged { on, account } => {
            let (subject, heading, what) = if on {
                (
                    tr("Double authentification activée", "Two-factor authentication turned on"),
                    tr("Double authentification activée", "Two-factor authentication on"),
                    tr(
                        "La double authentification vient d'être activée sur votre compte KyberNow.",
                        "Two-factor authentication was just turned on for your KyberNow account.",
                    ),
                )
            } else {
                (
                    tr("Double authentification coupée", "Two-factor authentication turned off"),
                    tr("Double authentification coupée", "Two-factor authentication off"),
                    tr(
                        "La double authentification vient d'être coupée sur votre compte KyberNow.",
                        "Two-factor authentication was just turned off for your KyberNow account.",
                    ),
                )
            };
            (
                subject,
                Body {
                    heading,
                    hello,
                    blocks: vec![
                        Block::P(what),
                        Block::Button { label: tr("Voir la sécurité du compte", "Review account security"), url: account.to_string() },
                        Block::Note {
                            lead: not_you,
                            text: tr(
                                "Changez votre mot de passe et fermez les sessions que vous ne reconnaissez pas, depuis la même page.",
                                "Change your password and close the sessions you don't recognise, from the same page.",
                            ),
                        },
                    ],
                    reason: security,
                },
            )
        }
        Message::PasswordChanged { forgot } => (
            tr("Mot de passe KyberNow changé", "KyberNow password changed"),
            Body {
                heading: tr("Mot de passe changé", "Password changed"),
                hello,
                blocks: vec![
                    Block::P(tr(
                        "Le mot de passe de votre compte KyberNow vient de changer ; les autres sessions sont fermées.",
                        "The password of your KyberNow account was just changed; the other sessions are closed.",
                    )),
                    Block::Note { lead: not_you, text: tr("Choisissez-en un autre tout de suite.", "Choose another one right away.") },
                    Block::Button { label: tr("Choisir un autre mot de passe", "Choose another password"), url: forgot.to_string() },
                ],
                reason: security,
            },
        ),
        Message::AdminOpened { by, computer, date } => {
            let who = if by.is_empty() { tr("Quelqu'un", "Someone") } else { by.to_string() };
            (
                fe(format!("Admin de {computer} ouvert à distance"), format!("{computer}'s Admin opened remotely")),
                Body {
                    heading: tr("Admin ouvert à distance", "Admin opened remotely"),
                    hello,
                    blocks: vec![
                        Block::P(fe(
                            format!("{who} a ouvert Admin de {computer} le {date}, par KyberNow Share."),
                            format!("{who} opened {computer}'s Admin on {date}, through KyberNow Share."),
                        )),
                        Block::Detail { label: tr("Ordinateur", "Computer"), main: computer.to_string(), sub: Some(format!("{who} · {date}")) },
                        Block::Note {
                            lead: tr("Ce n'est pas normal ?", "Not expected?"),
                            text: tr(
                                "Coupez « Admin à distance » dans Admin, sur l'ordinateur (Hors de chez vous).",
                                "Turn off \u{201c}Remote Admin\u{201d} in Admin, on the computer (Away from home).",
                            ),
                        },
                    ],
                    reason: tr(
                        "Vous recevez cet e-mail parce que vous êtes propriétaire de cet ordinateur.",
                        "You're receiving this email because you own this computer.",
                    ),
                },
            )
        }
        Message::NewDevice { device, computer, person, account } => {
            let device = if device.is_empty() { tr("Un appareil", "A device") } else { device.to_string() };
            let whose = match person {
                None => tr("avec votre compte KyberNow", "with your KyberNow account"),
                Some(p) => fe(format!("avec le compte KyberNow de {p}"), format!("with {p}'s KyberNow account")),
            };
            let note = if person.is_none() {
                Block::Note {
                    lead: not_you,
                    text: tr(
                        "Déconnectez cet appareil et changez votre mot de passe ; dans Admin, « Compte KyberNow » permet aussi de le bloquer.",
                        "Disconnect this device and change your password; in Admin, \"KyberNow account\" can also block it.",
                    ),
                }
            } else {
                Block::Note {
                    lead: tr("Quelque chose ne va pas ?", "Something wrong?"),
                    text: tr(
                        "Retirez cette personne de l'ordinateur ; dans Admin, « Compte KyberNow » permet aussi de bloquer l'appareil.",
                        "Remove this person from the computer; in Admin, \"KyberNow account\" can also block the device.",
                    ),
                }
            };
            (
                fe(format!("Nouvel appareil sur {computer}"), format!("New device on {computer}")),
                Body {
                    heading: tr("Nouvel appareil", "New device"),
                    hello,
                    blocks: vec![
                        Block::P(fe(
                            format!("{device} vient d'ouvrir {computer} pour la première fois, {whose}."),
                            format!("{device} just opened {computer} for the first time, {whose}."),
                        )),
                        Block::Detail { label: tr("Appareil", "Device"), main: device.clone(), sub: Some(computer.to_string()) },
                        Block::Button { label: tr("Voir l'ordinateur", "View the computer"), url: account.to_string() },
                        note,
                    ],
                    reason: tr(
                        "Vous recevez cet e-mail parce que vous êtes propriétaire de cet ordinateur.",
                        "You're receiving this email because you own this computer.",
                    ),
                },
            )
        }
        Message::ComputerShared { by, computer, rights, until, has_account, link } => {
            let until = match until {
                None => String::new(),
                Some(d) => fe(format!(", jusqu'au {d}"), format!(", until {d}")),
            };
            let mut blocks = vec![
                Block::P(fe(format!("{by} vous ajoute à {computer}, avec ces droits :"), format!("{by} added you to {computer}, with these rights:"))),
                Block::Detail { label: tr("Ordinateur", "Computer"), main: computer.to_string(), sub: Some(format!("{rights}{until}")) },
            ];
            if has_account {
                blocks.push(Block::P(tr(
                    "L'ordinateur apparaît déjà dans vos apps KyberNow, connectées à votre compte.",
                    "The computer already shows up in your KyberNow apps, signed in to your account.",
                )));
                blocks.push(Block::Button { label: tr("Voir mes ordinateurs", "See my computers"), url: link.to_string() });
            } else {
                blocks.push(Block::P(tr(
                    "Créez votre compte KyberNow avec cette adresse : l'ordinateur y sera dès l'adresse vérifiée.",
                    "Create your KyberNow account with this address: the computer will be there once the address is verified.",
                )));
                blocks.push(Block::Button { label: tr("Créer mon compte", "Create my account"), url: link.to_string() });
                blocks.push(Block::Note {
                    lead: fe(format!("Vous ne connaissez pas {by} ?"), format!("You don't know {by}?")),
                    text: tr("Ignorez cet e-mail : sans compte, rien ne vous est ouvert.", "Ignore this email: without an account, nothing is opened to you."),
                });
            }
            (
                fe(format!("{by} vous ouvre {computer}"), format!("{by} opened {computer} for you")),
                Body {
                    heading: fe(format!("{by} vous ouvre un ordinateur"), format!("{by} opened a computer for you")),
                    hello,
                    blocks,
                    reason: fe(
                        format!("Vous recevez cet e-mail parce que {by} a saisi votre adresse dans KyberNow."),
                        format!("You're receiving this email because {by} entered your address in KyberNow."),
                    ),
                },
            )
        }
        Message::OrgInvite { by, org, role, has_account, link } => {
            let mut blocks = vec![
                Block::P(fe(format!("{by} vous ajoute à {org} sur KyberNow, comme {role}."), format!("{by} added you to {org} on KyberNow, as {role}."))),
                Block::Detail { label: tr("Invitation", "Invitation"), main: org.to_string(), sub: Some(role.to_string()) },
            ];
            if has_account {
                blocks.push(Block::P(tr("Ses ordinateurs ouverts à vous apparaissent dans vos apps.", "Its computers opened to you show up in your apps.")));
                blocks.push(Block::Button { label: fe(format!("Voir {org}"), format!("See {org}")), url: link.to_string() });
            } else {
                blocks.push(Block::P(tr(
                    "Créez votre compte KyberNow avec cette adresse : vous serez membre dès l'adresse vérifiée.",
                    "Create your KyberNow account with this address: you'll be a member once the address is verified.",
                )));
                blocks.push(Block::Button { label: tr("Créer mon compte", "Create my account"), url: link.to_string() });
                blocks.push(Block::Note {
                    lead: fe(format!("Vous ne connaissez pas {by} ?"), format!("You don't know {by}?")),
                    text: tr("Ignorez cet e-mail : sans compte, rien ne vous est ouvert.", "Ignore this email: without an account, nothing is opened to you."),
                });
            }
            (
                fe(format!("{by} vous invite dans {org}"), format!("{by} invited you to {org}")),
                Body {
                    heading: fe(format!("{by} vous invite dans {org}"), format!("{by} invited you to {org}")),
                    hello,
                    blocks,
                    reason: fe(
                        format!("Vous recevez cet e-mail parce que {by} a saisi votre adresse dans KyberNow."),
                        format!("You're receiving this email because {by} entered your address in KyberNow."),
                    ),
                },
            )
        }
        Message::PasskeyAdded { name, account } => (
            tr("Passkey ajoutée à votre compte KyberNow", "Passkey added to your KyberNow account"),
            Body {
                heading: tr("Nouvelle passkey", "New passkey"),
                hello,
                blocks: vec![
                    Block::P(tr(
                        "Une passkey vient d'être ajoutée à votre compte KyberNow. Elle ouvre le compte sans mot de passe.",
                        "A passkey was just added to your KyberNow account. It opens the account without a password.",
                    )),
                    Block::Detail { label: tr("Passkey", "Passkey"), main: name.to_string(), sub: None },
                    Block::Button { label: tr("Voir la sécurité du compte", "Review account security"), url: account.to_string() },
                    Block::Note {
                        lead: not_you,
                        text: tr(
                            "Retirez cette passkey, changez votre mot de passe et fermez les sessions que vous ne reconnaissez pas, depuis la même page.",
                            "Remove this passkey, change your password and close the sessions you don't recognise, from the same page.",
                        ),
                    },
                ],
                reason: security,
            },
        ),
        Message::AccountDeleted => (
            tr("Votre compte KyberNow est supprimé", "Your KyberNow account is deleted"),
            Body {
                heading: tr("Compte supprimé", "Account deleted"),
                hello,
                blocks: vec![
                    Block::P(tr(
                        "Votre compte KyberNow vient d'être supprimé, avec ses passkeys, ses sessions, ses appareils connectés et son journal ; ses ordinateurs sont détachés.",
                        "Your KyberNow account was just deleted, along with its passkeys, sessions, connected devices and log; its computers are detached.",
                    )),
                    Block::P(tr(
                        "Les sauvegardes du relais l'oublient sous 30 jours. Les logiciels KyberNow restent utilisables sans compte.",
                        "The relay's backups forget it within 30 days. The KyberNow software stays usable without an account.",
                    )),
                ],
                reason: tr(
                    "Vous recevez cet e-mail parce que votre compte KyberNow vient d'être supprimé.",
                    "You're receiving this email because your KyberNow account was just deleted.",
                ),
            },
        ),
    }
}

/// La version texte : la même chose, sans mise en forme ; le code et le détail en retrait.
fn text_of(lang: Lang, body: &Body) -> String {
    let colon = t(lang, " :", ":");
    let mut parts = vec![body.hello.clone()];
    for block in &body.blocks {
        parts.push(match block {
            Block::P(p) => p.clone(),
            Block::Detail { main, sub, .. } => match sub {
                Some(sub) => format!("    {main}\n    {sub}"),
                None => format!("    {main}"),
            },
            Block::Code(code) => format!("    {code}"),
            Block::Button { label, url } | Block::Link { label, url } => format!("{label}{colon}\n{url}"),
            Block::Note { lead, text } => format!("{lead} {text}"),
        });
    }
    parts.push("KyberNow".to_string());
    parts.join("\n\n") + "\n"
}

const FONT: &str = "-apple-system,BlinkMacSystemFont,'Segoe UI',Roboto,Helvetica,Arial,sans-serif";
const MONO: &str = "ui-monospace,Menlo,Consolas,monospace";
/// Les couleurs de la charte sur fond clair (brand/v2/tokens/kybernow.css).
const INK: &str = "#0B1117";
const INK_2: &str = "#2F3B45";
const MUTED: &str = "#5B6873";
const PAPER: &str = "#F2F5F6";
const LINE: &str = "#E1E7EA";
const SIGNAL: &str = "#5340E6";

/// La version HTML : des tableaux et des styles en ligne, pour Outlook et les webmails.
fn html_of(site: &str, lang: Lang, subject: &str, body: &Body) -> String {
    let mut inner = String::new();
    let mut after_button = false;
    let mut after_note = false;
    for block in &body.blocks {
        let note_before = std::mem::take(&mut after_note);
        match block {
            Block::P(p) => inner.push_str(&format!("<p style=\"margin:0 0 18px;font:400 16px/1.6 {FONT};color:{INK_2};\">{}</p>\n", esc(p))),
            Block::Detail { label, main, sub } => {
                let sub = sub.as_ref().map(|s| format!("<br><span style=\"font-weight:400;font-size:15px;color:{INK_2};\">{}</span>", esc(s))).unwrap_or_default();
                inner.push_str(&format!(
                    "<table role=\"presentation\" width=\"100%\" cellpadding=\"0\" cellspacing=\"0\" style=\"margin:0 0 24px;\"><tr><td style=\"background:{PAPER};border-radius:10px;padding:14px 16px;font:600 16px/1.5 {FONT};color:{INK};\">\
<span style=\"display:block;font:500 11px/1.4 {MONO};letter-spacing:.08em;text-transform:uppercase;color:{MUTED};margin-bottom:2px;\">{}</span>{}{sub}</td></tr></table>\n",
                    esc(label),
                    esc(main)
                ));
            }
            Block::Code(code) => inner.push_str(&format!(
                "<table role=\"presentation\" width=\"100%\" cellpadding=\"0\" cellspacing=\"0\" style=\"margin:0 0 20px;\"><tr><td align=\"center\" style=\"background:{PAPER};border-radius:12px;padding:20px 16px;font:600 34px/1 {MONO};letter-spacing:.18em;color:{INK};\">{}</td></tr></table>\n",
                esc(code)
            )),
            Block::Button { label, url } => {
                let top = if note_before { "20px" } else { "4px" };
                inner.push_str(&format!(
                    "<table role=\"presentation\" cellpadding=\"0\" cellspacing=\"0\" style=\"margin:{top} 0 0;\"><tr><td style=\"background:{SIGNAL};border-radius:10px;\">\
<a href=\"{}\" style=\"display:inline-block;padding:13px 22px;font:600 15px/1 {FONT};color:#FFFFFF;text-decoration:none;\">{}</a></td></tr></table>\n",
                    esc(url),
                    esc(label)
                ));
                after_button = true;
            }
            Block::Link { label, url } => inner.push_str(&format!(
                "<p style=\"margin:16px 0 0;font:400 15px/1.6 {FONT};\"><a href=\"{}\" style=\"color:{SIGNAL};\">{}</a></p>\n",
                esc(url),
                esc(label)
            )),
            Block::Note { lead, text } => {
                let top = if after_button { "24px" } else { "6px" };
                inner.push_str(&format!(
                    "<p style=\"margin:{top} 0 0;padding-top:20px;border-top:1px solid #E5E9EC;font:400 14px/1.6 {FONT};color:{MUTED};\"><b style=\"color:{INK};\">{}</b> {}</p>\n",
                    esc(lead),
                    esc(text)
                ));
                after_button = false;
                after_note = true;
                continue;
            }
        }
    }
    // Le premier paragraphe sert d'aperçu dans la liste des e-mails.
    let preview = body.blocks.iter().find_map(|b| if let Block::P(p) = b { Some(p.as_str()) } else { None }).unwrap_or("");
    let home = format!("{site}/?lang={}", lang.code());
    let privacy = format!("{site}/confidentialite?lang={}", lang.code());
    let host = site.trim_start_matches("https://").trim_start_matches("http://");
    format!(
        "<!doctype html>\n<html lang=\"{code}\">\n<head>\n<meta charset=\"utf-8\">\n<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n\
<meta name=\"color-scheme\" content=\"light\">\n<meta name=\"supported-color-schemes\" content=\"light\">\n<title>{title}</title>\n</head>\n\
<body style=\"margin:0;padding:0;background:{PAPER};\">\n\
<div style=\"display:none;max-height:0;overflow:hidden;opacity:0;\">{preview}</div>\n\
<table role=\"presentation\" width=\"100%\" cellpadding=\"0\" cellspacing=\"0\" style=\"background:{PAPER};\"><tr><td align=\"center\" style=\"padding:32px 16px;\">\n\
<table role=\"presentation\" width=\"100%\" cellpadding=\"0\" cellspacing=\"0\" style=\"max-width:520px;\">\n\
<tr><td style=\"padding:0 4px 20px;\"><a href=\"{home}\"><img src=\"{site}/assets/kybernow-share-mail.png\" width=\"164\" height=\"30\" alt=\"KyberNow Share\" style=\"display:block;border:0;font:700 18px/30px {FONT};color:{INK};\"></a></td></tr>\n\
<tr><td style=\"background:#FFFFFF;border-radius:14px;border:1px solid {LINE};padding:32px 32px 28px;\">\n\
<h1 style=\"margin:0 0 14px;font:700 24px/1.2 {FONT};letter-spacing:-.02em;color:{INK};\">{heading}</h1>\n{inner}</td></tr>\n\
<tr><td style=\"padding:20px 4px 0;font:400 12px/1.6 {FONT};color:{MUTED};\">{reason}<br>\
<a href=\"{home}\" style=\"color:{MUTED};\">{host}</a> · <a href=\"{privacy}\" style=\"color:{MUTED};\">{privacy_label}</a></td></tr>\n\
</table>\n</td></tr></table>\n</body>\n</html>\n",
        code = lang.code(),
        title = esc(subject),
        preview = esc(preview),
        home = esc(&home),
        privacy = esc(&privacy),
        host = esc(host),
        heading = esc(&body.heading),
        reason = esc(&body.reason),
        privacy_label = t(lang, "Confidentialité", "Privacy"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const SITE: &str = "https://share.test";

    fn all() -> Vec<Message<'static>> {
        vec![
            Message::Verify { name: "Ana", code: "123456", link: "https://share.test/compte/verifier?jeton=x&lang=fr" },
            Message::Verify { name: "", code: "123456", link: "https://share.test/v" },
            Message::Reset { link: "https://share.test/x" },
            Message::SignupTaken { sign_in: "https://share.test/s", forgot: "https://share.test/f" },
            Message::TotpChanged { on: true, account: "https://share.test/c" },
            Message::TotpChanged { on: false, account: "https://share.test/c" },
            Message::PasswordChanged { forgot: "https://share.test/f" },
            Message::NewDevice { device: "iPad de Léa", computer: "PC du salon", person: None, account: "https://share.test/c" },
            Message::NewDevice { device: "", computer: "PC du salon", person: Some("Léa"), account: "https://share.test/c" },
            Message::ComputerShared { by: "Ana", computer: "PC du salon", rights: "Session, Partage", until: Some("31/12/2026"), has_account: true, link: "https://share.test/c" },
            Message::ComputerShared { by: "Ana", computer: "PC <du> salon", rights: "Prêt", until: None, has_account: false, link: "https://share.test/c" },
            Message::OrgInvite { by: "Ana", org: "Les Lignes Claires", role: "Membre", has_account: false, link: "https://share.test/c" },
            Message::OrgInvite { by: "Ana", org: "Famille Martin", role: "Membre", has_account: true, link: "https://share.test/c" },
            Message::PasskeyAdded { name: "Chrome, Mac", account: "https://share.test/c" },
            Message::AccountDeleted,
            Message::AdminOpened { by: "Camille", computer: "PC du salon", date: "25/11/2026" },
        ]
    }

    #[test]
    fn e_mails_dans_les_deux_langues() {
        let fr = compose(SITE, "a@exemple.org", Lang::Fr, Message::Verify { name: "Ana", code: "123456", link: "https://share.test/compte/verifier?jeton=x" });
        assert_eq!(fr.subject, "Votre code KyberNow\u{202F}: 123 456");
        assert!(fr.text.starts_with("Bonjour Ana,"));
        assert!(fr.text.contains("    123 456\n"));
        assert!(fr.text.contains("https://share.test/compte/verifier?jeton=x"));
        assert!(!fr.text.contains(" :") && !fr.text.contains(" ?"), "typographie française");
        assert!(fr.html.contains(">123 456</td>") && fr.html.contains("href=\"https://share.test/compte/verifier?jeton=x\""));
        let en = compose(SITE, "a@exemple.org", Lang::En, Message::Reset { link: "https://share.test/x" });
        assert_eq!(en.subject, "New KyberNow password");
        assert!(en.text.contains("valid for one hour"));
        for m in all() {
            for lang in [Lang::Fr, Lang::En] {
                let mail = compose(SITE, "a@exemple.org", lang, m);
                assert!(!mail.subject.is_empty() && mail.text.ends_with("KyberNow\n"));
                assert!(!mail.text.contains('\u{2014}') && !mail.html.contains('\u{2014}'), "tiret cadratin");
                assert!(mail.html.contains("https://share.test/assets/kybernow-share-mail.png"), "logo");
                assert!(!mail.html.contains("<du>"), "noms échappés");
                if lang == Lang::Fr {
                    assert!(!mail.text.contains(" :") && !mail.text.contains(" ?") && !mail.html.contains(" :") && !mail.html.contains(" ?"), "typographie : {}", mail.subject);
                }
            }
        }
    }

    /// Écrit les e-mails dans `KN_PAGES_DIR` pour les regarder : `KN_PAGES_DIR=… cargo test ecrire_les_e_mails -- --ignored`.
    #[test]
    #[ignore]
    fn ecrire_les_e_mails() {
        let dir = std::path::PathBuf::from(std::env::var("KN_PAGES_DIR").expect("KN_PAGES_DIR"));
        let site = std::env::var("KN_MAIL_SITE").unwrap_or_else(|_| SITE.into());
        for (i, m) in all().into_iter().enumerate() {
            for lang in [Lang::Fr, Lang::En] {
                let mail = compose(&site, "a@exemple.org", lang, m);
                std::fs::write(dir.join(format!("mail-{i:02}-{}.html", lang.code())), &mail.html).unwrap();
                std::fs::write(dir.join(format!("mail-{i:02}-{}.txt", lang.code())), &mail.text).unwrap();
            }
        }
    }
}
