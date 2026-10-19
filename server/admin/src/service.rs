//! Pilotage du service Windows `kyber`.
//!
//! Sur les autres systèmes (développement sur Mac), un service simulé garde
//! son état en mémoire pour pouvoir tester l'interface.

use std::time::Duration;

use anyhow::Result;
use serde::Serialize;

pub const KYBER_SERVICE: &str = "kyber";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ServiceState {
    Running,
    Stopped,
    Starting,
    Stopping,
    NotInstalled,
    Unknown,
}

pub trait ServiceControl: Send + Sync {
    fn state(&self) -> Result<ServiceState>;
    fn start(&self) -> Result<()>;
    fn stop(&self) -> Result<()>;
    fn is_simulated(&self) -> bool {
        false
    }
}

/// Arrête le service, attend qu'il soit arrêté, puis le redémarre.
pub async fn restart(control: &dyn ServiceControl) -> Result<()> {
    if control.state()? != ServiceState::Stopped {
        control.stop()?;
    }
    for _ in 0..60 {
        if control.state()? == ServiceState::Stopped {
            return control.start();
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
    anyhow::bail!("le service ne s'est pas arrêté au bout de 30 s")
}

pub fn platform() -> Box<dyn ServiceControl> {
    #[cfg(windows)]
    {
        Box::new(windows::WindowsService)
    }
    #[cfg(not(windows))]
    {
        Box::new(Simulated::default())
    }
}

#[cfg(not(windows))]
#[derive(Default)]
pub struct Simulated(std::sync::Mutex<bool>);

#[cfg(not(windows))]
impl ServiceControl for Simulated {
    fn state(&self) -> Result<ServiceState> {
        Ok(if *self.0.lock().unwrap() { ServiceState::Running } else { ServiceState::Stopped })
    }
    fn start(&self) -> Result<()> {
        *self.0.lock().unwrap() = true;
        tracing::info!("(simulé) démarrage du service {KYBER_SERVICE}");
        Ok(())
    }
    fn stop(&self) -> Result<()> {
        *self.0.lock().unwrap() = false;
        tracing::info!("(simulé) arrêt du service {KYBER_SERVICE}");
        Ok(())
    }
    fn is_simulated(&self) -> bool {
        true
    }
}

#[cfg(windows)]
mod windows {
    use std::ffi::OsStr;

    use anyhow::{Context, Result};
    use windows_service::service::{ServiceAccess, ServiceState as WinState};
    use windows_service::service_manager::{ServiceManager, ServiceManagerAccess};

    use super::{KYBER_SERVICE, ServiceControl, ServiceState};

    pub struct WindowsService;

    /// Code Windows ERROR_SERVICE_DOES_NOT_EXIST.
    const SERVICE_DOES_NOT_EXIST: i32 = 1060;

    fn open(access: ServiceAccess) -> Result<Option<windows_service::service::Service>> {
        let manager = ServiceManager::local_computer(None::<&str>, ServiceManagerAccess::CONNECT)
            .context("connexion au gestionnaire de services")?;
        match manager.open_service(KYBER_SERVICE, access) {
            Ok(s) => Ok(Some(s)),
            Err(windows_service::Error::Winapi(e))
                if e.raw_os_error() == Some(SERVICE_DOES_NOT_EXIST) =>
            {
                Ok(None)
            }
            Err(e) => Err(e).context("ouverture du service kyber"),
        }
    }

    impl ServiceControl for WindowsService {
        fn state(&self) -> Result<ServiceState> {
            let Some(service) = open(ServiceAccess::QUERY_STATUS)? else {
                return Ok(ServiceState::NotInstalled);
            };
            Ok(match service.query_status()?.current_state {
                WinState::Running => ServiceState::Running,
                WinState::Stopped => ServiceState::Stopped,
                WinState::StartPending | WinState::ContinuePending => ServiceState::Starting,
                WinState::StopPending | WinState::PausePending => ServiceState::Stopping,
                _ => ServiceState::Unknown,
            })
        }

        fn start(&self) -> Result<()> {
            let service = open(ServiceAccess::START)?.context("service kyber non installé")?;
            service.start::<&OsStr>(&[]).context("démarrage du service kyber")?;
            Ok(())
        }

        fn stop(&self) -> Result<()> {
            let service = open(ServiceAccess::STOP)?.context("service kyber non installé")?;
            service.stop().context("arrêt du service kyber")?;
            Ok(())
        }
    }
}
