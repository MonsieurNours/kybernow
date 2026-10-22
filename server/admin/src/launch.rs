//! App ouverte sur l'ordinateur au début de la session d'un appareil :
//! Steam Big Picture pour l'Apple TV, Playnite, Excel pour un bureau à
//! distance… Réglée dans Admin (Accès › Appareils), gardée sur l'appareil
//! (`Device.launch`, `devices.json`).
//!
//! **Catalogue.** Pour ne jamais taper de chemin, Admin propose tout ce que
//! montre le menu Démarrer de l'utilisateur (`shell:AppsFolder` : apps
//! classiques par leurs raccourcis, apps du Store, jeux ajoutés au menu), avec
//! leur icône, plus des suggestions détectées (Steam Big Picture, Playnite,
//! Xbox, GOG Galaxy, Epic, EA app, Ubisoft Connect, Battle.net) et un
//! explorateur limité à quelques dossiers (Programmes, Bureau…). Le menu
//! Démarrer et les apps du Store sont propres au compte connecté : un agent
//! (`kyber-admin display-agent apps`) les lit **avec le compte de
//! l'utilisateur** dans sa session, écrit les icônes dans son profil, et
//! Admin recopie le tout dans son dossier de données (`apps/`).
//!
//! **Lancement.** Une fois l'écran virtuel en place (l'app s'ouvre dessus, à
//! la bonne définition), `display-agent launch` tourne avec le compte de
//! l'utilisateur : `ShellExecute` d'une adresse (`steam://open/bigpicture`),
//! d'un raccourci du menu Démarrer (`shell:AppsFolder\…`) ou d'un exécutable.
//! Une app déjà ouverte est ramenée au premier plan. Session Windows
//! verrouillée : on attend le déverrouillage (2 min au plus). À la fin de la
//! session, l'app est fermée si l'appareil le demande (adresse de fermeture
//! de la suggestion, sinon `WM_CLOSE` à ses fenêtres) ; si un autre appareil
//! prend la main avec la même app, elle reste ouverte.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::activity::Activity;
use crate::display::now_secs;
use crate::i18n::Msg;

const DIR: &str = "apps";
const CATALOGUE_FILE: &str = "catalogue.json";
const ICONS_DIR: &str = "icons";
/// Temps laissé à la session Windows pour être déverrouillée avant de renoncer.
pub const UNLOCK_WAIT: Duration = Duration::from_secs(120);
/// Reprise de main : l'app du précédent est gardée pour le suivant pendant ce délai.
const KEEP_WINDOW: Duration = Duration::from_secs(60);
/// Temps laissé aux agents (catalogue, lancement, fermeture).
#[cfg_attr(not(windows), allow(dead_code))]
const AGENT_TIMEOUT_MS: u32 = 30_000;
/// Tour de veille : une app encore ouverte pour un appareil qui n'est plus en session.
const TICK: Duration = Duration::from_secs(2);

/// Ce qu'on ouvre : une entrée du menu Démarrer (identifiant d'application
/// Windows, AUMID), une adresse (`steam://…`) ou un exécutable (ou raccourci).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LaunchKind {
    App,
    Uri,
    Path,
}

/// Le réglage d'un appareil.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Launch {
    pub kind: LaunchKind,
    pub target: String,
    /// Nom affiché (celui du catalogue).
    pub name: String,
    /// Fichier d'icône dans `apps/icons` (`<16 hex>.png`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    /// Exécutable, pour reconnaître l'app déjà ouverte et la fermer.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exe: Option<String>,
    /// Adresse qui ferme l'app (`steam://close/bigpicture`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub close: Option<String>,
    /// Fermer l'app à la fin de la session.
    #[serde(default)]
    pub close_at_end: bool,
    /// Ne pas l'ouvrir quand l'appareil se connecte en profil Bureau.
    #[serde(default)]
    pub skip_desktop: bool,
}

impl Launch {
    /// L'app sait-elle se fermer proprement (adresse de fermeture, ou
    /// exécutable connu dont on ferme les fenêtres) ?
    pub fn can_close(&self) -> bool {
        self.close.is_some() || self.exe.is_some() || self.kind != LaunchKind::Uri
    }
}

/// Une entrée du menu Démarrer.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AppEntry {
    /// Identifiant d'application Windows (AUMID).
    pub id: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exe: Option<String>,
}

/// Un lanceur connu, détecté sur l'ordinateur.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Suggestion {
    pub id: String,
    pub name: String,
    pub kind: LaunchKind,
    pub target: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub close: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exe: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
}

/// Un dossier où l'explorateur « Autre… » peut aller.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Root {
    /// `programs`, `programs86`, `desktop`, `userPrograms` (libellé dans l'interface).
    pub id: String,
    pub path: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Catalogue {
    #[serde(default)]
    pub refreshed_at: u64,
    /// Compte Windows dont c'est le menu Démarrer.
    #[serde(default)]
    pub user: Option<String>,
    #[serde(default)]
    pub apps: Vec<AppEntry>,
    #[serde(default)]
    pub suggestions: Vec<Suggestion>,
    #[serde(default)]
    pub roots: Vec<Root>,
    /// Pourquoi la dernière lecture a échoué (personne de connecté…).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Ce que l'agent `apps` renvoie (écrit avec le compte de l'utilisateur).
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct AgentCatalogue {
    pub user: Option<String>,
    pub apps: Vec<AppEntry>,
    pub suggestions: Vec<Suggestion>,
    pub roots: Vec<Root>,
    /// Dossier où l'agent a écrit les icônes (dans le profil de l'utilisateur).
    pub icons_dir: String,
}

/// L'app ouverte en ce moment par Admin, et pour qui.
#[derive(Debug, Clone)]
struct Running {
    device_id: String,
    device_name: String,
    launch: Launch,
    pid: Option<u32>,
}

#[derive(Default)]
struct State {
    running: Option<Running>,
    /// Reprise de main : l'app visée par l'appareil qui arrive, avec son nom.
    keep: Option<(String, String, Instant)>,
}

type EventSink = Arc<dyn Fn(&str, Msg) + Send + Sync>;

pub struct Launcher {
    dir: PathBuf,
    state: Mutex<State>,
    catalogue: Mutex<Option<Catalogue>>,
    /// Une lecture du catalogue à la fois.
    refresh_lock: Mutex<()>,
    events: Mutex<Option<EventSink>>,
}

impl Launcher {
    pub fn new(data_dir: &Path) -> Arc<Self> {
        Arc::new(Self {
            dir: data_dir.join(DIR),
            state: Mutex::new(State::default()),
            catalogue: Mutex::new(None),
            refresh_lock: Mutex::new(()),
            events: Mutex::new(None),
        })
    }

    /// Fil des événements de la vue d'ensemble.
    pub fn on_event(&self, f: EventSink) {
        *self.events.lock().unwrap() = Some(f);
    }

    fn event(&self, kind: &str, msg: Msg) {
        if let Some(f) = self.events.lock().unwrap().as_ref() {
            f(kind, msg);
        } else {
            tracing::info!("{msg}");
        }
    }

    // --- Catalogue -----------------------------------------------------------

    fn icons_dir(&self) -> PathBuf {
        self.dir.join(ICONS_DIR)
    }

    /// Le catalogue tel qu'il a été lu la dernière fois (vide avant la première lecture).
    pub fn catalogue(&self) -> Catalogue {
        let mut cached = self.catalogue.lock().unwrap();
        if let Some(c) = cached.as_ref() {
            return c.clone();
        }
        let c: Catalogue = std::fs::read(self.dir.join(CATALOGUE_FILE))
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default();
        *cached = Some(c.clone());
        c
    }

    /// Relit le menu Démarrer et les suggestions (bloquant : l'agent tourne
    /// dans la session de l'utilisateur), et garde le résultat. Sans
    /// utilisateur connecté, le catalogue précédent reste, avec l'erreur.
    pub fn refresh(&self) -> Catalogue {
        let _one = self.refresh_lock.lock().unwrap();
        let mut previous = self.catalogue();
        match platform::read_catalogue() {
            Ok(agent) => {
                let icons = self.icons_dir();
                let _ = std::fs::create_dir_all(&icons);
                let mut kept = Vec::new();
                for icon in agent.apps.iter().filter_map(|a| a.icon.as_deref()).chain(agent.suggestions.iter().filter_map(|s| s.icon.as_deref())) {
                    if !icon_name_ok(icon) {
                        continue;
                    }
                    let from = Path::new(&agent.icons_dir).join(icon);
                    if from != icons.join(icon) && std::fs::copy(&from, icons.join(icon)).is_err() {
                        continue;
                    }
                    kept.push(icon.to_string());
                }
                // Les icônes des apps disparues partent.
                if let Ok(entries) = std::fs::read_dir(&icons) {
                    for e in entries.flatten() {
                        let name = e.file_name().to_string_lossy().into_owned();
                        if !kept.iter().any(|k| *k == name) {
                            let _ = std::fs::remove_file(e.path());
                        }
                    }
                }
                let catalogue = Catalogue {
                    refreshed_at: now_secs(),
                    user: agent.user,
                    apps: agent
                        .apps
                        .into_iter()
                        .map(|mut a| {
                            a.icon = a.icon.filter(|i| kept.contains(i));
                            a
                        })
                        .collect(),
                    suggestions: agent
                        .suggestions
                        .into_iter()
                        .map(|mut s| {
                            s.icon = s.icon.filter(|i| kept.contains(i));
                            s
                        })
                        .collect(),
                    roots: agent.roots,
                    error: None,
                };
                let _ = std::fs::create_dir_all(&self.dir);
                if let Ok(bytes) = serde_json::to_vec_pretty(&catalogue) {
                    let _ = std::fs::write(self.dir.join(CATALOGUE_FILE), bytes);
                }
                tracing::info!(
                    "catalogue des apps relu : {} apps, {} suggestions ({})",
                    catalogue.apps.len(),
                    catalogue.suggestions.len(),
                    catalogue.user.as_deref().unwrap_or("?")
                );
                *self.catalogue.lock().unwrap() = Some(catalogue.clone());
                catalogue
            }
            Err(e) => {
                tracing::warn!("catalogue des apps : {e:#}");
                previous.error = Some(format!("{e:#}"));
                *self.catalogue.lock().unwrap() = Some(previous.clone());
                previous
            }
        }
    }

    /// Chemin d'une icône du catalogue, si le nom est sain et le fichier là.
    pub fn icon_path(&self, name: &str) -> Option<PathBuf> {
        if !icon_name_ok(name) {
            return None;
        }
        let path = self.icons_dir().join(name);
        path.is_file().then_some(path)
    }

    // --- Explorateur « Autre… » ----------------------------------------------

    /// Les dossiers autorisés (sans `path`), ou le contenu d'un dossier :
    /// sous-dossiers, exécutables et raccourcis.
    pub fn browse(&self, path: Option<&str>) -> Result<Value> {
        let roots = self.catalogue().roots;
        let Some(path) = path.filter(|p| !p.trim().is_empty()) else {
            return Ok(json!({ "roots": roots }));
        };
        let (dir, root) = within_roots(path, &roots)?;
        let mut folders = Vec::new();
        let mut files = Vec::new();
        for entry in std::fs::read_dir(&dir).with_context(|| format!("lecture de {}", dir.display()))?.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            if name.starts_with('.') || name.starts_with('$') {
                continue;
            }
            let Ok(kind) = entry.file_type() else { continue };
            let full = entry.path().display().to_string();
            if kind.is_dir() {
                folders.push(json!({ "name": name, "path": full }));
            } else if is_launchable(&name) {
                files.push(json!({ "name": name, "path": full }));
            }
        }
        let by_name = |a: &Value, b: &Value| {
            a["name"].as_str().unwrap_or("").to_lowercase().cmp(&b["name"].as_str().unwrap_or("").to_lowercase())
        };
        folders.sort_by(by_name);
        files.sort_by(by_name);
        let parent = if same_path(&dir, Path::new(&root.path)) {
            None
        } else {
            dir.parent().map(|p| p.display().to_string())
        };
        Ok(json!({ "path": dir.display().to_string(), "root": root.id, "parent": parent, "folders": folders, "files": files }))
    }

    /// Vérifie et normalise un réglage venu de l'interface.
    pub fn validate(&self, mut launch: Launch) -> Result<Launch> {
        launch.name = launch.name.trim().chars().take(80).collect();
        launch.target = launch.target.trim().to_string();
        if launch.name.is_empty() || launch.target.is_empty() || launch.target.len() > 400 {
            bail!("réglage d'app invalide");
        }
        launch.icon = launch.icon.filter(|i| icon_name_ok(i));
        let catalogue = self.catalogue();
        match launch.kind {
            LaunchKind::App => {
                if let Some(app) = catalogue.apps.iter().find(|a| a.id == launch.target) {
                    launch.exe = app.exe.clone();
                    if launch.icon.is_none() {
                        launch.icon = app.icon.clone();
                    }
                }
                launch.close = None;
            }
            LaunchKind::Uri => {
                // Seules les adresses des suggestions : pas d'adresse libre.
                let Some(s) = catalogue.suggestions.iter().find(|s| s.kind == LaunchKind::Uri && s.target == launch.target) else {
                    bail!("adresse inconnue : {}", launch.target);
                };
                launch.close = s.close.clone();
                launch.exe = s.exe.clone();
                if launch.icon.is_none() {
                    launch.icon = s.icon.clone();
                }
            }
            LaunchKind::Path => {
                let suggested = catalogue.suggestions.iter().find(|s| s.kind == LaunchKind::Path && s.target == launch.target);
                if suggested.is_none() {
                    let (file, _) = within_roots(&launch.target, &catalogue.roots)?;
                    let file_name = file.file_name().map(|f| f.to_string_lossy().into_owned()).unwrap_or_default();
                    if !file.is_file() || !is_launchable(&file_name) {
                        bail!("fichier introuvable ou non exécutable : {}", launch.target);
                    }
                    launch.target = file.display().to_string();
                }
                launch.close = suggested.and_then(|s| s.close.clone());
                launch.exe = suggested.and_then(|s| s.exe.clone()).or_else(|| {
                    launch.target.to_lowercase().ends_with(".exe").then(|| launch.target.clone())
                });
                if launch.icon.is_none() {
                    launch.icon = suggested.and_then(|s| s.icon.clone());
                }
            }
        }
        if !launch.can_close() {
            launch.close_at_end = false;
        }
        Ok(launch)
    }

    // --- Sessions ------------------------------------------------------------

    /// Un appareil ouvre une session : son app est lancée en fond (après le
    /// déverrouillage de Windows s'il le faut).
    pub fn session_started(self: &Arc<Self>, device_id: String, device_name: String, launch: Launch, profile: Option<String>) {
        if launch.skip_desktop && profile.as_deref() == Some("Bureau") {
            tracing::info!("{device_name} en profil Bureau : {} n'est pas ouvert", launch.name);
            return;
        }
        let me = self.clone();
        tokio::task::spawn_blocking(move || me.start_blocking(&device_id, &device_name, launch));
    }

    fn start_blocking(&self, device_id: &str, device_name: &str, launch: Launch) {
        // Une autre app ouverte par Admin (autre appareil, autre réglage) : fermée d'abord.
        let previous = {
            let mut s = self.state.lock().unwrap();
            s.keep = None;
            match s.running.as_ref() {
                Some(r) if r.launch.target != launch.target => s.running.take(),
                _ => None,
            }
        };
        if let Some(r) = previous {
            self.close_blocking(&r);
        }

        let started = Instant::now();
        let mut waited = false;
        while platform::session_locked() {
            if started.elapsed() >= UNLOCK_WAIT {
                self.event("warn", Msg::new("events.launchLocked").with("app", launch.name.as_str()).with("name", device_name));
                return;
            }
            waited = true;
            std::thread::sleep(Duration::from_secs(1));
        }

        match platform::launch(&launch) {
            Ok(v) => {
                let front = v["front"].as_bool().unwrap_or(false);
                let pid = v["pid"].as_u64().map(|p| p as u32);
                let mut s = self.state.lock().unwrap();
                let pid = pid.or(s.running.as_ref().and_then(|r| r.pid));
                s.running = Some(Running { device_id: device_id.to_string(), device_name: device_name.to_string(), launch: launch.clone(), pid });
                drop(s);
                let key = if front {
                    "events.launchFront"
                } else if waited {
                    "events.launchedAfterUnlock"
                } else {
                    "events.launched"
                };
                self.event("info", Msg::new(key).with("app", launch.name.as_str()).with("name", device_name));
            }
            Err(e) => {
                self.event("warn", Msg::new("events.launchFailed").with("app", launch.name.as_str()).with("name", device_name).with("error", format!("{e:#}")));
            }
        }
    }

    /// Un appareil prend la main : si l'arrivant veut la même app que celui
    /// qui part, elle reste ouverte au lieu d'être fermée puis rouverte.
    pub fn expect_takeover(&self, next_name: &str, next: Option<&Launch>) {
        let mut s = self.state.lock().unwrap();
        s.keep = next.map(|l| (l.target.clone(), next_name.to_string(), Instant::now()));
    }

    /// Fin de la session d'un appareil : son app est fermée si elle le demande.
    pub fn session_ended(&self, device_id: &str) {
        let (running, keep) = {
            let mut s = self.state.lock().unwrap();
            let Some(r) = s.running.as_ref().filter(|r| r.device_id == device_id).cloned() else { return };
            let keep = s.keep.as_ref().filter(|(target, _, since)| *target == r.launch.target && since.elapsed() < KEEP_WINDOW).map(|(_, next, _)| next.clone());
            if keep.is_none() {
                s.running = None;
            }
            (r, keep)
        };
        match keep {
            Some(next) => self.event("info", Msg::new("events.launchKept").with("app", running.launch.name.as_str()).with("next", next)),
            None => self.close_blocking(&running),
        }
    }

    fn close_blocking(&self, running: &Running) {
        if !running.launch.close_at_end {
            return;
        }
        let app = running.launch.name.as_str();
        match platform::close(&running.launch, running.pid) {
            Ok(_) => self.event("info", Msg::new("events.launchClosed").with("app", app).with("name", running.device_name.as_str())),
            Err(e) => self.event("warn", Msg::new("events.launchCloseFailed").with("app", app).with("error", format!("{e:#}"))),
        }
    }

    /// Un tour de veille : l'app encore ouverte pour un appareil qui n'est
    /// plus en session est fermée (fin sans prévenir, reprise de main sans
    /// la même app).
    fn tick(&self, activity: &Activity) {
        let owner = { self.state.lock().unwrap().running.as_ref().map(|r| r.device_id.clone()) };
        let Some(owner) = owner else { return };
        if activity.live_device_id().as_deref() != Some(owner.as_str()) {
            self.session_ended(&owner);
        }
    }

    pub fn supervise(self: Arc<Self>, activity: Arc<Activity>) {
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(TICK);
            loop {
                interval.tick().await;
                let (me, a) = (self.clone(), activity.clone());
                let _ = tokio::task::spawn_blocking(move || me.tick(&a)).await;
            }
        });
    }

    /// Arrêt d'Admin : l'app ouverte est fermée si l'appareil le voulait.
    pub fn shutdown(&self) {
        let running = self.state.lock().unwrap().running.take();
        if let Some(r) = running {
            self.close_blocking(&r);
        }
    }
}

/// `<16 hex>.png`, rien d'autre : le nom vient de l'interface.
fn icon_name_ok(name: &str) -> bool {
    name.len() == 20 && name.ends_with(".png") && name[..16].bytes().all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
}

/// Nom d'icône d'une entrée (les 16 premiers caractères du SHA-256 de son identifiant).
#[cfg_attr(not(windows), allow(dead_code))]
pub fn icon_name(id: &str) -> String {
    use sha2::{Digest, Sha256};
    let digest = hex::encode(Sha256::digest(id.as_bytes()));
    format!("{}.png", &digest[..16])
}

fn is_launchable(name: &str) -> bool {
    let lower = name.to_lowercase();
    lower.ends_with(".exe") || lower.ends_with(".lnk")
}

fn same_path(a: &Path, b: &Path) -> bool {
    let norm = |p: &Path| p.display().to_string().replace('/', "\\").trim_end_matches('\\').to_lowercase();
    if cfg!(windows) { norm(a) == norm(b) } else { a == b }
}

/// Le chemin, s'il est dans un des dossiers autorisés (sans remonter avec `..`).
fn within_roots(path: &str, roots: &[Root]) -> Result<(PathBuf, Root)> {
    let path = path.trim();
    if path.is_empty() || path.contains('\0') {
        bail!("chemin vide");
    }
    let candidate = PathBuf::from(path);
    if candidate.components().any(|c| matches!(c, std::path::Component::ParentDir)) {
        bail!("chemin refusé : {path}");
    }
    let norm = |s: &str| {
        let s = if cfg!(windows) { s.replace('/', "\\") } else { s.to_string() };
        let s = s.trim_end_matches(['\\', '/']).to_string();
        if cfg!(windows) { s.to_lowercase() } else { s }
    };
    let wanted = norm(path);
    for root in roots {
        let r = norm(&root.path);
        if r.is_empty() {
            continue;
        }
        let sep = if cfg!(windows) { '\\' } else { '/' };
        if wanted == r || wanted.starts_with(&format!("{r}{sep}")) {
            return Ok((candidate, root.clone()));
        }
    }
    bail!("dossier hors des emplacements autorisés : {path}")
}

/// Encode une image RGBA (8 bits par voie, lignes de haut en bas) en PNG.
#[cfg_attr(not(windows), allow(dead_code))]
pub fn encode_png(width: u32, height: u32, rgba: &[u8]) -> Vec<u8> {
    use std::io::Write;
    let stride = width as usize * 4;
    let mut raw = Vec::with_capacity((stride + 1) * height as usize);
    for row in rgba.chunks(stride).take(height as usize) {
        raw.push(0); // filtre « aucun »
        raw.extend_from_slice(row);
    }
    let mut z = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
    let _ = z.write_all(&raw);
    let idat = z.finish().unwrap_or_default();
    let mut out = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
    let mut ihdr = Vec::with_capacity(13);
    ihdr.extend_from_slice(&width.to_be_bytes());
    ihdr.extend_from_slice(&height.to_be_bytes());
    ihdr.extend_from_slice(&[8, 6, 0, 0, 0]); // 8 bits, RGBA, deflate, filtre, pas d'entrelacement
    png_chunk(&mut out, b"IHDR", &ihdr);
    png_chunk(&mut out, b"IDAT", &idat);
    png_chunk(&mut out, b"IEND", &[]);
    out
}

#[cfg_attr(not(windows), allow(dead_code))]
fn png_chunk(out: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) {
    out.extend_from_slice(&(data.len() as u32).to_be_bytes());
    let start = out.len();
    out.extend_from_slice(kind);
    out.extend_from_slice(data);
    let crc = crc32(&out[start..]);
    out.extend_from_slice(&crc.to_be_bytes());
}

#[cfg_attr(not(windows), allow(dead_code))]
fn crc32(bytes: &[u8]) -> u32 {
    let mut c = 0xFFFF_FFFFu32;
    for &b in bytes {
        c ^= u32::from(b);
        for _ in 0..8 {
            c = if c & 1 != 0 { 0xEDB8_8320 ^ (c >> 1) } else { c >> 1 };
        }
    }
    !c
}

// --- Ce que les agents font, dans la session de l'utilisateur ---------------

/// `display-agent apps --icons <dossier>` : le menu Démarrer, les suggestions
/// et les dossiers de l'explorateur, pour le compte qui lance l'agent.
pub fn agent_apps(icons: &Path) -> Result<Value> {
    let catalogue = platform::enumerate(icons)?;
    Ok(serde_json::to_value(catalogue)?)
}

/// `display-agent launch …` : ouvre l'app (ou la ramène au premier plan).
pub fn agent_launch(kind: LaunchKind, target: &str, exe: Option<&str>) -> Result<Value> {
    platform::agent_launch(kind, target, exe)
}

/// `display-agent close …` : ferme l'app.
pub fn agent_close(uri: Option<&str>, pid: Option<u32>, exe: Option<&str>) -> Result<Value> {
    platform::agent_close(uri, pid, exe)
}

#[cfg(windows)]
mod platform {
    //! Windows : agents avec le compte de l'utilisateur, API du shell.

    use super::*;
    use crate::display::agent;

    /// Lit le catalogue par l'agent, dans la session de l'utilisateur.
    pub fn read_catalogue() -> Result<AgentCatalogue> {
        let icons = agent::user_local_app_data()?.join("KyberNow").join(DIR).join(ICONS_DIR);
        let args = vec!["apps".to_string(), "--icons".into(), icons.display().to_string()];
        let value = agent::run_as_user(&args, AGENT_TIMEOUT_MS)?;
        Ok(serde_json::from_value(value)?)
    }

    pub fn launch(launch: &Launch) -> Result<Value> {
        let kind = match launch.kind {
            LaunchKind::App => "app",
            LaunchKind::Uri => "uri",
            LaunchKind::Path => "path",
        };
        let mut args = vec!["launch".to_string(), "--kind".into(), kind.into(), "--target".into(), launch.target.clone()];
        if let Some(exe) = &launch.exe {
            args.push("--exe".into());
            args.push(exe.clone());
        }
        agent::run_as_user(&args, AGENT_TIMEOUT_MS)
    }

    pub fn close(launch: &Launch, pid: Option<u32>) -> Result<Value> {
        let mut args = vec!["close".to_string()];
        if let Some(uri) = &launch.close {
            args.push("--uri".into());
            args.push(uri.clone());
        }
        if let Some(pid) = pid {
            args.push("--pid".into());
            args.push(pid.to_string());
        }
        if let Some(exe) = &launch.exe {
            args.push("--exe".into());
            args.push(exe.clone());
        }
        agent::run_as_user(&args, AGENT_TIMEOUT_MS)
    }

    /// La session console de Windows est-elle verrouillée ? (Depuis le service.)
    pub fn session_locked() -> bool {
        use std::ffi::c_void;
        use windows_sys::Win32::System::RemoteDesktop::{
            WTS_CURRENT_SERVER_HANDLE, WTS_SESSIONSTATE_LOCK, WTSFreeMemory, WTSGetActiveConsoleSessionId, WTSINFOEXW,
            WTSQuerySessionInformationW, WTSSessionInfoEx,
        };
        unsafe {
            let session = WTSGetActiveConsoleSessionId();
            if session == 0xFFFF_FFFF {
                return false;
            }
            let mut buf: *mut u16 = std::ptr::null_mut();
            let mut len = 0u32;
            if WTSQuerySessionInformationW(WTS_CURRENT_SERVER_HANDLE, session, WTSSessionInfoEx, &mut buf, &mut len) == 0 || buf.is_null() {
                return false;
            }
            let info = &*(buf as *const WTSINFOEXW);
            let flags = if info.Level == 1 { info.Data.WTSInfoExLevel1.SessionFlags } else { -1 };
            WTSFreeMemory(buf as *mut c_void);
            flags == WTS_SESSIONSTATE_LOCK as i32
        }
    }

    // --- Côté agent (compte de l'utilisateur) ------------------------------

    use std::ffi::c_void;
    use std::ptr::null_mut;

    use windows::Win32::Foundation::{PROPERTYKEY, SIZE};
    use windows::Win32::Storage::EnhancedStorage::{PKEY_AppUserModel_ID, PKEY_Link_TargetParsingPath};
    use windows::Win32::System::Com::{COINIT_APARTMENTTHREADED, CoInitializeEx, CoTaskMemFree};
    use windows::Win32::UI::Shell::{
        BHID_EnumItems, FOLDERID_AppsFolder, FOLDERID_Desktop, IEnumShellItems, IShellItem, IShellItem2, IShellItemImageFactory,
        KF_FLAG_DEFAULT, SHCreateItemFromParsingName, SHGetKnownFolderItem, SHGetKnownFolderPath, SIGDN_NORMALDISPLAY, SIIGBF,
        SIIGBF_BIGGERSIZEOK, SIIGBF_ICONONLY,
    };
    use windows::core::{Interface, PCWSTR, PWSTR};
    use windows_sys::Win32::Foundation::{CloseHandle, HWND, INVALID_HANDLE_VALUE, LPARAM};
    use windows_sys::Win32::Graphics::Gdi::{
        BI_RGB, BITMAP, BITMAPINFO, BITMAPINFOHEADER, CreateCompatibleDC, DIB_RGB_COLORS, DeleteDC, DeleteObject, GetDIBits, GetObjectW,
    };
    use windows_sys::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, PROCESSENTRY32W, Process32FirstW, Process32NextW, TH32CS_SNAPPROCESS,
    };
    use windows_sys::Win32::System::Registry::{HKEY, HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, RRF_RT_REG_SZ, RegGetValueW};
    use windows_sys::Win32::System::Threading::{
        GetProcessId, OpenProcess, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION, QueryFullProcessImageNameW,
    };
    use windows_sys::Win32::UI::Shell::{SEE_MASK_FLAG_NO_UI, SEE_MASK_NOASYNC, SEE_MASK_NOCLOSEPROCESS, SHELLEXECUTEINFOW, ShellExecuteExW};
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        EnumWindows, GW_OWNER, GetWindow, GetWindowThreadProcessId, IsIconic, IsWindowVisible, PostMessageW, SW_RESTORE, SW_SHOWNORMAL,
        SetForegroundWindow, ShowWindow, SwitchToThisWindow, WM_CLOSE,
    };

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }

    fn take_pwstr(p: PWSTR) -> Option<String> {
        if p.is_null() {
            return None;
        }
        let s = unsafe { p.to_string() }.ok();
        unsafe { CoTaskMemFree(Some(p.0 as *const c_void)) };
        s
    }

    fn display_name(item: &IShellItem) -> Option<String> {
        take_pwstr(unsafe { item.GetDisplayName(SIGDN_NORMALDISPLAY) }.ok()?)
    }

    fn string_prop(item: &IShellItem2, key: &PROPERTYKEY) -> Option<String> {
        take_pwstr(unsafe { item.GetString(key) }.ok()?)
    }

    /// L'icône d'un élément du shell, en PNG de 64 pixels dans `icons`.
    fn save_icon(item: &IShellItem, icons: &Path, id: &str) -> Option<String> {
        let factory: IShellItemImageFactory = item.cast().ok()?;
        let bitmap = unsafe { factory.GetImage(SIZE { cx: 64, cy: 64 }, SIIGBF(SIIGBF_ICONONLY.0 | SIIGBF_BIGGERSIZEOK.0)) }.ok()?;
        let (w, h, rgba) = bitmap_rgba(bitmap.0)?;
        let name = icon_name(id);
        std::fs::write(icons.join(&name), encode_png(w, h, &rgba)).ok()?;
        Some(name)
    }

    /// Les pixels d'un HBITMAP 32 bits (BGRA prémultiplié) en RGBA ; libère le bitmap.
    fn bitmap_rgba(hbm: *mut c_void) -> Option<(u32, u32, Vec<u8>)> {
        unsafe {
            let mut bm: BITMAP = std::mem::zeroed();
            if GetObjectW(hbm, size_of::<BITMAP>() as i32, &mut bm as *mut _ as *mut c_void) == 0 || bm.bmWidth <= 0 || bm.bmHeight <= 0 {
                DeleteObject(hbm);
                return None;
            }
            let (w, h) = (bm.bmWidth as u32, bm.bmHeight as u32);
            let mut info: BITMAPINFO = std::mem::zeroed();
            info.bmiHeader = BITMAPINFOHEADER {
                biSize: size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: w as i32,
                biHeight: -(h as i32), // de haut en bas
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB,
                ..std::mem::zeroed()
            };
            let mut bgra = vec![0u8; (w * h * 4) as usize];
            let hdc = CreateCompatibleDC(null_mut());
            let lines = GetDIBits(hdc, hbm, 0, h, bgra.as_mut_ptr() as *mut c_void, &mut info, DIB_RGB_COLORS);
            DeleteDC(hdc);
            DeleteObject(hbm);
            if lines == 0 {
                return None;
            }
            let opaque = bgra.chunks(4).all(|p| p[3] == 0);
            let mut rgba = Vec::with_capacity(bgra.len());
            for p in bgra.chunks(4) {
                let a = if opaque { 255 } else { p[3] };
                let un = |c: u8| if opaque || a == 0 { c } else { ((u32::from(c) * 255 + u32::from(a) / 2) / u32::from(a)).min(255) as u8 };
                rgba.extend_from_slice(&[un(p[2]), un(p[1]), un(p[0]), a]);
            }
            Some((w, h, rgba))
        }
    }

    fn file_icon(icons: &Path, id: &str, path: &str) -> Option<String> {
        let item: IShellItem = unsafe { SHCreateItemFromParsingName(PCWSTR(wide(path).as_ptr()), None) }.ok()?;
        save_icon(&item, icons, id)
    }

    fn app_entry(item: &IShellItem, icons: &Path) -> Option<AppEntry> {
        let name = display_name(item)?;
        let item2: IShellItem2 = item.cast().ok()?;
        let id = string_prop(&item2, &PKEY_AppUserModel_ID)?;
        if id.trim().is_empty() || name.trim().is_empty() {
            return None;
        }
        let exe = string_prop(&item2, &PKEY_Link_TargetParsingPath).filter(|p| p.to_lowercase().ends_with(".exe"));
        let icon = save_icon(item, icons, &id);
        Some(AppEntry { id, name, icon, exe })
    }

    /// Le menu Démarrer du compte qui lance l'agent (`shell:AppsFolder`).
    pub fn enumerate(icons: &Path) -> Result<AgentCatalogue> {
        std::fs::create_dir_all(icons).with_context(|| format!("création de {}", icons.display()))?;
        unsafe {
            let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        }
        let mut apps: Vec<AppEntry> = Vec::new();
        unsafe {
            let folder: IShellItem =
                SHGetKnownFolderItem(&FOLDERID_AppsFolder, KF_FLAG_DEFAULT, None).context("dossier des applications de Windows")?;
            let items: IEnumShellItems = folder.BindToHandler(None, &BHID_EnumItems).context("énumération des applications")?;
            loop {
                let mut slot: [Option<IShellItem>; 1] = [None];
                let mut fetched = 0u32;
                if items.Next(&mut slot, Some(&mut fetched)).is_err() || fetched == 0 {
                    break;
                }
                let Some(item) = slot[0].take() else { break };
                if let Some(entry) = app_entry(&item, icons) {
                    // Deux raccourcis pour la même app : le premier reste.
                    if !apps.iter().any(|a| a.id == entry.id) {
                        apps.push(entry);
                    }
                }
            }
        }
        apps.sort_by_key(|a| a.name.to_lowercase());
        let suggestions = suggestions(icons, &apps);
        Ok(AgentCatalogue {
            user: std::env::var("USERNAME").ok(),
            apps,
            suggestions,
            roots: roots(),
            icons_dir: icons.display().to_string(),
        })
    }

    fn reg_string(key: HKEY, sub: &str, value: &str) -> Option<String> {
        let (sub, value) = (wide(sub), wide(value));
        let mut buf = [0u16; 1024];
        let mut len = (buf.len() * 2) as u32;
        let ok = unsafe {
            RegGetValueW(key, sub.as_ptr(), value.as_ptr(), RRF_RT_REG_SZ, null_mut(), buf.as_mut_ptr() as *mut c_void, &mut len) == 0
        };
        if !ok {
            return None;
        }
        let end = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
        let s = String::from_utf16_lossy(&buf[..end]);
        (!s.trim().is_empty()).then(|| s.trim().to_string())
    }

    fn existing(path: PathBuf) -> Option<String> {
        path.is_file().then(|| path.display().to_string())
    }

    fn env_dir(name: &str) -> Option<PathBuf> {
        std::env::var_os(name).map(PathBuf::from).filter(|p| p.is_dir())
    }

    /// Un lanceur ouvert par son exécutable.
    fn path_suggestion(out: &mut Vec<Suggestion>, icons: &Path, id: &str, name: &str, exe: String) {
        out.push(Suggestion {
            id: id.into(),
            name: name.into(),
            kind: LaunchKind::Path,
            target: exe.clone(),
            close: None,
            exe: Some(exe.clone()),
            icon: file_icon(icons, id, &exe),
        });
    }

    /// Les lanceurs connus, s'ils sont installés.
    fn suggestions(icons: &Path, apps: &[AppEntry]) -> Vec<Suggestion> {
        let mut out = Vec::new();
        // Steam : Big Picture par son adresse, fermé par la sienne.
        if let Some(steam) = reg_string(HKEY_CURRENT_USER, r"Software\Valve\Steam", "SteamExe")
            .map(PathBuf::from)
            .and_then(existing)
            .or_else(|| reg_string(HKEY_LOCAL_MACHINE, r"SOFTWARE\WOW6432Node\Valve\Steam", "InstallPath").and_then(|d| existing(PathBuf::from(d).join("steam.exe"))))
        {
            out.push(Suggestion {
                id: "steam-bigpicture".into(),
                name: "Steam Big Picture".into(),
                kind: LaunchKind::Uri,
                target: "steam://open/bigpicture".into(),
                close: Some("steam://close/bigpicture".into()),
                exe: Some(steam.clone()),
                icon: file_icon(icons, "steam-bigpicture", &steam),
            });
        }
        if let Some(p) = env_dir("LOCALAPPDATA").and_then(|d| existing(d.join("Playnite").join("Playnite.FullscreenApp.exe"))) {
            path_suggestion(&mut out, icons, "playnite", "Playnite Fullscreen", p);
        }
        if let Some(a) = apps.iter().find(|a| a.id.starts_with("Microsoft.GamingApp_")) {
            out.push(Suggestion {
                id: "xbox".into(),
                name: a.name.clone(),
                kind: LaunchKind::App,
                target: a.id.clone(),
                close: None,
                exe: None,
                icon: a.icon.clone(),
            });
        }
        if let Some(p) = reg_string(HKEY_LOCAL_MACHINE, r"SOFTWARE\WOW6432Node\GOG.com\GalaxyClient\paths", "client")
            .and_then(|d| existing(PathBuf::from(d).join("GalaxyClient.exe")))
        {
            path_suggestion(&mut out, icons, "gog", "GOG Galaxy", p);
        }
        if let Some(p) = env_dir("ProgramFiles(x86)").and_then(|d| {
            existing(d.join("Epic Games").join("Launcher").join("Portal").join("Binaries").join("Win64").join("EpicGamesLauncher.exe"))
        }) {
            path_suggestion(&mut out, icons, "epic", "Epic Games", p);
        }
        if let Some(p) = env_dir("ProgramFiles").and_then(|d| existing(d.join("Electronic Arts").join("EA Desktop").join("EA Desktop").join("EADesktop.exe"))) {
            path_suggestion(&mut out, icons, "ea", "EA app", p);
        }
        if let Some(p) = reg_string(HKEY_LOCAL_MACHINE, r"SOFTWARE\WOW6432Node\Ubisoft\Launcher", "InstallDir")
            .and_then(|d| existing(PathBuf::from(&d).join("UbisoftConnect.exe")).or_else(|| existing(PathBuf::from(d).join("upc.exe"))))
        {
            path_suggestion(&mut out, icons, "ubisoft", "Ubisoft Connect", p);
        }
        if let Some(p) = env_dir("ProgramFiles(x86)").and_then(|d| existing(d.join("Battle.net").join("Battle.net Launcher.exe"))) {
            path_suggestion(&mut out, icons, "battlenet", "Battle.net", p);
        }
        out
    }

    /// Les dossiers de l'explorateur « Autre… ».
    fn roots() -> Vec<Root> {
        let mut out = Vec::new();
        if let Some(p) = env_dir("ProgramFiles") {
            out.push(Root { id: "programs".into(), path: p.display().to_string() });
        }
        if let Some(p) = env_dir("ProgramFiles(x86)") {
            out.push(Root { id: "programs86".into(), path: p.display().to_string() });
        }
        if let Some(desktop) = unsafe { SHGetKnownFolderPath(&FOLDERID_Desktop, KF_FLAG_DEFAULT, None) }.ok().and_then(take_pwstr) {
            if Path::new(&desktop).is_dir() {
                out.push(Root { id: "desktop".into(), path: desktop });
            }
        }
        if let Some(p) = env_dir("LOCALAPPDATA").map(|d| d.join("Programs")).filter(|p| p.is_dir()) {
            out.push(Root { id: "userPrograms".into(), path: p.display().to_string() });
        }
        out
    }

    /// `ShellExecuteEx` d'un fichier, d'une adresse ou d'un `shell:AppsFolder\…`.
    fn shell_execute(file: &str, directory: Option<&str>) -> Result<Option<u32>> {
        let (verb, file) = (wide("open"), wide(file));
        let dir = directory.map(wide);
        unsafe {
            let mut info: SHELLEXECUTEINFOW = std::mem::zeroed();
            info.cbSize = size_of::<SHELLEXECUTEINFOW>() as u32;
            info.fMask = SEE_MASK_NOCLOSEPROCESS | SEE_MASK_NOASYNC | SEE_MASK_FLAG_NO_UI;
            info.lpVerb = verb.as_ptr();
            info.lpFile = file.as_ptr();
            info.lpDirectory = dir.as_ref().map_or(std::ptr::null(), |d| d.as_ptr());
            info.nShow = SW_SHOWNORMAL;
            if ShellExecuteExW(&mut info) == 0 {
                bail!("lancement refusé : {}", std::io::Error::last_os_error());
            }
            let pid = if info.hProcess.is_null() {
                None
            } else {
                let pid = GetProcessId(info.hProcess);
                CloseHandle(info.hProcess);
                (pid != 0).then_some(pid)
            };
            Ok(pid)
        }
    }

    /// Chemin complet de l'exécutable d'un processus.
    fn process_path(pid: u32) -> Option<String> {
        unsafe {
            let h = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
            if h.is_null() {
                return None;
            }
            let mut buf = [0u16; 1024];
            let mut len = buf.len() as u32;
            let ok = QueryFullProcessImageNameW(h, PROCESS_NAME_WIN32, buf.as_mut_ptr(), &mut len) != 0;
            CloseHandle(h);
            ok.then(|| String::from_utf16_lossy(&buf[..len as usize]))
        }
    }

    /// Les processus dont l'exécutable est `exe` (même chemin, sans la casse).
    fn processes_by_exe(exe: &str) -> Vec<u32> {
        let file = Path::new(exe).file_name().map(|f| f.to_string_lossy().to_lowercase()).unwrap_or_default();
        let mut pids = Vec::new();
        unsafe {
            let snap = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
            if snap == INVALID_HANDLE_VALUE {
                return pids;
            }
            let mut entry = PROCESSENTRY32W { dwSize: size_of::<PROCESSENTRY32W>() as u32, ..std::mem::zeroed() };
            let mut ok = Process32FirstW(snap, &mut entry) != 0;
            while ok {
                let len = entry.szExeFile.iter().position(|&c| c == 0).unwrap_or(entry.szExeFile.len());
                if String::from_utf16_lossy(&entry.szExeFile[..len]).to_lowercase() == file
                    && process_path(entry.th32ProcessID).is_some_and(|p| p.eq_ignore_ascii_case(exe))
                {
                    pids.push(entry.th32ProcessID);
                }
                ok = Process32NextW(snap, &mut entry) != 0;
            }
            CloseHandle(snap);
        }
        pids
    }

    struct Windows {
        pid: u32,
        found: Vec<HWND>,
    }

    unsafe extern "system" fn collect(hwnd: HWND, lparam: LPARAM) -> i32 {
        unsafe {
            let list = &mut *(lparam as *mut Windows);
            let mut pid = 0u32;
            GetWindowThreadProcessId(hwnd, &mut pid);
            if pid == list.pid && IsWindowVisible(hwnd) != 0 && GetWindow(hwnd, GW_OWNER).is_null() {
                list.found.push(hwnd);
            }
        }
        1
    }

    /// Les fenêtres principales (visibles, sans propriétaire) d'un processus.
    fn top_windows(pid: u32) -> Vec<HWND> {
        let mut list = Windows { pid, found: Vec::new() };
        unsafe { EnumWindows(Some(collect), &mut list as *mut Windows as LPARAM) };
        list.found
    }

    /// Ramène la première fenêtre de `pid` au premier plan. Faux sans fenêtre.
    fn bring_to_front(pid: u32) -> bool {
        let Some(&hwnd) = top_windows(pid).first() else { return false };
        unsafe {
            if IsIconic(hwnd) != 0 {
                ShowWindow(hwnd, SW_RESTORE);
            }
            SwitchToThisWindow(hwnd, 1);
            SetForegroundWindow(hwnd);
        }
        true
    }

    pub fn agent_launch(kind: LaunchKind, target: &str, exe: Option<&str>) -> Result<Value> {
        unsafe {
            let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        }
        // Déjà ouverte : au premier plan (pas pour une adresse : Steam, lui,
        // doit passer en Big Picture, ce que l'adresse fait).
        if kind != LaunchKind::Uri {
            if let Some(pid) = exe.map(processes_by_exe).and_then(|p| p.into_iter().next()) {
                if bring_to_front(pid) {
                    return Ok(json!({ "front": true, "pid": pid }));
                }
            }
        }
        let (file, dir) = match kind {
            LaunchKind::App => (format!("shell:AppsFolder\\{target}"), None),
            LaunchKind::Uri => (target.to_string(), None),
            LaunchKind::Path => (target.to_string(), Path::new(target).parent().map(|p| p.display().to_string())),
        };
        let pid = shell_execute(&file, dir.as_deref())?;
        Ok(json!({ "front": false, "pid": pid }))
    }

    pub fn agent_close(uri: Option<&str>, pid: Option<u32>, exe: Option<&str>) -> Result<Value> {
        unsafe {
            let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        }
        if let Some(uri) = uri {
            shell_execute(uri, None)?;
            return Ok(json!({ "closed": "uri" }));
        }
        let mut pids: Vec<u32> = pid.into_iter().collect();
        if let Some(exe) = exe {
            for p in processes_by_exe(exe) {
                if !pids.contains(&p) {
                    pids.push(p);
                }
            }
        }
        let mut closed = 0;
        for pid in pids {
            for hwnd in top_windows(pid) {
                unsafe { PostMessageW(hwnd, WM_CLOSE, 0, 0) };
                closed += 1;
            }
        }
        Ok(json!({ "closed": closed }))
    }
}

#[cfg(not(windows))]
mod platform {
    //! Hors Windows (Mac de développement, tests) : un catalogue simulé, et
    //! des lancements qui ne font que s'écrire au journal.

    use super::*;

    pub fn read_catalogue() -> Result<AgentCatalogue> {
        let dir = std::env::temp_dir().join("kybernow-apps-sim");
        let _ = std::fs::create_dir_all(&dir);
        Ok(AgentCatalogue {
            user: Some("dev".into()),
            apps: vec![
                AppEntry { id: "Sim.Calculator".into(), name: "Calculatrice (simulée)".into(), icon: None, exe: None },
                AppEntry { id: "Sim.Excel".into(), name: "Excel (simulé)".into(), icon: None, exe: Some(r"C:\Program Files\Microsoft Office\root\Office16\EXCEL.EXE".into()) },
                AppEntry { id: "Microsoft.GamingApp_8wekyb3d8bbwe!Microsoft.Xbox.App".into(), name: "Xbox (simulé)".into(), icon: None, exe: None },
            ],
            suggestions: vec![
                Suggestion {
                    id: "steam-bigpicture".into(),
                    name: "Steam Big Picture".into(),
                    kind: LaunchKind::Uri,
                    target: "steam://open/bigpicture".into(),
                    close: Some("steam://close/bigpicture".into()),
                    exe: None,
                    icon: None,
                },
                Suggestion {
                    id: "xbox".into(),
                    name: "Xbox (simulé)".into(),
                    kind: LaunchKind::App,
                    target: "Microsoft.GamingApp_8wekyb3d8bbwe!Microsoft.Xbox.App".into(),
                    close: None,
                    exe: None,
                    icon: None,
                },
            ],
            roots: vec![Root { id: "programs".into(), path: "/Applications".into() }],
            icons_dir: dir.display().to_string(),
        })
    }

    pub fn launch(launch: &Launch) -> Result<Value> {
        tracing::info!("(simulé) lancement de {} ({:?} {})", launch.name, launch.kind, launch.target);
        Ok(json!({ "front": false, "pid": null }))
    }

    pub fn close(launch: &Launch, _pid: Option<u32>) -> Result<Value> {
        tracing::info!("(simulé) fermeture de {}", launch.name);
        Ok(json!({ "closed": 0 }))
    }

    pub fn session_locked() -> bool {
        false
    }

    pub fn enumerate(_icons: &Path) -> Result<AgentCatalogue> {
        read_catalogue()
    }

    pub fn agent_launch(_kind: LaunchKind, _target: &str, _exe: Option<&str>) -> Result<Value> {
        bail!("réservé à Windows")
    }

    pub fn agent_close(_uri: Option<&str>, _pid: Option<u32>, _exe: Option<&str>) -> Result<Value> {
        bail!("réservé à Windows")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn png_bien_forme() {
        let png = encode_png(2, 1, &[255, 0, 0, 255, 0, 0, 255, 128]);
        assert_eq!(&png[..8], &[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]);
        assert_eq!(&png[12..16], b"IHDR");
        assert_eq!(&png[16..24], &[0, 0, 0, 2, 0, 0, 0, 1]);
        assert_eq!(&png[png.len() - 8..png.len() - 4], b"IEND");
        // CRC de IEND, connu.
        assert_eq!(&png[png.len() - 4..], &[0xAE, 0x42, 0x60, 0x82]);
    }

    #[test]
    fn noms_d_icones() {
        assert!(icon_name_ok(&icon_name("steam-bigpicture")));
        assert!(!icon_name_ok("../secret.png"));
        assert!(!icon_name_ok("ABCDEF0123456789.png"));
        assert_eq!(icon_name("a").len(), 20);
    }

    #[test]
    fn dossiers_autorises() {
        let roots = vec![Root { id: "programs".into(), path: "/Applications".into() }];
        assert!(within_roots("/Applications/Utilities", &roots).is_ok());
        assert!(within_roots("/Applications", &roots).is_ok());
        assert!(within_roots("/ApplicationsX", &roots).is_err());
        assert!(within_roots("/Applications/../etc", &roots).is_err());
        assert!(within_roots("/etc", &roots).is_err());
    }

    #[test]
    fn reglage_valide_et_complete() {
        let dir = std::env::temp_dir().join(format!("kn-launch-{}", rand::random::<u64>()));
        let l = Launcher::new(&dir);
        l.refresh();
        // Une adresse qui n'est pas une suggestion est refusée.
        let bad = Launch { kind: LaunchKind::Uri, target: "http://example.com".into(), name: "X".into(), icon: None, exe: None, close: None, close_at_end: true, skip_desktop: false };
        assert!(l.validate(bad).is_err());
        // Big Picture reçoit son adresse de fermeture.
        let bp = Launch { kind: LaunchKind::Uri, target: "steam://open/bigpicture".into(), name: "Steam Big Picture".into(), icon: Some("x".into()), exe: None, close: None, close_at_end: true, skip_desktop: true };
        let v = l.validate(bp).unwrap();
        assert_eq!(v.close.as_deref(), Some("steam://close/bigpicture"));
        assert_eq!(v.icon, None);
        assert!(v.close_at_end && v.skip_desktop);
        // Une app du catalogue prend son exécutable.
        let app = Launch { kind: LaunchKind::App, target: "Sim.Excel".into(), name: "Excel".into(), icon: None, exe: None, close: None, close_at_end: false, skip_desktop: false };
        assert!(l.validate(app).unwrap().exe.is_some());
        // Un chemin hors des dossiers autorisés est refusé.
        let path = Launch { kind: LaunchKind::Path, target: "/etc/passwd".into(), name: "passwd".into(), icon: None, exe: None, close: None, close_at_end: false, skip_desktop: false };
        assert!(l.validate(path).is_err());
    }

    #[test]
    fn reprise_de_main_garde_la_meme_app() {
        let dir = std::env::temp_dir().join(format!("kn-launch-{}", rand::random::<u64>()));
        let l = Launcher::new(&dir);
        let bp = Launch { kind: LaunchKind::Uri, target: "steam://open/bigpicture".into(), name: "Steam Big Picture".into(), icon: None, exe: None, close: Some("steam://close/bigpicture".into()), close_at_end: true, skip_desktop: false };
        l.start_blocking("tv", "Apple TV", bp.clone());
        assert_eq!(l.state.lock().unwrap().running.as_ref().map(|r| r.device_id.clone()).as_deref(), Some("tv"));
        // Un Mac prend la main avec la même app : elle reste.
        l.expect_takeover("Mac", Some(&bp));
        l.session_ended("tv");
        assert!(l.state.lock().unwrap().running.is_some());
        l.start_blocking("mac", "Mac", bp);
        assert_eq!(l.state.lock().unwrap().running.as_ref().map(|r| r.device_id.clone()).as_deref(), Some("mac"));
        // Fin sans suite : fermée.
        l.session_ended("mac");
        assert!(l.state.lock().unwrap().running.is_none());
    }
}
