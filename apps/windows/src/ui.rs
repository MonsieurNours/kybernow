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
use crate::{audio, system};

#[derive(Debug, Clone, Default)]
struct Live {
    online: Option<bool>,
    info: Option<Info>,
    preview: Option<String>,
    preview_at: Option<Instant>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ComputerView {
    #[serde(flatten)]
    computer: Computer,
    online: Option<bool>,
    info: Option<Info>,
    preview: Option<String>,
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
}

impl UiState {
    fn with_store<T>(&self, f: impl FnOnce(&mut Store) -> T) -> T {
        let mut s = self.store.lock().unwrap();
        f(s.get_or_insert_with(Store::load))
    }

    fn views(&self) -> Vec<ComputerView> {
        let computers = self.with_store(|s| s.computers.clone());
        let live = self.live.lock().unwrap();
        computers
            .into_iter()
            .map(|c| {
                let l = live.get(&c.id).cloned().unwrap_or_default();
                ComputerView { computer: c, online: l.online, info: l.info, preview: l.preview }
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
    let _ = app.emit_to("main", "computers", state.views());
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
                live.get(&c.id).and_then(|l| l.preview_at).is_none_or(|t| t.elapsed() > Duration::from_secs(60))
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
    let fingerprint = console.fingerprint_seen().ok_or("aucun certificat vu")?;
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
    let rest = link.trim().strip_prefix("kybernow://").ok_or("ce n'est pas un lien kybernow://")?;
    let (authority, query) = rest.split_once('?').unwrap_or((rest, ""));
    let authority = authority.trim_end_matches('/');
    let (host, port) = if let Some(r) = authority.strip_prefix('[') {
        let (h, p) = r.split_once(']').ok_or("adresse invalide")?;
        (h.to_string(), p.trim_start_matches(':').to_string())
    } else {
        let (h, p) = authority.rsplit_once(':').ok_or("port absent du lien")?;
        (h.to_string(), p.to_string())
    };
    let port: u16 = port.parse().map_err(|_| "port invalide")?;
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
        return Err("le mot de passe n'a pas pu être rangé dans le gestionnaire d'identifiants".into());
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
    state.with_store(|s| {
        let c = s.get_mut(&id).ok_or("ordinateur inconnu")?;
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
        let c = s.get_mut(&id).ok_or("ordinateur inconnu")?;
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
    emit_computers(&app);
}

struct UiEvents {
    app: AppHandle,
    id: String,
    label: String,
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
            Status::Ended { error } => ("ended", error.clone()),
        };
        let _ = self.app.emit("session-status", Phase { id: self.id.clone(), phase, detail });
    }

    fn stats(&self, s: &Stats) {
        let _ = self.app.emit_to("bar", "stats", s);
    }

    fn visible(&self, foreground: bool) {
        if foreground {
            if let Some(w) = self.app.get_webview_window("main") {
                let _ = w.hide();
            }
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
        let label = self.label.clone();
        // Sur le fil de Tauri : la fenêtre de la session ne doit pas attendre.
        let _ = self.app.run_on_main_thread(move || {
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
                        label: self.label.clone(),
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

/// Lance la session vers un ordinateur avec ses réglages.
#[tauri::command(async)]
fn connect(app: AppHandle, id: String) -> Result<(), String> {
    let state = app.state::<UiState>();
    if state.session.lock().unwrap().is_some() {
        return Err("une session est déjà ouverte".into());
    }
    let c = state.with_store(|s| s.get(&id).cloned()).ok_or("ordinateur inconnu")?;
    let password = store::password(&id).ok_or("mot de passe introuvable : retirez puis ajoutez de nouveau cet ordinateur")?;
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
    let choices = Choices {
        profile: Profile::parse(&s.profile),
        codec: s.codec.as_deref().and_then(Codec::parse),
        yuv444: s.yuv444,
        mbps: s.mbps,
        reliable: s.reliable,
        max_4k: s.max_4k,
        size: None,
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
        return Err("une session est déjà ouverte".into());
    }
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
        Arc::new(UiEvents { app: app.clone(), id: id.clone(), label }),
    );
    *running = Some(Running { id, host });
    Ok(())
}

#[tauri::command(async)]
fn session_command(state: tauri::State<'_, UiState>, command: String) {
    let c = match command.as_str() {
        "disconnect" => Command::Disconnect,
        "fullscreen" => Command::Fullscreen,
        "mouse" => Command::MouseToggle,
        _ => return,
    };
    if let Some(r) = state.session.lock().unwrap().as_ref() {
        r.host.command(c);
    }
}

/// La barre se cache : le clavier revient à la session.
#[tauri::command(async)]
fn bar_hide(app: AppHandle) {
    if let Some(bar) = app.get_webview_window("bar") {
        let _ = bar.hide();
    }
    if let Some(r) = app.state::<UiState>().session.lock().unwrap().as_ref() {
        r.host.command(Command::Focus);
    }
}

pub fn run() {
    let app = tauri::Builder::default()
        .manage(UiState::default())
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
            bar_hide
        ])
        .setup(|app| {
            let handle = app.handle().clone();
            WebviewWindowBuilder::new(app, "main", WebviewUrl::App("index.html".into()))
                .title("KyberNow")
                .inner_size(1280.0, 800.0)
                .min_inner_size(960.0, 600.0)
                .decorations(false)
                .center()
                .build()?;
            WebviewWindowBuilder::new(app, "bar", WebviewUrl::App("bar.html".into()))
                .title("KyberNow — barre")
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
        Err(e) => system::error_box(&format!("L'app n'a pas pu démarrer : {e}")),
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
