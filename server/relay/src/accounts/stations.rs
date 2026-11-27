//! Les Stations rattachées à un compte (docs/comptes.md, « Rattacher une
//! Station », lot 2) et la clé de signature des jetons d'accès (« Le jeton
//! d'accès »).
//!
//! Le rattachement part d'Admin (« Compte KyberNow › Se connecter ») : par sa
//! connexion de contrôle, la Station demande un code (`attach_request`), le
//! relais le garde 15 minutes en mémoire et rend la page de la console à ouvrir
//! (`attach_code`, `/compte/rattacher?code=…`). La personne connectée y voit
//! « Rattacher {nom} », choisit le propriétaire et confirme ; le relais note le
//! lien (table `stations`) et envoie `attached {account, owner, keys}` à la
//! Station, qui épingle les clés. Personne ne peut donc rattacher la Station
//! d'un autre : le code ne vient que de sa connexion de contrôle.
//!
//! La clé de signature (Ed25519, PKCS#8) est dans un fichier à part, à côté
//! de la clé des comptes : `signing.key`, créé au premier démarrage (0600).
//! Rotation : une clé suivante `signing.next.key`, déposée par l'administrateur
//! du relais, est publiée avec la clé en cours (`GET /v1/keys`) et donnée aux
//! Stations qui se rattachent ou se reconnectent ; quand toutes l'ont, elle
//! remplace `signing.key`.

use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex, RwLock, Weak};

use anyhow::{Context, Result};
use kybernow_tunnel::access::{KeySet, Signer};
use kybernow_tunnel::id::base32_lower;
use kybernow_tunnel::msg::{AttachOwner, Attached, Msg};
use rusqlite::{OptionalExtension, params};
use serde_json::json;

use crate::pages::Lang;

use super::store::{Account, Store};
use super::{Accounts, random};

/// La migration 3 de la base (`super::store`) : Stations rattachées, appareils
/// connectés (`super::devices`), leurs codes et leurs premières venues.
pub const MIGRATION: &str = r#"
    CREATE TABLE stations (
        id TEXT PRIMARY KEY,
        name TEXT NOT NULL,
        owner_kind TEXT NOT NULL DEFAULT 'person',
        account INTEGER NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
        attached_by INTEGER REFERENCES accounts(id) ON DELETE SET NULL,
        attached_at INTEGER NOT NULL
    );
    CREATE INDEX stations_account ON stations(account);
    CREATE TABLE devices (
        id INTEGER PRIMARY KEY,
        public_id TEXT NOT NULL UNIQUE,
        hash BLOB NOT NULL UNIQUE,
        account INTEGER NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
        name TEXT NOT NULL,
        kind TEXT NOT NULL CHECK (kind IN ('app', 'tv')),
        created_at INTEGER NOT NULL,
        seen_at INTEGER NOT NULL,
        revoked_at INTEGER
    );
    CREATE INDEX devices_account ON devices(account);
    CREATE TABLE device_codes (
        hash BLOB PRIMARY KEY,
        kind TEXT NOT NULL CHECK (kind IN ('return', 'tv')),
        poll_hash BLOB UNIQUE,
        account INTEGER REFERENCES accounts(id) ON DELETE CASCADE,
        device_name TEXT NOT NULL,
        status TEXT NOT NULL CHECK (status IN ('pending', 'approved', 'denied')),
        created_at INTEGER NOT NULL,
        expires_at INTEGER NOT NULL
    );
    CREATE TABLE device_stations (
        device INTEGER NOT NULL REFERENCES devices(id) ON DELETE CASCADE,
        station TEXT NOT NULL,
        first_at INTEGER NOT NULL,
        PRIMARY KEY (device, station)
    );
"#;

/// Le fichier de la clé de signature, à côté de `secret_key`.
pub const SIGNING_KEY: &str = "signing.key";
/// La clé suivante, publiée pendant une rotation.
pub const NEXT_KEY: &str = "signing.next.key";
/// Un code de rattachement vaut 15 minutes.
pub const ATTACH_SECS: i64 = 15 * 60;
/// Codes de rattachement en attente, au plus (un par Station).
const MAX_PENDING: usize = 10_000;

/// Les Stations connectées au relais, vues des comptes : le hub du relais.
pub trait StationLink: Send + Sync {
    /// La Station est-elle connectée ?
    fn online(&self, station: &str) -> bool;
    /// Envoie un message à la Station, par sa connexion de contrôle ; faux si elle n'est pas là.
    fn send(&self, station: &str, msg: Msg) -> bool;
    /// L'empreinte d'Admin de la Station connectée, signée par elle (comme
    /// `GET /v1/codes/{code}`) : les apps l'épinglent pour l'ouvrir par le
    /// relais, sans premier contact (`GET /v1/computers`, lot 3).
    fn attest(&self, _station: &str) -> Option<Attest> {
        None
    }
    /// Les adresses d'Admin sur le réseau et « Admin à distance » de la Station connectée.
    fn admin(&self, _station: &str) -> Option<(Vec<String>, bool)> {
        None
    }
}

/// L'empreinte d'Admin d'une Station et de quoi la vérifier : sa clé publique
/// (SPKI en DER, base64), qui donne son identifiant, et sa signature.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Attest {
    pub station_key: String,
    pub admin_fp: String,
    pub sig: String,
}

/// La clé qui signe les jetons, et celles publiées.
pub struct SigningKeys {
    signer: Signer,
    published: KeySet,
}

impl SigningKeys {
    /// Une clé en mémoire seulement (essais, `kybernow-relay accounts add`).
    pub fn ephemeral() -> SigningKeys {
        let der = Signer::generate_pkcs8().expect("clé Ed25519");
        let signer = Signer::from_pkcs8(&der).expect("clé Ed25519");
        let published = KeySet { keys: vec![signer.public_key().clone()] };
        SigningKeys { signer, published }
    }

    /// Lit `signing.key` dans `dir` (ou la crée, réservée au relais), et `signing.next.key` s'il y en a une.
    pub fn load_or_create(dir: &Path) -> Result<SigningKeys> {
        let path = dir.join(SIGNING_KEY);
        if !path.exists() {
            std::fs::create_dir_all(dir).with_context(|| format!("dossier de la clé {}", dir.display()))?;
            let der = Signer::generate_pkcs8().map_err(|e| anyhow::anyhow!("clé de signature : {e}"))?;
            super::store::create_private(&path).with_context(|| format!("clé de signature {}", path.display()))?;
            std::fs::write(&path, &der).with_context(|| format!("clé de signature {}", path.display()))?;
            tracing::info!(fichier = %path.display(), "clé de signature des jetons créée");
        }
        let read = |p: &Path| -> Result<Signer> {
            let der = std::fs::read(p).with_context(|| format!("clé de signature {}", p.display()))?;
            Signer::from_pkcs8(&der).map_err(|_| anyhow::anyhow!("clé de signature {} illisible (Ed25519 en PKCS#8 attendu)", p.display()))
        };
        let signer = read(&path)?;
        let mut keys = vec![signer.public_key().clone()];
        let next = dir.join(NEXT_KEY);
        if next.exists() {
            let n = read(&next)?;
            if n.public_key() != signer.public_key() {
                tracing::info!(cle = %n.public_key().kid, "rotation : clé suivante publiée");
                keys.push(n.public_key().clone());
            }
        }
        tracing::info!(cle = %signer.public_key().kid, "clé de signature des jetons");
        Ok(SigningKeys { signer, published: KeySet { keys } })
    }

    pub fn signer(&self) -> &Signer {
        &self.signer
    }

    /// `GET /v1/keys`.
    pub fn key_set(&self) -> &KeySet {
        &self.published
    }
}

/// Un code de rattachement en attente.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingAttach {
    pub station: String,
    /// Le nom de l'ordinateur, donné par Admin.
    pub name: String,
    pub expires_at: i64,
}

/// L'état du rattachement : la clé, les codes en attente, le lien vers le hub.
pub struct Stations {
    pub keys: SigningKeys,
    pending: Mutex<HashMap<String, PendingAttach>>,
    link: RwLock<Option<Weak<dyn StationLink>>>,
}

impl Stations {
    pub fn new(keys: SigningKeys) -> Stations {
        Stations { keys, pending: Mutex::new(HashMap::new()), link: RwLock::new(None) }
    }

    /// Branche le hub (au démarrage du relais) ; les essais y mettent le leur.
    pub fn set_link(&self, link: Weak<dyn StationLink>) {
        *self.link.write().unwrap_or_else(|e| e.into_inner()) = Some(link);
    }

    pub fn link(&self) -> Option<Arc<dyn StationLink>> {
        self.link.read().unwrap_or_else(|e| e.into_inner()).as_ref().and_then(Weak::upgrade)
    }

    /// L'empreinte d'Admin attestée de la Station, si elle est connectée.
    pub fn attest(&self, station: &str) -> Option<Attest> {
        self.link().and_then(|l| l.attest(station))
    }

    pub fn online(&self, station: &str) -> bool {
        self.link().is_some_and(|l| l.online(station))
    }

    pub fn send(&self, station: &str, msg: Msg) -> bool {
        self.link().is_some_and(|l| l.send(station, msg))
    }

    /// Les adresses d'Admin sur le réseau et « Admin à distance » de la Station connectée.
    pub fn admin(&self, station: &str) -> Option<(Vec<String>, bool)> {
        self.link().and_then(|l| l.admin(station))
    }

    /// Un nouveau code pour cette Station (le précédent ne vaut plus).
    fn new_code(&self, station: &str, name: &str, now: i64) -> (String, i64) {
        let code = base32_lower(&random::<15>());
        let expires_at = now + ATTACH_SECS;
        let mut pending = self.pending.lock().unwrap_or_else(|e| e.into_inner());
        pending.retain(|_, p| p.station != station && p.expires_at > now);
        if pending.len() >= MAX_PENDING {
            // Les plus proches de leur échéance partent d'abord.
            let mut all: Vec<(String, i64)> = pending.iter().map(|(c, p)| (c.clone(), p.expires_at)).collect();
            all.sort_by_key(|(_, e)| *e);
            for (c, _) in all.into_iter().take(MAX_PENDING / 10) {
                pending.remove(&c);
            }
        }
        pending.insert(code.clone(), PendingAttach { station: station.into(), name: name.into(), expires_at });
        (code, expires_at)
    }

    /// Le code en attente, s'il vaut encore (sans le consommer).
    pub fn pending(&self, code: &str, now: i64) -> Option<PendingAttach> {
        self.pending.lock().unwrap_or_else(|e| e.into_inner()).get(code).filter(|p| p.expires_at > now).cloned()
    }

    /// Consomme le code.
    pub fn take(&self, code: &str, now: i64) -> Option<PendingAttach> {
        let mut pending = self.pending.lock().unwrap_or_else(|e| e.into_inner());
        let p = pending.remove(code)?;
        (p.expires_at > now).then_some(p)
    }

    /// Le ménage des codes échus.
    pub fn sweep(&self, now: i64) {
        self.pending.lock().unwrap_or_else(|e| e.into_inner()).retain(|_, p| p.expires_at > now);
    }
}

/// Une Station rattachée.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StationRow {
    pub id: String,
    pub name: String,
    /// `person` ou `organization` (lot 5).
    pub owner_kind: String,
    /// Le compte propriétaire (une personne).
    pub account: Option<i64>,
    /// L'organisation propriétaire (lot 5).
    pub org: Option<i64>,
    pub attached_at: i64,
}

pub(super) fn station_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<StationRow> {
    Ok(StationRow { id: r.get(0)?, name: r.get(1)?, owner_kind: r.get(2)?, account: r.get(3)?, org: r.get(4)?, attached_at: r.get(5)? })
}

const STATION_COLS: &str = "id, name, owner_kind, account, org, attached_at";
/// Les mêmes colonnes, préfixées (`stations s`).
pub(super) const STATION_COLS_S: &str = "s.id, s.name, s.owner_kind, s.account, s.org, s.attached_at";

impl Store {
    /// Rattache (ou rattache de nouveau, à un autre propriétaire) une Station à une personne.
    pub fn attach_station(&self, id: &str, name: &str, account: i64, by: i64, now: i64) -> Result<()> {
        self.attach_to(id, name, Some(account), None, by, None, now)
    }

    /// Rattache une Station à une organisation (lot 5) ; `enrolled_by` : le jeton d'enrôlement.
    pub fn attach_station_to_org(&self, id: &str, name: &str, org: i64, by: Option<i64>, enrolled_by: Option<i64>, now: i64) -> Result<()> {
        self.attach_to(id, name, None, Some(org), by.unwrap_or(0), enrolled_by, now)
    }

    #[allow(clippy::too_many_arguments)]
    fn attach_to(&self, id: &str, name: &str, account: Option<i64>, org: Option<i64>, by: i64, enrolled_by: Option<i64>, now: i64) -> Result<()> {
        let kind = if org.is_some() { "organization" } else { "person" };
        let by = (by > 0).then_some(by);
        // Tout ou rien, et possible dans un enrôlement déjà ouvert (REL-26).
        super::store::atomic(&self.conn, || self.attach_now(id, name, kind, account, org, by, enrolled_by, now))
    }

    #[allow(clippy::too_many_arguments)]
    fn attach_now(&self, id: &str, name: &str, kind: &str, account: Option<i64>, org: Option<i64>, by: Option<i64>, enrolled_by: Option<i64>, now: i64) -> Result<()> {
        let tx = &self.conn;
        // Un autre propriétaire : les droits donnés et les premières venues repartent de zéro.
        let previous: Option<(Option<i64>, Option<i64>)> =
            tx.query_row("SELECT account, org FROM stations WHERE id = ?1", [id], |r| Ok((r.get(0)?, r.get(1)?))).optional()?;
        if previous.is_some_and(|p| p != (account, org)) {
            tx.execute("DELETE FROM grants WHERE station = ?1", [id])?;
            tx.execute("DELETE FROM device_stations WHERE station = ?1", [id])?;
            // Ses liens d'accès, et ses groupes d'ordinateurs : elle quitte la famille ou l'équipe.
            tx.execute("DELETE FROM access_links WHERE station = ?1", [id])?;
            tx.execute("DELETE FROM station_group_members WHERE station = ?1", [id])?;
            // Migration 9 : les réglages synchronisés de cet ordinateur.
            tx.execute("DELETE FROM computer_settings WHERE station = ?1", [id])?;
        }
        tx.execute(
            "INSERT INTO stations (id, name, owner_kind, account, org, attached_by, attached_at, enrolled_by) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
             ON CONFLICT(id) DO UPDATE SET name = excluded.name, owner_kind = excluded.owner_kind, account = excluded.account, org = excluded.org,
             attached_by = excluded.attached_by, attached_at = excluded.attached_at, enrolled_by = excluded.enrolled_by",
            params![id, name, kind, account, org, by, now, enrolled_by],
        )?;
        Ok(())
    }

    pub fn station(&self, id: &str) -> Result<Option<StationRow>> {
        Ok(self.conn.query_row(&format!("SELECT {STATION_COLS} FROM stations WHERE id = ?1"), [id], station_row).optional()?)
    }

    /// Les Stations d'un compte (à lui), la plus récemment rattachée d'abord.
    pub fn stations_of(&self, account: i64) -> Result<Vec<StationRow>> {
        let mut stmt = self.conn.prepare(&format!("SELECT {STATION_COLS} FROM stations WHERE account = ?1 ORDER BY attached_at DESC, id"))?;
        let rows = stmt.query_map([account], station_row)?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    /// Les Stations d'une organisation (lot 5), par nom.
    pub fn stations_of_org(&self, org: i64) -> Result<Vec<StationRow>> {
        let mut stmt = self.conn.prepare(&format!("SELECT {STATION_COLS} FROM stations WHERE org = ?1 ORDER BY name COLLATE NOCASE, id"))?;
        let rows = stmt.query_map([org], station_row)?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    /// Détache ; `account` : seulement si elle est à ce compte. Faux si rien n'a changé.
    /// Tout ou rien (REL-25) : sans quoi des droits orphelins rouvriraient la machine à un
    /// prochain propriétaire.
    pub fn detach_station(&self, id: &str, account: Option<i64>) -> Result<bool> {
        super::store::atomic(&self.conn, || self.detach_station_now(id, account))
    }

    fn detach_station_now(&self, id: &str, account: Option<i64>) -> Result<bool> {
        let n = match account {
            Some(a) => self.conn.execute("DELETE FROM stations WHERE id = ?1 AND account = ?2", params![id, a])?,
            None => self.conn.execute("DELETE FROM stations WHERE id = ?1", [id])?,
        };
        if n > 0 {
            // Les premières venues des appareils et les droits donnés : un prochain rattachement repart de zéro.
            self.conn.execute("DELETE FROM device_stations WHERE station = ?1", [id])?;
            self.conn.execute("DELETE FROM grants WHERE station = ?1", [id])?;
            // Migration 7 : ses liens d'accès et ses groupes d'ordinateurs.
            self.conn.execute("DELETE FROM access_links WHERE station = ?1", [id])?;
            self.conn.execute("DELETE FROM station_group_members WHERE station = ?1", [id])?;
            // Migration 9 : les réglages synchronisés.
            self.conn.execute("DELETE FROM computer_settings WHERE station = ?1", [id])?;
        }
        Ok(n > 0)
    }
}

/// Un nom d'ordinateur lisible : 64 caractères au plus, sans caractère de contrôle.
pub fn clean_name(name: &str) -> String {
    clean_text(name, 64)
}

/// Un texte d'une ligne, `max` caractères au plus : sans caractère de contrôle ni de mise en
/// forme bidirectionnelle (qui retourneraient l'affichage d'un nom), espaces resserrés (REL-32).
pub fn clean_text(text: &str, max: usize) -> String {
    let bidi = |c: char| matches!(c, '\u{061C}' | '\u{200E}' | '\u{200F}' | '\u{202A}'..='\u{202E}' | '\u{2066}'..='\u{2069}');
    let spaced: String = text.chars().filter(|c| !bidi(*c)).map(|c| if c.is_control() { ' ' } else { c }).collect();
    spaced.split_whitespace().collect::<Vec<_>>().join(" ").chars().take(max).collect::<String>().trim_end().to_string()
}

impl Accounts {
    /// Le nom de ce relais dans les jetons (`relay`) et le rattachement :
    /// l'hôte de `public_url`, avec son port hors du 443 (`share.kybernow.app`).
    pub fn relay_name(&self) -> String {
        let host = self.public_url.trim_start_matches("https://").trim_end_matches('/');
        host.strip_suffix(":443").unwrap_or(host).to_string()
    }

    /// L'adresse d'Admin de cette Station par le relais (`https://<id>.share.kybernow.app`).
    pub fn station_origin(&self, station: &str) -> String {
        format!("https://{station}.{}", self.relay_name())
    }

    /// Admin vient d'être ouvert à distance (« Admin depuis KyberNow Share ») : une ligne au journal
    /// de la famille ou de l'équipe, un e-mail aux propriétaires (au plus un par heure et par personne).
    pub fn admin_opened(&self, station: &str, account: &str, name: &str) {
        let now = self.now();
        let Some(row) = self.db().station(station).ok().flatten() else { return };
        tracing::info!(station = %row.id, "Admin ouvert à distance");
        let recipients: Vec<Account> = {
            let db = self.db();
            match (row.account, row.org) {
                (Some(owner), _) => db.account(owner).ok().flatten().into_iter().collect(),
                (None, Some(org)) => {
                    let detail = json!({ "station": row.name, "who": name });
                    if let Err(e) = db.org_log(org, None, "admin_distance", detail, None, now) {
                        tracing::warn!("journal de l'organisation : {e:#}");
                    }
                    db.org_managers(org).unwrap_or_default()
                }
                (None, None) => Vec::new(),
            }
        };
        let Some(mailer) = &self.mailer else { return };
        for owner in recipients {
            // Une fois par heure, par la limite commune des comptes (REL-40).
            let key = format!("{station}:{}:{account}", owner.id);
            if !self.limits.lock().unwrap_or_else(|e| e.into_inner()).admin_notice(&key) {
                continue;
            }
            let lang = if owner.lang == "en" { Lang::En } else { Lang::Fr };
            let date = super::pages::date(lang, now);
            mailer.send(super::mail::compose(&self.public_url, &owner.email, lang, super::mail::Message::AdminOpened { by: name, computer: &row.name, date: &date }));
        }
    }

    /// L'identifiant public d'un compte (dans les jetons et chez la Station) :
    /// `acc_` et 16 signes tirés de la clé du relais, jamais le numéro de la base.
    pub fn public_account_id(&self, account: i64) -> String {
        let sig = self.keys.sign(format!("compte-public:{account}").as_bytes());
        format!("acc_{}", &sig[..16])
    }

    /// `attach_request` d'une Station : un code et la page à ouvrir.
    pub fn attach_request(&self, station: &str, name: &str) -> (String, String, u64) {
        let now = self.now();
        let name = clean_name(name);
        let (code, expires_at) = self.stations.new_code(station, &name, now);
        tracing::info!(station = %station, "rattachement demandé par la Station");
        let url = format!("{}/compte/rattacher?code={code}", self.public_url);
        (code, url, (expires_at - now) as u64)
    }

    /// Le message `attached` : `account` a rattaché la Station, qui est à `owner`.
    pub fn attached_msg(&self, account: &Account, owner: &Owner, req: Option<u32>) -> Msg {
        let id = self.public_account_id(account.id);
        let owner = match owner {
            Owner::Person => AttachOwner { kind: "person".into(), id: id.clone(), name: account.name.clone(), email: Some(account.email.clone()) },
            Owner::Org { public_id, name, .. } => AttachOwner { kind: "organization".into(), id: public_id.clone(), name: name.clone(), email: None },
        };
        Msg::Attached(Attached { req, relay: self.relay_name(), account: id, owner, keys: self.stations.keys.key_set().keys.clone() })
    }

    /// Le message `attached` d'une Station rattachée, d'après la base ; `None` : plus rattachée.
    pub fn attached_msg_of(&self, row: &StationRow, req: Option<u32>) -> Option<Msg> {
        let db = self.db();
        match (row.account, row.org) {
            (Some(a), _) => {
                let account = db.account(a).ok().flatten()?;
                drop(db);
                Some(self.attached_msg(&account, &Owner::Person, req))
            }
            (None, Some(o)) => {
                let org = db.org(o).ok().flatten()?;
                // `account` : qui a rattaché, ou l'organisation elle-même (enrôlée par un jeton, ou compte effacé depuis).
                let by: Option<Account> = db
                    .conn
                    .query_row("SELECT attached_by FROM stations WHERE id = ?1", [&row.id], |r| r.get::<_, Option<i64>>(0))
                    .ok()
                    .flatten()
                    .and_then(|a| db.account(a).ok().flatten());
                drop(db);
                let owner = Owner::Org { id: org.id, public_id: org.public_id.clone(), name: org.name.clone() };
                Some(match by {
                    Some(account) => self.attached_msg(&account, &owner, req),
                    None => Msg::Attached(Attached {
                        req,
                        relay: self.relay_name(),
                        account: org.public_id.clone(),
                        owner: AttachOwner { kind: "organization".into(), id: org.public_id, name: org.name, email: None },
                        keys: self.stations.keys.key_set().keys.clone(),
                    }),
                })
            }
            (None, None) => None,
        }
    }

    /// `attach_status` d'une Station : `attached` (clés à jour) ou `detached`.
    pub fn attach_status(&self, station: &str, req: u32) -> Msg {
        let row = self.db().station(station).ok().flatten();
        row.and_then(|r| self.attached_msg_of(&r, Some(req))).unwrap_or(Msg::Detached { req: Some(req) })
    }

    /// `detach` d'une Station : détachée dans Admin.
    pub fn station_detached(&self, station: &str) {
        let db = self.db();
        let row = db.station(station).ok().flatten();
        match db.detach_station(station, None) {
            Ok(true) => {
                let now = self.now();
                if let Some(row) = &row {
                    let _ = db.audit(row.account, "station_detachee_par_admin", None, now);
                    if let Some(org) = row.org {
                        let _ = db.org_log(org, None, "station_detachee_par_admin", serde_json::json!({ "station": row.name }), None, now);
                    }
                }
                tracing::info!(station = %station, "Station détachée dans Admin");
            }
            Ok(false) => {}
            Err(e) => tracing::warn!("détachement : {e:#}"),
        }
    }

    /// Confirme un rattachement dans la console ; faux si la Station n'a pas reçu `attached`.
    pub fn confirm_attach(&self, pending: &PendingAttach, account: &Account, owner: &Owner) -> Result<bool> {
        let now = self.now();
        {
            let db = self.db();
            match owner {
                Owner::Person => db.attach_station(&pending.station, &pending.name, account.id, account.id, now)?,
                Owner::Org { id, .. } => {
                    db.attach_station_to_org(&pending.station, &pending.name, *id, Some(account.id), None, now)?;
                    db.org_log(*id, Some(account), "station_rattachee", serde_json::json!({ "station": pending.name }), None, now)?;
                }
            }
        }
        Ok(self.stations.send(&pending.station, self.attached_msg(account, owner, None)))
    }

    /// Détache depuis la console (le propriétaire, ou un Propriétaire de
    /// l'organisation : REL-12) : la Station le sait si elle est connectée.
    pub fn detach_by_account(&self, station: &str, account: &Account) -> Result<bool> {
        let row = self.db().station(station)?;
        let Some(row) = row else { return Ok(false) };
        let org_owner = self.org_manager(account, row.org) && row.org.is_some_and(|o| self.db().org_role(o, account.id).ok().flatten() == Some(super::orgs::Role::Owner));
        if row.account != Some(account.id) && !org_owner {
            return Ok(false);
        }
        let done = self.db().detach_station(station, None)?;
        if done {
            if let Some(org) = row.org {
                let _ = self.db().org_log(org, Some(account), "station_detachee", serde_json::json!({ "station": row.name }), None, self.now());
            }
            self.stations.send(station, Msg::Detached { req: None });
        }
        Ok(done)
    }
}

/// À qui rattacher une Station (« Propriétaire » de la page « Rattacher »).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Owner {
    /// Le compte qui rattache.
    Person,
    /// Une organisation qu'il administre (lot 5).
    Org { id: i64, public_id: String, name: String },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn noms_sans_controle_ni_bidi() {
        // REL-32 : ni caractère de contrôle ni retournement bidi ; espaces resserrés.
        assert_eq!(clean_text("  Ana\u{202E}lirva \n  Martin\u{2066} ", 80), "Analirva Martin");
        assert_eq!(clean_text("Léa\u{200F}", 80), "Léa");
        assert_eq!(clean_text(&"x".repeat(100), 80).len(), 80);
    }

    #[test]
    fn cle_de_signature_creee_puis_relue_et_rotation() {
        let dir = std::env::temp_dir().join(format!("kn-signature-{}", kybernow_tunnel::id::random_id()));
        let a = SigningKeys::load_or_create(&dir).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(std::fs::metadata(dir.join(SIGNING_KEY)).unwrap().permissions().mode() & 0o777, 0o600, "clé réservée au relais");
        }
        let b = SigningKeys::load_or_create(&dir).unwrap();
        assert_eq!(a.key_set(), b.key_set(), "relue, pas recréée");
        assert_eq!(a.key_set().keys.len(), 1);
        // Rotation : la clé suivante est publiée, la clé en cours signe toujours.
        std::fs::write(dir.join(NEXT_KEY), Signer::generate_pkcs8().unwrap()).unwrap();
        let c = SigningKeys::load_or_create(&dir).unwrap();
        assert_eq!(c.key_set().keys.len(), 2);
        assert_eq!(&c.key_set().keys[0], a.signer().public_key());
        assert!(c.key_set().valid().len() == 2);
        std::fs::write(dir.join(SIGNING_KEY), b"pas une cle").unwrap();
        assert!(SigningKeys::load_or_create(&dir).is_err());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn codes_de_rattachement() {
        let s = Stations::new(SigningKeys::ephemeral());
        let (c1, _) = s.new_code("st1", "PC", 100);
        let (c2, exp) = s.new_code("st1", "PC", 110);
        assert_eq!(exp, 110 + ATTACH_SECS);
        assert!(s.pending(&c1, 120).is_none(), "le nouveau code remplace l'ancien");
        assert_eq!(s.pending(&c2, 120).unwrap().station, "st1");
        assert!(s.pending(&c2, exp).is_none(), "échu");
        assert!(s.take(&c2, 120).is_some());
        assert!(s.take(&c2, 120).is_none(), "une seule fois");
        assert_eq!(clean_name("  PC\u{7}du  salon  "), "PC du salon");
    }

    #[test]
    fn stations_d_un_compte() {
        let s = Store::memory(0).unwrap();
        let a = s.create_account("a@exemple.org", "A", "h", "fr", Some(0), 0).unwrap().unwrap();
        let b = s.create_account("b@exemple.org", "B", "h", "fr", Some(0), 0).unwrap().unwrap();
        s.attach_station("st1", "PC", a, a, 10).unwrap();
        s.attach_station("st2", "Mac", a, a, 20).unwrap();
        assert_eq!(s.stations_of(a).unwrap().iter().map(|r| r.id.as_str()).collect::<Vec<_>>(), ["st2", "st1"]);
        // Rattachée de nouveau par un autre compte : elle change de propriétaire.
        s.attach_station("st1", "PC du salon", b, b, 30).unwrap();
        assert_eq!(s.station("st1").unwrap().unwrap().account, Some(b));
        assert!(!s.detach_station("st1", Some(a)).unwrap(), "pas à ce compte");
        assert!(s.detach_station("st1", Some(b)).unwrap());
        assert!(s.station("st1").unwrap().is_none());
        assert!(s.detach_station("st2", None).unwrap());
    }
}
