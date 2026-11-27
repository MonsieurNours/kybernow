//! La console du lot 4 (docs/comptes.md, « Ajouter des personnes ») :
//! « Ordinateurs » (`/compte/ordinateurs`), la page d'un ordinateur
//! (`/compte/ordinateur?id=…`), ajouter une personne par e-mail, ouvrir à une
//! famille, une équipe ou un groupe (lot 5), retirer un accès ; et l'espace du
//! compte (« L'espace du compte ») : le menu, « Personnes », « Familles et
//! équipes », le tour d'accueil.

use kybernow_tunnel::access::{Right, Rights};
use serde_json::json;

use super::super::mail::Message;
use super::super::pages::console::{
    self, AllPeopleView, ComputerCard, ComputersView, FamiliesView, Granted, NavOrg, OrgCard, PeopleView, PersonRow, Shell, Start, Welcome, invite_state, rights_words,
};
use super::super::pages::{Notice, date};
use super::super::people::{Grant, Resend, Subject};
use super::super::stations::StationRow;
use super::super::store::Account;
use super::device_pages::sign_in_first;
use super::{Cx, Reply, lang_of, normalize_email};
use crate::pages::{Lang, t};

/// Les ordinateurs dont ce compte gère les accès : les siens, et ceux des familles et équipes qu'il gère.
pub(super) fn managed_stations(cx: &Cx<'_>, account: &Account) -> Vec<StationRow> {
    let visible = cx.acc.db().visible_stations(account.id, cx.now).unwrap_or_default();
    visible.into_iter().filter(|s| cx.acc.can_manage(account, s)).collect()
}

/// Ce que le menu de l'espace montre : ses familles et équipes, les nombres, « Pour bien démarrer ».
pub(super) fn shell(cx: &Cx<'_>, account: &Account) -> Shell {
    let managed = managed_stations(cx, account);
    let db = cx.acc.db();
    let orgs = db.orgs_of(account.id).unwrap_or_default();
    drop(db);
    let people = people_rows(cx, &managed).len();
    let db = cx.acc.db();
    Shell {
        name: account.name.clone(),
        email: account.email.clone(),
        computers: db.visible_stations(account.id, cx.now).map(|v| v.len()).unwrap_or(0),
        people,
        start: Start { protected: db.two_factor(account.id).unwrap_or(false), attached: !managed.is_empty(), opened: people > 0, grouped: !orgs.is_empty() },
        orgs: orgs.into_iter().map(|(o, _)| NavOrg { id: o.public_id, name: o.name, kind: o.kind }).collect(),
    }
}

/// Les droits cochés d'un formulaire (`session`, `lend`, `share`, `admin`).
pub(super) fn form_rights(cx: &Cx<'_>) -> Rights {
    Right::ALL.into_iter().filter(|r| cx.req.form(r.as_str()) == "1").collect()
}

/// Une date de fin (`2026-12-31`, ou `31/12/2026`) : la fin de ce jour (UTC), à venir ; `Some(None)` : pas de date.
pub(super) fn parse_until(text: &str, now: i64) -> Option<Option<i64>> {
    let text = text.trim();
    if text.is_empty() {
        return Some(None);
    }
    let parts: Vec<&str> = text.split(['-', '/', '.']).collect();
    let (y, m, d) = match parts.as_slice() {
        [y, m, d] if y.len() == 4 => (y.parse::<i64>().ok()?, m.parse::<i64>().ok()?, d.parse::<i64>().ok()?),
        [d, m, y] if y.len() == 4 => (y.parse::<i64>().ok()?, m.parse::<i64>().ok()?, d.parse::<i64>().ok()?),
        _ => return None,
    };
    if !(1..=12).contains(&m) || !(1..=31).contains(&d) || !(2000..=9999).contains(&y) {
        return None;
    }
    // Jours depuis 1970 (H. Hinnant, « days_from_civil »).
    let yy = if m <= 2 { y - 1 } else { y };
    let era = yy.div_euclid(400);
    let yoe = yy - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146_097 + doe - 719_468;
    // Pas de 31 février : la date relue doit être la même.
    if date(Lang::Fr, days * 86_400) != format!("{d:02}/{m:02}/{y}") {
        return None;
    }
    let end = (days + 1) * 86_400;
    (end > now).then_some(Some(end))
}

/// La personne connectée, ou la page de connexion (la page est retenue).
macro_rules! signed_in {
    ($cx:ident) => {
        match $cx.full_session() {
            Some((_, account)) => account,
            None => return sign_in_first($cx),
        }
    };
}
pub(super) use signed_in;

/// `GET /compte/ordinateurs` : les siens, ceux qu'on lui a ouverts, ceux de ses organisations.
pub(super) fn computers(cx: Cx<'_>, notice: Option<Notice>) -> Reply {
    let account = signed_in!(cx);
    let acc = cx.acc;
    // La première fois, le tour d'accueil d'abord.
    if notice.is_none() && !acc.db().welcomed(account.id).unwrap_or(true) {
        let q = cx.q();
        return cx.redirect(format!("/compte/bienvenue{q}"));
    }
    let orgs = acc.db().orgs_of(account.id).unwrap_or_default();
    let stations = acc.db().visible_stations(account.id, cx.now).unwrap_or_default();
    let mut view = ComputersView {
        orgs: orgs.iter().map(|(o, _)| (o.public_id.clone(), o.name.clone(), o.kind, Vec::new())).collect(),
        shell: shell(&cx, &account),
        ..ComputersView::default()
    };
    for s in &stations {
        let Some(standing) = acc.standing(&account, s) else { continue };
        let people = acc.db().grants_of(&s.id).map(|g| g.len()).unwrap_or(0);
        let detail = if standing.manage {
            match (people, cx.lang) {
                (0, _) => cx.tr("Vous seul", "Only you"),
                (n, Lang::Fr) => format!("Vous et {n} accès donné{}", if n > 1 { "s" } else { "" }),
                (n, Lang::En) => format!("You and {n} access{}", if n > 1 { "es" } else { "" }),
            }
        } else {
            owner_name(cx.acc, s).map(|n| t(cx.lang, &format!("de {n}"), &format!("from {n}"))).unwrap_or_default()
        };
        let card = ComputerCard {
            id: s.id.clone(),
            name: s.name.clone(),
            online: acc.stations.online(&s.id),
            detail,
            rights: (!standing.mine && !standing.manage).then(|| standing.rights.clone()),
            admin: standing.rights.has(Right::Admin),
        };
        match (standing.mine, s.org.and_then(|o| orgs.iter().position(|(org, _)| org.id == o))) {
            (true, _) => view.mine.push(card),
            (false, Some(i)) => view.orgs[i].3.push(card),
            (false, None) => view.opened.push(card),
        }
    }
    let page = console::computers(&cx.view(), &view, notice);
    cx.page(200, page)
}

/// Qui a accès, en mots : `(nom, détail)`.
fn subject_words(cx: &Cx<'_>, subject: &Subject) -> (String, String) {
    match subject {
        Subject::Person { name, email, .. } => (if name.is_empty() { email.clone() } else { name.clone() }, email.clone()),
        Subject::Invite { email } => (email.clone(), cx.tr("Invitation envoyée, pas encore de compte", "Invitation sent, no account yet")),
        Subject::Org { name, .. } => (name.clone(), cx.tr("Tous ses membres", "All its members")),
        Subject::Group { name, org_name, .. } => (format!("{org_name} › {name}"), cx.tr("Groupe", "Group")),
    }
}

/// Ceux à qui ces ordinateurs sont ouverts, chacun une fois (« Personnes ») :
/// une personne par son compte, ou par son adresse tant qu'elle n'a pas de
/// compte vérifié (une invitation dont l'adresse a depuis un compte vérifié
/// va avec ce compte) ; une famille, une équipe, un groupe.
pub(super) fn people_rows(cx: &Cx<'_>, stations: &[StationRow]) -> Vec<Granted> {
    let db = cx.acc.db();
    let mut keys: Vec<String> = Vec::new();
    let mut rows: Vec<Granted> = Vec::new();
    for s in stations {
        for g in db.grants_of(&s.id).unwrap_or_default() {
            let (key, name, detail) = match &g.subject {
                Subject::Invite { email } => match db.account_by_email(email).ok().flatten().filter(|a| a.verified()) {
                    Some(a) => (format!("compte:{}", a.id), if a.name.is_empty() { a.email.clone() } else { a.name.clone() }, a.email.clone()),
                    None => (format!("adresse:{email}"), email.clone(), invite_state(cx.lang, g.sent_at, cx.now)),
                },
                Subject::Person { account, .. } => {
                    let (name, detail) = subject_words(cx, &g.subject);
                    (format!("compte:{account}"), name, detail)
                }
                Subject::Org { org, .. } => {
                    let (name, detail) = subject_words(cx, &g.subject);
                    (format!("org:{org}"), name, detail)
                }
                Subject::Group { grp, .. } => {
                    let (name, detail) = subject_words(cx, &g.subject);
                    (format!("groupe:{grp}"), name, detail)
                }
            };
            let computer = (s.id.clone(), s.name.clone(), g.expired(cx.now));
            match keys.iter().position(|k| *k == key) {
                Some(i) => {
                    let row = &mut rows[i];
                    // Le compte l'emporte sur l'invitation : son nom et son adresse.
                    if matches!(g.subject, Subject::Person { .. }) {
                        row.name = name;
                        row.detail = detail;
                    }
                    if !row.computers.iter().any(|c| c.0 == computer.0) {
                        row.computers.push(computer);
                    }
                    row.rights = Rights::new(row.rights.iter().chain(g.rights.iter()));
                }
                None => {
                    keys.push(key);
                    rows.push(Granted { name, detail, computers: vec![computer], rights: g.rights.clone() });
                }
            }
        }
    }
    rows
}

/// `GET /compte/personnes` : ceux à qui il a ouvert ses ordinateurs, chacun une fois.
pub(super) fn all_people(cx: Cx<'_>) -> Reply {
    let account = signed_in!(cx);
    let mine = managed_stations(&cx, &account);
    let rows = people_rows(&cx, &mine);
    let view = AllPeopleView { rows, computers: mine.into_iter().map(|s| (s.id, s.name)).collect(), shell: shell(&cx, &account) };
    let page = console::all_people(&cx.view(), &view);
    cx.page(200, page)
}

/// `GET /compte/familles` : ses familles et équipes, et comment ça marche.
pub(super) fn families(cx: Cx<'_>) -> Reply {
    let account = signed_in!(cx);
    let orgs = {
        let db = cx.acc.db();
        db.orgs_of(account.id)
            .unwrap_or_default()
            .into_iter()
            .map(|(o, role)| OrgCard {
                members: db.members(o.id).map(|m| m.len()).unwrap_or(0),
                computers: db.stations_of_org(o.id).map(|s| s.len()).unwrap_or(0),
                id: o.public_id,
                name: o.name,
                kind: o.kind,
                role,
            })
            .collect()
    };
    let view = FamiliesView { orgs, shell: shell(&cx, &account) };
    let page = console::families(&cx.view(), &view);
    cx.page(200, page)
}

/// `GET /compte/bienvenue?etape=…` : le tour d'accueil.
pub(super) fn welcome(cx: Cx<'_>) -> Reply {
    let account = signed_in!(cx);
    let step = cx.req.query("etape").and_then(|s| s.parse::<u8>().ok()).unwrap_or(0).min(3);
    let computer = managed_stations(&cx, &account).into_iter().next().map(|s| s.name);
    let protected = cx.acc.db().two_factor(account.id).unwrap_or(false);
    let register = if protected { None } else { cx.registration(&account) };
    let view = Welcome { name: account.name.clone(), protected, register, computer };
    let page = console::welcome(&cx.view(), step, &view);
    cx.page(200, page)
}

/// `POST /compte/bienvenue` : le tour est vu ; `suite` : créer une famille, une équipe, ou l'espace.
pub(super) fn welcome_done(cx: Cx<'_>) -> Reply {
    let account = signed_in!(cx);
    let _ = cx.acc.db().set_welcomed(account.id, cx.now);
    let q = cx.q();
    match cx.req.form("suite") {
        "famille" => cx.redirect(format!("/compte/organisations/creer{q}&genre=famille")),
        "equipe" => cx.redirect(format!("/compte/organisations/creer{q}&genre=equipe")),
        _ => cx.redirect(format!("/compte/ordinateurs{q}")),
    }
}

/// Le nom du propriétaire d'une Station (la personne, ou l'organisation).
fn owner_name(acc: &super::Accounts, s: &StationRow) -> Option<String> {
    let db = acc.db();
    match (s.account, s.org) {
        (Some(a), _) => db.account(a).ok().flatten().map(|a| a.name),
        (None, Some(o)) => db.org(o).ok().flatten().map(|o| o.name),
        _ => None,
    }
}

/// La Station du formulaire ou de la requête (`id`), si ce compte y a un accès.
fn station_of(cx: &Cx<'_>, account: &Account, post: bool) -> Option<StationRow> {
    let id = if post { cx.req.form("id") } else { cx.req.query("id").unwrap_or("") };
    if !kybernow_tunnel::id::is_station_id(id) {
        return None;
    }
    let row = cx.acc.db().station(id).ok().flatten()?;
    cx.acc.standing(account, &row).map(|_| row)
}

pub(super) fn not_found(cx: Cx<'_>) -> Reply {
    let page = crate::pages::not_found(cx.lang);
    cx.page(404, page)
}

/// `GET /compte/ordinateur?id=…` : « Personnes ».
pub(super) fn people(cx: Cx<'_>) -> Reply {
    let account = signed_in!(cx);
    let Some(station) = station_of(&cx, &account, false) else { return not_found(cx) };
    people_page(cx, &account, &station, None, Kept::default())
}

/// Ce que la page d'un ordinateur garde d'un formulaire : « Ajouter une personne » tel qu'envoyé,
/// le lien d'accès tout juste créé (montré une seule fois), « Créer un lien » tel qu'envoyé.
#[derive(Default)]
pub(super) struct Kept {
    pub form: Option<(String, Rights, String)>,
    pub new_link: Option<String>,
    pub link_form: Option<(Rights, i64, String)>,
}

pub(super) fn people_page(cx: Cx<'_>, account: &Account, station: &StationRow, notice: Option<Notice>, kept: Kept) -> Reply {
    let acc = cx.acc;
    let Some(standing) = acc.standing(account, station) else { return not_found(cx) };
    let mut rows = Vec::new();
    let all = Rights::new(Right::ALL);
    {
        let db = acc.db();
        match (station.account, station.org) {
            (Some(owner), _) => {
                let name = if owner == account.id {
                    format!("{} ({})", account.name, cx.tr("vous", "you"))
                } else {
                    db.account(owner).ok().flatten().map(|a| a.name).unwrap_or_default()
                };
                rows.push(PersonRow { grant: None, group: None, name, detail: cx.tr("Propriétaire : tout", "Owner: everything"), rights: all.clone(), resend: false });
            }
            (None, Some(org)) => {
                let name = db.org(org).ok().flatten().map(|o| o.name).unwrap_or_default();
                rows.push(PersonRow {
                    grant: None,
                    group: None,
                    name,
                    detail: cx.tr("Propriétaires et administrateurs", "Owners and administrators"),
                    rights: all.clone(),
                    resend: false,
                });
            }
            _ => {}
        }
        for g in db.grants_of(&station.id).unwrap_or_default() {
            let until = g.expires_at.map(|e| match cx.lang {
                Lang::Fr => t(cx.lang, &format!("jusqu'au {}", date(cx.lang, e - 1)), ""),
                Lang::En => format!("until {}", date(cx.lang, e - 1)),
            });
            let expired = g.expired(cx.now).then(|| cx.tr("échu", "expired"));
            let (name, mut detail) = match &g.subject {
                Subject::Person { name, email, .. } => (if name.is_empty() { email.clone() } else { name.clone() }, vec![email.clone()]),
                Subject::Invite { email } => (email.clone(), vec![invite_state(cx.lang, g.sent_at, cx.now)]),
                Subject::Org { name, .. } => (name.clone(), vec![cx.tr("Tous ses membres", "All its members")]),
                Subject::Group { name, org_name, .. } => (format!("{org_name} › {name}"), vec![cx.tr("Groupe", "Group")]),
            };
            detail.extend(until);
            detail.extend(expired);
            let resend = matches!(g.subject, Subject::Invite { .. });
            rows.push(PersonRow { grant: Some(g.id), group: None, name, detail: detail.join(" · "), rights: g.rights.clone(), resend });
        }
        // Les droits venus d'un groupe d'ordinateurs : « Par le groupe {groupe} », sans « Retirer ».
        for g in db.group_grants_on(&station.id).unwrap_or_default() {
            let name = match &g.subject {
                Subject::Person { name, email, .. } => if name.is_empty() { email.clone() } else { name.clone() },
                Subject::Invite { email } => email.clone(),
                Subject::Org { name, .. } => name.clone(),
                Subject::Group { name, org_name, .. } => format!("{org_name} › {name}"),
            };
            let mut detail = vec![match cx.lang {
                Lang::Fr => t(cx.lang, &format!("Par le groupe {}", g.group_name), ""),
                Lang::En => format!("Through the group {}", g.group_name),
            }];
            detail.extend(g.expires_at.map(|e| match cx.lang {
                Lang::Fr => t(cx.lang, &format!("jusqu'au {}", date(cx.lang, e - 1)), ""),
                Lang::En => format!("until {}", date(cx.lang, e - 1)),
            }));
            detail.extend(g.expired(cx.now).then(|| cx.tr("échu", "expired")));
            rows.push(PersonRow { grant: None, group: Some(g.group_name.clone()), name, detail: detail.join(" · "), rights: g.rights.clone(), resend: false });
        }
    }
    // Ouvrir à : les organisations dont il est membre, et leurs groupes.
    let targets = if standing.manage {
        let db = acc.db();
        let mut list = Vec::new();
        for (org, _) in db.orgs_of(account.id).unwrap_or_default() {
            if station.org != Some(org.id) {
                let whole = match org.kind {
                    super::super::orgs::Kind::Family => t(cx.lang, &format!("{} (toute la famille)", org.name), &format!("{} (the whole family)", org.name)),
                    super::super::orgs::Kind::Team => t(cx.lang, &format!("{} (toute l'équipe)", org.name), &format!("{} (the whole team)", org.name)),
                };
                list.push((org.public_id.clone(), whole));
            }
            for g in db.groups(org.id).unwrap_or_default() {
                list.push((format!("{}:{}", org.public_id, g.id), format!("{} › {}", org.name, g.name)));
            }
        }
        list
    } else {
        Vec::new()
    };
    // La base n'est pas gardée pendant la construction de la vue (`shell` la prend aussi).
    let links = if standing.manage { acc.db().access_links(&station.id, 20).unwrap_or_default() } else { Vec::new() };
    let known = if standing.manage { acc.db().known_people(account.id).unwrap_or_default() } else { Vec::new() };
    // « Déplacer… » : vers « Moi » et les familles et équipes qu'il gère (docs/comptes.md, « Déplacer un ordinateur »).
    let moves: Vec<(String, String)> = acc
        .move_destinations(account, station)
        .into_iter()
        .map(|d| match d {
            super::super::moves::Dest::Me => ("moi".to_string(), cx.tr("Moi", "Me")),
            super::super::moves::Dest::Org(o) => (o.public_id, o.name),
        })
        .collect();
    let view = PeopleView {
        id: station.id.clone(),
        name: station.name.clone(),
        online: acc.stations.online(&station.id),
        rows,
        manage: standing.manage,
        can_detach: station.account == Some(account.id) || acc.org_manager(account, station.org),
        my_rights: standing.rights.clone(),
        admin_needs_2fa: standing.admin_needs_2fa,
        targets,
        form: kept.form.unwrap_or_else(|| (String::new(), Rights::new([Right::Session, Right::Share]), String::new())),
        admin: standing.rights.has(Right::Admin),
        links,
        new_link: kept.new_link,
        link_open: kept.link_form.is_some(),
        link_form: kept.link_form.unwrap_or_else(|| (Rights::new([Right::Session]), 7, String::new())),
        known,
        moves,
        now: cx.now,
        shell: shell(&cx, account),
    };
    let page = console::people(&cx.view(), &view, notice);
    cx.page(200, page)
}

/// L'échéance de ce que ce compte donne (REL-02) : `None` s'il donne des droits qu'il n'a pas ;
/// sinon `until`, ramenée pour un délégué (droit Administrer reçu) à l'échéance de ses propres droits.
pub(super) fn bounded(cx: &Cx<'_>, account: &Account, station: &StationRow, rights: &Rights, until: Option<i64>) -> Option<Option<i64>> {
    match cx.acc.delegable(account, station) {
        None => Some(until),
        Some(own) => super::super::people::bound_delegation(&own, rights, until),
    }
}

/// La Station d'un formulaire de gestion : il faut gérer ses accès.
pub(super) fn managed_station(cx: &Cx<'_>, account: &Account) -> Option<StationRow> {
    station_of(cx, account, true).filter(|s| cx.acc.can_manage(account, s))
}

/// Une ligne au journal de l'organisation propriétaire, s'il y en a une.
pub(super) fn log_rights(cx: &Cx<'_>, account: &Account, station: &StationRow, kind: &str, to: &str, rights: &Rights) {
    if let Some(org) = station.org {
        let detail = json!({ "station": station.name, "to": to, "rights": super::super::people::rights_text(rights) });
        let _ = cx.acc.db().org_log(org, Some(account), kind, detail, Some(&cx.ip()), cx.now);
    }
}

/// `POST /compte/ordinateur/ajouter` : ajouter une personne par son adresse.
pub(super) fn add(cx: Cx<'_>) -> Reply {
    let account = signed_in!(cx);
    let acc = cx.acc;
    let Some(station) = managed_station(&cx, &account) else { return not_found(cx) };
    let typed = cx.req.form("email").trim().to_string();
    let rights = form_rights(&cx);
    let until_text = cx.req.form("fin").to_string();
    let form = Some((typed.clone(), rights.clone(), until_text.clone()));
    let Some(email) = normalize_email(&typed) else {
        return people_page(cx, &account, &station, Some(Notice::BadEmail), Kept { form, ..Kept::default() });
    };
    if rights.is_empty() {
        return people_page(cx, &account, &station, Some(Notice::NoRights), Kept { form, ..Kept::default() });
    }
    let Some(until) = parse_until(&until_text, cx.now) else {
        return people_page(cx, &account, &station, Some(Notice::BadDate), Kept { form, ..Kept::default() });
    };
    // Qui a reçu Administrer ne donne que ses droits, jusqu'à leur échéance (REL-02).
    let Some(until) = bounded(&cx, &account, &station, &rights, until) else {
        return people_page(cx, &account, &station, Some(Notice::NotAllowed), Kept { form, ..Kept::default() });
    };
    // Pas à soi, ni au propriétaire.
    if email == account.email {
        return people_page(cx, &account, &station, Some(Notice::NotAllowed), Kept { form, ..Kept::default() });
    }
    let target = acc.db().account_by_email(&email).ok().flatten().filter(|a| a.verified());
    if target.as_ref().is_some_and(|t| station.account == Some(t.id)) {
        return people_page(cx, &account, &station, Some(Notice::NotAllowed), Kept { form, ..Kept::default() });
    }
    if let Err(n) = cx.invite_limit(&account) {
        let mut r = people_page(cx, &account, &station, Some(n), Kept::default());
        r.status = 429;
        return r;
    }
    // Déjà là ? Une personne qui a déjà un accès à cet ordinateur : ses droits changent ; une
    // invitation en attente pour la même adresse : remplacée (docs/comptes.md, « Ajouter quelqu'un »).
    let had_access = target.as_ref().is_some_and(|t| {
        acc.db().grants_of(&station.id).unwrap_or_default().iter().any(|g| matches!(g.subject, Subject::Person { account, .. } if account == t.id))
    });
    let done = match &target {
        Some(t) => acc.db().grant_person(&station.id, t.id, &rights, until, account.id, cx.now),
        None => acc.db().grant_invite(&station.id, &email, &rights, until, account.id, cx.now),
    };
    if let Err(e) = done {
        tracing::warn!("ajout d'une personne : {e:#}");
        return cx.failed();
    }
    // Ses droits changent et Administrer n'y est plus : ce qu'elle a donné part avec (REL-02).
    if let Some(t) = &target
        && had_access
        && !rights.has(kybernow_tunnel::access::Right::Admin)
    {
        let _ = acc.db().drop_delegated(&station.id, t.id, cx.now);
    }
    cx.audit(Some(account.id), if had_access { "droits_changes" } else { "personne_ajoutee" });
    let to = target.as_ref().map(|t| t.name.clone()).unwrap_or_else(|| email.clone());
    log_rights(&cx, &account, &station, "droits_donnes", &to, &rights);
    tracing::info!(station = %station.id, compte = target.is_some(), deja = had_access, "personne ajoutée à un ordinateur");
    send_shared(&cx, &account, &station, &email, target.as_ref(), &rights, until);
    let notice = match &target {
        Some(t) if had_access => Notice::AccessUpdated(if t.name.is_empty() { t.email.clone() } else { t.name.clone() }),
        Some(_) => Notice::PersonAdded,
        None => Notice::InviteSent,
    };
    people_page(cx, &account, &station, Some(notice), Kept::default())
}

/// L'e-mail « {nom} vous a ouvert {ordinateur} », dans la langue de la personne (la page sinon).
fn send_shared(cx: &Cx<'_>, account: &Account, station: &StationRow, email: &str, target: Option<&Account>, rights: &Rights, until: Option<i64>) {
    if !cx.may_mail(email) {
        return;
    }
    let lang = target.map(lang_of).unwrap_or(cx.lang);
    let link = match target {
        Some(_) => cx.url(&format!("/compte/ordinateurs?lang={}", lang.code())),
        None => cx.url(&format!("/compte/inscription?lang={}", lang.code())),
    };
    let words = rights_words(lang, rights);
    let until = until.map(|e| date(lang, e - 1));
    let by = if account.name.is_empty() { account.email.clone() } else { account.name.clone() };
    cx.mail(email, lang, Message::ComputerShared { by: &by, computer: &station.name, rights: &words, until: until.as_deref(), has_account: target.is_some(), link: &link });
}

/// `POST /compte/ordinateur/renvoyer` : « Renvoyer l'invitation » (`acces`), même expirée ; une fois par heure.
pub(super) fn resend(cx: Cx<'_>) -> Reply {
    let account = signed_in!(cx);
    let acc = cx.acc;
    let Some(station) = managed_station(&cx, &account) else { return not_found(cx) };
    let Ok(id) = cx.req.form("acces").parse::<i64>() else { return people_page(cx, &account, &station, None, Kept::default()) };
    let grant: Option<Grant> = acc.db().grant(&station.id, id).ok().flatten().filter(|g| matches!(g.subject, Subject::Invite { .. }));
    let Some(grant) = grant else { return people_page(cx, &account, &station, None, Kept::default()) };
    if let Err(n) = cx.invite_limit(&account) {
        let mut r = people_page(cx, &account, &station, Some(n), Kept::default());
        r.status = 429;
        return r;
    }
    let resent = acc.db().resend_grant_invite(&station.id, id, cx.now);
    let email = match resent {
        Ok(Resend::Done(email)) => email,
        Ok(Resend::TooSoon) => return people_page(cx, &account, &station, Some(Notice::ResentTooSoon), Kept::default()),
        Ok(Resend::Missing) => return people_page(cx, &account, &station, None, Kept::default()),
        Err(e) => {
            tracing::warn!("renvoi d'une invitation : {e:#}");
            return cx.failed();
        }
    };
    // L'adresse a depuis un compte vérifié : l'invitation, de nouveau valable, passe à lui tout de suite.
    let target = acc.db().account_by_email(&email).ok().flatten().filter(|a| a.verified());
    if let Some(t) = &target {
        let _ = acc.db().claim_invitations(t.id, cx.now);
    }
    cx.audit(Some(account.id), "invitation_renvoyee");
    if let Some(org) = station.org {
        let detail = json!({ "station": station.name, "to": email });
        let _ = acc.db().org_log(org, Some(&account), "invitation_renvoyee", detail, Some(&cx.ip()), cx.now);
    }
    send_shared(&cx, &account, &station, &email, target.as_ref(), &grant.rights, grant.expires_at);
    people_page(cx, &account, &station, Some(Notice::InviteResent(email)), Kept::default())
}

/// `POST /compte/ordinateur/ouvrir` : ouvrir à une organisation (`org_…`) ou à un groupe (`org_…:<n>`).
pub(super) fn open_to(cx: Cx<'_>) -> Reply {
    let account = signed_in!(cx);
    let acc = cx.acc;
    let Some(station) = managed_station(&cx, &account) else { return not_found(cx) };
    let rights = form_rights(&cx);
    if rights.is_empty() {
        return people_page(cx, &account, &station, Some(Notice::NoRights), Kept::default());
    }
    // Un droit d'équipe ou de groupe n'a pas d'échéance : seulement par qui peut donner sans borne,
    // ou par un délégué dont les droits donnés n'en ont pas non plus (REL-02).
    if bounded(&cx, &account, &station, &rights, None) != Some(None) {
        return people_page(cx, &account, &station, Some(Notice::NotAllowed), Kept::default());
    }
    let target = cx.req.form("a").to_string();
    let (org_id, grp) = match target.split_once(':') {
        Some((o, g)) => match g.parse::<i64>() {
            Ok(g) => (o.to_string(), Some(g)),
            Err(_) => return people_page(cx, &account, &station, Some(Notice::NotAllowed), Kept::default()),
        },
        None => (target.clone(), None),
    };
    // Seulement une organisation dont il est membre.
    let org = acc.db().org_by_public(&org_id).ok().flatten();
    let Some(org) = org.filter(|o| acc.db().org_role(o.id, account.id).ok().flatten().is_some()) else {
        return people_page(cx, &account, &station, Some(Notice::NotAllowed), Kept::default());
    };
    // Le nom du groupe d'abord : la base n'est pas gardée pendant le `match`.
    let group = grp.and_then(|g| acc.db().group_name(org.id, g).ok().flatten());
    let done = match grp {
        Some(g) => match group {
            Some(name) => acc.db().grant_group(&station.id, g, &rights, account.id, cx.now).map(|_| format!("{} › {name}", org.name)),
            None => return people_page(cx, &account, &station, Some(Notice::NotAllowed), Kept::default()),
        },
        None => acc.db().grant_org(&station.id, org.id, &rights, account.id, cx.now).map(|_| org.name.clone()),
    };
    let to = match done {
        Ok(to) => to,
        Err(e) => {
            tracing::warn!("ouvrir à une organisation : {e:#}");
            return cx.failed();
        }
    };
    cx.audit(Some(account.id), "ordinateur_ouvert_a_une_organisation");
    let detail = json!({ "station": station.name, "to": to, "rights": super::super::people::rights_text(&rights) });
    let _ = acc.db().org_log(org.id, Some(&account), "droits_donnes", detail, Some(&cx.ip()), cx.now);
    people_page(cx, &account, &station, Some(Notice::AccessChanged), Kept::default())
}

/// `POST /compte/ordinateur/retirer` : retirer un accès.
pub(super) fn remove(cx: Cx<'_>) -> Reply {
    let account = signed_in!(cx);
    let acc = cx.acc;
    let Some(station) = managed_station(&cx, &account) else { return not_found(cx) };
    let Ok(id) = cx.req.form("acces").parse::<i64>() else { return people_page(cx, &account, &station, None, Kept::default()) };
    let grant = acc.db().grant(&station.id, id).ok().flatten();
    let Some(grant) = grant else { return people_page(cx, &account, &station, None, Kept::default()) };
    if !acc.db().delete_grant(&station.id, id).unwrap_or(false) {
        return people_page(cx, &account, &station, None, Kept::default());
    }
    // Ce que cette personne a donné sur l'ordinateur part avec son droit (REL-02).
    if let Subject::Person { account: gone, .. } = &grant.subject {
        let _ = acc.db().drop_delegated(&station.id, *gone, cx.now);
    }
    cx.audit(Some(account.id), "acces_retire");
    let to = match &grant.subject {
        Subject::Person { name, email, .. } => if name.is_empty() { email.clone() } else { name.clone() },
        Subject::Invite { email } => email.clone(),
        Subject::Org { name, .. } => name.clone(),
        Subject::Group { name, org_name, .. } => format!("{org_name} › {name}"),
    };
    log_rights(&cx, &account, &station, "droits_retires", &to, &grant.rights);
    // Ouvert à une autre organisation : son journal le dit aussi.
    if let Subject::Org { org, .. } = grant.subject
        && station.org != Some(org)
    {
        let _ = acc.db().org_log(org, Some(&account), "droits_retires", json!({ "station": station.name, "to": to }), Some(&cx.ip()), cx.now);
    }
    people_page(cx, &account, &station, Some(Notice::AccessRemoved), Kept::default())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dates_de_fin() {
        let now = 1_795_392_000; // 23/11/2026
        assert_eq!(parse_until("", now), Some(None));
        let end = parse_until("2026-12-31", now).unwrap().unwrap();
        assert_eq!(date(Lang::Fr, end - 1), "31/12/2026");
        assert_eq!(parse_until("31/12/2026", now), Some(Some(end)));
        assert_eq!(parse_until("2026-11-23", now), Some(Some(now + 86_400)), "aujourd'hui : jusqu'à ce soir");
        assert_eq!(parse_until("2026-11-22", now), None, "passée");
        assert_eq!(parse_until("2027-02-30", now), None);
        assert_eq!(parse_until("demain", now), None);
    }
}
