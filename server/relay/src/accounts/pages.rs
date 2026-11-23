//! Les pages des comptes (`/compte/…`), en français ou en anglais, sur le
//! modèle de la maquette (rangée « Inscription et connexion ») : le logo de
//! KyberNow Share en haut, une carte, des formulaires sans script.
//!
//! Lot 1b : les boutons « Se connecter avec une passkey » et « Ajouter une
//! passkey », et « Supprimer le compte », ne sont pas encore affichés (voir les
//! `TODO lot 1b` plus bas).

use super::password::Weak;
use crate::assets;
use crate::pages::{Lang, t};

/// Échappe un texte pour le HTML (adresses, noms, appareils).
pub fn esc(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(c),
        }
    }
    out
}

/// Ce que toutes les pages des comptes savent de la requête.
#[derive(Debug, Clone, Copy)]
pub struct View<'a> {
    pub lang: Lang,
    /// Le jeton CSRF des formulaires.
    pub csrf: &'a str,
    /// La page vient d'une app (`?appareil=…`, lot 3) : pas d'inscription.
    pub appareil: Option<&'a str>,
}

impl View<'_> {
    /// `?lang=fr[&appareil=…]`, pour les liens et les formulaires.
    pub fn q(&self) -> String {
        self.q_lang(self.lang)
    }

    fn q_lang(&self, lang: Lang) -> String {
        match self.appareil {
            Some(a) => format!("?lang={}&appareil={}", lang.code(), esc(a)),
            None => format!("?lang={}", lang.code()),
        }
    }

    fn tr(&self, fr: &str, en: &str) -> String {
        t(self.lang, fr, en)
    }

    fn csrf_field(&self) -> String {
        format!(r#"<input type="hidden" name="csrf" value="{}">"#, esc(self.csrf))
    }

    /// Un formulaire en POST vers `path`, avec la langue et le jeton CSRF.
    fn form(&self, path: &str, class: &str, inner: &str) -> String {
        let class = if class.is_empty() { String::new() } else { format!(r#" class="{class}""#) };
        format!("<form method=\"post\" action=\"{path}{q}\"{class}>\n{csrf}\n{inner}\n</form>", q = self.q(), csrf = self.csrf_field())
    }
}

/// Un avis au-dessus d'un formulaire.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Notice {
    BadCredentials,
    /// Trop d'essais : réessayer dans ce nombre de minutes.
    TooMany(u64),
    FormExpired,
    WrongCode,
    /// Le code reçu par e-mail ne vaut plus.
    CodeGone,
    Resent,
    BadEmail,
    NameMissing,
    Weak(Weak),
    PasswordChanged,
    Verified,
    BadPassword,
    /// Compte pas encore vérifié, et le relais n'envoie pas d'e-mails.
    UnverifiedNoMail,
    TotpOn,
    TotpOff,
    SessionClosed,
}

impl Notice {
    fn is_ok(self) -> bool {
        matches!(self, Notice::Resent | Notice::PasswordChanged | Notice::Verified | Notice::TotpOn | Notice::TotpOff | Notice::SessionClosed)
    }

    pub fn text(self, lang: Lang) -> String {
        let tr = |f: &str, e: &str| t(lang, f, e);
        match self {
            Notice::BadCredentials => tr("Adresse e-mail ou mot de passe incorrect.", "Incorrect email address or password."),
            Notice::TooMany(min) => {
                let min = min.max(1);
                match lang {
                    Lang::Fr => t(lang, &format!("Trop d'essais : réessayez dans {min} min."), ""),
                    Lang::En => format!("Too many attempts: try again in {min} min."),
                }
            }
            Notice::FormExpired => tr("Le formulaire a expiré : réessayez.", "The form expired: try again."),
            Notice::WrongCode => tr("Code incorrect ou expiré.", "Incorrect or expired code."),
            Notice::CodeGone => tr("Ce code ne vaut plus : demandez un nouvel e-mail.", "This code is no longer valid: ask for a new email."),
            Notice::Resent => tr(
                "Un nouvel e-mail vient de partir. Pensez à regarder dans les indésirables.",
                "A new email is on its way. Check your spam folder too.",
            ),
            Notice::BadEmail => tr("Cette adresse e-mail ne semble pas valide.", "This email address doesn't look valid."),
            Notice::NameMissing => tr("Entrez votre nom.", "Enter your name."),
            Notice::Weak(w) => weak_text(lang, w),
            Notice::PasswordChanged => tr("Mot de passe changé : connectez-vous avec le nouveau.", "Password changed: sign in with the new one."),
            Notice::Verified => tr("Adresse vérifiée : connectez-vous.", "Address verified: sign in."),
            Notice::BadPassword => tr("Mot de passe incorrect.", "Incorrect password."),
            Notice::UnverifiedNoMail => tr(
                "Cette adresse n'est pas vérifiée et ce relais n'envoie pas d'e-mails : demandez à son administrateur.",
                "This address isn't verified and this relay doesn't send email: ask its administrator.",
            ),
            Notice::TotpOn => tr("Double authentification activée.", "Two-factor authentication is on."),
            Notice::TotpOff => tr("Double authentification coupée.", "Two-factor authentication is off."),
            Notice::SessionClosed => tr("Session fermée.", "Session closed."),
        }
    }
}

/// Pourquoi un mot de passe est refusé.
pub fn weak_text(lang: Lang, weak: Weak) -> String {
    match weak {
        Weak::TooShort => t(lang, "Le mot de passe a 10 caractères au moins.", "The password needs at least 10 characters."),
        Weak::TooLong => t(lang, "Le mot de passe a 256 caractères au plus.", "The password can have at most 256 characters."),
        Weak::Known => t(lang, "Ce mot de passe est trop connu : choisissez-en un autre.", "This password is too common: choose another one."),
        Weak::LikeEmail => t(lang, "Le mot de passe ne peut pas être votre adresse e-mail.", "The password can't be your email address."),
    }
}

fn notice_html(lang: Lang, notice: Option<Notice>) -> String {
    match notice {
        None => String::new(),
        Some(n) if n.is_ok() => format!("<p class=\"notice ok\" role=\"status\">{}</p>\n", n.text(lang)),
        Some(n) => format!("<p class=\"notice\" role=\"alert\">{}</p>\n", n.text(lang)),
    }
}

/// Un champ : l'étiquette, puis le champ.
fn field(label: &str, input: &str) -> String {
    format!("<div class=\"field\">\n<label class=\"mono-label\"{for_}>{label}</label>\n{input}\n</div>", for_ = input_id(input))
}

fn input_id(input: &str) -> String {
    input.split("id=\"").nth(1).and_then(|r| r.split('"').next()).map(|id| format!(" for=\"{id}\"")).unwrap_or_default()
}

/// La page : le logo, la carte, la langue et la confidentialité.
fn page(v: &View<'_>, title: &str, here: &str, wide: bool, card: &str) -> String {
    let lang = v.lang;
    let (other, other_name) = match lang {
        Lang::Fr => (Lang::En, "English"),
        Lang::En => (Lang::Fr, "Français"),
    };
    let wide = if wide { " acct-wide" } else { "" };
    format!(
        r##"<!doctype html>
<html lang="{lang}">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<meta name="color-scheme" content="dark">
<meta name="theme-color" content="#05070A">
<meta name="robots" content="noindex">
<link rel="icon" type="image/svg+xml" href="{favicon}">
<link rel="preload" href="{font}" as="font" type="font/woff2" crossorigin>
<link rel="stylesheet" href="{css}">
<title>{title} · KyberNow</title>
</head>
<body class="solo">
<main class="solo-in acct{wide}">
<a class="acct-logo" href="/compte{q}" aria-label="{logo_label}"><img class="solo-logo" src="{logo}" alt="KyberNow Share" width="260" height="48"></a>
<div class="acct-card">
{card}
</div>
<nav class="solo-links"><a href="{here}{q_other}" hreflang="{other_code}" lang="{other_code}">{other_name}</a><a href="/privacy">{privacy}</a></nav>
</main>
</body>
</html>
"##,
        lang = lang.code(),
        favicon = assets::url("kybernow-favicon.svg"),
        font = assets::url("archivo.woff2"),
        css = assets::url("share.css"),
        logo = assets::url("kybernow-share-dark.svg"),
        q = v.q(),
        q_other = v.q_lang(other),
        other_code = other.code(),
        logo_label = v.tr("Compte KyberNow", "KyberNow account"),
        privacy = v.tr("Confidentialité", "Privacy"),
    )
}

fn email_input(id: &str, value: &str, autocomplete: &str) -> String {
    format!(
        r#"<input id="{id}" name="email" type="email" inputmode="email" autocomplete="{autocomplete}" autocapitalize="off" spellcheck="false" maxlength="254" value="{}" required>"#,
        esc(value)
    )
}

fn password_input(id: &str, autocomplete: &str) -> String {
    let min = if autocomplete == "new-password" { r#" minlength="10""# } else { "" };
    format!(r#"<input id="{id}" name="password" type="password" autocomplete="{autocomplete}" maxlength="256"{min} required>"#)
}

fn code_input(name: &str, autofocus: bool) -> String {
    let (mode, pattern, max, placeholder) = if name == "secours" {
        ("text", "[A-Za-z0-9 -]*", 12, "xxxx-xxxx")
    } else {
        ("numeric", "[0-9 ]*", 7, "123 456")
    };
    let af = if autofocus { " autofocus" } else { "" };
    format!(
        r#"<input id="{name}" class="code-input" name="{name}" inputmode="{mode}" autocomplete="one-time-code" autocapitalize="off" spellcheck="false" pattern="{pattern}" maxlength="{max}" placeholder="{placeholder}" required{af}>"#
    )
}

/// « Se connecter ».
pub fn sign_in(v: &View<'_>, email: &str, notice: Option<Notice>, can_sign_up: bool, can_forgot: bool) -> String {
    let title = v.tr("Se connecter", "Sign in");
    // TODO lot 1b : « Se connecter avec une passkey » d'abord (docs/comptes.md, « Pages »), puis « ou ».
    let forgot = if can_forgot {
        format!("<p class=\"field-link\"><a href=\"/compte/oubli{}\">{}</a></p>", v.q(), v.tr("Mot de passe oublié ?", "Forgot your password?"))
    } else {
        String::new()
    };
    let inner = format!(
        "{email}\n{password}\n{forgot}\n<button class=\"btn kn-primary\" type=\"submit\">{submit}</button>",
        email = field(&v.tr("Adresse e-mail", "Email address"), &email_input("email", email, "username")),
        password = field(&v.tr("Mot de passe", "Password"), &password_input("password", "current-password")),
        submit = title,
    );
    // Depuis une app (`?appareil=`), pas de « Créer un compte » : l'app ne crée pas de compte (App Store, 5.1.1(v)).
    let sign_up = if can_sign_up && v.appareil.is_none() {
        format!(
            "<p class=\"acct-links\">{} <a href=\"/compte/inscription{}\">{}</a></p>",
            v.tr("Pas encore de compte ?", "No account yet?"),
            v.q(),
            v.tr("Créer un compte", "Create an account")
        )
    } else {
        String::new()
    };
    let card = format!(
        "<h1>{title}</h1>\n<p class=\"help\">{help}</p>\n{notice}{form}\n{sign_up}",
        help = v.tr("Avec votre compte KyberNow.", "With your KyberNow account."),
        notice = notice_html(v.lang, notice),
        form = v.form("/compte/connexion", "", &inner),
    );
    page(v, &title, "/compte/connexion", false, &card)
}

/// Pourquoi l'inscription n'est pas ouverte ici.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SignUp {
    Open,
    /// La page vient d'une app.
    FromApp,
    /// Pas de SMTP : l'administrateur du relais crée les comptes.
    Closed,
}

/// « Créer un compte ».
pub fn sign_up(v: &View<'_>, state: SignUp, name: &str, email: &str, notice: Option<Notice>) -> String {
    let title = v.tr("Créer un compte", "Create an account");
    let sign_in_link = format!(
        "<p class=\"acct-links\">{} <a href=\"/compte/connexion{}\">{}</a></p>",
        v.tr("Déjà un compte ?", "Already have an account?"),
        v.q(),
        v.tr("Se connecter", "Sign in")
    );
    let body = match state {
        SignUp::FromApp => format!(
            "<p class=\"help\">{}</p>\n{sign_in_link}",
            v.tr(
                "Les comptes se créent sur le site du relais, dans un navigateur. Revenez ensuite ici pour vous connecter.",
                "Accounts are created on the relay's website, in a browser. Then come back here to sign in.",
            )
        ),
        SignUp::Closed => format!(
            "<p class=\"help\">{}</p>\n{sign_in_link}",
            v.tr(
                "Ce relais n'envoie pas d'e-mails : c'est son administrateur qui crée les comptes.",
                "This relay doesn't send email: its administrator creates the accounts.",
            )
        ),
        SignUp::Open => {
            let inner = format!(
                "{name}\n{email}\n{password}\n<p class=\"hint\">{hint}</p>\n<p class=\"hint\">{legal}</p>\n<button class=\"btn kn-primary\" type=\"submit\">{submit}</button>",
                name = field(
                    &v.tr("Nom", "Name"),
                    &format!(r#"<input id="name" name="name" autocomplete="name" maxlength="80" value="{}" required>"#, esc(name))
                ),
                email = field(&v.tr("Adresse e-mail", "Email address"), &email_input("email", email, "email")),
                password = field(&v.tr("Mot de passe", "Password"), &password_input("password", "new-password")),
                hint = v.tr(
                    "10 caractères au moins. Quelques mots qui n'ont rien à voir entre eux font un mot de passe sûr et facile à retenir.",
                    "At least 10 characters. A few unrelated words make a password that is safe and easy to remember.",
                ),
                legal = match v.lang {
                    Lang::Fr => t(v.lang, "En créant un compte, vous acceptez la <a href=\"/privacy\">politique de confidentialité</a> du relais.", ""),
                    Lang::En => "By creating an account, you accept the relay's <a href=\"/privacy\">privacy policy</a>.".to_string(),
                },
                submit = v.tr("Créer le compte", "Create the account"),
            );
            format!(
                "<p class=\"help\">{help}</p>\n{notice}{form}\n{sign_in_link}",
                help = v.tr(
                    "Un compte KyberNow ouvre vos ordinateurs depuis n'importe quel appareil.",
                    "A KyberNow account opens your computers from any device.",
                ),
                notice = notice_html(v.lang, notice),
                form = v.form("/compte/inscription", "", &inner),
            )
        }
    };
    page(v, &title, "/compte/inscription", false, &format!("<h1>{title}</h1>\n{body}"))
}

/// « Vérifiez votre e-mail » ; `change` : le champ pour changer d'adresse.
pub fn verify(v: &View<'_>, email: &str, notice: Option<Notice>, change: bool) -> String {
    let title = v.tr("Vérifiez votre e-mail", "Check your email");
    let body = if change {
        let inner = format!(
            "<input type=\"hidden\" name=\"action\" value=\"changer\">\n{field}\n<button class=\"btn kn-primary\" type=\"submit\">{submit}</button>",
            field = field(&v.tr("Nouvelle adresse e-mail", "New email address"), &email_input("email", "", "email")),
            submit = v.tr("Envoyer le code à cette adresse", "Send the code to this address"),
        );
        format!(
            "{notice}{form}\n<p class=\"acct-links\"><a href=\"/compte/verifier{q}\">{back}</a></p>",
            notice = notice_html(v.lang, notice),
            form = v.form("/compte/verifier", "", &inner),
            q = v.q(),
            back = v.tr("Garder cette adresse", "Keep this address"),
        )
    } else {
        let sent = match v.lang {
            Lang::Fr => t(v.lang, &format!("Un code à six chiffres et un lien viennent de partir à <b>{}</b>. Le lien vaut 24 heures.", esc(email)), ""),
            Lang::En => format!("A six-digit code and a link were just sent to <b>{}</b>. The link is valid for 24 hours.", esc(email)),
        };
        let code_form = v.form(
            "/compte/verifier",
            "",
            &format!(
                "<input type=\"hidden\" name=\"action\" value=\"code\">\n{field}\n<button class=\"btn kn-primary\" type=\"submit\">{submit}</button>",
                field = field(&v.tr("Code", "Code"), &code_input("code", notice.is_some_and(|n| !n.is_ok()))),
                submit = v.tr("Vérifier", "Verify"),
            ),
        );
        let resend = v.form(
            "/compte/verifier",
            "acct-inline",
            &format!(
                "<input type=\"hidden\" name=\"action\" value=\"renvoyer\">\n<button class=\"btn btn-ghost btn-sm\" type=\"submit\">{}</button>",
                v.tr("Renvoyer l'e-mail", "Resend the email")
            ),
        );
        format!(
            "<p class=\"help\">{sent}</p>\n{notice}{code_form}\n<div class=\"acct-actions\">\n{resend}\n<a class=\"btn btn-ghost btn-sm\" href=\"/compte/verifier{q}&amp;changer=1\">{change}</a>\n</div>",
            notice = notice_html(v.lang, notice),
            q = v.q(),
            change = v.tr("Le changer", "Change it"),
        )
    };
    page(v, &title, "/compte/verifier", false, &format!("<h1>{title}</h1>\n{body}"))
}

/// « Protégez votre compte », après l'activation.
pub fn protect(v: &View<'_>, totp_on: bool) -> String {
    let title = v.tr("Protégez votre compte", "Protect your account");
    // TODO lot 1b : la carte « Passkey (conseillée) », avant la double authentification.
    let action = if totp_on {
        format!("<span class=\"pill pill-on\">{}</span>", v.tr("Activée", "On"))
    } else {
        format!("<a class=\"btn kn-primary btn-sm\" href=\"/compte/double-authentification{}\">{}</a>", v.q(), v.tr("Activer", "Turn on"))
    };
    let card = format!(
        "<h1>{title}</h1>\n<p class=\"help\">{help}</p>\n<div class=\"option\">\n<div><h2>{totp}</h2>\n<p>{totp_text}</p></div>\n{action}\n</div>\n<p class=\"acct-links\"><a href=\"/compte{q}\">{later}</a></p>",
        help = v.tr(
            "Votre adresse est vérifiée. Ajoutez une deuxième protection à votre mot de passe.",
            "Your address is verified. Add a second protection to your password.",
        ),
        totp = v.tr("Double authentification", "Two-factor authentication"),
        totp_text = v.tr(
            "Un code à six chiffres de votre application d'authentification, en plus du mot de passe.",
            "A six-digit code from your authenticator app, on top of your password.",
        ),
        q = v.q(),
        later = v.tr("Plus tard", "Later"),
    );
    page(v, &title, "/compte/proteger", false, &card)
}

/// « Double authentification », à la connexion ; `recovery` : avec un code de secours.
pub fn code(v: &View<'_>, notice: Option<Notice>, recovery: bool) -> String {
    let title = v.tr("Double authentification", "Two-factor authentication");
    let (help, input, other) = if recovery {
        (
            v.tr("Entrez un de vos codes de secours. Chacun ne sert qu'une fois.", "Enter one of your recovery codes. Each one works only once."),
            field(&v.tr("Code de secours", "Recovery code"), &code_input("secours", true)),
            format!("<a href=\"/compte/code{}\">{}</a>", v.q(), v.tr("Utiliser l'application", "Use the app")),
        )
    } else {
        (
            v.tr("Entrez le code à six chiffres de votre application d'authentification.", "Enter the six-digit code from your authenticator app."),
            field(&v.tr("Code", "Code"), &code_input("code", true)),
            format!("<a href=\"/compte/code{}&amp;secours=1\">{}</a>", v.q(), v.tr("Utiliser un code de secours", "Use a recovery code")),
        )
    };
    let inner = format!(
        "{input}\n<label class=\"check\"><input type=\"checkbox\" name=\"confiance\" value=\"1\"> {trust}</label>\n<button class=\"btn kn-primary\" type=\"submit\">{submit}</button>",
        trust = v.tr("Ne plus demander sur cet appareil pendant 30 jours", "Don't ask again on this device for 30 days"),
        submit = v.tr("Continuer", "Continue"),
    );
    let card = format!(
        "<h1>{title}</h1>\n<p class=\"help\">{help}</p>\n{notice}{form}\n<p class=\"acct-links\">{other}</p>",
        notice = notice_html(v.lang, notice),
        form = v.form("/compte/code", "", &inner),
    );
    page(v, &title, "/compte/code", false, &card)
}

/// L'état de « Mot de passe oublié ».
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Forgot {
    Form(Option<Notice>),
    /// La réponse, toujours la même.
    Sent,
    /// Pas de SMTP.
    Unavailable,
}

/// « Mot de passe oublié ».
pub fn forgot(v: &View<'_>, state: Forgot) -> String {
    let title = v.tr("Mot de passe oublié", "Forgot your password");
    let back = format!("<p class=\"acct-links\"><a href=\"/compte/connexion{}\">{}</a></p>", v.q(), v.tr("Retour à la connexion", "Back to sign in"));
    let body = match state {
        Forgot::Unavailable => format!(
            "<p class=\"help\">{}</p>\n{back}",
            v.tr(
                "Ce relais n'envoie pas d'e-mails : demandez à son administrateur de changer votre mot de passe.",
                "This relay doesn't send email: ask its administrator to change your password.",
            )
        ),
        Forgot::Sent => format!(
            "<p class=\"notice ok\" role=\"status\">{sent}</p>\n<p class=\"help\">{more}</p>\n{back}",
            sent = v.tr("Si un compte existe à cette adresse, un lien vient d'y partir.", "If an account exists at this address, a link is on its way."),
            more = v.tr(
                "Il vaut une heure. Pensez à regarder dans les indésirables.",
                "It is valid for one hour. Check your spam folder too.",
            ),
        ),
        Forgot::Form(notice) => {
            let inner = format!(
                "{field}\n<button class=\"btn kn-primary\" type=\"submit\">{submit}</button>",
                field = field(&v.tr("Adresse e-mail", "Email address"), &email_input("email", "", "username")),
                submit = v.tr("Envoyer le lien", "Send the link"),
            );
            format!(
                "<p class=\"help\">{help}</p>\n{notice}{form}\n{back}",
                help = v.tr(
                    "Entrez l'adresse de votre compte : un lien pour choisir un nouveau mot de passe y partira.",
                    "Enter your account's address: a link to choose a new password will be sent there.",
                ),
                notice = notice_html(v.lang, notice),
                form = v.form("/compte/oubli", "", &inner),
            )
        }
    };
    page(v, &title, "/compte/oubli", false, &format!("<h1>{title}</h1>\n{body}"))
}

/// « Nouveau mot de passe », depuis le lien de l'e-mail.
pub fn new_password(v: &View<'_>, token: &str, notice: Option<Notice>) -> String {
    let title = v.tr("Nouveau mot de passe", "New password");
    let inner = format!(
        "<input type=\"hidden\" name=\"jeton\" value=\"{token}\">\n{field}\n<p class=\"hint\">{hint}</p>\n<button class=\"btn kn-primary\" type=\"submit\">{submit}</button>",
        token = esc(token),
        field = field(&v.tr("Nouveau mot de passe", "New password"), &password_input("password", "new-password")),
        hint = v.tr("10 caractères au moins.", "At least 10 characters."),
        submit = v.tr("Changer le mot de passe", "Change the password"),
    );
    let card = format!(
        "<h1>{title}</h1>\n<p class=\"help\">{help}</p>\n{notice}{form}",
        help = v.tr("Les autres sessions de votre compte seront fermées.", "The other sessions of your account will be closed."),
        notice = notice_html(v.lang, notice),
        form = v.form("/compte/nouveau-mot-de-passe", "", &inner),
    );
    page(v, &title, "/compte/nouveau-mot-de-passe", false, &card)
}

/// Une page de message : un titre, un texte, un lien.
pub fn message(v: &View<'_>, here: &str, title: &str, text: &str, link: (&str, &str)) -> String {
    let card = format!(
        "<h1>{title}</h1>\n<p class=\"help\">{text}</p>\n<p class=\"acct-links\"><a href=\"{href}{q}\">{label}</a></p>",
        href = link.0,
        q = v.q(),
        label = link.1,
    );
    page(v, title, here, false, &card)
}

/// Une session ouverte, sur la page du compte.
#[derive(Debug, Clone)]
pub struct SessionView {
    pub id: i64,
    pub device: String,
    pub created_at: i64,
    pub seen_at: i64,
    pub current: bool,
}

/// Ce que montre la page du compte.
#[derive(Debug, Clone)]
pub struct AccountView {
    pub email: String,
    pub name: String,
    pub totp_on: bool,
    pub recovery_left: usize,
    pub sessions: Vec<SessionView>,
}

/// Le jour d'une heure Unix (UTC) : `23/11/2026`, ou `Nov 23, 2026`.
pub fn date(lang: Lang, unix: i64) -> String {
    let days = unix.div_euclid(86_400);
    // Jours depuis 1970 vers le calendrier (H. Hinnant, « civil_from_days »).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    match lang {
        Lang::Fr => format!("{d:02}/{m:02}/{y}"),
        Lang::En => {
            const MONTHS: [&str; 12] = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
            format!("{} {d}, {y}", MONTHS[(m - 1) as usize])
        }
    }
}

/// « Votre compte » (en attendant la console du lot 4).
pub fn account(v: &View<'_>, a: &AccountView, notice: Option<Notice>) -> String {
    let title = v.tr("Votre compte", "Your account");
    let tr = |f: &str, e: &str| v.tr(f, e);
    let totp = if a.totp_on {
        let left = match v.lang {
            Lang::Fr => format!("{} code{s} de secours restant{s}", a.recovery_left, s = if a.recovery_left > 1 { "s" } else { "" }),
            Lang::En => format!("{} recovery code{s} left", a.recovery_left, s = if a.recovery_left == 1 { "" } else { "s" }),
        };
        // Un seul mot de passe pour les deux actions : chaque bouton envoie le formulaire à sa route.
        let inner = format!(
            "{field}\n<div class=\"acct-actions\">\n<button class=\"btn btn-ghost btn-sm\" type=\"submit\" formaction=\"/compte/codes-de-secours{q}\">{codes}</button>\n<button class=\"btn btn-ghost btn-sm\" type=\"submit\">{off}</button>\n</div>",
            field = field(&tr("Mot de passe, pour confirmer", "Password, to confirm"), &password_input("password", "current-password")),
            q = v.q(),
            codes = tr("Nouveaux codes de secours", "New recovery codes"),
            off = tr("Couper", "Turn off"),
        );
        format!(
            "<div class=\"row\"><div><b>{name}</b><small>{left}</small></div><span class=\"pill pill-on\">{on}</span></div>\n{form}",
            name = tr("Double authentification", "Two-factor authentication"),
            on = tr("Activée", "On"),
            form = v.form("/compte/double-authentification/couper", "acct-sub", &inner),
        )
    } else {
        format!(
            "<div class=\"row\"><div><b>{name}</b><small>{text}</small></div><a class=\"btn kn-primary btn-sm\" href=\"/compte/double-authentification{q}\">{on}</a></div>",
            name = tr("Double authentification", "Two-factor authentication"),
            text = tr("Coupée", "Off"),
            q = v.q(),
            on = tr("Activer", "Turn on"),
        )
    };
    // TODO lot 1b : les passkeys (liste, « Ajouter une passkey »), sous la double authentification.
    let sessions: String = a
        .sessions
        .iter()
        .map(|s| {
            let when = match v.lang {
                Lang::Fr => format!("Ouverte le {}, vue le {}", date(v.lang, s.created_at), date(v.lang, s.seen_at)),
                Lang::En => format!("Opened {}, last seen {}", date(v.lang, s.created_at), date(v.lang, s.seen_at)),
            };
            let act = if s.current {
                format!("<span class=\"pill\">{}</span>", tr("Cette session", "This session"))
            } else {
                v.form(
                    "/compte/sessions/fermer",
                    "acct-inline",
                    &format!("<input type=\"hidden\" name=\"id\" value=\"{}\">\n<button class=\"btn btn-ghost btn-sm\" type=\"submit\">{}</button>", s.id, tr("Fermer", "Close")),
                )
            };
            format!("<div class=\"row\"><div><b>{}</b><small>{when}</small></div>{act}</div>\n", esc(&s.device))
        })
        .collect();
    let sign_out = v.form(
        "/compte/deconnexion",
        "",
        &format!("<button class=\"btn btn-ghost\" type=\"submit\">{}</button>", tr("Se déconnecter", "Sign out")),
    );
    // TODO lot 1b : « Supprimer le compte » et « Exporter mes données ».
    let card = format!(
        "<h1>{title}</h1>\n{notice}<section>\n<h2>{email_h}</h2>\n<div class=\"row\"><div><b>{email}</b><small>{name}</small></div></div>\n</section>\n<section>\n<h2>{security}</h2>\n{totp}\n</section>\n<section>\n<h2>{sessions_h}</h2>\n{sessions}</section>\n{sign_out}",
        notice = notice_html(v.lang, notice),
        email_h = tr("Adresse e-mail", "Email address"),
        email = esc(&a.email),
        name = esc(&a.name),
        security = tr("Sécurité", "Security"),
        sessions_h = tr("Sessions ouvertes", "Open sessions"),
    );
    page(v, &title, "/compte", true, &card)
}

/// Activer la double authentification : le QR code, la clé, le code.
pub fn totp_setup(v: &View<'_>, qr_svg: &str, secret_grouped: &str, uri: &str, notice: Option<Notice>) -> String {
    let title = v.tr("Double authentification", "Two-factor authentication");
    let inner = format!(
        "{field}\n<button class=\"btn kn-primary\" type=\"submit\">{submit}</button>",
        field = field(&v.tr("Code affiché", "Code shown"), &code_input("code", notice.is_some())),
        submit = v.tr("Activer", "Turn on"),
    );
    let card = format!(
        "<h1>{title}</h1>\n<p class=\"help\">{scan}</p>\n<div class=\"qr\" role=\"img\" aria-label=\"{qr_label}\">{qr_svg}</div>\n<p class=\"hint\">{manual}</p>\n<p class=\"secret\">{secret}</p>\n<p class=\"field-link\"><a href=\"{uri}\">{open}</a></p>\n<p class=\"help\">{enter}</p>\n{notice}{form}\n<p class=\"acct-links\"><a href=\"/compte{q}\">{cancel}</a></p>",
        scan = v.tr(
            "Scannez ce QR code avec votre application d'authentification.",
            "Scan this QR code with your authenticator app.",
        ),
        qr_label = v.tr("QR code de la double authentification", "Two-factor authentication QR code"),
        manual = v.tr("Ou entrez cette clé à la main :", "Or enter this key by hand:"),
        secret = esc(secret_grouped),
        uri = esc(uri),
        open = v.tr("Ouvrir dans l'application de ce téléphone", "Open in this phone's app"),
        enter = v.tr("Puis entrez le code à six chiffres qu'elle affiche.", "Then enter the six-digit code it shows."),
        notice = notice_html(v.lang, notice),
        form = v.form("/compte/double-authentification", "", &inner),
        q = v.q(),
        cancel = v.tr("Annuler", "Cancel"),
    );
    page(v, &title, "/compte/double-authentification", false, &card)
}

/// Les codes de secours, montrés une seule fois.
pub fn recovery_codes(v: &View<'_>, codes: &[String], notice: Option<Notice>) -> String {
    let title = v.tr("Codes de secours", "Recovery codes");
    let list: String = codes.iter().map(|c| format!("<li>{}</li>", esc(c))).collect();
    let card = format!(
        "<h1>{title}</h1>\n{notice}<p class=\"help\">{help}</p>\n<ol class=\"codes\">{list}</ol>\n<p class=\"hint\">{once}</p>\n<a class=\"btn kn-primary\" href=\"/compte{q}\">{done}</a>",
        notice = notice_html(v.lang, notice),
        help = v.tr(
            "Gardez ces codes en lieu sûr (gestionnaire de mots de passe, papier) : si vous perdez votre téléphone, chacun ouvre votre compte une fois.",
            "Keep these codes somewhere safe (password manager, paper): if you lose your phone, each one opens your account once.",
        ),
        once = v.tr("Ils ne s'afficheront plus.", "They won't be shown again."),
        q = v.q(),
        done = v.tr("J'ai gardé mes codes", "I've saved my codes"),
    );
    page(v, &title, "/compte", false, &card)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text_of(html: &str) -> String {
        let mut out = String::new();
        let mut in_tag = false;
        for c in html.chars() {
            match c {
                '<' => in_tag = true,
                '>' => {
                    in_tag = false;
                    out.push(' ');
                }
                _ if !in_tag => out.push(c),
                _ => {}
            }
        }
        out
    }

    fn all(lang: Lang, appareil: Option<&str>) -> Vec<(&'static str, String)> {
        let v = View { lang, csrf: "jeton-csrf", appareil };
        let account_view = AccountView {
            email: "ana@exemple.org".into(),
            name: "Ana <b>".into(),
            totp_on: true,
            recovery_left: 1,
            sessions: vec![
                SessionView { id: 1, device: "Firefox, Windows".into(), created_at: 1_700_000_000, seen_at: 1_700_000_000, current: true },
                SessionView { id: 2, device: "Safari, iPhone".into(), created_at: 1_700_000_000, seen_at: 1_700_100_000, current: false },
            ],
        };
        let mut pages = vec![
            ("connexion", sign_in(&v, "ana@exemple.org", Some(Notice::BadCredentials), true, true)),
            ("connexion/limite", sign_in(&v, "", Some(Notice::TooMany(15)), true, false)),
            ("inscription", sign_up(&v, SignUp::Open, "Ana", "", Some(Notice::Weak(Weak::Known)))),
            ("inscription/app", sign_up(&v, SignUp::FromApp, "", "", None)),
            ("inscription/fermee", sign_up(&v, SignUp::Closed, "", "", None)),
            ("verifier", verify(&v, "ana@exemple.org", Some(Notice::Resent), false)),
            ("verifier/changer", verify(&v, "ana@exemple.org", None, true)),
            ("proteger", protect(&v, false)),
            ("code", code(&v, Some(Notice::WrongCode), false)),
            ("code/secours", code(&v, None, true)),
            ("oubli", forgot(&v, Forgot::Form(None))),
            ("oubli/envoye", forgot(&v, Forgot::Sent)),
            ("oubli/indisponible", forgot(&v, Forgot::Unavailable)),
            ("nouveau", new_password(&v, "jeton", Some(Notice::Weak(Weak::TooShort)))),
            ("compte", account(&v, &account_view, Some(Notice::TotpOn))),
            ("compte/sans-2fa", account(&v, &AccountView { totp_on: false, ..account_view.clone() }, None)),
            ("2fa", totp_setup(&v, "<svg></svg>", "GEZD GNBV", "otpauth://totp/x", Some(Notice::WrongCode))),
            ("secours", recovery_codes(&v, &["abcd-efgh".into()], None)),
            ("message", message(&v, "/compte/verifier", "Titre", "Texte.", ("/compte/connexion", "Lien"))),
        ];
        for n in [
            Notice::FormExpired,
            Notice::CodeGone,
            Notice::BadEmail,
            Notice::NameMissing,
            Notice::PasswordChanged,
            Notice::Verified,
            Notice::BadPassword,
            Notice::UnverifiedNoMail,
            Notice::TotpOff,
            Notice::SessionClosed,
            Notice::Weak(Weak::TooLong),
            Notice::Weak(Weak::LikeEmail),
        ] {
            pages.push(("avis", sign_in(&v, "", Some(n), true, true)));
        }
        pages
    }

    #[test]
    fn typographie_francaise_et_pas_de_tiret_cadratin() {
        for (name, page) in all(Lang::Fr, None) {
            let text = text_of(&page);
            for bad in [" :", " ;", " !", " ?", "« ", " »"] {
                assert!(!text.contains(bad), "{name} : « {bad} » dans le texte");
            }
        }
        for lang in [Lang::Fr, Lang::En] {
            for (name, page) in all(lang, None) {
                assert!(!page.contains('\u{2014}'), "{name} : tiret cadratin");
            }
        }
        assert!(sign_in(&View { lang: Lang::Fr, csrf: "", appareil: None }, "", None, true, true).contains("Mot de passe oublié\u{202F}?"));
    }

    #[test]
    fn rien_qui_charge_du_dehors_et_pas_de_script() {
        for lang in [Lang::Fr, Lang::En] {
            for (name, page) in all(lang, None) {
                for bad in ["<script", "<iframe", "style=", "onclick=", "onload=", "analytics"] {
                    assert!(!page.contains(bad), "{name} : {bad}");
                }
                for part in page.split("src=\"").skip(1).chain(page.split("<link ").skip(1).map(|l| l.split("href=\"").nth(1).unwrap_or(""))) {
                    assert!(part.starts_with('/'), "{name} : ressource hors du relais");
                }
                assert!(!page.contains("https://"), "{name} : adresse absolue");
                // Chaque formulaire est en POST, avec son jeton CSRF et la langue.
                for form in page.split("<form ").skip(1) {
                    assert!(form.starts_with("method=\"post\""), "{name} : formulaire en GET");
                    assert!(form.contains(&format!("?lang={}", lang.code())), "{name} : langue du formulaire");
                    assert!(form.contains(r#"name="csrf" value="jeton-csrf""#), "{name} : jeton CSRF");
                }
                assert!(page.contains(&format!("<html lang=\"{}\">", lang.code())));
            }
        }
    }

    #[test]
    fn textes_echappes() {
        let page = all(Lang::Fr, None).into_iter().find(|(n, _)| *n == "compte").unwrap().1;
        assert!(page.contains("Ana &lt;b&gt;") && !page.contains("Ana <b>"));
        assert!(page.contains("1 code de secours restant<"));
        assert!(page.contains("Cette session") && page.contains(r#"name="id" value="2""#));
        assert!(!page.contains(r#"name="id" value="1""#), "pas de « Fermer » pour la session en cours");
        assert_eq!(esc(r#"<a href="x">'&"#), "&lt;a href=&quot;x&quot;&gt;&#39;&amp;");
    }

    #[test]
    fn depuis_une_app_pas_d_inscription() {
        let v = View { lang: Lang::Fr, csrf: "c", appareil: Some("abc") };
        let page = sign_in(&v, "", None, true, true);
        assert!(!page.contains("Créer un compte"));
        assert!(page.contains("action=\"/compte/connexion?lang=fr&appareil=abc\""));
        assert!(sign_in(&View { appareil: None, ..v }, "", None, true, true).contains("Créer un compte"));
        assert!(!sign_in(&View { appareil: None, ..v }, "", None, false, true).contains("Créer un compte"), "sans SMTP, pas d'inscription");
        let refused = sign_up(&v, SignUp::FromApp, "", "", None);
        assert!(refused.contains("dans un navigateur") && !refused.contains("<form"));
    }

    #[test]
    fn passkeys_pas_encore_la() {
        // Lot 1b : aucune mention des passkeys tant qu'elles ne marchent pas.
        for lang in [Lang::Fr, Lang::En] {
            for (name, page) in all(lang, None) {
                assert!(!page.to_lowercase().contains("passkey"), "{name} : passkey");
                assert!(!page.contains("Supprimer le compte") && !page.contains("Delete the account"), "{name}");
            }
        }
    }

    #[test]
    fn dates() {
        assert_eq!(date(Lang::Fr, 0), "01/01/1970");
        assert_eq!(date(Lang::Fr, 1_795_392_000), "23/11/2026");
        assert_eq!(date(Lang::En, 1_795_392_000), "Nov 23, 2026");
        assert_eq!(date(Lang::Fr, 951_782_400), "29/02/2000");
    }
}
