//! Les liens d'accès dans la console (docs/comptes.md, « Regarder, liens
//! d'accès et groupes d'ordinateurs ») : les créer et les révoquer depuis la
//! page d'un ordinateur (qui gère ses accès), et les ouvrir
//! (`/compte/lien/kla_…`).
//!
//! Ouvrir un lien sans être connecté : la page propose de se connecter ou de
//! créer un compte, et le lien est retenu (cookie signé `__Host-kn_suite`, une
//! heure) pour y revenir après la connexion ou la vérification de l'adresse.
//! Le lien n'est jamais écrit dans les journaux ; les essais de liens inconnus
//! sont limités comme les codes.

use kybernow_tunnel::access::Right;
use serde_json::json;

use super::super::links::{LINK_DAYS, LinkState, MAX_PENDING, link_token_ok, new_link_token};
use super::super::pages::{self, Notice, console};
use super::super::people::rights_text;
use super::super::stations::StationRow;
use super::super::store::Account;
use super::console::{Kept, bounded, form_rights, managed_station, not_found, parse_until_tz, people_page, signed_in, tz_of};
use super::device_pages::{set_next_for, sign_in_first};
use super::{Cx, Reply, token_hash};

/// Le lien retenu pendant la connexion ou l'inscription (le temps de vérifier l'adresse).
const LINK_NEXT_SECS: i64 = 3600;

/// `POST /compte/ordinateur/liens/creer` : `id`, droits, `jours` (1, 7 ou 30), `fin` (facultatif).
pub(super) fn create(cx: Cx<'_>) -> Reply {
    let account = signed_in!(cx);
    let acc = cx.acc;
    let Some(station) = managed_station(&cx, &account) else { return not_found(cx) };
    let rights = form_rights(&cx);
    let days = cx.req.form("jours").trim().parse::<i64>().unwrap_or(0);
    let until_text = cx.req.form("fin").to_string();
    let kept = |days: i64| Kept { link_form: Some((rights.clone(), if LINK_DAYS.contains(&days) { days } else { 7 }, until_text.clone())), ..Kept::default() };
    if rights.is_empty() {
        return people_page(cx, &account, &station, Some(Notice::NoRights), kept(days));
    }
    if !LINK_DAYS.contains(&days) {
        return people_page(cx, &account, &station, Some(Notice::BadNumber), kept(days));
    }
    let Some(until) = parse_until_tz(&until_text, cx.now, tz_of(&cx)) else {
        return people_page(cx, &account, &station, Some(Notice::BadDate), kept(days));
    };
    // Qui a reçu Administrer ne donne que ses droits, jusqu'à leur échéance (REL-02).
    let Some(until) = bounded(&cx, &account, &station, &rights, until) else {
        return people_page(cx, &account, &station, Some(Notice::NotAllowed), kept(days));
    };
    if acc.db().pending_links(&station.id, cx.now).unwrap_or(MAX_PENDING) >= MAX_PENDING {
        return people_page(cx, &account, &station, Some(Notice::TooManyLinks), Kept::default());
    }
    let token = new_link_token();
    let hint: String = token.chars().take(9).collect();
    let created = acc.db().create_access_link(&station.id, &token_hash(&token), &hint, &rights, cx.now + days * 86_400, until, account.id, cx.now);
    if let Err(e) = created {
        tracing::warn!("lien d'accès : {e:#}");
        return cx.failed();
    }
    cx.audit(Some(account.id), "lien_cree");
    if let Some(org) = station.org {
        let detail = json!({ "station": station.name, "rights": rights_text(&rights) });
        let _ = acc.db().org_log(org, Some(&account), "lien_cree", detail, Some(&cx.ip()), cx.now);
    }
    tracing::info!(station = %station.id, jours = days, droits = %rights_text(&rights), "lien d'accès créé");
    let url = cx.url(&format!("/compte/lien/{token}"));
    people_page(cx, &account, &station, None, Kept { new_link: Some(url), ..Kept::default() })
}

/// `POST /compte/ordinateur/liens/revoquer` : `id`, `lien`.
pub(super) fn revoke(cx: Cx<'_>) -> Reply {
    let account = signed_in!(cx);
    let acc = cx.acc;
    let Some(station) = managed_station(&cx, &account) else { return not_found(cx) };
    let Ok(id) = cx.req.form("lien").parse::<i64>() else { return people_page(cx, &account, &station, None, Kept::default()) };
    let revoked = acc.db().revoke_access_link(&station.id, id, cx.now).ok().flatten();
    let Some(link) = revoked else { return people_page(cx, &account, &station, None, Kept::default()) };
    cx.audit(Some(account.id), "lien_revoque");
    if let Some(org) = station.org {
        let detail = json!({ "station": station.name, "link": link.hint });
        let _ = acc.db().org_log(org, Some(&account), "lien_revoque", detail, Some(&cx.ip()), cx.now);
    }
    people_page(cx, &account, &station, Some(Notice::LinkRevoked), Kept::default())
}

/// Pourquoi un lien ne s'ouvre pas.
#[derive(Clone, Copy)]
enum Refusal {
    Used,
    Expired,
    Revoked,
    Unreadable,
}

fn refused(cx: Cx<'_>, here: &str, why: Refusal) -> Reply {
    let (status, fr, en) = match why {
        Refusal::Used => (410, "Ce lien a déjà servi.", "This link was already used."),
        Refusal::Expired => (410, "Ce lien a expiré.", "This link has expired."),
        Refusal::Revoked => (410, "Ce lien a été révoqué.", "This link was revoked."),
        Refusal::Unreadable => (404, "Ce lien est illisible.", "This link can't be read."),
    };
    let ask = cx.tr(
        "Demandez un nouveau lien à la personne qui vous l'a envoyé.",
        "Ask the person who sent it to you for a new link.",
    );
    let page = pages::message(
        &cx.view(),
        here,
        &cx.tr("Lien d'accès", "Access link"),
        &format!("{} {ask}", cx.tr(fr, en)),
        ("/compte/ordinateurs", &cx.tr("Vos ordinateurs", "Your computers")),
    );
    cx.page(status, page)
}

/// Le nom d'une personne : son nom, sinon son adresse.
fn display_name(a: &Account) -> String {
    if a.name.is_empty() { a.email.clone() } else { a.name.clone() }
}

/// `GET/POST /compte/lien/<lien>` : « {nom} vous ouvre {ordinateur} », puis « Accepter ».
pub(super) fn open(mut cx: Cx<'_>, token: &str, post: bool) -> Reply {
    let acc = cx.acc;
    let here = format!("/compte/lien/{token}");
    // Les essais de liens inconnus sont limités (comme les codes) ; un lien connu rend son essai.
    let limit_key = format!("lien:{}", super::super::ip_key(cx.req.ip));
    let limit = acc.limits.lock().unwrap_or_else(|e| e.into_inner()).code(cx.req.ip, &limit_key);
    if let Err(wait) = limit {
        return cx.limited(wait, |cx, n| {
            pages::message(&cx.view(), &here, &cx.tr("Lien d'accès", "Access link"), &n.text(cx.lang), ("/compte/ordinateurs", &cx.tr("Vos ordinateurs", "Your computers")))
        });
    }
    let link = if link_token_ok(token) { acc.db().access_link_by_hash(&token_hash(token)).ok().flatten() } else { None };
    let Some(link) = link else { return refused(cx, &here, Refusal::Unreadable) };
    acc.limits.lock().unwrap_or_else(|e| e.into_inner()).code_ok(cx.req.ip, &limit_key);
    match link.state(cx.now) {
        LinkState::Used => return refused(cx, &here, Refusal::Used),
        LinkState::Expired => return refused(cx, &here, Refusal::Expired),
        LinkState::Revoked => return refused(cx, &here, Refusal::Revoked),
        LinkState::Pending => {}
    }
    // L'ordinateur, et qui a créé le lien : il doit encore gérer ses accès, sinon le lien ne vaut plus.
    let station = acc.db().station(&link.station).ok().flatten();
    let Some(station) = station else { return refused(cx, &here, Refusal::Unreadable) };
    let creator = link.created_by.and_then(|c| acc.db().account(c).ok().flatten());
    let Some(creator) = creator.filter(|c| acc.can_manage(c, &station)) else { return refused(cx, &here, Refusal::Revoked) };
    let by = display_name(&creator);
    let Some((_, account)) = cx.full_session() else {
        // Mot de passe donné, code attendu : le code d'abord, puis le lien.
        if cx.session().is_some_and(|(s, _)| !s.mfa_done) {
            return sign_in_first(cx);
        }
        let path = format!("{here}{}", cx.q());
        set_next_for(&mut cx, &path, LINK_NEXT_SECS);
        let page = console::link_sign_in(&cx.view(), &here, &by, &station.name);
        return cx.page(200, page);
    };
    // Le propriétaire (ou qui gère déjà l'ordinateur, ou qui a créé le lien) : le lien reste en attente.
    let own = station.account == Some(account.id) || acc.org_manager(&account, station.org) || link.created_by == Some(account.id);
    if own {
        let page = pages::message(
            &cx.view(),
            &here,
            &console::link_title(cx.lang, &pages::esc(&by), &pages::esc(&station.name)),
            &cx.tr("C'est votre ordinateur : ce lien est pour quelqu'un d'autre.", "It's your computer: this link is for someone else."),
            ("/compte/ordinateurs", &cx.tr("Vos ordinateurs", "Your computers")),
        );
        return cx.page(200, page);
    }
    let needs_2fa = link.rights.has(Right::Admin) && !acc.db().two_factor(account.id).unwrap_or(false);
    if !post {
        let page = console::link_accept(&cx.view(), &here, &by, &station.name, &link.rights, link.access_expires_at, needs_2fa);
        return cx.page(200, page);
    }
    // Le créateur a reçu Administrer : le lien ne donne que ce qu'il a encore, jusqu'à son échéance (REL-02).
    let Some(end) = bounded(&cx, &creator, &station, &link.rights, link.access_expires_at) else { return refused(cx, &here, Refusal::Revoked) };
    let used = acc.db().use_access_link(link.id, account.id, end, cx.now);
    match used {
        Ok(true) => {}
        Ok(false) => {
            // Utilisé (ou révoqué) entre-temps : le refus dit lequel.
            let state = acc.db().access_link_by_hash(&token_hash(token)).ok().flatten().map(|l| l.state(cx.now));
            let why = match state {
                Some(LinkState::Expired) => Refusal::Expired,
                Some(LinkState::Revoked) => Refusal::Revoked,
                _ => Refusal::Used,
            };
            return refused(cx, &here, why);
        }
        Err(e) => {
            tracing::warn!("lien d'accès : {e:#}");
            return cx.failed();
        }
    }
    accepted(cx, &account, &creator, &station, &link.rights)
}

/// Le lien a servi : le journal, puis la page de l'ordinateur.
fn accepted(cx: Cx<'_>, account: &Account, creator: &Account, station: &StationRow, rights: &kybernow_tunnel::access::Rights) -> Reply {
    let acc = cx.acc;
    cx.audit(Some(account.id), "lien_accepte");
    if let Err(e) = acc.db().audit(Some(creator.id), "lien_utilise", None, cx.now) {
        tracing::warn!("journal des comptes : {e:#}");
    }
    if let Some(org) = station.org {
        let detail = json!({ "station": station.name, "who": display_name(account), "rights": rights_text(rights) });
        let _ = acc.db().org_log(org, Some(account), "lien_utilise", detail, Some(&cx.ip()), cx.now);
    }
    tracing::info!(station = %station.id, "lien d'accès utilisé");
    // Seulement Administrer, sans double authentification : rien ne compte encore, la page le dit.
    if acc.standing(account, station).is_none() {
        let page = pages::message(
            &cx.view(),
            "/compte/ordinateurs",
            &cx.tr("Lien accepté", "Link accepted"),
            &cx.tr(
                "Le droit Administrer comptera dès que vous aurez activé la double authentification (ou ajouté une passkey).",
                "The Administer right will count as soon as you turn on two-factor authentication (or add a passkey).",
            ),
            ("/compte/double-authentification", &cx.tr("L'activer", "Turn it on")),
        );
        return cx.page(200, page);
    }
    people_page(cx, account, station, Some(Notice::LinkAccepted), Kept::default())
}
