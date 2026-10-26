//! L'app (Tauri, interface Svelte dans `ui/`) : accueil, ajout d'un
//! ordinateur, réglages, passage vers la session et barre de session.
//!
//! Fenêtres : `main` (accueil, sans cadre de Windows : la barre de titre est
//! dessinée par l'interface) et `bar` (barre de session, transparente, montrée
//! quand le pointeur touche le haut de l'écran de la session).
//! La session elle-même est une fenêtre Win32 sur son propre fil (`host.rs`).
//!
//! L'app vit aussi dans l'icône près de l'horloge (`tray.rs`) : fermer
//! l'accueil le cache, « Quitter KyberNow Link » la quitte. Ouverte à
//! l'ouverture de session (`--background`), elle démarre sans fenêtre.

use std::collections::HashMap;
use std::sync::{Arc, Condvar, Mutex, OnceLock};
use std::time::{Duration, Instant};

use base64::Engine;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindowBuilder};
use windows::Win32::Foundation::HWND;
use windows::Win32::Graphics::Gdi::{GetMonitorInfoW, MONITOR_DEFAULTTONEAREST, MONITORINFO, MonitorFromWindow};

use kybernow_core::console::{Console, Info, PairStatus, StationError};
use kybernow_core::profile::{Codec, Profile};
use kybernow_core::session::{SessionParams, Status};

use crate::discovery::{self, Discovery, Found};
use crate::host::{HostEvents, HostOptions, SessionHost, Stats, Summary};
use crate::plan::{Choices, plan};
use crate::store::{self, Computer, LastSession, Settings, Store};
use crate::window::Command;
use crate::pc::{self, ThisPc};
use crate::i18n::t;
use crate::follow::{self, Kind};
use crate::menu::{self, Cmd};
use crate::{audio, i18n, instance, login, shell, snapshot, system, tray, window};
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
    /// Réveil par le réseau envoyé : l'app attend l'ordinateur (2 min au plus).
    waking: bool,
    /// `restart` ou `update` : la Station redémarre ou s'installe, l'app
    /// attend son retour (`station-outcome` à la fin).
    station_pending: Option<&'static str>,
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
    sent: Arc<Mutex<Sent>>,
    /// Dernières mesures de la barre (menu de l'icône).
    stats: Arc<Mutex<Option<Stats>>>,
}

/// Ce que la session transmet (rubrique Statistiques à sa fin) : réglages du
/// flux, suivis quand le profil change depuis la barre, et mode de l'image,
/// à la taille que l'ordinateur envoie vraiment.
struct Sent {
    settings: kybernow_core::profile::StreamSettings,
    mode: kybernow_core::console::Mode,
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
    /// Réveils en cours (paquet envoyé à cet instant), suivis toutes les 2 s.
    waking: Mutex<HashMap<String, Instant>>,
    /// Redémarrages et mises à jour de la Station en attente de son retour.
    following: Mutex<HashMap<String, (follow::Pending, Instant)>>,
    /// Relevé demandé tout de suite (ouverture du menu de l'icône).
    poll_asked: Mutex<bool>,
    poll_signal: Condvar,
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
    /// L'accueil sur cet ordinateur (notification).
    Show { id: String },
    /// Ses réglages, sur une rubrique (`station`) ou la première.
    Settings { id: String, rubric: Option<String> },
    /// Les réglages de l'app ; `check` : en cherchant une mise à jour.
    App { check: bool },
}

fn parse_action(args: &[String]) -> Option<Action> {
    let first = args.first()?;
    if first.starts_with("kybernow://") {
        // Actions des notifications (link.rs) : kybernow://connect?id=…,
        // kybernow://open (?id=… : sur cet ordinateur), kybernow://station?id=…
        // (sa rubrique Station) ; les autres liens viennent d'Admin.
        return Some(match crate::link::action(first) {
            Some(("connect", Some(id))) => Action::Connect { id },
            Some(("open", Some(id))) => Action::Show { id },
            Some(("station", Some(id))) => Action::Settings { id, rubric: Some("station".into()) },
            Some(_) => Action::Open,
            None => Action::Link { link: first.clone() },
        });
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
    /// Sur batterie : `alleger` (30 Hz au plus) ou `garder` ; absent, repris
    /// des réglages des ordinateurs (voir `battery_choice`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub battery: Option<String>,
    /// « Ouvrir à l'ouverture de session » : absent avant la version qui
    /// l'apporte (elle le coche alors une fois, voir `login.rs`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub open_at_login: Option<bool>,
    /// La notification « KyberNow Link est toujours là » a été montrée.
    pub close_hint: bool,
}

/// Sur batterie, pour toutes les sessions de ce PC : `alleger` ou `garder`.
/// Sans choix dans `app.json`, celui que gardaient les réglages des
/// ordinateurs (`garder` si l'un l'avait).
fn battery_choice(state: &UiState) -> &'static str {
    let chosen = state.prefs.lock().unwrap().battery.clone();
    let keep = match chosen.as_deref() {
        Some(b) => b == "garder",
        None => state.with_store(|s| s.computers.iter().any(|c| c.settings.battery == "garder")),
    };
    if keep { "garder" } else { "alleger" }
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
/// `KyberNow-Link-Setup.exe` d'abord, puis `KyberNow-Client-Setup.exe`, seul
/// nom que listent les manifestes des versions 0.3.1 et avant (publié en copie
/// pour l'app de ces versions).
const INSTALLER: &[&str] = &["KyberNow-Link-Setup.exe", "KyberNow-Client-Setup.exe"];

impl UiState {
    fn with_store<T>(&self, f: impl FnOnce(&mut Store) -> T) -> T {
        let mut s = self.store.lock().unwrap();
        f(s.get_or_insert_with(Store::load))
    }

    fn views(&self) -> Vec<ComputerView> {
        let computers = self.with_store(|s| s.computers.clone());
        let waking: Vec<String> = self.waking.lock().unwrap().keys().cloned().collect();
        let pending: HashMap<String, Kind> = self.following.lock().unwrap().iter().map(|(id, (p, _))| (id.clone(), p.kind)).collect();
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
                let waking = waking.contains(&c.id);
                let station_pending = pending.get(&c.id).map(|k| k.code());
                ComputerView { computer: c, online: l.online, info: l.info, preview, preview_source, waking, station_pending }
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
    tray::refresh();
}

/// Une autre ouverture (raccourci, lien, barre des tâches, notification) :
/// l'accueil revient devant et reçoit la demande.
fn handle_args(app: &AppHandle, args: Vec<String>) {
    match args.first().map(String::as_str) {
        // Ouverture de session alors que l'app tourne déjà : rien à montrer.
        Some(login::BACKGROUND) => return,
        // Installateur ou désinstalleur : l'app se ferme (une session d'abord).
        Some("--quit") => return quit(app),
        // Bouton « Mettre à jour » d'une notification : comme le menu de l'icône.
        Some(link) if crate::link::action(link).is_some_and(|(a, _)| a == "station-update") => {
            if let Some((_, Some(id))) = crate::link::action(link) {
                tray_command(app, Cmd::Update(id));
            }
            return;
        }
        _ => {}
    }
    show_action(app, parse_action(&args).unwrap_or(Action::Open));
}

/// L'accueil revient devant et reçoit `action` (gardée aussi pour
/// l'interface, si elle n'écoute pas encore). Pendant une session, il reste
/// caché pour une connexion (une session à la fois) et se montre pour le
/// reste (réglages, ajout, menu de l'icône).
fn show_action(app: &AppHandle, action: Action) {
    let connect = matches!(action, Action::Connect { .. });
    if !matches!(action, Action::Open) {
        *app.state::<UiState>().action.lock().unwrap() = Some(action);
    }
    let a = app.clone();
    let _ = app.run_on_main_thread(move || {
        if let Some(w) = a.get_webview_window("main") {
            let in_session = a.state::<UiState>().session.lock().unwrap().is_some();
            if !in_session || !connect {
                let _ = w.unminimize();
                let _ = w.show();
                let _ = w.set_focus();
            }
        }
        // L'interface la reprend par take_action (une seule fois).
        let _ = a.emit_to("main", "action-ready", ());
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
    /// Sur batterie : `alleger` ou `garder`.
    battery: &'static str,
    /// « Ouvrir à l'ouverture de session », tel que Windows le fera.
    open_at_login: bool,
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
        battery: battery_choice(&state),
        open_at_login: login::enabled(),
    }
}

/// « Ouvrir à l'ouverture de session » dans les réglages de l'app. Rend
/// l'état tel que Windows le fera.
#[tauri::command(async)]
fn set_open_at_login(app: AppHandle, on: bool) -> bool {
    let state = app.state::<UiState>();
    let prefs = {
        let mut p = state.prefs.lock().unwrap();
        p.open_at_login = Some(on);
        p.clone()
    };
    save_prefs(&prefs);
    login::set(on)
}

/// « Sur batterie » dans les réglages de l'app : `alleger` ou `garder`.
#[tauri::command(async)]
fn set_battery(app: AppHandle, battery: String) -> &'static str {
    let state = app.state::<UiState>();
    let prefs = {
        let mut p = state.prefs.lock().unwrap();
        p.battery = Some(if battery == "garder" { "garder" } else { "alleger" }.into());
        p.clone()
    };
    save_prefs(&prefs);
    battery_choice(&state)
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

/// Interroge Admin d'un ordinateur : en ligne ou non, ce qu'il dit de lui
/// (`info`, Station comprise) et un aperçu de son écran, une fois par minute
/// au plus. Rend vrai si l'accueil doit le revoir.
fn refresh(app: &AppHandle, c: &Computer) -> bool {
    let state = app.state::<UiState>();
    let console = console_for(c);
    let info = console.info().ok();
    let online = Some(info.is_some());
    // Les cartes réseau pour le réveil, gardées avec l'ordinateur pour le
    // réveiller une fois éteint (le fichier n'est réécrit que si elles changent).
    let mut wake_changed = false;
    if let Some(i) = info.as_ref().filter(|i| !i.wake.is_empty() && i.wake != c.wake) {
        let wake = i.wake.clone();
        wake_changed = state.with_store(|s| {
            let found = s.get_mut(&c.id).map(|m| m.wake = wake).is_some();
            if found {
                s.save();
            }
            found
        });
    }
    if let Some(i) = &info {
        station_news(app, c, i);
    }
    let mut preview = None;
    let wants_preview = {
        let live = state.live.lock().unwrap();
        // Pas d'aperçu quand la dernière image de la session est là (comme sur Mac).
        live.get(&c.id).is_none_or(|l| l.last.is_none() && l.preview_at.is_none_or(|t| t.elapsed() > Duration::from_secs(60)))
    };
    if info.is_some() && wants_preview {
        if let Ok(jpeg) = console.preview(1280) {
            if !jpeg.is_empty() {
                preview = Some(data_url(&jpeg));
            }
        }
    }
    let mut live = state.live.lock().unwrap();
    let l = live.entry(c.id.clone()).or_default();
    // La Station suivie après un redémarrage ou une mise à jour : son état
    // (version, installation, redémarrage) compte aussi.
    let station_changed = info.as_ref().is_some_and(|i| l.info.as_ref().map(|o| &o.station) != Some(&i.station));
    let changed = l.online != online || preview.is_some() || station_changed || wake_changed;
    l.online = online;
    if info.is_some() {
        l.info = info;
    }
    if let Some(p) = preview {
        l.preview = Some(p);
        l.preview_at = Some(Instant::now());
    }
    changed
}

/// Une version de la Station prête (installable depuis l'app, ou avec
/// l'installateur) : une notification, une fois par version et par
/// ordinateur, quand l'app n'est pas au premier plan.
fn station_news(app: &AppHandle, c: &Computer, info: &Info) {
    let Some(u) = info.station.as_ref().map(|s| &s.update) else { return };
    let ready = u.available && !u.installing && (u.installable || u.requires_setup);
    let Some(v) = u.version.as_deref().filter(|_| ready) else { return };
    let state = app.state::<UiState>();
    if c.station_notified.as_deref() == Some(v) || state.following.lock().unwrap().contains_key(&c.id) || shell::app_in_front() {
        return;
    }
    let title = t("notify.station_update_title", &[("name", &c.name)]);
    let station = format!("kybernow://station?id={}", c.id);
    if u.installable {
        let button = t("notify.update", &[]);
        let link = format!("kybernow://station-update?id={}", c.id);
        shell::notify_to(&station, &title, &t("notify.station_update_body", &[("version", v)]), Some((&button, &link)));
    } else {
        shell::notify_to(&station, &title, &t("notify.station_update_setup", &[("version", v)]), None);
    }
    log::info!("notification : Station {v} prête pour {}", c.name);
    state.with_store(|s| {
        if let Some(m) = s.get_mut(&c.id) {
            m.station_notified = Some(v.to_string());
            s.save();
        }
    });
}

/// Chaque ordinateur, sauf celui de la session : toutes les 8 s quand
/// l'accueil est affiché, toutes les 30 s sans lui (l'app dans l'icône), et
/// tout de suite à l'ouverture du menu de l'icône (`poll_soon`).
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
            changed |= refresh(&app, &c);
        }
        if changed {
            emit_computers(&app);
        }
        let shown = app.get_webview_window("main").and_then(|w| w.is_visible().ok()).unwrap_or(true);
        let wait = Duration::from_secs(if shown { 8 } else { 30 });
        let asked = state.poll_asked.lock().unwrap();
        let (mut asked, _) = state.poll_signal.wait_timeout_while(asked, wait, |a| !*a).unwrap();
        *asked = false;
    }
}

/// Un relevé de tous les ordinateurs tout de suite (menu de l'icône ouvert).
pub fn poll_soon(app: &AppHandle) {
    let state = app.state::<UiState>();
    *state.poll_asked.lock().unwrap() = true;
    state.poll_signal.notify_all();
}

/// Les suivis en cours (réveils, Station qui redémarre ou s'installe), toutes
/// les 2 s : l'ordinateur est interrogé jusqu'à la fin du suivi, qui se dit à
/// l'interface et, selon le cas, par une notification (docs/link-ui.md).
fn follow_loop(app: AppHandle) {
    loop {
        std::thread::sleep(follow::POLL);
        let state = app.state::<UiState>();
        let in_session = state.session.lock().unwrap().as_ref().map(|r| r.id.clone());
        let waking: Vec<(String, Instant)> = state.waking.lock().unwrap().iter().map(|(k, v)| (k.clone(), *v)).collect();
        let following: Vec<String> = state.following.lock().unwrap().keys().cloned().collect();
        if waking.is_empty() && following.is_empty() {
            continue;
        }
        let mut changed = false;
        let mut ids: Vec<&String> = waking.iter().map(|(id, _)| id).chain(&following).collect();
        ids.sort();
        ids.dedup();
        for id in ids {
            if in_session.as_deref() == Some(id.as_str()) {
                continue;
            }
            let Some(c) = state.with_store(|s| s.get(id).cloned()) else {
                state.waking.lock().unwrap().remove(id);
                state.following.lock().unwrap().remove(id);
                continue;
            };
            changed |= refresh(&app, &c);
        }
        for (id, since) in waking {
            let Some(c) = state.with_store(|s| s.get(&id).cloned()) else { continue };
            let online = state.live.lock().unwrap().get(&id).and_then(|l| l.online);
            let step = follow::wake(online, since.elapsed());
            if step == follow::Wake::Waiting {
                continue;
            }
            state.waking.lock().unwrap().remove(&id);
            changed = true;
            log::info!("réveil de {} : {step:?}", c.name);
            if shell::app_in_front() {
                continue;
            }
            let home = format!("kybernow://open?id={id}");
            if step == follow::Wake::Online {
                let link = format!("kybernow://connect?id={id}");
                let button = t("notify.connect", &[]);
                shell::notify_to(&home, &t("notify.online_title", &[("name", &c.name)]), &t("notify.online_body", &[]), Some((&button, &link)));
            } else {
                shell::notify_to(&home, &t("notify.no_answer_title", &[("name", &c.name)]), &t("notify.no_answer_body", &[]), None);
            }
        }
        for id in following {
            let Some(c) = state.with_store(|s| s.get(&id).cloned()) else { continue };
            let (online, info) = state.live.lock().unwrap().get(&id).map(|l| (l.online, l.info.clone())).unwrap_or_default();
            let seen = info.as_ref().and_then(|i| i.station.as_ref()).map(|s| follow::Seen {
                version: &s.version,
                restarting: s.restarting,
                installing: s.update.installing,
            });
            let outcome = {
                let mut f = state.following.lock().unwrap();
                let Some((p, since)) = f.get_mut(&id) else { continue };
                let since = *since;
                let outcome = follow::station(p, online, seen, since.elapsed());
                outcome.map(|o| (o, f.remove(&id).map(|(p, _)| p.from_menu).unwrap_or(false)))
            };
            let Some((outcome, from_menu)) = outcome else { continue };
            changed = true;
            log::info!("Station de {} : {outcome:?}", c.name);
            #[derive(Serialize, Clone)]
            struct StationOutcome<'a> {
                id: &'a str,
                outcome: &'static str,
                version: Option<&'a str>,
            }
            let version = match &outcome {
                follow::Outcome::Updated(v) => Some(v.as_str()),
                _ => None,
            };
            let _ = app.emit_to("main", "station-outcome", StationOutcome { id: &id, outcome: outcome.code(), version });
            let text = match &outcome {
                follow::Outcome::Restarted => t("station.restarted", &[]),
                follow::Outcome::Updated(v) => t("station.updated", &[("version", v)]),
                follow::Outcome::UpdateFailed => t("station.update_failed", &[]),
                follow::Outcome::SlowBack => t("station.slow_back", &[]),
                follow::Outcome::Silent => continue,
            };
            if from_menu && !shell::app_in_front() {
                shell::notify_to(&format!("kybernow://station?id={id}"), &c.name, &text, None);
            }
        }
        if changed {
            emit_computers(&app);
        }
    }
}

/// Un ordinateur tout de suite : « Réessayer » à l'accueil, et le suivi de
/// la Station qui redémarre ou se met à jour (plus souvent que toutes les 8 s).
#[tauri::command(async)]
fn poll_now(app: AppHandle, id: String) -> Result<(), String> {
    let state = app.state::<UiState>();
    if state.session.lock().unwrap().as_ref().is_some_and(|r| r.id == id) {
        return Ok(());
    }
    let c = state.with_store(|s| s.get(&id).cloned()).ok_or_else(|| t("errors.unknown_computer", &[]))?;
    refresh(&app, &c);
    // Toujours : l'accueil sort de « Recherche… » même si rien n'a changé.
    emit_computers(&app);
    Ok(())
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
fn add(app: &AppHandle, candidate: &Candidate, user: &str, password: &str, by_code: bool) -> Result<String, String> {
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
            by_code,
            wake: info.wake.clone(),
            station_notified: None,
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
    add(&app, &candidate, user.trim(), &password, false)
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
            let id = add(&app, &candidate, &username, &password, true)?;
            *state.pairing.lock().unwrap() = None;
            Ok(id)
        }
    }
}

#[tauri::command(async)]
fn pair_cancel(state: tauri::State<'_, UiState>) {
    *state.pairing.lock().unwrap() = None;
}

/// Admin de l'ordinateur `id` (compte et empreinte retenus).
fn console_of(app: &AppHandle, id: &str) -> Result<Console, String> {
    let c = app.state::<UiState>().with_store(|s| s.get(id).cloned()).ok_or_else(|| t("errors.unknown_computer", &[]))?;
    Ok(console_for(&c))
}

/// Refus d'une commande de la Station, comme `StationError::to_json` : `kind`
/// (`in_session` : redemander avec `force` coupe la session de `device` ;
/// `too_soon` : attendre `seconds` ; `nothing_to_install`, `requires_setup`,
/// `busy`, `unsupported`, `other`) et `message`, dans la langue de l'app.
fn station_refusal(message: String) -> serde_json::Value {
    serde_json::json!({ "kind": "other", "message": message, "device": null, "seconds": null })
}

/// L'état de la Station change tout de suite à l'accueil (redémarrage ou
/// installation en cours), sans attendre le relevé suivant.
fn mark_station(app: &AppHandle, id: &str, change: impl FnOnce(&mut kybernow_core::console::StationInfo)) {
    let state = app.state::<UiState>();
    if let Some(station) = state.live.lock().unwrap().get_mut(id).and_then(|l| l.info.as_mut()).and_then(|i| i.station.as_mut()) {
        change(station);
    }
    emit_computers(app);
}

/// Refus d'un redémarrage ou d'une mise à jour : celui d'Admin, ou un
/// texte (ordinateur inconnu).
enum Refusal {
    Station(StationError),
    Text(String),
}

impl Refusal {
    fn to_json(&self) -> serde_json::Value {
        match self {
            Refusal::Station(e) => e.to_json(),
            Refusal::Text(m) => station_refusal(m.clone()),
        }
    }
}

/// Redémarre la Station de `id` (Admin et le serveur Kyber s'arrêtent puis
/// repartent, Kyber d'abord : environ 30 secondes, pendant lesquelles Admin
/// ne répond pas) ou la met à jour (la même installation qu'« Installer
/// maintenant » dans Admin ; bloque le temps qu'Admin télécharge le paquet).
/// `force` : même si un appareil est en session (elle est coupée). L'app suit
/// ensuite la Station jusqu'à son retour (`follow_loop`), que la demande
/// vienne des réglages ou du menu de l'icône (`from_menu` : une notification
/// à la fin). Rend la version installée.
fn station_call(app: &AppHandle, id: &str, kind: Kind, force: bool, from_menu: bool) -> Result<Option<String>, Refusal> {
    let console = console_of(app, id).map_err(Refusal::Text)?;
    let device = system::device_info().id;
    let version = match kind {
        Kind::Restart => {
            console.station_restart(Some(&device), force).map_err(Refusal::Station)?;
            log::info!("redémarrage de la Station de {} demandé", console.host());
            None
        }
        Kind::Update => {
            let v = console.station_update(Some(&device), force).map_err(Refusal::Station)?;
            log::info!("mise à jour de la Station de {} vers {v} lancée", console.host());
            Some(v)
        }
    };
    let state = app.state::<UiState>();
    let known = state.live.lock().unwrap().get(id).and_then(|l| l.info.as_ref()?.station.as_ref()?.update.version.clone());
    let expected = version.clone().filter(|v| !v.is_empty()).or(known).filter(|_| kind == Kind::Update);
    let pending = follow::Pending { kind, version: expected, down: false, from_menu };
    state.following.lock().unwrap().insert(id.to_string(), (pending, Instant::now()));
    mark_station(app, id, |s| match kind {
        Kind::Restart => s.restarting = true,
        Kind::Update => s.update.installing = true,
    });
    Ok(version)
}

/// « Redémarrer la Station » dans les réglages de l'ordinateur `id`.
/// Refus : voir `station_refusal`.
#[tauri::command(async)]
fn station_restart(app: AppHandle, id: String, force: bool) -> Result<(), serde_json::Value> {
    station_call(&app, &id, Kind::Restart, force, false).map(|_| ()).map_err(|e| e.to_json())
}

/// « Mettre à jour la Station » : rend la version installée. Refus : voir `station_refusal`.
#[tauri::command(async)]
fn station_update(app: AppHandle, id: String, force: bool) -> Result<String, serde_json::Value> {
    station_call(&app, &id, Kind::Update, force, false).map(|v| v.unwrap_or_default()).map_err(|e| e.to_json())
}

/// Redémarrer ou mettre à jour depuis le menu de l'icône (ou le bouton d'une
/// notification), comme la rubrique Station : la même confirmation quand un
/// appareil est en session, les mêmes refus, le même suivi.
fn station_from_menu(app: &AppHandle, id: &str, kind: Kind) {
    let mut force = false;
    loop {
        let refusal = match station_call(app, id, kind, force, true) {
            Ok(_) => return,
            Err(r) => r,
        };
        let text = match &refusal {
            Refusal::Station(StationError::InSession { device, .. }) if !force => {
                let title = match device {
                    Some(d) => t("station.in_session_title", &[("device", d)]),
                    None => t("station.in_session_anonymous", &[]),
                };
                let (message, yes) = match kind {
                    Kind::Restart => (t("station.in_session_restart", &[]), t("station.restart_anyway", &[])),
                    Kind::Update => (t("station.in_session_update", &[]), t("station.update_anyway", &[])),
                };
                if !tray::confirm(&title, &message, &yes) {
                    return;
                }
                force = true;
                continue;
            }
            Refusal::Station(StationError::TooSoon { .. }) => t("station.too_soon", &[]),
            Refusal::Station(StationError::Unsupported) => t("station.too_old", &[]),
            Refusal::Station(StationError::RequiresSetup { .. }) => {
                let state = app.state::<UiState>();
                let v = state.live.lock().unwrap().get(id).and_then(|l| l.info.as_ref()?.station.as_ref()?.update.version.clone());
                t("station.requires_setup", &[("version", &v.unwrap_or_default())])
            }
            Refusal::Station(e) => e.to_string(),
            Refusal::Text(m) => m.clone(),
        };
        tray::alert(&text);
        return;
    }
}

/// « Ouvrir Admin » : son adresse dans le navigateur par défaut.
#[tauri::command(async)]
fn open_admin(app: AppHandle, id: String) -> Result<(), String> {
    shell::open_url(&console_of(&app, &id)?.admin_url())
}

/// « Réveiller l'ordinateur » `id` (hors ligne) : le paquet magique vers ses
/// cartes réseau gardées (`wake` de `Computer`, voir `kybernow_core::wake`),
/// en diffusion et en direct. Rend le nombre de paquets partis. Refus, dans
/// la langue de l'app : aucune carte connue (Station d'avant la 0.5.0, ou
/// jamais vue allumée depuis), ou aucun paquet parti.
#[tauri::command(async)]
fn wake(app: AppHandle, id: String) -> Result<usize, String> {
    let c = app.state::<UiState>().with_store(|s| s.get(&id).cloned()).ok_or_else(|| t("errors.unknown_computer", &[]))?;
    let sent = kybernow_core::wake::send(&c.wake).map_err(|e| e.to_string())?;
    log::info!("réveil de {} : {sent} paquets envoyés", c.name);
    // L'app l'attend (2 min au plus), que la fenêtre soit ouverte ou non.
    app.state::<UiState>().waking.lock().unwrap().insert(id, Instant::now());
    emit_computers(&app);
    Ok(sent)
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
    sent: Arc<Mutex<Sent>>,
    stats: Arc<Mutex<Option<Stats>>>,
}

/// `bar-shown` : la barre se montre ; `pinned` : demandée par « Statistiques »
/// (menu de l'icône), elle ne repart pas d'elle-même tant que le pointeur n'y
/// est pas passé.
#[derive(Clone, Serialize)]
struct BarShown {
    #[serde(flatten)]
    label: BarLabel,
    pinned: bool,
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

/// `session-station-restarting` : la Station redémarre en dernier recours.
#[derive(Clone, Serialize)]
struct StationRestarting {
    id: String,
}

impl HostEvents for UiEvents {
    fn status(&self, s: &Status) {
        let (phase, detail) = match s {
            Status::Preparing => ("preparing", None),
            Status::Connecting => ("connecting", None),
            Status::FirstImages => ("first_images", None),
            Status::Streaming => ("streaming", None),
            Status::Reconnecting { attempt, max } => ("reconnecting", Some(format!("{attempt}/{max}"))),
            // L'étape « Connexion », puis `session-station-restarting` (ci-dessous).
            Status::StationRestarting => ("connecting", None),
            Status::Ending => ("ending", None),
            Status::Ended { error, .. } => ("ended", error.clone()),
        };
        let _ = self.app.emit("session-status", Phase { id: self.id.clone(), phase, detail });
        if matches!(s, Status::StationRestarting) {
            // Dernier recours de la connexion : « La Station redémarre… »
            // jusqu'au prochain `session-status` (voir ui/src/lib/api.ts).
            let _ = self.app.emit("session-station-restarting", StationRestarting { id: self.id.clone() });
        }
    }

    fn stats(&self, s: &Stats) {
        // L'image telle que l'ordinateur l'envoie (sans écran virtuel, la
        // taille de son écran, pas celle demandée).
        if s.width > 0 && s.height > 0 {
            let mut sent = self.sent.lock().unwrap();
            sent.mode.width = s.width;
            sent.mode.height = s.height;
        }
        let _ = self.app.emit_to("bar", "stats", s);
        *self.stats.lock().unwrap() = Some(s.clone());
        tray::refresh();
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
        let app = self.app.clone();
        let label = self.label.lock().unwrap().clone();
        // Sur le fil de Tauri : la fenêtre de la session ne doit pas attendre.
        let _ = self.app.run_on_main_thread(move || show_bar(&app, hwnd, label, false));
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
        // Une session d'au moins 3 s qui a mesuré sa latence : la rubrique
        // Statistiques de cet ordinateur la montre.
        if s.seconds >= 3 && s.median_ms.is_some() {
            let at = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0);
            let last = {
                let sent = self.sent.lock().unwrap();
                let m = sent.mode;
                LastSession {
                    median_ms: s.median_ms,
                    p95_ms: s.p95_ms,
                    seconds: s.seconds,
                    at,
                    width: Some(m.width),
                    height: Some(m.height),
                    refresh: Some(m.refresh),
                    codec: Some(sent.settings.codec.label().into()),
                    mbps: Some(sent.settings.bitrate(m.width, m.height, m.refresh) / 1_000_000),
                    label: String::new(),
                }
            };
            state.with_store(|st| {
                if let Some(c) = st.get_mut(&self.id) {
                    c.last = Some(last);
                    st.save();
                }
            });
        }
        if let Some(bar) = self.app.get_webview_window("bar") {
            let _ = bar.hide();
        }
        tray::refresh();
        let quitting = state.quitting.load(std::sync::atomic::Ordering::Relaxed);
        if let Some(w) = self.app.get_webview_window("main").filter(|_| !quitting) {
            let _ = w.show();
            let _ = w.set_focus();
        }
        // Personne ne regardait (autre app devant) : une notification. Un clic
        // dessus ouvre l'accueil sur cet ordinateur.
        if !shell::app_in_front() && !quitting {
            let name = state.with_store(|st| st.get(&self.id).map(|c| c.name.clone())).unwrap_or_default();
            let home = format!("kybernow://open?id={}", self.id);
            let again = format!("kybernow://connect?id={}", self.id);
            match &s.error {
                Some(e) => shell::notify_to(&home, &t("notify.stopped_title", &[("name", &name)]), e, Some((&t("notify.resume", &[]), &again))),
                None => shell::notify_to(
                    &home,
                    &t("notify.ended_title", &[("name", &name)]),
                    &t("notify.ended_body", &[]),
                    Some((&t("notify.take_back", &[]), &again)),
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
        if quitting {
            self.app.exit(0);
        }
    }
}

/// Réglages de la session et mode de l'écran pour un ordinateur, d'après ses
/// réglages, ce que ce PC sait décoder et « Sur batterie » des réglages de
/// l'app (`lighten` : alléger, et ce PC est sur batterie).
fn session_plan(c: &Computer, pc: &ThisPc, lighten: bool) -> (kybernow_core::profile::StreamSettings, kybernow_core::console::Mode, String) {
    let s = &c.settings;
    let choices = Choices {
        profile: Profile::parse(&s.profile),
        codec: s.codec.as_deref().and_then(Codec::parse),
        yuv444: s.yuv444,
        mbps: s.mbps,
        reliable: s.reliable,
        max_4k: s.max_4k && s.custom.is_none(),
        size: s.custom.filter(|m| m.width >= 640 && m.height >= 360 && m.refresh >= 24),
        lighten,
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
    let lighten = battery_choice(&state) == "alleger" && pc::on_battery();
    let (settings, mode, label) = session_plan(&c, &pc, lighten);
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
        settings: settings.clone(),
        auto_reconnect: s.auto_reconnect,
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
    let sent = Arc::new(Mutex::new(Sent { settings, mode }));
    let stats = Arc::new(Mutex::new(None));
    let host = SessionHost::start(
        c.name.clone(),
        params,
        HostOptions {
            vsync: s.vsync,
            hardware: true,
            fullscreen: true,
            mouse,
            window_size: system::initial_window_size(mode),
            title_stats: false,
        },
        Arc::new(UiEvents { app: app.clone(), id: id.clone(), label: bar.clone(), sent: sent.clone(), stats: stats.clone() }),
    );
    *running = Some(Running { id, host, bar, sent, stats });
    drop(running);
    tray::refresh();
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
    let Some((id, bar, sent)) = state.session.lock().unwrap().as_ref().map(|r| (r.id.clone(), r.bar.clone(), r.sent.clone())) else {
        return Err(t("errors.no_session", &[]));
    };
    Profile::parse(&profile).ok_or_else(|| t("errors.unknown_profile", &[]))?;
    let c = save_profile(&state, &id, &profile)?;
    emit_computers(&app);
    let pc = state.this_pc.lock().unwrap().clone().unwrap_or_else(pc::this_pc);
    let lighten = battery_choice(&state) == "alleger" && pc::on_battery();
    let (settings, _, label) = session_plan(&c, &pc, lighten);
    sent.lock().unwrap().settings = settings.clone();
    let shown = BarLabel { label, profile };
    *bar.lock().unwrap() = shown.clone();
    let _ = app.emit_to("bar", "bar-label", &shown);
    window::set_session_codec(settings.codec.fourcc());
    window::current_session().ok_or_else(|| t("errors.session_not_ready", &[]))?.set_settings(settings);
    Ok(())
}

/// Le profil d'un ordinateur, retouches effacées (comme l'accueil).
fn save_profile(state: &UiState, id: &str, profile: &str) -> Result<Computer, String> {
    state.with_store(|s| {
        let c = s.get_mut(id).ok_or_else(|| t("errors.unknown_computer", &[]))?;
        c.settings.profile = profile.into();
        c.settings.codec = None;
        c.settings.yuv444 = None;
        c.settings.mbps = None;
        c.settings.reliable = None;
        let c = c.clone();
        s.save();
        Ok::<_, String>(c)
    })
}

/// Un profil choisi sur l'accueil ou dans le menu de l'icône : enregistré
/// pour cet ordinateur ; pendant sa session, appliqué tout de suite, comme
/// depuis la barre.
#[tauri::command(async)]
fn set_profile(app: AppHandle, id: String, profile: String) -> Result<(), String> {
    Profile::parse(&profile).ok_or_else(|| t("errors.unknown_profile", &[]))?;
    let state = app.state::<UiState>();
    if state.session.lock().unwrap().as_ref().is_some_and(|r| r.id == id) {
        return session_profile(app.clone(), profile);
    }
    save_profile(&state, &id, &profile)?;
    emit_computers(&app);
    Ok(())
}

/// Largeur de la fenêtre de la barre de session, en points (la barre
/// elle-même, centrée dedans, est plus étroite : `bar_width`).
const BAR_WINDOW_WIDTH: f64 = 1040.0;

/// La barre de session au bord haut de la zone cliente de la session (en
/// plein écran, le haut de l'écran), centrée sur elle : là où le trait la
/// signale et où le pointeur l'ouvre. À sa taille sans menu.
fn show_bar(app: &AppHandle, hwnd: isize, label: BarLabel, pinned: bool) {
    let mut mi = MONITORINFO { cbSize: std::mem::size_of::<MONITORINFO>() as u32, ..Default::default() };
    unsafe {
        let m = MonitorFromWindow(HWND(hwnd as *mut _), MONITOR_DEFAULTTONEAREST);
        let _ = GetMonitorInfoW(m, &mut mi);
    }
    let r = mi.rcMonitor;
    // Toujours à sa taille sans menu en se montrant.
    bar_menu(app.clone(), false);
    let Some(bar) = app.get_webview_window("bar") else { return };
    // Session réduite ou sans fenêtre : le haut de l'écran.
    let (center, top, scale) =
        window::bar_anchor(hwnd).unwrap_or(((r.left + r.right) / 2, r.top, bar.scale_factor().unwrap_or(1.0) as f32));
    // Sa largeur à l'échelle de l'écran où elle va, pas de celui où elle était.
    let width = (BAR_WINDOW_WIDTH * scale as f64).round() as i32;
    let x = (center - width / 2).max(r.left).min(r.right - width);
    let _ = bar.set_position(tauri::PhysicalPosition::new(x, top.max(r.top)));
    let _ = bar.set_always_on_top(true);
    let _ = bar.show();
    window::set_bar_visible(true);
    let _ = app.emit_to("bar", "bar-shown", BarShown { label, pinned });
}

/// « Statistiques » du menu de l'icône : la barre et ses mesures en direct,
/// ou cachée si elle est déjà là ; le clavier reste à la session.
fn toggle_stats(app: &AppHandle) {
    let state = app.state::<UiState>();
    let Some((hwnd, label)) = state.session.lock().unwrap().as_ref().map(|r| (r.host.hwnd(), r.bar.lock().unwrap().clone())) else {
        return;
    };
    let a = app.clone();
    let _ = app.run_on_main_thread(move || {
        let shown = a.get_webview_window("bar").and_then(|b| b.is_visible().ok()).unwrap_or(false);
        if shown {
            bar_hide(a.clone());
            return;
        }
        show_bar(&a, hwnd, label, true);
        if let Some(r) = a.state::<UiState>().session.lock().unwrap().as_ref() {
            r.host.command(Command::Focus);
        }
    });
}

/// Un menu de la barre s'ouvre : la fenêtre grandit vers le bas le temps du
/// menu (transparente, elle ne doit pas cacher la session le reste du temps).
#[tauri::command(async)]
fn bar_menu(app: AppHandle, open: bool) {
    if let Some(bar) = app.get_webview_window("bar") {
        let scale = bar.scale_factor().unwrap_or(1.0);
        let h = if open { 300.0 } else { 76.0 };
        let _ = bar.set_size(tauri::Size::Physical(tauri::PhysicalSize::new((BAR_WINDOW_WIDTH * scale) as u32, (h * scale) as u32)));
    }
}

/// La barre dit sa largeur en points (le texte des mesures et la langue la
/// font varier) : le pointeur l'ouvre dans une zone du bord haut centrée de
/// cette largeur.
#[tauri::command]
fn bar_width(width: f64) {
    window::set_bar_width(width);
}

/// La barre se cache : le clavier revient à la session.
#[tauri::command(async)]
fn bar_hide(app: AppHandle) {
    if let Some(bar) = app.get_webview_window("bar") {
        let _ = bar.hide();
    }
    window::set_bar_visible(false);
    // Un menu ouvert laissait la fenêtre agrandie pour la fois suivante.
    bar_menu(app.clone(), false);
    if let Some(r) = app.state::<UiState>().session.lock().unwrap().as_ref() {
        r.host.command(Command::Focus);
    }
}

/// Les réglages s'écartent du profil (« Personnalisé ») : `isCustom` de
/// l'interface (ui/src/lib/api.ts).
fn is_custom(s: &Settings, pc: Option<&ThisPc>) -> bool {
    let base = if pc.is_none_or(|p| p.hevc) { "hevc" } else { "h264" };
    s.codec.as_deref().is_some_and(|c| c != base) || s.yuv444.is_some() || s.mbps.is_some() || s.reliable.is_some()
}

/// Ce que le menu de l'icône montre (`menu.rs`), lu à chaque ouverture et,
/// menu ouvert, chaque seconde.
pub fn tray_state(app: &AppHandle) -> menu::State {
    let state = app.state::<UiState>();
    let pc = state.this_pc.lock().unwrap().clone();
    let computers = state.with_store(|s| s.computers.clone());
    let waking: Vec<String> = state.waking.lock().unwrap().keys().cloned().collect();
    let following: HashMap<String, Kind> = state.following.lock().unwrap().iter().map(|(k, (p, _))| (k.clone(), p.kind)).collect();
    let session = state.session.lock().unwrap().as_ref().map(|r| {
        let stats = r.stats.lock().unwrap().clone();
        let sent = r.sent.lock().unwrap();
        let m = sent.mode;
        menu::Session {
            id: r.id.clone(),
            name: computers.iter().find(|c| c.id == r.id).map(|c| c.name.clone()).unwrap_or_default(),
            latency_ms: stats.as_ref().and_then(|s| s.latency_ms),
            fps: stats.as_ref().map(|s| s.fps),
            mbps: Some(sent.settings.bitrate(m.width, m.height, m.refresh) / 1_000_000),
            fullscreen: window::fullscreen_now(),
            mouse: computers.iter().find(|c| c.id == r.id).map(|c| c.settings.mouse.clone()).unwrap_or_default(),
        }
    });
    let live = state.live.lock().unwrap();
    let computers = computers
        .iter()
        .map(|c| {
            let l = live.get(&c.id);
            let station = l.and_then(|l| l.info.as_ref()?.station.as_ref()).map(|s| menu::Station {
                busy: match following.get(&c.id) {
                    Some(Kind::Restart) => Some(menu::Busy::Restarting),
                    Some(Kind::Update) => Some(menu::Busy::Updating),
                    None if s.restarting => Some(menu::Busy::Restarting),
                    None if s.update.installing => Some(menu::Busy::Updating),
                    None => None,
                },
                update: s.update.version.clone().filter(|_| s.update.available && s.update.installable),
            });
            menu::Computer {
                id: c.id.clone(),
                name: c.name.clone(),
                online: l.and_then(|l| l.online),
                profile: c.settings.profile.clone(),
                custom: is_custom(&c.settings, pc.as_ref()),
                can_wake: !c.wake.is_empty(),
                waking: waking.contains(&c.id),
                station,
            }
        })
        .collect();
    menu::State { session, computers, app_update: state.update.lock().unwrap().as_ref().map(|u| u.version.clone()) }
}

/// Ce qui est choisi dans le menu de l'icône, sur son propre fil (les appels
/// à Admin et les questions ne bloquent pas l'icône). Les mêmes commandes que
/// l'accueil, les réglages et la barre.
pub fn tray_command(app: &AppHandle, cmd: Cmd) {
    let app = app.clone();
    std::thread::spawn(move || {
        let state = app.state::<UiState>();
        let session = |c: Command| {
            if let Some(r) = state.session.lock().unwrap().as_ref() {
                r.host.command(c);
            }
        };
        let fail = |r: Result<(), String>| {
            if let Err(e) = r {
                tray::alert(&e);
            }
        };
        match cmd {
            // Le clavier revient ensuite à la session (le menu l'avait pris).
            Cmd::Fullscreen => {
                session(Command::Fullscreen);
                session(Command::Focus);
            }
            Cmd::Mouse(mode) => {
                // Gardé pour l'ordinateur de la session, comme dans ses réglages.
                let id = state.session.lock().unwrap().as_ref().map(|r| r.id.clone());
                if let Some(id) = id {
                    state.with_store(|s| {
                        if let Some(c) = s.get_mut(&id) {
                            c.settings.mouse = mode.into();
                            s.save();
                        }
                    });
                    emit_computers(&app);
                }
                session(match mode {
                    "jeu" => Command::MouseGame,
                    "bureau" => Command::MouseDesktop,
                    _ => Command::MouseAuto,
                });
                session(Command::Focus);
            }
            Cmd::Statistics => toggle_stats(&app),
            Cmd::Disconnect => session(Command::Disconnect),
            Cmd::Connect(id) => show_action(&app, Action::Connect { id }),
            Cmd::Wake(id) => fail(wake(app.clone(), id).map(|_| ())),
            Cmd::Retry(id) => {
                let _ = poll_now(app.clone(), id);
            }
            Cmd::Profile(id, p) => fail(set_profile(app.clone(), id, p.into())),
            Cmd::Settings(id) => show_action(&app, Action::Settings { id, rubric: None }),
            Cmd::Admin(id) => fail(open_admin(app.clone(), id)),
            Cmd::Restart(id) => station_from_menu(&app, &id, Kind::Restart),
            Cmd::Update(id) => station_from_menu(&app, &id, Kind::Update),
            Cmd::Open => show_action(&app, Action::Open),
            Cmd::Add => show_action(&app, Action::Add),
            Cmd::AppSettings => show_action(&app, Action::App { check: false }),
            Cmd::CheckUpdates => show_action(&app, Action::App { check: true }),
            Cmd::Quit => quit(&app),
        }
    });
}

/// « Quitter KyberNow Link » : une session en cours se termine d'abord
/// (Admin prévenue, écrans rendus), puis l'app se ferme.
pub fn quit(app: &AppHandle) {
    let state = app.state::<UiState>();
    let session = state.session.lock().unwrap();
    if let Some(r) = session.as_ref() {
        log::info!("quitter : fin de la session d'abord");
        state.quitting.store(true, std::sync::atomic::Ordering::Relaxed);
        r.host.command(Command::Disconnect);
    } else {
        drop(session);
        log::info!("quitter");
        app.exit(0);
    }
}

/// La première fermeture de l'accueil : une notification dit où l'app
/// continue, une seule fois.
fn first_close(app: &AppHandle) {
    let state = app.state::<UiState>();
    let prefs = {
        let mut p = state.prefs.lock().unwrap();
        if p.close_hint {
            return;
        }
        p.close_hint = true;
        p.clone()
    };
    save_prefs(&prefs);
    shell::notify(&t("notify.close_title", &[]), &t("notify.close_body", &[]), None);
}

/// « Ouvrir à l'ouverture de session » : coché une fois, au premier lancement
/// de la version qui l'apporte ; ensuite le choix de l'utilisateur (et de
/// Windows, voir `login.rs`) est gardé.
fn open_at_login_default(state: &UiState) {
    let first = {
        let mut p = state.prefs.lock().unwrap();
        let first = p.open_at_login.is_none();
        if first {
            p.open_at_login = Some(true);
        }
        first.then(|| p.clone())
    };
    match first {
        Some(prefs) => {
            save_prefs(&prefs);
            login::set(true);
        }
        None => login::refresh(),
    }
}

pub fn run(args: Vec<String>) {
    // Ouverte à l'ouverture de session : sans fenêtre, dans l'icône seule.
    let background = args.iter().any(|a| a == login::BACKGROUND);
    // Bouton « Mettre à jour » d'une notification, l'app fermée : la mise à
    // jour de la Station suit le lancement.
    let update_asked = match args.first().and_then(|a| crate::link::action(a)) {
        Some(("station-update", Some(id))) => Some(id),
        _ => None,
    };
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
            bar_width,
            session_profile,
            take_action,
            app_info,
            check_update,
            set_channel,
            set_language,
            set_battery,
            install_update,
            poll_now,
            station_restart,
            station_update,
            open_admin,
            wake,
            set_profile,
            set_open_at_login
        ])
        .setup(move |app| {
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
                .visible(!background)
                .build()?;
            WebviewWindowBuilder::new(app, "bar", WebviewUrl::App("bar.html".into()))
                .initialization_script(&lang_script)
                .title(t("app.bar_title", &[]))
                .inner_size(BAR_WINDOW_WIDTH, 76.0)
                .decorations(false)
                .transparent(true)
                .always_on_top(true)
                .skip_taskbar(true)
                .resizable(false)
                .shadow(false)
                .focused(false)
                .visible(false)
                .build()?;

            log::info!("app : fenêtres créées{}", if background { " (ouverture de session : accueil caché)" } else { "" });
            shell::register();
            open_at_login_default(&handle.state::<UiState>());
            tray::start(handle.clone());
            if let Some(id) = update_asked.clone() {
                tray_command(&handle, Cmd::Update(id));
            }
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
            let h = handle.clone();
            std::thread::spawn(move || follow_loop(h));
            log::info!("app : prête");
            Ok(())
        })
        .on_window_event(|window, event| {
            let tauri::WindowEvent::CloseRequested { api, .. } = event else { return };
            let app = window.app_handle();
            match window.label() {
                // Fermer l'accueil le cache : l'app continue dans l'icône près
                // de l'horloge (« Quitter KyberNow Link » la quitte). Une
                // session en cours continue.
                "main" => {
                    api.prevent_close();
                    let _ = window.hide();
                    first_close(app);
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
        Ok(app) => app.run(|_, event| {
            // L'icône part avec l'app (sinon elle reste jusqu'au passage de la souris).
            if let tauri::RunEvent::Exit = event {
                tray::remove();
            }
        }),
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
