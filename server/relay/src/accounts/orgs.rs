//! Les organisations (docs/comptes.md, « Flotte (organisations) », lot 5).
//!
//! Une **organisation** a des membres avec un rôle (Propriétaire,
//! Administrateur, Membre), des **groupes**, des **ordinateurs** rattachés à
//! elle plutôt qu'à une personne, un **journal** (qui a ouvert quoi, quand,
//! d'où, et chaque changement de droits) et des **jetons d'enrôlement** pour
//! déployer une flotte de Stations sans écran.
//!
//! - Les Propriétaires et Administrateurs gèrent l'organisation et ont tous
//!   les droits sur ses ordinateurs ; **avec la double authentification
//!   seulement** (TOTP ou passkey) : sans elle, la console le leur demande et
//!   ils n'ont que les droits donnés comme à un membre.
//! - Les droits se donnent à une personne, à toute l'organisation, ou à un
//!   groupe (table `grants`, [`super::people`]) ; un ordinateur d'une personne
//!   peut aussi être ouvert à une organisation ou à l'un de ses groupes.
//! - Inviter un membre : avec un compte vérifié, il est membre tout de suite ;
//!   sans compte, l'invitation attend l'adresse ([`claim_org_invites`]).
//! - Jeton d'enrôlement : `kne_…`, montré une seule fois, gardé en empreinte ;
//!   une durée, un nombre de Stations, et des droits donnés à un groupe sur
//!   chaque Station enrôlée. La Station le présente par sa connexion de
//!   contrôle (`enroll`), le relais la rattache à l'organisation
//!   ([`Accounts::enroll`]) et, si le jeton le dit, la range dans un groupe
//!   d'ordinateurs ([`super::station_groups`]).

use anyhow::Result;
use kybernow_tunnel::access::Rights;
use kybernow_tunnel::id::base32_lower;
use kybernow_tunnel::msg::{ErrorCode, Msg};
use rusqlite::{Connection, OptionalExtension, params};
use serde_json::{Value, json};

use super::people::{INVITE_SECS, RESEND_GAP_SECS, Resend, invite_live, parse_rights, rights_text};
use super::stations::{Owner, clean_name};
use super::store::{Account, Store};
use super::{Accounts, random, token_hash};

/// La migration 5 de la base (`super::store`) : organisations, membres,
/// invitations, groupes, journal, jetons d'enrôlement ; `stations` reconstruite
/// (une organisation peut en être propriétaire : `account` facultatif) ; les
/// droits donnés à une organisation ou à un groupe.
pub const MIGRATION: &str = r#"
    CREATE TABLE organizations (
        id INTEGER PRIMARY KEY,
        public_id TEXT NOT NULL UNIQUE,
        name TEXT NOT NULL,
        plan TEXT NOT NULL DEFAULT 'free',
        created_at INTEGER NOT NULL
    );
    CREATE TABLE org_members (
        org INTEGER NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
        account INTEGER NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
        role TEXT NOT NULL CHECK (role IN ('owner', 'admin', 'member')),
        joined_at INTEGER NOT NULL,
        PRIMARY KEY (org, account)
    );
    CREATE INDEX org_members_account ON org_members(account);
    CREATE TABLE org_invites (
        id INTEGER PRIMARY KEY,
        org INTEGER NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
        email TEXT NOT NULL,
        role TEXT NOT NULL CHECK (role IN ('owner', 'admin', 'member')),
        invited_by INTEGER REFERENCES accounts(id) ON DELETE SET NULL,
        created_at INTEGER NOT NULL,
        UNIQUE (org, email)
    );
    CREATE INDEX org_invites_email ON org_invites(email);
    CREATE TABLE org_groups (
        id INTEGER PRIMARY KEY,
        org INTEGER NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
        name TEXT NOT NULL,
        created_at INTEGER NOT NULL,
        UNIQUE (org, name)
    );
    CREATE TABLE group_members (
        grp INTEGER NOT NULL REFERENCES org_groups(id) ON DELETE CASCADE,
        account INTEGER NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
        PRIMARY KEY (grp, account)
    );
    CREATE INDEX group_members_account ON group_members(account);
    CREATE TABLE org_log (
        id INTEGER PRIMARY KEY,
        org INTEGER NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
        actor INTEGER REFERENCES accounts(id) ON DELETE SET NULL,
        actor_name TEXT NOT NULL DEFAULT '',
        kind TEXT NOT NULL,
        detail TEXT NOT NULL DEFAULT '{}',
        ip TEXT,
        at INTEGER NOT NULL
    );
    CREATE INDEX org_log_org ON org_log(org, at);
    CREATE TABLE enroll_tokens (
        id INTEGER PRIMARY KEY,
        org INTEGER NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
        name TEXT NOT NULL,
        hash BLOB NOT NULL UNIQUE,
        hint TEXT NOT NULL,
        max_stations INTEGER NOT NULL,
        used INTEGER NOT NULL DEFAULT 0,
        grp INTEGER REFERENCES org_groups(id) ON DELETE SET NULL,
        rights TEXT NOT NULL DEFAULT '',
        created_by INTEGER REFERENCES accounts(id) ON DELETE SET NULL,
        created_at INTEGER NOT NULL,
        expires_at INTEGER NOT NULL,
        revoked_at INTEGER
    );
    CREATE INDEX enroll_tokens_org ON enroll_tokens(org);
    CREATE TABLE stations_v5 (
        id TEXT PRIMARY KEY,
        name TEXT NOT NULL,
        owner_kind TEXT NOT NULL DEFAULT 'person' CHECK (owner_kind IN ('person', 'organization')),
        account INTEGER REFERENCES accounts(id) ON DELETE CASCADE,
        org INTEGER REFERENCES organizations(id) ON DELETE CASCADE,
        attached_by INTEGER REFERENCES accounts(id) ON DELETE SET NULL,
        attached_at INTEGER NOT NULL,
        enrolled_by INTEGER REFERENCES enroll_tokens(id) ON DELETE SET NULL
    );
    INSERT INTO stations_v5 (id, name, owner_kind, account, attached_by, attached_at)
        SELECT id, name, owner_kind, account, attached_by, attached_at FROM stations;
    DROP TABLE stations;
    ALTER TABLE stations_v5 RENAME TO stations;
    CREATE INDEX stations_account ON stations(account);
    CREATE INDEX stations_org ON stations(org);
    ALTER TABLE grants ADD COLUMN org INTEGER REFERENCES organizations(id) ON DELETE CASCADE;
    ALTER TABLE grants ADD COLUMN grp INTEGER REFERENCES org_groups(id) ON DELETE CASCADE;
    CREATE UNIQUE INDEX grants_org ON grants(station, org) WHERE org IS NOT NULL;
    CREATE UNIQUE INDEX grants_grp ON grants(station, grp) WHERE grp IS NOT NULL;
"#;

/// Le préfixe d'un jeton d'enrôlement.
pub const ENROLL_PREFIX: &str = "kne_";
/// Un jeton d'enrôlement vaut au plus un an.
pub const ENROLL_MAX_DAYS: i64 = 365;
/// Et enrôle au plus ce nombre de Stations.
pub const ENROLL_MAX_STATIONS: i64 = 10_000;

/// Le rôle d'un membre.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Role {
    Owner,
    Admin,
    Member,
}

impl Role {
    pub const ALL: [Role; 3] = [Role::Owner, Role::Admin, Role::Member];

    pub fn as_str(self) -> &'static str {
        match self {
            Role::Owner => "owner",
            Role::Admin => "admin",
            Role::Member => "member",
        }
    }

    pub fn parse(s: &str) -> Option<Role> {
        Role::ALL.into_iter().find(|r| r.as_str() == s)
    }

    /// Gère l'organisation (avec la double authentification).
    pub fn manages(self) -> bool {
        matches!(self, Role::Owner | Role::Admin)
    }
}

/// Une organisation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Org {
    pub id: i64,
    /// `org_…` : dans les adresses de la console et chez la Station.
    pub public_id: String,
    pub name: String,
    pub created_at: i64,
    /// Famille ou équipe : les mêmes droits, d'autres mots (docs/comptes.md, « L'espace du compte »).
    pub kind: Kind,
}

/// Le genre d'une organisation, choisi à sa création.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// Pour la maison : pas de jetons d'enrôlement.
    Family,
    /// Pour un atelier, une association, une entreprise.
    Team,
}

impl Kind {
    pub fn as_str(self) -> &'static str {
        match self {
            Kind::Family => "family",
            Kind::Team => "team",
        }
    }

    /// `family` ou `famille` ; tout le reste : une équipe.
    pub fn parse(s: &str) -> Kind {
        match s {
            "family" | "famille" => Kind::Family,
            _ => Kind::Team,
        }
    }
}

fn org_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<Org> {
    Ok(Org { id: r.get(0)?, public_id: r.get(1)?, name: r.get(2)?, created_at: r.get(3)?, kind: Kind::parse(&r.get::<_, String>(4)?) })
}

/// Un membre.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Member {
    pub account: i64,
    pub name: String,
    pub email: String,
    pub role: Role,
    pub joined_at: i64,
    /// Double authentification (TOTP ou passkey).
    pub two_factor: bool,
    /// Ses groupes, par nom.
    pub groups: Vec<String>,
}

/// Une invitation en attente (adresse sans compte).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Invite {
    pub id: i64,
    pub email: String,
    pub role: Role,
    pub created_at: i64,
    /// Son dernier envoi (création, remplacement, « Renvoyer l'invitation »).
    pub sent_at: i64,
}

impl Invite {
    /// Quand elle expire (30 jours après son dernier envoi).
    pub fn expires_at(&self) -> i64 {
        self.sent_at + INVITE_SECS
    }
}

/// Un groupe et ses membres.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Group {
    pub id: i64,
    pub name: String,
    /// `(compte, nom)`.
    pub members: Vec<(i64, String)>,
}

/// Une ligne du journal de l'organisation.
#[derive(Debug, Clone, PartialEq)]
pub struct LogLine {
    pub actor_name: String,
    pub kind: String,
    pub detail: Value,
    pub ip: Option<String>,
    pub at: i64,
}

/// Un jeton d'enrôlement (jamais le jeton lui-même : son empreinte).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnrollToken {
    pub id: i64,
    pub name: String,
    /// Le début du jeton (`kne_7qm2…`), pour le reconnaître.
    pub hint: String,
    pub max_stations: i64,
    pub used: i64,
    pub grp: Option<i64>,
    pub group_name: Option<String>,
    pub rights: Rights,
    pub created_at: i64,
    pub expires_at: i64,
    pub revoked_at: Option<i64>,
    /// Le groupe d'ordinateurs où ranger chaque Station enrôlée (« Ranger dans le groupe »).
    pub sgroup: Option<i64>,
    pub sgroup_name: Option<String>,
}

impl EnrollToken {
    /// Pourquoi il n'enrôle plus (`None` : il enrôle).
    pub fn refusal(&self, now: i64) -> Option<&'static str> {
        if self.revoked_at.is_some() {
            Some("revoked")
        } else if self.expires_at <= now {
            Some("expired")
        } else if self.used >= self.max_stations {
            Some("used_up")
        } else {
            None
        }
    }
}

/// Un nom d'organisation ou de groupe lisible : 64 caractères au plus.
pub fn clean_org_name(name: &str) -> String {
    clean_name(name)
}

/// Les invitations d'organisation qui attendaient cette adresse passent au compte
/// (dans la transaction de [`Store::claim_invitations`]).
/// Les invitations expirées (30 jours) restent, sans passer.
pub(super) fn claim_org_invites(conn: &Connection, account: i64, email: &str, now: i64) -> Result<usize> {
    let live = invite_live("org_invites", "?3");
    let n = conn.execute(
        &format!(
            "INSERT OR IGNORE INTO org_members (org, account, role, joined_at)
             SELECT org, ?1, role, created_at FROM org_invites WHERE email = ?2 AND {live}"
        ),
        params![account, email, now],
    )?;
    let live2 = invite_live("org_invites", "?2");
    conn.execute(&format!("DELETE FROM org_invites WHERE email = ?1 AND {live2}"), params![email, now])?;
    Ok(n)
}

/// Avant d'effacer un compte : une organisation dont il est le seul
/// Propriétaire en reçoit un autre (le plus ancien Administrateur, sinon le
/// plus ancien Membre) ; sans autre membre, elle part avec ses ordinateurs.
pub(super) fn before_account_deleted(conn: &Connection, account: i64) -> Result<()> {
    let orgs: Vec<i64> = {
        let mut stmt = conn.prepare(
            "SELECT m.org FROM org_members m WHERE m.account = ?1 AND m.role = 'owner'
             AND NOT EXISTS (SELECT 1 FROM org_members o WHERE o.org = m.org AND o.role = 'owner' AND o.account <> ?1)",
        )?;
        stmt.query_map([account], |r| r.get(0))?.collect::<rusqlite::Result<_>>()?
    };
    for org in orgs {
        let heir: Option<i64> = conn
            .query_row(
                "SELECT account FROM org_members WHERE org = ?1 AND account <> ?2 ORDER BY role = 'member', joined_at, account LIMIT 1",
                params![org, account],
                |r| r.get(0),
            )
            .optional()?;
        match heir {
            Some(heir) => {
                conn.execute("UPDATE org_members SET role = 'owner' WHERE org = ?1 AND account = ?2", params![org, heir])?;
            }
            None => {
                conn.execute("DELETE FROM device_stations WHERE station IN (SELECT id FROM stations WHERE org = ?1)", [org])?;
                conn.execute("DELETE FROM grants WHERE station IN (SELECT id FROM stations WHERE org = ?1)", [org])?;
                conn.execute("DELETE FROM access_links WHERE station IN (SELECT id FROM stations WHERE org = ?1)", [org])?;
                conn.execute("DELETE FROM computer_settings WHERE station IN (SELECT id FROM stations WHERE org = ?1)", [org])?;
                conn.execute("DELETE FROM organizations WHERE id = ?1", [org])?;
            }
        }
    }
    Ok(())
}

const ORG_COLS: &str = "id, public_id, name, created_at, kind";

impl Store {
    // --- Organisations et membres ---------------------------------------------

    /// Crée une organisation ; son créateur en est le Propriétaire.
    pub fn create_org(&self, name: &str, kind: Kind, owner: i64, now: i64) -> Result<Org> {
        let public_id = format!("org_{}", base32_lower(&random::<10>()));
        let tx = self.conn.unchecked_transaction()?;
        tx.execute("INSERT INTO organizations (public_id, name, created_at, kind) VALUES (?1, ?2, ?3, ?4)", params![public_id, name, now, kind.as_str()])?;
        let id = tx.last_insert_rowid();
        tx.execute("INSERT INTO org_members (org, account, role, joined_at) VALUES (?1, ?2, 'owner', ?3)", params![id, owner, now])?;
        tx.commit()?;
        Ok(Org { id, public_id, name: name.into(), created_at: now, kind })
    }

    pub fn org(&self, id: i64) -> Result<Option<Org>> {
        Ok(self.conn.query_row(&format!("SELECT {ORG_COLS} FROM organizations WHERE id = ?1"), [id], org_row).optional()?)
    }

    pub fn org_by_public(&self, public_id: &str) -> Result<Option<Org>> {
        Ok(self.conn.query_row(&format!("SELECT {ORG_COLS} FROM organizations WHERE public_id = ?1"), [public_id], org_row).optional()?)
    }

    /// Les organisations d'un compte et son rôle, par nom.
    pub fn orgs_of(&self, account: i64) -> Result<Vec<(Org, Role)>> {
        let mut stmt = self.conn.prepare(
            "SELECT o.id, o.public_id, o.name, o.created_at, o.kind, m.role FROM organizations o JOIN org_members m ON m.org = o.id
             WHERE m.account = ?1 ORDER BY o.name COLLATE NOCASE, o.id",
        )?;
        let rows = stmt.query_map([account], |r| Ok((org_row(r)?, r.get::<_, String>(5)?)))?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?.into_iter().filter_map(|(o, r)| Some((o, Role::parse(&r)?))).collect())
    }

    pub fn rename_org(&self, org: i64, name: &str) -> Result<()> {
        self.conn.execute("UPDATE organizations SET name = ?2 WHERE id = ?1", params![org, name])?;
        Ok(())
    }

    /// Le rôle d'un compte dans une organisation.
    pub fn org_role(&self, org: i64, account: i64) -> Result<Option<Role>> {
        let role: Option<String> =
            self.conn.query_row("SELECT role FROM org_members WHERE org = ?1 AND account = ?2", params![org, account], |r| r.get(0)).optional()?;
        Ok(role.as_deref().and_then(Role::parse))
    }

    /// Les membres, Propriétaires d'abord.
    pub fn members(&self, org: i64) -> Result<Vec<Member>> {
        let mut stmt = self.conn.prepare(
            "SELECT a.id, a.name, a.email, m.role, m.joined_at FROM org_members m JOIN accounts a ON a.id = m.account
             WHERE m.org = ?1 ORDER BY CASE m.role WHEN 'owner' THEN 0 WHEN 'admin' THEN 1 ELSE 2 END, a.name COLLATE NOCASE, a.id",
        )?;
        let rows: Vec<(i64, String, String, String, i64)> =
            stmt.query_map([org], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)))?.collect::<rusqlite::Result<_>>()?;
        let mut out = Vec::with_capacity(rows.len());
        for (account, name, email, role, joined_at) in rows {
            let mut g = self.conn.prepare(
                "SELECT g.name FROM org_groups g JOIN group_members gm ON gm.grp = g.id WHERE g.org = ?1 AND gm.account = ?2 ORDER BY g.name COLLATE NOCASE",
            )?;
            let groups = g.query_map(params![org, account], |r| r.get(0))?.collect::<rusqlite::Result<_>>()?;
            out.push(Member {
                account,
                name,
                email,
                role: Role::parse(&role).unwrap_or(Role::Member),
                joined_at,
                two_factor: self.two_factor(account)?,
                groups,
            });
        }
        Ok(out)
    }

    /// Ajoute un membre (ou change son rôle) ; une invitation de son adresse (expirée) part.
    pub fn set_member(&self, org: i64, account: i64, role: Role, now: i64) -> Result<()> {
        self.conn.execute(
            "DELETE FROM org_invites WHERE org = ?1 AND email = (SELECT email FROM accounts WHERE id = ?2)",
            params![org, account],
        )?;
        self.conn.execute(
            "INSERT INTO org_members (org, account, role, joined_at) VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(org, account) DO UPDATE SET role = excluded.role",
            params![org, account, role.as_str(), now],
        )?;
        Ok(())
    }

    /// Retire un membre (de ses groupes aussi, et ses droits individuels sur les ordinateurs de
    /// l'équipe : REL-04) ; faux s'il n'y était pas.
    ///
    /// Un simple membre qui avait reçu Administrer sur un ordinateur de l'équipe : ce qu'il y a
    /// donné part aussi (REL-02). Ce qu'un Propriétaire ou un Administrateur a donné, au nom de
    /// l'équipe, reste.
    pub fn remove_member(&self, org: i64, account: i64, now: i64) -> Result<bool> {
        let delegate = self.org_role(org, account)?.is_some_and(|r| !r.manages());
        let stations: Vec<String> = {
            let mut stmt = self.conn.prepare("SELECT station FROM grants WHERE account = ?2 AND station IN (SELECT id FROM stations WHERE org = ?1)")?;
            stmt.query_map(params![org, account], |r| r.get(0))?.collect::<rusqlite::Result<_>>()?
        };
        let tx = self.conn.unchecked_transaction()?;
        tx.execute("DELETE FROM group_members WHERE account = ?2 AND grp IN (SELECT id FROM org_groups WHERE org = ?1)", params![org, account])?;
        tx.execute("DELETE FROM grants WHERE account = ?2 AND station IN (SELECT id FROM stations WHERE org = ?1)", params![org, account])?;
        let n = tx.execute("DELETE FROM org_members WHERE org = ?1 AND account = ?2", params![org, account])?;
        tx.commit()?;
        if delegate {
            for st in stations {
                self.drop_delegated(&st, account, now)?;
            }
        }
        Ok(n == 1)
    }

    /// Le nombre de Propriétaires.
    pub fn owners(&self, org: i64) -> Result<usize> {
        let n: i64 = self.conn.query_row("SELECT COUNT(*) FROM org_members WHERE org = ?1 AND role = 'owner'", [org], |r| r.get(0))?;
        Ok(n as usize)
    }

    /// Les Propriétaires et Administrateurs (les avis de l'organisation).
    pub fn org_managers(&self, org: i64) -> Result<Vec<Account>> {
        let ids: Vec<i64> = {
            let mut stmt = self.conn.prepare("SELECT account FROM org_members WHERE org = ?1 AND role IN ('owner', 'admin') ORDER BY account")?;
            stmt.query_map([org], |r| r.get(0))?.collect::<rusqlite::Result<_>>()?
        };
        Ok(ids.into_iter().filter_map(|id| self.account(id).ok().flatten()).collect())
    }

    // --- Invitations ------------------------------------------------------------

    /// Invite une adresse sans compte ; une invitation en attente pour la même
    /// adresse est remplacée (rôle, et envoyée de nouveau : 30 jours).
    pub fn invite_member(&self, org: i64, email: &str, role: Role, by: i64, now: i64) -> Result<()> {
        self.conn.execute(
            "INSERT INTO org_invites (org, email, role, invited_by, created_at, sent_at) VALUES (?1, ?2, ?3, ?4, ?5, ?5)
             ON CONFLICT(org, email) DO UPDATE SET role = excluded.role, invited_by = excluded.invited_by, sent_at = excluded.sent_at",
            params![org, email, role.as_str(), by, now],
        )?;
        Ok(())
    }

    pub fn org_invites(&self, org: i64) -> Result<Vec<Invite>> {
        let mut stmt = self
            .conn
            .prepare("SELECT id, email, role, created_at, COALESCE(sent_at, created_at) FROM org_invites WHERE org = ?1 ORDER BY created_at, id")?;
        let rows = stmt.query_map([org], |r| {
            Ok(Invite {
                id: r.get(0)?,
                email: r.get(1)?,
                role: Role::parse(&r.get::<_, String>(2)?).unwrap_or(Role::Member),
                created_at: r.get(3)?,
                sent_at: r.get(4)?,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    /// Une invitation en attente : `(adresse, rôle)`.
    pub fn org_invite(&self, org: i64, id: i64) -> Result<Option<(String, Role)>> {
        let row: Option<(String, String)> =
            self.conn.query_row("SELECT email, role FROM org_invites WHERE id = ?1 AND org = ?2", params![id, org], |r| Ok((r.get(0)?, r.get(1)?))).optional()?;
        Ok(row.map(|(e, r)| (e, Role::parse(&r).unwrap_or(Role::Member))))
    }

    /// « Renvoyer l'invitation » d'une famille ou d'une équipe : repart pour 30
    /// jours (même expirée), une fois par heure au plus.
    pub fn resend_org_invite(&self, org: i64, id: i64, now: i64) -> Result<Resend> {
        let found: Option<(String, Option<i64>)> = self
            .conn
            .query_row("SELECT email, resent_at FROM org_invites WHERE id = ?1 AND org = ?2", params![id, org], |r| Ok((r.get(0)?, r.get(1)?)))
            .optional()?;
        let Some((email, resent_at)) = found else { return Ok(Resend::Missing) };
        if resent_at.is_some_and(|at| now - at < RESEND_GAP_SECS) {
            return Ok(Resend::TooSoon);
        }
        self.conn.execute("UPDATE org_invites SET sent_at = ?2, resent_at = ?2 WHERE id = ?1", params![id, now])?;
        Ok(Resend::Done(email))
    }

    /// Retire une invitation ; son adresse, si elle y était.
    pub fn cancel_invite(&self, org: i64, id: i64) -> Result<Option<String>> {
        let email: Option<String> =
            self.conn.query_row("SELECT email FROM org_invites WHERE id = ?1 AND org = ?2", params![id, org], |r| r.get(0)).optional()?;
        self.conn.execute("DELETE FROM org_invites WHERE id = ?1 AND org = ?2", params![id, org])?;
        Ok(email)
    }

    // --- Groupes ----------------------------------------------------------------

    /// Crée un groupe ; `None` si le nom est déjà pris.
    pub fn create_group(&self, org: i64, name: &str, now: i64) -> Result<Option<i64>> {
        let n = self
            .conn
            .execute("INSERT INTO org_groups (org, name, created_at) VALUES (?1, ?2, ?3) ON CONFLICT(org, name) DO NOTHING", params![org, name, now])?;
        Ok((n == 1).then(|| self.conn.last_insert_rowid()))
    }

    pub fn groups(&self, org: i64) -> Result<Vec<Group>> {
        let list: Vec<(i64, String)> = {
            let mut stmt = self.conn.prepare("SELECT id, name FROM org_groups WHERE org = ?1 ORDER BY name COLLATE NOCASE, id")?;
            stmt.query_map([org], |r| Ok((r.get(0)?, r.get(1)?)))?.collect::<rusqlite::Result<_>>()?
        };
        let mut out = Vec::with_capacity(list.len());
        for (id, name) in list {
            let mut stmt = self
                .conn
                .prepare("SELECT a.id, a.name FROM group_members gm JOIN accounts a ON a.id = gm.account WHERE gm.grp = ?1 ORDER BY a.name COLLATE NOCASE, a.id")?;
            let members = stmt.query_map([id], |r| Ok((r.get(0)?, r.get(1)?)))?.collect::<rusqlite::Result<_>>()?;
            out.push(Group { id, name, members });
        }
        Ok(out)
    }

    /// Le groupe de cette organisation, son nom.
    pub fn group_name(&self, org: i64, grp: i64) -> Result<Option<String>> {
        Ok(self.conn.query_row("SELECT name FROM org_groups WHERE id = ?1 AND org = ?2", params![grp, org], |r| r.get(0)).optional()?)
    }

    pub fn delete_group(&self, org: i64, grp: i64) -> Result<bool> {
        Ok(self.conn.execute("DELETE FROM org_groups WHERE id = ?1 AND org = ?2", params![grp, org])? == 1)
    }

    /// Met un membre dans un groupe (`true`) ou l'en retire ; faux si rien n'a changé.
    pub fn set_group_member(&self, org: i64, grp: i64, account: i64, inside: bool) -> Result<bool> {
        if self.group_name(org, grp)?.is_none() || self.org_role(org, account)?.is_none() {
            return Ok(false);
        }
        let n = if inside {
            self.conn.execute("INSERT OR IGNORE INTO group_members (grp, account) VALUES (?1, ?2)", params![grp, account])?
        } else {
            self.conn.execute("DELETE FROM group_members WHERE grp = ?1 AND account = ?2", params![grp, account])?
        };
        Ok(n == 1)
    }

    // --- Droits donnés à une organisation ou à un groupe -----------------------

    /// Ouvre une Station à toute une organisation (ou change ses droits).
    pub fn grant_org(&self, station: &str, org: i64, rights: &Rights, by: i64, now: i64) -> Result<i64> {
        self.grant_to(station, "org", org, rights, by, now)
    }

    /// Ouvre une Station à un groupe (ou change ses droits).
    pub fn grant_group(&self, station: &str, grp: i64, rights: &Rights, by: i64, now: i64) -> Result<i64> {
        self.grant_to(station, "grp", grp, rights, by, now)
    }

    fn grant_to(&self, station: &str, column: &str, id: i64, rights: &Rights, by: i64, now: i64) -> Result<i64> {
        let text = rights_text(rights);
        let by = (by > 0).then_some(by);
        let found: Option<i64> =
            self.conn.query_row(&format!("SELECT id FROM grants WHERE station = ?1 AND {column} = ?2"), params![station, id], |r| r.get(0)).optional()?;
        if let Some(g) = found {
            self.conn.execute("UPDATE grants SET rights = ?2 WHERE id = ?1", params![g, text])?;
            return Ok(g);
        }
        self.conn.execute(
            &format!("INSERT INTO grants (station, {column}, rights, invited_by, created_at) VALUES (?1, ?2, ?3, ?4, ?5)"),
            params![station, id, text, by, now],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    // --- Journal ----------------------------------------------------------------

    /// Une ligne au journal de l'organisation ; `actor` : qui (aucun : la Station, le relais).
    pub fn org_log(&self, org: i64, actor: Option<&Account>, kind: &str, detail: Value, ip: Option<&str>, now: i64) -> Result<()> {
        self.conn.execute(
            "INSERT INTO org_log (org, actor, actor_name, kind, detail, ip, at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![org, actor.map(|a| a.id), actor.map(|a| a.name.as_str()).unwrap_or(""), kind, detail.to_string(), ip, now],
        )?;
        Ok(())
    }

    /// Les dernières lignes du journal, la plus récente d'abord.
    pub fn org_log_of(&self, org: i64, limit: usize) -> Result<Vec<LogLine>> {
        let mut stmt = self.conn.prepare("SELECT actor_name, kind, detail, ip, at FROM org_log WHERE org = ?1 ORDER BY at DESC, id DESC LIMIT ?2")?;
        let rows = stmt.query_map(params![org, limit as i64], |r| {
            Ok(LogLine {
                actor_name: r.get(0)?,
                kind: r.get(1)?,
                detail: serde_json::from_str(&r.get::<_, String>(2)?).unwrap_or(Value::Null),
                ip: r.get(3)?,
                at: r.get(4)?,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    // --- Jetons d'enrôlement ----------------------------------------------------

    #[allow(clippy::too_many_arguments)]
    pub fn create_enroll_token(
        &self,
        org: i64,
        name: &str,
        hash: &[u8],
        hint: &str,
        max_stations: i64,
        grp: Option<i64>,
        rights: &Rights,
        sgroup: Option<i64>,
        by: i64,
        now: i64,
        expires_at: i64,
    ) -> Result<i64> {
        self.conn.execute(
            "INSERT INTO enroll_tokens (org, name, hash, hint, max_stations, grp, rights, created_by, created_at, expires_at, sgroup)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![org, name, hash, hint, max_stations, grp, rights_text(rights), by, now, expires_at, sgroup],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn enroll_tokens(&self, org: i64) -> Result<Vec<EnrollToken>> {
        self.enroll_query("WHERE t.org = ?1 ORDER BY t.revoked_at IS NOT NULL, t.created_at DESC, t.id DESC", params![org])
    }

    fn enroll_by_hash(&self, hash: &[u8]) -> Result<Option<(i64, EnrollToken)>> {
        let mut stmt = self.conn.prepare("SELECT org FROM enroll_tokens WHERE hash = ?1")?;
        let org: Option<i64> = stmt.query_row([hash], |r| r.get(0)).optional()?;
        let Some(org) = org else { return Ok(None) };
        let list = self.enroll_query("WHERE t.hash = ?1", params![hash])?;
        Ok(list.into_iter().next().map(|t| (org, t)))
    }

    fn enroll_query(&self, filter: &str, p: impl rusqlite::Params) -> Result<Vec<EnrollToken>> {
        let mut stmt = self.conn.prepare(&format!(
            "SELECT t.id, t.name, t.hint, t.max_stations, t.used, t.grp, g.name, t.rights, t.created_at, t.expires_at, t.revoked_at, t.sgroup, sg.name
             FROM enroll_tokens t LEFT JOIN org_groups g ON g.id = t.grp LEFT JOIN station_groups sg ON sg.id = t.sgroup {filter}"
        ))?;
        let rows = stmt.query_map(p, |r| {
            Ok(EnrollToken {
                id: r.get(0)?,
                name: r.get(1)?,
                hint: r.get(2)?,
                max_stations: r.get(3)?,
                used: r.get(4)?,
                grp: r.get(5)?,
                group_name: r.get(6)?,
                rights: parse_rights(&r.get::<_, String>(7)?),
                created_at: r.get(8)?,
                expires_at: r.get(9)?,
                revoked_at: r.get(10)?,
                sgroup: r.get(11)?,
                sgroup_name: r.get(12)?,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    pub fn revoke_enroll_token(&self, org: i64, id: i64, now: i64) -> Result<bool> {
        Ok(self.conn.execute("UPDATE enroll_tokens SET revoked_at = ?3 WHERE id = ?1 AND org = ?2 AND revoked_at IS NULL", params![id, org, now])? == 1)
    }

    /// Les Stations enrôlées par ce jeton : `(nom, quand)`.
    pub fn enrolled_by(&self, token: i64) -> Result<Vec<(String, i64)>> {
        let mut stmt = self.conn.prepare("SELECT name, attached_at FROM stations WHERE enrolled_by = ?1 ORDER BY attached_at, id")?;
        let rows = stmt.query_map([token], |r| Ok((r.get(0)?, r.get(1)?)))?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    /// Le ménage du journal des organisations : adresses IP après quelques jours, lignes après 12 mois.
    pub fn sweep_orgs(&self, now: i64) -> Result<()> {
        self.conn.execute("UPDATE org_log SET ip = NULL WHERE ip IS NOT NULL AND at <= ?1", [now - super::store::AUDIT_IP_SECS])?;
        self.conn.execute("DELETE FROM org_log WHERE at <= ?1", [now - super::store::AUDIT_SECS])?;
        Ok(())
    }
}

/// Un nouveau jeton d'enrôlement : `kne_` et 32 signes (160 bits).
pub fn new_enroll_token() -> String {
    format!("{ENROLL_PREFIX}{}", base32_lower(&random::<20>()))
}

impl Accounts {
    /// Ce compte gère-t-il cette organisation (Propriétaire ou Administrateur, avec la double authentification) ?
    pub fn org_manager(&self, account: &Account, org: Option<i64>) -> bool {
        let Some(org) = org else { return false };
        let db = self.db();
        db.org_role(org, account.id).ok().flatten().is_some_and(Role::manages) && db.two_factor(account.id).unwrap_or(false)
    }

    /// Les organisations que ce compte gère : où rattacher un ordinateur.
    pub fn managed_orgs(&self, account: &Account) -> Vec<Org> {
        let db = self.db();
        if !db.two_factor(account.id).unwrap_or(false) {
            return Vec::new();
        }
        db.orgs_of(account.id).unwrap_or_default().into_iter().filter(|(_, r)| r.manages()).map(|(o, _)| o).collect()
    }

    /// Le propriétaire choisi sur la page « Rattacher » : « moi », ou l'identifiant d'une organisation gérée.
    pub fn owner_choice(&self, account: &Account, choice: &str) -> Option<Owner> {
        if choice.is_empty() || choice == "moi" {
            return Some(Owner::Person);
        }
        self.managed_orgs(account).into_iter().find(|o| o.public_id == choice).map(|o| Owner::Org { id: o.id, public_id: o.public_id, name: o.name })
    }

    /// `enroll` d'une Station (connexion de contrôle) : le jeton d'enrôlement
    /// la rattache à l'organisation ; `attached`, ou `error {code: "enroll_refused"}`.
    pub fn enroll(&self, station: &str, token: &str, name: &str, req: u32) -> Msg {
        let now = self.now();
        let refused = |why: &str| Msg::error(Some(req), ErrorCode::EnrollRefused, Some(why.to_string()));
        let token = token.trim();
        if !token.starts_with(ENROLL_PREFIX) || token.len() > 128 {
            return refused("unknown");
        }
        let name = clean_name(name);
        let db = self.db();
        let Ok(Some((org, t))) = db.enroll_by_hash(&token_hash(token)) else {
            drop(db);
            tracing::info!(station = %station, "enrôlement refusé : jeton inconnu");
            return refused("unknown");
        };
        // Déjà enrôlée dans cette organisation : la même réponse, sans compter une Station de plus.
        let row = db.station(station).ok().flatten();
        if row.as_ref().is_some_and(|r| r.org == Some(org)) {
            drop(db);
            return self.attached_msg_of(&row.expect("vue"), Some(req)).unwrap_or_else(|| refused("unknown"));
        }
        if let Some(why) = t.refusal(now) {
            drop(db);
            tracing::info!(station = %station, raison = why, "enrôlement refusé");
            return refused(why);
        }
        let done = (|| -> Result<()> {
            let tx = db.conn.unchecked_transaction()?;
            // Compté d'abord : deux Stations en même temps ne dépassent pas le nombre.
            let n = tx.execute("UPDATE enroll_tokens SET used = used + 1 WHERE id = ?1 AND used < max_stations", [t.id])?;
            anyhow::ensure!(n == 1, "jeton épuisé");
            tx.commit()?;
            db.attach_station_to_org(station, &name, org, None, Some(t.id), now)?;
            if let Some(grp) = t.grp.filter(|_| !t.rights.is_empty()) {
                db.grant_group(station, grp, &t.rights, 0, now)?;
            }
            // « Ranger dans le groupe » : le groupe d'ordinateurs, s'il existe encore.
            if let Some(sgroup) = t.sgroup {
                db.set_station_group_member(org, sgroup, station, true)?;
            }
            db.org_log(org, None, "station_enrolee", json!({ "station": name, "token": t.name, "group": t.sgroup_name.clone().unwrap_or_default() }), None, now)?;
            Ok(())
        })();
        if let Err(e) = done {
            drop(db);
            tracing::warn!("enrôlement : {e:#}");
            return refused("used_up");
        }
        let row = db.station(station).ok().flatten();
        drop(db);
        tracing::info!(station = %station, "Station enrôlée dans une organisation");
        row.and_then(|r| self.attached_msg_of(&r, Some(req))).unwrap_or_else(|| refused("unknown"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kybernow_tunnel::access::Right;

    #[test]
    fn membres_roles_groupes_et_droits() {
        let s = Store::memory(0).unwrap();
        let a = s.create_account("a@exemple.org", "A", "h", "fr", Some(0), 0).unwrap().unwrap();
        let b = s.create_account("b@exemple.org", "B", "h", "fr", Some(0), 0).unwrap().unwrap();
        let org = s.create_org("Les Lignes Claires", Kind::Team, a, 10).unwrap();
        assert!(org.public_id.starts_with("org_"));
        assert_eq!(s.org_role(org.id, a).unwrap(), Some(Role::Owner));
        s.set_member(org.id, b, Role::Member, 20).unwrap();
        let g = s.create_group(org.id, "Graphistes", 30).unwrap().unwrap();
        assert!(s.create_group(org.id, "Graphistes", 30).unwrap().is_none(), "nom déjà pris");
        assert!(s.set_group_member(org.id, g, b, true).unwrap());
        // Une Station de l'organisation, ouverte au groupe.
        s.attach_station_to_org("st1", "Station de rendu", org.id, Some(a), None, 40).unwrap();
        let row = s.station("st1").unwrap().unwrap();
        assert_eq!((row.owner_kind.as_str(), row.account, row.org), ("organization", None, Some(org.id)));
        assert!(s.granted_rights(b, "st1", 50).unwrap().is_empty());
        s.grant_group("st1", g, &Rights::new([Right::Session]), a, 50).unwrap();
        assert_eq!(s.granted_rights(b, "st1", 60).unwrap(), Rights::new([Right::Session]));
        assert_eq!(s.visible_stations(b, 60).unwrap().len(), 1);
        assert_eq!(s.visible_stations(a, 60).unwrap().len(), 1, "Propriétaire");
        // Toute l'organisation : Prêt de plus.
        s.grant_org("st1", org.id, &Rights::new([Right::Lend]), a, 60).unwrap();
        assert_eq!(s.granted_rights(b, "st1", 70).unwrap(), Rights::new([Right::Session, Right::Lend]));
        assert_eq!(s.members(org.id).unwrap()[1].groups, vec!["Graphistes".to_string()]);
        // Retiré de l'organisation : plus rien.
        assert!(s.remove_member(org.id, b, 0).unwrap());
        assert!(s.granted_rights(b, "st1", 70).unwrap().is_empty());
        assert!(s.groups(org.id).unwrap()[0].members.is_empty());
    }

    #[test]
    fn invitations_reprises_et_compte_efface() {
        let s = Store::memory(0).unwrap();
        let a = s.create_account("a@exemple.org", "A", "h", "fr", Some(0), 0).unwrap().unwrap();
        let org = s.create_org("Org", Kind::Team, a, 10).unwrap();
        s.invite_member(org.id, "c@exemple.org", Role::Admin, a, 20).unwrap();
        assert_eq!(s.org_invites(org.id).unwrap().len(), 1);
        let c = s.create_account("c@exemple.org", "C", "h", "fr", Some(30), 30).unwrap().unwrap();
        assert_eq!(s.claim_invitations(c, 30).unwrap(), 1);
        assert_eq!(s.org_role(org.id, c).unwrap(), Some(Role::Admin));
        assert!(s.org_invites(org.id).unwrap().is_empty());
        // Le seul Propriétaire s'en va : l'Administrateur le devient.
        s.attach_station_to_org("st1", "PC", org.id, Some(a), None, 40).unwrap();
        assert!(s.delete_account(a).unwrap());
        assert_eq!(s.org_role(org.id, c).unwrap(), Some(Role::Owner));
        assert!(s.station("st1").unwrap().is_some(), "l'ordinateur reste à l'organisation");
        // Le dernier membre s'en va : l'organisation part avec ses ordinateurs.
        assert!(s.delete_account(c).unwrap());
        assert!(s.org(org.id).unwrap().is_none());
        assert!(s.station("st1").unwrap().is_none());
    }

    #[test]
    fn invitations_d_organisation_expirees_remplacees_renvoyees() {
        let s = Store::memory(0).unwrap();
        let a = s.create_account("a@exemple.org", "A", "h", "fr", Some(0), 0).unwrap().unwrap();
        let org = s.create_org("Org", Kind::Team, a, 10).unwrap();
        let t0 = 1_000_000;
        s.invite_member(org.id, "c@exemple.org", Role::Member, a, t0).unwrap();
        s.invite_member(org.id, "c@exemple.org", Role::Admin, a, t0 + 100).unwrap();
        let list = s.org_invites(org.id).unwrap();
        assert_eq!(list.len(), 1, "remplacée, pas doublée");
        assert_eq!((list[0].role, list[0].sent_at, list[0].expires_at()), (Role::Admin, t0 + 100, t0 + 100 + INVITE_SECS));
        // Expirée : pas reprise, toujours là.
        let later = t0 + 100 + INVITE_SECS;
        let c = s.create_account("c@exemple.org", "C", "h", "fr", None, t0).unwrap().unwrap();
        assert_eq!(s.claim_invitations(c, later).unwrap(), 0);
        assert_eq!(s.org_role(org.id, c).unwrap(), None);
        assert_eq!(s.org_invites(org.id).unwrap().len(), 1);
        // Renvoyée : une fois par heure ; puis reprise.
        let id = list[0].id;
        assert_eq!(s.org_invite(org.id, id).unwrap(), Some(("c@exemple.org".to_string(), Role::Admin)));
        assert_eq!(s.resend_org_invite(org.id, id, later).unwrap(), Resend::Done("c@exemple.org".into()));
        assert_eq!(s.resend_org_invite(org.id, id, later + 10).unwrap(), Resend::TooSoon);
        assert_eq!(s.resend_org_invite(org.id + 1, id, later + RESEND_GAP_SECS).unwrap(), Resend::Missing);
        assert_eq!(s.claim_invitations(c, later + 20).unwrap(), 1);
        assert_eq!(s.org_role(org.id, c).unwrap(), Some(Role::Admin));
        assert!(s.org_invites(org.id).unwrap().is_empty());
        // Ajouter un compte par `set_member` retire une invitation expirée de son adresse.
        s.invite_member(org.id, "d@exemple.org", Role::Member, a, t0).unwrap();
        let d = s.create_account("d@exemple.org", "D", "h", "fr", Some(later), later).unwrap().unwrap();
        s.set_member(org.id, d, Role::Member, later).unwrap();
        assert!(s.org_invites(org.id).unwrap().is_empty());
    }

    #[test]
    fn jetons_d_enrolement() {
        let s = Store::memory(0).unwrap();
        let a = s.create_account("a@exemple.org", "A", "h", "fr", Some(0), 0).unwrap().unwrap();
        let org = s.create_org("Org", Kind::Team, a, 10).unwrap();
        let token = new_enroll_token();
        assert!(token.starts_with("kne_") && token.len() == 36);
        let id = s.create_enroll_token(org.id, "Postes", &token_hash(&token), &token[..9], 2, None, &Rights::default(), None, a, 10, 100).unwrap();
        let (o, t) = s.enroll_by_hash(&token_hash(&token)).unwrap().unwrap();
        assert_eq!((o, t.id, t.refusal(50)), (org.id, id, None));
        assert_eq!(t.refusal(100), Some("expired"));
        assert!(s.revoke_enroll_token(org.id, id, 60).unwrap());
        assert_eq!(s.enroll_tokens(org.id).unwrap()[0].refusal(70), Some("revoked"));
        // Le journal : IP oubliée après quelques jours, lignes après un an.
        s.org_log(org.id, None, "x", json!({}), Some("192.0.2.1"), 0).unwrap();
        s.sweep_orgs(super::super::store::AUDIT_IP_SECS).unwrap();
        assert_eq!(s.org_log_of(org.id, 10).unwrap()[0].ip, None);
        s.sweep_orgs(super::super::store::AUDIT_SECS).unwrap();
        assert!(s.org_log_of(org.id, 10).unwrap().is_empty());
    }
}
