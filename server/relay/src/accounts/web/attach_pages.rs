//! Les pages du rattachement (docs/comptes.md, « Rattacher une Station »,
//! lot 2) : « Rattacher {nom} » (`/compte/rattacher?code=…`, ouverte par
//! Admin), et « Détacher » un ordinateur depuis la page du compte.

use super::super::pages::{self, ComputerView, Notice, esc};
use super::super::stations::Stations;
use super::super::store::Store;
use super::device_pages::sign_in_first;
use super::{Cx, Reply, account_page};
use crate::pages::{Lang, t};

/// Le code ne vaut plus : on recommence depuis Admin.
fn expired(cx: Cx<'_>) -> Reply {
    let page = pages::message(
        &cx.view(),
        "/compte/rattacher",
        &cx.tr("Lien expiré", "Link expired"),
        &cx.tr(
            "Ce lien de rattachement a déjà servi ou ne vaut plus (15 minutes). Recommencez depuis Admin : « Compte KyberNow », « Se connecter ».",
            "This attach link was already used or is no longer valid (15 minutes). Start again from Admin: \"KyberNow account\", \"Sign in\".",
        ),
        ("/compte", &cx.tr("Votre compte", "Your account")),
    );
    cx.page(400, page)
}

/// `/compte/rattacher` : la page, puis la confirmation.
pub(super) fn attach(cx: Cx<'_>, post: bool) -> Reply {
    let acc = cx.acc;
    let code = if post { cx.req.form("code").to_string() } else { cx.req.query("code").unwrap_or("").to_string() };
    if code.is_empty() || code.len() > 64 {
        return expired(cx);
    }
    let Some((_, account)) = cx.full_session() else {
        return sign_in_first(cx);
    };
    if !post {
        let Some(pending) = acc.stations.pending(&code, cx.now) else {
            return expired(cx);
        };
        let page = pages::attach(&cx.view(), &code, &pending.name, &account.name, &account.email);
        return cx.page(200, page);
    }
    if cx.req.form("action") != "rattacher" {
        // Annulé : le code reste valable, Admin peut rouvrir la page.
        let page = pages::done(
            &cx.view(),
            "/compte/rattacher",
            &cx.tr("Rattachement annulé", "Attaching cancelled"),
            &cx.tr("L'ordinateur n'est pas rattaché à votre compte.", "The computer isn't attached to your account."),
        );
        return cx.page(200, page);
    }
    // TODO lot 5 : `proprietaire` peut désigner une organisation que ce compte administre.
    let Some(pending) = acc.stations.take(&code, cx.now) else {
        return expired(cx);
    };
    let delivered = match acc.confirm_attach(&pending, &account) {
        Ok(d) => d,
        Err(e) => {
            tracing::warn!("rattachement : {e:#}");
            return cx.failed();
        }
    };
    cx.audit(Some(account.id), "station_rattachee");
    tracing::info!(station = %pending.station, en_ligne = delivered, "Station rattachée à un compte");
    let name = if pending.name.is_empty() { cx.tr("L'ordinateur", "The computer") } else { pending.name.clone() };
    let text = match (cx.lang, delivered) {
        (Lang::Fr, true) => t(cx.lang, &format!("{} est rattaché à votre compte : Admin l'affiche déjà, et il apparaît dans vos apps.", esc(&name)), ""),
        (Lang::En, true) => format!("{} is attached to your account: Admin already shows it, and it appears in your apps.", esc(&name)),
        (Lang::Fr, false) => t(
            cx.lang,
            &format!("{} est rattaché à votre compte. Il n'est pas connecté au relais en ce moment : Admin le saura à sa prochaine connexion.", esc(&name)),
            "",
        ),
        (Lang::En, false) => {
            format!("{} is attached to your account. It isn't connected to the relay right now: Admin will know at its next connection.", esc(&name))
        }
    };
    let page = pages::done(&cx.view(), "/compte/rattacher", &cx.tr("Ordinateur rattaché", "Computer attached"), &text);
    cx.page(200, page)
}

/// Les ordinateurs rattachés, pour la page du compte.
pub(super) fn computers(db: &Store, stations: &Stations, account: i64) -> Vec<ComputerView> {
    db.stations_of(account)
        .unwrap_or_default()
        .into_iter()
        .map(|s| ComputerView { online: stations.online(&s.id), id: s.id, name: s.name, attached_at: s.attached_at })
        .collect()
}

/// « Détacher » un ordinateur depuis la console : la Station le sait si elle est connectée.
pub(super) fn detach(cx: Cx<'_>) -> Reply {
    let acc = cx.acc;
    let Some((_, account)) = cx.full_session() else {
        return sign_in_first(cx);
    };
    let id = cx.req.form("id").to_string();
    match acc.detach_by_account(&id, account.id) {
        Ok(true) => {
            cx.audit(Some(account.id), "station_detachee");
            account_page(cx, Some(Notice::StationDetached))
        }
        Ok(false) => account_page(cx, None),
        Err(e) => {
            tracing::warn!("détachement : {e:#}");
            cx.failed()
        }
    }
}
