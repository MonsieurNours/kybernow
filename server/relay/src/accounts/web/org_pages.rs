//! La console du lot 5 (docs/comptes.md, « Flotte (organisations) ») :
//! créer une organisation, sa page (membres et rôles, invitations, groupes,
//! ordinateurs, journal) et « Déployer des Stations » (jetons d'enrôlement).
//!
//! Gérer demande d'être Propriétaire ou Administrateur **avec la double
//! authentification** ; seul un Propriétaire nomme ou retire un Propriétaire,
//! et une organisation en garde toujours un.

use kybernow_tunnel::access::Right;
use serde_json::json;

use super::super::mail::Message;
use super::super::orgs::{ENROLL_MAX_DAYS, ENROLL_MAX_STATIONS, Kind, Org, Role, clean_org_name, new_enroll_token};
use super::super::pages::Notice;
use super::super::pages::console::{self, DeployView, OrgComputer, OrgView, Tab, rights_words, role_name};
use super::super::people::Subject;
use super::super::store::Account;
use super::console::{form_rights, shell, signed_in};
use super::device_pages::sign_in_first;
use super::{Cx, Reply, lang_of, normalize_email, token_hash};

/// L'organisation de la requête (`id`), et le rôle de ce compte (membre).
fn org_of(cx: &Cx<'_>, account: &Account, post: bool) -> Option<(Org, Role)> {
    let id = if post { cx.req.form("id") } else { cx.req.query("id").unwrap_or("") };
    if id.is_empty() || id.len() > 64 {
        return None;
    }
    let db = cx.acc.db();
    let org = db.org_by_public(id).ok().flatten()?;
    let role = db.org_role(org.id, account.id).ok().flatten()?;
    Some((org, role))
}

fn not_found(cx: Cx<'_>) -> Reply {
    let page = crate::pages::not_found(cx.lang);
    cx.page(404, page)
}

/// Une ligne au journal de l'organisation.
fn log(cx: &Cx<'_>, org: &Org, account: &Account, kind: &str, detail: serde_json::Value) {
    if let Err(e) = cx.acc.db().org_log(org.id, Some(account), kind, detail, Some(&cx.ip()), cx.now) {
        tracing::warn!("journal de l'organisation : {e:#}");
    }
}

/// `GET /compte/organisation?id=…&vue=…`.
pub(super) fn page(cx: Cx<'_>) -> Reply {
    let account = signed_in!(cx);
    let Some((org, role)) = org_of(&cx, &account, false) else { return not_found(cx) };
    let tab = Tab::parse(cx.req.query("vue").unwrap_or(""));
    show(cx, &account, &org, role, tab, None)
}

fn show(cx: Cx<'_>, account: &Account, org: &Org, role: Role, tab: Tab, notice: Option<Notice>) -> Reply {
    let acc = cx.acc;
    let manage = acc.org_manager(account, Some(org.id));
    let view = {
        let db = acc.db();
        let computers = db
            .stations_of_org(org.id)
            .unwrap_or_default()
            .into_iter()
            .map(|s| {
                let access: Vec<String> = db
                    .grants_of(&s.id)
                    .unwrap_or_default()
                    .into_iter()
                    .map(|g| {
                        let who = match g.subject {
                            Subject::Person { name, email, .. } => if name.is_empty() { email } else { name },
                            Subject::Invite { email } => email,
                            Subject::Org { name, .. } => name,
                            Subject::Group { name, .. } => match cx.lang {
                                crate::pages::Lang::Fr => format!("Groupe {name}"),
                                crate::pages::Lang::En => format!("Group {name}"),
                            },
                        };
                        format!("{who} : {}", rights_words(cx.lang, &g.rights))
                    })
                    .collect();
                let access = if access.is_empty() { cx.tr("Propriétaires et Administrateurs seulement", "Owners and Administrators only") } else { access.join(" · ") };
                OrgComputer { online: acc.stations.online(&s.id), id: s.id, name: s.name, access: crate::pages::t(cx.lang, &access, &access.replace(" : ", ": ")) }
            })
            .collect();
        let limit = if tab == Tab::Log { 200 } else { 8 };
        OrgView {
            id: org.public_id.clone(),
            name: org.name.clone(),
            kind: org.kind,
            role,
            manage,
            needs_2fa: role.manages() && !manage,
            tab,
            me: account.id,
            members: db.members(org.id).unwrap_or_default(),
            invites: db.org_invites(org.id).unwrap_or_default(),
            groups: db.groups(org.id).unwrap_or_default(),
            computers,
            log: if role.manages() || tab == Tab::Log { db.org_log_of(org.id, limit).unwrap_or_default() } else { Vec::new() },
            shell: Default::default(),
        }
    };
    // Le journal : les Propriétaires et Administrateurs seulement.
    let view = OrgView {
        log: if manage { view.log } else { Vec::new() },
        tab: if !manage && tab == Tab::Log { Tab::Members } else { view.tab },
        shell: shell(&cx, account),
        ..view
    };
    let page = console::org(&cx.view(), &view, notice);
    cx.page(200, page)
}

/// Un formulaire de gestion : l'organisation, si ce compte la gère ; sinon la page qui le dit.
macro_rules! managed {
    ($cx:ident, $account:ident, $tab:expr) => {
        match org_of(&$cx, &$account, true) {
            None => return not_found($cx),
            Some((org, role)) if !$cx.acc.org_manager(&$account, Some(org.id)) => {
                let notice = if role.manages() { Notice::NeedTwoFactor } else { Notice::NotAllowed };
                return show($cx, &$account, &org, role, $tab, Some(notice));
            }
            Some(found) => found,
        }
    };
}

/// `GET/POST /compte/organisations/creer`.
pub(super) fn create(cx: Cx<'_>, post: bool) -> Reply {
    let account = signed_in!(cx);
    let acc = cx.acc;
    let two_factor = acc.db().two_factor(account.id).unwrap_or(false);
    let nav = shell(&cx, &account);
    // Le genre : choisi sur « Familles et équipes » (`genre=famille` ou `equipe`) ; sans genre, le choix.
    let genre = if post { cx.req.form("genre").to_string() } else { cx.req.query("genre").unwrap_or("").to_string() };
    let kind = match genre.as_str() {
        "famille" | "family" => Some(Kind::Family),
        "equipe" | "team" => Some(Kind::Team),
        // Un formulaire sans genre (d'avant l'espace du compte) : une équipe.
        _ if post => Some(Kind::Team),
        _ => None,
    };
    if !post || !two_factor || kind.is_none() {
        let notice = (post && !two_factor).then_some(Notice::NeedTwoFactor);
        let page = console::create_org(&cx.view(), "", kind, two_factor, &nav, notice);
        return cx.page(200, page);
    }
    let kind = kind.unwrap_or(Kind::Team);
    let name = clean_org_name(cx.req.form("nom"));
    if name.is_empty() {
        let page = console::create_org(&cx.view(), "", Some(kind), true, &nav, Some(Notice::OrgNameMissing));
        return cx.page(200, page);
    }
    let created = acc.db().create_org(&name, kind, account.id, cx.now);
    let org = match created {
        Ok(o) => o,
        Err(e) => {
            tracing::warn!("création d'une organisation : {e:#}");
            return cx.failed();
        }
    };
    cx.audit(Some(account.id), "organisation_creee");
    log(&cx, &org, &account, "organisation_creee", json!({}));
    show(cx, &account, &org, Role::Owner, Tab::Members, Some(Notice::OrgCreated))
}

/// `POST /compte/organisation/renommer` (Propriétaires).
pub(super) fn rename(cx: Cx<'_>) -> Reply {
    let account = signed_in!(cx);
    let (org, role) = managed!(cx, account, Tab::Members);
    let name = clean_org_name(cx.req.form("nom"));
    if role != Role::Owner || name.is_empty() {
        let notice = if name.is_empty() { Notice::OrgNameMissing } else { Notice::NotAllowed };
        return show(cx, &account, &org, role, Tab::Members, Some(notice));
    }
    if cx.acc.db().rename_org(org.id, &name).is_err() {
        return cx.failed();
    }
    log(&cx, &org, &account, "organisation_renommee", json!({ "name": name }));
    let org = Org { name, ..org };
    show(cx, &account, &org, role, Tab::Members, Some(Notice::OrgRenamed))
}

/// `POST /compte/organisation/inviter` : un membre par son adresse.
pub(super) fn invite(cx: Cx<'_>) -> Reply {
    let account = signed_in!(cx);
    let acc = cx.acc;
    let (org, role) = managed!(cx, account, Tab::Members);
    let Some(email) = normalize_email(cx.req.form("email")) else { return show(cx, &account, &org, role, Tab::Members, Some(Notice::BadEmail)) };
    let wanted = Role::parse(cx.req.form("role")).unwrap_or(Role::Member);
    // Seul un Propriétaire en nomme un autre.
    if wanted == Role::Owner && role != Role::Owner {
        return show(cx, &account, &org, role, Tab::Members, Some(Notice::NotAllowed));
    }
    let target = acc.db().account_by_email(&email).ok().flatten().filter(|a| a.verified());
    if let Some(t) = &target
        && acc.db().org_role(org.id, t.id).ok().flatten().is_some()
    {
        return show(cx, &account, &org, role, Tab::Members, Some(Notice::NotAllowed));
    }
    let done = match &target {
        Some(t) => acc.db().set_member(org.id, t.id, wanted, cx.now),
        None => acc.db().invite_member(org.id, &email, wanted, account.id, cx.now),
    };
    if let Err(e) = done {
        tracing::warn!("invitation dans une organisation : {e:#}");
        return cx.failed();
    }
    match &target {
        Some(t) => log(&cx, &org, &account, "membre_ajoute", json!({ "member": t.name, "role": wanted.as_str() })),
        None => log(&cx, &org, &account, "membre_invite", json!({ "email": email, "role": wanted.as_str() })),
    }
    if cx.may_mail(&email) {
        let lang = target.as_ref().map(lang_of).unwrap_or(cx.lang);
        let link = match target {
            Some(_) => cx.url(&format!("/compte/organisation?id={}&lang={}", org.public_id, lang.code())),
            None => cx.url(&format!("/compte/inscription?lang={}", lang.code())),
        };
        let by = if account.name.is_empty() { account.email.clone() } else { account.name.clone() };
        let role_word = role_name(lang, wanted);
        cx.mail(&email, lang, Message::OrgInvite { by: &by, org: &org.name, role: &role_word, has_account: target.is_some(), link: &link });
    }
    let notice = if target.is_some() { Notice::MemberAdded } else { Notice::InviteSent };
    show(cx, &account, &org, role, Tab::Members, Some(notice))
}

/// `POST /compte/organisation/role` : changer le rôle d'un membre.
pub(super) fn set_role(cx: Cx<'_>) -> Reply {
    let account = signed_in!(cx);
    let acc = cx.acc;
    let (org, role) = managed!(cx, account, Tab::Members);
    let (Ok(target), Some(wanted)) = (cx.req.form("compte").parse::<i64>(), Role::parse(cx.req.form("role"))) else {
        return show(cx, &account, &org, role, Tab::Members, None);
    };
    let Some(current) = acc.db().org_role(org.id, target).ok().flatten() else { return show(cx, &account, &org, role, Tab::Members, None) };
    if current == wanted {
        return show(cx, &account, &org, role, Tab::Members, None);
    }
    // Un Administrateur ne touche pas aux Propriétaires ; un Propriétaire reste au moins.
    if (current == Role::Owner || wanted == Role::Owner) && role != Role::Owner {
        return show(cx, &account, &org, role, Tab::Members, Some(Notice::NotAllowed));
    }
    if current == Role::Owner && acc.db().owners(org.id).unwrap_or(0) <= 1 {
        return show(cx, &account, &org, role, Tab::Members, Some(Notice::LastOwner));
    }
    if acc.db().set_member(org.id, target, wanted, cx.now).is_err() {
        return cx.failed();
    }
    let name = acc.db().account(target).ok().flatten().map(|a| a.name).unwrap_or_default();
    log(&cx, &org, &account, "role_change", json!({ "member": name, "role": wanted.as_str() }));
    // Lui-même, Propriétaire devenu Membre : son rôle change sur la page.
    let role = if target == account.id { wanted } else { role };
    show(cx, &account, &org, role, Tab::Members, Some(Notice::RoleChanged))
}

/// `POST /compte/organisation/retirer` : un membre (`compte`) ou une invitation (`invitation`).
pub(super) fn remove(cx: Cx<'_>) -> Reply {
    let account = signed_in!(cx);
    let acc = cx.acc;
    let (org, role) = managed!(cx, account, Tab::Members);
    if let Ok(invite) = cx.req.form("invitation").parse::<i64>() {
        let cancelled = acc.db().cancel_invite(org.id, invite);
        return match cancelled {
            Ok(Some(email)) => {
                log(&cx, &org, &account, "invitation_annulee", json!({ "email": email }));
                show(cx, &account, &org, role, Tab::Members, Some(Notice::InviteCancelled))
            }
            _ => show(cx, &account, &org, role, Tab::Members, None),
        };
    }
    let Ok(target) = cx.req.form("compte").parse::<i64>() else { return show(cx, &account, &org, role, Tab::Members, None) };
    let Some(current) = acc.db().org_role(org.id, target).ok().flatten() else { return show(cx, &account, &org, role, Tab::Members, None) };
    if target == account.id || (current == Role::Owner && role != Role::Owner) {
        return show(cx, &account, &org, role, Tab::Members, Some(Notice::NotAllowed));
    }
    let name = acc.db().account(target).ok().flatten().map(|a| a.name).unwrap_or_default();
    if !acc.db().remove_member(org.id, target).unwrap_or(false) {
        return show(cx, &account, &org, role, Tab::Members, None);
    }
    log(&cx, &org, &account, "membre_retire", json!({ "member": name }));
    show(cx, &account, &org, role, Tab::Members, Some(Notice::MemberRemoved))
}

/// `POST /compte/organisation/quitter`.
pub(super) fn leave(cx: Cx<'_>) -> Reply {
    let account = signed_in!(cx);
    let acc = cx.acc;
    let Some((org, role)) = org_of(&cx, &account, true) else { return not_found(cx) };
    if role == Role::Owner && acc.db().owners(org.id).unwrap_or(0) <= 1 {
        return show(cx, &account, &org, role, Tab::Members, Some(Notice::LastOwner));
    }
    if !acc.db().remove_member(org.id, account.id).unwrap_or(false) {
        return cx.failed();
    }
    log(&cx, &org, &account, "membre_parti", json!({}));
    cx.audit(Some(account.id), "organisation_quittee");
    super::console::computers(cx, Some(Notice::OrgLeft))
}

/// `POST /compte/organisation/groupes/creer`.
pub(super) fn group_create(cx: Cx<'_>) -> Reply {
    let account = signed_in!(cx);
    let (org, role) = managed!(cx, account, Tab::Groups);
    let name = clean_org_name(cx.req.form("nom"));
    if name.is_empty() {
        return show(cx, &account, &org, role, Tab::Groups, Some(Notice::OrgNameMissing));
    }
    let created = cx.acc.db().create_group(org.id, &name, cx.now);
    match created {
        Ok(Some(_)) => {
            log(&cx, &org, &account, "groupe_cree", json!({ "group": name }));
            show(cx, &account, &org, role, Tab::Groups, Some(Notice::GroupCreated))
        }
        Ok(None) => show(cx, &account, &org, role, Tab::Groups, Some(Notice::GroupTaken)),
        Err(_) => cx.failed(),
    }
}

/// `POST /compte/organisation/groupes/supprimer` (ses droits donnés partent avec lui).
pub(super) fn group_delete(cx: Cx<'_>) -> Reply {
    let account = signed_in!(cx);
    let acc = cx.acc;
    let (org, role) = managed!(cx, account, Tab::Groups);
    let Ok(grp) = cx.req.form("groupe").parse::<i64>() else { return show(cx, &account, &org, role, Tab::Groups, None) };
    let name = acc.db().group_name(org.id, grp).ok().flatten();
    let Some(name) = name.filter(|_| acc.db().delete_group(org.id, grp).unwrap_or(false)) else {
        return show(cx, &account, &org, role, Tab::Groups, None);
    };
    log(&cx, &org, &account, "groupe_supprime", json!({ "group": name }));
    show(cx, &account, &org, role, Tab::Groups, Some(Notice::GroupDeleted))
}

/// `POST /compte/organisation/groupes/membre` : `action` `ajouter` ou `retirer`.
pub(super) fn group_member(cx: Cx<'_>) -> Reply {
    let account = signed_in!(cx);
    let acc = cx.acc;
    let (org, role) = managed!(cx, account, Tab::Groups);
    let (Ok(grp), Ok(target)) = (cx.req.form("groupe").parse::<i64>(), cx.req.form("compte").parse::<i64>()) else {
        return show(cx, &account, &org, role, Tab::Groups, None);
    };
    let inside = cx.req.form("action") == "ajouter";
    if !acc.db().set_group_member(org.id, grp, target, inside).unwrap_or(false) {
        return show(cx, &account, &org, role, Tab::Groups, None);
    }
    let group = acc.db().group_name(org.id, grp).ok().flatten().unwrap_or_default();
    let member = acc.db().account(target).ok().flatten().map(|a| a.name).unwrap_or_default();
    log(&cx, &org, &account, if inside { "groupe_membre_ajoute" } else { "groupe_membre_retire" }, json!({ "member": member, "group": group }));
    show(cx, &account, &org, role, Tab::Groups, Some(Notice::GroupChanged))
}

// --- Déployer des Stations ---------------------------------------------------------

fn deploy_page(cx: Cx<'_>, account: &Account, org: &Org, new_token: Option<String>, notice: Option<Notice>) -> Reply {
    let acc = cx.acc;
    let view = {
        let db = acc.db();
        let tokens = db
            .enroll_tokens(org.id)
            .unwrap_or_default()
            .into_iter()
            .map(|t| {
                let stations = db.enrolled_by(t.id).unwrap_or_default();
                (t, stations)
            })
            .collect();
        DeployView {
            id: org.public_id.clone(),
            name: org.name.clone(),
            role: db.org_role(org.id, account.id).ok().flatten().unwrap_or(Role::Member),
            tokens,
            groups: db.groups(org.id).unwrap_or_default().into_iter().map(|g| (g.id, g.name)).collect(),
            new_token,
            relay: acc.relay_name(),
            now: cx.now,
            shell: Default::default(),
        }
    };
    let view = DeployView { shell: shell(&cx, account), ..view };
    let page = console::deploy(&cx.view(), &view, notice);
    cx.page(200, page)
}

/// `GET /compte/organisation/deployer?id=…`.
pub(super) fn deploy(cx: Cx<'_>) -> Reply {
    let account = signed_in!(cx);
    let Some((org, role)) = org_of(&cx, &account, false) else { return not_found(cx) };
    if !cx.acc.org_manager(&account, Some(org.id)) {
        let notice = if role.manages() { Notice::NeedTwoFactor } else { Notice::NotAllowed };
        return show(cx, &account, &org, role, Tab::Computers, Some(notice));
    }
    deploy_page(cx, &account, &org, None, None)
}

/// `POST /compte/organisation/jetons/creer` : un jeton, montré une seule fois.
pub(super) fn token_create(cx: Cx<'_>) -> Reply {
    let account = signed_in!(cx);
    let acc = cx.acc;
    let (org, _) = managed!(cx, account, Tab::Computers);
    let name = clean_org_name(cx.req.form("nom"));
    let days = cx.req.form("jours").trim().parse::<i64>().ok().filter(|d| (1..=ENROLL_MAX_DAYS).contains(d));
    let count = cx.req.form("nombre").trim().parse::<i64>().ok().filter(|n| (1..=ENROLL_MAX_STATIONS).contains(n));
    if name.is_empty() {
        return deploy_page(cx, &account, &org, None, Some(Notice::OrgNameMissing));
    }
    let (Some(days), Some(count)) = (days, count) else { return deploy_page(cx, &account, &org, None, Some(Notice::BadNumber)) };
    let grp = match cx.req.form("groupe") {
        "" => None,
        g => match g.parse::<i64>().ok().filter(|g| acc.db().group_name(org.id, *g).ok().flatten().is_some()) {
            Some(g) => Some(g),
            None => return deploy_page(cx, &account, &org, None, Some(Notice::NotAllowed)),
        },
    };
    let rights = if grp.is_some() { form_rights(&cx) } else { Default::default() };
    if grp.is_some() && rights.is_empty() {
        return deploy_page(cx, &account, &org, None, Some(Notice::NoRights));
    }
    let token = new_enroll_token();
    let hint: String = token.chars().take(9).collect();
    let created = acc.db().create_enroll_token(org.id, &name, &token_hash(&token), &hint, count, grp, &rights, account.id, cx.now, cx.now + days * 86_400);
    if let Err(e) = created {
        tracing::warn!("jeton d'enrôlement : {e:#}");
        return cx.failed();
    }
    log(&cx, &org, &account, "jeton_cree", json!({ "token": name }));
    tracing::info!("jeton d'enrôlement créé ({count} Stations, {days} jours, droits {})", rights.iter().map(Right::as_str).collect::<Vec<_>>().join(","));
    deploy_page(cx, &account, &org, Some(token), None)
}

/// `POST /compte/organisation/jetons/revoquer`.
pub(super) fn token_revoke(cx: Cx<'_>) -> Reply {
    let account = signed_in!(cx);
    let acc = cx.acc;
    let (org, _) = managed!(cx, account, Tab::Computers);
    let Ok(id) = cx.req.form("jeton").parse::<i64>() else { return deploy_page(cx, &account, &org, None, None) };
    if !acc.db().revoke_enroll_token(org.id, id, cx.now).unwrap_or(false) {
        return deploy_page(cx, &account, &org, None, None);
    }
    let name = acc.db().enroll_tokens(org.id).unwrap_or_default().into_iter().find(|t| t.id == id).map(|t| t.name).unwrap_or_default();
    log(&cx, &org, &account, "jeton_revoque", json!({ "token": name }));
    deploy_page(cx, &account, &org, None, Some(Notice::TokenRevoked))
}
