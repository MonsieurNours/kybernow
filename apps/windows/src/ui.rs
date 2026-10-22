//! L'app (Tauri, interface Svelte dans `ui/`) : accueil, ajout d'un
//! ordinateur, réglages, passage vers la session et barre de session.
//!
//! Fenêtres : `main` (accueil, sans cadre de Windows : la barre de titre est
//! dessinée par l'interface) et `bar` (barre de session, transparente, montrée
//! quand le pointeur touche le haut de l'écran de la session).
//! La session elle-même est une fenêtre Win32 sur son propre fil (`host.rs`).

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

use base64::Engine;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindowBuilder};
use windows::Win32::Foundation::HWND;
use windows::Win32::Graphics::Gdi::{GetMonitorInfoW, MONITOR_DEFAULTTONEAREST, MONITORINFO, MonitorFromWindow};

use kybernow_core::console::{Console, Info, PairStatus};
use kybernow_core::profile::{Codec, Profile};
use kybernow_core::session::{SessionParams, Status};

use crate::discovery::{self, Discovery, Found};
use crate::host::{HostEvents, HostOptions, SessionHost, Stats, Summary};
use crate::plan::{Choices, plan};
use crate::store::{self, Computer, LastSession, Settings, Store};
use crate::window::Command;
use crate::pc::{self, ThisPc};
use crate::i18n::t;
use crate::{audio, i18n, instance, shell, snapshot, system, window};
use kybernow_core::update::{self, Available, Channel};

#[derive(Debug, Clone, Default)]
struct Live {
    online: Option<bool>,
    info: Option<Info>,
    preview: Option<String>,
    preview_at: Option<Instant>,
    /// Dernière image de la dernière session (passe avant l'aperçu).
    last: Option<String>,
    last_loaded: bool,
}

fn data_url(jpeg: &[u8]) -> String {
    format!("data:image/jpeg;base64,{}", base64::engine::general_purpose::STANDARD.encode(jpeg))
}

#[derive(Debug, Clone, Serialize)]
pub struct ComputerView {
    #[serde(flatten)]
    computer: Computer,
    online: Option<bool>,
    info: Option<Info>,
    preview: Option<String>,
    /// `last` : dernière image de votre session ; `preview` : aperçu de son écran.
    preview_source: Option<&'static str>,
}

/// Un ordinateur à ajouter : adresse et port de sa console, empreinte
/// attendue (lien, annonce du réseau) ou vue au premier contact.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Candidate {
    pub host: String,
    pub port: u16,
    pub name: Option<String>,
    pub fingerprint: Option<String>,
}

struct Pairing {
    candidate: Candidate,
    id: String,
}

struct Running {
    id: String,
    host: SessionHost,
    bar: Arc<Mutex<BarLabel>>,
}

#[derive(Default)]
pub struct UiState {
    store: Mutex<Option<Store>>,
    live: Mutex<HashMap<String, Live>>,
    discovery: OnceLock<Arc<Discovery>>,
    this_pc: Mutex<Option<ThisPc>>,
    pairing: Mutex<Option<Pairing>>,
    session: Mutex<Option<Running>>,
    /// Fermeture demandée pendant une session : quitter à sa fin.
    quitting: std::sync::atomic::AtomicBool,
    /// Demande du lancement (lien, barre des tâches…), lue par l'interface.
    action: Mutex<Option<Action>>,
    prefs: Mutex<Prefs>,
    update: Mutex<Option<Available>>,
}

/// Ce que l'ouverture demande à l'app.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum Action {
    Connect { id: String },
    Add,
    Link { link: String },
    Updated,
    Open,
}

fn parse_action(args: &[String]) -> Option<Action> {
    let first = args.first()?;
    if let Some(rest) = first.strip_prefix("kybernow://") {
        // Actions des notifications : kybernow://connect?id=…, kybernow://open.
        if let Some(q) = rest.strip_prefix("connect?id=") {
            return Some(Action::Connect { id: q.chars().filter(|c| c.is_ascii_alphanumeric()).collect() });
        }
        if rest.trim_end_matches('/') == "open" {
            return Some(Action::Open);
        }
        return Some(Action::Link { link: first.clone() });
    }
    match first.as_str() {
        "--connect" => args.get(1).map(|id| Action::Connect { id: id.clone() }),
        "--add" => Some(Action::Add),
        "--updated" => Some(Action::Updated),
        _ => None,
    }
}

/// Réglages de l'app (`%APPDATA%\KyberNow\app.json`).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Prefs {
    pub channel: Channel,
    /// Langue de l'app : `auto` (celle de Windows, aussi si absent), `fr`, `en`.
    pub language: String,
}

/// `auto`, `fr` ou `en`.
fn language_choice(p: &Prefs) -> &'static str {
    match p.language.as_str() {
        "fr" => "fr",
        "en" => "en",
        _ => "auto",
    }
}

fn prefs_path() -> std::path::PathBuf {
    std::env::var_os("APPDATA").map(std::path::PathBuf::from).unwrap_or_else(std::env::temp_dir).join("KyberNow").join("app.json")
}

pub fn load_prefs() -> Prefs {
    std::fs::read_to_string(prefs_path()).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default()
}

/// Installateur publié : un seul pour x64 et ARM64, il pose la version du
/// processeur (une app x64 émulée sur ARM passe ainsi à la version ARM64).
/// Son nom actuel d'abord, puis celui d'avant le renommage en KyberNow Link
/// (publié aussi pendant quelques versions ; seul nom des versions 0.3.1 et avant).
const INSTALLER: &[&str] = &["KyberNow-Link-Setup.exe", "KyberNow-Client-Setup.exe"];

impl UiState {
    fn with_store<T>(&self, f: impl FnOnce(&mut Store) -> T) -> T {
        let mut s = self.store.lock().unwrap();
        f(s.get_or_insert_with(Store::load))
    }

    fn views(&self) -> Vec<ComputerView> {
        let computers = self.with_store(|s| s.computers.clone());
        let mut live = self.live.lock().unwrap();
        computers
            .into_iter()
            .map(|c| {
                let l = live.entry(c.id.clone()).or_default();
                // Dernière image gardée sur le disque : lue une fois.
                if !l.last_loaded {
                    l.last_loaded = true;
                    if c.settings.keep_last_image {
                        l.last = snapshot::load(&c.id).map(|j| data_url(&j));
                    }
                }
                let l = l.clone();
                let (preview, preview_source) = match (l.last, l.preview) {
                    (Some(last), _) => (Some(last), Some("last")),
                    (None, Some(p)) => (Some(p), Some("preview")),
                    (None, None) => (None, None),
                };
                ComputerView { computer: c, online: l.online, info: l.info, preview, preview_source }
            })
            .collect()
    }
}

#[derive(Serialize)]
pub struct Snapshot {
    computers: Vec<ComputerView>,
    discovered: Vec<Found>,
    this_pc: Option<ThisPc>,
    session: Option<String>,
    version: String,
}

fn emit_computers(app: &AppHandle) {
    let state = app.state::<UiState>();
    let views = state.views();
    shell::update_jump_list(views.iter().map(|v| (v.computer.id.clone(), v.computer.name.clone())).collect());
    let _ = app.emit_to("main", "computers", views);
}

/// Une autre ouverture : l'accueil revient devant et reçoit la demande
/// (gardée aussi pour l'interface, si elle n'écoute pas encore).
fn handle_args(app: &AppHandle, args: Vec<String>) {
    let action = parse_action(&args).unwrap_or(Action::Open);
    if !matches!(action, Action::Open) {
        *app.state::<UiState>().action.lock().unwrap() = Some(action.clone());
    }
    let a = app.clone();
    let _ = app.run_on_main_thread(move || {
        if let Some(w) = a.get_webview_window("main") {
            let in_session = a.state::<UiState>().session.lock().unwrap().is_some();
            if !in_session {
                let _ = w.unminimize();
                let _ = w.show();
                let _ = w.set_focus();
            }
        }
        // L'interface la reprend par take_action (une seule fois).
        let _ = a.emit_to("main", "action-ready", ());
        let _ = action;
    });
}

#[tauri::command(async)]
fn take_action(state: tauri::State<'_, UiState>) -> Option<Action> {
    state.action.lock().unwrap().take()
}

#[derive(Serialize)]
struct AppInfo {
    version: &'static str,
    channel: Channel,
    /// Choix de la langue (`auto`, `fr`, `en`) et langue appliquée (`fr`, `en`).
    language: &'static str,
    lang: &'static str,
    update: Option<Available>,
    arch: &'static str,
}

#[tauri::command(async)]
fn app_info(state: tauri::State<'_, UiState>) -> AppInfo {
    let prefs = state.prefs.lock().unwrap().clone();
    AppInfo {
        version: env!("CARGO_PKG_VERSION"),
        channel: prefs.channel,
        language: language_choice(&prefs),
        lang: i18n::lang().code(),
        update: state.update.lock().unwrap().clone(),
        arch: if cfg!(target_arch = "aarch64") { "ARM64" } else { "x64" },
    }
}

/// Cherche une version plus récente sur le canal choisi.
fn check_now(app: &AppHandle) -> Result<Option<Available>, String> {
    let state = app.state::<UiState>();
    let channel = state.prefs.lock().unwrap().channel;
    let found = update::check(env!("CARGO_PKG_VERSION"), channel, INSTALLER)?;
    let fresh = found.is_some() && state.update.lock().unwrap().as_ref().map(|u| &u.version) != found.as_ref().map(|u| &u.version);
    *state.update.lock().unwrap() = found.clone();
    let _ = app.emit_to("main", "update", &found);
    if fresh && !shell::app_in_front() {
        if let Some(u) = &found {
            shell::notify(&t("notify.update_title", &[("version", &u.version)]), &t("notify.update_body", &[]), None);
        }
    }
    Ok(found)
}

#[tauri::command(async)]
fn check_update(app: AppHandle) -> Result<Option<Available>, String> {
    check_now(&app)
}

#[tauri::command(async)]
fn set_channel(app: AppHandle, channel: Channel) -> Result<Option<Available>, String> {
    let state = app.state::<UiState>();
    let prefs = {
        let mut p = state.prefs.lock().unwrap();
        p.channel = channel;
        p.clone()
    };
    save_prefs(&prefs);
    check_now(&app)
}

fn save_prefs(prefs: &Prefs) {
    if let Some(dir) = prefs_path().parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let _ = std::fs::write(prefs_path(), serde_json::to_string_pretty(prefs).unwrap_or_default());
}

/// Langue choisie dans les réglages de l'app (`auto`, `fr`, `en`) : appliquée
/// tout de suite aux deux fenêtres, aux messages du côté Rust, aux notes de
/// version et à la liste de la barre des tâches. Rend la langue appliquée.
#[tauri::command(async)]
fn set_language(app: AppHandle, language: String) -> &'static str {
    let state = app.state::<UiState>();
    let prefs = {
        let mut p = state.prefs.lock().unwrap();
        p.language = language;
        p.language = language_choice(&p).to_string();
        p.clone()
    };
    save_prefs(&prefs);
    let lang = i18n::apply(&prefs.language);
    log::info!("langue : {} ({})", prefs.language, lang.code());
    let update = {
        let mut u = state.update.lock().unwrap();
        if let Some(u) = u.as_mut() {
            u.localize(lang);
        }
        u.clone()
    };
    let _ = app.emit("lang", lang.code());
    let _ = app.emit_to("main", "update", &update);
    emit_computers(&app);
    lang.code()
}

/// Télécharge l'installateur vérifié, le lance en silence et quitte : il
/// attend la fermeture de l'app, remplace ses fichiers et la relance.
#[tauri::command(async)]
fn install_update(app: AppHandle) -> Result<(), String> {
    use std::os::windows::process::CommandExt;
    let state = app.state::<UiState>();
    if state.session.lock().unwrap().is_some() {
        return Err(t("errors.session_open_install_later", &[]));
    }
    let u = state.update.lock().unwrap().clone().ok_or_else(|| t("errors.no_update", &[]))?;
    let dir = std::env::temp_dir().join("KyberNow");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let path = dir.join(format!("KyberNow-{}-{}", u.version, u.file));
    // Installateurs des mises à jour précédentes.
    for e in std::fs::read_dir(&dir).into_iter().flatten().flatten() {
        if e.path() != path && e.file_name().to_string_lossy().ends_with(".exe") {
            let _ = std::fs::remove_file(e.path());
        }
    }
    let size = u.size.max(1);
    let a = app.clone();
    let last = std::sync::atomic::AtomicU64::new(u64::MAX);
    update::download(&u, &path, move |got| {
        let pct = got * 100 / size;
        if last.swap(pct, std::sync::atomic::Ordering::Relaxed) != pct {
            let _ = a.emit_to("main", "update-progress", pct);
        }
    })?;
    log::info!("mise à jour {} vérifiée, installation", u.version);
    const DETACHED_PROCESS: u32 = 0x0000_0008;
    const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
    const CREATE_BREAKAWAY_FROM_JOB: u32 = 0x0100_0000;
    // Hors du « job » de l'app si on peut (un lanceur qui tue ses enfants à
    // la fermeture emporterait l'installateur), sinon tel quel.
    // Pas le dossier de l'app comme dossier courant : l'installateur doit pouvoir le remplacer.
    let spawn = |flags: u32| std::process::Command::new(&path).args(["/S", "/UPDATE"]).current_dir(&dir).creation_flags(flags).spawn();
    spawn(DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP | CREATE_BREAKAWAY_FROM_JOB)
        .or_else(|_| spawn(DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP))
        .map_err(|e| t("errors.installer_launch", &[("error", &e.to_string())]))?;
    app.exit(0);
    Ok(())
}

fn console_for(c: &Computer) -> Console {
    let password = store::password(&c.id).unwrap_or_default();
    Console::new(&c.host, c.port, &c.user, &password, c.fingerprint.clone())
}

/// Toutes les 8 s : qui est en ligne ; un aperçu de l'écran par minute au plus.
fn poll(app: AppHandle) {
    loop {
        let state = app.state::<UiState>();
        let in_session = state.session.lock().unwrap().as_ref().map(|r| r.id.clone());
        let computers = state.with_store(|s| s.computers.clone());
        let mut changed = false;
        for c in computers {
            if in_session.as_deref() == Some(c.id.as_str()) {
                continue;
            }
            let console = console_for(&c);
            let info = console.info().ok();
            let online = Some(info.is_some());
            let mut preview = None;
            let wants_preview = {
                let live = state.live.lock().unwrap();
                // Pas d'aperçu quand la dernière image de la session est là (comme sur Mac).
                live.get(&c.id).is_none_or(|l| l.last.is_none() && l.preview_at.is_none_or(|t| t.elapsed() > Duration::from_secs(60)))
            };
            if info.is_some() && wants_preview {
                if let Ok(jpeg) = console.preview(1280) {
                    if !jpeg.is_empty() {
                        preview = Some(format!(
                            "data:image/jpeg;base64,{}",
                            base64::engine::general_purpose::STANDARD.encode(jpeg)
                        ));
                    }
                }
            }
            let mut live = state.live.lock().unwrap();
            let l = live.entry(c.id.clone()).or_default();
            if l.online != online || preview.is_some() || (info.is_some() && l.info.is_none()) {
                changed = true;
            }
            l.online = online;
            if info.is_some() {
                l.info = info;
            }
            if let Some(p) = preview {
                l.preview = Some(p);
                l.preview_at = Some(Instant::now());
            }
        }
        if changed {
            emit_computers(&app);
        }
        std::thread::sleep(Duration::from_secs(8));
    }
}

#[tauri::command(async)]
fn snapshot(state: tauri::State<'_, UiState>) -> Snapshot {
    Snapshot {
        computers: state.views(),
        discovered: state.discovery.get().map(|d| d.list()).unwrap_or_default(),
        this_pc: state.this_pc.lock().unwrap().clone(),
        session: state.session.lock().unwrap().as_ref().map(|r| r.id.clone()),
        version: env!("CARGO_PKG_VERSION").into(),
    }
}

/// Premier contact par adresse : l'empreinte de la console, à comparer à
/// celle qu'elle affiche.
#[tauri::command(async)]
fn probe_address(host: String, port: u16) -> Result<Candidate, String> {
    let console = Console::new(host.trim(), port, "", "", None);
    let info = console.probe().map_err(|e| e.to_string())?;
    let fingerprint = console.fingerprint_seen().ok_or_else(|| t("errors.no_certificate", &[]))?;
    Ok(Candidate {
        host: host.trim().into(),
        port,
        name: info.map(|i| i.hostname).filter(|n| !n.is_empty()),
        fingerprint: Some(fingerprint),
    })
}

fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let hex = |c: u8| (c as char).to_digit(16).map(|d| d as u8);
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            b'%' if i + 2 < b.len() => {
                if let (Some(h), Some(l)) = (hex(b[i + 1]), hex(b[i + 2])) {
                    out.push(h << 4 | l);
                    i += 3;
                    continue;
                }
                out.push(b'%');
            }
            b'+' => out.push(b' '),
            c => out.push(c),
        }
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// `kybernow://adresse:port?name=…&fp=sha256:…` (lien ou QR code de la console).
pub fn parse_link_text(link: &str) -> Result<Candidate, String> {
    let rest = link.trim().strip_prefix("kybernow://").ok_or_else(|| t("errors.not_a_link", &[]))?;
    let (authority, query) = rest.split_once('?').unwrap_or((rest, ""));
    let authority = authority.trim_end_matches('/');
    let (host, port) = if let Some(r) = authority.strip_prefix('[') {
        let (h, p) = r.split_once(']').ok_or_else(|| t("errors.bad_address", &[]))?;
        (h.to_string(), p.trim_start_matches(':').to_string())
    } else {
        let (h, p) = authority.rsplit_once(':').ok_or_else(|| t("errors.no_port", &[]))?;
        (h.to_string(), p.to_string())
    };
    let port: u16 = port.parse().map_err(|_| t("errors.bad_port", &[]))?;
    let mut name = None;
    let mut fingerprint = None;
    for pair in query.split('&') {
        let (k, v) = pair.split_once('=').unwrap_or((pair, ""));
        match k {
            "name" => name = Some(percent_decode(v)),
            "fp" => fingerprint = Some(percent_decode(v)),
            _ => {}
        }
    }
    Ok(Candidate { host, port, name, fingerprint })
}

#[tauri::command(async)]
fn parse_link(link: String) -> Result<Candidate, String> {
    parse_link_text(&link)
}

/// Vérifie le compte auprès de la console (et son certificat), puis retient
/// l'ordinateur et son mot de passe.
fn add(app: &AppHandle, candidate: &Candidate, user: &str, password: &str) -> Result<String, String> {
    let console = Console::new(&candidate.host, candidate.port, user, password, candidate.fingerprint.clone());
    let info = console.info().map_err(|e| e.to_string())?;
    let fingerprint = candidate.fingerprint.clone().or_else(|| console.fingerprint_seen());
    let state = app.state::<UiState>();
    let id = state.with_store(|s| {
        s.upsert(Computer {
            id: store::new_id(),
            name: if info.hostname.is_empty() {
                candidate.name.clone().unwrap_or_else(|| candidate.host.clone())
            } else {
                info.hostname.clone()
            },
            host: candidate.host.clone(),
            port: candidate.port,
            user: user.into(),
            fingerprint,
            os: info.os.clone(),
            gpu: info.gpu.clone(),
            settings: Settings::default(),
            last: None,
        })
    });
    if !store::set_password(&id, user, password) {
        return Err(t("errors.password_store", &[]));
    }
    state.live.lock().unwrap().insert(id.clone(), Live { online: Some(true), info: Some(info), ..Default::default() });
    emit_computers(app);
    Ok(id)
}

#[tauri::command(async)]
fn add_computer(app: AppHandle, candidate: Candidate, user: String, password: String) -> Result<String, String> {
    add(&app, &candidate, user.trim(), &password)
}

/// Code à six chiffres envoyé à la console, à y accepter.
#[tauri::command(async)]
fn pair_start(state: tauri::State<'_, UiState>, candidate: Candidate) -> Result<String, String> {
    let mut b = [0u8; 4];
    let _ = getrandom::getrandom(&mut b);
    let code = format!("{:06}", u32::from_le_bytes(b) % 1_000_000);
    let console = Console::new(&candidate.host, candidate.port, "", "", candidate.fingerprint.clone());
    let id = console.pair_request(&code, &system::device_info()).map_err(|e| e.to_string())?;
    *state.pairing.lock().unwrap() = Some(Pairing { candidate, id });
    Ok(code)
}

/// `pending`, `refused`, `expired`, ou l'identifiant de l'ordinateur ajouté.
#[tauri::command(async)]
fn pair_poll(app: AppHandle) -> Result<String, String> {
    let state = app.state::<UiState>();
    let (candidate, id) = match state.pairing.lock().unwrap().as_ref() {
        Some(p) => (p.candidate.clone(), p.id.clone()),
        None => return Ok("expired".into()),
    };
    let console = Console::new(&candidate.host, candidate.port, "", "", candidate.fingerprint.clone());
    match console.pair_status(&id).map_err(|e| e.to_string())? {
        PairStatus::Pending => Ok("pending".into()),
        PairStatus::Refused => Ok("refused".into()),
        PairStatus::Expired => Ok("expired".into()),
        PairStatus::Accepted { username, password } => {
            // La demande n'est oubliée qu'une fois l'ordinateur ajouté : la
            // console ne redonne pas ce compte.
            let id = add(&app, &candidate, &username, &password)?;
            *state.pairing.lock().unwrap() = None;
            Ok(id)
        }
    }
}

#[tauri::command(async)]
fn pair_cancel(state: tauri::State<'_, UiState>) {
    *state.pairing.lock().unwrap() = None;
}

#[tauri::command(async)]
fn save_settings(app: AppHandle, id: String, settings: Settings) -> Result<(), String> {
    let state = app.state::<UiState>();
    // « Garder la dernière image » coupé : effacée tout de suite.
    if !settings.keep_last_image {
        snapshot::forget(&id);
        if let Some(l) = state.live.lock().unwrap().get_mut(&id) {
            l.last = None;
        }
    }
    state.with_store(|s| {
        let c = s.get_mut(&id).ok_or_else(|| t("errors.unknown_computer", &[]))?;
        c.settings = settings;
        s.save();
        Ok::<_, String>(())
    })?;
    emit_computers(&app);
    Ok(())
}

#[tauri::command(async)]
fn rename_computer(app: AppHandle, id: String, name: String) -> Result<(), String> {
    let state = app.state::<UiState>();
    state.with_store(|s| {
        let c = s.get_mut(&id).ok_or_else(|| t("errors.unknown_computer", &[]))?;
        if !name.trim().is_empty() {
            c.name = name.trim().into();
        }
        s.save();
        Ok::<_, String>(())
    })?;
    emit_computers(&app);
    Ok(())
}

#[tauri::command(async)]
fn remove_computer(app: AppHandle, id: String) {
    let state = app.state::<UiState>();
    state.with_store(|s| s.remove(&id));
    state.live.lock().unwrap().remove(&id);
    snapshot::forget(&id);
    emit_computers(&app);
}

struct UiEvents {
    app: AppHandle,
    id: String,
    /// Profil et format montrés dans la barre (le profil peut changer en session).
    label: Arc<Mutex<BarLabel>>,
}

#[derive(Clone, Serialize)]
struct BarLabel {
    label: String,
    profile: String,
}

#[derive(Clone, Serialize)]
struct Phase {
    id: String,
    phase: &'static str,
    detail: Option<String>,
}

impl HostEvents for UiEvents {
    fn status(&self, s: &Status) {
        let (phase, detail) = match s {
            Status::Preparing => ("preparing", None),
            Status::Connecting => ("connecting", None),
            Status::FirstImages => ("first_images", None),
            Status::Streaming => ("streaming", None),
            Status::Reconnecting { attempt, max } => ("reconnecting", Some(format!("{attempt}/{max}"))),
            Status::Ending => ("ending", None),
            Status::Ended { error, .. } => ("ended", error.clone()),
        };
        let _ = self.app.emit("session-status", Phase { id: self.id.clone(), phase, detail });
    }

    fn stats(&self, s: &Stats) {
        let _ = self.app.emit_to("bar", "stats", s);
    }

    /// Première image à l'écran : l'accueil laisse la place à la session,
    /// même si Windows a refusé le premier plan à celle-ci (l'accueil,
    /// resté devant, montrait le passage ouvert sur du noir).
    fn visible(&self, foreground: bool) {
        if !foreground {
            log::info!("session hors du premier plan : accueil caché quand même");
        }
        // Le passage s'ouvre à la première image montrée, pas à la connexion.
        let _ = self.app.emit_to("main", "session-shown", &self.id);
        if let Some(w) = self.app.get_webview_window("main") {
            let _ = w.hide();
        }
    }

    fn top_edge(&self, hwnd: isize) {
        let mut mi = MONITORINFO { cbSize: std::mem::size_of::<MONITORINFO>() as u32, ..Default::default() };
        unsafe {
            let m = MonitorFromWindow(HWND(hwnd as *mut _), MONITOR_DEFAULTTONEAREST);
            let _ = GetMonitorInfoW(m, &mut mi);
        }
        let r = mi.rcMonitor;
        let app = self.app.clone();
        let label = self.label.lock().unwrap().clone();
        // Sur le fil de Tauri : la fenêtre de la session ne doit pas attendre.
        let _ = self.app.run_on_main_thread(move || {
            // Toujours à sa taille sans menu en se montrant.
            bar_menu(app.clone(), false);
            let Some(bar) = app.get_webview_window("bar") else { return };
            let size = bar.outer_size().unwrap_or(tauri::PhysicalSize::new(1040, 76));
            let x = r.left + ((r.right - r.left) - size.width as i32) / 2;
            let _ = bar.set_position(tauri::PhysicalPosition::new(x, r.top));
            let _ = bar.set_always_on_top(true);
            let _ = bar.show();
            let _ = app.emit_to("bar", "bar-shown", &label);
        });
    }

    fn ended(&self, s: &Summary) {
        let state = self.app.state::<UiState>();
        *state.session.lock().unwrap() = None;
        // Dernière image : le fond de cet ordinateur sur l'accueil.
        let keep = state.with_store(|st| st.get(&self.id).map(|c| c.settings.keep_last_image)).unwrap_or(false);
        if let (true, Some(jpeg)) = (keep, &s.last_image) {
            snapshot::save(&self.id, jpeg);
            state.live.lock().unwrap().entry(self.id.clone()).or_default().last = Some(data_url(jpeg));
        }
        if s.error.is_none() && s.seconds > 0 {
            let at = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0);
            state.with_store(|st| {
                if let Some(c) = st.get_mut(&self.id) {
                    c.last = Some(LastSession {
                        median_ms: s.median_ms,
                        p95_ms: s.p95_ms,
                        seconds: s.seconds,
                        label: self.label.lock().unwrap().label.clone(),
                        at,
                    });
                    st.save();
                }
            });
        }
        if let Some(bar) = self.app.get_webview_window("bar") {
            let _ = bar.hide();
        }
        if let Some(w) = self.app.get_webview_window("main") {
            let _ = w.show();
            let _ = w.set_focus();
        }
        // Personne ne regardait (autre app devant) : une notification.
        if !shell::app_in_front() {
            let name = state.with_store(|st| st.get(&self.id).map(|c| c.name.clone())).unwrap_or_default();
            match &s.error {
                Some(e) => shell::notify(
                    &t("notify.stopped_title", &[("name", &name)]),
                    e,
                    Some((&t("notify.resume", &[]), &format!("kybernow://connect?id={}", self.id))),
                ),
                None => shell::notify(
                    &t("notify.ended_title", &[("name", &name)]),
                    &t("notify.ended_body", &[]),
                    Some((&t("notify.take_back", &[]), &format!("kybernow://connect?id={}", self.id))),
                ),
            }
        }
        #[derive(Serialize, Clone)]
        struct Ended<'a> {
            id: &'a str,
            summary: &'a Summary,
        }
        let _ = self.app.emit_to("main", "session-ended", Ended { id: &self.id, summary: s });
        emit_computers(&self.app);
        // L'app se fermait : la console est prévenue, on peut partir.
        if state.quitting.load(std::sync::atomic::Ordering::Relaxed) {
            self.app.exit(0);
        }
    }
}

/// Réglages de la session et mode de l'écran pour un ordinateur, d'après ses
/// réglages et ce que ce PC sait décoder.
fn session_plan(c: &Computer, pc: &ThisPc) -> (kybernow_core::profile::StreamSettings, kybernow_core::console::Mode, String) {
    let s = &c.settings;
    let choices = Choices {
        profile: Profile::parse(&s.profile),
        codec: s.codec.as_deref().and_then(Codec::parse),
        yuv444: s.yuv444,
        mbps: s.mbps,
        reliable: s.reliable,
        max_4k: s.max_4k && s.custom.is_none(),
        size: s.custom.filter(|m| m.width >= 640 && m.height >= 360 && m.refresh >= 24),
        lighten: s.battery == "alleger" && pc::on_battery(),
    };
    let caps = kybernow_core::profile::Capabilities { hevc: pc.hevc, hevc444: pc.hevc444, av1: pc.av1 };
    let (settings, mode) = plan(&choices, caps, system::screen_mode());
    let label = format!(
        "{} · {}{}",
        settings.label(),
        settings.codec.label(),
        if settings.yuv444 { " 4:4:4" } else { "" }
    );
    (settings, mode, label)
}

/// Lance la session vers un ordinateur avec ses réglages.
#[tauri::command(async)]
fn connect(app: AppHandle, id: String) -> Result<(), String> {
    let state = app.state::<UiState>();
    if state.session.lock().unwrap().is_some() {
        return Err(t("errors.session_already_open", &[]));
    }
    let c = state.with_store(|s| s.get(&id).cloned()).ok_or_else(|| t("errors.unknown_computer", &[]))?;
    // Sur lui-même, l'écran virtuel éteindrait les écrans où l'app s'affiche.
    let me = std::env::var("COMPUTERNAME").unwrap_or_default();
    let host_name = state.live.lock().unwrap().get(&id).and_then(|l| l.info.clone()).map(|i| i.hostname).unwrap_or_default();
    let local = system::is_this_pc(&c.host);
    if local || (!me.is_empty() && (host_name.eq_ignore_ascii_case(&me) || c.name.eq_ignore_ascii_case(&me))) {
        log::warn!("session refusée : {} ({}) est ce PC", c.name, c.host);
        return Err(t("errors.this_pc", &[]));
    }
    let password = store::password(&id).ok_or_else(|| t("errors.password_missing", &[]))?;
    let pc = {
        let cached = state.this_pc.lock().unwrap().clone();
        match cached {
            Some(pc) => pc,
            None => {
                let pc = pc::this_pc();
                *state.this_pc.lock().unwrap() = Some(pc.clone());
                pc
            }
        }
    };
    let s = &c.settings;
    let (settings, mode, label) = session_plan(&c, &pc);
    let audio_channels = match s.audio.as_str() {
        "auto" => audio::default_output_channels(),
        other => other.parse().unwrap_or(2),
    };
    let params = SessionParams {
        console_host: c.host.clone(),
        console_port: c.port,
        user: c.user.clone(),
        password,
        console_fingerprint: c.fingerprint.clone(),
        device: system::device_info(),
        mode,
        audio_channels,
        virtual_display: s.virtual_display,
        settings,
        auto_reconnect: true,
        gamepad_xbox360: s.gamepad != "fidele",
        clipboard: s.clipboard,
        recovery: Default::default(),
    };
    let mouse = match s.mouse.as_str() {
        "jeu" => Some(true),
        "bureau" => Some(false),
        _ => None,
    };
    log::info!("session vers {} ({}) : {label}, {}×{} à {} Hz", c.name, c.host, mode.width, mode.height, mode.refresh);
    // Tenu jusqu'à l'inscription de la session : une fin très rapide (dans
    // `ended`) ne peut pas passer avant.
    let mut running = state.session.lock().unwrap();
    if running.is_some() {
        return Err(t("errors.session_already_open", &[]));
    }
    let bar = Arc::new(Mutex::new(BarLabel { label: label.clone(), profile: s.profile.clone() }));
    let host = SessionHost::start(
        c.name.clone(),
        params,
        HostOptions {
            vsync: s.vsync,
            hardware: true,
            fullscreen: true,
            mouse,
            window_size: system::initial_window_size(mode),
        },
        Arc::new(UiEvents { app: app.clone(), id: id.clone(), label: bar.clone() }),
    );
    *running = Some(Running { id, host, bar });
    Ok(())
}

#[tauri::command(async)]
fn session_command(state: tauri::State<'_, UiState>, command: String) {
    let c = match command.as_str() {
        "disconnect" => Command::Disconnect,
        "fullscreen" => Command::Fullscreen,
        "mouse" => Command::MouseToggle,
        "keys:ctrl-alt-del" => Command::CtrlAltDel,
        "keys:win" => Command::WinKey,
        "keys:alt-tab" => Command::AltTab,
        "keys:print" => Command::PrintScreen,
        _ => return,
    };
    if let Some(r) = state.session.lock().unwrap().as_ref() {
        r.host.command(c);
    }
}

/// Autre profil depuis la barre : enregistré pour cet ordinateur (comme
/// depuis l'accueil) et appliqué tout de suite à la session.
#[tauri::command(async)]
fn session_profile(app: AppHandle, profile: String) -> Result<(), String> {
    let state = app.state::<UiState>();
    let Some((id, bar)) = state.session.lock().unwrap().as_ref().map(|r| (r.id.clone(), r.bar.clone())) else {
        return Err(t("errors.no_session", &[]));
    };
    Profile::parse(&profile).ok_or_else(|| t("errors.unknown_profile", &[]))?;
    let c = state.with_store(|s| {
        let c = s.get_mut(&id).ok_or_else(|| t("errors.unknown_computer", &[]))?;
        c.settings.profile = profile.clone();
        c.settings.codec = None;
        c.settings.yuv444 = None;
        c.settings.mbps = None;
        c.settings.reliable = None;
        let c = c.clone();
        s.save();
        Ok::<_, String>(c)
    })?;
    emit_computers(&app);
    let pc = state.this_pc.lock().unwrap().clone().unwrap_or_else(pc::this_pc);
    let (settings, _, label) = session_plan(&c, &pc);
    let shown = BarLabel { label, profile };
    *bar.lock().unwrap() = shown.clone();
    let _ = app.emit_to("bar", "bar-label", &shown);
    window::set_session_codec(settings.codec.fourcc());
    window::current_session().ok_or_else(|| t("errors.session_not_ready", &[]))?.set_settings(settings);
    Ok(())
}

/// Un menu de la barre s'ouvre : la fenêtre grandit vers le bas le temps du
/// menu (transparente, elle ne doit pas cacher la session le reste du temps).
#[tauri::command(async)]
fn bar_menu(app: AppHandle, open: bool) {
    if let Some(bar) = app.get_webview_window("bar") {
        let scale = bar.scale_factor().unwrap_or(1.0);
        let h = if open { 300.0 } else { 76.0 };
        let _ = bar.set_size(tauri::Size::Physical(tauri::PhysicalSize::new((1040.0 * scale) as u32, (h * scale) as u32)));
    }
}

/// La barre se cache : le clavier revient à la session.
#[tauri::command(async)]
fn bar_hide(app: AppHandle) {
    if let Some(bar) = app.get_webview_window("bar") {
        let _ = bar.hide();
    }
    // Un menu ouvert laissait la fenêtre agrandie pour la fois suivante.
    bar_menu(app.clone(), false);
    if let Some(r) = app.state::<UiState>().session.lock().unwrap().as_ref() {
        r.host.command(Command::Focus);
    }
}

pub fn run(args: Vec<String>) {
    let state = UiState::default();
    *state.action.lock().unwrap() = parse_action(&args);
    *state.prefs.lock().unwrap() = load_prefs();
    let app = tauri::Builder::default()
        .manage(state)
        .invoke_handler(tauri::generate_handler![
            snapshot,
            probe_address,
            parse_link,
            add_computer,
            pair_start,
            pair_poll,
            pair_cancel,
            save_settings,
            rename_computer,
            remove_computer,
            connect,
            session_command,
            bar_hide,
            bar_menu,
            session_profile,
            take_action,
            app_info,
            check_update,
            set_channel,
            set_language,
            install_update
        ])
        .setup(|app| {
            let handle = app.handle().clone();
            // Langue connue de l'interface dès son premier affichage (ensuite : l'événement `lang`).
            let lang_script = format!("window.__KN_LANG = '{}';", i18n::lang().code());
            WebviewWindowBuilder::new(app, "main", WebviewUrl::App("index.html".into()))
                .initialization_script(&lang_script)
                .title(shell::DISPLAY_NAME)
                .inner_size(1280.0, 800.0)
                .min_inner_size(960.0, 600.0)
                .decorations(false)
                .center()
                .build()?;
            WebviewWindowBuilder::new(app, "bar", WebviewUrl::App("bar.html".into()))
                .initialization_script(&lang_script)
                .title(t("app.bar_title", &[]))
                .inner_size(1040.0, 76.0)
                .decorations(false)
                .transparent(true)
                .always_on_top(true)
                .skip_taskbar(true)
                .resizable(false)
                .shadow(false)
                .focused(false)
                .visible(false)
                .build()?;

            log::info!("app : fenêtres créées");
            shell::register();
            let h = handle.clone();
            instance::set_sink(move |args| handle_args(&h, args));
            let views = handle.state::<UiState>().views();
            shell::update_jump_list(views.iter().map(|v| (v.computer.id.clone(), v.computer.name.clone())).collect());
            // Mises à jour : 15 s après le lancement, puis toutes les 6 heures.
            let h = handle.clone();
            std::thread::spawn(move || {
                std::thread::sleep(std::time::Duration::from_secs(15));
                loop {
                    if let Err(e) = check_now(&h) {
                        log::warn!("mises à jour : {e}");
                    }
                    std::thread::sleep(std::time::Duration::from_secs(6 * 3600));
                }
            });
            let h = handle.clone();
            std::thread::spawn(move || {
                let pc = pc::this_pc();
                *h.state::<UiState>().this_pc.lock().unwrap() = Some(pc.clone());
                let _ = h.emit_to("main", "this-pc", pc);
            });
            let h = handle.clone();
            let d = discovery::start(move |list| {
                let _ = h.emit_to("main", "discovered", list);
            });
            let _ = handle.state::<UiState>().discovery.set(d);
            let h = handle.clone();
            std::thread::spawn(move || poll(h));
            log::info!("app : prête");
            Ok(())
        })
        .on_window_event(|window, event| {
            let tauri::WindowEvent::CloseRequested { api, .. } = event else { return };
            let app = window.app_handle();
            match window.label() {
                // Fermer l'accueil quitte l'app ; pendant une session, après
                // sa fin propre (console prévenue, écrans rendus).
                "main" => {
                    let state = app.state::<UiState>();
                    let session = state.session.lock().unwrap();
                    if let Some(r) = session.as_ref() {
                        api.prevent_close();
                        let _ = window.hide();
                        state.quitting.store(true, std::sync::atomic::Ordering::Relaxed);
                        r.host.command(Command::Disconnect);
                    } else {
                        drop(session);
                        app.exit(0);
                    }
                }
                // La barre ne se ferme pas (Alt+F4) : elle se cache.
                _ => {
                    api.prevent_close();
                    let _ = window.hide();
                    if let Some(r) = app.state::<UiState>().session.lock().unwrap().as_ref() {
                        r.host.command(Command::Focus);
                    }
                }
            }
        })
        .build(tauri::generate_context!());
    match app {
        Ok(app) => app.run(|_, _| {}),
        Err(e) => system::error_box(&t("app.start_failed", &[("error", &e.to_string())])),
    }
}

#[cfg(test)]
mod tests {
    use super::parse_link_text;

    #[test]
    fn lien_de_la_console() {
        let c = parse_link_text("kybernow://192.168.1.100:31544?name=Bureau%20Salon&fp=sha256:aa:bb").unwrap();
        assert_eq!((c.host.as_str(), c.port), ("192.168.1.100", 31544));
        assert_eq!(c.name.as_deref(), Some("Bureau Salon"));
        assert_eq!(c.fingerprint.as_deref(), Some("sha256:aa:bb"));
        let v6 = parse_link_text("kybernow://[fe80::1]:9443").unwrap();
        assert_eq!((v6.host.as_str(), v6.port), ("fe80::1", 9443));
    }
}
