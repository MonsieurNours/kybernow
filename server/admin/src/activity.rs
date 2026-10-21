//! Ce que la console sait des appareils et de ce qui s'est passé : les
//! appareils qui se sont connectés (`devices.json`), la session en cours,
//! et le fil des événements (`events.json`, les 300 derniers), écrits en
//! phrases pour la vue d'ensemble.
//!
//! Un événement garde sa clé et ses paramètres (`key`, `params`, voir
//! `i18n::Msg`) : la phrase est écrite à la lecture, dans la langue de qui
//! regarde. Il garde aussi sa phrase française (`text`), que relisent les
//! consoles d'avant les deux langues (retour à une version précédente) ; les
//! événements enregistrés avant n'ont qu'elle et s'affichent tels quels.
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
use crate::i18n::{self, Lang, Msg};

/// Sans signal de vie d'une app pendant ce temps (il part toutes les 5 s),
/// la console tient sa session pour interrompue : le fil d'événements la
/// note, l'écran virtuel de l'app est rendu et la veille du plein écran
/// s'arrête. Même délai partout (`activity`, `display`, `foreground`) : un
/// appareil n'est jamais encore « en session » pour l'un et plus pour l'autre.
pub const HEARTBEAT_GRACE: std::time::Duration = std::time::Duration::from_secs(20);

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
    /// `user` est le compte propre à l'appareil, créé à son appairage par code :
    /// le retirer supprime ce compte.
    #[serde(default)]
    pub own_account: bool,
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
    /// Nom de l'appareil qui prend la main (la fin a été demandée pour lui).
    #[serde(default)]
    pub taking_over: Option<String>,
    /// Dernier signal de vie (secondes).
    #[serde(skip)]
    last_beat: u64,
}

/// Réponse à un appareil qui veut ouvrir une session.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Claim {
    /// Personne d'autre en session (ou le même appareil) : il peut commencer.
    Free,
    /// Un autre appareil est en session : on lui a demandé de finir, il faut l'attendre.
    Waiting { previous: String },
}

/// Identité d'un appareil : son identifiant et son nom. Sans présentation
/// (app d'avant 0.2.1), un appareil par compte.
pub fn identity(info: Option<&DeviceInfo>, user: &str) -> (String, String) {
    match info.filter(|i| !i.id.trim().is_empty() && !i.name.trim().is_empty()) {
        Some(i) => (i.id.clone(), i.name.clone()),
        None => (format!("compte:{user}"), format!("Un appareil ({user})")),
    }
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
    /// La phrase en français (seule gardée par les consoles d'avant 0.3.0).
    pub text: String,
    /// Clé du message (`events.connected`…) et ses paramètres : la phrase
    /// est réécrite à la lecture, dans la langue demandée.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub key: Option<String>,
    #[serde(default, skip_serializing_if = "serde_json::Map::is_empty")]
    pub params: serde_json::Map<String, Value>,
}

impl Event {
    /// La phrase dans `lang` ; un événement d'avant les deux langues garde son texte.
    pub fn in_lang(mut self, lang: Lang) -> Event {
        if let Some(key) = &self.key {
            self.text = i18n::render(lang, key, &self.params);
        }
        self
    }
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

    fn push(&self, s: &mut State, kind: &str, msg: Msg) {
        let text = msg.to_string();
        tracing::info!("événement : {text}");
        s.events.push_back(Event { at: now_secs(), kind: kind.to_string(), text, key: Some(msg.key), params: msg.params });
        while s.events.len() > MAX_EVENTS {
            s.events.pop_front();
        }
        self.save_events(s);
    }

    /// Un événement pour le fil de la vue d'ensemble.
    pub fn event(&self, kind: &str, msg: Msg) {
        let mut s = self.state.lock().unwrap();
        self.push(&mut s, kind, msg);
    }

    /// Un appareil veut ouvrir une session. Si un autre appareil est en
    /// session, la console lui demande de finir (il l'apprend à son signal de
    /// vie suivant) : l'arrivant doit attendre qu'il ait rendu la main.
    pub fn claim(&self, device_id: &str, newcomer: &str) -> Claim {
        self.expire();
        let mut s = self.state.lock().unwrap();
        let Some(live) = s.live.as_mut() else { return Claim::Free };
        if live.device_id == device_id {
            return Claim::Free;
        }
        let previous = live.name.clone();
        let already = live.taking_over.as_deref() == Some(newcomer);
        live.end_requested = true;
        live.taking_over = Some(newcomer.to_string());
        if !already {
            self.push(&mut s, "info", Msg::new("events.takeover").with("newcomer", newcomer).with("previous", previous.as_str()));
        }
        Claim::Waiting { previous }
    }

    /// Identifiant de l'appareil en session, s'il y en a un.
    pub fn live_device_id(&self) -> Option<String> {
        self.expire();
        self.state.lock().unwrap().live.as_ref().map(|l| l.device_id.clone())
    }

    /// Une app ouvre une session.
    pub fn begin(&self, info: Option<DeviceInfo>, user: &str, mode: Option<ModeInfo>, profile: Option<String>) {
        let mut s = self.state.lock().unwrap();
        let now = now_secs();
        let (id, name) = identity(info.as_ref(), user);
        let info = DeviceInfo { id, name, ..info.unwrap_or_default() };
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
                // Connecté avec un autre compte que le sien : ce n'est plus son accès.
                if d.user != user {
                    d.own_account = false;
                }
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
                own_account: false,
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
            taking_over: None,
            last_beat: now,
        });
        self.save_devices(&s);
        let msg = match mode {
            Some(m) => Msg::new("events.connectedMode")
                .with("name", info.name.as_str())
                .with("width", m.width)
                .with("height", m.height)
                .with("refresh", m.refresh),
            None => Msg::new("events.connected").with("name", info.name.as_str()),
        };
        self.push(&mut s, "info", msg);
    }

    /// La session en cours est-elle celle de cet appareil ? Les apps à partir
    /// de 0.2.5 donnent leur identifiant ; avant, on ne connaît que le compte.
    fn is_live(live: &Live, user: &str, device_id: Option<&str>) -> bool {
        match device_id {
            Some(id) => live.device_id == id,
            None => live.user == user,
        }
    }

    /// Signal de vie d'une app : sa latence, son profil. Vrai si la console
    /// demande la fin de la session (depuis la console, ou pour un autre appareil).
    pub fn heartbeat(&self, user: &str, device_id: Option<&str>, latency_ms: Option<f64>, profile: Option<String>) -> bool {
        let mut s = self.state.lock().unwrap();
        let Some(live) = s.live.as_mut().filter(|l| Self::is_live(l, user, device_id)) else { return false };
        live.last_beat = now_secs();
        if latency_ms.is_some() {
            live.latency_ms = latency_ms;
        }
        if profile.is_some() {
            live.profile = profile;
        }
        live.end_requested
    }

    /// Fin de session (l'app se déconnecte), avec sa latence médiane. Vrai si
    /// c'était bien la session en cours : un autre appareil qui se déconnecte
    /// ne touche pas à celle qui a pris la main.
    pub fn end(&self, user: &str, device_id: Option<&str>, latency_ms: Option<f64>) -> bool {
        let mut s = self.state.lock().unwrap();
        let Some(live) = s.live.take() else { return false };
        if !Self::is_live(&live, user, device_id) {
            s.live = Some(live);
            return false;
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
        let msg = Msg::new("events.disconnected")
            .with("name", live.name.as_str())
            .with("duration", json!({ "duration": now.saturating_sub(live.started_at) }))
            .with("latency", latency.map_or(json!(""), |ms| Msg::new("events.latencySuffix").with("ms", json!({ "ms": ms })).to_value()))
            .with("takeover", live.taking_over.map_or(json!(""), |next| Msg::new("events.takeoverSuffix").with("next", next).to_value()));
        self.push(&mut s, "info", msg);
        true
    }

    /// La console demande la fin de la session d'une app. Faux sans session d'app.
    pub fn request_end(&self) -> bool {
        self.expire();
        let mut s = self.state.lock().unwrap();
        let Some(live) = s.live.as_mut() else { return false };
        live.end_requested = true;
        let name = live.name.clone();
        self.push(&mut s, "info", Msg::new("events.endRequested").with("name", name));
        true
    }

    /// Session d'app en cours, perdue sans fin propre (signal de vie arrêté) :
    /// `reason` complète la phrase (« plus de signal de l'appareil »).
    pub fn drop_live(&self, reason: Msg) {
        let mut s = self.state.lock().unwrap();
        if let Some(live) = s.live.take() {
            s.stored.last = Some(LastSession {
                device_id: live.device_id.clone(),
                name: live.name.clone(),
                started_at: live.started_at,
                ended_at: now_secs(),
            });
            self.save_devices(&s);
            self.push(&mut s, "warn", Msg::new("events.interrupted").with("name", live.name.as_str()).with_msg("reason", reason));
        }
    }

    pub fn live(&self) -> Option<Live> {
        self.expire();
        self.state.lock().unwrap().live.clone()
    }

    /// Plus de signal de vie depuis `HEARTBEAT_GRACE` : l'app est partie sans prévenir.
    fn expire(&self) {
        let stale = self.state.lock().unwrap().live.as_ref().is_some_and(|l| now_secs().saturating_sub(l.last_beat) > HEARTBEAT_GRACE.as_secs());
        if stale {
            self.drop_live(Msg::new("events.noSignal"));
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

    /// Un appareil appairé par code reçoit son compte propre ; il apparaît
    /// dans la liste avant sa première session. Rend son ancien compte propre,
    /// à supprimer, s'il en avait un autre.
    pub fn paired(&self, info: &DeviceInfo, user: &str) -> Option<String> {
        let mut s = self.state.lock().unwrap();
        let now = now_secs();
        let kind = info.kind.clone().unwrap_or_else(|| "other".into());
        let previous = match s.stored.devices.iter_mut().find(|d| d.id == info.id) {
            Some(d) => {
                let previous = (d.own_account && d.user != user).then(|| d.user.clone());
                d.name = info.name.clone();
                d.kind = kind;
                d.os = info.os.clone().or(d.os.take());
                d.app = info.app.clone().or(d.app.take());
                d.user = user.to_string();
                d.own_account = true;
                d.last_seen = now;
                previous
            }
            None => {
                s.stored.devices.push(Device {
                    id: info.id.clone(),
                    name: info.name.clone(),
                    kind,
                    os: info.os.clone(),
                    app: info.app.clone(),
                    user: user.to_string(),
                    first_seen: now,
                    last_seen: now,
                    sessions: 0,
                    last_mode: None,
                    latency_ms: None,
                    own_account: true,
                });
                None
            }
        };
        s.stored.devices.sort_by(|a, b| b.last_seen.cmp(&a.last_seen));
        s.stored.devices.truncate(MAX_DEVICES);
        self.save_devices(&s);
        self.push(&mut s, "info", Msg::new("events.paired").with("name", info.name.as_str()));
        previous
    }

    /// Retire un appareil de la liste ; rend ce qu'on en savait.
    pub fn forget(&self, id: &str) -> Option<Device> {
        let mut s = self.state.lock().unwrap();
        let index = s.stored.devices.iter().position(|d| d.id == id)?;
        let device = s.stored.devices.remove(index);
        self.save_devices(&s);
        Some(device)
    }

    /// Compte propre à un appareil appairé → nom de l'appareil.
    pub fn own_accounts(&self) -> Vec<(String, String)> {
        let s = self.state.lock().unwrap();
        s.stored.devices.iter().filter(|d| d.own_account).map(|d| (d.user.clone(), d.name.clone())).collect()
    }

    /// Les `limit` derniers événements, du plus récent au plus ancien, écrits en `lang`.
    pub fn events(&self, limit: usize, lang: Lang) -> Vec<Event> {
        self.state.lock().unwrap().events.iter().rev().take(limit).cloned().map(|e| e.in_lang(lang)).collect()
    }
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
        assert!(!a.heartbeat("matthieu", Some("abc"), Some(8.3), None));
        assert!(a.request_end());
        assert!(a.heartbeat("matthieu", Some("abc"), None, None));
        assert!(a.end("matthieu", Some("abc"), Some(8.1)));
        assert!(a.live().is_none());
        let d = &a.devices()[0];
        assert_eq!((d.sessions, d.latency_ms), (1, Some(8.1)));
        // Relu depuis le disque.
        let again = Activity::new(&dir);
        assert_eq!(again.devices().len(), 1);
        assert!(again.events(10, Lang::Fr)[0].text.contains("déconnecté"));
        assert!(again.events(10, Lang::En)[0].text.contains("disconnected after less than a minute, median latency 8.1 ms"));
        a.begin(Some(mac), "matthieu", None, None);
        assert_eq!(a.devices()[0].sessions, 2);
        assert!(a.forget("abc").is_some());
        assert!(a.devices().is_empty());
        assert!(a.forget("abc").is_none());
    }

    #[test]
    fn appairage_par_code() {
        let a = Activity::new(&temp());
        let tv = DeviceInfo { id: "tv1".into(), name: "Apple TV".into(), kind: Some("tv".into()), os: None, app: None };
        assert_eq!(a.paired(&tv, "Apple-TV-0001"), None);
        let d = &a.devices()[0];
        assert_eq!((d.sessions, d.own_account, d.user.as_str()), (0, true, "Apple-TV-0001"));
        assert_eq!(a.own_accounts(), vec![("Apple-TV-0001".to_string(), "Apple TV".to_string())]);
        // Appairé de nouveau : l'ancien compte est à supprimer.
        assert_eq!(a.paired(&tv, "Apple-TV-0002").as_deref(), Some("Apple-TV-0001"));
        a.begin(Some(tv.clone()), "Apple-TV-0002", None, None);
        assert!(a.devices()[0].own_account);
        assert!(a.end("Apple-TV-0002", Some("tv1"), None));
        // Connecté avec le compte de la console : ce n'est plus son accès.
        a.begin(Some(tv), "matthieu", None, None);
        assert!(!a.devices()[0].own_account);
        assert!(a.own_accounts().is_empty());
    }

    #[test]
    fn app_sans_presentation() {
        let a = Activity::new(&temp());
        a.begin(None, "lea", None, None);
        assert_eq!(a.devices()[0].id, "compte:lea");
        assert!(!a.end("autre", None, None));
        assert!(a.live().is_some(), "un autre compte ne termine pas la session");
        // Sans identifiant d'appareil, le compte suffit (apps d'avant 0.2.5).
        assert!(!a.heartbeat("lea", None, None, None));
        assert!(a.end("lea", None, None));
    }

    #[test]
    fn reprise_de_main_avec_preavis() {
        let a = Activity::new(&temp());
        let mac = DeviceInfo { id: "mac".into(), name: "MacBook".into(), kind: Some("mac".into()), os: None, app: None };
        let tv = DeviceInfo { id: "tv".into(), name: "Apple TV".into(), kind: Some("tv".into()), os: None, app: None };
        a.begin(Some(mac.clone()), "matthieu", None, None);
        // Le même appareil qui redemande son écran : rien ne change.
        assert_eq!(a.claim("mac", "MacBook"), Claim::Free);
        // Un autre appareil (même compte) : on demande au premier de finir.
        assert_eq!(a.claim("tv", "Apple TV"), Claim::Waiting { previous: "MacBook".into() });
        assert_eq!(a.live().unwrap().taking_over.as_deref(), Some("Apple TV"));
        // Le premier l'apprend à son signal de vie ; le second n'est pas encore en session.
        assert!(a.heartbeat("matthieu", Some("mac"), None, None));
        assert!(!a.heartbeat("matthieu", Some("tv"), None, None));
        // Une fin de session venue du second ne touche pas à celle du premier.
        assert!(!a.end("matthieu", Some("tv"), None));
        assert!(a.live().is_some());
        // Le premier rend la main ; le second commence.
        assert!(a.end("matthieu", Some("mac"), None));
        assert_eq!(a.live_device_id(), None);
        assert_eq!(a.claim("tv", "Apple TV"), Claim::Free);
        a.begin(Some(tv), "matthieu", None, None);
        assert_eq!(a.live_device_id().as_deref(), Some("tv"));
        assert!(a.events(3, Lang::Fr).iter().any(|e| e.text.contains("prend la main")));
        assert!(a.events(3, Lang::En).iter().any(|e| e.text == "MacBook disconnected after less than a minute; Apple TV is taking over"));
    }

    #[test]
    fn fil_d_avant_les_deux_langues() {
        let dir = temp();
        // events.json d'une console 0.2.9 : la phrase seule.
        fs::write(dir.join(EVENTS_FILE), r#"[{"at":1,"kind":"info","text":"Compte lea ajouté"}]"#).unwrap();
        let a = Activity::new(&dir);
        assert_eq!(a.events(5, Lang::En)[0].text, "Compte lea ajouté");
        a.event("info", Msg::new("events.accountAdded").with("username", "max"));
        let e = &a.events(5, Lang::En)[0];
        assert_eq!((e.text.as_str(), e.key.as_deref()), ("Account max added", Some("events.accountAdded")));
        // Sur le disque : la phrase française reste, à côté de la clé et des paramètres.
        let stored: Vec<Value> = serde_json::from_slice(&fs::read(dir.join(EVENTS_FILE)).unwrap()).unwrap();
        assert_eq!(stored[1], json!({ "at": stored[1]["at"], "kind": "info", "text": "Compte max ajouté", "key": "events.accountAdded", "params": { "username": "max" } }));
    }
}
