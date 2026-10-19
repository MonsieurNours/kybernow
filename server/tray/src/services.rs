//! Pilotage des services `kyber` et `kyber-admin` sans droits administrateur.
//!
//! L'installateur ouvre aux utilisateurs interactifs le démarrage, l'arrêt et la
//! lecture de l'état de ces deux services (`sc sdset`, voir `SERVICE_SDDL` dans
//! l'installateur). Changer leur type de démarrage reste réservé aux
//! administrateurs : voir `startup.rs`.

use std::ptr::null;
use std::time::{Duration, Instant};

use anyhow::{Result, bail};
use windows_sys::Win32::Foundation::{
    ERROR_ACCESS_DENIED, ERROR_INSUFFICIENT_BUFFER, ERROR_SERVICE_ALREADY_RUNNING, ERROR_SERVICE_DOES_NOT_EXIST,
    ERROR_SERVICE_NOT_ACTIVE, GetLastError,
};
use windows_sys::Win32::System::Services::*;

/// Serveur de streaming (kyservice).
pub const KYBER: &str = "kyber";
/// Console d'administration.
pub const ADMIN: &str = "kyber-admin";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    Running,
    Stopped,
    Starting,
    Stopping,
    Missing,
    Unknown,
}

impl State {
    pub fn is_up(self) -> bool {
        matches!(self, State::Running | State::Starting)
    }
}

struct Handle(SC_HANDLE);

impl Drop for Handle {
    fn drop(&mut self) {
        unsafe { CloseServiceHandle(self.0) };
    }
}

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(Some(0)).collect()
}

/// Erreur Windows avec un message lisible pour l'utilisateur.
fn failure(service: &str, what: &str, code: u32) -> anyhow::Error {
    if code == ERROR_ACCESS_DENIED {
        anyhow::anyhow!(
            "Windows refuse de vous laisser {what} le service « {service} ». \
             Relancez l'installation ou la mise à jour de KyberNow pour autoriser l'icône à le piloter."
        )
    } else {
        anyhow::anyhow!("Impossible de {what} le service « {service} » ({}).", std::io::Error::from_raw_os_error(code as i32))
    }
}

fn open(name: &str, access: u32) -> Result<Handle, u32> {
    unsafe {
        let scm = OpenSCManagerW(null(), null(), SC_MANAGER_CONNECT);
        if scm.is_null() {
            return Err(GetLastError());
        }
        let scm = Handle(scm);
        let service = OpenServiceW(scm.0, wide(name).as_ptr(), access);
        if service.is_null() {
            return Err(GetLastError());
        }
        Ok(Handle(service))
    }
}

pub fn state(name: &str) -> State {
    let service = match open(name, SERVICE_QUERY_STATUS) {
        Ok(s) => s,
        Err(ERROR_SERVICE_DOES_NOT_EXIST) => return State::Missing,
        Err(_) => return State::Unknown,
    };
    let mut status: SERVICE_STATUS = unsafe { std::mem::zeroed() };
    if unsafe { QueryServiceStatus(service.0, &mut status) } == 0 {
        return State::Unknown;
    }
    match status.dwCurrentState {
        SERVICE_RUNNING => State::Running,
        SERVICE_STOPPED => State::Stopped,
        SERVICE_START_PENDING | SERVICE_CONTINUE_PENDING => State::Starting,
        SERVICE_STOP_PENDING | SERVICE_PAUSE_PENDING | SERVICE_PAUSED => State::Stopping,
        _ => State::Unknown,
    }
}

pub fn start(name: &str) -> Result<()> {
    let service = open(name, SERVICE_START).map_err(|c| failure(name, "démarrer", c))?;
    if unsafe { StartServiceW(service.0, 0, null()) } == 0 {
        let code = unsafe { GetLastError() };
        if code != ERROR_SERVICE_ALREADY_RUNNING {
            return Err(failure(name, "démarrer", code));
        }
    }
    Ok(())
}

pub fn stop(name: &str) -> Result<()> {
    let service = open(name, SERVICE_STOP).map_err(|c| failure(name, "arrêter", c))?;
    let mut status: SERVICE_STATUS = unsafe { std::mem::zeroed() };
    if unsafe { ControlService(service.0, SERVICE_CONTROL_STOP, &mut status) } == 0 {
        let code = unsafe { GetLastError() };
        if code != ERROR_SERVICE_NOT_ACTIVE {
            return Err(failure(name, "arrêter", code));
        }
    }
    Ok(())
}

/// Attend qu'un service atteigne l'état voulu.
pub fn wait(name: &str, target: State, timeout: Duration) -> Result<()> {
    let deadline = Instant::now() + timeout;
    loop {
        let now = state(name);
        if now == target {
            return Ok(());
        }
        if now == State::Stopped && target == State::Running {
            bail!("Le service « {name} » s'est arrêté au démarrage. Consultez les journaux dans la console.");
        }
        if Instant::now() > deadline {
            bail!("Le service « {name} » ne répond pas au bout de {} secondes.", timeout.as_secs());
        }
        std::thread::sleep(Duration::from_millis(300));
    }
}

/// `Some(true)` si le service démarre avec l'ordinateur.
pub fn auto_start(name: &str) -> Option<bool> {
    let service = open(name, SERVICE_QUERY_CONFIG).ok()?;
    let mut needed = 0u32;
    unsafe {
        QueryServiceConfigW(service.0, std::ptr::null_mut(), 0, &mut needed);
        if GetLastError() != ERROR_INSUFFICIENT_BUFFER {
            return None;
        }
        // u64 : alignement suffisant pour QUERY_SERVICE_CONFIGW.
        let mut buf = vec![0u64; (needed as usize).div_ceil(8)];
        let config = buf.as_mut_ptr() as *mut QUERY_SERVICE_CONFIGW;
        if QueryServiceConfigW(service.0, config, needed, &mut needed) == 0 {
            return None;
        }
        Some((*config).dwStartType == SERVICE_AUTO_START)
    }
}

/// Démarrage automatique ou manuel. `Err(ERROR_ACCESS_DENIED)` sans droits administrateur.
pub fn set_auto_start(name: &str, on: bool) -> Result<(), u32> {
    let service = open(name, SERVICE_CHANGE_CONFIG)?;
    let start = if on { SERVICE_AUTO_START } else { SERVICE_DEMAND_START };
    let ok = unsafe {
        ChangeServiceConfigW(
            service.0,
            SERVICE_NO_CHANGE,
            start,
            SERVICE_NO_CHANGE,
            null(),
            null(),
            std::ptr::null_mut(),
            null(),
            null(),
            null(),
            null(),
        )
    };
    if ok == 0 { Err(unsafe { GetLastError() }) } else { Ok(()) }
}

/// Démarre le serveur de streaming, et la console si elle était arrêtée.
pub fn start_server() -> Result<()> {
    start(KYBER)?;
    if state(ADMIN) == State::Stopped {
        start(ADMIN)?;
    }
    wait(KYBER, State::Running, Duration::from_secs(30))
}

/// Arrête le serveur de streaming. La console reste disponible.
pub fn stop_server() -> Result<()> {
    stop(KYBER)?;
    wait(KYBER, State::Stopped, Duration::from_secs(30))
}

/// Relance le serveur de streaming et la console.
pub fn restart_server() -> Result<()> {
    for name in [ADMIN, KYBER] {
        if state(name) != State::Stopped {
            stop(name)?;
        }
    }
    for name in [ADMIN, KYBER] {
        wait(name, State::Stopped, Duration::from_secs(30))?;
    }
    start(KYBER)?;
    start(ADMIN)?;
    wait(KYBER, State::Running, Duration::from_secs(30))?;
    wait(ADMIN, State::Running, Duration::from_secs(30))
}
