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
/// Nom affiché du service (le nom technique `kyber-admin` ne change pas).
/// Repris par l'applicateur des mises à jour (`update::apply`) pour les
/// installations faites avant le renommage.
pub const DISPLAY_NAME: &str = "KyberNow Admin";
pub const DESCRIPTION: &str = "Administration web de KyberNow Station";

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
        display_name: OsString::from(DISPLAY_NAME),
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
    service.set_description(DESCRIPTION)?;
    set_failure_actions(&service);
    service.start::<&str>(&[]).context("démarrage du service")?;
    println!("Service {SERVICE_NAME} installé et démarré.");
    Ok(())
}

/// Relance automatique après un arrêt anormal ou en échec (panique) : à 5 s,
/// 5 s puis 30 s, compteur remis à zéro au bout d'un jour. Reposée à chaque
/// démarrage d'Admin (`update::apply`), comme celle de `kyber`.
fn set_failure_actions(service: &windows_service::service::Service) {
    use windows_service::service::{ServiceAction, ServiceActionType, ServiceFailureActions, ServiceFailureResetPeriod};
    let restart = |secs| ServiceAction { action_type: ServiceActionType::Restart, delay: Duration::from_secs(secs) };
    let actions = ServiceFailureActions {
        reset_period: ServiceFailureResetPeriod::After(Duration::from_secs(86400)),
        reboot_msg: None,
        command: None,
        actions: Some(vec![restart(5), restart(5), restart(30)]),
    };
    if let Err(e) = service.update_failure_actions(actions) {
        eprintln!("relance automatique du service non posée : {e}");
    }
    let _ = service.set_failure_actions_on_non_crash_failures(true);
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
        // Arrêt en échec : le gestionnaire de services relance Admin.
        std::process::exit(1);
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
