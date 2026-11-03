//! Les consoles PlayStation du réseau de la Station (docs/relais.md,
//! « Consoles PlayStation ») : Admin les cherche, les annonce aux appareils
//! appairés (`GET /api/consoles`) et fait suivre, par le relais, les paquets
//! d'une partie hors de chez soi (`relay.rs`, flux `to: "console"`).
//!
//! - **Recherche** : `SRCH` en diffusion vers les ports 9302 (PS5) et 987
//!   (PS4), et en direct vers les consoles déjà vues, toutes les 30 s, et à
//!   la demande (`GET /api/consoles` quand la dernière a plus de 10 s). Le
//!   texte est celui du cœur des apps (`apps/core/src/playstation/discovery.rs`,
//!   d'après chiaki-ng) ; cette copie réduite lit seulement ce qu'Admin
//!   annonce (identifiant, nom, modèle, état) et l'adresse d'où vient la réponse.
//! - **Jamais l'adresse** d'une console dans une réponse : la Station n'est
//!   pas un mandataire ouvert. Un flux ne va qu'à l'adresse trouvée ici pour
//!   son identifiant, et seulement aux ports de la lecture à distance.
//! - **Autorisation** : un appareil appairé, par le relais, avec « Accès hors
//!   de chez vous », renouvelée par `POST /api/consoles/beat` (30 s) ; un
//!   invité de KyberNow Share n'y a jamais accès (compte Basic exigé).

use std::collections::HashMap;
use std::io;
use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4, UdpSocket};
use std::sync::Mutex;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use axum::Json;
use axum::extract::State;
use axum::http::HeaderMap;
use serde_json::{Value, json};

use crate::AppState;
use crate::api::ApiResult;

/// Port de découverte et de réveil d'une PS5.
pub const PORT_PS5: u16 = 9302;
/// Port de découverte et de réveil d'une PS4.
pub const PORT_PS4: u16 = 987;
/// Ports TCP qu'un appareil peut joindre par le relais : session, canal de
/// contrôle et association (9295), dialogue de la PS4 (997).
pub const TCP_PORTS: [u16; 2] = [9295, 997];
/// Ports UDP : recherche de l'association (9295), Takion (9296), état et réveil (9302, 987).
pub const UDP_PORTS: [u16; 4] = [9295, 9296, PORT_PS5, PORT_PS4];
/// Recherche de fond.
const SEARCH_EVERY: Duration = Duration::from_secs(30);
/// `GET /api/consoles` cherche de nouveau quand la dernière recherche a plus que ça.
const FRESH: Duration = Duration::from_secs(10);
/// Une console muette depuis ce temps n'est plus annoncée.
const FORGET_AFTER: Duration = Duration::from_secs(5 * 60);
/// Attente des réponses : de fond, à la demande.
const WAIT_BACKGROUND: Duration = Duration::from_millis(1500);
const WAIT_ON_DEMAND: Duration = Duration::from_millis(1000);
/// Ports locaux essayés avant un port au hasard, comme l'app de Sony.
const LOCAL_PORTS: std::ops::RangeInclusive<u16> = 9303..=9319;

/// PS5 ou PS4.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Model {
    Ps5,
    Ps4,
}

impl Model {
    fn protocol(self) -> &'static str {
        match self {
            Model::Ps5 => "00030010",
            Model::Ps4 => "00020020",
        }
    }

    fn port(self) -> u16 {
        match self {
            Model::Ps5 => PORT_PS5,
            Model::Ps4 => PORT_PS4,
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Model::Ps5 => "ps5",
            Model::Ps4 => "ps4",
        }
    }
}

/// Une console qui a répondu.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Found {
    /// `host-id`, en majuscules.
    pub host_id: String,
    pub name: String,
    pub model: Model,
    /// Allumée (`200`) ; sinon en mode repos (`620`) ou autre.
    pub ready: bool,
    pub standby: bool,
    pub addr: Ipv4Addr,
}

/// Texte de la recherche, octet nul compris (le même que le cœur des apps).
pub fn search_packet(model: Model) -> Vec<u8> {
    let mut p = format!("SRCH * HTTP/1.1\ndevice-discovery-protocol-version:{}\n", model.protocol()).into_bytes();
    p.push(0);
    p
}

/// Lit une réponse : `None` sans statut HTTP ni `host-id`.
pub fn parse_response(addr: Ipv4Addr, data: &[u8]) -> Option<Found> {
    let end = data.iter().position(|&b| b == 0).unwrap_or(data.len());
    let text = std::str::from_utf8(&data[..end]).ok()?;
    let mut lines = text.split('\n').map(|l| l.trim_end_matches('\r'));
    let mut status = lines.next()?.splitn(3, ' ');
    if !status.next()?.starts_with("HTTP/") {
        return None;
    }
    let code: u16 = status.next()?.trim().parse().ok()?;
    let (mut host_id, mut name, mut host_type, mut protocol) = (String::new(), String::new(), String::new(), String::new());
    for line in lines {
        let Some((k, v)) = line.split_once(':') else { continue };
        let v = v.trim().to_string();
        match k.trim() {
            "host-id" => host_id = v,
            "host-name" => name = v,
            "host-type" => host_type = v,
            "device-discovery-protocol-version" => protocol = v,
            _ => {}
        }
    }
    if host_id.is_empty() {
        return None;
    }
    // Comme le cœur : d'après la version du protocole, à défaut `host-type`.
    let model = if protocol == Model::Ps5.protocol() || (protocol.is_empty() && host_type.eq_ignore_ascii_case("PS5")) {
        Model::Ps5
    } else {
        Model::Ps4
    };
    Some(Found { host_id: host_id.to_ascii_uppercase(), name, model, ready: code == 200, standby: code == 620, addr })
}

/// Cherche pendant `wait` : en diffusion et vers `direct`, renvoyé toutes
/// les 500 ms (l'UDP peut se perdre). Bloquant.
pub fn search(wait: Duration, direct: &[Ipv4Addr]) -> io::Result<Vec<Found>> {
    let socket = LOCAL_PORTS
        .clone()
        .find_map(|p| UdpSocket::bind((Ipv4Addr::UNSPECIFIED, p)).ok())
        .map(Ok)
        .unwrap_or_else(|| UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0)))?;
    socket.set_broadcast(true)?;
    let deadline = Instant::now() + wait;
    let mut next_send = Instant::now();
    let mut found: Vec<Found> = Vec::new();
    let mut buf = [0u8; 1024];
    loop {
        let now = Instant::now();
        if now >= deadline {
            break;
        }
        if now >= next_send {
            for addr in std::iter::once(Ipv4Addr::BROADCAST).chain(direct.iter().copied()) {
                for model in [Model::Ps5, Model::Ps4] {
                    let _ = socket.send_to(&search_packet(model), SocketAddrV4::new(addr, model.port()));
                }
            }
            next_send = now + Duration::from_millis(500);
        }
        socket.set_read_timeout(Some(deadline.min(next_send).saturating_duration_since(now).max(Duration::from_millis(1))))?;
        match socket.recv_from(&mut buf) {
            Ok((n, SocketAddr::V4(from))) => {
                if let Some(c) = parse_response(*from.ip(), &buf[..n]) {
                    match found.iter_mut().find(|f| f.host_id == c.host_id) {
                        Some(f) => *f = c,
                        None => found.push(c),
                    }
                }
            }
            Ok(_) => {}
            Err(e) if matches!(e.kind(), io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut) => {}
            Err(e) => return Err(e),
        }
    }
    Ok(found)
}

struct Seen {
    found: Found,
    at: Instant,
    unix: u64,
}

/// Les consoles vues, par identifiant.
#[derive(Default)]
pub struct Consoles {
    seen: Mutex<HashMap<String, Seen>>,
    last: Mutex<Option<Instant>>,
    /// Une recherche à la fois.
    searching: tokio::sync::Mutex<()>,
}

fn now_unix() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

impl Consoles {
    /// Ce que dit `GET /api/consoles` (jamais l'adresse).
    pub fn list(&self) -> Value {
        let seen = self.seen.lock().unwrap();
        let mut list: Vec<&Seen> = seen.values().filter(|s| s.at.elapsed() < FORGET_AFTER).collect();
        list.sort_by(|a, b| a.found.name.cmp(&b.found.name).then(a.found.host_id.cmp(&b.found.host_id)));
        let consoles: Vec<Value> = list
            .iter()
            .map(|s| {
                let state = if s.found.ready {
                    "ready"
                } else if s.found.standby {
                    "standby"
                } else {
                    "unknown"
                };
                json!({ "host_id": s.found.host_id, "name": s.found.name, "model": s.found.model.as_str(), "state": state, "seen": s.unix })
            })
            .collect();
        json!({ "consoles": consoles })
    }

    /// L'adresse trouvée ici pour cet identifiant (console vue il y a moins de 5 minutes).
    pub fn addr_of(&self, host_id: &str) -> Option<Ipv4Addr> {
        let seen = self.seen.lock().unwrap();
        seen.get(&host_id.to_ascii_uppercase()).filter(|s| s.at.elapsed() < FORGET_AFTER).map(|s| s.found.addr)
    }

    /// Retient les consoles d'une recherche.
    pub fn note(&self, found: Vec<Found>) {
        let mut seen = self.seen.lock().unwrap();
        let now = Instant::now();
        for f in found {
            let new = !seen.contains_key(&f.host_id);
            if new {
                tracing::info!("consoles : {} « {} » sur le réseau", f.model.as_str(), f.name);
            }
            seen.insert(f.host_id.clone(), Seen { found: f, at: now, unix: now_unix() });
        }
        seen.retain(|_, s| s.at.elapsed() < FORGET_AFTER * 2);
    }

    /// Une recherche (diffusion et consoles connues).
    pub async fn refresh(&self, wait: Duration) {
        let _one = self.searching.lock().await;
        let direct: Vec<Ipv4Addr> = self.seen.lock().unwrap().values().map(|s| s.found.addr).collect();
        match tokio::task::spawn_blocking(move || search(wait, &direct)).await {
            Ok(Ok(found)) => self.note(found),
            Ok(Err(e)) => tracing::debug!("consoles : recherche impossible : {e}"),
            Err(_) => {}
        }
        *self.last.lock().unwrap() = Some(Instant::now());
    }

    /// À la demande : une recherche si la dernière a plus de 10 s.
    pub async fn fresh(&self) {
        let stale = self.last.lock().unwrap().is_none_or(|t| t.elapsed() >= FRESH);
        if stale {
            self.refresh(WAIT_ON_DEMAND).await;
        }
    }
}

/// La recherche de fond, toutes les 30 s.
pub fn supervise(s: AppState) {
    tokio::spawn(async move {
        loop {
            s.consoles.refresh(WAIT_BACKGROUND).await;
            tokio::time::sleep(SEARCH_EVERY).await;
        }
    });
}

/// `GET /api/consoles` : un appareil appairé (compte Basic), sur le réseau ou
/// par le relais (« Accès hors de chez vous », garde de `relay.rs`).
pub async fn get(State(s): State<AppState>, headers: HeaderMap) -> ApiResult<Json<Value>> {
    crate::api::stream_user(&s, &headers)?;
    s.consoles.fresh().await;
    Ok(Json(s.consoles.list()))
}

/// `POST /api/consoles/beat` : par le relais, ce client joint les consoles de
/// la Station 30 s de plus (état, réveil, partie) ; la réponse est celle de
/// `GET /api/consoles`. Sur le réseau, rien à autoriser.
pub async fn beat(State(s): State<AppState>, headers: HeaderMap, via: Option<axum::Extension<crate::relay::Via>>) -> ApiResult<Json<Value>> {
    crate::api::stream_user(&s, &headers)?;
    if let Some(axum::Extension(via)) = &via {
        s.relay.grant_console(via);
    }
    s.consoles.fresh().await;
    Ok(Json(s.consoles.list()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recherche_comme_le_coeur() {
        assert_eq!(search_packet(Model::Ps5), b"SRCH * HTTP/1.1\ndevice-discovery-protocol-version:00030010\n\0".to_vec());
        assert_eq!(search_packet(Model::Ps4), b"SRCH * HTTP/1.1\ndevice-discovery-protocol-version:00020020\n\0".to_vec());
    }

    #[test]
    fn reponses_lues() {
        let reply = b"HTTP/1.1 620 Server Standby\nhost-id:c0151b2dae08\nhost-type:PS5\nhost-name:PS5 de Matthieu\n\
            host-request-port:997\ndevice-discovery-protocol-version:00030010\nsystem-version:09060000\n\0";
        let f = parse_response(Ipv4Addr::new(192, 168, 1, 110), reply).unwrap();
        assert_eq!((f.host_id.as_str(), f.name.as_str(), f.model, f.ready, f.standby), ("C0151B2DAE08", "PS5 de Matthieu", Model::Ps5, false, true));
        let ps4 = parse_response(Ipv4Addr::new(10, 0, 0, 2), b"HTTP/1.1 200 Ok\r\nhost-id:001122334455\r\nhost-type:PS4\r\n").unwrap();
        assert_eq!((ps4.model, ps4.ready), (Model::Ps4, true));
        assert_eq!(parse_response(Ipv4Addr::LOCALHOST, b"SRCH * HTTP/1.1\n"), None);
        assert_eq!(parse_response(Ipv4Addr::LOCALHOST, b"HTTP/1.1 200 Ok\nhost-name:sans id\n"), None);
    }

    #[test]
    fn jamais_l_adresse_dans_la_liste() {
        let c = Consoles::default();
        c.note(vec![Found { host_id: "C0151B2DAE08".into(), name: "PS5".into(), model: Model::Ps5, ready: true, standby: false, addr: Ipv4Addr::new(192, 168, 1, 110) }]);
        let list = c.list();
        assert_eq!(list["consoles"][0]["host_id"], "C0151B2DAE08");
        assert_eq!(list["consoles"][0]["model"], "ps5");
        assert_eq!(list["consoles"][0]["state"], "ready");
        assert!(!list.to_string().contains("192.168"), "{list}");
        assert_eq!(c.addr_of("c0151b2dae08"), Some(Ipv4Addr::new(192, 168, 1, 110)));
        assert_eq!(c.addr_of("AAAAAAAAAAAA"), None);
    }
}
