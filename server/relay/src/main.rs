//! `kybernow-relay` : le relais de KyberNow.
//!
//! - `kybernow-relay [--config /config/relay.toml]` : lance le relais ;
//! - `kybernow-relay --healthcheck` : contrôle de santé de Docker (`/healthz`
//!   sur l'adresse des métriques), code de sortie 0 ou 1 ;
//! - `kybernow-relay --version` ;
//! - `kybernow-relay accounts add <e-mail> [--name <nom>] [--password-file <fichier>]` :
//!   crée un compte déjà vérifié (docs/comptes.md : sans SMTP, l'administrateur du
//!   relais crée les comptes) ; sans `--password-file`, le mot de passe se lit sur
//!   l'entrée standard.

use std::path::PathBuf;
use std::process::ExitCode;

use kybernow_relay::config::Config;

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    let mut config: Option<PathBuf> = std::env::var_os("KN_RELAY_CONFIG").map(PathBuf::from);
    let mut healthcheck = false;
    let mut command: Option<Vec<String>> = None;
    while let Some(a) = args.next() {
        match a.as_str() {
            "--config" => config = args.next().map(PathBuf::from),
            "--healthcheck" => healthcheck = true,
            "accounts" => command = Some(args.by_ref().collect()),
            "--version" => {
                println!("kybernow-relay {}", kybernow_relay::hub::VERSION);
                return ExitCode::SUCCESS;
            }
            other => {
                eprintln!("argument inconnu : {other}\n{USAGE}");
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
    if let Some(command) = command {
        return accounts_command(&cfg, &command);
    }
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

const USAGE: &str = "usage : kybernow-relay [--config fichier.toml] [--healthcheck] [--version]\n        kybernow-relay [--config fichier.toml] accounts add <e-mail> [--name <nom>] [--password-file <fichier>]";

/// `kybernow-relay accounts …`.
fn accounts_command(cfg: &Config, command: &[String]) -> ExitCode {
    use kybernow_relay::accounts::{Accounts, Keys, password::Hasher, store::Store, system_clock};
    let mut it = command.iter();
    if it.next().map(String::as_str) != Some("add") {
        eprintln!("{USAGE}");
        return ExitCode::from(2);
    }
    let (mut email, mut name, mut password_file) = (None, String::new(), None);
    while let Some(a) = it.next() {
        match a.as_str() {
            "--name" => name = it.next().cloned().unwrap_or_default(),
            "--password-file" => password_file = it.next().map(PathBuf::from),
            other if email.is_none() && !other.starts_with("--") => email = Some(other.to_string()),
            other => {
                eprintln!("argument inconnu : {other}\n{USAGE}");
                return ExitCode::from(2);
            }
        }
    }
    let Some(email) = email else {
        eprintln!("l'adresse e-mail manque\n{USAGE}");
        return ExitCode::from(2);
    };
    let password = match password_file {
        Some(p) => match std::fs::read_to_string(&p) {
            Ok(t) => t.trim_end_matches(['\r', '\n']).to_string(),
            Err(e) => {
                eprintln!("{} : {e}", p.display());
                return ExitCode::FAILURE;
            }
        },
        None => {
            if std::io::IsTerminal::is_terminal(&std::io::stdin()) {
                eprint!("Mot de passe (10 caractères au moins, visible à l'écran) : ");
            }
            let mut line = String::new();
            if std::io::stdin().read_line(&mut line).is_err() {
                return ExitCode::FAILURE;
            }
            line.trim_end_matches(['\r', '\n']).to_string()
        }
    };
    if !cfg.accounts.enabled {
        eprintln!("note : les comptes sont coupés sur ce relais ([accounts] enabled = false)");
    }
    let clock = system_clock();
    let opened = Keys::load_or_create(&cfg.accounts.secret_key).and_then(|keys| Ok((keys, Store::open(&cfg.accounts.database, clock())?)));
    let (keys, store) = match opened {
        Ok(o) => o,
        Err(e) => {
            eprintln!("comptes : {e:#}");
            return ExitCode::FAILURE;
        }
    };
    let accounts = Accounts::with(store, keys, None, cfg.public_url(cfg.link_port(cfg.listen.port())), Hasher::new(), clock);
    let name = if name.trim().is_empty() { email.split('@').next().unwrap_or("").to_string() } else { name };
    match accounts.add_verified(&email, &name, &password) {
        Ok(_) => {
            println!("compte créé, adresse vérifiée");
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("compte non créé : {e:#}");
            ExitCode::FAILURE
        }
    }
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
