//! Les données du compte (lot 1b ; docs/juridique/README.md, « Ce que ces
//! textes demandent au produit ») : l'export (un fichier JSON à télécharger)
//! et la suppression du compte, confirmée par le mot de passe ou une passkey.
//!
//! La suppression efface le compte et tout ce qui lui appartient (sessions,
//! passkeys, double authentification, journal ; Stations rattachées, qui sont
//! prévenues, et appareils connectés du lot 2) tout de suite ; les
//! sauvegardes l'oublient à leur rotation (30 jours au plus). Les comptes
//! jamais vérifiés sont effacés au ménage, sept jours après l'inscription
//! ([`super::super::store::UNVERIFIED_SECS`]).
//!
//! Ce module est un enfant de [`super`] (`web.rs`) : il se sert de son contexte.

use kybernow_tunnel::msg::Msg;

use super::super::pages::{self, Notice};
use super::{Cx, Download, Reply, SESSION_COOKIE, TRUST_COOKIE, account_page, clear_cookie, lang_of, to_sign_in};
use crate::accounts::mail::Message;

/// Le nom du fichier de l'export.
pub const EXPORT_FILE: &str = "kybernow-compte.json";

/// Les passkeys du compte (base64url), pour la confirmation.
fn credentials(cx: &Cx<'_>, account: i64) -> Vec<String> {
    cx.acc.db().passkeys(account).unwrap_or_default().iter().map(|p| super::super::b64(&p.credential)).collect()
}

/// `GET/POST /compte/supprimer` : la confirmation, puis l'effacement.
pub(super) fn delete(mut cx: Cx<'_>, post: bool) -> Reply {
    let acc = cx.acc;
    let Some((_, account)) = cx.full_session() else { return to_sign_in(cx) };
    let allow = credentials(&cx, account.id);
    let show = |cx: Cx<'_>, notice: Option<Notice>, status: u16| {
        let page = pages::delete_account(&cx.view(), &allow, notice);
        cx.page(status, page)
    };
    if !post {
        return show(cx, None, 200);
    }
    let limit = acc.limits.lock().unwrap_or_else(|e| e.into_inner()).sign_in(cx.req.ip, &account.email, false);
    if let Err(wait) = limit {
        let minutes = wait.0.as_secs().div_ceil(60);
        let mut r = show(cx, Some(Notice::TooMany(minutes)), 429);
        r.retry_after = Some(wait.0.as_secs().max(1));
        return r;
    }
    let by_passkey = cx.req.form("action") == "passkey";
    let ok = if by_passkey { cx.check_passkey(Some(account.id)).is_ok() } else { acc.hasher.verify(cx.req.form("password"), Some(&account.password)) };
    if !ok {
        cx.audit(Some(account.id), "suppression_refusee");
        return show(cx, Some(if by_passkey { Notice::PasskeyFailed } else { Notice::BadPassword }), 200);
    }
    acc.limits.lock().unwrap_or_else(|e| e.into_inner()).sign_in_ok(cx.req.ip, &account.email);
    // Ses Stations rattachées (lot 2) : détachées avec le compte, et prévenues si elles sont connectées.
    let stations = acc.db().stations_of(account.id).unwrap_or_default();
    match acc.db().delete_account(account.id) {
        Ok(true) => {
            for station in &stations {
                acc.stations.send(&station.id, Msg::Detached { req: None });
            }
        }
        Ok(false) => return to_sign_in(cx),
        Err(e) => {
            tracing::warn!("compte non supprimé : {e:#}");
            return cx.failed();
        }
    }
    // Le compte et son journal sont effacés : une ligne sans compte ni adresse dans les journaux du relais.
    tracing::info!("compte supprimé à la demande de son titulaire");
    cx.mail(&account.email, lang_of(&account), Message::AccountDeleted);
    cx.cookies.push(clear_cookie(SESSION_COOKIE));
    cx.cookies.push(clear_cookie(TRUST_COOKIE));
    let page = pages::message(
        &cx.view(),
        "/compte",
        &cx.tr("Compte supprimé", "Account deleted"),
        &cx.tr(
            "Votre compte et ses données sont effacés. Les sauvegardes du relais les oublient sous 30 jours.",
            "Your account and its data are erased. The relay's backups forget them within 30 days.",
        ),
        ("/compte/connexion", &cx.tr("Retour à la connexion", "Back to sign in")),
    );
    cx.page(200, page)
}

/// `POST /compte/export` : les données du compte, en JSON à télécharger.
pub(super) fn export(cx: Cx<'_>) -> Reply {
    let acc = cx.acc;
    let Some((_, account)) = cx.full_session() else { return to_sign_in(cx) };
    cx.audit(Some(account.id), "export_des_donnees");
    let data = acc.db().export(account.id, cx.now);
    match data {
        Ok(Some(value)) => {
            let body = serde_json::to_string_pretty(&value).unwrap_or_default();
            Reply {
                status: 200,
                html: None,
                location: None,
                cookies: cx.cookies,
                retry_after: None,
                download: Some(Download { filename: EXPORT_FILE.to_string(), body }),
            }
        }
        Ok(None) => to_sign_in(cx),
        Err(e) => {
            tracing::warn!("export des données : {e:#}");
            account_page(cx, None)
        }
    }
}
