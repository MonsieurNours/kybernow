//! Le relais pour l'app Android (docs/relais.md) : les mêmes fonctions que
//! le pont C (`apps/apple/src/relay.rs`), pour `Native`.

use jni::objects::{JClass, JString};
use jni::sys::{jint, jlong, jstring};

use kybernow_core::console::{Console, ConsoleError, Info};
use kybernow_core::i18n;
use kybernow_core::relay::{self, RelayAddr, RelayMode, RelayRoute, RouteTransport};

use crate::exports::{envelope, give, optional, string, Env};

/// Comme `consoleNew`, par le relais quand `route` est donné (`{server,
/// station, admin_fp}`) ; erreurs du relais : codes 30 (relais muet) et 31
/// (ordinateur absent du relais).
#[unsafe(no_mangle)]
pub extern "system" fn Java_fr_leslignesclaires_kybernow_core_Native_consoleNewRelay(
    mut env: Env,
    _: JClass,
    host: JString,
    port: jint,
    user: JString,
    password: JString,
    pinned: JString,
    route: JString,
) -> jlong {
    let (host, user, password, pinned) = (string(&mut env, &host), string(&mut env, &user), string(&mut env, &password), optional(&mut env, &pinned));
    let route = optional(&mut env, &route).and_then(|r| serde_json::from_str::<RelayRoute>(&r).ok());
    Box::into_raw(Box::new(Console::new(&host, port as u16, &user, &password, pinned).with_relay(route))) as jlong
}

/// La route d'un ordinateur par son relais d'après son `info` et l'empreinte d'Admin connue ; nul sinon.
#[unsafe(no_mangle)]
pub extern "system" fn Java_fr_leslignesclaires_kybernow_core_Native_relayRouteFromInfo(mut env: Env, _: JClass, info: JString, admin_fp: JString) -> jstring {
    let (info, fp) = (string(&mut env, &info), string(&mut env, &admin_fp));
    let Ok(info) = serde_json::from_str::<Info>(&info) else { return std::ptr::null_mut() };
    match info.relay.and_then(|r| r.route(&fp)) {
        Some(route) => give(&mut env, &serde_json::to_string(&route).unwrap_or_default()),
        None => std::ptr::null_mut(),
    }
}

/// Quels ordinateurs sont joignables par ce relais : `value` `[true, false, …]`. Bloquant.
#[unsafe(no_mangle)]
pub extern "system" fn Java_fr_leslignesclaires_kybernow_core_Native_relayPresence(mut env: Env, _: JClass, relay_addr: JString, stations: JString) -> jstring {
    let (addr, stations) = (string(&mut env, &relay_addr), string(&mut env, &stations));
    let s = match (addr.parse::<RelayAddr>(), serde_json::from_str::<Vec<String>>(&stations)) {
        (Ok(addr), Ok(stations)) => envelope(relay::presence(&addr, &stations).map_err(ConsoleError::Relay)),
        _ => serde_json::json!({ "code": 1, "error": "relais ou identifiants illisibles" }).to_string(),
    };
    give(&mut env, &s)
}

/// `KYBERNOW_RELAY` : 0 normal, 1 toujours par le relais, 2 jamais.
#[unsafe(no_mangle)]
pub extern "system" fn Java_fr_leslignesclaires_kybernow_core_Native_relayMode(_: Env, _: JClass) -> jint {
    RelayMode::current().code()
}

/// La ligne « Chemin » : 0 « Direct », 1 « Par le relais », 2 « Par le relais (TCP) ».
#[unsafe(no_mangle)]
pub extern "system" fn Java_fr_leslignesclaires_kybernow_core_Native_relayPathLabel(mut env: Env, _: JClass, transport: jint) -> jstring {
    let t = match transport {
        0 => RouteTransport::Direct,
        1 => RouteTransport::Relay,
        2 => RouteTransport::RelayTcp,
        _ => return std::ptr::null_mut(),
    };
    give(&mut env, &t.label(i18n::current()))
}
