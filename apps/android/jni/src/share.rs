//! KyberNow Share pour l'app Android (docs/kybernow-share.md) : les mêmes
//! fonctions que le pont C des apps Apple (`apps/apple/src/share.rs`), pour
//! `fr.leslignesclaires.kybernow.core.Native`. Réponses en JSON comme les
//! autres appels d'Admin : `{"code": 0, "value": …}` ou `{"code": …,
//! "error": "…"}` ; « Rejoindre » ajoute ses refus : 20 aucun partage avec ce
//! code, 21 complet, 22 trop d'essais, 23 format de l'image illisible ; 29 un
//! code de prêt, 33 un code d'invitation (`value` : de quoi emprunter, ou
//! ajouter l'ordinateur avec `inviteRedeem`). « Regarder » (`shareWatch`,
//! `consoleShareWatch`) : 34 Station qui ne sait pas encore regarder, 35
//! ordinateur prêté, 36 partage coupé dans Admin, 21 complet ; les autres
//! refus avec le texte d'Admin.

use jni::objects::{JClass, JString};
use jni::sys::{jboolean, jint, jlong, jstring};

use kybernow_core::console::Info;
use kybernow_core::i18n;
use kybernow_core::profile::Capabilities;
use kybernow_core::share::{self, Found, JoinRequest, ShareAvailability, ShareError, ShareRole, ShareTarget};

use kybernow_core::screens::DeviceScreen;
use kybernow_core::share::{DecoderCaps, ShareDisplay, ShareImage, ShareQuality, WatchRequest, WatchScreen};

use crate::exports::{console, envelope, give, handle, optional, string, Env};

fn share_envelope(r: Result<Found, ShareError>) -> String {
    let e = match r {
        Ok(found) => return serde_json::json!({ "code": 0, "value": found }).to_string(),
        Err(ShareError::Other(e)) => return envelope::<()>(Err(e)),
        // Un code de prêt : `value` dit à qui l'emprunter (`lendBorrow`).
        Err(ShareError::Lend(code)) => {
            let text = ShareError::Lend(code.clone()).to_string();
            return serde_json::json!({ "code": 29, "error": text, "kind": "lend", "value": code }).to_string();
        }
        // Un code d'invitation (docs/invitations.md) : `value` `{code, target,
        // route, computer}`, à donner à `inviteRedeem` avec le mot de passe.
        Err(ShareError::Invite(code)) => {
            let text = ShareError::Invite(code.clone()).to_string();
            return serde_json::json!({ "code": 33, "error": text, "kind": "invite", "value": code }).to_string();
        }
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
        ShareError::Lend(_) => 29,
        ShareError::Invite(_) => 33,
        ShareError::WatchUnsupported => 34,
        ShareError::Lent { .. } => 35,
        ShareError::SharingOff => 36,
        ShareError::Unavailable => 37,
        ShareError::Other(_) => 1,
    };
    serde_json::json!({ "code": code, "error": e.to_string(), "kind": e.kind() }).to_string()
}

fn bad(e: impl std::fmt::Display) -> String {
    serde_json::json!({ "code": 1, "error": e.to_string() }).to_string()
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
/// plus : 24 refusé, 25 sans réponse, 26 trop de demandes ; 30, 31 relais ;
/// 29 code de prêt, 33 code d'invitation (`value` : `{code, target, route,
/// computer}`).
#[unsafe(no_mangle)]
pub extern "system" fn Java_fr_leslignesclaires_kybernow_core_Native_shareJoinAnywhere(
    mut env: Env,
    _: JClass,
    targets: JString,
    relays: JString,
    request: JString,
    capabilities: JString,
) -> jstring {
    kybernow_core::ffi_guard!(null => {
        let (targets, relays, request, caps) = (string(&mut env, &targets), string(&mut env, &relays), string(&mut env, &request), optional(&mut env, &capabilities));
        let targets: Vec<ShareTarget> = serde_json::from_str(&targets).unwrap_or_default();
        let relays: Vec<kybernow_core::relay::RelayAddr> = serde_json::from_str::<Vec<String>>(&relays).unwrap_or_default().iter().filter_map(|r| r.parse().ok()).collect();
        let caps = caps.and_then(|c| serde_json::from_str::<Capabilities>(&c).ok());
        let s = match serde_json::from_str::<JoinRequest>(&request) {
            Ok(r) => joining_envelope(share::join_anywhere(&targets, &relays, &r, caps)),
            Err(e) => bad(e),
        };
        give(&mut env, &s)
    })
}

/// Un lien de partage (`link` : celui de `shareParseLink`), réseau d'abord puis relais ; même `value`.
#[unsafe(no_mangle)]
pub extern "system" fn Java_fr_leslignesclaires_kybernow_core_Native_shareJoinLink(mut env: Env, _: JClass, link: JString, request: JString, capabilities: JString) -> jstring {
    kybernow_core::ffi_guard!(null => {
        let (link, request, caps) = (string(&mut env, &link), string(&mut env, &request), optional(&mut env, &capabilities));
        let caps = caps.and_then(|c| serde_json::from_str::<Capabilities>(&c).ok());
        let s = match (serde_json::from_str::<share::JoinLink>(&link), serde_json::from_str::<JoinRequest>(&request)) {
            (Ok(l), Ok(r)) => joining_envelope(share::join_link(&l, &r, caps)),
            (Err(e), _) => bad(e),
            (_, Err(e)) => bad(e),
        };
        give(&mut env, &s)
    })
}

/// L'hôte a-t-il répondu (`pending` : la demande en attente) ? Tenu 25 s au
/// plus : `{"code": 0, "value": {target, joined}}` (accepté), `{"code": 27}`
/// (toujours en attente : rappeler), ou un refus (24 refusé, 25 sans réponse,
/// 28 partage arrêté par l'hôte, 20…). Bloquant.
#[unsafe(no_mangle)]
pub extern "system" fn Java_fr_leslignesclaires_kybernow_core_Native_shareWait(mut env: Env, _: JClass, pending: JString, capabilities: JString) -> jstring {
    kybernow_core::ffi_guard!(null => {
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
    })
}

/// « Annuler » une demande en attente. Bloquant.
#[unsafe(no_mangle)]
pub extern "system" fn Java_fr_leslignesclaires_kybernow_core_Native_shareCancel(mut env: Env, _: JClass, pending: JString) -> jint {
    kybernow_core::ffi_guard!(1 => {
        match serde_json::from_str::<share::PendingJoin>(&string(&mut env, &pending)) {
            Ok(p) => {
                share::cancel(&p);
                0
            }
            Err(_) => 1,
        }
    })
}

/// L'invité repart avant d'avoir ouvert sa session ; `found` : la `value` de `shareJoin`.
#[unsafe(no_mangle)]
pub extern "system" fn Java_fr_leslignesclaires_kybernow_core_Native_shareLeave(mut env: Env, _: JClass, found: JString) -> jint {
    kybernow_core::ffi_guard!(1 => {
        match serde_json::from_str::<Found>(&string(&mut env, &found)) {
            Ok(f) => {
                share::leave(&f);
                0
            }
            Err(_) => 1,
        }
    })
}

/// Lien de partage lu (`{host, port, fingerprint, code}`), nul sinon.
#[unsafe(no_mangle)]
pub extern "system" fn Java_fr_leslignesclaires_kybernow_core_Native_shareParseLink(mut env: Env, _: JClass, url: JString) -> jstring {
    kybernow_core::ffi_guard!(null => {
        match share::parse_link(&string(&mut env, &url)) {
            Some(link) => give(&mut env, &serde_json::to_string(&link).unwrap_or_default()),
            None => std::ptr::null_mut(),
        }
    })
}

/// Le code à afficher : « 123 456 ».
#[unsafe(no_mangle)]
pub extern "system" fn Java_fr_leslignesclaires_kybernow_core_Native_shareFormatCode(mut env: Env, _: JClass, code: JString) -> jstring {
    kybernow_core::ffi_guard!(null => {
        let s = share::format_code(&string(&mut env, &code));
        give(&mut env, &s)
    })
}

/// « Partager » avec cet ordinateur d'après l'info (JSON de `consoleInfo`) :
/// 0 Station trop ancienne, 1 partage coupé, 2 disponible.
#[unsafe(no_mangle)]
pub extern "system" fn Java_fr_leslignesclaires_kybernow_core_Native_shareAvailability(mut env: Env, _: JClass, info: JString) -> jint {
    kybernow_core::ffi_guard!({
        let info: Info = serde_json::from_str(&string(&mut env, &info)).unwrap_or_default();
        ShareAvailability::of(&info).code()
    })
}

/// Le texte de `shareAvailability` (langue de l'app), nul si disponible.
#[unsafe(no_mangle)]
pub extern "system" fn Java_fr_leslignesclaires_kybernow_core_Native_shareAvailabilityText(mut env: Env, _: JClass, availability: jint) -> jstring {
    kybernow_core::ffi_guard!(null => {
        let a = match availability {
            0 => ShareAvailability::Unsupported,
            1 => ShareAvailability::Disabled,
            _ => return std::ptr::null_mut(),
        };
        match a.text(i18n::current()) {
            Some(t) => give(&mut env, &t),
            None => std::ptr::null_mut(),
        }
    })
}

// MARK: - Qualité de l'image d'un invité (0 Auto, 1 comme l'hôte, 2 1080p60, 3 720p30, 4 2160p60)

/// « Auto » (comme `kn_share_auto_quality`) : `screen` (`pixel_width`,
/// `pixel_height`, `refresh`), `decoder` (`hevc`, `hevc444`, `av1`,
/// `max_width`, `max_height`, `max_fps`), `host` (l'écran partagé) ou nul ;
/// rend `{width, height, fps, codecs, yuv444}` pour `auto` de `shareJoin`, nul
/// si illisible.
#[unsafe(no_mangle)]
pub extern "system" fn Java_fr_leslignesclaires_kybernow_core_Native_shareAutoQuality(mut env: Env, _: JClass, screen: JString, decoder: JString, host: JString) -> jstring {
    kybernow_core::ffi_guard!(null => {
        let (screen, decoder, host) = (string(&mut env, &screen), string(&mut env, &decoder), optional(&mut env, &host));
        let (Ok(screen), Ok(decoder)) = (serde_json::from_str::<DeviceScreen>(&screen), serde_json::from_str::<DecoderCaps>(&decoder)) else {
            return std::ptr::null_mut();
        };
        let host = host.and_then(|h| serde_json::from_str::<ShareDisplay>(&h).ok());
        let s = serde_json::to_string(&share::auto_quality(&screen, &decoder, host.as_ref())).unwrap_or_default();
        give(&mut env, &s)
    })
}

/// La qualité reçue (« 1080p · 60 Hz ») d'une image (`onShareImage`, `guests[].image`), nul si illisible.
#[unsafe(no_mangle)]
pub extern "system" fn Java_fr_leslignesclaires_kybernow_core_Native_shareImageLabel(mut env: Env, _: JClass, image: JString) -> jstring {
    kybernow_core::ffi_guard!(null => {
        match serde_json::from_str::<ShareImage>(&string(&mut env, &image)) {
            Ok(i) => match i.label() {
                Some(l) => give(&mut env, &l),
                None => std::ptr::null_mut(),
            },
            Err(_) => std::ptr::null_mut(),
        }
    })
}

/// Session d'invité : la « Qualité de l'image » choisie ; l'image reçue
/// arrive par `onShareImage`. Faux pour un code inconnu.
#[unsafe(no_mangle)]
pub extern "system" fn Java_fr_leslignesclaires_kybernow_core_Native_sessionSetShareQuality(_: Env, _: JClass, h: jlong, quality: jint) -> jboolean {
    kybernow_core::ffi_guard!({
        let (Some(h), Some(q)) = (handle(h), ShareQuality::from_code(quality)) else { return 0 };
        h.session.set_share_quality(q);
        1
    })
}

// MARK: - Hôte (console de l'ordinateur, compte de l'appareil en session)

#[unsafe(no_mangle)]
pub extern "system" fn Java_fr_leslignesclaires_kybernow_core_Native_consoleShareOpen(mut env: Env, _: JClass, h: jlong) -> jstring {
    kybernow_core::ffi_guard!(null => {
        let Some(c) = console(h) else { return std::ptr::null_mut() };
        let s = envelope(c.share_open());
        give(&mut env, &s)
    })
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_fr_leslignesclaires_kybernow_core_Native_consoleShareState(mut env: Env, _: JClass, h: jlong) -> jstring {
    kybernow_core::ffi_guard!(null => {
        let Some(c) = console(h) else { return std::ptr::null_mut() };
        let s = envelope(c.share_state());
        give(&mut env, &s)
    })
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_fr_leslignesclaires_kybernow_core_Native_consoleShareCode(mut env: Env, _: JClass, h: jlong) -> jstring {
    kybernow_core::ffi_guard!(null => {
        let Some(c) = console(h) else { return std::ptr::null_mut() };
        let s = envelope(c.share_code());
        give(&mut env, &s)
    })
}

/// « Laisser les invités parler » (docs/micro.md) : `value`, le partage à
/// jour (`guests_speak`) ; vaut tout de suite pour les invités déjà là.
#[unsafe(no_mangle)]
pub extern "system" fn Java_fr_leslignesclaires_kybernow_core_Native_consoleShareSpeak(mut env: Env, _: JClass, h: jlong, on: jboolean) -> jstring {
    kybernow_core::ffi_guard!(null => {
        let Some(c) = console(h) else { return std::ptr::null_mut() };
        let s = envelope(c.share_speak(on != 0));
        give(&mut env, &s)
    })
}

/// `role` : 0 spectateur, 1 manettes, 2 manettes avec clavier et souris, 3
/// clavier et souris seuls.
#[unsafe(no_mangle)]
pub extern "system" fn Java_fr_leslignesclaires_kybernow_core_Native_consoleShareGuest(mut env: Env, _: JClass, h: jlong, guest: JString, role: jint) -> jstring {
    kybernow_core::ffi_guard!(null => {
        let Some(c) = console(h) else { return std::ptr::null_mut() };
        let guest = string(&mut env, &guest);
        let Some(role) = ShareRole::from_code(role) else { return give(&mut env, &bad("rôle inconnu")) };
        let s = envelope(c.share_guest(&guest, role));
        give(&mut env, &s)
    })
}

/// « Accepter » la demande d'un invité venu par le relais (`guest` : son identifiant dans la liste) ; `value` :
/// le partage à jour. Admin refuse (409) quand les quatre places sont prises.
#[unsafe(no_mangle)]
pub extern "system" fn Java_fr_leslignesclaires_kybernow_core_Native_consoleShareAccept(mut env: Env, _: JClass, h: jlong, guest: JString) -> jstring {
    kybernow_core::ffi_guard!(null => {
        let Some(c) = console(h) else { return std::ptr::null_mut() };
        let guest = string(&mut env, &guest);
        let s = envelope(c.share_accept(&guest));
        give(&mut env, &s)
    })
}

/// « Refuser » la demande d'un invité venu par le relais ; `value` : le partage à jour.
#[unsafe(no_mangle)]
pub extern "system" fn Java_fr_leslignesclaires_kybernow_core_Native_consoleShareRefuse(mut env: Env, _: JClass, h: jlong, guest: JString) -> jstring {
    kybernow_core::ffi_guard!(null => {
        let Some(c) = console(h) else { return std::ptr::null_mut() };
        let guest = string(&mut env, &guest);
        let s = envelope(c.share_refuse(&guest));
        give(&mut env, &s)
    })
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_fr_leslignesclaires_kybernow_core_Native_consoleShareKick(mut env: Env, _: JClass, h: jlong, guest: JString) -> jstring {
    kybernow_core::ffi_guard!(null => {
        let Some(c) = console(h) else { return std::ptr::null_mut() };
        let guest = string(&mut env, &guest);
        let s = envelope(c.share_kick(&guest));
        give(&mut env, &s)
    })
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_fr_leslignesclaires_kybernow_core_Native_consoleShareClose(mut env: Env, _: JClass, h: jlong) -> jstring {
    kybernow_core::ffi_guard!(null => {
        let Some(c) = console(h) else { return std::ptr::null_mut() };
        let s = envelope(c.share_close());
        give(&mut env, &s)
    })
}

// MARK: - Regarder (droit « Regarder » d'un compte KyberNow, docs/comptes.md)

/// « Regarder » un ordinateur du compte (comme `kn_share_watch`) : `access`
/// `{account, station}`, `lan` `{host, port, fingerprint}` ou nul, `route`
/// (`accountRoute`) ou nul, `request` `{app, display?, quality, auto?}`,
/// `capabilities` ou nul. `value` : `{target, joined}` (avec `joined.watch` :
/// `{screens, switch, display}`). Bloquant (12 s au plus), hors du fil principal.
#[unsafe(no_mangle)]
pub extern "system" fn Java_fr_leslignesclaires_kybernow_core_Native_shareWatch(
    mut env: Env,
    _: JClass,
    access: JString,
    lan: JString,
    route: JString,
    request: JString,
    capabilities: JString,
) -> jstring {
    kybernow_core::ffi_guard!(null => {
        let (access, request) = (string(&mut env, &access), string(&mut env, &request));
        let (lan, route, caps) = (optional(&mut env, &lan), optional(&mut env, &route), optional(&mut env, &capabilities));
        let s = match (serde_json::from_str::<kybernow_core::account::AccessParams>(&access), serde_json::from_str::<WatchRequest>(&request)) {
            (Ok(params), Ok(req)) => {
                let lan = lan.and_then(|l| serde_json::from_str::<ShareTarget>(&l).ok());
                let route = route.and_then(|r| serde_json::from_str::<kybernow_core::relay::RelayRoute>(&r).ok());
                let caps = caps.and_then(|c| serde_json::from_str::<Capabilities>(&c).ok());
                share_envelope(share::watch_account(kybernow_core::account::StationAccess::new(params), lan, route, &req, caps))
            }
            (Err(e), _) => bad(e),
            (_, Err(e)) => bad(e),
        };
        give(&mut env, &s)
    })
}

/// « Regarder {nom} » (titre des étapes de la connexion).
#[unsafe(no_mangle)]
pub extern "system" fn Java_fr_leslignesclaires_kybernow_core_Native_shareWatchTitle(mut env: Env, _: JClass, computer: JString) -> jstring {
    kybernow_core::ffi_guard!(null => {
        let computer = string(&mut env, &computer);
        give(&mut env, &share::watch_title(i18n::current(), &computer))
    })
}

/// `action` « Regarder », `stop` « Arrêter de regarder », `screen` « Écran ».
#[unsafe(no_mangle)]
pub extern "system" fn Java_fr_leslignesclaires_kybernow_core_Native_shareWatchWord(mut env: Env, _: JClass, word: JString) -> jstring {
    kybernow_core::ffi_guard!(null => {
        let word = string(&mut env, &word);
        give(&mut env, &share::watch_word(i18n::current(), &word))
    })
}

/// Une ligne du menu « Écran » (« Écran 1 — 3840 × 2160 ») ; nul si illisible.
#[unsafe(no_mangle)]
pub extern "system" fn Java_fr_leslignesclaires_kybernow_core_Native_shareWatchScreenLabel(mut env: Env, _: JClass, screen: JString) -> jstring {
    kybernow_core::ffi_guard!(null => {
        match serde_json::from_str::<WatchScreen>(&string(&mut env, &screen)) {
            Ok(s) => give(&mut env, &s.label(i18n::current())),
            Err(_) => std::ptr::null_mut(),
        }
    })
}

/// Session de qui regarde : « Écran » choisi (`display` : un `id` de `joined.watch.screens`).
#[unsafe(no_mangle)]
pub extern "system" fn Java_fr_leslignesclaires_kybernow_core_Native_sessionWatchScreen(_: Env, _: JClass, h: jlong, display: jlong) {
    kybernow_core::ffi_guard!({
        if let Some(h) = handle(h) {
            h.session.watch_screen(display as u32);
        }
    })
}

/// Session de qui regarde : l'écran regardé (0 hors d'une telle session).
#[unsafe(no_mangle)]
pub extern "system" fn Java_fr_leslignesclaires_kybernow_core_Native_sessionWatchDisplay(_: Env, _: JClass, h: jlong) -> jlong {
    kybernow_core::ffi_guard!({
        handle(h).and_then(|h| h.session.watch_display()).map_or(0, jlong::from)
    })
}
