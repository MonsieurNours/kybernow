//! Bureaux wlroots (Sway, Hyprland) : écrans virtuels « sans tête »
//! (headless) créés par le compositeur, capturés par Kyber lui-même
//! (fournisseur wayland de libavgrab, wlr-screencopy) ; le fichier des écrans
//! les désigne par leur nom (`source = "wayland:HEADLESS-1"`, correctif Kyber
//! Linux 0013).
//!
//! - Sway : `swaymsg create_output` (sortie HEADLESS-n), `output … mode
//!   --custom`, `output … unplug` ; disposition par `output … enable pos …
//!   scale …` et `output … disable`. Lecture : `swaymsg -t get_outputs`.
//! - Hyprland : `hyprctl output create headless NOM`, `hyprctl output remove
//!   NOM` ; mode, place, échelle et extinction par `hyprctl keyword monitor`.
//!   Lecture : `hyprctl -j monitors all`.
//!
//! Places en points (unités du bureau) pour les deux. Pas d'écran
//! « principal » sous wlroots : le premier écran reçoit le focus.
//!
//! Filet de sécurité : ces écrans ne partent pas avec l'agent (ils sont au
//! compositeur). Leurs noms sont gardés dans un fichier ; au démarrage,
//! l'agent retire ceux d'une session interrompue et rallume les écrans de
//! l'ordinateur.

use std::path::PathBuf;
use std::process::Command;
use std::sync::Mutex;

use serde_json::Value;

use crate::{Cast, CastKey, Desk, Mon, Place, Recorder, Result, err};

/// Échelles qu'acceptent Sway et Hyprland (toutes, en pratique).
const SCALES: &[f64] = &[1.0, 1.25, 1.5, 1.75, 2.0];

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Flavor {
    Sway,
    Hyprland,
}

/// Sway ou Hyprland, d'après l'environnement de la session.
pub fn present() -> Option<Flavor> {
    let env = |k: &str| std::env::var(k).unwrap_or_default();
    if !env("HYPRLAND_INSTANCE_SIGNATURE").is_empty() {
        return Some(Flavor::Hyprland);
    }
    if !env("SWAYSOCK").is_empty() {
        return Some(Flavor::Sway);
    }
    let desktop = env("XDG_CURRENT_DESKTOP").to_lowercase();
    if desktop.split(':').any(|d| d == "hyprland") {
        Some(Flavor::Hyprland)
    } else if desktop.split(':').any(|d| d == "sway") {
        Some(Flavor::Sway)
    } else {
        None
    }
}

fn run(program: &str, args: &[&str]) -> Result<String> {
    let out = Command::new(program).args(args).output().map_err(err(program))?;
    let text = String::from_utf8_lossy(&out.stdout).into_owned();
    if !out.status.success() {
        let why = String::from_utf8_lossy(&out.stderr);
        return Err(format!("{program} {} : {}", args.join(" "), why.trim()));
    }
    Ok(text)
}

/// Sway répond `[{"success": false, "error": …}]` même quand swaymsg réussit.
fn sway(cmd: &str) -> Result<()> {
    let text = run("swaymsg", &[cmd])?;
    let replies: Value = serde_json::from_str(&text).unwrap_or(Value::Null);
    if let Some(e) = replies.as_array().and_then(|r| r.iter().find(|r| r["success"] == false)) {
        return Err(format!("swaymsg {cmd} : {}", e["error"].as_str().unwrap_or("échec")));
    }
    Ok(())
}

/// Hyprland répond « ok » ou un message d'erreur, toujours avec le code 0.
fn hypr(args: &[&str]) -> Result<()> {
    let text = run("hyprctl", args)?;
    let text = text.trim();
    if text.is_empty() || text.lines().all(|l| l.trim() == "ok") {
        Ok(())
    } else {
        Err(format!("hyprctl {} : {text}", args.join(" ")))
    }
}

/// Écrans sans tête créés par l'agent (Sway les nomme HEADLESS-n, comme
/// ceux qu'aurait l'utilisateur ; Hyprland, comme nous le demandons).
static CREATED: Mutex<Vec<String>> = Mutex::new(Vec::new());

fn is_headless(name: &str) -> bool {
    name.starts_with("KyberNow-") || CREATED.lock().unwrap().iter().any(|n| n == name)
}

/// Le fichier des écrans créés, à côté du fichier des écrans de Kyber.
fn created_file() -> PathBuf {
    crate::screens_file().with_file_name("wlroots-ecrans")
}

fn save_created(names: &[String]) {
    let path = created_file();
    let written = if names.is_empty() {
        std::fs::remove_file(&path).or_else(|e| if e.kind() == std::io::ErrorKind::NotFound { Ok(()) } else { Err(e) })
    } else {
        std::fs::write(&path, names.join("\n") + "\n")
    };
    if let Err(e) = written {
        crate::log(&format!("{} : {e}", path.display()));
    }
}

fn remember(name: &str) {
    let mut created = CREATED.lock().unwrap();
    created.push(name.to_string());
    save_created(&created);
}

fn forget(name: &str) {
    let mut created = CREATED.lock().unwrap();
    created.retain(|n| n != name);
    save_created(&created);
}

fn sway_monitors() -> Result<Vec<Mon>> {
    let text = run("swaymsg", &["-r", "-t", "get_outputs"])?;
    let outputs: Value = serde_json::from_str(&text).map_err(err("swaymsg get_outputs"))?;
    Ok(outputs
        .as_array()
        .cloned()
        .unwrap_or_default()
        .iter()
        .map(|o| {
            let name = o["name"].as_str().unwrap_or_default().to_string();
            let mode = &o["current_mode"];
            Mon {
                virt: is_headless(&name),
                enabled: o["active"].as_bool().unwrap_or(false),
                x: o["rect"]["x"].as_i64().unwrap_or(0) as i32,
                y: o["rect"]["y"].as_i64().unwrap_or(0) as i32,
                width: mode["width"].as_u64().unwrap_or(0) as u32,
                height: mode["height"].as_u64().unwrap_or(0) as u32,
                // Sway donne la fréquence en mHz.
                refresh: (mode["refresh"].as_u64().unwrap_or(60_000) as f64 / 1000.0).round() as u32,
                scale: o["scale"].as_f64().filter(|s| *s > 0.0).unwrap_or(1.0),
                primary: o["focused"].as_bool().unwrap_or(false),
                scales: SCALES.to_vec(),
                connector: name,
            }
        })
        .collect())
}

fn hypr_monitors() -> Result<Vec<Mon>> {
    let text = run("hyprctl", &["-j", "monitors", "all"])?;
    let monitors: Value = serde_json::from_str(&text).map_err(err("hyprctl monitors"))?;
    Ok(monitors
        .as_array()
        .cloned()
        .unwrap_or_default()
        .iter()
        .map(|m| {
            let name = m["name"].as_str().unwrap_or_default().to_string();
            Mon {
                virt: is_headless(&name),
                enabled: !m["disabled"].as_bool().unwrap_or(false),
                x: m["x"].as_i64().unwrap_or(0) as i32,
                y: m["y"].as_i64().unwrap_or(0) as i32,
                width: m["width"].as_u64().unwrap_or(0) as u32,
                height: m["height"].as_u64().unwrap_or(0) as u32,
                refresh: m["refreshRate"].as_f64().unwrap_or(60.0).round() as u32,
                scale: m["scale"].as_f64().filter(|s| *s > 0.0).unwrap_or(1.0),
                primary: m["focused"].as_bool().unwrap_or(false),
                scales: SCALES.to_vec(),
                connector: name,
            }
        })
        .collect())
}

/// La commande de Hyprland qui donne ce mode, cette place et cette échelle.
fn hypr_rule(name: &str, width: u32, height: u32, refresh: u32, x: i32, y: i32, scale: f64) -> String {
    format!("{name},{width}x{height}@{refresh},{x}x{y},{scale}")
}

/// Lecture et disposition des écrans.
pub struct WlrDesk(pub Flavor);

impl Desk for WlrDesk {
    fn name(&self) -> &'static str {
        match self.0 {
            Flavor::Sway => "sway",
            Flavor::Hyprland => "hyprland",
        }
    }

    fn monitors(&self) -> Result<Vec<Mon>> {
        match self.0 {
            Flavor::Sway => sway_monitors(),
            Flavor::Hyprland => hypr_monitors(),
        }
    }

    fn physical_units(&self) -> Result<bool> {
        Ok(false)
    }

    fn apply(&self, places: &[Place]) -> Result<()> {
        let mons = self.monitors()?;
        // D'abord allumer et placer, puis éteindre les autres : le bureau
        // n'est jamais sans écran.
        match self.0 {
            Flavor::Sway => {
                let mut cmds: Vec<String> = places
                    .iter()
                    .map(|p| format!("output {} enable pos {} {} scale {}", p.connector, p.x, p.y, p.scale))
                    .collect();
                cmds.extend(
                    mons.iter()
                        .filter(|m| m.enabled && !places.iter().any(|p| p.connector == m.connector))
                        .map(|m| format!("output {} disable", m.connector)),
                );
                if let Some(first) = places.iter().find(|p| p.primary).or(places.first()) {
                    cmds.push(format!("focus output {}", first.connector));
                }
                sway(&cmds.join("; "))
            }
            Flavor::Hyprland => {
                let mut cmds: Vec<String> = places
                    .iter()
                    .map(|p| {
                        let m = mons.iter().find(|m| m.connector == p.connector);
                        let (w, h, r) = m.map_or((0, 0, 60), |m| (m.width, m.height, m.refresh));
                        let mode = if w == 0 { "preferred".to_string() } else { format!("{w}x{h}@{r}") };
                        format!("keyword monitor {},{mode},{}x{},{}", p.connector, p.x, p.y, p.scale)
                    })
                    .collect();
                cmds.extend(
                    mons.iter()
                        .filter(|m| m.enabled && !places.iter().any(|p| p.connector == m.connector))
                        .map(|m| format!("keyword monitor {},disable", m.connector)),
                );
                if let Some(first) = places.iter().find(|p| p.primary).or(places.first()) {
                    cmds.push(format!("dispatch focusmonitor {}", first.connector));
                }
                hypr(&["--batch", &cmds.join(" ; ")])
            }
        }
    }
}

/// Création des écrans sans tête.
pub struct Wlr {
    flavor: Flavor,
}

impl Wlr {
    pub fn new(flavor: Flavor) -> Result<Self> {
        // Le compositeur répond : sinon, rien à faire ici.
        let shown: Vec<String> = WlrDesk(flavor).monitors()?.into_iter().map(|m| m.connector).collect();
        let wlr = Self { flavor };
        // Session interrompue (agent arrêté net) : ses écrans retirés, ceux
        // de l'ordinateur rallumés.
        let left: Vec<String> = std::fs::read_to_string(created_file())
            .unwrap_or_default()
            .lines()
            .map(str::trim)
            .filter(|n| !n.is_empty() && shown.iter().any(|s| s == n))
            .map(String::from)
            .collect();
        if !left.is_empty() {
            crate::log(&format!("session interrompue : écrans {} retirés, écrans de l'ordinateur rallumés", left.join(", ")));
            for name in &left {
                let _ = wlr.remove(name);
            }
            let _ = match flavor {
                Flavor::Sway => sway("output * enable"),
                Flavor::Hyprland => hypr(&["reload"]),
            };
        }
        save_created(&[]);
        Ok(wlr)
    }

    fn names(&self) -> Result<Vec<String>> {
        Ok(WlrDesk(self.flavor).monitors()?.into_iter().map(|m| m.connector).collect())
    }

    fn set_mode(&self, name: &str, width: u32, height: u32, refresh: u32) -> Result<()> {
        match self.flavor {
            Flavor::Sway => sway(&format!("output {name} mode --custom {width}x{height}@{refresh}Hz")),
            Flavor::Hyprland => {
                // Place et échelle gardées : la disposition les donne ensuite.
                let mons = hypr_monitors()?;
                let m = mons.iter().find(|m| m.connector == name);
                let (x, y, scale) = m.map_or((0, 0, 1.0), |m| (m.x, m.y, m.scale));
                hypr(&["keyword", "monitor", &hypr_rule(name, width, height, refresh, x, y, scale)])
            }
        }
    }
}

fn wlr_cast(name: &str, headless: bool) -> Cast {
    Cast { key: CastKey::Wlr { name: name.to_string(), headless }, node: 0, serial: name.to_string() }
}

impl Recorder for Wlr {
    fn virtual_output(&mut self, slot: usize, width: u32, height: u32, hidpi: bool) -> Result<(Cast, Option<String>)> {
        let before = self.names()?;
        match self.flavor {
            Flavor::Sway => sway("create_output")?,
            Flavor::Hyprland => hypr(&["output", "create", "headless", &format!("KyberNow-{}", slot + 1)])?,
        }
        // Le nom de la nouvelle sortie : celle qui n'était pas là avant.
        let mut name = None;
        for _ in 0..20 {
            name = self.names()?.into_iter().find(|n| !before.contains(n));
            if name.is_some() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
        // Sway : la nouvelle, sans tête, que l'agent n'a pas encore notée.
        let name = name.or_else(|| self.names().ok()?.into_iter().find(|n| !before.contains(n)));
        let name = name.ok_or("écran sans tête créé, mais introuvable")?;
        remember(&name);
        // 60 Hz au départ ; Admin donne la fréquence de l'appareil ensuite.
        if let Err(e) = self.set_mode(&name, width, height, 60) {
            let _ = self.remove(&name);
            return Err(e);
        }
        let _ = hidpi; // l'échelle vient avec la disposition
        Ok((wlr_cast(&name, true), Some(name)))
    }

    fn monitor(&mut self, connector: &str) -> Result<Cast> {
        // Kyber capture lui-même l'écran de l'ordinateur, par son nom.
        Ok(wlr_cast(connector, false))
    }

    fn stop(&mut self, cast: &Cast) {
        if let CastKey::Wlr { name, headless: true } = &cast.key
            && let Err(e) = self.remove(name)
        {
            crate::log(&format!("écran sans tête {name} : {e}"));
        }
    }

    fn resize(&mut self, cast: &Cast, width: u32, height: u32, refresh: u32) -> Result<bool> {
        match &cast.key {
            CastKey::Wlr { name, .. } => self.set_mode(name, width, height, refresh).map(|()| true),
            _ => Ok(false),
        }
    }
}

impl Wlr {
    fn remove(&self, name: &str) -> Result<()> {
        let done = match self.flavor {
            Flavor::Sway => sway(&format!("output {name} unplug")),
            Flavor::Hyprland => hypr(&["output", "remove", name]),
        };
        forget(name);
        done
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn regle_de_hyprland() {
        assert_eq!(hypr_rule("KyberNow-1", 2560, 1440, 120, -1280, 0, 2.0), "KyberNow-1,2560x1440@120,-1280x0,2");
    }

    #[test]
    fn ecrans_sans_tete() {
        // HEADLESS-n n'est à nous que si l'agent l'a créé (Sway).
        assert!(!is_headless("HEADLESS-7"));
        CREATED.lock().unwrap().push("HEADLESS-7".into());
        assert!(is_headless("HEADLESS-7"));
        assert!(is_headless("KyberNow-2"));
        assert!(!is_headless("DP-1"));
    }
}
