//! KyberNow Share pour les apps Apple (docs/kybernow-share.md, textes dans
//! docs/link-ui.md, « Partage ») : rejoindre un partage (code envoyé aux
//! Stations du réseau, lien), et, côté hôte, ouvrir le partage, changer le
//! code, le rôle d'un invité, le retirer, arrêter. La session d'invité
//! s'ouvre ensuite par `kn_session_start` avec `guest` (voir lib.rs).

use std::ffi::c_char;

use kybernow_core::console::Info;
use kybernow_core::i18n;
use kybernow_core::profile::Capabilities;
use kybernow_core::share::{self, Found, JoinRequest, ShareAvailability, ShareError, ShareRole, ShareTarget};

use crate::{KN_ERROR, KN_OK, KnConsole, console_result, failure, give, give_to, optional, text};

/// Refus de « Rejoindre » (`kn_share_join`), en plus des codes des autres
/// appels : aucun partage avec ce code sur le réseau, partage complet (quatre
/// invités au plus), trop d'essais (10 par minute et par adresse), format de
/// l'image de l'hôte que l'appareil ne lit pas. `error` : le texte à montrer.
pub const KN_SHARE_NO_SHARE: i32 = 20;
pub const KN_SHARE_FULL: i32 = 21;
pub const KN_SHARE_TOO_MANY: i32 = 22;
pub const KN_SHARE_UNREADABLE: i32 = 23;

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

fn share_failure(e: ShareError, error: *mut *mut c_char) -> i32 {
    let message = e.to_string();
    let code = match e {
        ShareError::Other(e) => return failure(e, error),
        ShareError::NoShare => KN_SHARE_NO_SHARE,
        ShareError::Full => KN_SHARE_FULL,
        ShareError::TooManyTries { .. } => KN_SHARE_TOO_MANY,
        ShareError::Unreadable { .. } => KN_SHARE_UNREADABLE,
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

// MARK: - Rejoindre

/// « Rejoindre » : envoie le code à ces Stations à la fois et rend la
/// première qui l'accepte. `targets_json` : `[{"host", "port",
/// "fingerprint"}]` (les ordinateurs trouvés sur le réseau, comme pour
/// l'appairage, ou celui du lien ; `fingerprint` facultatif) ;
/// `request_json` : `{"code" (six chiffres), "name" (le prénom), "device"
/// (l'identifiant de l'appareil), "app" (mac, ios, tvos)}` ;
/// `capabilities_json` (`{hevc, hevc444, av1}`, nul : pas de vérification) :
/// le format de l'image de l'hôte est vérifié, et l'invité repart tout de
/// suite s'il ne le lit pas (`KN_SHARE_UNREADABLE`). `out` : `{"target":
/// {host, port, fingerprint}, "joined": {…}}` : `target` donne
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
        Err(e) => share_failure(e, error),
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

/// Le partage en cours : `{"open", "code", "link", "web", "display",
/// "guests": [{"id", "name", "app", "role", "since"}]}`.
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
