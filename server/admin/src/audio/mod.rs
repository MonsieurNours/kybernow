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
use std::sync::atomic::{AtomicBool, Ordering};
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
    /// Ce système a une sortie son virtuelle (Station Mac : le pilote KyberNow est installé).
    fn supported(&self) -> bool {
        true
    }
    /// Sortie son simulée (développement hors Windows et hors Station Mac).
    fn simulated(&self) -> bool {
        false
    }
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
    /// Un partage lancé depuis le PC est ouvert (KyberNow Share) : la sortie
    /// son de l'utilisateur ne change pas (« Pour toute connexion »).
    sharing: AtomicBool,
}

impl Router {
    pub fn new(data_dir: PathBuf, settings: AudioSettings) -> Arc<Self> {
        #[cfg(windows)]
        let backend: Box<dyn Backend> = Box::new(windows::WinBackend { data_dir: data_dir.clone() });
        // Station Mac : la sortie KyberNow (server/macos/audio-driver), par l'agent
        // `kybernow-display audio …` (docs/station-macos.md, jalon 4).
        #[cfg(target_os = "macos")]
        let backend: Box<dyn Backend> = match (crate::mac::is_station(), crate::display::mac_agent_path()) {
            (true, Some(agent)) => Box::new(mac::MacBackend { agent }),
            (true, None) => Box::new(Absent),
            (false, _) => Box::new(Simulated),
        };
        // Station Linux : une sortie PipeWire « KyberNow » le temps de la session
        // (docs/station-linux.md).
        #[cfg(target_os = "linux")]
        let backend: Box<dyn Backend> =
            if crate::linux::is_station() { Box::new(linux::LinuxBackend) } else { Box::new(Simulated) };
        #[cfg(all(not(windows), not(target_os = "macos"), not(target_os = "linux")))]
        let backend: Box<dyn Backend> = Box::new(Simulated);
        Arc::new(Self {
            backend,
            data_dir,
            active: Mutex::new(None),
            ops: Mutex::new(()),
            last_error: Mutex::new(None),
            settings: Mutex::new(settings),
            sharing: AtomicBool::new(false),
        })
    }

    /// Partage depuis le PC ouvert ou fermé (voir `sharing`).
    pub fn set_sharing(&self, on: bool) {
        self.sharing.store(on, Ordering::SeqCst);
    }

    pub fn sharing(&self) -> bool {
        self.sharing.load(Ordering::SeqCst)
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
        if !self.allowed(origin) || requested == Some(0) || !self.backend.supported() {
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
                let _ = crate::durable::write(&self.record_path(), serde_json::to_vec(&record).unwrap_or_default());
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

    /// Arrêt du service : la sortie d'origine revient tout de suite.
    pub fn shutdown(&self) {
        self.stop("arrêt de la console");
    }

    fn stop_locked(&self, reason: &str) {
        let Some(active) = self.active.lock().unwrap().take() else { return };
        tracing::info!("fin de la sortie audio virtuelle : {reason}");
        match self.backend.restore(&active.record) {
            Ok(()) => {
                let _ = std::fs::remove_file(self.record_path());
            }
            // La sortie d'origine n'est pas revenue : le fichier reste pour
            // que le prochain démarrage réessaie.
            Err(e) => self.fail(&e),
        }
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
            "supported": !self.backend.simulated() && self.backend.supported(),
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
                        // Partage depuis le PC : le son reste sur la sortie de l'utilisateur.
                        if encoding && self.settings.lock().unwrap().trigger == Trigger::All && !self.sharing.load(Ordering::SeqCst) {
                            self.start(Origin::Auto, None);
                        }
                    }
                }
            }
        });
    }
}

/// Hors Windows : sorties simulées, pour développer l'interface sur Mac.
/// Station Mac sans sortie son virtuelle : le son de l'écran capturé part tel
/// quel (Kyber le prend par ScreenCaptureKit), et rien n'est annoncé aux apps.
#[cfg(not(windows))]
#[cfg_attr(target_os = "linux", allow(dead_code))]
struct Absent;

#[cfg(not(windows))]
impl Backend for Absent {
    fn status(&self) -> Result<Value> {
        Ok(json!({ "speakers": null, "default": null }))
    }
    fn supported(&self) -> bool {
        false
    }
    fn route(&self, _: u16) -> Result<(Record, u16)> {
        anyhow::bail!("pas de sortie son virtuelle sur Mac")
    }
    fn restore(&self, _: &Record) -> Result<()> {
        Ok(())
    }
}

#[cfg(not(windows))]
struct Simulated;

#[cfg(not(windows))]
impl Backend for Simulated {
    fn simulated(&self) -> bool {
        true
    }
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

/// Station Mac : la sortie KyberNow (pilote Core Audio de server/macos/audio-driver),
/// réglée et choisie par l'agent `kybernow-display audio …`. `previous_format`
/// garde la sortie des alertes d'avant (Windows : le format d'origine).
#[cfg(target_os = "macos")]
mod mac {
    use std::path::{Path, PathBuf};
    use std::process::Command;

    use super::*;

    /// Le pilote installé (coreaudiod le charge au démarrage).
    const DRIVER: &str = "/Library/Audio/Plug-Ins/HAL/KyberNow.driver";

    pub struct MacBackend {
        pub agent: PathBuf,
    }

    impl MacBackend {
        fn run(&self, args: &[&str]) -> Result<Value> {
            let out = Command::new(&self.agent).arg("audio").args(args).output()?;
            let text = String::from_utf8_lossy(&out.stdout);
            let mut v: Value = serde_json::from_str(text.lines().last().unwrap_or_default())
                .map_err(|e| anyhow::anyhow!("réponse de l'agent du son illisible ({e}) : {text}"))?;
            if v["ok"] != json!(true) {
                anyhow::bail!("{}", v["error"].as_str().unwrap_or("agent du son en échec"));
            }
            v.as_object_mut().map(|o| o.remove("ok"));
            Ok(v)
        }
    }

    impl Backend for MacBackend {
        fn status(&self) -> Result<Value> {
            self.run(&["status"])
        }
        fn supported(&self) -> bool {
            Path::new(DRIVER).exists()
        }
        fn route(&self, channels: u16) -> Result<(Record, u16)> {
            let v = self.run(&["route", "--channels", &channels.to_string()])?;
            let got = v.get("channels").and_then(Value::as_u64).unwrap_or(u64::from(channels)) as u16;
            let text = |k: &str| v.get(k).and_then(Value::as_str).unwrap_or_default().to_string();
            let system = text("previous_system");
            Ok((
                Record { previous: text("previous"), previous_format: (!system.is_empty()).then_some(system), speakers: text("speakers") },
                got,
            ))
        }
        fn restore(&self, record: &Record) -> Result<()> {
            let mut args = vec!["restore", "--previous", record.previous.as_str()];
            if let Some(system) = &record.previous_format {
                args.extend(["--previous-system", system.as_str()]);
            }
            self.run(&args).map(|_| ())
        }
    }
}

/// Station Linux : le son de la session part vers une sortie « KyberNow »
/// créée pour l'occasion (module `module-null-sink` de PipeWire, par son
/// serveur compatible PulseAudio et `pactl`), au nombre de voies de
/// l'appareil ; elle devient la sortie par défaut, les sons en cours y sont
/// déplacés, et Kyber capture son retour (le moniteur de la sortie par
/// défaut). Rien ne sort des haut-parleurs de l'ordinateur pendant la
/// session. À la fin : sortie d'origine rendue, sons ramenés, module retiré.
/// `previous_format` garde le numéro du module.
#[cfg(target_os = "linux")]
mod linux {
    use std::process::Command;

    use super::*;

    const SINK: &str = "kybernow";

    pub struct LinuxBackend;

    fn pactl(args: &[&str]) -> Result<String> {
        let out = Command::new("pactl").args(args).output().map_err(|e| anyhow::anyhow!("pactl : {e}"))?;
        anyhow::ensure!(out.status.success(), "pactl {} : {}", args.join(" "), String::from_utf8_lossy(&out.stderr).trim());
        Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
    }

    /// Ordre des voies de PulseAudio pour 2, 6 et 8 voies.
    pub(super) fn channel_map(channels: u16) -> &'static str {
        match channels {
            6 => "front-left,front-right,front-center,lfe,rear-left,rear-right",
            8 => "front-left,front-right,front-center,lfe,rear-left,rear-right,side-left,side-right",
            _ => "front-left,front-right",
        }
    }

    fn sink_inputs() -> Vec<String> {
        pactl(&["list", "short", "sink-inputs"])
            .map(|t| t.lines().filter_map(|l| l.split_whitespace().next().map(str::to_string)).collect())
            .unwrap_or_default()
    }

    fn move_all(to: &str) {
        for input in sink_inputs() {
            let _ = pactl(&["move-sink-input", &input, to]);
        }
    }

    /// Modules « kybernow » restés d'une session interrompue.
    fn unload_ours() {
        if let Ok(list) = pactl(&["list", "short", "modules"]) {
            for line in list.lines().filter(|l| l.contains("module-null-sink") && l.contains(&format!("sink_name={SINK}"))) {
                if let Some(id) = line.split_whitespace().next() {
                    let _ = pactl(&["unload-module", id]);
                }
            }
        }
    }

    impl Backend for LinuxBackend {
        fn status(&self) -> Result<Value> {
            let default = pactl(&["get-default-sink"]).ok();
            // La sortie KyberNow est créée pour chaque session : elle est là
            // dès que le serveur de son répond.
            let ours = pactl(&["info"]).is_ok();
            Ok(json!({
                "speakers": ours.then(|| json!({ "id": SINK, "name": "KyberNow", "channels": 8 })),
                "default": default.map(|d| json!({ "id": d, "name": d, "channels": Value::Null })),
            }))
        }
        fn supported(&self) -> bool {
            pactl(&["info"]).is_ok()
        }
        fn route(&self, channels: u16) -> Result<(Record, u16)> {
            let previous = pactl(&["get-default-sink"])?;
            unload_ours();
            let module = pactl(&[
                "load-module",
                "module-null-sink",
                &format!("sink_name={SINK}"),
                &format!("channels={channels}"),
                &format!("channel_map={}", channel_map(channels)),
                "rate=48000",
                "sink_properties=device.description=KyberNow",
            ])?;
            pactl(&["set-default-sink", SINK])?;
            move_all(SINK);
            tracing::info!("son vers la sortie KyberNow ({channels} voies, module {module}), sortie d'avant : {previous}");
            Ok((Record { previous, previous_format: Some(module), speakers: SINK.into() }, channels))
        }
        fn restore(&self, record: &Record) -> Result<()> {
            if !record.previous.is_empty() && record.previous != SINK {
                let _ = pactl(&["set-default-sink", &record.previous]);
                move_all(&record.previous);
            }
            match &record.previous_format {
                Some(module) if !module.is_empty() => {
                    let _ = pactl(&["unload-module", module]);
                }
                _ => {}
            }
            unload_ours();
            tracing::info!("sortie son d'origine rendue ({})", record.previous);
            Ok(())
        }
    }

    #[cfg(test)]
    mod tests {
        #[test]
        fn channel_orders() {
            assert_eq!(super::channel_map(2), "front-left,front-right");
            assert_eq!(super::channel_map(6).split(',').count(), 6);
            assert_eq!(super::channel_map(8).split(',').count(), 8);
        }
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
