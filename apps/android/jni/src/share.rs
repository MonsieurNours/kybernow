//! KyberNow Share pour l'app Android (docs/kybernow-share.md) : les mêmes
//! fonctions que le pont C des apps Apple (`apps/apple/src/share.rs`), pour
//! `fr.leslignesclaires.kybernow.core.Native`. Réponses en JSON comme les
//! autres appels d'Admin : `{"code": 0, "value": …}` ou `{"code": …,
//! "error": "…"}` ; « Rejoindre » ajoute ses refus : 20 aucun partage avec ce
//! code, 21 complet, 22 trop d'essais, 23 format de l'image illisible.

use jni::objects::{JClass, JString};
use jni::sys::{jboolean, jint, jlong, jstring};

use kybernow_core::console::Info;
use kybernow_core::i18n;
use kybernow_core::profile::Capabilities;
use kybernow_core::share::{self, Found, JoinRequest, ShareAvailability, ShareError, ShareRole, ShareTarget};

use crate::exports::{console, envelope, give, optional, string, Env};

fn share_envelope(r: Result<Found, ShareError>) -> String {
    let e = match r {
        Ok(found) => return serde_json::json!({ "code": 0, "value": found }).to_string(),
        Err(ShareError::Other(e)) => return envelope::<()>(Err(e)),
        Err(e) => e,
    };
    let code = match &e {
        ShareError::NoShare => 20,
        ShareError::Full => 21,
        ShareError::TooManyTries { .. } => 22,
        ShareError::Unreadable { .. } => 23,
        ShareError::Other(_) => 1,
    };
    serde_json::json!({ "code": code, "error": e.to_string(), "kind": e.kind() }).to_string()
}

fn bad(e: impl std::fmt::Display) -> String {
    serde_json::json!({ "code": 1, "error": e.to_string() }).to_string()
}

/// « Rejoindre » (comme `kn_share_join`) : `targets` `[{host, port,
/// fingerprint}]`, `request` `{code, name, device, app}`, `capabilities`
/// `{hevc, hevc444, av1}` ou nul. `value` : `{target, joined}`. Bloquant
/// (6 s au plus), hors du fil principal.
#[unsafe(no_mangle)]
pub extern "system" fn Java_fr_leslignesclaires_kybernow_core_Native_shareJoin(
    mut env: Env,
    _: JClass,
    targets: JString,
    request: JString,
    capabilities: JString,
) -> jstring {
    let (targets, request, caps) = (string(&mut env, &targets), string(&mut env, &request), optional(&mut env, &capabilities));
    let s = match (serde_json::from_str::<Vec<ShareTarget>>(&targets), serde_json::from_str::<JoinRequest>(&request)) {
        (Ok(t), Ok(r)) => match caps.and_then(|c| serde_json::from_str::<Capabilities>(&c).ok()) {
            Some(caps) => share_envelope(share::join_checked(&t, &r, caps)),
            None => share_envelope(share::join(&t, &r)),
        },
        (Err(e), _) | (_, Err(e)) => bad(e),
    };
    give(&mut env, &s)
}

/// L'invité repart avant d'avoir ouvert sa session ; `found` : la `value` de `shareJoin`.
#[unsafe(no_mangle)]
pub extern "system" fn Java_fr_leslignesclaires_kybernow_core_Native_shareLeave(mut env: Env, _: JClass, found: JString) -> jint {
    match serde_json::from_str::<Found>(&string(&mut env, &found)) {
        Ok(f) => {
            share::leave(&f);
            0
        }
        Err(_) => 1,
    }
}

/// Lien de partage lu (`{host, port, fingerprint, code}`), nul sinon.
#[unsafe(no_mangle)]
pub extern "system" fn Java_fr_leslignesclaires_kybernow_core_Native_shareParseLink(mut env: Env, _: JClass, url: JString) -> jstring {
    match share::parse_link(&string(&mut env, &url)) {
        Some(link) => give(&mut env, &serde_json::to_string(&link).unwrap_or_default()),
        None => std::ptr::null_mut(),
    }
}

/// Les six chiffres d'un code tapé, nul sinon.
#[unsafe(no_mangle)]
pub extern "system" fn Java_fr_leslignesclaires_kybernow_core_Native_shareNormalizeCode(mut env: Env, _: JClass, code: JString) -> jstring {
    match share::normalize_code(&string(&mut env, &code)) {
        Some(c) => give(&mut env, &c),
        None => std::ptr::null_mut(),
    }
}

/// Le code à afficher : « 123 456 ».
#[unsafe(no_mangle)]
pub extern "system" fn Java_fr_leslignesclaires_kybernow_core_Native_shareFormatCode(mut env: Env, _: JClass, code: JString) -> jstring {
    let s = share::format_code(&string(&mut env, &code));
    give(&mut env, &s)
}

/// « Partager » avec cet ordinateur d'après l'info (JSON de `consoleInfo`) :
/// 0 Station trop ancienne, 1 partage coupé, 2 disponible.
#[unsafe(no_mangle)]
pub extern "system" fn Java_fr_leslignesclaires_kybernow_core_Native_shareAvailability(mut env: Env, _: JClass, info: JString) -> jint {
    let info: Info = serde_json::from_str(&string(&mut env, &info)).unwrap_or_default();
    ShareAvailability::of(&info).code()
}

/// Le texte de `shareAvailability` (langue de l'app), nul si disponible.
#[unsafe(no_mangle)]
pub extern "system" fn Java_fr_leslignesclaires_kybernow_core_Native_shareAvailabilityText(mut env: Env, _: JClass, availability: jint) -> jstring {
    let a = match availability {
        0 => ShareAvailability::Unsupported,
        1 => ShareAvailability::Disabled,
        _ => return std::ptr::null_mut(),
    };
    match a.text(i18n::current()) {
        Some(t) => give(&mut env, &t),
        None => std::ptr::null_mut(),
    }
}

/// Le rôle avec les manettes données ou reprises, le clavier et la souris
/// tels quels (codes de `consoleShareGuest` ; −1 pour un code inconnu).
#[unsafe(no_mangle)]
pub extern "system" fn Java_fr_leslignesclaires_kybernow_core_Native_shareRoleWithPads(_: Env, _: JClass, role: jint, pads: jboolean) -> jint {
    ShareRole::from_code(role).map_or(-1, |r| r.with_pads(pads != 0).code())
}

/// Le rôle avec le clavier et la souris donnés ou repris, les manettes telles quelles.
#[unsafe(no_mangle)]
pub extern "system" fn Java_fr_leslignesclaires_kybernow_core_Native_shareRoleWithKbm(_: Env, _: JClass, role: jint, kbm: jboolean) -> jint {
    ShareRole::from_code(role).map_or(-1, |r| r.with_kbm(kbm != 0).code())
}

// MARK: - Hôte (console de l'ordinateur, compte de l'appareil en session)

#[unsafe(no_mangle)]
pub extern "system" fn Java_fr_leslignesclaires_kybernow_core_Native_consoleShareOpen(mut env: Env, _: JClass, h: jlong) -> jstring {
    let Some(c) = console(h) else { return std::ptr::null_mut() };
    let s = envelope(c.share_open());
    give(&mut env, &s)
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_fr_leslignesclaires_kybernow_core_Native_consoleShareState(mut env: Env, _: JClass, h: jlong) -> jstring {
    let Some(c) = console(h) else { return std::ptr::null_mut() };
    let s = envelope(c.share_state());
    give(&mut env, &s)
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_fr_leslignesclaires_kybernow_core_Native_consoleShareCode(mut env: Env, _: JClass, h: jlong) -> jstring {
    let Some(c) = console(h) else { return std::ptr::null_mut() };
    let s = envelope(c.share_code());
    give(&mut env, &s)
}

/// `role` : 0 spectateur, 1 manettes, 2 manettes avec clavier et souris, 3
/// clavier et souris seuls.
#[unsafe(no_mangle)]
pub extern "system" fn Java_fr_leslignesclaires_kybernow_core_Native_consoleShareGuest(mut env: Env, _: JClass, h: jlong, guest: JString, role: jint) -> jstring {
    let Some(c) = console(h) else { return std::ptr::null_mut() };
    let guest = string(&mut env, &guest);
    let Some(role) = ShareRole::from_code(role) else { return give(&mut env, &bad("rôle inconnu")) };
    let s = envelope(c.share_guest(&guest, role));
    give(&mut env, &s)
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_fr_leslignesclaires_kybernow_core_Native_consoleShareKick(mut env: Env, _: JClass, h: jlong, guest: JString) -> jstring {
    let Some(c) = console(h) else { return std::ptr::null_mut() };
    let guest = string(&mut env, &guest);
    let s = envelope(c.share_kick(&guest));
    give(&mut env, &s)
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_fr_leslignesclaires_kybernow_core_Native_consoleShareClose(mut env: Env, _: JClass, h: jlong) -> jstring {
    let Some(c) = console(h) else { return std::ptr::null_mut() };
    let s = envelope(c.share_close());
    give(&mut env, &s)
}
