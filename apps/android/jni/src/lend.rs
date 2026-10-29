//! Le prêt de l'ordinateur pour l'app Android (docs/pret.md) : les mêmes
//! fonctions que le pont C (`apps/apple/src/lend.rs`), pour `Native`.
//! Réponses en JSON comme les autres appels : `{"code": 0, "value": …}` ou
//! `{"code": …, "error": "…", "kind": "…"}` ; refus de l'emprunt : 40
//! identifiant ou mot de passe refusé, 41 code qui ne sert plus, 42 déjà
//! prêté, 43 verrouillé, 44 prêt coupé, 45 trop d'essais, 46 refusé par
//! l'hôte, 47 sans réponse de l'hôte, 48 Station sans prêt ; 49 `lendWait`
//! toujours en attente.

use jni::objects::{JClass, JString};
use jni::sys::{jboolean, jint, jlong, jstring};

use kybernow_core::i18n;
use kybernow_core::lend::{self, BorrowRequest, LendAvailability, LendError, LendNotice, LendSteam, Loan, PendingBorrow};
use kybernow_core::relay::RelayRoute;
use kybernow_core::share::ShareTarget;

use crate::exports::{console, envelope, give, optional, string, Env};

fn lend_envelope<T: serde::Serialize>(r: Result<T, LendError>, host: &str) -> String {
    match r {
        Ok(v) => serde_json::json!({ "code": 0, "value": v }).to_string(),
        Err(LendError::Other(e)) => envelope::<()>(Err(e)),
        Err(e) => serde_json::json!({ "code": e.code().unwrap_or(1), "error": e.text(i18n::current(), host), "kind": e.kind() }).to_string(),
    }
}

fn bad(e: impl std::fmt::Display) -> String {
    serde_json::json!({ "code": 1, "error": e.to_string() }).to_string()
}

fn route(env: &mut Env, route: &JString) -> Option<RelayRoute> {
    optional(env, route).and_then(|r| serde_json::from_str::<RelayRoute>(&r).ok())
}

/// La carte « Ordinateur de {hôte} » : `target` `{host, port, fingerprint}`,
/// `route` sa route par le relais (ou `null`) ; `value` `{state, host,
/// computer, slot}`. Bloquant.
#[unsafe(no_mangle)]
pub extern "system" fn Java_fr_leslignesclaires_kybernow_core_Native_lendStatus(mut env: Env, _: JClass, target: JString, route_json: JString, login: JString, password: JString) -> jstring {
    let (target, route, login, password) = (string(&mut env, &target), route(&mut env, &route_json), string(&mut env, &login), string(&mut env, &password));
    let s = match serde_json::from_str::<ShareTarget>(&target) {
        Ok(t) => lend_envelope(lend::status(&t, route.as_ref(), &login, &password), ""),
        Err(e) => bad(e),
    };
    give(&mut env, &s)
}

/// « Emprunter » : `request` `{"code"}` ou `{"login","password"}` avec
/// `name`, `device`, `app`, `screen`, `codecs` ; `value` `{"ready": {target,
/// borrowed}}` ou `{"pending": {…}}`. Bloquant (30 s au plus).
#[unsafe(no_mangle)]
pub extern "system" fn Java_fr_leslignesclaires_kybernow_core_Native_lendBorrow(mut env: Env, _: JClass, target: JString, route_json: JString, request: JString) -> jstring {
    let (target, route, request) = (string(&mut env, &target), route(&mut env, &route_json), string(&mut env, &request));
    let s = match (serde_json::from_str::<ShareTarget>(&target), serde_json::from_str::<BorrowRequest>(&request)) {
        (Ok(t), Ok(r)) => lend_envelope(lend::borrow(&t, route.as_ref(), &r), ""),
        (Err(e), _) => bad(e),
        (_, Err(e)) => bad(e),
    };
    give(&mut env, &s)
}

/// L'hôte a-t-il répondu ? Tenu 30 s : `value` `{target, borrowed}`
/// (accepté), code 49 (rappeler), 46 refusé, 47 sans réponse. Bloquant.
#[unsafe(no_mangle)]
pub extern "system" fn Java_fr_leslignesclaires_kybernow_core_Native_lendWait(mut env: Env, _: JClass, pending: JString) -> jstring {
    let pending = string(&mut env, &pending);
    let s = match serde_json::from_str::<PendingBorrow>(&pending) {
        Ok(p) => match lend::wait(&p) {
            Ok(Some(loan)) => serde_json::json!({ "code": 0, "value": loan }).to_string(),
            Ok(None) => serde_json::json!({ "code": lend::CODE_WAITING }).to_string(),
            Err(e) => lend_envelope::<()>(Err(e), &p.host),
        },
        Err(e) => bad(e),
    };
    give(&mut env, &s)
}

/// « Annuler » une demande en attente. Bloquant.
#[unsafe(no_mangle)]
pub extern "system" fn Java_fr_leslignesclaires_kybernow_core_Native_lendCancel(mut env: Env, _: JClass, pending: JString) -> jint {
    match serde_json::from_str::<PendingBorrow>(&string(&mut env, &pending)) {
        Ok(p) => {
            lend::cancel(&p);
            0
        }
        Err(_) => 1,
    }
}

/// « Rendre l'ordinateur » hors d'une session (`loan` : `{target,
/// borrowed}`) ; en session : `sessionGiveBack`. Bloquant.
#[unsafe(no_mangle)]
pub extern "system" fn Java_fr_leslignesclaires_kybernow_core_Native_lendLeave(mut env: Env, _: JClass, loan: JString) -> jstring {
    let loan = string(&mut env, &loan);
    let s = match serde_json::from_str::<Loan>(&loan) {
        Ok(l) => lend_envelope(lend::leave(&l), ""),
        Err(e) => bad(e),
    };
    give(&mut env, &s)
}

/// Un lien d'invitation : `{host, port, fingerprint, login, relay, sid,
/// route}` (sert aussi de `target`), `null` si ce n'en est pas un.
#[unsafe(no_mangle)]
pub extern "system" fn Java_fr_leslignesclaires_kybernow_core_Native_lendParseInvite(mut env: Env, _: JClass, url: JString) -> jstring {
    let url = string(&mut env, &url);
    match lend::parse_invite(&url) {
        Some(invite) => {
            let mut v = serde_json::to_value(&invite).unwrap_or_default();
            v["route"] = serde_json::to_value(invite.relay_route()).unwrap_or_default();
            give(&mut env, &v.to_string())
        }
        None => std::ptr::null_mut(),
    }
}

/// L'état de la carte (`free`, `busy`, `lent`, `locked`, `off`) dans la
/// langue de l'app ; `null` pour un état inconnu.
#[unsafe(no_mangle)]
pub extern "system" fn Java_fr_leslignesclaires_kybernow_core_Native_lendAvailabilityLabel(mut env: Env, _: JClass, state: JString) -> jstring {
    let state = string(&mut env, &state);
    match LendAvailability::parse(&state) {
        Some(a) => give(&mut env, &a.label(i18n::current())),
        None => std::ptr::null_mut(),
    }
}

/// Le texte d'un préavis (`notice` `{reason, at}`) ; `name` : l'hôte chez
/// l'ami, l'ami pour `slot_start` chez l'hôte.
#[unsafe(no_mangle)]
pub extern "system" fn Java_fr_leslignesclaires_kybernow_core_Native_lendNoticeText(mut env: Env, _: JClass, notice: JString, name: JString) -> jstring {
    let (notice, name) = (string(&mut env, &notice), string(&mut env, &name));
    match serde_json::from_str::<LendNotice>(&notice) {
        Ok(n) => give(&mut env, &n.text(i18n::current(), &name)),
        Err(_) => std::ptr::null_mut(),
    }
}

// L'hôte, sur la console de l'ordinateur : `value` = la vue du prêt.

#[unsafe(no_mangle)]
pub extern "system" fn Java_fr_leslignesclaires_kybernow_core_Native_consoleLendState(mut env: Env, _: JClass, h: jlong) -> jstring {
    let Some(c) = console(h) else { return std::ptr::null_mut() };
    let s = envelope(c.lend_state());
    give(&mut env, &s)
}

/// « Créer un code » : `duration` en secondes (0 ou moins : « Jusqu'à ce
/// que je reprenne »), `steam` 0 son compte, 1 celui de l'hôte.
#[unsafe(no_mangle)]
pub extern "system" fn Java_fr_leslignesclaires_kybernow_core_Native_consoleLendCode(mut env: Env, _: JClass, h: jlong, duration: jlong, steam: jint) -> jstring {
    let Some(c) = console(h) else { return std::ptr::null_mut() };
    let duration = (duration > 0).then_some(duration as u64);
    let s = envelope(c.lend_code(duration, LendSteam::from_code(steam)));
    give(&mut env, &s)
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_fr_leslignesclaires_kybernow_core_Native_consoleLendCancelCode(mut env: Env, _: JClass, h: jlong) -> jstring {
    let Some(c) = console(h) else { return std::ptr::null_mut() };
    let s = envelope(c.lend_cancel_code());
    give(&mut env, &s)
}

/// « Accepter » (`accept`, avec `steam`) ou « Refuser » une demande.
#[unsafe(no_mangle)]
pub extern "system" fn Java_fr_leslignesclaires_kybernow_core_Native_consoleLendAnswer(mut env: Env, _: JClass, h: jlong, request: JString, accept: jboolean, steam: jint) -> jstring {
    let Some(c) = console(h) else { return std::ptr::null_mut() };
    let request = string(&mut env, &request);
    let s = envelope(c.lend_answer(&request, accept != 0, LendSteam::from_code(steam)));
    give(&mut env, &s)
}

/// « Reprendre l'ordinateur » (préavis de 2 min) ou, `now`, tout de suite.
#[unsafe(no_mangle)]
pub extern "system" fn Java_fr_leslignesclaires_kybernow_core_Native_consoleLendTakeBack(mut env: Env, _: JClass, h: jlong, now: jboolean) -> jstring {
    let Some(c) = console(h) else { return std::ptr::null_mut() };
    let s = envelope(c.lend_take_back(now != 0));
    give(&mut env, &s)
}
