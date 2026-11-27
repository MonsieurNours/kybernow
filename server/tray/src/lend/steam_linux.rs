//! Steam pendant un prêt sur une Station Linux (docs/pret.md, « Station
//! Linux ») : le même déroulé que `steam_mac.rs`, avec Steam pour Linux.
//!
//! - Le programme : `steam` (paquet de la distribution, dans le `PATH`), ou
//!   celui qu'Admin donne ; le processus du client s'appelle `steam` ;
//! - ses données : `~/.local/share/Steam` (ou ce que montre `~/.steam/steam`) ;
//!   les comptes retenus (`config/loginusers.vdf`, `config/config.vdf`,
//!   `local.vdf`) mis de côté comme sous Windows (`store::steam_files`) ;
//! - la connexion automatique et le jeu en cours : `AutoLoginUser` et
//!   `RunningAppID` du bloc `HKCU/Software/Valve/Steam` de
//!   `~/.steam/registry.vdf`, la « base de registre » de Steam hors de Windows.
//!   Le Flatpak de Steam (`com.valvesoftware.Steam`) a ses données à part.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail};

use super::store;
use super::system;

fn home() -> PathBuf {
    std::env::var_os("HOME").map(PathBuf::from).unwrap_or_else(std::env::temp_dir)
}

/// Le Flatpak de Steam, s'il est là (et pas le paquet de la distribution).
fn flatpak_home() -> Option<PathBuf> {
    let base = home().join(".var/app/com.valvesoftware.Steam");
    base.join(".local/share/Steam").is_dir().then_some(base)
}

/// Le dossier « maison » de Steam : `$HOME`, ou celui du Flatpak.
fn steam_home() -> PathBuf {
    if which("steam").is_none() && let Some(base) = flatpak_home() {
        return base;
    }
    home()
}

/// Les données de Steam : ce que montre `~/.steam/steam`, sinon `~/.local/share/Steam`.
fn data_dir() -> PathBuf {
    let h = steam_home();
    std::fs::canonicalize(h.join(".steam/steam")).unwrap_or_else(|_| h.join(".local/share/Steam"))
}

/// Le dossier qui contient celui de Steam : `store::steam_files` y trouve `Steam/local.vdf`.
fn support() -> PathBuf {
    data_dir().parent().map(Path::to_path_buf).unwrap_or_else(|| steam_home().join(".local/share"))
}

fn registry() -> PathBuf {
    steam_home().join(".steam/registry.vdf")
}

/// Un programme du `PATH`.
fn which(name: &str) -> Option<PathBuf> {
    std::env::var_os("PATH").and_then(|paths| std::env::split_paths(&paths).map(|d| d.join(name)).find(|p| p.is_file()))
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

/// Le programme de Steam : celui qu'Admin donne, sinon `steam` du `PATH`,
/// sinon `flatpak` (Steam en Flatpak).
pub fn exe(from_admin: Option<&str>) -> Option<PathBuf> {
    from_admin
        .map(PathBuf::from)
        .filter(|p| p.is_file())
        .or_else(|| which("steam"))
        .or_else(|| flatpak_home().and_then(|_| which("flatpak")))
}

pub fn running() -> bool {
    !system::processes("steam").is_empty()
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

/// Ferme Steam (`steam -shutdown`, 20 s au plus, puis arrêté de force).
fn shutdown(exe: &Path) -> bool {
    if !running() {
        return true;
    }
    if let Err(e) = command(exe).arg("-shutdown").spawn() {
        system::log(&format!("steam -shutdown : {e}"));
    }
    if wait_gone(Duration::from_secs(20)) {
        return true;
    }
    system::log("Steam ne s'est pas fermé en 20 s : arrêt forcé");
    for pid in system::processes("steam") {
        system::kill(pid);
    }
    wait_gone(Duration::from_secs(5))
}

/// La commande de Steam : le programme, ou `flatpak run com.valvesoftware.Steam`.
fn command(exe: &Path) -> Command {
    if exe.file_name().is_some_and(|n| n == "flatpak") {
        let mut c = Command::new(exe);
        c.args(["run", "com.valvesoftware.Steam"]);
        return c;
    }
    Command::new(exe)
}

/// Ouvre Steam avec un argument, détaché de l'agent (dans sa propre unité
/// systemd quand c'est possible : il survit à la fin de l'agent).
fn launch(exe: &Path, arg: &str) {
    let mut run = Command::new("systemd-run");
    run.args(["--user", "--quiet", "--collect", "--scope"]);
    let cmd = command(exe);
    run.arg(cmd.get_program()).args(cmd.get_args()).arg(arg);
    if run.spawn().is_ok() {
        return;
    }
    if let Err(e) = command(exe).arg(arg).spawn() {
        system::log(&format!("steam {arg} : {e}"));
    }
}

fn open_url(url: &str) {
    let _ = Command::new("xdg-open").arg(url).spawn();
}

/// « Son compte Steam » : bascule Steam sur l'écran de connexion. `persist`
/// écrit l'état sur le disque avant que rien ne bouge.
/// `lend` et `keeper` ne servent que sous Windows (ici, Admin tourne avec le
/// compte de l'hôte : les fichiers sont renommés à côté).
pub fn switch(exe: &Path, _lend: &str, _keeper: &dyn store::Keeper, persist: &mut dyn FnMut(&store::Steam) -> std::io::Result<()>) -> Result<store::Steam> {
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
        held: false,
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
    store::Steam { friend: false, exe: exe.to_string_lossy().into_owned(), was_running: running(), auto_login: None, files: Vec::new(), held: false }
}

/// Fin du prêt : Steam comme avant.
pub fn restore(_lend: &str, saved: &store::Steam, _keeper: &dyn store::Keeper) -> Result<()> {
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
