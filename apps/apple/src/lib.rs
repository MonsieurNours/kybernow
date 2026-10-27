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

use serde::Deserialize;

use kybernow_core::console::{Console, ConsoleError, DeviceInfo, Mode, PairStatus, StationError};
use kybernow_core::focus::{self, FocusId};
use kybernow_core::gamepads;
use kybernow_core::i18n::{self, Lang};
use kybernow_core::kynput_rs::Pipeline;
use kybernow_core::latency::{self, Second};
use kybernow_core::media::{Packet, PacketHandler};
use kybernow_core::profile::{Codec, Profile, StreamSettings};
use kybernow_core::screens::{self, DisplaysChoice, DisplaysSetting, Screens};
use kybernow_core::session::{EndReport, Notice, Platform, Recovery, Session, SessionParams, Status, StreamScreen};
use kybernow_core::wake;
use kybernow_core::{Display, WindowHandle};

pub mod audio;
pub mod input;

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
    if !s.is_null() {
        drop(unsafe { CString::from_raw(s) });
    }
}

/// Libère des octets rendus par le cœur (`kn_console_preview`).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_bytes_free(data: *mut u8, len: usize) {
    if !data.is_null() {
        drop(unsafe { Vec::from_raw_parts(data, len, len) });
    }
}

// MARK: - Langue et journal

/// Langue de l'app (`fr` ou `en`) : messages du cœur et `Accept-Language`
/// vers la console.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_set_language(tag: *const c_char) {
    i18n::set_current(Lang::from_tag(&text(tag)));
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
        let Some(cb) = *LOG_CB.lock().unwrap() else { return };
        let target = CString::new(record.target().replace('\0', " ")).unwrap_or_default();
        let message = CString::new(record.args().to_string().replace('\0', " ")).unwrap_or_default();
        unsafe { cb(record.level() as u32, target.as_ptr(), message.as_ptr()) };
    }

    fn flush(&self) {}
}

/// Branche le journal du cœur (à appeler une fois, avant tout le reste).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_set_log_callback(cb: KnLogCb) {
    *LOG_CB.lock().unwrap() = cb;
    static ONCE: OnceLock<()> = OnceLock::new();
    ONCE.get_or_init(|| {
        let _ = log::set_logger(&LOGGER);
        log::set_max_level(log::LevelFilter::Info);
    });
}

/// Maintenant, sur l'horloge des mesures de Kyber (µs, CLOCK_MONOTONIC).
#[unsafe(no_mangle)]
pub extern "C" fn kn_now_micros() -> i64 {
    latency::now_micros()
}

// MARK: - Console

/// Client de la console d'un ordinateur (hors session).
pub struct KnConsole(Console);

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
    Box::into_raw(Box::new(KnConsole(Console::new(
        &text(host),
        port,
        &text(user),
        &text(password),
        optional(pinned),
    ))))
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_console_free(c: *mut KnConsole) {
    if !c.is_null() {
        drop(unsafe { Box::from_raw(c) });
    }
}

/// Empreinte du certificat présenté par la console (nulle avant tout contact).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_console_fingerprint_seen(c: *const KnConsole) -> *mut c_char {
    match unsafe { c.as_ref() }.and_then(|c| c.0.fingerprint_seen()) {
        Some(fp) => give(fp),
        None => std::ptr::null_mut(),
    }
}

/// Codes de retour des appels à la console : `KN_OK`, `KN_ERROR` (console
/// injoignable, réponse illisible), `KN_CERTIFICATE_CHANGED`, ou l'opposé
/// du code HTTP d'une réponse d'erreur (−401 : compte refusé, −404 : console
/// d'une version qui ne connaît pas l'appel) ; `error` porte le message.
pub const KN_OK: i32 = 0;
pub const KN_ERROR: i32 = 1;
pub const KN_CERTIFICATE_CHANGED: i32 = 3;

fn failure(e: ConsoleError, error: *mut *mut c_char) -> i32 {
    let code = match &e {
        ConsoleError::Http { status, .. } => -(*status as i32),
        ConsoleError::CertificateChanged { .. } => KN_CERTIFICATE_CHANGED,
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
    let Some(c) = (unsafe { c.as_ref() }) else { return KN_ERROR };
    console_result(c.0.info(), out, error)
}

/// Premier contact sans compte : la poignée de main suffit à voir le
/// certificat (`kn_console_fingerprint_seen`), la console répondant 401.
/// `out` : l'info si le compte est accepté, sinon nul.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_console_probe(c: *const KnConsole, out: *mut *mut c_char, error: *mut *mut c_char) -> i32 {
    let Some(c) = (unsafe { c.as_ref() }) else { return KN_ERROR };
    match c.0.probe() {
        Ok(Some(i)) => {
            give_to(out, serde_json::to_string(&i).unwrap_or_default());
            KN_OK
        }
        Ok(None) => KN_OK,
        Err(e) => failure(e, error),
    }
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
    let Some(c) = (unsafe { c.as_ref() }) else { return KN_ERROR };
    match c.0.station_restart(optional(device_id).as_deref(), force) {
        Ok(()) => {
            give_to(out, serde_json::json!({ "ok": true }).to_string());
            KN_OK
        }
        Err(e) => station_failure(e, out, error),
    }
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
    let Some(c) = (unsafe { c.as_ref() }) else { return KN_ERROR };
    match c.0.station_update(optional(device_id).as_deref(), force) {
        Ok(version) => {
            give_to(out, serde_json::json!({ "version": version }).to_string());
            KN_OK
        }
        Err(e) => station_failure(e, out, error),
    }
}

/// Adresse d'Admin à ouvrir dans le navigateur (`https://adresse:port/`,
/// IPv6 entre crochets) ; à libérer par `kn_string_free`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_console_admin_url(c: *const KnConsole) -> *mut c_char {
    match unsafe { c.as_ref() } {
        Some(c) => give(c.0.admin_url()),
        None => std::ptr::null_mut(),
    }
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
    match wake::send(&wake::targets_from_json(&text(targets_json))) {
        Ok(_) => KN_OK,
        Err(wake::WakeError::NoTarget) => KN_WAKE_NO_TARGET,
        Err(e) => {
            log::warn!("réveil : {e}");
            KN_ERROR
        }
    }
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
            screen: self.screen,
        }
    }
}

impl PacketHandler for Sink {
    fn packet(&self, packet: Packet<'_>) {
        let p = match packet {
            Packet::Codec { codec } => KnPacket { codec, ..self.empty(KN_PACKET_CODEC) },
            Packet::Media { is_config, is_key, pts, data } => KnPacket {
                is_config,
                is_key,
                pts,
                data: data.as_ptr(),
                len: data.len(),
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
}

struct ApplePlatform {
    cb: KnPlatform,
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
        Some(Arc::new(Sink { cb, userdata: self.cb.userdata, stream, screen }))
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
            Status::Ending => (self.ending_error.lock().unwrap().take(), None),
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

    /// Gardée pour `KN_STATUS_ENDING`, qui suit tout de suite.
    fn on_ending(&self, error: Option<&str>) {
        *self.ending_error.lock().unwrap() = error.map(str::to_string);
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
        }
        unsafe { f(self.cb.userdata, &n) };
    }
}

// MARK: - Session : réglages (JSON)

#[derive(Deserialize)]
struct SettingsWire {
    /// `jeu`, `bureau`, `max` ; absent : personnalisé.
    profile: Option<String>,
    codec: String,
    #[serde(default)]
    yuv444: bool,
    mbps: u32,
    #[serde(default)]
    reliable: bool,
}

impl SettingsWire {
    fn settings(&self) -> Result<StreamSettings, String> {
        let codec = Codec::parse(&self.codec).ok_or_else(|| format!("format inconnu : {}", self.codec))?;
        Ok(StreamSettings {
            profile: self.profile.as_deref().and_then(Profile::parse),
            codec,
            yuv444: self.yuv444,
            mbps: self.mbps,
            reliable: self.reliable,
        })
    }
}

#[derive(Deserialize)]
struct RecoveryWire {
    #[serde(default)]
    watch_image: bool,
    /// Plafond du repli, en bit/s (comme `Recovery::fallback_bitrate`).
    fallback_bitrate: Option<u32>,
    /// Dernier recours (`Recovery::restart_station`) : vrai si absent.
    #[serde(default = "yes")]
    restart_station: bool,
}

impl Default for RecoveryWire {
    fn default() -> Self {
        Self { watch_image: false, fallback_bitrate: None, restart_station: true }
    }
}

#[derive(Deserialize)]
struct ParamsWire {
    console_host: String,
    console_port: u16,
    user: String,
    password: String,
    console_fingerprint: Option<String>,
    device: DeviceInfo,
    mode: Mode,
    #[serde(default)]
    audio_channels: u16,
    #[serde(default = "yes")]
    virtual_display: bool,
    settings: SettingsWire,
    #[serde(default = "yes")]
    auto_reconnect: bool,
    #[serde(default = "yes")]
    gamepad_xbox360: bool,
    #[serde(default = "yes")]
    clipboard: bool,
    #[serde(default)]
    recovery: RecoveryWire,
    /// Multi-écrans : les écrans de l'appareil (`list`, `limit_4k`).
    #[serde(default)]
    screens: Option<Screens>,
}

fn yes() -> bool {
    true
}

/// Une session (voir `kybernow_core::session`).
pub struct KnSession {
    session: Arc<Session>,
}

/// Ouvre une session. `params_json` : `console_host`, `console_port`,
/// `user`, `password`, `console_fingerprint` (optionnel), `device` (`id`,
/// `name`, `kind`, `os`, `app`), `mode` (`width`, `height`, `refresh`),
/// `audio_channels` (0 : son laissé sur l'ordinateur), `virtual_display`,
/// `settings` (`profile`, `codec`, `yuv444`, `mbps`, `reliable`),
/// `auto_reconnect`, `gamepad_xbox360`, `clipboard`, `recovery`
/// (`watch_image`, `fallback_bitrate`, `restart_station` : vrai si absent),
/// et pour le multi-écrans `screens` : `list`, les écrans de l'appareil
/// (`x`, `y`, `width`, `height` : cadre dans le repère logique de l'appareil,
/// y vers le bas ; `pixel_width`, `pixel_height` : définition native ;
/// `refresh` ; `primary`), `current` (« Cet écran » : celui de la fenêtre),
/// `limit_4k` (« Au plus 4K »), `custom_resolution`, `setting` (`auto`,
/// `this`, `all`) et `max_screens` (`virtual_display.max_screens` de
/// `kn_console_info` ; absent : 1). Le cœur en tire le nombre d'écrans, au
/// départ et quand le profil change (`kn_session_set_settings`). `mode` :
/// le mode de « Cet écran », comme sans `screens`. La plateforme reçoit
/// `on_screens` et les paquets de chaque écran (`KnPacket::screen`).
/// Nul si les paramètres sont illisibles (`error`). La session commence tout
/// de suite ; `on_status` suit.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_session_start(
    params_json: *const c_char,
    platform: *const KnPlatform,
    error: *mut *mut c_char,
) -> *mut KnSession {
    let Some(cb) = (unsafe { platform.as_ref() }) else { return std::ptr::null_mut() };
    let wire: ParamsWire = match serde_json::from_str(&text(params_json)) {
        Ok(w) => w,
        Err(e) => {
            give_to(error, e.to_string());
            return std::ptr::null_mut();
        }
    };
    let settings = match wire.settings.settings() {
        Ok(s) => s,
        Err(e) => {
            give_to(error, e);
            return std::ptr::null_mut();
        }
    };
    let params = SessionParams {
        console_host: wire.console_host,
        console_port: wire.console_port,
        user: wire.user,
        password: wire.password,
        console_fingerprint: wire.console_fingerprint,
        device: wire.device,
        mode: wire.mode,
        audio_channels: wire.audio_channels,
        virtual_display: wire.virtual_display,
        settings,
        auto_reconnect: wire.auto_reconnect,
        gamepad_xbox360: wire.gamepad_xbox360,
        clipboard: wire.clipboard,
        recovery: Recovery {
            watch_image: wire.recovery.watch_image,
            fallback_bitrate: wire.recovery.fallback_bitrate,
            restart_station: wire.recovery.restart_station,
        },
    };
    let platform = Arc::new(ApplePlatform { cb: *cb, streams: AtomicU64::new(0), ending_error: Mutex::new(None) });
    let session = match wire.screens {
        Some(screens) => Session::start_with_screens(params, screens, platform),
        None => Session::start(params, platform),
    };
    Box::into_raw(Box::new(KnSession { session }))
}

fn with<R>(s: *const KnSession, f: impl FnOnce(&Session) -> R) -> Option<R> {
    unsafe { s.as_ref() }.map(|s| f(&s.session))
}

/// Demande la fin de la session (sans attendre : `KN_STATUS_ENDED` suit).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_session_stop(s: *const KnSession) {
    with(s, |s| s.stop());
}

/// Attend la fin du fil de session (console prévenue). Jamais depuis un rappel.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_session_wait(s: *const KnSession) {
    with(s, |s| s.wait());
}

/// Libère la session (après `kn_session_wait`, ou l'état terminé reçu).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_session_free(s: *mut KnSession) {
    if !s.is_null() {
        drop(unsafe { Box::from_raw(s) });
    }
}

/// Une image clé demandée une fois (plateforme qui tient sa propre veille).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_session_request_keyframe(s: *const KnSession) {
    with(s, |s| s.request_keyframe());
}

/// Le décodeur a perdu une image de référence : image clé redemandée toutes
/// les 300 ms jusqu'à `kn_session_keyframe_recovered`, flux relancé à 1,5 s.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_session_keyframe_lost(s: *const KnSession) {
    with(s, |s| s.keyframe_lost());
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_session_keyframe_recovered(s: *const KnSession) {
    with(s, |s| s.keyframe_recovered());
}

/// Aucune image depuis le démarrage du flux (plateforme qui tient sa propre veille).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_session_no_image(s: *const KnSession) {
    with(s, |s| s.no_image());
}

/// Autres réglages pendant la session (profil depuis la barre), même JSON que
/// `settings` au départ : débit changé sur-le-champ, ou flux relancé si le
/// format, le 4:4:4 ou le transport changent.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_session_set_settings(s: *const KnSession, settings_json: *const c_char) -> i32 {
    let wire: SettingsWire = match serde_json::from_str(&text(settings_json)) {
        Ok(w) => w,
        Err(_) => return KN_ERROR,
    };
    let Ok(settings) = wire.settings() else { return KN_ERROR };
    with(s, |s| s.set_settings(settings));
    KN_OK
}

/// Plafond de débit (bit/s) le temps que la fenêtre est occultée ; 0 le retire.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_session_set_bitrate_cap(s: *const KnSession, cap: u32) {
    with(s, |s| s.set_bitrate_cap((cap > 0).then_some(cap)));
}

/// L'app fige l'image (arrière-plan, fenêtre cachée) ou la reprend : la
/// veille « aucune image » attend le retour.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_session_set_hidden(s: *const KnSession, hidden: bool) {
    with(s, |s| s.set_hidden(hidden));
}

/// Taille des images décodées par l'app (à chaque changement).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_session_decoded_size(s: *const KnSession, width: u32, height: u32) {
    with(s, |s| s.decoded_size(width, height));
}

/// Sans écran virtuel : afficher un autre écran de l'ordinateur.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_session_pick_display(s: *const KnSession, id: u32) {
    with(s, |s| s.pick_display(id));
}

/// L'écran de l'appareil a changé de mode : la console refait l'écran
/// virtuel à ce mode, puis l'image repart dessus.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_session_set_mode(s: *const KnSession, mode: KnMode) {
    with(s, |s| s.set_mode(Mode { width: mode.width, height: mode.height, refresh: mode.refresh }));
}

/// « Remettre » : la console remet le mode demandé au début de la session.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_session_restore_mode(s: *const KnSession) {
    with(s, |s| s.restore_mode());
}

/// Multi-écrans : les écrans de l'appareil ont changé (même JSON que
/// `screens` dans `kn_session_start`) ; la session se refait avec eux, sans
/// effet sur une session ouverte sans `screens`. `KN_ERROR` si illisible.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_session_set_screens(s: *const KnSession, screens_json: *const c_char) -> i32 {
    let Ok(screens) = serde_json::from_str::<Screens>(&text(screens_json)) else { return KN_ERROR };
    with(s, |s| s.set_screens(screens));
    KN_OK
}

/// Réglage « Écrans » : faux avec une définition sur mesure ou sans écran
/// virtuel ; il est alors inactif, sur « Cet écran ».
#[unsafe(no_mangle)]
pub extern "C" fn kn_screens_setting_enabled(custom_resolution: bool, virtual_display: bool) -> bool {
    DisplaysChoice { setting: DisplaysSetting::Auto, profile: None, custom_resolution, virtual_display }.setting_enabled()
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
}

/// Plusieurs ordinateurs : les écrans d'une session ouverte pendant qu'une
/// autre tourne, « Cet écran » quel que soit le réglage
/// (`screens::screens_for_extra_session`) : même JSON que `screens` de
/// `kn_session_start`, `current` étant l'écran de la fenêtre active, où
/// s'ouvre la nouvelle. Nul si illisible. À libérer par `kn_string_free`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_screens_for_extra_session(screens_json: *const c_char) -> *mut c_char {
    let Ok(screens) = serde_json::from_str::<Screens>(&text(screens_json)) else { return std::ptr::null_mut() };
    match serde_json::to_string(&screens::screens_for_extra_session(&screens)) {
        Ok(json) => give(json),
        Err(_) => std::ptr::null_mut(),
    }
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
    let Ok(screens) = serde_json::from_str::<Screens>(&text(screens_json)) else { return false };
    let (Some(screen), Some(out)) = (screens.list.get(screens.current), unsafe { out.as_mut() }) else { return false };
    let w = screens::extra_session_window(screen);
    *out = KnExtraWindow { x: w.x, y: w.y, width: w.width, height: w.height, mode: w.mode.into() };
    true
}

/// Index de l'écran principal dans `list` (même JSON que `screens`) : celui
/// où s'affiche l'écran de connexion. -1 si illisible ou vide.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_screens_primary_index(screens_json: *const c_char) -> i32 {
    match serde_json::from_str::<Screens>(&text(screens_json)) {
        Ok(s) if !s.list.is_empty() => screens::primary_index(&s.list) as i32,
        _ => -1,
    }
}

/// Image `pts` reçue, décodée puis montrée (µs, `kn_now_micros`).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_session_frame_displayed(s: *const KnSession, pts: u64, received: i64, decoded: i64, at: i64) {
    with(s, |s| s.frame_displayed(pts, received, decoded, at));
}

/// Image remplacée par une plus récente avant d'être montrée.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_session_frame_skipped(s: *const KnSession, pts: u64) {
    with(s, |s| s.latency().skipped(pts));
}

/// Paquet d'image reçu (débit).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_session_bytes_received(s: *const KnSession, bytes: usize) {
    with(s, |s| s.latency().received_bytes(bytes));
}

/// Trou dans le flux.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_session_hole(s: *const KnSession) {
    with(s, |s| s.latency().hole());
}

// MARK: - Plusieurs ordinateurs à la fois (docs/plusieurs-ordinateurs.md)

/// La session devient active (vrai) ou passe derrière une autre : pas
/// active, rien ne part de l'appareil vers son ordinateur (clavier, souris,
/// texte, touches et axes des manettes), ce qu'elle tenait est relâché, ses
/// manettes sont au repos, la souris relative est coupée, le presse-papiers
/// est en pause et les vibrations sont ignorées ; de nouveau active, le
/// contenu du presse-papiers de l'appareil est renvoyé à l'ordinateur. Tenu
/// d'une image à l'autre. Active par défaut ; le registre (`kn_focus_*`)
/// s'en charge pour les sessions inscrites.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_session_set_active(s: *const KnSession, active: bool) {
    with(s, |s| s.set_active(active));
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_session_is_active(s: *const KnSession) -> bool {
    with(s, |s| s.is_active()).unwrap_or(true)
}

/// Son de la session coupé (vrai) ou rendu, fondu de 20 ms, sur chaque
/// `KnAudio` donné par `kn_session_attach_audio`. Tenu d'une image à
/// l'autre. Pas coupé par défaut ; le registre s'en charge pour les
/// sessions inscrites (sauf « Garder le son »).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_session_set_muted(s: *const KnSession, muted: bool) {
    with(s, |s| s.set_muted(muted));
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_session_is_muted(s: *const KnSession) -> bool {
    with(s, |s| s.is_muted()).unwrap_or(false)
}

/// Le moteur de son de la session (le son va à Swift, qui le donne à son
/// `KnAudio` : le cœur ne le voit pas passer). À appeler pour chaque
/// `KnAudio` de la session, dès sa création : il prend tout de suite l'état
/// « son coupé » de la session, puis le suit. Le `KnAudio` peut être libéré
/// sans prévenir.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_session_attach_audio(s: *const KnSession, audio: *const audio::KnAudio) {
    let Some(a) = (unsafe { audio.as_ref() }) else { return };
    with(s, |s| s.attach_audio(&a.0));
}

/// Inscrit la session dans le registre des sessions de l'app, `name` : le
/// nom de son ordinateur (au début des lignes de son journal). Elle devient
/// active ; les autres passent derrière, leur son coupé (sauf « Garder le
/// son »). Rend son identifiant (positif), le même si elle est déjà
/// inscrite ; -1 au-delà de quatre sessions (`kn_focus_max`) ou sans session.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_focus_register(s: *const KnSession, name: *const c_char) -> i64 {
    let name = text(name);
    with(s, |s| focus::global().register(s, &name).map_or(-1, |id| id.as_u64() as i64)).unwrap_or(-1)
}

fn focus_id(id: i64) -> Option<FocusId> {
    (id > 0).then(|| FocusId::from_u64(id as u64))
}

/// Désinscrit une session (fenêtre fermée, « Se déconnecter », fin de la
/// session) : elle passe derrière, son son est coupé ; si elle avait la main,
/// la session qui l'avait juste avant la reprend.
#[unsafe(no_mangle)]
pub extern "C" fn kn_focus_unregister(id: i64) {
    if let Some(id) = focus_id(id) {
        focus::global().unregister(id);
    }
}

/// La fenêtre de cette session passe au premier plan : elle devient active.
#[unsafe(no_mangle)]
pub extern "C" fn kn_focus_activate(id: i64) {
    if let Some(id) = focus_id(id) {
        focus::global().activate(id);
    }
}

/// « Garder le son » : la session reste audible sans être active.
#[unsafe(no_mangle)]
pub extern "C" fn kn_focus_set_keep_sound(id: i64, keep: bool) {
    if let Some(id) = focus_id(id) {
        focus::global().set_keep_sound(id, keep);
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn kn_focus_keep_sound(id: i64) -> bool {
    focus_id(id).is_some_and(|id| focus::global().keep_sound(id))
}

/// La session active (-1 : aucune).
#[unsafe(no_mangle)]
pub extern "C" fn kn_focus_active() -> i64 {
    focus::global().active().map_or(-1, |id| id.as_u64() as i64)
}

/// « Ordinateur suivant » : la session ouverte après l'active, dans l'ordre
/// d'ouverture (-1 : aucune). À l'app de faire passer sa fenêtre devant.
#[unsafe(no_mangle)]
pub extern "C" fn kn_focus_next() -> i64 {
    focus::global().next().map_or(-1, |id| id.as_u64() as i64)
}

/// Sessions inscrites.
#[unsafe(no_mangle)]
pub extern "C" fn kn_focus_count() -> u32 {
    focus::global().count() as u32
}

/// Sessions au plus (4).
#[unsafe(no_mangle)]
pub extern "C" fn kn_focus_max() -> u32 {
    focus::MAX_SESSIONS as u32
}

/// Mac : le lecteur de manettes du cœur, sur son fil SDL, gardé pour toute
/// la vie de l'app, manettes ouvertes dès l'appel (diode de joueur, mode
/// HIDAPI). Inutile d'ordinaire : le lecteur démarre avec la première chaîne
/// d'entrées et se ferme 5 s après la dernière ; seulement pour lire les
/// manettes hors session. Faux sur l'Apple TV, l'iPhone et l'iPad (pas de
/// SDL : leurs manettes passent par GameController dans l'app), si libkynput
/// a encore son lecteur SDL, ou si SDL n'a pas pu s'ouvrir.
#[unsafe(no_mangle)]
pub extern "C" fn kn_gamepads_start_sdl() -> bool {
    gamepads::start_sdl()
}

/// Latence de la session : médiane des secondes et 95e centile (ms). Faux
/// sans mesure.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_session_latency(s: *const KnSession, median: *mut f64, p95: *mut f64) -> bool {
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
}
