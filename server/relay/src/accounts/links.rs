//! Les liens d'accès (docs/comptes.md, « Regarder, liens d'accès et groupes
//! d'ordinateurs ») : ouvrir un ordinateur à quelqu'un dont on n'a pas
//! l'adresse, par un lien à **usage unique**.
//!
//! - Le lien (`https://{relais}/compte/lien/kla_…`, 32 signes après `kla_`,
//!   160 bits) est montré une seule fois ; le relais n'en garde que
//!   l'empreinte et le début (pour le reconnaître dans la liste).
//! - Il sert pendant 1, 7 ou 30 jours ; il porte des droits et, s'il le faut,
//!   la fin de l'accès donné. Dix liens en attente au plus par ordinateur.
//! - L'accepter donne ces droits au compte, ajoutés à ceux qu'il a déjà sur
//!   l'ordinateur (l'échéance la plus lointaine l'emporte, pas d'échéance
//!   l'emporte sur toutes), et le marque **utilisé** dans la même transaction :
//!   deux personnes qui l'ouvrent en même temps, une seule l'obtient
//!   ([`Store::use_access_link`]).
//!
//! La migration 7 (`MIGRATION`) porte aussi les groupes d'ordinateurs
//! ([`super::station_groups`]).

use anyhow::Result;
use kybernow_tunnel::access::Rights;
use kybernow_tunnel::id::base32_lower;
use rusqlite::{Connection, OptionalExtension, params};

use super::people::{parse_rights, rights_text};
use super::random;
use super::store::Store;

/// La migration 7 de la base (`super::store`) : les liens d'accès, les groupes
/// d'ordinateurs, leurs droits donnés, et le groupe où un jeton d'enrôlement
/// range ses Stations.
///
/// Pas de clé étrangère vers `stations` (la migration 5 a dû reconstruire cette
/// table) : les lignes d'une Station partent avec son détachement ou son
/// rattachement à un autre propriétaire ([`Store::detach_station`]).
pub const MIGRATION: &str = r#"
    CREATE TABLE access_links (
        id INTEGER PRIMARY KEY,
        station TEXT NOT NULL,
        hash BLOB NOT NULL UNIQUE,
        hint TEXT NOT NULL,
        rights TEXT NOT NULL,
        link_expires_at INTEGER NOT NULL,
        access_expires_at INTEGER,
        created_by INTEGER REFERENCES accounts(id) ON DELETE SET NULL,
        created_at INTEGER NOT NULL,
        used_by INTEGER REFERENCES accounts(id) ON DELETE SET NULL,
        used_at INTEGER,
        revoked_at INTEGER
    );
    CREATE INDEX access_links_station ON access_links(station, created_at);
    CREATE TABLE station_groups (
        id INTEGER PRIMARY KEY,
        org INTEGER NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
        name TEXT NOT NULL,
        created_at INTEGER NOT NULL,
        UNIQUE (org, name)
    );
    CREATE TABLE station_group_members (
        sgroup INTEGER NOT NULL REFERENCES station_groups(id) ON DELETE CASCADE,
        station TEXT NOT NULL,
        PRIMARY KEY (sgroup, station)
    );
    CREATE INDEX station_group_members_station ON station_group_members(station);
    CREATE TABLE group_grants (
        id INTEGER PRIMARY KEY,
        sgroup INTEGER NOT NULL REFERENCES station_groups(id) ON DELETE CASCADE,
        account INTEGER REFERENCES accounts(id) ON DELETE CASCADE,
        org INTEGER REFERENCES organizations(id) ON DELETE CASCADE,
        grp INTEGER REFERENCES org_groups(id) ON DELETE CASCADE,
        rights TEXT NOT NULL,
        expires_at INTEGER,
        invited_by INTEGER REFERENCES accounts(id) ON DELETE SET NULL,
        created_at INTEGER NOT NULL
    );
    CREATE UNIQUE INDEX group_grants_person ON group_grants(sgroup, account) WHERE account IS NOT NULL;
    CREATE UNIQUE INDEX group_grants_org ON group_grants(sgroup, org) WHERE org IS NOT NULL;
    CREATE UNIQUE INDEX group_grants_grp ON group_grants(sgroup, grp) WHERE grp IS NOT NULL;
    CREATE INDEX group_grants_account ON group_grants(account);
    ALTER TABLE enroll_tokens ADD COLUMN sgroup INTEGER REFERENCES station_groups(id) ON DELETE SET NULL;
"#;

/// Le préfixe d'un lien d'accès.
pub const LINK_PREFIX: &str = "kla_";
/// Les durées proposées (jours) ; 7 par défaut.
pub const LINK_DAYS: [i64; 3] = [1, 7, 30];
/// Liens en attente au plus, par ordinateur.
pub const MAX_PENDING: i64 = 10;
/// Un lien fermé (utilisé, révoqué, expiré) est oublié après ce temps.
pub const LINK_KEEP_SECS: i64 = 30 * 86_400;

/// Un nouveau lien : `kla_` et 32 signes (160 bits).
pub fn new_link_token() -> String {
    format!("{LINK_PREFIX}{}", base32_lower(&random::<20>()))
}

/// A-t-il la forme d'un lien (`kla_` et 32 signes base32 minuscules) ?
pub fn link_token_ok(token: &str) -> bool {
    token.len() == LINK_PREFIX.len() + 32
        && token.starts_with(LINK_PREFIX)
        && token[LINK_PREFIX.len()..].bytes().all(|b| matches!(b, b'a'..=b'z' | b'2'..=b'7'))
}

/// Où en est un lien.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinkState {
    Pending,
    Used,
    Revoked,
    Expired,
}

/// Un lien d'accès (jamais le lien lui-même : son empreinte).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccessLink {
    pub id: i64,
    pub station: String,
    /// Le début du lien (`kla_7qm2x`), pour le reconnaître.
    pub hint: String,
    pub rights: Rights,
    /// Le lien ne sert plus après.
    pub link_expires_at: i64,
    /// La fin de l'accès donné (`None` : pas de fin).
    pub access_expires_at: Option<i64>,
    pub created_by: Option<i64>,
    /// Le nom de qui l'a créé (vide : compte effacé).
    pub created_by_name: String,
    pub created_at: i64,
    pub used_by: Option<i64>,
    /// Le nom de qui l'a utilisé (vide : compte effacé).
    pub used_by_name: String,
    pub used_at: Option<i64>,
    pub revoked_at: Option<i64>,
}

impl AccessLink {
    /// Utilisé d'abord (il a servi, même révoqué ensuite), puis révoqué, puis expiré.
    pub fn state(&self, now: i64) -> LinkState {
        if self.used_at.is_some() {
            LinkState::Used
        } else if self.revoked_at.is_some() {
            LinkState::Revoked
        } else if self.link_expires_at <= now {
            LinkState::Expired
        } else {
            LinkState::Pending
        }
    }
}

const LINK_COLS: &str = "l.id, l.station, l.hint, l.rights, l.link_expires_at, l.access_expires_at, l.created_by, c.name, l.created_at, l.used_by, u.name, l.used_at, l.revoked_at
     FROM access_links l LEFT JOIN accounts c ON c.id = l.created_by LEFT JOIN accounts u ON u.id = l.used_by";

fn link_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<AccessLink> {
    Ok(AccessLink {
        id: r.get(0)?,
        station: r.get(1)?,
        hint: r.get(2)?,
        rights: parse_rights(&r.get::<_, String>(3)?),
        link_expires_at: r.get(4)?,
        access_expires_at: r.get(5)?,
        created_by: r.get(6)?,
        created_by_name: r.get::<_, Option<String>>(7)?.unwrap_or_default(),
        created_at: r.get(8)?,
        used_by: r.get(9)?,
        used_by_name: r.get::<_, Option<String>>(10)?.unwrap_or_default(),
        used_at: r.get(11)?,
        revoked_at: r.get(12)?,
    })
}

/// Ajoute des droits à ceux qu'un compte a déjà sur une Station : l'union des
/// droits, et l'échéance la plus lointaine (pas d'échéance l'emporte). Un droit
/// déjà échu est remplacé.
pub(super) fn add_person_rights(conn: &Connection, station: &str, account: i64, rights: &Rights, expires_at: Option<i64>, by: Option<i64>, now: i64) -> Result<()> {
    let found: Option<(i64, String, Option<i64>)> = conn
        .query_row("SELECT id, rights, expires_at FROM grants WHERE station = ?1 AND account = ?2", params![station, account], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
        .optional()?;
    match found {
        Some((id, old, old_end)) if old_end.is_none_or(|e| e > now) => {
            let all = Rights::new(parse_rights(&old).iter().chain(rights.iter()));
            let end = match (old_end, expires_at) {
                (Some(a), Some(b)) => Some(a.max(b)),
                _ => None,
            };
            conn.execute("UPDATE grants SET rights = ?2, expires_at = ?3 WHERE id = ?1", params![id, rights_text(&all), end])?;
        }
        Some((id, _, _)) => {
            conn.execute("UPDATE grants SET rights = ?2, expires_at = ?3 WHERE id = ?1", params![id, rights_text(rights), expires_at])?;
        }
        None => {
            conn.execute(
                "INSERT INTO grants (station, account, rights, expires_at, invited_by, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![station, account, rights_text(rights), expires_at, by, now],
            )?;
        }
    }
    Ok(())
}

impl Store {
    #[allow(clippy::too_many_arguments)]
    pub fn create_access_link(
        &self,
        station: &str,
        hash: &[u8],
        hint: &str,
        rights: &Rights,
        link_expires_at: i64,
        access_expires_at: Option<i64>,
        by: i64,
        now: i64,
    ) -> Result<i64> {
        self.conn.execute(
            "INSERT INTO access_links (station, hash, hint, rights, link_expires_at, access_expires_at, created_by, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![station, hash, hint, rights_text(rights), link_expires_at, access_expires_at, by, now],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    /// Les liens en attente sur cette Station (ni utilisés, ni révoqués, ni expirés).
    pub fn pending_links(&self, station: &str, now: i64) -> Result<i64> {
        Ok(self.conn.query_row(
            "SELECT COUNT(*) FROM access_links WHERE station = ?1 AND used_at IS NULL AND revoked_at IS NULL AND link_expires_at > ?2",
            params![station, now],
            |r| r.get(0),
        )?)
    }

    /// Les liens d'une Station, le plus récent d'abord.
    pub fn access_links(&self, station: &str, limit: usize) -> Result<Vec<AccessLink>> {
        let mut stmt = self.conn.prepare(&format!("SELECT {LINK_COLS} WHERE l.station = ?1 ORDER BY l.created_at DESC, l.id DESC LIMIT ?2"))?;
        let rows = stmt.query_map(params![station, limit as i64], link_row)?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    pub fn access_link_by_hash(&self, hash: &[u8]) -> Result<Option<AccessLink>> {
        Ok(self.conn.query_row(&format!("SELECT {LINK_COLS} WHERE l.hash = ?1"), [hash], link_row).optional()?)
    }

    /// Révoque un lien de cette Station ; le lien, s'il ne l'était pas déjà.
    pub fn revoke_access_link(&self, station: &str, id: i64, now: i64) -> Result<Option<AccessLink>> {
        let n = self.conn.execute("UPDATE access_links SET revoked_at = ?3 WHERE id = ?1 AND station = ?2 AND revoked_at IS NULL", params![id, station, now])?;
        if n == 0 {
            return Ok(None);
        }
        Ok(self.conn.query_row(&format!("SELECT {LINK_COLS} WHERE l.id = ?1"), [id], link_row).optional()?)
    }

    /// Utilise un lien : ses droits passent au compte, et il ne sert plus.
    /// Faux s'il ne sert plus (déjà utilisé, révoqué, expiré) : rien n'a changé.
    pub fn use_access_link(&self, id: i64, account: i64, now: i64) -> Result<bool> {
        let tx = self.conn.unchecked_transaction()?;
        // Marqué d'abord : deux acceptations en même temps, une seule passe.
        let n = tx.execute(
            "UPDATE access_links SET used_by = ?2, used_at = ?3 WHERE id = ?1 AND used_at IS NULL AND revoked_at IS NULL AND link_expires_at > ?3",
            params![id, account, now],
        )?;
        if n != 1 {
            return Ok(false);
        }
        let (station, rights, end, by): (String, String, Option<i64>, Option<i64>) =
            tx.query_row("SELECT station, rights, access_expires_at, created_by FROM access_links WHERE id = ?1", [id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))?;
        add_person_rights(&tx, &station, account, &parse_rights(&rights), end, by, now)?;
        tx.commit()?;
        Ok(true)
    }

    /// Le ménage : les liens fermés depuis [`LINK_KEEP_SECS`].
    pub fn sweep_links(&self, now: i64) -> Result<()> {
        self.conn.execute("DELETE FROM access_links WHERE COALESCE(used_at, revoked_at, link_expires_at) <= ?1", [now - LINK_KEEP_SECS])?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::accounts::token_hash;
    use kybernow_tunnel::access::Right;

    fn rights(words: &[Right]) -> Rights {
        Rights::new(words.iter().copied())
    }

    #[test]
    fn forme_des_liens() {
        let t = new_link_token();
        assert!(link_token_ok(&t), "{t}");
        assert!(!link_token_ok("kla_court"));
        assert!(!link_token_ok(&t.to_uppercase()));
        assert!(!link_token_ok(&t.replacen("kla_", "kne_", 1)));
    }

    #[test]
    fn un_lien_sert_une_fois_et_ajoute_ses_droits() {
        let s = Store::memory(0).unwrap();
        let a = s.create_account("a@exemple.org", "A", "h", "fr", Some(0), 0).unwrap().unwrap();
        let b = s.create_account("b@exemple.org", "B", "h", "fr", Some(0), 0).unwrap().unwrap();
        let c = s.create_account("c@exemple.org", "C", "h", "fr", Some(0), 0).unwrap().unwrap();
        s.attach_station("st1", "PC", a, a, 10).unwrap();
        // B a déjà Session jusqu'à 500 ; le lien donne Prêt jusqu'à 300 : l'union, et 500 reste.
        s.grant_person("st1", b, &rights(&[Right::Session]), Some(500), a, 10).unwrap();
        let tok = new_link_token();
        let id = s.create_access_link("st1", &token_hash(&tok), &tok[..9], &rights(&[Right::Lend]), 200, Some(300), a, 20).unwrap();
        assert_eq!(s.pending_links("st1", 30).unwrap(), 1);
        let link = s.access_link_by_hash(&token_hash(&tok)).unwrap().unwrap();
        assert_eq!((link.id, link.state(30), link.created_by_name.as_str()), (id, LinkState::Pending, "A"));
        assert_eq!(link.state(200), LinkState::Expired);
        assert!(s.use_access_link(id, b, 40).unwrap());
        assert!(!s.use_access_link(id, c, 41).unwrap(), "une seule fois");
        assert_eq!(s.granted_rights(b, "st1", 450).unwrap(), rights(&[Right::Session, Right::Lend]));
        assert!(s.granted_rights(c, "st1", 50).unwrap().is_empty());
        let link = s.access_link_by_hash(&token_hash(&tok)).unwrap().unwrap();
        assert_eq!((link.state(50), link.used_by_name.as_str()), (LinkState::Used, "B"));
        assert_eq!(s.pending_links("st1", 50).unwrap(), 0);
        // Révoquer un lien utilisé ne retire pas l'accès ; il reste « utilisé ».
        assert!(s.revoke_access_link("st1", id, 60).unwrap().is_some());
        assert_eq!(s.access_links("st1", 10).unwrap()[0].state(60), LinkState::Used);
        assert_eq!(s.granted_rights(b, "st1", 60).unwrap(), rights(&[Right::Session, Right::Lend]));
        // Un lien sans échéance d'accès : plus d'échéance du tout.
        let tok2 = new_link_token();
        let id2 = s.create_access_link("st1", &token_hash(&tok2), &tok2[..9], &rights(&[Right::Share]), 1000, None, a, 70).unwrap();
        assert!(s.use_access_link(id2, b, 80).unwrap());
        assert_eq!(s.granted_rights(b, "st1", 10_000).unwrap(), rights(&[Right::Session, Right::Lend, Right::Share]));
        // Révoqué, expiré : il ne sert pas.
        let tok3 = new_link_token();
        let id3 = s.create_access_link("st1", &token_hash(&tok3), &tok3[..9], &rights(&[Right::Admin]), 1000, None, a, 90).unwrap();
        assert!(s.revoke_access_link("st1", id3, 95).unwrap().is_some());
        assert!(s.revoke_access_link("st1", id3, 96).unwrap().is_none(), "déjà révoqué");
        assert!(!s.use_access_link(id3, c, 100).unwrap());
        let tok4 = new_link_token();
        let id4 = s.create_access_link("st1", &token_hash(&tok4), &tok4[..9], &rights(&[Right::View]), 1000, None, a, 90).unwrap();
        assert!(!s.use_access_link(id4, c, 1000).unwrap(), "expiré");
        assert!(s.granted_rights(c, "st1", 100).unwrap().is_empty());
        // Le ménage, puis le détachement : les liens partent.
        s.sweep_links(1000 + LINK_KEEP_SECS).unwrap();
        assert!(s.access_links("st1", 10).unwrap().is_empty());
        let tok5 = new_link_token();
        s.create_access_link("st1", &token_hash(&tok5), &tok5[..9], &rights(&[Right::View]), 1000, None, a, 90).unwrap();
        assert!(s.detach_station("st1", None).unwrap());
        assert!(s.access_link_by_hash(&token_hash(&tok5)).unwrap().is_none());
    }

    #[test]
    fn un_droit_echu_est_remplace() {
        let s = Store::memory(0).unwrap();
        let a = s.create_account("a@exemple.org", "A", "h", "fr", Some(0), 0).unwrap().unwrap();
        let b = s.create_account("b@exemple.org", "B", "h", "fr", Some(0), 0).unwrap().unwrap();
        s.attach_station("st1", "PC", a, a, 10).unwrap();
        s.grant_person("st1", b, &rights(&[Right::Admin]), Some(50), a, 10).unwrap();
        add_person_rights(&s.conn, "st1", b, &rights(&[Right::View]), Some(200), Some(a), 100).unwrap();
        assert_eq!(s.granted_rights(b, "st1", 150).unwrap(), rights(&[Right::View]), "Administrer, échu, ne revient pas");
        add_person_rights(&s.conn, "st1", b, &rights(&[Right::Session]), Some(150), Some(a), 110).unwrap();
        assert_eq!(s.granted_rights(b, "st1", 180).unwrap(), rights(&[Right::View, Right::Session]), "200 l'emporte sur 150");
    }
}
