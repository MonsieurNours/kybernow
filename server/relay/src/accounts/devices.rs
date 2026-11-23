//! Les appareils connectés à un compte (docs/comptes.md, « Créer son compte et
//! se connecter », moitié relais du lot 3) et les jetons d'accès (« Le jeton
//! d'accès », lot 2).
//!
//! Une **session d'appareil** n'est pas une session de navigateur : un jeton
//! porteur de 256 bits (`Authorization: Bearer …`), gardé en empreinte,
//! révocable depuis `/compte` (« Appareils connectés »). Deux façons de
//! l'obtenir, sans que l'app voie jamais le mot de passe :
//!
//! - **fenêtre du système** : `GET /compte/connexion?appareil=<nom>&retour=kybernow://compte`,
//!   la connexion habituelle, puis « Connecter KyberNow Link sur {appareil} ? » ;
//!   le retour vers l'app porte un code à usage unique (5 min), échangé à
//!   `POST /v1/devices/token` ;
//! - **code (téléviseurs)** : `POST /v1/devices/code` → `{code, poll, expires}` ;
//!   le code de huit signes (`XXXX-XXXX`) s'approuve depuis un téléphone
//!   (`/tv`, puis `/compte/appareil?code=…`) ; l'appareil demande
//!   `POST /v1/devices/poll` toutes les 5 secondes.
//!
//! Avec sa session, l'appareil liste les ordinateurs du compte
//! (`GET /v1/computers`) et demande un jeton d'accès signé (5 min) pour l'un
//! d'eux (`POST /v1/access {station}`). Le premier jeton d'un appareil pour une
//! Station envoie un e-mail au propriétaire et met une ligne au journal
//! (garde-fou 4, « Avis à chaque nouvel appareil »).
//!
//! Pour l'instant (lot 2) : les ordinateurs d'un compte sont ceux qu'il
//! possède, avec tous les droits ; les personnes et les droits viennent au lot 4.

use std::net::IpAddr;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use anyhow::Result;
use kybernow_tunnel::access::{self, Claims, Names, Right, Rights};
use kybernow_tunnel::id::{base32_lower, is_station_id};
use rusqlite::{OptionalExtension, params};
use serde_json::{Value, json};

use super::mail::{self, Message};
use super::store::{Account, Store};
use super::{Accounts, ip_key, new_token, random, token_hash};
use crate::limits::Buckets;
use crate::pages::Lang;

/// Le code du retour vers l'app : 5 minutes.
pub const RETURN_SECS: i64 = 5 * 60;
/// Le code d'un téléviseur : 10 minutes.
pub const TV_SECS: i64 = 10 * 60;
/// L'appareil demande où en est son code au plus toutes les 5 secondes.
pub const POLL_EVERY: u64 = 5;
/// Le seul retour permis : le schéma des apps KyberNow.
pub const RETURN_URI: &str = "kybernow://compte";
/// Les signes d'un code de téléviseur : ni voyelle (pas de mot), ni 0, 1, O, I.
const TV_ALPHABET: &[u8] = b"BCDFGHJKLMNPQRSTVWXZ23456789";
/// Les essais de code de téléviseur, par adresse.
const TV_TRIES: u32 = 10;
/// Les codes de téléviseur demandés, par adresse.
const TV_CODES: u32 = 20;
const WINDOW: Duration = Duration::from_secs(15 * 60);

/// Les limites des appareils.
pub struct Devices {
    tv_tries: Mutex<Buckets<IpAddr>>,
    tv_codes: Mutex<Buckets<IpAddr>>,
}

impl Default for Devices {
    fn default() -> Self {
        Devices { tv_tries: Mutex::new(Buckets::new(TV_TRIES, WINDOW)), tv_codes: Mutex::new(Buckets::new(TV_CODES, WINDOW)) }
    }
}

impl Devices {
    /// Un essai de code de téléviseur de plus depuis cette adresse ; `Err` : attendre.
    pub fn tv_try(&self, ip: IpAddr) -> Result<(), Duration> {
        self.tv_tries.lock().unwrap_or_else(|e| e.into_inner()).take(&ip_key(ip), Instant::now())
    }

    /// Un code accepté ne compte pas dans les essais.
    pub fn tv_try_ok(&self, ip: IpAddr) {
        self.tv_tries.lock().unwrap_or_else(|e| e.into_inner()).refund(&ip_key(ip), Instant::now());
    }

    pub fn sweep(&self) {
        let now = Instant::now();
        self.tv_tries.lock().unwrap_or_else(|e| e.into_inner()).sweep(now);
        self.tv_codes.lock().unwrap_or_else(|e| e.into_inner()).sweep(now);
    }
}

/// Un appareil connecté.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Device {
    pub id: i64,
    /// `dev_…` : dans les jetons d'accès et chez la Station.
    pub public_id: String,
    pub account: i64,
    pub name: String,
    /// `app` (fenêtre du système) ou `tv` (code).
    pub kind: String,
    pub created_at: i64,
    pub seen_at: i64,
}

const DEVICE_COLS: &str = "id, public_id, account, name, kind, created_at, seen_at";

fn device_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<Device> {
    Ok(Device { id: r.get(0)?, public_id: r.get(1)?, account: r.get(2)?, name: r.get(3)?, kind: r.get(4)?, created_at: r.get(5)?, seen_at: r.get(6)? })
}

/// Un code d'appareil : retour vers l'app, ou code de téléviseur.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceCode {
    pub hash: Vec<u8>,
    pub kind: String,
    pub account: Option<i64>,
    pub device_name: String,
    /// `pending`, `approved`, `denied`.
    pub status: String,
    pub expires_at: i64,
}

const CODE_COLS: &str = "hash, kind, account, device_name, status, expires_at";

fn code_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<DeviceCode> {
    Ok(DeviceCode { hash: r.get(0)?, kind: r.get(1)?, account: r.get(2)?, device_name: r.get(3)?, status: r.get(4)?, expires_at: r.get(5)? })
}

impl Store {
    pub fn create_device(&self, public_id: &str, hash: &[u8], account: i64, name: &str, kind: &str, now: i64) -> Result<i64> {
        self.conn.execute(
            "INSERT INTO devices (public_id, hash, account, name, kind, created_at, seen_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?6)",
            params![public_id, hash, account, name, kind, now],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    /// L'appareil d'un jeton porteur (pas révoqué).
    pub fn device_by_token(&self, hash: &[u8]) -> Result<Option<Device>> {
        Ok(self.conn.query_row(&format!("SELECT {DEVICE_COLS} FROM devices WHERE hash = ?1 AND revoked_at IS NULL"), [hash], device_row).optional()?)
    }

    pub fn touch_device(&self, id: i64, now: i64) -> Result<()> {
        self.conn.execute("UPDATE devices SET seen_at = ?2 WHERE id = ?1", params![id, now])?;
        Ok(())
    }

    /// Les appareils connectés d'un compte, le plus récemment vu d'abord.
    pub fn devices_of(&self, account: i64) -> Result<Vec<Device>> {
        let mut stmt =
            self.conn.prepare(&format!("SELECT {DEVICE_COLS} FROM devices WHERE account = ?1 AND revoked_at IS NULL ORDER BY seen_at DESC, id DESC"))?;
        let rows = stmt.query_map([account], device_row)?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    /// Déconnecte un appareil du compte ; faux s'il n'est pas à lui.
    pub fn revoke_device(&self, id: i64, account: i64) -> Result<bool> {
        Ok(self.conn.execute("DELETE FROM devices WHERE id = ?1 AND account = ?2", params![id, account])? == 1)
    }

    /// Déconnecte tous les appareils du compte (nouveau mot de passe).
    pub fn revoke_devices(&self, account: i64) -> Result<usize> {
        Ok(self.conn.execute("DELETE FROM devices WHERE account = ?1", [account])?)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn insert_device_code(
        &self,
        hash: &[u8],
        kind: &str,
        poll_hash: Option<&[u8]>,
        account: Option<i64>,
        name: &str,
        status: &str,
        now: i64,
        expires_at: i64,
    ) -> Result<()> {
        self.conn.execute(
            "INSERT INTO device_codes (hash, kind, poll_hash, account, device_name, status, created_at, expires_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![hash, kind, poll_hash, account, name, status, now, expires_at],
        )?;
        Ok(())
    }

    /// Un code valide de cette sorte.
    pub fn device_code(&self, hash: &[u8], kind: &str, now: i64) -> Result<Option<DeviceCode>> {
        Ok(self
            .conn
            .query_row(&format!("SELECT {CODE_COLS} FROM device_codes WHERE hash = ?1 AND kind = ?2 AND expires_at > ?3"), params![hash, kind, now], code_row)
            .optional()?)
    }

    /// Le code d'un téléviseur, d'après son jeton de suivi (échu compris : l'appareil doit le savoir).
    pub fn device_code_by_poll(&self, poll_hash: &[u8]) -> Result<Option<DeviceCode>> {
        Ok(self.conn.query_row(&format!("SELECT {CODE_COLS} FROM device_codes WHERE poll_hash = ?1"), [poll_hash], code_row).optional()?)
    }

    /// Approuve ou refuse un code de téléviseur encore en attente ; faux sinon.
    pub fn answer_device_code(&self, hash: &[u8], account: i64, approve: bool, now: i64) -> Result<bool> {
        let status = if approve { "approved" } else { "denied" };
        Ok(self.conn.execute(
            "UPDATE device_codes SET status = ?2, account = ?3 WHERE hash = ?1 AND status = 'pending' AND expires_at > ?4",
            params![hash, status, account, now],
        )? == 1)
    }

    /// Consomme un code ; faux s'il a déjà servi.
    pub fn delete_device_code(&self, hash: &[u8]) -> Result<bool> {
        Ok(self.conn.execute("DELETE FROM device_codes WHERE hash = ?1", [hash])? == 1)
    }

    /// Note la venue d'un appareil sur une Station ; vrai la première fois.
    pub fn first_visit(&self, device: i64, station: &str, now: i64) -> Result<bool> {
        Ok(self.conn.execute("INSERT OR IGNORE INTO device_stations (device, station, first_at) VALUES (?1, ?2, ?3)", params![device, station, now])? == 1)
    }

    /// Le ménage des codes d'appareil échus (un téléviseur a encore une minute pour l'apprendre).
    pub fn sweep_device_codes(&self, now: i64) -> Result<()> {
        self.conn.execute("DELETE FROM device_codes WHERE expires_at <= ?1", [now - 60])?;
        Ok(())
    }
}

/// Un nom d'appareil donné par l'app : 64 caractères au plus, sans caractère de contrôle.
pub fn clean_device_name(name: &str) -> String {
    let spaced: String = name.chars().map(|c| if c.is_control() { ' ' } else { c }).collect();
    spaced.split_whitespace().collect::<Vec<_>>().join(" ").chars().take(64).collect::<String>().trim_end().to_string()
}

/// Un nouveau code de téléviseur : `XXXX-XXXX`.
pub fn new_tv_code() -> String {
    let bytes = random::<8>();
    let s: String = bytes.iter().map(|b| TV_ALPHABET[*b as usize % TV_ALPHABET.len()] as char).collect();
    format!("{}-{}", &s[..4], &s[4..])
}

/// Un code de téléviseur tel que tapé (minuscules, espaces, tiret) : `XXXXXXXX`, ou `None`.
pub fn normalize_tv_code(input: &str) -> Option<String> {
    let s: String = input.chars().filter(|c| c.is_ascii_alphanumeric()).map(|c| c.to_ascii_uppercase()).collect();
    (s.len() == 8 && s.bytes().all(|b| TV_ALPHABET.contains(&b))).then_some(s)
}

/// `XXXXXXXX` → `XXXX-XXXX`.
pub fn dashed(code: &str) -> String {
    if code.len() == 8 { format!("{}-{}", &code[..4], &code[4..]) } else { code.to_string() }
}

fn tv_hash(normalized: &str) -> Vec<u8> {
    token_hash(&format!("tv:{normalized}"))
}

/// Une requête d'API (`/v1/…`) déjà lue.
#[derive(Debug, Clone)]
pub struct ApiRequest {
    pub post: bool,
    pub path: String,
    /// Le jeton porteur (`Authorization: Bearer …`).
    pub bearer: Option<String>,
    pub body: Value,
    pub ip: IpAddr,
}

/// La réponse : un statut et du JSON.
#[derive(Debug, Clone, PartialEq)]
pub struct ApiReply {
    pub status: u16,
    pub body: Value,
    pub retry_after: Option<u64>,
}

fn reply(status: u16, body: Value) -> ApiReply {
    ApiReply { status, body, retry_after: None }
}

fn error(status: u16, code: &str) -> ApiReply {
    reply(status, json!({ "error": code }))
}

/// Les chemins de l'API des appareils.
pub fn is_api_path(path: &str) -> bool {
    matches!(path, "/v1/devices/code" | "/v1/devices/poll" | "/v1/devices/token" | "/v1/devices/revoke" | "/v1/computers" | "/v1/access")
}

/// Le texte d'un champ du corps JSON.
fn field<'a>(body: &'a Value, key: &str) -> &'a str {
    body.get(key).and_then(Value::as_str).unwrap_or("")
}

impl Accounts {
    /// Ouvre une session d'appareil : le jeton porteur et ce que l'app affiche.
    pub fn open_device(&self, account: &Account, name: &str, kind: &str) -> Result<Value> {
        let (token, hash) = new_token();
        let public_id = format!("dev_{}", base32_lower(&random::<10>()));
        let name = clean_device_name(name);
        let now = self.now();
        let db = self.db();
        db.create_device(&public_id, &hash, account.id, &name, kind, now)?;
        db.audit(Some(account.id), if kind == "tv" { "appareil_connecte_par_code" } else { "appareil_connecte" }, None, now)?;
        drop(db);
        Ok(json!({
            "token": token,
            "device": { "id": public_id, "name": name },
            "account": { "id": self.public_account_id(account.id), "name": account.name, "email": account.email },
            "relay": self.relay_name(),
        }))
    }

    /// Le code du retour vers l'app, après « Connecter » (fenêtre du système).
    pub fn return_code(&self, account: &Account, device_name: &str) -> Result<String> {
        let (code, hash) = new_token();
        let now = self.now();
        self.db().insert_device_code(&hash, "return", None, Some(account.id), &clean_device_name(device_name), "approved", now, now + RETURN_SECS)?;
        Ok(code)
    }

    /// Le code de téléviseur tapé, s'il est en attente.
    pub fn tv_code(&self, typed: &str) -> Option<(String, DeviceCode)> {
        let code = normalize_tv_code(typed)?;
        let found = self.db().device_code(&tv_hash(&code), "tv", self.now()).ok().flatten()?;
        Some((code, found))
    }

    /// Approuve (`true`) ou refuse un code de téléviseur.
    pub fn answer_tv_code(&self, code: &str, account: i64, approve: bool) -> bool {
        let now = self.now();
        let db = self.db();
        let ok = db.answer_device_code(&tv_hash(code), account, approve, now).unwrap_or(false);
        if ok {
            let _ = db.audit(Some(account), if approve { "appareil_approuve" } else { "appareil_refuse" }, None, now);
        }
        ok
    }

    /// L'appareil du jeton porteur, et son compte (vérifié).
    fn bearer_device(&self, bearer: Option<&str>) -> Option<(Device, Account)> {
        let token = bearer?.trim();
        if token.is_empty() || token.len() > 128 {
            return None;
        }
        let db = self.db();
        let device = db.device_by_token(&token_hash(token)).ok()??;
        let account = db.account(device.account).ok()??;
        if !account.verified() {
            return None;
        }
        let now = self.now();
        if now - device.seen_at > 300 {
            let _ = db.touch_device(device.id, now);
        }
        Some((device, account))
    }

    /// Sert une requête de l'API des appareils (`/v1/devices/…`, `/v1/computers`, `/v1/access`).
    pub fn api(&self, req: &ApiRequest) -> ApiReply {
        match (req.path.as_str(), req.post) {
            ("/v1/devices/code", true) => self.api_tv_code(req),
            ("/v1/devices/poll", true) => self.api_poll(req),
            ("/v1/devices/token", true) => self.api_token(req),
            ("/v1/devices/revoke", true) => {
                let Some((device, account)) = self.bearer_device(req.bearer.as_deref()) else {
                    return error(401, "bad_session");
                };
                let db = self.db();
                let _ = db.revoke_device(device.id, account.id);
                let _ = db.audit(Some(account.id), "appareil_deconnecte", None, self.now());
                reply(200, json!({ "ok": true }))
            }
            ("/v1/computers", false) => self.api_computers(req),
            ("/v1/access", true) => self.api_access(req),
            _ => error(405, "method_not_allowed"),
        }
    }

    fn api_tv_code(&self, req: &ApiRequest) -> ApiReply {
        let name = clean_device_name(field(&req.body, "name"));
        if name.is_empty() {
            return error(400, "bad_request");
        }
        if let Err(wait) = self.devices.tv_codes.lock().unwrap_or_else(|e| e.into_inner()).take(&ip_key(req.ip), Instant::now()) {
            let mut r = error(429, "too_many");
            r.retry_after = Some(wait.as_secs().max(1));
            return r;
        }
        let now = self.now();
        // Deux codes tirés pareils : on en tire un autre.
        for _ in 0..4 {
            let code = new_tv_code();
            let normalized = normalize_tv_code(&code).expect("code bien formé");
            let (poll, poll_hash) = new_token();
            let inserted = self.db().insert_device_code(&tv_hash(&normalized), "tv", Some(&poll_hash), None, &name, "pending", now, now + TV_SECS);
            if inserted.is_ok() {
                return reply(
                    200,
                    json!({
                        "code": code,
                        "poll": poll,
                        "expires": TV_SECS,
                        "interval": POLL_EVERY,
                        "url": format!("{}/tv", self.public_url),
                        "url_complete": format!("{}/compte/appareil?code={code}", self.public_url),
                    }),
                );
            }
        }
        error(500, "unavailable")
    }

    fn api_poll(&self, req: &ApiRequest) -> ApiReply {
        let poll = field(&req.body, "poll");
        if poll.is_empty() {
            return error(400, "bad_request");
        }
        let now = self.now();
        let found = self.db().device_code_by_poll(&token_hash(poll)).ok().flatten();
        let Some(code) = found else {
            return error(404, "unknown_code");
        };
        if code.expires_at <= now {
            let _ = self.db().delete_device_code(&code.hash);
            return error(410, "expired");
        }
        match (code.status.as_str(), code.account) {
            ("approved", Some(account)) => {
                // Consommé avant d'ouvrir la session : deux demandes ne font pas deux appareils.
                if !self.db().delete_device_code(&code.hash).unwrap_or(false) {
                    return error(404, "unknown_code");
                }
                let account = self.db().account(account).ok().flatten();
                let Some(account) = account else {
                    return error(404, "unknown_code");
                };
                match self.open_device(&account, &code.device_name, "tv") {
                    Ok(v) => reply(200, v),
                    Err(e) => {
                        tracing::warn!("session d'appareil : {e:#}");
                        error(500, "unavailable")
                    }
                }
            }
            ("denied", _) => {
                let _ = self.db().delete_device_code(&code.hash);
                error(403, "denied")
            }
            _ => reply(202, json!({ "pending": true, "interval": POLL_EVERY })),
        }
    }

    fn api_token(&self, req: &ApiRequest) -> ApiReply {
        let code = field(&req.body, "code");
        if code.is_empty() || code.len() > 128 {
            return error(400, "bad_code");
        }
        let hash = token_hash(code);
        let found = self.db().device_code(&hash, "return", self.now()).ok().flatten();
        let Some(found) = found.filter(|c| c.status == "approved") else {
            return error(400, "bad_code");
        };
        if !self.db().delete_device_code(&hash).unwrap_or(false) {
            return error(400, "bad_code");
        }
        let account = found.account.and_then(|a| self.db().account(a).ok().flatten());
        let Some(account) = account else {
            return error(400, "bad_code");
        };
        match self.open_device(&account, &found.device_name, "app") {
            Ok(v) => reply(200, v),
            Err(e) => {
                tracing::warn!("session d'appareil : {e:#}");
                error(500, "unavailable")
            }
        }
    }

    /// Les droits d'un compte sur une Station rattachée (lot 2 : le propriétaire a tout).
    fn rights_on(&self, account: &Account, owner: i64) -> Option<Rights> {
        // TODO lot 4 : les droits donnés à une personne ; lot 5 : à une organisation ou un groupe.
        (account.id == owner).then(|| Rights::new(Right::ALL))
    }

    fn api_computers(&self, req: &ApiRequest) -> ApiReply {
        let Some((_, account)) = self.bearer_device(req.bearer.as_deref()) else {
            return error(401, "bad_session");
        };
        let stations = self.db().stations_of(account.id).unwrap_or_default();
        let owner = json!({ "kind": "person", "id": self.public_account_id(account.id), "name": account.name });
        let list: Vec<Value> = stations
            .iter()
            .filter_map(|s| {
                let rights = self.rights_on(&account, s.account)?;
                Some(json!({
                    "station": s.id,
                    "name": s.name,
                    "rights": rights,
                    "owner": owner,
                    "mine": true,
                    "online": self.stations.online(&s.id),
                    "attached_at": s.attached_at,
                }))
            })
            .collect();
        reply(200, json!({ "computers": list }))
    }

    fn api_access(&self, req: &ApiRequest) -> ApiReply {
        let Some((device, account)) = self.bearer_device(req.bearer.as_deref()) else {
            return error(401, "bad_session");
        };
        let station_id = field(&req.body, "station");
        if !is_station_id(station_id) {
            return error(400, "bad_request");
        }
        let station = self.db().station(station_id).ok().flatten();
        let Some((station, rights)) = station.and_then(|s| self.rights_on(&account, s.account).map(|r| (s, r))) else {
            return error(404, "unknown_station");
        };
        let now = self.now();
        let names = Names { account: account.name.clone(), email: Some(account.email.clone()), device: device.name.clone() };
        let claims = Claims::issue(&self.relay_name(), &station.id, &self.public_account_id(account.id), &device.public_id, rights, names, now.max(0) as u64);
        let token = self.stations.keys.signer().sign(&claims);
        let first = self.db().first_visit(device.id, &station.id, now).unwrap_or(false);
        if first {
            self.new_device_notice(&station, &device, req.ip);
        }
        reply(200, json!({ "token": token, "station": station.id, "rights": claims.rights, "expires": claims.exp, "lifetime": access::LIFETIME_SECS }))
    }

    /// Garde-fou 4 : la première venue d'un appareil sur une Station, au propriétaire.
    fn new_device_notice(&self, station: &super::stations::StationRow, device: &Device, ip: IpAddr) {
        let now = self.now();
        let owner = self.db().account(station.account).ok().flatten();
        let Some(owner) = owner else { return };
        let ip = ip.to_canonical().to_string();
        if let Err(e) = self.db().audit(Some(owner.id), "nouvel_appareil", Some(&ip), now) {
            tracing::warn!("journal des comptes : {e:#}");
        }
        tracing::info!(station = %station.id, "nouvel appareil sur une Station : avis au propriétaire");
        let Some(mailer) = &self.mailer else { return };
        if !self.limits.lock().unwrap_or_else(|e| e.into_inner()).mail(&owner.email) {
            return;
        }
        let lang = if owner.lang == "en" { Lang::En } else { Lang::Fr };
        let link = format!("{}/compte?lang={}", self.public_url, lang.code());
        mailer.send(mail::compose(&owner.email, lang, Message::NewDevice { device: &device.name, computer: &station.name, account: &link }));
    }

    /// Le ménage des codes d'appareil.
    pub fn sweep_devices(&self) {
        self.devices.sweep();
        self.stations.sweep(self.now());
        if let Err(e) = self.db().sweep_device_codes(self.now()) {
            tracing::warn!("ménage des codes d'appareil : {e:#}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codes_de_televiseur() {
        for _ in 0..50 {
            let code = new_tv_code();
            assert_eq!(code.len(), 9);
            assert_eq!(code.as_bytes()[4], b'-');
            let n = normalize_tv_code(&code).unwrap();
            assert_eq!(dashed(&n), code);
            assert!(!code.contains(['A', 'E', 'I', 'O', 'U', 'Y', '0', '1']));
        }
        assert_eq!(normalize_tv_code(" bcdf ghjk "), Some("BCDFGHJK".into()));
        assert_eq!(normalize_tv_code("BCDF-GHJ"), None);
        assert_eq!(normalize_tv_code("BCDF-GHJA"), None, "A n'est pas dans l'alphabet");
        assert_eq!(clean_device_name(" Apple TV\n du salon "), "Apple TV du salon");
    }

    #[test]
    fn appareils_et_premieres_venues() {
        let s = Store::memory(0).unwrap();
        let a = s.create_account("a@exemple.org", "A", "h", "fr", Some(0), 0).unwrap().unwrap();
        let d = s.create_device("dev_1", b"jeton", a, "iPad", "app", 10).unwrap();
        assert_eq!(s.device_by_token(b"jeton").unwrap().unwrap().public_id, "dev_1");
        assert!(s.first_visit(d, "st1", 20).unwrap());
        assert!(!s.first_visit(d, "st1", 30).unwrap());
        assert!(s.first_visit(d, "st2", 30).unwrap());
        assert_eq!(s.devices_of(a).unwrap().len(), 1);
        assert!(!s.revoke_device(d, a + 1).unwrap());
        assert!(s.revoke_device(d, a).unwrap());
        assert!(s.device_by_token(b"jeton").unwrap().is_none());
        // Un code de téléviseur : en attente, approuvé une seule fois.
        s.insert_device_code(b"c", "tv", Some(b"p"), None, "TV", "pending", 0, 100).unwrap();
        assert!(s.device_code(b"c", "return", 10).unwrap().is_none(), "pas de cette sorte");
        assert!(s.answer_device_code(b"c", a, true, 10).unwrap());
        assert!(!s.answer_device_code(b"c", a, false, 10).unwrap(), "déjà répondu");
        assert_eq!(s.device_code_by_poll(b"p").unwrap().unwrap().status, "approved");
        s.sweep_device_codes(200).unwrap();
        assert!(s.device_code_by_poll(b"p").unwrap().is_none());
    }
}
