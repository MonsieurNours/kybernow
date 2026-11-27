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
    // 2 : lot 1b (passkeys, conditions et confidentialité acceptées, défis WebAuthn déjà servis).
    r#"
    ALTER TABLE accounts ADD COLUMN webauthn_user BLOB;
    ALTER TABLE accounts ADD COLUMN terms_version TEXT;
    ALTER TABLE accounts ADD COLUMN terms_accepted_at INTEGER;
    ALTER TABLE accounts ADD COLUMN privacy_version TEXT;
    ALTER TABLE accounts ADD COLUMN privacy_accepted_at INTEGER;
    ALTER TABLE passkeys ADD COLUMN alg INTEGER NOT NULL DEFAULT -7;
    ALTER TABLE passkeys ADD COLUMN transports TEXT NOT NULL DEFAULT '';
    CREATE INDEX passkeys_account ON passkeys(account);
    CREATE TABLE webauthn_used (
        hash BLOB PRIMARY KEY,
        expires_at INTEGER NOT NULL
    );
    "#,
    // 3 : lot 2 (Stations rattachées) et lot 3 (appareils connectés), voir `super::stations`.
    super::stations::MIGRATION,
    // 4 : lot 4 (droits donnés aux personnes), voir `super::people`.
    super::people::MIGRATION,
    // 5 : lot 5 (organisations, groupes, journal, jetons d'enrôlement ; `stations` reconstruite), voir `super::orgs`.
    super::orgs::MIGRATION,
    // 6 : l'espace du compte (docs/comptes.md, « L'espace du compte ») : famille ou équipe, tour d'accueil vu.
    r#"
    ALTER TABLE organizations ADD COLUMN kind TEXT NOT NULL DEFAULT 'team';
    ALTER TABLE accounts ADD COLUMN welcomed_at INTEGER;
    "#,
    // 7 : liens d'accès et groupes d'ordinateurs (docs/comptes.md, « Regarder, liens d'accès et groupes
    // d'ordinateurs »), voir `super::links` et `super::station_groups`.
    super::links::MIGRATION,
    // 8 : l'envoi des invitations, qui expirent après 30 jours (docs/comptes.md, « Invitations, ajout de
    // membres et sessions »), voir `super::people`.
    super::people::INVITES_MIGRATION,
    // 9 : les réglages synchronisés, par compte et par ordinateur (docs/comptes.md, « Réglages synchronisés et
    // déplacer un ordinateur »), voir `super::settings`.
    super::settings::MIGRATION,
    // 10 : une échéance par droit donné (audit du 27/11, REL-03), voir `super::people`.
    super::people::RIGHT_ENDS_MIGRATION,
];

/// Les adresses IP du journal sont effacées après ce temps (docs/comptes.md : « quelques jours »).
pub const AUDIT_IP_SECS: i64 = 7 * 24 * 3600;
/// Le journal lui-même : 12 mois.
pub const AUDIT_SECS: i64 = 365 * 24 * 3600;
/// Un compte jamais vérifié est effacé après ce temps (docs/juridique/README.md).
pub const UNVERIFIED_SECS: i64 = 7 * 24 * 3600;

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

/// Une passkey (lot 1b) : la clé publique, jamais la clé privée.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Passkey {
    pub id: i64,
    pub account: i64,
    /// L'identifiant de la passkey, donné par l'authentificateur.
    pub credential: Vec<u8>,
    /// La clé publique : ES256, le point non compressé ; Ed25519, 32 octets ; RS256, `len(n)`, `n`, `e`.
    pub public_key: Vec<u8>,
    /// L'algorithme COSE : -7 (ES256), -8 (Ed25519), -257 (RS256).
    pub alg: i64,
    pub counter: i64,
    pub name: String,
    pub transports: String,
    pub created_at: i64,
    pub seen_at: Option<i64>,
}

const PASSKEY_COLS: &str = "id, account, credential, public_key, alg, counter, name, transports, created_at, seen_at";

fn passkey_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<Passkey> {
    Ok(Passkey {
        id: r.get(0)?,
        account: r.get(1)?,
        credential: r.get(2)?,
        public_key: r.get(3)?,
        alg: r.get(4)?,
        counter: r.get(5)?,
        name: r.get(6)?,
        transports: r.get(7)?,
        created_at: r.get(8)?,
        seen_at: r.get(9)?,
    })
}

/// Une nouvelle passkey, vérifiée, à garder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewPasskey {
    pub credential: Vec<u8>,
    pub public_key: Vec<u8>,
    pub alg: i64,
    pub counter: i64,
}

/// Les versions des conditions d'utilisation et de la politique de confidentialité acceptées, et quand.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Terms {
    pub terms: Option<String>,
    pub terms_at: Option<i64>,
    pub privacy: Option<String>,
    pub privacy_at: Option<i64>,
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
    /// Les modules du lot 2 (`super::stations`, `super::devices`) y ajoutent leurs requêtes.
    pub(super) conn: Connection,
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

    /// Le tour d'accueil de l'espace est vu (ou passé).
    pub fn set_welcomed(&self, account: i64, now: i64) -> Result<()> {
        self.conn.execute("UPDATE accounts SET welcomed_at = ?2 WHERE id = ?1 AND welcomed_at IS NULL", params![account, now])?;
        Ok(())
    }

    /// Le tour d'accueil a été vu (ou passé).
    pub fn welcomed(&self, account: i64) -> Result<bool> {
        let at: Option<Option<i64>> = self.conn.query_row("SELECT welcomed_at FROM accounts WHERE id = ?1", [account], |r| r.get(0)).optional()?;
        Ok(at.flatten().is_some())
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

    /// L'adresse est vérifiée ; les invitations qui l'attendaient passent au compte (lots 4 et 5).
    pub fn set_verified(&self, id: i64, now: i64) -> Result<()> {
        self.conn.execute("UPDATE accounts SET verified_at = COALESCE(verified_at, ?2) WHERE id = ?1", params![id, now])?;
        let claimed = self.claim_invitations(id, now)?;
        if claimed > 0 {
            tracing::info!(invitations = claimed, "invitations reprises par un compte vérifié");
        }
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
    /// Le code tapé, sous ses formes possibles (`candidates` : son empreinte, et le code lui-même
    /// pour un code gardé en clair avant REL-22).
    pub fn check_email_code(&self, account: i64, purpose: &str, candidates: &[&str], now: i64) -> Result<CodeCheck> {
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
        if candidates.iter().any(|c| super::ct_eq(expected.as_bytes(), c.as_bytes())) {
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
    /// Le nombre de comptes dont la double authentification (TOTP) est activée.
    pub fn active_totp_count(&self) -> Result<i64> {
        Ok(self.conn.query_row("SELECT COUNT(*) FROM totp WHERE activated_at IS NOT NULL", [], |r| r.get(0))?)
    }

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

    // --- Passkeys (lot 1b) ----------------------------------------------------

    /// L'identifiant WebAuthn du compte (`user.id`) : 32 octets aléatoires, créés à la première passkey.
    pub fn webauthn_user(&self, account: i64) -> Result<Vec<u8>> {
        let current: Option<Vec<u8>> = self.conn.query_row("SELECT webauthn_user FROM accounts WHERE id = ?1", [account], |r| r.get(0))?;
        if let Some(id) = current {
            return Ok(id);
        }
        let id = super::random::<32>().to_vec();
        self.conn.execute("UPDATE accounts SET webauthn_user = ?2 WHERE id = ?1 AND webauthn_user IS NULL", params![account, id])?;
        Ok(self.conn.query_row("SELECT webauthn_user FROM accounts WHERE id = ?1", [account], |r| r.get(0))?)
    }

    /// Garde une passkey ; `None` si cet identifiant est déjà connu (de ce compte ou d'un autre).
    pub fn add_passkey(&self, account: i64, key: &NewPasskey, name: &str, transports: &str, now: i64) -> Result<Option<i64>> {
        let n = self.conn.execute(
            "INSERT INTO passkeys (account, credential, public_key, alg, counter, name, transports, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8) ON CONFLICT(credential) DO NOTHING",
            params![account, key.credential, key.public_key, key.alg, key.counter, name, transports, now],
        )?;
        Ok((n == 1).then(|| self.conn.last_insert_rowid()))
    }

    /// Les passkeys d'un compte, la plus ancienne d'abord.
    pub fn passkeys(&self, account: i64) -> Result<Vec<Passkey>> {
        let mut stmt = self.conn.prepare(&format!("SELECT {PASSKEY_COLS} FROM passkeys WHERE account = ?1 ORDER BY created_at, id"))?;
        let rows = stmt.query_map([account], passkey_row)?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    /// La passkey de cet identifiant, si son compte existe.
    pub fn passkey_by_credential(&self, credential: &[u8]) -> Result<Option<Passkey>> {
        Ok(self
            .conn
            .query_row(
                &format!(
                    "SELECT {PASSKEY_COLS} FROM passkeys WHERE credential = ?1 AND account IN (SELECT id FROM accounts WHERE deleted_at IS NULL)"
                ),
                [credential],
                passkey_row,
            )
            .optional()?)
    }

    /// Note une connexion par la passkey : son compteur, l'heure.
    pub fn use_passkey(&self, id: i64, counter: i64, now: i64) -> Result<()> {
        self.conn.execute("UPDATE passkeys SET counter = ?2, seen_at = ?3 WHERE id = ?1", params![id, counter, now])?;
        Ok(())
    }

    /// Renomme une passkey du compte ; faux si elle n'est pas à lui.
    pub fn rename_passkey(&self, id: i64, account: i64, name: &str) -> Result<bool> {
        Ok(self.conn.execute("UPDATE passkeys SET name = ?3 WHERE id = ?1 AND account = ?2", params![id, account, name])? == 1)
    }

    /// Retire une passkey du compte ; faux si elle n'est pas à lui.
    /// Retire les passkeys ajoutées à partir de `since` ; leur nombre.
    pub fn delete_passkeys_since(&self, account: i64, since: i64) -> Result<usize> {
        Ok(self.conn.execute("DELETE FROM passkeys WHERE account = ?1 AND created_at >= ?2", params![account, since])?)
    }

    pub fn delete_passkey(&self, id: i64, account: i64) -> Result<bool> {
        Ok(self.conn.execute("DELETE FROM passkeys WHERE id = ?1 AND account = ?2", params![id, account])? == 1)
    }

    /// Note un défi WebAuthn servi ; faux s'il l'a déjà été (rejeu).
    pub fn use_challenge(&self, hash: &[u8], expires_at: i64) -> Result<bool> {
        Ok(self.conn.execute("INSERT OR IGNORE INTO webauthn_used (hash, expires_at) VALUES (?1, ?2)", params![hash, expires_at])? == 1)
    }

    // --- Conditions, export, suppression (lot 1b) ----------------------------

    /// Les versions des conditions et de la politique de confidentialité acceptées à l'inscription.
    pub fn set_terms(&self, account: i64, terms: &str, privacy: &str, now: i64) -> Result<()> {
        self.conn.execute(
            "UPDATE accounts SET terms_version = ?2, terms_accepted_at = ?4, privacy_version = ?3, privacy_accepted_at = ?4 WHERE id = ?1",
            params![account, terms, privacy, now],
        )?;
        Ok(())
    }

    /// Ce que le compte a accepté (rien pour un compte créé par l'administrateur du relais).
    pub fn terms(&self, account: i64) -> Result<Terms> {
        Ok(self.conn.query_row(
            "SELECT terms_version, terms_accepted_at, privacy_version, privacy_accepted_at FROM accounts WHERE id = ?1",
            [account],
            |r| Ok(Terms { terms: r.get(0)?, terms_at: r.get(1)?, privacy: r.get(2)?, privacy_at: r.get(3)? }),
        )?)
    }

    /// Les données d'un compte, telles que l'export les donne (sans empreinte ni secret).
    pub fn export(&self, account: i64, now: i64) -> Result<Option<serde_json::Value>> {
        use serde_json::json;
        let row: Option<(String, String, String, Option<i64>, i64, String)> = self
            .conn
            .query_row(
                "SELECT email, name, lang, verified_at, created_at, plan FROM accounts WHERE id = ?1 AND deleted_at IS NULL",
                [account],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?)),
            )
            .optional()?;
        let Some((email, name, lang, verified_at, created_at, plan)) = row else { return Ok(None) };
        let Terms { terms, terms_at, privacy, privacy_at } = self.terms(account)?;
        let totp = self.totp(account)?.and_then(|t| t.activated_at);
        let passkeys: Vec<_> = self
            .passkeys(account)?
            .into_iter()
            .map(|p| {
                json!({
                    "name": p.name,
                    "credential": super::b64(&p.credential),
                    "algorithm": p.alg,
                    "transports": p.transports,
                    "created_at": iso(p.created_at),
                    "last_used_at": p.seen_at.map(iso),
                })
            })
            .collect();
        let sessions: Vec<_> = self
            .sessions(account, now)?
            .into_iter()
            .map(|s| json!({ "device": s.device, "created_at": iso(s.created_at), "last_seen_at": iso(s.seen_at), "expires_at": iso(s.expires_at) }))
            .collect();
        let trusted: i64 =
            self.conn.query_row("SELECT COUNT(*) FROM trusted_browsers WHERE account = ?1 AND expires_at > ?2", params![account, now], |r| r.get(0))?;
        let stations: Vec<_> = self
            .stations_of(account)?
            .into_iter()
            .map(|s| json!({ "id": s.id, "name": s.name, "owner": s.owner_kind, "attached_at": iso(s.attached_at) }))
            .collect();
        // Lots 4 et 5 : les ordinateurs ouverts à ce compte (et ses droits), ses organisations.
        let opened: Vec<_> = self
            .visible_stations(account, now)?
            .into_iter()
            .filter(|s| s.account != Some(account))
            .map(|s| {
                let rights = self.granted_rights(account, &s.id, now).unwrap_or_default();
                json!({ "id": s.id, "name": s.name, "rights": rights })
            })
            .collect();
        let orgs: Vec<_> = self
            .orgs_of(account)?
            .into_iter()
            .map(|(o, role)| json!({ "id": o.public_id, "name": o.name, "role": role.as_str() }))
            .collect();
        let devices: Vec<_> = self
            .devices_of(account)?
            .into_iter()
            .map(|d| json!({ "name": d.name, "kind": d.kind, "created_at": iso(d.created_at), "last_seen_at": iso(d.seen_at) }))
            .collect();
        let mut stmt = self.conn.prepare("SELECT kind, ip, at FROM audit WHERE account = ?1 ORDER BY at, id")?;
        let audit: Vec<_> = stmt
            .query_map([account], |r| Ok((r.get::<_, String>(0)?, r.get::<_, Option<String>>(1)?, r.get::<_, i64>(2)?)))?
            .collect::<rusqlite::Result<Vec<_>>>()?
            .into_iter()
            .map(|(kind, ip, at)| json!({ "event": kind, "ip": ip, "at": iso(at) }))
            .collect();
        Ok(Some(json!({
            "format": "kybernow-account-export/1",
            "exported_at": iso(now),
            "account": {
                "email": email,
                "name": name,
                "language": lang,
                "plan": plan,
                "created_at": iso(created_at),
                "verified_at": verified_at.map(iso),
                "terms": { "version": terms, "accepted_at": terms_at.map(iso) },
                "privacy_policy": { "version": privacy, "accepted_at": privacy_at.map(iso) },
            },
            "security": {
                "two_factor_since": totp.map(iso),
                "recovery_codes_left": self.recovery_codes_left(account)?,
                "trusted_browsers": trusted,
                "passkeys": passkeys,
            },
            "sessions": sessions,
            "stations": stations,
            "opened_to_me": opened,
            "organizations": orgs,
            "devices": devices,
            // Migration 9 : les réglages synchronisés, par ordinateur.
            "synced_settings": self
                .settings_of(account)?
                .into_iter()
                .map(|s| json!({ "station": s.station, "values": s.values, "updated_at": iso(s.updated_at) }))
                .collect::<Vec<_>>(),
            "audit": audit,
        })))
    }

    /// Efface un compte et tout ce qui lui appartient (sessions, passkeys,
    /// journal, Stations rattachées, appareils connectés…). Les premières
    /// venues d'appareils (d'autres comptes) sur ses Stations partent avec elles,
    /// comme au détachement.
    pub fn delete_account(&self, account: i64) -> Result<bool> {
        let tx = self.conn.unchecked_transaction()?;
        tx.execute("DELETE FROM device_stations WHERE station IN (SELECT id FROM stations WHERE account = ?1)", [account])?;
        // Lots 4 et 5 : les droits donnés sur ses Stations ; ses organisations gardent un Propriétaire.
        tx.execute("DELETE FROM grants WHERE station IN (SELECT id FROM stations WHERE account = ?1)", [account])?;
        // Migration 7 : les liens d'accès de ses Stations.
        tx.execute("DELETE FROM access_links WHERE station IN (SELECT id FROM stations WHERE account = ?1)", [account])?;
        // Migration 9 : les réglages des autres comptes sur ses Stations (les siens partent avec le compte).
        tx.execute("DELETE FROM computer_settings WHERE station IN (SELECT id FROM stations WHERE account = ?1)", [account])?;
        super::orgs::before_account_deleted(&tx, account)?;
        let n = tx.execute("DELETE FROM accounts WHERE id = ?1", [account])?;
        tx.commit()?;
        Ok(n == 1)
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
    /// journal après [`AUDIT_IP_SECS`], le journal après [`AUDIT_SECS`] ;
    /// comptes jamais vérifiés après [`UNVERIFIED_SECS`] ; défis WebAuthn expirés.
    pub fn sweep(&self, now: i64) -> Result<()> {
        let tx = self.conn.unchecked_transaction()?;
        tx.execute(
            "DELETE FROM device_stations WHERE station IN (SELECT s.id FROM stations s JOIN accounts a ON a.id = s.account
             WHERE a.verified_at IS NULL AND a.created_at <= ?1)",
            [now - UNVERIFIED_SECS],
        )?;
        let gone = tx.execute("DELETE FROM accounts WHERE verified_at IS NULL AND created_at <= ?1", [now - UNVERIFIED_SECS])?;
        tx.commit()?;
        if gone > 0 {
            tracing::info!(comptes = gone, "comptes jamais vérifiés effacés");
        }
        self.conn.execute("DELETE FROM webauthn_used WHERE expires_at <= ?1", [now])?;
        self.conn.execute("DELETE FROM email_tokens WHERE expires_at <= ?1", [now])?;
        self.conn.execute("DELETE FROM sessions WHERE expires_at <= ?1", [now])?;
        // Les sessions d'appareil qui n'ont pas servi depuis 90 jours (docs/comptes.md, « Sessions »).
        self.expire_idle_devices(now)?;
        self.conn.execute("DELETE FROM trusted_browsers WHERE expires_at <= ?1", [now])?;
        self.conn.execute("UPDATE audit SET ip = NULL WHERE ip IS NOT NULL AND at <= ?1", [now - AUDIT_IP_SECS])?;
        self.conn.execute("DELETE FROM audit WHERE at <= ?1", [now - AUDIT_SECS])?;
        // Lot 4 : les invitations échues n'attendent plus ; lot 5 : le journal des organisations.
        self.conn.execute("DELETE FROM grants WHERE expires_at IS NOT NULL AND expires_at <= ?1", [now - AUDIT_IP_SECS])?;
        self.sweep_orgs(now)?;
        // Migration 7 : les liens d'accès fermés depuis un moment, les droits échus des groupes d'ordinateurs.
        self.sweep_links(now)?;
        self.conn.execute("DELETE FROM group_grants WHERE expires_at IS NOT NULL AND expires_at <= ?1", [now - AUDIT_IP_SECS])?;
        // Migration 9 : les réglages synchronisés des ordinateurs détachés ou qui ne sont plus ouverts au compte.
        self.sweep_settings(now)?;
        Ok(())
    }
}

/// Une heure Unix en ISO 8601 (UTC) : `2026-11-23T08:30:00Z`.
pub fn iso(unix: i64) -> String {
    let days = unix.div_euclid(86_400);
    let secs = unix.rem_euclid(86_400);
    // Jours depuis 1970 vers le calendrier (H. Hinnant, « civil_from_days »).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!("{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}Z", secs / 3600, secs / 60 % 60, secs % 60)
}

/// Des écritures tout ou rien, même dans une transaction déjà ouverte (un `SAVEPOINT`, qui
/// s'imbrique, au lieu de `BEGIN`, qui ne s'imbrique pas) : REL-25 et REL-26.
pub(super) fn atomic<T>(conn: &Connection, f: impl FnOnce() -> Result<T>) -> Result<T> {
    conn.execute_batch("SAVEPOINT kn_atomic")?;
    match f() {
        Ok(v) => {
            conn.execute_batch("RELEASE kn_atomic")?;
            Ok(v)
        }
        Err(e) => {
            let _ = conn.execute_batch("ROLLBACK TO kn_atomic; RELEASE kn_atomic");
            Err(e)
        }
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
    fn tout_ou_rien_meme_imbrique() {
        // REL-25, REL-26 : une erreur au milieu défait tout, dans une transaction déjà ouverte aussi.
        let s = Store::memory(0).unwrap();
        let a = s.create_account("a@exemple.org", "A", "h", "fr", Some(0), 0).unwrap().unwrap();
        s.attach_station("st1", "PC", a, a, 10).unwrap();
        let r: Result<()> = atomic(&s.conn, || {
            s.conn.execute("DELETE FROM stations WHERE id = 'st1'", [])?;
            anyhow::bail!("panne")
        });
        assert!(r.is_err());
        assert!(s.station("st1").unwrap().is_some(), "rien n'a changé");
        let tx = s.conn.unchecked_transaction().unwrap();
        s.attach_station("st2", "PC 2", a, a, 10).unwrap();
        tx.commit().unwrap();
        assert!(s.station("st2").unwrap().is_some(), "un rattachement dans une transaction ouverte passe");
        assert!(s.detach_station("st2", None).unwrap());
    }

    #[test]
    fn migration_8_les_invitations_en_attente_repartent_pour_30_jours() {
        // Une base en version 7, avec une invitation d'ordinateur et une d'organisation, anciennes.
        let mut conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("PRAGMA foreign_keys = ON; CREATE TABLE migrations (version INTEGER PRIMARY KEY, applied_at INTEGER NOT NULL)").unwrap();
        for (i, sql) in MIGRATIONS.iter().take(7).enumerate() {
            conn.execute_batch(sql).unwrap();
            conn.execute("INSERT INTO migrations (version, applied_at) VALUES (?1, 0)", [(i + 1) as i64]).unwrap();
        }
        conn.execute("INSERT INTO accounts (email, name, password, created_at, verified_at) VALUES ('a@exemple.org', 'A', 'h', 0, 0)", []).unwrap();
        conn.execute("INSERT INTO organizations (public_id, name, created_at) VALUES ('org_x', 'Org', 0)", []).unwrap();
        conn.execute("INSERT INTO grants (station, email, rights, created_at) VALUES ('st1', 'c@exemple.org', 'session', 100)", []).unwrap();
        conn.execute("INSERT INTO grants (station, account, rights, created_at) VALUES ('st1', 1, 'session', 100)", []).unwrap();
        conn.execute("INSERT INTO org_invites (org, email, role, created_at) VALUES (1, 'c@exemple.org', 'member', 100)", []).unwrap();
        let before = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs() as i64;
        migrate(&mut conn, before).unwrap();
        let sent: Vec<Option<i64>> = conn.prepare("SELECT sent_at FROM grants ORDER BY id").unwrap().query_map([], |r| r.get(0)).unwrap().map(|r| r.unwrap()).collect();
        assert!(sent[0].is_some_and(|t| t >= before && t < before + 60), "invitation : l'heure de la migration");
        assert_eq!(sent[1], None, "un droit donné à un compte n'est pas une invitation");
        let org_sent: i64 = conn.query_row("SELECT sent_at FROM org_invites", [], |r| r.get(0)).unwrap();
        assert!(org_sent >= before && org_sent < before + 60);
    }

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
            assert_eq!(s.check_email_code(id, "verify", &["000000"], 10).unwrap(), CodeCheck::Wrong);
        }
        assert_eq!(s.check_email_code(id, "verify", &["123456"], 10).unwrap(), CodeCheck::None, "trop d'essais : le code ne vaut plus");
        s.replace_email_token(id, "verify", b"autre", Some("654321"), 1000).unwrap();
        assert_eq!(s.check_email_code(id, "verify", &["654321"], 10).unwrap(), CodeCheck::Ok);
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

    #[test]
    fn passkeys_conditions_export_et_suppression() {
        let mut s = Store::memory(0).unwrap();
        let id = s.create_account("a@exemple.org", "A", "h", "fr", Some(0), 0).unwrap().unwrap();
        let other = s.create_account("b@exemple.org", "B", "h", "fr", Some(0), 0).unwrap().unwrap();
        // L'identifiant WebAuthn : créé une fois, propre au compte.
        let user = s.webauthn_user(id).unwrap();
        assert_eq!(user.len(), 32);
        assert_eq!(s.webauthn_user(id).unwrap(), user);
        assert_ne!(s.webauthn_user(other).unwrap(), user);
        let key = NewPasskey { credential: b"cred".to_vec(), public_key: vec![4; 65], alg: -7, counter: 3 };
        let pk = s.add_passkey(id, &key, "Firefox, Windows", "internal", 10).unwrap().unwrap();
        assert_eq!(s.add_passkey(other, &key, "x", "", 10).unwrap(), None, "un identifiant n'appartient qu'à un compte");
        let found = s.passkey_by_credential(b"cred").unwrap().unwrap();
        assert_eq!((found.account, found.alg, found.counter, found.seen_at), (id, -7, 3, None));
        s.use_passkey(pk, 4, 20).unwrap();
        assert!(!s.rename_passkey(pk, other, "volée").unwrap());
        assert!(s.rename_passkey(pk, id, "Clé USB").unwrap());
        let list = s.passkeys(id).unwrap();
        assert_eq!((list[0].name.as_str(), list[0].counter, list[0].seen_at), ("Clé USB", 4, Some(20)));
        assert!(!s.delete_passkey(pk, other).unwrap());
        // Un défi ne sert qu'une fois.
        assert!(s.use_challenge(b"defi", 100).unwrap());
        assert!(!s.use_challenge(b"defi", 100).unwrap());
        // Conditions acceptées ; l'export ne donne ni empreinte ni secret.
        s.set_terms(id, "0.1", "0.2", 30).unwrap();
        assert_eq!(s.terms(id).unwrap(), Terms { terms: Some("0.1".into()), terms_at: Some(30), privacy: Some("0.2".into()), privacy_at: Some(30) });
        s.create_session(b"s", id, "Firefox", 10, 1000, true).unwrap();
        s.audit(Some(id), "connexion", Some("192.0.2.1"), 10).unwrap();
        s.set_pending_totp(id, b"secret").unwrap();
        s.activate_totp(id, 1, 15).unwrap();
        s.replace_recovery_codes(id, &[b"r1".to_vec()]).unwrap();
        let export = s.export(id, 40).unwrap().unwrap();
        assert_eq!(export["account"]["email"], "a@exemple.org");
        assert_eq!(export["account"]["terms"]["version"], "0.1");
        assert_eq!(export["account"]["privacy_policy"]["accepted_at"], "1970-01-01T00:00:30Z");
        assert_eq!(export["security"]["passkeys"][0]["name"], "Clé USB");
        assert_eq!(export["security"]["two_factor_since"], "1970-01-01T00:00:15Z");
        assert_eq!(export["sessions"][0]["device"], "Firefox");
        assert_eq!(export["audit"][0]["ip"], "192.0.2.1");
        let text = export.to_string();
        for secret in ["secret", "\"h\"", "r1", "public_key"] {
            assert!(!text.contains(secret), "{secret} dans l'export");
        }
        // La suppression efface tout ce qui appartient au compte.
        assert!(s.delete_account(id).unwrap());
        assert!(s.account(id).unwrap().is_none() && s.export(id, 40).unwrap().is_none());
        assert!(s.passkey_by_credential(b"cred").unwrap().is_none());
        for table in ["sessions", "passkeys", "totp", "recovery_codes", "audit"] {
            let n: i64 = s.conn.query_row(&format!("SELECT COUNT(*) FROM {table} WHERE account = ?1"), [id], |r| r.get(0)).unwrap();
            assert_eq!(n, 0, "{table}");
        }
        assert!(s.account(other).unwrap().is_some());
    }

    #[test]
    fn comptes_jamais_verifies_effaces_apres_sept_jours() {
        let s = Store::memory(0).unwrap();
        let unverified = s.create_account("a@exemple.org", "A", "h", "fr", None, 100).unwrap().unwrap();
        let verified = s.create_account("b@exemple.org", "B", "h", "fr", Some(200), 100).unwrap().unwrap();
        s.sweep(100 + UNVERIFIED_SECS - 1).unwrap();
        assert!(s.account(unverified).unwrap().is_some());
        s.use_challenge(b"d", 100 + UNVERIFIED_SECS).unwrap();
        s.sweep(100 + UNVERIFIED_SECS).unwrap();
        assert!(s.account(unverified).unwrap().is_none());
        assert!(s.account(verified).unwrap().is_some());
        assert!(s.use_challenge(b"d", 0).unwrap(), "défi expiré oublié");
    }

    #[test]
    fn heures_iso() {
        assert_eq!(iso(0), "1970-01-01T00:00:00Z");
        assert_eq!(iso(1_795_392_000 + 8 * 3600 + 30 * 60 + 5), "2026-11-23T08:30:05Z");
        assert_eq!(iso(951_782_400), "2000-02-29T00:00:00Z");
    }
}
