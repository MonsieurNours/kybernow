//! « Démarrer avec l'ordinateur » : démarrage automatique des deux services
//! et lancement de l'icône à l'ouverture de session.
//!
//! La clé Run (HKCU) s'écrit sans droits particuliers. Le type de démarrage des
//! services demande des droits administrateur : sans eux, l'icône se relance
//! élevée (`--start-with-computer on|off`), ce qui affiche une invite UAC.

use anyhow::{Result, bail};
use windows_sys::Win32::Foundation::{CloseHandle, ERROR_ACCESS_DENIED, ERROR_CANCELLED, GetLastError, WAIT_OBJECT_0};
use windows_sys::Win32::System::Registry::{HKEY_CURRENT_USER, REG_SZ, RegDeleteKeyValueW, RegSetKeyValueW};
use windows_sys::Win32::System::Threading::{GetExitCodeProcess, INFINITE, WaitForSingleObject};
use windows_sys::Win32::UI::Shell::{SEE_MASK_NOASYNC, SEE_MASK_NOCLOSEPROCESS, SHELLEXECUTEINFOW, ShellExecuteExW};
use windows_sys::Win32::UI::WindowsAndMessaging::SW_HIDE;

use crate::services::{self, ADMIN, KYBER};

const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
/// Nom de la valeur dans la clé Run, le même que celui écrit par l'installateur.
const RUN_VALUE: &str = "KyberNow";
/// Argument de la relance élevée.
pub const ELEVATED_ARG: &str = "--start-with-computer";

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(Some(0)).collect()
}

/// L'état de la case : le serveur démarre-t-il avec l'ordinateur ?
pub fn enabled() -> Option<bool> {
    services::auto_start(KYBER)
}

/// Bascule le tout. `Ok(false)` si l'utilisateur a refusé l'invite UAC.
pub fn set(on: bool) -> Result<bool> {
    let direct = set_services(on);
    match direct {
        Ok(()) => {}
        Err(ERROR_ACCESS_DENIED) => {
            if !run_elevated(on)? {
                return Ok(false);
            }
        }
        Err(code) => bail!("Impossible de changer le démarrage des services ({}).", std::io::Error::from_raw_os_error(code as i32)),
    }
    set_run_entry(on)?;
    Ok(true)
}

/// Type de démarrage des deux services (droits administrateur).
pub fn set_services(on: bool) -> Result<(), u32> {
    services::set_auto_start(KYBER, on)?;
    services::set_auto_start(ADMIN, on)
}

/// Lancement de l'icône à l'ouverture de session de l'utilisateur courant.
fn set_run_entry(on: bool) -> Result<()> {
    let key = wide(RUN_KEY);
    let name = wide(RUN_VALUE);
    let code = if on {
        let exe = std::env::current_exe()?;
        let data = wide(&format!("\"{}\"", exe.display()));
        unsafe {
            RegSetKeyValueW(HKEY_CURRENT_USER, key.as_ptr(), name.as_ptr(), REG_SZ, data.as_ptr().cast(), (data.len() * 2) as u32)
        }
    } else {
        match unsafe { RegDeleteKeyValueW(HKEY_CURRENT_USER, key.as_ptr(), name.as_ptr()) } {
            2 => 0, // ERROR_FILE_NOT_FOUND : déjà absente
            c => c,
        }
    };
    if code != 0 {
        bail!("Impossible d'écrire le lancement à l'ouverture de session ({}).", std::io::Error::from_raw_os_error(code as i32));
    }
    Ok(())
}

/// Relance l'icône en administrateur pour changer le démarrage des services.
fn run_elevated(on: bool) -> Result<bool> {
    match elevate(&format!("{ELEVATED_ARG} {}", if on { "on" } else { "off" }))? {
        None => Ok(false),
        Some(0) => Ok(true),
        Some(code) => bail!("Le démarrage des services n'a pas pu être changé ({}).", std::io::Error::from_raw_os_error(code as i32)),
    }
}

/// Relance l'icône en administrateur avec `params` et attend sa fin : son code
/// de sortie, ou `None` si l'utilisateur a refusé l'invite UAC.
pub fn elevate(params: &str) -> Result<Option<u32>> {
    let exe = wide(&std::env::current_exe()?.display().to_string());
    let params = wide(params);
    let verb = wide("runas");
    let mut info: SHELLEXECUTEINFOW = unsafe { std::mem::zeroed() };
    info.cbSize = size_of::<SHELLEXECUTEINFOW>() as u32;
    info.fMask = SEE_MASK_NOCLOSEPROCESS | SEE_MASK_NOASYNC;
    info.lpVerb = verb.as_ptr();
    info.lpFile = exe.as_ptr();
    info.lpParameters = params.as_ptr();
    info.nShow = SW_HIDE;
    if unsafe { ShellExecuteExW(&mut info) } == 0 {
        if unsafe { GetLastError() } == ERROR_CANCELLED {
            return Ok(None);
        }
        bail!("Impossible d'obtenir les droits administrateur.");
    }
    let mut code = 1u32;
    unsafe {
        if WaitForSingleObject(info.hProcess, INFINITE) == WAIT_OBJECT_0 {
            GetExitCodeProcess(info.hProcess, &mut code);
        }
        CloseHandle(info.hProcess);
    }
    Ok(Some(code))
}

/// Partie élevée : change le démarrage des services, renvoie le code de sortie.
pub fn elevated_main(arg: Option<&str>) -> i32 {
    let on = match arg {
        Some("on") => true,
        Some("off") => false,
        _ => return 87, // ERROR_INVALID_PARAMETER
    };
    match set_services(on) {
        Ok(()) => 0,
        Err(code) => code as i32,
    }
}
