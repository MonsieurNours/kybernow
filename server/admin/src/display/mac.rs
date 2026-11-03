//! Écrans virtuels d'une Station Mac (docs/station-macos.md, jalon 3).
//!
//! Admin lance l'agent `kybernow-display` (`server/macos/display-agent`) et lui
//! parle par lignes JSON. L'agent tient les écrans `CGVirtualDisplay` tant
//! qu'il vit, met les écrans du Mac en recopie de l'écran virtuel principal
//! (seul l'écran virtuel reste actif) et fait ses changements « pour ce
//! programme seulement » : si Admin s'arrête (fin de l'entrée de l'agent) ou
//! si l'agent plante, macOS défait tout. C'est le filet de sécurité qui
//! remplace, sur Mac, le chien de garde de SudoVDA et `recover`.
//!
//! Nom d'écran (`gdi_name` du reste du module) : `mac:<CGDirectDisplayID>`.
//! Retina dès 2048 pixels de large ([`hidpi`]).

use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::Mutex;

use anyhow::{Context, Result, bail};
use serde_json::{Value, json};

use super::{Backend, Created, MAX_SCREENS, Mode, Observed, PhysicalDisplay, PointerRecord, Screen, hdr, slot_guid};

/// Variable d'environnement : chemin de l'agent, s'il n'est pas à côté d'Admin.
pub const AGENT_ENV: &str = "KYBERNOW_DISPLAY_AGENT";

/// Mode Retina (la moitié en points) dès cette largeur : une interface lisible
/// sur un téléphone, une tablette, un Mac ou un téléviseur 4K ; un écran 1080p
/// reste en 1x.
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

fn name(id: u32) -> String {
    format!("mac:{id}")
}

fn id_of(gdi_name: &str) -> Result<u32> {
    gdi_name.strip_prefix("mac:").and_then(|n| n.parse().ok()).with_context(|| format!("écran inconnu : {gdi_name}"))
}

struct Agent {
    child: Child,
    stdin: ChildStdin,
    out: BufReader<ChildStdout>,
}

pub struct MacBackend {
    path: PathBuf,
    agent: Mutex<Option<Agent>>,
    /// Écrans virtuels en place, par rang.
    created: Mutex<[Option<u32>; MAX_SCREENS]>,
    /// Écrans suivis par la veille, le principal en premier.
    watched: Mutex<Vec<u32>>,
}

impl MacBackend {
    pub fn new(path: PathBuf) -> Self {
        Self { path, agent: Mutex::new(None), created: Mutex::new([None; MAX_SCREENS]), watched: Mutex::new(Vec::new()) }
    }

    /// Une requête à l'agent. `start` : le lancer s'il ne tourne pas (sinon,
    /// sans agent, il n'y a rien à faire et la réponse est `None`).
    fn call(&self, req: Value, start: bool) -> Result<Option<Value>> {
        let mut agent = self.agent.lock().unwrap();
        if let Some(a) = agent.as_mut()
            && a.child.try_wait().ok().flatten().is_some()
        {
            *agent = None;
        }
        if agent.is_none() {
            if !start {
                return Ok(None);
            }
            let mut child = Command::new(&self.path)
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .spawn()
                .with_context(|| format!("lancement de {}", self.path.display()))?;
            let stdin = child.stdin.take().context("entrée de l'agent")?;
            let out = BufReader::new(child.stdout.take().context("sortie de l'agent")?);
            tracing::info!("agent des écrans virtuels lancé ({})", self.path.display());
            *agent = Some(Agent { child, stdin, out });
        }
        let a = agent.as_mut().unwrap();
        let mut answer = String::new();
        let sent = writeln!(a.stdin, "{req}").and_then(|()| a.stdin.flush()).and_then(|()| a.out.read_line(&mut answer));
        match sent {
            Ok(n) if n > 0 => {}
            Ok(_) | Err(_) => {
                // L'agent s'est arrêté : macOS a déjà tout défait.
                let _ = a.child.kill();
                let _ = a.child.wait();
                *agent = None;
                bail!("l'agent des écrans virtuels ne répond plus");
            }
        }
        let v: Value = serde_json::from_str(&answer).context("réponse de l'agent illisible")?;
        if v["ok"] != json!(true) {
            bail!("{}", v["error"].as_str().unwrap_or("échec de l'agent"));
        }
        Ok(Some(v))
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
            applied["scales"] = v["scales"].clone();
        }
        Ok(applied)
    }
}

fn mode_of(v: &Value) -> Mode {
    let n = |k: &str| v[k].as_u64().unwrap_or(0) as u32;
    Mode { width: n("width"), height: n("height"), refresh: n("refresh") }
}

impl Backend for MacBackend {
    fn driver(&self) -> Result<Value> {
        Ok(json!({ "version": "CGVirtualDisplay (macOS)", "watchdog_seconds": 0, "agent": self.path }))
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
        bail!("pointeur : rien à faire sur Mac")
    }
    fn enlarge_pointer(&self, _: u32, _: u32) -> Result<Value> {
        bail!("pointeur : rien à faire sur Mac")
    }
    fn restore_pointer(&self, _: &str, _: PointerRecord) -> Result<Value> {
        bail!("pointeur : rien à faire sur Mac")
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
        self.created.lock().unwrap()[slot] = None;
        self.call(json!({ "cmd": "remove", "slot": slot }), false)?;
        Ok(())
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
        // Sans écran virtuel restant, l'agent s'arrête après sa réponse.
        let _ = self.call(json!({ "cmd": "restore" }), false)?;
        if self.created.lock().unwrap().iter().all(Option::is_none) {
            let mut agent = self.agent.lock().unwrap();
            if let Some(mut a) = agent.take() {
                let _ = a.child.wait();
            }
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
        let v = self.call(json!({ "cmd": "state", "ids": ids }), false).ok()??;
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
        let displays = list
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
            .collect();
        // Personne d'autre que nous ne demandait l'agent : il peut s'arrêter.
        if self.created.lock().unwrap().iter().all(Option::is_none) {
            let _ = self.call(json!({ "cmd": "restore" }), false);
            if let Some(mut a) = self.agent.lock().unwrap().take() {
                let _ = a.child.wait();
            }
        }
        Ok(displays)
    }
    fn set_hdr(&self, _: &[hdr::HdrAsk]) -> Result<Vec<hdr::HdrSeen>> {
        bail!("HDR des écrans virtuels du Mac : pas encore (docs/station-macos.md)")
    }
    fn display_hdr(&self) -> Result<hdr::HdrSeen> {
        Ok(hdr::HdrSeen { read: true, ..Default::default() })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retina_from_2048_pixels_wide() {
        assert!(!hidpi(1920), "1080p : 1x");
        assert!(hidpi(2556), "iPhone : Retina");
        assert!(hidpi(3840), "4K : Retina (1920 × 1080 points)");
    }

    #[test]
    fn screen_names() {
        assert_eq!(name(19), "mac:19");
        assert_eq!(id_of("mac:19").unwrap(), 19);
        assert!(id_of("\\\\.\\DISPLAY1").is_err());
    }
}
