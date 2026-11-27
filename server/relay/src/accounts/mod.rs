//! Les comptes KyberNow sur le relais (docs/comptes.md, « Lot 1 : comptes sur
//! le relais ») : la base ([`store`]), les mots de passe ([`password`]), la
//! double authentification ([`totp`]), les e-mails ([`mail`]), les pages
//! `/compte/…` ([`pages`]) et leur aiguillage ([`web`]).
//!
//! Lot 1b : les passkeys (`passkeys.rs`), l'export des données et la
//! suppression du compte (`data.rs`), deux modules enfants de [`web`] ; la
//! version des conditions acceptées ([`crate::legal`]) ; l'effacement des
//! comptes jamais vérifiés ([`store::UNVERIFIED_SECS`]).
//!
//! Lot 2 : la clé de signature et les Stations rattachées ([`stations`]) ;
//! moitié relais du lot 3 : les appareils connectés et les jetons d'accès
//! ([`devices`]).
//!
//! Lot 4 : les personnes et leurs droits ([`people`]) ; lot 5 : les
//! organisations, groupes, journal et jetons d'enrôlement ([`orgs`]). La
//! console (« Ordinateurs », « Personnes », « Organisation », « Déployer des
//! Stations ») est dans `web/console.rs` et `web/org_pages.rs`.
//!
//! Migration 7 : les liens d'accès ([`links`], `web/link_pages.rs`) et les
//! groupes d'ordinateurs ([`station_groups`], onglet « Ordinateurs » de
//! `web/org_pages.rs`).
//!
//! Migration 9 : les réglages synchronisés ([`settings`], `GET /v1/settings`,
//! `PUT /v1/settings/<station>`) ; déplacer un ordinateur ([`moves`],
//! `web/move_pages.rs`).
//!
//! Tout ici est synchrone : le relais appelle [`Accounts::handle`] hors de
//! l'exécuteur (`spawn_blocking`), quelques requêtes à la fois ([`Accounts::gate`]),
//! parce qu'Argon2 prend du temps et de la mémoire exprès.

pub mod devices;
pub mod links;
pub mod mail;
pub mod orgs;
pub mod moves;
pub mod pages;
pub mod people;
pub mod password;
pub mod settings;
pub mod station_groups;
pub mod stations;
pub mod store;
pub mod totp;
pub mod web;

use std::net::IpAddr;
use std::path::Path;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use ring::{aead, digest, hmac, rand::SecureRandom};

use crate::config::AccountsConfig;
use crate::limits::Buckets;
use mail::Mailer;
use password::Hasher;
use store::Store;

/// Des octets aléatoires (le générateur du système).
pub fn random<const N: usize>() -> [u8; N] {
    let mut out = [0u8; N];
    ring::rand::SystemRandom::new().fill(&mut out).expect("générateur aléatoire du système");
    out
}

/// Comparaison en temps constant.
pub fn ct_eq(a: &[u8], b: &[u8]) -> bool {
    a.len() == b.len() && a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

pub fn b64(bytes: &[u8]) -> String {
    URL_SAFE_NO_PAD.encode(bytes)
}

pub fn unb64(s: &str) -> Option<Vec<u8>> {
    URL_SAFE_NO_PAD.decode(s).ok()
}

/// L'empreinte gardée d'un jeton.
pub fn token_hash(token: &str) -> Vec<u8> {
    digest::digest(&digest::SHA256, token.as_bytes()).as_ref().to_vec()
}

/// Un jeton de 256 bits et son empreinte.
pub fn new_token() -> (String, Vec<u8>) {
    let token = b64(&random::<32>());
    let hash = token_hash(&token);
    (token, hash)
}

/// Un code à six chiffres.
pub fn new_code() -> String {
    let n = u32::from_be_bytes(random::<4>()) % 1_000_000;
    format!("{n:06}")
}

/// Une adresse e-mail en minuscules, sans espaces autour ; `None` si elle n'en a pas l'air.
pub fn normalize_email(input: &str) -> Option<String> {
    let email = input.trim().to_lowercase();
    let (local, domain) = email.split_once('@')?;
    let ok = !local.is_empty()
        && local.len() <= 64
        && email.len() <= 254
        && domain.contains('.')
        && !domain.starts_with('.')
        && !domain.ends_with('.')
        && !domain.contains("..")
        && !email.contains(|c: char| c.is_whitespace() || c.is_control() || "<>()[],;:\"\\".contains(c))
        && domain.chars().all(|c| c.is_alphanumeric() || c == '.' || c == '-')
        && !local.contains('@');
    ok.then_some(email)
}

/// Les clés tirées du fichier `secret_key`.
pub struct Keys {
    totp: aead::LessSafeKey,
    mac: hmac::Key,
}

impl Keys {
    pub fn new(secret: &[u8]) -> Keys {
        let derive = |label: &[u8]| hmac::sign(&hmac::Key::new(hmac::HMAC_SHA256, secret), label);
        let totp = aead::LessSafeKey::new(aead::UnboundKey::new(&aead::AES_256_GCM, derive(b"kybernow comptes totp v1").as_ref()).expect("clé AES"));
        let mac = hmac::Key::new(hmac::HMAC_SHA256, derive(b"kybernow comptes cookies v1").as_ref());
        Keys { totp, mac }
    }

    /// Lit la clé (au moins 32 caractères), ou la crée (64 chiffres hexadécimaux, réservée au relais).
    pub fn load_or_create(path: &Path) -> Result<Keys> {
        if !path.exists() {
            if let Some(dir) = path.parent().filter(|d| !d.as_os_str().is_empty()) {
                std::fs::create_dir_all(dir).with_context(|| format!("dossier de la clé {}", dir.display()))?;
            }
            store::create_private(path).with_context(|| format!("clé des comptes {}", path.display()))?;
            let hex: String = random::<32>().iter().map(|b| format!("{b:02x}")).collect();
            std::fs::write(path, format!("{hex}\n")).with_context(|| format!("clé des comptes {}", path.display()))?;
            tracing::info!(fichier = %path.display(), "clé des comptes créée");
        }
        let text = std::fs::read(path).with_context(|| format!("clé des comptes {}", path.display()))?;
        let secret = text.trim_ascii();
        anyhow::ensure!(secret.len() >= 32, "clé des comptes {} : 32 caractères au moins", path.display());
        Ok(Keys::new(secret))
    }

    /// Chiffre un secret TOTP, lié au compte : `nonce || chiffré`.
    pub fn seal(&self, account: i64, plain: &[u8]) -> Vec<u8> {
        let nonce = random::<12>();
        let mut buf = plain.to_vec();
        self.totp
            .seal_in_place_append_tag(aead::Nonce::assume_unique_for_key(nonce), aead::Aad::from(account.to_be_bytes()), &mut buf)
            .expect("chiffrement");
        let mut out = nonce.to_vec();
        out.extend_from_slice(&buf);
        out
    }

    pub fn open(&self, account: i64, sealed: &[u8]) -> Option<Vec<u8>> {
        if sealed.len() < 12 + 16 {
            return None;
        }
        let (nonce, rest) = sealed.split_at(12);
        let nonce = aead::Nonce::try_assume_unique_for_key(nonce).ok()?;
        let mut buf = rest.to_vec();
        let plain = self.totp.open_in_place(nonce, aead::Aad::from(account.to_be_bytes()), &mut buf).ok()?;
        Some(plain.to_vec())
    }

    /// Signature (HMAC-SHA256) d'une valeur de cookie ou d'un jeton CSRF.
    pub fn sign(&self, data: &[u8]) -> String {
        b64(hmac::sign(&self.mac, data).as_ref())
    }

    pub fn check(&self, data: &[u8], signature: &str) -> bool {
        ct_eq(self.sign(data).as_bytes(), signature.as_bytes())
    }
}

/// La clé de limite d'une adresse : l'adresse en IPv4, son /64 en IPv6.
pub fn ip_key(ip: IpAddr) -> IpAddr {
    match ip.to_canonical() {
        IpAddr::V6(v6) => {
            let s = v6.segments();
            IpAddr::V6(std::net::Ipv6Addr::new(s[0], s[1], s[2], s[3], 0, 0, 0, 0))
        }
        v4 => v4,
    }
}

const WINDOW: Duration = Duration::from_secs(15 * 60);

/// Les limites d'essais (docs/comptes.md, « Sécurité ») : 10 connexions par
/// adresse et 5 par compte en 15 minutes ; de même pour les codes ; le mot de
/// passe oublié et l'inscription par adresse ; trois e-mails par quart d'heure
/// vers une même adresse. Un essai réussi est rendu : seuls les échecs comptent.
pub struct Limiter {
    sign_in_ip: Buckets<IpAddr>,
    sign_in_account: Buckets<String>,
    code_ip: Buckets<IpAddr>,
    code_account: Buckets<String>,
    forgot_ip: Buckets<IpAddr>,
    sign_up_ip: Buckets<IpAddr>,
    mail_to: Buckets<String>,
    invites_hour: Buckets<i64>,
    invites_day: Buckets<i64>,
}

/// Les invitations et ajouts qu'un compte envoie (REL-13) : par heure, et par jour.
pub const INVITES_PER_HOUR: u32 = 20;
pub const INVITES_PER_DAY: u32 = 100;
/// Les familles et équipes qu'un compte peut créer.
pub const MAX_OWNED_ORGS: usize = 20;

/// Une limite atteinte : réessayer dans ce temps.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limited(pub Duration);

impl Default for Limiter {
    fn default() -> Self {
        Limiter {
            sign_in_ip: Buckets::new(10, WINDOW),
            sign_in_account: Buckets::new(5, WINDOW),
            code_ip: Buckets::new(10, WINDOW),
            code_account: Buckets::new(5, WINDOW),
            forgot_ip: Buckets::new(10, WINDOW),
            sign_up_ip: Buckets::new(10, Duration::from_secs(3600)),
            mail_to: Buckets::new(3, WINDOW),
            invites_hour: Buckets::new(INVITES_PER_HOUR, Duration::from_secs(3600)),
            invites_day: Buckets::new(INVITES_PER_DAY, Duration::from_secs(86_400)),
        }
    }
}

impl Limiter {
    pub fn sign_in(&mut self, ip: IpAddr, account: &str) -> Result<(), Limited> {
        let now = Instant::now();
        self.sign_in_ip.take(&ip_key(ip), now).map_err(Limited)?;
        self.sign_in_account.take(&account.to_string(), now).map_err(Limited)
    }

    pub fn code(&mut self, ip: IpAddr, account: &str) -> Result<(), Limited> {
        let now = Instant::now();
        self.code_ip.take(&ip_key(ip), now).map_err(Limited)?;
        self.code_account.take(&account.to_string(), now).map_err(Limited)
    }

    /// Une connexion réussie ne compte pas dans les essais.
    pub fn sign_in_ok(&mut self, ip: IpAddr, account: &str) {
        let now = Instant::now();
        self.sign_in_ip.refund(&ip_key(ip), now);
        self.sign_in_account.refund(&account.to_string(), now);
    }

    /// Un code accepté ne compte pas dans les essais.
    pub fn code_ok(&mut self, ip: IpAddr, account: &str) {
        let now = Instant::now();
        self.code_ip.refund(&ip_key(ip), now);
        self.code_account.refund(&account.to_string(), now);
    }

    pub fn forgot(&mut self, ip: IpAddr) -> Result<(), Limited> {
        self.forgot_ip.take(&ip_key(ip), Instant::now()).map_err(Limited)
    }

    pub fn sign_up(&mut self, ip: IpAddr) -> Result<(), Limited> {
        self.sign_up_ip.take(&ip_key(ip), Instant::now()).map_err(Limited)
    }

    /// Une invitation (ou un ajout) de plus envoyée par ce compte ?
    pub fn invite(&mut self, account: i64) -> Result<(), Limited> {
        let now = Instant::now();
        self.invites_day.take(&account, now).map_err(Limited)?;
        self.invites_hour.take(&account, now).map_err(|wait| {
            self.invites_day.refund(&account, now);
            Limited(wait)
        })
    }

    /// Un e-mail de plus vers cette adresse ?
    pub fn mail(&mut self, to: &str) -> bool {
        self.mail_to.take(&to.to_string(), Instant::now()).is_ok()
    }

    fn sweep(&mut self) {
        let now = Instant::now();
        self.sign_in_ip.sweep(now);
        self.sign_in_account.sweep(now);
        self.code_ip.sweep(now);
        self.code_account.sweep(now);
        self.forgot_ip.sweep(now);
        self.sign_up_ip.sweep(now);
        self.mail_to.sweep(now);
        self.invites_hour.sweep(now);
        self.invites_day.sweep(now);
    }
}

/// L'horloge des comptes (secondes Unix) ; les essais la remplacent.
pub type Clock = Arc<dyn Fn() -> i64 + Send + Sync>;

pub fn system_clock() -> Clock {
    Arc::new(|| std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0))
}

/// Les requêtes de comptes servies en même temps (Argon2 : 19 Mio et quelques dizaines de millisecondes chacune).
pub const CONCURRENT: usize = 8;

/// Les comptes du relais.
pub struct Accounts {
    db: Mutex<Store>,
    pub keys: Keys,
    pub mailer: Option<Arc<dyn Mailer>>,
    /// L'adresse publique des pages (`https://share.kybernow.app`), pour les liens des e-mails.
    pub public_url: String,
    pub hasher: Hasher,
    pub limits: Mutex<Limiter>,
    clock: Clock,
    /// Quelques requêtes à la fois, hors de l'exécuteur.
    pub gate: tokio::sync::Semaphore,
    /// Lot 2 : la clé de signature, les codes de rattachement, le lien vers les Stations.
    pub stations: stations::Stations,
    /// Lot 3 : les limites des appareils.
    pub devices: devices::Devices,
}

impl Accounts {
    /// Ouvre la base et la clé, démarre l'envoi des e-mails.
    pub fn open(cfg: &AccountsConfig, public_url: String) -> Result<Accounts> {
        let clock = system_clock();
        let store = Store::open(&cfg.database, clock())?;
        // Une base qui a des doubles authentifications sans sa clé (restaurée à part ?) : une
        // nouvelle clé les rendrait illisibles. On refuse de démarrer plutôt (REL-07).
        if !cfg.secret_key.exists() {
            let active = store.active_totp_count()?;
            anyhow::ensure!(
                active == 0,
                "clé des comptes {} absente, mais {active} compte(s) ont la double authentification : remettez la clé d'origine",
                cfg.secret_key.display()
            );
        }
        let keys = Keys::load_or_create(&cfg.secret_key)?;
        let mailer: Option<Arc<dyn Mailer>> = match &cfg.smtp {
            Some(smtp) => Some(Arc::new(mail::Smtp::start(smtp)?)),
            None => {
                tracing::info!("comptes sans SMTP : ni vérification par e-mail ni mot de passe oublié");
                None
            }
        };
        let mut accounts = Accounts::with(store, keys, mailer, public_url, Hasher::new(), clock);
        let dir = cfg.secret_key.parent().filter(|d| !d.as_os_str().is_empty()).unwrap_or(Path::new("."));
        accounts.stations = stations::Stations::new(stations::SigningKeys::load_or_create(dir)?);
        Ok(accounts)
    }

    pub fn with(store: Store, keys: Keys, mailer: Option<Arc<dyn Mailer>>, public_url: String, hasher: Hasher, clock: Clock) -> Accounts {
        Accounts {
            db: Mutex::new(store),
            keys,
            mailer,
            public_url,
            hasher,
            limits: Mutex::new(Limiter::default()),
            clock,
            gate: tokio::sync::Semaphore::new(CONCURRENT),
            stations: stations::Stations::new(stations::SigningKeys::ephemeral()),
            devices: devices::Devices::default(),
        }
    }

    pub fn now(&self) -> i64 {
        (self.clock)()
    }

    pub fn db(&self) -> MutexGuard<'_, Store> {
        self.db.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Le nom de ce relais dans les applications d'authentification : « KyberNow (share.kybernow.app) ».
    pub fn issuer(&self) -> String {
        let host = self.public_url.trim_start_matches("https://");
        format!("KyberNow ({host})")
    }

    /// `kybernow-relay accounts add` : un compte déjà vérifié, créé par l'administrateur du relais.
    pub fn add_verified(&self, email: &str, name: &str, password: &str) -> Result<i64> {
        let email = normalize_email(email).context("adresse e-mail invalide")?;
        if let Err(weak) = password::check_new(password, &email) {
            anyhow::bail!("mot de passe refusé : {}", pages::weak_text(crate::pages::Lang::Fr, weak));
        }
        let hash = self.hasher.hash(password);
        let now = self.now();
        let db = self.db();
        let id = db.create_account(&email, name.trim(), &hash, "fr", Some(now), now)?.context("cette adresse a déjà un compte")?;
        // Lots 4 et 5 : les invitations qui attendaient cette adresse.
        db.claim_invitations(id, now)?;
        db.audit(Some(id), "compte_cree_par_l_administrateur", None, now)?;
        Ok(id)
    }

    /// Le ménage, toutes les 30 secondes : limites revenues au calme, jetons et sessions expirés,
    /// comptes jamais vérifiés depuis sept jours.
    pub fn sweep(&self) {
        self.limits.lock().unwrap_or_else(|e| e.into_inner()).sweep();
        if let Err(e) = self.db().sweep(self.now()) {
            tracing::warn!("ménage de la base des comptes : {e:#}");
        }
        self.sweep_devices();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adresses_e_mail() {
        assert_eq!(normalize_email("  Ana.Martin@Exemple.ORG "), Some("ana.martin@exemple.org".into()));
        assert_eq!(normalize_email("a+tag@b.co"), Some("a+tag@b.co".into()));
        for bad in ["", "a", "a@b", "@b.org", "a@.org", "a@b..org", "a b@c.org", "a@b@c.org", "<a@b.org>", "a@b.org.", "a@b_c.org"] {
            assert_eq!(normalize_email(bad), None, "{bad}");
        }
    }

    #[test]
    fn secrets_chiffres_et_lies_au_compte() {
        let keys = Keys::new(&[7u8; 32]);
        let sealed = keys.seal(1, b"secret totp");
        assert_ne!(&sealed[12..sealed.len() - 16], b"secret totp");
        assert_eq!(keys.open(1, &sealed).as_deref(), Some(&b"secret totp"[..]));
        assert_eq!(keys.open(2, &sealed), None, "lié au compte");
        assert_eq!(Keys::new(&[8u8; 32]).open(1, &sealed), None, "autre clé");
        assert_ne!(keys.seal(1, b"x"), keys.seal(1, b"x"), "nonce aléatoire");
        let sig = keys.sign(b"cookie");
        assert!(keys.check(b"cookie", &sig) && !keys.check(b"cookie2", &sig));
    }

    #[test]
    fn pas_de_nouvelle_cle_sous_des_doubles_authentifications() {
        // REL-07 : la base a un TOTP activé, la clé manque : le relais refuse de démarrer.
        let dir = std::env::temp_dir().join(format!("kn-cle-totp-{}", kybernow_tunnel::id::random_id()));
        let cfg = crate::config::AccountsConfig { enabled: true, database: dir.join("accounts.db"), secret_key: dir.join("secret.key"), public_url: None, smtp: None };
        {
            let store = Store::open(&cfg.database, 100).unwrap();
            let id = store.create_account("a@exemple.org", "A", "h", "fr", Some(0), 100).unwrap().unwrap();
            store.set_pending_totp(id, b"chiffre").unwrap();
            store.activate_totp(id, 0, 100).unwrap();
        }
        let err = Accounts::open(&cfg, "https://share.test".into()).err().expect("refusé");
        assert!(format!("{err:#}").contains("remettez la clé"), "{err:#}");
        assert!(!cfg.secret_key.exists(), "aucune clé créée");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn cle_creee_au_premier_demarrage() {
        let dir = std::env::temp_dir().join(format!("kn-cle-{}", kybernow_tunnel::id::random_id()));
        let path = dir.join("secret.key");
        let a = Keys::load_or_create(&path).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert_eq!(text.trim().len(), 64);
        let b = Keys::load_or_create(&path).unwrap();
        assert_eq!(a.sign(b"x"), b.sign(b"x"), "relue, pas recréée");
        std::fs::write(&path, "court").unwrap();
        assert!(Keys::load_or_create(&path).is_err());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn jetons_et_codes() {
        let (token, hash) = new_token();
        assert_eq!(token.len(), 43, "256 bits en base64url");
        assert_eq!(hash, token_hash(&token));
        assert_eq!(hash.len(), 32);
        let code = new_code();
        assert!(code.len() == 6 && code.bytes().all(|b| b.is_ascii_digit()));
        assert_eq!(ip_key("2001:db8:1:2:3:4:5:6".parse().unwrap()), "2001:db8:1:2::".parse::<IpAddr>().unwrap());
        assert_eq!(ip_key("::ffff:192.0.2.1".parse().unwrap()), "192.0.2.1".parse::<IpAddr>().unwrap());
    }
}
