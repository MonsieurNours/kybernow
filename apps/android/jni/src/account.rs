//! Les comptes KyberNow pour l'app Android (docs/comptes.md, lot 3 ; textes
//! dans docs/link-ui.md, « Comptes KyberNow ») : les mêmes fonctions que le
//! pont C (`apps/apple/src/account.rs`), pour `Native`.
//!
//! Téléphone et tablette : `accountSignIn` donne la page de connexion du
//! relais, à ouvrir dans un onglet personnalisé (Custom Tabs) ; le retour
//! `kybernow://compte?…` arrive à l'activité, `accountFinishSignIn` l'échange
//! contre le compte. Android TV : `accountTvCode`, puis `accountTvPoll`
//! toutes les `interval` secondes. Le compte rendu se garde **entier**
//! (jeton compris) dans un stockage chiffré par l'Android Keystore.
//!
//! Réponses en JSON comme les autres appels : `{"code": 0, "value": …}` ou
//! `{"code": …, "error": "…", "kind": "…"}` ; refus (`AccountError::code`) :
//! 70 session de l'appareil fermée (oublier le compte), 71 ordinateur retiré
//! du compte, 72 code refusé sur le téléphone, 73 code échu, 74 retour
//! refusé, 75 annulé, 76 trop d'essais, 77 relais sans comptes, 78 adresse du
//! relais illisible, 79 relais muet, 80 le reste ; 1 : JSON illisible.
//! Appels bloquants : hors du fil principal.
//!
//! Réglages synchronisés : `accountSettingsFetch`, `accountSettingsPut`,
//! `accountSettingsKnown`, `accountSettingsFromStream`,
//! `accountSettingsApply`, `accountSyncSettingsTexts`.

use jni::objects::{JClass, JString};
use jni::sys::{jboolean, jint, jlong, jstring, JNI_FALSE, JNI_TRUE};

use kybernow_core::account::{self, AccessParams, Account, AccountComputer, AccountError, Paired, SignIn, SyncedSettings, SyncedValues, TvCode, TvPoll};
use kybernow_core::console::Console;
use kybernow_core::i18n;
use kybernow_core::profile::{Capabilities, StreamSettings};
use kybernow_core::relay::RelayRoute;

use crate::exports::{give, optional, string, Env};

fn envelope<T: serde::Serialize>(r: Result<T, AccountError>) -> String {
    match r {
        Ok(v) => serde_json::json!({ "code": 0, "value": v }).to_string(),
        Err(e) => serde_json::json!({ "code": e.code(), "error": e.text(i18n::current()), "kind": e.kind() }).to_string(),
    }
}

fn bad(e: impl std::fmt::Display) -> String {
    serde_json::json!({ "code": 1, "error": e.to_string() }).to_string()
}

fn with<T: serde::de::DeserializeOwned>(json: &str, f: impl FnOnce(T) -> String) -> String {
    match serde_json::from_str::<T>(json) {
        Ok(v) => f(v),
        Err(e) => bad(e),
    }
}

/// La forme normale d'un relais tapé (« Un autre relais… ») ; vide : `share.kybernow.app`.
#[unsafe(no_mangle)]
pub extern "system" fn Java_fr_leslignesclaires_kybernow_core_Native_accountNormalizeRelay(mut env: Env, _: JClass, relay: JString) -> jstring {
    kybernow_core::ffi_guard!(null => {
        let relay = string(&mut env, &relay);
        let s = envelope(account::normalize_relay(&relay));
        give(&mut env, &s)
    })
}

/// « Se connecter à un compte KyberNow » : `value` `{relay, url, state,
/// callback_scheme}`, à garder jusqu'au retour.
#[unsafe(no_mangle)]
pub extern "system" fn Java_fr_leslignesclaires_kybernow_core_Native_accountSignIn(mut env: Env, _: JClass, relay: JString, device_name: JString) -> jstring {
    kybernow_core::ffi_guard!(null => {
        let (relay, name) = (string(&mut env, &relay), string(&mut env, &device_name));
        let s = envelope(account::sign_in(&relay, &name));
        give(&mut env, &s)
    })
}

/// Le retour `kybernow://compte?…` : `value`, le compte à garder entier.
/// Code 75 : « Annuler » dans la page (rien à montrer). Bloquant.
#[unsafe(no_mangle)]
pub extern "system" fn Java_fr_leslignesclaires_kybernow_core_Native_accountFinishSignIn(mut env: Env, _: JClass, sign_in: JString, callback: JString) -> jstring {
    kybernow_core::ffi_guard!(null => {
        let (sign_in, callback) = (string(&mut env, &sign_in), string(&mut env, &callback));
        let s = with::<SignIn>(&sign_in, |si| envelope(account::finish_sign_in(&si, &callback)));
        give(&mut env, &s)
    })
}

/// Vrai pour un retour de la connexion (`kybernow://compte…`).
#[unsafe(no_mangle)]
pub extern "system" fn Java_fr_leslignesclaires_kybernow_core_Native_accountIsCallback(mut env: Env, _: JClass, url: JString) -> jboolean {
    kybernow_core::ffi_guard!({
        if account::is_callback(&string(&mut env, &url)) { JNI_TRUE } else { JNI_FALSE }
    })
}

/// Android TV : un code (`value` `{relay, code, poll, expires, interval, url,
/// url_complete, url_label, issued_at}`). Bloquant.
#[unsafe(no_mangle)]
pub extern "system" fn Java_fr_leslignesclaires_kybernow_core_Native_accountTvCode(mut env: Env, _: JClass, relay: JString, device_name: JString) -> jstring {
    kybernow_core::ffi_guard!(null => {
        let (relay, name) = (string(&mut env, &relay), string(&mut env, &device_name));
        let s = envelope(account::tv_code(&relay, &name));
        give(&mut env, &s)
    })
}

/// Une demande : `value` `{"status":"pending"}` ou `{"status":"approved",
/// "account":{…}}`. 73 : demander un autre code ; 72 : refusé. Bloquant.
#[unsafe(no_mangle)]
pub extern "system" fn Java_fr_leslignesclaires_kybernow_core_Native_accountTvPoll(mut env: Env, _: JClass, code: JString) -> jstring {
    kybernow_core::ffi_guard!(null => {
        let code = string(&mut env, &code);
        let s = with::<TvCode>(&code, |c| {
            envelope(account::tv_poll(&c).map(|p| match p {
                TvPoll::Pending => serde_json::json!({ "status": "pending" }),
                TvPoll::Approved(a) => serde_json::json!({ "status": "approved", "account": a }),
            }))
        });
        give(&mut env, &s)
    })
}

/// Minutes avant l'échéance du code (« Le code change dans {n} min. »).
#[unsafe(no_mangle)]
pub extern "system" fn Java_fr_leslignesclaires_kybernow_core_Native_accountTvMinutesLeft(mut env: Env, _: JClass, code: JString) -> jint {
    kybernow_core::ffi_guard!({
        let code = string(&mut env, &code);
        serde_json::from_str::<TvCode>(&code).map(|c| c.minutes_left(account::now_secs()) as jint).unwrap_or(0)
    })
}

/// « Se déconnecter » (l'app oublie le compte de toute façon). Bloquant.
#[unsafe(no_mangle)]
pub extern "system" fn Java_fr_leslignesclaires_kybernow_core_Native_accountSignOut(mut env: Env, _: JClass, account_json: JString) -> jstring {
    kybernow_core::ffi_guard!(null => {
        let a = string(&mut env, &account_json);
        let s = with::<Account>(&a, |a| envelope(account::sign_out(&a)));
        give(&mut env, &s)
    })
}

/// Les réglages « Comptes » : `value` `{key, relay, relay_label, account,
/// device, console}`, sans le jeton.
#[unsafe(no_mangle)]
pub extern "system" fn Java_fr_leslignesclaires_kybernow_core_Native_accountSummary(mut env: Env, _: JClass, account_json: JString) -> jstring {
    kybernow_core::ffi_guard!(null => {
        let a = string(&mut env, &account_json);
        let s = with::<Account>(&a, |a| envelope::<_>(Ok(a.summary())));
        give(&mut env, &s)
    })
}

/// Les ordinateurs du compte : `value` `[{account, relay, station, name,
/// rights, owner, mine, online, admin_fp, attached_at}]`. 70 : oublier le
/// compte. Bloquant.
#[unsafe(no_mangle)]
pub extern "system" fn Java_fr_leslignesclaires_kybernow_core_Native_accountComputers(mut env: Env, _: JClass, account_json: JString) -> jstring {
    kybernow_core::ffi_guard!(null => {
        let a = string(&mut env, &account_json);
        let s = with::<Account>(&a, |a| envelope(account::computers(&a)));
        give(&mut env, &s)
    })
}

/// La liste qui arrive avec les empreintes d'Admin déjà vues : `value`, la liste.
#[unsafe(no_mangle)]
pub extern "system" fn Java_fr_leslignesclaires_kybernow_core_Native_accountRememberFingerprints(mut env: Env, _: JClass, fresh: JString, known: JString) -> jstring {
    kybernow_core::ffi_guard!(null => {
        let (fresh, known) = (string(&mut env, &fresh), string(&mut env, &known));
        let s = with::<Vec<AccountComputer>>(&fresh, |mut fresh| {
            let known = serde_json::from_str::<Vec<AccountComputer>>(&known).unwrap_or_default();
            account::remember_fingerprints(&mut fresh, &known);
            envelope::<_>(Ok(fresh))
        });
        give(&mut env, &s)
    })
}

/// L'accueil, un ordinateur une fois : `paired` `[{id, fingerprint,
/// station}]`, `computers` ceux des comptes ; `value` `{linked: [[id,
/// ordinateur]], added: [ordinateur]}`.
#[unsafe(no_mangle)]
pub extern "system" fn Java_fr_leslignesclaires_kybernow_core_Native_accountMerge(mut env: Env, _: JClass, paired: JString, computers: JString) -> jstring {
    kybernow_core::ffi_guard!(null => {
        let (paired, computers) = (string(&mut env, &paired), string(&mut env, &computers));
        let paired = serde_json::from_str::<Vec<Paired>>(&paired).unwrap_or_default();
        let s = with::<Vec<AccountComputer>>(&computers, |c| envelope::<_>(Ok(account::merge(&paired, &c))));
        give(&mut env, &s)
    })
}

/// La pastille d'origine d'un ordinateur du compte ; `paired` vrai : celle
/// d'un ordinateur appairé (`computer` ignoré).
#[unsafe(no_mangle)]
pub extern "system" fn Java_fr_leslignesclaires_kybernow_core_Native_accountOrigin(mut env: Env, _: JClass, computer: JString, several_relays: jboolean, paired: jboolean) -> jstring {
    kybernow_core::ffi_guard!(null => {
        let text = if paired != JNI_FALSE {
            account::origin_paired(i18n::current())
        } else {
            let computer = string(&mut env, &computer);
            serde_json::from_str::<AccountComputer>(&computer).map(|c| c.origin(i18n::current(), several_relays != JNI_FALSE)).unwrap_or_default()
        };
        give(&mut env, &text)
    })
}

/// La route par le relais d'un ordinateur du compte (`RelayRoute` en JSON),
/// nulle sans empreinte d'Admin connue.
#[unsafe(no_mangle)]
pub extern "system" fn Java_fr_leslignesclaires_kybernow_core_Native_accountRoute(mut env: Env, _: JClass, computer: JString) -> jstring {
    kybernow_core::ffi_guard!(null => {
        let computer = string(&mut env, &computer);
        match serde_json::from_str::<AccountComputer>(&computer).ok().and_then(|c| c.route()) {
            Some(r) => give(&mut env, &serde_json::to_string(&r).unwrap_or_default()),
            None => std::ptr::null_mut(),
        }
    })
}

/// « Regarder » est proposé (droit `view`) ; faux pour un JSON illisible.
#[unsafe(no_mangle)]
pub extern "system" fn Java_fr_leslignesclaires_kybernow_core_Native_accountCanWatch(mut env: Env, _: JClass, computer: JString) -> jboolean {
    kybernow_core::ffi_guard!({
        let computer = string(&mut env, &computer);
        if serde_json::from_str::<AccountComputer>(&computer).is_ok_and(|c| c.can_watch()) { JNI_TRUE } else { JNI_FALSE }
    })
}

/// Le droit `view` sans `session` : « Regarder » à la place de « Se connecter » sur la carte.
#[unsafe(no_mangle)]
pub extern "system" fn Java_fr_leslignesclaires_kybernow_core_Native_accountWatchOnly(mut env: Env, _: JClass, computer: JString) -> jboolean {
    kybernow_core::ffi_guard!({
        let computer = string(&mut env, &computer);
        if serde_json::from_str::<AccountComputer>(&computer).is_ok_and(|c| c.watch_only()) { JNI_TRUE } else { JNI_FALSE }
    })
}

/// La console d'un ordinateur du compte (comme `consoleNewRelay`) : `access`
/// `{account, station}`, `host` et `port` d'Admin sur le réseau (vides sinon),
/// `pinned` l'empreinte d'Admin, `route` par le relais (`accountRoute`), nul
/// sur le réseau. 0 pour un JSON illisible.
#[unsafe(no_mangle)]
pub extern "system" fn Java_fr_leslignesclaires_kybernow_core_Native_consoleNewAccount(
    mut env: Env,
    _: JClass,
    host: JString,
    port: jint,
    pinned: JString,
    access: JString,
    route: JString,
) -> jlong {
    kybernow_core::ffi_guard!({
        let (host, pinned, access) = (string(&mut env, &host), optional(&mut env, &pinned), string(&mut env, &access));
        let route = optional(&mut env, &route).and_then(|r| serde_json::from_str::<RelayRoute>(&r).ok());
        let Ok(params) = serde_json::from_str::<AccessParams>(&access) else { return 0 };
        let console = Console::account(&host, port as u16, account::StationAccess::new(params), pinned).with_relay(route);
        Box::into_raw(Box::new(console)) as jlong
    })
}

// MARK: - Réglages synchronisés (apps/core/README.md, « Réglages synchronisés »)

/// Les réglages synchronisés de tous les ordinateurs du compte : `value`
/// `[{station, updated_at, values}]`, à garder avec le compte. 70 : oublier le
/// compte ; 77 : relais sans réglages synchronisés, garder les réglages
/// locaux sans rien dire. Bloquant.
#[unsafe(no_mangle)]
pub extern "system" fn Java_fr_leslignesclaires_kybernow_core_Native_accountSettingsFetch(mut env: Env, _: JClass, account_json: JString) -> jstring {
    kybernow_core::ffi_guard!(null => {
        let a = string(&mut env, &account_json);
        let s = with::<Account>(&a, |a| envelope(account::fetch_settings(&a)));
        give(&mut env, &s)
    })
}

/// Envoie les réglages d'un ordinateur : `values` les clés changées
/// (`{profile, codec, yuv444, mbps, hdr, immersive}`, facultatives ;
/// `"profile": null` : personnalisé), posées sur `last` (le dernier
/// `{station, updated_at, values}` connu, `null` sinon). `value` : le nouveau
/// `{station, updated_at, values}` à garder. 71 : l'ordinateur n'est plus au
/// compte, oublier ses réglages. Bloquant.
#[unsafe(no_mangle)]
pub extern "system" fn Java_fr_leslignesclaires_kybernow_core_Native_accountSettingsPut(
    mut env: Env,
    _: JClass,
    account_json: JString,
    station: JString,
    last: JString,
    values: JString,
) -> jstring {
    kybernow_core::ffi_guard!(null => {
        let (a, station, values) = (string(&mut env, &account_json), string(&mut env, &station), string(&mut env, &values));
        let last = optional(&mut env, &last).and_then(|l| serde_json::from_str::<SyncedSettings>(&l).ok());
        let s = with::<Account>(&a, |a| with::<SyncedValues>(&values, |v| envelope(account::put_settings(&a, &station, last.as_ref(), &v))));
        give(&mut env, &s)
    })
}

/// Les clés connues d'un `{station, updated_at, values}`, vérifiées : `value`
/// `{profile, codec, yuv444, mbps, hdr, immersive}` (absentes omises,
/// `"profile": null` pour personnalisé).
#[unsafe(no_mangle)]
pub extern "system" fn Java_fr_leslignesclaires_kybernow_core_Native_accountSettingsKnown(mut env: Env, _: JClass, synced: JString) -> jstring {
    kybernow_core::ffi_guard!(null => {
        let synced = string(&mut env, &synced);
        let s = with::<SyncedSettings>(&synced, |s| envelope::<_>(Ok(s.known())));
        give(&mut env, &s)
    })
}

/// Les clés à envoyer d'après les réglages de session de l'app (`settings`,
/// le JSON de `sessionStart`) et son mode immersif (`fullscreen`, `always`,
/// `never` ; `null` : pas envoyé) : `value` `{profile, codec, yuv444, mbps,
/// hdr, immersive}`.
#[unsafe(no_mangle)]
pub extern "system" fn Java_fr_leslignesclaires_kybernow_core_Native_accountSettingsFromStream(mut env: Env, _: JClass, settings: JString, immersive: JString) -> jstring {
    kybernow_core::ffi_guard!(null => {
        let (settings, immersive) = (string(&mut env, &settings), optional(&mut env, &immersive));
        let s = match StreamSettings::from_json(&settings) {
            Ok(st) => envelope::<_>(Ok(SyncedValues::from_stream(&st).with_immersive(immersive.as_deref()))),
            Err(e) => bad(e),
        };
        give(&mut env, &s)
    })
}

/// Les réglages de session de l'app (`base`, comme `settings` de
/// `sessionStart`) avec les clés synchronisées (`values`), ramenés à ce que
/// l'appareil décode (`caps`, `{hevc, hevc444, av1, hdr, pyrowave}`) : `value`,
/// le même JSON que `base`.
#[unsafe(no_mangle)]
pub extern "system" fn Java_fr_leslignesclaires_kybernow_core_Native_accountSettingsApply(mut env: Env, _: JClass, values: JString, base: JString, caps: JString) -> jstring {
    kybernow_core::ffi_guard!(null => {
        let (values, base, caps) = (string(&mut env, &values), string(&mut env, &base), string(&mut env, &caps));
        let caps = serde_json::from_str::<Capabilities>(&caps).unwrap_or_default();
        let s = with::<SyncedValues>(&values, |v| match StreamSettings::from_json(&base) {
            Ok(b) => envelope::<_>(Ok(v.apply_to(&b, caps).to_json())),
            Err(e) => bad(e),
        });
        give(&mut env, &s)
    })
}

/// « Synchroniser les réglages des ordinateurs » et son aide (réglages de
/// l'app, section « Comptes » ; allumé par défaut, gardé sur l'appareil) :
/// `[titre, aide]` en JSON.
#[unsafe(no_mangle)]
pub extern "system" fn Java_fr_leslignesclaires_kybernow_core_Native_accountSyncSettingsTexts(mut env: Env, _: JClass) -> jstring {
    kybernow_core::ffi_guard!(null => {
        let (title, help) = account::sync_settings_texts(i18n::current());
        give(&mut env, &serde_json::json!([title, help]).to_string())
    })
}
