//! kyber-admin : interface web d'administration d'un serveur Kyber.

mod api;
mod assets;
mod auth;
mod display;
mod gpu;
mod kyber_config;
mod logs;
mod schema;
mod service;
mod settings;
mod tls;
#[cfg(windows)]
mod winsvc;

use std::future::Future;
use std::net::SocketAddr;
use std::ops::Deref;
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use anyhow::{Context, Result};
use axum::Router;
use clap::{Args, Parser, Subcommand};

use crate::kyber_config::KyberConfig;
use crate::settings::Settings;

#[derive(Parser)]
#[command(version, about = "Interface web d'administration d'un serveur Kyber")]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
    #[command(flatten)]
    run: RunArgs,
}

#[derive(Args, Clone, Default)]
pub struct RunArgs {
    /// Dossier des données de kyber-admin (réglages, certificat, sauvegardes)
    #[arg(long, global = true)]
    data_dir: Option<PathBuf>,
    /// Dossier d'installation de Kyber (contient kyber_config.toml)
    #[arg(long, global = true)]
    kyber_dir: Option<PathBuf>,
    /// Adresse d'écoute, par défaut 0.0.0.0:9443
    #[arg(long, global = true)]
    listen: Option<String>,
    /// HTTP sans TLS, réservé au développement local
    #[arg(long)]
    http: bool,
}

#[derive(Subcommand)]
enum Command {
    /// Configuration initiale, utilisée par l'installateur KyberNow.
    /// Le mot de passe (Kyber et console) est lu dans la variable KYBERNOW_PASSWORD.
    Init {
        /// Identifiant du compte Kyber (connexion Basic)
        #[arg(long)]
        user: String,
        /// Port d'écoute du serveur Kyber (TCP et UDP)
        #[arg(long)]
        kyber_port: u16,
        /// Encodeur vidéo (nvenc, x264, amf, qsv)
        #[arg(long)]
        encoder: Option<String>,
    },
    /// Efface le mot de passe administrateur (un nouveau code d'initialisation sera émis)
    ResetPassword,
    /// Agent d'affichage, lancé par le service dans la session de l'écran
    #[command(hide = true)]
    DisplayAgent {
        #[command(subcommand)]
        action: AgentAction,
    },
    /// Gestion du service Windows kyber-admin
    #[cfg(windows)]
    Service {
        #[command(subcommand)]
        action: winsvc::Action,
    },
    /// Point d'entrée utilisé par le gestionnaire de services Windows
    #[cfg(windows)]
    #[command(hide = true)]
    ServiceRun,
}

#[derive(Subcommand)]
enum AgentAction {
    Isolate {
        #[arg(long, allow_hyphen_values = true)]
        luid_low: u32,
        #[arg(long, allow_hyphen_values = true)]
        luid_high: i32,
        #[arg(long)]
        target: u32,
        #[arg(long)]
        width: u32,
        #[arg(long)]
        height: u32,
        #[arg(long)]
        refresh: u32,
        #[arg(long)]
        out: PathBuf,
    },
    Restore {
        #[arg(long)]
        out: PathBuf,
    },
}

/// Exécute une opération d'affichage et écrit son résultat (JSON) dans `out`.
fn display_agent(action: AgentAction) -> Result<()> {
    #[cfg(windows)]
    let (out, result) = match action {
        AgentAction::Isolate { luid_low, luid_high, target, width, height, refresh, out } => (
            out,
            display::topology::isolate(luid_low, luid_high, target, width, height, refresh)
                .and_then(|a| Ok(serde_json::to_value(a)?)),
        ),
        AgentAction::Restore { out } => (out, display::topology::restore().map(|_| serde_json::json!({ "ok": true }))),
    };
    #[cfg(not(windows))]
    let (out, result): (PathBuf, Result<serde_json::Value>) = match action {
        AgentAction::Isolate { out, .. } | AgentAction::Restore { out } => (out, Err(anyhow::anyhow!("réservé à Windows"))),
    };
    let value = match result {
        Ok(v) => v,
        Err(e) => serde_json::json!({ "error": format!("{e:#}") }),
    };
    std::fs::write(out, serde_json::to_vec(&value)?)?;
    Ok(())
}

pub struct Inner {
    pub auth: Arc<auth::Auth>,
    pub kyber: KyberConfig,
    pub display: Arc<display::Manager>,
    pub service: Box<dyn service::ServiceControl>,
    pub data_dir: PathBuf,
    pub kyber_dir: PathBuf,
    pub settings: Mutex<Settings>,
    pub restart_pending: AtomicBool,
    pub secure_cookies: bool,
}

#[derive(Clone)]
pub struct AppState(Arc<Inner>);

impl Deref for AppState {
    type Target = Inner;
    fn deref(&self) -> &Inner {
        &self.0
    }
}

impl AppState {
    pub fn log_dir(&self) -> PathBuf {
        self.kyber_dir.join("log")
    }

    pub fn update_settings(&self, f: impl FnOnce(&mut Settings)) -> Result<()> {
        let mut settings = self.settings.lock().unwrap();
        f(&mut settings);
        settings.save(&self.data_dir)
    }
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let data_dir = cli.run.data_dir.clone().unwrap_or_else(settings::default_data_dir);

    match cli.command {
        Some(Command::Init { user, kyber_port, encoder }) => init(&cli.run, &data_dir, &user, kyber_port, encoder),
        Some(Command::DisplayAgent { action }) => display_agent(action),
        Some(Command::ResetPassword) => {
            let mut s = Settings::load(&data_dir)?;
            s.admin_password_hash = None;
            s.save(&data_dir)?;
            println!("Mot de passe effacé. Redémarrez kyber-admin pour obtenir un code d'initialisation.");
            Ok(())
        }
        #[cfg(windows)]
        Some(Command::Service { action }) => winsvc::manage(action),
        #[cfg(windows)]
        Some(Command::ServiceRun) => winsvc::dispatch(),
        None => {
            init_logging(None);
            let runtime = tokio::runtime::Runtime::new()?;
            runtime.block_on(run(cli.run, async {
                let _ = tokio::signal::ctrl_c().await;
            }))
        }
    }
}

fn init(args: &RunArgs, data_dir: &std::path::Path, user: &str, kyber_port: u16, encoder: Option<String>) -> Result<()> {
    let password = std::env::var("KYBERNOW_PASSWORD").context("variable KYBERNOW_PASSWORD absente")?;
    let kyber_dir = args
        .kyber_dir
        .clone()
        .or_else(settings::default_kyber_dir)
        .context("précisez le dossier de Kyber avec --kyber-dir")?;
    std::fs::create_dir_all(data_dir)?;
    let config = KyberConfig::new(kyber_dir.join("kyber_config.toml"), data_dir.join("backups"));

    let mut changes = std::collections::BTreeMap::new();
    changes.insert("network.port".to_string(), Some(serde_json::Value::from(kyber_port)));
    if let Some(encoder) = encoder {
        changes.insert("video.encoder".to_string(), Some(serde_json::Value::from(encoder)));
    }
    config.apply(&changes)?;
    config.reset_users(user, &password)?;
    let secret: [u8; 32] = rand::random();
    config.set_jwt_secret(&hex::encode(secret))?;

    let mut s = Settings::load(data_dir)?;
    s.admin_password_hash = Some(auth::hash_password(&password)?);
    s.kyber_dir = Some(kyber_dir);
    if let Some(listen) = &args.listen {
        listen.parse::<SocketAddr>().context("adresse d'écoute invalide")?;
        s.listen = Some(listen.clone());
    }
    s.save(data_dir)?;
    println!("Configuration initiale terminée.");
    Ok(())
}

pub fn init_logging(file: Option<PathBuf>) {
    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"));
    let builder = tracing_subscriber::fmt().with_env_filter(filter);
    match file.and_then(|p| std::fs::OpenOptions::new().create(true).append(true).open(p).ok()) {
        Some(f) => builder.with_ansi(false).with_writer(Mutex::new(f)).init(),
        None => builder.init(),
    }
}

pub async fn run(args: RunArgs, shutdown: impl Future<Output = ()> + Send + 'static) -> Result<()> {
    let _ = rustls::crypto::ring::default_provider().install_default();

    let data_dir = args.data_dir.unwrap_or_else(settings::default_data_dir);
    std::fs::create_dir_all(&data_dir)
        .with_context(|| format!("création de {}", data_dir.display()))?;
    let settings = Settings::load(&data_dir)?;

    let kyber_dir = args
        .kyber_dir
        .or_else(|| settings.kyber_dir.clone())
        .or_else(settings::default_kyber_dir)
        .context("précisez le dossier de Kyber avec --kyber-dir")?;
    let config_path = kyber_dir.join("kyber_config.toml");
    if !config_path.exists() {
        tracing::warn!("{} introuvable : Kyber est-il installé ?", config_path.display());
    }
    let listen: SocketAddr = args
        .listen
        .or_else(|| settings.listen.clone())
        .unwrap_or_else(|| settings::DEFAULT_LISTEN.to_string())
        .parse()
        .context("adresse d'écoute invalide")?;

    let service = service::platform();
    if service.is_simulated() {
        tracing::warn!("service Kyber simulé (pas sous Windows) : démarrer/arrêter ne fait rien");
    }

    // Écran virtuel : nettoyage d'une session interrompue, puis surveillance.
    let display = display::Manager::new(data_dir.clone(), settings.virtual_display.clone());
    {
        let d = display.clone();
        tokio::task::spawn_blocking(move || d.recover()).await?;
    }
    display.clone().supervise();

    let state = AppState(Arc::new(Inner {
        auth: auth::Auth::new(data_dir.clone(), settings.admin_password_hash.clone()),
        kyber: KyberConfig::new(config_path, data_dir.join("backups")),
        display,
        service,
        data_dir: data_dir.clone(),
        kyber_dir,
        settings: Mutex::new(settings),
        restart_pending: AtomicBool::new(false),
        secure_cookies: !args.http,
    }));

    let app = Router::new()
        .merge(api::router(state.clone()))
        .fallback(assets::serve)
        .with_state(state);

    let handle = axum_server::Handle::new();
    let h = handle.clone();
    tokio::spawn(async move {
        shutdown.await;
        tracing::info!("arrêt de kyber-admin");
        h.graceful_shutdown(Some(Duration::from_secs(5)));
    });

    if args.http {
        tracing::info!("kyber-admin en écoute sur http://{listen} (sans TLS)");
        axum_server::bind(listen).handle(handle).serve(app.into_make_service()).await?;
    } else {
        let (cert, key) = tls::ensure_certificate(&data_dir)?;
        let tls = axum_server::tls_rustls::RustlsConfig::from_pem_file(cert, key).await?;
        tracing::info!("kyber-admin en écoute sur https://{listen}");
        axum_server::bind_rustls(listen, tls).handle(handle).serve(app.into_make_service()).await?;
    }
    Ok(())
}
