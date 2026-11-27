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
use crate::i18n::Msg;

pub const COOKIE: &str = "kyber_admin_session";
/// La session d'Admin ouverte à distance, par le relais (docs/comptes.md, « Admin depuis KyberNow Share ») :
/// un cookie `__Host-` que la page d'une autre Station, sur une origine sœur, ne peut pas poser.
pub const REMOTE_COOKIE: &str = "__Host-kyber_admin_session";
/// Une session à distance se ferme après 30 minutes sans activité…
pub const REMOTE_IDLE: Duration = Duration::from_secs(30 * 60);
/// … et au plus 8 heures après son ouverture.
pub const REMOTE_MAX: Duration = Duration::from_secs(8 * 3600);
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
/// Une adresse (IPv6 : son /64) sans essai manqué depuis ce temps est oubliée.
const FORGET_AFTER: Duration = Duration::from_secs(3600);
/// Vérifications argon2 menées de front (≈ 19 Mio chacune) : au-delà, on attend.
const HASH_PARALLELISM: usize = 2;

#[derive(Default)]
struct Failures {
    count: u32,
    locked_until: Option<Instant>,
    /// Dernier essai manqué : une adresse sans échec depuis une heure est oubliée.
    last: Option<Instant>,
}

pub struct Auth {
    data_dir: PathBuf,
    password_hash: Mutex<Option<String>>,
    setup_code: Mutex<Option<String>>,
    sessions: Mutex<HashMap<String, Instant>>,
    /// Les sessions ouvertes à distance : le nom de la personne, la dernière activité, l'échéance.
    /// Sessions à distance : (nom, dernière activité, fin, ouverte de bout en bout).
    remote: Mutex<HashMap<String, (String, Instant, Instant, bool)>>,
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
            remote: Mutex::new(HashMap::new()),
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
        // Les adresses sans échec depuis une heure ne s'accumulent pas (des
        // adresses IPv6 tournantes feraient grossir la table).
        failures.retain(|_, f| f.locked_until.is_some_and(|u| u > now) || f.last.is_some_and(|l| now.saturating_duration_since(l) < FORGET_AFTER));
        let until = failures.get(&crate::relay::ip_key(ip))?.locked_until?;
        (until > now).then(|| until - now)
    }

    /// Un essai manqué : au bout de cinq, l'adresse attend 30 s, puis le double à chaque fois.
    pub fn note_failure(&self, ip: IpAddr) {
        let mut failures = self.failures.lock().unwrap();
        let f = failures.entry(crate::relay::ip_key(ip)).or_default();
        f.count += 1;
        f.last = Some(Instant::now());
        if f.count >= LOCKOUT_AFTER {
            let factor = 2u32.saturating_pow(f.count - LOCKOUT_AFTER);
            let wait = LOCKOUT_BASE.saturating_mul(factor).min(LOCKOUT_MAX);
            f.locked_until = Some(Instant::now() + wait);
            tracing::warn!("{ip} : {} essais manqués, connexion bloquée {} s", f.count, wait.as_secs());
        }
    }

    pub fn note_success(&self, ip: IpAddr) {
        self.failures.lock().unwrap().remove(&crate::relay::ip_key(ip));
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
                // Comparaison à temps constant, sans tenir compte de la casse.
                Some(c) if crate::lend::same(&c.to_ascii_uppercase(), &code.trim().to_ascii_uppercase()) => {}
                _ => bail!(Msg::new("auth.wrongSetupCode")),
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
            bail!(Msg::new("auth.wrongCurrentPassword"));
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
        self.remote.lock().unwrap().remove(token);
    }

    /// Une session ouverte à distance, au nom de cette personne ; `e2e` : par le chemin chiffré de
    /// bout en bout (`browser_tls`), sinon déchiffré par le relais (`admin_http`).
    pub fn open_remote_session(&self, name: &str, e2e: bool) -> String {
        let token = self.open_session();
        let now = Instant::now();
        self.remote.lock().unwrap().insert(token.clone(), (name.to_string(), now, now + REMOTE_MAX, e2e));
        token
    }

    /// La session à distance vaut-elle encore, sur ce chemin (`e2e` : de bout en bout) ? Elle compte
    /// alors comme une activité. Une session ne vaut que sur le chemin où elle s'est ouverte.
    pub fn remote_session_valid(&self, token: &str, e2e: bool) -> bool {
        let now = Instant::now();
        let ok = {
            let mut remote = self.remote.lock().unwrap();
            match remote.get_mut(token) {
                Some((_, _, _, opened_e2e)) if *opened_e2e != e2e => return false,
                Some((_, last, until, _)) if now < *until && now.duration_since(*last) < REMOTE_IDLE => {
                    *last = now;
                    true
                }
                Some(_) => {
                    remote.remove(token);
                    false
                }
                None => false,
            }
        };
        if !ok {
            self.sessions.lock().unwrap().remove(token);
        }
        ok && self.session_valid(token)
    }

    /// Ferme toutes les sessions à distance (« Admin à distance » coupé, Station détachée).
    pub fn close_remote_sessions(&self) {
        let tokens: Vec<String> = self.remote.lock().unwrap().drain().map(|(t, _)| t).collect();
        let mut sessions = self.sessions.lock().unwrap();
        for t in tokens {
            sessions.remove(&t);
        }
    }

    /// Ferme les sessions à distance ouvertes en clair (`admin_http`) : la Station vient d'être de bout
    /// en bout avec son relais (docs/relais.md, « Navigateur chiffré de bout en bout »).
    pub fn close_remote_sessions_except_e2e(&self) {
        let tokens: Vec<String> = {
            let mut remote = self.remote.lock().unwrap();
            let gone: Vec<String> = remote.iter().filter(|(_, r)| !r.3).map(|(t, _)| t.clone()).collect();
            for t in &gone {
                remote.remove(t);
            }
            gone
        };
        let mut sessions = self.sessions.lock().unwrap();
        for t in tokens {
            sessions.remove(&t);
        }
    }

    /// Les personnes qui ont Admin ouvert à distance en ce moment.
    pub fn remote_names(&self) -> Vec<String> {
        let now = Instant::now();
        let mut remote = self.remote.lock().unwrap();
        remote.retain(|_, (_, last, until, _)| now < *until && now.duration_since(*last) < REMOTE_IDLE);
        let mut names: Vec<String> = remote.values().map(|(n, _, _, _)| n.clone()).collect();
        names.sort();
        names.dedup();
        names
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
        bail!(Msg::new("auth.adminPasswordLength"));
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

/// Le cookie de la session à distance (par le relais, toujours en HTTPS).
pub fn remote_session_cookie(token: &str) -> String {
    format!("{REMOTE_COOKIE}={token}; Path=/; Secure; HttpOnly; SameSite=Strict; Max-Age={}", REMOTE_MAX.as_secs())
}

/// Le jeton de la session à distance de ce navigateur.
pub fn remote_session_token(headers: &HeaderMap) -> Option<String> {
    headers
        .get_all(header::COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .flat_map(|v| v.split(';'))
        .filter_map(|c| c.trim().split_once('='))
        .find(|(k, _)| *k == REMOTE_COOKIE)
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
    let public = is_public(req.uri().path());
    // Le retour de « Se connecter avec KyberNow » en formulaire : une
    // navigation, sans l'en-tête (jeton à usage unique lié au cookie du navigateur).
    let login_form = req.method() == Method::POST && req.uri().path() == "/api/login/kybernow/return";
    if req.method() != Method::GET && !login_form && req.headers().get(CSRF_HEADER).is_none() {
        return (StatusCode::FORBIDDEN, Msg::new("auth.adminHeader").text()).into_response();
    }
    if !public {
        let ok = session_token(req.headers()).is_some_and(|t| state.auth.session_valid(&t));
        if !ok {
            return (StatusCode::UNAUTHORIZED, Msg::new("auth.sessionRequired").text()).into_response();
        }
    }
    next.run(req).await
}

/// Les routes qui vérifient elles-mêmes qui appelle. Les appels des
/// appareils s'authentifient eux-mêmes (compte Kyber). Les routes
/// `/api/local/*` vérifient qu'on les appelle depuis l'ordinateur (ou avec
/// la session d'Admin). L'appairage par code se fait sans compte : la
/// demande, puis son état. Le partage (KyberNow Share) vérifie lui-même :
/// compte de l'appareil hôte, secret de l'invité, ou code pour rejoindre
/// (limité).
fn is_public(path: &str) -> bool {
    matches!(path, "/api/session" | "/api/login" | "/api/login/kybernow" | "/api/login/kybernow/start" | "/api/login/kybernow/return" | "/api/setup" | "/api/share")
        // La clé d'Admin à distance : le code d'ajout, ou l'ouverture qui attend la clé (son cookie).
        || matches!(path, "/api/login/key" | "/api/login/key/options" | "/api/login/key/add" | "/api/login/key/add/options")
        || path.starts_with("/api/local/")
        || path.starts_with("/api/stream/")
        || path.starts_with("/api/share/")
        || is_lend_public(path)
        || is_pair_request(path)
}

/// Le prêt (docs/pret.md) : l'ami (code, compte d'ami, secret) et l'hôte
/// (compte d'un appareil, ou session d'Admin) vérifient eux-mêmes ; les
/// réglages et les comptes d'amis demandent la session d'Admin.
fn is_lend_public(path: &str) -> bool {
    matches!(
        path,
        "/api/lend" | "/api/lend/status" | "/api/lend/borrow" | "/api/lend/wait" | "/api/lend/beat" | "/api/lend/leave" | "/api/lend/code" | "/api/lend/take-back"
    ) || path.strip_prefix("/api/lend/requests/").is_some_and(|id| !id.is_empty() && !id.contains('/'))
}

/// `/api/pair/request`, `/api/pair/request/{id}` et `/api/pair/invite`, seulement.
fn is_pair_request(path: &str) -> bool {
    // L'appareil invité vérifie lui-même : code et mot de passe de l'invitation.
    path == "/api/pair/request"
        || path == "/api/pair/invite"
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
    fn le_partage_verifie_lui_meme() {
        for path in ["/api/share", "/api/share/join", "/api/share/beat", "/api/share/leave", "/api/share/open", "/api/share/guest/abc", "/api/local/displays", "/api/local/share/open"] {
            assert!(is_public(path), "{path}");
        }
        for path in ["/api/lend/borrow", "/api/lend", "/api/lend/requests/ab12", "/api/lend/take-back"] {
            assert!(is_public(path), "{path}");
        }
        for path in ["/api/status", "/api/shares", "/api/sharex/join", "/api/devices", "/api/events", "/api/lend/settings", "/api/lend/history", "/api/lend/friends", "/api/lend/friends/x/invite", "/api/lend/requests/a/b"] {
            assert!(!is_public(path), "{path}");
        }
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
