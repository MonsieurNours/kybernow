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
//! - une session client sans nouvelles depuis 20 s, et sans flux NVENC, s'arrête.

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
        if let Err(e) = self.backend.restore() {
            self.fail(&e);
        }
        let _ = std::fs::remove_file(self.record_path());
    }

    /// Démarre (ou reprend) une session à `mode`. Un autre mode remplace la session en cours.
    pub fn start(&self, origin: Origin, mode: Mode, user: Option<String>, duration: Option<Duration>) -> Result<Value> {
        mode.validate()?;
        let _op = self.ops.lock().unwrap();
        {
            let mut session = self.session.lock().unwrap();
            if let Some(s) = session.as_mut() {
                if s.mode == mode {
                    s.last_seen = Instant::now();
                    s.origin = origin;
                    s.until = duration.map(|d| Instant::now() + d);
                    s.user = user.or(s.user.take());
                    return Ok(s.applied.clone());
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
            started_at: now_secs(),
            last_seen: Instant::now(),
            until: duration.map(|d| Instant::now() + d),
            applied: applied.clone(),
        });
        Ok(applied)
    }

    pub fn heartbeat(&self) -> bool {
        match self.session.lock().unwrap().as_mut() {
            Some(s) => {
                s.last_seen = Instant::now();
                true
            }
            None => false,
        }
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
        if let Err(e) = self.backend.restore() {
            self.fail(&e);
        }
        let _ = std::fs::remove_file(self.record_path());
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
                let encoding = crate::gpu::encoder_sessions().unwrap_or(0) > 0;
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
                    let mode = Mode { width: settings.width, height: settings.height, refresh: settings.refresh };
                    let _ = self.start(Origin::Auto, mode, None, None);
                }
            }
        });
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
