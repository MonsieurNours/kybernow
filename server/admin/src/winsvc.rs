//! Intégration au gestionnaire de services Windows.

use std::ffi::OsString;
use std::time::Duration;

use anyhow::{Context, Result};
use clap::Subcommand;
use windows_service::service::{
    ServiceAccess, ServiceControl, ServiceControlAccept, ServiceErrorControl, ServiceExitCode,
    ServiceInfo, ServiceStartType, ServiceState, ServiceStatus, ServiceType, SessionChangeReason,
};
use windows_service::service_control_handler::{self, ServiceControlHandlerResult};
use windows_service::service_manager::{ServiceManager, ServiceManagerAccess};
use windows_service::{define_windows_service, service_dispatcher};

use crate::settings;

pub const SERVICE_NAME: &str = "kyber-admin";

#[derive(Subcommand)]
pub enum Action {
    /// Installe kyber-admin comme service Windows (démarrage automatique) et le démarre
    Install,
    /// Arrête et supprime le service
    Uninstall,
}

pub fn manage(action: Action) -> Result<()> {
    match action {
        Action::Install => install(),
        Action::Uninstall => uninstall(),
    }
}

fn install() -> Result<()> {
    let manager = ServiceManager::local_computer(
        None::<&str>,
        ServiceManagerAccess::CONNECT | ServiceManagerAccess::CREATE_SERVICE,
    )
    .context("connexion au gestionnaire de services (lancez en administrateur)")?;
    let info = ServiceInfo {
        name: OsString::from(SERVICE_NAME),
        display_name: OsString::from("KyberNow Console"),
        service_type: ServiceType::OWN_PROCESS,
        start_type: ServiceStartType::AutoStart,
        error_control: ServiceErrorControl::Normal,
        executable_path: std::env::current_exe()?,
        launch_arguments: vec![OsString::from("service-run")],
        dependencies: vec![],
        account_name: None, // LocalSystem : nécessaire pour piloter le service kyber
        account_password: None,
    };
    let service = manager
        .create_service(&info, ServiceAccess::CHANGE_CONFIG | ServiceAccess::START)
        .context("création du service")?;
    service.set_description("Console d'administration web de KyberNow")?;
    service.start::<&str>(&[]).context("démarrage du service")?;
    println!("Service {SERVICE_NAME} installé et démarré.");
    Ok(())
}

fn uninstall() -> Result<()> {
    let manager = ServiceManager::local_computer(None::<&str>, ServiceManagerAccess::CONNECT)?;
    let service = manager
        .open_service(
            SERVICE_NAME,
            ServiceAccess::QUERY_STATUS | ServiceAccess::STOP | ServiceAccess::DELETE,
        )
        .context("ouverture du service (lancez en administrateur)")?;
    if service.query_status()?.current_state != ServiceState::Stopped {
        let _ = service.stop();
        for _ in 0..20 {
            if service.query_status()?.current_state == ServiceState::Stopped {
                break;
            }
            std::thread::sleep(Duration::from_millis(500));
        }
    }
    service.delete()?;
    println!("Service {SERVICE_NAME} supprimé.");
    Ok(())
}

define_windows_service!(ffi_service_main, service_main);

pub fn dispatch() -> Result<()> {
    service_dispatcher::start(SERVICE_NAME, ffi_service_main).context("lancement en tant que service")
}

fn service_main(_args: Vec<OsString>) {
    let data_dir = settings::default_data_dir();
    let _ = std::fs::create_dir_all(&data_dir);
    crate::init_logging(Some(data_dir.join("kyber-admin.log")));
    if let Err(e) = run_service() {
        tracing::error!("kyber-admin s'est arrêté sur une erreur : {e:#}");
    }
}

fn service_status(state: ServiceState, accept: ServiceControlAccept, wait_hint: Duration) -> ServiceStatus {
    ServiceStatus {
        service_type: ServiceType::OWN_PROCESS,
        current_state: state,
        controls_accepted: accept,
        exit_code: ServiceExitCode::Win32(0),
        checkpoint: 0,
        wait_hint,
        process_id: None,
    }
}

fn run_service() -> Result<()> {
    let (tx, rx) = tokio::sync::oneshot::channel::<()>();
    let tx = std::sync::Mutex::new(Some(tx));
    // Le gestionnaire de services apprend que l'arrêt est en cours et peut
    // prendre jusqu'à 30 s : le temps de rendre les écrans et le son.
    let status_for_stop = std::sync::Arc::new(std::sync::Mutex::new(None::<service_control_handler::ServiceStatusHandle>));
    let status_in_handler = status_for_stop.clone();
    let handler = move |control| match control {
        ServiceControl::Stop | ServiceControl::Shutdown => {
            if let Some(status) = *status_in_handler.lock().unwrap() {
                let _ = status.set_service_status(service_status(
                    ServiceState::StopPending,
                    ServiceControlAccept::empty(),
                    Duration::from_secs(30),
                ));
            }
            if let Some(tx) = tx.lock().unwrap().take() {
                let _ = tx.send(());
            }
            ServiceControlHandlerResult::NoError
        }
        ServiceControl::Interrogate => ServiceControlHandlerResult::NoError,
        // Une session s'ouvre ou reprend l'écran : le jeton des routes locales
        // est renouvelé et copié dans cette session (voir `local_token`).
        ServiceControl::SessionChange(change) => {
            if matches!(change.reason, SessionChangeReason::SessionLogon | SessionChangeReason::ConsoleConnect) {
                crate::local_token::session_changed();
            }
            ServiceControlHandlerResult::NoError
        }
        _ => ServiceControlHandlerResult::NotImplemented,
    };
    let status = service_control_handler::register(SERVICE_NAME, handler)?;
    *status_for_stop.lock().unwrap() = Some(status);
    let set = |state, accept| status.set_service_status(service_status(state, accept, Duration::default()));
    set(ServiceState::Running, ServiceControlAccept::STOP | ServiceControlAccept::SHUTDOWN | ServiceControlAccept::SESSION_CHANGE)?;

    let runtime = tokio::runtime::Runtime::new()?;
    let result = runtime.block_on(crate::run(crate::RunArgs::default(), async {
        let _ = rx.await;
    }));
    set(ServiceState::Stopped, ServiceControlAccept::empty())?;
    result
}
