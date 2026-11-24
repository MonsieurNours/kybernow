//! La console des comptes (docs/comptes.md, lots 4 et 5), sur le modèle de la
//! maquette : « Ordinateurs » (les siens, ceux qu'on lui a ouverts, ceux de
//! ses organisations), « Personnes » d'un ordinateur avec « Ajouter une
//! personne », « Organisation » (membres et rôles, groupes, ordinateurs,
//! journal) et « Déployer des Stations » (jetons d'enrôlement). Comme les
//! autres pages des comptes : des formulaires, sans script.

use kybernow_tunnel::access::{Right, Rights};

use super::super::orgs::{EnrollToken, Group, Invite, LogLine, Member, Role};
use super::{Notice, View, date, esc, field, notice_html, page_with, pct};
use crate::pages::{Lang, t};

/// Où l'on est dans la barre de la console.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Here {
    Computers,
    Org(String),
    Account,
    None,
}

/// La barre : « Ordinateurs », chaque organisation, « Compte ».
pub fn nav(v: &View<'_>, orgs: &[(String, String)], here: Here) -> String {
    let link = |href: String, label: &str, current: bool| {
        let cur = if current { " aria-current=\"page\"" } else { "" };
        format!("<a href=\"{href}\"{cur}>{}</a>", esc(label))
    };
    let mut items = vec![link(format!("/compte/ordinateurs{}", v.q()), &v.tr("Ordinateurs", "Computers"), here == Here::Computers)];
    for (id, name) in orgs {
        items.push(link(format!("/compte/organisation{}&amp;id={}", v.q(), pct(id)), name, here == Here::Org(id.clone())));
    }
    items.push(link(format!("/compte{}", v.q()), &v.tr("Compte", "Account"), here == Here::Account));
    format!("<nav class=\"console-nav\" aria-label=\"{}\">{}</nav>\n", v.tr("Console", "Console"), items.join(""))
}

fn console(v: &View<'_>, title: &str, here_path: &str, card: &str) -> String {
    page_with(v, title, here_path, " acct-console", card)
}

/// Le nom d'un droit.
pub fn right_name(lang: Lang, r: Right) -> String {
    match r {
        Right::Session => t(lang, "Session", "Session"),
        Right::Lend => t(lang, "Prêt", "Lending"),
        Right::Share => t(lang, "Partage", "Sharing"),
        Right::Admin => t(lang, "Administrer", "Administer"),
    }
}

/// Les droits en mots : « Session, Partage ».
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

fn right_help(v: &View<'_>, r: Right) -> String {
    match r {
        Right::Session => v.tr("Le bureau entier, comme vous.", "The whole desktop, like you."),
        Right::Lend => v.tr("Le lanceur, avec les apps permises dans Admin.", "The launcher, with the apps allowed in Admin."),
        Right::Share => v.tr("Rejoindre vos partages sans code.", "Join your shares without a code."),
        Right::Admin => v.tr(
            "Gérer les accès, redémarrer et mettre à jour la Station. Double authentification exigée.",
            "Manage access, restart and update the Station. Two-factor authentication required.",
        ),
    }
}

/// Les cases des droits (`session`, `lend`, `share`, `admin`).
fn rights_fieldset(v: &View<'_>, legend: &str, checked: &Rights, with_help: bool) -> String {
    let boxes: String = Right::ALL
        .iter()
        .map(|r| {
            let on = if checked.has(*r) { " checked" } else { "" };
            let help = if with_help { format!("<br><small>{}</small>", right_help(v, *r)) } else { String::new() };
            format!("<label class=\"check\"><input type=\"checkbox\" name=\"{}\" value=\"1\"{on}> <span>{}{help}</span></label>\n", r.as_str(), right_name(v.lang, *r))
        })
        .collect();
    format!("<fieldset>\n<legend class=\"mono-label\">{legend}</legend>\n{boxes}</fieldset>")
}

fn hidden(name: &str, value: &str) -> String {
    format!("<input type=\"hidden\" name=\"{name}\" value=\"{}\">", esc(value))
}

fn small_button(label: &str) -> String {
    format!("<button class=\"btn btn-ghost btn-sm\" type=\"submit\">{label}</button>")
}

fn state_pill(v: &View<'_>, online: bool) -> String {
    if online {
        format!("<span class=\"pill pill-on\">{}</span>", v.tr("En ligne", "Online"))
    } else {
        format!("<span class=\"pill\">{}</span>", v.tr("Hors ligne", "Offline"))
    }
}

// --- Ordinateurs -----------------------------------------------------------------------

/// Un ordinateur, en carte.
#[derive(Debug, Clone)]
pub struct ComputerCard {
    pub id: String,
    pub name: String,
    pub online: bool,
    /// « de Léa Martin », « Groupe Graphistes · 4 personnes »…
    pub detail: String,
    /// Ses droits, quand l'ordinateur n'est pas à lui.
    pub rights: Option<Rights>,
}

/// La page « Ordinateurs ».
#[derive(Debug, Clone, Default)]
pub struct ComputersView {
    pub mine: Vec<ComputerCard>,
    pub opened: Vec<ComputerCard>,
    /// `(identifiant, nom, ordinateurs)` par organisation.
    pub orgs: Vec<(String, String, Vec<ComputerCard>)>,
    pub nav_orgs: Vec<(String, String)>,
}

fn cards(v: &View<'_>, list: &[ComputerCard]) -> String {
    let items: String = list
        .iter()
        .map(|c| {
            let rights = match &c.rights {
                Some(r) => format!(
                    "<span class=\"pills\">{}</span>",
                    r.iter().map(|r| format!("<span class=\"pill\">{}</span>", right_name(v.lang, r))).collect::<Vec<_>>().join("")
                ),
                None => String::new(),
            };
            format!(
                "<a class=\"card-link\" href=\"/compte/ordinateur{q}&amp;id={id}\"><span class=\"card-top\"><b>{name}</b>{state}</span><small>{detail}</small>{rights}</a>\n",
                q = v.q(),
                id = pct(&c.id),
                name = esc(&c.name),
                state = state_pill(v, c.online),
                detail = esc(&c.detail),
            )
        })
        .collect();
    format!("<div class=\"cards\">\n{items}</div>")
}

pub fn computers(v: &View<'_>, c: &ComputersView, notice: Option<Notice>) -> String {
    let title = v.tr("Ordinateurs", "Computers");
    let mut body = String::new();
    let section = |h: &str, list: &[ComputerCard]| format!("<section>\n<h2 class=\"mono-label\">{}</h2>\n{}\n</section>\n", esc(h), cards(v, list));
    if !c.mine.is_empty() {
        body.push_str(&section(&v.tr("À vous", "Yours"), &c.mine));
    }
    if !c.opened.is_empty() {
        body.push_str(&section(&v.tr("Ouverts pour vous", "Opened for you"), &c.opened));
    }
    for (_, name, list) in &c.orgs {
        if !list.is_empty() {
            body.push_str(&section(name, list));
        }
    }
    if body.is_empty() {
        body = format!("<p class=\"help\">{}</p>\n", v.tr("Aucun ordinateur pour l'instant.", "No computers yet."));
    }
    let card = format!(
        "{nav}<div class=\"console-head\"><div><h1>{title}</h1>\n<p class=\"help\">{help}</p></div><p class=\"hint\">{attach}</p></div>\n{notice}{body}<p class=\"acct-links\"><a href=\"/compte/organisations/creer{q}\">{create}</a></p>",
        nav = nav(v, &c.nav_orgs, Here::Computers),
        help = v.tr("Les vôtres, ceux qu'on vous a ouverts, et ceux de vos organisations.", "Yours, those opened for you, and those of your organizations."),
        attach = v.tr(
            "Rattacher un ordinateur : Admin › Compte KyberNow › « Se connecter », sur l'ordinateur.",
            "Attach a computer: Admin › KyberNow account › \"Sign in\", on the computer.",
        ),
        notice = notice_html(v.lang, notice),
        q = v.q(),
        create = v.tr("Créer une organisation", "Create an organization"),
    );
    console(v, &title, "/compte/ordinateurs", &card)
}

// --- Personnes d'un ordinateur ---------------------------------------------------------

/// Une ligne de « Personnes ».
#[derive(Debug, Clone)]
pub struct PersonRow {
    /// Le droit donné (`None` : le propriétaire, ou les gérants de l'organisation).
    pub grant: Option<i64>,
    pub name: String,
    pub detail: String,
    pub rights: Rights,
}

/// La page « Personnes » d'un ordinateur.
#[derive(Debug, Clone)]
pub struct PeopleView {
    pub id: String,
    pub name: String,
    pub online: bool,
    pub rows: Vec<PersonRow>,
    /// Ses accès se gèrent ici.
    pub manage: bool,
    /// « Détacher » (propriétaire, gérant de l'organisation).
    pub can_detach: bool,
    /// Ses droits à lui, s'il ne gère pas.
    pub my_rights: Rights,
    /// Le droit Administrer lui est donné, sans double authentification.
    pub admin_needs_2fa: bool,
    /// « Ouvrir à » : `(valeur, libellé)` des organisations et groupes possibles.
    pub targets: Vec<(String, String)>,
    /// Le formulaire « Ajouter une personne » tel qu'envoyé (adresse, droits, échéance).
    pub form: (String, Rights, String),
    pub nav_orgs: Vec<(String, String)>,
}

fn dots(v: &View<'_>, rights: &Rights) -> String {
    Right::ALL
        .iter()
        .map(|r| {
            let (class, word) = if rights.has(*r) { ("dot on", v.tr("oui", "yes")) } else { ("dot", v.tr("non", "no")) };
            format!("<td><span class=\"{class}\" role=\"img\" aria-label=\"{} : {word}\"></span></td>", right_name(v.lang, *r))
        })
        .collect()
}

pub fn people(v: &View<'_>, p: &PeopleView, notice: Option<Notice>) -> String {
    let title = v.tr("Personnes", "People");
    let heads: String = Right::ALL.iter().map(|r| format!("<th scope=\"col\">{}</th>", right_name(v.lang, *r))).collect();
    let rows: String = p
        .rows
        .iter()
        .map(|r| {
            let remove = match (p.manage, r.grant) {
                (true, Some(g)) => v.form(
                    "/compte/ordinateur/retirer",
                    "acct-inline",
                    &format!("{}\n{}\n{}", hidden("id", &p.id), hidden("acces", &g.to_string()), small_button(&v.tr("Retirer", "Remove"))),
                ),
                _ => String::new(),
            };
            format!(
                "<tr><th scope=\"row\"><b>{}</b><small>{}</small></th>{}<td>{remove}</td></tr>\n",
                esc(&r.name),
                esc(&r.detail),
                dots(v, &r.rights)
            )
        })
        .collect();
    let table = format!(
        "<div class=\"table-scroll\"><table class=\"rights-table\">\n<thead><tr><th scope=\"col\">{who}</th>{heads}<th><span class=\"visually-hidden\">{act}</span></th></tr></thead>\n<tbody>\n{rows}</tbody>\n</table></div>\n<p class=\"hint\">{legend}</p>",
        who = v.tr("Personne", "Person"),
        act = v.tr("Actions", "Actions"),
        legend = v.tr(
            "● violet : droit donné · gris : non. Le prêt suit les apps et les créneaux réglés dans Admin (page « Prêt »).",
            "● purple: right given · grey: not. Lending follows the apps and time slots set in Admin (\"Lending\" page).",
        ),
    );
    let mut forms = String::new();
    if p.manage {
        let (email, rights, until) = &p.form;
        let inner = format!(
            "{id}\n{email}\n{rights}\n{until}\n<button class=\"btn kn-primary\" type=\"submit\">{send}</button>",
            id = hidden("id", &p.id),
            email = field(
                &v.tr("Adresse e-mail", "Email address"),
                &format!(
                    r#"<input id="email" name="email" type="email" inputmode="email" autocomplete="off" autocapitalize="off" spellcheck="false" maxlength="254" value="{}" required>"#,
                    esc(email)
                ),
            ),
            rights = rights_fieldset(v, &v.tr("Droits", "Rights"), rights, true),
            until = field(
                &v.tr("Jusqu'au (facultatif)", "Until (optional)"),
                &format!(r#"<input id="fin" name="fin" type="date" value="{}" placeholder="{}">"#, esc(until), v.tr("jj/mm/aaaa", "dd/mm/yyyy")),
            ),
            send = v.tr("Envoyer l'invitation", "Send the invitation"),
        );
        forms.push_str(&format!(
            "<section class=\"console-box\" id=\"ajouter\">\n<h2>{}</h2>\n<p class=\"hint\">{}</p>\n{}\n</section>\n",
            v.tr("Ajouter une personne", "Add a person"),
            v.tr(
                "Avec un compte, l'ordinateur apparaît chez elle ; sans compte, l'e-mail lui propose d'en créer un. Ajouter la même adresse change ses droits.",
                "With an account, the computer shows up for them; without one, the email offers to create it. Adding the same address changes their rights.",
            ),
            v.form("/compte/ordinateur/ajouter", "", &inner)
        ));
        if !p.targets.is_empty() {
            let options: String = p.targets.iter().map(|(val, label)| format!("<option value=\"{}\">{}</option>", esc(val), esc(label))).collect();
            let inner = format!(
                "{id}\n<div class=\"field\"><label class=\"mono-label\" for=\"a\">{to}</label>\n<select id=\"a\" name=\"a\" required>{options}</select></div>\n{rights}\n<button class=\"btn btn-ghost\" type=\"submit\">{open}</button>",
                id = hidden("id", &p.id),
                to = v.tr("À", "To"),
                rights = rights_fieldset(v, &v.tr("Droits", "Rights"), &Rights::new([Right::Session]), false),
                open = v.tr("Ouvrir", "Open"),
            );
            forms.push_str(&format!(
                "<section class=\"console-box\">\n<h2>{}</h2>\n<p class=\"hint\">{}</p>\n{}\n</section>\n",
                v.tr("Ouvrir à une organisation", "Open to an organization"),
                v.tr(
                    "Toute l'organisation, ou l'un de ses groupes, comme une personne. Ouvrir de nouveau change les droits.",
                    "The whole organization, or one of its groups, like a person. Opening again changes the rights.",
                ),
                v.form("/compte/ordinateur/ouvrir", "", &inner)
            ));
        }
    } else {
        let mine = if p.my_rights.is_empty() { v.tr("aucun", "none") } else { rights_words(v.lang, &p.my_rights) };
        forms.push_str(&format!("<p class=\"help\">{} <b>{}</b></p>\n", v.tr("Vos droits sur cet ordinateur :", "Your rights on this computer:"), esc(&mine)));
    }
    if p.admin_needs_2fa {
        forms.push_str(&format!(
            "<p class=\"notice\" role=\"status\">{} <a href=\"/compte/double-authentification{}\">{}</a></p>\n",
            v.tr(
                "Le droit Administrer vous est donné, mais il ne compte qu'avec la double authentification.",
                "The Administer right is given to you, but it only counts with two-factor authentication.",
            ),
            v.q(),
            v.tr("L'activer", "Turn it on"),
        ));
    }
    if p.can_detach {
        forms.push_str(&v.form(
            "/compte/ordinateurs/detacher",
            "acct-inline console-detach",
            &format!("{}\n{}\n{}", hidden("id", &p.id), hidden("depuis", "console"), small_button(&v.tr("Détacher cet ordinateur", "Detach this computer"))),
        ));
    }
    let card = format!(
        "{nav}<p class=\"crumbs\"><a href=\"/compte/ordinateurs{q}\">{computers}</a> › <b>{name}</b> {state}</p>\n<h1>{title}</h1>\n{notice}{table}\n{forms}",
        nav = nav(v, &p.nav_orgs, Here::None),
        q = v.q(),
        computers = v.tr("Ordinateurs", "Computers"),
        name = esc(&p.name),
        state = state_pill(v, p.online),
        notice = notice_html(v.lang, notice),
    );
    console(v, &format!("{title} · {}", esc(&p.name)), "/compte/ordinateur", &card)
}

// --- Organisation ----------------------------------------------------------------------

/// L'onglet de la page d'une organisation.
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

/// Un ordinateur de l'organisation.
#[derive(Debug, Clone)]
pub struct OrgComputer {
    pub id: String,
    pub name: String,
    pub online: bool,
    /// Ses accès donnés, en mots (« Groupe Graphistes : Session »).
    pub access: String,
}

/// La page d'une organisation.
#[derive(Debug, Clone)]
pub struct OrgView {
    pub id: String,
    pub name: String,
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
    pub log: Vec<LogLine>,
    pub nav_orgs: Vec<(String, String)>,
}

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
        ("station_enrolee", Lang::Fr) => fr(format!("{} enrôlée par le jeton {}", d("station"), d("token"))),
        ("station_enrolee", Lang::En) => format!("{} enrolled with the token {}", d("station"), d("token")),
        ("jeton_cree", Lang::Fr) => fr(format!("{who} a créé le jeton d'enrôlement {}", d("token"))),
        ("jeton_cree", Lang::En) => format!("{who} created the enrollment token {}", d("token")),
        ("jeton_revoque", Lang::Fr) => fr(format!("{who} a révoqué le jeton d'enrôlement {}", d("token"))),
        ("jeton_revoque", Lang::En) => format!("{who} revoked the enrollment token {}", d("token")),
        ("acces", Lang::Fr) => fr(format!("{who} a ouvert {} ({})", d("station"), d("device"))),
        ("acces", Lang::En) => format!("{who} opened {} ({})", d("station"), d("device")),
        ("nouvel_appareil", Lang::Fr) => fr(format!("Nouvel appareil pour {} : {} sur {} · e-mail envoyé aux Administrateurs", d("who"), d("device"), d("station"))),
        ("nouvel_appareil", Lang::En) => format!("New device for {}: {} on {} · email sent to the Administrators", d("who"), d("device"), d("station")),
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

fn log_html(v: &View<'_>, log: &[LogLine]) -> String {
    if log.is_empty() {
        return format!("<p class=\"hint\">{}</p>", v.tr("Rien encore.", "Nothing yet."));
    }
    log.iter()
        .map(|l| {
            let from = l.ip.as_deref().map(|ip| format!(" · {}", esc(ip))).unwrap_or_default();
            format!("<div class=\"row\"><div><span>{}</span><small>{}{from}</small></div></div>\n", esc(&log_text(v.lang, l)), when(v.lang, l.at))
        })
        .collect()
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

pub fn org(v: &View<'_>, o: &OrgView, notice: Option<Notice>) -> String {
    let q_id = format!("{}&amp;id={}", v.q(), pct(&o.id));
    let tabs: String = [
        (Tab::Members, v.tr("Membres", "Members")),
        (Tab::Groups, v.tr("Groupes", "Groups")),
        (Tab::Computers, v.tr("Ordinateurs", "Computers")),
        (Tab::Log, v.tr("Journal", "Log")),
    ]
    .iter()
    .map(|(tab, label)| {
        let cur = if *tab == o.tab { " aria-current=\"page\"" } else { "" };
        format!("<a href=\"/compte/organisation{q_id}&amp;vue={}\"{cur}>{label}</a>", tab.word())
    })
    .collect();
    let you = match v.lang {
        Lang::Fr => t(v.lang, &format!("Organisation · vous êtes {} · double authentification exigée des Propriétaires et Administrateurs", role_name(v.lang, o.role)), ""),
        Lang::En => format!("Organization · you are {} · two-factor authentication required of Owners and Administrators", role_name(v.lang, o.role)),
    };
    let warn = if o.needs_2fa {
        format!(
            "<p class=\"notice\" role=\"status\">{} <a href=\"/compte/double-authentification{}\">{}</a></p>\n",
            v.tr(
                "Pour gérer l'organisation, activez la double authentification (ou ajoutez une passkey) sur votre compte.",
                "To manage the organization, turn on two-factor authentication (or add a passkey) on your account.",
            ),
            v.q(),
            v.tr("L'activer", "Turn it on"),
        )
    } else {
        String::new()
    };
    let id_field = hidden("id", &o.id);
    let body = match o.tab {
        Tab::Members => {
            let rows: String = o
                .members
                .iter()
                .map(|m| {
                    let mut detail = vec![m.email.clone()];
                    detail.push(if m.two_factor { v.tr("double authentification", "two-factor") } else { v.tr("sans double authentification", "no two-factor") });
                    detail.extend(m.groups.iter().cloned());
                    let name = if m.account == o.me { format!("{} ({})", m.name, v.tr("vous", "you")) } else { m.name.clone() };
                    // Un Administrateur ne touche pas aux Propriétaires ; seul un Propriétaire en nomme.
                    let editable = o.manage && (o.role == Role::Owner || m.role != Role::Owner);
                    let acts = if editable {
                        let role = v.form(
                            "/compte/organisation/role",
                            "acct-inline console-role",
                            &format!(
                                "{id_field}\n{}\n{}\n{}",
                                hidden("compte", &m.account.to_string()),
                                role_select(v, "role", m.role, o.role == Role::Owner),
                                small_button(&v.tr("Changer", "Change"))
                            ),
                        );
                        let remove = if m.account == o.me {
                            String::new()
                        } else {
                            v.form(
                                "/compte/organisation/retirer",
                                "acct-inline",
                                &format!("{id_field}\n{}\n{}", hidden("compte", &m.account.to_string()), small_button(&v.tr("Retirer", "Remove"))),
                            )
                        };
                        format!("<div class=\"acts\">{role}\n{remove}</div>")
                    } else {
                        format!("<span class=\"pill\">{}</span>", role_name(v.lang, m.role))
                    };
                    format!("<div class=\"row\"><div><b>{}</b><small>{}</small></div>{acts}</div>\n", esc(&name), esc(&detail.join(" · ")))
                })
                .collect();
            let invites: String = o
                .invites
                .iter()
                .map(|i| {
                    let cancel = if o.manage {
                        v.form(
                            "/compte/organisation/retirer",
                            "acct-inline",
                            &format!("{id_field}\n{}\n{}", hidden("invitation", &i.id.to_string()), small_button(&v.tr("Annuler", "Cancel"))),
                        )
                    } else {
                        String::new()
                    };
                    format!(
                        "<div class=\"row\"><div><b>{}</b><small>{} · {}</small></div>{cancel}</div>\n",
                        esc(&i.email),
                        v.tr("Invitation envoyée", "Invitation sent"),
                        role_name(v.lang, i.role)
                    )
                })
                .collect();
            let invite = if o.manage {
                let inner = format!(
                    "{id_field}\n<div class=\"inline-fields\">{email}\n<div class=\"field\"><label class=\"mono-label\" for=\"role\">{role_l}</label>\n{role}</div></div>\n<button class=\"btn kn-primary\" type=\"submit\">{send}</button>",
                    email = field(
                        &v.tr("Adresse e-mail", "Email address"),
                        r#"<input id="email" name="email" type="email" inputmode="email" autocomplete="off" autocapitalize="off" spellcheck="false" maxlength="254" required>"#,
                    ),
                    role_l = v.tr("Rôle", "Role"),
                    role = role_select(v, "role", Role::Member, o.role == Role::Owner).replacen("<select ", "<select id=\"role\" ", 1),
                    send = v.tr("Inviter un membre", "Invite a member"),
                );
                format!("<section class=\"console-box\">\n<h2>{}</h2>\n{}\n</section>\n", v.tr("Inviter un membre", "Invite a member"), v.form("/compte/organisation/inviter", "", &inner))
            } else {
                String::new()
            };
            let leave = v.form(
                "/compte/organisation/quitter",
                "acct-inline console-detach",
                &format!("{id_field}\n{}", small_button(&v.tr("Quitter l'organisation", "Leave the organization"))),
            );
            let rename = if o.manage && o.role == Role::Owner {
                let inner = format!(
                    "{id_field}\n<div class=\"inline-fields\">{}</div>\n<button class=\"btn btn-ghost\" type=\"submit\">{}</button>",
                    field(&v.tr("Nom de l'organisation", "Organization name"), &format!(r#"<input id="nom" name="nom" maxlength="64" value="{}" required>"#, esc(&o.name))),
                    v.tr("Renommer", "Rename"),
                );
                v.form("/compte/organisation/renommer", "", &inner)
            } else {
                String::new()
            };
            format!(
                "<div class=\"console-cols\">\n<section class=\"console-box\">\n<h2>{members_h}</h2>\n{rows}{invites}</section>\n<section class=\"console-box\">\n<h2>{log_h}</h2>\n{log}\n<p class=\"acct-links\"><a href=\"/compte/organisation{q_id}&amp;vue=journal\">{all}</a></p></section>\n</div>\n{invite}{rename}{leave}",
                members_h = v.tr("Membres", "Members"),
                log_h = v.tr("Journal", "Log"),
                log = log_html(v, &o.log[..o.log.len().min(8)]),
                all = v.tr("Tout le journal", "The whole log"),
            )
        }
        Tab::Groups => {
            let list: String = o
                .groups
                .iter()
                .map(|g| {
                    let members: String = g
                        .members
                        .iter()
                        .map(|(acc, name)| {
                            let out = if o.manage {
                                v.form(
                                    "/compte/organisation/groupes/membre",
                                    "acct-inline",
                                    &format!(
                                        "{id_field}\n{}\n{}\n{}\n{}",
                                        hidden("groupe", &g.id.to_string()),
                                        hidden("compte", &acc.to_string()),
                                        hidden("action", "retirer"),
                                        small_button(&v.tr("Retirer", "Remove"))
                                    ),
                                )
                            } else {
                                String::new()
                            };
                            format!("<div class=\"row\"><div><b>{}</b></div>{out}</div>\n", esc(name))
                        })
                        .collect();
                    let manage = if o.manage {
                        let options: String = o
                            .members
                            .iter()
                            .filter(|m| !g.members.iter().any(|(a, _)| *a == m.account))
                            .map(|m| format!("<option value=\"{}\">{}</option>", m.account, esc(&m.name)))
                            .collect();
                        let add = if options.is_empty() {
                            String::new()
                        } else {
                            v.form(
                                "/compte/organisation/groupes/membre",
                                "acct-inline console-role",
                                &format!(
                                    "{id_field}\n{}\n{}\n<select name=\"compte\" aria-label=\"{}\">{options}</select>\n{}",
                                    hidden("groupe", &g.id.to_string()),
                                    hidden("action", "ajouter"),
                                    v.tr("Membre", "Member"),
                                    small_button(&v.tr("Ajouter au groupe", "Add to the group"))
                                ),
                            )
                        };
                        let delete = v.form(
                            "/compte/organisation/groupes/supprimer",
                            "acct-inline",
                            &format!("{id_field}\n{}\n{}", hidden("groupe", &g.id.to_string()), small_button(&v.tr("Supprimer le groupe", "Delete the group"))),
                        );
                        format!("<div class=\"acct-actions\">{add}\n{delete}</div>")
                    } else {
                        String::new()
                    };
                    let count = match v.lang {
                        Lang::Fr => format!("{} personne{}", g.members.len(), if g.members.len() > 1 { "s" } else { "" }),
                        Lang::En => format!("{} {}", g.members.len(), if g.members.len() == 1 { "person" } else { "people" }),
                    };
                    format!("<section class=\"console-box\">\n<h2>{} <small>{count}</small></h2>\n{members}{manage}\n</section>\n", esc(&g.name))
                })
                .collect();
            let create = if o.manage {
                let inner = format!(
                    "{id_field}\n<div class=\"inline-fields\">{}</div>\n<button class=\"btn kn-primary\" type=\"submit\">{}</button>",
                    field(&v.tr("Nom du groupe", "Group name"), r#"<input id="nom" name="nom" maxlength="64" placeholder="Graphistes" required>"#),
                    v.tr("Créer le groupe", "Create the group"),
                );
                format!("<section class=\"console-box\">\n<h2>{}</h2>\n{}\n</section>\n", v.tr("Nouveau groupe", "New group"), v.form("/compte/organisation/groupes/creer", "", &inner))
            } else {
                String::new()
            };
            let empty = if o.groups.is_empty() { format!("<p class=\"help\">{}</p>\n", v.tr("Aucun groupe pour l'instant.", "No groups yet.")) } else { String::new() };
            format!(
                "<p class=\"hint\">{}</p>\n{empty}{list}{create}",
                v.tr(
                    "Les droits se donnent à un groupe depuis la page « Personnes » d'un ordinateur, ou avec un jeton d'enrôlement.",
                    "Rights are given to a group from a computer's \"People\" page, or with an enrollment token.",
                )
            )
        }
        Tab::Computers => {
            let rows: String = o
                .computers
                .iter()
                .map(|c| {
                    format!(
                        "<div class=\"row\"><div><b><a href=\"/compte/ordinateur{}&amp;id={}\">{}</a></b><small>{}</small></div>{}</div>\n",
                        v.q(),
                        pct(&c.id),
                        esc(&c.name),
                        esc(&c.access),
                        state_pill(v, c.online)
                    )
                })
                .collect();
            let empty = if o.computers.is_empty() { format!("<p class=\"help\">{}</p>\n", v.tr("Aucun ordinateur pour l'instant.", "No computers yet.")) } else { String::new() };
            let deploy = if o.manage {
                format!("<p class=\"acct-links\"><a class=\"btn kn-primary\" href=\"/compte/organisation/deployer{q_id}\">{}</a></p>", v.tr("Déployer des Stations", "Deploy Stations"))
            } else {
                String::new()
            };
            format!(
                "<p class=\"hint\">{}</p>\n<section class=\"console-box\">\n{empty}{rows}</section>\n{deploy}",
                v.tr(
                    "Rattacher un ordinateur à l'organisation : Admin › Compte KyberNow › « Se connecter », puis choisir l'organisation comme propriétaire ; ou un jeton d'enrôlement.",
                    "Attach a computer to the organization: Admin › KyberNow account › \"Sign in\", then choose the organization as owner; or an enrollment token.",
                )
            )
        }
        Tab::Log => format!("<section class=\"console-box\">\n{}</section>\n", log_html(v, &o.log)),
    };
    let card = format!(
        "{nav}<div class=\"console-head\"><div><h1>{name}</h1>\n<p class=\"hint\">{you}</p></div></div>\n<nav class=\"tabs\" aria-label=\"{tabs_l}\">{tabs}</nav>\n{notice}{warn}{body}",
        nav = nav(v, &o.nav_orgs, Here::Org(o.id.clone())),
        name = esc(&o.name),
        tabs_l = v.tr("Sections", "Sections"),
        notice = notice_html(v.lang, notice),
    );
    console(v, &o.name, "/compte/organisation", &card)
}

/// « Créer une organisation ».
pub fn create_org(v: &View<'_>, name: &str, two_factor: bool, nav_orgs: &[(String, String)], notice: Option<Notice>) -> String {
    let title = v.tr("Créer une organisation", "Create an organization");
    let body = if two_factor {
        let inner = format!(
            "{}\n<button class=\"btn kn-primary\" type=\"submit\">{}</button>",
            field(&v.tr("Nom", "Name"), &format!(r#"<input id="nom" name="nom" maxlength="64" placeholder="Les Lignes Claires" value="{}" required>"#, esc(name))),
            v.tr("Créer", "Create"),
        );
        v.form("/compte/organisations/creer", "", &inner)
    } else {
        format!(
            "<p class=\"notice\" role=\"status\">{} <a href=\"/compte/double-authentification{}\">{}</a></p>",
            v.tr(
                "Les Propriétaires d'une organisation ont la double authentification : activez-la d'abord (ou ajoutez une passkey).",
                "An organization's Owners have two-factor authentication: turn it on first (or add a passkey).",
            ),
            v.q(),
            v.tr("L'activer", "Turn it on"),
        )
    };
    let card = format!(
        "{nav}<h1>{title}</h1>\n<p class=\"help\">{help}</p>\n{notice}{body}",
        nav = nav(v, nav_orgs, Here::None),
        help = v.tr(
            "Une organisation a des membres, des groupes et des ordinateurs, et un journal de qui ouvre quoi.",
            "An organization has members, groups and computers, and a log of who opens what.",
        ),
        notice = notice_html(v.lang, notice),
    );
    console(v, &title, "/compte/organisations/creer", &card)
}

// --- Déployer des Stations -------------------------------------------------------------

/// La page « Déployer des Stations ».
#[derive(Debug, Clone)]
pub struct DeployView {
    pub id: String,
    pub name: String,
    /// Les jetons et leurs Stations enrôlées `(nom, quand)`.
    pub tokens: Vec<(EnrollToken, Vec<(String, i64)>)>,
    pub groups: Vec<(i64, String)>,
    /// Le jeton tout juste créé, montré une seule fois.
    pub new_token: Option<String>,
    /// Le relais, pour la ligne de commande (`/relay=`).
    pub relay: String,
    pub now: i64,
    pub nav_orgs: Vec<(String, String)>,
}

pub fn deploy(v: &View<'_>, d: &DeployView, notice: Option<Notice>) -> String {
    let title = v.tr("Déployer des Stations", "Deploy Stations");
    let q_id = format!("{}&amp;id={}", v.q(), pct(&d.id));
    let id_field = hidden("id", &d.id);
    let fresh = match &d.new_token {
        Some(token) => format!(
            "<section class=\"console-box console-new\">\n<h2>{h}</h2>\n<p class=\"hint\">{once}</p>\n<p class=\"token\">{token}</p>\n<p class=\"mono-label\">{silent}</p>\n<p class=\"token token-sm\">KyberNow-Station-Setup.exe /quiet /enroll={token} /relay={relay}</p>\n</section>\n",
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
            let state = match tk.refusal(d.now) {
                None => String::new(),
                Some("revoked") => format!("<span class=\"pill\">{}</span>", v.tr("Révoqué", "Revoked")),
                Some("expired") => format!("<span class=\"pill\">{}</span>", v.tr("Échu", "Expired")),
                Some(_) => format!("<span class=\"pill\">{}</span>", v.tr("Épuisé", "Used up")),
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
            let list: String = stations
                .iter()
                .map(|(name, at)| {
                    let when = match v.lang {
                        Lang::Fr => format!("enrôlée le {}", date(v.lang, *at)),
                        Lang::En => format!("enrolled {}", date(v.lang, *at)),
                    };
                    format!("<div class=\"row\"><div><b>{}</b></div><small>{when}</small></div>\n", esc(name))
                })
                .collect();
            let list = if list.is_empty() {
                String::new()
            } else {
                format!("<h3 class=\"mono-label\">{}</h3>\n{list}", v.tr("Stations enrôlées par ce jeton", "Stations enrolled with this token"))
            };
            format!(
                "<section class=\"console-box\">\n<div class=\"row\"><div><b>{name}</b><small>{hint}… · {parts}</small></div><div class=\"acts\">{state}{revoke}</div></div>\n{list}</section>\n",
                name = esc(&tk.name),
                hint = esc(&tk.hint),
                parts = esc(&parts.join(" · ")),
            )
        })
        .collect();
    let groups: String = std::iter::once(format!("<option value=\"\">{}</option>", v.tr("Aucun", "None")))
        .chain(d.groups.iter().map(|(id, name)| format!("<option value=\"{id}\">{}</option>", esc(name))))
        .collect();
    let inner = format!(
        "{id_field}\n<div class=\"inline-fields\">{name}\n{days}\n{count}</div>\n<div class=\"field\"><label class=\"mono-label\" for=\"groupe\">{group_l}</label>\n<select id=\"groupe\" name=\"groupe\">{groups}</select></div>\n{rights}\n<button class=\"btn kn-primary\" type=\"submit\">{create}</button>",
        name = field(&v.tr("Nom", "Name"), r#"<input id="nom" name="nom" maxlength="64" placeholder="Postes graphistes" required>"#),
        days = field(&v.tr("Durée (jours)", "Duration (days)"), r#"<input id="jours" name="jours" type="number" min="1" max="365" value="30" required>"#),
        count = field(&v.tr("Nombre de Stations", "Number of Stations"), r#"<input id="nombre" name="nombre" type="number" min="1" max="10000" value="10" required>"#),
        group_l = v.tr("Droits donnés au groupe", "Rights given to the group"),
        rights = rights_fieldset(v, &v.tr("Droits", "Rights"), &Rights::new([Right::Session]), false),
        create = v.tr("Nouveau jeton", "New token"),
    );
    let card = format!(
        "{nav}<p class=\"crumbs\"><a href=\"/compte/organisation{q_id}&amp;vue=ordinateurs\">{org} › {computers}</a> › <b>{tokens_l}</b></p>\n<h1>{title}</h1>\n{notice}{fresh}{tokens}<section class=\"console-box\">\n<h2>{new_h}</h2>\n<p class=\"hint\">{help}</p>\n{form}\n</section>\n<p class=\"hint\">{revoke_help}</p>",
        nav = nav(v, &d.nav_orgs, Here::Org(d.id.clone())),
        org = esc(&d.name),
        computers = v.tr("Ordinateurs", "Computers"),
        tokens_l = v.tr("Jetons d'enrôlement", "Enrollment tokens"),
        notice = notice_html(v.lang, notice),
        new_h = v.tr("Nouveau jeton", "New token"),
        help = v.tr(
            "L'installateur de la Station le prend en silencieux et rattache la Station à l'organisation, sans écran.",
            "The Station installer takes it silently and attaches the Station to the organization, with no screen.",
        ),
        form = v.form("/compte/organisation/jetons/creer", "", &inner),
        revoke_help = v.tr(
            "Révoquer le jeton empêche de nouveaux enrôlements ; les Stations déjà rattachées restent à l'organisation.",
            "Revoking the token prevents new enrollments; the Stations already attached stay with the organization.",
        ),
    );
    console(v, &title, "/compte/organisation/deployer", &card)
}

// --- Admin sur le réseau ---------------------------------------------------------------

/// « Ouvrir Admin de {ordinateur} ? » : Admin s'ouvrira à `host`, sur le réseau.
pub fn open_admin(v: &View<'_>, computer: &str, host: &str, fields: &[(&str, &str)]) -> String {
    let title = match v.lang {
        Lang::Fr => t(v.lang, &format!("Ouvrir Admin de {} ?", esc(computer)), ""),
        Lang::En => format!("Open Admin of {}?", esc(computer)),
    };
    let hidden_fields: String = fields.iter().map(|(k, val)| hidden(k, val)).collect::<Vec<_>>().join("\n");
    let inner = format!(
        "{hidden_fields}\n<button class=\"btn kn-primary\" type=\"submit\" name=\"action\" value=\"ouvrir\">{open}</button>\n<button class=\"btn btn-ghost\" type=\"submit\" name=\"action\" value=\"annuler\">{cancel}</button>",
        open = v.tr("Ouvrir Admin", "Open Admin"),
        cancel = v.tr("Annuler", "Cancel"),
    );
    let card = format!(
        "<h1>{title}</h1>\n<p class=\"help\">{help}</p>\n<p class=\"help\"><b>{host}</b></p>\n<p class=\"hint\">{check}</p>\n{form}",
        help = v.tr(
            "Avec le droit Administrer, votre compte ouvre Admin de cet ordinateur, sur le réseau seulement, à cette adresse :",
            "With the Administer right, your account opens this computer's Admin, on the network only, at this address:",
        ),
        host = esc(host),
        check = v.tr(
            "Vous n'avez pas demandé à ouvrir Admin à l'instant ? Annulez.",
            "You didn't just ask to open Admin? Cancel.",
        ),
        form = v.form("/compte/ouvrir-admin", "", &inner),
    );
    page_with(v, &title, "/compte/ouvrir-admin", "", &card)
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
    let html = page_with(v, &title, "/compte/ouvrir-admin", "", &card);
    html.replacen("<meta name=\"robots\" content=\"noindex\">", &format!("<meta name=\"robots\" content=\"noindex\">\n<meta http-equiv=\"refresh\" content=\"0;url={}\">", esc(target)), 1)
}

#[cfg(test)]
pub(super) mod tests {
    use super::*;
    use serde_json::json;

    /// Toutes les pages de la console, pour les essais de `super::tests` (typographie, rien du dehors, CSRF).
    pub fn all(v: &View<'_>) -> Vec<(&'static str, String)> {
        let card = |id: &str, rights: Option<Rights>| ComputerCard { id: id.into(), name: "PC du <salon>".into(), online: id == "st1", detail: "de Léa".into(), rights };
        let computers_view = ComputersView {
            mine: vec![card("st1", None)],
            opened: vec![card("st2", Some(Rights::new([Right::Lend, Right::Share])))],
            orgs: vec![("org_abc".into(), "Les Lignes Claires".into(), vec![card("st3", None)])],
            nav_orgs: vec![("org_abc".into(), "Les Lignes Claires".into())],
        };
        let people_view = PeopleView {
            id: "st1".into(),
            name: "PC du <salon>".into(),
            online: true,
            rows: vec![
                PersonRow { grant: None, name: "Ana (vous)".into(), detail: "Propriétaire".into(), rights: Rights::new(Right::ALL) },
                PersonRow { grant: Some(4), name: "Léa".into(), detail: "lea@exemple.fr · jusqu'au 31/12/2026".into(), rights: Rights::new([Right::Session]) },
            ],
            manage: true,
            can_detach: true,
            my_rights: Rights::new(Right::ALL),
            admin_needs_2fa: false,
            targets: vec![("org_abc".into(), "Les Lignes Claires (toute l'organisation)".into()), ("org_abc:3".into(), "Les Lignes Claires › Graphistes".into())],
            form: ("sam@exemple.fr".into(), Rights::new([Right::Session]), "2026-12-31".into()),
            nav_orgs: vec![],
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
        ];
        let org_view = |tab: Tab, role: Role, manage: bool| OrgView {
            id: "org_abc".into(),
            name: "Les Lignes Claires".into(),
            role,
            manage,
            needs_2fa: role.manages() && !manage,
            tab,
            me: 1,
            members: vec![member(1, Role::Owner), member(2, Role::Admin), member(3, Role::Member)],
            invites: vec![Invite { id: 9, email: "paul@exemple.fr".into(), role: Role::Member, created_at: 1_700_000_000 }],
            groups: vec![Group { id: 3, name: "Graphistes".into(), members: vec![(3, "Membre 3".into())] }],
            computers: vec![OrgComputer { id: "st3".into(), name: "Station de rendu".into(), online: true, access: t(v.lang, "Groupe Graphistes : Session", "Group Graphistes: Session") }],
            log: kinds.iter().map(|k| line(k)).collect(),
            nav_orgs: vec![("org_abc".into(), "Les Lignes Claires".into())],
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
        };
        let deploy_view = DeployView {
            id: "org_abc".into(),
            name: "Les Lignes Claires".into(),
            tokens: vec![(token.clone(), vec![("GRAPH-01".into(), 1_700_000_000)]), (EnrollToken { revoked_at: Some(1), ..token }, vec![])],
            groups: vec![(3, "Graphistes".into())],
            new_token: Some("kne_abcdefghijklmnopqrstuvwxyz234567".into()),
            relay: "share.test".into(),
            now: 1_700_000_000,
            nav_orgs: vec![],
        };
        vec![
            ("ordinateurs", computers(v, &computers_view, Some(Notice::StationDetached))),
            ("ordinateurs/vide", computers(v, &ComputersView::default(), None)),
            ("personnes", people(v, &people_view, Some(Notice::PersonAdded))),
            ("personnes/sans-gerer", people(v, &PeopleView { manage: false, admin_needs_2fa: true, my_rights: Rights::new([Right::Session]), ..people_view.clone() }, None)),
            ("organisation", org(v, &org_view(Tab::Members, Role::Owner, true), Some(Notice::OrgCreated))),
            ("organisation/membre", org(v, &org_view(Tab::Members, Role::Member, false), None)),
            ("organisation/sans-2fa", org(v, &org_view(Tab::Members, Role::Admin, false), Some(Notice::NeedTwoFactor))),
            ("organisation/groupes", org(v, &org_view(Tab::Groups, Role::Admin, true), None)),
            ("organisation/ordinateurs", org(v, &org_view(Tab::Computers, Role::Owner, true), None)),
            ("organisation/journal", org(v, &org_view(Tab::Log, Role::Owner, true), None)),
            ("organisation/creer", create_org(v, "", true, &[], None)),
            ("organisation/creer-sans-2fa", create_org(v, "", false, &[], None)),
            ("deployer", deploy(v, &deploy_view, Some(Notice::TokenRevoked))),
            ("ouvrir-admin", open_admin(v, "PC du salon", "192.168.1.20:8443", &[("station", "st1"), ("nonce", "n"), ("retour", "x")])),
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

