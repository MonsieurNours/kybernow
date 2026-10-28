//! Le relais pour les apps Apple (docs/relais.md) : la console par le
//! relais, la route d'un ordinateur d'après son `info`, la présence des
//! ordinateurs chez le relais (accueil), le mode d'essai, les libellés du
//! chemin. La session y passe par `route` et `relay` dans `kn_session_start`.

use std::ffi::c_char;

use kybernow_core::console::{Console, Info};
use kybernow_core::i18n;
use kybernow_core::relay::{self, RelayAddr, RelayMode, RelayRoute, RouteTransport};

use crate::{KN_ERROR, KN_OK, KnConsole, failure, give, give_to, optional, text};

/// Comme `kn_console_new`, par le relais quand `route_json` est donné
/// (`{server, station, admin_fp}`) : tous les appels passent par le tunnel,
/// Admin épinglé sur `admin_fp` (jamais de premier contact). Erreurs du
/// relais : `KN_RELAY_UNREACHABLE`, `KN_RELAY_STATION_OFFLINE`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_console_new_relay(
    host: *const c_char,
    port: u16,
    user: *const c_char,
    password: *const c_char,
    pinned: *const c_char,
    route_json: *const c_char,
) -> *mut KnConsole {
    let route = optional(route_json).and_then(|r| serde_json::from_str::<RelayRoute>(&r).ok());
    let console = Console::new(&text(host), port, &text(user), &text(password), optional(pinned)).with_relay(route);
    Box::into_raw(Box::new(KnConsole(console)))
}

/// La route d'un ordinateur par son relais, d'après son `info` (`relay`) et
/// l'empreinte d'Admin connue de l'appareil : `{server, station, admin_fp}`,
/// à garder et à donner à `kn_session_start` (`relay`) et à
/// `kn_console_new_relay` ; nul sans relais ou sans empreinte.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_relay_route_from_info(info_json: *const c_char, admin_fp: *const c_char) -> *mut c_char {
    let Ok(info) = serde_json::from_str::<Info>(&text(info_json)) else { return std::ptr::null_mut() };
    match info.relay.and_then(|r| r.route(&text(admin_fp))) {
        Some(route) => give(serde_json::to_string(&route).unwrap_or_default()),
        None => std::ptr::null_mut(),
    }
}

/// Quels ordinateurs sont joignables par ce relais (accueil : « Hors de chez
/// vous ») : `stations_json` `["<id>", …]` (16 au plus) ; `out` : `[true,
/// false, …]` dans le même ordre. Bloquant.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_relay_presence(relay_addr: *const c_char, stations_json: *const c_char, out: *mut *mut c_char, error: *mut *mut c_char) -> i32 {
    let Ok(addr) = text(relay_addr).parse::<RelayAddr>() else { return KN_ERROR };
    let Ok(stations) = serde_json::from_str::<Vec<String>>(&text(stations_json)) else { return KN_ERROR };
    match relay::presence(&addr, &stations) {
        Ok(online) => {
            give_to(out, serde_json::to_string(&online).unwrap_or_default());
            KN_OK
        }
        Err(e) => failure(kybernow_core::console::ConsoleError::Relay(e), error),
    }
}

/// `KYBERNOW_RELAY` : 0 normal, 1 toujours par le relais (essais), 2 jamais.
#[unsafe(no_mangle)]
pub extern "C" fn kn_relay_mode() -> i32 {
    RelayMode::current().code()
}

/// La ligne « Chemin » des statistiques : 0 « Direct », 1 « Par le relais »,
/// 2 « Par le relais (TCP) » (langue de l'app) ; nul pour un code inconnu.
#[unsafe(no_mangle)]
pub extern "C" fn kn_relay_path_label(transport: i32) -> *mut c_char {
    let t = match transport {
        0 => RouteTransport::Direct,
        1 => RouteTransport::Relay,
        2 => RouteTransport::RelayTcp,
        _ => return std::ptr::null_mut(),
    };
    give(t.label(i18n::current()))
}

/// Le relais public, `share.kybernow.app` (port 443).
#[unsafe(no_mangle)]
pub extern "C" fn kn_relay_default() -> *mut c_char {
    give(relay::DEFAULT_RELAY.to_string())
}

/// Une adresse de relais sous sa forme normale (sans port pour le 443 :
/// `share.kybernow.app` ; `relais.exemple.org:9443` garde le sien) ; nul si
/// elle ne se lit pas. Deux relais sont le même si leurs formes sont égales.
///
/// # Safety
/// `addr` : chaîne C valable ou nul.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_relay_normalize(addr: *const c_char) -> *mut c_char {
    match relay::normalize(&text(addr)) {
        Some(a) => give(a),
        None => std::ptr::null_mut(),
    }
}

/// Le texte de link-ui d'une erreur du relais (`KN_RELAY_UNREACHABLE`,
/// `KN_RELAY_STATION_OFFLINE`, `KN_RELAY_ACCESS_OFF`), avec le nom de
/// l'ordinateur (`name` nul ou vide : sans nom), dans la langue de l'app :
/// « {nom} n'est pas joignable hors de chez vous (accès coupé dans
/// Admin). », « {nom} n'est pas joignable : l'ordinateur est éteint ou en
/// veille. », « Le relais ne répond pas. » ; nul pour un autre code.
///
/// # Safety
/// `name` : chaîne C valable ou nul.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_relay_error_text(code: i32, name: *const c_char) -> *mut c_char {
    let name = text(name);
    match relay::RelayError::from_code(code) {
        Some(e) => give(e.text(i18n::current(), Some(&name))),
        None => std::ptr::null_mut(),
    }
}
