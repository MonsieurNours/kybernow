//! Sortie audio virtuelle pendant les sessions.
//!
//! Au début d'une session, le son de Windows part vers des haut-parleurs
//! virtuels (Steam Streaming Speakers), réglés au nombre de canaux voulu par
//! l'appareil : les haut-parleurs de l'ordinateur se taisent, et Kyber capture
//! le son au bon format (stéréo, 5.1, 7.1). À la fin, la sortie d'origine est
//! rétablie.
//!
//! Mêmes filets de sécurité que l'écran virtuel : état écrit sur disque et
//! rétabli au démarrage suivant, fin automatique quand l'appareil ne donne
//! plus de nouvelles et que plus rien n'est encodé.

#[cfg(windows)]
pub mod endpoint;

use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::display::{Origin, Trigger};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum Channels {
    /// Ce que demande l'appareil (stéréo s'il ne dit rien).
    #[default]
    Auto,
    Stereo,
    #[serde(rename = "5.1")]
    Surround51,
    #[serde(rename = "7.1")]
    Surround71,
}

impl Channels {
    fn count(self, requested: Option<u16>) -> u16 {
        match self {
            Channels::Auto => match requested {
                Some(6) => 6,
                Some(8) => 8,
                _ => 2,
            },
            Channels::Stereo => 2,
            Channels::Surround51 => 6,
            Channels::Surround71 => 8,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioSettings {
    #[serde(default)]
    pub trigger: Trigger,
    #[serde(default)]
    pub channels: Channels,
}

impl Default for AudioSettings {
    fn default() -> Self {
        Self { trigger: Trigger::Apps, channels: Channels::Auto }
    }
}

/// Fichier écrit pendant une session, pour rétablir le son après un plantage.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Record {
    pub previous: String,
    pub previous_format: Option<String>,
    pub speakers: String,
}

struct Active {
    origin: Origin,
    channels: u16,
    record: Record,
    started_at: u64,
    last_seen: Instant,
}

trait Backend: Send + Sync {
    fn status(&self) -> Result<Value>;
    /// Route le son vers les haut-parleurs virtuels ; renvoie de quoi revenir en arrière
    /// et le nombre de canaux obtenu.
    fn route(&self, channels: u16) -> Result<(Record, u16)>;
    fn restore(&self, record: &Record) -> Result<()>;
}

pub struct Router {
    backend: Box<dyn Backend>,
    data_dir: PathBuf,
    active: Mutex<Option<Active>>,
    ops: Mutex<()>,
    last_error: Mutex<Option<String>>,
    pub settings: Mutex<AudioSettings>,
}

impl Router {
    pub fn new(data_dir: PathBuf, settings: AudioSettings) -> Arc<Self> {
        #[cfg(windows)]
        let backend: Box<dyn Backend> = Box::new(windows::WinBackend { data_dir: data_dir.clone() });
        #[cfg(not(windows))]
        let backend: Box<dyn Backend> = Box::new(Simulated);
        Arc::new(Self {
            backend,
            data_dir,
            active: Mutex::new(None),
            ops: Mutex::new(()),
            last_error: Mutex::new(None),
            settings: Mutex::new(settings),
        })
    }

    fn record_path(&self) -> PathBuf {
        self.data_dir.join("audio-session.json")
    }

    fn fail(&self, e: &anyhow::Error) {
        tracing::error!("sortie audio virtuelle : {e:#}");
        *self.last_error.lock().unwrap() = Some(format!("{e:#}"));
    }

    fn allowed(&self, origin: Origin) -> bool {
        match self.settings.lock().unwrap().trigger {
            Trigger::Off => false,
            Trigger::Apps => origin != Origin::Auto,
            Trigger::All => true,
        }
    }

    /// Après un plantage ou une coupure : rétablit la sortie audio d'origine.
    pub fn recover(&self) {
        let Ok(text) = std::fs::read_to_string(self.record_path()) else { return };
        tracing::warn!("session audio interrompue trouvée : rétablissement de la sortie d'origine");
        if let Ok(record) = serde_json::from_str::<Record>(&text) {
            if let Err(e) = self.backend.restore(&record) {
                self.fail(&e);
            }
        }
        let _ = std::fs::remove_file(self.record_path());
    }

    /// Démarre (ou reprend) le routage. `requested` : canaux demandés par
    /// l'appareil (2, 6 ou 8 ; 0 = pas de son).
    pub fn start(&self, origin: Origin, requested: Option<u16>) -> Value {
        if !self.allowed(origin) || requested == Some(0) {
            return json!({ "routed": false });
        }
        let channels = self.settings.lock().unwrap().channels.count(requested);
        let _op = self.ops.lock().unwrap();
        {
            let mut active = self.active.lock().unwrap();
            if let Some(a) = active.as_mut() {
                if a.channels == channels {
                    a.last_seen = Instant::now();
                    a.origin = origin;
                    return json!({ "routed": true, "channels": a.channels });
                }
            }
        }
        self.stop_locked("autre nombre de canaux demandé");
        match self.backend.route(channels) {
            Ok((record, got)) => {
                let _ = std::fs::write(self.record_path(), serde_json::to_vec(&record).unwrap_or_default());
                tracing::info!("son envoyé vers les haut-parleurs virtuels, {got} canaux ({origin:?})");
                *self.last_error.lock().unwrap() = None;
                *self.active.lock().unwrap() = Some(Active {
                    origin,
                    channels: got,
                    record,
                    started_at: crate::display::now_secs(),
                    last_seen: Instant::now(),
                });
                json!({ "routed": true, "channels": got })
            }
            Err(e) => {
                self.fail(&e);
                json!({ "routed": false, "error": format!("{e:#}") })
            }
        }
    }

    pub fn heartbeat(&self) {
        if let Some(a) = self.active.lock().unwrap().as_mut() {
            a.last_seen = Instant::now();
        }
    }

    pub fn stop(&self, reason: &str) {
        let _op = self.ops.lock().unwrap();
        self.stop_locked(reason);
    }

    fn stop_locked(&self, reason: &str) {
        let Some(active) = self.active.lock().unwrap().take() else { return };
        tracing::info!("fin de la sortie audio virtuelle : {reason}");
        if let Err(e) = self.backend.restore(&active.record) {
            self.fail(&e);
        }
        let _ = std::fs::remove_file(self.record_path());
    }

    pub fn status(&self) -> Value {
        let active = self.active.lock().unwrap().as_ref().map(|a| {
            json!({ "origin": a.origin, "channels": a.channels, "started_at": a.started_at })
        });
        let devices = match self.backend.status() {
            Ok(v) => v,
            Err(e) => json!({ "error": format!("{e:#}") }),
        };
        json!({
            "supported": cfg!(windows),
            "settings": *self.settings.lock().unwrap(),
            "devices": devices,
            "session": active,
            "last_error": *self.last_error.lock().unwrap(),
        })
    }

    /// Surveillance (fil dédié) : fin de session, mode « pour toute connexion ».
    pub fn supervise(self: Arc<Self>) {
        std::thread::spawn(move || {
            let mut idle_since: Option<Instant> = None;
            loop {
                std::thread::sleep(Duration::from_secs(1));
                let encoding = crate::gpu::encoding();
                let state = self.active.lock().unwrap().as_ref().map(|a| (a.origin, a.last_seen.elapsed()));
                match state {
                    Some((origin, silent)) => {
                        idle_since = if encoding { None } else { idle_since.or(Some(Instant::now())) };
                        let idle_long = idle_since.is_some_and(|t| t.elapsed() > Duration::from_secs(10));
                        if origin == Origin::App && silent > Duration::from_secs(20) && !encoding {
                            self.stop("l'appareil ne donne plus de nouvelles");
                        } else if origin == Origin::Auto && idle_long {
                            self.stop("plus aucun flux en cours");
                        }
                    }
                    None => {
                        idle_since = None;
                        if encoding && self.settings.lock().unwrap().trigger == Trigger::All {
                            self.start(Origin::Auto, None);
                        }
                    }
                }
            }
        });
    }
}

/// Hors Windows : sorties simulées, pour développer l'interface sur Mac.
#[cfg(not(windows))]
struct Simulated;

#[cfg(not(windows))]
impl Backend for Simulated {
    fn status(&self) -> Result<Value> {
        Ok(json!({
            "speakers": { "id": "sim-speakers", "name": "Haut-parleurs (Steam Streaming Speakers)", "channels": 2 },
            "default": { "id": "sim-default", "name": "Haut-parleurs (Realtek(R) Audio)", "channels": 2 },
        }))
    }
    fn route(&self, channels: u16) -> Result<(Record, u16)> {
        tracing::info!("(simulé) son vers les haut-parleurs virtuels, {channels} canaux");
        Ok((Record { previous: "sim-default".into(), previous_format: None, speakers: "sim-speakers".into() }, channels))
    }
    fn restore(&self, _: &Record) -> Result<()> {
        tracing::info!("(simulé) sortie audio d'origine rétablie");
        Ok(())
    }
}

#[cfg(windows)]
mod windows {
    use super::*;
    use crate::display::agent;

    pub struct WinBackend {
        pub data_dir: PathBuf,
    }

    impl Backend for WinBackend {
        fn status(&self) -> Result<Value> {
            Ok(serde_json::to_value(endpoint::status()?)?)
        }
        fn route(&self, channels: u16) -> Result<(Record, u16)> {
            let v = agent::run(&self.data_dir, &["audio-route".into(), "--channels".into(), channels.to_string()])?;
            let got = v.get("channels").and_then(Value::as_u64).unwrap_or(u64::from(channels)) as u16;
            let record = Record {
                previous: v.get("previous").and_then(Value::as_str).unwrap_or_default().to_string(),
                previous_format: v.get("previous_format").and_then(Value::as_str).map(str::to_string),
                speakers: v.get("speakers").and_then(Value::as_str).unwrap_or_default().to_string(),
            };
            Ok((record, got))
        }
        fn restore(&self, record: &Record) -> Result<()> {
            let mut args = vec![
                "audio-restore".to_string(),
                "--previous".into(),
                record.previous.clone(),
                "--speakers".into(),
                record.speakers.clone(),
            ];
            if let Some(f) = &record.previous_format {
                args.push("--previous-format".into());
                args.push(f.clone());
            }
            agent::run(&self.data_dir, &args)?;
            Ok(())
        }
    }
}
