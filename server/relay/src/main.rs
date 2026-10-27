//! `kybernow-relay` : le relais de KyberNow.
//!
//! - `kybernow-relay [--config /config/relay.toml]` : lance le relais ;
//! - `kybernow-relay --healthcheck` : contrôle de santé de Docker (`/healthz`
//!   sur l'adresse des métriques), code de sortie 0 ou 1 ;
//! - `kybernow-relay --version`.

use std::path::PathBuf;
use std::process::ExitCode;

use kybernow_relay::config::Config;

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    let mut config: Option<PathBuf> = std::env::var_os("KN_RELAY_CONFIG").map(PathBuf::from);
    let mut healthcheck = false;
    while let Some(a) = args.next() {
        match a.as_str() {
            "--config" => config = args.next().map(PathBuf::from),
            "--healthcheck" => healthcheck = true,
            "--version" => {
                println!("kybernow-relay {}", kybernow_relay::hub::VERSION);
                return ExitCode::SUCCESS;
            }
            other => {
                eprintln!("argument inconnu : {other}\nusage : kybernow-relay [--config fichier.toml] [--healthcheck] [--version]");
                return ExitCode::from(2);
            }
        }
    }
    let config = config.unwrap_or_else(|| PathBuf::from("/config/relay.toml"));
    let cfg = match Config::load(Some(&config)) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("réglages : {e:#}");
            return ExitCode::from(2);
        }
    };
    if healthcheck {
        let ok = cfg.metrics.listen.is_some_and(kybernow_relay::metrics::healthcheck);
        return if ok { ExitCode::SUCCESS } else { ExitCode::FAILURE };
    }
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")))
        .with_target(false)
        // Pas de couleurs dans les journaux de Docker.
        .with_ansi(std::io::IsTerminal::is_terminal(&std::io::stdout()))
        .init();
    let rt = match tokio::runtime::Builder::new_multi_thread().enable_all().build() {
        Ok(rt) => rt,
        Err(e) => {
            eprintln!("runtime : {e}");
            return ExitCode::FAILURE;
        }
    };
    rt.block_on(async move {
        let relay = match kybernow_relay::start(cfg).await {
            Ok(r) => r,
            Err(e) => {
                tracing::error!("démarrage impossible : {e:#}");
                return ExitCode::FAILURE;
            }
        };
        wait_for_stop().await;
        tracing::info!("arrêt du relais");
        drop(relay);
        ExitCode::SUCCESS
    })
}

#[cfg(unix)]
async fn wait_for_stop() {
    use tokio::signal::unix::{SignalKind, signal};
    let mut term = signal(SignalKind::terminate()).expect("signal TERM");
    tokio::select! {
        _ = tokio::signal::ctrl_c() => {}
        _ = term.recv() => {}
    }
}

#[cfg(not(unix))]
async fn wait_for_stop() {
    let _ = tokio::signal::ctrl_c().await;
}
