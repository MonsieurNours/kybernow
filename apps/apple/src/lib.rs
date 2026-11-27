//! Pont C du cœur KyberNow pour les apps Apple (voir docs/coeur-apple.md).
//!
//! Tout ce que Swift appelle passe par ici : la console (hors session), la
//! session (préparation, Kyber, image, reconnexion, fin) et la latence ; les
//! entrées et le son dans `input.rs` et `audio.rs`. Les objets sont opaques
//! (`KnConsole`, `KnSession`, `KnInput`, `KnPointer`, `KnAudio`, `KnCombo`), les résultats composés
//! voyagent en JSON, les mesures de chaque seconde et les paquets d'image et
//! de son en structures C plates. Les rappels de `KnPlatform` sont appelés
//! depuis les fils du cœur (fil de session, fil réseau de Kyber pour les
//! paquets) : Swift remonte sur le fil principal ce qui touche l'interface.
//!
//! Chaînes rendues par le cœur : à libérer par `kn_string_free` ; octets
//! (aperçu) : par `kn_bytes_free`.

#![allow(clippy::missing_safety_doc)]

use std::ffi::{c_char, c_void, CStr, CString};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};


use kybernow_core::console::{Console, ConsoleError, DeviceInfo, Mode, PairStatus, StationError};
use kybernow_core::focus::{self, FocusId};
use kybernow_core::i18n::{self, Lang};
use kybernow_core::kynput_rs::Pipeline;
use kybernow_core::latency::{self, Second};
use kybernow_core::media::{Packet, PacketHandler};
use kybernow_core::profile::{self, Capabilities, Codec, HdrLuminance, Profile, StreamSettings};
use kybernow_core::screens::{self, DisplaysChoice, DisplaysSetting, Screens};
use kybernow_core::session::{EndReport, Notice, Platform, Session, SessionParams, Status, StreamScreen};
use kybernow_core::share::{ShareEnd, ShareImage, ShareQuality, ShareRole, ShareState};
use kybernow_core::wake;
use kybernow_core::{Display, WindowHandle};

pub mod account;
pub mod audio;
pub mod autotest;
pub mod camera;
pub mod input;
pub mod invite;
pub mod journal;
pub mod lend;
pub mod micro;
pub mod playstation;
pub mod recording;
pub mod relay;
pub mod rules;
pub mod share;
pub mod welcome;

// MARK: - Chaînes

fn text(p: *const c_char) -> String {
    if p.is_null() {
        return String::new();
    }
    unsafe { CStr::from_ptr(p) }.to_string_lossy().into_owned()
}

fn optional(p: *const c_char) -> Option<String> {
    let s = text(p);
    (!s.is_empty()).then_some(s)
}

fn give(s: String) -> *mut c_char {
    CString::new(s.replace('\0', " ")).map(CString::into_raw).unwrap_or(std::ptr::null_mut())
}

fn give_to(out: *mut *mut c_char, s: String) {
    if !out.is_null() {
        unsafe { *out = give(s) };
    }
}

/// Libère une chaîne rendue par le cœur.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_string_free(s: *mut c_char) {
    kybernow_core::ffi_guard!({
        if !s.is_null() {
            drop(unsafe { CString::from_raw(s) });
        }
    })
}

/// Libère des octets rendus par le cœur (`kn_console_preview`).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_bytes_free(data: *mut u8, len: usize) {
    kybernow_core::ffi_guard!({
        if !data.is_null() {
            drop(unsafe { Vec::from_raw_parts(data, len, len) });
        }
    })
}

// MARK: - Langue et journal

/// Langue de l'app (`fr` ou `en`) : messages du cœur et `Accept-Language`
/// vers la console.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_set_language(tag: *const c_char) {
    kybernow_core::ffi_guard!({
        i18n::set_current(Lang::from_tag(&text(tag)));
    })
}

/// Rappel du journal : niveau (1 erreur, 2 avertissement, 3 information,
/// 4 débogage), cible (module du cœur), message.
pub type KnLogCb = Option<unsafe extern "C" fn(level: u32, target: *const c_char, message: *const c_char)>;

struct Logger;

static LOG_CB: Mutex<KnLogCb> = Mutex::new(None);
static LOGGER: Logger = Logger;

impl log::Log for Logger {
    fn enabled(&self, m: &log::Metadata) -> bool {
        m.level() <= log::Level::Info
    }

    fn log(&self, record: &log::Record) {
        if !self.enabled(record.metadata()) {
            return;
        }
        let Some(cb) = *LOG_CB.lock().unwrap_or_else(|e| e.into_inner()) else { return };
        let target = CString::new(record.target().replace('\0', " ")).unwrap_or_default();
        let message = CString::new(record.args().to_string().replace('\0', " ")).unwrap_or_default();
        unsafe { cb(record.level() as u32, target.as_ptr(), message.as_ptr()) };
    }

    fn flush(&self) {}
}

/// Branche le journal du cœur (à appeler une fois, avant tout le reste).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_set_log_callback(cb: KnLogCb) {
    kybernow_core::ffi_guard!({
        *LOG_CB.lock().unwrap_or_else(|e| e.into_inner()) = cb;
        static ONCE: OnceLock<()> = OnceLock::new();
        ONCE.get_or_init(|| {
            let _ = log::set_logger(&LOGGER);
            log::set_max_level(log::LevelFilter::Info);
        });
    })
}

/// Maintenant, sur l'horloge des mesures de Kyber (µs, CLOCK_MONOTONIC).
#[unsafe(no_mangle)]
pub extern "C" fn kn_now_micros() -> i64 {
    kybernow_core::ffi_guard!({
        latency::now_micros()
    })
}

// MARK: - Console

/// Client de la console d'un ordinateur (hors session).
/// La console est partagée (`Arc`) : le test du profil Auto la garde sur
/// son fil, même si l'app libère la sienne entre-temps.
pub struct KnConsole(Arc<Console>);

/// `pinned` : empreinte attendue (`sha256:…`), ou vide : le premier
/// certificat vu est accepté et lisible par `kn_console_fingerprint_seen`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_console_new(
    host: *const c_char,
    port: u16,
    user: *const c_char,
    password: *const c_char,
    pinned: *const c_char,
) -> *mut KnConsole {
    kybernow_core::ffi_guard!(null => {
        Box::into_raw(Box::new(KnConsole(Arc::new(Console::new(
            &text(host),
            port,
            &text(user),
            &text(password),
            optional(pinned),
        )))))
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_console_free(c: *mut KnConsole) {
    kybernow_core::ffi_guard!({
        if !c.is_null() {
            drop(unsafe { Box::from_raw(c) });
        }
    })
}

/// Empreinte du certificat présenté par la console (nulle avant tout contact).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_console_fingerprint_seen(c: *const KnConsole) -> *mut c_char {
    kybernow_core::ffi_guard!(null => {
        match unsafe { c.as_ref() }.and_then(|c| c.0.fingerprint_seen()) {
            Some(fp) => give(fp),
            None => std::ptr::null_mut(),
        }
    })
}

/// Codes de retour des appels à la console : `KN_OK`, `KN_ERROR` (console
/// injoignable, réponse illisible), `KN_CERTIFICATE_CHANGED`, ou l'opposé
/// du code HTTP d'une réponse d'erreur (−401 : compte refusé, −404 : console
/// d'une version qui ne connaît pas l'appel) ; `error` porte le message.
pub const KN_OK: i32 = 0;
pub const KN_ERROR: i32 = 1;
pub const KN_CERTIFICATE_CHANGED: i32 = 3;
/// Par le relais (docs/relais.md) : le relais ne répond pas, l'ordinateur
/// n'y est pas (éteint, en veille), « Accès hors de chez vous » est coupé
/// dans Admin (403 `remote_access_off`) ; `error` porte le texte à montrer
/// (sans le nom de l'ordinateur : `kn_relay_error_text` le donne avec).
pub const KN_RELAY_UNREACHABLE: i32 = 30;
pub const KN_RELAY_STATION_OFFLINE: i32 = 31;
pub const KN_RELAY_ACCESS_OFF: i32 = 32;

fn failure(e: ConsoleError, error: *mut *mut c_char) -> i32 {
    let code = match &e {
        ConsoleError::Http { status, .. } => -(*status as i32),
        ConsoleError::CertificateChanged { .. } => KN_CERTIFICATE_CHANGED,
        // 30, 31, 32 (`RelayError::code`).
        ConsoleError::Relay(r) => r.code(),
        // Un ordinateur d'un compte KyberNow sans jeton d'accès : 70 et
        // suivants (`account::KN_ACCOUNT_*`).
        ConsoleError::Account(a) => a.code(),
        _ => KN_ERROR,
    };
    give_to(error, e.to_string());
    code
}

fn console_result<T: serde::Serialize>(r: Result<T, ConsoleError>, out: *mut *mut c_char, error: *mut *mut c_char) -> i32 {
    match r {
        Ok(v) => {
            give_to(out, serde_json::to_string(&v).unwrap_or_default());
            KN_OK
        }
        Err(e) => failure(e, error),
    }
}

/// `GET /api/stream/info`, en JSON (`hostname`, `os`, `gpu`, `av1`,
/// `admin_version`, `virtual_display.available`, `audio.speakers`,
/// `station` : `version`, `update` (`available`, `version`, `installable`,
/// `requires_setup`, `installing`) et `restarting` ; nul avec une Station
/// 0.4.9 ou plus ancienne, qui ne sait ni redémarrer ni se mettre à jour
/// depuis un appareil ; et `wake` : les cartes réseau de l'ordinateur pour le
/// réveiller, Ethernet d'abord, chacune avec `mac`, `ipv4`, `broadcast`,
/// `kind` (`ethernet`, `wifi`), `armed` (Windows la laisse réveiller
/// l'ordinateur ; nul : inconnu) et `name`, liste vide avec une Station
/// d'avant la 0.5.0). L'app garde `wake` pour chaque ordinateur, tel quel,
/// quand la liste n'est pas vide, et le rend à `kn_wake`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_console_info(c: *const KnConsole, out: *mut *mut c_char, error: *mut *mut c_char) -> i32 {
    kybernow_core::ffi_guard!(crate::KN_ERROR => {
        let Some(c) = (unsafe { c.as_ref() }) else { return KN_ERROR };
        console_result(c.0.info(), out, error)
    })
}

/// Manettes que voit le lecteur du cœur en ce moment (SDL sur Mac, celles
/// données par l'app ailleurs). Une partie sur console ignore le clavier et
/// la souris dès qu'il y en a une : l'app suit ce nombre, pas GameController (MAC-24).
/// −1 : le lecteur SDL du cœur ne tourne pas encore (Mac, hors partie).
#[unsafe(no_mangle)]
pub extern "C" fn kn_gamepads_count() -> i32 {
    kybernow_core::ffi_guard!(-1 => {
        use kybernow_core::gamepads;
        if gamepads::sdl_compiled() && !gamepads::sdl_running() {
            return -1;
        }
        gamepads::global().pads().len() as i32
    })
}

/// L'ordinateur crée les manettes de l'appareil, d'après `info_json`
/// (`kn_console_info`) : faux quand la Station le refuse (`gamepads.available`,
/// Station Mac sans le droit d'Apple) ; vrai sans le champ ou JSON illisible.
/// docs/link-ui.md, « Manettes indisponibles ».
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_gamepads_available(info_json: *const c_char) -> bool {
    kybernow_core::ffi_guard!({
        serde_json::from_str::<kybernow_core::console::Info>(&text(info_json)).map_or(true, |i| i.gamepads_available())
    })
}

/// Premier contact sans compte : la poignée de main suffit à voir le
/// certificat (`kn_console_fingerprint_seen`), la console répondant 401.
/// `out` : l'info si le compte est accepté, sinon nul.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_console_probe(c: *const KnConsole, out: *mut *mut c_char, error: *mut *mut c_char) -> i32 {
    kybernow_core::ffi_guard!(crate::KN_ERROR => {
        let Some(c) = (unsafe { c.as_ref() }) else { return KN_ERROR };
        match c.0.probe() {
            Ok(Some(i)) => {
                give_to(out, serde_json::to_string(&i).unwrap_or_default());
                KN_OK
            }
            Ok(None) => KN_OK,
            Err(e) => failure(e, error),
        }
    })
}

/// Aperçu JPEG de l'écran de l'ordinateur ; `data` vide si la console ne
/// peut pas capturer (session en cours, écran verrouillé, version plus ancienne).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_console_preview(
    c: *const KnConsole,
    width: u32,
    data: *mut *mut u8,
    len: *mut usize,
    error: *mut *mut c_char,
) -> i32 {
    kybernow_core::ffi_guard!(crate::KN_ERROR => {
        let Some(c) = (unsafe { c.as_ref() }) else { return KN_ERROR };
        match c.0.preview(width) {
            Ok(bytes) => {
                let mut bytes = bytes;
                bytes.shrink_to_fit();
                let n = bytes.len();
                let ptr = if n == 0 { std::ptr::null_mut() } else { Box::leak(bytes.into_boxed_slice()).as_mut_ptr() };
                unsafe {
                    if !data.is_null() {
                        *data = ptr;
                    }
                    if !len.is_null() {
                        *len = n;
                    }
                }
                KN_OK
            }
            Err(e) => failure(e, error),
        }
    })
}

/// Demande d'accès par code à six chiffres ; `device_json` : l'appareil
/// (`id`, `name`, `kind`, `os`, `app`). Rend l'identifiant de la demande.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_console_pair_request(
    c: *const KnConsole,
    code: *const c_char,
    device_json: *const c_char,
    out: *mut *mut c_char,
    error: *mut *mut c_char,
) -> i32 {
    kybernow_core::ffi_guard!(crate::KN_ERROR => {
        let Some(c) = (unsafe { c.as_ref() }) else { return KN_ERROR };
        let device: DeviceInfo = match serde_json::from_str(&text(device_json)) {
            Ok(d) => d,
            Err(e) => {
                give_to(error, e.to_string());
                return KN_ERROR;
            }
        };
        match c.0.pair_request(&text(code), &device) {
            Ok(id) => {
                give_to(out, id);
                KN_OK
            }
            Err(e) => failure(e, error),
        }
    })
}

/// État d'une demande d'appairage, en JSON : `status` (`pending`,
/// `accepted`, `refused`, `expired`) et, accepté, `username` et `password`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_console_pair_status(
    c: *const KnConsole,
    id: *const c_char,
    out: *mut *mut c_char,
    error: *mut *mut c_char,
) -> i32 {
    kybernow_core::ffi_guard!(crate::KN_ERROR => {
        let Some(c) = (unsafe { c.as_ref() }) else { return KN_ERROR };
        match c.0.pair_status(&text(id)) {
            Ok(s) => {
                let v = match s {
                    PairStatus::Pending => serde_json::json!({ "status": "pending" }),
                    PairStatus::Refused => serde_json::json!({ "status": "refused" }),
                    PairStatus::Expired => serde_json::json!({ "status": "expired" }),
                    PairStatus::Accepted { username, password } => {
                        serde_json::json!({ "status": "accepted", "username": username, "password": password })
                    }
                };
                give_to(out, v.to_string());
                KN_OK
            }
            Err(e) => failure(e, error),
        }
    })
}

// MARK: - Station (rubrique « Station » des réglages d'un ordinateur)

/// Refus des appels de la Station (`kn_console_station_*`), en plus des
/// codes des autres appels : un appareil est en session (refaire l'appel avec
/// `force` coupe sa session), trop tôt (un redémarrage par minute, ou trop
/// d'essais ratés), rien à installer depuis un appareil, version à installer
/// avec l'installateur sur l'ordinateur, installation ou redémarrage déjà en
/// cours, Station 0.4.9 ou plus ancienne.
pub const KN_STATION_IN_SESSION: i32 = 10;
pub const KN_STATION_TOO_SOON: i32 = 11;
pub const KN_STATION_NOTHING_TO_INSTALL: i32 = 12;
pub const KN_STATION_REQUIRES_SETUP: i32 = 13;
pub const KN_STATION_BUSY: i32 = 14;
pub const KN_STATION_UNSUPPORTED: i32 = 15;

/// Un refus : son code, `error` (le message), `out` (le détail en JSON :
/// `kind`, `message`, `device`, `seconds`). Le reste comme les autres appels.
fn station_failure(e: StationError, out: *mut *mut c_char, error: *mut *mut c_char) -> i32 {
    let code = match &e {
        StationError::InSession { .. } => KN_STATION_IN_SESSION,
        StationError::TooSoon { .. } => KN_STATION_TOO_SOON,
        StationError::NothingToInstall { .. } => KN_STATION_NOTHING_TO_INSTALL,
        StationError::RequiresSetup { .. } => KN_STATION_REQUIRES_SETUP,
        StationError::Busy { .. } => KN_STATION_BUSY,
        StationError::Unsupported => KN_STATION_UNSUPPORTED,
        StationError::Other(_) => KN_ERROR,
    };
    if let StationError::Other(e) = e {
        return failure(e, error);
    }
    give_to(out, e.to_json().to_string());
    give_to(error, e.to_string());
    code
}

/// Redémarre la Station entière : le serveur Kyber et Admin (environ une
/// minute, Admin ne répond plus pendant ce temps). `device_id` : cet appareil
/// (son nom va au fil d'Admin), ou vide ; `force` : même si un appareil est
/// en session (elle est coupée). `out` : `{"ok": true}`, ou le détail d'un
/// refus (`KN_STATION_*`). Appel bloquant (15 s au plus).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_console_station_restart(
    c: *const KnConsole,
    device_id: *const c_char,
    force: bool,
    out: *mut *mut c_char,
    error: *mut *mut c_char,
) -> i32 {
    kybernow_core::ffi_guard!(crate::KN_ERROR => {
        let Some(c) = (unsafe { c.as_ref() }) else { return KN_ERROR };
        match c.0.station_restart(optional(device_id).as_deref(), force) {
            Ok(()) => {
                give_to(out, serde_json::json!({ "ok": true }).to_string());
                KN_OK
            }
            Err(e) => station_failure(e, out, error),
        }
    })
}

/// Lance la mise à jour de la Station vers la version de `station.update`
/// (comme « Installer maintenant » dans Admin). Mêmes paramètres que
/// `kn_console_station_restart` ; `out` : `{"version": "…"}`, ou le détail
/// d'un refus. Bloque le temps qu'Admin télécharge et vérifie le paquet
/// (quelques minutes au plus) ; la Station s'arrête ensuite et revient dans
/// la nouvelle version, en une minute environ.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_console_station_update(
    c: *const KnConsole,
    device_id: *const c_char,
    force: bool,
    out: *mut *mut c_char,
    error: *mut *mut c_char,
) -> i32 {
    kybernow_core::ffi_guard!(crate::KN_ERROR => {
        let Some(c) = (unsafe { c.as_ref() }) else { return KN_ERROR };
        match c.0.station_update(optional(device_id).as_deref(), force) {
            Ok(version) => {
                give_to(out, serde_json::json!({ "version": version }).to_string());
                KN_OK
            }
            Err(e) => station_failure(e, out, error),
        }
    })
}

/// Demande à la Station de vérifier ses mises à jour tout de suite
/// (`POST /api/stream/station/check`, docs/link-ui.md « Mise à jour des
/// Stations ») : `out`, le `station` qu'elle annonce ensuite (comme dans
/// `kn_console_info`), ou nul pour une Station antérieure à la 0.11.3, qui ne
/// connaît pas la route (on se fie alors à son annonce). Appel bloquant
/// (20 s au plus), hors du fil principal.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_console_station_check(c: *const KnConsole, out: *mut *mut c_char, error: *mut *mut c_char) -> i32 {
    kybernow_core::ffi_guard!(crate::KN_ERROR => {
        let Some(c) = (unsafe { c.as_ref() }) else { return KN_ERROR };
        match c.0.station_check() {
            Ok(Some(station)) => {
                give_to(out, serde_json::to_string(&station).unwrap_or_default());
                KN_OK
            }
            Ok(None) => KN_OK,
            Err(e) => failure(e, error),
        }
    })
}

/// Mise à jour des Stations proposée par l'app (`station_offer::evaluate`) :
/// `input_json`, ce que l'app sait de l'ordinateur (`station` de
/// `kn_console_info`, `may_update`, `online`, `away`, `in_session`, `now` en
/// secondes Unix), sa mémoire pour cet ordinateur (`memory` :
/// `snoozed_version`, `snoozed_at`, `checked_app_version`, `checked_at`) et
/// `app_version`. Rend `{"banner": {"kind": "none" | "update" | "installing"
/// | "setup" | "away", "version"?}, "chip", "notice_version", "should_check"}`
/// ; une entrée illisible donne « rien à montrer ». À libérer par
/// `kn_string_free`. Calcul local, sans réseau.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_station_offer_evaluate(input_json: *const c_char) -> *mut c_char {
    kybernow_core::ffi_guard!(null => {
        give(kybernow_core::station_offer::evaluate_json(&text(input_json)))
    })
}

/// Premier lancement après une mise à jour de l'app
/// (`station_offer::app_updated`) : `last_launched`, la dernière version
/// lancée que l'app a gardée (vide au tout premier lancement : faux),
/// `current`, la sienne. L'avis « {nom} a aussi une mise à jour » peut alors
/// s'afficher.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_station_offer_app_updated(last_launched: *const c_char, current: *const c_char) -> bool {
    kybernow_core::ffi_guard!({
        kybernow_core::station_offer::app_updated(optional(last_launched).as_deref(), &text(current))
    })
}

/// Adresse d'Admin à ouvrir dans le navigateur (`https://adresse:port/`,
/// IPv6 entre crochets) ; à libérer par `kn_string_free`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_console_admin_url(c: *const KnConsole) -> *mut c_char {
    kybernow_core::ffi_guard!(null => {
        match unsafe { c.as_ref() } {
            Some(c) => give(c.0.admin_url()),
            None => std::ptr::null_mut(),
        }
    })
}

// MARK: - Réveil par le réseau

/// Aucune cible lisible dans `kn_wake` (ni adresse MAC) : l'app ne connaît
/// pas encore la carte réseau de l'ordinateur.
pub const KN_WAKE_NO_TARGET: i32 = 20;

/// Réveille un ordinateur (Wake-on-LAN) : `targets_json`, les cartes gardées
/// par l'app pour cet ordinateur (`wake` de `kn_console_info`, une liste ; une
/// cible seule ou l'info entière passent aussi). Le paquet magique part vers
/// la diffusion du sous-réseau de chaque carte, vers 255.255.255.255 et vers
/// sa dernière adresse IPv4, en direct. Rend `KN_OK` dès qu'un paquet est
/// parti, `KN_WAKE_NO_TARGET` sans cible lisible, `KN_ERROR` si aucun n'est
/// parti. Sur iPhone, iPad et Apple TV, la diffusion demande l'autorisation
/// `com.apple.developer.networking.multicast` (sinon refusée, souvent
/// `EHOSTUNREACH`) : l'envoi direct part quand même, et l'appel réussit.
/// Rapide, mais hors du fil principal comme tout envoi réseau.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_wake(targets_json: *const c_char) -> i32 {
    kybernow_core::ffi_guard!(crate::KN_ERROR => {
        match wake::send(&wake::targets_from_json(&text(targets_json))) {
            Ok(_) => KN_OK,
            Err(wake::WakeError::NoTarget) => KN_WAKE_NO_TARGET,
            Err(e) => {
                log::warn!("réveil : {e}");
                KN_ERROR
            }
        }
    })
}

// MARK: - Paquets d'image et de son

/// Début d'un flux : son format.
pub const KN_PACKET_CODEC: u32 = 0;
/// Données : jeu de paramètres (`is_config`) ou image / trame Opus.
pub const KN_PACKET_MEDIA: u32 = 1;
/// Perte : pour le son, `missing_samples` échantillons à masquer.
pub const KN_PACKET_HOLE: u32 = 2;
/// Dernier appel pour ce flux : plus rien ne viendra, l'objet peut être rendu.
pub const KN_PACKET_CLOSED: u32 = 3;

/// Un paquet livré par Kyber (fil réseau : ne jamais bloquer, recopier ce
/// qui doit l'être). `stream` numérote les flux d'une session : un nouveau
/// numéro, un nouveau décodeur.
#[repr(C)]
pub struct KnPacket {
    pub kind: u32,
    pub stream: u64,
    /// FourCC : `h264`, `h265` (HEVC), `av1` (AV1, 0x00617631), `opus`.
    pub codec: u32,
    pub is_config: bool,
    pub is_key: bool,
    pub pts: u64,
    pub data: *const u8,
    pub len: usize,
    pub missing_samples: u32,
    /// Image de reprise après une perte (docs/reprise-apres-perte.md) : elle
    /// ne dépend que d'images jusqu'à ce pts. `u64::MAX` : une image comme
    /// une autre.
    pub resume_bound: u64,
    /// Multi-écrans (`screens` dans `kn_session_start`) : index de l'écran de
    /// l'appareil auquel va ce flux d'image (dans `screens.list`) ; 0 sinon,
    /// et pour le son.
    pub screen: u32,
}

pub type KnPacketCb = Option<unsafe extern "C" fn(userdata: *mut c_void, packet: *const KnPacket)>;

/// Un flux (image ou son) : le rappel de Swift, retenu par Kyber jusqu'au
/// dernier paquet ; sa libération signale `KN_PACKET_CLOSED`.
struct Sink {
    cb: unsafe extern "C" fn(*mut c_void, *const KnPacket),
    userdata: *mut c_void,
    stream: u64,
    screen: u32,
    /// Le `userdata` reste à la plateforme tant qu'un flux peut rappeler (MAC-01).
    _owner: Arc<Owner>,
}

/// Le `userdata` d'une plateforme (l'objet Swift, retenu par `passRetained`),
/// partagé par tous ceux qui peuvent le rappeler : la plateforme de la
/// session, chaque flux que Kyber garde. Le dernier rend l'objet par
/// `release_userdata` (MAC-01 : Kyber gardait un flux plus de 30 s après la
/// fin, et son `Drop` rappelait un objet déjà libéré). Sans `release_userdata`
/// (une app d'avant), rien n'est rendu : l'app libère elle-même, comme avant.
pub(crate) struct Owner {
    userdata: *mut c_void,
    release: Option<unsafe extern "C" fn(userdata: *mut c_void)>,
}

unsafe impl Send for Owner {}
unsafe impl Sync for Owner {}

impl Owner {
    pub(crate) fn new(userdata: *mut c_void, release: Option<unsafe extern "C" fn(userdata: *mut c_void)>) -> Arc<Self> {
        Arc::new(Self { userdata, release })
    }
}

impl Drop for Owner {
    fn drop(&mut self) {
        if let Some(release) = self.release {
            unsafe { release(self.userdata) };
        }
    }
}

unsafe impl Send for Sink {}
unsafe impl Sync for Sink {}

impl Sink {
    fn empty(&self, kind: u32) -> KnPacket {
        KnPacket {
            kind,
            stream: self.stream,
            codec: 0,
            is_config: false,
            is_key: false,
            pts: 0,
            data: std::ptr::null(),
            len: 0,
            missing_samples: 0,
            resume_bound: u64::MAX,
            screen: self.screen,
        }
    }
}

impl PacketHandler for Sink {
    fn packet(&self, packet: Packet<'_>) {
        let p = match packet {
            Packet::Codec { codec } => KnPacket { codec, ..self.empty(KN_PACKET_CODEC) },
            Packet::Media { is_config, is_key, pts, data, resume } => KnPacket {
                is_config,
                is_key,
                pts,
                data: data.as_ptr(),
                len: data.len(),
                resume_bound: resume.unwrap_or(u64::MAX),
                ..self.empty(KN_PACKET_MEDIA)
            },
            Packet::Hole { missing_samples } => KnPacket { missing_samples, ..self.empty(KN_PACKET_HOLE) },
        };
        unsafe { (self.cb)(self.userdata, &p) };
    }
}

impl Drop for Sink {
    fn drop(&mut self) {
        let p = self.empty(KN_PACKET_CLOSED);
        unsafe { (self.cb)(self.userdata, &p) };
    }
}

// MARK: - Session : ce que la plateforme reçoit

#[repr(C)]
#[derive(Clone, Copy)]
pub struct KnDisplay {
    pub id: u32,
    pub width: i32,
    pub height: i32,
    pub x: i32,
    pub y: i32,
}

impl From<&Display> for KnDisplay {
    fn from(d: &Display) -> Self {
        Self { id: d.id, width: d.width, height: d.height, x: d.x, y: d.y }
    }
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct KnMode {
    pub width: u32,
    pub height: u32,
    pub refresh: u32,
}

impl From<Mode> for KnMode {
    fn from(m: Mode) -> Self {
        Self { width: m.width, height: m.height, refresh: m.refresh }
    }
}

/// Multi-écrans : un écran de l'ordinateur pris par la session (`on_screens`).
#[repr(C)]
#[derive(Clone, Copy)]
pub struct KnStreamScreen {
    /// Index de l'écran de l'appareil (dans `screens.list` de `kn_session_start`).
    pub screen: u32,
    /// L'écran de l'ordinateur : `id` (pour `kn_input_set_screens`), taille
    /// en pixels (repère des positions de cet écran), position sur le bureau.
    pub display: KnDisplay,
    /// Fréquence de l'écran virtuel.
    pub refresh: u32,
    /// L'écran principal de Windows (toujours le premier).
    pub primary: bool,
}

impl From<&StreamScreen> for KnStreamScreen {
    fn from(s: &StreamScreen) -> Self {
        Self { screen: s.screen as u32, display: KnDisplay::from(&s.display), refresh: s.refresh, primary: s.primary }
    }
}

pub const KN_STATUS_PREPARING: u32 = 0;
pub const KN_STATUS_CONNECTING: u32 = 1;
pub const KN_STATUS_FIRST_IMAGES: u32 = 2;
pub const KN_STATUS_STREAMING: u32 = 3;
pub const KN_STATUS_RECONNECTING: u32 = 4;
pub const KN_STATUS_ENDING: u32 = 5;
pub const KN_STATUS_ENDED: u32 = 6;

/// État de la session. Fin en cours (`KN_STATUS_ENDING`) et terminée
/// (`KN_STATUS_ENDED`) : `error` dit pourquoi si la fin n'était pas voulue
/// (la même raison les deux fois ; dès `KN_STATUS_ENDING`, l'app sait que
/// c'est un échec sans attendre la réponse d'Admin) ; nul sinon, et pour les
/// autres états. Terminée : `has_report` et les champs suivants, ce que la
/// console a confirmé (écrans et son rétablis, ou son erreur).
#[repr(C)]
pub struct KnStatus {
    pub kind: u32,
    pub attempt: u32,
    pub max: u32,
    pub error: *const c_char,
    pub has_report: bool,
    pub display_restored: bool,
    pub audio_restored: bool,
    pub report_error: *const c_char,
}

/// Mesures de la seconde écoulée (voir `latency::Second`).
#[repr(C)]
#[derive(Default)]
pub struct KnSecond {
    pub has_latency: bool,
    pub latency_ms: f64,
    pub has_chain: bool,
    /// Capture → encodage, encodage, envoi, réseau, décodage, affichage (ms).
    pub chain: [f64; 6],
    pub displayed: u32,
    pub measured: u32,
    pub rejected: u32,
    pub lost_frames: u32,
    pub holes: u32,
    pub keyframe_requests: u32,
    pub lost_packets: u32,
    pub has_rtt: bool,
    pub rtt_ms: f64,
    pub bytes: u64,
    /// Un écran au moins de l'image en cours est en HDR (statistiques :
    /// « HDR » après le format).
    pub hdr: bool,
    /// PyroWave (docs/pyrowave.md) : définition ramenée en place
    /// (Statistiques, ligne « Définition ramenée ») : `lowered_width` ×
    /// `lowered_height` à `lowered_refresh` Hz pour tenir dans `lowered_mbps`.
    pub lowered: bool,
    pub lowered_width: u32,
    pub lowered_height: u32,
    pub lowered_refresh: u32,
    pub lowered_mbps: u32,
    /// Débit visé (Statistiques, ligne « Débit visé » et trait sur la courbe
    /// du débit ; docs/link-ui.md) : `target_bitrate` demandé à l'ordinateur
    /// et `target_profile` du profil (ou réglé à la main), en bit/s ;
    /// `target_reason` : `KN_TARGET_*`. Texte : `kn_target_bitrate_text`.
    /// Faux avant l'image ou pour un invité sur l'image de l'hôte.
    pub has_target: bool,
    pub target_bitrate: u32,
    pub target_profile: u32,
    pub target_reason: u32,
}

/// Débit visé : débit adaptatif allumé, au débit de base (« 60 Mbit/s · adaptatif »).
pub const KN_TARGET_AUTO: u32 = 0;
/// Baissé par le débit automatique (« 42 / 60 Mbit/s · baissé, réseau saturé »).
pub const KN_TARGET_LOWERED: u32 = 1;
/// Débit adaptatif coupé : le débit reste fixe (« 80 Mbit/s · fixe »).
pub const KN_TARGET_MANUAL: u32 = 2;
/// PyroWave, débit adaptatif en pause (« 320 Mbit/s · PyroWave, adaptatif en pause »).
pub const KN_TARGET_PYROWAVE_PAUSED: u32 = 3;
/// Plafonné par l'app, `kn_session_set_bitrate_cap` (« 20 Mbit/s · fenêtre cachée »).
pub const KN_TARGET_APP_CAPPED: u32 = 4;
/// Flux allégé après un repli (« 15 Mbit/s · flux allégé »).
pub const KN_TARGET_FALLBACK: u32 = 5;

/// La ligne « Débit visé » des Statistiques, dans la langue de l'app, pour
/// `target_bitrate`, `target_profile` et `target_reason` de `KnSecond`. Nul
/// pour une raison inconnue. À libérer par `kn_string_free`.
#[unsafe(no_mangle)]
pub extern "C" fn kn_target_bitrate_text(bitrate: u32, profile: u32, reason: u32) -> *mut c_char {
    kybernow_core::ffi_guard!(null => {
        match kybernow_core::adaptive::TargetReason::from_code(reason) {
            Some(reason) => give(kybernow_core::adaptive::TargetBitrate { bitrate, profile, reason }.text(i18n::current())),
            None => std::ptr::null_mut(),
        }
    })
}

/// Le libellé de la ligne : « Débit visé » / “Target bitrate”. À libérer
/// par `kn_string_free`.
#[unsafe(no_mangle)]
pub extern "C" fn kn_target_bitrate_label() -> *mut c_char {
    kybernow_core::ffi_guard!(null => {
        give(kybernow_core::adaptive::TargetBitrate::label(i18n::current()))
    })
}

impl From<&Second> for KnSecond {
    fn from(s: &Second) -> Self {
        Self {
            has_latency: s.latency_ms.is_some(),
            latency_ms: s.latency_ms.unwrap_or(0.0),
            has_chain: s.chain.is_some(),
            chain: s.chain.map(|c| c.all()).unwrap_or([0.0; 6]),
            displayed: s.displayed,
            measured: s.measured,
            rejected: s.rejected,
            lost_frames: s.lost_frames,
            holes: s.holes,
            keyframe_requests: s.keyframe_requests,
            lost_packets: s.lost_packets,
            has_rtt: s.rtt_ms.is_some(),
            rtt_ms: s.rtt_ms.unwrap_or(0.0),
            bytes: s.bytes,
            hdr: s.hdr,
            lowered: s.lowered.is_some(),
            lowered_width: s.lowered.map_or(0, |l| l.width),
            lowered_height: s.lowered.map_or(0, |l| l.height),
            lowered_refresh: s.lowered.map_or(0, |l| l.refresh),
            lowered_mbps: s.lowered.map_or(0, |l| l.mbps),
            has_target: s.target.is_some(),
            target_bitrate: s.target.map_or(0, |t| t.bitrate),
            target_profile: s.target.map_or(0, |t| t.profile),
            target_reason: s.target.map_or(0, |t| t.reason.code()),
        }
    }
}

/// « L'image a été relancée. »
pub const KN_NOTICE_RELAUNCHED: u32 = 0;
/// L'écran affiché a changé de taille sur l'ordinateur (`width`, `height` ;
/// `back` : revenu au mode du départ).
pub const KN_NOTICE_MODE_CHANGED: u32 = 1;
/// Flux allégé pour cette session (`level` 1 : débit ramené à
/// `bitrate_mbps` ; 2 : 4:2:0).
pub const KN_NOTICE_FALLBACK: u32 = 2;
/// Adaptation à l'écran de l'appareil demandée (`width`, `height`).
pub const KN_NOTICE_ADAPTING: u32 = 3;
pub const KN_NOTICE_ADAPTED: u32 = 4;
/// Adaptation refusée (`error`).
pub const KN_NOTICE_ADAPT_FAILED: u32 = 5;
/// « Remettre W × H » à proposer (`has_mode`) ou à retirer.
pub const KN_NOTICE_RESTORE_OFFER: u32 = 6;
/// Dernier recours de la connexion : l'image n'arrivait pas, ou Kyber ne
/// répondait pas, et la Station redémarre (une fois, sans couper la session
/// d'un autre appareil). L'écran de connexion affiche « La Station
/// redémarre… » jusqu'au prochain `on_status` (`KN_STATUS_PREPARING` si elle
/// revient, `KN_STATUS_ENDING` puis `KN_STATUS_ENDED` sinon). Toujours juste
/// après un `KN_STATUS_CONNECTING`, l'étape que montre une app qui ne connaît
/// pas cet avis (pas de nouvel état : les apps prennent un état inconnu pour
/// la fin de session).
pub const KN_NOTICE_STATION_RESTARTING: u32 = 7;
/// HDR (docs/hdr.md) : Windows a refusé le HDR d'un écran virtuel, l'image
/// de cet écran est en SDR ; une fois par session. `error` : le texte à
/// montrer, « L'ordinateur n'a pas pu passer en HDR : image en SDR. » (langue
/// de l'app).
pub const KN_NOTICE_HDR_REFUSED: u32 = 8;
/// PyroWave (docs/pyrowave.md, « Définition ramenée ») : `has_mode` vrai,
/// la définition demandée à l'ordinateur est ramenée à `width` × `height`
/// pour tenir dans `bitrate_mbps` ; `error` : le texte du bandeau,
/// « Définition ramenée à {l} × {h} pour tenir dans {n} Mbit/s. » (langue de
/// l'app). `has_mode` faux : revenue à celle de l'appareil, bandeau retiré.
pub const KN_NOTICE_RESOLUTION_LOWERED: u32 = 9;
/// PyroWave (docs/debit-automatique.md) : le réseau sature depuis 5 s
/// d'affilée ; bandeau « Le réseau sature » avec « Passer en HEVC », comme
/// celui du début de session. Une fois par session, pour l'hôte seulement.
pub const KN_NOTICE_PYROWAVE_STRAINED: u32 = 10;
/// L'encodeur de l'ordinateur a refusé l'image au départ : le débit a dû
/// redescendre jusqu'à `bitrate_mbps` (Mbit/s par écran). L'app le garde
/// comme plafond de cet ordinateur (`encoder_max` des réglages de session,
/// docs/profil-auto.md, « L'encodeur de l'ordinateur ») ; rien à l'écran.
pub const KN_NOTICE_ENCODER_CEILING: u32 = 11;

#[repr(C)]
pub struct KnNotice {
    pub kind: u32,
    pub width: u32,
    pub height: u32,
    pub back: bool,
    pub level: u8,
    pub bitrate_mbps: u32,
    pub has_mode: bool,
    pub error: *const c_char,
}

/// Ce que la plateforme fournit et reçoit. Les rappels nuls sont ignorés.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct KnPlatform {
    pub userdata: *mut c_void,
    /// `NSView *` (macOS) ou `UIView *` (iOS, tvOS) de l'image, pour Kyber.
    pub window_handle: u64,
    /// Le son est décodé par l'appareil (`audio_packet` appelé).
    pub has_audio: bool,
    /// Rend la chaîne d'entrées kynput de l'image qui démarre
    /// (`kynput_Pipeline *` avec une référence prise : `kynput_pipeline_ref`).
    pub input_pipeline: Option<unsafe extern "C" fn(userdata: *mut c_void) -> *mut c_void>,
    pub video_packet: KnPacketCb,
    pub audio_packet: KnPacketCb,
    pub on_status: Option<unsafe extern "C" fn(userdata: *mut c_void, status: *const KnStatus)>,
    /// L'écran de l'ordinateur affiché (avant `input_pipeline`), aussi quand sa taille change.
    pub on_display: Option<unsafe extern "C" fn(userdata: *mut c_void, display: KnDisplay)>,
    /// Les écrans annoncés par Kyber.
    pub on_displays: Option<unsafe extern "C" fn(userdata: *mut c_void, displays: *const KnDisplay, count: usize)>,
    /// Mode de l'écran de la session (préparé, adapté, changé par l'ordinateur),
    /// et si c'est un écran virtuel.
    pub on_mode: Option<unsafe extern "C" fn(userdata: *mut c_void, mode: KnMode, virtual_display: bool)>,
    /// L'image vient d'être demandée à Kyber (presse-papiers à démarrer, manette à rendre ou garder).
    pub on_stream_started: Option<unsafe extern "C" fn(userdata: *mut c_void)>,
    /// L'image vient d'être arrêtée (relance, fin) : chaîne d'entrées à arrêter.
    pub on_stream_stopped: Option<unsafe extern "C" fn(userdata: *mut c_void)>,
    /// Entrées reliées à l'ordinateur (début, reconnexion, relance).
    pub on_input_connected: Option<unsafe extern "C" fn(userdata: *mut c_void)>,
    pub on_second: Option<unsafe extern "C" fn(userdata: *mut c_void, second: *const KnSecond)>,
    /// Un jeu est en plein écran sur l'ordinateur (souris Auto).
    pub on_game_fullscreen: Option<unsafe extern "C" fn(userdata: *mut c_void, game: bool)>,
    pub on_notice: Option<unsafe extern "C" fn(userdata: *mut c_void, notice: *const KnNotice)>,
    /// Multi-écrans : les écrans pris par la session, principal en premier
    /// (après `on_display`, avant `input_pipeline`, à chaque démarrage de
    /// l'image ; et quand la taille de l'un d'eux change), et les écrans de
    /// l'appareil que l'ordinateur n'a pas pu créer (`not_created`, index
    /// dans `screens.list` : « L'ordinateur n'a pas pu créer cet écran. »).
    /// Un écran du plan dans aucune des deux listes : la Station ne gère
    /// qu'un écran, la session se fait sur l'écran principal seul.
    pub on_screens: Option<
        unsafe extern "C" fn(userdata: *mut c_void, screens: *const KnStreamScreen, count: usize, not_created: *const u32, not_created_count: usize),
    >,
    /// KyberNow Share, session d'hôte : le partage ouvert et ses invités en
    /// JSON (`open`, `code`, `link`, `web`, `display` {`id`, `label`},
    /// `guests` [{`id`, `name`, `app`, `role`, `since`}]), nul sans partage ;
    /// quand cela change (signe de vie).
    pub on_guests: Option<unsafe extern "C" fn(userdata: *mut c_void, share_json: *const c_char)>,
    /// KyberNow Share, session d'invité : son rôle (`KN_SHARE_ROLE_*`) au
    /// départ (`previous` = −1), puis quand l'hôte le change.
    pub on_share_role: Option<unsafe extern "C" fn(userdata: *mut c_void, role: i32, previous: i32)>,
    /// KyberNow Share, session d'invité finie par le partage
    /// (`KN_SHARE_END_*`), juste avant `KN_STATUS_ENDING` ; `text` : la raison
    /// à montrer (la même que l'`error` de la fin), jamais « Reprendre la main ».
    pub on_share_end: Option<unsafe extern "C" fn(userdata: *mut c_void, end: i32, text: *const c_char)>,
    /// KyberNow Share, session d'invité : l'image qu'il reçoit, en JSON
    /// (`quality`, `width`, `height`, `fps`, `codec`, `follows_host`,
    /// `reason` : `asked`, `capped`, `limit`, `lag`), au départ si la
    /// Station la dit déjà et quand elle change ; `previous_json` : celle
    /// d'avant (nul au départ) ; `notice` : l'avis bref à montrer (« Image
    /// allégée pour suivre. », « L'ordinateur ne peut pas faire une image de
    /// plus : vous recevez {qualité}. »), nul sinon. La barre montre
    /// `kn_share_image_label`.
    pub on_share_image: Option<unsafe extern "C" fn(userdata: *mut c_void, image_json: *const c_char, previous_json: *const c_char, notice: *const c_char)>,
    /// Par où passe la session (docs/relais.md), en JSON : `transport`
    /// (`direct`, `relay`, `relay_tcp`), `relay`, `rtt_client`, `rtt_station`
    /// (allers-retours en ms, nuls s'ils ne sont pas sus) ; au début, puis quand il change (toutes les 5 s au
    /// plus). Puce « Par le relais », ligne « Chemin » (`kn_relay_path_label`),
    /// `route` de la « Dernière session ».
    pub on_route: Option<unsafe extern "C" fn(userdata: *mut c_void, route_json: *const c_char)>,
    /// Par le relais : la session finit parce que l'ordinateur n'est pas
    /// joignable hors de chez soi, juste avant l'état `KN_STATUS_ENDING` ;
    /// `code` : `KN_RELAY_ACCESS_OFF` (accès coupé dans Admin),
    /// `KN_RELAY_STATION_OFFLINE` (éteint ou en veille),
    /// `KN_RELAY_UNREACHABLE` (relais muet) ; `text` : le texte de link-ui,
    /// qui est aussi la raison de la fin, avec le nom de l'ordinateur donné
    /// à `kn_focus_register` (sans nom sinon ; `kn_relay_error_text` le
    /// refait avec un nom). Choisir l'état de l'accueil par `code`, jamais
    /// par le texte.
    pub on_relay_error: Option<unsafe extern "C" fn(userdata: *mut c_void, code: i32, text: *const c_char)>,
    /// Session d'emprunt (docs/pret.md) : le préavis de 2 minutes, quand il
    /// arrive ; `notice_json` `{reason, at}` (`reason` : `taken_back`,
    /// `time_up`, `slot_end` ; `at` : l'heure de la fin, secondes Unix) ;
    /// `text` : l'avis de link-ui (« {hôte} reprend l'ordinateur dans 2 min :
    /// sauvegardez votre partie. »).
    pub on_lend_notice: Option<unsafe extern "C" fn(userdata: *mut c_void, notice_json: *const c_char, text: *const c_char)>,
    /// Session d'emprunt : le prêt finit la session, juste avant
    /// `KN_STATUS_ENDING` ; `end` : `KN_LEND_END_*` ; `text` : la raison de
    /// link-ui ; `KN_LEND_END_LEFT` (rendu depuis le lanceur) est une fin
    /// voulue, sans erreur, avec « Vous avez rendu l'ordinateur. » ; rendu
    /// par `kn_session_give_back`, ce rappel ne vient pas. Une autre fin
    /// d'emprunt sans ce rappel (fenêtre fermée, réseau perdu) laisse
    /// revenir pendant 15 min (`kn_lend_borrow` de nouveau).
    pub on_lend_end: Option<unsafe extern "C" fn(userdata: *mut c_void, end: i32, text: *const c_char)>,
    /// Session d'hôte : la vue du prêt de l'ordinateur (comme
    /// `kn_console_lend_state`), quand elle change ; nulle quand il n'y a
    /// rien à dire (ni code, ni demande, ni prêt).
    pub on_lend: Option<unsafe extern "C" fn(userdata: *mut c_void, lend_json: *const c_char)>,
    /// HDR (docs/hdr.md) : à chaque démarrage de l'image (après `on_display`
    /// et `on_screens`, avant `input_pipeline`), quels écrans sont en HDR,
    /// `count` booléens dans l'ordre des écrans du flux (`on_screens`,
    /// principal en premier ; un seul pour une session à un écran). Un écran
    /// en HDR se rend en PQ / BT.2020 (`CAMetalLayer` en `rgba16Float`,
    /// `itur_2100_PQ`), les autres en SDR.
    pub on_hdr: Option<unsafe extern "C" fn(userdata: *mut c_void, hdr: *const bool, count: usize)>,
    /// L'enregistrement de la session (docs/enregistrement.md), hôte comme
    /// invité, quand il change (jamais pour le temps qui passe) :
    /// `recording_json` comme `kn_console_recording` (`available`,
    /// `allowed`, `active`, `elapsed`, `texts` avec `{time}` à remplir par
    /// `kn_recording_clock` chaque seconde et `{shortcut}`) ; `notice` :
    /// l'avis bref de link-ui à montrer, nul s'il n'y a rien à dire. Une
    /// Station qui n'enregistre pas n'appelle jamais.
    pub on_recording: Option<unsafe extern "C" fn(userdata: *mut c_void, recording_json: *const c_char, notice: *const c_char)>,
    /// Micro de l'appareil (docs/micro.md) : à chaque changement d'état, en
    /// JSON (`MicState::to_json` du cœur) : `{"state":"off"}`,
    /// `{"state":"waiting"}` (allumé, réponse pas encore venue), `{"state":"on"}`,
    /// `{"state":"refused","reason":"no_device","text":"…"}` (`reason` :
    /// `not_allowed`, `no_device`, `unsupported`, `old_station` ; `text` :
    /// la raison de link-ui, dans la langue de l'app). L'app capte tant que
    /// l'état est `on` ou `waiting` (`kn_session_push_microphone`), sinon elle
    /// arrête la capture. Jamais appelé sans `kn_set_device_has_microphone(true)`.
    pub on_microphone: Option<unsafe extern "C" fn(userdata: *mut c_void, state_json: *const c_char)>,
    /// Caméra de l'appareil (docs/webcam.md) : l'app sait la capturer et
    /// l'encoder ; le cœur la demande à l'ordinateur à chaque départ de
    /// l'image (jamais pour un invité d'un partage).
    pub sends_camera: bool,
    /// L'ordinateur propose la caméra pour cette image, ou plus : l'entrée
    /// « Caméra » apparaît ou disparaît ; allumée, la caméra repart d'elle-même
    /// quand le flux revient (`kn_camera_*`).
    pub on_camera_offer: Option<unsafe extern "C" fn(userdata: *mut c_void, offered: bool)>,
    /// Rend `userdata` quand plus rien ne peut le rappeler (la session finie
    /// et le dernier flux lâché par Kyber), d'un fil quelconque ; nul : l'app
    /// le garde et le rend elle-même. Ajouté en dernier.
    pub release_userdata: Option<unsafe extern "C" fn(userdata: *mut c_void)>,
    /// PyroWave n'a pas pu démarrer sur l'appareil (après
    /// `kn_session_pyrowave_failed`) : l'image repart en `codec` (`hevc`,
    /// `av1`, `h264`) pour cette session, le réglage ne change pas ; `text` :
    /// l'avis bref de link-ui (« Image en HEVC : PyroWave n'a pas pu démarrer
    /// sur cet appareil. »), à montrer une fois. Ajouté après `release_userdata`.
    pub on_pyrowave_fallback: Option<unsafe extern "C" fn(userdata: *mut c_void, codec: *const c_char, text: *const c_char)>,
}

struct ApplePlatform {
    cb: KnPlatform,
    owner: Arc<Owner>,
    streams: AtomicU64,
    /// Raison d'un échec, donnée juste avant `Status::Ending` (`on_ending`).
    ending_error: Mutex<Option<String>>,
}

unsafe impl Send for ApplePlatform {}
unsafe impl Sync for ApplePlatform {}

impl ApplePlatform {
    fn sink(&self, cb: KnPacketCb, screen: u32) -> Option<Arc<dyn PacketHandler>> {
        let cb = cb?;
        let stream = self.streams.fetch_add(1, Ordering::Relaxed) + 1;
        Some(Arc::new(Sink { cb, userdata: self.cb.userdata, stream, screen, _owner: self.owner.clone() }))
    }
}

/// Rappel sans paquet : une plateforme sans image ne reçoit rien.
struct Mute;

impl PacketHandler for Mute {
    fn packet(&self, _: Packet<'_>) {}
}

impl Platform for ApplePlatform {
    fn window_handle(&self) -> WindowHandle {
        #[cfg(target_os = "macos")]
        {
            WindowHandle::AppKit(self.cb.window_handle)
        }
        #[cfg(not(target_os = "macos"))]
        {
            WindowHandle::UiKit(self.cb.window_handle)
        }
    }

    fn video_handler(&self) -> Arc<dyn PacketHandler> {
        self.sink(self.cb.video_packet, 0).unwrap_or_else(|| Arc::new(Mute))
    }

    fn screen_video_handler(&self, screen: usize) -> Arc<dyn PacketHandler> {
        self.sink(self.cb.video_packet, screen as u32).unwrap_or_else(|| Arc::new(Mute))
    }

    fn on_guests(&self, share: Option<&ShareState>) {
        let Some(f) = self.cb.on_guests else { return };
        let json = share.map(|s| CString::new(serde_json::to_string(s).unwrap_or_default().replace('\0', " ")).unwrap_or_default());
        unsafe { f(self.cb.userdata, json.as_ref().map_or(std::ptr::null(), |c| c.as_ptr())) };
    }

    fn on_share_role(&self, role: ShareRole, previous: Option<ShareRole>) {
        if let Some(f) = self.cb.on_share_role {
            unsafe { f(self.cb.userdata, role.code(), previous.map_or(-1, ShareRole::code)) };
        }
    }

    fn on_share_end(&self, end: &ShareEnd, text: &str) {
        let Some(f) = self.cb.on_share_end else { return };
        let c = CString::new(text.replace('\0', " ")).unwrap_or_default();
        unsafe { f(self.cb.userdata, end.code(), c.as_ptr()) };
    }

    fn on_route(&self, route: &kybernow_core::relay::RouteInfo) {
        let Some(f) = self.cb.on_route else { return };
        let c = CString::new(serde_json::to_string(route).unwrap_or_default()).unwrap_or_default();
        unsafe { f(self.cb.userdata, c.as_ptr()) };
    }

    fn on_lend_notice(&self, notice: &kybernow_core::lend::LendNotice, text: &str) {
        let Some(f) = self.cb.on_lend_notice else { return };
        let json = CString::new(serde_json::to_string(notice).unwrap_or_default()).unwrap_or_default();
        let c = CString::new(text.replace('\0', " ")).unwrap_or_default();
        unsafe { f(self.cb.userdata, json.as_ptr(), c.as_ptr()) };
    }

    fn on_lend_end(&self, end: kybernow_core::lend::LendEnd, text: &str) {
        let Some(f) = self.cb.on_lend_end else { return };
        let c = CString::new(text.replace('\0', " ")).unwrap_or_default();
        unsafe { f(self.cb.userdata, end.code(), c.as_ptr()) };
    }

    fn on_recording(&self, state: &kybernow_core::recording::RecordingState, notice: Option<&str>, computer: Option<&str>) {
        let Some(f) = self.cb.on_recording else { return };
        let json = CString::new(state.ui_json(kybernow_core::i18n::current(), computer).to_string().replace('\0', " ")).unwrap_or_default();
        let notice_c = notice.map(|n| CString::new(n.replace('\0', " ")).unwrap_or_default());
        unsafe { f(self.cb.userdata, json.as_ptr(), notice_c.as_ref().map_or(std::ptr::null(), |c| c.as_ptr())) };
    }

    fn on_lend(&self, lend: Option<&kybernow_core::lend::LendView>) {
        let Some(f) = self.cb.on_lend else { return };
        let json = lend.map(|l| CString::new(serde_json::to_string(l).unwrap_or_default().replace('\0', " ")).unwrap_or_default());
        unsafe { f(self.cb.userdata, json.as_ref().map_or(std::ptr::null(), |c| c.as_ptr())) };
    }

    fn on_relay_error(&self, error: &kybernow_core::relay::RelayError, text: &str) {
        let Some(f) = self.cb.on_relay_error else { return };
        let c = CString::new(text.replace('\0', " ")).unwrap_or_default();
        unsafe { f(self.cb.userdata, error.code(), c.as_ptr()) };
    }

    fn on_share_image(&self, image: &ShareImage, previous: Option<&ShareImage>, notice: Option<&str>) {
        let Some(f) = self.cb.on_share_image else { return };
        let json = |i: &ShareImage| CString::new(serde_json::to_string(i).unwrap_or_default().replace('\0', " ")).unwrap_or_default();
        let image_c = json(image);
        let previous_c = previous.map(json);
        let notice_c = notice.map(|n| CString::new(n.replace('\0', " ")).unwrap_or_default());
        unsafe {
            f(
                self.cb.userdata,
                image_c.as_ptr(),
                previous_c.as_ref().map_or(std::ptr::null(), |c| c.as_ptr()),
                notice_c.as_ref().map_or(std::ptr::null(), |c| c.as_ptr()),
            )
        };
    }

    fn on_screens(&self, screens: &[StreamScreen], not_created: &[usize]) {
        if let Some(f) = self.cb.on_screens {
            let list: Vec<KnStreamScreen> = screens.iter().map(KnStreamScreen::from).collect();
            let missing: Vec<u32> = not_created.iter().map(|&i| i as u32).collect();
            unsafe { f(self.cb.userdata, list.as_ptr(), list.len(), missing.as_ptr(), missing.len()) };
        }
    }

    fn audio_handler(&self) -> Option<Arc<dyn PacketHandler>> {
        if !self.cb.has_audio {
            return None;
        }
        self.sink(self.cb.audio_packet, 0)
    }

    fn input_pipeline(&self) -> Pipeline {
        let ptr = self.cb.input_pipeline.map(|f| unsafe { f(self.cb.userdata) }).unwrap_or(std::ptr::null_mut());
        Pipeline::from_ptr(ptr as *mut kynput_sys::kynput_Pipeline)
    }

    fn on_status(&self, status: &Status) {
        let Some(f) = self.cb.on_status else { return };
        let (kind, attempt, max) = match status {
            Status::Preparing => (KN_STATUS_PREPARING, 0, 0),
            Status::Connecting => (KN_STATUS_CONNECTING, 0, 0),
            Status::FirstImages => (KN_STATUS_FIRST_IMAGES, 0, 0),
            Status::Streaming => (KN_STATUS_STREAMING, 0, 0),
            Status::Reconnecting { attempt, max } => (KN_STATUS_RECONNECTING, *attempt, *max),
            // Puis l'avis KN_NOTICE_STATION_RESTARTING, ci-dessous.
            Status::StationRestarting => (KN_STATUS_CONNECTING, 0, 0),
            Status::Ending => (KN_STATUS_ENDING, 0, 0),
            Status::Ended { .. } => (KN_STATUS_ENDED, 0, 0),
        };
        let (error, report): (Option<String>, Option<EndReport>) = match status {
            Status::Ended { error, report } => (error.clone(), report.clone()),
            Status::Ending => (self.ending_error.lock().unwrap_or_else(|e| e.into_inner()).take(), None),
            _ => (None, None),
        };
        let error_c = error.map(|e| CString::new(e.replace('\0', " ")).unwrap_or_default());
        let report_error_c = report
            .as_ref()
            .and_then(|r| r.error.clone())
            .map(|e| CString::new(e.replace('\0', " ")).unwrap_or_default());
        let s = KnStatus {
            kind,
            attempt,
            max,
            error: error_c.as_ref().map(|c| c.as_ptr()).unwrap_or(std::ptr::null()),
            has_report: report.is_some(),
            display_restored: report.as_ref().is_some_and(|r| r.display_restored),
            audio_restored: report.as_ref().is_some_and(|r| r.audio_restored),
            report_error: report_error_c.as_ref().map(|c| c.as_ptr()).unwrap_or(std::ptr::null()),
        };
        unsafe { f(self.cb.userdata, &s) };
        if matches!(status, Status::StationRestarting) {
            if let Some(notice) = self.cb.on_notice {
                let n = KnNotice {
                    kind: KN_NOTICE_STATION_RESTARTING,
                    width: 0,
                    height: 0,
                    back: false,
                    level: 0,
                    bitrate_mbps: 0,
                    has_mode: false,
                    error: std::ptr::null(),
                };
                unsafe { notice(self.cb.userdata, &n) };
            }
        }
    }

    fn on_display(&self, display: &Display) {
        if let Some(f) = self.cb.on_display {
            unsafe { f(self.cb.userdata, KnDisplay::from(display)) };
        }
    }

    fn on_displays(&self, displays: &[Display]) {
        if let Some(f) = self.cb.on_displays {
            let list: Vec<KnDisplay> = displays.iter().map(KnDisplay::from).collect();
            unsafe { f(self.cb.userdata, list.as_ptr(), list.len()) };
        }
    }

    fn on_mode(&self, mode: Mode, virtual_display: bool) {
        if let Some(f) = self.cb.on_mode {
            unsafe { f(self.cb.userdata, KnMode::from(mode), virtual_display) };
        }
    }

    fn on_stream_started(&self) {
        if let Some(f) = self.cb.on_stream_started {
            unsafe { f(self.cb.userdata) };
        }
    }

    fn on_stream_stopped(&self) {
        if let Some(f) = self.cb.on_stream_stopped {
            unsafe { f(self.cb.userdata) };
        }
    }

    fn sends_camera(&self) -> bool {
        self.cb.sends_camera
    }

    fn on_camera_offer(&self, offered: bool) {
        if let Some(f) = self.cb.on_camera_offer {
            unsafe { f(self.cb.userdata, offered) };
        }
    }

    fn on_input_connected(&self) {
        if let Some(f) = self.cb.on_input_connected {
            unsafe { f(self.cb.userdata) };
        }
    }

    fn on_second(&self, second: &Second) {
        if let Some(f) = self.cb.on_second {
            let s = KnSecond::from(second);
            unsafe { f(self.cb.userdata, &s) };
        }
    }

    fn on_game_fullscreen(&self, game: bool) {
        if let Some(f) = self.cb.on_game_fullscreen {
            unsafe { f(self.cb.userdata, game) };
        }
    }

    fn on_hdr(&self, hdr: &[bool]) {
        if let Some(f) = self.cb.on_hdr {
            unsafe { f(self.cb.userdata, hdr.as_ptr(), hdr.len()) };
        }
    }

    fn on_microphone(&self, state: kybernow_core::micro::MicState) {
        let Some(f) = self.cb.on_microphone else { return };
        let c = CString::new(state.to_json().replace('\0', " ")).unwrap_or_default();
        unsafe { f(self.cb.userdata, c.as_ptr()) };
    }

    /// Gardée pour `KN_STATUS_ENDING`, qui suit tout de suite.
    fn on_ending(&self, error: Option<&str>) {
        *self.ending_error.lock().unwrap_or_else(|e| e.into_inner()) = error.map(str::to_string);
    }

    fn on_pyrowave_fallback(&self, codec: Codec, text: &str) {
        let Some(f) = self.cb.on_pyrowave_fallback else { return };
        let codec = CString::new(codec.key()).unwrap_or_default();
        let c = CString::new(text.replace('\0', " ")).unwrap_or_default();
        unsafe { f(self.cb.userdata, codec.as_ptr(), c.as_ptr()) };
    }

    fn on_notice(&self, notice: &Notice) {
        let Some(f) = self.cb.on_notice else { return };
        let mut n = KnNotice {
            kind: 0,
            width: 0,
            height: 0,
            back: false,
            level: 0,
            bitrate_mbps: 0,
            has_mode: false,
            error: std::ptr::null(),
        };
        let error_c;
        match notice {
            Notice::Relaunched => n.kind = KN_NOTICE_RELAUNCHED,
            Notice::ModeChanged { width, height, back } => {
                n.kind = KN_NOTICE_MODE_CHANGED;
                n.width = *width;
                n.height = *height;
                n.back = *back;
            }
            Notice::Fallback { level, bitrate_mbps } => {
                n.kind = KN_NOTICE_FALLBACK;
                n.level = *level;
                n.bitrate_mbps = *bitrate_mbps;
            }
            Notice::Adapting { width, height } => {
                n.kind = KN_NOTICE_ADAPTING;
                n.width = *width;
                n.height = *height;
            }
            Notice::Adapted { width, height } => {
                n.kind = KN_NOTICE_ADAPTED;
                n.width = *width;
                n.height = *height;
            }
            Notice::AdaptFailed { error } => {
                n.kind = KN_NOTICE_ADAPT_FAILED;
                error_c = CString::new(error.replace('\0', " ")).unwrap_or_default();
                n.error = error_c.as_ptr();
            }
            Notice::RestoreOffer { mode } => {
                n.kind = KN_NOTICE_RESTORE_OFFER;
                if let Some(m) = mode {
                    n.has_mode = true;
                    n.width = m.width;
                    n.height = m.height;
                }
            }
            Notice::HdrRefused => {
                n.kind = KN_NOTICE_HDR_REFUSED;
                error_c = CString::new(profile::hdr_refused_text(i18n::current())).unwrap_or_default();
                n.error = error_c.as_ptr();
            }
            Notice::ResolutionLowered { lowered } => {
                n.kind = KN_NOTICE_RESOLUTION_LOWERED;
                if let Some(l) = lowered {
                    n.has_mode = true;
                    n.width = l.width;
                    n.height = l.height;
                    n.bitrate_mbps = l.mbps;
                    error_c = CString::new(l.text(i18n::current())).unwrap_or_default();
                    n.error = error_c.as_ptr();
                }
            }
            Notice::PyroWaveStrained => n.kind = KN_NOTICE_PYROWAVE_STRAINED,
            Notice::EncoderCeiling { mbps } => {
                n.kind = KN_NOTICE_ENCODER_CEILING;
                n.bitrate_mbps = *mbps;
            }
        }
        unsafe { f(self.cb.userdata, &n) };
    }
}

// MARK: - Session

/// Une session (voir `kybernow_core::session`).
pub struct KnSession {
    session: Arc<Session>,
}

/// Ouvre une session. `params_json` : `console_host`, `console_port`,
/// `user`, `password`, `console_fingerprint` (optionnel), `device` (`id`,
/// `name`, `kind`, `os`, `app`), `mode` (`width`, `height`, `refresh`),
/// `audio_channels` (0 : son laissé sur l'ordinateur), `virtual_display`,
/// `settings` (`profile`, `codec`, `yuv444`, `mbps`, `reliable`, `hdr` : le
/// choix « HDR » de l'app pour cet ordinateur ; absent : éteint),
/// `auto_reconnect`, `gamepad_xbox360`, `clipboard`, `recovery`
/// (`watch_image`, `fallback_bitrate`, `restart_station` : vrai si absent),
/// et pour le multi-écrans `screens` : `list`, les écrans de l'appareil
/// (`x`, `y`, `width`, `height` : cadre dans le repère logique de l'appareil,
/// y vers le bas ; `pixel_width`, `pixel_height` : définition native ;
/// `refresh` ; `primary` ; `hdr` : l'écran est HDR ; `luminance` : ses
/// valeurs HDR, `{max, full_frame, min, sdr_white}`), `current` (« Cet
/// écran » : celui de la fenêtre),
/// `limit_4k` (« Au plus 4K »), `custom_resolution`, `setting` (`auto`,
/// `this`, `all`) et `max_screens` (`virtual_display.max_screens` de
/// `kn_console_info` ; absent : 1). Le cœur en tire le nombre d'écrans, au
/// départ et quand le profil change (`kn_session_set_settings`). `mode` :
/// le mode de « Cet écran », comme sans `screens`. La plateforme reçoit
/// `on_screens` et les paquets de chaque écran (`KnPacket::screen`).
/// HDR (docs/hdr.md) : `screen_hdr` et `screen_luminance` (l'écran de
/// l'appareil d'une session à un écran, sans `screens`, est HDR, et ses
/// valeurs ; avec `screens`, leurs `hdr` et `luminance`) et
/// `capabilities` (`{hevc, hevc444, av1, hdr, pyrowave}` : le décodeur ; absent, l'app
/// répond du décodeur). La session envoie `hdr` à la Station et à Kyber
/// quand le choix est allumé et possible (`kn_hdr_state`), impose alors
/// 4:2:0 et HEVC ou AV1, et dit les écrans en HDR par `on_hdr`.
/// KyberNow Share, session d'invité : `guest` (`{"joined": <la réponse de
/// kn_share_join>, "capabilities": {hevc, hevc444, av1}}`), `console_host`,
/// `console_port` et `console_fingerprint` de la Station trouvée (`target`
/// de kn_share_join) ; `user`, `password` et `mode` sont alors facultatifs.
/// La plateforme reçoit `on_share_role` et `on_share_end` ; une session
/// d'hôte reçoit `on_guests`. Par le relais (docs/relais.md) : `route`
/// (`direct`, `relay` : toujours par le relais, `auto` : d'abord en direct,
/// 1,5 s) et `relay` (`{server, station, admin_fp}`, `kn_relay_route_from_info`) ;
/// absent : en direct, ou `auto` si `relay` est donné. Un invité venu par le
/// relais y reste (la route voyage dans `joined`). La plateforme reçoit `on_route`.
/// Prêt de l'ordinateur (docs/pret.md), session d'emprunt : `borrower`
/// (`{"borrowed": <borrowed de kn_lend_borrow>}`), `console_host`,
/// `console_port` et `console_fingerprint` de `target` ; `user`, `password`
/// et `mode` facultatifs (l'écran virtuel du prêt). Elle reçoit
/// `on_lend_notice` et `on_lend_end` ; une session d'hôte reçoit `on_lend`.
/// Nul si les paramètres sont illisibles (`error`). La session commence tout
/// de suite ; `on_status` suit.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_session_start(
    params_json: *const c_char,
    platform: *const KnPlatform,
    error: *mut *mut c_char,
) -> *mut KnSession {
    kybernow_core::ffi_guard!(null => {
        let Some(cb) = (unsafe { platform.as_ref() }) else { return std::ptr::null_mut() };
        let (params, screens) = match SessionParams::from_json(&text(params_json)) {
            Ok(p) => p,
            Err(e) => {
                give_to(error, e);
                return std::ptr::null_mut();
            }
        };
        let owner = Owner::new(cb.userdata, cb.release_userdata);
        let platform = Arc::new(ApplePlatform { cb: *cb, owner, streams: AtomicU64::new(0), ending_error: Mutex::new(None) });
        let session = match screens {
            Some(screens) => Session::start_with_screens(params, screens, platform),
            None => Session::start(params, platform),
        };
        Box::into_raw(Box::new(KnSession { session }))
    })
}

fn with<R>(s: *const KnSession, f: impl FnOnce(&Session) -> R) -> Option<R> {
    unsafe { s.as_ref() }.map(|s| f(&s.session))
}

/// Demande la fin de la session (sans attendre : `KN_STATUS_ENDED` suit).
/// Une session d'emprunt fermée ainsi ne rend pas l'ordinateur (retour
/// possible pendant 15 min).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_session_stop(s: *const KnSession) {
    kybernow_core::ffi_guard!({
        with(s, |s| s.stop());
    })
}

/// Session d'emprunt : « Rendre l'ordinateur » (après sa confirmation) : le
/// prêt finit, puis la session, sans erreur. Sans effet hors d'un emprunt.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_session_give_back(s: *const KnSession) {
    kybernow_core::ffi_guard!({
        with(s, |s| s.give_back());
    })
}

/// Attend la fin du fil de session (console prévenue). Jamais depuis un rappel.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_session_wait(s: *const KnSession) {
    kybernow_core::ffi_guard!({
        with(s, |s| s.wait());
    })
}

/// Libère la session (après `kn_session_wait`, ou l'état terminé reçu).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_session_free(s: *mut KnSession) {
    kybernow_core::ffi_guard!({
        if !s.is_null() {
            drop(unsafe { Box::from_raw(s) });
        }
    })
}

/// Une image clé demandée une fois (plateforme qui tient sa propre veille).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_session_request_keyframe(s: *const KnSession) {
    kybernow_core::ffi_guard!({
        with(s, |s| s.request_keyframe());
    })
}

/// Le décodeur a perdu une image de référence : image clé redemandée toutes
/// les 300 ms jusqu'à `kn_session_keyframe_recovered`, flux relancé à 1,5 s.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_session_keyframe_lost(s: *const KnSession) {
    kybernow_core::ffi_guard!({
        with(s, |s| s.keyframe_lost());
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_session_keyframe_recovered(s: *const KnSession) {
    kybernow_core::ffi_guard!({
        with(s, |s| s.keyframe_recovered());
    })
}

/// Autres réglages pendant la session (profil depuis la barre), même JSON que
/// `settings` au départ : débit changé sur-le-champ, ou flux relancé si le
/// format, le 4:4:4 ou le transport changent.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_session_set_settings(s: *const KnSession, settings_json: *const c_char) -> i32 {
    kybernow_core::ffi_guard!(crate::KN_ERROR => {
        let Ok(settings) = StreamSettings::from_json(&text(settings_json)) else { return KN_ERROR };
        with(s, |s| s.set_settings(settings));
        KN_OK
    })
}

/// Plafond de débit (bit/s) le temps que la fenêtre est occultée ; 0 le retire.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_session_set_bitrate_cap(s: *const KnSession, cap: u32) {
    kybernow_core::ffi_guard!({
        with(s, |s| s.set_bitrate_cap((cap > 0).then_some(cap)));
    })
}

/// PyroWave (docs/pyrowave.md, « Définition ramenée ») : le débit que
/// l'appareil sait tenir vers l'ordinateur, en Mbit/s (vitesse du lien ou
/// débit mesuré ; 0 : inconnu). À donner juste après le démarrage, puis à
/// chaque changement durable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_session_set_link_mbps(s: *const KnSession, mbps: u32) {
    kybernow_core::ffi_guard!({
        with(s, |s| s.set_link_mbps((mbps > 0).then_some(mbps)));
    })
}

/// L'app fige l'image (arrière-plan, fenêtre cachée) ou la reprend : la
/// veille « aucune image » attend le retour.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_session_set_hidden(s: *const KnSession, hidden: bool) {
    kybernow_core::ffi_guard!({
        with(s, |s| s.set_hidden(hidden));
    })
}

/// Taille des images décodées par l'app (à chaque changement).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_session_decoded_size(s: *const KnSession, width: u32, height: u32) {
    kybernow_core::ffi_guard!({
        with(s, |s| s.decoded_size(width, height));
    })
}

/// Sans écran virtuel : afficher un autre écran de l'ordinateur.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_session_pick_display(s: *const KnSession, id: u32) {
    kybernow_core::ffi_guard!({
        with(s, |s| s.pick_display(id));
    })
}

/// L'écran de l'appareil a changé de mode : la console refait l'écran
/// virtuel à ce mode, puis l'image repart dessus.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_session_set_mode(s: *const KnSession, mode: KnMode) {
    kybernow_core::ffi_guard!({
        with(s, |s| s.set_mode(Mode { width: mode.width, height: mode.height, refresh: mode.refresh }));
    })
}

/// « Remettre » : la console remet le mode demandé au début de la session.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_session_restore_mode(s: *const KnSession) {
    kybernow_core::ffi_guard!({
        with(s, |s| s.restore_mode());
    })
}

/// KyberNow Share, session d'invité : la « Qualité de l'image » choisie
/// (`KN_SHARE_QUALITY_*`), dite à la Station sans attendre ; Kyber relance
/// l'image de l'invité seulement, l'image reçue arrive par `on_share_image`.
/// Plus d'un changement par seconde : Admin refuse, le cœur réessaie une
/// fois après son délai (seulement si rien de plus récent n'est demandé).
/// `KN_ERROR` pour un code inconnu ; sans effet pour un hôte.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_session_set_share_quality(s: *const KnSession, quality: i32) -> i32 {
    kybernow_core::ffi_guard!(crate::KN_ERROR => {
        let Some(q) = ShareQuality::from_code(quality) else { return KN_ERROR };
        with(s, |s| s.set_share_quality(q));
        KN_OK
    })
}

/// Multi-écrans : les écrans de l'appareil ont changé (même JSON que
/// `screens` dans `kn_session_start`) ; la session se refait avec eux, sans
/// effet sur une session ouverte sans `screens`. `KN_ERROR` si illisible.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_session_set_screens(s: *const KnSession, screens_json: *const c_char) -> i32 {
    kybernow_core::ffi_guard!(crate::KN_ERROR => {
        let Ok(screens) = serde_json::from_str::<Screens>(&text(screens_json)) else { return KN_ERROR };
        with(s, |s| s.set_screens(screens));
        KN_OK
    })
}

/// HDR (docs/hdr.md) : le choix « HDR » changé en session (barre, menu de
/// l'icône). La Station passe ses écrans virtuels en HDR ou en SDR, puis
/// l'image des écrans touchés est relancée (étape « Premières images ») ;
/// `on_hdr` suit. Sans effet pour un invité. Un profil changé par
/// `kn_session_set_settings` garde ce choix.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_session_set_hdr(s: *const KnSession, hdr: bool) {
    kybernow_core::ffi_guard!({
        with(s, |s| s.set_hdr(hdr));
    })
}

/// HDR : les valeurs d'un écran de l'appareil, en nits (docs/hdr.md,
/// « Réglage du HDR ») : luminosité maximale (petite zone), sur tout
/// l'écran, noir, blanc du bureau. En JSON (`screens.list[i].luminance`,
/// `screen_luminance`, `screen.luminance` de `kn_lend_borrow`) :
/// `{max, full_frame, min, sdr_white}`.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct KnHdrLuminance {
    pub max: f64,
    pub full_frame: f64,
    pub min: f64,
    pub sdr_white: f64,
}

impl From<HdrLuminance> for KnHdrLuminance {
    fn from(l: HdrLuminance) -> Self {
        Self { max: l.max, full_frame: l.full_frame, min: l.min, sdr_white: l.sdr_white }
    }
}

/// Les valeurs par défaut du cœur (aussi la valeur rendue après une panique).
impl Default for KnHdrLuminance {
    fn default() -> Self {
        HdrLuminance::default().into()
    }
}

impl From<KnHdrLuminance> for HdrLuminance {
    fn from(l: KnHdrLuminance) -> Self {
        Self { max: l.max, full_frame: l.full_frame, min: l.min, sdr_white: l.sdr_white }
    }
}

/// Les valeurs HDR par défaut : 1 000 nits, 1 000 nits sur tout l'écran,
/// 0 nit, blanc du bureau à 203 nits.
#[unsafe(no_mangle)]
pub extern "C" fn kn_hdr_luminance_default() -> KnHdrLuminance {
    kybernow_core::ffi_guard!({
        HdrLuminance::default().into()
    })
}

/// Les valeurs bornées comme le cœur les envoie (maximale de 100 à 10 000
/// nits, tout l'écran au plus la maximale, noir de 0 à 1, blanc du bureau
/// de 80 à 480) : pour l'écran « Réglage du HDR ».
#[unsafe(no_mangle)]
pub extern "C" fn kn_hdr_luminance_clamp(luminance: KnHdrLuminance) -> KnHdrLuminance {
    kybernow_core::ffi_guard!({
        HdrLuminance::from(luminance).clamped().into()
    })
}

/// HDR : l'écran de l'appareil `screen` (index dans `screens.list` ; 0 pour
/// une session à un écran) gagne ou perd le HDR (fenêtre glissée sur un
/// écran SDR, téléviseur qui coupe le HDR), ou ses valeurs changent.
/// `luminance` : les valeurs de l'écran où l'image se trouve maintenant
/// (nul : par défaut), données à la Station avec le HDR. L'image de cet
/// écran repasse en SDR, puis en HDR quand il le redevient, sans message ;
/// le choix reste.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_session_screen_hdr_changed(s: *const KnSession, screen: u32, hdr: bool, luminance: *const KnHdrLuminance) {
    kybernow_core::ffi_guard!({
        let luminance = unsafe { luminance.as_ref() }.map(|l| HdrLuminance::from(*l));
        with(s, |s| s.screen_hdr_changed(screen as usize, hdr, luminance));
    })
}

/// Regarder : « Écran » choisi dans la barre (`display` : un `id` de
/// `joined.watch.screens`) ; la session se reconnecte sur cet écran (étapes
/// « Premières images », `on_mode`). Sans effet hors d'une session de qui
/// regarde, ou si `joined.watch.switch` est faux.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_session_watch_screen(s: *const KnSession, display: u32) {
    kybernow_core::ffi_guard!({
        with(s, |s| s.watch_screen(display));
    })
}

/// Regarder : l'écran regardé (à cocher dans « Écran ») ; 0 hors d'une
/// session de qui regarde.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_session_watch_display(s: *const KnSession) -> u32 {
    kybernow_core::ffi_guard!({
        with(s, |s| s.watch_display()).flatten().unwrap_or(0)
    })
}

/// HDR : un écran au moins a été en HDR pendant la session (« HDR » de la
/// dernière session, après le format).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_session_hdr_used(s: *const KnSession) -> bool {
    kybernow_core::ffi_guard!({
        with(s, |s| s.hdr_used()).unwrap_or(false)
    })
}

/// HDR : la puce de l'accueil, la ligne de Réglages › Image et celle de la
/// barre (`profile::hdr_state`). `info_json` : la réponse de
/// `kn_console_info` (`hdr` : `{available, reason}` ; absent : Station
/// d'avant le HDR) ; `settings_json` : les réglages de la session (même JSON
/// que `settings` de `kn_session_start` ; seul le format compte) ;
/// `capabilities_json` : `{hevc, hevc444, av1, hdr, pyrowave}` ; `screen_hdr` : l'écran
/// de l'appareil est HDR. Rend `{"shown", "available", "reason", "kind"}` :
/// `shown` faux, aucune trace du HDR ; `available` faux, puce inactive avec
/// `reason` (texte de link-ui dans la langue de l'app) et `kind` (`off`,
/// `windows`, `encoder`, `codec`, `decoder`, `display`). Nul si un JSON est
/// illisible. À libérer par `kn_string_free`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_hdr_state(
    info_json: *const c_char,
    settings_json: *const c_char,
    capabilities_json: *const c_char,
    screen_hdr: bool,
) -> *mut c_char {
    kybernow_core::ffi_guard!(null => {
        let Ok(info) = serde_json::from_str::<kybernow_core::console::Info>(&text(info_json)) else { return std::ptr::null_mut() };
        let Ok(settings) = StreamSettings::from_json(&text(settings_json)) else { return std::ptr::null_mut() };
        let Ok(caps) = serde_json::from_str::<Capabilities>(&text(capabilities_json)) else { return std::ptr::null_mut() };
        match serde_json::to_string(&profile::hdr_state(&info, &settings, caps, screen_hdr)) {
            Ok(json) => give(json),
            Err(_) => std::ptr::null_mut(),
        }
    })
}

/// Réglage « Écrans » : faux avec une définition sur mesure ou sans écran
/// virtuel ; il est alors inactif, sur « Cet écran ».
#[unsafe(no_mangle)]
pub extern "C" fn kn_screens_setting_enabled(custom_resolution: bool, virtual_display: bool) -> bool {
    kybernow_core::ffi_guard!({
        DisplaysChoice { setting: DisplaysSetting::Auto, profile: None, custom_resolution, virtual_display }.setting_enabled()
    })
}

/// Les écrans que prendra la session pour ces écrans de l'appareil (même JSON
/// que `screens` de `kn_session_start`, réglage et `max_screens` compris),
/// ce profil (`jeu`, `bureau`, `max` ; vide : Personnalisé), cet écran
/// virtuel et ce format (`h264`, `hevc`, `av1`) : une liste JSON d'au moins
/// deux écrans, principal en premier, de `device` (index dans `list`),
/// `width`, `height`, `refresh`, `x`, `y`, `primary` ; quatre au plus, et
/// pas plus que la Station n'en crée. Nul : une session à un écran (réglage,
/// profil, Station antérieure, un seul écran, JSON illisible). Son nombre
/// d'écrans est celui de la ligne de l'accueil, et les fenêtres à ouvrir ;
/// c'est le même calcul que la session. À libérer par `kn_string_free`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_screens_session_plan(
    screens_json: *const c_char,
    profile: *const c_char,
    virtual_display: bool,
    codec: *const c_char,
) -> *mut c_char {
    kybernow_core::ffi_guard!(null => {
        let Ok(screens) = serde_json::from_str::<Screens>(&text(screens_json)) else { return std::ptr::null_mut() };
        let codec = Codec::parse(&text(codec)).unwrap_or(Codec::Hevc);
        let Some(plan) = screens::session_plan(&screens, Profile::parse(&text(profile)), virtual_display, codec) else { return std::ptr::null_mut() };
        let list: Vec<serde_json::Value> = plan
            .iter()
            .map(|p| {
                serde_json::json!({
                    "device": p.device, "width": p.mode.width, "height": p.mode.height,
                    "refresh": p.mode.refresh, "x": p.x, "y": p.y, "primary": p.primary,
                })
            })
            .collect();
        give(serde_json::Value::Array(list).to_string())
    })
}

/// Plusieurs ordinateurs : les écrans d'une session ouverte pendant qu'une
/// autre tourne, « Cet écran » quel que soit le réglage
/// (`screens::screens_for_extra_session`) : même JSON que `screens` de
/// `kn_session_start`, `current` étant l'écran de la fenêtre active, où
/// s'ouvre la nouvelle. Nul si illisible. À libérer par `kn_string_free`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_screens_for_extra_session(screens_json: *const c_char) -> *mut c_char {
    kybernow_core::ffi_guard!(null => {
        let Ok(screens) = serde_json::from_str::<Screens>(&text(screens_json)) else { return std::ptr::null_mut() };
        match serde_json::to_string(&screens::screens_for_extra_session(&screens)) {
            Ok(json) => give(json),
            Err(_) => std::ptr::null_mut(),
        }
    })
}

/// La fenêtre d'une session de plus (`kn_screens_extra_session_window`).
#[repr(C)]
#[derive(Clone, Copy)]
pub struct KnExtraWindow {
    /// Cadre de la zone d'image, dans le repère logique de l'appareil (celui
    /// de `screens.list`, y vers le bas).
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    /// Le mode à demander pour l'écran de l'ordinateur : la zone d'image en
    /// pixels de l'écran (largeur et hauteur multiples de 8, le cadre
    /// ci-dessus étant celui de ce mode), à sa fréquence.
    pub mode: KnMode,
}

/// Plusieurs ordinateurs : la fenêtre d'une session ouverte pendant qu'une
/// autre tourne, « centrée, aux deux tiers de cet écran, en 16:9 »
/// (`screens::extra_session_window`), sur l'écran `current` du JSON `screens`
/// (même JSON que `kn_session_start`). Faux si illisible ou sans cet écran.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_screens_extra_session_window(screens_json: *const c_char, out: *mut KnExtraWindow) -> bool {
    kybernow_core::ffi_guard!({
        let Ok(screens) = serde_json::from_str::<Screens>(&text(screens_json)) else { return false };
        let (Some(screen), Some(out)) = (screens.list.get(screens.current), unsafe { out.as_mut() }) else { return false };
        let w = screens::extra_session_window(screen);
        *out = KnExtraWindow { x: w.x, y: w.y, width: w.width, height: w.height, mode: w.mode.into() };
        true
    })
}

/// Image `pts` reçue, décodée puis montrée (µs, `kn_now_micros`).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_session_frame_displayed(s: *const KnSession, pts: u64, received: i64, decoded: i64, at: i64) {
    kybernow_core::ffi_guard!({
        with(s, |s| s.frame_displayed(pts, received, decoded, at));
    })
}

/// Image remplacée par une plus récente avant d'être montrée.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_session_frame_skipped(s: *const KnSession, pts: u64) {
    kybernow_core::ffi_guard!({
        with(s, |s| s.latency().skipped(pts));
    })
}

/// Paquet d'image reçu (débit).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_session_bytes_received(s: *const KnSession, bytes: usize) {
    kybernow_core::ffi_guard!({
        with(s, |s| s.latency().received_bytes(bytes));
    })
}

/// Trou dans le flux.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_session_hole(s: *const KnSession) {
    kybernow_core::ffi_guard!({
        with(s, |s| s.latency().hole());
    })
}

// MARK: - Plusieurs ordinateurs à la fois (docs/plusieurs-ordinateurs.md)

/// Le moteur de son de la session (le son va à Swift, qui le donne à son
/// `KnAudio` : le cœur ne le voit pas passer). À appeler pour chaque
/// `KnAudio` de la session, dès sa création : il prend tout de suite l'état
/// « son coupé » de la session, puis le suit. Le `KnAudio` peut être libéré
/// sans prévenir.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_session_attach_audio(s: *const KnSession, audio: *const audio::KnAudio) {
    kybernow_core::ffi_guard!({
        let Some(a) = (unsafe { audio.as_ref() }) else { return };
        with(s, |s| s.attach_audio(&a.0));
    })
}

// MARK: - Micro de l'appareil (docs/micro.md)

/// L'appareil a un micro (Mac, iPhone, iPad ; jamais l'Apple TV) : le micro
/// est proposé à chaque démarrage de l'image, même coupé (rien ne part), et
/// la session donne son état par `on_microphone`. Une fois au lancement.
#[unsafe(no_mangle)]
pub extern "C" fn kn_set_device_has_microphone(has: bool) {
    kybernow_core::ffi_guard!({
        kybernow_core::micro::set_device_has_microphone(has);
    })
}

/// « Micro » allumé (vrai) ou coupé : le choix retenu par l'app pour cet
/// ordinateur, donné juste après `kn_session_start` puis à chaque changement
/// (puce de la barre, tuile, réglage « Micro » de la rubrique Son). Ne
/// relance rien ; l'état suit par `on_microphone`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_session_set_microphone(s: *const KnSession, on: bool) {
    kybernow_core::ffi_guard!({
        with(s, |s| s.set_microphone(on));
    })
}

/// Échantillons du micro, flottants mono (`count`), à `rate_hz` (n'importe
/// quelle fréquence : le cœur ramène à 48 kHz, encode en Opus et envoie).
/// Depuis le fil de capture : ne bloque pas ; jetés tant que l'ordinateur
/// ne prend pas le micro.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_session_push_microphone(s: *const KnSession, samples: *const f32, count: usize, rate_hz: u32) {
    kybernow_core::ffi_guard!({
        if samples.is_null() || count == 0 || rate_hz == 0 {
            return;
        }
        let samples = unsafe { std::slice::from_raw_parts(samples, count) };
        with(s, |s| s.push_microphone(samples, rate_hz));
    })
}

/// Inscrit la session dans le registre des sessions de l'app, `name` : le
/// nom de son ordinateur (au début des lignes de son journal). Elle devient
/// active ; les autres passent derrière, leur son coupé (sauf « Garder le
/// son »). Rend son identifiant (positif), le même si elle est déjà
/// inscrite ; -1 au-delà de quatre sessions (`kn_focus_max`) ou sans session.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_focus_register(s: *const KnSession, name: *const c_char) -> i64 {
    kybernow_core::ffi_guard!({
        let name = text(name);
        with(s, |s| focus::global().register(s, &name).map_or(-1, |id| id.as_u64() as i64)).unwrap_or(-1)
    })
}

fn focus_id(id: i64) -> Option<FocusId> {
    (id > 0).then(|| FocusId::from_u64(id as u64))
}

/// Désinscrit une session (fenêtre fermée, « Se déconnecter », fin de la
/// session) : elle passe derrière, son son est coupé ; si elle avait la main,
/// la session qui l'avait juste avant la reprend.
#[unsafe(no_mangle)]
pub extern "C" fn kn_focus_unregister(id: i64) {
    kybernow_core::ffi_guard!({
        if let Some(id) = focus_id(id) {
            focus::global().unregister(id);
        }
    })
}

/// La fenêtre de cette session passe au premier plan : elle devient active.
#[unsafe(no_mangle)]
pub extern "C" fn kn_focus_activate(id: i64) {
    kybernow_core::ffi_guard!({
        if let Some(id) = focus_id(id) {
            focus::global().activate(id);
        }
    })
}

/// « Garder le son » : la session reste audible sans être active.
#[unsafe(no_mangle)]
pub extern "C" fn kn_focus_set_keep_sound(id: i64, keep: bool) {
    kybernow_core::ffi_guard!({
        if let Some(id) = focus_id(id) {
            focus::global().set_keep_sound(id, keep);
        }
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn kn_focus_keep_sound(id: i64) -> bool {
    kybernow_core::ffi_guard!({
        focus_id(id).is_some_and(|id| focus::global().keep_sound(id))
    })
}

/// La session active (-1 : aucune).
#[unsafe(no_mangle)]
pub extern "C" fn kn_focus_active() -> i64 {
    kybernow_core::ffi_guard!({
        focus::global().active().map_or(-1, |id| id.as_u64() as i64)
    })
}

/// « Ordinateur suivant » : la session ouverte après l'active, dans l'ordre
/// d'ouverture (-1 : aucune). À l'app de faire passer sa fenêtre devant.
#[unsafe(no_mangle)]
pub extern "C" fn kn_focus_next() -> i64 {
    kybernow_core::ffi_guard!({
        focus::global().next().map_or(-1, |id| id.as_u64() as i64)
    })
}

/// Sessions inscrites.
#[unsafe(no_mangle)]
pub extern "C" fn kn_focus_count() -> u32 {
    kybernow_core::ffi_guard!({
        focus::global().count() as u32
    })
}

/// Sessions au plus (4).
#[unsafe(no_mangle)]
pub extern "C" fn kn_focus_max() -> u32 {
    kybernow_core::ffi_guard!({
        focus::MAX_SESSIONS as u32
    })
}

/// Latence de la session : médiane des secondes et 95e centile (ms). Faux
/// sans mesure.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_session_latency(s: *const KnSession, median: *mut f64, p95: *mut f64) -> bool {
    kybernow_core::ffi_guard!({
        let Some((med, p)) = with(s, |s| s.latency().session()) else { return false };
        let Some(med) = med else { return false };
        unsafe {
            if !median.is_null() {
                *median = med;
            }
            if !p95.is_null() {
                *p95 = p.unwrap_or(med);
            }
        }
        true
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manettes_de_l_ordinateur() {
        let available = |json: &str| {
            let c = CString::new(json).unwrap();
            unsafe { kn_gamepads_available(c.as_ptr()) }
        };
        assert!(available("{}") && available("illisible"), "Station d'avant ou réponse illisible : disponibles");
        assert!(!available(r#"{"os": "macOS 26.1", "gamepads": {"available": false, "reason": "entitlement"}}"#));
        assert!(available(r#"{"gamepads": {"available": true}}"#));
    }

    static RELEASED: AtomicU64 = AtomicU64::new(0);
    static CLOSED: AtomicU64 = AtomicU64::new(0);
    unsafe extern "C" fn release(ud: *mut c_void) {
        assert_eq!(ud as usize, 0x1234);
        RELEASED.fetch_add(1, Ordering::SeqCst);
    }
    unsafe extern "C" fn packet(ud: *mut c_void, p: *const KnPacket) {
        assert_eq!(ud as usize, 0x1234);
        // Le `userdata` est encore là quand le flux se ferme (MAC-01).
        assert_eq!(RELEASED.load(Ordering::SeqCst), 0);
        if unsafe { (*p).kind } == KN_PACKET_CLOSED {
            CLOSED.fetch_add(1, Ordering::SeqCst);
        }
    }

    #[test]
    fn userdata_rendu_au_dernier_detenteur() {
        let owner = Owner::new(0x1234 as *mut c_void, Some(release));
        let sink = Sink { cb: packet, userdata: 0x1234 as *mut c_void, stream: 1, screen: 0, _owner: owner.clone() };
        drop(owner);
        assert_eq!(RELEASED.load(Ordering::SeqCst), 0, "un flux tenu par Kyber garde le userdata");
        drop(sink);
        assert_eq!(CLOSED.load(Ordering::SeqCst), 1, "le flux dit sa fermeture");
        assert_eq!(RELEASED.load(Ordering::SeqCst), 1, "rendu une fois, après la fermeture du dernier flux");
        // Sans `release_userdata` (app d'avant) : rien n'est rendu.
        drop(Owner::new(0x1234 as *mut c_void, None));
        assert_eq!(RELEASED.load(Ordering::SeqCst), 1);
    }
}
