//! Les comptes KyberNow pour les apps Apple (docs/comptes.md, lot 3 ; textes
//! dans docs/link-ui.md, « Comptes KyberNow »).
//!
//! - iPhone, iPad, Mac : `kn_account_sign_in` donne la page de connexion du
//!   relais à ouvrir dans `ASWebAuthenticationSession` (schéma `kybernow`),
//!   `kn_account_finish_sign_in` échange le retour contre le compte ;
//! - Apple TV : `kn_account_tv_code` (code et QR code), puis
//!   `kn_account_tv_poll` toutes les `interval` secondes ;
//! - le compte rendu (`{relay, token, account, device}`) se garde **entier**
//!   dans le Trousseau (le jeton est la session de l'appareil) ;
//! - `kn_account_computers` : les ordinateurs du compte ; `kn_account_merge` :
//!   l'accueil, un ordinateur une fois ; `kn_console_new_account` : la
//!   console d'un ordinateur du compte (jeton d'accès renouvelé par le cœur) ;
//!   une session s'ouvre avec `account` dans le JSON de `kn_session_start`
//!   (`{"account": <compte>, "station": "<identifiant>"}`).
//!
//! - réglages synchronisés : `kn_account_settings_fetch`,
//!   `kn_account_settings_put`, `kn_account_settings_known`,
//!   `kn_account_settings_from_stream`, `kn_account_settings_apply`
//!   (apps/core/README.md, « Réglages synchronisés »).
//!
//! Refus : `KN_ACCOUNT_*` (`AccountError::code` du cœur), le texte à montrer
//! dans `error` ; `KN_ERROR` pour un JSON illisible. Appels bloquants : hors
//! du fil principal.

use std::ffi::c_char;

use kybernow_core::account::{self, AccessParams, Account, AccountComputer, AccountError, Paired, SignIn, SyncedSettings, SyncedValues, TvCode, TvPoll};
use kybernow_core::console::Console;
use kybernow_core::i18n;
use kybernow_core::profile::{Capabilities, StreamSettings};

use crate::{KN_ERROR, KN_OK, KnConsole, give, give_to, optional, text};

/// `401 bad_session` : la session de l'appareil est fermée ; l'app oublie le
/// compte (Trousseau compris) et le dit.
pub const KN_ACCOUNT_SIGNED_OUT: i32 = 70;
/// L'ordinateur n'est plus au compte.
pub const KN_ACCOUNT_UNKNOWN_COMPUTER: i32 = 71;
/// Code de téléviseur refusé sur le téléphone.
pub const KN_ACCOUNT_DENIED: i32 = 72;
/// Code de téléviseur échu : en demander un autre.
pub const KN_ACCOUNT_EXPIRED: i32 = 73;
/// Retour de la fenêtre refusé (code échu, autre état).
pub const KN_ACCOUNT_BAD_CODE: i32 = 74;
/// « Annuler » dans la page du relais : rien à dire.
pub const KN_ACCOUNT_CANCELLED: i32 = 75;
pub const KN_ACCOUNT_TOO_MANY: i32 = 76;
/// Le relais n'a pas de comptes.
pub const KN_ACCOUNT_NO_ACCOUNTS: i32 = 77;
pub const KN_ACCOUNT_BAD_RELAY: i32 = 78;
pub const KN_ACCOUNT_UNREACHABLE: i32 = 79;
pub const KN_ACCOUNT_OTHER: i32 = 80;

fn refused(e: AccountError, error: *mut *mut c_char) -> i32 {
    give_to(error, e.text(i18n::current()));
    e.code()
}

fn parse<T: serde::de::DeserializeOwned>(json: *const c_char, error: *mut *mut c_char) -> Result<T, i32> {
    serde_json::from_str(&text(json)).map_err(|e| {
        give_to(error, e.to_string());
        KN_ERROR
    })
}

fn answer<T: serde::Serialize>(r: Result<T, AccountError>, out: *mut *mut c_char, error: *mut *mut c_char) -> i32 {
    match r {
        Ok(v) => {
            give_to(out, serde_json::to_string(&v).unwrap_or_default());
            KN_OK
        }
        Err(e) => refused(e, error),
    }
}

/// La forme normale d'un relais tapé (« Un autre relais… ») ; vide :
/// `share.kybernow.app`. Refus : `KN_ACCOUNT_BAD_RELAY`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_account_normalize_relay(relay: *const c_char, out: *mut *mut c_char, error: *mut *mut c_char) -> i32 {
    match account::normalize_relay(&text(relay)) {
        Ok(r) => {
            give_to(out, r);
            KN_OK
        }
        Err(e) => refused(e, error),
    }
}

/// « Se connecter à un compte KyberNow » (fenêtre du système) : `out`
/// `{relay, url, state, callback_scheme}` ; ouvrir `url` dans
/// `ASWebAuthenticationSession` avec `callback_scheme`, garder le tout pour
/// `kn_account_finish_sign_in`. `device_name` : « iPad de Léa ».
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_account_sign_in(relay: *const c_char, device_name: *const c_char, out: *mut *mut c_char, error: *mut *mut c_char) -> i32 {
    answer(account::sign_in(&text(relay), &text(device_name)), out, error)
}

/// Le retour de la fenêtre (`kybernow://compte?…`) : `out`, le compte à
/// garder entier dans le Trousseau. `KN_ACCOUNT_CANCELLED` : « Annuler »
/// dans la page (rien à montrer). Bloquant.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_account_finish_sign_in(sign_in_json: *const c_char, callback_url: *const c_char, out: *mut *mut c_char, error: *mut *mut c_char) -> i32 {
    let sign_in: SignIn = match parse(sign_in_json, error) {
        Ok(s) => s,
        Err(c) => return c,
    };
    answer(account::finish_sign_in(&sign_in, &text(callback_url)), out, error)
}

/// Vrai pour un retour de la fenêtre (`kybernow://compte…`).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_account_is_callback(url: *const c_char) -> bool {
    account::is_callback(&text(url))
}

/// Apple TV : un code pour cet appareil (« Apple TV du salon »). `out`
/// `{relay, code, poll, expires, interval, url, url_complete, url_label,
/// issued_at}` : QR code de `url_complete`, « ou ouvrez {url_label} ». Bloquant.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_account_tv_code(relay: *const c_char, device_name: *const c_char, out: *mut *mut c_char, error: *mut *mut c_char) -> i32 {
    answer(account::tv_code(&text(relay), &text(device_name)), out, error)
}

/// Une demande, toutes les `interval` secondes : `out` `{"status":"pending"}`
/// ou `{"status":"approved","account":{…}}` (à garder dans le Trousseau).
/// `KN_ACCOUNT_EXPIRED` : demander un autre code ; `KN_ACCOUNT_DENIED` :
/// refusé sur le téléphone. Bloquant.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_account_tv_poll(tv_code_json: *const c_char, out: *mut *mut c_char, error: *mut *mut c_char) -> i32 {
    let code: TvCode = match parse(tv_code_json, error) {
        Ok(c) => c,
        Err(c) => return c,
    };
    let r = account::tv_poll(&code).map(|p| match p {
        TvPoll::Pending => serde_json::json!({ "status": "pending" }),
        TvPoll::Approved(a) => serde_json::json!({ "status": "approved", "account": a }),
    });
    answer(r, out, error)
}

/// Minutes avant l'échéance du code (« Le code change dans {n} min. »).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_account_tv_minutes_left(tv_code_json: *const c_char) -> u64 {
    serde_json::from_str::<TvCode>(&text(tv_code_json)).map(|c| c.minutes_left(account::now_secs())).unwrap_or(0)
}

/// « Se déconnecter » : la session de l'appareil est fermée chez le relais ;
/// l'app oublie le compte de toute façon. Bloquant.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_account_sign_out(account_json: *const c_char, error: *mut *mut c_char) -> i32 {
    let a: Account = match parse(account_json, error) {
        Ok(a) => a,
        Err(c) => return c,
    };
    match account::sign_out(&a) {
        Ok(()) => KN_OK,
        Err(e) => refused(e, error),
    }
}

/// Ce que montrent les réglages « Comptes » : `{key, relay, relay_label,
/// account: {id, name, email}, device, console}`, sans le jeton.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_account_summary(account_json: *const c_char) -> *mut c_char {
    match serde_json::from_str::<Account>(&text(account_json)) {
        Ok(a) => give(serde_json::to_string(&a.summary()).unwrap_or_default()),
        Err(_) => std::ptr::null_mut(),
    }
}

/// Les ordinateurs du compte : `out` `[{account, relay, station, name,
/// rights, owner, mine, online, admin_fp, attached_at}]`.
/// `KN_ACCOUNT_SIGNED_OUT` : oublier le compte. Bloquant.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_account_computers(account_json: *const c_char, out: *mut *mut c_char, error: *mut *mut c_char) -> i32 {
    let a: Account = match parse(account_json, error) {
        Ok(a) => a,
        Err(c) => return c,
    };
    answer(account::computers(&a), out, error)
}

/// La liste qui arrive (`fresh_json`) avec les empreintes d'Admin déjà vues
/// (`known_json`, la liste gardée) : le relais ne les donne que pour une
/// Station connectée.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_account_remember_fingerprints(fresh_json: *const c_char, known_json: *const c_char) -> *mut c_char {
    let Ok(mut fresh) = serde_json::from_str::<Vec<AccountComputer>>(&text(fresh_json)) else { return std::ptr::null_mut() };
    let known = serde_json::from_str::<Vec<AccountComputer>>(&text(known_json)).unwrap_or_default();
    account::remember_fingerprints(&mut fresh, &known);
    give(serde_json::to_string(&fresh).unwrap_or_default())
}

/// L'accueil, un ordinateur une fois : `paired_json` `[{id, fingerprint,
/// station}]` (les ordinateurs appairés), `computers_json` ceux des comptes ;
/// rend `{linked: [[id, ordinateur]], added: [ordinateur]}`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_account_merge(paired_json: *const c_char, computers_json: *const c_char) -> *mut c_char {
    let paired = serde_json::from_str::<Vec<Paired>>(&text(paired_json)).unwrap_or_default();
    let computers = serde_json::from_str::<Vec<AccountComputer>>(&text(computers_json)).unwrap_or_default();
    give(serde_json::to_string(&account::merge(&paired, &computers)).unwrap_or_default())
}

/// La pastille d'origine d'un ordinateur du compte (« Compte », « Compte ·
/// {relais} » avec plusieurs relais, l'organisation, « Ouvert par {prénom} »).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_account_origin(computer_json: *const c_char, several_relays: bool) -> *mut c_char {
    match serde_json::from_str::<AccountComputer>(&text(computer_json)) {
        Ok(c) => give(c.origin(i18n::current(), several_relays)),
        Err(_) => std::ptr::null_mut(),
    }
}

/// « Regarder » est proposé pour cet ordinateur du compte (droit `view`) :
/// sur la carte si `kn_account_watch_only`, sinon dans son menu. Faux pour un
/// JSON illisible.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_account_can_watch(computer_json: *const c_char) -> bool {
    serde_json::from_str::<AccountComputer>(&text(computer_json)).is_ok_and(|c| c.can_watch())
}

/// Le droit `view` sans `session` : la carte propose « Regarder » à la place
/// de « Se connecter » (`kn_share_watch`).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_account_watch_only(computer_json: *const c_char) -> bool {
    serde_json::from_str::<AccountComputer>(&text(computer_json)).is_ok_and(|c| c.watch_only())
}

/// La pastille d'un ordinateur appairé (« Appairé »), quand un compte est connecté.
#[unsafe(no_mangle)]
pub extern "C" fn kn_account_origin_paired() -> *mut c_char {
    give(account::origin_paired(i18n::current()))
}

/// La route par le relais d'un ordinateur du compte (`RelayRoute` en JSON,
/// pour `route` de `kn_session_start`), nulle sans empreinte d'Admin connue.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_account_route(computer_json: *const c_char) -> *mut c_char {
    match serde_json::from_str::<AccountComputer>(&text(computer_json)).ok().and_then(|c| c.route()) {
        Some(r) => give(serde_json::to_string(&r).unwrap_or_default()),
        None => std::ptr::null_mut(),
    }
}

/// La console d'un ordinateur du compte : `access_json` `{account, station}`
/// (la valeur `account` de `kn_session_start`) ; `host`, `port` : Admin sur
/// le réseau s'il y est vu (vide sinon) ; `pinned` : l'empreinte d'Admin ;
/// `route_json` : par le relais (`kn_account_route`), nul sur le réseau. Nul
/// pour un JSON illisible.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_console_new_account(
    host: *const c_char,
    port: u16,
    pinned: *const c_char,
    access_json: *const c_char,
    route_json: *const c_char,
) -> *mut KnConsole {
    let Ok(params) = serde_json::from_str::<AccessParams>(&text(access_json)) else { return std::ptr::null_mut() };
    let route = crate::relay::route_or_broken(optional(route_json));
    let access = account::StationAccess::new(params);
    let console = Console::account(&text(host), port, access, optional(pinned)).with_relay(route);
    Box::into_raw(Box::new(KnConsole(std::sync::Arc::new(console))))
}

// MARK: - Réglages synchronisés

/// Les réglages synchronisés de tous les ordinateurs du compte (docs/comptes.md,
/// « Réglages synchronisés ») : `out` `[{station, updated_at, values}]`, à
/// garder avec le compte (le dernier connu de chaque ordinateur sert à
/// `kn_account_settings_put`). `KN_ACCOUNT_SIGNED_OUT` : oublier le compte ;
/// `KN_ACCOUNT_NO_ACCOUNTS` : relais sans réglages synchronisés, garder les
/// réglages locaux sans rien dire. Bloquant.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_account_settings_fetch(account_json: *const c_char, out: *mut *mut c_char, error: *mut *mut c_char) -> i32 {
    let a: Account = match parse(account_json, error) {
        Ok(a) => a,
        Err(c) => return c,
    };
    answer(account::fetch_settings(&a), out, error)
}

/// Envoie les réglages d'un ordinateur : `values_json` les clés changées
/// (`{profile, codec, yuv444, mbps, hdr, immersive}`, toutes facultatives ;
/// `"profile": null` : personnalisé), posées sur `last_json` (le dernier
/// `{station, updated_at, values}` connu de cet ordinateur, nul s'il n'y en a
/// pas : les clés que l'app ne connaît pas y restent). `out` : le nouveau
/// `{station, updated_at, values}` à garder à la place. `KN_ACCOUNT_UNKNOWN_COMPUTER` :
/// l'ordinateur n'est plus au compte, oublier ses réglages gardés. Bloquant.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_account_settings_put(
    account_json: *const c_char,
    station: *const c_char,
    last_json: *const c_char,
    values_json: *const c_char,
    out: *mut *mut c_char,
    error: *mut *mut c_char,
) -> i32 {
    let a: Account = match parse(account_json, error) {
        Ok(a) => a,
        Err(c) => return c,
    };
    let values: SyncedValues = match parse(values_json, error) {
        Ok(v) => v,
        Err(c) => return c,
    };
    let last = optional(last_json).and_then(|l| serde_json::from_str::<SyncedSettings>(&l).ok());
    answer(account::put_settings(&a, &text(station), last.as_ref(), &values), out, error)
}

/// Les clés connues d'un `{station, updated_at, values}`, vérifiées :
/// `{profile, codec, yuv444, mbps, hdr, immersive}`, une clé absente ou
/// illisible omise, `"profile": null` pour personnalisé. Nul pour un JSON
/// illisible.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_account_settings_known(synced_json: *const c_char) -> *mut c_char {
    match serde_json::from_str::<SyncedSettings>(&text(synced_json)) {
        Ok(s) => give(serde_json::to_string(&s.known()).unwrap_or_default()),
        Err(_) => std::ptr::null_mut(),
    }
}

/// Les clés à envoyer d'après les réglages de session de l'app pour cet
/// ordinateur (`settings_json`, le JSON de `settings` de
/// `kn_session_start`) et son mode immersif (`fullscreen`, `always`,
/// `never` ; nul : pas envoyé). Nul pour un JSON illisible.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_account_settings_from_stream(settings_json: *const c_char, immersive: *const c_char) -> *mut c_char {
    match StreamSettings::from_json(&text(settings_json)) {
        Ok(s) => give(serde_json::to_string(&SyncedValues::from_stream(&s).with_immersive(optional(immersive).as_deref())).unwrap_or_default()),
        Err(_) => std::ptr::null_mut(),
    }
}

/// Les réglages de session de l'app (`base_json`, comme `settings` de
/// `kn_session_start`) avec les clés synchronisées (`values_json`, de
/// `kn_account_settings_known`), ramenés à ce que l'appareil décode
/// (`caps_json`, `{hevc, hevc444, av1, hdr, pyrowave}`) : un profil prend
/// ses réglages sur cet appareil ; personnalisé, le format s'il se décode,
/// le 4:4:4 s'il existe, le débit ; le HDR tel quel. Rend le même JSON que
/// `base_json` ; nul pour un JSON illisible.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_account_settings_apply(values_json: *const c_char, base_json: *const c_char, caps_json: *const c_char) -> *mut c_char {
    let Ok(values) = serde_json::from_str::<SyncedValues>(&text(values_json)) else { return std::ptr::null_mut() };
    let Ok(base) = StreamSettings::from_json(&text(base_json)) else { return std::ptr::null_mut() };
    let caps = serde_json::from_str::<Capabilities>(&text(caps_json)).unwrap_or_default();
    give(values.apply_to(&base, caps).to_json().to_string())
}

/// « Synchroniser les réglages des ordinateurs » (réglages de l'app, section
/// « Comptes » ; allumé par défaut, gardé sur l'appareil).
#[unsafe(no_mangle)]
pub extern "C" fn kn_account_sync_settings_title() -> *mut c_char {
    give(account::sync_settings_texts(i18n::current()).0)
}

/// L'aide de l'interrupteur : « Le profil, le format, le débit, le HDR et le
/// mode immersif de chaque ordinateur vous suivent sur vos appareils. »
#[unsafe(no_mangle)]
pub extern "C" fn kn_account_sync_settings_help() -> *mut c_char {
    give(account::sync_settings_texts(i18n::current()).1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codes_du_coeur() {
        assert_eq!(KN_ACCOUNT_SIGNED_OUT, AccountError::SignedOut.code());
        assert_eq!(KN_ACCOUNT_CANCELLED, AccountError::Cancelled.code());
        assert_eq!(KN_ACCOUNT_OTHER, AccountError::Other(String::new()).code());
    }
}
