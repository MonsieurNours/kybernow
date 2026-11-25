//! Les groupes d'ordinateurs (docs/comptes.md, « Regarder, liens d'accès et
//! groupes d'ordinateurs ») : une liste d'ordinateurs d'une famille ou d'une
//! équipe, choisis à la main (« Postes de l'atelier »).
//!
//! - Un ordinateur peut être dans plusieurs groupes ; seulement ceux de la
//!   famille ou de l'équipe. Il quitte ses groupes quand il la quitte
//!   (détaché, ou rattaché à un autre propriétaire : [`Store::detach_station`]).
//! - **Ouvrir un groupe** (table `group_grants`) : à toute la famille ou
//!   l'équipe, à un de ses groupes de personnes, ou à un de ses membres, avec
//!   des droits et une échéance facultative. Ces droits valent pour chaque
//!   ordinateur du groupe, présent et à venir : ils comptent dans les deux
//!   requêtes des droits ([`Store::granted_rights`], [`Store::visible_stations`]),
//!   en plus des droits donnés ordinateur par ordinateur (`grants`).
//! - Un droit donné à un membre ne vaut que tant qu'il est membre.
//! - Gérer : les Propriétaires et Administrateurs, avec la double authentification.
//!
//! Les tables sont dans la migration 7 ([`super::links::MIGRATION`]).

use anyhow::Result;
use kybernow_tunnel::access::Rights;
use rusqlite::{OptionalExtension, params};

use super::people::{Subject, parse_rights, rights_text};
use super::store::Store;

/// Les droits donnés sur un groupe d'ordinateurs qui valent pour `?1` (compte)
/// sur un ordinateur `m.station`, pas échus à `?2` : à utiliser après
/// `FROM group_grants gg JOIN station_groups sg ON sg.id = gg.sgroup
/// JOIN station_group_members m ON m.sgroup = gg.sgroup`.
pub(super) const GROUP_GRANT_FOR: &str = "(gg.expires_at IS NULL OR gg.expires_at > ?2) AND (
       (gg.account = ?1 AND EXISTS (SELECT 1 FROM org_members om WHERE om.org = sg.org AND om.account = ?1))
       OR (gg.account IS NULL AND gg.grp IS NULL AND gg.org IN (SELECT org FROM org_members WHERE account = ?1))
       OR gg.grp IN (SELECT grp FROM group_members WHERE account = ?1))";

/// À qui ouvrir un groupe d'ordinateurs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GroupTarget {
    /// Toute la famille ou l'équipe.
    Org(i64),
    /// Un de ses groupes de personnes.
    People(i64),
    /// Un de ses membres.
    Member(i64),
}

impl GroupTarget {
    fn column(self) -> (&'static str, i64) {
        match self {
            GroupTarget::Org(id) => ("org", id),
            GroupTarget::People(id) => ("grp", id),
            GroupTarget::Member(id) => ("account", id),
        }
    }
}

/// Un droit donné sur un groupe d'ordinateurs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GroupGrant {
    pub id: i64,
    pub sgroup: i64,
    pub group_name: String,
    pub subject: Subject,
    pub rights: Rights,
    pub expires_at: Option<i64>,
}

impl GroupGrant {
    pub fn expired(&self, now: i64) -> bool {
        self.expires_at.is_some_and(|e| e <= now)
    }
}

/// Un groupe d'ordinateurs, ses ordinateurs `(id, nom)` et ses droits donnés.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StationGroup {
    pub id: i64,
    pub name: String,
    pub stations: Vec<(String, String)>,
    pub grants: Vec<GroupGrant>,
}

const GRANT_COLS: &str = "gg.id, gg.sgroup, sg.name, gg.account, a.name, a.email, gg.org, o.name, gg.grp, pg.name, po.name, gg.rights, gg.expires_at
     FROM group_grants gg JOIN station_groups sg ON sg.id = gg.sgroup
     LEFT JOIN accounts a ON a.id = gg.account
     LEFT JOIN organizations o ON o.id = gg.org
     LEFT JOIN org_groups pg ON pg.id = gg.grp
     LEFT JOIN organizations po ON po.id = pg.org";

fn grant_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<GroupGrant> {
    let text = |i: usize| -> rusqlite::Result<String> { Ok(r.get::<_, Option<String>>(i)?.unwrap_or_default()) };
    let subject = match (r.get::<_, Option<i64>>(3)?, r.get::<_, Option<i64>>(6)?, r.get::<_, Option<i64>>(8)?) {
        (Some(account), _, _) => Subject::Person { account, name: text(4)?, email: text(5)? },
        (None, _, Some(grp)) => Subject::Group { grp, name: text(9)?, org_name: text(10)? },
        (None, Some(org), None) => Subject::Org { org, name: text(7)? },
        (None, None, None) => Subject::Invite { email: String::new() },
    };
    Ok(GroupGrant {
        id: r.get(0)?,
        sgroup: r.get(1)?,
        group_name: r.get(2)?,
        subject,
        rights: parse_rights(&r.get::<_, String>(11)?),
        expires_at: r.get(12)?,
    })
}

impl Store {
    /// Crée un groupe d'ordinateurs ; `None` si le nom est déjà pris.
    pub fn create_station_group(&self, org: i64, name: &str, now: i64) -> Result<Option<i64>> {
        let n = self
            .conn
            .execute("INSERT INTO station_groups (org, name, created_at) VALUES (?1, ?2, ?3) ON CONFLICT(org, name) DO NOTHING", params![org, name, now])?;
        Ok((n == 1).then(|| self.conn.last_insert_rowid()))
    }

    /// Le nom d'un groupe d'ordinateurs de cette organisation.
    pub fn station_group_name(&self, org: i64, sgroup: i64) -> Result<Option<String>> {
        Ok(self.conn.query_row("SELECT name FROM station_groups WHERE id = ?1 AND org = ?2", params![sgroup, org], |r| r.get(0)).optional()?)
    }

    /// Supprime un groupe d'ordinateurs (ses droits donnés partent avec lui).
    pub fn delete_station_group(&self, org: i64, sgroup: i64) -> Result<bool> {
        Ok(self.conn.execute("DELETE FROM station_groups WHERE id = ?1 AND org = ?2", params![sgroup, org])? == 1)
    }

    /// Les groupes d'ordinateurs d'une organisation, par nom.
    pub fn station_groups(&self, org: i64) -> Result<Vec<StationGroup>> {
        let list: Vec<(i64, String)> = {
            let mut stmt = self.conn.prepare("SELECT id, name FROM station_groups WHERE org = ?1 ORDER BY name COLLATE NOCASE, id")?;
            stmt.query_map([org], |r| Ok((r.get(0)?, r.get(1)?)))?.collect::<rusqlite::Result<_>>()?
        };
        let mut out = Vec::with_capacity(list.len());
        for (id, name) in list {
            let stations = {
                let mut stmt = self.conn.prepare(
                    "SELECT s.id, s.name FROM station_group_members m JOIN stations s ON s.id = m.station
                     WHERE m.sgroup = ?1 ORDER BY s.name COLLATE NOCASE, s.id",
                )?;
                stmt.query_map([id], |r| Ok((r.get(0)?, r.get(1)?)))?.collect::<rusqlite::Result<_>>()?
            };
            out.push(StationGroup { id, name, stations, grants: self.group_grants(id)? });
        }
        Ok(out)
    }

    /// Met un ordinateur de l'organisation dans un de ses groupes (`true`) ou l'en
    /// retire ; faux si rien n'a changé (ou si l'ordinateur n'est pas à elle).
    pub fn set_station_group_member(&self, org: i64, sgroup: i64, station: &str, inside: bool) -> Result<bool> {
        if self.station_group_name(org, sgroup)?.is_none() {
            return Ok(false);
        }
        let n = if inside {
            let ours: bool = self.conn.query_row("SELECT COUNT(*) FROM stations WHERE id = ?1 AND org = ?2", params![station, org], |r| r.get::<_, i64>(0))? == 1;
            if !ours {
                return Ok(false);
            }
            self.conn.execute("INSERT OR IGNORE INTO station_group_members (sgroup, station) VALUES (?1, ?2)", params![sgroup, station])?
        } else {
            self.conn.execute("DELETE FROM station_group_members WHERE sgroup = ?1 AND station = ?2", params![sgroup, station])?
        };
        Ok(n == 1)
    }

    /// Ouvre un groupe d'ordinateurs (ou change les droits et l'échéance déjà donnés à cette cible).
    pub fn grant_station_group(&self, sgroup: i64, target: GroupTarget, rights: &Rights, expires_at: Option<i64>, by: i64, now: i64) -> Result<i64> {
        let (column, id) = target.column();
        let text = rights_text(rights);
        let found: Option<i64> =
            self.conn.query_row(&format!("SELECT id FROM group_grants WHERE sgroup = ?1 AND {column} = ?2"), params![sgroup, id], |r| r.get(0)).optional()?;
        if let Some(g) = found {
            self.conn.execute("UPDATE group_grants SET rights = ?2, expires_at = ?3 WHERE id = ?1", params![g, text, expires_at])?;
            return Ok(g);
        }
        self.conn.execute(
            &format!("INSERT INTO group_grants (sgroup, {column}, rights, expires_at, invited_by, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)"),
            params![sgroup, id, text, expires_at, (by > 0).then_some(by), now],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    /// Les droits donnés sur un groupe d'ordinateurs.
    pub fn group_grants(&self, sgroup: i64) -> Result<Vec<GroupGrant>> {
        let mut stmt = self.conn.prepare(&format!("SELECT {GRANT_COLS} WHERE gg.sgroup = ?1 ORDER BY gg.created_at, gg.id"))?;
        let rows = stmt.query_map([sgroup], grant_row)?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    /// Retire un droit donné sur un groupe d'ordinateurs de cette organisation ; le droit, s'il y était.
    pub fn delete_group_grant(&self, org: i64, id: i64) -> Result<Option<GroupGrant>> {
        let grant = self.conn.query_row(&format!("SELECT {GRANT_COLS} WHERE gg.id = ?1 AND sg.org = ?2"), params![id, org], grant_row).optional()?;
        if grant.is_some() {
            self.conn.execute("DELETE FROM group_grants WHERE id = ?1", [id])?;
        }
        Ok(grant)
    }

    /// Les droits venus des groupes d'ordinateurs où est cette Station (« Par le groupe {groupe} »).
    pub fn group_grants_on(&self, station: &str) -> Result<Vec<GroupGrant>> {
        let mut stmt = self.conn.prepare(&format!(
            "SELECT {GRANT_COLS} JOIN station_group_members m ON m.sgroup = gg.sgroup
             JOIN stations s ON s.id = m.station AND s.org = sg.org
             WHERE m.station = ?1 ORDER BY sg.name COLLATE NOCASE, gg.created_at, gg.id"
        ))?;
        let rows = stmt.query_map([station], grant_row)?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    /// Les groupes d'ordinateurs d'une organisation, `(id, nom)` (le choix « Ranger dans le groupe »).
    pub fn station_group_names(&self, org: i64) -> Result<Vec<(i64, String)>> {
        let mut stmt = self.conn.prepare("SELECT id, name FROM station_groups WHERE org = ?1 ORDER BY name COLLATE NOCASE, id")?;
        let rows = stmt.query_map([org], |r| Ok((r.get(0)?, r.get(1)?)))?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::accounts::orgs::{Kind, Role};
    use kybernow_tunnel::access::Right;

    fn rights(words: &[Right]) -> Rights {
        Rights::new(words.iter().copied())
    }

    #[test]
    fn droits_d_un_groupe_presents_et_a_venir() {
        let s = Store::memory(0).unwrap();
        let a = s.create_account("a@exemple.org", "A", "h", "fr", Some(0), 0).unwrap().unwrap();
        let b = s.create_account("b@exemple.org", "B", "h", "fr", Some(0), 0).unwrap().unwrap();
        let c = s.create_account("c@exemple.org", "C", "h", "fr", Some(0), 0).unwrap().unwrap();
        let org = s.create_org("Atelier", Kind::Team, a, 10).unwrap();
        s.set_member(org.id, b, Role::Member, 10).unwrap();
        s.set_member(org.id, c, Role::Member, 10).unwrap();
        let people = s.create_group(org.id, "Graphistes", 10).unwrap().unwrap();
        s.set_group_member(org.id, people, c, true).unwrap();
        s.attach_station_to_org("st1", "Poste 1", org.id, Some(a), None, 20).unwrap();
        s.attach_station_to_org("st2", "Poste 2", org.id, Some(a), None, 20).unwrap();
        s.attach_station("perso", "PC de A", a, a, 20).unwrap();
        let g = s.create_station_group(org.id, "Postes de l'atelier", 30).unwrap().unwrap();
        assert!(s.create_station_group(org.id, "Postes de l'atelier", 30).unwrap().is_none(), "nom déjà pris");
        assert!(s.set_station_group_member(org.id, g, "st1", true).unwrap());
        assert!(!s.set_station_group_member(org.id, g, "st1", true).unwrap(), "déjà dedans");
        assert!(!s.set_station_group_member(org.id, g, "perso", true).unwrap(), "pas un ordinateur de l'équipe");
        // Toute l'équipe : Regarder ; les Graphistes : Session jusqu'à 100 ; B seul : Prêt.
        s.grant_station_group(g, GroupTarget::Org(org.id), &rights(&[Right::View]), None, a, 40).unwrap();
        s.grant_station_group(g, GroupTarget::People(people), &rights(&[Right::Session]), Some(100), a, 40).unwrap();
        let gb = s.grant_station_group(g, GroupTarget::Member(b), &rights(&[Right::Lend]), None, a, 40).unwrap();
        assert_eq!(s.granted_rights(b, "st1", 50).unwrap(), rights(&[Right::View, Right::Lend]));
        assert_eq!(s.granted_rights(c, "st1", 50).unwrap(), rights(&[Right::View, Right::Session]));
        assert_eq!(s.granted_rights(c, "st1", 100).unwrap(), rights(&[Right::View]), "échu");
        assert!(s.granted_rights(b, "st2", 50).unwrap().is_empty(), "pas dans le groupe");
        assert_eq!(s.visible_stations(b, 50).unwrap().len(), 1);
        // En plus des droits donnés ordinateur par ordinateur.
        s.grant_person("st1", b, &rights(&[Right::Share]), None, a, 45).unwrap();
        assert_eq!(s.granted_rights(b, "st1", 50).unwrap(), rights(&[Right::View, Right::Lend, Right::Share]));
        // Un ordinateur ajouté ensuite : les mêmes droits.
        s.set_station_group_member(org.id, g, "st2", true).unwrap();
        assert_eq!(s.granted_rights(b, "st2", 50).unwrap(), rights(&[Right::View, Right::Lend]));
        assert_eq!(s.visible_stations(b, 50).unwrap().len(), 2);
        assert_eq!(s.group_grants_on("st2").unwrap().len(), 3);
        // B quitte l'équipe : plus rien par le groupe (même ce qui était donné à lui seul).
        s.remove_member(org.id, b).unwrap();
        assert!(s.granted_rights(b, "st2", 50).unwrap().is_empty());
        s.set_member(org.id, b, Role::Member, 60).unwrap();
        assert!(s.delete_group_grant(org.id, gb).unwrap().is_some());
        assert_eq!(s.granted_rights(b, "st2", 70).unwrap(), rights(&[Right::View]));
        // L'ordinateur détaché, ou rattaché à quelqu'un d'autre, quitte ses groupes.
        s.detach_station("st2", None).unwrap();
        s.attach_station_to_org("st2", "Poste 2", org.id, Some(a), None, 80).unwrap();
        assert!(s.granted_rights(b, "st2", 90).unwrap().is_empty());
        s.attach_station("st1", "Poste 1", a, a, 90).unwrap();
        assert!(s.station_groups(org.id).unwrap()[0].stations.is_empty());
        assert!(s.group_grants_on("st1").unwrap().is_empty());
        // Supprimer le groupe : ses droits partent avec lui.
        s.attach_station_to_org("st1", "Poste 1", org.id, Some(a), None, 100).unwrap();
        s.set_station_group_member(org.id, g, "st1", true).unwrap();
        assert!(!s.granted_rights(c, "st1", 95).unwrap().is_empty());
        assert!(s.delete_station_group(org.id, g).unwrap());
        assert!(s.granted_rights(c, "st1", 95).unwrap().is_empty());
        assert_eq!(s.conn.query_row("SELECT COUNT(*) FROM group_grants", [], |r| r.get::<_, i64>(0)).unwrap(), 0);
    }
}
