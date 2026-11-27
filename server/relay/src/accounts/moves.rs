//! Déplacer un ordinateur (docs/comptes.md, « Réglages synchronisés et
//! déplacer un ordinateur ») : vers soi, ou vers une famille ou une équipe que
//! l'on gère, depuis soi ou depuis une famille ou une équipe que l'on gère,
//! avec la double authentification. C'est l'autre façon, mesurée, de changer
//! de propriétaire : rattacher de nouveau à un autre propriétaire efface tous
//! les droits donnés, déplacer montre d'abord « Ce qui change ».
//!
//! Ce qui part :
//! - les droits donnés à des familles, des équipes ou des groupes, sauf ceux
//!   donnés à la destination elle-même (ou à l'un de ses groupes) ;
//! - les groupes d'ordinateurs où il était, et donc les accès qui en venaient ;
//! - les liens d'accès en attente (révoqués) ;
//! - les droits donnés une par une à des personnes (et les invitations), si
//!   la case « Garder les accès donnés à des personnes » est décochée ; ceux du
//!   nouveau propriétaire, s'il en avait, qui a désormais tout ;
//! - les premières venues des appareils (le nouveau propriétaire est prévenu
//!   de chaque appareil, comme après un rattachement) ;
//! - les réglages synchronisés des comptes qui n'y ont plus droit.
//!
//! Les deux journaux le notent (famille ou équipe ; journal du compte pour une
//! personne) et la Station l'apprend comme un rattachement (`attached`).

use std::collections::BTreeSet;

use anyhow::Result;
use rusqlite::params;
use serde_json::json;

use super::Accounts;
use super::orgs::Org;
use super::people::Subject;
use super::stations::StationRow;
use super::store::{Account, Store};

/// Où déplacer un ordinateur.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Dest {
    /// Vers soi.
    Me,
    /// Vers une famille ou une équipe que l'on gère.
    Org(Org),
}

impl Dest {
    fn org_id(&self) -> Option<i64> {
        match self {
            Dest::Me => None,
            Dest::Org(o) => Some(o.id),
        }
    }
}

/// « Ce qui change », calculé d'après la base.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MovePlan {
    /// « Perdent l'accès : » les familles, équipes et groupes, et les accès venus d'un groupe d'ordinateurs.
    pub losing: Vec<String>,
    /// « Quitte les groupes d'ordinateurs : »
    pub station_groups: Vec<String>,
    /// « Liens en attente annulés : »
    pub pending_links: i64,
    /// Les personnes (et adresses invitées) dont l'accès reste si la case est cochée.
    pub people: Vec<String>,
}

/// Le nom d'un sujet de droit, comme la page d'un ordinateur l'écrit.
fn subject_name(subject: &Subject) -> String {
    match subject {
        Subject::Person { name, email, .. } => super::store::shown_name(name, email),
        Subject::Invite { email } => email.clone(),
        Subject::Org { name, .. } => name.clone(),
        Subject::Group { name, org_name, .. } => format!("{org_name} › {name}"),
    }
}

impl Store {
    /// Le droit donné à cette organisation, ou à l'un de ses groupes ?
    fn grant_to_org(&self, subject: &Subject, org: Option<i64>) -> bool {
        let Some(org) = org else { return false };
        match subject {
            Subject::Org { org: o, .. } => *o == org,
            Subject::Group { grp, .. } => self.group_name(org, *grp).ok().flatten().is_some(),
            _ => false,
        }
    }

    /// « Ce qui change » si cette Station va à `to` (`to_account` : la personne qui la reçoit).
    pub fn move_plan(&self, station: &str, to_org: Option<i64>, to_account: Option<i64>, now: i64) -> Result<MovePlan> {
        let mut losing = BTreeSet::new();
        let mut people = Vec::new();
        for g in self.grants_of(station)? {
            match &g.subject {
                Subject::Org { .. } | Subject::Group { .. } => {
                    if !self.grant_to_org(&g.subject, to_org) {
                        losing.insert(subject_name(&g.subject));
                    }
                }
                Subject::Person { account, .. } if Some(*account) == to_account => {}
                Subject::Person { .. } | Subject::Invite { .. } => people.push(subject_name(&g.subject)),
            }
        }
        for g in self.group_grants_on(station)? {
            losing.insert(subject_name(&g.subject));
        }
        let mut station_groups: Vec<String> = {
            let mut stmt = self.conn.prepare("SELECT sg.name FROM station_groups sg JOIN station_group_members m ON m.sgroup = sg.id WHERE m.station = ?1")?;
            stmt.query_map([station], |r| r.get(0))?.collect::<rusqlite::Result<_>>()?
        };
        station_groups.sort_by_key(|n: &String| n.to_lowercase());
        Ok(MovePlan { losing: losing.into_iter().collect(), station_groups, pending_links: self.pending_links(station, now)?, people })
    }

    /// Déplace la Station (une transaction) : nouveau propriétaire, et ce que [`Store::move_plan`] annonce.
    pub fn move_station(&self, station: &str, to_account: Option<i64>, to_org: Option<i64>, by: i64, keep_people: bool, now: i64) -> Result<bool> {
        let kind = if to_org.is_some() { "organization" } else { "person" };
        let tx = self.conn.unchecked_transaction()?;
        let n = tx.execute(
            "UPDATE stations SET owner_kind = ?2, account = ?3, org = ?4, attached_by = ?5, enrolled_by = NULL WHERE id = ?1",
            params![station, kind, to_account, to_org, by],
        )?;
        if n != 1 {
            return Ok(false);
        }
        // Familles, équipes et groupes : seuls ceux de la destination restent.
        tx.execute(
            "DELETE FROM grants WHERE station = ?1 AND (org IS NOT NULL OR grp IS NOT NULL)
             AND NOT (org IS NOT NULL AND org IS ?2) AND NOT (grp IS NOT NULL AND grp IN (SELECT id FROM org_groups WHERE org IS ?2))",
            params![station, to_org],
        )?;
        if !keep_people {
            tx.execute("DELETE FROM grants WHERE station = ?1 AND (account IS NOT NULL OR email IS NOT NULL)", [station])?;
        }
        if let Some(a) = to_account {
            tx.execute("DELETE FROM grants WHERE station = ?1 AND account = ?2", params![station, a])?;
        }
        tx.execute("DELETE FROM station_group_members WHERE station = ?1", [station])?;
        tx.execute(
            "UPDATE access_links SET revoked_at = ?2 WHERE station = ?1 AND used_at IS NULL AND revoked_at IS NULL AND link_expires_at > ?2",
            params![station, now],
        )?;
        tx.execute("DELETE FROM device_stations WHERE station = ?1", [station])?;
        tx.commit()?;
        Ok(true)
    }
}

/// Pourquoi un déplacement est refusé.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MoveRefusal {
    /// Ni son propriétaire, ni gérant de sa famille ou de son équipe.
    NotAllowed,
    /// « Activez d'abord la double authentification. »
    NeedTwoFactor,
    /// « Vous ne gérez pas {destination}. »
    NotManager(String),
}

impl Accounts {
    /// Ce compte peut-il proposer de déplacer cette Station (son propriétaire, ou
    /// Propriétaire de sa famille ou de son équipe, double authentification ou pas
    /// encore) ? Un Administrateur ne sort pas un ordinateur de l'équipe : les
    /// Propriétaires le perdraient (REL-12).
    pub fn may_move(&self, account: &Account, station: &StationRow) -> bool {
        if station.account == Some(account.id) {
            return true;
        }
        station.org.is_some_and(|o| self.db().org_role(o, account.id).ok().flatten() == Some(super::orgs::Role::Owner))
    }

    /// Les destinations proposées : « Moi » (sauf la sienne) et les familles et
    /// équipes qu'il gère (par son rôle), sauf la propriétaire actuelle.
    pub fn move_destinations(&self, account: &Account, station: &StationRow) -> Vec<Dest> {
        if !self.may_move(account, station) {
            return Vec::new();
        }
        let mut out = Vec::new();
        if station.account != Some(account.id) {
            out.push(Dest::Me);
        }
        let orgs = self.db().orgs_of(account.id).unwrap_or_default();
        out.extend(orgs.into_iter().filter(|(o, r)| r.manages() && station.org != Some(o.id)).map(|(o, _)| Dest::Org(o)));
        out
    }

    /// La destination choisie (`moi` ou `org_…`), vérifiée : il gère les deux côtés, avec la double authentification.
    pub fn move_check(&self, account: &Account, station: &StationRow, choice: &str) -> Result<Dest, MoveRefusal> {
        if !self.may_move(account, station) {
            return Err(MoveRefusal::NotAllowed);
        }
        if !self.db().two_factor(account.id).unwrap_or(false) {
            return Err(MoveRefusal::NeedTwoFactor);
        }
        if choice == "moi" {
            return if station.account == Some(account.id) { Err(MoveRefusal::NotAllowed) } else { Ok(Dest::Me) };
        }
        let org = self.db().org_by_public(choice).ok().flatten();
        let Some(org) = org.filter(|o| station.org != Some(o.id)) else { return Err(MoveRefusal::NotAllowed) };
        let role = self.db().org_role(org.id, account.id).ok().flatten();
        match role {
            Some(r) if r.manages() => Ok(Dest::Org(org)),
            // Membre seulement : il la connaît, on peut la nommer.
            Some(_) => Err(MoveRefusal::NotManager(org.name)),
            None => Err(MoveRefusal::NotAllowed),
        }
    }

    /// « Ce qui change » pour ce déplacement.
    pub fn move_plan(&self, account: &Account, station: &StationRow, dest: &Dest) -> MovePlan {
        let to_account = matches!(dest, Dest::Me).then_some(account.id);
        self.db().move_plan(&station.id, dest.org_id(), to_account, self.now()).unwrap_or_default()
    }

    /// Déplace la Station : la base, les réglages des comptes qui n'y ont plus droit,
    /// les deux journaux, et `attached` à la Station (le nouveau propriétaire).
    pub fn move_station(&self, account: &Account, station: &StationRow, dest: &Dest, keep_people: bool, ip: &str) -> Result<bool> {
        let now = self.now();
        let to_account = matches!(dest, Dest::Me).then_some(account.id);
        let from = self.owner_label(station);
        let to = match dest {
            Dest::Me => account.name.clone(),
            Dest::Org(o) => o.name.clone(),
        };
        if !self.db().move_station(&station.id, to_account, dest.org_id(), account.id, keep_people, now)? {
            return Ok(false);
        }
        self.forget_settings_without_rights(&station.id);
        let detail = json!({ "station": station.name, "from": from, "to": to });
        {
            let db = self.db();
            if let Some(org) = station.org {
                db.org_log(org, Some(account), "ordinateur_deplace", detail.clone(), Some(ip), now)?;
            }
            if let Some(org) = dest.org_id() {
                db.org_log(org, Some(account), "ordinateur_deplace", detail, Some(ip), now)?;
            }
            // Une personne d'un côté (la seule possible : lui) : son journal.
            db.audit(Some(account.id), "ordinateur_deplace", Some(ip), now)?;
        }
        tracing::info!(station = %station.id, vers = if to_account.is_some() { "personne" } else { "organisation" }, "ordinateur déplacé");
        // La Station l'apprend comme un rattachement : `attached`, le nouveau propriétaire.
        let row = self.db().station(&station.id)?;
        if let Some(msg) = row.and_then(|row| self.attached_msg_of(&row, None)) {
            self.stations.send(&station.id, msg);
        }
        Ok(true)
    }

    /// Le nom du propriétaire : la personne ou la famille ou l'équipe.
    pub fn owner_label(&self, station: &StationRow) -> String {
        let db = self.db();
        match (station.account, station.org) {
            (Some(a), _) => db.account(a).ok().flatten().map(|a| a.name).unwrap_or_default(),
            (None, Some(o)) => db.org(o).ok().flatten().map(|o| o.name).unwrap_or_default(),
            _ => String::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::accounts::orgs::Kind;
    use kybernow_tunnel::access::{Right, Rights};

    #[test]
    fn plan_et_deplacement_d_une_personne_vers_une_equipe() {
        let s = Store::memory(0).unwrap();
        let ana = s.create_account("ana@exemple.org", "Ana", "h", "fr", Some(0), 0).unwrap().unwrap();
        let lea = s.create_account("lea@exemple.org", "Léa", "h", "fr", Some(0), 0).unwrap().unwrap();
        let team = s.create_org("L'atelier", Kind::Team, ana, 0).unwrap();
        let fam = s.create_org("Famille Martin", Kind::Family, ana, 0).unwrap();
        let grp = s.create_group(team.id, "Graphistes", 0).unwrap().unwrap();
        s.attach_station("st1", "PC", ana, ana, 0).unwrap();
        let r = Rights::new([Right::Session]);
        s.grant_person("st1", lea, &r, None, ana, 0).unwrap();
        s.grant_invite("st1", "sam@exemple.org", &r, None, ana, 0).unwrap();
        s.grant_org("st1", fam.id, &r, ana, 0).unwrap();
        s.grant_group("st1", grp, &r, ana, 0).unwrap();
        s.create_access_link("st1", &crate::accounts::token_hash("kla_x"), "kla_x", &r, 100, None, ana, 0).unwrap();
        let plan = s.move_plan("st1", Some(team.id), None, 10).unwrap();
        assert_eq!(plan.losing, vec!["Famille Martin".to_string()], "le groupe de la destination reste");
        assert_eq!(plan.people, vec!["Léa".to_string(), "sam@exemple.org".to_string()]);
        assert_eq!(plan.pending_links, 1);
        assert!(s.move_station("st1", None, Some(team.id), ana, true, 10).unwrap());
        let row = s.station("st1").unwrap().unwrap();
        assert_eq!((row.owner_kind.as_str(), row.account, row.org), ("organization", None, Some(team.id)));
        let left: Vec<String> = s.grants_of("st1").unwrap().iter().map(|g| subject_name(&g.subject)).collect();
        assert_eq!(left, ["Léa", "sam@exemple.org", "L'atelier › Graphistes"]);
        assert_eq!(s.pending_links("st1", 10).unwrap(), 0);
        // Retour vers Ana, sans garder les personnes : il ne reste rien.
        assert!(s.move_station("st1", Some(ana), None, ana, false, 20).unwrap());
        assert!(s.grants_of("st1").unwrap().is_empty());
        assert!(!s.move_station("inconnue", Some(ana), None, ana, true, 20).unwrap());
    }
}
