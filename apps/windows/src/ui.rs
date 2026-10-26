//! L'app (Tauri, interface Svelte dans `ui/`) : accueil, ajout d'un
//! ordinateur, réglages, passage vers la session et barre de session.
//!
//! Fenêtres : `main` (accueil, sans cadre de Windows : la barre de titre est
//! dessinée par l'interface), `bar` (barre de session, transparente, montrée
//! quand le pointeur touche le haut de l'écran de la session) et `fond-N` (le
//! fond de la connexion sur les écrans de la session autres que l'écran
//! principal, `fonds.rs`).
//! La session elle-même est faite de fenêtres Win32 (une par écran) sur son
//! propre fil (`host.rs`).
//!
//! L'app vit aussi dans l'icône près de l'horloge (`tray.rs`) : fermer
//! l'accueil le cache, « Quitter KyberNow Link » la quitte. Ouverte à
//! l'ouverture de session (`--background`), elle démarre sans fenêtre.
//!
//! Plusieurs ordinateurs à la fois (docs/plusieurs-ordinateurs.md) : une
//! table des sessions (quatre au plus), chacune repérée par son ordinateur
//! dans chaque événement et chaque commande. La première s'ouvre comme
//! d'habitude (passage de l'accueil, plein écran, fonds des autres écrans) ;
//! chaque suivante s'ouvre dans une fenêtre, sur l'écran de la fenêtre active,
//! et sa connexion se montre dans une fenêtre à sa place (`connexion-N`,
//! `connexion.html`), sans passer par l'accueil.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Condvar, Mutex, OnceLock};
use std::time::{Duration, Instant};

use base64::Engine;
use serde::{Deserialize, Serialize};
use tauri::window::Color;
use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindowBuilder};
use windows::Win32::Foundation::HWND;
use windows::Win32::Graphics::Gdi::{GetMonitorInfoW, MONITOR_DEFAULTTONEAREST, MONITORINFO, MonitorFromWindow};

use kybernow_core::console::{Console, Info, PairStatus, StationError};
use kybernow_core::profile::{Codec, Profile};
use kybernow_core::screens::{self, DisplaysSetting};
use kybernow_core::session::{SessionParams, Status};

use crate::discovery::{self, Discovery, Found};
use crate::displays::{self, Layout, Spec};
use crate::fonds::{self, Fonds};
use crate::host::{HostEvents, HostOptions, Role, ScreenSet, SessionHost, Stats, Summary};
use crate::plan::{Choices, plan};
use crate::store::{self, Computer, LastSession, Settings, Store};
use crate::window::Command;
use crate::pc::{self, ThisPc};
use crate::i18n::t;
use crate::follow::{self, Kind};
use crate::menu::{self, Cmd};
use crate::{audio, focus, i18n, instance, login, shell, snapshot, system, tray, window};
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

/// Une session en cours (une par ordinateur).
struct Running {
    id: String,
    host: SessionHost,
    bar: Arc<Mutex<BarLabel>>,
    sent: Arc<Mutex<Sent>>,
    /// Dernières mesures de la barre (menu de l'icône).
    stats: Arc<Mutex<Option<Stats>>>,
}

/// La connexion d'une session ouverte pendant qu'une autre tourne (plusieurs
/// ordinateurs) : une fenêtre à la place de celle de la session
/// (`connexion.html`, le même passage que l'accueil), jusqu'à la première
/// image ; un échec y reste, avec sa raison, jusqu'à « Fermer ».
#[derive(Default)]
struct Connexion {
    /// Sa fenêtre Tauri quand elle est ouverte.
    label: Mutex<Option<String>>,
    /// Ce qu'elle montre (lu par la page à son ouverture, puis les événements).
    view: Mutex<ConnexionView>,
}

#[derive(Clone, Serialize, Default)]
struct ConnexionView {
    id: String,
    phase: &'static str,
    restarting: bool,
    /// La session s'est terminée avant sa première image : la raison.
    error: Option<String>,
}

/// Numéro des fenêtres de connexion (`connexion-N`).
static CONNEXION_SEQ: AtomicU32 = AtomicU32::new(0);

/// Ce que la session transmet (rubrique Statistiques à sa fin) : réglages du
/// flux, suivis quand le profil change depuis la barre, et mode de l'image,
/// à la taille que l'ordinateur envoie vraiment.
struct Sent {
    settings: kybernow_core::profile::StreamSettings,
    /// Définition et fréquence de l'écran principal.
    mode: kybernow_core::console::Mode,
    /// Écrans de la session, et leurs pixels par seconde ensemble (débit total).
    screens: u32,
    pixel_rate: f64,
    /// L'écran virtuel est demandé (sans lui, une session à un écran).
    virtual_display: bool,
}

impl Sent {
    /// Débit demandé à l'ordinateur, en Mbit/s : celui du profil pour
    /// l'écran, ou pour la somme des écrans.
    fn mbps(&self) -> u32 {
        let bits = if self.screens > 1 && self.pixel_rate > 0.0 {
            self.settings.bitrate_for_pixel_rate(self.pixel_rate)
        } else {
            self.settings.bitrate(self.mode.width, self.mode.height, self.mode.refresh)
        };
        bits / 1_000_000
    }
}

/// L'accueil et la connexion de la session sur les écrans (multi-écrans,
/// docs/link-ui.md « Plusieurs écrans »). La connexion se lit sur l'écran
/// principal de ce PC : l'accueil y passe, et reprend sa place à la fin.
#[derive(Default)]
struct Spot {
    /// L'écran principal de la session : son nom et son cadre.
    primary: Option<fonds::Target>,
    /// Les autres écrans pris par la session (le fond de la connexion y est).
    others: Vec<fonds::Target>,
    /// Où était l'accueil avant de passer sur l'écran principal.
    before: Option<tauri::PhysicalPosition<i32>>,
    /// Une image a été montrée : la session est établie, la connexion qui
    /// reprend ensuite (écrans de ce PC ou profil changés) est une reprise.
    shown: bool,
    /// Les écrans qui ont montré leur image depuis le début de la connexion
    /// (ou de sa reprise) : leur fond est parti, il ne revient pas.
    visible: Vec<String>,
    /// Reprise de la connexion en session : l'accueil couvre l'écran principal.
    replanning: Option<Replan>,
}

/// L'accueil avant qu'il couvre l'écran principal pour une reprise de la connexion.
struct Replan {
    visible: bool,
    position: Option<tauri::PhysicalPosition<i32>>,
    size: Option<tauri::PhysicalSize<u32>>,
}

#[derive(Default)]
pub struct UiState {
    store: Mutex<Option<Store>>,
    live: Mutex<HashMap<String, Live>>,
    discovery: OnceLock<Arc<Discovery>>,
    this_pc: Mutex<Option<ThisPc>>,
    pairing: Mutex<Option<Pairing>>,
    /// Les sessions en cours, dans l'ordre d'ouverture (plusieurs ordinateurs,
    /// quatre au plus).
    sessions: Mutex<Vec<Running>>,
    /// Les ordinateurs dont la connexion se lance (`connect`, jusqu'à
    /// l'inscription de leur session) : ils comptent dans les quatre.
    launching: Mutex<Vec<String>>,
    /// Fermeture demandée pendant des sessions : quitter à la fin de la dernière.
    quitting: std::sync::atomic::AtomicBool,
    /// L'ordinateur de la session que sert la barre (une seule barre).
    bar_session: Mutex<Option<String>>,
    /// Les fenêtres de connexion ouvertes (libellé → connexion), y compris
    /// celles qui montrent un échec après la fin de leur session.
    connexions: Mutex<HashMap<String, Arc<Connexion>>>,
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
    /// Fond de la connexion sur les écrans autres que l'écran principal.
    fonds: OnceLock<Arc<Fonds>>,
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

    /// Les ordinateurs en session, dans l'ordre d'ouverture.
    fn session_ids(&self) -> Vec<String> {
        self.sessions.lock().unwrap().iter().map(|r| r.id.clone()).collect()
    }

    fn in_session(&self, id: &str) -> bool {
        self.sessions.lock().unwrap().iter().any(|r| r.id == id)
    }

    /// `f` sur la session de cet ordinateur, s'il est en session.
    fn with_session<T>(&self, id: &str, f: impl FnOnce(&Running) -> T) -> Option<T> {
        self.sessions.lock().unwrap().iter().find(|r| r.id == id).map(f)
    }

    /// La session active (celle de la dernière fenêtre de session au premier
    /// plan), sinon la première.
    fn active_id(&self) -> Option<String> {
        let sessions = self.sessions.lock().unwrap();
        sessions.iter().find(|r| r.host.is_active()).or_else(|| sessions.first()).map(|r| r.id.clone())
    }

    /// Quatre ordinateurs sont en session ou s'y connectent (ou le registre
    /// du cœur en tient quatre).
    fn full(&self) -> bool {
        let sessions = self.sessions.lock().unwrap();
        sessions.len() + self.launching.lock().unwrap().len() >= focus::MAX_SESSIONS || focus::count() >= focus::MAX_SESSIONS
    }
}

#[derive(Serialize)]
pub struct Snapshot {
    computers: Vec<ComputerView>,
    discovered: Vec<Found>,
    this_pc: Option<ThisPc>,
    /// Les ordinateurs en session, dans l'ordre d'ouverture.
    sessions: Vec<String>,
    version: String,
}

fn emit_computers(app: &AppHandle) {
    let state = app.state::<UiState>();
    let views = state.views();
    shell::update_jump_list(views.iter().map(|v| (v.computer.id.clone(), v.computer.name.clone())).collect());
    let _ = app.emit_to("main", "computers", views);
    emit_bar_computers(app);
    tray::refresh();
}

/// Les sessions ont changé (une de plus, une de moins) : l'accueil (« Afficher
/// la session », quatre au plus), la barre (menu « Ordinateurs ») et l'icône.
fn emit_sessions(app: &AppHandle) {
    let ids = app.state::<UiState>().session_ids();
    let _ = app.emit("sessions", &ids);
    emit_bar_computers(app);
    tray::refresh();
}

/// Un ordinateur du menu « Ordinateurs » de la barre (plusieurs ordinateurs) :
/// son état et son point de couleur comme dans le menu de l'icône, et ce que
/// fait un clic (`menu::Pick`).
#[derive(Clone, Serialize)]
struct BarComputer {
    id: String,
    name: String,
    /// `online`, `offline`, `searching`.
    state: &'static str,
    /// `ok`, `bad`, `warn`, `faint`.
    dot: &'static str,
    /// `show`, `connect`, `full`, `wake`, `waking`, `retry`, `busy`.
    pick: &'static str,
}

fn bar_computer_list(app: &AppHandle) -> Vec<BarComputer> {
    let s = tray_state(app);
    s.computers
        .iter()
        .map(|c| {
            let (seen, dot) = menu::status(c);
            BarComputer {
                id: c.id.clone(),
                name: c.name.clone(),
                state: seen.code(),
                dot: match dot {
                    menu::Dot::Ok => "ok",
                    menu::Dot::Bad => "bad",
                    menu::Dot::Warn => "warn",
                    menu::Dot::Faint => "faint",
                },
                pick: menu::pick(&s, c).code(),
            }
        })
        .collect()
}

/// La liste du menu « Ordinateurs » à la barre (qui écarte l'ordinateur de sa
/// session), seulement quand elle est affichée.
fn emit_bar_computers(app: &AppHandle) {
    if app.get_webview_window("bar").and_then(|b| b.is_visible().ok()).unwrap_or(false) {
        let _ = app.emit_to("bar", "bar-computers", bar_computer_list(app));
    }
}

/// Le menu « Ordinateurs » de la barre s'ouvre.
#[tauri::command(async)]
fn bar_computers(app: AppHandle) -> Vec<BarComputer> {
    bar_computer_list(&app)
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
/// l'interface, si elle n'écoute pas encore). Une connexion pendant une
/// session (`--connect`, liste de la barre des tâches, notification, menu de
/// l'icône) ouvre une session de plus sans passer par l'accueil : sa connexion
/// se montre dans sa propre fenêtre (plusieurs ordinateurs) ; un ordinateur
/// déjà en session montre sa fenêtre.
fn show_action(app: &AppHandle, action: Action) {
    if let Action::Connect { id } = &action {
        let state = app.state::<UiState>();
        let busy = !state.sessions.lock().unwrap().is_empty() || !state.launching.lock().unwrap().is_empty();
        if busy {
            let (a, id) = (app.clone(), id.clone());
            std::thread::spawn(move || {
                if let Err(e) = connect(a, id) {
                    tray::alert(&e);
                }
            });
            return;
        }
    }
    if !matches!(action, Action::Open) {
        *app.state::<UiState>().action.lock().unwrap() = Some(action);
    }
    let a = app.clone();
    let _ = app.run_on_main_thread(move || {
        if let Some(w) = a.get_webview_window("main") {
            let _ = w.unminimize();
            let _ = w.show();
            let _ = w.set_focus();
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
    if !state.sessions.lock().unwrap().is_empty() {
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

/// Chaque ordinateur, sauf ceux en session : toutes les 8 s quand
/// l'accueil est affiché, toutes les 30 s sans lui (l'app dans l'icône), et
/// tout de suite à l'ouverture du menu de l'icône (`poll_soon`).
fn poll(app: AppHandle) {
    loop {
        let state = app.state::<UiState>();
        let in_session = state.session_ids();
        let computers = state.with_store(|s| s.computers.clone());
        let mut changed = false;
        for c in computers {
            if in_session.contains(&c.id) {
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
        let in_session = state.session_ids();
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
            if in_session.contains(id) {
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
    if state.in_session(&id) {
        return Ok(());
    }
    let c = state.with_store(|s| s.get(&id).cloned()).ok_or_else(|| t("errors.unknown_computer", &[]))?;
    refresh(&app, &c);
    // Toujours : l'accueil sort de « Recherche… » même si rien n'a changé.
    emit_computers(&app);
    Ok(())
}

/// Les écrans, vus par l'accueil et les réglages d'un ordinateur : ce que le
/// cœur en dit (`Screens::choice`, `screens_profile`, `session_plan_for`), le
/// même calcul que la session.
#[derive(Serialize)]
struct DisplaysInfo {
    /// Le réglage « Écrans » se change ; sinon il est inactif, sur « Cet
    /// écran » (définition sur mesure, ou écran virtuel pas demandé ou pas
    /// utilisable sur la Station).
    enabled: bool,
    /// Le réglage tel que les réglages le montrent : `auto`, `this` ou `all`.
    shown: &'static str,
    /// Écrans que la session prendra (1 : un seul).
    count: usize,
    /// L'écran virtuel servira : demandé et utilisable sur la Station ; sinon
    /// la ligne de l'accueil dit « Les écrans de l'ordinateur, tels quels ».
    virtual_display: bool,
}

/// `settings` : ceux de l'ordinateur `id`, enregistrés ou en cours de
/// réglage (la ligne de l'accueil et le réglage se mettent à jour à chaque
/// changement). Les écrans de ce PC sont relevés à chaque appel.
#[tauri::command(async)]
fn displays_info(app: AppHandle, id: String, settings: Settings) -> Result<DisplaysInfo, String> {
    let state = app.state::<UiState>();
    let mut c = state.with_store(|s| s.get(&id).cloned()).ok_or_else(|| t("errors.unknown_computer", &[]))?;
    c.settings = settings;
    let pc = state.this_pc.lock().unwrap().clone().unwrap_or_default();
    let lighten = battery_choice(&state) == "alleger" && pc::on_battery();
    let (stream, _, _) = session_plan(&c, &pc, lighten, None);
    let info = state.live.lock().unwrap().get(&id).and_then(|l| l.info.clone());
    let screens = Layout::now(displays_spec(&c.settings, info.as_ref(), lighten, &pc)).screens();
    let choice = screens.choice(screens::screens_profile(&stream, screens.capabilities), c.settings.virtual_display);
    let count = screens::session_plan_for(&screens, &stream, c.settings.virtual_display).map_or(1, |p| p.len());
    Ok(DisplaysInfo { enabled: choice.setting_enabled(), shown: choice.shown_setting().as_str(), count, virtual_display: choice.virtual_display })
}

/// Les écrans de ce PC ont changé (branché, débranché, disposition) : l'accueil
/// revoit ce que la session prendra (événement `displays`), et la définition
/// de « Cet écran ».
pub fn displays_changed(app: &AppHandle) {
    refresh_this_screen(app);
    let _ = app.emit_to("main", "displays", ());
}

/// Le dernier « Cet écran » vu (nom du périphérique de l'écran de l'accueil).
static HOME_SCREEN: Mutex<Option<String>> = Mutex::new(None);

/// L'accueil a bougé : s'il a changé d'écran, « Cet écran » change, et avec
/// lui la définition que la session demandera.
fn home_moved(app: &AppHandle) {
    let key = system::home_monitor_key();
    {
        let mut last = HOME_SCREEN.lock().unwrap();
        if *last == key {
            return;
        }
        *last = key;
    }
    refresh_this_screen(app);
}

/// La définition et la fréquence de « Ce PC » (ligne de l'accueil, débit des
/// réglages) relues sur « Cet écran », l'écran de l'accueil maintenant : la
/// session les relit au même endroit à la connexion (`system::screen_mode`),
/// l'accueil annonce donc ce qu'elle demandera.
fn refresh_this_screen(app: &AppHandle) {
    let mode = system::screen_mode();
    let state = app.state::<UiState>();
    let pc = {
        let mut cached = state.this_pc.lock().unwrap();
        // Pas encore relevé : la sonde du lancement lira l'écran à sa fin.
        let Some(pc) = cached.as_mut() else { return };
        if (pc.width, pc.height, pc.refresh) == (mode.width, mode.height, mode.refresh) {
            return;
        }
        (pc.width, pc.height, pc.refresh) = (mode.width, mode.height, mode.refresh);
        pc.clone()
    };
    log::info!("cet écran : {} × {} à {} Hz", mode.width, mode.height, mode.refresh);
    let _ = app.emit_to("main", "this-pc", pc);
}

/// Ce que montre le fond de la connexion d'un écran (`fond.html`).
#[tauri::command]
fn fond_state(window: tauri::WebviewWindow, state: tauri::State<'_, UiState>) -> fonds::FondState {
    state.fonds.get().map_or(fonds::FondState { not_created: false }, |f| f.state_of(window.label()))
}

#[tauri::command(async)]
fn snapshot(state: tauri::State<'_, UiState>) -> Snapshot {
    Snapshot {
        computers: state.views(),
        discovered: state.discovery.get().map(|d| d.list()).unwrap_or_default(),
        this_pc: state.this_pc.lock().unwrap().clone(),
        sessions: state.session_ids(),
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
    sent: Arc<Mutex<Sent>>,
    stats: Arc<Mutex<Option<Stats>>>,
    spec: Spec,
    /// « Cet écran » au lancement de la session (`Layout::this_screen`).
    this_screen: Option<String>,
    spot: Arc<Mutex<Spot>>,
    /// La première session (plusieurs ordinateurs) : passage de l'accueil,
    /// fonds des autres écrans. Les suivantes : leur fenêtre de connexion.
    first: bool,
    connexion: Arc<Connexion>,
}

/// `bar-shown` : la barre se montre pour la session `id` (une seule barre,
/// elle sert la session de la fenêtre où elle s'ouvre) ; `pinned` : demandée
/// par « Statistiques » (menu de l'icône), elle ne repart pas d'elle-même
/// tant que le pointeur n'y est pas passé ; `multi` : plusieurs écrans (pas
/// de « Plein écran ») ; `keep_sound` : « Garder le son » de cette session.
#[derive(Clone, Serialize)]
struct BarShown {
    id: String,
    #[serde(flatten)]
    label: BarLabel,
    pinned: bool,
    multi: bool,
    keep_sound: bool,
    /// Sessions ouvertes : « Garder le son » à partir de deux (ensuite,
    /// l'événement `sessions`).
    sessions: usize,
}

/// `bar-label` : le profil de la session `id` a changé.
#[derive(Clone, Serialize)]
struct BarLabelEvent {
    id: String,
    #[serde(flatten)]
    label: BarLabel,
}

/// `stats` : les mesures de la seconde écoulée de la session `id`.
#[derive(Clone, Serialize)]
struct StatsEvent<'a> {
    id: &'a str,
    #[serde(flatten)]
    stats: &'a Stats,
}

/// `session-keep-sound` : « Garder le son » de la session `id`.
#[derive(Clone, Serialize)]
struct KeepSoundEvent {
    id: String,
    on: bool,
}

/// `session-replan` : la connexion reprend en session (écrans de ce PC ou
/// profil changés) ; `screens` : le nombre d'écrans qu'elle prendra.
#[derive(Clone, Serialize)]
struct ReplanEvent {
    id: String,
    screens: usize,
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
        {
            // Ce que montre la fenêtre de connexion d'une session suivante.
            let mut view = self.connexion.view.lock().unwrap();
            view.phase = phase;
            view.restarting = matches!(s, Status::StationRestarting);
        }
        let _ = self.app.emit("session-status", Phase { id: self.id.clone(), phase, detail });
        if matches!(s, Status::StationRestarting) {
            // Dernier recours de la connexion : « La Station redémarre… »
            // jusqu'au prochain `session-status` (voir ui/src/lib/api.ts).
            let _ = self.app.emit("session-station-restarting", StationRestarting { id: self.id.clone() });
        }
        // La connexion reprend en session (écrans de ce PC ou profil changés) :
        // ses étapes reviennent sur l'écran principal, le fond de la connexion
        // sur les autres écrans, jusqu'à la première image de chacun.
        if matches!(s, Status::Preparing) && self.spot.lock().unwrap().shown {
            self.begin_replan();
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
        let _ = self.app.emit_to("bar", "stats", StatsEvent { id: &self.id, stats: s });
        *self.stats.lock().unwrap() = Some(s.clone());
        tray::refresh();
    }

    /// Première image à l'écran : l'accueil laisse la place à la session,
    /// même si Windows a refusé le premier plan à celle-ci (l'accueil,
    /// resté devant, montrait le passage ouvert sur du noir). Une session
    /// suivante (plusieurs ordinateurs) : sa fenêtre de connexion s'ouvre et
    /// part, l'accueil ne bouge pas.
    fn visible(&self, foreground: bool) {
        if !foreground {
            log::info!("session hors du premier plan : {}", if self.first { "accueil caché quand même" } else { "elle attend dans la barre des tâches" });
        }
        self.spot.lock().unwrap().shown = true;
        // Le passage s'ouvre à la première image montrée, pas à la connexion.
        let _ = self.app.emit("session-shown", &self.id);
        if !self.first {
            close_connexion(&self.app, &self.connexion, Duration::from_millis(900));
            return;
        }
        if let Some(w) = self.app.get_webview_window("main") {
            let _ = w.hide();
        }
    }

    /// Les écrans de la session : le fond de la connexion sur ceux qui n'ont
    /// pas encore leur image, la ligne d'un écran que l'ordinateur n'a pas pu
    /// créer, rien pour ceux que la session ne prend pas (une Station qui ne
    /// gère qu'un écran, le principal seul accepté).
    fn screens(&self, set: &ScreenSet) {
        let target = |s: &crate::host::ScreenState| fonds::Target { key: s.key.clone(), x: s.x, y: s.y, width: s.width, height: s.height };
        let mut primary = None;
        let mut others = Vec::new();
        let mut not_created = Vec::new();
        let (mut count, mut pixel_rate) = (0u32, 0.0f64);
        for s in &set.screens {
            match &s.role {
                Role::Stream { session_primary, width, height, refresh } => {
                    count += 1;
                    pixel_rate += f64::from(*width) * f64::from(*height) * f64::from((*refresh).max(1));
                    if *session_primary {
                        primary = Some(target(s));
                        // Mesures de l'écran principal.
                        let mut sent = self.sent.lock().unwrap();
                        sent.mode = kybernow_core::console::Mode { width: *width, height: *height, refresh: *refresh };
                    } else {
                        others.push(target(s));
                    }
                }
                Role::NotCreated => not_created.push(target(s)),
                Role::Unused => {}
            }
        }
        {
            let mut sent = self.sent.lock().unwrap();
            sent.screens = count.max(1);
            sent.pixel_rate = pixel_rate;
        }
        let waiting: Vec<fonds::Target> = {
            let mut spot = self.spot.lock().unwrap();
            spot.primary = primary;
            spot.others = others.clone();
            // Un écran qui n'est plus pris oublie qu'il a montré son image.
            let keys: Vec<String> = others.iter().map(|t| t.key.clone()).chain(spot.primary.iter().map(|t| t.key.clone())).collect();
            spot.visible.retain(|k| keys.contains(k));
            // Sans image encore (connexion, ou reprise), un fond sans texte.
            others.into_iter().filter(|t| !spot.visible.contains(&t.key)).collect()
        };
        let _ = self.app.emit("session-screens", ScreensEvent { id: self.id.clone(), screens: count.max(1) as usize });
        tray::refresh();
        // Les fonds de la connexion : seulement pour la première session (les
        // suivantes prennent un écran, en fenêtre).
        if !self.first {
            return;
        }
        let Some(fonds) = self.app.state::<UiState>().fonds.get().cloned() else { return };
        let keep: Vec<String> = waiting.iter().chain(&not_created).map(|t| t.key.clone()).collect();
        fonds.keep_only(&keep);
        fonds.show(&waiting);
        for t in &not_created {
            fonds.not_created(t);
        }
    }

    /// Un écran montre son image : son fond de connexion s'efface ; pour
    /// l'écran principal après une reprise, les étapes se referment et
    /// l'accueil rend la place à la session.
    fn screen_visible(&self, key: &str) {
        let (replanning, primary) = {
            let mut spot = self.spot.lock().unwrap();
            if !spot.visible.iter().any(|k| k == key) {
                spot.visible.push(key.to_string());
            }
            (spot.replanning.is_some(), spot.primary.as_ref().is_some_and(|p| p.key == key))
        };
        if self.first
            && let Some(fonds) = self.app.state::<UiState>().fonds.get()
        {
            fonds.hide(key);
        }
        if replanning && primary {
            self.end_replan();
        }
    }

    fn top_edge(&self, hwnd: isize) {
        let app = self.app.clone();
        let id = self.id.clone();
        // Sur le fil de Tauri : la fenêtre de la session ne doit pas attendre.
        let _ = self.app.run_on_main_thread(move || show_bar(&app, &id, hwnd, false));
    }

    /// « Afficher », « Se connecter » sur cet ordinateur ou Ctrl+Alt+Maj+O
    /// pendant qu'il se connecte : sa fenêtre de connexion passe devant (pour
    /// la première session, l'accueil et son passage).
    fn connexion_front(&self) {
        // La première session se connecte sur l'accueil (son passage).
        let label = if self.first { Some("main".to_string()) } else { self.connexion.label.lock().unwrap().clone() };
        let Some(label) = label else { return };
        let app = self.app.clone();
        let _ = self.app.run_on_main_thread(move || {
            if let Some(w) = app.get_webview_window(&label) {
                let _ = w.unminimize();
                let _ = w.show();
                let _ = w.set_focus();
            }
        });
    }

    fn ended(&self, s: &Summary) {
        let state = self.app.state::<UiState>();
        let remaining = {
            let mut sessions = state.sessions.lock().unwrap();
            sessions.retain(|r| r.id != self.id);
            sessions.len()
        };
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
                    screens: Some(sent.screens.max(1)),
                    codec: Some(sent.settings.codec.label().into()),
                    mbps: Some(sent.mbps()),
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
        // La barre servait cette session : elle part.
        let served = {
            let mut bar = state.bar_session.lock().unwrap();
            let served = bar.as_deref() == Some(self.id.as_str());
            if served {
                *bar = None;
            }
            served
        };
        if served && let Some(bar) = self.app.get_webview_window("bar") {
            let _ = bar.hide();
        }
        let (before, replan) = {
            let mut spot = self.spot.lock().unwrap();
            let before = spot.before.take();
            let replan = spot.replanning.take();
            *spot = Spot::default();
            (before, replan)
        };
        let quitting = state.quitting.load(std::sync::atomic::Ordering::Relaxed);
        // Le message de l'échec reste dans la fenêtre de connexion (« Fermer »).
        let mut kept_in_window = false;
        if self.first {
            // Plusieurs écrans : les fonds de la connexion partent (un échec se
            // lit sur l'écran principal, seul), l'accueil reprend sa place s'il
            // avait changé d'écran.
            if let Some(fonds) = state.fonds.get() {
                fonds.close_all();
            }
            if let Some(w) = self.app.get_webview_window("main").filter(|_| !quitting) {
                // Reprise de la connexion interrompue : l'accueil quitte l'écran principal.
                if let Some(r) = replan {
                    let _ = w.set_always_on_top(false);
                    if let Some(size) = r.size {
                        let _ = w.set_size(size);
                    }
                    if let Some(at) = r.position {
                        let _ = w.set_position(at);
                    }
                }
                // Revenu où il était, sauf après une erreur : elle se lit sur l'écran principal.
                if let (None, Some(at)) = (&s.error, before) {
                    let _ = w.set_position(at);
                }
            }
        } else {
            // Session suivante : sa fenêtre de connexion part, sauf un échec
            // pendant qu'elle est ouverte (avant la première image, ou quand
            // la connexion a repris en session), qu'elle montre jusqu'à « Fermer ».
            let open = self.connexion.label.lock().unwrap().is_some();
            kept_in_window = open && s.error.is_some() && !quitting;
            if kept_in_window {
                self.connexion.view.lock().unwrap().error = s.error.clone();
            } else {
                close_connexion(&self.app, &self.connexion, Duration::ZERO);
            }
        }
        // La dernière session fermée ramène l'accueil, comme avant, s'il ne
        // reste ni connexion qui se lance ni fenêtre de connexion ouverte
        // (l'accueil ne passe pas devant elles) ; s'il reste des sessions,
        // l'accueil ne leur prend pas le premier plan, et si celle-ci était
        // devant, la session active reprend la place.
        if remaining == 0 {
            show_home_if_idle(&self.app);
        } else if (s.was_front || served) && !quitting && let Some(next) = state.active_id() {
            // Sa fenêtre (ou sa barre : « Se déconnecter ») était devant.
            state.with_session(&next, |r| r.host.focus(0));
        }
        // Une fin non demandée (pas « Se déconnecter », ni la fenêtre fermée)
        // dont la fenêtre disparaît : une notification si personne ne
        // regardait (autre app devant), et toujours quand d'autres sessions
        // continuent, même app au premier plan et même si c'était la session
        // active. Pas quand le message de l'échec reste dans sa fenêtre de
        // connexion avec « Fermer » (docs/link-ui.md, `notice.rs`). Un clic
        // dessus ouvre l'accueil sur cet ordinateur.
        let notify = crate::notice::end_notification(&crate::notice::End {
            requested: s.requested,
            quitting,
            app_in_front: shell::app_in_front(),
            others_continue: remaining > 0,
            started: self.first || s.shown,
            kept_in_window,
        });
        if notify {
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
        let _ = self.app.emit("session-ended", Ended { id: &self.id, summary: s });
        emit_sessions(&self.app);
        emit_computers(&self.app);
        // L'app se fermait : la console est prévenue, on peut partir quand la
        // dernière session est finie et qu'aucune connexion ne se lance.
        if quitting {
            exit_when_idle(&self.app);
        }
    }
}

/// `session-screens` : le nombre d'écrans de la session (l'étape « Préparation
/// des écrans » de la connexion, la barre).
#[derive(Clone, Serialize)]
struct ScreensEvent {
    id: String,
    screens: usize,
}

/// Le nombre d'écrans que la session prendra avec ces réglages et les écrans
/// de ce PC maintenant : le calcul du cœur (`session_plan_for`), comme à
/// l'accueil et dans la session.
fn expected_screens(spec: Spec, this_screen: Option<String>, settings: &kybernow_core::profile::StreamSettings, virtual_display: bool) -> usize {
    let layout = Layout { monitors: displays::enumerate(), spec, this_screen, extra_session: false };
    screens::session_plan_for(&layout.screens(), settings, virtual_display).map_or(1, |p| p.len())
}

impl UiEvents {
    /// La barre servait cette session : elle se referme.
    fn hide_bar_if_mine(&self) {
        let state = self.app.state::<UiState>();
        let mut bar = state.bar_session.lock().unwrap();
        if bar.as_deref() != Some(self.id.as_str()) {
            return;
        }
        *bar = None;
        drop(bar);
        let app = self.app.clone();
        let _ = self.app.run_on_main_thread(move || {
            window::set_bar_session(None);
            if let Some(bar) = app.get_webview_window("bar") {
                let _ = bar.hide();
            }
        });
    }

    /// La connexion reprend en session : l'accueil couvre l'écran principal
    /// avec ses étapes (le passage), les autres écrans reprennent le fond de
    /// la connexion, chacun jusqu'à sa première image. Une session suivante
    /// (plusieurs ordinateurs) : sa fenêtre de connexion revient par-dessus la
    /// sienne.
    fn begin_replan(&self) {
        if !self.first {
            let fresh = {
                let mut spot = self.spot.lock().unwrap();
                let fresh = spot.replanning.is_none();
                spot.replanning = Some(Replan { visible: false, position: None, size: None });
                fresh
            };
            if !fresh {
                return;
            }
            log::info!("la connexion de {} reprend en session, dans sa fenêtre", self.id);
            let _ = self.app.emit("session-replan", ReplanEvent { id: self.id.clone(), screens: 1 });
            self.hide_bar_if_mine();
            let state = self.app.state::<UiState>();
            let hwnd = state.with_session(&self.id, |r| r.host.primary_window()).unwrap_or(0);
            let title = state.with_store(|s| s.get(&self.id).map(|c| c.name.clone())).unwrap_or_default();
            if let Some((at, client)) = window::window_frame(hwnd) {
                open_connexion(&self.app, &self.connexion, at, client, &title);
            }
            return;
        }
        let (primary, others) = {
            let mut spot = self.spot.lock().unwrap();
            if spot.replanning.is_some() {
                return;
            }
            spot.replanning = Some(Replan { visible: false, position: None, size: None });
            spot.visible.clear();
            (spot.primary.clone(), spot.others.clone())
        };
        let screens = {
            let sent = self.sent.lock().unwrap();
            expected_screens(self.spec, self.this_screen.clone(), &sent.settings, sent.virtual_display)
        };
        log::info!("la connexion reprend en session : {screens} écran(s)");
        let _ = self.app.emit("session-replan", ReplanEvent { id: self.id.clone(), screens });
        // La barre se referme.
        self.hide_bar_if_mine();
        let app = self.app.clone();
        let spot = self.spot.clone();
        let _ = self.app.run_on_main_thread(move || {
            let Some(main) = app.get_webview_window("main") else { return };
            let before = Replan { visible: main.is_visible().unwrap_or(false), position: main.outer_position().ok(), size: main.outer_size().ok() };
            if let Some(r) = spot.lock().unwrap().replanning.as_mut() {
                *r = before;
            }
            if let Some(p) = primary {
                let _ = main.set_always_on_top(true);
                let _ = main.set_position(tauri::PhysicalPosition::new(p.x, p.y));
                let _ = main.set_size(tauri::PhysicalSize::new(p.width, p.height));
                let _ = main.show();
            }
            if let Some(fonds) = app.state::<UiState>().fonds.get() {
                fonds.show(&others);
            }
        });
    }

    /// L'écran principal a repris son image : les étapes se referment et
    /// l'accueil rend la place à la session.
    fn end_replan(&self) {
        let Some(r) = self.spot.lock().unwrap().replanning.take() else { return };
        let _ = self.app.emit("session-shown", &self.id);
        if !self.first {
            close_connexion(&self.app, &self.connexion, Duration::from_millis(900));
            return;
        }
        let app = self.app.clone();
        let _ = self.app.run_on_main_thread(move || {
            let Some(main) = app.get_webview_window("main") else { return };
            let _ = main.set_always_on_top(false);
            if let Some(size) = r.size {
                let _ = main.set_size(size);
            }
            if let Some(at) = r.position {
                let _ = main.set_position(at);
            }
            if !r.visible {
                let _ = main.hide();
            }
        });
    }
}

/// Réglages de la session et mode de l'écran pour un ordinateur, d'après ses
/// réglages, ce que ce PC sait décoder et « Sur batterie » des réglages de
/// l'app (`lighten` : alléger, et ce PC est sur batterie). `window` : le mode
/// de la fenêtre d'une session suivante (plusieurs ordinateurs,
/// `extra_session_window` du cœur), qui passe avant la définition des
/// réglages, sur mesure comprise ; sinon, le mode de « Cet écran »
/// (`system::screen_mode`).
fn session_plan(
    c: &Computer,
    pc: &ThisPc,
    lighten: bool,
    window: Option<kybernow_core::console::Mode>,
) -> (kybernow_core::profile::StreamSettings, kybernow_core::console::Mode, String) {
    let s = &c.settings;
    let choices = Choices {
        profile: Profile::parse(&s.profile),
        codec: s.codec.as_deref().and_then(Codec::parse),
        yuv444: s.yuv444,
        mbps: s.mbps,
        reliable: s.reliable,
        max_4k: s.max_4k && s.custom.is_none(),
        size: window.or_else(|| custom_mode(s)),
        lighten,
    };
    let (settings, mode) = plan(&choices, capabilities(pc), window.unwrap_or_else(system::screen_mode));
    let label = format!(
        "{} · {}{}",
        settings.label(),
        settings.codec.label(),
        if settings.yuv444 { " 4:4:4" } else { "" }
    );
    (settings, mode, label)
}

/// La définition sur mesure choisie, si elle est utilisable.
fn custom_mode(s: &Settings) -> Option<kybernow_core::console::Mode> {
    s.custom.filter(|m| m.width >= 640 && m.height >= 360 && m.refresh >= 24)
}

/// Ce qui décide du nombre d'écrans d'une session, d'après les réglages de
/// l'ordinateur, ce que sa Station sait créer (`info` ; sans elle, un écran)
/// et ce que ce PC décode (`pc`, la règle « Personnalisé » du cœur). Comme
/// sur Mac, l'écran virtuel compte comme utilisable tant que la Station n'a
/// pas répondu.
fn displays_spec(s: &Settings, info: Option<&Info>, lighten: bool, pc: &ThisPc) -> Spec {
    Spec {
        setting: DisplaysSetting::parse(&s.displays).unwrap_or_default(),
        limit_4k: s.max_4k && s.custom.is_none(),
        custom_resolution: custom_mode(s).is_some(),
        max_screens: displays::max_screens(info.map_or(1, |i| i.virtual_display.max_screens())),
        lighten,
        virtual_display_available: info.is_none_or(|i| i.virtual_display.available),
        capabilities: Some(capabilities(pc)),
    }
}

/// Ce que ce PC décode (sonde de la carte graphique).
fn capabilities(pc: &ThisPc) -> kybernow_core::profile::Capabilities {
    kybernow_core::profile::Capabilities { hevc: pc.hevc, hevc444: pc.hevc444, av1: pc.av1 }
}

/// Réponse de `connect` : les écrans que la session prend (l'étape « Préparation
/// des écrans » de la connexion) ; `windowed` : une session suivante
/// (plusieurs ordinateurs), qui s'ouvre dans sa fenêtre sans passer par
/// l'accueil, ou un ordinateur déjà en session, dont la fenêtre passe devant.
#[derive(Serialize)]
struct ConnectInfo {
    screens: usize,
    windowed: bool,
}

/// La fenêtre de connexion d'une session suivante (plusieurs ordinateurs) :
/// le cadre `at` (pixels du bureau) et la zone cliente `client` de la fenêtre
/// de la session qui va s'y montrer, titrée du nom de l'ordinateur comme
/// elle. Rouverte quand la connexion reprend en session.
fn open_connexion(app: &AppHandle, conn: &Arc<Connexion>, at: (i32, i32), client: (u32, u32), name: &str) {
    if conn.label.lock().unwrap().is_some() {
        return;
    }
    let label = format!("connexion-{}", CONNEXION_SEQ.fetch_add(1, Ordering::Relaxed) + 1);
    *conn.label.lock().unwrap() = Some(label.clone());
    let id = conn.view.lock().unwrap().id.clone();
    app.state::<UiState>().connexions.lock().unwrap().insert(label.clone(), conn.clone());
    let title = format!("{name} · {}", shell::DISPLAY_NAME);
    let a = app.clone();
    let _ = app.run_on_main_thread(move || {
        let script = format!(
            "window.__KN_LANG = '{}'; window.__KN_SESSION = {};",
            i18n::lang().code(),
            serde_json::to_string(&id).unwrap_or_else(|_| "''".into())
        );
        let built = WebviewWindowBuilder::new(&a, &label, WebviewUrl::App("connexion.html".into()))
            .initialization_script(&script)
            .title(&title)
            .inner_size(960.0, 540.0)
            .theme(Some(tauri::Theme::Dark))
            // Le fond de l'écran de connexion, avant même que la page soit là.
            .background_color(Color(5, 7, 10, 255))
            .visible(false)
            .build();
        let w = match built {
            Ok(w) => w,
            Err(e) => {
                log::warn!("fenêtre de connexion de {id} : {e}");
                return;
            }
        };
        let place = move |w: &tauri::WebviewWindow| {
            let _ = w.set_size(tauri::PhysicalSize::new(client.0.max(320), client.1.max(180)));
            let _ = w.set_position(tauri::PhysicalPosition::new(at.0, at.1));
        };
        place(&w);
        let _ = w.show();
        let _ = w.set_focus();
        log::info!("fenêtre de connexion de {id} : {}×{} en ({}, {})", client.0, client.1, at.0, at.1);
        // Windows règle la taille d'une fenêtre à l'échelle de l'écran où elle
        // arrive : une seconde fois, une fois posée.
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(150));
            place(&w);
        });
    });
}

/// La fenêtre de connexion part (après `delay` : le passage s'ouvre sur la
/// fenêtre de la session, montrée par-dessus).
fn close_connexion(app: &AppHandle, conn: &Arc<Connexion>, delay: Duration) {
    let Some(label) = conn.label.lock().unwrap().take() else { return };
    app.state::<UiState>().connexions.lock().unwrap().remove(&label);
    let a = app.clone();
    std::thread::spawn(move || {
        if !delay.is_zero() {
            std::thread::sleep(delay);
        }
        let b = a.clone();
        let _ = a.run_on_main_thread(move || {
            if let Some(w) = b.get_webview_window(&label) {
                let _ = w.destroy();
            }
        });
    });
}

/// Ce que montre la fenêtre de connexion qui le demande (à son ouverture).
#[tauri::command]
fn connexion_state(window: tauri::WebviewWindow, state: tauri::State<'_, UiState>) -> Option<ConnexionView> {
    state.connexions.lock().unwrap().get(window.label()).map(|c| c.view.lock().unwrap().clone())
}

/// « Fermer » sous un échec, ou la fenêtre de connexion fermée : « Annuler » si
/// la connexion se fait encore, sinon elle part.
#[tauri::command(async)]
fn connexion_close(app: AppHandle, window: tauri::WebviewWindow) {
    connexion_closed(&app, window.label());
}

fn connexion_closed(app: &AppHandle, label: &str) {
    let state = app.state::<UiState>();
    let conn = state.connexions.lock().unwrap().get(label).cloned();
    let Some(conn) = conn else {
        if let Some(w) = app.get_webview_window(label) {
            let _ = w.destroy();
        }
        return;
    };
    let id = conn.view.lock().unwrap().id.clone();
    let cancelled = state.with_session(&id, |r| r.host.command(Command::Disconnect)).is_some();
    if !cancelled {
        close_connexion(app, &conn, Duration::ZERO);
        // L'échec d'une session suivante refermé alors que plus rien ne
        // tourne : l'accueil revient.
        show_home_if_idle(app);
    }
}

/// Une fenêtre de connexion passe au premier plan : sa session devient la
/// session active du registre du cœur.
fn connexion_focused(app: &AppHandle, label: &str) {
    let state = app.state::<UiState>();
    let id = state.connexions.lock().unwrap().get(label).map(|c| c.view.lock().unwrap().id.clone());
    if let Some(id) = id {
        state.with_session(&id, |r| r.host.activate());
    }
}

/// Plus rien ne tourne : ni session, ni connexion qui se lance, ni fenêtre de
/// connexion ouverte (l'échec d'une session suivante).
fn idle(state: &UiState) -> bool {
    state.sessions.lock().unwrap().is_empty() && state.launching.lock().unwrap().is_empty() && state.connexions.lock().unwrap().is_empty()
}

/// La dernière session finie (ou la fenêtre d'un échec refermée) ramène
/// l'accueil, sauf si une connexion se lance ou qu'une fenêtre de connexion
/// est encore ouverte : il ne passe pas devant elles.
fn show_home_if_idle(app: &AppHandle) {
    let state = app.state::<UiState>();
    if state.quitting.load(Ordering::Relaxed) || !idle(&state) {
        return;
    }
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.show();
        let _ = w.set_focus();
    }
}

/// « Quitter » : l'app part quand la dernière session est finie et qu'aucune
/// connexion ne se lance (une fenêtre de connexion restée sur un échec part
/// avec elle).
fn exit_when_idle(app: &AppHandle) {
    let state = app.state::<UiState>();
    let done = state.sessions.lock().unwrap().is_empty() && state.launching.lock().unwrap().is_empty();
    if done {
        log::info!("quitter");
        app.exit(0);
    }
}

/// L'accueil passe sur l'écran principal de ce PC, où la connexion se lit
/// (docs/link-ui.md, « Plusieurs écrans ») ; rend où il était, pour qu'il y
/// revienne à la fin. Rien s'il y est déjà.
fn home_to_primary(app: &AppHandle, monitors: &[displays::Monitor]) -> Option<tauri::PhysicalPosition<i32>> {
    let main = app.get_webview_window("main")?;
    if main.is_minimized().unwrap_or(false) {
        return None;
    }
    let pos = main.outer_position().ok()?;
    let size = main.outer_size().ok()?;
    let on = displays::at(monitors, pos.x + size.width as i32 / 2, pos.y + size.height as i32 / 2)?;
    if monitors[on].primary {
        return None;
    }
    let m = &monitors[displays::primary(monitors)];
    let x = (m.x + (m.width - size.width as i32) / 2).max(m.x);
    let y = (m.y + (m.height - size.height as i32) / 2).max(m.y);
    let _ = main.set_position(tauri::PhysicalPosition::new(x, y));
    Some(pos)
}

/// La fenêtre d'une session suivante (plusieurs ordinateurs) : le mode de
/// l'écran de l'ordinateur, la zone cliente (pixels), le coin haut gauche du
/// cadre et l'écran de ce PC où elle s'ouvre.
struct Follow {
    mode: kybernow_core::console::Mode,
    client: (i32, i32),
    at: (i32, i32),
    monitor: String,
}

/// Une connexion qui se lance : sa place (une des quatre) est rendue en
/// sortant de `connect`, session inscrite ou connexion refusée ; si l'app se
/// fermait pendant ce temps, elle part alors (s'il ne reste rien).
struct Launching<'a> {
    app: &'a AppHandle,
    id: String,
}

impl Drop for Launching<'_> {
    fn drop(&mut self) {
        let state = self.app.state::<UiState>();
        {
            let mut launching = state.launching.lock().unwrap();
            if let Some(i) = launching.iter().position(|x| *x == self.id) {
                launching.remove(i);
            }
        }
        if state.quitting.load(Ordering::Relaxed) {
            exit_when_idle(self.app);
        }
    }
}

/// Un ordinateur déjà en session : sa fenêtre passe devant (« Afficher »).
fn show_session_of(state: &UiState, id: &str) -> bool {
    state.with_session(id, |r| r.host.focus(0)).is_some()
}

/// « Afficher la session » de l'accueil, « Afficher » de la barre et du menu
/// de l'icône (plusieurs ordinateurs).
#[tauri::command(async)]
fn show_session(state: tauri::State<'_, UiState>, id: String) -> Result<(), String> {
    if show_session_of(&state, &id) { Ok(()) } else { Err(t("errors.no_session", &[])) }
}

/// Lance la session vers un ordinateur avec ses réglages. Pendant une autre
/// session (plusieurs ordinateurs, quatre au plus), elle s'ouvre dans une
/// fenêtre ; un ordinateur déjà en session montre la sienne.
#[tauri::command(async)]
fn connect(app: AppHandle, id: String) -> Result<ConnectInfo, String> {
    let state = app.state::<UiState>();
    // L'app se ferme : rien de nouveau ne démarre.
    if state.quitting.load(Ordering::Relaxed) {
        return Ok(ConnectInfo { screens: 0, windowed: true });
    }
    // Une place parmi les quatre, jamais deux connexions vers le même
    // ordinateur, et tous les contrôles avant de rien montrer (fonds des
    // autres écrans, accueil passé sur l'écran principal, fenêtre de
    // connexion) : une connexion refusée ne laisse rien derrière elle.
    let first = {
        let sessions = state.sessions.lock().unwrap();
        let mut launching = state.launching.lock().unwrap();
        if sessions.iter().any(|r| r.id == id) {
            drop(launching);
            drop(sessions);
            show_session_of(&state, &id);
            return Ok(ConnectInfo { screens: 0, windowed: true });
        }
        if launching.contains(&id) {
            return Err(t("errors.session_already_open", &[]));
        }
        // Les sessions de l'app et les connexions qui se lancent, et le
        // registre du cœur (qui refuserait la cinquième de toute façon).
        if sessions.len() + launching.len() >= focus::MAX_SESSIONS || focus::count() >= focus::MAX_SESSIONS {
            return Err(t("errors.four_at_most", &[]));
        }
        let first = sessions.is_empty() && launching.is_empty();
        launching.push(id.clone());
        first
    };
    let _launching = Launching { app: &app, id: id.clone() };
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
    // Les écrans de ce PC et ce que la Station sait créer : le cœur en fait une
    // session à un ou plusieurs écrans (le même calcul que la ligne de
    // l'accueil). « Cet écran » est relevé ici, avant que l'accueil passe sur
    // l'écran principal : la session le garde (comme la définition, ci-dessous).
    let info = state.live.lock().unwrap().get(&id).and_then(|l| l.info.clone());
    let spec = displays_spec(s, info.as_ref(), lighten, &pc);
    // Plusieurs ordinateurs : une session ouverte pendant qu'une autre tourne
    // prend « Cet écran » en fenêtre, quel que soit le réglage « Écrans » —
    // l'écran de la fenêtre active —, dans une fenêtre centrée aux deux tiers
    // de cet écran en 16:9, dont la zone cliente est l'écran de l'ordinateur :
    // les deux règles du cœur (`screens_for_extra_session`, dans
    // `Layout::screens`, et `extra_session_window`).
    let (layout, follow) = if first {
        (Layout::now(spec), None)
    } else {
        let fallback = state.active_id().and_then(|a| state.with_session(&a, |r| r.host.bar_window())).unwrap_or(0);
        let layout = Layout { monitors: displays::enumerate(), spec, this_screen: system::active_monitor_key(fallback), extra_session: true };
        let screens = layout.screens();
        let w = screens::extra_session_window(&screens.list[screens.current()]);
        // Les écrans de ce PC sont donnés au cœur en pixels : cadre et mode aussi.
        let client = (w.mode.width as i32, w.mode.height as i32);
        let at = system::frame_origin((w.x.round() as i32, w.y.round() as i32), client);
        let monitor = layout.monitors.get(screens.current()).map(|m| m.key.clone()).unwrap_or_default();
        (layout, Some(Follow { mode: w.mode, client, at, monitor }))
    };
    let (settings, mode, label) = session_plan(&c, &pc, lighten, follow.as_ref().map(|f| f.mode));
    let plan = screens::session_plan_for(&layout.screens(), &settings, s.virtual_display);
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
    if let Some(p) = &plan {
        log::info!("session sur {} écrans de ce PC (la Station en crée {} au plus)", p.len(), layout.spec.max_screens);
    }
    let bar = Arc::new(Mutex::new(BarLabel { label: label.clone(), profile: s.profile.clone() }));
    let sent = Arc::new(Mutex::new(Sent {
        settings,
        mode,
        screens: plan.as_ref().map_or(1, |p| p.len() as u32),
        pixel_rate: 0.0,
        virtual_display: s.virtual_display,
    }));
    let stats = Arc::new(Mutex::new(None));
    // Plusieurs écrans : la connexion se lit sur l'écran principal (l'accueil
    // y passe), les autres montrent le fond de la connexion sans texte.
    let spot = Arc::new(Mutex::new(Spot::default()));
    if let Some(p) = &plan {
        let target = |i: usize| {
            let m = &layout.monitors[i];
            fonds::Target { key: m.key.clone(), x: m.x, y: m.y, width: m.width.max(0) as u32, height: m.height.max(0) as u32 }
        };
        let others: Vec<fonds::Target> = p.iter().filter(|x| !x.primary).map(|x| target(x.device)).collect();
        {
            let mut sp = spot.lock().unwrap();
            sp.primary = p.iter().find(|x| x.primary).map(|x| target(x.device));
            sp.others = others.clone();
            sp.before = home_to_primary(&app, &layout.monitors);
        }
        if let Some(f) = state.fonds.get() {
            f.show(&others);
        }
    }
    let spec = layout.spec;
    let this_screen = layout.this_screen.clone();
    // Session suivante : sa connexion se montre tout de suite dans une
    // fenêtre, là où la sienne va s'ouvrir.
    let connexion = Arc::new(Connexion::default());
    connexion.view.lock().unwrap().id = id.clone();
    connexion.view.lock().unwrap().phase = "preparing";
    if let Some(f) = &follow {
        log::info!("session suivante : fenêtre de {}×{} sur {} (plusieurs ordinateurs)", f.client.0, f.client.1, f.monitor);
        open_connexion(&app, &connexion, f.at, (f.client.0 as u32, f.client.1 as u32), &c.name);
    }
    // Tenu jusqu'à l'inscription de la session : une fin très rapide (dans
    // `ended`) ne peut pas passer avant.
    let mut running = state.sessions.lock().unwrap();
    let host = SessionHost::start(
        c.name.clone(),
        params,
        HostOptions {
            vsync: s.vsync,
            hardware: true,
            fullscreen: first,
            mouse,
            window_size: match &follow {
                Some(f) => f.client,
                None => system::initial_window_size(mode),
            },
            at: follow.as_ref().map(|f| f.at),
            title_stats: false,
            layout,
        },
        Arc::new(UiEvents {
            app: app.clone(),
            id: id.clone(),
            sent: sent.clone(),
            stats: stats.clone(),
            spec,
            this_screen,
            spot: spot.clone(),
            first,
            connexion: connexion.clone(),
        }),
    );
    let screens = plan.as_ref().map_or(1, |p| p.len());
    running.push(Running { id: id.clone(), host, bar, sent, stats });
    drop(running);
    drop(_launching);
    // « Quitter » pendant le lancement : elle s'arrête tout de suite (Admin
    // prévenue), l'app part à sa fin.
    if state.quitting.load(Ordering::Relaxed) {
        state.with_session(&id, |r| r.host.command(Command::Disconnect));
    }
    emit_sessions(&app);
    Ok(ConnectInfo { screens, windowed: !first })
}

/// Un ordre de la barre (ou du passage, « Annuler ») à la session de cet ordinateur.
#[tauri::command(async)]
fn session_command(state: tauri::State<'_, UiState>, id: String, command: String) {
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
    state.with_session(&id, |r| r.host.command(c));
}

/// « Garder le son » de la barre (bouton du haut-parleur) ou du menu de
/// l'icône, pour la session de cet ordinateur.
#[tauri::command(async)]
fn session_keep_sound(app: AppHandle, id: String, on: bool) {
    set_keep_sound(&app, &id, on);
}

fn set_keep_sound(app: &AppHandle, id: &str, on: bool) {
    if app.state::<UiState>().with_session(id, |r| r.host.set_keep_sound(on)).is_some() {
        let _ = app.emit("session-keep-sound", KeepSoundEvent { id: id.to_string(), on });
        tray::refresh();
    }
}

/// Autre profil depuis la barre : enregistré pour cet ordinateur (comme
/// depuis l'accueil) et appliqué tout de suite à sa session.
#[tauri::command(async)]
fn session_profile(app: AppHandle, id: String, profile: String) -> Result<(), String> {
    let state = app.state::<UiState>();
    let Some((bar, sent)) = state.with_session(&id, |r| (r.bar.clone(), r.sent.clone())) else {
        return Err(t("errors.no_session", &[]));
    };
    Profile::parse(&profile).ok_or_else(|| t("errors.unknown_profile", &[]))?;
    let c = save_profile(&state, &id, &profile)?;
    emit_computers(&app);
    let pc = state.this_pc.lock().unwrap().clone().unwrap_or_else(pc::this_pc);
    let lighten = battery_choice(&state) == "alleger" && pc::on_battery();
    // Les réglages du profil seulement : la taille de l'écran ne change pas.
    let (settings, _, label) = session_plan(&c, &pc, lighten, None);
    sent.lock().unwrap().settings = settings.clone();
    let shown = BarLabel { label, profile };
    *bar.lock().unwrap() = shown.clone();
    let _ = app.emit_to("bar", "bar-label", BarLabelEvent { id: id.clone(), label: shown });
    let applied = state.with_session(&id, |r| r.host.set_settings(settings)).unwrap_or(false);
    if applied { Ok(()) } else { Err(t("errors.session_not_ready", &[])) }
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
    if state.in_session(&id) {
        return session_profile(app.clone(), id, profile);
    }
    save_profile(&state, &id, &profile)?;
    emit_computers(&app);
    Ok(())
}

/// Largeur de la fenêtre de la barre de session, en points (la barre
/// elle-même, centrée dedans, est plus étroite : `bar_width`).
const BAR_WINDOW_WIDTH: f64 = 1240.0;

/// La barre de session au bord haut de la zone cliente de la session (en
/// plein écran, le haut de l'écran), centrée sur elle : là où le trait la
/// signale et où le pointeur l'ouvre. À sa taille sans menu. Une seule barre :
/// elle sert la session de l'ordinateur `id`, celle de la fenêtre `hwnd`.
fn show_bar(app: &AppHandle, id: &str, hwnd: isize, pinned: bool) {
    let state = app.state::<UiState>();
    let Some((label, multi, keep_sound, session)) =
        state.with_session(id, |r| (r.bar.lock().unwrap().clone(), r.host.stream_count() > 1, r.host.keep_sound(), r.host.id()))
    else {
        return;
    };
    *state.bar_session.lock().unwrap() = Some(id.to_string());
    let mut mi = MONITORINFO { cbSize: std::mem::size_of::<MONITORINFO>() as u32, ..Default::default() };
    unsafe {
        let m = MonitorFromWindow(HWND(hwnd as *mut _), MONITOR_DEFAULTTONEAREST);
        let _ = GetMonitorInfoW(m, &mut mi);
    }
    let r = mi.rcMonitor;
    // Toujours à sa taille sans menu en se montrant.
    bar_menu(app.clone(), false, None);
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
    window::set_bar_session(Some(session));
    let sessions = state.sessions.lock().unwrap().len();
    let _ = app.emit_to("bar", "bar-shown", BarShown { id: id.to_string(), label, pinned, multi, keep_sound, sessions });
    let _ = app.emit_to("bar", "bar-computers", bar_computer_list(app));
}

/// « Statistiques » du menu de l'icône : la barre et ses mesures en direct
/// pour la session de cet ordinateur (la session active), ou cachée si elle
/// est déjà là ; le clavier reste à la session.
fn toggle_stats(app: &AppHandle, id: &str) {
    let state = app.state::<UiState>();
    // Sur la fenêtre de cette session où est le pointeur (une seule barre).
    let Some(hwnd) = state.with_session(id, |r| r.host.bar_window()) else {
        return;
    };
    let (a, id) = (app.clone(), id.to_string());
    let _ = app.run_on_main_thread(move || {
        let shown = a.get_webview_window("bar").and_then(|b| b.is_visible().ok()).unwrap_or(false);
        let same = a.state::<UiState>().bar_session.lock().unwrap().as_deref() == Some(id.as_str());
        if shown && same {
            bar_hide(a.clone(), Some(id), None);
            return;
        }
        show_bar(&a, &id, hwnd, true);
        // La fenêtre où la barre s'ouvre devient la fenêtre active.
        a.state::<UiState>().with_session(&id, |r| r.host.focus(hwnd));
    });
}

/// Un menu de la barre s'ouvre : la fenêtre grandit vers le bas le temps du
/// menu (transparente, elle ne doit pas cacher la session le reste du temps),
/// de `height` points quand le menu le dit (« Ordinateurs », selon leur nombre).
#[tauri::command(async)]
fn bar_menu(app: AppHandle, open: bool, height: Option<f64>) {
    if let Some(bar) = app.get_webview_window("bar") {
        let scale = bar.scale_factor().unwrap_or(1.0);
        let h = if open { height.filter(|h| h.is_finite()).unwrap_or(300.0).clamp(300.0, 900.0) } else { 76.0 };
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

/// La barre se cache : le clavier revient à la session qu'elle servait
/// (`id`, sinon celle qu'elle sert pour l'app), sauf `focus: false` (une
/// session de plus ouverte depuis la barre : sa fenêtre de connexion le garde).
#[tauri::command(async)]
fn bar_hide(app: AppHandle, id: Option<String>, focus: Option<bool>) {
    if let Some(bar) = app.get_webview_window("bar") {
        let _ = bar.hide();
    }
    window::set_bar_session(None);
    // Un menu ouvert laissait la fenêtre agrandie pour la fois suivante.
    bar_menu(app.clone(), false, None);
    let state = app.state::<UiState>();
    let served = state.bar_session.lock().unwrap().take();
    if focus == Some(false) {
        return;
    }
    if let Some(id) = id.or(served) {
        state.with_session(&id, |r| r.host.command(Command::Focus));
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
    let sessions: Vec<menu::Session> = state
        .sessions
        .lock()
        .unwrap()
        .iter()
        .map(|r| {
            let stats = r.stats.lock().unwrap().clone();
            let sent = r.sent.lock().unwrap();
            menu::Session {
                id: r.id.clone(),
                name: computers.iter().find(|c| c.id == r.id).map(|c| c.name.clone()).unwrap_or_default(),
                latency_ms: stats.as_ref().and_then(|s| s.latency_ms),
                fps: stats.as_ref().map(|s| s.fps),
                mbps: Some(sent.mbps()),
                fullscreen: r.host.fullscreen_now(),
                multi: r.host.stream_count() > 1,
                mouse: computers.iter().find(|c| c.id == r.id).map(|c| c.settings.mouse.clone()).unwrap_or_default(),
                keep_sound: r.host.keep_sound(),
            }
        })
        .collect();
    let active = state.active_id();
    let full = state.full();
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
    menu::State { sessions, active, full, computers, app_update: state.update.lock().unwrap().as_ref().map(|u| u.version.clone()) }
}

/// Ce qui est choisi dans le menu de l'icône, sur son propre fil (les appels
/// à Admin et les questions ne bloquent pas l'icône). Les mêmes commandes que
/// l'accueil, les réglages et la barre.
pub fn tray_command(app: &AppHandle, cmd: Cmd) {
    let app = app.clone();
    std::thread::spawn(move || {
        let state = app.state::<UiState>();
        // Les commandes d'une session portent son ordinateur.
        let session = |id: &str, c: Command| {
            state.with_session(id, |r| r.host.command(c));
        };
        let fail = |r: Result<(), String>| {
            if let Err(e) = r {
                tray::alert(&e);
            }
        };
        match cmd {
            // Le clavier revient ensuite à la session (le menu l'avait pris).
            Cmd::Fullscreen(id) => {
                session(&id, Command::Fullscreen);
                session(&id, Command::Focus);
            }
            Cmd::Mouse(id, mode) => {
                // Gardé pour l'ordinateur de la session, comme dans ses réglages.
                if state.in_session(&id) {
                    state.with_store(|s| {
                        if let Some(c) = s.get_mut(&id) {
                            c.settings.mouse = mode.into();
                            s.save();
                        }
                    });
                    emit_computers(&app);
                }
                session(
                    &id,
                    match mode {
                        "jeu" => Command::MouseGame,
                        "bureau" => Command::MouseDesktop,
                        _ => Command::MouseAuto,
                    },
                );
                session(&id, Command::Focus);
            }
            Cmd::Statistics(id) => toggle_stats(&app, &id),
            Cmd::Disconnect(id) => session(&id, Command::Disconnect),
            Cmd::Show(id) => {
                show_session_of(&state, &id);
            }
            Cmd::KeepSound(id) => {
                let on = state.with_session(&id, |r| r.host.keep_sound()).unwrap_or(false);
                set_keep_sound(&app, &id, !on);
            }
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

/// « Quitter KyberNow Link » : les sessions en cours se terminent d'abord
/// (Admin de chaque ordinateur prévenue, écrans rendus), puis l'app se ferme
/// à la fin de la dernière.
pub fn quit(app: &AppHandle) {
    let state = app.state::<UiState>();
    // D'abord : une connexion qui se lance voit qu'il faut s'arrêter.
    state.quitting.store(true, Ordering::Relaxed);
    {
        let sessions = state.sessions.lock().unwrap();
        if !sessions.is_empty() {
            log::info!("quitter : fin des {} session(s) d'abord", sessions.len());
        }
        for r in sessions.iter() {
            r.host.command(Command::Disconnect);
        }
    }
    // Rien en cours : tout de suite. Sinon à la fin de la dernière session,
    // ou de la connexion qui se lance (`ended`, `Launching`).
    exit_when_idle(app);
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
            show_session,
            session_command,
            session_keep_sound,
            bar_hide,
            bar_menu,
            bar_width,
            bar_computers,
            connexion_state,
            connexion_close,
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
            set_open_at_login,
            displays_info,
            fond_state
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
            // « Cet écran » : celui de la fenêtre de l'accueil (docs/multi-ecrans.md).
            if let Some(home) = app.get_webview_window("main")
                && let Ok(hwnd) = home.hwnd()
            {
                system::set_home_window(hwnd.0 as isize);
            }
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
            let _ = handle.state::<UiState>().fonds.set(Fonds::new(handle.clone()));
            // La session active ou « Garder le son » change (plusieurs
            // ordinateurs) : le menu de l'icône suit (son haut est pour la
            // session active).
            focus::on_change(tray::refresh);
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
            // « Cet écran » suit l'accueil d'un écran à l'autre.
            if matches!(event, tauri::WindowEvent::Moved(_) | tauri::WindowEvent::ScaleFactorChanged { .. }) {
                if window.label() == "main" {
                    home_moved(window.app_handle());
                }
                return;
            }
            // La fenêtre de connexion d'une session suivante passe devant :
            // sa session devient la session active (plusieurs ordinateurs).
            if let tauri::WindowEvent::Focused(true) = event {
                if window.label().starts_with("connexion-") {
                    connexion_focused(window.app_handle(), window.label());
                }
                return;
            }
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
                // Fenêtre de connexion d'une session suivante (plusieurs
                // ordinateurs) : la fermer, c'est « Annuler » (ou « Fermer » sous
                // un échec).
                label if label.starts_with("connexion-") => {
                    api.prevent_close();
                    connexion_closed(app, label);
                }
                // La barre ne se ferme pas (Alt+F4) : elle se cache, le
                // clavier revient à la session qu'elle servait.
                _ => {
                    api.prevent_close();
                    bar_hide(app.clone(), None, None);
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
