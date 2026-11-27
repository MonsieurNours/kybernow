//! Écrans d'une Station Linux (docs/station-linux.md) : bureaux GNOME et KDE Plasma.
//!
//! Admin lance l'agent `kybernow-display` (`server/linux/display-agent`,
//! Rust) dès son démarrage et lui parle par lignes JSON, avec le même
//! protocole que l'agent du Mac (`display/mac.rs`). L'agent passe par le
//! compositeur du bureau (Mutter sous GNOME, KWin sous KDE) : écrans
//! virtuels, capture des écrans de l'ordinateur sans le dialogue du partage
//! d'écran, et disposition temporaire du bureau. Il tient aussi le fichier
//! des écrans que Kyber lit (correctif Kyber Linux 0001).
//!
//! Différence avec le Mac : l'agent reste en marche tant qu'Admin tourne,
//! même sans session, puisque c'est lui qui tient la capture des écrans de
//! l'ordinateur pour Kyber. Filet de sécurité : les écrans virtuels vivent
//! avec sa connexion au bus de la session ; s'il s'arrête, GNOME les retire
//! et revient à sa disposition enregistrée.
//!
//! Nom d'écran (`gdi_name` du reste du module) : `linux:<identifiant Kyber>`
//! (101 pour le premier écran virtuel, 1, 2… pour ceux de l'ordinateur).

use std::path::PathBuf;
use std::sync::Mutex;

use anyhow::{Context, Result, bail};
use serde_json::{Value, json};

use super::agent_pipe::{AgentPipe, CALL_TIMEOUT, OBSERVE_TIMEOUT};
use super::{Backend, Created, MAX_SCREENS, Mode, Observed, PhysicalDisplay, PointerRecord, Screen, hdr, slot_guid};

/// Variable d'environnement : chemin de l'agent, s'il n'est pas à côté d'Admin.
pub const AGENT_ENV: &str = "KYBERNOW_DISPLAY_AGENT";

/// Échelle 2 dès cette largeur (comme le Retina d'une Station Mac).
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

pub fn name(id: u32) -> String {
    format!("linux:{id}")
}

fn id_of(gdi_name: &str) -> Result<u32> {
    gdi_name.strip_prefix("linux:").and_then(|n| n.parse().ok()).with_context(|| format!("écran inconnu : {gdi_name}"))
}

pub struct LinuxBackend {
    pipe: AgentPipe,
    created: Mutex<[Option<u32>; MAX_SCREENS]>,
    watched: Mutex<Vec<u32>>,
    /// Bureau que l'agent a trouvé (« GNOME », « KDE »).
    desktop: Mutex<String>,
}

impl LinuxBackend {
    pub fn new(path: PathBuf) -> Self {
        let backend = Self {
            pipe: AgentPipe::new(path),
            created: Mutex::new([None; MAX_SCREENS]),
            watched: Mutex::new(Vec::new()),
            desktop: Mutex::new(String::new()),
        };
        // Kyber a besoin de ses écrans dès son démarrage : l'agent tout de suite.
        match backend.call(json!({ "cmd": "ping" }), true) {
            Ok(Some(v)) => {
                let desktop = v["desktop"].as_str().unwrap_or_default().to_string();
                tracing::info!("agent des écrans prêt, bureau : {desktop}");
                *backend.desktop.lock().unwrap() = desktop;
            }
            Ok(None) => {}
            Err(e) => tracing::warn!("agent des écrans (kybernow-display) : {e:#}"),
        }
        backend
    }

    /// Une requête à l'agent, relancé s'il s'est arrêté ; réponse attendue au plus [`CALL_TIMEOUT`].
    fn call(&self, req: Value, _start: bool) -> Result<Option<Value>> {
        self.pipe.call(&req, true, CALL_TIMEOUT)
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
                json!({ "gdi_name": name(id), "width": s["width"], "height": s["height"], "refresh": s["refresh"], "x": s["x"], "y": s["y"] })
            })
            .collect();
        let primary = placed.first().filter(|p| !p.is_null()).context("l'écran virtuel principal n'est pas actif")?;
        let mut applied = json!({
            "gdi_name": primary["gdi_name"], "width": primary["width"], "height": primary["height"],
            "refresh": primary["refresh"], "disabled_others": v["disabled_others"],
        });
        if screens.len() > 1 {
            applied["screens"] = Value::Array(placed);
        }
        Ok(applied)
    }
}

fn mode_of(v: &Value) -> Mode {
    let n = |k: &str| v[k].as_u64().unwrap_or(0) as u32;
    Mode { width: n("width"), height: n("height"), refresh: n("refresh") }
}

impl Backend for LinuxBackend {
    fn driver(&self) -> Result<Value> {
        let desktop = self.desktop.lock().unwrap().clone();
        let version = match desktop.as_str() {
            "GNOME" => "Mutter ScreenCast (GNOME)",
            "KDE" => "KWin zkde_screencast (KDE Plasma)",
            _ => "Linux",
        };
        Ok(json!({ "version": version, "watchdog_seconds": 0, "agent": self.pipe.path(), "desktop": desktop }))
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
    fn read_pointer(&self) -> Result<(String, PointerRecord)> {
        bail!("pointeur : rien à faire sous Linux")
    }
    fn enlarge_pointer(&self, _: u32, _: u32) -> Result<Value> {
        bail!("pointeur : rien à faire sous Linux")
    }
    fn restore_pointer(&self, _: &str, _: PointerRecord) -> Result<Value> {
        bail!("pointeur : rien à faire sous Linux")
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
        // Oublié une fois retiré, ou parti avec l'agent ; un refus de l'agent le laisse en place.
        if removed.is_ok() || !self.pipe.running() {
            self.created.lock().unwrap()[slot] = None;
        }
        removed.map(|_| ())
    }
    fn ping(&self) -> Result<()> {
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
        let v = self.pipe.call(&json!({ "cmd": "state", "ids": ids }), true, OBSERVE_TIMEOUT).ok()??;
        let modes: Vec<Mode> = v["modes"].as_array()?.iter().map(mode_of).collect();
        let extra = ids.iter().zip(&modes).skip(1).map(|(id, m)| (*m, name(*id))).collect();
        Some(Observed {
            mode: *modes.first()?,
            gdi_name: name(first),
            displays: v["displays"].as_u64().unwrap_or(0) as u32,
            others: v["others"].as_u64().unwrap_or(0) as u32,
            extra,
        })
    }
    fn set_mode(&self, gdi_name: &str, mode: Mode) -> Result<Mode> {
        let req = json!({ "cmd": "set_mode", "id": id_of(gdi_name)?, "width": mode.width, "height": mode.height,
                          "refresh": mode.refresh, "hidpi": hidpi(mode.width) });
        Ok(mode_of(&self.call(req, false)?.context("agent arrêté")?))
    }
    fn displays(&self) -> Result<Vec<PhysicalDisplay>> {
        let v = self.call(json!({ "cmd": "displays" }), true)?.context("agent arrêté")?;
        let list = v["displays"].as_array().cloned().unwrap_or_default();
        Ok(list
            .iter()
            .map(|d| PhysicalDisplay {
                name: name(d["id"].as_u64().unwrap_or(0) as u32),
                x: d["x"].as_i64().unwrap_or(0) as i32,
                y: d["y"].as_i64().unwrap_or(0) as i32,
                width: d["width"].as_u64().unwrap_or(0) as u32,
                height: d["height"].as_u64().unwrap_or(0) as u32,
                refresh: d["refresh"].as_u64().unwrap_or(60) as u32,
                primary: d["primary"].as_bool().unwrap_or(false),
            })
            .collect())
    }
    /// Pas encore de HDR depuis une Station Linux : ni GNOME ni KDE ne mettent
    /// leurs écrans virtuels en BT.2100 pour un programme (docs/station-linux.md).
    fn set_hdr(&self, asks: &[hdr::HdrAsk]) -> Result<Vec<hdr::HdrSeen>> {
        Ok(asks
            .iter()
            .map(|ask| hdr::HdrSeen {
                read: true,
                gdi_name: Some(name(ask.created.target)),
                error: ask.on.then(|| "pas de HDR sur une Station Linux pour l'instant".to_string()),
                ..Default::default()
            })
            .collect())
    }
    fn display_hdr(&self) -> Result<hdr::HdrSeen> {
        Ok(hdr::HdrSeen { read: true, ..Default::default() })
    }
    fn hdr_user(&self, _: &hdr::UserRequest) -> Result<Vec<hdr::UserSeen>> {
        Ok(Vec::new())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scale_two_from_2048_pixels_wide() {
        assert!(!hidpi(1920));
        assert!(hidpi(2560));
    }

    #[test]
    fn screen_names() {
        assert_eq!(name(101), "linux:101");
        assert_eq!(id_of("linux:101").unwrap(), 101);
        assert!(id_of("mac:19").is_err());
    }
}
