//! Écran virtuel adapté à l'appareil qui se connecte.
//!
//! Au début d'une session : un écran virtuel SudoVDA est créé au mode du
//! client, puis il devient le seul écran actif (les écrans physiques
//! s'éteignent). À la fin : l'écran virtuel est retiré et la disposition
//! habituelle des écrans physiques est réappliquée.
//!
//! Filets de sécurité :
//! - le pilote retire ses écrans s'il ne reçoit plus de signal de vie (3 s) ;
//! - l'état d'une session est écrit sur disque et nettoyé au démarrage suivant ;
//! - une session client sans nouvelles depuis 20 s, et sans flux encodé (NVIDIA, AMD ou Intel), s'arrête.
//!
//! Pendant la session, la console relit chaque seconde le mode de l'écran
//! virtuel et les écrans actifs (agent de veille, `watch.rs`) : le mode
//! courant est donné aux apps avec le signal de vie, et un écran physique
//! qui se rallume (câble rebranché, disposition réappliquée par Windows) est
//! éteint de nouveau après 2 s. Si le mode change (un jeu qui passe en
//! 1080p), la politique décide : `follow` laisse faire et l'annonce,
//! `restore` remet le mode de la session (au plus une fois toutes les 5 s).

#[cfg(windows)]
pub mod agent;
#[cfg(windows)]
pub mod pointer;
#[cfg(windows)]
mod sudovda;
#[cfg(windows)]
pub mod topology;
#[cfg(windows)]
pub mod watch;

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum Trigger {
    /// Jamais d'écran virtuel.
    Off,
    /// Seulement quand une app KyberNow annonce sa résolution.
    #[default]
    Apps,
    /// Pour toute connexion (navigateur, client Kyber…), au mode par défaut.
    All,
}

/// Que faire quand le mode de l'écran virtuel change pendant la session.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ModePolicy {
    /// Laisser faire (un jeu choisit sa définition) et l'annoncer.
    #[default]
    Follow,
    /// Remettre le mode demandé par l'appareil.
    Restore,
}

/// Ce qu'un appareil demande (`mode_policy` de `prepare` et du signal de
/// vie) : une politique, ou `default` pour revenir au réglage de la console
/// (profil Max ou personnalisé choisi en session).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PolicyChoice {
    Follow,
    Restore,
    Default,
}

impl PolicyChoice {
    fn policy(self) -> Option<ModePolicy> {
        match self {
            PolicyChoice::Follow => Some(ModePolicy::Follow),
            PolicyChoice::Restore => Some(ModePolicy::Restore),
            PolicyChoice::Default => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VdSettings {
    #[serde(default)]
    pub trigger: Trigger,
    #[serde(default = "default_width")]
    pub width: u32,
    #[serde(default = "default_height")]
    pub height: u32,
    #[serde(default = "default_refresh")]
    pub refresh: u32,
    /// Politique par défaut, quand l'app n'en donne pas (`mode_policy`).
    #[serde(default)]
    pub on_mode_change: ModePolicy,
}

fn default_width() -> u32 {
    1920
}
fn default_height() -> u32 {
    1080
}
fn default_refresh() -> u32 {
    60
}

impl Default for VdSettings {
    fn default() -> Self {
        Self {
            trigger: Trigger::Apps,
            width: default_width(),
            height: default_height(),
            refresh: default_refresh(),
            on_mode_change: ModePolicy::default(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Mode {
    pub width: u32,
    pub height: u32,
    pub refresh: u32,
}

impl Mode {
    pub fn validate(&self) -> Result<()> {
        if !(640..=7680).contains(&self.width) || !(480..=4320).contains(&self.height) {
            bail!("résolution hors limites (640 × 480 à 7680 × 4320)");
        }
        if !(24..=500).contains(&self.refresh) {
            bail!("fréquence hors limites (24 à 500 Hz)");
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Origin {
    App,
    Auto,
    Test,
}

/// Où retrouver l'écran créé dans la configuration d'affichage.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Created {
    pub luid_low: u32,
    pub luid_high: i32,
    pub target: u32,
}

/// Ce que Windows affiche pendant la session (agent de veille).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Observed {
    /// Mode courant de l'écran virtuel (0 × 0 s'il n'est plus actif).
    pub mode: Mode,
    #[serde(skip)]
    pub gdi_name: String,
    /// Écrans actifs, écran virtuel compris.
    pub displays: u32,
    /// Écrans actifs autres que l'écran virtuel (écrans physiques rallumés).
    pub others: u32,
}

/// Un écran physique rallumé n'est éteint qu'après ce délai de stabilité.
const PHYSICAL_DEBOUNCE: Duration = Duration::from_secs(2);
/// Au plus un nouvel isolement dans cet intervalle (Windows qui insiste).
const REISOLATE_COOLDOWN: Duration = Duration::from_secs(10);
/// Politique `restore` : au plus une remise du mode dans cet intervalle.
const MODE_FIX_INTERVAL: Duration = Duration::from_secs(5);
/// Un jeu qui remet sa définition après chaque remise : au bout de trois
/// remises en une minute, la console laisse faire pour le reste de la session.
const MAX_MODE_FIXES: usize = 3;
const MODE_FIX_WINDOW: Duration = Duration::from_secs(60);
/// Windows qui rallume sans cesse un écran : au bout de cinq isolements en
/// deux minutes, ou de trois échecs de suite, la console renonce.
const MAX_REISOLATIONS: usize = 5;
const REISOLATE_WINDOW: Duration = Duration::from_secs(120);
const MAX_REISOLATE_FAILURES: u32 = 3;

/// Même mode, à 1 Hz près (Windows dit 59 pour 59,94).
fn same_mode(a: Mode, b: Mode) -> bool {
    a.width == b.width && a.height == b.height && a.refresh.abs_diff(b.refresh) <= 1
}

/// Mode réellement obtenu à la création (`applied`), sinon celui demandé.
fn applied_mode(applied: &Value, requested: Mode) -> Mode {
    let n = |k: &str| applied[k].as_u64().filter(|v| *v > 0).map(|v| v as u32);
    match (n("width"), n("height")) {
        (Some(width), Some(height)) => Mode { width, height, refresh: n("refresh").unwrap_or(requested.refresh) },
        _ => requested,
    }
}

fn label(m: Mode) -> String {
    format!("{} × {} à {} Hz", m.width, m.height, m.refresh)
}

struct Session {
    origin: Origin,
    mode: Mode,
    guid: [u8; 16],
    user: Option<String>,
    /// Appareil pour lequel l'écran a été créé (apps à partir de 0.2.5).
    device: Option<String>,
    started_at: u64,
    last_seen: Instant,
    until: Option<Instant>,
    applied: Value,
    created: Created,
    /// Mode obtenu à la création : la référence pour dire que le mode a changé.
    target: Mode,
    /// Dernier état lu par la veille.
    observed: Option<Observed>,
    /// Écrans physiques vus allumés depuis cet instant.
    others_since: Option<Instant>,
    last_reisolate: Option<Instant>,
    /// Dernier mode inattendu annoncé dans le fil (une fois par mode).
    announced: Option<Mode>,
    /// Politique donnée par l'app (sinon, celle des réglages).
    policy: Option<ModePolicy>,
    last_mode_fix: Option<Instant>,
    /// Remises du mode de la dernière minute ; au-delà, la console renonce.
    mode_fixes: Vec<Instant>,
    restore_gave_up: bool,
    /// Isolements refaits des deux dernières minutes, échecs de suite.
    reisolations: Vec<Instant>,
    reisolate_failures: u32,
    reisolate_gave_up: bool,
}

impl Session {
    /// Politique en vigueur : celle de l'app, sinon le réglage ; `follow` si
    /// la console a renoncé à remettre le mode.
    fn effective_policy(&self, default: ModePolicy) -> ModePolicy {
        if self.restore_gave_up { ModePolicy::Follow } else { self.policy.unwrap_or(default) }
    }
}

/// Fil des événements de la console (vue d'ensemble).
pub type EventSink = Arc<dyn Fn(&str, &str) + Send + Sync>;

/// Fichier écrit pendant une session, pour nettoyer après un plantage.
#[derive(Serialize, Deserialize)]
struct Record {
    guid: String,
}

trait Backend: Send + Sync {
    fn driver(&self) -> Result<Value>;
    fn create(&self, guid: [u8; 16], mode: Mode) -> Result<Created>;
    fn remove(&self, guid: [u8; 16]) -> Result<()>;
    fn ping(&self) -> Result<()>;
    fn isolate(&self, created: Created, mode: Mode) -> Result<Value>;
    fn restore(&self) -> Result<()>;
    /// Début et fin de la veille de l'écran créé.
    fn watch(&self, created: Created);
    fn unwatch(&self);
    /// Dernier état vu par la veille (None : inconnu pour l'instant).
    fn observe(&self) -> Option<Observed>;
    /// Remet `mode` sur l'écran `gdi_name` ; rend le mode obtenu.
    fn set_mode(&self, gdi_name: &str, mode: Mode) -> Result<Mode>;
}

pub struct Manager {
    backend: Box<dyn Backend>,
    data_dir: PathBuf,
    session: Mutex<Option<Session>>,
    /// Sérialise les opérations longues (création, isolement, restauration).
    ops: Mutex<()>,
    last_error: Mutex<Option<String>>,
    /// Un écran virtuel existe : le fil des signaux de vie doit pinguer le pilote.
    alive: AtomicBool,
    pub settings: Mutex<VdSettings>,
    events: Mutex<Option<EventSink>>,
}

/// « KyberNow écran virtuel », identifiant stable du moniteur SudoVDA.
const VIRTUAL_DISPLAY_GUID: [u8; 16] = *b"KyberNow-Display";

pub fn now_secs() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

impl Manager {
    pub fn new(data_dir: PathBuf, settings: VdSettings) -> Arc<Self> {
        #[cfg(windows)]
        let backend: Box<dyn Backend> = Box::new(windows::WinBackend::new(data_dir.clone()));
        #[cfg(not(windows))]
        let backend: Box<dyn Backend> = Box::new(Simulated::default());
        Self::with_backend(data_dir, settings, backend)
    }

    fn with_backend(data_dir: PathBuf, settings: VdSettings, backend: Box<dyn Backend>) -> Arc<Self> {
        Arc::new(Self {
            backend,
            data_dir,
            session: Mutex::new(None),
            ops: Mutex::new(()),
            last_error: Mutex::new(None),
            alive: AtomicBool::new(false),
            settings: Mutex::new(settings),
            events: Mutex::new(None),
        })
    }

    /// Branche le fil des événements (vue d'ensemble de la console).
    pub fn on_event(&self, sink: EventSink) {
        *self.events.lock().unwrap() = Some(sink);
    }

    fn event(&self, kind: &str, text: &str) {
        let sink = self.events.lock().unwrap().clone();
        match sink {
            Some(sink) => sink(kind, text),
            None => tracing::info!("écran virtuel : {text}"),
        }
    }

    fn record_path(&self) -> PathBuf {
        self.data_dir.join("display-session.json")
    }

    fn fail(&self, e: &anyhow::Error) {
        tracing::error!("écran virtuel : {e:#}");
        *self.last_error.lock().unwrap() = Some(format!("{e:#}"));
    }

    /// Après un plantage ou une coupure : retire l'écran et restaure les écrans physiques.
    pub fn recover(&self) {
        let Ok(text) = std::fs::read_to_string(self.record_path()) else { return };
        tracing::warn!("session d'écran virtuel interrompue trouvée : restauration des écrans");
        if let Ok(record) = serde_json::from_str::<Record>(&text) {
            if let Ok(bytes) = hex::decode(record.guid) {
                if let Ok(guid) = <[u8; 16]>::try_from(bytes.as_slice()) {
                    let _ = self.backend.remove(guid);
                }
            }
        }
        self.alive.store(false, Ordering::SeqCst);
        std::thread::sleep(Duration::from_millis(500));
        match self.backend.restore() {
            Ok(()) => {
                let _ = std::fs::remove_file(self.record_path());
            }
            // Les écrans ne sont pas rendus (pas de session console, par
            // exemple) : on garde le fichier pour réessayer au prochain démarrage.
            Err(e) => self.fail(&e),
        }
    }

    /// Démarre (ou reprend) une session à `mode`. Un autre mode remplace la
    /// session en cours, sauf qu'une session d'app n'est jamais remplacée par
    /// le déclenchement automatique ni par un essai depuis la console.
    /// `policy` : ce que l'app veut quand le mode change (sinon, le réglage).
    pub fn start(
        &self,
        origin: Origin,
        mode: Mode,
        user: Option<String>,
        device: Option<String>,
        duration: Option<Duration>,
        policy: Option<PolicyChoice>,
    ) -> Result<Value> {
        mode.validate()?;
        let _op = self.ops.lock().unwrap();
        self.start_locked(origin, mode, user, device, duration, policy)
    }

    fn start_locked(
        &self,
        origin: Origin,
        mode: Mode,
        user: Option<String>,
        device: Option<String>,
        duration: Option<Duration>,
        policy: Option<PolicyChoice>,
    ) -> Result<Value> {
        let default_policy = self.settings.lock().unwrap().on_mode_change;
        let mut fix: Option<(String, Mode)> = None;
        let kept = {
            let mut session = self.session.lock().unwrap();
            match session.as_mut() {
                Some(s) if s.mode == mode => {
                    s.last_seen = Instant::now();
                    // Une app garde son écran : « Auto » ne la rétrograde pas.
                    if !(s.origin == Origin::App && origin == Origin::Auto) {
                        s.origin = origin;
                        s.until = duration.map(|d| Instant::now() + d);
                    }
                    s.user = user.clone().or(s.user.take());
                    s.device = device.clone().or(s.device.take());
                    if let Some(choice) = policy {
                        s.policy = choice.policy();
                    }
                    // « Remettre » dans l'app : la console réessaie, même si
                    // elle avait renoncé face à un jeu qui insistait.
                    if policy == Some(PolicyChoice::Restore) {
                        s.restore_gave_up = false;
                        s.mode_fixes.clear();
                    }
                    // Même écran redemandé alors que le mode a changé : en
                    // politique `restore` (« Remettre » dans l'app), on le remet.
                    if s.effective_policy(default_policy) == ModePolicy::Restore {
                        if let Some(o) = s.observed.as_ref().filter(|o| o.mode.width > 0 && !same_mode(o.mode, s.target)) {
                            fix = Some((o.gdi_name.clone(), s.target));
                        }
                    }
                    Some(s.applied.clone())
                }
                Some(s) if s.origin == Origin::App => match origin {
                    // Le fil de surveillance pendant une session d'app : rien à faire.
                    Origin::Auto => return Ok(s.applied.clone()),
                    Origin::Test => bail!("un appareil utilise l'écran virtuel : essai impossible pendant sa session"),
                    Origin::App => None,
                },
                _ => None,
            }
        };
        if let Some(applied) = kept {
            if let Some((gdi_name, target)) = fix {
                self.fix_mode(&gdi_name, target, Instant::now());
            }
            return Ok(applied);
        }
        // Mode différent (un autre appareil prend la main) : on repart de zéro.
        self.stop_locked("remplacée par une nouvelle session");

        // Identifiant fixe : Windows retrouve la disposition de session
        // enregistrée pour cet écran (voir topology.rs).
        let guid = VIRTUAL_DISPLAY_GUID;
        let begun = Instant::now();
        let created = self.backend.create(guid, mode).inspect_err(|e| self.fail(e))?;
        // Pinguer dès maintenant : l'isolement peut durer plus que le chien de garde.
        self.alive.store(true, Ordering::SeqCst);
        let _ = std::fs::write(self.record_path(), serde_json::to_vec(&Record { guid: hex::encode(guid) })?);
        let applied = match self.backend.isolate(created, mode) {
            Ok(v) => v,
            Err(e) => {
                self.fail(&e);
                let _ = self.backend.remove(guid);
                self.alive.store(false, Ordering::SeqCst);
                std::thread::sleep(Duration::from_millis(500));
                let _ = self.backend.restore();
                let _ = std::fs::remove_file(self.record_path());
                return Err(e);
            }
        };
        tracing::info!(
            "écran virtuel {}×{} à {} Hz actif ({origin:?}), prêt en {} ms",
            mode.width,
            mode.height,
            mode.refresh,
            begun.elapsed().as_millis()
        );
        *self.last_error.lock().unwrap() = None;
        self.backend.watch(created);
        *self.session.lock().unwrap() = Some(Session {
            origin,
            mode,
            guid,
            user,
            device,
            started_at: now_secs(),
            last_seen: Instant::now(),
            until: duration.map(|d| Instant::now() + d),
            target: applied_mode(&applied, mode),
            applied: applied.clone(),
            created,
            observed: None,
            others_since: None,
            last_reisolate: None,
            announced: None,
            policy: policy.and_then(PolicyChoice::policy),
            last_mode_fix: None,
            mode_fixes: Vec::new(),
            restore_gave_up: false,
            reisolations: Vec::new(),
            reisolate_failures: 0,
            reisolate_gave_up: false,
        });
        Ok(applied)
    }

    /// Signal de vie d'un appareil. Vrai si l'écran virtuel en place est le
    /// sien : un appareil dont l'écran a été remplacé par celui d'un autre
    /// l'apprend ainsi, au lieu de le redemander sans fin.
    /// `policy` : celle du profil choisi en ce moment dans l'app.
    pub fn heartbeat(&self, device: Option<&str>, policy: Option<PolicyChoice>) -> bool {
        match self.session.lock().unwrap().as_mut() {
            Some(s) => {
                let owner = match (device, s.device.as_deref()) {
                    (Some(d), Some(owner)) => d == owner,
                    _ => true,
                };
                if owner {
                    s.last_seen = Instant::now();
                    if let Some(choice) = policy {
                        s.policy = choice.policy();
                    }
                }
                owner
            }
            None => false,
        }
    }

    /// Politique en vigueur pour la session en cours.
    pub fn policy(&self) -> Option<ModePolicy> {
        let default = self.settings.lock().unwrap().on_mode_change;
        self.session.lock().unwrap().as_ref().map(|s| s.effective_policy(default))
    }

    /// Remet le mode de la session sur l'écran virtuel (politique `restore`).
    fn fix_mode(&self, gdi_name: &str, target: Mode, now: Instant) {
        if let Some(s) = self.session.lock().unwrap().as_mut() {
            s.last_mode_fix = Some(now);
        }
        match self.backend.set_mode(gdi_name, target) {
            Ok(got) if same_mode(got, target) => tracing::info!("mode de l'écran virtuel remis en {}", label(target)),
            Ok(got) => tracing::warn!("mode de l'écran virtuel : {} demandé, {} obtenu", label(target), label(got)),
            Err(e) => self.fail(&e),
        }
    }

    /// L'écran virtuel en place a-t-il été créé pour cet appareil ?
    pub fn owned_by(&self, device: &str) -> bool {
        self.session.lock().unwrap().as_ref().is_some_and(|s| s.device.as_deref() == Some(device))
    }

    pub fn stop(&self, reason: &str) {
        let _op = self.ops.lock().unwrap();
        self.stop_locked(reason);
    }

    fn stop_locked(&self, reason: &str) {
        let Some(session) = self.session.lock().unwrap().take() else { return };
        tracing::info!("fin de l'écran virtuel : {reason}");
        self.backend.unwatch();
        if let Err(e) = self.backend.remove(session.guid) {
            self.fail(&e);
        }
        self.alive.store(false, Ordering::SeqCst);
        std::thread::sleep(Duration::from_millis(500));
        match self.backend.restore() {
            // Le fichier de session reste tant que les écrans ne sont pas
            // rendus : le prochain démarrage réessaiera.
            Ok(()) => {
                let _ = std::fs::remove_file(self.record_path());
            }
            Err(e) => self.fail(&e),
        }
    }

    pub fn status(&self) -> Value {
        let observed = self.observed();
        let session = self.session.lock().unwrap();
        let session = session.as_ref().map(|s| {
            json!({
                "origin": s.origin,
                "mode": s.mode,
                "user": s.user,
                "started_at": s.started_at,
                "seconds_left": s.until.map(|u| u.saturating_duration_since(Instant::now()).as_secs()),
                "applied": s.applied,
                "mode_policy": s.effective_policy(self.settings.lock().unwrap().on_mode_change),
            })
        });
        let driver = match self.backend.driver() {
            Ok(v) => json!({ "available": true, "info": v }),
            Err(e) => json!({ "available": false, "error": format!("{e:#}") }),
        };
        json!({
            "supported": cfg!(windows),
            "settings": *self.settings.lock().unwrap(),
            "driver": driver,
            "session": session,
            // Mode courant de l'écran virtuel et nombre d'écrans actifs, en session.
            "mode": observed.as_ref().map(|o| o.mode).filter(|m| m.width > 0),
            "displays": observed.as_ref().map(|o| o.displays),
            "last_error": *self.last_error.lock().unwrap(),
        })
    }

    /// Ce que la veille a vu en dernier, pendant une session.
    pub fn observed(&self) -> Option<Observed> {
        self.session.lock().unwrap().as_ref().and_then(|s| s.observed.clone())
    }

    /// Un tour de veille (chaque seconde pendant une session) : relit le mode
    /// de l'écran virtuel et les écrans actifs, annonce un changement de mode,
    /// et éteint de nouveau un écran physique rallumé depuis 2 s.
    pub fn watch_tick(&self, now: Instant) {
        // Une création ou une fin en cours a la priorité : on repassera.
        let Ok(_op) = self.ops.try_lock() else { return };
        if self.session.lock().unwrap().is_none() {
            return;
        }
        let observed = self.backend.observe();
        let mut notes: Vec<(&str, String)> = Vec::new();
        let mut reisolate: Option<(Created, Mode, u32)> = None;
        let mut fix: Option<(String, Mode)> = None;
        let default_policy = self.settings.lock().unwrap().on_mode_change;
        {
            let mut guard = self.session.lock().unwrap();
            let Some(s) = guard.as_mut() else { return };
            let policy = s.effective_policy(default_policy);
            s.observed = observed.clone();
            let Some(o) = observed else {
                // Veille muette : la stabilité d'un écran rallumé se recompte à son retour.
                s.others_since = None;
                return;
            };

            // Écran physique rallumé pendant la session.
            if o.others > 0 && !s.reisolate_gave_up {
                let since = *s.others_since.get_or_insert(now);
                // Après un échec, les essais s'espacent (10, 20, 40 s).
                let cooldown = REISOLATE_COOLDOWN * (1 << s.reisolate_failures.min(4));
                let cooled = s.last_reisolate.is_none_or(|t| now.duration_since(t) >= cooldown);
                if now.duration_since(since) >= PHYSICAL_DEBOUNCE && cooled {
                    s.reisolations.retain(|t| now.duration_since(*t) < REISOLATE_WINDOW);
                    if s.reisolations.len() >= MAX_REISOLATIONS {
                        s.reisolate_gave_up = true;
                        notes.push(("warn", "Windows rallume sans cesse un écran de l'ordinateur : il reste allumé pour le reste de la session".to_string()));
                    } else {
                        // En `follow`, le mode en cours est gardé (un jeu a pu le choisir).
                        let keep = if policy == ModePolicy::Follow && o.mode.width > 0 { o.mode } else { s.target };
                        reisolate = Some((s.created, keep, o.others));
                    }
                }
            } else {
                s.others_since = None;
            }

            // Mode changé sur l'ordinateur (jeu, réglage de Windows).
            if reisolate.is_none() && o.mode.width > 0 && !same_mode(o.mode, s.target) {
                if s.announced != Some(o.mode) {
                    s.announced = Some(o.mode);
                    notes.push(("info", match policy {
                        ModePolicy::Follow => format!("L'écran virtuel est passé en {} (demandé : {}), laissé tel quel", label(o.mode), label(s.target)),
                        ModePolicy::Restore => format!("L'écran virtuel est passé en {} : remis en {}", label(o.mode), label(s.target)),
                    }));
                }
                let due = s.last_mode_fix.is_none_or(|t| now.duration_since(t) >= MODE_FIX_INTERVAL);
                if policy == ModePolicy::Restore && due {
                    s.mode_fixes.retain(|t| now.duration_since(*t) < MODE_FIX_WINDOW);
                    if s.mode_fixes.len() >= MAX_MODE_FIXES {
                        // Le jeu remet sa définition après chaque remise : on
                        // arrête la ronde (et les relances d'image qu'elle coûte).
                        s.restore_gave_up = true;
                        notes.push(("warn", format!("La définition de l'écran virtuel revient sans cesse à {} (un jeu l'impose) : laissée telle quelle pour le reste de la session", label(o.mode))));
                    } else {
                        s.mode_fixes.push(now);
                        fix = Some((o.gdi_name.clone(), s.target));
                    }
                }
            } else if same_mode(o.mode, s.target) {
                s.announced = None;
            }
        }
        for (kind, text) in notes {
            self.event(kind, &text);
        }
        if let Some((gdi_name, target)) = fix {
            self.fix_mode(&gdi_name, target, now);
        }
        if let Some((created, mode, others)) = reisolate {
            match self.backend.isolate(created, mode) {
                Ok(applied) => {
                    if let Some(s) = self.session.lock().unwrap().as_mut() {
                        s.applied = applied;
                        s.last_reisolate = Some(now);
                        s.others_since = None;
                        s.reisolations.push(now);
                        s.reisolate_failures = 0;
                    }
                    self.event("info", &if others > 1 {
                        format!("{others} écrans de l'ordinateur se sont rallumés pendant la session : éteints de nouveau")
                    } else {
                        "Un écran de l'ordinateur s'est rallumé pendant la session : éteint de nouveau".to_string()
                    });
                }
                Err(e) => {
                    self.fail(&e);
                    let (first, gave_up) = match self.session.lock().unwrap().as_mut() {
                        Some(s) => {
                            s.last_reisolate = Some(now);
                            s.reisolate_failures += 1;
                            s.reisolate_gave_up = s.reisolate_failures >= MAX_REISOLATE_FAILURES;
                            (s.reisolate_failures == 1, s.reisolate_gave_up)
                        }
                        None => (false, false),
                    };
                    // Annoncé au premier échec et quand la console renonce, pas à chaque essai.
                    if first {
                        self.event("warn", &format!("Un écran de l'ordinateur s'est rallumé pendant la session et n'a pas pu être éteint : {e:#}"));
                    } else if gave_up {
                        self.event("warn", "L'écran rallumé n'a pas pu être éteint après trois essais : il reste allumé pour le reste de la session");
                    }
                }
            }
        }
    }

    /// Fils dédiés : signaux de vie du pilote, puis surveillance des sessions.
    pub fn supervise(self: Arc<Self>) {
        // Signaux de vie : un fil à part, que rien d'autre ne retarde (NVML,
        // arrêt d'une session…), pour que le chien de garde (3 s) ne se déclenche
        // pas en pleine session.
        let pinger = self.clone();
        std::thread::spawn(move || {
            let mut ping_failed = false;
            loop {
                std::thread::sleep(Duration::from_millis(500));
                if !pinger.alive.load(Ordering::SeqCst) {
                    ping_failed = false;
                    continue;
                }
                match pinger.backend.ping() {
                    Ok(()) => ping_failed = false,
                    Err(e) if !ping_failed => {
                        ping_failed = true;
                        pinger.fail(&e);
                    }
                    Err(_) => {}
                }
            }
        });

        std::thread::spawn(move || {
            let mut idle_since: Option<Instant> = None;
            loop {
                std::thread::sleep(Duration::from_secs(1));
                let encoding = crate::gpu::encoding();
                let (active, expired_test, client_gone, origin) = {
                    let s = self.session.lock().unwrap();
                    match s.as_ref() {
                        Some(s) => (
                            true,
                            s.until.is_some_and(|u| Instant::now() > u),
                            s.last_seen.elapsed() > crate::activity::HEARTBEAT_GRACE,
                            Some(s.origin),
                        ),
                        None => (false, false, false, None),
                    }
                };

                if active {
                    self.watch_tick(Instant::now());
                    idle_since = if encoding { None } else { idle_since.or(Some(Instant::now())) };
                    let idle_long = idle_since.is_some_and(|t| t.elapsed() > Duration::from_secs(10));
                    if expired_test {
                        self.stop("fin du test");
                    } else if origin == Some(Origin::App) && client_gone && !encoding {
                        self.stop("l'appareil ne donne plus de nouvelles");
                    } else if origin == Some(Origin::Auto) && idle_long {
                        self.stop("plus aucun flux en cours");
                    }
                    continue;
                }

                idle_since = None;
                let settings = self.settings.lock().unwrap().clone();
                if settings.trigger == Trigger::All && encoding {
                    // Une opération en cours (l'écran d'une app en train d'être
                    // créé) a la priorité : on ne lui passe pas devant.
                    let Ok(_op) = self.ops.try_lock() else { continue };
                    if self.session.lock().unwrap().is_some() {
                        continue;
                    }
                    let mode = Mode { width: settings.width, height: settings.height, refresh: settings.refresh };
                    let _ = self.start_locked(Origin::Auto, mode, None, None, None, None);
                }
            }
        });
    }

    /// Arrêt du service : l'ordinateur retrouve ses écrans tout de suite,
    /// sans attendre le prochain démarrage.
    pub fn shutdown(&self) {
        self.stop("arrêt de la console");
    }
}

/// Hors Windows : pilote simulé, pour développer l'interface sur Mac. La
/// veille y voit le mode demandé ; les tests changent l'écran à la main.
#[cfg(not(windows))]
#[derive(Clone, Default)]
struct Simulated {
    screen: Arc<Mutex<SimScreen>>,
}

#[cfg(not(windows))]
#[derive(Default)]
struct SimScreen {
    /// Mode de l'écran virtuel, s'il existe.
    mode: Option<Mode>,
    /// Écrans physiques allumés.
    others: u32,
    watching: bool,
    isolations: u32,
    mode_fixes: u32,
    /// Les isolements échouent (écran virtuel introuvable, par exemple).
    isolate_fails: bool,
    isolate_attempts: u32,
}

#[cfg(not(windows))]
impl Backend for Simulated {
    fn driver(&self) -> Result<Value> {
        Ok(json!({ "version": "simulation", "watchdog_seconds": 3 }))
    }
    fn create(&self, _: [u8; 16], mode: Mode) -> Result<Created> {
        tracing::info!("(simulé) création d'un écran {}×{} à {} Hz", mode.width, mode.height, mode.refresh);
        self.screen.lock().unwrap().mode = Some(mode);
        Ok(Created { luid_low: 0, luid_high: 0, target: 0 })
    }
    fn remove(&self, _: [u8; 16]) -> Result<()> {
        self.screen.lock().unwrap().mode = None;
        Ok(())
    }
    fn ping(&self) -> Result<()> {
        Ok(())
    }
    fn isolate(&self, _: Created, mode: Mode) -> Result<Value> {
        let mut screen = self.screen.lock().unwrap();
        screen.isolate_attempts += 1;
        if screen.isolate_fails {
            bail!("(simulé) l'écran virtuel n'apparaît pas dans la configuration d'affichage");
        }
        let disabled = screen.others;
        screen.mode = Some(mode);
        screen.others = 0;
        screen.isolations += 1;
        Ok(json!({ "gdi_name": "\\\\.\\DISPLAY9", "width": mode.width, "height": mode.height, "refresh": mode.refresh, "disabled_others": disabled }))
    }
    fn restore(&self) -> Result<()> {
        tracing::info!("(simulé) restauration des écrans");
        Ok(())
    }
    fn watch(&self, _: Created) {
        self.screen.lock().unwrap().watching = true;
    }
    fn unwatch(&self) {
        self.screen.lock().unwrap().watching = false;
    }
    fn set_mode(&self, _: &str, mode: Mode) -> Result<Mode> {
        let mut screen = self.screen.lock().unwrap();
        screen.mode = Some(mode);
        screen.mode_fixes += 1;
        Ok(mode)
    }
    fn observe(&self) -> Option<Observed> {
        let screen = self.screen.lock().unwrap();
        let mode = screen.mode.filter(|_| screen.watching)?;
        Some(Observed { mode, gdi_name: "\\\\.\\DISPLAY9".into(), displays: 1 + screen.others, others: screen.others })
    }
}

#[cfg(windows)]
mod windows {
    use super::*;

    pub struct WinBackend {
        data_dir: PathBuf,
        device: Mutex<Option<sudovda::Device>>,
        watch: watch::Watch,
    }

    impl WinBackend {
        pub fn new(data_dir: PathBuf) -> Self {
            Self { watch: watch::Watch::new(&data_dir), data_dir, device: Mutex::new(None) }
        }

        fn with_device<T>(&self, f: impl FnOnce(&sudovda::Device) -> Result<T>) -> Result<T> {
            let mut guard = self.device.lock().unwrap();
            if guard.is_none() {
                *guard = Some(sudovda::Device::open()?);
            }
            let result = f(guard.as_ref().unwrap());
            if result.is_err() {
                // Le pilote a pu être rechargé : on rouvrira au prochain appel.
                *guard = None;
            }
            result
        }
    }

    impl Backend for WinBackend {
        fn driver(&self) -> Result<Value> {
            self.with_device(|d| {
                let (timeout, _) = d.watchdog().unwrap_or((0, 0));
                Ok(json!({ "version": d.protocol_version()?, "watchdog_seconds": timeout }))
            })
        }
        fn create(&self, guid: [u8; 16], mode: Mode) -> Result<Created> {
            let c = self.with_device(|d| d.add(guid, mode.width, mode.height, mode.refresh))?;
            Ok(Created { luid_low: c.adapter_luid_low, luid_high: c.adapter_luid_high, target: c.target_id })
        }
        fn remove(&self, guid: [u8; 16]) -> Result<()> {
            self.with_device(|d| d.remove(guid))
        }
        fn ping(&self) -> Result<()> {
            self.with_device(|d| d.ping())
        }
        fn isolate(&self, c: Created, mode: Mode) -> Result<Value> {
            let args = [
                "isolate".to_string(),
                "--luid-low".into(), c.luid_low.to_string(),
                "--luid-high".into(), c.luid_high.to_string(),
                "--target".into(), c.target.to_string(),
                "--width".into(), mode.width.to_string(),
                "--height".into(), mode.height.to_string(),
                "--refresh".into(), mode.refresh.to_string(),
            ];
            agent::run(&self.data_dir, &args)
        }
        fn restore(&self) -> Result<()> {
            agent::run(&self.data_dir, &["restore".to_string()]).map(|_| ())
        }
        fn watch(&self, created: Created) {
            self.watch.start(&self.data_dir, created);
        }
        fn unwatch(&self) {
            self.watch.stop();
        }
        fn observe(&self) -> Option<Observed> {
            self.watch.observe(&self.data_dir)
        }
        fn set_mode(&self, gdi_name: &str, mode: Mode) -> Result<Mode> {
            if gdi_name.is_empty() {
                bail!("écran virtuel introuvable pour remettre son mode");
            }
            let args = [
                "set-mode".to_string(),
                "--gdi-name".into(), gdi_name.to_string(),
                "--width".into(), mode.width.to_string(),
                "--height".into(), mode.height.to_string(),
                "--refresh".into(), mode.refresh.to_string(),
            ];
            let v = agent::run_with_timeout(&self.data_dir, &args, 10_000)?;
            let n = |k: &str| v[k].as_u64().unwrap_or(0) as u32;
            Ok(Mode { width: n("width"), height: n("height"), refresh: n("refresh") })
        }
    }
}

#[cfg(all(test, not(windows)))]
mod tests {
    use super::*;

    fn manager() -> Arc<Manager> {
        let dir = std::env::temp_dir().join(format!("kn-display-{}", rand::random::<u64>()));
        std::fs::create_dir_all(&dir).unwrap();
        Manager::new(dir, VdSettings::default())
    }

    const MAC: Mode = Mode { width: 3456, height: 2234, refresh: 120 };
    const TV: Mode = Mode { width: 3840, height: 2160, refresh: 60 };
    const GAME: Mode = Mode { width: 1920, height: 1080, refresh: 60 };

    /// Gestionnaire sur un écran simulé que le test peut changer, et le fil
    /// des événements qu'il écrit.
    fn simulated(settings: VdSettings) -> (Arc<Manager>, Simulated, Arc<Mutex<Vec<String>>>) {
        let dir = std::env::temp_dir().join(format!("kn-display-{}", rand::random::<u64>()));
        std::fs::create_dir_all(&dir).unwrap();
        let sim = Simulated::default();
        let m = Manager::with_backend(dir, settings, Box::new(sim.clone()));
        let events = Arc::new(Mutex::new(Vec::new()));
        let feed = events.clone();
        m.on_event(Arc::new(move |_: &str, text: &str| feed.lock().unwrap().push(text.to_string())));
        (m, sim, events)
    }

    fn screen_mode(sim: &Simulated) -> Mode {
        sim.screen.lock().unwrap().mode.unwrap()
    }

    #[test]
    fn mode_change_en_session_politique_follow() {
        let (m, sim, events) = simulated(VdSettings::default());
        m.start(Origin::App, TV, Some("matthieu".into()), Some("tv".into()), None, Some(PolicyChoice::Follow)).unwrap();
        let t0 = Instant::now();
        m.watch_tick(t0);
        assert_eq!(m.observed().unwrap().mode, TV);
        assert_eq!(m.status()["mode"]["width"], 3840);
        assert_eq!(m.status()["displays"], 1);

        // Un jeu passe l'écran virtuel en 1080p : la console laisse faire et l'annonce une fois.
        sim.screen.lock().unwrap().mode = Some(GAME);
        for i in 0..8 {
            m.watch_tick(t0 + Duration::from_secs(1 + i));
        }
        assert_eq!(screen_mode(&sim), GAME);
        assert_eq!(sim.screen.lock().unwrap().mode_fixes, 0);
        assert_eq!(m.observed().unwrap().mode, GAME);
        assert_eq!(m.policy(), Some(ModePolicy::Follow));
        let feed = events.lock().unwrap().clone();
        assert_eq!(feed.len(), 1, "{feed:?}");
        assert!(feed[0].contains("1920 × 1080") && feed[0].contains("laissé tel quel"), "{feed:?}");

        // Un prepare au même mode (Kyber redémarré) ne touche pas au mode en follow.
        m.start(Origin::App, TV, Some("matthieu".into()), Some("tv".into()), None, None).unwrap();
        assert_eq!(screen_mode(&sim), GAME);

        // Le jeu rend la main : retour au mode demandé, plus rien à annoncer ;
        // un nouveau changement est annoncé de nouveau.
        sim.screen.lock().unwrap().mode = Some(TV);
        m.watch_tick(t0 + Duration::from_secs(10));
        sim.screen.lock().unwrap().mode = Some(GAME);
        m.watch_tick(t0 + Duration::from_secs(11));
        assert_eq!(events.lock().unwrap().len(), 2);
        // 59 Hz pour 60 : le même mode.
        sim.screen.lock().unwrap().mode = Some(Mode { refresh: 59, ..TV });
        m.watch_tick(t0 + Duration::from_secs(12));
        assert_eq!(events.lock().unwrap().len(), 2);
    }

    #[test]
    fn mode_change_en_session_politique_restore() {
        let (m, sim, events) = simulated(VdSettings::default());
        m.start(Origin::App, TV, Some("matthieu".into()), Some("tv".into()), None, Some(PolicyChoice::Restore)).unwrap();
        let t0 = Instant::now();
        m.watch_tick(t0);
        assert_eq!(sim.screen.lock().unwrap().mode_fixes, 0);

        // Le mode change : remis aussitôt.
        sim.screen.lock().unwrap().mode = Some(GAME);
        m.watch_tick(t0 + Duration::from_secs(1));
        assert_eq!(screen_mode(&sim), TV);
        assert_eq!(sim.screen.lock().unwrap().mode_fixes, 1);
        assert!(events.lock().unwrap()[0].contains("remis en 3840 × 2160"));

        // Le jeu insiste : au plus une remise toutes les 5 s.
        sim.screen.lock().unwrap().mode = Some(GAME);
        m.watch_tick(t0 + Duration::from_secs(2));
        m.watch_tick(t0 + Duration::from_secs(5));
        assert_eq!(sim.screen.lock().unwrap().mode_fixes, 1);
        assert_eq!(screen_mode(&sim), GAME);
        m.watch_tick(t0 + Duration::from_secs(6));
        assert_eq!(sim.screen.lock().unwrap().mode_fixes, 2);
        assert_eq!(screen_mode(&sim), TV);
        // Annoncé une seule fois tant que le mode inattendu est le même.
        assert_eq!(events.lock().unwrap().len(), 1);

        // « Remettre » dans l'app : un prepare au même mode remet tout de suite.
        sim.screen.lock().unwrap().mode = Some(GAME);
        m.watch_tick(t0 + Duration::from_secs(7));
        assert_eq!(sim.screen.lock().unwrap().mode_fixes, 2);
        m.start(Origin::App, TV, Some("matthieu".into()), Some("tv".into()), None, Some(PolicyChoice::Restore)).unwrap();
        assert_eq!(sim.screen.lock().unwrap().mode_fixes, 3);
        assert_eq!(screen_mode(&sim), TV);

        // Profil Jeu choisi en session (signal de vie) : la console laisse faire.
        assert!(m.heartbeat(Some("tv"), Some(PolicyChoice::Follow)));
        sim.screen.lock().unwrap().mode = Some(GAME);
        m.watch_tick(t0 + Duration::from_secs(20));
        assert_eq!(screen_mode(&sim), GAME);
        assert_eq!(sim.screen.lock().unwrap().mode_fixes, 3);
    }

    #[test]
    fn politique_restore_face_a_un_jeu_qui_insiste() {
        let (m, sim, events) = simulated(VdSettings::default());
        m.start(Origin::App, TV, None, Some("tv".into()), None, Some(PolicyChoice::Restore)).unwrap();
        let t0 = Instant::now();
        // Le jeu repasse en 1080p après chaque remise ; la veille voit les deux.
        for i in 0..12u64 {
            let at = t0 + Duration::from_secs(i * 3);
            sim.screen.lock().unwrap().mode = Some(GAME);
            m.watch_tick(at);
            m.watch_tick(at + Duration::from_secs(1));
        }
        // Trois remises, puis la console laisse faire pour le reste de la session.
        assert_eq!(sim.screen.lock().unwrap().mode_fixes, 3);
        assert_eq!(m.policy(), Some(ModePolicy::Follow));
        assert_eq!(m.status()["session"]["mode_policy"], "follow");
        let feed = events.lock().unwrap().clone();
        assert!(feed.last().unwrap().contains("laissée telle quelle"), "{feed:?}");
        assert!(feed.len() <= 5, "{feed:?}");
        // Le signal de vie (profil Bureau) ne relance pas la ronde…
        assert!(m.heartbeat(Some("tv"), Some(PolicyChoice::Restore)));
        assert_eq!(m.policy(), Some(ModePolicy::Follow));
        // … « Remettre » dans l'app, si.
        m.start(Origin::App, TV, None, Some("tv".into()), None, Some(PolicyChoice::Restore)).unwrap();
        assert_eq!(m.policy(), Some(ModePolicy::Restore));
        assert_eq!(screen_mode(&sim), TV);
    }

    #[test]
    fn profil_sans_politique_revient_au_reglage() {
        let (m, _, _) = simulated(VdSettings::default());
        m.start(Origin::App, TV, None, Some("tv".into()), None, Some(PolicyChoice::Restore)).unwrap();
        assert_eq!(m.policy(), Some(ModePolicy::Restore));
        // Profil Max choisi en session : le réglage de la console (follow).
        assert!(m.heartbeat(Some("tv"), Some(PolicyChoice::Default)));
        assert_eq!(m.policy(), Some(ModePolicy::Follow));
    }

    #[test]
    fn politique_par_defaut_du_reglage() {
        let settings = VdSettings { on_mode_change: ModePolicy::Restore, ..VdSettings::default() };
        let (m, sim, _) = simulated(settings);
        // Client sans politique (navigateur, app ancienne) : celle du réglage.
        m.start(Origin::App, TV, None, Some("web".into()), None, None).unwrap();
        assert_eq!(m.policy(), Some(ModePolicy::Restore));
        assert_eq!(m.status()["session"]["mode_policy"], "restore");
        sim.screen.lock().unwrap().mode = Some(GAME);
        m.watch_tick(Instant::now());
        assert_eq!(screen_mode(&sim), TV);
        // Un signal de vie sans politique ne la change pas.
        assert!(m.heartbeat(Some("web"), None));
        assert_eq!(m.policy(), Some(ModePolicy::Restore));
    }

    #[test]
    fn ecran_physique_revenu_en_session() {
        let (m, sim, events) = simulated(VdSettings::default());
        m.start(Origin::App, TV, Some("matthieu".into()), Some("tv".into()), None, None).unwrap();
        assert_eq!(sim.screen.lock().unwrap().isolations, 1);
        let t0 = Instant::now();

        // Un jeu a choisi 1080p, puis Windows rallume l'écran physique.
        sim.screen.lock().unwrap().mode = Some(GAME);
        sim.screen.lock().unwrap().others = 1;
        m.watch_tick(t0);
        assert_eq!(m.observed().unwrap().displays, 2);
        assert_eq!(m.status()["displays"], 2);
        // Pas avant 2 s de stabilité.
        m.watch_tick(t0 + Duration::from_millis(1500));
        assert_eq!(sim.screen.lock().unwrap().isolations, 1);
        m.watch_tick(t0 + Duration::from_secs(2));
        {
            let screen = sim.screen.lock().unwrap();
            assert_eq!(screen.isolations, 2);
            assert_eq!(screen.others, 0);
            // Politique follow : le mode choisi par le jeu est gardé.
            assert_eq!(screen.mode, Some(GAME));
        }
        assert!(events.lock().unwrap().iter().any(|e| e.contains("rallumé")), "{:?}", events.lock().unwrap());

        // Un écran qui clignote (rallumé moins de 2 s) est laissé tranquille.
        sim.screen.lock().unwrap().others = 1;
        m.watch_tick(t0 + Duration::from_secs(3));
        sim.screen.lock().unwrap().others = 0;
        m.watch_tick(t0 + Duration::from_secs(4));
        sim.screen.lock().unwrap().others = 1;
        m.watch_tick(t0 + Duration::from_secs(5));
        // Stable depuis 2 s, mais au plus un isolement toutes les 10 s.
        m.watch_tick(t0 + Duration::from_secs(7));
        assert_eq!(sim.screen.lock().unwrap().isolations, 2);
        m.watch_tick(t0 + Duration::from_secs(12));
        assert_eq!(sim.screen.lock().unwrap().isolations, 3);

        // Fin de session : plus de veille.
        m.stop("fin du test");
        assert!(m.observed().is_none());
        assert!(!sim.screen.lock().unwrap().watching);
        assert!(m.status()["mode"].is_null());
    }

    #[test]
    fn ecran_physique_qui_revient_sans_cesse() {
        let (m, sim, events) = simulated(VdSettings::default());
        m.start(Origin::App, TV, None, Some("tv".into()), None, None).unwrap();
        let t0 = Instant::now();
        // Windows le rallume juste après chaque isolement.
        for i in 0..40u64 {
            sim.screen.lock().unwrap().others = 1;
            m.watch_tick(t0 + Duration::from_secs(i * 3));
        }
        // Cinq isolements en deux minutes, puis la console renonce, une seule annonce.
        assert_eq!(sim.screen.lock().unwrap().isolations, 1 + 5);
        let feed = events.lock().unwrap().clone();
        assert_eq!(feed.iter().filter(|e| e.contains("sans cesse")).count(), 1, "{feed:?}");
    }

    #[test]
    fn ecran_physique_qui_ne_s_eteint_pas() {
        let (m, sim, events) = simulated(VdSettings::default());
        m.start(Origin::App, TV, None, Some("tv".into()), None, None).unwrap();
        sim.screen.lock().unwrap().isolate_fails = true;
        sim.screen.lock().unwrap().others = 1;
        let t0 = Instant::now();
        let attempts = |sim: &Simulated| sim.screen.lock().unwrap().isolate_attempts - 1;
        // Premier essai après 2 s de stabilité, puis 20 s et 40 s plus tard.
        m.watch_tick(t0);
        m.watch_tick(t0 + Duration::from_secs(2));
        assert_eq!(attempts(&sim), 1);
        m.watch_tick(t0 + Duration::from_secs(21));
        assert_eq!(attempts(&sim), 1);
        m.watch_tick(t0 + Duration::from_secs(22));
        assert_eq!(attempts(&sim), 2);
        m.watch_tick(t0 + Duration::from_secs(61));
        assert_eq!(attempts(&sim), 2);
        m.watch_tick(t0 + Duration::from_secs(62));
        assert_eq!(attempts(&sim), 3);
        // Puis plus rien : la console a renoncé, deux annonces en tout.
        for i in 63..400u64 {
            m.watch_tick(t0 + Duration::from_secs(i));
        }
        assert_eq!(attempts(&sim), 3);
        let feed = events.lock().unwrap().clone();
        assert_eq!(feed.len(), 2, "{feed:?}");
        assert!(feed[1].contains("trois essais"), "{feed:?}");
    }

    #[test]
    fn ecran_physique_revenu_politique_restore() {
        let (m, sim, _) = simulated(VdSettings::default());
        m.start(Origin::App, TV, None, Some("tv".into()), None, Some(PolicyChoice::Restore)).unwrap();
        let t0 = Instant::now();
        sim.screen.lock().unwrap().others = 2;
        sim.screen.lock().unwrap().mode = Some(GAME);
        m.watch_tick(t0);
        m.watch_tick(t0 + Duration::from_secs(2));
        // L'isolement remet aussi le mode de la session.
        let screen = sim.screen.lock().unwrap();
        assert_eq!((screen.isolations, screen.others, screen.mode), (2, 0, Some(TV)));
    }

    #[test]
    fn une_app_garde_son_ecran_face_au_declenchement_automatique() {
        let m = manager();
        m.start(Origin::App, MAC, Some("matthieu".into()), Some("mac".into()), None, None).unwrap();
        // Le fil de surveillance (mode par défaut) ne remplace pas l'écran de l'app.
        m.start(Origin::Auto, TV, None, None, None, None).unwrap();
        let status = m.status();
        assert_eq!(status["session"]["mode"]["width"], 3456);
        assert_eq!(status["session"]["origin"], "app");
        // Même mode en « Auto » : l'origine reste celle de l'app.
        m.start(Origin::Auto, MAC, None, None, None, None).unwrap();
        assert_eq!(m.status()["session"]["origin"], "app");
        // Un essai depuis la console est refusé pendant la session de l'app.
        assert!(m.start(Origin::Test, TV, None, None, Some(Duration::from_secs(5)), None).is_err());
    }

    #[test]
    fn signal_de_vie_du_proprietaire_seulement() {
        let m = manager();
        assert!(!m.heartbeat(Some("mac"), None));
        m.start(Origin::App, MAC, Some("matthieu".into()), Some("mac".into()), None, None).unwrap();
        assert!(m.heartbeat(Some("mac"), None));
        assert!(m.owned_by("mac"));
        // Un autre appareil prend la main : le premier n'a plus « son » écran.
        m.start(Origin::App, TV, Some("matthieu".into()), Some("tv".into()), None, None).unwrap();
        assert!(!m.heartbeat(Some("mac"), None));
        assert!(m.heartbeat(Some("tv"), None));
        // Une app d'avant 0.2.5 (sans identifiant) est crue sur parole.
        assert!(m.heartbeat(None, None));
        m.stop("fin du test");
        assert!(!m.heartbeat(Some("tv"), None));
    }
}
