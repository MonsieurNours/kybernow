//! « Envoyer le journal » sur le téléviseur (docs/relais.md, « Dépôt d'un journal ») : le journal de l'app lu
//! (`kybernow.log.1` puis `kybernow.log`), chiffré (`crate::log_send::seal`) et déposé sur le relais
//! (`POST /v1/journal`), son état relu (`GET /v1/journal/<id>/state`) et le dépôt retiré à « Terminé »
//! (`DELETE /v1/journal/<id>`). Hors du fil de l'interface ; le relais est vérifié comme par le tunnel et les comptes
//! (autorités publiques, ou empreinte épinglée d'un relais chez soi).

use std::sync::mpsc::Sender;
use std::sync::Arc;
use std::time::{Duration, Instant};

use kybernow_core::relay::{RelayAddr, DEFAULT_RELAY};

use super::Event;
use crate::log_send::{page_url, seal, valid_id, LogDeposit, LogFail, LogRemote};

const CONNECT: Duration = Duration::from_secs(5);
const TIMEOUT: Duration = Duration::from_secs(30);

/// Le relais du compte (`relay`, vide : le relais public).
fn relay_addr(relay: &str) -> Option<RelayAddr> {
    let r = if relay.trim().is_empty() { DEFAULT_RELAY } else { relay.trim() };
    r.parse::<RelayAddr>().map_err(|e| log::warn!("journal : relais {r} illisible ({e})")).ok()
}

fn agent(relay: &RelayAddr) -> Option<ureq::Agent> {
    let tls = kybernow_tunnel::tls::client_config(relay, None, b"http/1.1", None).map_err(|e| log::warn!("journal : TLS du relais ({e})")).ok()?;
    Some(ureq::AgentBuilder::new().tls_config(Arc::new(tls)).timeout_connect(CONNECT).timeout(TIMEOUT).build())
}

/// Le journal de l'app, du plus ancien au plus récent : le lancement d'avant (gardé en `.1` au lancement, celui qui
/// a pu se fermer mal), puis celui-ci.
fn read_log() -> String {
    let dir = crate::model::Store::dir();
    let mut body = String::new();
    for name in ["kybernow.log.1", "kybernow.log"] {
        if let Ok(bytes) = std::fs::read(dir.join(name)) {
            body.push_str(&format!("===== {name} =====\n"));
            body.push_str(&String::from_utf8_lossy(&bytes));
            if !body.ends_with('\n') {
                body.push('\n');
            }
        }
    }
    body
}

/// Prépare, chiffre et dépose le journal ; dit à la boucle quand il part, puis le dépôt ou le refus.
pub fn send(attempt: u32, relay: String, header: String, tx: Sender<Event>) {
    std::thread::spawn(move || {
        let result = upload(attempt, &relay, &header, &tx);
        let _ = tx.send(Event::LogSent { attempt, result });
        super::sdl::wake_main();
    });
}

fn upload(attempt: u32, relay: &str, header: &str, tx: &Sender<Event>) -> Result<LogDeposit, LogFail> {
    let addr = relay_addr(relay).ok_or(LogFail::Unreachable)?;
    let (deposit, key) = seal(header, &read_log()).map_err(|e| {
        log::warn!("journal : chiffrement impossible ({e})");
        LogFail::Unreachable
    })?;
    let _ = tx.send(Event::LogUploading { attempt });
    super::sdl::wake_main();
    let agent = agent(&addr).ok_or(LogFail::Unreachable)?;
    let url = format!("https://{}/v1/journal", addr.web_authority());
    let resp = agent.post(&url).set("Content-Type", "application/octet-stream").send_bytes(&deposit).map_err(|e| match e {
        ureq::Error::Status(status, _) => {
            log::warn!("journal : dépôt refusé par le relais ({status})");
            match status {
                404 => LogFail::OldRelay,
                // 413 n'arrive pas (le journal est tenu dans la limite) ; 503 : relais plein, les dépôts partent au
                // bout d'une heure.
                413 | 429 | 503 => LogFail::TooMany,
                _ => LogFail::Unreachable,
            }
        }
        e => {
            log::warn!("journal : relais {} injoignable ({e})", addr.web_authority());
            LogFail::Unreachable
        }
    })?;
    let v: serde_json::Value = resp.into_json().unwrap_or_default();
    let (Some(id), Some(owner)) = (v["id"].as_str().filter(|id| valid_id(id)), v["owner"].as_str()) else {
        log::warn!("journal : réponse du relais illisible");
        return Err(LogFail::Unreachable);
    };
    log::info!("journal : déposé ({} octets)", deposit.len());
    let expires = Duration::from_secs(v["expires_in"].as_u64().unwrap_or(3600));
    Ok(LogDeposit { relay: relay.to_string(), id: id.to_string(), owner: owner.to_string(), url: page_url(&addr.web_authority(), id, &key), until: Instant::now() + expires })
}

/// L'état d'un dépôt, relu pour l'écran du QR code.
pub fn check(attempt: u32, relay: String, id: String, owner: String, tx: Sender<Event>) {
    std::thread::spawn(move || {
        let remote = state(&relay, &id, &owner);
        let _ = tx.send(Event::LogState { attempt, remote });
        super::sdl::wake_main();
    });
}

fn state(relay: &str, id: &str, owner: &str) -> LogRemote {
    let Some(addr) = relay_addr(relay) else { return LogRemote::Unknown };
    let Some(agent) = agent(&addr) else { return LogRemote::Unknown };
    match agent.get(&format!("https://{}/v1/journal/{id}/state", addr.web_authority())).set("x-kn-owner", owner).call() {
        Ok(resp) => match resp.into_json::<serde_json::Value>().ok().and_then(|v| v["state"].as_str().map(str::to_string)).as_deref() {
            Some("opened") => LogRemote::Opened,
            Some(_) => LogRemote::Waiting,
            None => LogRemote::Unknown,
        },
        Err(ureq::Error::Status(404, _)) => LogRemote::Gone,
        Err(_) => LogRemote::Unknown,
    }
}

/// « Terminé » : retire un dépôt pas encore ouvert (sans effet sinon).
pub fn discard(relay: String, id: String, owner: String) {
    std::thread::spawn(move || {
        let Some(addr) = relay_addr(&relay) else { return };
        let Some(agent) = agent(&addr) else { return };
        match agent.delete(&format!("https://{}/v1/journal/{id}", addr.web_authority())).set("x-kn-owner", &owner).call() {
            Ok(_) => log::info!("journal : dépôt retiré"),
            Err(ureq::Error::Status(404, _)) => {}
            Err(e) => log::warn!("journal : retrait impossible ({e})"),
        }
    });
}
