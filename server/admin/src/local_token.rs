//! Jeton des routes locales (`/api/local/*` : état pour l'icône, fin de
//! session, réponse à une demande d'appairage).
//!
//! Ces routes servent à la personne devant l'écran, par l'icône de la zone de
//! notification. Un simple test d'adresse loopback ne suffit pas sur un
//! ordinateur partagé : n'importe quel processus local, d'un autre compte ou
//! en bac à sable, pourrait couper la session ou accepter l'appairage d'un
//! appareil inconnu. La console tire donc 32 octets aléatoires à son
//! démarrage et à chaque ouverture de session Windows, les écrit dans
//! `C:\ProgramData\KyberNow\local-token` (réservé à SYSTEM et aux
//! Administrateurs) et les fait copier par un agent lancé dans la session de
//! l'utilisateur (`display-agent local-token`) dans son profil
//! (`%LOCALAPPDATA%\KyberNow\local-token`), que seul ce compte (et les
//! administrateurs) peut lire. L'icône l'envoie dans l'en-tête
//! `x-kyber-local` ; les routes exigent loopback et le jeton.
//!
//! Hors Windows (développement), le jeton est seulement écrit dans le dossier
//! de données (`dev-data/local-token`).
//!
//! Un second jeton, `installer-token`, n'est écrit que dans le dossier de
//! données (jamais copié dans un profil) : seul un administrateur (l'installateur,
//! élevé) le lit. Il garde les routes qui rattachent la Station à un compte
//! ou changent son relais (`/api/local/account/*`) : sans lui, un compte
//! Windows standard (ou l'ami d'un prêt, dans la session de l'hôte) pourrait
//! rattacher la Station à son propre compte, puis entrer dans Admin.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use axum::http::HeaderMap;

/// Fichier du jeton, dans le dossier de données et dans le profil de l'utilisateur.
pub const FILE: &str = "local-token";
/// En-tête porté par l'icône.
pub const HEADER: &str = "x-kyber-local";
/// Jeton de l'installateur, dans le dossier de données seulement.
pub const INSTALLER_FILE: &str = "installer-token";
/// En-tête porté par l'installateur (en plus de `x-kyber-local`).
pub const INSTALLER_HEADER: &str = "x-kyber-installer";
/// Copies à réessayer (5 s d'écart) quand la session de l'utilisateur n'est
/// pas prête ; une ouverture de session relance la copie de toute façon.
const COPY_RETRIES: u32 = 6;
const COPY_RETRY_EVERY: Duration = Duration::from_secs(5);

/// Levé par le gestionnaire de services à chaque ouverture de session Windows.
static SESSION_CHANGED: AtomicBool = AtomicBool::new(false);

#[cfg_attr(not(windows), allow(dead_code))]
/// Une session Windows vient de s'ouvrir (ou de reprendre la console) : un
/// nouveau jeton sera tiré et copié dans cette session.
pub fn session_changed() {
    SESSION_CHANGED.store(true, Ordering::SeqCst);
}

pub struct LocalToken {
    data_dir: PathBuf,
    /// Jeton en cours, en hexadécimal (64 caractères).
    current: Mutex<String>,
    /// Jeton de l'installateur (jamais copié dans la session).
    installer: Mutex<String>,
    /// Copies encore à tenter dans la session de l'utilisateur.
    copies_left: AtomicU32,
}

impl LocalToken {
    pub fn new(data_dir: &Path) -> Arc<Self> {
        Arc::new(Self {
            data_dir: data_dir.to_path_buf(),
            current: Mutex::new(String::new()),
            installer: Mutex::new(String::new()),
            copies_left: AtomicU32::new(0),
        })
    }

    /// Nouveau jeton : écrit dans le dossier de données, puis copié dans la
    /// session de l'utilisateur (réessayé par `supervise` si elle n'est pas prête).
    pub fn renew(&self) {
        let token = hex::encode(rand::random::<[u8; 32]>());
        *self.current.lock().unwrap() = token.clone();
        let installer = hex::encode(rand::random::<[u8; 32]>());
        let installer_path = self.data_dir.join(INSTALLER_FILE);
        match write_atomically(&installer_path, &installer) {
            Ok(()) => *self.installer.lock().unwrap() = installer,
            Err(e) => tracing::error!("jeton de l'installateur non écrit dans {} : {e:#}", installer_path.display()),
        }
        let path = self.data_dir.join(FILE);
        if let Err(e) = write_atomically(&path, &token) {
            tracing::error!("jeton local non écrit dans {} : {e:#}", path.display());
            return;
        }
        tracing::info!("jeton local renouvelé");
        self.copies_left.store(COPY_RETRIES, Ordering::SeqCst);
        self.copy_to_session();
    }

    /// L'en-tête `x-kyber-local` porte le jeton en cours (comparaison à temps constant).
    pub fn accepts(&self, headers: &HeaderMap) -> bool {
        same_token(headers, HEADER, &self.current.lock().unwrap())
    }

    /// L'en-tête `x-kyber-installer` porte le jeton de l'installateur.
    pub fn accepts_installer(&self, headers: &HeaderMap) -> bool {
        same_token(headers, INSTALLER_HEADER, &self.installer.lock().unwrap())
    }

    /// Copie dans le profil de l'utilisateur de la session console, par l'agent.
    fn copy_to_session(&self) {
        if self.copies_left.load(Ordering::SeqCst) == 0 {
            return;
        }
        #[cfg(windows)]
        {
            let args = vec!["local-token".to_string(), "--dir".to_string(), self.data_dir.display().to_string()];
            match crate::display::agent::run_with_timeout(&self.data_dir, &args, 10_000) {
                Ok(v) => {
                    tracing::info!("jeton local copié dans {}", v["path"].as_str().unwrap_or("le profil de l'utilisateur"));
                    self.copies_left.store(0, Ordering::SeqCst);
                }
                Err(e) => {
                    let left = self.copies_left.fetch_sub(1, Ordering::SeqCst) - 1;
                    if left == 0 {
                        tracing::warn!("jeton local non copié dans la session de l'utilisateur : {e:#} (nouvel essai à la prochaine ouverture de session)");
                    } else {
                        tracing::debug!("jeton local non copié ({e:#}), {left} essai(s) restant(s)");
                    }
                }
            }
        }
        #[cfg(not(windows))]
        self.copies_left.store(0, Ordering::SeqCst);
    }

    /// Renouvelle le jeton à chaque ouverture de session Windows (2 s après,
    /// le temps que la session soit prête) et réessaie les copies manquées.
    pub fn supervise(self: Arc<Self>) {
        std::thread::spawn(move || {
            let mut renew_at: Option<Instant> = None;
            let mut last_copy = Instant::now();
            loop {
                std::thread::sleep(Duration::from_secs(1));
                if SESSION_CHANGED.swap(false, Ordering::SeqCst) {
                    renew_at = Some(Instant::now() + Duration::from_secs(2));
                }
                if renew_at.is_some_and(|t| Instant::now() >= t) {
                    renew_at = None;
                    self.renew();
                    last_copy = Instant::now();
                } else if self.copies_left.load(Ordering::SeqCst) > 0 && last_copy.elapsed() >= COPY_RETRY_EVERY {
                    last_copy = Instant::now();
                    self.copy_to_session();
                }
            }
        });
    }
}

fn same_token(headers: &HeaderMap, header: &str, current: &str) -> bool {
    let Some(given) = headers.get(header).and_then(|v| v.to_str().ok()) else { return false };
    let (a, b) = (given.trim().as_bytes(), current.as_bytes());
    !b.is_empty() && a.len() == b.len() && a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

fn write_atomically(path: &Path, text: &str) -> Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    crate::durable::write(path, text).with_context(|| format!("remplacement de {}", path.display()))?;
    Ok(())
}

/// Agent, dans la session de l'utilisateur (jeton SYSTEM rattaché à la
/// session) : recopie le jeton du dossier de données dans le profil du compte
/// connecté à cette session (`%LOCALAPPDATA%\KyberNow\local-token`), où les
/// droits hérités du profil ne le laissent lire qu'à ce compte et aux
/// administrateurs.
#[cfg(windows)]
pub fn hand_over(data_dir: &Path) -> Result<serde_json::Value> {
    let token = std::fs::read_to_string(data_dir.join(FILE)).context("jeton local illisible")?;
    let dir = user_local_app_data()?.join("KyberNow");
    let path = dir.join(FILE);
    write_atomically(&path, token.trim())?;
    Ok(serde_json::json!({ "ok": true, "path": path }))
}

/// `AppData\Local` du compte connecté à la session console.
#[cfg(windows)]
fn user_local_app_data() -> Result<PathBuf> {
    crate::display::agent::user_local_app_data()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn jeton_ecrit_et_compare() {
        let dir = std::env::temp_dir().join(format!("kybernow-local-token-{}-{}", std::process::id(), rand::random::<u32>()));
        let t = LocalToken::new(&dir);
        let mut headers = HeaderMap::new();
        assert!(!t.accepts(&headers), "sans jeton tiré, rien n'est accepté");
        t.renew();
        let written = std::fs::read_to_string(dir.join(FILE)).unwrap();
        assert_eq!(written.len(), 64);
        headers.insert(HEADER, written.parse().unwrap());
        assert!(t.accepts(&headers));
        headers.insert(HEADER, format!("{}0", &written[1..]).parse().unwrap());
        assert!(!t.accepts(&headers));
        headers.insert(HEADER, "court".parse().unwrap());
        assert!(!t.accepts(&headers));
        t.renew();
        headers.insert(HEADER, written.parse().unwrap());
        assert!(!t.accepts(&headers), "l'ancien jeton ne vaut plus");
        let _ = std::fs::remove_dir_all(dir);
    }
}
