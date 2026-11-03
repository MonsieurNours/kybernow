//! Steam pendant un prêt sur une Station Mac (docs/pret.md) : le même
//! déroulé que `steam.rs` (Windows), avec Steam pour Mac.
//!
//! - L'app : `/Applications/Steam.app` (ou celle de l'utilisateur), son
//!   exécutable `Contents/MacOS/steam_osx` ;
//! - ses données : `~/Library/Application Support/Steam` ; les comptes retenus
//!   (`config/loginusers.vdf`, `config/config.vdf`, `local.vdf`) mis de côté
//!   comme sous Windows (`store::steam_files`) ;
//! - la connexion automatique et le jeu en cours : `AutoLoginUser` et
//!   `RunningAppID` du bloc `HKCU/Software/Valve/Steam` de `registry.vdf`, la
//!   « base de registre » de Steam hors de Windows (un fichier texte VDF).

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail};

use super::store;
use super::system;

fn home() -> PathBuf {
    std::env::var_os("HOME").map(PathBuf::from).unwrap_or_else(std::env::temp_dir)
}

/// `~/Library/Application Support` : `store::steam_files` y trouve `Steam/local.vdf`.
fn support() -> PathBuf {
    home().join("Library/Application Support")
}

fn data_dir() -> PathBuf {
    support().join("Steam")
}

fn registry() -> PathBuf {
    data_dir().join("registry.vdf")
}

/// Une ligne `"nom"		"valeur"` du bloc `HKCU/Software/Valve/Steam`.
fn line_value(line: &str, name: &str) -> Option<String> {
    let rest = line.trim_start().strip_prefix(&format!("\"{name}\""))?;
    let value = rest.trim();
    value.strip_prefix('"')?.strip_suffix('"').map(str::to_string)
}

/// Bornes (lignes) du bloc `HKCU/…/Steam` : la ligne de son `{` et celle de son `}`.
fn hkcu_steam(lines: &[String]) -> Option<(usize, usize)> {
    let hkcu = lines.iter().position(|l| l.trim() == "\"HKCU\"")?;
    let name = (hkcu..lines.len()).find(|&i| lines[i].trim() == "\"Steam\"")?;
    let open = name + 1;
    if lines.get(open)?.trim() != "{" {
        return None;
    }
    let mut depth = 0;
    for (i, l) in lines.iter().enumerate().skip(open) {
        match l.trim() {
            "{" => depth += 1,
            "}" => {
                depth -= 1;
                if depth == 0 {
                    return Some((open, i));
                }
            }
            _ => {}
        }
    }
    None
}

fn read_lines() -> Vec<String> {
    std::fs::read_to_string(registry()).map(|t| t.lines().map(str::to_string).collect()).unwrap_or_default()
}

/// Valeur d'un nom du bloc (au premier niveau du bloc seulement).
fn get_in(lines: &[String], name: &str) -> Option<String> {
    let (open, close) = hkcu_steam(lines)?;
    let indent = lines[open].len() - lines[open].trim_start().len() + 1;
    lines[open + 1..close].iter().filter(|l| l.len() - l.trim_start().len() == indent).find_map(|l| line_value(l, name))
}

fn get(name: &str) -> Option<String> {
    get_in(&read_lines(), name)
}

/// `Some` : écrit la valeur ; `None` : l'efface (elle n'existait pas avant le prêt).
fn set_in(lines: &mut Vec<String>, name: &str, data: Option<&str>) -> Result<()> {
    let (open, close) = hkcu_steam(lines).context("registry.vdf : bloc HKCU/Software/Valve/Steam introuvable")?;
    let indent_len = lines[open].len() - lines[open].trim_start().len() + 1;
    let indent = "\t".repeat(indent_len);
    let at = lines[open + 1..close].iter().position(|l| l.len() - l.trim_start().len() == indent_len && line_value(l, name).is_some()).map(|i| i + open + 1);
    match (at, data) {
        (Some(i), Some(v)) => lines[i] = format!("{indent}\"{name}\"\t\t\"{v}\""),
        (Some(i), None) => {
            lines.remove(i);
        }
        (None, Some(v)) => lines.insert(open + 1, format!("{indent}\"{name}\"\t\t\"{v}\"")),
        (None, None) => {}
    }
    Ok(())
}

fn set(name: &str, data: Option<&str>) -> Result<()> {
    let mut lines = read_lines();
    if lines.is_empty() {
        // Pas de registry.vdf : Steam n'a jamais tourné, rien à retenir.
        return Ok(());
    }
    set_in(&mut lines, name, data)?;
    let path = registry();
    let tmp = path.with_extension("vdf.kybernow-tmp");
    std::fs::write(&tmp, lines.join("\n") + "\n").context("registry.vdf")?;
    std::fs::rename(&tmp, &path).context("registry.vdf")?;
    Ok(())
}

/// Le jeu Steam en cours (`RunningAppID`, 0 sans jeu), pour la dernière partie de l'ami.
pub fn running_app() -> Option<u32> {
    get("RunningAppID").and_then(|v| v.parse().ok()).filter(|v| *v > 0)
}

/// L'exécutable de Steam : celui qu'Admin donne, sinon l'app dans Applications.
pub fn exe(from_admin: Option<&str>) -> Option<PathBuf> {
    let apps = [PathBuf::from("/Applications/Steam.app"), home().join("Applications/Steam.app")];
    from_admin
        .map(PathBuf::from)
        .into_iter()
        .chain(apps.into_iter().map(|a| a.join("Contents/MacOS/steam_osx")))
        .find(|p| p.is_file())
}

pub fn running() -> bool {
    !system::processes("steam_osx").is_empty()
}

fn wait_gone(limit: Duration) -> bool {
    let start = Instant::now();
    while running() {
        if start.elapsed() > limit {
            return false;
        }
        std::thread::sleep(Duration::from_millis(250));
    }
    true
}

/// Ferme Steam (`steam_osx -shutdown`, 20 s au plus, puis arrêté de force).
fn shutdown(exe: &Path) -> bool {
    if !running() {
        return true;
    }
    if let Err(e) = Command::new(exe).arg("-shutdown").spawn() {
        system::log(&format!("steam -shutdown : {e}"));
    }
    if wait_gone(Duration::from_secs(20)) {
        return true;
    }
    system::log("Steam ne s'est pas fermé en 20 s : arrêt forcé");
    for pid in system::processes("steam_osx") {
        system::kill(pid);
    }
    wait_gone(Duration::from_secs(5))
}

/// L'app Steam (`…/Steam.app`) de l'exécutable.
fn app_of(exe: &Path) -> PathBuf {
    exe.ancestors().find(|p| p.extension().is_some_and(|e| e == "app")).map(Path::to_path_buf).unwrap_or_else(|| PathBuf::from("/Applications/Steam.app"))
}

/// Ouvre Steam (`open -a`, comme depuis le Finder) avec un argument.
fn launch(exe: &Path, arg: &str) {
    if let Err(e) = Command::new("/usr/bin/open").arg("-a").arg(app_of(exe)).args(["--args", arg]).spawn() {
        system::log(&format!("steam {arg} : {e}"));
    }
}

fn open_url(url: &str) {
    let _ = Command::new("/usr/bin/open").arg(url).spawn();
}

/// « Son compte Steam » : bascule Steam sur l'écran de connexion. `persist`
/// écrit l'état sur le disque avant que rien ne bouge.
pub fn switch(exe: &Path, persist: &mut dyn FnMut(&store::Steam) -> std::io::Result<()>) -> Result<store::Steam> {
    let was_running = running();
    if !shutdown(exe) {
        bail!("Steam ne se ferme pas");
    }
    let saved = store::Steam {
        friend: true,
        exe: exe.to_string_lossy().into_owned(),
        was_running,
        auto_login: get("AutoLoginUser"),
        files: store::existing(&store::steam_files(&data_dir(), Some(&support()))),
    };
    persist(&saved).context("état du prêt")?;
    store::put_aside(&saved.files).context("comptes de Steam mis de côté")?;
    set("AutoLoginUser", Some(""))?;
    // Big Picture d'emblée, avec son écran de connexion (à la manette).
    launch(exe, "-gamepadui");
    Ok(saved)
}

/// « Votre compte Steam » : rien ne change, on note seulement si Steam tournait.
pub fn host(exe: &Path) -> store::Steam {
    store::Steam { friend: false, exe: exe.to_string_lossy().into_owned(), was_running: running(), auto_login: None, files: Vec::new() }
}

/// Fin du prêt : Steam comme avant.
pub fn restore(saved: &store::Steam) -> Result<()> {
    let exe = PathBuf::from(&saved.exe);
    if !saved.friend {
        if running() {
            if saved.was_running {
                open_url("steam://close/bigpicture");
            } else if !shutdown(&exe) {
                bail!("Steam ne se ferme pas");
            }
        }
        return Ok(());
    }
    if !shutdown(&exe) {
        bail!("Steam ne se ferme pas");
    }
    store::put_back(&saved.files).context("comptes de Steam remis")?;
    set("AutoLoginUser", saved.auto_login.as_deref())?;
    if saved.was_running {
        launch(&exe, "-silent");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const VDF: &str = "\"Registry\"\n{\n\t\"HKLM\"\n\t{\n\t\t\"Software\"\n\t\t{\n\t\t\t\"Valve\"\n\t\t\t{\n\t\t\t\t\"Steam\"\n\t\t\t\t{\n\t\t\t\t\t\"SteamPID\"\t\t\"0\"\n\t\t\t\t}\n\t\t\t}\n\t\t}\n\t}\n\t\"HKCU\"\n\t{\n\t\t\"Software\"\n\t\t{\n\t\t\t\"Valve\"\n\t\t\t{\n\t\t\t\t\"Steam\"\n\t\t\t\t{\n\t\t\t\t\t\"steamglobal\"\n\t\t\t\t\t{\n\t\t\t\t\t\t\"AutoLoginUser\"\t\t\"pas-ici\"\n\t\t\t\t\t}\n\t\t\t\t\t\"AutoLoginUser\"\t\t\"matthieu\"\n\t\t\t\t\t\"Rate\"\t\t\"30000\"\n\t\t\t\t}\n\t\t\t}\n\t\t}\n\t}\n}";

    fn lines() -> Vec<String> {
        VDF.lines().map(str::to_string).collect()
    }

    #[test]
    fn reads_and_writes_the_hkcu_steam_block() {
        let mut l = lines();
        assert_eq!(get_in(&l, "AutoLoginUser").as_deref(), Some("matthieu"));
        assert_eq!(get_in(&l, "SteamPID"), None);
        set_in(&mut l, "AutoLoginUser", Some("")).unwrap();
        assert_eq!(get_in(&l, "AutoLoginUser").as_deref(), Some(""));
        set_in(&mut l, "AutoLoginUser", None).unwrap();
        assert_eq!(get_in(&l, "AutoLoginUser"), None);
        // Le bloc imbriqué n'est pas touché.
        assert!(l.iter().any(|x| x.contains("pas-ici")));
        set_in(&mut l, "AutoLoginUser", Some("matthieu")).unwrap();
        assert_eq!(get_in(&l, "AutoLoginUser").as_deref(), Some("matthieu"));
        assert_eq!(get_in(&l, "Rate").as_deref(), Some("30000"));
    }
}
