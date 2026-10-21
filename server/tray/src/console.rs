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

use crate::http;

fn env_dir(var: &str, fallback: &str) -> PathBuf {
    std::env::var_os(var).map(PathBuf::from).unwrap_or_else(|| PathBuf::from(fallback))
}

/// `C:\Program Files\KyberNow\kybernow.json`, écrit par l'installateur et
/// complété par la console, sans secret et lisible par tous (modifiable par
/// les administrateurs seulement). L'icône ne lit rien dans
/// `C:\ProgramData\KyberNow` : ce dossier (où `kyber-admin.toml` garde
/// l'empreinte du mot de passe) est réservé à SYSTEM et aux administrateurs.
/// Relu à chaque fois : une réinstallation peut changer le port ou le certificat.
fn record() -> Option<Value> {
    let json = env_dir("ProgramFiles", r"C:\Program Files").join(r"KyberNow\kybernow.json");
    serde_json::from_slice(&std::fs::read(json).ok()?).ok()
}

/// Port de la console (`console_port`).
pub fn port() -> Option<u16> {
    record()?["console_port"].as_u64().and_then(|p| u16::try_from(p).ok())
}

/// Empreinte du certificat de la console (`console_fingerprint`, « sha256:… »),
/// écrite par `kyber-admin init` et par la console à chaque démarrage. Sans
/// elle (installation d'avant 0.2.6 dont la console n'a pas encore redémarré),
/// l'icône ne parle pas à la console.
pub fn fingerprint() -> Option<String> {
    record()?["console_fingerprint"].as_str().map(str::to_owned).filter(|f| !f.is_empty())
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
    /// Accès d'appareils que Kyber ne connaît pas encore : il redémarre de
    /// lui-même dès la fin de la session en cours (compte, appareil, donné ou retiré).
    pub pending_access: Vec<Value>,
}

/// Client épinglé sur le certificat de la console. Sans empreinte connue,
/// aucun appel n'aboutit.
pub struct Client {
    /// Empreinte sur laquelle le client est construit (relue par `watch`).
    pub fingerprint: Option<String>,
    http: Option<http::Client>,
}

impl Client {
    pub fn new() -> Self {
        let fingerprint = fingerprint();
        let http = fingerprint.as_deref().and_then(|fp| match http::Client::pinned(fp) {
            Ok(c) => Some(c),
            Err(e) => {
                eprintln!("certificat de la console : {e:#}");
                None
            }
        });
        Client { fingerprint, http }
    }

    fn get(&self, port: u16, path: &str, headers: &[(&str, &str)]) -> Option<http::Reply> {
        self.http.as_ref()?.call("GET", port, path, headers, Duration::from_secs(3)).ok()
    }

    fn post(&self, port: u16, path: &str, timeout: Duration) -> anyhow::Result<http::Reply> {
        let Some(http) = self.http.as_ref() else {
            anyhow::bail!("Le certificat de la console n'est pas encore connu de l'icône (kybernow.json) : réessayez dans quelques secondes.");
        };
        let token = local_token();
        http.call("POST", port, path, &[("x-kyber-admin", "1"), (LOCAL_HEADER, &token)], timeout)
    }

    /// La console a-t-elle un mot de passe ? (`GET /api/session`, sans compte)
    pub fn configured(&self, port: u16) -> Option<bool> {
        let reply = self.get(port, "/api/session", &[])?;
        let body: Value = serde_json::from_slice(&reply.body).ok()?;
        body["configured"].as_bool()
    }

    pub fn local_state(&self, port: u16) -> Option<LocalState> {
        let token = local_token();
        let reply = self.get(port, "/api/local/state", &[(LOCAL_HEADER, &token)])?;
        if reply.status != 200 {
            return None;
        }
        let body: Value = serde_json::from_slice(&reply.body).ok()?;
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
            pending_access: body["pending_access"].as_array().cloned().unwrap_or_default(),
        })
    }

    /// Met fin à la session en cours (route réservée à l'ordinateur lui-même).
    pub fn end_session(&self, port: u16) -> anyhow::Result<()> {
        let reply = self.post(port, "/api/local/end", Duration::from_secs(10)).map_err(|e| anyhow::anyhow!("La console n'a pas pu mettre fin à la session ({e:#})."))?;
        if reply.status >= 400 {
            anyhow::bail!("La console n'a pas pu mettre fin à la session ({}).", describe(&reply));
        }
        Ok(())
    }

    /// Accepte ou refuse une demande d'accès par code (route réservée à l'ordinateur lui-même).
    pub fn pair_answer(&self, port: u16, id: &str, accept: bool) -> anyhow::Result<()> {
        let action = if accept { "accept" } else { "refuse" };
        let reply = self.post(port, &format!("/api/local/pair/{id}/{action}"), Duration::from_secs(15)).map_err(|e| anyhow::anyhow!("La console n'a pas pu répondre à la demande ({e:#})."))?;
        match reply.status {
            s if s < 400 => Ok(()),
            410 => anyhow::bail!("Cette demande a expiré : l'appareil en enverra une nouvelle avec un autre code."),
            _ => anyhow::bail!("La console n'a pas pu répondre à la demande ({}).", describe(&reply)),
        }
    }
}

/// « 403 : jeton local absent ou périmé », d'après le JSON d'erreur de la console.
fn describe(reply: &http::Reply) -> String {
    let message = serde_json::from_slice::<Value>(&reply.body).ok().and_then(|v| v["error"].as_str().map(str::to_owned));
    match message {
        Some(m) => format!("{} : {m}", reply.status),
        None => reply.status.to_string(),
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
