//! Steam pendant un prêt (`docs/pret.md`) :
//! - « Son compte Steam » : Steam fermé (`steam.exe -shutdown`), les comptes
//!   retenus mis de côté (`store::steam_files`) et la connexion automatique
//!   (`HKCU\Software\Valve\Steam\AutoLoginUser`) effacée, puis Steam rouvert
//!   en Big Picture sur l'écran de connexion ; à la fin, Steam fermé, les
//!   fichiers de l'utilisateur remis, la connexion automatique rendue, Steam
//!   rouvert s'il tournait avant ;
//! - « Votre compte Steam » : rien à basculer ; à la fin, Big Picture fermé
//!   (ou Steam, s'il ne tournait pas avant le prêt).

use std::path::{Path, PathBuf};
use std::process::Command;
use std::ptr::{null, null_mut};
use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail};
use windows_sys::Win32::System::Registry::{HKEY_CURRENT_USER, REG_SZ, RRF_RT_REG_SZ, RegDeleteKeyValueW, RegGetValueW, RegSetKeyValueW};
use windows_sys::Win32::UI::Shell::ShellExecuteW;
use windows_sys::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

use super::store;
use super::system::{self, wide};

const KEY: &str = r"Software\Valve\Steam";

fn get(value: &str) -> Option<String> {
    let (key, name) = (wide(KEY), wide(value));
    let mut size = 0u32;
    unsafe {
        if RegGetValueW(HKEY_CURRENT_USER, key.as_ptr(), name.as_ptr(), RRF_RT_REG_SZ, null_mut(), null_mut(), &mut size) != 0 {
            return None;
        }
        let mut buf = vec![0u16; (size as usize).div_ceil(2).max(1)];
        if RegGetValueW(HKEY_CURRENT_USER, key.as_ptr(), name.as_ptr(), RRF_RT_REG_SZ, null_mut(), buf.as_mut_ptr().cast(), &mut size) != 0 {
            return None;
        }
        let len = buf.iter().position(|c| *c == 0).unwrap_or(buf.len());
        Some(String::from_utf16_lossy(&buf[..len]))
    }
}

/// `Some` : écrit la valeur ; `None` : l'efface (elle n'existait pas avant le prêt).
fn set(value: &str, data: Option<&str>) -> Result<()> {
    let (key, name) = (wide(KEY), wide(value));
    let code = unsafe {
        match data {
            Some(text) => {
                let data = wide(text);
                RegSetKeyValueW(HKEY_CURRENT_USER, key.as_ptr(), name.as_ptr(), REG_SZ, data.as_ptr().cast(), (data.len() * 2) as u32)
            }
            None => match RegDeleteKeyValueW(HKEY_CURRENT_USER, key.as_ptr(), name.as_ptr()) {
                2 => 0, // ERROR_FILE_NOT_FOUND : déjà absente
                c => c,
            },
        }
    };
    if code != 0 {
        bail!("{value} : {}", std::io::Error::from_raw_os_error(code as i32));
    }
    Ok(())
}

/// `steam.exe` : celui qu'Admin donne, sinon `SteamExe` (avec des « / »).
pub fn exe(from_admin: Option<&str>) -> Option<PathBuf> {
    [from_admin.map(str::to_owned), get("SteamExe")]
        .into_iter()
        .flatten()
        .map(|p| PathBuf::from(p.replace('/', "\\")))
        .find(|p| p.is_file())
}

pub fn running() -> bool {
    !system::processes("steam.exe").is_empty()
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

/// Ferme Steam (`-shutdown`, 20 s au plus, puis arrêté de force) ; vrai
/// quand il ne tourne plus.
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
    for pid in system::processes("steam.exe") {
        system::kill(pid);
    }
    wait_gone(Duration::from_secs(5))
}

fn launch(exe: &Path, arg: &str) {
    if let Err(e) = Command::new(exe).arg(arg).spawn() {
        system::log(&format!("steam {arg} : {e}"));
    }
}

fn open_url(url: &str) {
    let (verb, url) = (wide("open"), wide(url));
    unsafe { ShellExecuteW(null_mut(), verb.as_ptr(), url.as_ptr(), null(), null(), SW_SHOWNORMAL) };
}

fn local_app_data() -> Option<PathBuf> {
    std::env::var_os("LOCALAPPDATA").map(PathBuf::from)
}

/// « Son compte Steam » : bascule Steam sur l'écran de connexion. `persist`
/// écrit l'état sur le disque avant que rien ne bouge (un agent tombé ensuite
/// sera réparé).
pub fn switch(exe: &Path, persist: &mut dyn FnMut(&store::Steam) -> std::io::Result<()>) -> Result<store::Steam> {
    let was_running = running();
    if !shutdown(exe) {
        bail!("Steam ne se ferme pas");
    }
    let dir = exe.parent().context("dossier de Steam")?;
    let saved = store::Steam {
        friend: true,
        exe: exe.to_string_lossy().into_owned(),
        was_running,
        auto_login: get("AutoLoginUser"),
        files: store::existing(&store::steam_files(dir, local_app_data().as_deref())),
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

/// Fin du prêt : Steam comme avant. Une erreur laisse l'état sur le disque,
/// pour réessayer au prochain démarrage de l'icône.
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
        // Comme à l'ouverture de session : dans la zone de notification.
        launch(&exe, "-silent");
    }
    Ok(())
}
