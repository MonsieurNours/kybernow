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

#[cfg(windows)]
pub mod agent;
#[cfg(windows)]
pub mod pointer;
#[cfg(windows)]
mod sudovda;
#[cfg(windows)]
pub mod topology;

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
        Self { trigger: Trigger::Apps, width: default_width(), height: default_height(), refresh: default_refresh() }
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
}

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
        let backend: Box<dyn Backend> = Box::new(Simulated);
        Arc::new(Self {
            backend,
            data_dir,
            session: Mutex::new(None),
            ops: Mutex::new(()),
            last_error: Mutex::new(None),
            alive: AtomicBool::new(false),
            settings: Mutex::new(settings),
        })
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
    pub fn start(
        &self,
        origin: Origin,
        mode: Mode,
        user: Option<String>,
        device: Option<String>,
        duration: Option<Duration>,
    ) -> Result<Value> {
        mode.validate()?;
        let _op = self.ops.lock().unwrap();
        self.start_locked(origin, mode, user, device, duration)
    }

    fn start_locked(
        &self,
        origin: Origin,
        mode: Mode,
        user: Option<String>,
        device: Option<String>,
        duration: Option<Duration>,
    ) -> Result<Value> {
        {
            let mut session = self.session.lock().unwrap();
            if let Some(s) = session.as_mut() {
                if s.mode == mode {
                    s.last_seen = Instant::now();
                    // Une app garde son écran : « Auto » ne la rétrograde pas.
                    if !(s.origin == Origin::App && origin == Origin::Auto) {
                        s.origin = origin;
                        s.until = duration.map(|d| Instant::now() + d);
                    }
                    s.user = user.or(s.user.take());
                    s.device = device.or(s.device.take());
                    return Ok(s.applied.clone());
                }
                if s.origin == Origin::App {
                    match origin {
                        // Le fil de surveillance pendant une session d'app : rien à faire.
                        Origin::Auto => return Ok(s.applied.clone()),
                        Origin::Test => bail!("un appareil utilise l'écran virtuel : essai impossible pendant sa session"),
                        Origin::App => {}
                    }
                }
            }
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
        *self.session.lock().unwrap() = Some(Session {
            origin,
            mode,
            guid,
            user,
            device,
            started_at: now_secs(),
            last_seen: Instant::now(),
            until: duration.map(|d| Instant::now() + d),
            applied: applied.clone(),
        });
        Ok(applied)
    }

    /// Signal de vie d'un appareil. Vrai si l'écran virtuel en place est le
    /// sien : un appareil dont l'écran a été remplacé par celui d'un autre
    /// l'apprend ainsi, au lieu de le redemander sans fin.
    pub fn heartbeat(&self, device: Option<&str>) -> bool {
        match self.session.lock().unwrap().as_mut() {
            Some(s) => {
                let owner = match (device, s.device.as_deref()) {
                    (Some(d), Some(owner)) => d == owner,
                    _ => true,
                };
                if owner {
                    s.last_seen = Instant::now();
                }
                owner
            }
            None => false,
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
        let session = self.session.lock().unwrap();
        let session = session.as_ref().map(|s| {
            json!({
                "origin": s.origin,
                "mode": s.mode,
                "user": s.user,
                "started_at": s.started_at,
                "seconds_left": s.until.map(|u| u.saturating_duration_since(Instant::now()).as_secs()),
                "applied": s.applied,
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
            "last_error": *self.last_error.lock().unwrap(),
        })
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
                            s.last_seen.elapsed() > Duration::from_secs(20),
                            Some(s.origin),
                        ),
                        None => (false, false, false, None),
                    }
                };

                if active {
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
                    let _ = self.start_locked(Origin::Auto, mode, None, None, None);
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

/// Hors Windows : pilote simulé, pour développer l'interface sur Mac.
#[cfg(not(windows))]
struct Simulated;

#[cfg(not(windows))]
impl Backend for Simulated {
    fn driver(&self) -> Result<Value> {
        Ok(json!({ "version": "simulation", "watchdog_seconds": 3 }))
    }
    fn create(&self, _: [u8; 16], mode: Mode) -> Result<Created> {
        tracing::info!("(simulé) création d'un écran {}×{} à {} Hz", mode.width, mode.height, mode.refresh);
        Ok(Created { luid_low: 0, luid_high: 0, target: 0 })
    }
    fn remove(&self, _: [u8; 16]) -> Result<()> {
        Ok(())
    }
    fn ping(&self) -> Result<()> {
        Ok(())
    }
    fn isolate(&self, _: Created, mode: Mode) -> Result<Value> {
        Ok(json!({ "gdi_name": "\\\\.\\DISPLAY9", "width": mode.width, "height": mode.height, "refresh": mode.refresh, "disabled_others": 1 }))
    }
    fn restore(&self) -> Result<()> {
        tracing::info!("(simulé) restauration des écrans");
        Ok(())
    }
}

#[cfg(windows)]
mod windows {
    use super::*;

    pub struct WinBackend {
        data_dir: PathBuf,
        device: Mutex<Option<sudovda::Device>>,
    }

    impl WinBackend {
        pub fn new(data_dir: PathBuf) -> Self {
            Self { data_dir, device: Mutex::new(None) }
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

    #[test]
    fn une_app_garde_son_ecran_face_au_declenchement_automatique() {
        let m = manager();
        m.start(Origin::App, MAC, Some("matthieu".into()), Some("mac".into()), None).unwrap();
        // Le fil de surveillance (mode par défaut) ne remplace pas l'écran de l'app.
        m.start(Origin::Auto, TV, None, None, None).unwrap();
        let status = m.status();
        assert_eq!(status["session"]["mode"]["width"], 3456);
        assert_eq!(status["session"]["origin"], "app");
        // Même mode en « Auto » : l'origine reste celle de l'app.
        m.start(Origin::Auto, MAC, None, None, None).unwrap();
        assert_eq!(m.status()["session"]["origin"], "app");
        // Un essai depuis la console est refusé pendant la session de l'app.
        assert!(m.start(Origin::Test, TV, None, None, Some(Duration::from_secs(5))).is_err());
    }

    #[test]
    fn signal_de_vie_du_proprietaire_seulement() {
        let m = manager();
        assert!(!m.heartbeat(Some("mac")));
        m.start(Origin::App, MAC, Some("matthieu".into()), Some("mac".into()), None).unwrap();
        assert!(m.heartbeat(Some("mac")));
        assert!(m.owned_by("mac"));
        // Un autre appareil prend la main : le premier n'a plus « son » écran.
        m.start(Origin::App, TV, Some("matthieu".into()), Some("tv".into()), None).unwrap();
        assert!(!m.heartbeat(Some("mac")));
        assert!(m.heartbeat(Some("tv")));
        // Une app d'avant 0.2.5 (sans identifiant) est crue sur parole.
        assert!(m.heartbeat(None));
        m.stop("fin du test");
        assert!(!m.heartbeat(Some("tv")));
    }
}
