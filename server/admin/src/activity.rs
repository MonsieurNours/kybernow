//! Ce que la console sait des appareils et de ce qui s'est passé : les
//! appareils qui se sont connectés (`devices.json`), la session en cours,
//! et le fil des événements (`events.json`, les 300 derniers), écrits en
//! phrases pour la vue d'ensemble.
//!
//! Les apps KyberNow se présentent en début de session (`/api/stream/prepare`),
//! donnent leur latence avec leur signal de vie, et la latence de toute la
//! session à la fin. La console peut demander la fin d'une session : l'app
//! l'apprend au signal de vie suivant et se déconnecte proprement.

use std::collections::VecDeque;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::display::now_secs;

const DEVICES_FILE: &str = "devices.json";
const EVENTS_FILE: &str = "events.json";
const MAX_EVENTS: usize = 300;
const MAX_DEVICES: usize = 50;

/// Ce qu'une app dit d'elle-même en début de session.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct DeviceInfo {
    /// Identifiant stable, tiré par l'app à sa première ouverture.
    pub id: String,
    /// « MacBook Pro de Matthieu »
    pub name: String,
    /// mac, tv, phone, tablet, browser, other
    #[serde(default)]
    pub kind: Option<String>,
    /// « macOS 26.0 »
    #[serde(default)]
    pub os: Option<String>,
    /// « KyberNow 0.2.1 »
    #[serde(default)]
    pub app: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct ModeInfo {
    pub width: u32,
    pub height: u32,
    pub refresh: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Device {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub os: Option<String>,
    pub app: Option<String>,
    /// Compte Kyber de la dernière session.
    pub user: String,
    pub first_seen: u64,
    pub last_seen: u64,
    pub sessions: u32,
    pub last_mode: Option<ModeInfo>,
    /// Latence médiane de la dernière session, en millisecondes.
    pub latency_ms: Option<f64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Live {
    pub device_id: String,
    pub name: String,
    pub kind: String,
    pub user: String,
    pub started_at: u64,
    pub mode: Option<ModeInfo>,
    pub profile: Option<String>,
    /// Dernière latence médiane annoncée par l'app.
    pub latency_ms: Option<f64>,
    pub end_requested: bool,
    /// Dernier signal de vie (secondes).
    #[serde(skip)]
    last_beat: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LastSession {
    pub device_id: String,
    pub name: String,
    pub started_at: u64,
    pub ended_at: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Event {
    pub at: u64,
    /// « info » ou « warn »
    pub kind: String,
    pub text: String,
}

#[derive(Default, Serialize, Deserialize)]
struct Stored {
    devices: Vec<Device>,
    last: Option<LastSession>,
}

struct State {
    stored: Stored,
    events: VecDeque<Event>,
    live: Option<Live>,
}

pub struct Activity {
    dir: PathBuf,
    state: Mutex<State>,
}

impl Activity {
    pub fn new(data_dir: &Path) -> Self {
        let stored = fs::read(data_dir.join(DEVICES_FILE))
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default();
        let events = fs::read(data_dir.join(EVENTS_FILE))
            .ok()
            .and_then(|b| serde_json::from_slice::<VecDeque<Event>>(&b).ok())
            .unwrap_or_default();
        Self { dir: data_dir.to_path_buf(), state: Mutex::new(State { stored, events, live: None }) }
    }

    fn save_devices(&self, s: &State) {
        if let Ok(bytes) = serde_json::to_vec_pretty(&s.stored) {
            let _ = fs::write(self.dir.join(DEVICES_FILE), bytes);
        }
    }

    fn save_events(&self, s: &State) {
        if let Ok(bytes) = serde_json::to_vec(&s.events) {
            let _ = fs::write(self.dir.join(EVENTS_FILE), bytes);
        }
    }

    fn push(&self, s: &mut State, kind: &str, text: String) {
        tracing::info!("événement : {text}");
        s.events.push_back(Event { at: now_secs(), kind: kind.to_string(), text });
        while s.events.len() > MAX_EVENTS {
            s.events.pop_front();
        }
        self.save_events(s);
    }

    /// Un événement pour le fil de la vue d'ensemble.
    pub fn event(&self, kind: &str, text: impl Into<String>) {
        let mut s = self.state.lock().unwrap();
        self.push(&mut s, kind, text.into());
    }

    /// Une app ouvre une session.
    pub fn begin(&self, info: Option<DeviceInfo>, user: &str, mode: Option<ModeInfo>, profile: Option<String>) {
        let mut s = self.state.lock().unwrap();
        let now = now_secs();
        // Sans présentation (app d'avant 0.2.1) : un appareil par compte.
        let info = info.filter(|i| !i.id.trim().is_empty() && !i.name.trim().is_empty()).unwrap_or_else(|| DeviceInfo {
            id: format!("compte:{user}"),
            name: format!("Un appareil ({user})"),
            ..Default::default()
        });
        let kind = info.kind.clone().unwrap_or_else(|| "other".into());
        // Même appareil déjà en session : il redemande son écran après un
        // redémarrage de Kyber. La session continue.
        if let Some(live) = s.live.as_mut().filter(|l| l.device_id == info.id) {
            live.last_beat = now;
            if mode.is_some() {
                live.mode = mode;
            }
            live.profile = profile.or(live.profile.take());
            return;
        }
        match s.stored.devices.iter_mut().find(|d| d.id == info.id) {
            Some(d) => {
                d.name = info.name.clone();
                d.kind = kind.clone();
                d.os = info.os.clone().or(d.os.take());
                d.app = info.app.clone().or(d.app.take());
                d.user = user.to_string();
                d.last_seen = now;
                d.sessions += 1;
                if mode.is_some() {
                    d.last_mode = mode;
                }
            }
            None => s.stored.devices.push(Device {
                id: info.id.clone(),
                name: info.name.clone(),
                kind: kind.clone(),
                os: info.os.clone(),
                app: info.app.clone(),
                user: user.to_string(),
                first_seen: now,
                last_seen: now,
                sessions: 1,
                last_mode: mode,
                latency_ms: None,
            }),
        }
        // Les plus anciens partent au-delà de 50 appareils.
        s.stored.devices.sort_by(|a, b| b.last_seen.cmp(&a.last_seen));
        s.stored.devices.truncate(MAX_DEVICES);
        s.live = Some(Live {
            device_id: info.id.clone(),
            name: info.name.clone(),
            kind,
            user: user.to_string(),
            started_at: now,
            mode,
            profile,
            latency_ms: None,
            end_requested: false,
            last_beat: now,
        });
        self.save_devices(&s);
        let text = match mode {
            Some(m) => format!("{} s'est connecté, écran de {} × {} à {} Hz", info.name, m.width, m.height, m.refresh),
            None => format!("{} s'est connecté", info.name),
        };
        self.push(&mut s, "info", text);
    }

    /// Signal de vie d'une app : sa latence, son profil. Vrai si la console
    /// demande la fin de la session.
    pub fn heartbeat(&self, user: &str, latency_ms: Option<f64>, profile: Option<String>) -> bool {
        let mut s = self.state.lock().unwrap();
        let Some(live) = s.live.as_mut().filter(|l| l.user == user) else { return false };
        live.last_beat = now_secs();
        if latency_ms.is_some() {
            live.latency_ms = latency_ms;
        }
        if profile.is_some() {
            live.profile = profile;
        }
        live.end_requested
    }

    /// Fin de session (l'app se déconnecte), avec sa latence médiane.
    pub fn end(&self, user: &str, latency_ms: Option<f64>) {
        let mut s = self.state.lock().unwrap();
        let Some(live) = s.live.take() else { return };
        if live.user != user {
            s.live = Some(live);
            return;
        }
        let now = now_secs();
        let latency = latency_ms.or(live.latency_ms);
        if let Some(d) = s.stored.devices.iter_mut().find(|d| d.id == live.device_id) {
            d.last_seen = now;
            if latency.is_some() {
                d.latency_ms = latency;
            }
        }
        s.stored.last = Some(LastSession {
            device_id: live.device_id.clone(),
            name: live.name.clone(),
            started_at: live.started_at,
            ended_at: now,
        });
        self.save_devices(&s);
        let mut text = format!("{} s'est déconnecté après {}", live.name, duration(now.saturating_sub(live.started_at)));
        if let Some(ms) = latency {
            text.push_str(&format!(", écart médian {}", milliseconds(ms)));
        }
        self.push(&mut s, "info", text);
    }

    /// La console demande la fin de la session d'une app. Faux sans session d'app.
    pub fn request_end(&self) -> bool {
        let mut s = self.state.lock().unwrap();
        let Some(live) = s.live.as_mut() else { return false };
        live.end_requested = true;
        let name = live.name.clone();
        self.push(&mut s, "info", format!("Fin de la session de {name} demandée depuis la console"));
        true
    }

    /// Session d'app en cours, perdue sans fin propre (signal de vie arrêté).
    pub fn drop_live(&self, reason: &str) {
        let mut s = self.state.lock().unwrap();
        if let Some(live) = s.live.take() {
            s.stored.last = Some(LastSession {
                device_id: live.device_id.clone(),
                name: live.name.clone(),
                started_at: live.started_at,
                ended_at: now_secs(),
            });
            self.save_devices(&s);
            self.push(&mut s, "warn", format!("Session de {} interrompue : {reason}", live.name));
        }
    }

    pub fn live(&self) -> Option<Live> {
        self.expire();
        self.state.lock().unwrap().live.clone()
    }

    /// Plus de signal de vie depuis 30 s : l'app est partie sans prévenir.
    fn expire(&self) {
        let stale = self.state.lock().unwrap().live.as_ref().is_some_and(|l| now_secs().saturating_sub(l.last_beat) > 30);
        if stale {
            self.drop_live("plus de signal de l'appareil");
        }
    }

    pub fn summary(&self) -> Value {
        self.expire();
        let s = self.state.lock().unwrap();
        json!({ "live": s.live, "last": s.stored.last })
    }

    pub fn devices(&self) -> Vec<Device> {
        self.state.lock().unwrap().stored.devices.clone()
    }

    pub fn forget(&self, id: &str) -> bool {
        let mut s = self.state.lock().unwrap();
        let before = s.stored.devices.len();
        s.stored.devices.retain(|d| d.id != id);
        let removed = s.stored.devices.len() != before;
        if removed {
            self.save_devices(&s);
        }
        removed
    }

    /// Les `limit` derniers événements, du plus récent au plus ancien.
    pub fn events(&self, limit: usize) -> Vec<Event> {
        self.state.lock().unwrap().events.iter().rev().take(limit).cloned().collect()
    }
}

fn duration(seconds: u64) -> String {
    let minutes = seconds / 60;
    match minutes {
        0 => "moins d'une minute".into(),
        1 => "1 minute".into(),
        m if m < 60 => format!("{m} minutes"),
        m => format!("{} h {:02}", m / 60, m % 60),
    }
}

fn milliseconds(ms: f64) -> String {
    if ms >= 100.0 { format!("{} ms", ms.round() as u64) } else { format!("{:.1} ms", ms).replace('.', ",") }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp() -> PathBuf {
        let dir = std::env::temp_dir().join(format!("kn-activity-{}", rand::random::<u64>()));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn session_d_une_app() {
        let dir = temp();
        let a = Activity::new(&dir);
        let mac = DeviceInfo { id: "abc".into(), name: "MacBook Pro".into(), kind: Some("mac".into()), os: Some("macOS 26".into()), app: None };
        a.begin(Some(mac.clone()), "matthieu", Some(ModeInfo { width: 3456, height: 2234, refresh: 120 }), Some("Bureau".into()));
        assert_eq!(a.live().unwrap().name, "MacBook Pro");
        assert!(!a.heartbeat("matthieu", Some(8.3), None));
        assert!(a.request_end());
        assert!(a.heartbeat("matthieu", None, None));
        a.end("matthieu", Some(8.1));
        assert!(a.live().is_none());
        let d = &a.devices()[0];
        assert_eq!((d.sessions, d.latency_ms), (1, Some(8.1)));
        // Relu depuis le disque.
        let again = Activity::new(&dir);
        assert_eq!(again.devices().len(), 1);
        assert!(again.events(10)[0].text.contains("déconnecté"));
        a.begin(Some(mac), "matthieu", None, None);
        assert_eq!(a.devices()[0].sessions, 2);
        assert!(a.forget("abc"));
        assert!(a.devices().is_empty());
    }

    #[test]
    fn app_sans_presentation() {
        let a = Activity::new(&temp());
        a.begin(None, "lea", None, None);
        assert_eq!(a.devices()[0].id, "compte:lea");
        a.end("autre", None);
        assert!(a.live().is_some(), "un autre compte ne termine pas la session");
    }
}
