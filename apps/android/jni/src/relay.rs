//! Le relais pour l'app Android (docs/relais.md) : les mêmes fonctions que
//! le pont C (`apps/apple/src/relay.rs`), pour `Native`.

use jni::objects::{JClass, JString};
use jni::sys::{jboolean, jint, jlong, jstring};

use kybernow_core::console::{Console, ConsoleError, Info};
use kybernow_core::i18n;
use kybernow_core::relay::{self, RelayAddr, RelayMode, RelayRoute, RouteTransport};

use crate::exports::{envelope, give, optional, string, Env};

/// Comme `consoleNew`, par le relais quand `route` est donné (`{server,
/// station, admin_fp}`) ; erreurs du relais : codes 30 (relais muet), 31
/// (ordinateur absent du relais) et 32 (accès coupé dans Admin).
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
    kybernow_core::ffi_guard!({
        let (host, user, password, pinned) = (string(&mut env, &host), string(&mut env, &user), string(&mut env, &password), optional(&mut env, &pinned));
        let route = optional(&mut env, &route).and_then(|r| serde_json::from_str::<RelayRoute>(&r).ok());
        Box::into_raw(Box::new(Console::new(&host, port as u16, &user, &password, pinned).with_relay(route))) as jlong
    })
}

/// La route d'un ordinateur par son relais d'après son `info` et l'empreinte d'Admin connue ; nul sinon.
#[unsafe(no_mangle)]
pub extern "system" fn Java_fr_leslignesclaires_kybernow_core_Native_relayRouteFromInfo(mut env: Env, _: JClass, info: JString, admin_fp: JString) -> jstring {
    kybernow_core::ffi_guard!(null => {
        let (info, fp) = (string(&mut env, &info), string(&mut env, &admin_fp));
        let Ok(info) = serde_json::from_str::<Info>(&info) else { return std::ptr::null_mut() };
        match info.relay.and_then(|r| r.route(&fp)) {
            Some(route) => give(&mut env, &serde_json::to_string(&route).unwrap_or_default()),
            None => std::ptr::null_mut(),
        }
    })
}

/// Quels ordinateurs sont joignables par ce relais : `value` `[true, false, …]`. Bloquant.
#[unsafe(no_mangle)]
pub extern "system" fn Java_fr_leslignesclaires_kybernow_core_Native_relayPresence(mut env: Env, _: JClass, relay_addr: JString, stations: JString) -> jstring {
    kybernow_core::ffi_guard!(null => {
        let (addr, stations) = (string(&mut env, &relay_addr), string(&mut env, &stations));
        let s = match (addr.parse::<RelayAddr>(), serde_json::from_str::<Vec<String>>(&stations)) {
            (Ok(addr), Ok(stations)) => envelope(relay::presence(&addr, &stations).map_err(ConsoleError::Relay)),
            _ => serde_json::json!({ "code": 1, "error": "relais ou identifiants illisibles" }).to_string(),
        };
        give(&mut env, &s)
    })
}

/// `KYBERNOW_RELAY` : 0 normal, 1 toujours par le relais, 2 jamais.
#[unsafe(no_mangle)]
pub extern "system" fn Java_fr_leslignesclaires_kybernow_core_Native_relayMode(_: Env, _: JClass) -> jint {
    kybernow_core::ffi_guard!({
        RelayMode::current().code()
    })
}

/// La ligne « Chemin » : 0 « Direct », 1 « Par le relais », 2 « Par le relais (TCP) ».
#[unsafe(no_mangle)]
pub extern "system" fn Java_fr_leslignesclaires_kybernow_core_Native_relayPathLabel(mut env: Env, _: JClass, transport: jint) -> jstring {
    kybernow_core::ffi_guard!(null => {
        let t = match transport {
            0 => RouteTransport::Direct,
            1 => RouteTransport::Relay,
            2 => RouteTransport::RelayTcp,
            _ => return std::ptr::null_mut(),
        };
        give(&mut env, &t.label(i18n::current()))
    })
}

/// Une adresse de relais sous sa forme normale (`share.kybernow.app` pour le
/// 443, `relais.exemple.org:9443` sinon) ; `null` si elle ne se lit pas.
#[unsafe(no_mangle)]
pub extern "system" fn Java_fr_leslignesclaires_kybernow_core_Native_relayNormalize(mut env: Env, _: JClass, addr: JString) -> jstring {
    kybernow_core::ffi_guard!(null => {
        let addr = string(&mut env, &addr);
        match relay::normalize(&addr) {
            Some(a) => give(&mut env, &a),
            None => std::ptr::null_mut(),
        }
    })
}

/// Le texte de link-ui d'une erreur du relais (30, 31, 32), avec le nom de
/// l'ordinateur (`name` `null` ou vide : sans nom) ; `null` pour un autre code.
#[unsafe(no_mangle)]
pub extern "system" fn Java_fr_leslignesclaires_kybernow_core_Native_relayErrorText(mut env: Env, _: JClass, code: jint, name: JString) -> jstring {
    kybernow_core::ffi_guard!(null => {
        let name = optional(&mut env, &name).unwrap_or_default();
        match relay::RelayError::from_code(code) {
            Some(e) => give(&mut env, &e.text(i18n::current(), Some(&name))),
            None => std::ptr::null_mut(),
        }
    })
}

/// « Envoyer le journal » par QR code (`journal::deposit` du cœur : en-tête
/// et journal compressés, chiffrés `KNJ1`, déposés sur `relay`, celui du
/// compte de l'appareil, empreinte épinglée comprise ; vide : le relais
/// public). Rend `{relay, id, owner, url, expires_in}`, ou `{error, detail}`
/// (`error` : 90 injoignable, 91 relais d'avant, 92 trop d'envois, 1 autre).
/// Bloquant (30 s au plus) : hors du fil principal.
#[unsafe(no_mangle)]
pub extern "system" fn Java_fr_leslignesclaires_kybernow_core_Native_journalDeposit(mut env: Env, _: JClass, relay: JString, header: JString, body: JString) -> jstring {
    kybernow_core::ffi_guard!(null => {
        let (relay, header, body) = (string(&mut env, &relay), string(&mut env, &header), string(&mut env, &body));
        let json = match kybernow_core::journal::deposit(&relay, &header, &body) {
            Ok(d) => d.to_json(),
            Err(e) => {
                log::warn!("journal : dépôt impossible ({})", e.detail());
                serde_json::json!({ "error": e.code(), "detail": e.detail() }).to_string()
            }
        };
        give(&mut env, &json)
    })
}

/// Où en est un dépôt : 0 en attente, 1 ouvert, 93 plus là, 90 pas de
/// réponse (`journal::Remote::code`). Bloquant.
#[unsafe(no_mangle)]
pub extern "system" fn Java_fr_leslignesclaires_kybernow_core_Native_journalState(mut env: Env, _: JClass, relay: JString, id: JString, owner: JString) -> jint {
    kybernow_core::ffi_guard!(90 => {
        let (relay, id, owner) = (string(&mut env, &relay), string(&mut env, &id), string(&mut env, &owner));
        kybernow_core::journal::state(&relay, &id, &owner).code()
    })
}

/// « Terminé » : retire un dépôt pas encore ouvert ; faux si le relais n'a
/// pas répondu. Bloquant.
#[unsafe(no_mangle)]
pub extern "system" fn Java_fr_leslignesclaires_kybernow_core_Native_journalDiscard(mut env: Env, _: JClass, relay: JString, id: JString, owner: JString) -> jboolean {
    kybernow_core::ffi_guard!(0 => {
        let (relay, id, owner) = (string(&mut env, &relay), string(&mut env, &id), string(&mut env, &owner));
        u8::from(kybernow_core::journal::discard(&relay, &id, &owner))
    })
}

/// Le relais tel qu'on le montre (`rules::relay::label`) : `share.kybernow.app`,
/// `relais.exemple.org:9443`, sans empreinte épinglée ni port 443.
#[unsafe(no_mangle)]
pub extern "system" fn Java_fr_leslignesclaires_kybernow_core_Native_relayLabel(mut env: Env, _: JClass, addr: JString) -> jstring {
    kybernow_core::ffi_guard!(null => {
        let addr = string(&mut env, &addr);
        give(&mut env, &kybernow_core::rules::relay::label(&addr))
    })
}
