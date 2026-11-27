//! Enregistrement des sessions (docs/enregistrement.md, textes dans
//! docs/link-ui.md, « Enregistrement de la session ») : Kyber écrit le fichier
//! (correctif 0070 : `POST /kybernow/recording`, l'image de l'hôte et le son
//! tels qu'ils sont encodés, en MP4) ; Admin décide qui peut le lancer
//! (l'appareil en session, et Admin), où il va, combien de temps on le garde,
//! et le montre à tous.
//!
//! - l'appareil en session (compte de l'appareil, comme le partage) :
//!   `POST /api/stream/recording` `{"record": true|false, "device_id"}` ;
//!   l'état dans chaque signe de vie (`recording`) ;
//! - les invités du partage : l'état dans leur signe de vie (`recording`) ;
//! - Admin : `GET /api/recording`, `PUT /api/recording/settings`,
//!   `POST /api/recording`, `GET|DELETE /api/recording/files/{nom}`.
//!
//! La veille (toutes les 2 s pendant un enregistrement) relit l'état de
//! Kyber, l'arrête sous 2 Go libres et quand un prêt commence ; toutes les
//! heures, elle efface les enregistrements plus vieux que la durée choisie.

use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use axum::Json;
use axum::body::Body;
use axum::extract::{Path as AxPath, Query, State};
use axum::http::{HeaderMap, HeaderValue, Method, StatusCode, header};
use axum::response::{IntoResponse, Response};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::AppState;
use crate::i18n::Msg;

/// Sous ce qui reste libre, on n'enregistre pas (et on s'arrête).
pub const MIN_FREE_BYTES: u64 = 2 * 1024 * 1024 * 1024;
/// Les fichiers d'Admin commencent par ce nom : ce sont les seuls effacés.
const PREFIX: &str = "KyberNow ";
const DEFAULT_KEEP_DAYS: u32 = 30;
const KEEP_CHOICES: [u32; 4] = [7, 30, 90, 0];

// --- Réglages ------------------------------------------------------------------

fn yes() -> bool {
    true
}

fn default_keep() -> u32 {
    DEFAULT_KEEP_DAYS
}

/// `[recording]` des réglages d'Admin.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RecordingSettings {
    /// « Permettre l'enregistrement des sessions » (allumé par défaut).
    #[serde(default = "yes")]
    pub allowed: bool,
    /// Le dossier choisi ; `None` : Vidéos › KyberNow (Films sur Mac).
    #[serde(default)]
    pub folder: Option<PathBuf>,
    /// Jours gardés : 7, 30, 90 ; 0 : toujours.
    #[serde(default = "default_keep")]
    pub keep_days: u32,
}

impl Default for RecordingSettings {
    fn default() -> Self {
        RecordingSettings { allowed: true, folder: None, keep_days: DEFAULT_KEEP_DAYS }
    }
}

/// Vidéos › KyberNow de l'utilisateur de l'ordinateur.
pub fn default_folder() -> PathBuf {
    videos_dir().join("KyberNow")
}

#[cfg(windows)]
fn videos_dir() -> PathBuf {
    // Le service tourne sous SYSTEM : le profil est celui de la session ouverte.
    crate::display::agent::user_profile_dir()
        .map(|p| p.join("Videos"))
        .unwrap_or_else(|_| PathBuf::from(std::env::var_os("PUBLIC").unwrap_or_else(|| "C:\\Users\\Public".into())).join("Videos"))
}

#[cfg(target_os = "macos")]
fn videos_dir() -> PathBuf {
    home().join("Movies")
}

#[cfg(all(unix, not(target_os = "macos")))]
fn videos_dir() -> PathBuf {
    let config = std::env::var_os("XDG_CONFIG_HOME").map(PathBuf::from).unwrap_or_else(|| home().join(".config"));
    std::fs::read_to_string(config.join("user-dirs.dirs"))
        .ok()
        .and_then(|text| xdg_videos(&text, &home()))
        .unwrap_or_else(|| home().join("Videos"))
}

#[cfg(unix)]
fn home() -> PathBuf {
    std::env::var_os("HOME").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("/tmp"))
}

/// `XDG_VIDEOS_DIR="$HOME/Vidéos"` de `user-dirs.dirs`.
#[cfg_attr(any(windows, target_os = "macos"), allow(dead_code))]
fn xdg_videos(text: &str, home: &Path) -> Option<PathBuf> {
    let line = text.lines().map(str::trim).find(|l| l.starts_with("XDG_VIDEOS_DIR="))?;
    let value = line.trim_start_matches("XDG_VIDEOS_DIR=").trim_matches('"');
    let path = match value.strip_prefix("$HOME") {
        Some(rest) => home.join(rest.trim_start_matches('/')),
        None => PathBuf::from(value),
    };
    // `$HOME/` seul : le dossier personnel (pas de dossier Vidéos).
    (path != home && path.is_absolute()).then_some(path)
}

pub fn folder(s: &AppState) -> PathBuf {
    s.settings.lock().unwrap().recording.folder.clone().unwrap_or_else(default_folder)
}

fn settings(s: &AppState) -> RecordingSettings {
    s.settings.lock().unwrap().recording.clone()
}

// --- Place libre -----------------------------------------------------------------

/// Octets libres sous `dir` (ou son premier parent qui existe).
pub fn free_bytes(dir: &Path) -> Option<u64> {
    let mut at = dir;
    while !at.exists() {
        at = at.parent()?;
    }
    free_at(at)
}

#[cfg(windows)]
fn free_at(dir: &Path) -> Option<u64> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::GetDiskFreeSpaceExW;
    let wide: Vec<u16> = dir.as_os_str().encode_wide().chain(Some(0)).collect();
    let mut free = 0u64;
    let ok = unsafe { GetDiskFreeSpaceExW(wide.as_ptr(), &mut free, std::ptr::null_mut(), std::ptr::null_mut()) };
    (ok != 0).then_some(free)
}

#[cfg(unix)]
fn free_at(dir: &Path) -> Option<u64> {
    use std::os::unix::ffi::OsStrExt;
    let c = std::ffi::CString::new(dir.as_os_str().as_bytes()).ok()?;
    let mut st: libc::statvfs = unsafe { std::mem::zeroed() };
    if unsafe { libc::statvfs(c.as_ptr(), &mut st) } != 0 {
        return None;
    }
    Some(st.f_bavail as u64 * st.f_frsize as u64)
}

// --- État --------------------------------------------------------------------------

#[derive(Debug, Clone)]
struct Active {
    /// Heure de Kyber (ms Unix).
    started_at_ms: u64,
    /// L'appareil qui l'a lancé, ou `None` (Admin, ou repris au démarrage).
    by: Option<String>,
    files: Vec<String>,
    bytes: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EndReason {
    Stopped,
    Ended,
    Write,
    DiskFull,
    Codec,
    Lent,
}

#[derive(Debug, Clone, Serialize)]
struct LastEnd {
    reason: EndReason,
    at: u64,
}

#[derive(Default)]
pub struct Recorder {
    state: Mutex<(Option<Active>, Option<LastEnd>)>,
    /// Un lancement ou un arrêt à la fois.
    busy: tokio::sync::Mutex<()>,
}

fn now_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_millis() as u64)
}

impl Recorder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn active(&self) -> bool {
        self.state.lock().unwrap().0.is_some()
    }

    fn current_files(&self) -> Vec<String> {
        self.state.lock().unwrap().0.as_ref().map(|a| a.files.clone()).unwrap_or_default()
    }
}

/// Ce que voient les apps (signe de vie de l'hôte, des invités) : `allowed`,
/// `active`, `elapsed` (secondes, compté par la Station : tous les appareils
/// montrent le même temps), `started_at` (ms Unix), `last_end`.
pub fn for_session(s: &AppState) -> Value {
    let allowed = settings(s).allowed;
    let (active, last) = s.recording.state.lock().unwrap().clone();
    let now = now_ms();
    json!({
        "allowed": allowed,
        "active": active.is_some(),
        "elapsed": active.as_ref().map(|a| now.saturating_sub(a.started_at_ms) / 1000),
        "started_at": active.as_ref().map(|a| a.started_at_ms),
        "last_end": last,
    })
}

// --- Refus ---------------------------------------------------------------------

/// Refus : le message (langue de la requête) et un code stable (`disabled`,
/// `not_in_session`, `no_session`, `lent`, `codec`, `disk_full`, `folder`,
/// `unavailable`, `failed`).
pub struct RecError {
    status: StatusCode,
    code: &'static str,
    error: String,
}

impl RecError {
    fn new(status: StatusCode, code: &'static str, msg: Msg) -> Self {
        RecError { status, code, error: msg.text() }
    }
}

impl IntoResponse for RecError {
    fn into_response(self) -> Response {
        (self.status, Json(json!({ "error": self.error, "code": self.code }))).into_response()
    }
}

type RecResult<T> = Result<T, RecError>;

fn conflict(code: &'static str, key: &str) -> RecError {
    RecError::new(StatusCode::CONFLICT, code, Msg::new(key))
}

// --- Lancer, arrêter --------------------------------------------------------------

/// Le nom d'un fichier : `KyberNow 2026-11-26 14-32-05`, à l'heure de l'ordinateur.
fn file_base() -> String {
    let now = time::OffsetDateTime::now_local().unwrap_or_else(|_| time::OffsetDateTime::now_utc());
    format!(
        "{PREFIX}{:04}-{:02}-{:02} {:02}-{:02}-{:02}",
        now.year(),
        now.month() as u8,
        now.day(),
        now.hour(),
        now.minute(),
        now.second()
    )
}

/// Lance l'enregistrement de la session en cours ; `by` : le nom de
/// l'appareil qui le demande (`None` : Admin).
pub async fn start(s: &AppState, by: Option<String>) -> RecResult<Value> {
    let _busy = s.recording.busy.lock().await;
    if s.recording.active() {
        return Ok(for_session(s));
    }
    if !settings(s).allowed {
        return Err(conflict("disabled", "recording.disabled"));
    }
    if let Some(name) = s.lend.borrower() {
        return Err(RecError::new(StatusCode::CONFLICT, "lent", Msg::new("lend.inUse").with("name", name)));
    }
    let Some(live) = s.activity.live() else {
        return Err(conflict("no_session", "recording.noSession"));
    };
    let dir = folder(s);
    if std::fs::create_dir_all(&dir).is_err() {
        return Err(conflict("folder", "recording.folder"));
    }
    if free_bytes(&dir).is_some_and(|f| f < MIN_FREE_BYTES) {
        return Err(conflict("disk_full", "recording.diskFull"));
    }
    let body = json!({ "record": true, "dir": dir.display().to_string(), "name": file_base() });
    let (status, v) = crate::kyber_proxy::kyber_call(s, Method::POST, "/kybernow/recording", Some(body))
        .await
        .map_err(|e| {
            tracing::warn!("enregistrement : Kyber injoignable : {e:#}");
            conflict("failed", "recording.failed")
        })?;
    match status {
        StatusCode::OK => {}
        StatusCode::NOT_FOUND => return Err(conflict("unavailable", "recording.unavailable")),
        StatusCode::CONFLICT => {
            return Err(match v["code"].as_str() {
                Some("codec") => conflict("codec", "recording.codec"),
                Some("folder") => conflict("folder", "recording.folder"),
                Some("no_session") => conflict("no_session", "recording.noSession"),
                _ => conflict("failed", "recording.failed"),
            });
        }
        other => {
            tracing::warn!("enregistrement : Kyber répond {other} : {v}");
            return Err(conflict("failed", "recording.failed"));
        }
    }
    let started_at_ms = v["started_at_ms"].as_u64().unwrap_or_else(now_ms);
    let name = by.clone().unwrap_or_else(|| live.name.clone());
    *s.recording.state.lock().unwrap() = (
        Some(Active { started_at_ms, by, files: files_of(&v), bytes: v["bytes"].as_u64().unwrap_or(0) }),
        None,
    );
    tracing::info!("enregistrement lancé ({name}) dans {}", dir.display());
    s.activity.event("info", Msg::new("events.recordingStarted").with("name", name));
    Ok(for_session(s))
}

fn files_of(v: &Value) -> Vec<String> {
    v["files"].as_array().map(|a| a.iter().filter_map(|f| f.as_str().map(str::to_owned)).collect()).unwrap_or_default()
}

/// Arrête l'enregistrement (s'il y en a un) et note pourquoi.
pub async fn stop(s: &AppState, reason: EndReason) -> Value {
    let _busy = s.recording.busy.lock().await;
    if s.recording.active() {
        match crate::kyber_proxy::kyber_call(s, Method::POST, "/kybernow/recording", Some(json!({ "record": false }))).await {
            Ok((StatusCode::OK, _)) => {}
            Ok((status, v)) => tracing::warn!("enregistrement : arrêt refusé par Kyber ({status}) : {v}"),
            Err(e) => tracing::warn!("enregistrement : Kyber injoignable pour l'arrêt : {e:#}"),
        }
        ended(s, reason);
    }
    for_session(s)
}

/// L'enregistrement est fini (arrêté ici ou par Kyber) : état, fil d'Admin.
fn ended(s: &AppState, reason: EndReason) {
    let active = {
        let mut st = s.recording.state.lock().unwrap();
        let active = st.0.take();
        if active.is_some() {
            st.1 = Some(LastEnd { reason, at: now_ms() });
        }
        active
    };
    let Some(active) = active else { return };
    let seconds = now_ms().saturating_sub(active.started_at_ms) / 1000;
    tracing::info!("enregistrement terminé ({reason:?}, {seconds} s, {} octets, {:?})", active.bytes, active.files);
    let msg = match reason {
        EndReason::DiskFull | EndReason::Write => Msg::new("events.recordingDiskFull"),
        EndReason::Codec => Msg::new("events.recordingCodec"),
        _ => Msg::new("events.recordingSaved").with("duration", json!({ "duration": seconds })),
    };
    s.activity.event(if matches!(reason, EndReason::DiskFull | EndReason::Write | EndReason::Codec) { "warn" } else { "info" }, msg);
}

// --- Veille --------------------------------------------------------------------

pub fn supervise(s: AppState) {
    tokio::spawn(async move {
        // Admin redémarré pendant un enregistrement de Kyber : il le reprend.
        adopt(&s).await;
        let mut tick = tokio::time::interval(Duration::from_secs(2));
        let mut last_cleanup: Option<std::time::Instant> = None;
        loop {
            tick.tick().await;
            if s.recording.active() {
                watch(&s).await;
            }
            if last_cleanup.is_none_or(|t| t.elapsed() >= Duration::from_secs(3600)) {
                last_cleanup = Some(std::time::Instant::now());
                let st = s.clone();
                let _ = tokio::task::spawn_blocking(move || cleanup(&st)).await;
            }
        }
    });
}

async fn kyber_state(s: &AppState) -> Option<Value> {
    match crate::kyber_proxy::kyber_call(s, Method::GET, "/kybernow/recording", None).await {
        Ok((StatusCode::OK, v)) => Some(v),
        _ => None,
    }
}

async fn adopt(s: &AppState) {
    let Some(v) = kyber_state(s).await else { return };
    if v["active"].as_bool() == Some(true) {
        let started_at_ms = v["started_at_ms"].as_u64().unwrap_or_else(now_ms);
        *s.recording.state.lock().unwrap() =
            (Some(Active { started_at_ms, by: None, files: files_of(&v), bytes: v["bytes"].as_u64().unwrap_or(0) }), None);
        tracing::info!("enregistrement en cours repris");
    }
}

async fn watch(s: &AppState) {
    if s.lend.borrower().is_some() {
        stop(s, EndReason::Lent).await;
        return;
    }
    if free_bytes(&folder(s)).is_some_and(|f| f < MIN_FREE_BYTES) {
        tracing::warn!("enregistrement : moins de 2 Go libres, arrêt");
        stop(s, EndReason::DiskFull).await;
        return;
    }
    let Some(v) = kyber_state(s).await else { return };
    if v["active"].as_bool() == Some(true) {
        if let Some(a) = s.recording.state.lock().unwrap().0.as_mut() {
            a.files = files_of(&v);
            a.bytes = v["bytes"].as_u64().unwrap_or(a.bytes);
        }
        return;
    }
    // Kyber l'a fini de lui-même : la session de l'hôte, l'écriture, le format.
    let reason = match v["last_end"]["reason"].as_str() {
        Some("write") if free_bytes(&folder(s)).is_some_and(|f| f < MIN_FREE_BYTES) => EndReason::DiskFull,
        Some("write") => EndReason::Write,
        Some("codec") => EndReason::Codec,
        Some("stopped") => EndReason::Stopped,
        _ => EndReason::Ended,
    };
    let _busy = s.recording.busy.lock().await;
    ended(s, reason);
}

/// Efface les enregistrements d'Admin plus vieux que la durée choisie (pas
/// celui qui s'écrit, ni les autres fichiers du dossier).
fn cleanup(s: &AppState) -> usize {
    let keep = settings(s).keep_days;
    if keep == 0 {
        return 0;
    }
    let current = s.recording.current_files();
    let limit = SystemTime::now() - Duration::from_secs(keep as u64 * 86_400);
    let mut removed = 0;
    for f in list_files(&folder(s)) {
        if current.contains(&f.name) || !f.name.starts_with(PREFIX) {
            continue;
        }
        if f.modified.is_some_and(|m| m < limit) && std::fs::remove_file(&f.path).is_ok() {
            tracing::info!("enregistrement effacé (plus de {keep} jours) : {}", f.name);
            removed += 1;
        }
    }
    removed
}

// --- Fichiers ------------------------------------------------------------------

struct FileEntry {
    name: String,
    path: PathBuf,
    size: u64,
    modified: Option<SystemTime>,
}

fn list_files(dir: &Path) -> Vec<FileEntry> {
    let Ok(read) = std::fs::read_dir(dir) else { return Vec::new() };
    let mut out: Vec<FileEntry> = read
        .filter_map(Result::ok)
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().into_owned();
            if !name.to_ascii_lowercase().ends_with(".mp4") {
                return None;
            }
            let meta = e.metadata().ok().filter(|m| m.is_file())?;
            Some(FileEntry { name, path: e.path(), size: meta.len(), modified: meta.modified().ok() })
        })
        .collect();
    out.sort_by(|a, b| b.modified.cmp(&a.modified).then_with(|| b.name.cmp(&a.name)));
    out
}

/// La durée écrite à la fermeture (`mehd`, ms) ; `None` pour un fichier en
/// cours ou coupé avant sa fin.
fn mp4_duration_ms(path: &Path) -> Option<u64> {
    use std::io::Read;
    let mut head = vec![0u8; 256 * 1024];
    let mut file = std::fs::File::open(path).ok()?;
    let n = file.read(&mut head).ok()?;
    let head = &head[..n];
    let at = head.windows(4).position(|w| w == b"mehd")?;
    let version = *head.get(at + 4)?;
    let value = if version == 1 {
        u64::from_be_bytes(head.get(at + 8..at + 16)?.try_into().ok()?)
    } else {
        u32::from_be_bytes(head.get(at + 8..at + 12)?.try_into().ok()?) as u64
    };
    (value > 0).then_some(value)
}

/// Un nom de fichier du dossier, jamais un chemin.
fn checked_name(name: &str) -> Option<&str> {
    let ok = !name.is_empty()
        && !name.contains(['/', '\\', ':'])
        && !name.starts_with('.')
        && name.to_ascii_lowercase().ends_with(".mp4");
    ok.then_some(name)
}

// --- Routes de l'appareil en session ------------------------------------------------

#[derive(Deserialize, Default)]
pub struct StreamBody {
    #[serde(default)]
    record: bool,
    #[serde(default)]
    device_id: Option<String>,
}

/// `POST /api/stream/recording` : l'appareil en session lance ou arrête.
pub async fn stream_recording(State(s): State<AppState>, headers: HeaderMap, body: axum::body::Bytes) -> RecResult<Json<Value>> {
    let body: StreamBody = serde_json::from_slice(&body).unwrap_or_default();
    let user = crate::api::stream_user(&s, &headers).map_err(|e| RecError { status: e.0, code: "unauthorized", error: e.1 })?;
    let live = s
        .activity
        .live()
        .filter(|l| crate::activity::Activity::is_live(l, &user, body.device_id.as_deref()))
        .ok_or_else(|| conflict("not_in_session", "recording.notInSession"))?;
    if body.record {
        Ok(Json(start(&s, Some(live.name)).await?))
    } else {
        Ok(Json(stop(&s, EndReason::Stopped).await))
    }
}

// --- Routes d'Admin ------------------------------------------------------------

/// `GET /api/recording` : réglages, état, place libre, fichiers.
pub async fn admin_get(State(s): State<AppState>) -> Json<Value> {
    let st = s.clone();
    let value = tokio::task::spawn_blocking(move || admin_view(&st)).await.unwrap_or(Value::Null);
    Json(value)
}

fn admin_view(s: &AppState) -> Value {
    let set = settings(s);
    let dir = folder(s);
    let current = s.recording.current_files();
    let files: Vec<Value> = list_files(&dir)
        .into_iter()
        .map(|f| {
            let in_progress = current.contains(&f.name);
            json!({
                "name": f.name,
                "size": f.size,
                "modified": f.modified.and_then(|m| m.duration_since(UNIX_EPOCH).ok()).map(|d| d.as_secs()),
                "duration_ms": if in_progress { None } else { mp4_duration_ms(&f.path) },
                "in_progress": in_progress,
            })
        })
        .collect();
    let by = s.recording.state.lock().unwrap().0.as_ref().and_then(|a| a.by.clone());
    json!({
        "settings": {
            "allowed": set.allowed,
            "folder": dir.display().to_string(),
            "custom_folder": set.folder.is_some(),
            "default_folder": default_folder().display().to_string(),
            "keep_days": set.keep_days,
        },
        "state": for_session(s),
        "by": by,
        "session": s.activity.live().map(|l| l.name),
        "free": free_bytes(&dir),
        "files": files,
    })
}

#[derive(Deserialize)]
pub struct SettingsBody {
    allowed: Option<bool>,
    /// Chaîne vide : le dossier par défaut.
    folder: Option<String>,
    keep_days: Option<u32>,
}

/// `PUT /api/recording/settings`.
pub async fn admin_settings(State(s): State<AppState>, Json(body): Json<SettingsBody>) -> Result<Json<Value>, crate::api::ApiError> {
    if let Some(days) = body.keep_days
        && !KEEP_CHOICES.contains(&days)
    {
        return Err(crate::api::ApiError::msg(StatusCode::BAD_REQUEST, Msg::new("recording.keepChoice")));
    }
    let folder = match body.folder.as_deref().map(str::trim) {
        None => None,
        Some("") => Some(None),
        Some(path) => {
            let path = PathBuf::from(path);
            if !path.is_absolute() {
                return Err(crate::api::ApiError::msg(StatusCode::BAD_REQUEST, Msg::new("recording.folderAbsolute")));
            }
            Some(Some(path))
        }
    };
    let turned_off = body.allowed == Some(false);
    s.update_settings(|set| {
        if let Some(on) = body.allowed {
            set.recording.allowed = on;
        }
        if let Some(f) = folder {
            set.recording.folder = f;
        }
        if let Some(days) = body.keep_days {
            set.recording.keep_days = days;
        }
    })?;
    if turned_off {
        stop(&s, EndReason::Stopped).await;
    }
    let st = s.clone();
    let _ = tokio::task::spawn_blocking(move || cleanup(&st)).await;
    Ok(admin_get(State(s)).await)
}

#[derive(Deserialize)]
pub struct AdminRecordBody {
    record: bool,
}

/// `POST /api/recording` : Admin lance ou arrête l'enregistrement de la session en cours.
pub async fn admin_record(State(s): State<AppState>, Json(body): Json<AdminRecordBody>) -> RecResult<Json<Value>> {
    if body.record {
        start(&s, None).await.map(Json)
    } else {
        Ok(Json(stop(&s, EndReason::Stopped).await))
    }
}

#[derive(Deserialize, Default)]
pub struct FileQuery {
    #[serde(default)]
    download: bool,
}

/// `GET /api/recording/files/{nom}` : le fichier, par morceaux (`Range`) pour
/// le lecteur de la page ; `?download=1` : à enregistrer.
pub async fn admin_file(
    State(s): State<AppState>,
    AxPath(name): AxPath<String>,
    Query(q): Query<FileQuery>,
    headers: HeaderMap,
) -> Response {
    let Some(name) = checked_name(&name) else { return StatusCode::BAD_REQUEST.into_response() };
    let path = folder(&s).join(name);
    let Ok(meta) = tokio::fs::metadata(&path).await else { return StatusCode::NOT_FOUND.into_response() };
    let size = meta.len();
    let range = headers.get(header::RANGE).and_then(|v| v.to_str().ok()).and_then(|r| parse_range(r, size));
    let (start, end) = range.unwrap_or((0, size.saturating_sub(1)));
    let len = if size == 0 { 0 } else { end - start + 1 };
    let file = match tokio::fs::File::open(&path).await {
        Ok(f) => f,
        Err(_) => return StatusCode::NOT_FOUND.into_response(),
    };
    use tokio::io::{AsyncReadExt, AsyncSeekExt};
    let mut file = file;
    if file.seek(std::io::SeekFrom::Start(start)).await.is_err() {
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    }
    let mut res = Response::new(Body::new(FileBody { file: file.take(len) }));
    *res.status_mut() = if range.is_some() { StatusCode::PARTIAL_CONTENT } else { StatusCode::OK };
    let h = res.headers_mut();
    h.insert(header::CONTENT_TYPE, HeaderValue::from_static("video/mp4"));
    h.insert(header::ACCEPT_RANGES, HeaderValue::from_static("bytes"));
    h.insert(header::CONTENT_LENGTH, HeaderValue::from(len));
    h.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    if range.is_some()
        && let Ok(v) = HeaderValue::from_str(&format!("bytes {start}-{end}/{size}"))
    {
        h.insert(header::CONTENT_RANGE, v);
    }
    if q.download {
        let ascii: String = name.chars().map(|c| if c.is_ascii_graphic() || c == ' ' { c } else { '_' }).filter(|c| *c != '"').collect();
        let value = format!("attachment; filename=\"{ascii}\"; filename*=UTF-8''{}", urlencoding::encode(name));
        if let Ok(v) = HeaderValue::from_str(&value) {
            h.insert(header::CONTENT_DISPOSITION, v);
        }
    }
    res
}

/// Le fichier en morceaux de 256 Kio, lu au fil de l'envoi.
struct FileBody {
    file: tokio::io::Take<tokio::fs::File>,
}

impl http_body::Body for FileBody {
    type Data = bytes::Bytes;
    type Error = std::io::Error;

    fn poll_frame(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Result<http_body::Frame<bytes::Bytes>, std::io::Error>>> {
        use std::task::Poll;
        let mut buf = vec![0u8; 256 * 1024];
        let mut read = tokio::io::ReadBuf::new(&mut buf);
        match tokio::io::AsyncRead::poll_read(std::pin::Pin::new(&mut self.file), cx, &mut read) {
            Poll::Pending => Poll::Pending,
            Poll::Ready(Err(e)) => Poll::Ready(Some(Err(e))),
            Poll::Ready(Ok(())) => {
                let n = read.filled().len();
                if n == 0 {
                    return Poll::Ready(None);
                }
                buf.truncate(n);
                Poll::Ready(Some(Ok(http_body::Frame::data(bytes::Bytes::from(buf)))))
            }
        }
    }
}

/// `bytes=a-b`, `bytes=a-`, `bytes=-n` ; une seule plage.
fn parse_range(value: &str, size: u64) -> Option<(u64, u64)> {
    if size == 0 {
        return None;
    }
    let spec = value.trim().strip_prefix("bytes=")?;
    if spec.contains(',') {
        return None;
    }
    let (a, b) = spec.split_once('-')?;
    let (start, end) = match (a.trim(), b.trim()) {
        ("", n) => {
            let n: u64 = n.parse().ok()?;
            (size.saturating_sub(n.min(size)), size - 1)
        }
        (a, "") => (a.parse().ok()?, size - 1),
        (a, b) => (a.parse().ok()?, b.parse::<u64>().ok()?.min(size - 1)),
    };
    (start <= end && start < size).then_some((start, end))
}

/// `DELETE /api/recording/files/{nom}` (pas celui qui s'écrit).
pub async fn admin_delete(State(s): State<AppState>, AxPath(name): AxPath<String>) -> Result<Json<Value>, crate::api::ApiError> {
    let Some(name) = checked_name(&name) else {
        return Err(crate::api::ApiError::msg(StatusCode::BAD_REQUEST, Msg::new("recording.notFound")));
    };
    if s.recording.current_files().iter().any(|f| f == name) {
        return Err(crate::api::ApiError::msg(StatusCode::CONFLICT, Msg::new("recording.inProgress")));
    }
    let path = folder(&s).join(name);
    if tokio::fs::remove_file(&path).await.is_err() {
        return Err(crate::api::ApiError::msg(StatusCode::NOT_FOUND, Msg::new("recording.notFound")));
    }
    tracing::info!("enregistrement supprimé depuis Admin : {name}");
    Ok(admin_get(State(s)).await)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ranges() {
        assert_eq!(parse_range("bytes=0-99", 1000), Some((0, 99)));
        assert_eq!(parse_range("bytes=900-", 1000), Some((900, 999)));
        assert_eq!(parse_range("bytes=-100", 1000), Some((900, 999)));
        assert_eq!(parse_range("bytes=0-5000", 1000), Some((0, 999)));
        assert_eq!(parse_range("bytes=1000-", 1000), None);
        assert_eq!(parse_range("bytes=0-1,5-6", 1000), None);
        assert_eq!(parse_range("items=0-1", 1000), None);
    }

    #[test]
    fn names() {
        assert_eq!(checked_name("KyberNow 2026-11-26 14-32-05.mp4"), Some("KyberNow 2026-11-26 14-32-05.mp4"));
        assert_eq!(checked_name("../secret.mp4"), None);
        assert_eq!(checked_name("a\\b.mp4"), None);
        assert_eq!(checked_name("C:x.mp4"), None);
        assert_eq!(checked_name("notes.txt"), None);
    }

    #[test]
    fn xdg() {
        let home = Path::new("/home/ana");
        assert_eq!(xdg_videos("XDG_VIDEOS_DIR=\"$HOME/Vidéos\"\n", home), Some(PathBuf::from("/home/ana/Vidéos")));
        assert_eq!(xdg_videos("XDG_VIDEOS_DIR=\"/data/films\"", home), Some(PathBuf::from("/data/films")));
        assert_eq!(xdg_videos("XDG_VIDEOS_DIR=\"$HOME/\"", home), None);
        assert_eq!(xdg_videos("XDG_MUSIC_DIR=\"$HOME/Musique\"", home), None);
    }

    #[test]
    fn settings_default_and_old_files() {
        let set: RecordingSettings = toml::from_str("").unwrap();
        assert_eq!(set, RecordingSettings::default());
        assert!(set.allowed);
        assert_eq!(set.keep_days, 30);
    }

    #[test]
    fn reads_the_duration_written_at_the_end() {
        let dir = std::env::temp_dir().join(format!("kn-admin-rec-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("x.mp4");
        // ftyp, then moov › mvex › mehd (version 1, 754 321 ms).
        let mut data = vec![0, 0, 0, 16];
        data.extend_from_slice(b"ftypisom\0\0\x02\0");
        data.extend_from_slice(&[0, 0, 0, 20]);
        data.extend_from_slice(b"mehd");
        data.extend_from_slice(&[1, 0, 0, 0]);
        data.extend_from_slice(&754_321u64.to_be_bytes());
        std::fs::write(&path, &data).unwrap();
        assert_eq!(mp4_duration_ms(&path), Some(754_321));
        data.truncate(data.len() - 8);
        data.extend_from_slice(&0u64.to_be_bytes());
        std::fs::write(&path, &data).unwrap();
        assert_eq!(mp4_duration_ms(&path), None, "a file cut short");
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
