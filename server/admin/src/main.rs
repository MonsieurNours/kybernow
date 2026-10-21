//! kyber-admin : interface web d'administration d'un serveur Kyber.

mod activity;
mod api;
mod assets;
mod audio;
mod auth;
mod discovery;
mod display;
mod foreground;
mod gpu;
mod kyber_config;
mod kyber_proxy;
mod local_token;
mod logs;
mod pairing;
mod preview;
mod schema;
mod service;
mod settings;
mod system;
mod tls;
mod update;
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
    /// Annonce Bonjour avec `kyber=1` même sans serveur Kyber (développement :
    /// tester l'ajout et l'appairage depuis les apps)
    #[arg(long)]
    announce_kyber: bool,
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
    /// Remet un mode sur l'écran `gdi_name` (politique « restore »).
    SetMode {
        #[arg(long)]
        gdi_name: String,
        #[arg(long)]
        width: u32,
        #[arg(long)]
        height: u32,
        #[arg(long)]
        refresh: u32,
        #[arg(long)]
        out: PathBuf,
    },
    /// Son vers les haut-parleurs virtuels, à `channels` canaux.
    AudioRoute {
        #[arg(long)]
        channels: u16,
        #[arg(long)]
        out: PathBuf,
    },
    /// Veille de l'écran virtuel (mode courant, écrans actifs) pendant une session.
    Watch {
        #[arg(long)]
        dir: PathBuf,
        /// Jeton de la session : l'agent s'arrête quand `run` en porte un autre.
        #[arg(long)]
        token: String,
        #[arg(long, allow_hyphen_values = true)]
        luid_low: u32,
        #[arg(long, allow_hyphen_values = true)]
        luid_high: i32,
        #[arg(long)]
        target: u32,
    },
    /// Veille de l'app au premier plan (plein écran, jeu) pendant une session.
    Foreground {
        #[arg(long)]
        dir: PathBuf,
    },
    /// Copie le jeton des routes locales dans le profil de l'utilisateur de la session.
    LocalToken {
        #[arg(long)]
        dir: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },
    /// Aperçu de l'écran principal en JPEG, `width` pixels de large.
    Preview {
        #[arg(long)]
        width: u32,
        #[arg(long)]
        jpeg: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },
    /// Rétablit la sortie audio d'avant la session.
    AudioRestore {
        #[arg(long, allow_hyphen_values = true)]
        previous: String,
        #[arg(long)]
        previous_format: Option<String>,
        #[arg(long, allow_hyphen_values = true)]
        speakers: String,
        #[arg(long)]
        out: PathBuf,
    },
}

/// Dossier de travail de l'agent (celui de son fichier de résultat).
#[cfg(windows)]
fn agent_dir(out: &std::path::Path) -> &std::path::Path {
    out.parent().unwrap_or(std::path::Path::new("."))
}

/// Exécute une opération d'affichage et écrit son résultat (JSON) dans `out`.
fn display_agent(action: AgentAction) -> Result<()> {
    // Boucle de veille : pas de résultat unique à écrire.
    if let AgentAction::Foreground { dir } = &action {
        return foreground::run_agent(dir);
    }
    if let AgentAction::Watch { dir, token, luid_low, luid_high, target } = &action {
        #[cfg(windows)]
        return display::watch::run_agent(dir, token, *luid_low, *luid_high, *target);
        #[cfg(not(windows))]
        {
            let _ = (dir, token, luid_low, luid_high, target);
            anyhow::bail!("réservé à Windows");
        }
    }
    #[cfg(windows)]
    let (out, result) = match action {
        AgentAction::Isolate { luid_low, luid_high, target, width, height, refresh, out } => {
            display::pointer::begin(agent_dir(&out));
            let r = display::topology::isolate(luid_low, luid_high, target, width, height, refresh)
                .and_then(|a| Ok(serde_json::to_value(a)?));
            (out, r)
        }
        AgentAction::Restore { out } => {
            display::pointer::end(agent_dir(&out));
            (out, display::topology::restore().map(|_| serde_json::json!({ "ok": true })))
        }
        AgentAction::SetMode { gdi_name, width, height, refresh, out } => {
            let (w, h, hz) = display::topology::set_mode(&gdi_name, width, height, refresh);
            (out, Ok(serde_json::json!({ "width": w, "height": h, "refresh": hz })))
        }
        AgentAction::AudioRoute { channels, out } => {
            display::pointer::begin(agent_dir(&out));
            (out, audio::endpoint::route(channels).and_then(|r| Ok(serde_json::to_value(r)?)))
        }
        AgentAction::AudioRestore { previous, previous_format, speakers, out } => {
            display::pointer::end(agent_dir(&out));
            let r = audio::endpoint::restore(&previous, previous_format.as_deref(), &speakers)
                .map(|_| serde_json::json!({ "ok": true }));
            (out, r)
        }
        AgentAction::Preview { width, jpeg, out } => (out, preview::capture_to(&jpeg, width).map(|_| serde_json::json!({ "ok": true }))),
        AgentAction::LocalToken { dir, out } => (out, local_token::hand_over(&dir)),
        AgentAction::Foreground { .. } | AgentAction::Watch { .. } => unreachable!(),
    };
    #[cfg(not(windows))]
    let (out, result): (PathBuf, Result<serde_json::Value>) = match action {
        AgentAction::Isolate { out, .. }
        | AgentAction::Restore { out }
        | AgentAction::SetMode { out, .. }
        | AgentAction::AudioRoute { out, .. }
        | AgentAction::AudioRestore { out, .. }
        | AgentAction::LocalToken { out, .. } => (out, Err(anyhow::anyhow!("réservé à Windows"))),
        AgentAction::Preview { width, jpeg, out } => (out, preview::capture_to(&jpeg, width).map(|_| serde_json::json!({ "ok": true }))),
        AgentAction::Foreground { .. } | AgentAction::Watch { .. } => unreachable!(),
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
    pub audio: Arc<audio::Router>,
    /// App au premier plan pendant une session (plein écran, jeu).
    pub foreground: foreground::Watcher,
    /// Aperçu de l'écran pour l'accueil des apps et la console.
    pub preview: preview::Preview,
    /// Appareils connus, session en cours, fil des événements.
    pub activity: Arc<activity::Activity>,
    /// Demandes d'appairage par code (app Apple TV), en mémoire.
    pub pairing: pairing::Pairing,
    /// Jeton des routes locales (icône de la zone de notification).
    pub local_token: Arc<local_token::LocalToken>,
    /// Mises à jour depuis les versions publiées sur GitHub.
    pub updater: Arc<update::Updater>,
    pub service: Box<dyn service::ServiceControl>,
    pub data_dir: PathBuf,
    pub kyber_dir: PathBuf,
    pub settings: Mutex<Settings>,
    pub restart_pending: AtomicBool,
    pub secure_cookies: bool,
    /// Empreinte du certificat de la console (appairage des apps).
    pub tls_fingerprint: Option<String>,
    /// Port d'écoute de la console.
    pub listen_port: u16,
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
    /// Dossiers de journaux : Kyber 0.29 écrit dans `%ProgramData%\\Kyber\\logs`
    /// (les versions précédentes à côté de l'exécutable), la console dans son
    /// dossier de données.
    pub fn log_sources(&self) -> Vec<logs::Source> {
        let mut sources = Vec::new();
        if let Some(pd) = std::env::var_os("ProgramData") {
            sources.push(logs::Source { prefix: "kyber", dir: PathBuf::from(pd).join("Kyber").join("logs") });
        }
        sources.push(logs::Source { prefix: "kyber", dir: self.kyber_dir.join("log") });
        sources.push(logs::Source { prefix: "kybernow", dir: self.data_dir.clone() });
        sources
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
    config.set_jwt_secret_file(&data_dir.join(kyber_config::JWT_KEY_FILE), &hex::encode(secret))?;
    config.secure_secrets();

    let mut s = Settings::load(data_dir)?;
    s.admin_password_hash = Some(auth::hash_password(&password)?);
    s.install_account = Some(user.to_string());
    // Le certificat de la console existe dès l'installation : l'icône
    // l'épingle par l'empreinte notée dans kybernow.json.
    let (cert, _) = tls::ensure_certificate(data_dir)?;
    if let Some(fp) = tls::fingerprint(&cert) {
        tls::publish_fingerprint(&fp, &kyber_dir);
    }
    s.kyber_dir = Some(kyber_dir);
    if let Some(listen) = &args.listen {
        listen.parse::<SocketAddr>().context("adresse d'écoute invalide")?;
        s.listen = Some(listen.clone());
    }
    s.save(data_dir)?;
    println!("Configuration initiale terminée.");
    Ok(())
}

/// Au démarrage : les secrets de Kyber réservés à SYSTEM et aux Administrateurs
/// (une mise à jour les a remplacés par des fichiers aux droits de Program
/// Files), la clé JWT déplacée de `kyber_config.toml` vers le dossier de
/// données, et le compte d'installation mémorisé s'il ne l'est pas encore
/// (installation d'avant 0.2.6 : le premier compte de la liste).
fn kyber_secrets_at_start(kyber: &KyberConfig, mut settings: Settings, data_dir: &std::path::Path) -> Settings {
    if !kyber.path().exists() {
        return settings;
    }
    match kyber.move_jwt_secret_to_file(&data_dir.join(kyber_config::JWT_KEY_FILE)) {
        Ok(true) => tracing::info!("clé JWT de Kyber déplacée dans {}", kyber_config::JWT_KEY_FILE),
        Ok(false) => {}
        Err(e) => tracing::warn!("clé JWT de Kyber non déplacée : {e:#}"),
    }
    kyber.secure_secrets();
    if settings.install_account.is_none()
        && let Some(first) = kyber.users().ok().and_then(|u| u.into_iter().next())
    {
        settings.install_account = Some(first.username.clone());
        match settings.save(data_dir) {
            Ok(()) => tracing::info!("compte d'installation mémorisé : {}", first.username),
            Err(e) => tracing::warn!("compte d'installation non mémorisé : {e:#}"),
        }
    }
    settings
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

/// Tout ce qui n'est pas l'API : la console, le client web (`/ecran`) et les
/// appels de son moteur, relayés à Kyber.
async fn web(state: axum::extract::State<AppState>, req: axum::extract::Request) -> axum::response::Response {
    let path = req.uri().path();
    if kyber_proxy::is_kyber_path(path) || kyber_proxy::engine_file(path).is_some() {
        return kyber_proxy::proxy(state, req).await;
    }
    assets::serve(req.uri().clone()).await
}

pub async fn run(args: RunArgs, shutdown: impl Future<Output = ()> + Send + 'static) -> Result<()> {
    let _ = rustls::crypto::ring::default_provider().install_default();

    let data_dir = args.data_dir.unwrap_or_else(settings::default_data_dir);
    std::fs::create_dir_all(&data_dir)
        .with_context(|| format!("création de {}", data_dir.display()))?;
    update::secure_data_dir(&data_dir);
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
    let kyber = KyberConfig::new(config_path.clone(), data_dir.join("backups"));
    let settings = kyber_secrets_at_start(&kyber, settings, &data_dir);
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

    // Cartes graphiques : liste et activité relevées en fond (flux en cours, console).
    gpu::start();

    // Écran virtuel : nettoyage d'une session interrompue, puis surveillance.
    let display = display::Manager::new(data_dir.clone(), settings.virtual_display.clone());
    {
        let d = display.clone();
        tokio::task::spawn_blocking(move || d.recover()).await?;
    }
    display.clone().supervise();

    // Sortie audio virtuelle : même principe.
    let audio = audio::Router::new(data_dir.clone(), settings.virtual_audio.clone());
    {
        let a = audio.clone();
        tokio::task::spawn_blocking(move || a.recover()).await?;
    }
    audio.clone().supervise();

    // Certificat (créé au premier lancement) et annonce sur le réseau local.
    let (cert, key) = tls::ensure_certificate(&data_dir)?;
    let tls_fingerprint = tls::fingerprint(&cert);
    if let Some(fp) = &tls_fingerprint {
        tls::publish_fingerprint(fp, &kyber_dir);
    }
    // Les apps n'affichent que les consoles qui ont un serveur Kyber derrière.
    let kyber_ready = discovery::kyber_ready(args.announce_kyber, config_path.exists(), service.as_ref());
    if !kyber_ready {
        tracing::warn!("pas de serveur Kyber utilisable : annoncée avec kyber=0, les apps ne la proposent pas");
    }
    let _bonjour = discovery::announce(listen.port(), tls_fingerprint.as_deref(), kyber_ready);

    // Mises à jour : vérification de fond, et relance de l'icône après une installation.
    let updater = Arc::new(update::Updater::new(&data_dir));
    updater.clone().supervise();
    update::relaunch_tray_if_asked(&data_dir);

    // Jeton des routes locales : tiré maintenant, puis à chaque ouverture de session.
    let local_token = local_token::LocalToken::new(&data_dir);
    {
        let t = local_token.clone();
        tokio::task::spawn_blocking(move || t.renew()).await?;
    }
    local_token.clone().supervise();

    // Fil des événements : l'écran virtuel y annonce ce que la veille voit.
    let activity = Arc::new(activity::Activity::new(&data_dir));
    {
        let a = activity.clone();
        display.on_event(Arc::new(move |kind: &str, text: &str| a.event(kind, text)));
    }

    let state = AppState(Arc::new(Inner {
        auth: auth::Auth::new(data_dir.clone(), settings.admin_password_hash.clone()),
        kyber,
        display,
        audio,
        foreground: foreground::Watcher::new(&data_dir),
        preview: preview::Preview::new(&data_dir),
        activity,
        pairing: pairing::Pairing::new(),
        local_token,
        updater: updater.clone(),
        service,
        data_dir: data_dir.clone(),
        kyber_dir,
        settings: Mutex::new(settings),
        restart_pending: AtomicBool::new(false),
        secure_cookies: !args.http,
        tls_fingerprint,
        listen_port: listen.port(),
    }));

    let on_shutdown = state.clone();
    let app = Router::new()
        .merge(api::router(state.clone()))
        .fallback(web)
        .with_state(state);

    let handle = axum_server::Handle::new();
    let h = handle.clone();
    tokio::spawn(async move {
        shutdown.await;
        tracing::info!("arrêt de kyber-admin : écrans et son rendus à l'ordinateur");
        // Sans cela, un `Stop-Service` en pleine session laissait le son sur
        // les haut-parleurs virtuels et les écrans éteints jusqu'au prochain
        // démarrage (où `recover` les aurait rendus).
        let st = on_shutdown.clone();
        let _ = tokio::task::spawn_blocking(move || {
            st.foreground.stop();
            st.display.shutdown();
            st.audio.shutdown();
        })
        .await;
        h.graceful_shutdown(Some(Duration::from_secs(5)));
    });

    if args.http {
        tracing::info!("kyber-admin en écoute sur http://{listen} (sans TLS)");
        axum_server::bind(listen).handle(handle).serve(app.into_make_service_with_connect_info::<SocketAddr>()).await?;
    } else {
        let tls = axum_server::tls_rustls::RustlsConfig::from_pem_file(cert, key).await?;
        tracing::info!("kyber-admin en écoute sur https://{listen}");
        axum_server::bind_rustls(listen, tls).handle(handle).serve(app.into_make_service_with_connect_info::<SocketAddr>()).await?;
    }
    Ok(())
}
