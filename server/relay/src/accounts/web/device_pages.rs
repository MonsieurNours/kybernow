//! Les pages des appareils (docs/comptes.md, moitié relais du lot 3) : la
//! suite après la connexion (cookie `__Host-kn_suite`), « Connecter
//! KyberNow Link sur {appareil} ? » et le retour vers l'app, la saisie du code
//! d'un téléviseur (`/tv`) et son approbation (`/compte/appareil`), la
//! déconnexion d'un appareil depuis la page du compte.

use super::super::devices::{RETURN_URI, dashed};
use super::super::pages::{self, DeviceView, Notice, pct};
use super::super::store::Store;
use super::{Cx, Reply, account_page, appareil, b64, clear_cookie, set_cookie, unb64};

/// Où aller après la connexion (une page du rattachement ou d'un appareil), signé.
pub const NEXT_COOKIE: &str = "__Host-kn_suite";
/// La suite vaut 15 minutes.
const NEXT_SECS: i64 = 15 * 60;

/// Une suite permise : une page des comptes, rien d'autre (pas de redirection ouverte).
fn next_ok(path: &str) -> bool {
    path.starts_with("/compte/") && !path.starts_with("//") && !path.contains(['\r', '\n', '\\'])
}

/// Retient la page où revenir après la connexion.
pub(super) fn set_next(cx: &mut Cx<'_>, path: &str) {
    set_next_for(cx, path, NEXT_SECS);
}

/// Retient la page où revenir, pour ce temps (un lien d'accès : le temps de créer un compte et de vérifier l'adresse).
pub(super) fn set_next_for(cx: &mut Cx<'_>, path: &str, secs: i64) {
    if !next_ok(path) {
        return;
    }
    let value = b64(path.as_bytes());
    let exp = cx.now + secs;
    let sig = cx.acc.keys.sign(format!("suite:{value}.{exp}").as_bytes());
    cx.cookies.push(set_cookie(NEXT_COOKIE, &format!("{value}.{exp}.{sig}"), Some(secs)));
}

/// La suite retenue (et l'efface), si elle vaut encore.
pub(super) fn take_next(cx: &mut Cx<'_>) -> Option<String> {
    let raw = cx.req.cookie(NEXT_COOKIE)?.to_string();
    cx.cookies.push(clear_cookie(NEXT_COOKIE));
    let mut parts = raw.split('.');
    let (value, exp, sig) = (parts.next()?, parts.next()?, parts.next()?);
    if !cx.acc.keys.check(format!("suite:{value}.{exp}").as_bytes(), sig) || exp.parse::<i64>().ok()? <= cx.now {
        return None;
    }
    String::from_utf8(unb64(value)?).ok().filter(|p| next_ok(p))
}

/// La page de la requête, avec sa requête (`/compte/rattacher?code=…&lang=fr`).
pub(super) fn here(cx: &Cx<'_>) -> String {
    let query: Vec<String> = cx.req.query.iter().map(|(k, v)| format!("{}={}", pct(k), pct(v))).collect();
    if query.is_empty() { cx.req.path.clone() } else { format!("{}?{}", cx.req.path, query.join("&")) }
}

/// Pas de session entière : retenir cette page, puis se connecter (ou donner le code).
pub(super) fn sign_in_first(mut cx: Cx<'_>) -> Reply {
    let path = here(&cx);
    set_next(&mut cx, &path);
    let q = cx.q();
    if cx.session().is_some_and(|(s, _)| !s.mfa_done) {
        return cx.redirect(format!("/compte/code{q}"));
    }
    cx.redirect(format!("/compte/connexion{q}"))
}

/// Après la connexion (mot de passe, ou code) : la suite retenue, sinon la
/// demande de l'app (`?appareil=`), sinon la page du compte.
pub(super) fn after_sign_in(mut cx: Cx<'_>) -> Reply {
    if let Some(next) = take_next(&mut cx) {
        return cx.redirect(next);
    }
    let q = cx.q();
    if appareil(cx.req).is_some() {
        return cx.redirect(format!("/compte/connecter-appareil{q}"));
    }
    // L'espace s'ouvre sur « Ordinateurs » (ou le tour d'accueil, la première fois).
    cx.redirect(format!("/compte/ordinateurs{q}"))
}

/// Efface la suite retenue (en-tête à ajouter aux cookies de la réponse).
pub(super) fn clear_next() -> String {
    clear_cookie(NEXT_COOKIE)
}

/// `etat` donné par l'app : 128 signes d'adresse au plus, sinon rien.
fn etat(value: &str) -> &str {
    if value.len() <= 128 && value.bytes().all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~')) { value } else { "" }
}

/// `GET /compte/connexion?appareil=…&retour=kybernow://compte` : la demande
/// de l'app est retenue pour après la connexion ; rend la page de la demande
/// (« Connecter KyberNow Link sur … ? », avec son `etat`). `None` : retour refusé.
pub(super) fn remember_device_request(cx: &mut Cx<'_>) -> Option<String> {
    if cx.req.query("retour").is_some_and(|r| r != RETURN_URI) {
        return None;
    }
    let mut path = format!("/compte/connecter-appareil{}", cx.q());
    let state = etat(cx.req.query("etat").unwrap_or(""));
    if !state.is_empty() {
        path.push_str(&format!("&etat={state}"));
    }
    set_next(cx, &path);
    Some(path)
}

/// Le retour vers l'app refusé (`retour` autre que `kybernow://compte`).
pub(super) fn bad_return(cx: Cx<'_>) -> Reply {
    let page = pages::message(
        &cx.view(),
        "/compte/connexion",
        &cx.tr("Retour refusé", "Return refused"),
        &cx.tr("Cette demande de connexion ne vient pas d'une app KyberNow.", "This sign-in request doesn't come from a KyberNow app."),
        ("/compte", &cx.tr("Votre compte", "Your account")),
    );
    cx.page(400, page)
}

/// « Connecter KyberNow Link sur {appareil} ? », puis le retour vers l'app.
pub(super) fn consent(cx: Cx<'_>, post: bool) -> Reply {
    let acc = cx.acc;
    let Some(device) = appareil(cx.req).map(str::to_string) else {
        let q = cx.q();
        return cx.redirect(format!("/compte{q}"));
    };
    let Some((_, account)) = cx.full_session() else {
        return sign_in_first(cx);
    };
    if !post {
        let state = etat(cx.req.query("etat").unwrap_or("")).to_string();
        let page = pages::device_consent(&cx.view(), &device, &account.email, &state);
        return cx.page(200, page);
    }
    let state = etat(cx.req.form("etat")).to_string();
    let suffix = if state.is_empty() { String::new() } else { format!("&etat={state}") };
    if cx.req.form("action") != "connecter" {
        cx.audit(Some(account.id), "appareil_annule");
        let page = pages::device_return(&cx.view(), &format!("{RETURN_URI}?erreur=annule{suffix}"), false);
        return cx.page(200, page);
    }
    let code = match acc.return_code(&account, &device) {
        Ok(code) => code,
        Err(e) => {
            tracing::warn!("code de retour : {e:#}");
            return cx.failed();
        }
    };
    cx.audit(Some(account.id), "appareil_autorise");
    // Une page plutôt qu'une redirection : `form-action 'self'` ne laisse pas un formulaire mener ailleurs.
    let page = pages::device_return(&cx.view(), &format!("{RETURN_URI}?code={code}{suffix}"), true);
    cx.page(200, page)
}

/// `/tv` : la saisie du code.
pub(super) fn tv(cx: Cx<'_>) -> Reply {
    let page = pages::tv_entry(&cx.view(), "", None);
    cx.page(200, page)
}

/// `/compte/appareil?code=…` : « Connecter {appareil} ? », « Ce n'est pas moi ».
pub(super) fn approve(cx: Cx<'_>, post: bool) -> Reply {
    let acc = cx.acc;
    let typed = if post { cx.req.form("code").to_string() } else { cx.req.query("code").unwrap_or("").to_string() };
    if typed.trim().is_empty() {
        let q = cx.q();
        return cx.redirect(format!("/tv{q}"));
    }
    let Some((_, account)) = cx.full_session() else {
        return sign_in_first(cx);
    };
    if let Err(wait) = acc.devices.tv_try(cx.req.ip) {
        return cx.limited(super::super::Limited(wait), |cx, n| pages::tv_entry(&cx.view(), &typed, Some(n)));
    }
    let Some((code, found)) = acc.tv_code(&typed) else {
        cx.audit(Some(account.id), "code_d_appareil_refuse");
        let page = pages::tv_entry(&cx.view(), &typed, Some(Notice::WrongCode));
        return cx.page(200, page);
    };
    acc.devices.tv_try_ok(cx.req.ip);
    if found.status != "pending" {
        let page = pages::tv_entry(&cx.view(), &typed, Some(Notice::WrongCode));
        return cx.page(200, page);
    }
    if !post {
        let page = pages::tv_approve(&cx.view(), &dashed(&code), &found.device_name, &account.email);
        return cx.page(200, page);
    }
    let yes = cx.req.form("action") == "connecter";
    if !acc.answer_tv_code(&code, account.id, yes) {
        let page = pages::tv_entry(&cx.view(), &typed, Some(Notice::WrongCode));
        return cx.page(200, page);
    }
    let name = if found.device_name.is_empty() { cx.tr("L'appareil", "The device") } else { found.device_name.clone() };
    let (title, text) = if yes {
        (
            cx.tr("Appareil connecté", "Device connected"),
            match cx.lang {
                crate::pages::Lang::Fr => crate::pages::t(cx.lang, &format!("{} est connecté à votre compte : il continue tout seul.", pages::esc(&name)), ""),
                crate::pages::Lang::En => format!("{} is connected to your account: it carries on by itself.", pages::esc(&name)),
            },
        )
    } else {
        (
            cx.tr("Appareil refusé", "Device refused"),
            match cx.lang {
                crate::pages::Lang::Fr => crate::pages::t(
                    cx.lang,
                    &format!("{} n'est pas connecté. Si quelqu'un d'autre a pu voir ce code, rien n'a changé : il ne vaut plus.", pages::esc(&name)),
                    "",
                ),
                crate::pages::Lang::En => {
                    format!("{} isn't connected. If someone else could see this code, nothing has changed: it's no longer valid.", pages::esc(&name))
                }
            },
        )
    };
    let page = pages::done(&cx.view(), "/compte/appareil", &title, &text);
    cx.page(200, page)
}

/// Les appareils connectés, pour la page du compte (pas ceux qui n'ont pas servi depuis 90 jours).
pub(super) fn devices(db: &Store, account: i64, now: i64) -> Vec<DeviceView> {
    db.devices_of(account)
        .unwrap_or_default()
        .into_iter()
        .filter(|d| now - d.seen_at < super::super::devices::DEVICE_IDLE_SECS)
        .map(|d| DeviceView { id: d.id, name: d.name, tv: d.kind == "tv", created_at: d.created_at, seen_at: d.seen_at })
        .collect()
}

/// « Déconnecter » un appareil.
pub(super) fn revoke(cx: Cx<'_>) -> Reply {
    let acc = cx.acc;
    let Some((_, account)) = cx.full_session() else {
        return sign_in_first(cx);
    };
    let Ok(id) = cx.req.form("id").parse::<i64>() else {
        return account_page(cx, None);
    };
    if acc.db().revoke_device(id, account.id).unwrap_or(false) {
        cx.audit(Some(account.id), "appareil_deconnecte");
        return account_page(cx, Some(Notice::DeviceClosed));
    }
    account_page(cx, None)
}
