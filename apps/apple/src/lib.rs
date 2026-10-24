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

use kybernow_core::console::{Console, ConsoleError, DeviceInfo, Mode, PairStatus};
use kybernow_core::i18n::{self, Lang};
use kybernow_core::kynput_rs::Pipeline;
use kybernow_core::latency::{self, Second};
use kybernow_core::media::{Packet, PacketHandler};
use kybernow_core::profile::{Codec, Profile, StreamSettings};
use kybernow_core::session::{EndReport, Notice, Platform, Recovery, Session, SessionParams, Status};
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
/// `admin_version`, `virtual_display.available`, `audio.speakers`).
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
}

pub type KnPacketCb = Option<unsafe extern "C" fn(userdata: *mut c_void, packet: *const KnPacket)>;

/// Un flux (image ou son) : le rappel de Swift, retenu par Kyber jusqu'au
/// dernier paquet ; sa libération signale `KN_PACKET_CLOSED`.
struct Sink {
    cb: unsafe extern "C" fn(*mut c_void, *const KnPacket),
    userdata: *mut c_void,
    stream: u64,
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

pub const KN_STATUS_PREPARING: u32 = 0;
pub const KN_STATUS_CONNECTING: u32 = 1;
pub const KN_STATUS_FIRST_IMAGES: u32 = 2;
pub const KN_STATUS_STREAMING: u32 = 3;
pub const KN_STATUS_RECONNECTING: u32 = 4;
pub const KN_STATUS_ENDING: u32 = 5;
pub const KN_STATUS_ENDED: u32 = 6;

/// État de la session. Terminée : `error` dit pourquoi si la fin n'était
/// pas voulue ; `has_report` et les champs suivants, ce que la console a
/// confirmé (écrans et son rétablis, ou son erreur).
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
}

struct ApplePlatform {
    cb: KnPlatform,
    streams: AtomicU64,
}

unsafe impl Send for ApplePlatform {}
unsafe impl Sync for ApplePlatform {}

impl ApplePlatform {
    fn sink(&self, cb: KnPacketCb) -> Option<Arc<dyn PacketHandler>> {
        let cb = cb?;
        let stream = self.streams.fetch_add(1, Ordering::Relaxed) + 1;
        Some(Arc::new(Sink { cb, userdata: self.cb.userdata, stream }))
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
        self.sink(self.cb.video_packet).unwrap_or_else(|| Arc::new(Mute))
    }

    fn audio_handler(&self) -> Option<Arc<dyn PacketHandler>> {
        if !self.cb.has_audio {
            return None;
        }
        self.sink(self.cb.audio_packet)
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
            Status::Ending => (KN_STATUS_ENDING, 0, 0),
            Status::Ended { .. } => (KN_STATUS_ENDED, 0, 0),
        };
        let (error, report): (Option<String>, Option<EndReport>) = match status {
            Status::Ended { error, report } => (error.clone(), report.clone()),
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

#[derive(Deserialize, Default)]
struct RecoveryWire {
    #[serde(default)]
    watch_image: bool,
    /// Plafond du repli, en bit/s (comme `Recovery::fallback_bitrate`).
    fallback_bitrate: Option<u32>,
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
/// (`watch_image`, `fallback_bitrate`). Nul si les paramètres sont illisibles
/// (`error`). La session commence tout de suite ; `on_status` suit.
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
        },
    };
    let platform = Arc::new(ApplePlatform { cb: *cb, streams: AtomicU64::new(0) });
    let session = Session::start(params, platform);
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
