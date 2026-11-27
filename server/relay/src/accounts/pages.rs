//! Les pages des comptes (`/compte/…`), en français ou en anglais, sur le
//! modèle de la maquette (rangée « Inscription et connexion ») : le logo de
//! KyberNow Share en haut, une carte, des formulaires sans script.
//!
//! Lot 1b : « Continuer avec une passkey », « Ajouter une passkey », la liste
//! des passkeys, l'export des données et « Supprimer le compte ». Les
//! formulaires des passkeys (`data-passkey`) sont cachés tant que
//! `/assets/passkey.js`, le seul script des pages, ne les montre pas : sans
//! script, ou dans un navigateur sans WebAuthn, tout passe par le mot de passe.
//!
//! Lots 4 et 5 : la console (`console_pages.rs`), la barre de la console sur
//! la page du compte, le choix du propriétaire sur « Rattacher ».
//!
//! Lot 2 et moitié relais du lot 3 : « Rattacher {nom} », les ordinateurs
//! rattachés et les appareils connectés sur la page du compte, « Connecter
//! KyberNow Link sur {appareil} ? » et la page `/tv`.

use super::password::Weak;

/// La console des lots 4 et 5 : « Ordinateurs », « Personnes », « Organisation », « Déployer des Stations ».
#[path = "console_pages.rs"]
pub mod console;
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

/// Le texte d'un `<title>` (REL-01) : les appelants passent tantôt du texte brut (un nom d'ordinateur
/// ou d'équipe), tantôt un fragment déjà échappé (le même que le `<h1>`). On retire les balises, on
/// décode les cinq entités que produit [`esc`], puis on échappe tout : rien ne sort du `<title>`,
/// quelle que soit l'entrée.
pub fn title_text(s: &str) -> String {
    let mut plain = String::with_capacity(s.len());
    let mut in_tag = false;
    for c in s.chars() {
        match c {
            '<' if !in_tag => in_tag = true,
            '>' if in_tag => in_tag = false,
            _ if in_tag => {}
            _ => plain.push(c),
        }
    }
    let plain = plain.replace("&lt;", "<").replace("&gt;", ">").replace("&quot;", "\"").replace("&#39;", "'").replace("&amp;", "&");
    esc(&plain)
}

/// Encode une valeur pour une adresse (`?appareil=iPad%20de%20L%C3%A9a`).
pub fn pct(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~') {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
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
    /// Le défi WebAuthn de la page (connexion, confirmation par passkey) ; vide : pas de passkey.
    pub challenge: &'a str,
}

impl View<'_> {
    /// `?lang=fr[&appareil=…]`, pour les liens et les formulaires.
    pub fn q(&self) -> String {
        self.q_lang(self.lang)
    }

    fn q_lang(&self, lang: Lang) -> String {
        match self.appareil {
            Some(a) => format!("?lang={}&appareil={}", lang.code(), esc(&pct(a))),
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
#[derive(Debug, Clone, PartialEq, Eq)]
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
    /// Lot 2 : un ordinateur détaché depuis la console.
    StationDetached,
    /// Lot 3 : un appareil déconnecté.
    DeviceClosed,
    /// Lot 1b : la case « conditions et confidentialité » n'est pas cochée.
    TermsMissing,
    PasskeyFailed,
    PasskeyAdded,
    PasskeyRenamed,
    PasskeyRemoved,
    /// Lot 4 : la console (personnes et droits).
    PersonAdded,
    InviteSent,
    AccessChanged,
    AccessRemoved,
    NoRights,
    BadDate,
    /// Le droit Administrer, ou gérer une organisation, demande la double authentification.
    NeedTwoFactor,
    NotAllowed,
    /// Lot 5 : les organisations.
    OrgCreated,
    OrgRenamed,
    OrgNameMissing,
    MemberAdded,
    RoleChanged,
    MemberRemoved,
    InviteCancelled,
    LastOwner,
    OrgLeft,
    GroupCreated,
    GroupTaken,
    GroupDeleted,
    GroupChanged,
    TokenRevoked,
    BadNumber,
    /// Liens d'accès (docs/comptes.md, « Regarder, liens d'accès et groupes d'ordinateurs »).
    LinkRevoked,
    TooManyLinks,
    LinkAccepted,
    /// Invitations, ajout de membres (docs/comptes.md, « Invitations, ajout de membres et sessions ») :
    /// « {nom} avait déjà accès : ses droits sont mis à jour. »
    AccessUpdated(String),
    /// « {nom} en fait déjà partie. »
    AlreadyMember(String),
    /// « Invitation renvoyée à {adresse}. »
    InviteResent(String),
    /// « Invitation déjà renvoyée il y a moins d'une heure. »
    ResentTooSoon,
    /// Déplacer un ordinateur (docs/comptes.md, « Réglages synchronisés et déplacer un ordinateur ») :
    /// « {ordinateur} est maintenant à {destination}. »
    Moved(String, String),
    /// « Vous ne gérez pas {destination}. »
    NotManager(String),
}

impl Notice {
    fn is_ok(&self) -> bool {
        matches!(
            self,
            Notice::Resent
                | Notice::PasswordChanged
                | Notice::Verified
                | Notice::TotpOn
                | Notice::TotpOff
                | Notice::SessionClosed
                | Notice::StationDetached
                | Notice::DeviceClosed
                | Notice::PasskeyAdded
                | Notice::PasskeyRenamed
                | Notice::PasskeyRemoved
                | Notice::PersonAdded
                | Notice::InviteSent
                | Notice::AccessChanged
                | Notice::AccessRemoved
                | Notice::OrgCreated
                | Notice::OrgRenamed
                | Notice::MemberAdded
                | Notice::RoleChanged
                | Notice::MemberRemoved
                | Notice::InviteCancelled
                | Notice::OrgLeft
                | Notice::GroupCreated
                | Notice::GroupDeleted
                | Notice::GroupChanged
                | Notice::TokenRevoked
                | Notice::LinkRevoked
                | Notice::LinkAccepted
                | Notice::AccessUpdated(_)
                | Notice::InviteResent(_)
                | Notice::Moved(..)
        )
    }

    /// Le texte de l'avis, en HTML (un nom ou une adresse y est échappé).
    pub fn text(&self, lang: Lang) -> String {
        let tr = |f: &str, e: &str| t(lang, f, e);
        match self {
            Notice::BadCredentials => tr("Adresse e-mail ou mot de passe incorrect.", "Incorrect email address or password."),
            Notice::TooMany(min) => {
                let min = (*min).max(1);
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
            Notice::Weak(w) => weak_text(lang, *w),
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
            Notice::StationDetached => tr("Ordinateur détaché.", "Computer detached."),
            Notice::DeviceClosed => tr("Appareil déconnecté.", "Device disconnected."),
            Notice::TermsMissing => tr(
                "Cochez la case pour accepter les conditions d'utilisation et la politique de confidentialité.",
                "Tick the box to accept the terms of use and the privacy policy.",
            ),
            Notice::PasskeyFailed => tr(
                "La passkey n'a pas été acceptée : réessayez, ou utilisez votre mot de passe.",
                "The passkey wasn't accepted: try again, or use your password.",
            ),
            Notice::PasskeyAdded => tr("Passkey ajoutée.", "Passkey added."),
            Notice::PasskeyRenamed => tr("Passkey renommée.", "Passkey renamed."),
            Notice::PasskeyRemoved => tr("Passkey retirée.", "Passkey removed."),
            Notice::PersonAdded => tr(
                "Personne ajoutée : l'ordinateur apparaît dans ses apps, et un e-mail le lui dit.",
                "Person added: the computer shows up in their apps, and an email tells them.",
            ),
            Notice::InviteSent => tr(
                "Invitation envoyée : cette adresse n'a pas encore de compte ; l'ordinateur y sera dès son compte créé.",
                "Invitation sent: this address has no account yet; the computer will be there as soon as the account is created.",
            ),
            Notice::AccessChanged => tr("Accès enregistré.", "Access saved."),
            Notice::AccessRemoved => tr("Accès retiré.", "Access removed."),
            Notice::NoRights => tr("Cochez au moins un droit.", "Tick at least one right."),
            Notice::BadDate => tr("Cette date ne va pas : une date à venir, au format jj/mm/aaaa.", "This date doesn't work: a date to come, as dd/mm/yyyy."),
            Notice::NeedTwoFactor => tr(
                "Activez d'abord la double authentification (ou ajoutez une passkey) sur votre compte.",
                "First turn on two-factor authentication (or add a passkey) on your account.",
            ),
            Notice::NotAllowed => tr("Vous ne pouvez pas faire cela ici.", "You can't do that here."),
            Notice::OrgCreated => tr("C'est créé : vous en êtes propriétaire. Invitez maintenant ses membres.", "Created: you are its owner. Now invite its members."),
            Notice::OrgRenamed => tr("Nom changé.", "Name changed."),
            Notice::OrgNameMissing => tr("Entrez un nom.", "Enter a name."),
            Notice::MemberAdded => tr("Membre ajouté, et prévenu par e-mail.", "Member added, and told by email."),
            Notice::RoleChanged => tr("Rôle changé.", "Role changed."),
            Notice::MemberRemoved => tr("Membre retiré.", "Member removed."),
            Notice::InviteCancelled => tr("Invitation annulée.", "Invitation cancelled."),
            Notice::LastOwner => tr(
                "Il faut toujours au moins un propriétaire : nommez-en un autre d'abord.",
                "There must always be at least one owner: name another one first.",
            ),
            Notice::OrgLeft => tr("Vous êtes parti. Ses ordinateurs ne vous sont plus ouverts.", "You left. Its computers are no longer open to you."),
            Notice::GroupCreated => tr("Groupe créé.", "Group created."),
            Notice::GroupTaken => tr("Un groupe porte déjà ce nom.", "A group already has this name."),
            Notice::GroupDeleted => tr("Groupe supprimé.", "Group deleted."),
            Notice::GroupChanged => tr("Groupe changé.", "Group changed."),
            Notice::TokenRevoked => tr(
                "Jeton révoqué : plus aucune Station ne s'enrôle avec lui ; celles déjà rattachées restent.",
                "Token revoked: no more Stations enroll with it; those already attached stay.",
            ),
            Notice::BadNumber => tr("Ce nombre ne va pas.", "This number doesn't work."),
            Notice::LinkRevoked => tr("Lien révoqué : il ne servira plus.", "Link revoked: it won't work anymore."),
            Notice::TooManyLinks => tr("Révoquez un lien avant d'en créer un autre.", "Revoke a link before creating another one."),
            Notice::LinkAccepted => tr("C'est fait : l'ordinateur est dans vos ordinateurs.", "Done: the computer is now in your computers."),
            Notice::AccessUpdated(name) => match lang {
                Lang::Fr => t(lang, &format!("{} avait déjà accès : ses droits sont mis à jour.", esc(name)), ""),
                Lang::En => format!("{} already had access: their rights are updated.", esc(name)),
            },
            Notice::AlreadyMember(name) => match lang {
                Lang::Fr => t(lang, &format!("{} en fait déjà partie.", esc(name)), ""),
                Lang::En => format!("{} is already a member.", esc(name)),
            },
            Notice::InviteResent(email) => match lang {
                Lang::Fr => t(lang, &format!("Invitation renvoyée à {}.", esc(email)), ""),
                Lang::En => format!("Invitation resent to {}.", esc(email)),
            },
            Notice::ResentTooSoon => tr("Invitation déjà renvoyée il y a moins d'une heure.", "Invitation already resent less than an hour ago."),
            Notice::Moved(computer, dest) => match lang {
                Lang::Fr => t(lang, &format!("{} est maintenant à {}.", esc(computer), esc(dest)), ""),
                Lang::En => format!("{} now belongs to {}.", esc(computer), esc(dest)),
            },
            Notice::NotManager(dest) => match lang {
                Lang::Fr => t(lang, &format!("Vous ne gérez pas {}.", esc(dest)), ""),
                Lang::En => format!("You don't manage {}.", esc(dest)),
            },
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
    page_with(v, title, here, if wide { " acct-wide" } else { "" }, card)
}

/// La page, avec la classe de largeur (`acct-wide`, `acct-console`).
fn page_with(v: &View<'_>, title: &str, here: &str, wide: &str, card: &str) -> String {
    let lang = v.lang;
    let (other, other_name) = match lang {
        Lang::Fr => (Lang::En, "English"),
        Lang::En => (Lang::Fr, "Français"),
    };
    // Le seul script des pages, et seulement là où il y a une passkey (lot 1b).
    let script = if card.contains("data-passkey=\"") { format!("\n<script src=\"{}\" defer></script>", assets::url("passkey.js")) } else { String::new() };
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
<title>{title} · KyberNow</title>{script}
</head>
<body class="solo">
<main class="solo-in acct{wide}">
<a class="acct-logo" href="/compte{q}" aria-label="{logo_label}"><img class="solo-logo" src="{logo}" alt="KyberNow Share" width="260" height="48"></a>
<div class="acct-card">
{card}
</div>
<nav class="solo-links"><a href="{here}{q_other}" hreflang="{other_code}" lang="{other_code}">{other_name}</a><a href="/conditions{q_lang}">{terms}</a><a href="/confidentialite{q_lang}">{privacy}</a></nav>
</main>
</body>
</html>
"##,
        lang = lang.code(),
        title = title_text(title),
        here = esc(here),
        favicon = assets::url("kybernow-favicon.svg"),
        font = assets::url("archivo.woff2"),
        css = assets::url("share.css"),
        logo = assets::url("kybernow-share-dark.svg"),
        q = v.q(),
        q_other = v.q_lang(other),
        other_code = other.code(),
        logo_label = v.tr("Compte KyberNow", "KyberNow account"),
        privacy = v.tr("Confidentialité", "Privacy"),
        terms = v.tr("Conditions", "Terms"),
        q_lang = format!("?lang={}", lang.code()),
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
    // Lot 1b : « Continuer avec une passkey » d'abord (docs/comptes.md, « Pages »), puis « ou ».
    let passkey = if v.challenge.is_empty() {
        String::new()
    } else {
        format!(
            "{form}\n<p class=\"acct-or\" data-passkey-or hidden>{or}</p>",
            form = passkey_get_form(v, "/compte/connexion/passkey", &[], &v.tr("Continuer avec une passkey", "Continue with a passkey")),
            or = v.tr("ou", "or"),
        )
    };
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
        "<h1>{title}</h1>\n<p class=\"help\">{help}</p>\n{notice}{passkey}\n{form}\n{sign_up}",
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
                "{name}\n{email}\n{password}\n<p class=\"hint\">{hint}</p>\n<label class=\"check\"><input type=\"checkbox\" name=\"conditions\" value=\"1\" required> <span>{legal}</span></label>\n<button class=\"btn kn-primary\" type=\"submit\">{submit}</button>",
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
                // Lot 1b : les textes servis par le relais (crate::legal), version notée à l'inscription.
                legal = match v.lang {
                    Lang::Fr => format!(
                        "J'accepte les <a href=\"/conditions?lang=fr\">conditions d'utilisation</a> et la <a href=\"/confidentialite?lang=fr\">politique de confidentialité</a> (version {}).",
                        crate::legal::Doc::Terms.version()
                    ),
                    Lang::En => format!(
                        "I accept the <a href=\"/conditions?lang=en\">terms of use</a> and the <a href=\"/confidentialite?lang=en\">privacy policy</a> (version {}).",
                        crate::legal::Doc::Terms.version()
                    ),
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
                field = field(&v.tr("Code", "Code"), &code_input("code", notice.as_ref().is_some_and(|n| !n.is_ok()))),
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
pub fn protect(v: &View<'_>, totp_on: bool, register: Option<&Registration>) -> String {
    let title = v.tr("Protégez votre compte", "Protect your account");
    // Lot 1b : la carte « Passkey (conseillée) », avant la double authentification.
    let passkey_action = match register {
        Some(r) if r.exclude.is_empty() => passkey_create_form(v, r, "btn kn-primary btn-sm"),
        _ => format!("<span class=\"pill pill-on\">{}</span>", v.tr("Ajoutée", "Added")),
    };
    let passkey = format!(
        "<div class=\"option\">\n<div><h2>{name} <small>{advised}</small></h2>\n<p>{text}</p>\n{none}</div>\n{passkey_action}\n</div>\n",
        name = v.tr("Passkey", "Passkey"),
        advised = v.tr("(conseillée)", "(recommended)"),
        text = v.tr(
            "Empreinte, visage ou code de l'appareil : elle remplace le mot de passe et vaut double authentification.",
            "Fingerprint, face or device PIN: it replaces the password and counts as two-factor authentication.",
        ),
        none = passkey_unsupported(v),
    );
    let action = if totp_on {
        format!("<span class=\"pill pill-on\">{}</span>", v.tr("Activée", "On"))
    } else {
        format!("<a class=\"btn kn-primary btn-sm\" href=\"/compte/double-authentification{}\">{}</a>", v.q(), v.tr("Activer", "Turn on"))
    };
    let card = format!(
        "<h1>{title}</h1>\n<p class=\"help\">{help}</p>\n{passkey}<div class=\"option\">\n<div><h2>{totp}</h2>\n<p>{totp_text}</p></div>\n{action}\n</div>\n<p class=\"acct-links\"><a href=\"/compte{q}\">{later}</a></p>",
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
#[derive(Debug, Clone, PartialEq, Eq)]
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
        href = esc(link.0),
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
    /// Lot 2 : les ordinateurs rattachés au compte.
    pub computers: Vec<ComputerView>,
    /// Lot 3 : les appareils connectés (apps, téléviseurs).
    pub devices: Vec<DeviceView>,
    /// Lot 1b : les passkeys du compte, et de quoi en ajouter une (`None` : le maximum est atteint).
    pub passkeys: Vec<PasskeyView>,
    pub register: Option<Registration>,
    /// Lot 5 : ses organisations `(identifiant public, nom)`, pour la barre de la console.
    pub orgs: Vec<(String, String)>,
    /// L'heure de la page (« Utilisé il y a {durée} »).
    pub now: i64,
}

/// Un ordinateur rattaché, sur la page du compte.
#[derive(Debug, Clone)]
pub struct ComputerView {
    pub id: String,
    pub name: String,
    pub attached_at: i64,
    pub online: bool,
}

/// Un appareil connecté, sur la page du compte.
#[derive(Debug, Clone)]
pub struct DeviceView {
    pub id: i64,
    pub name: String,
    /// Connecté par un code (téléviseur).
    pub tv: bool,
    pub created_at: i64,
    pub seen_at: i64,
}

/// Une passkey, sur la page du compte.
#[derive(Debug, Clone)]
pub struct PasskeyView {
    pub id: i64,
    pub name: String,
    pub created_at: i64,
    pub seen_at: Option<i64>,
}

/// Les options d'un ajout de passkey, écrites dans le formulaire (`data-…`) pour `passkey.js`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Registration {
    pub challenge: String,
    /// L'identifiant WebAuthn du compte, en base64url.
    pub user_id: String,
    pub user_name: String,
    pub display_name: String,
    /// Les passkeys déjà là (base64url) : l'authentificateur n'en crée pas une deuxième.
    pub exclude: Vec<String>,
    /// La session n'est pas récente : le mot de passe est redemandé (REL-06).
    pub password: bool,
}

/// Une durée écoulée, en mots : « 3 jours », `3 days` (« moins d'une minute » en dessous).
pub fn duration_words(lang: Lang, secs: i64) -> String {
    let secs = secs.max(0);
    let (n, fr, en) = if secs < 60 {
        return t(lang, "moins d'une minute", "less than a minute");
    } else if secs < 3600 {
        (secs / 60, "minute", "minute")
    } else if secs < 86_400 {
        (secs / 3600, "heure", "hour")
    } else {
        (secs / 86_400, "jour", "day")
    };
    let s = if n > 1 { "s" } else { "" };
    match lang {
        Lang::Fr => format!("{n} {fr}{s}"),
        Lang::En => format!("{n} {en}{s}"),
    }
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
pub fn account(v: &View<'_>, a: &AccountView, shell: &console::Shell, notice: Option<Notice>) -> String {
    let title = v.tr("Compte et sécurité", "Account and security");
    let tr = |f: &str, e: &str| v.tr(f, e);
    let protected = a.totp_on || !a.passkeys.is_empty();
    let totp = if a.totp_on {
        let left = match v.lang {
            Lang::Fr => format!("Activée · {} code{s} de secours restant{s}", a.recovery_left, s = if a.recovery_left > 1 { "s" } else { "" }),
            Lang::En => format!("On · {} recovery code{s} left", a.recovery_left, s = if a.recovery_left == 1 { "" } else { "s" }),
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
            "<div class=\"row\">{icon}<div class=\"ka-row-main\"><b>{name}</b><small>{left}</small></div><div class=\"acts\"><details class=\"acct-rename ka-more\"><summary class=\"btn btn-ghost btn-sm\">{manage}</summary>\n{form}\n</details></div></div>\n",
            icon = console::ic("shield"),
            name = tr("Double authentification", "Two-factor authentication"),
            manage = tr("Gérer", "Manage"),
            form = v.form("/compte/double-authentification/couper", "acct-sub", &inner),
        )
    } else {
        format!(
            "<div class=\"row\">{icon}<div class=\"ka-row-main\"><b>{name}</b><small>{text}</small></div><div class=\"acts\"><a class=\"btn btn-ghost btn-sm\" href=\"/compte/double-authentification{q}\">{on}</a></div></div>\n",
            icon = console::ic("shield"),
            name = tr("Double authentification", "Two-factor authentication"),
            text = tr("Un code de votre application d'authentification, en plus du mot de passe.", "A code from your authenticator app, on top of your password."),
            q = v.q(),
            on = tr("Activer", "Turn on"),
        )
    };
    // Lot 1b : les passkeys (liste, « Ajouter une passkey »), au-dessus de la double authentification.
    let passkeys = passkeys_html(v, a);
    let sessions: String = a
        .sessions
        .iter()
        .map(|s| {
            let when = match v.lang {
                Lang::Fr => format!("Ouverte le {}, vue le {}", date(v.lang, s.created_at), date(v.lang, s.seen_at)),
                Lang::En => format!("Opened {}, last seen {}", date(v.lang, s.created_at), date(v.lang, s.seen_at)),
            };
            let act = if s.current {
                format!("<span class=\"ka-pill ka-pill-on\">{}</span>", tr("Cette session", "This session"))
            } else {
                v.form(
                    "/compte/sessions/fermer",
                    "acct-inline",
                    &format!("<input type=\"hidden\" name=\"id\" value=\"{}\">\n<button class=\"btn btn-ghost btn-sm\" type=\"submit\">{}</button>", s.id, tr("Fermer", "Close")),
                )
            };
            format!("<div class=\"row\">{}<div class=\"ka-row-main\"><b>{}</b><small>{when}</small></div><div class=\"acts\">{act}</div></div>\n", console::ic("monitor"), esc(&s.device))
        })
        .collect();
    let devices = devices_html(v, &a.devices, a.now);
    let sign_out = v.form(
        "/compte/deconnexion",
        "acct-inline",
        &format!("<button class=\"btn btn-ghost btn-sm\" type=\"submit\">{}</button>", tr("Se déconnecter", "Sign out")),
    );
    // Lot 1b : « Exporter mes données » et « Supprimer le compte ».
    let data = data_html(v);
    let state = if protected {
        format!("<span class=\"ka-pill ka-pill-on\">{}</span>", tr("Compte protégé", "Account protected"))
    } else {
        format!("<span class=\"ka-pill ka-pill-warn\">{}</span>", tr("À faire", "To do"))
    };
    let body = format!(
        "<header class=\"ka-head\"><div><h1><strong>{title}</strong></h1>\n<p class=\"ka-lede\">{lede}</p></div></header>\n{notice}<section class=\"ka-section\">\n<div class=\"ka-section-head\"><h2>{you}</h2></div>\n<div class=\"ka-list\">\n<div class=\"row\">{avatar}<div class=\"ka-row-main\"><b>{name}</b><small>{email}</small></div><div class=\"acts\">{sign_out}</div></div>\n</div>\n</section>\n<section class=\"ka-section\" id=\"securite\">\n<div class=\"ka-section-head\"><h2>{security}</h2>{state}</div>\n<div class=\"ka-list\">\n{passkeys}{totp}</div>\n</section>\n{devices}<section class=\"ka-section\">\n<div class=\"ka-section-head\"><h2>{sessions_h}</h2><small>{sessions_sub}</small></div>\n<div class=\"ka-list\">\n{sessions}</div>\n</section>\n{data}",
        lede = tr("Votre adresse, la façon dont vous vous connectez et les appareils où vous l'êtes.", "Your address, how you sign in and the devices where you are signed in."),
        notice = notice_html(v.lang, notice),
        you = tr("Vous", "You"),
        avatar = console::avatar(&a.name),
        name = esc(&a.name),
        email = esc(&a.email),
        security = tr("Sécurité", "Security"),
        sessions_h = tr("Sessions ouvertes", "Open sessions"),
        sessions_sub = tr("Dans les navigateurs", "In browsers"),
    );
    console::frame(v, shell, console::Here::Account, &title, "/compte", &body)
}

/// Activer la double authentification : le QR code, la clé, le code.
pub fn totp_setup(v: &View<'_>, qr_svg: &str, secret_grouped: &str, uri: &str, password: bool, notice: Option<Notice>) -> String {
    let title = v.tr("Double authentification", "Two-factor authentication");
    let inner = format!(
        "{field}{password}\n<button class=\"btn kn-primary\" type=\"submit\">{submit}</button>",
        field = field(&v.tr("Code affiché", "Code shown"), &code_input("code", notice.is_some())),
        password = if password { format!("\n{}", field(&v.tr("Mot de passe, pour confirmer", "Password, to confirm"), &password_input("password", "current-password"))) } else { String::new() },
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

// --- Lot 2 : ordinateurs rattachés ; lot 3 : appareils connectés -------------------------

/// « Appareils connectés » sur la page du compte (rien tant qu'aucun ne l'est).
fn devices_html(v: &View<'_>, devices: &[DeviceView], now: i64) -> String {
    if devices.is_empty() {
        return String::new();
    }
    let rows: String = devices
        .iter()
        .map(|d| {
            let name = if d.name.is_empty() { v.tr("Appareil", "Device") } else { d.name.clone() };
            let how = if d.tv { v.tr("par un code", "with a code") } else { v.tr("par l'app", "from the app") };
            let ago = duration_words(v.lang, now - d.seen_at);
            let when = match v.lang {
                Lang::Fr => format!("Utilisé il y a {ago} · connecté {how} le {}", date(v.lang, d.created_at)),
                Lang::En => format!("Used {ago} ago · connected {how} {}", date(v.lang, d.created_at)),
            };
            let close = v.form(
                "/compte/appareils/deconnecter",
                "acct-inline",
                &format!(
                    "<input type=\"hidden\" name=\"id\" value=\"{}\">\n<button class=\"btn btn-ghost btn-sm\" type=\"submit\">{}</button>",
                    d.id,
                    v.tr("Déconnecter", "Disconnect")
                ),
            );
            format!("<div class=\"row\">{}<div class=\"ka-row-main\"><b>{}</b><small>{when}</small></div><div class=\"acts\">{close}</div></div>\n", console::ic(if d.tv { "pad" } else { "monitor" }), esc(&name))
        })
        .collect();
    format!(
        "<section class=\"ka-section\">\n<div class=\"ka-section-head\"><h2>{}</h2><small>{}</small></div>\n<div class=\"ka-list\">\n{rows}</div>\n<p class=\"ka-muted ka-after\">{}</p>\n</section>\n",
        v.tr("Appareils connectés", "Connected devices"),
        v.tr("Ils vous montrent vos ordinateurs", "They show you your computers"),
        v.tr("Un appareil qui ne sert pas pendant 90 jours est déconnecté.", "A device unused for 90 days is signed out."),
    )
}

/// « Rattacher {nom} » : le propriétaire, puis confirmer (`/compte/rattacher`).
pub fn attach(v: &View<'_>, code: &str, computer: &str, name: &str, email: &str, orgs: &[(String, String)]) -> String {
    let computer = if computer.is_empty() { v.tr("cet ordinateur", "this computer") } else { computer.to_string() };
    let title = match v.lang {
        Lang::Fr => t(v.lang, &format!("Rattacher {}", esc(&computer)), ""),
        Lang::En => format!("Attach {}", esc(&computer)),
    };
    let me = if name.is_empty() { esc(email) } else { format!("{} <small>{}</small>", esc(name), esc(email)) };
    // Lot 5 : « Moi », ou une organisation que ce compte gère.
    let org_choices: String = orgs
        .iter()
        .map(|(id, org)| format!("\n<label class=\"check\"><input type=\"radio\" name=\"proprietaire\" value=\"{}\"> {}</label>", esc(id), esc(org)))
        .collect();
    let inner = format!(
        "<input type=\"hidden\" name=\"code\" value=\"{code}\">\n<fieldset class=\"field\">\n<legend class=\"mono-label\">{owner}</legend>\n<label class=\"check\"><input type=\"radio\" name=\"proprietaire\" value=\"moi\" checked> {me}</label>{org_choices}\n</fieldset>\n<p class=\"hint\">{after}</p>\n<button class=\"btn kn-primary\" type=\"submit\" name=\"action\" value=\"rattacher\">{title}</button>\n<button class=\"btn btn-ghost\" type=\"submit\" name=\"action\" value=\"annuler\">{cancel}</button>",
        code = esc(code),
        owner = v.tr("Propriétaire", "Owner"),
        after = v.tr(
            "L'ordinateur acceptera alors les comptes KyberNow, et l'accès hors de chez vous s'activera. Admin permet de couper l'un et l'autre, et de le détacher.",
            "The computer will then accept KyberNow accounts, and access away from home will turn on. Admin can turn either off, and detach it.",
        ),
        cancel = v.tr("Annuler", "Cancel"),
    );
    let card = format!(
        "<h1>{title}</h1>\n<p class=\"help\">{help}</p>\n{form}",
        help = v.tr(
            "Admin demande à rattacher cet ordinateur à un compte KyberNow : il apparaîtra dans vos apps, où que vous soyez.",
            "Admin asks to attach this computer to a KyberNow account: it will show up in your apps, wherever you are.",
        ),
        form = v.form("/compte/rattacher", "", &inner),
    );
    page(v, &title, "/compte/rattacher", false, &card)
}

/// Une page d'issue, sans formulaire : un titre, un texte, « Votre compte ».
pub fn done(v: &View<'_>, here: &str, title: &str, text: &str) -> String {
    let card = format!(
        "<h1>{title}</h1>\n<p class=\"help\">{text}</p>\n<p class=\"acct-links\"><a href=\"/compte{q}\">{account}</a></p>",
        q = v.q(),
        account = v.tr("Votre compte", "Your account"),
    );
    page(v, title, here, false, &card)
}

/// « Connecter KyberNow Link sur {appareil} ? » (fenêtre du système).
/// `etat` : la valeur que l'app a donnée (`?etat=`), rendue telle quelle au retour.
pub fn device_consent(v: &View<'_>, device: &str, email: &str, etat: &str, challenge: &str) -> String {
    let device = if device.is_empty() { v.tr("cet appareil", "this device") } else { device.to_string() };
    let title = match v.lang {
        Lang::Fr => t(v.lang, &format!("Connecter KyberNow Link sur {} ?", esc(&device)), ""),
        Lang::En => format!("Connect KyberNow Link on {}?", esc(&device)),
    };
    let inner = format!(
        "<input type=\"hidden\" name=\"etat\" value=\"{etat}\">{challenge}\n<button class=\"btn kn-primary\" type=\"submit\" name=\"action\" value=\"connecter\">{yes}</button>\n<button class=\"btn btn-ghost\" type=\"submit\" name=\"action\" value=\"annuler\">{no}</button>",
        etat = esc(etat),
        challenge = if challenge.is_empty() { String::new() } else { format!("\n<input type=\"hidden\" name=\"code_challenge\" value=\"{}\">", esc(challenge)) },
        yes = v.tr("Connecter", "Connect"),
        no = v.tr("Annuler", "Cancel"),
    );
    let help = match v.lang {
        Lang::Fr => t(
            v.lang,
            &format!(
                "L'app pourra ouvrir les ordinateurs du compte <b>{}</b>, sans connaître votre mot de passe. Vous pourrez la déconnecter depuis votre compte.",
                esc(email)
            ),
            "",
        ),
        Lang::En => format!("The app will be able to open the computers of the account <b>{}</b>, without knowing your password. You can disconnect it from your account.", esc(email)),
    };
    let card = format!("<h1>{title}</h1>\n<p class=\"help\">{help}</p>\n{form}", form = v.form("/compte/connecter-appareil", "", &inner));
    page(v, &title, "/compte/connecter-appareil", false, &card)
}

/// Le retour vers l'app (`kybernow://compte?code=…`) : la page y renvoie
/// d'elle-même, et un bouton le fait si le navigateur ne suit pas.
pub fn device_return(v: &View<'_>, target: &str, connected: bool) -> String {
    let title = v.tr("Retour à l'app", "Back to the app");
    let help = if connected {
        v.tr("C'est fait : KyberNow Link est connecté à votre compte.", "Done: KyberNow Link is connected to your account.")
    } else {
        v.tr("Connexion annulée : l'app n'est pas connectée.", "Sign-in cancelled: the app isn't connected.")
    };
    let card = format!(
        "<h1>{title}</h1>\n<p class=\"help\">{help}</p>\n<a class=\"btn kn-primary\" href=\"{target}\">{open}</a>",
        target = esc(target),
        open = v.tr("Revenir à KyberNow Link", "Return to KyberNow Link"),
    );
    let html = page(v, &title, "/compte/connecter-appareil", false, &card);
    // Sans script : un rafraîchissement vers le schéma de l'app.
    html.replacen("<meta name=\"robots\" content=\"noindex\">", &format!("<meta name=\"robots\" content=\"noindex\">\n<meta http-equiv=\"refresh\" content=\"0;url={}\">", esc(target)), 1)
}

/// `/tv` : le code affiché sur l'écran du téléviseur.
pub fn tv_entry(v: &View<'_>, typed: &str, notice: Option<Notice>) -> String {
    let title = v.tr("Connecter un appareil", "Connect a device");
    let wrong = notice.is_some();
    // En GET vers la page d'approbation : rien n'est changé avant « Connecter ».
    let card = format!(
        "<h1>{title}</h1>\n<p class=\"help\">{help}</p>\n{notice}<form method=\"get\" action=\"/compte/appareil\">\n<input type=\"hidden\" name=\"lang\" value=\"{lang}\">\n{field}\n<button class=\"btn kn-primary\" type=\"submit\">{submit}</button>\n</form>",
        help = v.tr(
            "Entrez le code de huit signes affiché sur l'écran de l'appareil (Apple TV, téléviseur, Steam Deck…).",
            "Enter the eight-character code shown on the device's screen (Apple TV, TV, Steam Deck…).",
        ),
        notice = notice_html(v.lang, notice),
        lang = v.lang.code(),
        field = field(
            &v.tr("Code", "Code"),
            &format!(
                r#"<input id="code" class="code-input" name="code" autocomplete="off" autocapitalize="characters" spellcheck="false" maxlength="12" placeholder="XXXX-XXXX" value="{}" required{}>"#,
                esc(typed),
                if wrong { " autofocus" } else { "" }
            )
        ),
        submit = v.tr("Continuer", "Continue"),
    );
    page(v, &title, "/tv", false, &card)
}

/// « Connecter l'appareil ? » (code d'un téléviseur), sur le modèle de la maquette.
pub fn tv_approve(v: &View<'_>, code: &str, device: &str, email: &str) -> String {
    let device = if device.is_empty() { v.tr("cet appareil", "this device") } else { device.to_string() };
    let title = match v.lang {
        Lang::Fr => t(v.lang, &format!("Connecter {} ?", esc(&device)), ""),
        Lang::En => format!("Connect {}?", esc(&device)),
    };
    let inner = format!(
        "<input type=\"hidden\" name=\"code\" value=\"{code}\">\n<button class=\"btn kn-primary\" type=\"submit\" name=\"action\" value=\"connecter\">{yes}</button>\n<button class=\"btn btn-ghost\" type=\"submit\" name=\"action\" value=\"refuser\">{no}</button>",
        code = esc(code),
        yes = v.tr("Connecter", "Connect"),
        no = v.tr("Ce n'est pas moi", "It wasn't me"),
    );
    let help = match v.lang {
        Lang::Fr => t(
            v.lang,
            &format!(
                "Le code <b>{}</b> s'affiche sur son écran. Il pourra ouvrir les ordinateurs du compte <b>{}</b> ; vous pourrez le déconnecter depuis votre compte.",
                esc(code),
                esc(email)
            ),
            "",
        ),
        Lang::En => format!(
            "The code <b>{}</b> is shown on its screen. It will be able to open the computers of the account <b>{}</b>; you can disconnect it from your account.",
            esc(code),
            esc(email)
        ),
    };
    let card = format!("<h1>{title}</h1>\n<p class=\"help\">{help}</p>\n{form}", form = v.form("/compte/appareil", "", &inner));
    page(v, &title, "/compte/appareil", false, &card)
}

// --- Lot 1b : passkeys, données du compte ---------------------------------------

/// Un attribut `data-…` échappé.
fn data_attr(name: &str, value: &str) -> String {
    format!(" data-{name}=\"{}\"", esc(value))
}

/// Le message montré par `passkey.js` quand le navigateur refuse ou que l'on annule.
fn passkey_error(v: &View<'_>) -> String {
    format!(
        "<p class=\"notice passkey-error\" role=\"alert\" hidden>{}</p>",
        v.tr("La passkey n'a pas abouti : réessayez, ou utilisez votre mot de passe.", "The passkey didn't go through: try again, or use your password.")
    )
}

/// Dit, sans script ou sans WebAuthn, que le navigateur ne sait pas faire de passkey ; `passkey.js` le cache.
fn passkey_unsupported(v: &View<'_>) -> String {
    format!(
        "<p class=\"hint\" data-passkey-none>{}</p>",
        v.tr(
            "Ce navigateur ne permet pas d'ajouter une passkey ici (scripts coupés ou WebAuthn absent).",
            "This browser can't add a passkey here (scripts off or no WebAuthn).",
        )
    )
}

/// Un formulaire de `navigator.credentials.get()` (connexion, confirmation), caché sans script.
fn passkey_get_form(v: &View<'_>, path: &str, allow: &[String], label: &str) -> String {
    let action = if path == "/compte/supprimer" { "<input type=\"hidden\" name=\"action\" value=\"passkey\">\n" } else { "" };
    passkey_get_form_with(v, path, allow, label, action)
}

/// Le même, avec des champs de plus.
fn passkey_get_form_with(v: &View<'_>, path: &str, allow: &[String], label: &str, action: &str) -> String {
    let allow = if allow.is_empty() { String::new() } else { data_attr("allow", &allow.join(",")) };
    format!(
        "<form method=\"post\" action=\"{path}{q}\" data-passkey=\"get\"{challenge}{allow} hidden>\n{csrf}\n{action}<button class=\"btn kn-primary\" type=\"submit\">{label}</button>\n{error}\n</form>",
        q = v.q(),
        challenge = data_attr("challenge", v.challenge),
        csrf = v.csrf_field(),
        error = passkey_error(v),
    )
}

/// Le formulaire « Ajouter une passkey » (`navigator.credentials.create()`), caché sans script.
fn passkey_create_form(v: &View<'_>, r: &Registration, class: &str) -> String {
    passkey_create_form_with(v, r, class, "")
}

/// Le même, avec des champs de plus (`suite` : où revenir après l'ajout).
fn passkey_create_form_with(v: &View<'_>, r: &Registration, class: &str, extra: &str) -> String {
    format!(
        "<form method=\"post\" action=\"/compte/passkeys/ajouter{q}\" class=\"acct-inline\" data-passkey=\"create\"{attrs} hidden>\n{csrf}\n{extra}<button class=\"{class}\" type=\"submit\">{label}</button>\n{error}\n</form>",
        q = v.q(),
        attrs = [
            data_attr("challenge", &r.challenge),
            data_attr("user-id", &r.user_id),
            data_attr("user-name", &r.user_name),
            data_attr("user-display", &r.display_name),
            data_attr("rp-name", "KyberNow"),
            data_attr("exclude", &r.exclude.join(",")),
        ]
        .concat(),
        csrf = v.csrf_field(),
        extra = if r.password { format!("{extra}{}\n", field(&v.tr("Mot de passe, pour confirmer", "Password, to confirm"), &password_input("password-passkey", "current-password"))) } else { extra.to_string() },
        label = v.tr("Ajouter une passkey", "Add a passkey"),
        error = passkey_error(v),
    )
}

/// La partie « Passkeys » de la page du compte.
fn passkeys_html(v: &View<'_>, a: &AccountView) -> String {
    let tr = |f: &str, e: &str| v.tr(f, e);
    let add = match &a.register {
        Some(r) => passkey_create_form(v, r, "btn kn-primary btn-sm"),
        None => format!("<span class=\"ka-pill ka-pill-off\">{}</span>", tr("20 au plus", "20 at most")),
    };
    let summary = if a.passkeys.is_empty() {
        tr("Aucune. Une passkey remplace le mot de passe et vaut double authentification.", "None. A passkey replaces the password and counts as two-factor authentication.")
    } else {
        tr("Elles remplacent le mot de passe et valent double authentification.", "They replace the password and count as two-factor authentication.")
    };
    let list: String = a
        .passkeys
        .iter()
        .map(|p| {
            let added = date(v.lang, p.created_at);
            let when = match (v.lang, p.seen_at) {
                (Lang::Fr, Some(seen)) => format!("Ajoutée le {added}, utilisée le {}", date(v.lang, seen)),
                (Lang::Fr, None) => format!("Ajoutée le {added}, jamais utilisée"),
                (Lang::En, Some(seen)) => format!("Added {added}, last used {}", date(v.lang, seen)),
                (Lang::En, None) => format!("Added {added}, never used"),
            };
            let id = format!("<input type=\"hidden\" name=\"id\" value=\"{}\">", p.id);
            let rename = format!(
                "<details class=\"acct-rename\"><summary class=\"btn btn-ghost btn-sm\">{label}</summary>\n{form}\n</details>",
                label = tr("Renommer", "Rename"),
                form = v.form(
                    "/compte/passkeys/renommer",
                    "acct-inline",
                    &format!(
                        "{id}\n<input name=\"name\" maxlength=\"60\" value=\"{name}\" aria-label=\"{aria}\" required>\n<button class=\"btn btn-ghost btn-sm\" type=\"submit\">{ok}</button>",
                        name = esc(&p.name),
                        aria = tr("Nom de la passkey", "Passkey name"),
                        ok = tr("Enregistrer", "Save"),
                    ),
                ),
            );
            let remove = v.form(
                "/compte/passkeys/retirer",
                "acct-inline",
                &format!("{id}\n<button class=\"btn btn-ghost btn-sm\" type=\"submit\">{}</button>", tr("Retirer", "Remove")),
            );
            format!("<div class=\"row ka-row-sub\"><span class=\"ka-lead-space\"></span><div class=\"ka-row-main\"><b>{}</b><small>{when}</small></div><div class=\"acts\">{rename}{remove}</div></div>\n", esc(&p.name))
        })
        .collect();
    format!(
        "<div class=\"row\">{icon}<div class=\"ka-row-main\"><b>{name}</b><small>{summary}</small>{none}</div><div class=\"acts\">{add}</div></div>\n{list}",
        icon = console::ic("key"),
        name = tr("Passkeys", "Passkeys"),
        none = if a.register.is_some() { passkey_unsupported(v) } else { String::new() },
    )
}

/// La partie « Vos données » de la page du compte : l'export, la suppression.
fn data_html(v: &View<'_>) -> String {
    let tr = |f: &str, e: &str| v.tr(f, e);
    format!(
        "<section class=\"ka-section\">\n<div class=\"ka-section-head\"><h2>{h}</h2></div>\n<div class=\"ka-list\">\n<div class=\"row\">{i_log}<div class=\"ka-row-main\"><b>{export}</b><small>{export_text}</small></div><div class=\"acts\">{export_form}</div></div>\n<div class=\"row\">{i_close}<div class=\"ka-row-main\"><b>{delete}</b><small>{delete_text}</small></div><div class=\"acts\"><a class=\"btn btn-danger btn-sm\" href=\"/compte/supprimer{q}\">{delete_btn}</a></div></div>\n</div>\n</section>\n",
        i_log = console::ic("log"),
        i_close = console::ic("close"),
        h = tr("Vos données", "Your data"),
        export = tr("Exporter mes données", "Export my data"),
        export_text = tr("Un fichier JSON : le compte, la sécurité, les sessions, le journal.", "A JSON file: the account, security, sessions, log."),
        export_form = v.form(
            "/compte/export",
            "acct-inline",
            &format!("<button class=\"btn btn-ghost btn-sm\" type=\"submit\">{}</button>", tr("Exporter", "Export")),
        ),
        delete = tr("Supprimer le compte", "Delete the account"),
        delete_text = tr("Le compte et ses données sont effacés.", "The account and its data are erased."),
        q = v.q(),
        delete_btn = tr("Supprimer…", "Delete…"),
    )
}

/// « Supprimer le compte » : confirmer avec une passkey (`allow` : celles du compte) ou le mot de passe.
pub fn delete_account(v: &View<'_>, allow: &[String], notice: Option<Notice>) -> String {
    let title = v.tr("Supprimer le compte", "Delete the account");
    let passkey = if allow.is_empty() || v.challenge.is_empty() {
        String::new()
    } else {
        format!(
            "{form}\n<p class=\"acct-or\" data-passkey-or hidden>{or}</p>",
            form = passkey_get_form(v, "/compte/supprimer", allow, &v.tr("Confirmer avec une passkey", "Confirm with a passkey")),
            or = v.tr("ou", "or"),
        )
    };
    let inner = format!(
        "<input type=\"hidden\" name=\"action\" value=\"password\">\n{field}\n<button class=\"btn kn-primary\" type=\"submit\">{submit}</button>",
        field = field(&v.tr("Mot de passe, pour confirmer", "Password, to confirm"), &password_input("password", "current-password")),
        submit = v.tr("Supprimer définitivement", "Delete permanently"),
    );
    let card = format!(
        "<h1>{title}</h1>\n<p class=\"help\">{help}</p>\n<p class=\"hint\">{export}</p>\n{notice}{passkey}\n{form}\n<p class=\"acct-links\"><a href=\"/compte{q}\">{cancel}</a></p>",
        help = v.tr(
            "Le compte, ses passkeys, ses sessions, sa double authentification et son journal seront effacés tout de suite. Les sauvegardes du relais les oublient sous 30 jours.",
            "The account, its passkeys, sessions, two-factor authentication and log will be erased right away. The relay's backups forget them within 30 days.",
        ),
        export = v.tr("Pensez à exporter vos données avant, depuis la page du compte.", "Remember to export your data first, from the account page."),
        notice = notice_html(v.lang, notice),
        form = v.form("/compte/supprimer", "", &inner),
        q = v.q(),
        cancel = v.tr("Annuler", "Cancel"),
    );
    page(v, &title, "/compte/supprimer", false, &card)
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

    fn registration() -> Registration {
        Registration {
            challenge: "defi-ajout".into(),
            user_id: "dXNlcg".into(),
            user_name: "ana@exemple.org".into(),
            display_name: "Ana \"<b>".into(),
            exclude: vec!["Y3JlZA".into()],
            password: true,
        }
    }

    fn all(lang: Lang, appareil: Option<&str>) -> Vec<(&'static str, String)> {
        let v = View { lang, csrf: "jeton-csrf", appareil, challenge: "defi" };
        let account_view = AccountView {
            email: "ana@exemple.org".into(),
            name: "Ana <b>".into(),
            totp_on: true,
            recovery_left: 1,
            sessions: vec![
                SessionView { id: 1, device: "Firefox, Windows".into(), created_at: 1_700_000_000, seen_at: 1_700_000_000, current: true },
                SessionView { id: 2, device: "Safari, iPhone".into(), created_at: 1_700_000_000, seen_at: 1_700_100_000, current: false },
            ],
            computers: vec![ComputerView { id: "st1".into(), name: "PC du <salon>".into(), attached_at: 1_700_000_000, online: true }],
            devices: vec![
                DeviceView { id: 3, name: "Apple TV".into(), tv: true, created_at: 1_700_000_000, seen_at: 1_700_000_000 },
                DeviceView { id: 4, name: "iPad de Léa".into(), tv: false, created_at: 1_700_000_000, seen_at: 1_700_600_000 },
            ],
            passkeys: vec![
                PasskeyView { id: 7, name: "Chrome, Mac <i>".into(), created_at: 1_700_000_000, seen_at: None },
                PasskeyView { id: 8, name: "Clé USB".into(), created_at: 1_700_000_000, seen_at: Some(1_700_100_000) },
            ],
            register: Some(registration()),
            orgs: vec![("org_abc".into(), "Les Lignes <Claires>".into())],
            now: 1_700_604_000,
        };
        let mut pages = vec![
            ("connexion", sign_in(&v, "ana@exemple.org", Some(Notice::BadCredentials), true, true)),
            ("connexion/limite", sign_in(&v, "", Some(Notice::TooMany(15)), true, false)),
            ("inscription", sign_up(&v, SignUp::Open, "Ana", "", Some(Notice::Weak(Weak::Known)))),
            ("inscription/app", sign_up(&v, SignUp::FromApp, "", "", None)),
            ("inscription/fermee", sign_up(&v, SignUp::Closed, "", "", None)),
            ("verifier", verify(&v, "ana@exemple.org", Some(Notice::Resent), false)),
            ("verifier/changer", verify(&v, "ana@exemple.org", None, true)),
            ("proteger", protect(&v, false, Some(&Registration { exclude: vec![], ..registration() }))),
            ("proteger/passkey", protect(&v, true, Some(&registration()))),
            ("supprimer", delete_account(&v, &["Y3JlZA".into()], Some(Notice::BadPassword))),
            ("supprimer/sans-passkey", delete_account(&v, &[], None)),
            ("code", code(&v, Some(Notice::WrongCode), false)),
            ("code/secours", code(&v, None, true)),
            ("oubli", forgot(&v, Forgot::Form(None))),
            ("oubli/envoye", forgot(&v, Forgot::Sent)),
            ("oubli/indisponible", forgot(&v, Forgot::Unavailable)),
            ("nouveau", new_password(&v, "jeton", Some(Notice::Weak(Weak::TooShort)))),
            ("compte", account(&v, &account_view, &console::tests::shell(), Some(Notice::TotpOn))),
            ("compte/sans-2fa", account(&v, &AccountView { totp_on: false, passkeys: vec![], register: None, ..account_view.clone() }, &console::tests::shell(), None)),
            ("2fa", totp_setup(&v, "<svg></svg>", "GEZD GNBV", "otpauth://totp/x", true, Some(Notice::WrongCode))),
            ("secours", recovery_codes(&v, &["abcd-efgh".into()], None)),
            ("message", message(&v, "/compte/verifier", "Titre", "Texte.", ("/compte/connexion", "Lien"))),
            ("rattacher", attach(&v, "code", "PC du salon", "Ana", "ana@exemple.org", &[])),
            ("rattacher/organisation", attach(&v, "code", "PC du salon", "Ana", "ana@exemple.org", &[("org_abc".into(), "Les Lignes <Claires>".into())])),
            ("rattache", done(&v, "/compte/rattacher", "Titre", "Texte.")),
            ("appareil", device_consent(&v, "iPad de Léa", "ana@exemple.org", "etat-1", "")),
            ("tv", tv_entry(&v, "", Some(Notice::WrongCode))),
            ("tv/approuver", tv_approve(&v, "BCDF-GHJK", "Apple TV du salon", "ana@exemple.org")),
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
            Notice::StationDetached,
            Notice::DeviceClosed,
            Notice::Weak(Weak::TooLong),
            Notice::Weak(Weak::LikeEmail),
            Notice::TermsMissing,
            Notice::PasskeyFailed,
            Notice::PasskeyAdded,
            Notice::PasskeyRenamed,
            Notice::PasskeyRemoved,
            Notice::PersonAdded,
            Notice::InviteSent,
            Notice::AccessChanged,
            Notice::AccessRemoved,
            Notice::NoRights,
            Notice::BadDate,
            Notice::NeedTwoFactor,
            Notice::NotAllowed,
            Notice::OrgCreated,
            Notice::OrgRenamed,
            Notice::OrgNameMissing,
            Notice::MemberAdded,
            Notice::RoleChanged,
            Notice::MemberRemoved,
            Notice::InviteCancelled,
            Notice::LastOwner,
            Notice::OrgLeft,
            Notice::GroupCreated,
            Notice::GroupTaken,
            Notice::GroupDeleted,
            Notice::GroupChanged,
            Notice::TokenRevoked,
            Notice::BadNumber,
            Notice::LinkRevoked,
            Notice::TooManyLinks,
            Notice::LinkAccepted,
            Notice::AccessUpdated("Léa <b>".into()),
            Notice::AlreadyMember("Paul".into()),
            Notice::InviteResent("lea@exemple.fr".into()),
            Notice::ResentTooSoon,
            Notice::Moved("PC".into(), "L'atelier".into()),
            Notice::NotManager("L'atelier".into()),
        ] {
            pages.push(("avis", sign_in(&v, "", Some(n), true, true)));
        }
        pages.extend(console::tests::all(&v));
        pages
    }

    /// Écrit toutes les pages et les fichiers du relais dans `KN_PAGES_DIR`, pour les regarder dans
    /// un navigateur sans relais (`python3 -m http.server`) : `KN_PAGES_DIR=… cargo test ecrire_les_pages -- --ignored`.
    #[test]
    #[ignore]
    fn ecrire_les_pages() {
        let dir = std::path::PathBuf::from(std::env::var("KN_PAGES_DIR").expect("KN_PAGES_DIR"));
        std::fs::create_dir_all(dir.join("assets")).unwrap();
        for name in assets::NAMES {
            std::fs::write(dir.join("assets").join(name), assets::get(name).unwrap().body).unwrap();
        }
        for lang in [Lang::Fr, Lang::En] {
            for (name, page) in all(lang, None) {
                let file = format!("{}-{}.html", name.replace('/', "-"), lang.code());
                std::fs::write(dir.join(file), page).unwrap();
            }
        }
    }

    #[test]
    fn typographie_francaise_et_pas_de_tiret_cadratin() {
        for (name, page) in all(Lang::Fr, None) {
            let text = text_of(&page);
            for bad in [" :", " ;", " !", " ?", "« ", " »"] {
                assert!(!text.contains(bad), "{name} : « {bad} » dans le texte : {:?}", text.find(bad).map(|i| text[i.saturating_sub(80)..].chars().take(120).collect::<String>()).unwrap_or_default());
            }
        }
        for lang in [Lang::Fr, Lang::En] {
            for (name, page) in all(lang, None) {
                assert!(!page.contains('\u{2014}'), "{name} : tiret cadratin");
            }
        }
        assert!(sign_in(&View { lang: Lang::Fr, csrf: "", appareil: None, challenge: "" }, "", None, true, true).contains("Mot de passe oublié\u{202F}?"));
    }

    #[test]
    fn rien_qui_charge_du_dehors_et_un_seul_script() {
        let script = format!("<script src=\"{}\" defer></script>", assets::url("passkey.js"));
        for lang in [Lang::Fr, Lang::En] {
            for (name, page) in all(lang, None) {
                // Le seul script : celui des passkeys, servi par le relais, là où il y a une passkey.
                let page_without = page.replace(&script, "");
                assert_eq!(page.contains(&script), page.contains("data-passkey=\""), "{name} : script des passkeys");
                for bad in ["<script", "<iframe", "style=", "onclick=", "onload=", "analytics", "javascript:"] {
                    assert!(!page_without.contains(bad), "{name} : {bad}");
                }
                for part in page.split("src=\"").skip(1).chain(page.split("<link ").skip(1).map(|l| l.split("href=\"").nth(1).unwrap_or(""))) {
                    assert!(part.starts_with('/'), "{name} : ressource hors du relais");
                }
                // Seule exception : « Ouvrir sur le réseau », vers l'adresse d'Admin sur le réseau de l'ordinateur.
                // Et le lien d'accès tout juste créé, montré une seule fois pour être copié.
                assert!(!page.contains("https://") || name == "admin-reseau" || name == "personnes/lien-cree", "{name} : adresse absolue");
                // Chaque formulaire est en POST, avec son jeton CSRF et la langue (sauf `/tv`, en GET, qui ne change rien).
                for form in page.split("<form ").skip(1).filter(|f| !f.starts_with("method=\"get\" action=\"/compte/appareil\"")) {
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
        let v = View { lang: Lang::Fr, csrf: "c", appareil: Some("abc"), challenge: "" };
        let page = sign_in(&v, "", None, true, true);
        assert!(!page.contains("Créer un compte"));
        assert!(page.contains("action=\"/compte/connexion?lang=fr&appareil=abc\""));
        assert!(sign_in(&View { appareil: None, ..v }, "", None, true, true).contains("Créer un compte"));
        assert!(!sign_in(&View { appareil: None, ..v }, "", None, false, true).contains("Créer un compte"), "sans SMTP, pas d'inscription");
        let refused = sign_up(&v, SignUp::FromApp, "", "", None);
        assert!(refused.contains("dans un navigateur") && !refused.contains("<form"));
    }

    #[test]
    fn passkeys_cachees_sans_script() {
        let pages = all(Lang::Fr, None);
        let get = |n: &str| pages.iter().find(|(name, _)| *name == n).unwrap().1.clone();
        // Chaque formulaire de passkey est caché (le script le montre), avec son message d'erreur caché.
        for (name, page) in &pages {
            for form in page.split("<form ").skip(1).filter(|f| f.contains("data-passkey=\"")) {
                let tag = &form[..form.find('>').unwrap()];
                assert!(tag.ends_with(" hidden"), "{name} : formulaire de passkey visible sans script");
                assert!(form.contains("class=\"notice passkey-error\" role=\"alert\" hidden"), "{name}");
            }
        }
        // Connexion : la passkey d'abord, avec le défi ; sans défi, pas de passkey.
        let sign_in_page = get("connexion");
        assert!(sign_in_page.find("Continuer avec une passkey").unwrap() < sign_in_page.find("name=\"password\"").unwrap());
        assert!(sign_in_page.contains(r#"action="/compte/connexion/passkey?lang=fr" data-passkey="get" data-challenge="defi" hidden"#));
        assert!(!sign_in(&View { lang: Lang::Fr, csrf: "c", appareil: None, challenge: "" }, "", None, true, true).contains("passkey"));
        // Ajout : les options échappées, les passkeys déjà là exclues.
        let account_page = get("compte");
        assert!(account_page.contains(r#"data-passkey="create" data-challenge="defi-ajout" data-user-id="dXNlcg" data-user-name="ana@exemple.org" data-user-display="Ana &quot;&lt;b&gt;" data-rp-name="KyberNow" data-exclude="Y3JlZA" hidden"#));
        assert!(account_page.contains("Chrome, Mac &lt;i&gt;") && account_page.contains("jamais utilisée"));
        assert!(account_page.contains(r#"name="id" value="7""#) && account_page.contains("/compte/passkeys/retirer?lang=fr"));
        assert!(account_page.contains("Exporter mes données") && account_page.contains("/compte/supprimer?lang=fr"));
        assert!(!get("compte/sans-2fa").contains("data-passkey"), "maximum atteint : pas d'ajout");
        // Protéger : la passkey conseillée, avant la double authentification ; « Ajoutée » s'il y en a une.
        let protect_page = get("proteger");
        assert!(protect_page.find("(conseillée)").unwrap() < protect_page.find("Double authentification").unwrap());
        assert!(protect_page.contains("data-passkey=\"create\""));
        assert!(!get("proteger/passkey").contains("data-passkey=\"") && get("proteger/passkey").contains("Ajoutée"));
        // Supprimer : par passkey (celles du compte seulement) ou par mot de passe.
        let delete_page = get("supprimer");
        assert!(delete_page.contains(r#"data-passkey="get" data-challenge="defi" data-allow="Y3JlZA" hidden"#));
        assert!(delete_page.contains(r#"name="action" value="passkey""#) && delete_page.contains(r#"name="action" value="password""#));
        assert!(!get("supprimer/sans-passkey").contains("data-passkey"));
        // Inscription : la case des conditions, avec leurs liens et leur version.
        let sign_up_page = get("inscription");
        assert!(sign_up_page.contains(r#"name="conditions" value="1" required"#));
        assert!(sign_up_page.contains("/conditions?lang=fr") && sign_up_page.contains("/confidentialite?lang=fr"));
        assert!(sign_up_page.contains(&format!("(version {})", crate::legal::Doc::Terms.version())));
    }

    #[test]
    fn ordinateurs_appareils_et_retour_vers_l_app() {
        let pages = all(Lang::Fr, None);
        let get = |n: &str| pages.iter().find(|(name, _)| *name == n).unwrap().1.clone();
        // Les ordinateurs sont sur « Ordinateurs » (l'espace du compte), « Détacher » sur la page de chacun.
        let computers = get("ordinateurs");
        assert!(computers.contains("PC du &lt;salon&gt;") && computers.contains("En ligne") && computers.contains("/compte/ordinateur?lang=fr&amp;id=st1"));
        let computer = get("personnes");
        assert!(computer.contains("action=\"/compte/ordinateurs/detacher?lang=fr\"") && computer.contains(r#"name="id" value="st1""#));
        let page = get("compte");
        assert!(page.contains("Appareils connectés") && page.contains("connecté par un code le"));
        // « Utilisé il y a {durée} » et l'aide des 90 jours (docs/comptes.md, « Sessions »).
        assert!(page.contains("Utilisé il y a 1 heure") && page.contains("Utilisé il y a 6 jours"), "{page}");
        assert!(page.contains("Un appareil qui ne sert pas pendant 90 jours est déconnecté."));
        assert_eq!(duration_words(Lang::Fr, 30), "moins d'une minute");
        assert_eq!(duration_words(Lang::En, 2 * 86_400 + 5), "2 days");
        assert_eq!(duration_words(Lang::Fr, 3 * 60), "3 minutes");
        let v = View { lang: Lang::Fr, csrf: "c", appareil: Some("iPad de Léa"), challenge: "" };
        let consent = device_consent(&v, "iPad de Léa", "ana@exemple.org", "etat-1", "");
        assert!(consent.contains("Connecter KyberNow Link sur iPad de Léa\u{202F}?"));
        assert!(consent.contains("action=\"/compte/connecter-appareil?lang=fr&appareil=iPad%20de%20L%C3%A9a\""));
        let back = device_return(&v, "kybernow://compte?code=abc&etat=x\"y", true);
        assert!(back.contains("<meta http-equiv=\"refresh\" content=\"0;url=kybernow://compte?code=abc&amp;etat=x&quot;y\">"));
        assert!(back.contains("href=\"kybernow://compte?code=abc&amp;etat=x&quot;y\""));
        let tv = tv_approve(&View { appareil: None, ..v }, "BCDF-GHJK", "Apple TV du salon", "ana@exemple.org");
        assert!(tv.contains("Connecter Apple TV du salon\u{202F}?") && tv.contains("Ce n'est pas moi") && tv.contains("BCDF-GHJK"));
        assert_eq!(pct("a b/é"), "a%20b%2F%C3%A9");
    }

    #[test]
    fn dates() {
        assert_eq!(date(Lang::Fr, 0), "01/01/1970");
        assert_eq!(date(Lang::Fr, 1_795_392_000), "23/11/2026");
        assert_eq!(date(Lang::En, 1_795_392_000), "Nov 23, 2026");
        assert_eq!(date(Lang::Fr, 951_782_400), "29/02/2000");
    }
}
