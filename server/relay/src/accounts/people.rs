//! Les personnes et leurs droits sur un ordinateur (docs/comptes.md,
//! « Ajouter des personnes », lot 4).
//!
//! Un **droit donné** (table `grants`) ouvre une Station rattachée à quelqu'un
//! d'autre que son propriétaire : une personne (son compte), une adresse sans
//! compte encore (l'invitation), et au lot 5 une organisation entière ou l'un
//! de ses groupes ([`super::orgs`]). Il porte des droits (Session, Prêt,
//! Partage, Administrer) et, s'il le faut, une échéance.
//!
//! - Ajouter une personne par son adresse : avec un compte vérifié, le droit
//!   est à elle tout de suite (l'ordinateur apparaît dans ses apps) ; sans
//!   compte, il attend l'adresse et passe au compte quand elle est vérifiée
//!   ([`Store::claim_invitations`]). Un e-mail le dit dans les deux cas.
//! - Les droits d'un compte sur une Station ([`Accounts::rights_on`]) : tout
//!   pour son propriétaire (et, au lot 5, pour les Propriétaires et
//!   Administrateurs de l'organisation propriétaire), sinon l'union des droits
//!   donnés pas échus. **Administrer** demande la double authentification
//!   (TOTP ou passkey) : sans elle, le droit ne compte pas.
//! - Gérer les accès d'un ordinateur (« Personnes ») : son propriétaire, les
//!   Propriétaires et Administrateurs de l'organisation propriétaire, et toute
//!   personne qui a le droit Administrer ([`Accounts::can_manage`]).

use anyhow::Result;
use kybernow_tunnel::access::{Right, Rights};
use rusqlite::{OptionalExtension, params};

use super::Accounts;
use super::station_groups::GROUP_GRANT_FOR;
use super::stations::StationRow;
use super::store::{Account, Store};

/// La migration 4 de la base (`super::store`) : les droits donnés.
///
/// Pas de clé étrangère vers `stations` : la migration 5 reconstruit cette
/// table (propriétaire organisation) ; les droits d'une Station partent avec
/// son détachement ([`Store::detach_station`]).
pub const MIGRATION: &str = r#"
    CREATE TABLE grants (
        id INTEGER PRIMARY KEY,
        station TEXT NOT NULL,
        account INTEGER REFERENCES accounts(id) ON DELETE CASCADE,
        email TEXT,
        rights TEXT NOT NULL,
        expires_at INTEGER,
        invited_by INTEGER REFERENCES accounts(id) ON DELETE SET NULL,
        created_at INTEGER NOT NULL
    );
    CREATE UNIQUE INDEX grants_person ON grants(station, account) WHERE account IS NOT NULL;
    CREATE UNIQUE INDEX grants_invite ON grants(station, email) WHERE email IS NOT NULL;
    CREATE INDEX grants_account ON grants(account);
    CREATE INDEX grants_email ON grants(email);
    ALTER TABLE device_stations ADD COLUMN seen_at INTEGER NOT NULL DEFAULT 0;
"#;

/// La migration 8 de la base (`super::store`) : l'envoi des invitations
/// (docs/comptes.md, « Invitations, ajout de membres et sessions »).
///
/// `sent_at` : le dernier envoi (création, remplacement ou « Renvoyer
/// l'invitation ») ; l'invitation expire [`INVITE_SECS`] après. `resent_at` :
/// le dernier « Renvoyer l'invitation » (une fois par heure au plus). Les
/// invitations déjà en attente reçoivent l'heure de la migration : 30 jours
/// pleins à partir de la mise à jour du relais, plutôt qu'une expiration
/// soudaine de celles envoyées il y a plus d'un mois.
pub const INVITES_MIGRATION: &str = r#"
    ALTER TABLE grants ADD COLUMN sent_at INTEGER;
    ALTER TABLE grants ADD COLUMN resent_at INTEGER;
    ALTER TABLE org_invites ADD COLUMN sent_at INTEGER;
    ALTER TABLE org_invites ADD COLUMN resent_at INTEGER;
    UPDATE grants SET sent_at = CAST(strftime('%s', 'now') AS INTEGER) WHERE email IS NOT NULL;
    UPDATE org_invites SET sent_at = CAST(strftime('%s', 'now') AS INTEGER);
"#;

/// Une invitation en attente expire après ce temps sans être reprise.
pub const INVITE_SECS: i64 = 30 * 86_400;
/// « Renvoyer l'invitation » : une fois par heure au plus, par invitation.
pub const RESEND_GAP_SECS: i64 = 3600;

/// Une invitation (d'ordinateur ou de famille ou d'équipe) en attente, pas expirée, en SQL :
/// `sent_at` dans les [`INVITE_SECS`] avant `now` (le paramètre donné).
pub(super) fn invite_live(alias: &str, now: &str) -> String {
    format!("COALESCE({alias}.sent_at, {alias}.created_at) > {now} - {INVITE_SECS}")
}

/// Ce qu'a donné « Renvoyer l'invitation ».
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Resend {
    /// Renvoyée : son adresse (l'e-mail est à envoyer).
    Done(String),
    /// Déjà renvoyée il y a moins d'une heure.
    TooSoon,
    /// Pas d'invitation en attente sous ce numéro.
    Missing,
}

/// Les suggestions d'adresses : ce nombre au plus.
pub const KNOWN_MAX: usize = 200;

/// Une venue sur une Station après ce temps sans jeton est une nouvelle ouverture (journal de l'organisation).
pub const VISIT_GAP_SECS: i64 = 3600;

/// Des droits en texte pour la base : `session,share`.
pub fn rights_text(rights: &Rights) -> String {
    rights.iter().map(Right::as_str).collect::<Vec<_>>().join(",")
}

/// Des droits lus de la base (un mot inconnu est ignoré).
pub fn parse_rights(text: &str) -> Rights {
    text.split(',').filter_map(|w| Right::parse(w.trim())).collect()
}

/// À qui un droit est donné.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Subject {
    /// Une personne, par son compte.
    Person { account: i64, name: String, email: String },
    /// Une adresse sans compte (vérifié) encore : l'invitation attend.
    Invite { email: String },
    /// Toute une organisation (lot 5).
    Org { org: i64, name: String },
    /// Un groupe d'une organisation (lot 5).
    Group { grp: i64, name: String, org_name: String },
}

/// Un droit donné sur une Station.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Grant {
    pub id: i64,
    pub station: String,
    pub subject: Subject,
    pub rights: Rights,
    pub expires_at: Option<i64>,
    pub created_at: i64,
    /// Une invitation : son dernier envoi (sinon `created_at`).
    pub sent_at: i64,
}

impl Grant {
    /// Échu à cette heure.
    pub fn expired(&self, now: i64) -> bool {
        self.expires_at.is_some_and(|e| e <= now)
    }

    /// Une invitation : quand elle expire (30 jours après son dernier envoi).
    pub fn invite_expires_at(&self) -> Option<i64> {
        matches!(self.subject, Subject::Invite { .. }).then_some(self.sent_at + INVITE_SECS)
    }
}

/// Un droit pas échu, vu depuis la base.
const LIVE: &str = "(g.expires_at IS NULL OR g.expires_at > ?2)";

impl Store {
    /// Donne (ou change) les droits d'une personne sur une Station ; une
    /// invitation de son adresse sur la même Station (expirée) part.
    pub fn grant_person(&self, station: &str, account: i64, rights: &Rights, expires_at: Option<i64>, by: i64, now: i64) -> Result<i64> {
        let text = rights_text(rights);
        self.conn.execute(
            "DELETE FROM grants WHERE station = ?1 AND email IS NOT NULL AND email = (SELECT email FROM accounts WHERE id = ?2)",
            params![station, account],
        )?;
        let found: Option<i64> =
            self.conn.query_row("SELECT id FROM grants WHERE station = ?1 AND account = ?2", params![station, account], |r| r.get(0)).optional()?;
        if let Some(id) = found {
            self.conn.execute("UPDATE grants SET rights = ?2, expires_at = ?3 WHERE id = ?1", params![id, text, expires_at])?;
            return Ok(id);
        }
        self.conn.execute(
            "INSERT INTO grants (station, account, rights, expires_at, invited_by, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![station, account, text, expires_at, by, now],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    /// Invite une adresse sans compte ; une invitation en attente pour la même
    /// adresse est remplacée (droits, échéance, et envoyée de nouveau : 30 jours).
    pub fn grant_invite(&self, station: &str, email: &str, rights: &Rights, expires_at: Option<i64>, by: i64, now: i64) -> Result<i64> {
        let text = rights_text(rights);
        let found: Option<i64> =
            self.conn.query_row("SELECT id FROM grants WHERE station = ?1 AND email = ?2", params![station, email], |r| r.get(0)).optional()?;
        if let Some(id) = found {
            self.conn.execute(
                "UPDATE grants SET rights = ?2, expires_at = ?3, invited_by = ?4, sent_at = ?5 WHERE id = ?1",
                params![id, text, expires_at, by, now],
            )?;
            return Ok(id);
        }
        self.conn.execute(
            "INSERT INTO grants (station, email, rights, expires_at, invited_by, created_at, sent_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?6)",
            params![station, email, text, expires_at, by, now],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    /// « Renvoyer l'invitation » d'un ordinateur : repart pour 30 jours (même
    /// expirée), une fois par heure au plus.
    pub fn resend_grant_invite(&self, station: &str, id: i64, now: i64) -> Result<Resend> {
        let found: Option<(Option<String>, Option<i64>)> = self
            .conn
            .query_row("SELECT email, resent_at FROM grants WHERE id = ?1 AND station = ?2", params![id, station], |r| Ok((r.get(0)?, r.get(1)?)))
            .optional()?;
        let Some((Some(email), resent_at)) = found else { return Ok(Resend::Missing) };
        if resent_at.is_some_and(|at| now - at < RESEND_GAP_SECS) {
            return Ok(Resend::TooSoon);
        }
        self.conn.execute("UPDATE grants SET sent_at = ?2, resent_at = ?2 WHERE id = ?1", params![id, now])?;
        Ok(Resend::Done(email))
    }

    /// Les droits donnés sur une Station : personnes, invitations, organisations, groupes.
    pub fn grants_of(&self, station: &str) -> Result<Vec<Grant>> {
        let mut stmt = self.conn.prepare(
            "SELECT g.id, g.station, g.account, a.name, a.email, g.email, g.org, o.name, g.grp, gr.name, og.name, g.rights, g.expires_at, g.created_at,
                    COALESCE(g.sent_at, g.created_at)
             FROM grants g
             LEFT JOIN accounts a ON a.id = g.account
             LEFT JOIN organizations o ON o.id = g.org
             LEFT JOIN org_groups gr ON gr.id = g.grp
             LEFT JOIN organizations og ON og.id = gr.org
             WHERE g.station = ?1
             ORDER BY g.account IS NULL, g.created_at, g.id",
        )?;
        let rows = stmt.query_map([station], |r| {
            let subject = match (r.get::<_, Option<i64>>(2)?, r.get::<_, Option<String>>(5)?, r.get::<_, Option<i64>>(6)?, r.get::<_, Option<i64>>(8)?) {
                (Some(account), _, _, _) => Subject::Person {
                    account,
                    name: r.get::<_, Option<String>>(3)?.unwrap_or_default(),
                    email: r.get::<_, Option<String>>(4)?.unwrap_or_default(),
                },
                (None, Some(email), _, _) => Subject::Invite { email },
                (None, None, _, Some(grp)) => Subject::Group {
                    grp,
                    name: r.get::<_, Option<String>>(9)?.unwrap_or_default(),
                    org_name: r.get::<_, Option<String>>(10)?.unwrap_or_default(),
                },
                (None, None, Some(org), None) => Subject::Org { org, name: r.get::<_, Option<String>>(7)?.unwrap_or_default() },
                (None, None, None, None) => Subject::Invite { email: String::new() },
            };
            Ok(Grant {
                id: r.get(0)?,
                station: r.get(1)?,
                subject,
                rights: parse_rights(&r.get::<_, String>(11)?),
                expires_at: r.get(12)?,
                created_at: r.get(13)?,
                sent_at: r.get(14)?,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    /// Un droit donné sur cette Station.
    pub fn grant(&self, station: &str, id: i64) -> Result<Option<Grant>> {
        Ok(self.grants_of(station)?.into_iter().find(|g| g.id == id))
    }

    /// Retire un droit donné sur cette Station ; faux s'il n'y est pas.
    pub fn delete_grant(&self, station: &str, id: i64) -> Result<bool> {
        Ok(self.conn.execute("DELETE FROM grants WHERE id = ?1 AND station = ?2", params![id, station])? == 1)
    }

    /// Les droits donnés à ce compte sur cette Station (à lui, à ses
    /// organisations, à ses groupes), pas échus ; et ceux donnés sur un groupe
    /// d'ordinateurs où elle est ([`super::station_groups`]).
    pub fn granted_rights(&self, account: i64, station: &str, now: i64) -> Result<Rights> {
        let mut stmt = self.conn.prepare(&format!(
            "SELECT g.rights FROM grants g WHERE g.station = ?3 AND {LIVE} AND (
                 g.account = ?1
                 OR (g.grp IS NULL AND g.org IN (SELECT org FROM org_members WHERE account = ?1))
                 OR g.grp IN (SELECT grp FROM group_members WHERE account = ?1))
             UNION ALL
             SELECT gg.rights FROM group_grants gg JOIN station_groups sg ON sg.id = gg.sgroup
                 JOIN station_group_members m ON m.sgroup = gg.sgroup
                 JOIN stations s ON s.id = m.station AND s.org = sg.org
             WHERE m.station = ?3 AND {GROUP_GRANT_FOR}"
        ))?;
        let rows = stmt.query_map(params![account, now, station], |r| r.get::<_, String>(0))?;
        let mut all = Vec::new();
        for text in rows {
            all.extend(parse_rights(&text?).iter());
        }
        Ok(Rights::new(all))
    }

    /// Les Stations qu'un compte peut voir : les siennes, celles des
    /// organisations qu'il administre, celles qu'on lui a ouvertes (une à une,
    /// ou par un groupe d'ordinateurs).
    pub fn visible_stations(&self, account: i64, now: i64) -> Result<Vec<StationRow>> {
        let mut stmt = self.conn.prepare(&format!(
            "SELECT {cols} FROM stations s WHERE s.account = ?1
               OR s.org IN (SELECT org FROM org_members WHERE account = ?1 AND role IN ('owner', 'admin'))
               OR s.id IN (SELECT g.station FROM grants g WHERE {LIVE} AND (
                    g.account = ?1
                    OR (g.grp IS NULL AND g.org IN (SELECT org FROM org_members WHERE account = ?1))
                    OR g.grp IN (SELECT grp FROM group_members WHERE account = ?1)))
               OR s.id IN (SELECT m.station FROM group_grants gg JOIN station_groups sg ON sg.id = gg.sgroup
                    JOIN station_group_members m ON m.sgroup = gg.sgroup
                    WHERE s.org = sg.org AND {GROUP_GRANT_FOR})
             ORDER BY s.account IS NOT ?1, s.name COLLATE NOCASE, s.id",
            cols = super::stations::STATION_COLS_S,
        ))?;
        let rows = stmt.query_map(params![account, now], super::stations::station_row)?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    /// L'adresse vient d'être vérifiée : les invitations qui l'attendaient
    /// passent au compte (ordinateurs, et au lot 5 organisations). Le nombre
    /// d'invitations reprises. Une invitation expirée (30 jours) ne passe pas :
    /// elle reste dans la liste jusqu'à ce qu'on la renvoie ou la retire.
    pub fn claim_invitations(&self, account: i64, now: i64) -> Result<usize> {
        let Some(email) = self.conn.query_row("SELECT email FROM accounts WHERE id = ?1", [account], |r| r.get::<_, String>(0)).optional()? else {
            return Ok(0);
        };
        let live = invite_live("grants", "?3");
        let tx = self.conn.unchecked_transaction()?;
        // Un droit déjà à ce compte sur la même Station l'emporte sur l'invitation.
        let n = tx.execute(
            &format!(
                "UPDATE grants SET account = ?1, email = NULL WHERE email = ?2 AND {live}
                 AND NOT EXISTS (SELECT 1 FROM grants g2 WHERE g2.station = grants.station AND g2.account = ?1)"
            ),
            params![account, email, now],
        )?;
        let live2 = invite_live("grants", "?2");
        tx.execute(&format!("DELETE FROM grants WHERE email = ?1 AND {live2}"), params![email, now])?;
        let m = super::orgs::claim_org_invites(&tx, account, &email, now)?;
        tx.commit()?;
        Ok(n + m)
    }

    /// Les personnes que ce compte connaît déjà, `(nom, adresse)` par nom
    /// (les suggestions de « Ajouter une personne » et « Inviter un membre ») :
    /// les membres de ses familles et équipes, et ceux à qui il a ouvert un
    /// ordinateur (les siens, ou par lui), comptes ou invitations. Personne
    /// d'autre : la liste ne montre jamais un inconnu.
    pub fn known_people(&self, account: i64) -> Result<Vec<(String, String)>> {
        let mut stmt = self.conn.prepare(
            "SELECT a.name, a.email FROM org_members m JOIN org_members me ON me.org = m.org AND me.account = ?1
                 JOIN accounts a ON a.id = m.account WHERE m.account <> ?1 AND a.deleted_at IS NULL
             UNION
             SELECT a.name, a.email FROM grants g JOIN accounts a ON a.id = g.account
                 WHERE g.account <> ?1 AND a.deleted_at IS NULL AND (g.invited_by = ?1 OR g.station IN (SELECT id FROM stations WHERE account = ?1))
             UNION
             SELECT '', g.email FROM grants g
                 WHERE g.email IS NOT NULL AND (g.invited_by = ?1 OR g.station IN (SELECT id FROM stations WHERE account = ?1))
             UNION
             SELECT '', i.email FROM org_invites i JOIN org_members me ON me.org = i.org AND me.account = ?1",
        )?;
        let rows: Vec<(String, String)> = stmt.query_map([account], |r| Ok((r.get(0)?, r.get(1)?)))?.collect::<rusqlite::Result<_>>()?;
        // Une adresse une fois, avec son nom s'il est connu.
        let mut by_email: std::collections::BTreeMap<String, String> = std::collections::BTreeMap::new();
        for (name, email) in rows {
            let entry = by_email.entry(email).or_default();
            if entry.is_empty() {
                *entry = name;
            }
        }
        let mut out: Vec<(String, String)> = by_email.into_iter().map(|(email, name)| (name, email)).collect();
        out.sort_by(|a, b| (a.0.is_empty(), a.0.to_lowercase(), &a.1).cmp(&(b.0.is_empty(), b.0.to_lowercase(), &b.1)));
        out.truncate(KNOWN_MAX);
        Ok(out)
    }

    /// Note la venue d'un appareil sur une Station : `(première fois, nouvelle ouverture)`
    /// (une ouverture : plus de [`VISIT_GAP_SECS`] sans jeton depuis la précédente).
    pub fn visit(&self, device: i64, station: &str, now: i64) -> Result<(bool, bool)> {
        if self.first_visit(device, station, now)? {
            self.conn.execute("UPDATE device_stations SET seen_at = ?3 WHERE device = ?1 AND station = ?2", params![device, station, now])?;
            return Ok((true, true));
        }
        let seen: i64 =
            self.conn.query_row("SELECT seen_at FROM device_stations WHERE device = ?1 AND station = ?2", params![device, station], |r| r.get(0))?;
        self.conn.execute("UPDATE device_stations SET seen_at = ?3 WHERE device = ?1 AND station = ?2", params![device, station, now])?;
        Ok((false, now - seen > VISIT_GAP_SECS))
    }

    /// La double authentification du compte : TOTP active, ou une passkey (qui la vaut).
    pub fn two_factor(&self, account: i64) -> Result<bool> {
        let n: i64 = self.conn.query_row(
            "SELECT (SELECT COUNT(*) FROM totp WHERE account = ?1 AND activated_at IS NOT NULL) + (SELECT COUNT(*) FROM passkeys WHERE account = ?1)",
            [account],
            |r| r.get(0),
        )?;
        Ok(n > 0)
    }
}

/// Ce qu'un compte peut faire d'un ordinateur.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Standing {
    /// Ses droits (Administrer seulement avec la double authentification).
    pub rights: Rights,
    /// Il en est le propriétaire (personne).
    pub mine: bool,
    /// Il gère ses accès (« Personnes ») : propriétaire, Propriétaire ou
    /// Administrateur de l'organisation propriétaire, ou droit Administrer.
    pub manage: bool,
    /// Le droit Administrer lui est donné, mais il n'a pas la double authentification.
    pub admin_needs_2fa: bool,
}

impl Accounts {
    /// Ce qu'un compte peut faire de cette Station (`None` : rien).
    pub fn standing(&self, account: &Account, station: &StationRow) -> Option<Standing> {
        let now = self.now();
        let db = self.db();
        let mine = station.account == Some(account.id);
        let two_factor = db.two_factor(account.id).unwrap_or(false);
        // Lot 5 : les Propriétaires et Administrateurs de l'organisation propriétaire ont tout, avec la double authentification.
        let org_admin = station.org.is_some_and(|o| db.org_role(o, account.id).ok().flatten().is_some_and(|r| r.manages())) && two_factor;
        let mut rights = if mine || org_admin { Rights::new(Right::ALL) } else { db.granted_rights(account.id, &station.id, now).unwrap_or_default() };
        let admin_needs_2fa = rights.has(Right::Admin) && !two_factor;
        if admin_needs_2fa {
            rights = rights.iter().filter(|r| *r != Right::Admin).collect();
        }
        if rights.is_empty() {
            return None;
        }
        let manage = mine || org_admin || rights.has(Right::Admin);
        Some(Standing { rights, mine, manage, admin_needs_2fa })
    }

    /// Les droits d'un compte sur une Station rattachée.
    pub fn rights_on(&self, account: &Account, station: &StationRow) -> Option<Rights> {
        self.standing(account, station).map(|s| s.rights)
    }

    /// Ce compte gère-t-il les accès de cette Station ?
    pub fn can_manage(&self, account: &Account, station: &StationRow) -> bool {
        self.standing(account, station).is_some_and(|s| s.manage)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rights(words: &[Right]) -> Rights {
        Rights::new(words.iter().copied())
    }

    #[test]
    fn droits_en_texte() {
        assert_eq!(rights_text(&rights(&[Right::Share, Right::Session])), "session,share");
        assert_eq!(parse_rights("session, admin,plus-tard"), rights(&[Right::Session, Right::Admin]));
        assert!(parse_rights("").is_empty());
    }

    #[test]
    fn personnes_invitations_et_echeances() {
        let s = Store::memory(0).unwrap();
        let a = s.create_account("a@exemple.org", "A", "h", "fr", Some(0), 0).unwrap().unwrap();
        let b = s.create_account("b@exemple.org", "B", "h", "fr", Some(0), 0).unwrap().unwrap();
        s.attach_station("st1", "PC", a, a, 10).unwrap();
        // Une personne : ses droits, changés ensuite.
        let g = s.grant_person("st1", b, &rights(&[Right::Session]), None, a, 20).unwrap();
        assert_eq!(s.grant_person("st1", b, &rights(&[Right::Share]), Some(100), a, 30).unwrap(), g, "le même droit, changé");
        assert_eq!(s.granted_rights(b, "st1", 50).unwrap(), rights(&[Right::Share]));
        assert!(s.granted_rights(b, "st1", 100).unwrap().is_empty(), "échu");
        assert_eq!(s.visible_stations(b, 50).unwrap().len(), 1);
        assert!(s.visible_stations(b, 100).unwrap().is_empty());
        assert_eq!(s.visible_stations(a, 100).unwrap().len(), 1, "la sienne");
        // Une adresse sans compte : reprise à la vérification.
        s.grant_invite("st1", "c@exemple.org", &rights(&[Right::Lend]), None, a, 40).unwrap();
        let list = s.grants_of("st1").unwrap();
        assert_eq!(list.len(), 2);
        assert!(matches!(&list[1].subject, Subject::Invite { email } if email == "c@exemple.org"));
        let c = s.create_account("c@exemple.org", "C", "h", "fr", None, 50).unwrap().unwrap();
        assert!(s.granted_rights(c, "st1", 60).unwrap().is_empty(), "pas encore vérifiée");
        assert_eq!(s.claim_invitations(c, 50).unwrap(), 1);
        assert_eq!(s.granted_rights(c, "st1", 60).unwrap(), rights(&[Right::Lend]));
        assert!(s.grants_of("st1").unwrap().iter().all(|g| !matches!(g.subject, Subject::Invite { .. })));
        // Retirer ; détacher efface les droits de la Station.
        assert!(!s.delete_grant("st2", g).unwrap(), "pas sur cette Station");
        assert!(s.delete_grant("st1", g).unwrap());
        assert!(s.granted_rights(b, "st1", 50).unwrap().is_empty());
        assert!(s.detach_station("st1", None).unwrap());
        assert!(s.grants_of("st1").unwrap().is_empty());
    }

    #[test]
    fn invitations_expirees_et_renvoyees() {
        let s = Store::memory(0).unwrap();
        let a = s.create_account("a@exemple.org", "A", "h", "fr", Some(0), 0).unwrap().unwrap();
        s.attach_station("st1", "PC", a, a, 10).unwrap();
        s.attach_station("st2", "PC 2", a, a, 10).unwrap();
        let t0 = 1_000_000;
        let g1 = s.grant_invite("st1", "c@exemple.org", &rights(&[Right::Lend]), None, a, t0).unwrap();
        s.grant_invite("st2", "c@exemple.org", &rights(&[Right::Session]), None, a, t0 + 20 * 86_400).unwrap();
        let g = s.grant("st1", g1).unwrap().unwrap();
        assert_eq!((g.sent_at, g.invite_expires_at()), (t0, Some(t0 + INVITE_SECS)));
        // Remplacée, pas doublée : mêmes numéro et adresse, nouveaux droits, envoyée de nouveau.
        assert_eq!(s.grant_invite("st1", "c@exemple.org", &rights(&[Right::Share]), None, a, t0 + 60).unwrap(), g1);
        let g = s.grant("st1", g1).unwrap().unwrap();
        assert_eq!((g.rights.clone(), g.sent_at, s.grants_of("st1").unwrap().len()), (rights(&[Right::Share]), t0 + 60, 1));
        // 31 jours plus tard, C vérifie son adresse : seule l'invitation encore valable passe ; l'autre reste.
        let c = s.create_account("c@exemple.org", "C", "h", "fr", None, t0).unwrap().unwrap();
        let later = t0 + 31 * 86_400;
        assert_eq!(s.claim_invitations(c, later).unwrap(), 1);
        assert!(s.granted_rights(c, "st1", later).unwrap().is_empty(), "expirée : pas reprise");
        assert_eq!(s.granted_rights(c, "st2", later).unwrap(), rights(&[Right::Session]));
        assert!(matches!(&s.grant("st1", g1).unwrap().unwrap().subject, Subject::Invite { email } if email == "c@exemple.org"), "toujours listée");
        // Renvoyer : 30 jours de nouveau, une fois par heure ; une personne ne se renvoie pas.
        assert_eq!(s.resend_grant_invite("st1", g1, later).unwrap(), Resend::Done("c@exemple.org".into()));
        assert_eq!(s.resend_grant_invite("st1", g1, later + RESEND_GAP_SECS - 1).unwrap(), Resend::TooSoon);
        assert_eq!(s.resend_grant_invite("st2", g1, later + RESEND_GAP_SECS).unwrap(), Resend::Missing, "pas sur cet ordinateur");
        assert_eq!(s.resend_grant_invite("st1", g1, later + RESEND_GAP_SECS).unwrap(), Resend::Done("c@exemple.org".into()));
        assert_eq!(s.grant("st1", g1).unwrap().unwrap().sent_at, later + RESEND_GAP_SECS);
        let person = s.grants_of("st2").unwrap()[0].id;
        assert_eq!(s.resend_grant_invite("st2", person, later + 2 * RESEND_GAP_SECS).unwrap(), Resend::Missing);
        assert_eq!(s.claim_invitations(c, later + RESEND_GAP_SECS).unwrap(), 1, "renvoyée : elle passe");
        assert_eq!(s.granted_rights(c, "st1", later + RESEND_GAP_SECS).unwrap(), rights(&[Right::Share]));
        // Une invitation expirée de la même adresse part quand on donne le droit au compte.
        s.grant_invite("st1", "d@exemple.org", &rights(&[Right::Lend]), None, a, t0).unwrap();
        let d = s.create_account("d@exemple.org", "D", "h", "fr", Some(later), later).unwrap().unwrap();
        s.grant_person("st1", d, &rights(&[Right::Session]), None, a, later).unwrap();
        assert!(s.grants_of("st1").unwrap().iter().all(|g| !matches!(g.subject, Subject::Invite { .. })));
    }

    #[test]
    fn personnes_connues_et_pas_d_inconnu() {
        let s = Store::memory(0).unwrap();
        let a = s.create_account("a@exemple.org", "Ana", "h", "fr", Some(0), 0).unwrap().unwrap();
        let b = s.create_account("b@exemple.org", "Bob", "h", "fr", Some(0), 0).unwrap().unwrap();
        let c = s.create_account("c@exemple.org", "Cléo", "h", "fr", Some(0), 0).unwrap().unwrap();
        let x = s.create_account("x@exemple.org", "Inconnu", "h", "fr", Some(0), 0).unwrap().unwrap();
        let org = s.create_org("Famille", super::super::orgs::Kind::Family, a, 0).unwrap();
        s.set_member(org.id, b, super::super::orgs::Role::Member, 0).unwrap();
        s.invite_member(org.id, "paul@exemple.org", super::super::orgs::Role::Member, a, 0).unwrap();
        s.attach_station("st1", "PC", a, a, 0).unwrap();
        s.grant_person("st1", c, &rights(&[Right::Session]), None, a, 0).unwrap();
        s.grant_invite("st1", "sam@exemple.org", &rights(&[Right::Session]), None, a, 0).unwrap();
        // L'inconnu a son propre ordinateur, ouvert à Bob : Ana ne le voit pas.
        s.attach_station("st9", "Autre", x, x, 0).unwrap();
        s.grant_person("st9", b, &rights(&[Right::Session]), None, x, 0).unwrap();
        let known = s.known_people(a).unwrap();
        let emails: Vec<&str> = known.iter().map(|(_, e)| e.as_str()).collect();
        assert_eq!(emails, vec!["b@exemple.org", "c@exemple.org", "paul@exemple.org", "sam@exemple.org"], "les noms d'abord, puis les adresses seules");
        assert_eq!(known[0].0, "Bob");
        // Bob : sa famille (Ana, l'invitation de Paul), pas l'inconnu qui lui a ouvert un ordinateur.
        let emails: Vec<String> = s.known_people(b).unwrap().into_iter().map(|(_, e)| e).collect();
        assert_eq!(emails, vec!["a@exemple.org".to_string(), "paul@exemple.org".to_string()]);
        // L'inconnu connaît Bob (il le lui a ouvert), pas Ana.
        let emails: Vec<String> = s.known_people(x).unwrap().into_iter().map(|(_, e)| e).collect();
        assert_eq!(emails, vec!["b@exemple.org".to_string()]);
    }

    #[test]
    fn double_authentification_totp_ou_passkey() {
        let s = Store::memory(0).unwrap();
        let a = s.create_account("a@exemple.org", "A", "h", "fr", Some(0), 0).unwrap().unwrap();
        assert!(!s.two_factor(a).unwrap());
        s.set_pending_totp(a, b"x").unwrap();
        assert!(!s.two_factor(a).unwrap(), "en attente : pas encore");
        s.activate_totp(a, 1, 10).unwrap();
        assert!(s.two_factor(a).unwrap());
        s.delete_totp(a).unwrap();
        let key = super::super::store::NewPasskey { credential: b"c".to_vec(), public_key: b"k".to_vec(), alg: -7, counter: 0 };
        s.add_passkey(a, &key, "Clé", "", 20).unwrap();
        assert!(s.two_factor(a).unwrap());
    }
}
