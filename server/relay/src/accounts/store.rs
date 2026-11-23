//! La base des comptes (docs/comptes.md, « Base ») : SQLite, un fichier
//! (`accounts.db` dans le volume du relais), des migrations numérotées
//! appliquées au démarrage. Tout est synchrone : [`super::Accounts`] garde la
//! connexion derrière un verrou, pris le temps d'une requête courte.
//!
//! Les jetons (sessions, liens des e-mails, navigateurs de confiance, codes de
//! secours) ne sont gardés qu'en empreinte SHA-256 ; le secret TOTP est chiffré
//! ([`super::Keys`]). Les heures sont des secondes Unix.

use std::path::Path;
use std::time::Duration;

use anyhow::{Context, Result};
use rusqlite::{Connection, OptionalExtension, params};

/// Les migrations, dans l'ordre : la n-ième porte la version n. On n'en
/// change jamais une déjà publiée ; on en ajoute une.
const MIGRATIONS: &[&str] = &[
    // 1 : lot 1 (comptes, vérification, sessions, double authentification, passkeys à venir).
    r#"
    CREATE TABLE accounts (
        id INTEGER PRIMARY KEY,
        email TEXT NOT NULL UNIQUE,
        name TEXT NOT NULL,
        password TEXT NOT NULL,
        lang TEXT NOT NULL DEFAULT 'fr',
        verified_at INTEGER,
        created_at INTEGER NOT NULL,
        deleted_at INTEGER,
        plan TEXT NOT NULL DEFAULT 'free'
    );
    CREATE TABLE email_tokens (
        hash BLOB PRIMARY KEY,
        account INTEGER NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
        purpose TEXT NOT NULL CHECK (purpose IN ('verify', 'reset')),
        code TEXT,
        attempts INTEGER NOT NULL DEFAULT 0,
        expires_at INTEGER NOT NULL
    );
    CREATE INDEX email_tokens_account ON email_tokens(account, purpose);
    CREATE TABLE sessions (
        id INTEGER PRIMARY KEY,
        hash BLOB NOT NULL UNIQUE,
        account INTEGER NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
        device TEXT NOT NULL,
        created_at INTEGER NOT NULL,
        seen_at INTEGER NOT NULL,
        expires_at INTEGER NOT NULL,
        mfa_done INTEGER NOT NULL
    );
    CREATE INDEX sessions_account ON sessions(account);
    CREATE TABLE totp (
        account INTEGER PRIMARY KEY REFERENCES accounts(id) ON DELETE CASCADE,
        secret BLOB NOT NULL,
        activated_at INTEGER,
        last_step INTEGER NOT NULL DEFAULT 0
    );
    CREATE TABLE recovery_codes (
        id INTEGER PRIMARY KEY,
        account INTEGER NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
        hash BLOB NOT NULL,
        used_at INTEGER
    );
    CREATE INDEX recovery_codes_account ON recovery_codes(account);
    CREATE TABLE passkeys (
        id INTEGER PRIMARY KEY,
        account INTEGER NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
        credential BLOB NOT NULL UNIQUE,
        public_key BLOB NOT NULL,
        counter INTEGER NOT NULL DEFAULT 0,
        name TEXT NOT NULL,
        created_at INTEGER NOT NULL,
        seen_at INTEGER
    );
    CREATE TABLE trusted_browsers (
        hash BLOB PRIMARY KEY,
        account INTEGER NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
        expires_at INTEGER NOT NULL
    );
    CREATE TABLE audit (
        id INTEGER PRIMARY KEY,
        account INTEGER REFERENCES accounts(id) ON DELETE CASCADE,
        kind TEXT NOT NULL,
        ip TEXT,
        at INTEGER NOT NULL
    );
    CREATE INDEX audit_account ON audit(account, at);
    "#,
];

/// Les adresses IP du journal sont effacées après ce temps (docs/comptes.md : « quelques jours »).
pub const AUDIT_IP_SECS: i64 = 7 * 24 * 3600;
/// Le journal lui-même : 12 mois.
pub const AUDIT_SECS: i64 = 365 * 24 * 3600;

/// Un compte.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Account {
    pub id: i64,
    pub email: String,
    pub name: String,
    /// L'empreinte Argon2id (chaîne PHC).
    pub password: String,
    pub lang: String,
    pub verified_at: Option<i64>,
}

impl Account {
    pub fn verified(&self) -> bool {
        self.verified_at.is_some()
    }
}

/// Une session de la console.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Session {
    pub id: i64,
    pub account: i64,
    pub device: String,
    pub created_at: i64,
    pub seen_at: i64,
    pub expires_at: i64,
    /// La double authentification est faite (ou le compte n'en a pas).
    pub mfa_done: bool,
}

/// La double authentification d'un compte.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Totp {
    /// Le secret chiffré.
    pub secret: Vec<u8>,
    /// `None` : en cours d'activation (le code n'a pas encore été confirmé).
    pub activated_at: Option<i64>,
    /// Le dernier pas de 30 s accepté : un code ne sert qu'une fois.
    pub last_step: i64,
}

/// Ce que donne un code à six chiffres reçu par e-mail.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CodeCheck {
    Ok,
    Wrong,
    /// Aucun code en cours (expiré, trop d'essais, ou jamais envoyé).
    None,
}

/// Essais permis sur un code reçu par e-mail ; au-delà, il faut en redemander un.
pub const CODE_ATTEMPTS: i64 = 5;

pub struct Store {
    conn: Connection,
}

const ACCOUNT_COLS: &str = "id, email, name, password, lang, verified_at";

fn account_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<Account> {
    Ok(Account { id: r.get(0)?, email: r.get(1)?, name: r.get(2)?, password: r.get(3)?, lang: r.get(4)?, verified_at: r.get(5)? })
}

const SESSION_COLS: &str = "id, account, device, created_at, seen_at, expires_at, mfa_done";

fn session_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<Session> {
    Ok(Session {
        id: r.get(0)?,
        account: r.get(1)?,
        device: r.get(2)?,
        created_at: r.get(3)?,
        seen_at: r.get(4)?,
        expires_at: r.get(5)?,
        mfa_done: r.get::<_, i64>(6)? != 0,
    })
}

/// Crée le fichier, réservé au relais, s'il n'existe pas encore.
#[cfg(unix)]
pub(crate) fn create_private(path: &Path) -> std::io::Result<()> {
    use std::os::unix::fs::OpenOptionsExt;
    match std::fs::OpenOptions::new().write(true).create_new(true).mode(0o600).open(path) {
        Ok(_) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => Ok(()),
        Err(e) => Err(e),
    }
}

#[cfg(not(unix))]
pub(crate) fn create_private(path: &Path) -> std::io::Result<()> {
    match std::fs::OpenOptions::new().write(true).create_new(true).open(path) {
        Ok(_) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => Ok(()),
        Err(e) => Err(e),
    }
}

impl Store {
    /// Ouvre (ou crée) la base et applique les migrations.
    pub fn open(path: &Path, now: i64) -> Result<Store> {
        if let Some(dir) = path.parent().filter(|d| !d.as_os_str().is_empty()) {
            std::fs::create_dir_all(dir).with_context(|| format!("dossier de la base {}", dir.display()))?;
        }
        create_private(path).with_context(|| format!("base des comptes {}", path.display()))?;
        let conn = Connection::open(path).with_context(|| format!("base des comptes {}", path.display()))?;
        // WAL : le relais et `kybernow-relay accounts add` peuvent l'ouvrir en même temps.
        let _: String = conn.query_row("PRAGMA journal_mode = WAL", [], |r| r.get(0))?;
        Store::setup(conn, now)
    }

    /// Une base en mémoire (essais).
    pub fn memory(now: i64) -> Result<Store> {
        Store::setup(Connection::open_in_memory()?, now)
    }

    fn setup(mut conn: Connection, now: i64) -> Result<Store> {
        conn.busy_timeout(Duration::from_secs(5))?;
        conn.execute_batch("PRAGMA foreign_keys = ON; PRAGMA synchronous = NORMAL;")?;
        migrate(&mut conn, now)?;
        Ok(Store { conn })
    }

    /// La version du schéma.
    pub fn version(&self) -> Result<usize> {
        let v: i64 = self.conn.query_row("SELECT COALESCE(MAX(version), 0) FROM migrations", [], |r| r.get(0))?;
        Ok(v as usize)
    }

    // --- Comptes -------------------------------------------------------------

    /// Crée un compte ; `None` si l'adresse est déjà prise.
    pub fn create_account(&self, email: &str, name: &str, password: &str, lang: &str, verified_at: Option<i64>, now: i64) -> Result<Option<i64>> {
        let n = self.conn.execute(
            "INSERT INTO accounts (email, name, password, lang, verified_at, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6) ON CONFLICT(email) DO NOTHING",
            params![email, name, password, lang, verified_at, now],
        )?;
        Ok((n == 1).then(|| self.conn.last_insert_rowid()))
    }

    pub fn account_by_email(&self, email: &str) -> Result<Option<Account>> {
        Ok(self
            .conn
            .query_row(&format!("SELECT {ACCOUNT_COLS} FROM accounts WHERE email = ?1 AND deleted_at IS NULL"), [email], account_row)
            .optional()?)
    }

    pub fn account(&self, id: i64) -> Result<Option<Account>> {
        Ok(self
            .conn
            .query_row(&format!("SELECT {ACCOUNT_COLS} FROM accounts WHERE id = ?1 AND deleted_at IS NULL"), [id], account_row)
            .optional()?)
    }

    pub fn set_verified(&self, id: i64, now: i64) -> Result<()> {
        self.conn.execute("UPDATE accounts SET verified_at = COALESCE(verified_at, ?2) WHERE id = ?1", params![id, now])?;
        Ok(())
    }

    pub fn set_password(&self, id: i64, hash: &str) -> Result<()> {
        self.conn.execute("UPDATE accounts SET password = ?2 WHERE id = ?1", params![id, hash])?;
        Ok(())
    }

    /// Change l'adresse d'un compte pas encore vérifié ; faux si elle est prise.
    pub fn set_email(&self, id: i64, email: &str) -> Result<bool> {
        let n = self.conn.execute(
            "UPDATE OR IGNORE accounts SET email = ?2 WHERE id = ?1 AND verified_at IS NULL AND NOT EXISTS (SELECT 1 FROM accounts WHERE email = ?2)",
            params![id, email],
        )?;
        Ok(n == 1)
    }

    // --- Jetons des e-mails --------------------------------------------------

    /// Un nouveau jeton (et son code) remplace les précédents du même usage.
    pub fn replace_email_token(&self, account: i64, purpose: &str, hash: &[u8], code: Option<&str>, expires_at: i64) -> Result<()> {
        self.conn.execute("DELETE FROM email_tokens WHERE account = ?1 AND purpose = ?2", params![account, purpose])?;
        self.conn.execute(
            "INSERT INTO email_tokens (hash, account, purpose, code, expires_at) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![hash, account, purpose, code, expires_at],
        )?;
        Ok(())
    }

    /// Le compte d'un jeton valide (sans le consommer).
    pub fn email_token_account(&self, hash: &[u8], purpose: &str, now: i64) -> Result<Option<i64>> {
        Ok(self
            .conn
            .query_row(
                "SELECT account FROM email_tokens WHERE hash = ?1 AND purpose = ?2 AND expires_at > ?3",
                params![hash, purpose, now],
                |r| r.get(0),
            )
            .optional()?)
    }

    /// Vérifie un code à six chiffres ; chaque erreur compte, et au-delà de
    /// [`CODE_ATTEMPTS`] le code ne vaut plus.
    pub fn check_email_code(&self, account: i64, purpose: &str, code: &str, now: i64) -> Result<CodeCheck> {
        let row: Option<(Vec<u8>, Option<String>, i64)> = self
            .conn
            .query_row(
                "SELECT hash, code, attempts FROM email_tokens WHERE account = ?1 AND purpose = ?2 AND expires_at > ?3",
                params![account, purpose, now],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .optional()?;
        let Some((hash, Some(expected), attempts)) = row else { return Ok(CodeCheck::None) };
        if attempts >= CODE_ATTEMPTS {
            return Ok(CodeCheck::None);
        }
        if super::ct_eq(expected.as_bytes(), code.as_bytes()) {
            return Ok(CodeCheck::Ok);
        }
        self.conn.execute("UPDATE email_tokens SET attempts = attempts + 1 WHERE hash = ?1", [hash])?;
        Ok(CodeCheck::Wrong)
    }

    pub fn delete_email_tokens(&self, account: i64, purpose: &str) -> Result<()> {
        self.conn.execute("DELETE FROM email_tokens WHERE account = ?1 AND purpose = ?2", params![account, purpose])?;
        Ok(())
    }

    // --- Sessions ------------------------------------------------------------

    pub fn create_session(&self, hash: &[u8], account: i64, device: &str, now: i64, expires_at: i64, mfa_done: bool) -> Result<i64> {
        self.conn.execute(
            "INSERT INTO sessions (hash, account, device, created_at, seen_at, expires_at, mfa_done) VALUES (?1, ?2, ?3, ?4, ?4, ?5, ?6)",
            params![hash, account, device, now, expires_at, mfa_done as i64],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn session(&self, hash: &[u8], now: i64) -> Result<Option<Session>> {
        Ok(self
            .conn
            .query_row(&format!("SELECT {SESSION_COLS} FROM sessions WHERE hash = ?1 AND expires_at > ?2"), params![hash, now], session_row)
            .optional()?)
    }

    pub fn touch_session(&self, id: i64, now: i64) -> Result<()> {
        self.conn.execute("UPDATE sessions SET seen_at = ?2 WHERE id = ?1", params![id, now])?;
        Ok(())
    }

    /// Les sessions ouvertes d'un compte (double authentification faite), la plus récente d'abord.
    pub fn sessions(&self, account: i64, now: i64) -> Result<Vec<Session>> {
        let mut stmt = self.conn.prepare(&format!(
            "SELECT {SESSION_COLS} FROM sessions WHERE account = ?1 AND expires_at > ?2 AND mfa_done = 1 ORDER BY seen_at DESC, id DESC"
        ))?;
        let rows = stmt.query_map(params![account, now], session_row)?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    /// Ferme une session du compte ; faux si elle n'est pas à lui.
    pub fn delete_session(&self, id: i64, account: i64) -> Result<bool> {
        Ok(self.conn.execute("DELETE FROM sessions WHERE id = ?1 AND account = ?2", params![id, account])? == 1)
    }

    /// Ferme toutes les sessions du compte, sauf `keep`.
    pub fn delete_sessions(&self, account: i64, keep: Option<i64>) -> Result<usize> {
        Ok(self.conn.execute("DELETE FROM sessions WHERE account = ?1 AND id IS NOT ?2", params![account, keep])?)
    }

    // --- Double authentification ---------------------------------------------

    pub fn totp(&self, account: i64) -> Result<Option<Totp>> {
        Ok(self
            .conn
            .query_row("SELECT secret, activated_at, last_step FROM totp WHERE account = ?1", [account], |r| {
                Ok(Totp { secret: r.get(0)?, activated_at: r.get(1)?, last_step: r.get(2)? })
            })
            .optional()?)
    }

    /// Un secret en attente de confirmation (remplace un secret en attente, jamais un secret actif).
    pub fn set_pending_totp(&self, account: i64, sealed: &[u8]) -> Result<()> {
        self.conn.execute(
            "INSERT INTO totp (account, secret) VALUES (?1, ?2) ON CONFLICT(account) DO UPDATE SET secret = excluded.secret, last_step = 0 WHERE activated_at IS NULL",
            params![account, sealed],
        )?;
        Ok(())
    }

    pub fn activate_totp(&self, account: i64, step: i64, now: i64) -> Result<()> {
        self.conn.execute("UPDATE totp SET activated_at = ?2, last_step = ?3 WHERE account = ?1", params![account, now, step])?;
        Ok(())
    }

    /// Note le pas accepté, s'il est plus récent que le dernier ; faux sinon (code rejoué).
    pub fn use_totp_step(&self, account: i64, step: i64) -> Result<bool> {
        Ok(self.conn.execute("UPDATE totp SET last_step = ?2 WHERE account = ?1 AND last_step < ?2", params![account, step])? == 1)
    }

    /// Coupe la double authentification : le secret, les codes de secours, les navigateurs de confiance.
    pub fn delete_totp(&self, account: i64) -> Result<()> {
        self.conn.execute("DELETE FROM totp WHERE account = ?1", [account])?;
        self.conn.execute("DELETE FROM recovery_codes WHERE account = ?1", [account])?;
        self.conn.execute("DELETE FROM trusted_browsers WHERE account = ?1", [account])?;
        Ok(())
    }

    // --- Codes de secours ----------------------------------------------------

    pub fn replace_recovery_codes(&mut self, account: i64, hashes: &[Vec<u8>]) -> Result<()> {
        let tx = self.conn.transaction()?;
        tx.execute("DELETE FROM recovery_codes WHERE account = ?1", [account])?;
        for h in hashes {
            tx.execute("INSERT INTO recovery_codes (account, hash) VALUES (?1, ?2)", params![account, h])?;
        }
        tx.commit()?;
        Ok(())
    }

    /// Prend un code de secours encore libre ; faux s'il n'existe pas ou a déjà servi.
    pub fn use_recovery_code(&self, account: i64, hash: &[u8], now: i64) -> Result<bool> {
        Ok(self.conn.execute(
            "UPDATE recovery_codes SET used_at = ?3 WHERE account = ?1 AND hash = ?2 AND used_at IS NULL",
            params![account, hash, now],
        )? == 1)
    }

    pub fn recovery_codes_left(&self, account: i64) -> Result<usize> {
        let n: i64 = self.conn.query_row("SELECT COUNT(*) FROM recovery_codes WHERE account = ?1 AND used_at IS NULL", [account], |r| r.get(0))?;
        Ok(n as usize)
    }

    // --- Navigateurs de confiance --------------------------------------------

    pub fn trust_browser(&self, hash: &[u8], account: i64, expires_at: i64) -> Result<()> {
        self.conn.execute(
            "INSERT OR REPLACE INTO trusted_browsers (hash, account, expires_at) VALUES (?1, ?2, ?3)",
            params![hash, account, expires_at],
        )?;
        Ok(())
    }

    pub fn is_trusted(&self, hash: &[u8], account: i64, now: i64) -> Result<bool> {
        let n: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM trusted_browsers WHERE hash = ?1 AND account = ?2 AND expires_at > ?3",
            params![hash, account, now],
            |r| r.get(0),
        )?;
        Ok(n > 0)
    }

    pub fn forget_trusted_browsers(&self, account: i64) -> Result<()> {
        self.conn.execute("DELETE FROM trusted_browsers WHERE account = ?1", [account])?;
        Ok(())
    }

    // --- Journal -------------------------------------------------------------

    pub fn audit(&self, account: Option<i64>, kind: &str, ip: Option<&str>, now: i64) -> Result<()> {
        self.conn.execute("INSERT INTO audit (account, kind, ip, at) VALUES (?1, ?2, ?3, ?4)", params![account, kind, ip, now])?;
        Ok(())
    }

    /// Les lignes du journal d'un compte : `(type, quand)`, la plus récente d'abord.
    pub fn audit_of(&self, account: i64) -> Result<Vec<(String, i64)>> {
        let mut stmt = self.conn.prepare("SELECT kind, at FROM audit WHERE account = ?1 ORDER BY at DESC, id DESC")?;
        let rows = stmt.query_map([account], |r| Ok((r.get(0)?, r.get(1)?)))?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    /// Le ménage : jetons, sessions et navigateurs expirés ; adresses IP du
    /// journal après [`AUDIT_IP_SECS`], le journal après [`AUDIT_SECS`].
    pub fn sweep(&self, now: i64) -> Result<()> {
        self.conn.execute("DELETE FROM email_tokens WHERE expires_at <= ?1", [now])?;
        self.conn.execute("DELETE FROM sessions WHERE expires_at <= ?1", [now])?;
        self.conn.execute("DELETE FROM trusted_browsers WHERE expires_at <= ?1", [now])?;
        self.conn.execute("UPDATE audit SET ip = NULL WHERE ip IS NOT NULL AND at <= ?1", [now - AUDIT_IP_SECS])?;
        self.conn.execute("DELETE FROM audit WHERE at <= ?1", [now - AUDIT_SECS])?;
        Ok(())
    }
}

fn migrate(conn: &mut Connection, now: i64) -> Result<()> {
    conn.execute_batch("CREATE TABLE IF NOT EXISTS migrations (version INTEGER PRIMARY KEY, applied_at INTEGER NOT NULL)")?;
    let current: i64 = conn.query_row("SELECT COALESCE(MAX(version), 0) FROM migrations", [], |r| r.get(0))?;
    let current = current as usize;
    anyhow::ensure!(current <= MIGRATIONS.len(), "base des comptes en version {current}, plus récente que ce relais ({})", MIGRATIONS.len());
    for (i, sql) in MIGRATIONS.iter().enumerate().skip(current) {
        let tx = conn.transaction()?;
        tx.execute_batch(sql).with_context(|| format!("migration {} de la base des comptes", i + 1))?;
        tx.execute("INSERT INTO migrations (version, applied_at) VALUES (?1, ?2)", params![(i + 1) as i64, now])?;
        tx.commit()?;
        tracing::info!(version = i + 1, "base des comptes migrée");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migrations_appliquees_une_fois() {
        let dir = std::env::temp_dir().join(format!("kn-comptes-{}", kybernow_tunnel::id::random_id()));
        let path = dir.join("accounts.db");
        let store = Store::open(&path, 100).unwrap();
        assert_eq!(store.version().unwrap(), MIGRATIONS.len());
        let id = store.create_account("a@exemple.org", "A", "h", "fr", None, 100).unwrap().unwrap();
        drop(store);
        // Rouverte : rien n'est rejoué, les données restent.
        let store = Store::open(&path, 200).unwrap();
        assert_eq!(store.version().unwrap(), MIGRATIONS.len());
        assert_eq!(store.account(id).unwrap().unwrap().email, "a@exemple.org");
        let applied: i64 = store.conn.query_row("SELECT COUNT(*) FROM migrations", [], |r| r.get(0)).unwrap();
        assert_eq!(applied as usize, MIGRATIONS.len());
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(std::fs::metadata(&path).unwrap().permissions().mode() & 0o777, 0o600, "base réservée au relais");
        }
        // Une base plus récente que le relais : refusée.
        store.conn.execute("INSERT INTO migrations (version, applied_at) VALUES (999, 0)", []).unwrap();
        drop(store);
        assert!(Store::open(&path, 300).is_err());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn adresse_unique_et_codes_limites() {
        let s = Store::memory(0).unwrap();
        let id = s.create_account("a@exemple.org", "A", "h", "fr", None, 0).unwrap().unwrap();
        assert!(s.create_account("a@exemple.org", "B", "h", "en", None, 0).unwrap().is_none());
        s.replace_email_token(id, "verify", b"jeton", Some("123456"), 1000).unwrap();
        assert_eq!(s.email_token_account(b"jeton", "verify", 10).unwrap(), Some(id));
        assert_eq!(s.email_token_account(b"jeton", "reset", 10).unwrap(), None);
        assert_eq!(s.email_token_account(b"jeton", "verify", 1000).unwrap(), None, "expiré");
        for _ in 0..CODE_ATTEMPTS {
            assert_eq!(s.check_email_code(id, "verify", "000000", 10).unwrap(), CodeCheck::Wrong);
        }
        assert_eq!(s.check_email_code(id, "verify", "123456", 10).unwrap(), CodeCheck::None, "trop d'essais : le code ne vaut plus");
        s.replace_email_token(id, "verify", b"autre", Some("654321"), 1000).unwrap();
        assert_eq!(s.check_email_code(id, "verify", "654321", 10).unwrap(), CodeCheck::Ok);
        assert_eq!(s.email_token_account(b"jeton", "verify", 10).unwrap(), None, "l'ancien jeton est remplacé");
        // Changer d'adresse : seulement vers une adresse libre, avant la vérification.
        let other = s.create_account("b@exemple.org", "B", "h", "fr", Some(1), 0).unwrap().unwrap();
        assert!(!s.set_email(id, "b@exemple.org").unwrap());
        assert!(s.set_email(id, "c@exemple.org").unwrap());
        assert!(!s.set_email(other, "d@exemple.org").unwrap(), "compte vérifié");
    }

    #[test]
    fn sessions_totp_et_menage() {
        let mut s = Store::memory(0).unwrap();
        let id = s.create_account("a@exemple.org", "A", "h", "fr", Some(0), 0).unwrap().unwrap();
        let a = s.create_session(b"s1", id, "Firefox", 10, 100, true).unwrap();
        let b = s.create_session(b"s2", id, "Safari", 20, 100, true).unwrap();
        s.create_session(b"s3", id, "Safari", 20, 100, false).unwrap();
        assert_eq!(s.sessions(id, 30).unwrap().len(), 2, "les demi-sessions (code à venir) ne sont pas listées");
        assert_eq!(s.delete_sessions(id, Some(a)).unwrap(), 2);
        assert!(s.session(b"s1", 30).unwrap().is_some());
        assert!(s.session(b"s2", 30).unwrap().is_none());
        assert!(!s.delete_session(b, id).unwrap());
        // Un secret en attente se remplace ; actif, il reste.
        s.set_pending_totp(id, b"x").unwrap();
        s.set_pending_totp(id, b"y").unwrap();
        assert_eq!(s.totp(id).unwrap().unwrap().secret, b"y");
        s.activate_totp(id, 5, 40).unwrap();
        s.set_pending_totp(id, b"z").unwrap();
        assert_eq!(s.totp(id).unwrap().unwrap().secret, b"y");
        assert!(s.use_totp_step(id, 6).unwrap());
        assert!(!s.use_totp_step(id, 6).unwrap(), "un pas ne sert qu'une fois");
        s.replace_recovery_codes(id, &[b"r1".to_vec(), b"r2".to_vec()]).unwrap();
        assert!(s.use_recovery_code(id, b"r1", 50).unwrap());
        assert!(!s.use_recovery_code(id, b"r1", 50).unwrap());
        assert_eq!(s.recovery_codes_left(id).unwrap(), 1);
        s.trust_browser(b"t", id, 100).unwrap();
        assert!(s.is_trusted(b"t", id, 50).unwrap());
        s.delete_totp(id).unwrap();
        assert!(s.totp(id).unwrap().is_none() && s.recovery_codes_left(id).unwrap() == 0 && !s.is_trusted(b"t", id, 50).unwrap());
        // Le ménage : sessions expirées, adresses du journal effacées après quelques jours.
        s.audit(Some(id), "connexion", Some("192.0.2.1"), 0).unwrap();
        s.sweep(AUDIT_IP_SECS + 1).unwrap();
        assert!(s.session(b"s1", 0).unwrap().is_none());
        let ip: Option<String> = s.conn.query_row("SELECT ip FROM audit", [], |r| r.get(0)).unwrap();
        assert_eq!(ip, None);
        assert_eq!(s.audit_of(id).unwrap().len(), 1);
        s.sweep(AUDIT_SECS + 1).unwrap();
        assert!(s.audit_of(id).unwrap().is_empty());
    }
}
