//! Les réglages synchronisés (docs/comptes.md, « Réglages synchronisés et
//! déplacer un ordinateur ») : par compte et par ordinateur, un petit objet
//! JSON que les apps lisent et écrivent par l'API des appareils.
//!
//! - `GET /v1/settings` → `{"settings":[{"station","updated_at","values"}]}`,
//!   seulement les ordinateurs sur lesquels le compte a des droits à cet
//!   instant ; les lignes des autres sont effacées au passage.
//! - `PUT /v1/settings/<station>` `{"values":{…}}` → `{"updated_at"}` ; le
//!   dernier changement l'emporte. `404 unknown_station` sans droit,
//!   `400 bad_values` si `values` n'est pas un objet, `413 too_large` au-delà
//!   de [`MAX_VALUES_BYTES`] une fois sérialisé.
//!
//! Les valeurs sont gardées telles quelles : une clé inconnue reste (une app
//! plus récente peut en ajouter). Les clés connues : `profile` (texte ou
//! `null`), `codec` (texte), `yuv444` (booléen), `mbps` (nombre), `hdr`
//! (booléen), `immersive` (`"fullscreen"`, `"always"`, `"never"`).
//!
//! Le ménage : avec le compte (clé étrangère), au détachement d'un ordinateur
//! ou à son rattachement à un autre propriétaire (toutes ses lignes), quand il
//! est déplacé (celles des comptes qui n'y ont plus droit), et au ménage
//! périodique (celles des ordinateurs que le compte ne voit plus).

use std::collections::HashSet;

use anyhow::Result;
use kybernow_tunnel::id::is_station_id;
use rusqlite::params;
use serde_json::{Value, json};

use super::Accounts;
use super::devices::{ApiReply, ApiRequest, error, reply};
use super::store::Store;

/// La migration 9 de la base (`super::store`). Pas de clé étrangère vers
/// `stations` (comme `grants`) : les lignes d'un ordinateur partent avec son
/// détachement ([`Store::detach_station`]).
pub const MIGRATION: &str = r#"
    CREATE TABLE computer_settings (
        account INTEGER NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
        station TEXT NOT NULL,
        "values" TEXT NOT NULL DEFAULT '{}',
        updated_at INTEGER NOT NULL,
        PRIMARY KEY (account, station)
    );
    CREATE INDEX computer_settings_station ON computer_settings(station);
"#;

/// Les valeurs d'un ordinateur, sérialisées : 4 Ko au plus.
pub const MAX_VALUES_BYTES: usize = 4096;

/// Le chemin de l'API.
pub const SETTINGS_PATH: &str = "/v1/settings";

/// Les réglages gardés d'un ordinateur.
#[derive(Debug, Clone, PartialEq)]
pub struct Synced {
    pub station: String,
    pub values: Value,
    pub updated_at: i64,
}

impl Store {
    /// Les réglages gardés de ce compte, par ordinateur.
    pub fn settings_of(&self, account: i64) -> Result<Vec<Synced>> {
        let mut stmt = self.conn.prepare(r#"SELECT station, "values", updated_at FROM computer_settings WHERE account = ?1 ORDER BY station"#)?;
        let rows = stmt.query_map([account], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, i64>(2)?)))?;
        let mut out = Vec::new();
        for row in rows {
            let (station, text, updated_at) = row?;
            let values = serde_json::from_str(&text).unwrap_or_else(|_| json!({}));
            out.push(Synced { station, values, updated_at });
        }
        Ok(out)
    }

    /// Garde les réglages de ce compte pour cet ordinateur (le dernier l'emporte).
    pub fn set_settings(&self, account: i64, station: &str, values: &Value, now: i64) -> Result<()> {
        self.conn.execute(
            r#"INSERT INTO computer_settings (account, station, "values", updated_at) VALUES (?1, ?2, ?3, ?4)
               ON CONFLICT(account, station) DO UPDATE SET "values" = excluded."values", updated_at = excluded.updated_at"#,
            params![account, station, values.to_string(), now],
        )?;
        Ok(())
    }

    /// Efface les réglages d'un compte pour un ordinateur.
    pub fn delete_settings(&self, account: i64, station: &str) -> Result<bool> {
        Ok(self.conn.execute("DELETE FROM computer_settings WHERE account = ?1 AND station = ?2", params![account, station])? > 0)
    }

    /// Les comptes qui ont des réglages gardés pour cet ordinateur.
    pub fn settings_accounts(&self, station: &str) -> Result<Vec<i64>> {
        let mut stmt = self.conn.prepare("SELECT account FROM computer_settings WHERE station = ?1 ORDER BY account")?;
        let rows = stmt.query_map([station], |r| r.get(0))?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    /// Le ménage : les réglages des ordinateurs détachés, et ceux qu'un compte ne voit plus.
    pub fn sweep_settings(&self, now: i64) -> Result<usize> {
        let mut gone = self.conn.execute("DELETE FROM computer_settings WHERE station NOT IN (SELECT id FROM stations)", [])?;
        let accounts: Vec<i64> = {
            let mut stmt = self.conn.prepare("SELECT DISTINCT account FROM computer_settings")?;
            stmt.query_map([], |r| r.get(0))?.collect::<rusqlite::Result<_>>()?
        };
        for account in accounts {
            let visible: HashSet<String> = self.visible_stations(account, now)?.into_iter().map(|s| s.id).collect();
            for synced in self.settings_of(account)? {
                if !visible.contains(&synced.station) && self.delete_settings(account, &synced.station)? {
                    gone += 1;
                }
            }
        }
        Ok(gone)
    }
}

impl Accounts {
    /// `GET /v1/settings` et `PUT /v1/settings/<station>`.
    pub(super) fn api_settings(&self, req: &ApiRequest) -> ApiReply {
        let Some((_, account)) = self.bearer_device(req.bearer.as_deref()) else {
            return error(401, "bad_session");
        };
        let now = self.now();
        if req.path == SETTINGS_PATH {
            if req.method != super::devices::ApiMethod::Get {
                return error(405, "method_not_allowed");
            }
            let all = self.db().settings_of(account.id).unwrap_or_default();
            let mut list = Vec::new();
            for synced in all {
                let row = self.db().station(&synced.station).ok().flatten();
                if row.is_some_and(|s| self.rights_on(&account, &s).is_some()) {
                    list.push(json!({ "station": synced.station, "updated_at": synced.updated_at, "values": synced.values }));
                } else {
                    // Plus ouvert à ce compte (ou détaché) : ses réglages partent.
                    let _ = self.db().delete_settings(account.id, &synced.station);
                }
            }
            return reply(200, json!({ "settings": list }));
        }
        let Some(station) = req.path.strip_prefix("/v1/settings/") else { return error(404, "unknown_station") };
        if req.method != super::devices::ApiMethod::Put {
            return error(405, "method_not_allowed");
        }
        if !is_station_id(station) {
            return error(404, "unknown_station");
        }
        let row = self.db().station(station).ok().flatten();
        if row.is_none_or(|s| self.rights_on(&account, &s).is_none()) {
            return error(404, "unknown_station");
        }
        let Some(values) = req.body.get("values").filter(|v| v.is_object()) else {
            return error(400, "bad_values");
        };
        if values.to_string().len() > MAX_VALUES_BYTES {
            return error(413, "too_large");
        }
        if let Err(e) = self.db().set_settings(account.id, station, values, now) {
            tracing::warn!("réglages synchronisés : {e:#}");
            return error(503, "unavailable");
        }
        reply(200, json!({ "updated_at": now }))
    }

    /// Après un changement de droits sur cet ordinateur : les réglages des comptes qui n'y ont plus droit partent.
    pub fn forget_settings_without_rights(&self, station: &str) {
        let row = self.db().station(station).ok().flatten();
        let accounts = self.db().settings_accounts(station).unwrap_or_default();
        for a in accounts {
            let account = self.db().account(a).ok().flatten();
            let keep = match (&row, account) {
                (Some(s), Some(account)) => self.rights_on(&account, s).is_some(),
                _ => false,
            };
            if !keep {
                let _ = self.db().delete_settings(a, station);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reglages_gardes_remplaces_et_menage() {
        let s = Store::memory(0).unwrap();
        let a = s.create_account("a@exemple.org", "A", "h", "fr", Some(0), 0).unwrap().unwrap();
        let b = s.create_account("b@exemple.org", "B", "h", "fr", Some(0), 0).unwrap().unwrap();
        s.attach_station("st1", "PC", a, a, 0).unwrap();
        s.set_settings(a, "st1", &json!({ "codec": "hevc", "futur": [1, 2] }), 10).unwrap();
        s.set_settings(a, "st1", &json!({ "codec": "av1", "mbps": 80 }), 20).unwrap();
        let got = s.settings_of(a).unwrap();
        assert_eq!(got, vec![Synced { station: "st1".into(), values: json!({ "codec": "av1", "mbps": 80 }), updated_at: 20 }], "le dernier l'emporte");
        // B n'y a pas droit : le ménage efface sa ligne ; celle d'A reste.
        s.set_settings(b, "st1", &json!({ "hdr": true }), 20).unwrap();
        assert_eq!(s.sweep_settings(30).unwrap(), 1);
        assert!(s.settings_of(b).unwrap().is_empty() && s.settings_of(a).unwrap().len() == 1);
        // Détaché : ses réglages partent ; le compte effacé : les siens aussi.
        s.attach_station("st2", "Mac", a, a, 0).unwrap();
        s.set_settings(a, "st2", &json!({}), 40).unwrap();
        assert!(s.detach_station("st1", None).unwrap());
        assert_eq!(s.settings_of(a).unwrap().iter().map(|x| x.station.as_str()).collect::<Vec<_>>(), ["st2"]);
        s.set_settings(b, "st2", &json!({}), 40).unwrap();
        assert!(s.delete_account(a).unwrap());
        let left: i64 = s.conn.query_row("SELECT COUNT(*) FROM computer_settings", [], |r| r.get(0)).unwrap();
        assert_eq!(left, 0, "les siens (clé étrangère), et ceux des autres sur ses ordinateurs");
    }

    #[test]
    fn rattache_a_un_autre_proprietaire_les_reglages_partent() {
        let s = Store::memory(0).unwrap();
        let a = s.create_account("a@exemple.org", "A", "h", "fr", Some(0), 0).unwrap().unwrap();
        let b = s.create_account("b@exemple.org", "B", "h", "fr", Some(0), 0).unwrap().unwrap();
        s.attach_station("st1", "PC", a, a, 0).unwrap();
        s.set_settings(a, "st1", &json!({ "hdr": true }), 10).unwrap();
        s.attach_station("st1", "PC", a, a, 11).unwrap();
        assert_eq!(s.settings_of(a).unwrap().len(), 1, "même propriétaire : rien ne change");
        s.attach_station("st1", "PC", b, b, 12).unwrap();
        assert!(s.settings_of(a).unwrap().is_empty());
    }
}
