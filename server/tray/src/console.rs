//! Console d'administration : son adresse, et l'état qu'elle donne à l'ordinateur
//! local (`GET /api/local/state` : serveur, session en cours).

use std::path::PathBuf;
use std::time::Duration;

use serde_json::Value;

fn env_dir(var: &str, fallback: &str) -> PathBuf {
    std::env::var_os(var).map(PathBuf::from).unwrap_or_else(|| PathBuf::from(fallback))
}

/// Port de la console : `listen` dans `C:\ProgramData\KyberNow\kyber-admin.toml`,
/// sinon `console_port` dans `C:\Program Files\KyberNow\kybernow.json`.
/// Relu à chaque fois : la console peut changer de port.
pub fn port() -> Option<u16> {
    let toml = env_dir("ProgramData", r"C:\ProgramData").join(r"KyberNow\kyber-admin.toml");
    let from_toml = std::fs::read_to_string(toml).ok().and_then(|text| {
        text.lines().find_map(|line| {
            let (key, value) = line.split_once('=')?;
            if key.trim() != "listen" {
                return None;
            }
            value.trim().trim_matches('"').rsplit(':').next()?.parse().ok()
        })
    });
    from_toml.or_else(|| {
        let json = env_dir("ProgramFiles", r"C:\Program Files").join(r"KyberNow\kybernow.json");
        let record: Value = serde_json::from_slice(&std::fs::read(json).ok()?).ok()?;
        record["console_port"].as_u64().and_then(|p| u16::try_from(p).ok())
    })
}

pub fn url(port: u16) -> String {
    format!("https://localhost:{port}/")
}

/// Ce que la console dit de la session en cours.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalState {
    pub session: bool,
    pub user: Option<String>,
    /// Version plus récente publiée sur GitHub, si la console en a vu une.
    pub update: Option<String>,
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

    pub fn local_state(&self, port: u16) -> Option<LocalState> {
        let mut response = self.0.get(&format!("https://127.0.0.1:{port}/api/local/state")).call().ok()?;
        let body: Value = response.body_mut().read_json().ok()?;
        Some(LocalState {
            session: body["session"].as_bool().unwrap_or(false),
            user: body["user"].as_str().map(str::to_owned),
            update: body["update_available"].as_str().map(str::to_owned),
        })
    }
}
