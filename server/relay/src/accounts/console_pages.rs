//! L'espace du compte (docs/comptes.md, « L'espace du compte ») : une appli à
//! menu latéral, de largeur constante, avec « Ordinateurs », « Personnes »,
//! « Familles et équipes » (chacune avec sa page : membres et rôles, groupes,
//! ordinateurs, journal, déploiement pour une équipe) et « Compte et
//! sécurité » ; le tour d'accueil (`/compte/bienvenue`) et la liste « Pour bien
//! démarrer ». Des formulaires, sans script : les panneaux s'ouvrent par
//! l'ancre de l'adresse (`#ajouter`), les pages passent de l'une à l'autre par
//! les transitions du navigateur (`@view-transition`).

use kybernow_tunnel::access::{Right, Rights};

use super::super::links::{AccessLink, LINK_DAYS, LinkState};
use super::super::orgs::{EnrollToken, Group, Invite, Kind, LogLine, Member, Role};
use super::super::station_groups::StationGroup;
use super::{Notice, Registration, View, date, esc, field, notice_html, passkey_create_form_with, passkey_get_form_with, passkey_unsupported, pct};
use crate::assets;
use crate::pages::{Lang, t};

// --- La coquille -----------------------------------------------------------------------

/// Où l'on est dans le menu.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Here {
    Computers,
    People,
    Families,
    Org(String),
    Account,
    None,
}

/// Une famille ou une équipe, dans le menu.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NavOrg {
    pub id: String,
    pub name: String,
    pub kind: Kind,
}

/// « Pour bien démarrer » : les quatre étapes.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Start {
    /// Une passkey ou la double authentification.
    pub protected: bool,
    /// Un ordinateur à lui.
    pub attached: bool,
    /// Un accès donné sur l'un de ses ordinateurs.
    pub opened: bool,
    /// Membre d'une famille ou d'une équipe.
    pub grouped: bool,
}

impl Start {
    pub fn done(&self) -> usize {
        [self.protected, self.attached, self.opened, self.grouped].iter().filter(|d| **d).count()
    }
}

/// Ce que le menu montre sur chaque page de l'espace.
#[derive(Debug, Clone, Default)]
pub struct Shell {
    pub name: String,
    pub email: String,
    pub orgs: Vec<NavOrg>,
    pub computers: usize,
    pub people: usize,
    pub start: Start,
}

/// Une icône du sprite de la page.
pub fn ic(name: &str) -> String {
    format!("<svg class=\"ka-ic\" aria-hidden=\"true\"><use href=\"#i-{name}\"></use></svg>")
}

/// Les icônes (traits de 1,7), une fois par page.
pub const SPRITE: &str = r##"<svg class="ka-sprite" aria-hidden="true"><defs>
<symbol id="i-monitor" viewBox="0 0 24 24"><rect x="3" y="4" width="18" height="12" rx="2"/><path d="M8 20h8M12 16v4"/></symbol>
<symbol id="i-people" viewBox="0 0 24 24"><circle cx="9" cy="8" r="3.2"/><path d="M3 19c.6-3.2 3-5 6-5s5.4 1.8 6 5"/><circle cx="17" cy="9" r="2.5"/><path d="M16 14.2c2.4.2 4.3 1.8 4.8 4.4"/></symbol>
<symbol id="i-home" viewBox="0 0 24 24"><path d="M4 11l8-6.5 8 6.5"/><path d="M6 9.8V19h12V9.8"/><path d="M10 19v-5h4v5"/></symbol>
<symbol id="i-team" viewBox="0 0 24 24"><rect x="3" y="7" width="18" height="13" rx="2"/><path d="M9 7V5.5A1.5 1.5 0 0 1 10.5 4h3A1.5 1.5 0 0 1 15 5.5V7"/><path d="M3 12.5h18"/></symbol>
<symbol id="i-user" viewBox="0 0 24 24"><circle cx="12" cy="8.5" r="3.5"/><path d="M5 20c.8-3.8 3.6-6 7-6s6.2 2.2 7 6"/></symbol>
<symbol id="i-shield" viewBox="0 0 24 24"><path d="M12 3l7 3v5.5c0 4.4-3 8-7 9.5-4-1.5-7-5.1-7-9.5V6z"/><path d="M9 12l2 2 4-4"/></symbol>
<symbol id="i-key" viewBox="0 0 24 24"><circle cx="8" cy="14" r="4"/><path d="M11 11l8-8M16 6l2 2M14 8l2 2"/></symbol>
<symbol id="i-plus" viewBox="0 0 24 24"><path d="M12 5v14M5 12h14"/></symbol>
<symbol id="i-arrow" viewBox="0 0 24 24"><path d="M5 12h14M13 6l6 6-6 6"/></symbol>
<symbol id="i-back" viewBox="0 0 24 24"><path d="M19 12H5M11 6l-6 6 6 6"/></symbol>
<symbol id="i-check" viewBox="0 0 24 24"><path d="M5 12.5l4.5 4.5L19 7.5"/></symbol>
<symbol id="i-close" viewBox="0 0 24 24"><path d="M6 6l12 12M18 6L6 18"/></symbol>
<symbol id="i-eye" viewBox="0 0 24 24"><path d="M2.5 12S6 5.5 12 5.5 21.5 12 21.5 12 18 18.5 12 18.5 2.5 12 2.5 12z"/><circle cx="12" cy="12" r="3"/></symbol>
<symbol id="i-desk" viewBox="0 0 24 24"><rect x="3" y="4" width="18" height="12" rx="2"/><path d="M7 8h6M7 11h10"/><path d="M9 20h6"/></symbol>
<symbol id="i-pad" viewBox="0 0 24 24"><path d="M6 9h12a3 3 0 0 1 3 3v2a3 3 0 0 1-5.2 2l-1-1H9.2l-1 1A3 3 0 0 1 3 14v-2a3 3 0 0 1 3-3z"/><path d="M8 11v3M6.5 12.5h3M16 12h.01M18 13.5h.01"/></symbol>
<symbol id="i-share" viewBox="0 0 24 24"><circle cx="6" cy="12" r="2.5"/><circle cx="18" cy="6" r="2.5"/><circle cx="18" cy="18" r="2.5"/><path d="M8.3 10.8l7.4-3.6M8.3 13.2l7.4 3.6"/></symbol>
<symbol id="i-gear" viewBox="0 0 24 24"><circle cx="12" cy="12" r="3"/><path d="M12 2.5v3M12 18.5v3M4.2 6.2l2.1 2.1M17.7 15.7l2.1 2.1M2.5 12h3M18.5 12h3M4.2 17.8l2.1-2.1M17.7 8.3l2.1-2.1"/></symbol>
<symbol id="i-info" viewBox="0 0 24 24"><circle cx="12" cy="12" r="9"/><path d="M12 11v5M12 8h.01"/></symbol>
<symbol id="i-log" viewBox="0 0 24 24"><path d="M5 5h14M5 10h14M5 15h9M5 20h6"/></symbol>
<symbol id="i-rocket" viewBox="0 0 24 24"><path d="M5 15c-1 1-1.5 3.5-1.5 5.5 2 0 4.5-.5 5.5-1.5"/><path d="M9 15l-2.5-2.5C8 8 11.5 4 19.5 4.5 20 12.5 16 16 11.5 17.5z"/><circle cx="14.5" cy="9.5" r="1.5"/></symbol>
<symbol id="i-out" viewBox="0 0 24 24"><path d="M15 4h3a2 2 0 0 1 2 2v12a2 2 0 0 1-2 2h-3"/><path d="M10 16l-4-4 4-4M6 12h10"/></symbol>
</defs></svg>"##;

/// Une pastille ronde avec l'initiale ; sa couleur suit le nom.
pub fn avatar(name: &str) -> String {
    let initial = name.trim().chars().next().map(|c| c.to_uppercase().to_string()).unwrap_or_else(|| "•".into());
    let n = name.bytes().fold(0u32, |a, b| a.wrapping_mul(31).wrapping_add(b as u32)) % 6;
    format!("<span class=\"ka-av ka-av-{n}\" aria-hidden=\"true\">{}</span>", esc(&initial))
}

/// « Famille » ou « Équipe ».
pub fn kind_name(lang: Lang, kind: Kind) -> String {
    match kind {
        Kind::Family => t(lang, "Famille", "Family"),
        Kind::Team => t(lang, "Équipe", "Team"),
    }
}

/// « la famille » ou « l'équipe », dans une phrase.
fn kind_the(lang: Lang, kind: Kind) -> String {
    match kind {
        Kind::Family => t(lang, "la famille", "the family"),
        Kind::Team => t(lang, "l'équipe", "the team"),
    }
}

fn kind_class(kind: Kind) -> &'static str {
    match kind {
        Kind::Family => "family",
        Kind::Team => "team",
    }
}

/// La pastille « Famille » ou « Équipe ».
fn kind_pill(lang: Lang, kind: Kind) -> String {
    format!("<span class=\"ka-pill ka-pill-{}\">{}</span>", kind_class(kind), kind_name(lang, kind))
}

/// L'initiale d'une famille ou d'une équipe, dans sa couleur.
fn badge(name: &str, kind: Kind, big: bool) -> String {
    let initial = name.trim().chars().next().map(|c| c.to_uppercase().to_string()).unwrap_or_else(|| "•".into());
    format!("<span class=\"ka-badge ka-badge-{}{}\" aria-hidden=\"true\">{}</span>", kind_class(kind), if big { " ka-badge-lg" } else { "" }, esc(&initial))
}

fn count_word(lang: Lang, n: usize, fr_one: &str, fr_many: &str, en_one: &str, en_many: &str) -> String {
    match lang {
        Lang::Fr => format!("{n} {}", if n > 1 { fr_many } else { fr_one }),
        Lang::En => format!("{n} {}", if n == 1 { en_one } else { en_many }),
    }
}

/// Le menu latéral (et la barre d'onglets du téléphone).
fn side(v: &View<'_>, shell: &Shell, here: &Here) -> (String, String) {
    let q = v.q();
    let cur = |h: &Here| if h == here { " aria-current=\"page\"" } else { "" };
    let count = |n: usize| if n == 0 { String::new() } else { format!("<span class=\"ka-count\">{n}</span>") };
    let orgs: String = shell
        .orgs
        .iter()
        .map(|o| {
            let current = if *here == Here::Org(o.id.clone()) { " aria-current=\"page\"" } else { "" };
            format!(
                "<a class=\"ka-sub\" href=\"/compte/organisation{q}&amp;id={id}\"{current}>{badge}<span>{name}</span></a>",
                id = pct(&o.id),
                badge = badge(&o.name, o.kind, false),
                name = esc(&o.name),
            )
        })
        .collect();
    let families_current = if matches!(here, Here::Families) { " aria-current=\"page\"" } else { "" };
    let n = shell.start.done();
    let starter = if n < 4 {
        format!(
            "<a class=\"ka-starter\" href=\"/compte/ordinateurs{q}#demarrer\"><b>{title}</b><small>{steps}</small><span class=\"ka-bar\"><i class=\"ka-w{n}\"></i></span></a>",
            title = v.tr("Pour bien démarrer", "Getting started"),
            steps = match v.lang {
                Lang::Fr => format!("{n} étape{} sur 4", if n > 1 { "s" } else { "" }),
                Lang::En => format!("{n} of 4 steps"),
            },
        )
    } else {
        String::new()
    };
    let sign_out = v.form(
        "/compte/deconnexion",
        "ka-out",
        &format!("<button class=\"ka-icon-btn\" type=\"submit\" title=\"{l}\" aria-label=\"{l}\">{}</button>", ic("out"), l = v.tr("Se déconnecter", "Sign out")),
    );
    let side = format!(
        "<aside class=\"ka-side\" aria-label=\"{menu}\">\n<a class=\"ka-logo\" href=\"/compte/ordinateurs{q}\"><img src=\"{logo}\" alt=\"KyberNow Share\" width=\"190\" height=\"35\"></a>\n<nav class=\"ka-nav\">\n<a href=\"/compte/ordinateurs{q}\"{c_computers}>{i_monitor}<span>{computers}</span>{n_computers}</a>\n<a href=\"/compte/personnes{q}\"{c_people}>{i_people}<span>{people}</span>{n_people}</a>\n<a href=\"/compte/familles{q}\"{families_current}>{i_home}<span>{families}</span></a>\n{orgs}\n<p class=\"ka-nav-label\">{you}</p>\n<a href=\"/compte{q}\"{c_account}>{i_user}<span>{account}</span></a>\n</nav>\n<div class=\"ka-side-foot\">\n{starter}\n<div class=\"ka-me\">{avatar}<div><b>{name}</b><small>{email}</small></div>{sign_out}</div>\n</div>\n</aside>",
        menu = v.tr("Menu", "Menu"),
        logo = assets::url("kybernow-share-dark.svg"),
        c_computers = cur(&Here::Computers),
        i_monitor = ic("monitor"),
        computers = v.tr("Ordinateurs", "Computers"),
        n_computers = count(shell.computers),
        c_people = cur(&Here::People),
        i_people = ic("people"),
        people = v.tr("Personnes", "People"),
        n_people = count(shell.people),
        i_home = ic("home"),
        families = v.tr("Familles et équipes", "Families and teams"),
        you = v.tr("Vous", "You"),
        c_account = cur(&Here::Account),
        i_user = ic("user"),
        account = v.tr("Compte et sécurité", "Account and security"),
        avatar = avatar(&shell.name),
        name = esc(&shell.name),
        email = esc(&shell.email),
    );
    let in_families = matches!(here, Here::Families | Here::Org(_));
    let tabbar = format!(
        "<nav class=\"ka-tabbar\" aria-label=\"{menu}\">\n<a href=\"/compte/ordinateurs{q}\"{c1}>{i1}<span>{l1}</span></a>\n<a href=\"/compte/personnes{q}\"{c2}>{i2}<span>{l2}</span></a>\n<a href=\"/compte/familles{q}\"{c3}>{i3}<span>{l3}</span></a>\n<a href=\"/compte{q}\"{c4}>{i4}<span>{l4}</span></a>\n</nav>",
        menu = v.tr("Menu", "Menu"),
        c1 = cur(&Here::Computers),
        i1 = ic("monitor"),
        l1 = v.tr("Ordinateurs", "Computers"),
        c2 = cur(&Here::People),
        i2 = ic("people"),
        l2 = v.tr("Personnes", "People"),
        c3 = if in_families { " aria-current=\"page\"" } else { "" },
        i3 = ic("home"),
        l3 = v.tr("Familles", "Families"),
        c4 = cur(&Here::Account),
        i4 = ic("user"),
        l4 = v.tr("Compte", "Account"),
    );
    (side, tabbar)
}

/// Une page de l'espace : le menu, la colonne de contenu (920 px), le pied.
pub fn frame(v: &View<'_>, shell: &Shell, here: Here, title: &str, here_path: &str, body: &str) -> String {
    let (side, tabbar) = side(v, shell, &here);
    let (other, other_name) = match v.lang {
        Lang::Fr => (Lang::En, "English"),
        Lang::En => (Lang::Fr, "Français"),
    };
    let script = if body.contains("data-passkey=\"") { format!("\n<script src=\"{}\" defer></script>", assets::url("passkey.js")) } else { String::new() };
    format!(
        r##"<!doctype html>
<html lang="{lang}">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1, viewport-fit=cover">
<meta name="color-scheme" content="dark">
<meta name="theme-color" content="#05070A">
<meta name="robots" content="noindex">
<link rel="icon" type="image/svg+xml" href="{favicon}">
<link rel="preload" href="{font}" as="font" type="font/woff2" crossorigin>
<link rel="stylesheet" href="{css}">
<title>{title} · KyberNow</title>{script}
</head>
<body class="ka-body">
{sprite}
<div class="ka-app">
{side}
<main class="ka-main">
<div class="ka-page">
{body}
</div>
<footer class="ka-foot"><a href="{here_path}{q_other}" hreflang="{other_code}" lang="{other_code}">{other_name}</a><a href="/conditions{q_lang}">{terms}</a><a href="/confidentialite{q_lang}">{privacy}</a></footer>
</main>
</div>
{tabbar}
</body>
</html>
"##,
        lang = v.lang.code(),
        favicon = assets::url("kybernow-favicon.svg"),
        font = assets::url("archivo.woff2"),
        css = assets::url("share.css"),
        sprite = SPRITE,
        q_other = v.q_lang(other),
        other_code = other.code(),
        q_lang = format!("?lang={}", v.lang.code()),
        terms = v.tr("Conditions", "Terms"),
        privacy = v.tr("Confidentialité", "Privacy"),
    )
}

/// Le titre d'une page : le surtitre, le titre, le chapeau, l'action à droite.
fn head(eyebrow: &str, title: &str, lede: &str, action: &str) -> String {
    let eyebrow = if eyebrow.is_empty() { String::new() } else { format!("<p class=\"ka-eyebrow\">{eyebrow}</p>\n") };
    let lede = if lede.is_empty() { String::new() } else { format!("\n<p class=\"ka-lede\">{lede}</p>") };
    format!("<header class=\"ka-head\"><div>{eyebrow}<h1>{title}</h1>{lede}</div>{action}</header>\n")
}

/// Un encadré d'explication.
fn help_box(icon: &str, html: &str) -> String {
    format!("<div class=\"ka-help\">{}<div>{html}</div></div>\n", ic(icon))
}

/// Un panneau latéral, ouvert par son ancre (`#id`) ; `open` : ouvert d'office (formulaire à corriger).
fn sheet(v: &View<'_>, id: &str, eyebrow: &str, title: &str, body: &str, open: bool) -> String {
    format!(
        "<div class=\"ka-sheet-wrap{open}\" id=\"{id}\">\n<a class=\"ka-scrim\" href=\"#\" aria-label=\"{close}\"></a>\n<section class=\"ka-sheet\" role=\"dialog\" aria-labelledby=\"{id}-titre\">\n<a class=\"ka-sheet-close\" href=\"#\" aria-label=\"{close}\">{x}</a>\n<p class=\"ka-eyebrow\">{eyebrow}</p>\n<h2 id=\"{id}-titre\">{title}</h2>\n{body}\n</section>\n</div>\n",
        open = if open { " is-open" } else { "" },
        close = v.tr("Fermer", "Close"),
        x = ic("close"),
        eyebrow = esc(eyebrow),
    )
}

/// Le nom d'un droit, tel que la console le montre.
pub fn right_name(lang: Lang, r: Right) -> String {
    match r {
        Right::View => t(lang, "Regarder", "Watch"),
        Right::Session => t(lang, "Ouvrir le bureau", "Open the desktop"),
        Right::Lend => t(lang, "Emprunter pour jouer", "Borrow to play"),
        Right::Share => t(lang, "Rejoindre vos partages", "Join your shares"),
        Right::Admin => t(lang, "Administrer", "Administer"),
    }
}

/// Les droits en mots : « Ouvrir le bureau, Rejoindre vos partages ».
pub fn rights_words(lang: Lang, rights: &Rights) -> String {
    rights.iter().map(|r| right_name(lang, r)).collect::<Vec<_>>().join(", ")
}

/// Le nom d'un rôle.
pub fn role_name(lang: Lang, role: Role) -> String {
    match role {
        Role::Owner => t(lang, "Propriétaire", "Owner"),
        Role::Admin => t(lang, "Administrateur", "Administrator"),
        Role::Member => t(lang, "Membre", "Member"),
    }
}

fn right_icon(r: Right) -> &'static str {
    match r {
        Right::View => "eye",
        Right::Session => "desk",
        Right::Lend => "pad",
        Right::Share => "share",
        Right::Admin => "gear",
    }
}

fn right_help(v: &View<'_>, r: Right) -> String {
    match r {
        Right::View => v.tr("Voir l'écran de l'ordinateur, sans clavier, souris ni manette.", "See the computer's screen, with no keyboard, mouse or controller."),
        Right::Session => v.tr("Se connecter à l'ordinateur et l'utiliser comme vous, de n'importe où.", "Connect to the computer and use it like you, from anywhere."),
        Right::Lend => v.tr("Jouer aux apps que vous avez permises dans Admin, quand l'ordinateur est libre.", "Play the apps you allowed in Admin, when the computer is free."),
        Right::Share => v.tr("Entrer dans un partage que vous ouvrez, sans taper de code.", "Join a share you open, without typing a code."),
        Right::Admin => v.tr(
            "Gérer les accès, redémarrer et mettre à jour la Station. Double authentification exigée.",
            "Manage access, restart and update the Station. Two-factor authentication required.",
        ),
    }
}

fn right_example(v: &View<'_>, r: Right) -> String {
    match r {
        Right::View => v.tr("Votre fils qui veut vous montrer sa partie.", "Your son who wants to show you his game."),
        Right::Session => v.tr("Votre conjoint qui reprend ses fichiers depuis le portable.", "Your partner picking up their files from the laptop."),
        Right::Lend => v.tr("Votre neveu qui joue le mercredi après-midi.", "Your nephew playing on Wednesday afternoons."),
        Right::Share => v.tr("Un ami qui vous rejoint pour une partie à deux.", "A friend joining you for a two-player game."),
        Right::Admin => v.tr("La personne qui s'occupe des ordinateurs de l'atelier.", "The person who looks after the studio's computers."),
    }
}

/// Les cinq droits, en tuiles à cocher (`view`, `session`, `lend`, `share`, `admin`).
fn rights_tiles(v: &View<'_>, legend: &str, checked: &Rights, prefix: &str) -> String {
    let tiles: String = Right::ALL
        .iter()
        .map(|r| {
            let on = if checked.has(*r) { " checked" } else { "" };
            format!(
                "<label class=\"ka-right ka-right-pick\"><input type=\"checkbox\" name=\"{name}\" value=\"1\" id=\"{prefix}-{name}\"{on}><span class=\"ka-tick\">{tick}</span>{icon}<b>{label}</b><small>{example}</small></label>\n",
                name = r.as_str(),
                tick = ic("check"),
                icon = ic(right_icon(*r)),
                label = right_name(v.lang, *r),
                example = right_example(v, *r),
            )
        })
        .collect();
    format!("<fieldset class=\"ka-fieldset\">\n<legend>{legend}</legend>\n<div class=\"ka-rights ka-rights-2\">\n{tiles}</div>\n</fieldset>")
}

/// Les droits en étiquettes.
fn rights_tags(v: &View<'_>, rights: &Rights) -> String {
    rights.iter().map(|r| format!("<span class=\"ka-tag\">{}{}</span>", ic(right_icon(r)), right_name(v.lang, r))).collect()
}

fn hidden(name: &str, value: &str) -> String {
    format!("<input type=\"hidden\" name=\"{name}\" value=\"{}\">", esc(value))
}

fn small_button(label: &str) -> String {
    format!("<button class=\"btn btn-ghost btn-sm\" type=\"submit\">{label}</button>")
}

fn state_pill(v: &View<'_>, online: bool) -> String {
    if online {
        format!("<span class=\"ka-pill ka-pill-on\">{}</span>", v.tr("En ligne", "Online"))
    } else {
        format!("<span class=\"ka-pill ka-pill-off\">{}</span>", v.tr("Hors ligne", "Offline"))
    }
}

/// Une ligne de liste : l'icône ou l'avatar, le nom et le détail, ce qu'on y fait.
fn row(lead: &str, name: &str, detail: &str, acts: &str) -> String {
    let detail = if detail.is_empty() { String::new() } else { format!("<small>{detail}</small>") };
    format!("<div class=\"row\">{lead}<div class=\"ka-row-main\"><b>{name}</b>{detail}</div><div class=\"acts\">{acts}</div></div>\n")
}

// --- Pour bien démarrer --------------------------------------------------------------

fn start_card(v: &View<'_>, start: &Start) -> String {
    let n = start.done();
    if n == 4 {
        return String::new();
    }
    let q = v.q();
    let item = |done: bool, i: usize, label: String, href: String| {
        let tick = if done { ic("check") } else { i.to_string() };
        format!("<a class=\"ka-step{}\" href=\"{href}\"><span class=\"ka-step-tick\">{tick}</span><span>{label}</span></a>", if done { " is-done" } else { "" })
    };
    // Un anneau de 4 crans : la longueur du cercle (r = 26) est 163,4.
    let offset = 163.4 * (1.0 - n as f64 / 4.0);
    format!(
        "<section class=\"ka-start\" id=\"demarrer\">\n<div><p class=\"ka-eyebrow\">{eyebrow}</p>\n<h2>{title}</h2>\n<div class=\"ka-steps\">\n{s1}\n{s2}\n{s3}\n{s4}\n</div></div>\n<svg class=\"ka-ring\" viewBox=\"0 0 64 64\" aria-hidden=\"true\"><circle cx=\"32\" cy=\"32\" r=\"26\" class=\"ka-ring-bg\"/><circle cx=\"32\" cy=\"32\" r=\"26\" class=\"ka-ring-on\" stroke-dasharray=\"163.4\" stroke-dashoffset=\"{offset:.1}\" transform=\"rotate(-90 32 32)\"/><text x=\"32\" y=\"37\" text-anchor=\"middle\">{n}/4</text></svg>\n</section>\n",
        eyebrow = v.tr("Pour bien démarrer", "Getting started"),
        title = match v.lang {
            Lang::Fr => format!("Encore {} étape{} pour profiter de tout", 4 - n, if 4 - n > 1 { "s" } else { "" }),
            Lang::En => format!("{} more step{} to make the most of it", 4 - n, if 4 - n > 1 { "s" } else { "" }),
        },
        s1 = item(start.attached, 1, v.tr("Rattacher votre ordinateur", "Attach your computer"), format!("/compte/ordinateurs{q}#rattacher")),
        s2 = item(start.protected, 2, v.tr("Protéger votre compte", "Protect your account"), format!("/compte{q}#securite")),
        s3 = item(start.opened, 3, v.tr("Ouvrir votre ordinateur à quelqu'un", "Open your computer to someone"), format!("/compte/personnes{q}")),
        s4 = item(start.grouped, 4, v.tr("Créer votre famille ou votre équipe", "Create your family or team"), format!("/compte/familles{q}")),
    )
}

// --- Ordinateurs -----------------------------------------------------------------------

/// Un ordinateur, en carte.
#[derive(Debug, Clone)]
pub struct ComputerCard {
    pub id: String,
    pub name: String,
    pub online: bool,
    /// « Vous et 2 accès donnés », « de Léa Martin »…
    pub detail: String,
    /// Ses droits, quand l'ordinateur n'est pas à lui.
    pub rights: Option<Rights>,
    /// Le droit Administrer : le raccourci « Admin ».
    pub admin: bool,
}

/// « Ouvrir Admin » (docs/comptes.md, « Admin depuis KyberNow Share ») : la page du relais qui choisit la voie.
fn admin_href(v: &View<'_>, id: &str) -> String {
    format!("/compte/admin{}&amp;id={}", v.q(), pct(id))
}
#[derive(Debug, Clone, Default)]
pub struct ComputersView {
    pub mine: Vec<ComputerCard>,
    pub opened: Vec<ComputerCard>,
    /// `(identifiant, nom, genre, ordinateurs)` par famille ou équipe.
    pub orgs: Vec<(String, String, Kind, Vec<ComputerCard>)>,
    pub shell: Shell,
}

fn pc_card(v: &View<'_>, c: &ComputerCard) -> String {
    let foot = match &c.rights {
        Some(r) => format!("<span class=\"ka-tags\">{}</span>", rights_tags(v, r)),
        None => format!("<small>{}</small>", esc(&c.detail)),
    };
    let detail = if c.rights.is_some() { format!("<small>{}</small>", esc(&c.detail)) } else { String::new() };
    // La carte entière mène à l'ordinateur ; le raccourci « Admin » passe au-dessus.
    let admin = if c.admin && c.online {
        format!("<a class=\"ka-admin-chip\" href=\"{}\" title=\"{}\">{}Admin</a>", admin_href(v, &c.id), v.tr("Ouvrir Admin", "Open Admin"), ic("gear"))
    } else {
        String::new()
    };
    format!(
        "<div class=\"ka-pc ka-pc-card\"><span class=\"ka-pc-top\"><span class=\"ka-pc-icon\">{icon}</span><span class=\"ka-pc-name\"><a class=\"ka-pc-link\" href=\"/compte/ordinateur{q}&amp;id={id}\"><b>{name}</b></a>{detail}</span>{state}</span><span class=\"ka-pc-foot\">{foot}{admin}</span></div>\n",
        q = v.q(),
        id = pct(&c.id),
        icon = ic("monitor"),
        name = esc(&c.name),
        state = state_pill(v, c.online),
    )
}

fn attach_steps(v: &View<'_>) -> String {
    format!(
        "<ol class=\"ka-points\">\n<li><span class=\"ka-n\">1</span><div>{s1}</div></li>\n<li><span class=\"ka-n\">2</span><div>{s2}</div></li>\n<li><span class=\"ka-n\">3</span><div>{s3}</div></li>\n</ol>\n",
        s1 = v.tr("Sur l'ordinateur, ouvrez <b>Admin</b> : l'icône KyberNow près de l'horloge.", "On the computer, open <b>Admin</b>: the KyberNow icon near the clock."),
        s2 = v.tr("Allez dans <b>Compte KyberNow</b>, puis <b>Se connecter</b>.", "Go to <b>KyberNow account</b>, then <b>Sign in</b>."),
        s3 = v.tr("Connectez-vous avec ce compte et confirmez. L'ordinateur apparaît ici aussitôt.", "Sign in with this account and confirm. The computer shows up here right away."),
    )
}

pub fn computers(v: &View<'_>, c: &ComputersView, notice: Option<Notice>) -> String {
    let title = v.tr("Ordinateurs", "Computers");
    let q = v.q();
    let add = format!(
        "<a class=\"ka-pc-add\" href=\"#rattacher\"><span class=\"ka-plus\">{plus}</span><b>{attach}</b><small>{from}</small></a>\n",
        plus = ic("plus"),
        attach = v.tr("Rattacher un ordinateur", "Attach a computer"),
        from = v.tr("Depuis Admin, sur l'ordinateur", "From Admin, on the computer"),
    );
    let mine: String = c.mine.iter().map(|p| pc_card(v, p)).collect();
    let mut body = format!(
        "<section class=\"ka-section\">\n<div class=\"ka-section-head\"><h2>{h}</h2><small>{n}</small></div>\n<div class=\"ka-grid\">\n{mine}{add}</div>\n</section>\n",
        h = v.tr("À vous", "Yours"),
        n = count_word(v.lang, c.mine.len(), "ordinateur", "ordinateurs", "computer", "computers"),
    );
    let opened = if c.opened.is_empty() {
        help_box(
            "info",
            &v.tr(
                "Quand quelqu'un vous ouvre son ordinateur, il apparaît ici, avec ce que vous pouvez y faire. <b>Pour l'instant, personne ne vous en a ouvert.</b>",
                "When someone opens their computer to you, it shows up here, with what you can do on it. <b>Nobody has opened one to you yet.</b>",
            ),
        )
    } else {
        format!("<div class=\"ka-grid\">\n{}</div>\n", c.opened.iter().map(|p| pc_card(v, p)).collect::<String>())
    };
    body.push_str(&format!("<section class=\"ka-section\">\n<div class=\"ka-section-head\"><h2>{}</h2></div>\n{opened}</section>\n", v.tr("Ouverts pour vous", "Opened for you")));
    for (id, name, kind, list) in &c.orgs {
        let cards: String = list.iter().map(|p| pc_card(v, p)).collect();
        let cards = if cards.is_empty() {
            format!("<p class=\"ka-muted\">{}</p>\n", v.tr("Aucun ordinateur pour l'instant.", "No computers yet."))
        } else {
            format!("<div class=\"ka-grid\">\n{cards}</div>\n")
        };
        body.push_str(&format!(
            "<section class=\"ka-section\">\n<div class=\"ka-section-head\"><h2><a href=\"/compte/organisation{q}&amp;id={id}\">{name}</a></h2>{pill}</div>\n{cards}</section>\n",
            id = pct(id),
            name = esc(name),
            pill = kind_pill(v.lang, *kind),
        ));
    }
    let hello = if c.shell.name.is_empty() {
        String::new()
    } else {
        match v.lang {
            Lang::Fr => t(v.lang, &format!("Bonjour {}", esc(&c.shell.name)), ""),
            Lang::En => format!("Hello {}", esc(&c.shell.name)),
        }
    };
    let page = format!(
        "{head}{notice}{start}{body}{sheet}",
        head = head(
            &hello,
            &format!("<strong>{}</strong>", v.tr("Vos ordinateurs", "Your computers")),
            &v.tr(
                "Ceux qui sont à vous, ceux qu'on vous a ouverts et ceux de vos familles et équipes. Vous les retrouvez dans KyberNow Link, sur tous vos appareils.",
                "Yours, those opened to you and those of your families and teams. You find them in KyberNow Link, on all your devices.",
            ),
            "",
        ),
        notice = notice_html(v.lang, notice),
        start = start_card(v, &c.shell.start),
        sheet = sheet(
            v,
            "rattacher",
            &v.tr("Sur l'ordinateur", "On the computer"),
            &v.tr("Rattacher un ordinateur", "Attach a computer"),
            &format!(
                "{steps}{help}<div class=\"ka-sheet-actions\"><a class=\"btn kn-primary\" href=\"#\">{ok}</a></div>",
                steps = attach_steps(v),
                help = help_box(
                    "info",
                    &v.tr(
                        "Vous installez une nouvelle Station ? L'installateur propose l'étape <b>Compte KyberNow</b> : rien d'autre à faire.",
                        "Installing a new Station? The installer offers the <b>KyberNow account</b> step: nothing else to do.",
                    ),
                ),
                ok = v.tr("Compris", "Got it"),
            ),
            false,
        ),
    );
    frame(v, &c.shell, Here::Computers, &title, "/compte/ordinateurs", &page)
}

// --- Un ordinateur : qui y a accès ------------------------------------------------------

/// Une ligne de « Qui y a accès ».
#[derive(Debug, Clone)]
pub struct PersonRow {
    /// Le droit donné (`None` : le propriétaire, ou les gérants de la famille ou de l'équipe, ou un groupe d'ordinateurs).
    pub grant: Option<i64>,
    /// Le droit vient d'un groupe d'ordinateurs : son nom (« Par le groupe … », pas de « Retirer »).
    pub group: Option<String>,
    pub name: String,
    pub detail: String,
    pub rights: Rights,
}

/// La page d'un ordinateur.
#[derive(Debug, Clone)]
pub struct PeopleView {
    pub id: String,
    pub name: String,
    pub online: bool,
    pub rows: Vec<PersonRow>,
    /// Ses accès se gèrent ici.
    pub manage: bool,
    /// « Détacher » (propriétaire, gérant de la famille ou de l'équipe).
    pub can_detach: bool,
    /// Ses droits à lui, s'il ne gère pas.
    pub my_rights: Rights,
    /// Le droit Administrer lui est donné, sans double authentification.
    pub admin_needs_2fa: bool,
    /// « Ouvrir à » : `(valeur, libellé)` des familles, équipes et groupes possibles.
    pub targets: Vec<(String, String)>,
    /// Le formulaire « Ajouter une personne » tel qu'envoyé (adresse, droits, échéance).
    pub form: (String, Rights, String),
    /// Le droit Administrer : « Ouvrir Admin ».
    pub admin: bool,
    /// Les liens d'accès de l'ordinateur, le plus récent d'abord.
    pub links: Vec<AccessLink>,
    /// Le lien tout juste créé (adresse entière), montré une seule fois.
    pub new_link: Option<String>,
    /// Le formulaire « Créer un lien » tel qu'envoyé (droits, jours, fin de l'accès), et s'il faut le rouvrir.
    pub link_form: (Rights, i64, String),
    pub link_open: bool,
    pub now: i64,
    pub shell: Shell,
}

/// L'état d'un lien d'accès, en mots.
fn link_state(v: &View<'_>, l: &AccessLink, now: i64) -> String {
    match (l.state(now), v.lang) {
        (LinkState::Pending, Lang::Fr) => t(v.lang, &format!("En attente · jusqu'au {}", date(v.lang, l.link_expires_at)), ""),
        (LinkState::Pending, Lang::En) => format!("Pending · until {}", date(v.lang, l.link_expires_at)),
        (LinkState::Used, Lang::Fr) => {
            let at = date(v.lang, l.used_at.unwrap_or(0));
            if l.used_by_name.is_empty() { t(v.lang, &format!("Utilisé le {at}"), "") } else { t(v.lang, &format!("Utilisé par {} le {at}", l.used_by_name), "") }
        }
        (LinkState::Used, Lang::En) => {
            let at = date(v.lang, l.used_at.unwrap_or(0));
            if l.used_by_name.is_empty() { format!("Used on {at}") } else { format!("Used by {} on {at}", l.used_by_name) }
        }
        (LinkState::Expired, _) => v.tr("Expiré", "Expired"),
        (LinkState::Revoked, _) => v.tr("Révoqué", "Revoked"),
    }
}

/// La section « Liens d'accès » d'un ordinateur, et ses panneaux (créer, révoquer).
fn links_section(v: &View<'_>, p: &PeopleView, sheets: &mut String) -> String {
    let fresh = match &p.new_link {
        Some(url) => format!(
            "<section class=\"ka-fresh\">\n<h2>{h}</h2>\n<p class=\"ka-muted\">{once}</p>\n<p class=\"mono-label\">{copy}</p>\n<p class=\"token\">{url}</p>\n</section>\n",
            h = v.tr("Nouveau lien", "New link"),
            once = v.tr("Envoyez-le à une seule personne : il sert une fois et n'est montré qu'ici.", "Send it to one person only: it works once and is only shown here."),
            copy = v.tr("Copier le lien", "Copy the link"),
            url = esc(url),
        ),
        None => String::new(),
    };
    let rows: String = p
        .links
        .iter()
        .map(|l| {
            let state = l.state(p.now);
            let mut detail = vec![esc(&link_state(v, l, p.now)), rights_words(v.lang, &l.rights)];
            if let Some(end) = l.access_expires_at {
                detail.push(match v.lang {
                    Lang::Fr => t(v.lang, &format!("accès jusqu'au {}", date(v.lang, end - 1)), ""),
                    Lang::En => format!("access until {}", date(v.lang, end - 1)),
                });
            }
            let frag = format!("lien-{}", l.id);
            let acts = if matches!(state, LinkState::Pending | LinkState::Used) {
                let used = if state == LinkState::Used {
                    format!(
                        "<p class=\"ka-muted\">{}</p>\n",
                        v.tr(
                            "Révoquer un lien déjà utilisé ne retire pas l'accès : on le retire dans « Qui y a accès ».",
                            "Revoking a link that was already used doesn't remove the access: you remove it in “Who has access”.",
                        )
                    )
                } else {
                    String::new()
                };
                sheets.push_str(&sheet(
                    v,
                    &frag,
                    &p.name,
                    &v.tr("Révoquer ce lien ? Il ne servira plus.", "Revoke this link? It won't work anymore."),
                    &format!(
                        "<p class=\"ka-muted\">{hint}… · {words}</p>\n{used}{form}",
                        hint = esc(&l.hint),
                        words = rights_words(v.lang, &l.rights),
                        form = v.form(
                            "/compte/ordinateur/liens/revoquer",
                            "",
                            &format!(
                                "{}\n{}\n<div class=\"ka-sheet-actions\"><a class=\"btn btn-ghost\" href=\"#\">{}</a><button class=\"btn btn-danger\" type=\"submit\">{}</button></div>",
                                hidden("id", &p.id),
                                hidden("lien", &l.id.to_string()),
                                v.tr("Annuler", "Cancel"),
                                v.tr("Révoquer", "Revoke"),
                            )
                        ),
                    ),
                    false,
                ));
                format!("<a class=\"btn btn-ghost btn-sm\" href=\"#{frag}\">{}</a>", v.tr("Révoquer", "Revoke"))
            } else {
                String::new()
            };
            row(&ic("key"), &format!("{}…", esc(&l.hint)), &detail.join(" · "), &acts)
        })
        .collect();
    let list = if rows.is_empty() { String::new() } else { format!("<div class=\"ka-list\">\n{rows}</div>\n") };
    let (rights, days, until) = &p.link_form;
    let options: String = LINK_DAYS
        .iter()
        .map(|d| {
            let sel = if d == days { " selected" } else { "" };
            let label = match (d, v.lang) {
                (1, _) => v.tr("1 jour", "1 day"),
                (7, Lang::Fr) => "7 jours (proposé)".to_string(),
                (7, Lang::En) => "7 days (suggested)".to_string(),
                (n, Lang::Fr) => format!("{n} jours"),
                (n, Lang::En) => format!("{n} days"),
            };
            format!("<option value=\"{d}\"{sel}>{label}</option>")
        })
        .collect();
    let inner = format!(
        "{id}\n{rights}\n<div class=\"field\"><label class=\"mono-label\" for=\"jours\">{during}</label>\n<select id=\"jours\" name=\"jours\">{options}</select></div>\n{until}\n<div class=\"ka-sheet-actions\"><a class=\"btn btn-ghost\" href=\"#\">{cancel}</a><button class=\"btn kn-primary\" type=\"submit\">{create}</button></div>",
        id = hidden("id", &p.id),
        rights = rights_tiles(v, &v.tr("Ce qu'elle pourra faire", "What they'll be able to do"), rights, "lien"),
        during = v.tr("Le lien sert pendant", "The link works for"),
        until = field(
            &v.tr("Accès jusqu'au (facultatif)", "Access until (optional)"),
            &format!(r#"<input id="lien-fin" name="fin" type="date" value="{}" placeholder="{}">"#, esc(until), v.tr("jj/mm/aaaa", "dd/mm/yyyy")),
        ),
        cancel = v.tr("Annuler", "Cancel"),
        create = v.tr("Créer le lien", "Create the link"),
    );
    sheets.push_str(&sheet(
        v,
        "lien",
        &p.name,
        &v.tr("Créer un lien d'accès", "Create an access link"),
        &format!(
            "<p class=\"ka-muted\">{}</p>\n{}",
            v.tr(
                "Pour ouvrir l'ordinateur à quelqu'un dont vous n'avez pas l'adresse. La personne ouvre le lien, se connecte (ou crée son compte) et accepte : il ne sert qu'une fois.",
                "To open the computer to someone whose address you don't have. They open the link, sign in (or create their account) and accept: it only works once.",
            ),
            v.form("/compte/ordinateur/liens/creer", "", &inner)
        ),
        p.link_open,
    ));
    format!(
        "<section class=\"ka-section\" id=\"liens\">\n<div class=\"ka-section-head\"><h2>{h}</h2><a class=\"btn btn-ghost btn-sm\" href=\"#lien\">{plus}{create}</a></div>\n{fresh}{list}<p class=\"ka-muted ka-after\">{help}</p>\n</section>\n",
        h = v.tr("Liens d'accès", "Access links"),
        plus = ic("plus"),
        create = v.tr("Créer un lien", "Create a link"),
        help = v.tr(
            "Un lien d'accès sert une seule fois. Révoquer un lien déjà utilisé ne retire pas l'accès : on le retire dans « Qui y a accès ».",
            "An access link works only once. Revoking a link that was already used doesn't remove the access: you remove it in “Who has access”.",
        ),
    )
}

pub fn people(v: &View<'_>, p: &PeopleView, notice: Option<Notice>) -> String {
    let q = v.q();
    let rows: String = p
        .rows
        .iter()
        .map(|r| {
            let remove = match (p.manage, r.grant.filter(|_| r.group.is_none())) {
                (true, Some(g)) => v.form(
                    "/compte/ordinateur/retirer",
                    "acct-inline",
                    &format!("{}\n{}\n{}", hidden("id", &p.id), hidden("acces", &g.to_string()), small_button(&v.tr("Retirer", "Remove"))),
                ),
                _ => String::new(),
            };
            let tags = if r.grant.is_none() && r.group.is_none() {
                format!("<span class=\"ka-tag is-lit\">{}</span>", v.tr("Tout", "Everything"))
            } else {
                rights_tags(v, &r.rights)
            };
            row(&avatar(&r.name), &esc(&r.name), &esc(&r.detail), &format!("<span class=\"ka-tags\">{tags}</span>{remove}"))
        })
        .collect();
    let add_button = if p.manage {
        format!("<a class=\"btn kn-primary btn-sm\" href=\"#ajouter\">{}{}</a>", ic("plus"), v.tr("Ajouter une personne", "Add a person"))
    } else {
        String::new()
    };
    let tiles: String = Right::ALL
        .iter()
        .map(|r| format!("<div class=\"ka-right\">{}<b>{}</b><small>{}</small></div>\n", ic(right_icon(*r)), right_name(v.lang, *r), right_help(v, *r)))
        .collect();
    let mut body = format!(
        "<section class=\"ka-section\">\n<div class=\"ka-section-head\"><h2>{who}</h2>{add_button}</div>\n<div class=\"ka-list\">\n{rows}</div>\n<p class=\"ka-muted ka-after\">{lend}</p>\n</section>\n",
        who = v.tr("Qui y a accès", "Who has access"),
        lend = v.tr(
            "Le prêt suit les apps et les créneaux réglés dans Admin, page « Prêt ».",
            "Lending follows the apps and time slots set in Admin, on the \"Lending\" page.",
        ),
    );
    let mut sheets = String::new();
    if p.manage {
        body.push_str(&links_section(v, p, &mut sheets));
    }
    if !p.manage {
        let mine = if p.my_rights.is_empty() { format!("<span class=\"ka-tag\">{}</span>", v.tr("aucun", "none")) } else { rights_tags(v, &p.my_rights) };
        body.push_str(&format!("<section class=\"ka-section\">\n<div class=\"ka-section-head\"><h2>{}</h2></div>\n<p class=\"ka-tags\">{mine}</p>\n</section>\n", v.tr("Ce que vous pouvez y faire", "What you can do on it")));
    }
    if p.admin_needs_2fa {
        body.push_str(&format!(
            "<p class=\"notice\" role=\"status\">{} <a href=\"/compte/double-authentification{q}\">{}</a></p>\n",
            v.tr(
                "Le droit Administrer vous est donné, mais il ne compte qu'avec la double authentification.",
                "The Administer right is given to you, but it only counts with two-factor authentication.",
            ),
            v.tr("L'activer", "Turn it on"),
        ));
    }
    body.push_str(&format!(
        "<section class=\"ka-section\">\n<div class=\"ka-section-head\"><h2>{h}</h2><small>{sub}</small></div>\n<div class=\"ka-rights\">\n{tiles}</div>\n</section>\n",
        h = v.tr("Ce que chaque droit permet", "What each right allows"),
        sub = v.tr("Quatre droits, à combiner", "Four rights, to combine"),
    ));
    let mut on_pc = String::new();
    if p.manage && !p.targets.is_empty() {
        on_pc.push_str(&row(
            &ic("home"),
            &v.tr("Ouvrir à une famille ou une équipe", "Open to a family or team"),
            &v.tr("Toute la famille ou l'équipe, ou l'un de ses groupes.", "The whole family or team, or one of its groups."),
            &format!("<a class=\"btn btn-ghost btn-sm\" href=\"#ouvrir\">{}</a>", v.tr("Ouvrir", "Open")),
        ));
    }
    if p.can_detach {
        on_pc.push_str(&row(
            &ic("close"),
            &v.tr("Détacher de votre compte", "Detach from your account"),
            &v.tr(
                "Les personnes ajoutées perdent leur accès. Le réseau local et l'appairage restent.",
                "The people added lose their access. The local network and pairing stay.",
            ),
            &v.form(
                "/compte/ordinateurs/detacher",
                "acct-inline",
                &format!("{}\n{}\n<button class=\"btn btn-danger btn-sm\" type=\"submit\">{}</button>", hidden("id", &p.id), hidden("depuis", "console"), v.tr("Détacher", "Detach")),
            ),
        ));
    }
    if !on_pc.is_empty() {
        body.push_str(&format!("<section class=\"ka-section\">\n<div class=\"ka-section-head\"><h2>{}</h2></div>\n<div class=\"ka-list\">\n{on_pc}</div>\n</section>\n", v.tr("Réglages", "Settings")));
    }
    if p.manage {
        let (email, rights, until) = &p.form;
        let inner = format!(
            "{id}\n{email}\n{rights}\n{until}\n<div class=\"ka-sheet-actions\"><a class=\"btn btn-ghost\" href=\"#\">{cancel}</a><button class=\"btn kn-primary\" type=\"submit\">{send}</button></div>",
            id = hidden("id", &p.id),
            email = field(
                &v.tr("Son adresse e-mail", "Their email address"),
                &format!(
                    r#"<input id="email" name="email" type="email" inputmode="email" autocomplete="off" autocapitalize="off" spellcheck="false" maxlength="254" placeholder="lea@exemple.fr" value="{}" required>"#,
                    esc(email)
                ),
            ),
            rights = rights_tiles(v, &v.tr("Ce qu'elle peut faire", "What they can do"), rights, "ajout"),
            until = field(
                &v.tr("Jusqu'au (facultatif)", "Until (optional)"),
                &format!(r#"<input id="fin" name="fin" type="date" value="{}" placeholder="{}">"#, esc(until), v.tr("jj/mm/aaaa", "dd/mm/yyyy")),
            ),
            cancel = v.tr("Annuler", "Cancel"),
            send = v.tr("Envoyer l'invitation", "Send the invitation"),
        );
        let failed = !email.is_empty() && notice.is_some_and(|n| !n.is_ok());
        sheets.push_str(&sheet(
            v,
            "ajouter",
            &p.name,
            &v.tr("Ajouter une personne", "Add a person"),
            &format!(
                "<p class=\"ka-muted\">{}</p>\n{}",
                v.tr(
                    "Avec un compte KyberNow, l'ordinateur apparaît tout de suite chez elle. Sinon, l'e-mail lui propose d'en créer un. Ajouter la même adresse change ses droits.",
                    "With a KyberNow account, the computer shows up for them right away. Otherwise, the email offers to create one. Adding the same address changes their rights.",
                ),
                v.form("/compte/ordinateur/ajouter", "", &inner)
            ),
            failed,
        ));
        if !p.targets.is_empty() {
            let options: String = p.targets.iter().map(|(val, label)| format!("<option value=\"{}\">{}</option>", esc(val), esc(label))).collect();
            let inner = format!(
                "{id}\n<div class=\"field\"><label class=\"mono-label\" for=\"a\">{to}</label>\n<select id=\"a\" name=\"a\" required>{options}</select></div>\n{rights}\n<div class=\"ka-sheet-actions\"><a class=\"btn btn-ghost\" href=\"#\">{cancel}</a><button class=\"btn kn-primary\" type=\"submit\">{open}</button></div>",
                id = hidden("id", &p.id),
                to = v.tr("À qui", "To whom"),
                rights = rights_tiles(v, &v.tr("Ce qu'ils peuvent faire", "What they can do"), &Rights::new([Right::Session]), "ouvrir"),
                cancel = v.tr("Annuler", "Cancel"),
                open = v.tr("Ouvrir", "Open"),
            );
            sheets.push_str(&sheet(
                v,
                "ouvrir",
                &p.name,
                &v.tr("Ouvrir à une famille ou une équipe", "Open to a family or team"),
                &format!(
                    "<p class=\"ka-muted\">{}</p>\n{}",
                    v.tr(
                        "Chaque membre, et chacun de ceux qui arriveront ensuite, reçoit ces droits. Ouvrir de nouveau change les droits.",
                        "Each member, and everyone who joins later, gets these rights. Opening again changes the rights.",
                    ),
                    v.form("/compte/ordinateur/ouvrir", "", &inner)
                ),
                false,
            ));
        }
    }
    let page = format!(
        "<nav class=\"ka-crumbs\"><a href=\"/compte/ordinateurs{q}\">{computers}</a><span>/</span><span>{name}</span></nav>\n<div class=\"ka-pc ka-pc-head\"><span class=\"ka-pc-top\"><span class=\"ka-pc-icon\">{icon}</span><span class=\"ka-pc-name\"><b>{name}</b><small>{whose}</small></span>{state}</span></div>\n{notice}{body}{sheets}",
        computers = v.tr("Ordinateurs", "Computers"),
        name = esc(&p.name),
        icon = ic("monitor"),
        whose = if p.manage { v.tr("Vous gérez ses accès", "You manage its access") } else { v.tr("Ouvert pour vous", "Opened for you") },
        state = if p.admin && p.online {
            format!("{}<a class=\"btn btn-ghost btn-sm\" href=\"{}\">{}{}</a>", state_pill(v, p.online), admin_href(v, &p.id), ic("gear"), v.tr("Ouvrir Admin", "Open Admin"))
        } else {
            state_pill(v, p.online)
        },
        notice = notice_html(v.lang, notice),
    );
    frame(v, &p.shell, Here::Computers, &p.name, "/compte/ordinateur", &page)
}

// --- Personnes -------------------------------------------------------------------------

/// Une personne (ou une famille, une équipe, un groupe) qui a accès à l'un de ses ordinateurs.
#[derive(Debug, Clone)]
pub struct Granted {
    pub name: String,
    pub detail: String,
    pub computer_id: String,
    pub computer: String,
    pub rights: Rights,
}

/// La page « Personnes ».
#[derive(Debug, Clone, Default)]
pub struct AllPeopleView {
    pub rows: Vec<Granted>,
    /// Ses ordinateurs, pour « Ajouter une personne » : `(identifiant, nom)`.
    pub computers: Vec<(String, String)>,
    pub shell: Shell,
}

pub fn all_people(v: &View<'_>, a: &AllPeopleView) -> String {
    let title = v.tr("Personnes", "People");
    let q = v.q();
    let action = match a.computers.as_slice() {
        [] => format!("<a class=\"btn kn-primary\" href=\"/compte/ordinateurs{q}#rattacher\">{}{}</a>", ic("plus"), v.tr("Rattacher un ordinateur", "Attach a computer")),
        [(id, _)] => format!("<a class=\"btn kn-primary\" href=\"/compte/ordinateur{q}&amp;id={}#ajouter\">{}{}</a>", pct(id), ic("plus"), v.tr("Ajouter une personne", "Add a person")),
        _ => format!("<a class=\"btn kn-primary\" href=\"#choisir\">{}{}</a>", ic("plus"), v.tr("Ajouter une personne", "Add a person")),
    };
    let list = if a.computers.is_empty() {
        help_box(
            "monitor",
            &v.tr(
                "Pour ouvrir un ordinateur à quelqu'un, <b>rattachez-le d'abord</b> à votre compte, ou à une famille ou une équipe que vous gérez.",
                "To open a computer to someone, <b>attach it first</b> to your account, or to a family or team you manage.",
            ),
        )
    } else if a.rows.is_empty() {
        help_box(
            "people",
            &v.tr(
                "Personne d'autre que vous n'a accès à vos ordinateurs. <b>Ajoutez une personne</b> par son adresse e-mail et choisissez ce qu'elle peut faire.",
                "Nobody but you has access to your computers. <b>Add a person</b> by their email address and choose what they can do.",
            ),
        )
    } else {
        let rows: String = a
            .rows
            .iter()
            .map(|g| {
                let detail = format!("<a href=\"/compte/ordinateur{q}&amp;id={}\">{}</a>{}", pct(&g.computer_id), esc(&g.computer), if g.detail.is_empty() { String::new() } else { format!(" · {}", esc(&g.detail)) });
                row(&avatar(&g.name), &esc(&g.name), &detail, &format!("<span class=\"ka-tags\">{}</span>", rights_tags(v, &g.rights)))
            })
            .collect();
        format!("<div class=\"ka-list\">\n{rows}</div>\n")
    };
    let choose = if a.computers.len() > 1 {
        let links: String = a
            .computers
            .iter()
            .map(|(id, name)| row(&ic("monitor"), &esc(name), "", &format!("<a class=\"btn btn-ghost btn-sm\" href=\"/compte/ordinateur{q}&amp;id={}#ajouter\">{}</a>", pct(id), v.tr("Choisir", "Choose"))))
            .collect();
        sheet(v, "choisir", &v.tr("Ajouter une personne", "Add a person"), &v.tr("Sur quel ordinateur ?", "On which computer?"), &format!("<div class=\"ka-list\">\n{links}</div>"), false)
    } else {
        String::new()
    };
    let page = format!(
        "{head}<section class=\"ka-section\">\n{list}</section>\n<section class=\"ka-section\">\n{help}</section>\n{choose}",
        head = head(
            "",
            &format!("<strong>{title}</strong>"),
            &v.tr(
                "Tous ceux à qui vous avez ouvert un ordinateur, seuls ou par une famille ou une équipe.",
                "Everyone you opened a computer to, on their own or through a family or team.",
            ),
            &action,
        ),
        help = help_box(
            "info",
            &v.tr(
                "La personne reçoit un e-mail. <b>Si elle a déjà un compte KyberNow</b>, l'ordinateur apparaît tout de suite dans son app. Sinon, le lien de l'e-mail lui propose d'en créer un. Vous pouvez retirer un accès à tout moment, depuis la page de l'ordinateur.",
                "They get an email. <b>If they already have a KyberNow account</b>, the computer shows up in their app right away. Otherwise, the link in the email offers to create one. You can remove an access at any time, from the computer's page.",
            ),
        ),
    );
    frame(v, &a.shell, Here::People, &title, "/compte/personnes", &page)
}

// --- Familles et équipes ---------------------------------------------------------------

/// Une famille ou une équipe, sur « Familles et équipes ».
#[derive(Debug, Clone)]
pub struct OrgCard {
    pub id: String,
    pub name: String,
    pub kind: Kind,
    pub role: Role,
    pub members: usize,
    pub computers: usize,
}

/// La page « Familles et équipes ».
#[derive(Debug, Clone, Default)]
pub struct FamiliesView {
    pub orgs: Vec<OrgCard>,
    pub shell: Shell,
}

/// Les deux cartes « Une famille », « Une équipe », vers la création.
fn kind_cards(v: &View<'_>) -> String {
    let q = v.q();
    format!(
        "<div class=\"ka-kinds\">\n<a class=\"ka-kind ka-kind-family\" href=\"/compte/organisations/creer{q}&amp;genre=famille\"><span class=\"ka-halo\"></span><span class=\"ka-kind-icon\">{home}</span><h3>{fam}</h3><p>{fam_text}</p><ul><li>{f1}</li><li>{f2}</li><li>{f3}</li></ul></a>\n<a class=\"ka-kind ka-kind-team\" href=\"/compte/organisations/creer{q}&amp;genre=equipe\"><span class=\"ka-halo\"></span><span class=\"ka-kind-icon\">{team}</span><h3>{tm}</h3><p>{tm_text}</p><ul><li>{t1}</li><li>{t2}</li><li>{t3}</li></ul></a>\n</div>\n",
        home = ic("home"),
        fam = v.tr("Une famille", "A family"),
        fam_text = v.tr("Pour la maison : les parents, les enfants, les grands-parents.", "For home: parents, children, grandparents."),
        f1 = v.tr("Le PC du salon pour toute la famille", "The living room PC for the whole family"),
        f2 = v.tr("Un groupe « Les enfants » qui peut emprunter pour jouer", "A \"Kids\" group that can borrow to play"),
        f3 = v.tr("Simple : vous gérez, les autres utilisent", "Simple: you manage, the others use"),
        team = ic("team"),
        tm = v.tr("Une équipe", "A team"),
        tm_text = v.tr("Pour un atelier, une association, une entreprise.", "For a studio, an association, a company."),
        t1 = v.tr("Des rôles : propriétaire, administrateur, membre", "Roles: owner, administrator, member"),
        t2 = v.tr("Des groupes, comme « Graphistes » ou « Montage »", "Groups, like \"Designers\" or \"Editing\""),
        t3 = v.tr("Un journal de qui a ouvert quoi, des Stations à déployer", "A log of who opened what, Stations to deploy"),
    )
}

pub fn families(v: &View<'_>, f: &FamiliesView) -> String {
    let title = v.tr("Familles et équipes", "Families and teams");
    let q = v.q();
    let how = format!(
        "<div class=\"ka-how\" role=\"img\" aria-label=\"{aria}\">\n<div class=\"ka-how-col\"><p class=\"ka-eyebrow\">{people}</p><span class=\"ka-chip\">{a1}Léa</span><span class=\"ka-chip\">{a2}Tom</span></div>\n<div class=\"ka-flow\"></div>\n<div class=\"ka-how-col\"><p class=\"ka-eyebrow\">{group}</p><div class=\"ka-how-group\"><b>{kids}</b>{in_family}</div></div>\n<div class=\"ka-flow\"></div>\n<div class=\"ka-how-col\"><p class=\"ka-eyebrow\">{computers}</p><span class=\"ka-chip ka-chip-pc\">{monitor}{pc}</span><small>{lend}</small></div>\n</div>\n<p class=\"ka-muted ka-after\">{text}</p>\n",
        aria = v.tr("Des personnes, réunies dans un groupe, ouvrent des ordinateurs.", "People, gathered in a group, open computers."),
        people = v.tr("Des personnes", "People"),
        a1 = avatar("Léa"),
        a2 = avatar("Tom"),
        group = v.tr("Un groupe", "A group"),
        kids = v.tr("Les enfants", "The kids"),
        in_family = v.tr("dans la famille", "in the family"),
        computers = v.tr("Vos ordinateurs", "Your computers"),
        monitor = ic("monitor"),
        pc = v.tr("PC du salon", "Living room PC"),
        lend = v.tr("Emprunter pour jouer", "Borrow to play"),
        text = v.tr(
            "Ajoutez Léa au groupe, et elle peut emprunter le PC du salon. Retirez-la, et l'accès disparaît. Un groupe n'est pas obligatoire : vous pouvez aussi ouvrir un ordinateur à toute la famille ou à toute l'équipe.",
            "Add Léa to the group, and she can borrow the living room PC. Remove her, and the access is gone. A group isn't required: you can also open a computer to the whole family or team.",
        ),
    );
    let mine = if f.orgs.is_empty() {
        String::new()
    } else {
        let cards: String = f
            .orgs
            .iter()
            .map(|o| {
                let detail = format!(
                    "{} · {} · {}",
                    kind_name(v.lang, o.kind),
                    count_word(v.lang, o.members, "membre", "membres", "member", "members"),
                    count_word(v.lang, o.computers, "ordinateur", "ordinateurs", "computer", "computers"),
                );
                format!(
                    "<a class=\"ka-org\" href=\"/compte/organisation{q}&amp;id={id}\">{badge}<span class=\"ka-row-main\"><b>{name}</b><small>{detail}</small></span><span class=\"ka-pill\">{role}</span>{arrow}</a>\n",
                    id = pct(&o.id),
                    badge = badge(&o.name, o.kind, true),
                    name = esc(&o.name),
                    role = role_name(v.lang, o.role),
                    arrow = ic("arrow"),
                )
            })
            .collect();
        format!("<section class=\"ka-section\">\n<div class=\"ka-section-head\"><h2>{}</h2></div>\n<div class=\"ka-orgs\">\n{cards}</div>\n</section>\n", v.tr("Les vôtres", "Yours"))
    };
    let page = format!(
        "{head}<section class=\"ka-section\">\n<div class=\"ka-section-head\"><h2>{how_h}</h2></div>\n{how}</section>\n{mine}<section class=\"ka-section\">\n<div class=\"ka-section-head\"><h2>{create_h}</h2></div>\n{kinds}</section>\n",
        head = head(
            "",
            &format!("<strong>{title}</strong>"),
            &v.tr(
                "Réunissez des personnes pour leur ouvrir vos ordinateurs d'un seul coup, au lieu de les ajouter une par une.",
                "Gather people to open your computers to them in one go, instead of adding them one by one.",
            ),
            "",
        ),
        how_h = v.tr("Comment ça marche", "How it works"),
        create_h = v.tr("En créer une", "Create one"),
        kinds = kind_cards(v),
    );
    frame(v, &f.shell, Here::Families, &title, "/compte/familles", &page)
}

/// « Créer votre famille » ou « Créer votre équipe » ; sans genre : le choix.
pub fn create_org(v: &View<'_>, name: &str, kind: Option<Kind>, two_factor: bool, shell: &Shell, notice: Option<Notice>) -> String {
    let q = v.q();
    let Some(kind) = kind else {
        let title = v.tr("Créer une famille ou une équipe", "Create a family or team");
        let page = format!(
            "<nav class=\"ka-crumbs\"><a href=\"/compte/familles{q}\">{families}</a><span>/</span><span>{create}</span></nav>\n{head}{notice}{kinds}",
            families = v.tr("Familles et équipes", "Families and teams"),
            create = v.tr("Créer", "Create"),
            head = head("", &format!("<strong>{title}</strong>"), &v.tr("Les deux ouvrent vos ordinateurs à plusieurs personnes. Choisissez les mots qui vous ressemblent.", "Both open your computers to several people. Choose the words that suit you."), ""),
            notice = notice_html(v.lang, notice),
            kinds = kind_cards(v),
        );
        return frame(v, shell, Here::Families, &title, "/compte/organisations/creer", &page);
    };
    let family = kind == Kind::Family;
    let title = if family { v.tr("Créer votre famille", "Create your family") } else { v.tr("Créer votre équipe", "Create your team") };
    let body = if two_factor {
        let inner = format!(
            "{genre}\n{name}\n<div class=\"ka-form-actions\"><a class=\"btn btn-ghost\" href=\"/compte/familles{q}\">{cancel}</a><button class=\"btn kn-primary\" type=\"submit\">{create}</button></div>",
            genre = hidden("genre", if family { "famille" } else { "equipe" }),
            name = field(
                &if family { v.tr("Nom de la famille", "Family name") } else { v.tr("Nom de l'équipe", "Team name") },
                &format!(
                    r#"<input id="nom" name="nom" maxlength="64" placeholder="{}" value="{}" required>"#,
                    if family { v.tr("Famille Martin", "The Martins") } else { v.tr("L'atelier", "The studio") },
                    esc(name)
                ),
            ),
            cancel = v.tr("Annuler", "Cancel"),
            create = if family { v.tr("Créer la famille", "Create the family") } else { v.tr("Créer l'équipe", "Create the team") },
        );
        v.form("/compte/organisations/creer", "ka-form", &inner)
    } else {
        format!(
            "<p class=\"notice\" role=\"status\">{} <a href=\"/compte{q}#securite\">{}</a></p>",
            v.tr(
                "Pour gérer une famille ou une équipe, protégez d'abord votre compte : une passkey, ou la double authentification.",
                "To manage a family or team, protect your account first: a passkey, or two-factor authentication.",
            ),
            v.tr("Protéger mon compte", "Protect my account"),
        )
    };
    let page = format!(
        "<nav class=\"ka-crumbs\"><a href=\"/compte/familles{q}\">{families}</a><span>/</span><span>{title}</span></nav>\n<div class=\"ka-create ka-create-{class}\"><span class=\"ka-halo\"></span><span class=\"ka-kind-icon\">{icon}</span>{head}</div>\n{notice}<section class=\"ka-section\">\n{body}\n</section>\n<section class=\"ka-section\">\n{help}</section>\n",
        families = v.tr("Familles et équipes", "Families and teams"),
        class = kind_class(kind),
        icon = ic(if family { "home" } else { "team" }),
        head = head(
            &kind_name(v.lang, kind),
            &format!("<strong>{title}</strong>"),
            &if family {
                v.tr("Ajoutez ensuite vos proches, puis ouvrez-leur vos ordinateurs en une fois.", "Then add your loved ones, and open your computers to them in one go.")
            } else {
                v.tr("Invitez ensuite vos collègues, créez des groupes et rattachez les ordinateurs de l'équipe.", "Then invite your colleagues, create groups and attach the team's computers.")
            },
            "",
        ),
        notice = notice_html(v.lang, notice),
        help = help_box(
            "shield",
            &v.tr(
                "Ceux qui gèrent une famille ou une équipe (propriétaires et administrateurs) protègent leur compte par une <b>passkey</b> ou la <b>double authentification</b>.",
                "Those who manage a family or team (owners and administrators) protect their account with a <b>passkey</b> or <b>two-factor authentication</b>.",
            ),
        ),
    );
    frame(v, shell, Here::Families, &title, "/compte/organisations/creer", &page)
}

// --- Une famille ou une équipe ----------------------------------------------------------

/// L'onglet de la page d'une famille ou d'une équipe.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Members,
    Groups,
    Computers,
    Log,
}

impl Tab {
    pub fn parse(s: &str) -> Tab {
        match s {
            "groupes" => Tab::Groups,
            "ordinateurs" => Tab::Computers,
            "journal" => Tab::Log,
            _ => Tab::Members,
        }
    }

    fn word(self) -> &'static str {
        match self {
            Tab::Members => "membres",
            Tab::Groups => "groupes",
            Tab::Computers => "ordinateurs",
            Tab::Log => "journal",
        }
    }
}

/// Un ordinateur de la famille ou de l'équipe.
#[derive(Debug, Clone)]
pub struct OrgComputer {
    pub id: String,
    pub name: String,
    pub online: bool,
    /// Ses accès donnés, en mots (« Groupe Graphistes : Ouvrir le bureau »).
    pub access: String,
    /// Le droit Administrer : « Admin ».
    pub admin: bool,
}

/// La page d'une famille ou d'une équipe.
#[derive(Debug, Clone)]
pub struct OrgView {
    pub id: String,
    pub name: String,
    pub kind: Kind,
    pub role: Role,
    /// Propriétaire ou Administrateur, avec la double authentification.
    pub manage: bool,
    /// Propriétaire ou Administrateur, sans la double authentification.
    pub needs_2fa: bool,
    pub tab: Tab,
    pub me: i64,
    pub members: Vec<Member>,
    pub invites: Vec<Invite>,
    pub groups: Vec<Group>,
    pub computers: Vec<OrgComputer>,
    /// Les groupes d'ordinateurs.
    pub station_groups: Vec<StationGroup>,
    /// « Ouvrir ce groupe » : `(valeur, libellé)` de toute la famille ou l'équipe, ses groupes, ses membres.
    pub group_targets: Vec<(String, String)>,
    pub log: Vec<LogLine>,
    pub now: i64,
    pub shell: Shell,
}

/// Un formulaire qui revient sur un panneau (l'ancre suit la réponse).
fn form_at(v: &View<'_>, path: &str, frag: &str, class: &str, inner: &str) -> String {
    let q = v.q();
    v.form(path, class, inner).replacen(&format!("action=\"{path}{q}\""), &format!("action=\"{path}{q}#{frag}\""), 1)
}

/// À qui un groupe d'ordinateurs est ouvert, en mots.
fn group_grant_who(v: &View<'_>, subject: &super::super::people::Subject, kind: Kind) -> String {
    use super::super::people::Subject;
    match subject {
        Subject::Org { .. } => match kind {
            Kind::Family => v.tr("Toute la famille", "The whole family"),
            Kind::Team => v.tr("Toute l'équipe", "The whole team"),
        },
        Subject::Group { name, .. } => match v.lang {
            Lang::Fr => t(v.lang, &format!("Groupe {name}"), ""),
            Lang::En => format!("Group {name}"),
        },
        Subject::Person { name, email, .. } => if name.is_empty() { email.clone() } else { name.clone() },
        Subject::Invite { email } => email.clone(),
    }
}

/// L'onglet « Ordinateurs » : les groupes d'ordinateurs, leurs panneaux.
fn station_groups_section(v: &View<'_>, o: &OrgView, sheets: &mut String) -> String {
    if o.station_groups.is_empty() && !o.manage {
        return String::new();
    }
    let id_field = hidden("id", &o.id);
    let cards: String = o
        .station_groups
        .iter()
        .map(|g| {
            let frag = format!("ordinateurs-{}", g.id);
            let chips = if g.stations.is_empty() {
                format!("<p class=\"ka-muted ka-group-empty\">{}</p>", v.tr("Aucun ordinateur pour l'instant.", "No computers yet."))
            } else {
                let chips: String = g.stations.iter().map(|(_, name)| format!("<span class=\"ka-chip\">{}{}</span>", ic("monitor"), esc(name))).collect();
                format!("<div class=\"ka-chips\">{chips}</div>")
            };
            let opened: Vec<String> = g
                .grants
                .iter()
                .map(|gr| {
                    let mut line = format!("{} : {}", esc(&group_grant_who(v, &gr.subject, o.kind)), rights_words(v.lang, &gr.rights));
                    if let Some(end) = gr.expires_at {
                        line.push_str(&match v.lang {
                            Lang::Fr => format!(" · jusqu'au {}", date(v.lang, end - 1)),
                            Lang::En => format!(" · until {}", date(v.lang, end - 1)),
                        });
                    }
                    if gr.expired(o.now) {
                        line.push_str(&format!(" · {}", v.tr("échu", "expired")));
                    }
                    t(v.lang, &line, &line.replace(" : ", ": "))
                })
                .collect();
            let opened = if opened.is_empty() {
                format!("<p class=\"ka-muted\">{}</p>", v.tr("Ouvert à personne pour l'instant.", "Not opened to anyone yet."))
            } else {
                format!("<p class=\"ka-muted\">{}</p>", opened.join("<br>"))
            };
            let foot = if o.manage {
                format!("<div class=\"ka-group-acts\"><a class=\"btn btn-ghost btn-sm\" href=\"#{frag}\">{}{}</a></div>", ic("gear"), v.tr("Gérer le groupe", "Manage the group"))
            } else {
                String::new()
            };
            if o.manage {
                let rows: String = o
                    .computers
                    .iter()
                    .map(|c| {
                        let inside = g.stations.iter().any(|(id, _)| *id == c.id);
                        let state = if inside { format!("<span class=\"ka-tag is-lit\">{}</span>", v.tr("Dans le groupe", "In the group")) } else { String::new() };
                        let form = form_at(
                            v,
                            "/compte/organisation/groupes-ordinateurs/ordinateur",
                            &frag,
                            "acct-inline",
                            &format!(
                                "{id_field}\n{}\n{}\n{}\n<button class=\"btn {} btn-sm\" type=\"submit\">{}</button>",
                                hidden("groupe", &g.id.to_string()),
                                hidden("ordinateur", &c.id),
                                hidden("action", if inside { "retirer" } else { "ajouter" }),
                                if inside { "btn-ghost" } else { "kn-primary" },
                                if inside { v.tr("Retirer", "Remove") } else { v.tr("Ajouter", "Add") },
                            ),
                        );
                        row(&ic("monitor"), &esc(&c.name), "", &format!("{state}{form}"))
                    })
                    .collect();
                let rows = if rows.is_empty() { format!("<p class=\"ka-muted\">{}</p>\n", v.tr("Aucun ordinateur pour l'instant.", "No computers yet.")) } else { format!("<div class=\"ka-list\">\n{rows}</div>\n") };
                let grants: String = g
                    .grants
                    .iter()
                    .map(|gr| {
                        let mut detail = vec![rights_words(v.lang, &gr.rights)];
                        if let Some(end) = gr.expires_at {
                            detail.push(match v.lang {
                                Lang::Fr => t(v.lang, &format!("jusqu'au {}", date(v.lang, end - 1)), ""),
                                Lang::En => format!("until {}", date(v.lang, end - 1)),
                            });
                        }
                        let remove = form_at(
                            v,
                            "/compte/organisation/groupes-ordinateurs/retirer",
                            &frag,
                            "acct-inline",
                            &format!("{id_field}\n{}\n{}\n{}", hidden("groupe", &g.id.to_string()), hidden("acces", &gr.id.to_string()), small_button(&v.tr("Retirer", "Remove"))),
                        );
                        let who = group_grant_who(v, &gr.subject, o.kind);
                        row(&avatar(&who), &esc(&who), &esc(&detail.join(" · ")), &remove)
                    })
                    .collect();
                let grants = if grants.is_empty() { String::new() } else { format!("<div class=\"ka-list\">\n{grants}</div>\n") };
                let options: String = o.group_targets.iter().map(|(val, label)| format!("<option value=\"{}\">{}</option>", esc(val), esc(label))).collect();
                let open = form_at(
                    v,
                    "/compte/organisation/groupes-ordinateurs/ouvrir",
                    &frag,
                    "",
                    &format!(
                        "{id_field}\n{grp}\n<div class=\"field\"><label class=\"mono-label\" for=\"a-{gid}\">{to}</label>\n<select id=\"a-{gid}\" name=\"a\" required>{options}</select></div>\n{rights}\n{until}\n<div class=\"ka-sheet-actions\"><button class=\"btn kn-primary\" type=\"submit\">{go}</button></div>",
                        grp = hidden("groupe", &g.id.to_string()),
                        gid = g.id,
                        to = v.tr("À qui", "To whom"),
                        rights = rights_tiles(v, &v.tr("Ce qu'ils peuvent faire", "What they can do"), &Rights::new([Right::Session]), &format!("groupe-{}", g.id)),
                        until = field(
                            &v.tr("Jusqu'au (facultatif)", "Until (optional)"),
                            &format!(r#"<input id="fin-{}" name="fin" type="date" placeholder="{}">"#, g.id, v.tr("jj/mm/aaaa", "dd/mm/yyyy")),
                        ),
                        go = v.tr("Ouvrir le groupe", "Open the group"),
                    ),
                );
                let title = match v.lang {
                    Lang::Fr => t(v.lang, &format!("Le groupe « {} »", esc(&g.name)), ""),
                    Lang::En => format!("The \"{}\" group", esc(&g.name)),
                };
                sheets.push_str(&sheet(
                    v,
                    &frag,
                    &o.name,
                    &title,
                    &format!(
                        "<p class=\"ka-muted\">{help}</p>\n<h3 class=\"mono-label ka-after\">{computers}</h3>\n{rows}<h3 class=\"mono-label ka-after\">{who}</h3>\n{grants}<h3 class=\"mono-label ka-after\">{open_h}</h3>\n{open}\n<div class=\"ka-sheet-actions ka-sheet-split\"><a class=\"btn btn-ghost\" href=\"#\">{done}</a><a class=\"btn btn-danger\" href=\"#supprimer-{frag}\">{delete}</a></div>",
                        help = v.tr(
                            "Les droits donnés au groupe valent pour chacun de ses ordinateurs, présents et à venir, en plus de ceux donnés ordinateur par ordinateur.",
                            "The rights given to the group apply to each of its computers, present and future, on top of those given computer by computer.",
                        ),
                        computers = v.tr("Ses ordinateurs", "Its computers"),
                        who = v.tr("Qui y a accès", "Who has access"),
                        open_h = v.tr("Ouvrir le groupe", "Open the group"),
                        done = v.tr("Terminé", "Done"),
                        delete = v.tr("Supprimer le groupe", "Delete the group"),
                    ),
                    false,
                ));
                let confirm = match v.lang {
                    Lang::Fr => t(v.lang, &format!("Supprimer le groupe « {} » ?", esc(&g.name)), ""),
                    Lang::En => format!("Delete the \"{}\" group?", esc(&g.name)),
                };
                sheets.push_str(&sheet(
                    v,
                    &format!("supprimer-{frag}"),
                    &o.name,
                    &confirm,
                    &format!(
                        "<p class=\"ka-muted\">{}</p>\n{}",
                        v.tr("Les droits donnés sur ce groupe partent avec lui.", "The rights given on this group go with it."),
                        v.form(
                            "/compte/organisation/groupes-ordinateurs/supprimer",
                            "",
                            &format!(
                                "{id_field}\n{}\n<div class=\"ka-sheet-actions\"><a class=\"btn btn-ghost\" href=\"#{frag}\">{}</a><button class=\"btn btn-danger\" type=\"submit\">{}</button></div>",
                                hidden("groupe", &g.id.to_string()),
                                v.tr("Annuler", "Cancel"),
                                v.tr("Supprimer", "Delete"),
                            ),
                        ),
                    ),
                    false,
                ));
            }
            format!(
                "<section class=\"ka-group\">\n<div class=\"ka-group-head\">{badge}<div><b>{name}</b><small>{count}</small></div></div>\n{chips}\n{opened}\n{foot}</section>\n",
                badge = badge(&g.name, o.kind, false),
                name = esc(&g.name),
                count = count_word(v.lang, g.stations.len(), "ordinateur", "ordinateurs", "computer", "computers"),
            )
        })
        .collect();
    let create = if o.manage {
        let inner = format!(
            "{id_field}\n{}\n<div class=\"ka-sheet-actions\"><a class=\"btn btn-ghost\" href=\"#\">{}</a><button class=\"btn kn-primary\" type=\"submit\">{}</button></div>",
            field(&v.tr("Nom", "Name"), &format!(r#"<input id="nom-groupe-ordinateurs" name="nom" maxlength="64" placeholder="{}" required>"#, v.tr("Postes de l'atelier", "Studio workstations"))),
            v.tr("Annuler", "Cancel"),
            v.tr("Créer le groupe", "Create the group"),
        );
        sheets.push_str(&sheet(
            v,
            "groupe-ordinateurs",
            &o.name,
            &v.tr("Créer un groupe d'ordinateurs", "Create a computer group"),
            &format!(
                "<p class=\"ka-muted\">{}</p>\n{}",
                v.tr("Vous y ajouterez ensuite des ordinateurs, puis vous l'ouvrirez à qui vous voulez.", "You'll then add computers to it, and open it to whoever you want."),
                v.form("/compte/organisation/groupes-ordinateurs/creer", "", &inner)
            ),
            false,
        ));
        format!("<a class=\"ka-pc-add ka-group-add\" href=\"#groupe-ordinateurs\"><span class=\"ka-plus\">{}</span><b>{}</b></a>\n", ic("plus"), v.tr("Créer un groupe d'ordinateurs", "Create a computer group"))
    } else {
        String::new()
    };
    format!(
        "<section class=\"ka-section\" id=\"groupes-ordinateurs\">\n<div class=\"ka-section-head\"><h2>{h}</h2></div>\n<p class=\"ka-muted\">{help}</p>\n<div class=\"ka-groups ka-after\">\n{cards}{create}</div>\n</section>\n",
        h = v.tr("Groupes d'ordinateurs", "Computer groups"),
        help = v.tr(
            "Une liste d'ordinateurs choisis à la main. Ouvrez le groupe : ses droits valent pour chacun de ses ordinateurs, présents et à venir.",
            "A list of computers picked by hand. Open the group: its rights apply to each of its computers, present and future.",
        ),
    )
}

fn role_select(v: &View<'_>, name: &str, current: Role, allow_owner: bool) -> String {
    let options: String = Role::ALL
        .iter()
        .filter(|r| allow_owner || **r != Role::Owner || current == Role::Owner)
        .map(|r| {
            let sel = if *r == current { " selected" } else { "" };
            format!("<option value=\"{}\"{sel}>{}</option>", r.as_str(), role_name(v.lang, *r))
        })
        .collect();
    format!("<select name=\"{name}\" aria-label=\"{}\">{options}</select>", v.tr("Rôle", "Role"))
}

/// Le haut de la page d'une famille ou d'une équipe : l'initiale, le nom, le genre, le rôle, les onglets.
#[allow(clippy::too_many_arguments)]
fn org_top(v: &View<'_>, id: &str, name: &str, kind: Kind, role: Role, manage: bool, current: &str, action: &str) -> String {
    let q_id = format!("{}&amp;id={}", v.q(), pct(id));
    let mut tabs = vec![
        ("membres", format!("/compte/organisation{q_id}&amp;vue=membres"), v.tr("Membres", "Members")),
        ("groupes", format!("/compte/organisation{q_id}&amp;vue=groupes"), v.tr("Groupes", "Groups")),
        ("ordinateurs", format!("/compte/organisation{q_id}&amp;vue=ordinateurs"), v.tr("Ordinateurs", "Computers")),
    ];
    if manage {
        tabs.push(("journal", format!("/compte/organisation{q_id}&amp;vue=journal"), v.tr("Journal", "Log")));
        if kind == Kind::Team {
            tabs.push(("deployer", format!("/compte/organisation/deployer{q_id}"), v.tr("Déployer", "Deploy")));
        }
    }
    let tabs: String = tabs
        .into_iter()
        .map(|(word, href, label)| {
            let cur = if word == current { " aria-current=\"page\"" } else { "" };
            format!("<a href=\"{href}\"{cur}>{label}</a>")
        })
        .collect();
    let you = match role {
        Role::Owner => v.tr("Vous êtes propriétaire", "You are the owner"),
        Role::Admin => v.tr("Vous êtes administrateur", "You are an administrator"),
        Role::Member => v.tr("Vous êtes membre", "You are a member"),
    };
    format!(
        "<nav class=\"ka-crumbs\"><a href=\"/compte/familles{q}\">{families}</a><span>/</span><span>{name}</span></nav>\n<header class=\"ka-head\"><div class=\"ka-org-head\">{badge}<div><h1><strong>{name}</strong></h1><p class=\"ka-pills\">{kind}<span class=\"ka-pill ka-pill-sig\">{you}</span></p></div></div>{action}</header>\n<nav class=\"ka-tabs\" aria-label=\"{sections}\">{tabs}</nav>\n",
        q = v.q(),
        families = v.tr("Familles et équipes", "Families and teams"),
        name = esc(name),
        badge = badge(name, kind, true),
        kind = kind_pill(v.lang, kind),
        sections = v.tr("Sections", "Sections"),
    )
}

pub fn org(v: &View<'_>, o: &OrgView, notice: Option<Notice>) -> String {
    let id_field = hidden("id", &o.id);
    let the = kind_the(v.lang, o.kind);
    let warn = if o.needs_2fa {
        format!(
            "<p class=\"notice\" role=\"status\">{} <a href=\"/compte{}#securite\">{}</a></p>\n",
            match v.lang {
                Lang::Fr => t(v.lang, &format!("Pour gérer {the}, protégez votre compte : une passkey, ou la double authentification."), ""),
                Lang::En => format!("To manage {the}, protect your account: a passkey, or two-factor authentication."),
            },
            v.q(),
            v.tr("Protéger mon compte", "Protect my account"),
        )
    } else {
        String::new()
    };
    let mut sheets = String::new();
    let body = match o.tab {
        Tab::Members => {
            let rows: String = o
                .members
                .iter()
                .map(|m| {
                    let mut detail = vec![esc(&m.email)];
                    if m.role.manages() {
                        detail.push(if m.two_factor { v.tr("compte protégé", "account protected") } else { v.tr("compte pas encore protégé", "account not protected yet") });
                    }
                    detail.extend(m.groups.iter().map(|g| esc(g)));
                    let name = if m.account == o.me { format!("{} ({})", esc(&m.name), v.tr("vous", "you")) } else { esc(&m.name) };
                    // Un Administrateur ne touche pas aux Propriétaires ; seul un Propriétaire en nomme.
                    let editable = o.manage && (o.role == Role::Owner || m.role != Role::Owner);
                    let acts = if editable {
                        let role = v.form(
                            "/compte/organisation/role",
                            "acct-inline ka-role",
                            &format!("{id_field}\n{}\n{}\n{}", hidden("compte", &m.account.to_string()), role_select(v, "role", m.role, o.role == Role::Owner), small_button(&v.tr("Changer", "Change"))),
                        );
                        let remove = if m.account == o.me {
                            String::new()
                        } else {
                            v.form("/compte/organisation/retirer", "acct-inline", &format!("{id_field}\n{}\n{}", hidden("compte", &m.account.to_string()), small_button(&v.tr("Retirer", "Remove"))))
                        };
                        format!("{role}\n{remove}")
                    } else {
                        format!("<span class=\"ka-tag{}\">{}</span>", if m.role == Role::Owner { " is-lit" } else { "" }, role_name(v.lang, m.role))
                    };
                    row(&avatar(&m.name), &name, &detail.join(" · "), &acts)
                })
                .collect();
            let invites: String = o
                .invites
                .iter()
                .map(|i| {
                    let cancel = if o.manage {
                        v.form("/compte/organisation/retirer", "acct-inline", &format!("{id_field}\n{}\n{}", hidden("invitation", &i.id.to_string()), small_button(&v.tr("Annuler", "Cancel"))))
                    } else {
                        String::new()
                    };
                    row(&avatar(&i.email), &esc(&i.email), &format!("{} · {}", v.tr("Invitation envoyée", "Invitation sent"), role_name(v.lang, i.role)), &cancel)
                })
                .collect();
            if o.manage {
                let inner = format!(
                    "{id_field}\n{email}\n<div class=\"field\"><label class=\"mono-label\" for=\"role\">{role_l}</label>\n{role}</div>\n<div class=\"ka-sheet-actions\"><a class=\"btn btn-ghost\" href=\"#\">{cancel}</a><button class=\"btn kn-primary\" type=\"submit\">{send}</button></div>",
                    email = field(
                        &v.tr("Son adresse e-mail", "Their email address"),
                        r#"<input id="email" name="email" type="email" inputmode="email" autocomplete="off" autocapitalize="off" spellcheck="false" maxlength="254" required>"#,
                    ),
                    role_l = v.tr("Son rôle", "Their role"),
                    role = role_select(v, "role", Role::Member, o.role == Role::Owner).replacen("<select ", "<select id=\"role\" ", 1),
                    cancel = v.tr("Annuler", "Cancel"),
                    send = v.tr("Envoyer l'invitation", "Send the invitation"),
                );
                sheets.push_str(&sheet(v, "inviter", &o.name, &v.tr("Inviter un membre", "Invite a member"), &v.form("/compte/organisation/inviter", "", &inner), false));
            }
            let roles = help_box(
                "info",
                &v.tr(
                    "<b>Propriétaire</b> : tout, y compris nommer d'autres propriétaires. <b>Administrateur</b> : gère les membres, les groupes et les ordinateurs. <b>Membre</b> : utilise ce qu'on lui ouvre.",
                    "<b>Owner</b>: everything, including naming other owners. <b>Administrator</b>: manages members, groups and computers. <b>Member</b>: uses what is opened to them.",
                ),
            );
            let rename = if o.manage && o.role == Role::Owner {
                let inner = format!(
                    "{id_field}\n<input name=\"nom\" maxlength=\"64\" value=\"{}\" aria-label=\"{}\" required>\n{}",
                    esc(&o.name),
                    v.tr("Nouveau nom", "New name"),
                    small_button(&v.tr("Renommer", "Rename"))
                );
                row(&ic("gear"), &v.tr("Renommer", "Rename"), "", &v.form("/compte/organisation/renommer", "acct-inline ka-inline-field", &inner))
            } else {
                String::new()
            };
            let leave = row(
                &ic("out"),
                &match v.lang {
                    Lang::Fr => t(v.lang, &format!("Quitter {the}"), ""),
                    Lang::En => format!("Leave {the}"),
                },
                &v.tr("Vous perdez les accès qu'elle vous donne.", "You lose the access it gives you."),
                &v.form("/compte/organisation/quitter", "acct-inline", &format!("{id_field}\n<button class=\"btn btn-danger btn-sm\" type=\"submit\">{}</button>", v.tr("Quitter", "Leave"))),
            );
            format!(
                "{roles}<div class=\"ka-list ka-after\">\n{rows}{invites}</div>\n<section class=\"ka-section\">\n<div class=\"ka-section-head\"><h2>{settings}</h2></div>\n<div class=\"ka-list\">\n{rename}{leave}</div>\n</section>\n",
                settings = v.tr("Réglages", "Settings"),
            )
        }
        Tab::Groups => {
            // Un formulaire qui revient sur le panneau du groupe (l'ancre suit la réponse).
            let at = |path: &str, frag: &str, class: &str, inner: &str| {
                let q = v.q();
                v.form(path, class, inner).replacen(&format!("action=\"{path}{q}\""), &format!("action=\"{path}{q}#{frag}\""), 1)
            };
            let member_form = |g: &Group, account: i64, add: bool, frag: &str| {
                at(
                    "/compte/organisation/groupes/membre",
                    frag,
                    "acct-inline",
                    &format!(
                        "{id_field}\n{}\n{}\n{}\n<button class=\"btn {} btn-sm\" type=\"submit\">{}</button>",
                        hidden("groupe", &g.id.to_string()),
                        hidden("compte", &account.to_string()),
                        hidden("action", if add { "ajouter" } else { "retirer" }),
                        if add { "kn-primary" } else { "btn-ghost" },
                        if add { v.tr("Ajouter", "Add") } else { v.tr("Retirer", "Remove") },
                    ),
                )
            };
            let cards: String = o
                .groups
                .iter()
                .map(|g| {
                    let frag = format!("groupe-{}", g.id);
                    let members = if g.members.is_empty() {
                        format!("<p class=\"ka-muted ka-group-empty\">{}</p>", v.tr("Personne pour l'instant.", "Nobody yet."))
                    } else {
                        let chips: String = g.members.iter().map(|(_, name)| format!("<span class=\"ka-chip\">{}{}</span>", avatar(name), esc(name))).collect();
                        format!("<div class=\"ka-chips\">{chips}</div>")
                    };
                    let foot = if o.manage {
                        format!(
                            "<div class=\"ka-group-acts\"><a class=\"btn {} btn-sm\" href=\"#{frag}\">{}{}</a></div>",
                            if g.members.is_empty() { "kn-primary" } else { "btn-ghost" },
                            ic("people"),
                            if g.members.is_empty() { v.tr("Ajouter des membres", "Add members") } else { v.tr("Gérer les membres", "Manage members") },
                        )
                    } else {
                        String::new()
                    };
                    if o.manage {
                        // Le panneau du groupe : chaque membre, dedans ou pas ; puis supprimer le groupe.
                        let rows: String = o
                            .members
                            .iter()
                            .map(|m| {
                                let inside = g.members.iter().any(|(a, _)| *a == m.account);
                                let state = if inside { format!("<span class=\"ka-tag is-lit\">{}</span>", v.tr("Dans le groupe", "In the group")) } else { String::new() };
                                row(&avatar(&m.name), &esc(&m.name), &esc(&m.email), &format!("{state}{}", member_form(g, m.account, !inside, &frag)))
                            })
                            .collect();
                        let delete = v.form(
                            "/compte/organisation/groupes/supprimer",
                            "acct-inline",
                            &format!("{id_field}\n{}\n<button class=\"btn btn-danger btn-sm\" type=\"submit\">{}</button>", hidden("groupe", &g.id.to_string()), v.tr("Supprimer le groupe", "Delete the group")),
                        );
                        sheets.push_str(&sheet(
                            v,
                            &frag,
                            &o.name,
                            &match v.lang {
                                Lang::Fr => t(v.lang, &format!("Le groupe « {} »", esc(&g.name)), ""),
                                Lang::En => format!("The \"{}\" group", esc(&g.name)),
                            },
                            &format!(
                                "<p class=\"ka-muted\">{help}</p>\n<div class=\"ka-list\">\n{rows}</div>\n<p class=\"ka-muted ka-after\">{invite}</p>\n<div class=\"ka-sheet-actions ka-sheet-split\"><a class=\"btn btn-ghost\" href=\"#\">{done}</a>{delete}</div>",
                                help = v.tr(
                                    "Ajoutez ou retirez des membres : chacun reçoit aussitôt les accès ouverts au groupe, ou les perd.",
                                    "Add or remove members: each one gets the access opened to the group right away, or loses it.",
                                ),
                                invite = v.tr("Quelqu'un manque ? Invitez-le d'abord, dans l'onglet Membres.", "Someone missing? Invite them first, in the Members tab."),
                                done = v.tr("Terminé", "Done"),
                            ),
                            false,
                        ));
                    }
                    format!(
                        "<section class=\"ka-group\">\n<div class=\"ka-group-head\">{badge}<div><b>{name}</b><small>{count}</small></div></div>\n{members}\n{foot}</section>\n",
                        badge = badge(&g.name, o.kind, false),
                        name = esc(&g.name),
                        count = count_word(v.lang, g.members.len(), "membre", "membres", "member", "members"),
                    )
                })
                .collect();
            let create = if o.manage {
                let inner = format!(
                    "{id_field}\n{}\n<div class=\"ka-sheet-actions\"><a class=\"btn btn-ghost\" href=\"#\">{}</a><button class=\"btn kn-primary\" type=\"submit\">{}</button></div>",
                    field(
                        &v.tr("Nom du groupe", "Group name"),
                        &format!(r#"<input id="nom" name="nom" maxlength="64" placeholder="{}" required>"#, if o.kind == Kind::Family { v.tr("Les enfants", "The kids") } else { v.tr("Graphistes", "Designers") }),
                    ),
                    v.tr("Annuler", "Cancel"),
                    v.tr("Créer le groupe", "Create the group"),
                );
                sheets.push_str(&sheet(
                    v,
                    "groupe",
                    &o.name,
                    &v.tr("Créer un groupe", "Create a group"),
                    &format!(
                        "<p class=\"ka-muted\">{}</p>\n{}",
                        v.tr("Vous y ajouterez ensuite des membres.", "You'll then add members to it."),
                        v.form("/compte/organisation/groupes/creer", "", &inner)
                    ),
                    false,
                ));
                format!("<a class=\"ka-pc-add ka-group-add\" href=\"#groupe\"><span class=\"ka-plus\">{}</span><b>{}</b></a>\n", ic("plus"), v.tr("Créer un groupe", "Create a group"))
            } else {
                String::new()
            };
            let empty = if o.groups.is_empty() && !o.manage { format!("<p class=\"ka-muted\">{}</p>\n", v.tr("Aucun groupe pour l'instant.", "No groups yet.")) } else { String::new() };
            format!(
                "{help}{empty}<div class=\"ka-groups ka-after\">\n{cards}{create}</div>\n",
                help = help_box(
                    "info",
                    &v.tr(
                        "<b>Un groupe réunit des membres qui ont les mêmes besoins.</b> Créez-le, ajoutez-y des membres, puis ouvrez-lui un ordinateur : sur la page de l'ordinateur, « Réglages » › « Ouvrir à une famille ou une équipe ». Chaque membre ajouté ensuite y a accès sans rien régler de plus.",
                        "<b>A group gathers members with the same needs.</b> Create it, add members, then open a computer to it: on the computer's page, “Settings” › “Open to a family or team”. Each member added later gets access without anything else to set.",
                    ),
                ),
            )
        }
        Tab::Computers => {
            let rows: String = o
                .computers
                .iter()
                .map(|c| {
                    row(
                        &ic("monitor"),
                        &format!("<a href=\"/compte/ordinateur{}&amp;id={}\">{}</a>", v.q(), pct(&c.id), esc(&c.name)),
                        &esc(&c.access),
                        &if c.admin && c.online {
                            format!("{}<a class=\"btn btn-ghost btn-sm\" href=\"{}\">{}Admin</a>", state_pill(v, c.online), admin_href(v, &c.id), ic("gear"))
                        } else {
                            state_pill(v, c.online)
                        },
                    )
                })
                .collect();
            let list = if o.computers.is_empty() {
                format!("<p class=\"ka-muted\">{}</p>\n", v.tr("Aucun ordinateur pour l'instant.", "No computers yet."))
            } else {
                format!("<div class=\"ka-list\">\n{rows}</div>\n")
            };
            let groups = station_groups_section(v, o, &mut sheets);
            format!(
                "{help}<div class=\"ka-after\">{list}</div>{groups}",
                help = help_box(
                    "info",
                    &match v.lang {
                        Lang::Fr => t(
                            v.lang,
                            &format!("Un ordinateur de {the} lui appartient : il reste là quand quelqu'un part. Pour en rattacher un : Admin › Compte KyberNow › « Se connecter », puis choisissez {the} comme propriétaire."),
                            "",
                        ),
                        Lang::En => format!("A computer of {the} belongs to it: it stays when someone leaves. To attach one: Admin › KyberNow account › \"Sign in\", then choose {the} as the owner."),
                    },
                ),
            )
        }
        Tab::Log => format!("<div class=\"ka-list\">\n{}</div>\n", log_html(v, &o.log, o.kind)),
    };
    let action = if o.manage && o.tab == Tab::Members {
        format!("<a class=\"btn kn-primary\" href=\"#inviter\">{}{}</a>", ic("plus"), v.tr("Inviter un membre", "Invite a member"))
    } else {
        String::new()
    };
    let page = format!(
        "{top}{notice}{warn}<div class=\"ka-tab-panel\">\n{body}</div>\n{sheets}",
        top = org_top(v, &o.id, &o.name, o.kind, o.role, o.manage, o.tab.word(), &action),
        notice = notice_html(v.lang, notice),
    );
    frame(v, &o.shell, Here::Org(o.id.clone()), &o.name, "/compte/organisation", &page)
}

// --- Déployer des Stations (équipes) --------------------------------------------------

/// La page « Déployer des Stations ».
#[derive(Debug, Clone)]
pub struct DeployView {
    pub id: String,
    pub name: String,
    pub role: Role,
    /// Les jetons et leurs Stations enrôlées `(nom, quand)`.
    pub tokens: Vec<(EnrollToken, Vec<(String, i64)>)>,
    pub groups: Vec<(i64, String)>,
    /// Les groupes d'ordinateurs (« Ranger dans le groupe »).
    pub station_groups: Vec<(i64, String)>,
    /// Le jeton tout juste créé, montré une seule fois.
    pub new_token: Option<String>,
    /// Le relais, pour la ligne de commande (`/relay=`).
    pub relay: String,
    pub now: i64,
    pub shell: Shell,
}

pub fn deploy(v: &View<'_>, d: &DeployView, notice: Option<Notice>) -> String {
    let title = v.tr("Déployer des Stations", "Deploy Stations");
    let id_field = hidden("id", &d.id);
    let fresh = match &d.new_token {
        Some(token) => format!(
            "<section class=\"ka-fresh\">\n<h2>{h}</h2>\n<p class=\"ka-muted\">{once}</p>\n<p class=\"token\">{token}</p>\n<p class=\"mono-label\">{silent}</p>\n<p class=\"token token-sm\">KyberNow-Station-Setup.exe /quiet /enroll={token} /relay={relay}</p>\n</section>\n",
            h = v.tr("Nouveau jeton", "New token"),
            once = v.tr("Montré une seule fois : copiez-le maintenant.", "Shown only once: copy it now."),
            token = esc(token),
            silent = v.tr("Installation silencieuse (Windows)", "Silent install (Windows)"),
            relay = esc(&d.relay),
        ),
        None => String::new(),
    };
    let tokens: String = d
        .tokens
        .iter()
        .map(|(tk, stations)| {
            let until = match v.lang {
                Lang::Fr => format!("jusqu'au {}", date(v.lang, tk.expires_at - 1)),
                Lang::En => format!("until {}", date(v.lang, tk.expires_at - 1)),
            };
            let mut parts = vec![format!("{} / {} Stations", tk.used, tk.max_stations), until];
            if let Some(g) = &tk.group_name
                && !tk.rights.is_empty()
            {
                parts.push(match v.lang {
                    Lang::Fr => t(v.lang, &format!("groupe {g} : {}", rights_words(v.lang, &tk.rights)), ""),
                    Lang::En => format!("group {g}: {}", rights_words(v.lang, &tk.rights)),
                });
            }
            if let Some(g) = &tk.sgroup_name {
                parts.push(match v.lang {
                    Lang::Fr => t(v.lang, &format!("rangées dans {g}"), ""),
                    Lang::En => format!("filed in {g}"),
                });
            }
            let state = match tk.refusal(d.now) {
                None => String::new(),
                Some("revoked") => format!("<span class=\"ka-pill ka-pill-off\">{}</span>", v.tr("Révoqué", "Revoked")),
                Some("expired") => format!("<span class=\"ka-pill ka-pill-off\">{}</span>", v.tr("Échu", "Expired")),
                Some(_) => format!("<span class=\"ka-pill ka-pill-off\">{}</span>", v.tr("Épuisé", "Used up")),
            };
            let revoke = if tk.revoked_at.is_none() {
                v.form(
                    "/compte/organisation/jetons/revoquer",
                    "acct-inline",
                    &format!("{id_field}\n{}\n<button class=\"btn btn-danger btn-sm\" type=\"submit\">{}</button>", hidden("jeton", &tk.id.to_string()), v.tr("Révoquer le jeton", "Revoke the token")),
                )
            } else {
                String::new()
            };
            let enrolled: String = stations
                .iter()
                .map(|(name, at)| {
                    let when = match v.lang {
                        Lang::Fr => format!("enrôlée le {}", date(v.lang, *at)),
                        Lang::En => format!("enrolled {}", date(v.lang, *at)),
                    };
                    row(&ic("monitor"), &esc(name), &when, "")
                })
                .collect();
            format!("{}{enrolled}", row(&ic("key"), &esc(&tk.name), &format!("{}… · {}", esc(&tk.hint), esc(&parts.join(" · "))), &format!("{state}{revoke}")))
        })
        .collect();
    let tokens = if tokens.is_empty() { String::new() } else { format!("<div class=\"ka-list ka-after\">\n{tokens}</div>\n") };
    let groups: String = std::iter::once(format!("<option value=\"\">{}</option>", v.tr("Aucun", "None")))
        .chain(d.groups.iter().map(|(id, name)| format!("<option value=\"{id}\">{}</option>", esc(name))))
        .collect();
    let sgroups: String = std::iter::once(format!("<option value=\"\">{}</option>", v.tr("Aucun", "None")))
        .chain(d.station_groups.iter().map(|(id, name)| format!("<option value=\"{id}\">{}</option>", esc(name))))
        .collect();
    let inner = format!(
        "{id_field}\n{name}\n<div class=\"ka-two\">{days}\n{count}</div>\n<div class=\"field\"><label class=\"mono-label\" for=\"groupe\">{group_l}</label>\n<select id=\"groupe\" name=\"groupe\">{groups}</select></div>\n{rights}\n<div class=\"field\"><label class=\"mono-label\" for=\"groupe_ordinateurs\">{sgroup_l}</label>\n<select id=\"groupe_ordinateurs\" name=\"groupe_ordinateurs\">{sgroups}</select></div>\n<div class=\"ka-sheet-actions\"><a class=\"btn btn-ghost\" href=\"#\">{cancel}</a><button class=\"btn kn-primary\" type=\"submit\">{create}</button></div>",
        name = field(&v.tr("Nom", "Name"), r#"<input id="nom" name="nom" maxlength="64" placeholder="Postes graphistes" required>"#),
        days = field(&v.tr("Durée (jours)", "Duration (days)"), r#"<input id="jours" name="jours" type="number" min="1" max="365" value="30" required>"#),
        count = field(&v.tr("Nombre de Stations", "Number of Stations"), r#"<input id="nombre" name="nombre" type="number" min="1" max="10000" value="10" required>"#),
        group_l = v.tr("Groupe qui reçoit les droits", "Group that gets the rights"),
        sgroup_l = v.tr("Ranger dans le groupe", "File in the group"),
        rights = rights_tiles(v, &v.tr("Droits donnés au groupe", "Rights given to the group"), &Rights::new([Right::Session]), "jeton"),
        cancel = v.tr("Annuler", "Cancel"),
        create = v.tr("Créer le jeton", "Create the token"),
    );
    let action = format!("<a class=\"btn kn-primary\" href=\"#jeton\">{}{}</a>", ic("plus"), v.tr("Créer un jeton", "Create a token"));
    let page = format!(
        "{top}{notice}<div class=\"ka-tab-panel\">\n{help}{fresh}{tokens}<p class=\"ka-muted ka-after\">{revoke_help}</p>\n</div>\n{sheet}",
        top = org_top(v, &d.id, &d.name, Kind::Team, d.role, true, "deployer", &action),
        notice = notice_html(v.lang, notice),
        help = help_box(
            "rocket",
            &v.tr(
                "<b>Pour installer plusieurs Stations d'un coup.</b> Créez un jeton, puis lancez l'installateur avec lui : chaque Station se rattache à l'équipe toute seule, sans écran.",
                "<b>To install several Stations at once.</b> Create a token, then run the installer with it: each Station attaches itself to the team, with no screen.",
            ),
        ),
        revoke_help = v.tr(
            "Révoquer un jeton empêche de nouveaux enrôlements ; les Stations déjà rattachées restent à l'équipe.",
            "Revoking a token prevents new enrollments; the Stations already attached stay with the team.",
        ),
        sheet = sheet(v, "jeton", &d.name, &v.tr("Nouveau jeton d'enrôlement", "New enrollment token"), &v.form("/compte/organisation/jetons/creer", "", &inner), false),
    );
    frame(v, &d.shell, Here::Org(d.id.clone()), &title, "/compte/organisation/deployer", &page)
}

// --- Le tour d'accueil -----------------------------------------------------------------

/// Ce que le tour sait du compte.
#[derive(Debug, Clone, Default)]
pub struct Welcome {
    pub name: String,
    /// Une passkey ou la double authentification.
    pub protected: bool,
    /// De quoi ajouter une passkey (`None` : déjà fait, ou le maximum).
    pub register: Option<Registration>,
    /// Le premier ordinateur à lui, s'il y en a un.
    pub computer: Option<String>,
}

/// L'illustration de chaque étape (SVG, sans script).
fn stage(step: u8) -> &'static str {
    match step {
        1 => r##"<svg viewBox="0 0 520 500" aria-hidden="true"><circle cx="260" cy="250" r="120" class="kt-ring"/><circle cx="260" cy="250" r="120" class="kt-ring kt-ring-2"/><g class="kt-glow"><path d="M260 140l90 38v70c0 56-38 102-90 120-52-18-90-64-90-120v-70z" class="kt-shield"/><circle cx="260" cy="240" r="30" class="kt-line"/><path d="M243 240c0-10 8-17 17-17M277 240c0 12-6 22-17 28M252 248c3 6 8 9 8 9" class="kt-line"/></g><text x="260" y="420" text-anchor="middle" class="kt-caption">TOUCH ID · FACE ID · WINDOWS HELLO</text></svg>"##,
        2 => r##"<svg viewBox="0 0 520 500" aria-hidden="true"><rect x="60" y="150" width="190" height="130" rx="12" class="kt-box"/><rect x="74" y="164" width="162" height="102" rx="6" class="kt-screen"/><text x="88" y="190" class="kt-caption">ADMIN</text><text x="88" y="214" class="kt-title">Compte KyberNow</text><rect x="88" y="228" width="100" height="26" rx="5" class="kt-btn"/><path d="M140 296h40M155 280v16" class="kt-dim"/><path d="M250 215 C 320 215, 300 300, 370 300" class="kt-beam kt-glow"/><g class="kt-glow"><rect x="360" y="250" width="110" height="100" rx="16" class="kt-shield"/><circle cx="415" cy="290" r="14" class="kt-dot"/><path d="M393 330c4-12 12-18 22-18s18 6 22 18" class="kt-line kt-line-sig"/></g></svg>"##,
        3 => r##"<svg viewBox="0 0 520 500" aria-hidden="true"><g class="kt-glow"><rect x="210" y="210" width="100" height="70" rx="10" class="kt-shield"/></g><text x="260" y="250" text-anchor="middle" class="kt-caption">PC</text><rect x="40" y="90" width="180" height="120" rx="20" class="kt-fam"/><circle cx="80" cy="160" r="18" class="kt-av kt-av-0"/><circle cx="130" cy="160" r="18" class="kt-av kt-av-1"/><circle cx="180" cy="160" r="18" class="kt-av kt-av-2"/><rect x="300" y="290" width="190" height="120" rx="20" class="kt-team"/><circle cx="340" cy="362" r="18" class="kt-av kt-av-3"/><circle cx="390" cy="362" r="18" class="kt-av kt-av-1"/><circle cx="440" cy="362" r="18" class="kt-av kt-av-4"/><path d="M200 205 L 230 225" class="kt-beam kt-beam-fam"/><path d="M320 295 L 290 270" class="kt-beam kt-beam-team"/></svg>"##,
        _ => r##"<svg viewBox="0 0 520 500" aria-hidden="true"><circle cx="260" cy="250" r="170" class="kt-orbit-line"/><circle cx="260" cy="250" r="110" class="kt-core"/><g class="kt-glow"><rect x="200" y="200" width="120" height="80" rx="10" class="kt-shield"/><path d="M240 300h40M260 280v20" class="kt-line kt-line-sig"/></g><g class="kt-orbit"><line x1="260" y1="250" x2="430" y2="250" class="kt-beam"/><line x1="260" y1="250" x2="312.5" y2="411.7" class="kt-beam"/><line x1="260" y1="250" x2="122.5" y2="349.9" class="kt-beam"/><line x1="260" y1="250" x2="122.5" y2="150.1" class="kt-beam"/><line x1="260" y1="250" x2="312.5" y2="88.3" class="kt-beam"/><circle cx="430" cy="250" r="26" class="kt-dev"/><circle cx="312.5" cy="411.7" r="26" class="kt-dev"/><circle cx="122.5" cy="349.9" r="26" class="kt-dev"/><circle cx="122.5" cy="150.1" r="26" class="kt-dev"/><circle cx="312.5" cy="88.3" r="26" class="kt-dev"/></g></svg>"##,
    }
}

/// `/compte/bienvenue?etape=…` : le tour d'accueil, en quatre étapes plein écran.
pub fn welcome(v: &View<'_>, step: u8, w: &Welcome) -> String {
    let q = v.q();
    let step = step.clamp(0, 3);
    let go = |s: u8| format!("/compte/bienvenue{q}&amp;etape={s}");
    // Finir (ou passer) : le tour est vu ; `suite` choisit où l'on va.
    let finish = |label: &str, suite: &str, class: &str| v.form("/compte/bienvenue", "ka-inline-form", &format!("{}\n<button class=\"{class}\" type=\"submit\">{label}</button>", hidden("suite", suite)));
    let point = |n: &str, html: String| format!("<li><span class=\"ka-n\">{n}</span><div>{html}</div></li>");
    let (eyebrow, title, body, next) = match step {
        0 => (
            match v.lang {
                Lang::Fr => t(v.lang, &if w.name.is_empty() { "Bienvenue".to_string() } else { format!("Bienvenue, {}", esc(&w.name)) }, ""),
                Lang::En => if w.name.is_empty() { "Welcome".to_string() } else { format!("Welcome, {}", esc(&w.name)) },
            },
            v.tr("Votre ordinateur, <strong>sur tous vos appareils</strong>", "Your computer, <strong>on all your devices</strong>"),
            format!(
                "<p>{text}</p>\n<ul class=\"ka-points\">{p1}{p2}{p3}</ul>",
                text = v.tr(
                    "Votre compte KyberNow relie vos ordinateurs à tous vos appareils. Connectez-vous une fois dans KyberNow Link : ils apparaissent tout de suite, sur la télé comme sur le téléphone.",
                    "Your KyberNow account links your computers to all your devices. Sign in once in KyberNow Link: they show up right away, on the TV as on the phone.",
                ),
                p1 = point("1", v.tr("<b>Protégez votre compte</b> en une touche.", "<b>Protect your account</b> with one touch.")),
                p2 = point("2", v.tr("<b>Rattachez votre ordinateur</b> depuis Admin.", "<b>Attach your computer</b> from Admin.")),
                p3 = point("3", v.tr("<b>Ouvrez-le à vos proches</b> ou à votre équipe, si vous voulez.", "<b>Open it to your loved ones</b> or your team, if you like.")),
            ),
            format!("<a class=\"btn kn-primary\" href=\"{}\">{}{}</a>", go(1), v.tr("Commencer", "Start"), ic("arrow")),
        ),
        1 => {
            let state = if w.protected {
                format!("<p class=\"ka-wait is-ok\"><span class=\"ka-spin\"></span><span>{}</span></p>", v.tr("<b>Votre compte est protégé.</b>", "<b>Your account is protected.</b>"))
            } else {
                String::new()
            };
            let action = match (&w.register, w.protected) {
                (Some(r), false) => format!(
                    "<a class=\"btn btn-ghost\" href=\"{later}\">{later_l}</a>\n{form}",
                    later = go(2),
                    later_l = v.tr("Plus tard", "Later"),
                    form = passkey_create_form_with(v, r, "btn kn-primary", &hidden("suite", "bienvenue")),
                ),
                _ => format!("<a class=\"btn kn-primary\" href=\"{}\">{}{}</a>", go(2), v.tr("Continuer", "Continue"), ic("arrow")),
            };
            (
                v.tr("Étape 1 sur 3", "Step 1 of 3"),
                v.tr("Protégez votre compte <strong>avec une passkey</strong>", "Protect your account <strong>with a passkey</strong>"),
                format!(
                    "<p>{text}</p>\n{state}{none}<p class=\"ka-muted\">{totp}</p>",
                    text = v.tr(
                        "Votre compte ouvre vos ordinateurs : il mérite mieux qu'un mot de passe. Une passkey utilise Touch ID, Face ID ou Windows Hello, et personne ne peut vous la voler par e-mail.",
                        "Your account opens your computers: it deserves better than a password. A passkey uses Touch ID, Face ID or Windows Hello, and nobody can steal it from you by email.",
                    ),
                    none = if w.protected { String::new() } else { passkey_unsupported(v) },
                    totp = match v.lang {
                        Lang::Fr => t(v.lang, &format!("Vous préférez une application d'authentification ? <a href=\"/compte/double-authentification{q}\">Activer la double authentification</a>"), ""),
                        Lang::En => format!("Prefer an authenticator app? <a href=\"/compte/double-authentification{q}\">Turn on two-factor authentication</a>"),
                    },
                ),
                action,
            )
        }
        2 => {
            let state = match &w.computer {
                Some(name) => format!(
                    "<p class=\"ka-wait is-ok\"><span class=\"ka-spin\"></span><span>{}</span></p>",
                    match v.lang {
                        Lang::Fr => t(v.lang, &format!("<b>{}</b> est rattaché à votre compte.", esc(name)), ""),
                        Lang::En => format!("<b>{}</b> is attached to your account.", esc(name)),
                    }
                ),
                None => format!("<p class=\"ka-wait\"><span class=\"ka-spin\"></span><span>{}</span></p>", v.tr("En attente de votre ordinateur…", "Waiting for your computer…")),
            };
            (
                v.tr("Étape 2 sur 3", "Step 2 of 3"),
                v.tr("Rattachez <strong>votre ordinateur</strong>", "Attach <strong>your computer</strong>"),
                format!("{steps}{state}", steps = attach_steps(v)),
                format!(
                    "<a class=\"btn {class}\" href=\"{}\">{}{}</a>",
                    go(3),
                    if w.computer.is_some() { v.tr("Continuer", "Continue") } else { v.tr("Plus tard", "Later") },
                    if w.computer.is_some() { ic("arrow") } else { String::new() },
                    class = if w.computer.is_some() { "kn-primary" } else { "btn-ghost" },
                ),
            )
        }
        _ => (
            v.tr("Étape 3 sur 3", "Step 3 of 3"),
            v.tr("Ouvrez-le <strong>à ceux que vous voulez</strong>", "Open it <strong>to whoever you like</strong>"),
            format!(
                "<p>{text}</p>\n<div class=\"ka-kinds ka-kinds-tour\">\n{family}\n{team}\n</div>",
                text = v.tr(
                    "Ajoutez une personne par son e-mail et choisissez ce qu'elle peut faire. Pour plusieurs personnes à la fois, réunissez-les :",
                    "Add a person by their email and choose what they can do. For several people at once, gather them:",
                ),
                family = v.form(
                    "/compte/bienvenue",
                    "ka-inline-form",
                    &format!(
                        "{}\n<button class=\"ka-kind ka-kind-family\" type=\"submit\"><span class=\"ka-halo\"></span><span class=\"ka-kind-icon\">{}</span><h3>{}</h3><p>{}</p></button>",
                        hidden("suite", "famille"),
                        ic("home"),
                        v.tr("Une famille", "A family"),
                        v.tr("Le PC du salon pour toute la maison.", "The living room PC for the whole home.")
                    )
                ),
                team = v.form(
                    "/compte/bienvenue",
                    "ka-inline-form",
                    &format!(
                        "{}\n<button class=\"ka-kind ka-kind-team\" type=\"submit\"><span class=\"ka-halo\"></span><span class=\"ka-kind-icon\">{}</span><h3>{}</h3><p>{}</p></button>",
                        hidden("suite", "equipe"),
                        ic("team"),
                        v.tr("Une équipe", "A team"),
                        v.tr("Des rôles, des groupes et un journal.", "Roles, groups and a log.")
                    )
                ),
            ),
            finish(&v.tr("Ouvrir mon espace", "Open my space"), "", "btn kn-primary"),
        ),
    };
    let progress: String = (0..4).map(|i| format!("<i{}></i>", if i <= step { " class=\"is-done\"" } else { "" })).collect();
    let back = if step > 0 { format!("<a class=\"btn btn-text\" href=\"{}\">{}{}</a>", go(step - 1), ic("back"), v.tr("Retour", "Back")) } else { "<span></span>".to_string() };
    let body = format!(
        "<div class=\"kt-top\"><img class=\"kt-logo\" src=\"{logo}\" alt=\"KyberNow Share\" width=\"200\" height=\"37\"><div class=\"kt-progress\" role=\"img\" aria-label=\"{of}\">{progress}</div>{skip}</div>\n<div class=\"kt-main kt-step-{step}\"><div class=\"kt-stage\">{stage}</div>\n<div class=\"kt-text\"><p class=\"ka-eyebrow\">{eyebrow}</p>\n<h1>{title}</h1>\n{body}</div></div>\n<div class=\"kt-foot\">{back}<div class=\"kt-actions\">{next}</div></div>",
        logo = assets::url("kybernow-share-dark.svg"),
        of = match v.lang {
            Lang::Fr => format!("Étape {} sur 4", step + 1),
            Lang::En => format!("Step {} of 4", step + 1),
        },
        skip = finish(&v.tr("Passer", "Skip"), "", "btn btn-text"),
        stage = stage(step),
    );
    let refresh = if step == 2 && w.computer.is_none() { "\n<meta http-equiv=\"refresh\" content=\"5\">" } else { "" };
    let script = if body.contains("data-passkey=\"") { format!("\n<script src=\"{}\" defer></script>", assets::url("passkey.js")) } else { String::new() };
    format!(
        r##"<!doctype html>
<html lang="{lang}">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1, viewport-fit=cover">
<meta name="color-scheme" content="dark">
<meta name="theme-color" content="#05070A">
<meta name="robots" content="noindex">{refresh}
<link rel="icon" type="image/svg+xml" href="{favicon}">
<link rel="preload" href="{font}" as="font" type="font/woff2" crossorigin>
<link rel="stylesheet" href="{css}">
<title>{title} · KyberNow</title>{script}
</head>
<body class="kt-page">
{sprite}
<main class="kt-tour">
{body}
</main>
</body>
</html>
"##,
        lang = v.lang.code(),
        title = v.tr("Bienvenue", "Welcome"),
        favicon = assets::url("kybernow-favicon.svg"),
        font = assets::url("archivo.woff2"),
        css = assets::url("share.css"),
        sprite = SPRITE,
    )
}

// --- Admin sur le réseau ---------------------------------------------------------------

/// « Ouvrir Admin de {ordinateur} ? » : Admin s'ouvrira à `host`, sur le réseau ; `remote` : de n'importe
/// où, par le relais, confirmé par une passkey (`Some(passkeys du compte)`, vide : il n'en a pas).
pub fn open_admin(v: &View<'_>, computer: &str, host: &str, fields: &[(&str, &str)], remote: Option<&[String]>) -> String {
    let title = match v.lang {
        Lang::Fr => t(v.lang, &format!("Ouvrir Admin de {} ?", esc(computer)), ""),
        Lang::En => format!("Open {}'s Admin?", esc(computer)),
    };
    let hidden_fields: String = fields.iter().map(|(k, val)| hidden(k, val)).collect::<Vec<_>>().join("\n");
    let cancel = v.form(
        "/compte/ouvrir-admin",
        "acct-inline",
        &format!("{hidden_fields}\n<button class=\"btn btn-ghost\" type=\"submit\" name=\"action\" value=\"annuler\">{}</button>", v.tr("Annuler", "Cancel")),
    );
    let (where_, form) = match remote {
        None => (
            v.tr(
                "Avec le droit Administrer, votre compte ouvre Admin de cet ordinateur, sur son réseau, à cette adresse :",
                "With the Administer right, your account opens this computer's Admin, on its network, at this address:",
            ),
            v.form(
                "/compte/ouvrir-admin",
                "",
                &format!(
                    "{hidden_fields}\n<button class=\"btn kn-primary\" type=\"submit\" name=\"action\" value=\"ouvrir\">{open}</button>\n<button class=\"btn btn-ghost\" type=\"submit\" name=\"action\" value=\"annuler\">{cancel}</button>",
                    open = v.tr("Ouvrir Admin", "Open Admin"),
                    cancel = v.tr("Annuler", "Cancel"),
                ),
            ),
        ),
        Some([]) => (
            v.tr("De n'importe où, par KyberNow Share, à cette adresse :", "From anywhere, through KyberNow Share, at this address:"),
            format!(
                "<p class=\"notice\" role=\"status\">{} <a href=\"/compte{}#securite\">{}</a></p>\n{cancel}",
                v.tr(
                    "Ajoutez d'abord une passkey à votre compte : elle protège l'ouverture d'Admin à distance.",
                    "Add a passkey to your account first: it protects opening Admin remotely.",
                ),
                v.q(),
                v.tr("Ajouter une passkey", "Add a passkey"),
            ),
        ),
        Some(allow) => (
            v.tr("De n'importe où, par KyberNow Share, à cette adresse :", "From anywhere, through KyberNow Share, at this address:"),
            format!(
                "{}\n{}{cancel}",
                passkey_get_form_with(v, "/compte/ouvrir-admin", allow, &v.tr("Confirmer avec une passkey", "Confirm with a passkey"), &format!("{hidden_fields}\n{}\n", hidden("action", "ouvrir"))),
                passkey_unsupported(v),
            ),
        ),
    };
    let card = format!(
        "<h1>{title}</h1>\n<p class=\"help\">{where_}</p>\n<p class=\"help\"><b>{host}</b></p>\n<p class=\"hint\">{check}</p>\n{form}",
        host = esc(host),
        check = v.tr("Vous n'avez pas demandé à ouvrir Admin à l'instant ? Annulez.", "You didn't just ask to open Admin? Cancel."),
    );
    super::page_with(v, &title, "/compte/ouvrir-admin", "", &card)
}

/// Admin à distance est coupé : le lien ne vaut que sur le réseau de l'ordinateur.
pub fn admin_lan_only(v: &View<'_>, computer: &str, lan: Option<&str>) -> String {
    let title = v.tr("Admin sur le réseau seulement", "Admin on the network only");
    let open = match lan {
        Some(url) => format!("<a class=\"btn kn-primary\" href=\"{}\">{}</a>", esc(url), v.tr("Ouvrir sur le réseau", "Open on the network")),
        None => String::new(),
    };
    let card = format!(
        "<h1>{title}</h1>\n<p class=\"help\">{text}</p>\n{open}\n<p class=\"acct-links\"><a href=\"/compte/ordinateurs{q}\">{back}</a></p>",
        text = match v.lang {
            Lang::Fr => t(
                v.lang,
                &format!("Admin à distance est coupé sur {} : ce lien ne marche que chez vous, sur le même réseau. Allumez-le dans Admin › Hors de chez vous.", esc(computer)),
                "",
            ),
            Lang::En => format!("Remote Admin is off on {}: this link only works at home, on the same network. Turn it on in Admin › Away from home.", esc(computer)),
        },
        q = v.q(),
        back = v.tr("Vos ordinateurs", "Your computers"),
    );
    super::page_with(v, &title, "/compte/admin", "", &card)
}

/// Vers Admin, avec le jeton (lien, et rafraîchissement sans script).
pub fn open_admin_go(v: &View<'_>, target: &str) -> String {
    let title = v.tr("Ouverture d'Admin", "Opening Admin");
    let card = format!(
        "<h1>{title}</h1>\n<p class=\"help\">{help}</p>\n<a class=\"btn kn-primary\" href=\"{target}\">{open}</a>",
        help = v.tr("Admin s'ouvre ; sinon, ce bouton (le lien vaut 5 minutes).", "Admin is opening; otherwise, this button (the link is valid for 5 minutes)."),
        target = esc(target),
        open = v.tr("Ouvrir Admin", "Open Admin"),
    );
    let html = super::page_with(v, &title, "/compte/ouvrir-admin", "", &card);
    html.replacen("<meta name=\"robots\" content=\"noindex\">", &format!("<meta name=\"robots\" content=\"noindex\">\n<meta http-equiv=\"refresh\" content=\"0;url={}\">", esc(target)), 1)
}

// --- Un lien d'accès ------------------------------------------------------------------

/// « {nom} vous ouvre {ordinateur} ».
pub fn link_title(lang: Lang, by: &str, computer: &str) -> String {
    match lang {
        Lang::Fr => t(lang, &format!("{by} vous ouvre {computer}"), ""),
        Lang::En => format!("{by} opens {computer} to you"),
    }
}

/// Un lien d'accès, sans être connecté : se connecter ou créer un compte, puis revenir au lien.
pub fn link_sign_in(v: &View<'_>, here: &str, by: &str, computer: &str) -> String {
    let title = link_title(v.lang, &esc(by), &esc(computer));
    let card = format!(
        "<h1>{title}</h1>\n<p class=\"help\">{help}</p>\n<div class=\"acct-actions\">\n<a class=\"btn kn-primary\" href=\"/compte/connexion{q}\">{sign_in}</a>\n<a class=\"btn btn-ghost\" href=\"/compte/inscription{q}\">{sign_up}</a>\n</div>",
        help = v.tr(
            "Connectez-vous à votre compte KyberNow, ou créez-en un : vous reviendrez ici pour accepter.",
            "Sign in to your KyberNow account, or create one: you'll come back here to accept.",
        ),
        q = v.q(),
        sign_in = v.tr("Se connecter", "Sign in"),
        sign_up = v.tr("Créer un compte", "Create an account"),
    );
    super::page(v, &title, here, false, &card)
}

/// Un lien d'accès, connecté : les droits en mots, l'échéance éventuelle, « Accepter ».
pub fn link_accept(v: &View<'_>, here: &str, by: &str, computer: &str, rights: &Rights, until: Option<i64>, needs_2fa: bool) -> String {
    let title = link_title(v.lang, &esc(by), &esc(computer));
    let until = match until {
        Some(end) => format!(
            "<p class=\"help\">{}</p>\n",
            match v.lang {
                Lang::Fr => t(v.lang, &format!("Jusqu'au {}.", date(v.lang, end - 1)), ""),
                Lang::En => format!("Until {}.", date(v.lang, end - 1)),
            }
        ),
        None => String::new(),
    };
    let admin = if needs_2fa {
        format!(
            "<p class=\"hint\">{}</p>\n",
            v.tr(
                "Le droit Administrer ne comptera qu'avec la double authentification (ou une passkey) sur votre compte.",
                "The Administer right will only count with two-factor authentication (or a passkey) on your account.",
            )
        )
    } else {
        String::new()
    };
    let inner = format!(
        "<div class=\"acct-actions\">\n<button class=\"btn kn-primary\" type=\"submit\">{accept}</button>\n<a class=\"btn btn-ghost\" href=\"/compte/ordinateurs{q}\">{later}</a>\n</div>",
        accept = v.tr("Accepter", "Accept"),
        later = v.tr("Pas maintenant", "Not now"),
        q = v.q(),
    );
    let card = format!(
        "<h1>{title}</h1>\n<p class=\"help\">{what}</p>\n<ul class=\"acct-rights\">{list}</ul>\n{until}{admin}{form}",
        what = v.tr("Ce que vous pourrez y faire :", "What you'll be able to do on it:"),
        list = rights.iter().map(|r| format!("<li><b>{}</b> · {}</li>", right_name(v.lang, r), right_help(v, r))).collect::<String>(),
        form = v.form(here, "", &inner),
    );
    super::page(v, &title, here, false, &card)
}

// --- Le journal ------------------------------------------------------------------------

/// Une ligne du journal en mots.
pub fn log_text(lang: Lang, l: &LogLine) -> String {
    let d = |k: &str| l.detail.get(k).and_then(|v| v.as_str()).unwrap_or("").to_string();
    let who = if l.actor_name.is_empty() { t(lang, "Quelqu'un", "Someone") } else { l.actor_name.clone() };
    let role = |k: &str| Role::parse(&d(k)).map(|r| role_name(lang, r)).unwrap_or_default();
    let rights = || rights_words(lang, &super::super::people::parse_rights(&d("rights")));
    let fr = |s: String| t(lang, &s, "");
    match (l.kind.as_str(), lang) {
        ("organisation_creee", Lang::Fr) => fr(format!("{who} a créé l'organisation")),
        ("organisation_creee", Lang::En) => format!("{who} created the organization"),
        ("organisation_renommee", Lang::Fr) => fr(format!("{who} a renommé l'organisation en {}", d("name"))),
        ("organisation_renommee", Lang::En) => format!("{who} renamed the organization to {}", d("name")),
        ("membre_invite", Lang::Fr) => fr(format!("{} invité comme {} par {who}", d("email"), role("role"))),
        ("membre_invite", Lang::En) => format!("{} invited as {} by {who}", d("email"), role("role")),
        ("membre_ajoute", Lang::Fr) => fr(format!("{who} a ajouté {} comme {}", d("member"), role("role"))),
        ("membre_ajoute", Lang::En) => format!("{who} added {} as {}", d("member"), role("role")),
        ("role_change", Lang::Fr) => fr(format!("{who} a fait de {} un {}", d("member"), role("role"))),
        ("role_change", Lang::En) => format!("{who} made {} {}", d("member"), role("role")),
        ("membre_retire", Lang::Fr) => fr(format!("{who} a retiré {}", d("member"))),
        ("membre_retire", Lang::En) => format!("{who} removed {}", d("member")),
        ("membre_parti", Lang::Fr) => fr(format!("{who} a quitté l'organisation")),
        ("membre_parti", Lang::En) => format!("{who} left the organization"),
        ("invitation_annulee", Lang::Fr) => fr(format!("{who} a annulé l'invitation de {}", d("email"))),
        ("invitation_annulee", Lang::En) => format!("{who} cancelled the invitation of {}", d("email")),
        ("groupe_cree", Lang::Fr) => fr(format!("{who} a créé le groupe {}", d("group"))),
        ("groupe_cree", Lang::En) => format!("{who} created the group {}", d("group")),
        ("groupe_supprime", Lang::Fr) => fr(format!("{who} a supprimé le groupe {}", d("group"))),
        ("groupe_supprime", Lang::En) => format!("{who} deleted the group {}", d("group")),
        ("groupe_membre_ajoute", Lang::Fr) => fr(format!("{who} a mis {} dans le groupe {}", d("member"), d("group"))),
        ("groupe_membre_ajoute", Lang::En) => format!("{who} put {} in the group {}", d("member"), d("group")),
        ("groupe_membre_retire", Lang::Fr) => fr(format!("{who} a retiré {} du groupe {}", d("member"), d("group"))),
        ("groupe_membre_retire", Lang::En) => format!("{who} took {} out of the group {}", d("member"), d("group")),
        ("droits_donnes", Lang::Fr) => fr(format!("{who} a donné {} à {} sur {}", rights(), d("to"), d("station"))),
        ("droits_donnes", Lang::En) => format!("{who} gave {} to {} on {}", rights(), d("to"), d("station")),
        ("droits_retires", Lang::Fr) => fr(format!("{who} a retiré l'accès de {} à {}", d("to"), d("station"))),
        ("droits_retires", Lang::En) => format!("{who} removed {}'s access to {}", d("to"), d("station")),
        ("station_rattachee", Lang::Fr) => fr(format!("{who} a rattaché {}", d("station"))),
        ("station_rattachee", Lang::En) => format!("{who} attached {}", d("station")),
        ("station_detachee", Lang::Fr) => fr(format!("{who} a détaché {}", d("station"))),
        ("station_detachee", Lang::En) => format!("{who} detached {}", d("station")),
        ("station_detachee_par_admin", Lang::Fr) => fr(format!("{} détachée dans Admin", d("station"))),
        ("station_detachee_par_admin", Lang::En) => format!("{} detached in Admin", d("station")),
        ("station_enrolee", Lang::Fr) if !d("group").is_empty() => fr(format!("{} enrôlée par le jeton {}, rangée dans {}", d("station"), d("token"), d("group"))),
        ("station_enrolee", Lang::En) if !d("group").is_empty() => format!("{} enrolled with the token {}, filed in {}", d("station"), d("token"), d("group")),
        ("station_enrolee", Lang::Fr) => fr(format!("{} enrôlée par le jeton {}", d("station"), d("token"))),
        ("station_enrolee", Lang::En) => format!("{} enrolled with the token {}", d("station"), d("token")),
        ("jeton_cree", Lang::Fr) => fr(format!("{who} a créé le jeton d'enrôlement {}", d("token"))),
        ("jeton_cree", Lang::En) => format!("{who} created the enrollment token {}", d("token")),
        ("jeton_revoque", Lang::Fr) => fr(format!("{who} a révoqué le jeton d'enrôlement {}", d("token"))),
        ("jeton_revoque", Lang::En) => format!("{who} revoked the enrollment token {}", d("token")),
        ("admin_distance", Lang::Fr) => fr(format!("{} a ouvert Admin de {} à distance", if d("who").is_empty() { who.clone() } else { d("who") }, d("station"))),
        ("admin_distance", Lang::En) => format!("{} opened {}'s Admin remotely", if d("who").is_empty() { who.clone() } else { d("who") }, d("station")),
        ("acces", Lang::Fr) => fr(format!("{who} a ouvert {} ({})", d("station"), d("device"))),
        ("acces", Lang::En) => format!("{who} opened {} ({})", d("station"), d("device")),
        ("nouvel_appareil", Lang::Fr) => fr(format!("Nouvel appareil pour {} : {} sur {} · e-mail envoyé aux Administrateurs", d("who"), d("device"), d("station"))),
        ("nouvel_appareil", Lang::En) => format!("New device for {}: {} on {} · email sent to the Administrators", d("who"), d("device"), d("station")),
        ("lien_cree", Lang::Fr) => fr(format!("{who} a créé un lien d'accès à {} ({})", d("station"), rights())),
        ("lien_cree", Lang::En) => format!("{who} created an access link to {} ({})", d("station"), rights()),
        ("lien_utilise", Lang::Fr) => fr(format!("{who} a utilisé un lien d'accès à {} ({})", d("station"), rights())),
        ("lien_utilise", Lang::En) => format!("{who} used an access link to {} ({})", d("station"), rights()),
        ("lien_revoque", Lang::Fr) => fr(format!("{who} a révoqué un lien d'accès à {}", d("station"))),
        ("lien_revoque", Lang::En) => format!("{who} revoked an access link to {}", d("station")),
        ("groupe_ordinateurs_cree", Lang::Fr) => fr(format!("{who} a créé le groupe d'ordinateurs {}", d("group"))),
        ("groupe_ordinateurs_cree", Lang::En) => format!("{who} created the computer group {}", d("group")),
        ("groupe_ordinateurs_supprime", Lang::Fr) => fr(format!("{who} a supprimé le groupe d'ordinateurs {}", d("group"))),
        ("groupe_ordinateurs_supprime", Lang::En) => format!("{who} deleted the computer group {}", d("group")),
        ("groupe_ordinateurs_ajoute", Lang::Fr) => fr(format!("{who} a mis {} dans le groupe d'ordinateurs {}", d("station"), d("group"))),
        ("groupe_ordinateurs_ajoute", Lang::En) => format!("{who} put {} in the computer group {}", d("station"), d("group")),
        ("groupe_ordinateurs_retire", Lang::Fr) => fr(format!("{who} a retiré {} du groupe d'ordinateurs {}", d("station"), d("group"))),
        ("groupe_ordinateurs_retire", Lang::En) => format!("{who} took {} out of the computer group {}", d("station"), d("group")),
        ("groupe_ordinateurs_ouvert", Lang::Fr) => fr(format!("{who} a donné {} à {} sur le groupe d'ordinateurs {}", rights(), d("to"), d("group"))),
        ("groupe_ordinateurs_ouvert", Lang::En) => format!("{who} gave {} to {} on the computer group {}", rights(), d("to"), d("group")),
        ("groupe_ordinateurs_ferme", Lang::Fr) => fr(format!("{who} a retiré l'accès de {} au groupe d'ordinateurs {}", d("to"), d("group"))),
        ("groupe_ordinateurs_ferme", Lang::En) => format!("{who} removed {}'s access to the computer group {}", d("to"), d("group")),
        (kind, _) => kind.to_string(),
    }
}

fn when(lang: Lang, unix: i64) -> String {
    let hm = format!("{:02}:{:02}", unix.rem_euclid(86_400) / 3600, unix.rem_euclid(3600) / 60);
    match lang {
        Lang::Fr => format!("{}, {} UTC", date(lang, unix), hm.replace(':', " h ")),
        Lang::En => format!("{}, {hm} UTC", date(lang, unix)),
    }
}

/// Une ligne du journal, avec les mots de la famille ou de l'équipe.
pub fn log_line(lang: Lang, l: &LogLine, kind: Kind) -> String {
    let text = log_text(lang, l);
    match (lang, kind) {
        (Lang::Fr, Kind::Family) => text.replace("l'organisation", "la famille"),
        (Lang::Fr, Kind::Team) => text.replace("l'organisation", "l'équipe"),
        (Lang::En, Kind::Family) => text.replace("the organization", "the family"),
        (Lang::En, Kind::Team) => text.replace("the organization", "the team"),
    }
}

fn log_html(v: &View<'_>, log: &[LogLine], kind: Kind) -> String {
    if log.is_empty() {
        return format!("<p class=\"ka-muted ka-pad\">{}</p>", v.tr("Rien encore.", "Nothing yet."));
    }
    log.iter()
        .map(|l| {
            let from = l.ip.as_deref().map(|ip| format!(" · {}", esc(ip))).unwrap_or_default();
            row(&ic("log"), &esc(&log_line(v.lang, l, kind)), &format!("{}{from}", when(v.lang, l.at)), "")
        })
        .collect()
}

#[cfg(test)]
pub(super) mod tests {
    use super::*;
    use serde_json::json;

    /// Le menu des essais : une équipe, une famille, deux étapes faites.
    pub fn shell() -> Shell {
        Shell {
            name: "Ana <b>".into(),
            email: "ana@exemple.org".into(),
            orgs: vec![
                NavOrg { id: "org_abc".into(), name: "Les Lignes Claires".into(), kind: Kind::Team },
                NavOrg { id: "org_fam".into(), name: "Famille Martin".into(), kind: Kind::Family },
            ],
            computers: 3,
            people: 1,
            start: Start { protected: true, attached: true, opened: false, grouped: false },
        }
    }

    /// Toutes les pages de la console, pour les essais de `super::tests` (typographie, rien du dehors, CSRF).
    pub fn all(v: &View<'_>) -> Vec<(&'static str, String)> {
        let link = |id: i64, used_by: Option<&str>, used_at: Option<i64>, revoked_at: Option<i64>, expires: i64| AccessLink {
            id,
            station: "st1".into(),
            hint: format!("kla_ab{id}cd"),
            rights: Rights::new([Right::View, Right::Session]),
            link_expires_at: expires,
            access_expires_at: (id == 1).then_some(1_800_000_000),
            created_by: Some(1),
            created_by_name: "Ana".into(),
            created_at: 1_699_900_000,
            used_by: used_by.map(|_| 2),
            used_by_name: used_by.unwrap_or("").into(),
            used_at,
            revoked_at,
        };
        let card = |id: &str, rights: Option<Rights>| ComputerCard { id: id.into(), name: "PC du <salon>".into(), online: id == "st1", detail: "de Léa".into(), admin: rights.is_none(), rights };
        let computers_view = ComputersView {
            mine: vec![card("st1", None)],
            opened: vec![card("st2", Some(Rights::new([Right::Lend, Right::Share])))],
            orgs: vec![("org_abc".into(), "Les Lignes Claires".into(), Kind::Team, vec![card("st3", None)])],
            shell: shell(),
        };
        let people_view = PeopleView {
            id: "st1".into(),
            name: "PC du <salon>".into(),
            online: true,
            rows: vec![
                PersonRow { grant: None, group: None, name: "Ana (vous)".into(), detail: "Propriétaire".into(), rights: Rights::new(Right::ALL) },
                PersonRow { grant: Some(4), group: None, name: "Léa".into(), detail: "lea@exemple.fr · jusqu'au 31/12/2026".into(), rights: Rights::new([Right::Session]) },
                PersonRow { grant: None, group: Some("Postes".into()), name: "Les Lignes Claires".into(), detail: t(v.lang, "Par le groupe Postes", "Through the group Postes"), rights: Rights::new([Right::View]) },
            ],
            manage: true,
            can_detach: true,
            my_rights: Rights::new(Right::ALL),
            admin_needs_2fa: false,
            targets: vec![("org_abc".into(), "Les Lignes Claires (toute l'organisation)".into()), ("org_abc:3".into(), "Les Lignes Claires › Graphistes".into())],
            form: ("sam@exemple.fr".into(), Rights::new([Right::Session]), "2026-12-31".into()),
            admin: true,
            links: vec![
                link(1, None, None, None, 1_700_000_000 + 86_400 * 7),
                link(2, Some("Hugo"), Some(1_700_000_100), None, 1_700_000_000 + 86_400),
                link(3, None, None, Some(1_700_000_200), 1_700_000_000 + 86_400),
                link(4, None, None, None, 1_699_000_000),
            ],
            new_link: None,
            link_form: (Rights::new([Right::Session]), 7, String::new()),
            link_open: false,
            now: 1_700_000_000,
            shell: shell(),
        };
        let member = |id: i64, role: Role| Member {
            account: id,
            name: format!("Membre {id}"),
            email: format!("m{id}@exemple.fr"),
            role,
            joined_at: 1_700_000_000,
            two_factor: id != 3,
            groups: vec!["Graphistes".into()],
        };
        let line = |kind: &str| LogLine {
            actor_name: "Claire".into(),
            kind: kind.into(),
            detail: json!({ "station": "Station de rendu", "device": "iPad", "to": "Graphistes", "rights": "session", "member": "Hugo", "role": "admin", "email": "paul@exemple.fr", "group": "Graphistes", "token": "Postes", "who": "Inès", "name": "LLC" }),
            ip: Some("192.0.2.1".into()),
            at: 1_700_000_000,
        };
        let kinds = [
            "organisation_creee",
            "organisation_renommee",
            "membre_invite",
            "membre_ajoute",
            "role_change",
            "membre_retire",
            "membre_parti",
            "invitation_annulee",
            "groupe_cree",
            "groupe_supprime",
            "groupe_membre_ajoute",
            "groupe_membre_retire",
            "droits_donnes",
            "droits_retires",
            "station_rattachee",
            "station_detachee",
            "station_detachee_par_admin",
            "station_enrolee",
            "jeton_cree",
            "jeton_revoque",
            "acces",
            "nouvel_appareil",
            "admin_distance",
            "lien_cree",
            "lien_utilise",
            "lien_revoque",
            "groupe_ordinateurs_cree",
            "groupe_ordinateurs_supprime",
            "groupe_ordinateurs_ajoute",
            "groupe_ordinateurs_retire",
            "groupe_ordinateurs_ouvert",
            "groupe_ordinateurs_ferme",
        ];
        let org_view = |tab: Tab, role: Role, manage: bool| OrgView {
            id: "org_abc".into(),
            name: "Les Lignes Claires".into(),
            kind: Kind::Team,
            role,
            manage,
            needs_2fa: role.manages() && !manage,
            tab,
            me: 1,
            members: vec![member(1, Role::Owner), member(2, Role::Admin), member(3, Role::Member)],
            invites: vec![Invite { id: 9, email: "paul@exemple.fr".into(), role: Role::Member, created_at: 1_700_000_000 }],
            groups: vec![Group { id: 3, name: "Graphistes".into(), members: vec![(3, "Membre 3".into())] }],
            computers: vec![
                OrgComputer { id: "st3".into(), name: "Station de rendu".into(), online: true, access: t(v.lang, "Groupe Graphistes : Session", "Group Graphistes: Session"), admin: manage },
                OrgComputer { id: "st4".into(), name: "Poste <2>".into(), online: false, access: String::new(), admin: manage },
            ],
            station_groups: vec![
                StationGroup {
                    id: 5,
                    name: "Postes de l'atelier".into(),
                    stations: vec![("st3".into(), "Station de rendu".into())],
                    grants: vec![
                        super::super::super::station_groups::GroupGrant {
                            id: 8,
                            sgroup: 5,
                            group_name: "Postes de l'atelier".into(),
                            subject: super::super::super::people::Subject::Org { org: 1, name: "Les Lignes Claires".into() },
                            rights: Rights::new([Right::View]),
                            expires_at: None,
                        },
                        super::super::super::station_groups::GroupGrant {
                            id: 9,
                            sgroup: 5,
                            group_name: "Postes de l'atelier".into(),
                            subject: super::super::super::people::Subject::Group { grp: 3, name: "Graphistes".into(), org_name: "Les Lignes Claires".into() },
                            rights: Rights::new([Right::Session, Right::Lend]),
                            expires_at: Some(1_800_000_000),
                        },
                    ],
                },
                StationGroup { id: 6, name: "Vide".into(), stations: vec![], grants: vec![] },
            ],
            group_targets: vec![("org".into(), "Les Lignes Claires (toute l'équipe)".into()), ("grp:3".into(), "Groupe Graphistes".into()), ("compte:3".into(), "Membre 3".into())],
            log: kinds.iter().map(|k| line(k)).collect(),
            now: 1_700_000_000,
            shell: shell(),
        };
        let token = EnrollToken {
            id: 1,
            name: "Postes graphistes".into(),
            hint: "kne_7qm2x".into(),
            max_stations: 10,
            used: 3,
            grp: Some(3),
            group_name: Some("Graphistes".into()),
            rights: Rights::new([Right::Session]),
            created_at: 1_700_000_000,
            expires_at: 1_800_000_000,
            revoked_at: None,
            sgroup: Some(5),
            sgroup_name: Some("Postes de l'atelier".into()),
        };
        let deploy_view = DeployView {
            id: "org_abc".into(),
            name: "Les Lignes Claires".into(),
            role: Role::Owner,
            tokens: vec![(token.clone(), vec![("GRAPH-01".into(), 1_700_000_000)]), (EnrollToken { revoked_at: Some(1), ..token }, vec![])],
            groups: vec![(3, "Graphistes".into())],
            station_groups: vec![(5, "Postes de l'atelier".into())],
            new_token: Some("kne_abcdefghijklmnopqrstuvwxyz234567".into()),
            relay: "share.test".into(),
            now: 1_700_000_000,
            shell: shell(),
        };
        let people_all = AllPeopleView {
            rows: vec![Granted { name: "Léa".into(), detail: "lea@exemple.fr".into(), computer_id: "st1".into(), computer: "PC du <salon>".into(), rights: Rights::new([Right::Lend]) }],
            computers: vec![("st1".into(), "PC du <salon>".into()), ("st2".into(), "Portable".into())],
            shell: shell(),
        };
        let families_view = FamiliesView {
            orgs: vec![OrgCard { id: "org_abc".into(), name: "Les Lignes Claires".into(), kind: Kind::Team, role: Role::Owner, members: 3, computers: 1 }],
            shell: shell(),
        };
        let welcome_view = Welcome {
            name: "Ana".into(),
            protected: false,
            register: Some(Registration { challenge: "defi".into(), user_id: "dXNlcg".into(), user_name: "ana@exemple.org".into(), display_name: "Ana".into(), exclude: vec![] }),
            computer: None,
        };
        vec![
            ("ordinateurs", computers(v, &computers_view, Some(Notice::StationDetached))),
            ("ordinateurs/vide", computers(v, &ComputersView::default(), None)),
            ("personnes", people(v, &people_view, Some(Notice::PersonAdded))),
            ("personnes/lien-cree", people(v, &PeopleView { new_link: Some("https://share.test/compte/lien/kla_abcdefghijklmnopqrstuvwxyz234567".into()), ..people_view.clone() }, None)),
            ("personnes/lien-trop", people(v, &PeopleView { link_open: true, link_form: (Rights::new([Right::View]), 30, "2026-12-31".into()), ..people_view.clone() }, Some(Notice::TooManyLinks))),
            ("lien/connexion", link_sign_in(v, "/compte/lien/kla_x", "Ana <b>", "PC du <salon>")),
            ("lien/accepter", link_accept(v, "/compte/lien/kla_x", "Ana <b>", "PC du <salon>", &Rights::new([Right::View, Right::Admin]), Some(1_800_000_000), true)),
            ("lien/accepter-sans-fin", link_accept(v, "/compte/lien/kla_x", "Ana", "PC", &Rights::new([Right::Session]), None, false)),
            ("organisation/ordinateurs-membre", org(v, &org_view(Tab::Computers, Role::Member, false), None)),
            ("personnes/sans-gerer", people(v, &PeopleView { manage: false, admin_needs_2fa: true, my_rights: Rights::new([Right::Session]), ..people_view.clone() }, None)),
            ("organisation", org(v, &org_view(Tab::Members, Role::Owner, true), Some(Notice::OrgCreated))),
            ("organisation/membre", org(v, &org_view(Tab::Members, Role::Member, false), None)),
            ("organisation/sans-2fa", org(v, &org_view(Tab::Members, Role::Admin, false), Some(Notice::NeedTwoFactor))),
            ("organisation/groupes", org(v, &org_view(Tab::Groups, Role::Admin, true), None)),
            ("organisation/ordinateurs", org(v, &org_view(Tab::Computers, Role::Owner, true), None)),
            ("organisation/journal", org(v, &org_view(Tab::Log, Role::Owner, true), None)),
            ("organisation/creer", create_org(v, "", Some(Kind::Team), true, &shell(), None)),
            ("organisation/creer-famille", create_org(v, "", Some(Kind::Family), true, &shell(), None)),
            ("organisation/creer-choix", create_org(v, "", None, true, &shell(), None)),
            ("organisation/creer-sans-2fa", create_org(v, "", Some(Kind::Family), false, &shell(), None)),
            ("organisation/famille", org(v, &OrgView { kind: Kind::Family, ..org_view(Tab::Groups, Role::Owner, true) }, None)),
            ("personnes-toutes", all_people(v, &people_all)),
            ("personnes-toutes/vide", all_people(v, &AllPeopleView { shell: shell(), ..AllPeopleView::default() })),
            ("familles", families(v, &families_view)),
            ("familles/vide", families(v, &FamiliesView { shell: shell(), ..FamiliesView::default() })),
            ("bienvenue/0", welcome(v, 0, &welcome_view)),
            ("bienvenue/1", welcome(v, 1, &welcome_view)),
            ("bienvenue/1-protege", welcome(v, 1, &Welcome { protected: true, register: None, ..welcome_view.clone() })),
            ("bienvenue/2", welcome(v, 2, &welcome_view)),
            ("bienvenue/2-rattache", welcome(v, 2, &Welcome { computer: Some("PC du <salon>".into()), ..welcome_view.clone() })),
            ("bienvenue/3", welcome(v, 3, &welcome_view)),
            ("deployer", deploy(v, &deploy_view, Some(Notice::TokenRevoked))),
            ("ouvrir-admin", open_admin(v, "PC du salon", "192.168.1.20:8443", &[("station", "st1"), ("nonce", "n"), ("retour", "x")], None)),
            ("ouvrir-admin/distance", open_admin(v, "PC du salon", "st1.share.test", &[("station", "st1"), ("nonce", "n"), ("retour", "x")], Some(&["Y3JlZA".to_string()]))),
            ("ouvrir-admin/sans-passkey", open_admin(v, "PC du salon", "st1.share.test", &[("station", "st1"), ("nonce", "n"), ("retour", "x")], Some(&[]))),
            ("admin-reseau", admin_lan_only(v, "PC du salon", Some("https://192.168.1.20:8443/api/login/kybernow/start"))),
        ]
    }

    #[test]
    fn journal_en_mots() {
        for lang in [Lang::Fr, Lang::En] {
            let text = log_text(lang, &LogLine { actor_name: "Claire".into(), kind: "droits_donnes".into(), detail: json!({ "station": "Station de rendu", "to": "Graphistes", "rights": "session,share" }), ip: None, at: 0 });
            assert!(text.contains("Claire") && text.contains("Graphistes") && text.contains("Station de rendu"), "{text}");
        }
        let fr = log_text(Lang::Fr, &LogLine { actor_name: String::new(), kind: "acces".into(), detail: json!({ "station": "PC", "device": "iPad" }), ip: None, at: 0 });
        assert_eq!(fr, "Quelqu'un a ouvert PC (iPad)");
    }
}

