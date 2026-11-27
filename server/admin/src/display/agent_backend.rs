//! Les écrans d'une Station Mac ou Linux, par l'agent `kybernow-display`
//! (STCOM-06) : un seul `Backend` pour les deux, qui parle à l'agent par
//! lignes JSON avec le même protocole (`agent_pipe.rs` : délais, agent muet
//! arrêté puis relancé). `mac.rs` et `linux.rs` ne gardent que ce qui leur est
//! propre : le démarrage (reprise des écrans du Mac éteints ; agent lancé tout
//! de suite sous Linux, puisqu'il tient la capture des écrans pour Kyber), le
//! HDR (Mac seulement) et l'arrêt de l'agent sans écran virtuel (Mac).
//!
//! Nom d'écran (`gdi_name` du reste du module) : `mac:<CGDirectDisplayID>` ou
//! `linux:<identifiant Kyber>`. Échelle 2 (Retina sur Mac) dès 2048 pixels de
//! large ([`hidpi`]).

use std::path::PathBuf;
use std::sync::Mutex;
use std::time::Duration;

use anyhow::{Context, Result, bail};
use serde_json::{Value, json};

use super::agent_pipe::{AgentPipe, CALL_TIMEOUT, OBSERVE_TIMEOUT};
use super::{Backend, Created, MAX_SCREENS, Mode, Observed, PhysicalDisplay, PointerRecord, Screen, hdr, slot_guid};

/// Variable d'environnement : chemin de l'agent, s'il n'est pas à côté d'Admin.
pub const AGENT_ENV: &str = "KYBERNOW_DISPLAY_AGENT";

/// Échelle 2 (Retina, la moitié en points) dès cette largeur : une interface
/// lisible sur un téléphone, une tablette, un Mac ou un téléviseur 4K ; un
/// écran 1080p reste à l'échelle 1.
const HIDPI_FROM_WIDTH: u32 = 2048;

pub fn hidpi(width: u32) -> bool {
    width >= HIDPI_FROM_WIDTH
}

/// Chemin de l'agent : `KYBERNOW_DISPLAY_AGENT`, sinon à côté de kyber-admin.
pub fn agent_path() -> Option<PathBuf> {
    let path = std::env::var_os(AGENT_ENV)
        .filter(|p| !p.is_empty())
        .map(PathBuf::from)
        .or_else(|| std::env::current_exe().ok().and_then(|e| e.parent().map(|d| d.join("kybernow-display"))))?;
    path.is_file().then_some(path)
}

/// L'agent, sans réponse, s'arrête de lui-même après `restore` : attendu ce temps au plus.
const FINISH_WITHIN: Duration = Duration::from_secs(5);

/// Le système de la Station : ce qui change d'un agent à l'autre.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum System {
    /// `CGVirtualDisplay` ; l'agent ne vit que tant qu'il a des écrans virtuels.
    #[cfg_attr(not(target_os = "macos"), allow(dead_code))]
    Mac,
    /// Mutter ou KWin ; l'agent vit tant qu'Admin tourne (il tient la capture).
    Linux,
}

impl System {
    fn prefix(self) -> &'static str {
        match self {
            System::Mac => "mac",
            System::Linux => "linux",
        }
    }

    /// L'agent est-il lancé (ou relancé) pour toute requête ?
    fn always_on(self) -> bool {
        self == System::Linux
    }

    pub fn name(self, id: u32) -> String {
        format!("{}:{id}", self.prefix())
    }

    pub fn id_of(self, gdi_name: &str) -> Result<u32> {
        gdi_name
            .strip_prefix(self.prefix())
            .and_then(|r| r.strip_prefix(':'))
            .and_then(|n| n.parse().ok())
            .with_context(|| format!("écran inconnu : {gdi_name}"))
    }
}

/// Le HDR d'un écran virtuel, propre au système (`mac::set_hdr`).
pub type SetHdr = fn(&AgentBackend, &[hdr::HdrAsk]) -> Result<Vec<hdr::HdrSeen>>;

pub struct AgentBackend {
    pub system: System,
    pipe: AgentPipe,
    /// Écrans virtuels en place, par rang.
    created: Mutex<[Option<u32>; MAX_SCREENS]>,
    /// Écrans suivis par la veille, le principal en premier.
    watched: Mutex<Vec<u32>>,
    /// Ce que l'agent a dit de lui au démarrage (`driver`), sinon le système.
    pub description: Mutex<String>,
    /// Bureau que l'agent a trouvé (Linux : « GNOME », « KDE »).
    pub desktop: Mutex<String>,
    set_hdr: Option<SetHdr>,
}

impl AgentBackend {
    pub fn new(system: System, path: PathBuf, description: &str, set_hdr: Option<SetHdr>) -> Self {
        Self {
            system,
            pipe: AgentPipe::new(path),
            created: Mutex::new([None; MAX_SCREENS]),
            watched: Mutex::new(Vec::new()),
            description: Mutex::new(description.to_string()),
            desktop: Mutex::new(String::new()),
            set_hdr,
        }
    }

    /// Une requête à l'agent, réponse attendue au plus [`CALL_TIMEOUT`].
    /// `start` : le lancer s'il ne tourne pas (sur Mac, sans agent, il n'y a
    /// rien à faire et la réponse est `None` ; sous Linux, toujours relancé).
    pub fn call(&self, req: Value, start: bool) -> Result<Option<Value>> {
        self.pipe.call(&req, start || self.system.always_on(), CALL_TIMEOUT)
    }

    fn name(&self, id: u32) -> String {
        self.system.name(id)
    }

    /// Sur Mac, sans écran virtuel restant, l'agent s'arrête après `restore`.
    fn finish_if_idle(&self) {
        if !self.system.always_on() && self.created.lock().unwrap().iter().all(Option::is_none) {
            let _ = self.call(json!({ "cmd": "restore" }), false);
            self.pipe.finish(FINISH_WITHIN);
        }
    }

    fn isolate_with(&self, screens: &[(Created, Screen)], keep_others: bool) -> Result<Value> {
        let list: Vec<Value> = screens
            .iter()
            .map(|(c, s)| {
                json!({ "id": c.target, "x": s.x, "y": s.y, "width": s.width, "height": s.height,
                        "refresh": s.refresh, "hidpi": hidpi(s.width) })
            })
            .collect();
        let v = self.call(json!({ "cmd": "isolate", "screens": list, "keep_others": keep_others }), false)?.context("agent arrêté")?;
        Ok(applied(self.system, screens.len(), &v)?)
    }
}

/// La réponse d'`isolate`, au format du reste du module : l'écran principal,
/// et avec plusieurs écrans leur place (et leurs échelles, si l'agent les donne).
fn applied(system: System, asked: usize, v: &Value) -> Result<Value> {
    let placed: Vec<Value> = v["screens"]
        .as_array()
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .map(|s| {
            if s.is_null() {
                return Value::Null;
            }
            let id = s["id"].as_u64().unwrap_or(0) as u32;
            json!({ "gdi_name": system.name(id), "width": s["width"], "height": s["height"], "refresh": s["refresh"], "x": s["x"], "y": s["y"] })
        })
        .collect();
    let primary = placed.first().filter(|p| !p.is_null()).context("l'écran virtuel principal n'est pas actif")?;
    let mut applied = json!({
        "gdi_name": primary["gdi_name"], "width": primary["width"], "height": primary["height"],
        "refresh": primary["refresh"], "disabled_others": v["disabled_others"],
    });
    if asked > 1 {
        applied["screens"] = Value::Array(placed);
        if !v["scales"].is_null() {
            applied["scales"] = v["scales"].clone();
        }
    }
    Ok(applied)
}

fn mode_of(v: &Value) -> Mode {
    let n = |k: &str| v[k].as_u64().unwrap_or(0) as u32;
    Mode { width: n("width"), height: n("height"), refresh: n("refresh") }
}

impl Backend for AgentBackend {
    fn driver(&self) -> Result<Value> {
        let mut v = json!({ "version": *self.description.lock().unwrap(), "watchdog_seconds": 0, "agent": self.pipe.path() });
        if self.system == System::Linux {
            v["desktop"] = json!(*self.desktop.lock().unwrap());
        }
        Ok(v)
    }
    fn windows_workarounds(&self) -> bool {
        false
    }
    fn process_names(&self) -> Vec<String> {
        Vec::new()
    }
    fn console_user(&self) -> Option<String> {
        None
    }
    // Le pointeur agrandi est un contournement propre à Windows.
    fn read_pointer(&self) -> Result<(String, PointerRecord)> {
        bail!("pointeur : rien à faire hors de Windows")
    }
    fn enlarge_pointer(&self, _: u32, _: u32) -> Result<Value> {
        bail!("pointeur : rien à faire hors de Windows")
    }
    fn restore_pointer(&self, _: &str, _: PointerRecord) -> Result<Value> {
        bail!("pointeur : rien à faire hors de Windows")
    }
    fn start_wgc(&self, _: &[(String, u32)]) -> Vec<(String, Result<Value>)> {
        Vec::new()
    }
    fn stop_wgc(&self) {}
    fn create(&self, _: [u8; 16], slot: usize, mode: Mode) -> Result<Created> {
        let req = json!({ "cmd": "create", "slot": slot, "width": mode.width, "height": mode.height,
                          "refresh": mode.refresh, "hidpi": hidpi(mode.width) });
        let v = self.call(req, true)?.context("agent arrêté")?;
        let id = v["id"].as_u64().context("identifiant d'écran absent")? as u32;
        self.created.lock().unwrap()[slot] = Some(id);
        tracing::info!("écran virtuel {} créé : {} × {} à {} Hz (écran {id})", slot + 1, mode.width, mode.height, mode.refresh);
        Ok(Created { luid_low: 0, luid_high: 0, target: id })
    }
    fn remove(&self, guid: [u8; 16]) -> Result<()> {
        let Some(slot) = (0..MAX_SCREENS).find(|i| slot_guid(*i) == guid) else { return Ok(()) };
        let removed = self.call(json!({ "cmd": "remove", "slot": slot }), false);
        // Oublié une fois retiré, ou parti avec l'agent ; un refus de l'agent le laisse en place (STMAC-08).
        if removed.is_ok() || !self.pipe.running() {
            self.created.lock().unwrap()[slot] = None;
        }
        removed.map(|_| ())
    }
    fn ping(&self) -> Result<()> {
        // Pas de chien de garde : l'écran vit avec l'agent, qui vit avec Admin.
        Ok(())
    }
    fn isolate(&self, screens: &[(Created, Screen)]) -> Result<Value> {
        self.isolate_with(screens, false)
    }
    fn isolate_keeping_others(&self, screens: &[(Created, Screen)]) -> Result<Value> {
        self.isolate_with(screens, true)
    }
    fn restore(&self) -> Result<()> {
        self.call(json!({ "cmd": "restore" }), false)?;
        if !self.system.always_on() && self.created.lock().unwrap().iter().all(Option::is_none) {
            self.pipe.finish(FINISH_WITHIN);
        }
        Ok(())
    }
    fn watch(&self, created: &[Created]) {
        *self.watched.lock().unwrap() = created.iter().map(|c| c.target).collect();
    }
    fn unwatch(&self) {
        self.watched.lock().unwrap().clear();
    }
    fn observe(&self) -> Option<Observed> {
        let ids = self.watched.lock().unwrap().clone();
        let first = *ids.first()?;
        // La veille (chaque seconde) n'attend pas plus de [`OBSERVE_TIMEOUT`] un agent bloqué.
        let v = self.pipe.call(&json!({ "cmd": "state", "ids": ids }), self.system.always_on(), OBSERVE_TIMEOUT).ok()??;
        let modes: Vec<Mode> = v["modes"].as_array()?.iter().map(mode_of).collect();
        let extra = ids.iter().zip(&modes).skip(1).map(|(id, m)| (*m, self.name(*id))).collect();
        Some(Observed {
            mode: *modes.first()?,
            gdi_name: self.name(first),
            displays: v["displays"].as_u64().unwrap_or(0) as u32,
            others: v["others"].as_u64().unwrap_or(0) as u32,
            extra,
        })
    }
    fn set_mode(&self, gdi_name: &str, mode: Mode) -> Result<Mode> {
        let req = json!({ "cmd": "set_mode", "id": self.system.id_of(gdi_name)?, "width": mode.width, "height": mode.height,
                          "refresh": mode.refresh, "hidpi": hidpi(mode.width) });
        Ok(mode_of(&self.call(req, false)?.context("agent arrêté")?))
    }
    fn displays(&self) -> Result<Vec<PhysicalDisplay>> {
        let v = self.call(json!({ "cmd": "displays" }), true)?.context("agent arrêté")?;
        let list = v["displays"].as_array().cloned().unwrap_or_default();
        let displays = list
            .iter()
            .map(|d| PhysicalDisplay {
                name: self.name(d["id"].as_u64().unwrap_or(0) as u32),
                x: d["x"].as_i64().unwrap_or(0) as i32,
                y: d["y"].as_i64().unwrap_or(0) as i32,
                width: d["width"].as_u64().unwrap_or(0) as u32,
                height: d["height"].as_u64().unwrap_or(0) as u32,
                refresh: d["refresh"].as_u64().unwrap_or(60) as u32,
                primary: d["primary"].as_bool().unwrap_or(false),
            })
            .collect();
        // Personne d'autre que nous ne demandait l'agent (Mac) : il peut s'arrêter.
        self.finish_if_idle();
        Ok(displays)
    }
    /// Le HDR propre au système (Mac : `set_hdr` de l'agent) ; sinon pas de
    /// HDR (Linux : ni GNOME ni KDE ne mettent leurs écrans virtuels en
    /// BT.2100 pour un programme, docs/station-linux.md).
    fn set_hdr(&self, asks: &[hdr::HdrAsk]) -> Result<Vec<hdr::HdrSeen>> {
        if let Some(set_hdr) = self.set_hdr {
            return set_hdr(self, asks);
        }
        Ok(asks
            .iter()
            .map(|ask| hdr::HdrSeen {
                read: true,
                gdi_name: Some(self.name(ask.created.target)),
                error: ask.on.then(|| "pas de HDR sur cette Station pour l'instant".to_string()),
                ..Default::default()
            })
            .collect())
    }
    fn display_hdr(&self) -> Result<hdr::HdrSeen> {
        Ok(hdr::HdrSeen { read: true, ..Default::default() })
    }
    /// Profils de couleur dans la portée du compte : propres à Windows ;
    /// aucun profil n'est associé (`color` reste vide), rien à faire.
    fn hdr_user(&self, _: &hdr::UserRequest) -> Result<Vec<hdr::UserSeen>> {
        Ok(Vec::new())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn echelle_deux_des_2048_pixels() {
        assert!(!hidpi(1920), "1080p : 1x");
        assert!(hidpi(2556), "iPhone : Retina");
        assert!(hidpi(3840), "4K : Retina (1920 × 1080 points)");
    }

    #[test]
    fn noms_des_ecrans() {
        assert_eq!(System::Mac.name(19), "mac:19");
        assert_eq!(System::Mac.id_of("mac:19").unwrap(), 19);
        assert!(System::Mac.id_of("\\\\.\\DISPLAY1").is_err());
        assert_eq!(System::Linux.name(101), "linux:101");
        assert_eq!(System::Linux.id_of("linux:101").unwrap(), 101);
        assert!(System::Linux.id_of("mac:19").is_err());
        assert!(System::Mac.id_of("macx:19").is_err());
    }

    #[test]
    fn reponse_d_isolate() {
        let v = json!({ "screens": [{ "id": 7, "width": 2560, "height": 1440, "refresh": 120, "x": 0, "y": 0 }, null], "disabled_others": 2, "scales": [2, 1] });
        let one = applied(System::Mac, 1, &v).unwrap();
        assert_eq!((one["gdi_name"].as_str(), one["refresh"].as_u64(), one["disabled_others"].as_u64()), (Some("mac:7"), Some(120), Some(2)));
        assert!(one["screens"].is_null() && one["scales"].is_null());
        let two = applied(System::Mac, 2, &v).unwrap();
        assert_eq!((two["screens"][1].is_null(), two["scales"][0].as_u64()), (true, Some(2)));
        // Sans échelles dans la réponse (Linux) : pas de champ vide.
        let linux = applied(System::Linux, 2, &json!({ "screens": [{ "id": 101 }, { "id": 102 }] })).unwrap();
        assert_eq!(linux["screens"][1]["gdi_name"], "linux:102");
        assert!(linux.get("scales").is_none());
        assert!(applied(System::Linux, 1, &json!({ "screens": [null] })).is_err());
    }
}
