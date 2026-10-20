//! Authentification de l'interface d'administration.
//!
//! Un seul compte administrateur, protégé par un mot de passe (empreinte
//! argon2). Au premier lancement, sans mot de passe, un code d'initialisation
//! est écrit dans le dossier de données : il faut avoir accès à l'ordinateur pour le lire.

use std::collections::HashMap;
use std::net::IpAddr;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use anyhow::{Result, bail};
use argon2::password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use argon2::Argon2;
use axum::extract::{Request, State};
use axum::http::{HeaderMap, Method, StatusCode, header};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use rand::Rng;

use crate::AppState;

pub const COOKIE: &str = "kyber_admin_session";
/// En-tête exigé sur toute requête qui modifie quelque chose : un site tiers ne
/// peut pas l'ajouter sans requête CORS préalable, que nous n'autorisons pas.
pub const CSRF_HEADER: &str = "x-kyber-admin";
const SESSION_TTL: Duration = Duration::from_secs(12 * 3600);
pub const SETUP_TOKEN_FILE: &str = "setup-code.txt";

/// Essais manqués avant un premier verrouillage.
const LOCKOUT_AFTER: u32 = 5;
/// Durée du premier verrouillage ; elle double à chaque essai manqué suivant.
const LOCKOUT_BASE: Duration = Duration::from_secs(30);
const LOCKOUT_MAX: Duration = Duration::from_secs(10 * 60);
/// Vérifications argon2 menées de front (≈ 19 Mio chacune) : au-delà, on attend.
const HASH_PARALLELISM: usize = 2;

#[derive(Default)]
struct Failures {
    count: u32,
    locked_until: Option<Instant>,
}

pub struct Auth {
    data_dir: PathBuf,
    password_hash: Mutex<Option<String>>,
    setup_code: Mutex<Option<String>>,
    sessions: Mutex<HashMap<String, Instant>>,
    /// Essais de connexion manqués, par adresse.
    failures: Mutex<HashMap<IpAddr, Failures>>,
    hashing: tokio::sync::Semaphore,
}

impl Auth {
    pub fn new(data_dir: PathBuf, password_hash: Option<String>) -> Arc<Self> {
        let auth = Arc::new(Self {
            data_dir,
            password_hash: Mutex::new(password_hash),
            setup_code: Mutex::new(None),
            sessions: Mutex::new(HashMap::new()),
            failures: Mutex::new(HashMap::new()),
            hashing: tokio::sync::Semaphore::new(HASH_PARALLELISM),
        });
        if !auth.is_configured() {
            auth.issue_setup_code();
        }
        auth
    }

    /// Un calcul argon2 (vérification ou nouvelle empreinte), hors du fil
    /// asynchrone et jamais plus de `HASH_PARALLELISM` à la fois : une rafale
    /// de connexions depuis le réseau ne peut pas épuiser la mémoire.
    pub async fn hash_limited<T: Send + 'static>(
        self: Arc<Self>,
        work: impl FnOnce() -> Result<T> + Send + 'static,
    ) -> Result<T> {
        let _permit = self.hashing.acquire().await.map_err(|_| anyhow::anyhow!("vérification impossible"))?;
        tokio::task::spawn_blocking(work).await.map_err(|e| anyhow::anyhow!("vérification interrompue : {e}"))?
    }

    /// Verrouillée pour combien de temps encore, après trop d'essais manqués.
    pub fn locked_for(&self, ip: IpAddr) -> Option<Duration> {
        let mut failures = self.failures.lock().unwrap();
        let now = Instant::now();
        // Les adresses oubliées depuis longtemps ne s'accumulent pas.
        failures.retain(|_, f| f.locked_until.is_some_and(|u| u > now) || f.count > 0);
        let until = failures.get(&ip.to_canonical())?.locked_until?;
        (until > now).then(|| until - now)
    }

    /// Un essai manqué : au bout de cinq, l'adresse attend 30 s, puis le double à chaque fois.
    pub fn note_failure(&self, ip: IpAddr) {
        let mut failures = self.failures.lock().unwrap();
        let f = failures.entry(ip.to_canonical()).or_default();
        f.count += 1;
        if f.count >= LOCKOUT_AFTER {
            let factor = 2u32.saturating_pow(f.count - LOCKOUT_AFTER);
            let wait = LOCKOUT_BASE.saturating_mul(factor).min(LOCKOUT_MAX);
            f.locked_until = Some(Instant::now() + wait);
            tracing::warn!("{ip} : {} essais manqués, connexion bloquée {} s", f.count, wait.as_secs());
        }
    }

    pub fn note_success(&self, ip: IpAddr) {
        self.failures.lock().unwrap().remove(&ip.to_canonical());
    }

    pub fn is_configured(&self) -> bool {
        self.password_hash.lock().unwrap().is_some()
    }

    fn issue_setup_code(&self) {
        let code: String = (0..8)
            .map(|_| char::from(b"ABCDEFGHJKLMNPQRSTUVWXYZ23456789"[rand::rng().random_range(0..32)]))
            .collect();
        let path = self.data_dir.join(SETUP_TOKEN_FILE);
        if let Err(e) = std::fs::create_dir_all(&self.data_dir).and_then(|_| std::fs::write(&path, &code)) {
            tracing::error!("impossible d'écrire {} : {e}", path.display());
        }
        tracing::warn!(
            "Aucun mot de passe administrateur. Code d'initialisation : {code} (aussi dans {})",
            path.display()
        );
        *self.setup_code.lock().unwrap() = Some(code);
    }

    /// Premier réglage du mot de passe, avec le code d'initialisation.
    pub fn setup(&self, code: &str, password: &str) -> Result<String> {
        {
            let expected = self.setup_code.lock().unwrap();
            match expected.as_deref() {
                Some(c) if c.eq_ignore_ascii_case(code.trim()) => {}
                _ => bail!("code d'initialisation incorrect"),
            }
        }
        let hash = hash_password(password)?;
        *self.password_hash.lock().unwrap() = Some(hash.clone());
        *self.setup_code.lock().unwrap() = None;
        let _ = std::fs::remove_file(self.data_dir.join(SETUP_TOKEN_FILE));
        Ok(hash)
    }

    pub fn change_password(&self, current: &str, new: &str) -> Result<String> {
        if !self.verify(current) {
            bail!("mot de passe actuel incorrect");
        }
        let hash = hash_password(new)?;
        *self.password_hash.lock().unwrap() = Some(hash.clone());
        // Toutes les autres sessions sont fermées.
        self.sessions.lock().unwrap().clear();
        Ok(hash)
    }

    pub fn verify(&self, password: &str) -> bool {
        let guard = self.password_hash.lock().unwrap();
        let Some(stored) = guard.as_deref() else { return false };
        let Ok(parsed) = PasswordHash::new(stored) else { return false };
        Argon2::default().verify_password(password.as_bytes(), &parsed).is_ok()
    }

    pub fn open_session(&self) -> String {
        let token: [u8; 32] = rand::random();
        let token = hex::encode(token);
        let mut sessions = self.sessions.lock().unwrap();
        let now = Instant::now();
        sessions.retain(|_, exp| *exp > now);
        sessions.insert(token.clone(), now + SESSION_TTL);
        token
    }

    pub fn close_session(&self, token: &str) {
        self.sessions.lock().unwrap().remove(token);
    }

    pub fn session_valid(&self, token: &str) -> bool {
        self.sessions
            .lock()
            .unwrap()
            .get(token)
            .is_some_and(|exp| *exp > Instant::now())
    }
}

pub fn hash_password(password: &str) -> Result<String> {
    if password.chars().count() < 10 {
        bail!("le mot de passe administrateur doit faire au moins 10 caractères");
    }
    let salt_bytes: [u8; 16] = rand::random();
    let salt = SaltString::encode_b64(&salt_bytes).map_err(|e| anyhow::anyhow!("{e}"))?;
    Ok(Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map_err(|e| anyhow::anyhow!("{e}"))?
        .to_string())
}

pub fn session_token(headers: &HeaderMap) -> Option<String> {
    headers
        .get_all(header::COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .flat_map(|v| v.split(';'))
        .filter_map(|c| c.trim().split_once('='))
        .find(|(k, _)| *k == COOKIE)
        .map(|(_, v)| v.to_string())
}

pub fn session_cookie(token: &str, secure: bool) -> String {
    format!(
        "{COOKIE}={token}; Path=/; HttpOnly; SameSite=Strict; Max-Age={}{}",
        SESSION_TTL.as_secs(),
        if secure { "; Secure" } else { "" }
    )
}

pub fn clear_cookie(secure: bool) -> String {
    format!(
        "{COOKIE}=; Path=/; HttpOnly; SameSite=Strict; Max-Age=0{}",
        if secure { "; Secure" } else { "" }
    )
}

/// Protège les routes `/api/*` autres que la connexion et l'initialisation.
pub async fn require_session(State(state): State<AppState>, req: Request, next: Next) -> Response {
    let path = req.uri().path();
    // Les appels des appareils s'authentifient eux-mêmes (compte Kyber).
    // Les routes /api/local/* vérifient elles-mêmes qu'on les appelle depuis l'ordinateur.
    // L'appairage par code se fait sans compte : la demande, puis son état.
    let public = matches!(path, "/api/session" | "/api/login" | "/api/setup")
        || path.starts_with("/api/local/")
        || path.starts_with("/api/stream/")
        || is_pair_request(path);
    if req.method() != Method::GET && req.headers().get(CSRF_HEADER).is_none() {
        return (StatusCode::FORBIDDEN, "en-tête x-kyber-admin manquant").into_response();
    }
    if !public {
        let ok = session_token(req.headers()).is_some_and(|t| state.auth.session_valid(&t));
        if !ok {
            return (StatusCode::UNAUTHORIZED, "session requise").into_response();
        }
    }
    next.run(req).await
}

/// `/api/pair/request` et `/api/pair/request/{id}`, seulement.
fn is_pair_request(path: &str) -> bool {
    path == "/api/pair/request"
        || path.strip_prefix("/api/pair/request/").is_some_and(|id| !id.is_empty() && !id.contains('/'))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn appairage_sans_compte() {
        assert!(is_pair_request("/api/pair/request"));
        assert!(is_pair_request("/api/pair/request/0a1b2c"));
        assert!(!is_pair_request("/api/pair/request/"));
        assert!(!is_pair_request("/api/pair/request/0a1b2c/accept"));
        assert!(!is_pair_request("/api/pair/accept/0a1b2c"));
        assert!(!is_pair_request("/api/pair/requests"));
    }

    #[test]
    fn hash_and_verify() {
        let dir = std::env::temp_dir().join(format!("kyber-admin-auth-{}", rand::random::<u64>()));
        let auth = Auth::new(dir.clone(), Some(hash_password("un-long-mot-de-passe").unwrap()));
        assert!(auth.verify("un-long-mot-de-passe"));
        assert!(!auth.verify("autre-chose-encore"));
        assert!(hash_password("court").is_err());
        let _ = std::fs::remove_dir_all(dir);
    }
}
