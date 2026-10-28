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

use kybernow_core::screens::DeviceScreen;
use kybernow_core::share::{DecoderCaps, ShareDisplay, ShareImage, ShareQuality};

use crate::exports::{console, envelope, give, handle, optional, string, Env};

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
        ShareError::Refused => 24,
        ShareError::Expired => 25,
        ShareError::PendingFull => 26,
        ShareError::Stopped => 28,
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

fn joining_envelope(r: Result<share::Joining, ShareError>) -> String {
    match r {
        Ok(j) => serde_json::json!({ "code": 0, "value": j }).to_string(),
        Err(e) => share_envelope(Err(e)),
    }
}

/// « Rejoindre » sur le réseau et hors de chez soi (comme
/// `kn_share_join_anywhere`) : `relays` `["relais.exemple.org:9443", …]` (le
/// relais public est toujours interrogé en plus).
/// `value` : `{"joined": {target, joined}}` ou `{"pending": {…}}` ; refus en
/// plus : 24 refusé, 25 sans réponse, 26 trop de demandes ; 30, 31 relais.
#[unsafe(no_mangle)]
pub extern "system" fn Java_fr_leslignesclaires_kybernow_core_Native_shareJoinAnywhere(
    mut env: Env,
    _: JClass,
    targets: JString,
    relays: JString,
    request: JString,
    capabilities: JString,
) -> jstring {
    let (targets, relays, request, caps) = (string(&mut env, &targets), string(&mut env, &relays), string(&mut env, &request), optional(&mut env, &capabilities));
    let targets: Vec<ShareTarget> = serde_json::from_str(&targets).unwrap_or_default();
    let relays: Vec<kybernow_core::relay::RelayAddr> = serde_json::from_str::<Vec<String>>(&relays).unwrap_or_default().iter().filter_map(|r| r.parse().ok()).collect();
    let caps = caps.and_then(|c| serde_json::from_str::<Capabilities>(&c).ok());
    let s = match serde_json::from_str::<JoinRequest>(&request) {
        Ok(r) => joining_envelope(share::join_anywhere(&targets, &relays, &r, caps)),
        Err(e) => bad(e),
    };
    give(&mut env, &s)
}

/// Un lien de partage (`link` : celui de `shareParseLink`), réseau d'abord puis relais ; même `value`.
#[unsafe(no_mangle)]
pub extern "system" fn Java_fr_leslignesclaires_kybernow_core_Native_shareJoinLink(mut env: Env, _: JClass, link: JString, request: JString, capabilities: JString) -> jstring {
    let (link, request, caps) = (string(&mut env, &link), string(&mut env, &request), optional(&mut env, &capabilities));
    let caps = caps.and_then(|c| serde_json::from_str::<Capabilities>(&c).ok());
    let s = match (serde_json::from_str::<share::JoinLink>(&link), serde_json::from_str::<JoinRequest>(&request)) {
        (Ok(l), Ok(r)) => joining_envelope(share::join_link(&l, &r, caps)),
        (Err(e), _) => bad(e),
        (_, Err(e)) => bad(e),
    };
    give(&mut env, &s)
}

/// L'hôte a-t-il répondu (`pending` : la demande en attente) ? Tenu 25 s au
/// plus : `{"code": 0, "value": {target, joined}}` (accepté), `{"code": 27}`
/// (toujours en attente : rappeler), ou un refus (24 refusé, 25 sans réponse,
/// 28 partage arrêté par l'hôte, 20…). Bloquant.
#[unsafe(no_mangle)]
pub extern "system" fn Java_fr_leslignesclaires_kybernow_core_Native_shareWait(mut env: Env, _: JClass, pending: JString, capabilities: JString) -> jstring {
    let (pending, caps) = (string(&mut env, &pending), optional(&mut env, &capabilities));
    let caps = caps.and_then(|c| serde_json::from_str::<Capabilities>(&c).ok());
    let s = match serde_json::from_str::<share::PendingJoin>(&pending) {
        Ok(p) => match share::wait(&p, caps) {
            Ok(Some(found)) => serde_json::json!({ "code": 0, "value": found }).to_string(),
            Ok(None) => serde_json::json!({ "code": 27 }).to_string(),
            Err(e) => share_envelope(Err(e)),
        },
        Err(e) => bad(e),
    };
    give(&mut env, &s)
}

/// « Annuler » une demande en attente. Bloquant.
#[unsafe(no_mangle)]
pub extern "system" fn Java_fr_leslignesclaires_kybernow_core_Native_shareCancel(mut env: Env, _: JClass, pending: JString) -> jint {
    match serde_json::from_str::<share::PendingJoin>(&string(&mut env, &pending)) {
        Ok(p) => {
            share::cancel(&p);
            0
        }
        Err(_) => 1,
    }
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

// MARK: - Qualité de l'image d'un invité (0 Auto, 1 comme l'hôte, 2 1080p60, 3 720p30, 4 2160p60)

/// Le libellé d'une qualité dans la langue de l'app, nul pour un code inconnu.
#[unsafe(no_mangle)]
pub extern "system" fn Java_fr_leslignesclaires_kybernow_core_Native_shareQualityLabel(mut env: Env, _: JClass, quality: jint) -> jstring {
    match ShareQuality::from_code(quality) {
        Some(q) => give(&mut env, &q.label(i18n::current())),
        None => std::ptr::null_mut(),
    }
}

/// « Auto » (comme `kn_share_auto_quality`) : `screen` (`pixel_width`,
/// `pixel_height`, `refresh`), `decoder` (`hevc`, `hevc444`, `av1`,
/// `max_width`, `max_height`, `max_fps`), `host` (l'écran partagé) ou nul ;
/// rend `{width, height, fps, codecs, yuv444}` pour `auto` de `shareJoin`, nul
/// si illisible.
#[unsafe(no_mangle)]
pub extern "system" fn Java_fr_leslignesclaires_kybernow_core_Native_shareAutoQuality(mut env: Env, _: JClass, screen: JString, decoder: JString, host: JString) -> jstring {
    let (screen, decoder, host) = (string(&mut env, &screen), string(&mut env, &decoder), optional(&mut env, &host));
    let (Ok(screen), Ok(decoder)) = (serde_json::from_str::<DeviceScreen>(&screen), serde_json::from_str::<DecoderCaps>(&decoder)) else {
        return std::ptr::null_mut();
    };
    let host = host.and_then(|h| serde_json::from_str::<ShareDisplay>(&h).ok());
    let s = serde_json::to_string(&share::auto_quality(&screen, &decoder, host.as_ref())).unwrap_or_default();
    give(&mut env, &s)
}

/// La qualité reçue (« 1080p · 60 Hz ») d'une image (`onShareImage`, `guests[].image`), nul si illisible.
#[unsafe(no_mangle)]
pub extern "system" fn Java_fr_leslignesclaires_kybernow_core_Native_shareImageLabel(mut env: Env, _: JClass, image: JString) -> jstring {
    match serde_json::from_str::<ShareImage>(&string(&mut env, &image)) {
        Ok(i) => match i.label() {
            Some(l) => give(&mut env, &l),
            None => std::ptr::null_mut(),
        },
        Err(_) => std::ptr::null_mut(),
    }
}

/// Session d'invité : la « Qualité de l'image » choisie ; l'image reçue
/// arrive par `onShareImage`. Faux pour un code inconnu.
#[unsafe(no_mangle)]
pub extern "system" fn Java_fr_leslignesclaires_kybernow_core_Native_sessionSetShareQuality(_: Env, _: JClass, h: jlong, quality: jint) -> jboolean {
    let (Some(h), Some(q)) = (handle(h), ShareQuality::from_code(quality)) else { return 0 };
    h.session.set_share_quality(q);
    1
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

/// « Accepter » la demande d'un invité venu par le relais (`guest` : son identifiant dans la liste) ; `value` :
/// le partage à jour. Admin refuse (409) quand les quatre places sont prises.
#[unsafe(no_mangle)]
pub extern "system" fn Java_fr_leslignesclaires_kybernow_core_Native_consoleShareAccept(mut env: Env, _: JClass, h: jlong, guest: JString) -> jstring {
    let Some(c) = console(h) else { return std::ptr::null_mut() };
    let guest = string(&mut env, &guest);
    let s = envelope(c.share_accept(&guest));
    give(&mut env, &s)
}

/// « Refuser » la demande d'un invité venu par le relais ; `value` : le partage à jour.
#[unsafe(no_mangle)]
pub extern "system" fn Java_fr_leslignesclaires_kybernow_core_Native_consoleShareRefuse(mut env: Env, _: JClass, h: jlong, guest: JString) -> jstring {
    let Some(c) = console(h) else { return std::ptr::null_mut() };
    let guest = string(&mut env, &guest);
    let s = envelope(c.share_refuse(&guest));
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
