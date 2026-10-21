//! Console d'administration : son adresse, et l'état qu'elle donne à l'ordinateur
//! local (`GET /api/local/state` : serveur, session en cours, demandes d'accès).
//!
//! Les routes `/api/local/*` exigent, en plus de l'adresse locale, le jeton
//! que la console tire à son démarrage et à chaque ouverture de session, et
//! qu'un agent copie dans le profil du compte (`%LOCALAPPDATA%\KyberNow\local-token`,
//! lisible par ce compte seulement) : l'icône l'envoie dans `x-kyber-local`.

use std::path::PathBuf;
use std::time::Duration;

use serde_json::Value;

fn env_dir(var: &str, fallback: &str) -> PathBuf {
    std::env::var_os(var).map(PathBuf::from).unwrap_or_else(|| PathBuf::from(fallback))
}

/// Port de la console : `console_port` dans `C:\Program Files\KyberNow\kybernow.json`,
/// écrit par l'installateur, sans secret et lisible par tous. L'icône ne lit
/// rien dans `C:\ProgramData\KyberNow` : ce dossier (où `kyber-admin.toml`
/// garde l'empreinte du mot de passe) est réservé à SYSTEM et aux
/// administrateurs. Relu à chaque fois : une réinstallation peut changer le port.
pub fn port() -> Option<u16> {
    let json = env_dir("ProgramFiles", r"C:\Program Files").join(r"KyberNow\kybernow.json");
    let record: Value = serde_json::from_slice(&std::fs::read(json).ok()?).ok()?;
    record["console_port"].as_u64().and_then(|p| u16::try_from(p).ok())
}

pub fn url(port: u16) -> String {
    format!("https://localhost:{port}/")
}

/// En-tête du jeton des routes locales.
const LOCAL_HEADER: &str = "x-kyber-local";

/// Jeton des routes locales, copié par la console dans le profil du compte.
pub fn local_token_path() -> Option<PathBuf> {
    std::env::var_os("LOCALAPPDATA").map(|dir| PathBuf::from(dir).join(r"KyberNow\local-token"))
}

/// Relu à chaque appel : la console le renouvelle à chaque ouverture de
/// session et à chaque démarrage (un 403 veut dire qu'il a changé).
fn local_token() -> String {
    local_token_path().and_then(|p| std::fs::read_to_string(p).ok()).map(|t| t.trim().to_string()).unwrap_or_default()
}

/// Ce que la console dit de la session en cours.
#[derive(Debug, Clone, PartialEq)]
pub struct LocalState {
    pub session: bool,
    pub user: Option<String>,
    /// Version plus récente publiée sur GitHub, si la console en a vu une.
    pub update: Option<String>,
    pub version: Option<String>,
    pub hostname: Option<String>,
    pub address: Option<String>,
    /// Session d'une app KyberNow en cours (nom de l'appareil, début, latence).
    pub live: Option<Value>,
    /// Dernière session terminée.
    pub last: Option<Value>,
    /// Appareils qui demandent l'accès par code (id, nom, type ; jamais le
    /// code, qui se compare dans la console).
    pub pair_requests: Vec<Value>,
}

pub struct Client(ureq::Agent);

impl Client {
    pub fn new() -> Self {
        // La console a un certificat auto-signé ; on ne parle qu'à l'ordinateur lui-même.
        let tls = ureq::tls::TlsConfig::builder().disable_verification(true).build();
        let config = ureq::Agent::config_builder()
            .tls_config(tls)
            .timeout_global(Some(Duration::from_secs(3)))
            .http_status_as_error(true)
            .build();
        Client(config.new_agent())
    }

    /// La console a-t-elle un mot de passe ? (`GET /api/session`, sans compte)
    pub fn configured(&self, port: u16) -> Option<bool> {
        let mut response = self.0.get(&format!("https://127.0.0.1:{port}/api/session")).call().ok()?;
        let body: Value = response.body_mut().read_json().ok()?;
        body["configured"].as_bool()
    }

    pub fn local_state(&self, port: u16) -> Option<LocalState> {
        let mut response = self.0.get(&format!("https://127.0.0.1:{port}/api/local/state")).header(LOCAL_HEADER, &local_token()).call().ok()?;
        let body: Value = response.body_mut().read_json().ok()?;
        let text = |key: &str| body[key].as_str().map(str::to_owned);
        let some = |v: &Value| (!v.is_null()).then(|| v.clone());
        Some(LocalState {
            session: body["session"].as_bool().unwrap_or(false),
            user: text("user"),
            update: text("update_available"),
            version: text("version"),
            hostname: text("hostname"),
            address: text("address"),
            live: some(&body["activity"]["live"]),
            last: some(&body["activity"]["last"]),
            pair_requests: body["pair_requests"].as_array().cloned().unwrap_or_default(),
        })
    }

    /// Met fin à la session en cours (route réservée à l'ordinateur lui-même).
    pub fn end_session(&self, port: u16) -> anyhow::Result<()> {
        self.0
            .post(&format!("https://127.0.0.1:{port}/api/local/end"))
            .header("x-kyber-admin", "1")
            .header(LOCAL_HEADER, &local_token())
            .send_empty()
            .map_err(|e| anyhow::anyhow!("La console n'a pas pu mettre fin à la session ({e})."))?;
        Ok(())
    }
}

impl Client {
    /// Accepte ou refuse une demande d'accès par code (route réservée à l'ordinateur lui-même).
    pub fn pair_answer(&self, port: u16, id: &str, accept: bool) -> anyhow::Result<()> {
        let action = if accept { "accept" } else { "refuse" };
        self.0
            .post(&format!("https://127.0.0.1:{port}/api/local/pair/{id}/{action}"))
            .header("x-kyber-admin", "1")
            .header(LOCAL_HEADER, &local_token())
            .config()
            .timeout_global(Some(Duration::from_secs(15)))
            .build()
            .send_empty()
            .map_err(|e| match e {
                ureq::Error::StatusCode(410) => anyhow::anyhow!("Cette demande a expiré : l'appareil en enverra une nouvelle avec un autre code."),
                e => anyhow::anyhow!("La console n'a pas pu répondre à la demande ({e})."),
            })?;
        Ok(())
    }
}

/// Code d'initialisation écrit par la console quand son mot de passe a été
/// effacé (lisible en administrateur seulement).
pub fn data_setup_code_path() -> PathBuf {
    env_dir("ProgramData", r"C:\ProgramData").join(r"KyberNow\setup-code.txt")
}

/// Copie du code laissée par la partie élevée de « Réinitialiser le mot de
/// passe » dans le profil du compte (`%LOCALAPPDATA%\KyberNow\setup-code.txt`).
pub fn handoff_path() -> Option<PathBuf> {
    std::env::var_os("LOCALAPPDATA").map(|dir| PathBuf::from(dir).join(r"KyberNow\setup-code.txt"))
}

pub fn read_code(path: &std::path::Path) -> Option<String> {
    std::fs::read_to_string(path).ok().map(|c| c.trim().to_string()).filter(|c| !c.is_empty())
}

/// Code d'initialisation pour l'icône ordinaire : la copie laissée par la
/// partie élevée (lue puis effacée), sinon celui de ProgramData si ce compte
/// peut le lire.
pub fn take_setup_code() -> Option<String> {
    if let Some(path) = handoff_path()
        && let Some(code) = read_code(&path)
    {
        let _ = std::fs::remove_file(&path);
        return Some(code);
    }
    read_code(&data_setup_code_path())
}
