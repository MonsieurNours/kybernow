//! KyberNow Share pour les apps Apple (docs/kybernow-share.md, textes dans
//! docs/link-ui.md, « Partage ») : rejoindre un partage (code envoyé aux
//! Stations du réseau, lien), et, côté hôte, ouvrir le partage, changer le
//! code, le rôle d'un invité, le retirer, arrêter. La session d'invité
//! s'ouvre ensuite par `kn_session_start` avec `guest` (voir lib.rs).

use std::ffi::c_char;

use kybernow_core::console::Info;
use kybernow_core::i18n;
use kybernow_core::profile::Capabilities;
use kybernow_core::screens::DeviceScreen;
use kybernow_core::share::{self, DecoderCaps, Found, JoinRequest, ShareAvailability, ShareDisplay, ShareError, ShareImage, ShareQuality, ShareRole, ShareTarget};

use crate::{KN_ERROR, KN_OK, KnConsole, console_result, failure, give, give_to, optional, text};

/// Refus de « Rejoindre » (`kn_share_join`), en plus des codes des autres
/// appels : aucun partage avec ce code sur le réseau, partage complet (quatre
/// invités au plus), trop d'essais (10 par minute et par adresse), format de
/// l'image de l'hôte que l'appareil ne lit pas. `error` : le texte à montrer.
pub const KN_SHARE_NO_SHARE: i32 = 20;
pub const KN_SHARE_FULL: i32 = 21;
pub const KN_SHARE_TOO_MANY: i32 = 22;
pub const KN_SHARE_UNREADABLE: i32 = 23;
/// Par le relais (docs/relais.md) : l'hôte a refusé (« L'hôte a refusé. »),
/// n'a pas répondu en 120 s (« L'hôte n'a pas répondu. »), trois demandes
/// attendent déjà ; `kn_share_wait` : toujours en attente (rappeler).
pub const KN_SHARE_REFUSED: i32 = 24;
pub const KN_SHARE_EXPIRED: i32 = 25;
pub const KN_SHARE_PENDING_FULL: i32 = 26;
pub const KN_SHARE_WAITING: i32 = 27;
/// `kn_share_wait` : l'hôte a arrêté le partage pendant l'attente
/// (« Ce partage est arrêté. »).
pub const KN_SHARE_STOPPED: i32 = 28;
/// « Rejoindre avec un code » : c'est un code de prêt de l'ordinateur
/// (docs/pret.md). `out` : `{code, target, route}`, à donner à
/// `kn_lend_borrow` (`target_json`, `route_json`, et `{"code"}` dans
/// `request_json`).
pub const KN_SHARE_LEND: i32 = 29;

/// Rôle d'un invité (`on_share_role`, `kn_console_share_guest`) : deux
/// droits indépendants, les manettes et le clavier et la souris. Aucun
/// (spectateur), les manettes (`PLAYER`), les deux (`PLAYER_KBM`), le
/// clavier et la souris seuls (`KBM`). « Joueur » dans la barre : un droit au
/// moins.
pub const KN_SHARE_ROLE_SPECTATOR: i32 = 0;
pub const KN_SHARE_ROLE_PLAYER: i32 = 1;
pub const KN_SHARE_ROLE_PLAYER_KBM: i32 = 2;
pub const KN_SHARE_ROLE_KBM: i32 = 3;

/// Fin d'une session d'invité par le partage (`on_share_end`).
pub const KN_SHARE_END_STOPPED: i32 = 0;
pub const KN_SHARE_END_REMOVED: i32 = 1;
pub const KN_SHARE_END_UNREADABLE: i32 = 2;

/// « Partager » avec cet ordinateur (`kn_share_availability`) : Station
/// trop ancienne, partage coupé sur l'ordinateur, disponible.
pub const KN_SHARE_UNSUPPORTED: i32 = 0;
pub const KN_SHARE_DISABLED: i32 = 1;
pub const KN_SHARE_AVAILABLE: i32 = 2;

fn share_failure(e: ShareError, out: *mut *mut c_char, error: *mut *mut c_char) -> i32 {
    let message = e.to_string();
    let code = match e {
        ShareError::Other(e) => return failure(e, error),
        // Un code de prêt : `out` dit à qui l'emprunter (`kn_lend_borrow`).
        ShareError::Lend(code) => {
            give_to(out, serde_json::to_string(&code).unwrap_or_default());
            KN_SHARE_LEND
        }
        ShareError::NoShare => KN_SHARE_NO_SHARE,
        ShareError::Full => KN_SHARE_FULL,
        ShareError::TooManyTries { .. } => KN_SHARE_TOO_MANY,
        ShareError::Unreadable { .. } => KN_SHARE_UNREADABLE,
        ShareError::Refused => KN_SHARE_REFUSED,
        ShareError::Expired => KN_SHARE_EXPIRED,
        ShareError::PendingFull => KN_SHARE_PENDING_FULL,
        ShareError::Stopped => KN_SHARE_STOPPED,
    };
    give_to(error, message);
    code
}

fn role(code: i32) -> Option<ShareRole> {
    match code {
        KN_SHARE_ROLE_SPECTATOR => Some(ShareRole::Spectator),
        KN_SHARE_ROLE_PLAYER => Some(ShareRole::Player),
        KN_SHARE_ROLE_PLAYER_KBM => Some(ShareRole::PlayerKbm),
        KN_SHARE_ROLE_KBM => Some(ShareRole::Kbm),
        _ => None,
    }
}

/// Le rôle avec les manettes données (`pads` vrai) ou reprises, le clavier et
/// la souris tels quels (`KN_SHARE_ROLE_*` ; −1 pour un code inconnu).
#[unsafe(no_mangle)]
pub extern "C" fn kn_share_role_with_pads(role_code: i32, pads: bool) -> i32 {
    role(role_code).map_or(-1, |r| r.with_pads(pads).code())
}

/// Le rôle avec le clavier et la souris donnés ou repris, les manettes telles quelles.
#[unsafe(no_mangle)]
pub extern "C" fn kn_share_role_with_kbm(role_code: i32, kbm: bool) -> i32 {
    role(role_code).map_or(-1, |r| r.with_kbm(kbm).code())
}

/// « Qualité de l'image » d'un invité (`kn_session_set_share_quality`,
/// `kn_share_quality_label`) ; dans `request_json` de `kn_share_join`, le
/// nom (`quality` : `auto`, `host`, `2160p60`, `1080p60`, `720p30`). Ordre
/// des menus : Auto, comme l'hôte, 4K · 60 Hz, 1080p, 720p (les codes ne le
/// suivent pas : 2160p60 est venu après).
pub const KN_SHARE_QUALITY_AUTO: i32 = 0;
pub const KN_SHARE_QUALITY_HOST: i32 = 1;
pub const KN_SHARE_QUALITY_1080P60: i32 = 2;
pub const KN_SHARE_QUALITY_720P30: i32 = 3;
pub const KN_SHARE_QUALITY_2160P60: i32 = 4;

// MARK: - Qualité de l'image

/// Le libellé d'une qualité (`KN_SHARE_QUALITY_*`) dans la langue de l'app :
/// « Auto », « Comme l'hôte », « 4K · 60 Hz », « 1080p · 60 Hz », « 720p · 30 Hz » ; nul
/// pour un code inconnu.
#[unsafe(no_mangle)]
pub extern "C" fn kn_share_quality_label(quality: i32) -> *mut c_char {
    ShareQuality::from_code(quality).map_or(std::ptr::null_mut(), |q| give(q.label(i18n::current())))
}

/// « Auto » : ce que l'appareil sait afficher et décoder, à mettre dans
/// `auto` de `request_json` (`kn_share_join`) avec `"quality": "auto"`.
/// `screen_json` : l'écran de l'appareil (celui de la fenêtre ; comme une
/// entrée de `screens.list` : `pixel_width`, `pixel_height`, `refresh`) ;
/// `decoder_json` : `{hevc, hevc444, av1, max_width, max_height, max_fps}`
/// (0 ou absent : pas de limite connue) ; `host_json` : l'écran partagé
/// (`display` de `joined`), ou nul. Rend `{width, height, fps, codecs,
/// yuv444}`, nul si un JSON est illisible.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_share_auto_quality(screen_json: *const c_char, decoder_json: *const c_char, host_json: *const c_char) -> *mut c_char {
    let Ok(screen) = serde_json::from_str::<DeviceScreen>(&text(screen_json)) else { return std::ptr::null_mut() };
    let Ok(decoder) = serde_json::from_str::<DecoderCaps>(&text(decoder_json)) else { return std::ptr::null_mut() };
    let host = optional(host_json).and_then(|h| serde_json::from_str::<ShareDisplay>(&h).ok());
    give(serde_json::to_string(&share::auto_quality(&screen, &decoder, host.as_ref())).unwrap_or_default())
}

/// La qualité reçue (`image_json` de `on_share_image`, ou `guests[].image`
/// chez l'hôte), telle que la barre et le panneau la montrent : « 1080p ·
/// 60 Hz » ; nul si illisible.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_share_image_label(image_json: *const c_char) -> *mut c_char {
    match serde_json::from_str::<ShareImage>(&text(image_json)) {
        Ok(i) => i.label().map_or(std::ptr::null_mut(), give),
        Err(_) => std::ptr::null_mut(),
    }
}

// MARK: - Rejoindre

/// « Rejoindre » : envoie le code à ces Stations à la fois et rend la
/// première qui l'accepte. `targets_json` : `[{"host", "port",
/// "fingerprint"}]` (les ordinateurs trouvés sur le réseau, comme pour
/// l'appairage, ou celui du lien ; `fingerprint` facultatif) ;
/// `request_json` : `{"code" (six chiffres), "name" (le prénom), "device"
/// (l'identifiant de l'appareil), "app" (mac, ios, tvos), "quality" (`auto`
/// par défaut, `host`, `1080p60`, `720p30`), "auto" (pour `auto` :
/// `kn_share_auto_quality`)}` ;
/// `capabilities_json` (`{hevc, hevc444, av1}`, nul : pas de vérification) :
/// un format de l'hôte que l'appareil ne lit pas n'empêche pas d'entrer
/// (Kyber lui fait une image à part dans un format qu'il lit) ; seul un
/// appareil qui ne lit aucun format possible repart (`KN_SHARE_UNREADABLE`).
/// `out` : `{"target": {host, port, fingerprint}, "joined": {…}}` : `target` donne
/// `console_host`, `console_port` et `console_fingerprint` de la session,
/// `joined` va tel quel dans `guest.joined` de `kn_session_start`. Bloquant
/// (6 s au plus).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_share_join(
    targets_json: *const c_char,
    request_json: *const c_char,
    capabilities_json: *const c_char,
    out: *mut *mut c_char,
    error: *mut *mut c_char,
) -> i32 {
    let targets: Vec<ShareTarget> = match serde_json::from_str(&text(targets_json)) {
        Ok(t) => t,
        Err(e) => {
            give_to(error, e.to_string());
            return KN_ERROR;
        }
    };
    let request: JoinRequest = match serde_json::from_str(&text(request_json)) {
        Ok(r) => r,
        Err(e) => {
            give_to(error, e.to_string());
            return KN_ERROR;
        }
    };
    let caps = optional(capabilities_json).and_then(|c| serde_json::from_str::<Capabilities>(&c).ok());
    let result = match caps {
        Some(caps) => share::join_checked(&targets, &request, caps),
        None => share::join(&targets, &request),
    };
    match result {
        Ok(found) => {
            give_to(out, serde_json::to_string(&found).unwrap_or_default());
            KN_OK
        }
        Err(e) => share_failure(e, out, error),
    }
}

/// « Rejoindre » sur le réseau **et hors de chez soi** (docs/relais.md) :
/// comme `kn_share_join`, plus `relays_json` (`["relais.exemple.org:9443",
/// …]` : ceux des ordinateurs connus, `info.relay.server` ; le relais public,
/// `share.kybernow.app`, est toujours interrogé en plus). Une Station du réseau qui répond en 1,5 s
/// est prise, sinon celle que donne un relais, par le tunnel. `out` :
/// `{"joined": {target, joined}}` (comme `kn_share_join`) ou `{"pending":
/// {route, guest, secret, expires}}` : « En attente de l'accord de l'hôte… »,
/// à donner à `kn_share_wait` (ou `kn_share_cancel`). Bloquant (8 s au plus).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_share_join_anywhere(
    targets_json: *const c_char,
    relays_json: *const c_char,
    request_json: *const c_char,
    capabilities_json: *const c_char,
    out: *mut *mut c_char,
    error: *mut *mut c_char,
) -> i32 {
    let targets: Vec<ShareTarget> = serde_json::from_str(&text(targets_json)).unwrap_or_default();
    let relays: Vec<kybernow_core::relay::RelayAddr> = serde_json::from_str::<Vec<String>>(&text(relays_json))
        .unwrap_or_default()
        .iter()
        .filter_map(|r| r.parse().ok())
        .collect();
    let request: JoinRequest = match serde_json::from_str(&text(request_json)) {
        Ok(r) => r,
        Err(e) => {
            give_to(error, e.to_string());
            return KN_ERROR;
        }
    };
    let caps = optional(capabilities_json).and_then(|c| serde_json::from_str::<Capabilities>(&c).ok());
    match share::join_anywhere(&targets, &relays, &request, caps) {
        Ok(joining) => {
            give_to(out, serde_json::to_string(&joining).unwrap_or_default());
            KN_OK
        }
        Err(e) => share_failure(e, out, error),
    }
}

/// Un lien de partage (`link_json` : celui de `kn_share_parse_link`) : la
/// Station du réseau d'abord (1,5 s), puis, avec `relay` et `sid`, par le
/// relais de bout en bout (Admin épinglé sur `fingerprint`). Même `out` que
/// `kn_share_join_anywhere`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_share_join_link(
    link_json: *const c_char,
    request_json: *const c_char,
    capabilities_json: *const c_char,
    out: *mut *mut c_char,
    error: *mut *mut c_char,
) -> i32 {
    let (Ok(link), Ok(request)) = (serde_json::from_str::<share::JoinLink>(&text(link_json)), serde_json::from_str::<JoinRequest>(&text(request_json))) else {
        return KN_ERROR;
    };
    let caps = optional(capabilities_json).and_then(|c| serde_json::from_str::<Capabilities>(&c).ok());
    match share::join_link(&link, &request, caps) {
        Ok(joining) => {
            give_to(out, serde_json::to_string(&joining).unwrap_or_default());
            KN_OK
        }
        Err(e) => share_failure(e, out, error),
    }
}

/// L'hôte a-t-il répondu à la demande (`pending_json` : `pending` de
/// `kn_share_join_anywhere`) ? Tenu 25 s au plus : `KN_OK` et `out` = `{target,
/// joined}` (accepté : `joined` pour `guest.joined`, la session passera par le
/// relais), `KN_SHARE_WAITING` (rappeler), `KN_SHARE_REFUSED`,
/// `KN_SHARE_EXPIRED`, `KN_SHARE_STOPPED` (l'hôte a arrêté le partage :
/// « Ce partage est arrêté. »), `KN_SHARE_NO_SHARE` (Station qui ne connaît
/// plus la demande). Bloquant.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_share_wait(pending_json: *const c_char, capabilities_json: *const c_char, out: *mut *mut c_char, error: *mut *mut c_char) -> i32 {
    let Ok(pending) = serde_json::from_str::<share::PendingJoin>(&text(pending_json)) else { return KN_ERROR };
    let caps = optional(capabilities_json).and_then(|c| serde_json::from_str::<Capabilities>(&c).ok());
    match share::wait(&pending, caps) {
        Ok(Some(found)) => {
            give_to(out, serde_json::to_string(&found).unwrap_or_default());
            KN_OK
        }
        Ok(None) => KN_SHARE_WAITING,
        Err(e) => share_failure(e, out, error),
    }
}

/// « Annuler » une demande en attente. Bloquant.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_share_cancel(pending_json: *const c_char) -> i32 {
    match serde_json::from_str::<share::PendingJoin>(&text(pending_json)) {
        Ok(p) => {
            share::cancel(&p);
            KN_OK
        }
        Err(_) => KN_ERROR,
    }
}

/// L'invité repart avant d'avoir ouvert sa session (l'utilisateur annule) ;
/// `found_json` : le `out` de `kn_share_join`. Une session d'invité, elle,
/// part seule. Bloquant.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_share_leave(found_json: *const c_char) -> i32 {
    match serde_json::from_str::<Found>(&text(found_json)) {
        Ok(found) => {
            share::leave(&found);
            KN_OK
        }
        Err(_) => KN_ERROR,
    }
}

/// Lit un lien de partage (`kybernow://join?host=…&port=…&fp=…&code=…`, ou
/// le lien du client web) : `{"host", "port", "fingerprint", "code"}`, nul
/// si ce n'en est pas un. `[{host, port, fingerprint}]` de ce JSON est la
/// liste de `kn_share_join`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_share_parse_link(url: *const c_char) -> *mut c_char {
    match share::parse_link(&text(url)) {
        Some(link) => give(serde_json::to_string(&link).unwrap_or_default()),
        None => std::ptr::null_mut(),
    }
}

/// Les six chiffres d'un code tapé (« 123 456 »), nul sinon.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_share_normalize_code(code: *const c_char) -> *mut c_char {
    match share::normalize_code(&text(code)) {
        Some(c) => give(c),
        None => std::ptr::null_mut(),
    }
}

/// Le code à afficher : « 123 456 ».
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_share_format_code(code: *const c_char) -> *mut c_char {
    give(share::format_code(&text(code)))
}

// MARK: - Hôte

/// « Partager » avec cet ordinateur, d'après `info_json` (`kn_console_info`) :
/// `KN_SHARE_UNSUPPORTED`, `KN_SHARE_DISABLED` ou `KN_SHARE_AVAILABLE`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_share_availability(info_json: *const c_char) -> i32 {
    let info: Info = serde_json::from_str(&text(info_json)).unwrap_or_default();
    ShareAvailability::of(&info).code()
}

/// Le texte de `kn_share_availability` dans la langue de l'app, nul quand
/// le partage est disponible.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_share_availability_text(availability: i32) -> *mut c_char {
    let a = match availability {
        KN_SHARE_UNSUPPORTED => ShareAvailability::Unsupported,
        KN_SHARE_DISABLED => ShareAvailability::Disabled,
        _ => return std::ptr::null_mut(),
    };
    a.text(i18n::current()).map(give).unwrap_or(std::ptr::null_mut())
}

/// Ouvre le partage de l'écran principal de la session de cet appareil :
/// `{"code", "link", "web", "display": {"id", "label"}}`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_console_share_open(c: *const KnConsole, out: *mut *mut c_char, error: *mut *mut c_char) -> i32 {
    let Some(c) = (unsafe { c.as_ref() }) else { return KN_ERROR };
    console_result(c.0.share_open(), out, error)
}

/// Le partage en cours : `{"open", "code", "link", "web", "web_local",
/// "relay": {"state": "joined"|"unreachable"} (nul sans relais), "display",
/// "guests": [{"id", "name", "app", "role", "since", "pending" (demande
/// venue par le relais, pas encore acceptée), "via" ("lan" ou "relay")}]}`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_console_share_state(c: *const KnConsole, out: *mut *mut c_char, error: *mut *mut c_char) -> i32 {
    let Some(c) = (unsafe { c.as_ref() }) else { return KN_ERROR };
    console_result(c.0.share_state(), out, error)
}

/// « Nouveau code » (même réponse que `kn_console_share_open`).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_console_share_code(c: *const KnConsole, out: *mut *mut c_char, error: *mut *mut c_char) -> i32 {
    let Some(c) = (unsafe { c.as_ref() }) else { return KN_ERROR };
    console_result(c.0.share_code(), out, error)
}

/// Le rôle d'un invité (`KN_SHARE_ROLE_*`) : « Donner les manettes »,
/// « Reprendre les manettes », « Donner le clavier et la souris »,
/// « Reprendre le clavier et la souris » (`kn_share_role_with_pads`,
/// `kn_share_role_with_kbm` : l'autre droit tel quel).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_console_share_guest(c: *const KnConsole, guest: *const c_char, role_code: i32, error: *mut *mut c_char) -> i32 {
    let Some(c) = (unsafe { c.as_ref() }) else { return KN_ERROR };
    let Some(r) = role(role_code) else { return KN_ERROR };
    match c.0.share_guest(&text(guest), r) {
        Ok(()) => KN_OK,
        Err(e) => failure(e, error),
    }
}

/// « Accepter » la demande d'un invité venu par le relais (`pending` dans
/// `guests` de `kn_console_share_state` et de `on_guests`) : il entre dans le
/// partage. `out` : la vue du partage, comme `kn_console_share_state` ;
/// −409 (`error` : le texte d'Admin) si les quatre places sont prises.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_console_share_accept(c: *const KnConsole, guest: *const c_char, out: *mut *mut c_char, error: *mut *mut c_char) -> i32 {
    let Some(c) = (unsafe { c.as_ref() }) else { return KN_ERROR };
    console_result(c.0.share_accept(&text(guest)), out, error)
}

/// « Refuser » la demande d'un invité venu par le relais (son appareil est
/// refusé dix minutes). `out` : la vue du partage.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_console_share_refuse(c: *const KnConsole, guest: *const c_char, out: *mut *mut c_char, error: *mut *mut c_char) -> i32 {
    let Some(c) = (unsafe { c.as_ref() }) else { return KN_ERROR };
    console_result(c.0.share_refuse(&text(guest)), out, error)
}

/// « Retirer » un invité.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_console_share_kick(c: *const KnConsole, guest: *const c_char, error: *mut *mut c_char) -> i32 {
    let Some(c) = (unsafe { c.as_ref() }) else { return KN_ERROR };
    match c.0.share_kick(&text(guest)) {
        Ok(()) => KN_OK,
        Err(e) => failure(e, error),
    }
}

/// « Arrêter le partage ».
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_console_share_close(c: *const KnConsole, error: *mut *mut c_char) -> i32 {
    let Some(c) = (unsafe { c.as_ref() }) else { return KN_ERROR };
    match c.0.share_close() {
        Ok(()) => KN_OK,
        Err(e) => failure(e, error),
    }
}
