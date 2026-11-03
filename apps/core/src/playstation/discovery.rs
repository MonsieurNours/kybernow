//! Découverte et réveil des consoles PlayStation, d'après `discovery.c` de
//! chiaki-ng.
//!
//! Le dialogue est du texte façon HTTP sur UDP, IPv4 seulement :
//!
//! - **Recherche** : l'app envoie `SRCH * HTTP/1.1` en diffusion, vers le
//!   port 9302 (PS5) et le port 987 (PS4). Chaque console répond depuis ce
//!   port avec un statut (`200` : allumée, `620` : en mode repos) et des
//!   en-têtes `clé:valeur` (nom, identifiant, version du système, appli en
//!   cours…).
//! - **Réveil** : `WAKEUP * HTTP/1.1`, en direct vers l'adresse de la
//!   console, avec `user-credential`, la clé d'association (`RP-RegistKey`)
//!   lue comme un nombre hexadécimal. Le réveil demande donc une console
//!   déjà associée.
//!
//! Chaque paquet se termine par un octet nul, comme chez chiaki-ng. Le port
//! local est pris entre 9303 et 9319 (sinon au hasard), comme l'app de Sony.
//!
//! Par le relais, la recherche et le réveil vont aux ports du tunnel sur la
//! boucle locale ([`search_at`], [`wake_at`], [`super::endpoint::Endpoint`]).

use std::io;
use std::net::{IpAddr, Ipv4Addr, SocketAddr, UdpSocket};
use std::time::{Duration, Instant};

use super::endpoint::Endpoint;

/// Port de découverte d'une PS5.
pub const PORT_PS5: u16 = 9302;
/// Port de découverte d'une PS4.
pub const PORT_PS4: u16 = 987;
/// Version du protocole de découverte annoncée à une PS5.
pub const PROTOCOL_PS5: &str = "00030010";
/// Version du protocole de découverte annoncée à une PS4.
pub const PROTOCOL_PS4: &str = "00020020";
/// Ports locaux essayés dans l'ordre avant un port au hasard.
pub const LOCAL_PORTS: std::ops::RangeInclusive<u16> = 9303..=9319;

/// Le modèle de console, d'après la version du protocole de découverte.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Model {
    Ps5,
    Ps4,
}

impl Model {
    /// Port de découverte et de réveil.
    pub fn port(self) -> u16 {
        match self {
            Model::Ps5 => PORT_PS5,
            Model::Ps4 => PORT_PS4,
        }
    }

    /// Version du protocole de découverte à annoncer.
    pub fn protocol(self) -> &'static str {
        match self {
            Model::Ps5 => PROTOCOL_PS5,
            Model::Ps4 => PROTOCOL_PS4,
        }
    }

    /// Nom court, tel qu'écrit dans les apps.
    pub fn name(self) -> &'static str {
        match self {
            Model::Ps5 => "PS5",
            Model::Ps4 => "PS4",
        }
    }
}

/// État annoncé par la console.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    /// Allumée (`200`).
    Ready,
    /// En mode repos (`620`) : se réveille par [`wake`].
    Standby,
    /// Autre statut.
    Unknown,
}

/// Version du protocole de la session, déduite de la version du système.
/// Les noms suivent `ChiakiTarget` de chiaki-ng ; la session (bloc 3) en
/// dépend (clés et messages différents).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Target {
    Ps4Unknown,
    /// PS4 avant le système 7.0.
    Ps4V8,
    /// PS4 7.x.
    Ps4V9,
    /// PS4 8.0 et après.
    Ps4V10,
    Ps5Unknown,
    /// PS5 1.0 et après.
    Ps5V1,
}

/// Une console qui a répondu à la recherche.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Console {
    /// Adresse d'où vient la réponse.
    pub addr: Ipv4Addr,
    pub state: State,
    /// `host-id` : identifiant de la console (adresse matérielle en
    /// hexadécimal), stable ; c'est lui qu'on garde.
    pub host_id: String,
    /// `host-name` : le nom donné à la console.
    pub name: String,
    /// `host-type` : `PS5` ou `PS4`.
    pub host_type: String,
    /// `system-version`, par exemple `09060000`.
    pub system_version: String,
    /// `device-discovery-protocol-version`.
    pub protocol_version: String,
    /// `host-request-port` : port du dialogue de contrôle (997 en général).
    pub request_port: u16,
    /// `running-app-titleid` et `running-app-name` : le jeu en cours, s'il y
    /// en a un.
    pub running_title_id: Option<String>,
    pub running_app: Option<String>,
}

impl Console {
    /// PS5 ou PS4, d'après la version du protocole (comme chiaki-ng), à
    /// défaut d'après `host-type`.
    pub fn model(&self) -> Model {
        if self.protocol_version == PROTOCOL_PS5 {
            Model::Ps5
        } else if self.protocol_version.is_empty() && self.host_type.eq_ignore_ascii_case("PS5") {
            Model::Ps5
        } else {
            Model::Ps4
        }
    }

    /// Version du protocole de session (`chiaki_discovery_host_system_version_target`).
    pub fn target(&self) -> Target {
        let version: u64 = self.system_version.trim().parse().unwrap_or(0);
        let ps5 = self.model() == Model::Ps5;
        match () {
            _ if ps5 && version >= 8_050_001 => Target::Ps5V1,
            _ if ps5 && version >= 8_050_000 => Target::Ps5Unknown,
            _ if version >= 8_000_000 => Target::Ps4V10,
            _ if version >= 7_000_000 => Target::Ps4V9,
            _ if version > 0 => Target::Ps4V8,
            _ => Target::Ps4Unknown,
        }
    }
}

/// Texte de la recherche, octet nul compris.
pub fn search_packet(model: Model) -> Vec<u8> {
    let mut packet = format!(
        "SRCH * HTTP/1.1\ndevice-discovery-protocol-version:{}\n",
        model.protocol()
    )
    .into_bytes();
    packet.push(0);
    packet
}

/// Texte du réveil, octet nul compris.
pub fn wake_packet(model: Model, credential: u64) -> Vec<u8> {
    let mut packet = format!(
        "WAKEUP * HTTP/1.1\n\
         client-type:vr\n\
         auth-type:R\n\
         model:w\n\
         app-type:r\n\
         user-credential:{credential}\n\
         device-discovery-protocol-version:{}\n",
        model.protocol()
    )
    .into_bytes();
    packet.push(0);
    packet
}

/// La clé d'association (`RP-RegistKey`, texte hexadécimal terminé par des
/// octets nuls) lue comme un nombre, à la façon de `strtoull(key, NULL, 16)`
/// chez chiaki-ng : les chiffres du début, jusqu'au premier qui n'en est pas
/// un. `None` si la clé ne commence pas par un chiffre hexadécimal.
pub fn credential_from_regist_key(key: &[u8]) -> Option<u64> {
    let digits: Vec<u8> = key
        .iter()
        .copied()
        .skip_while(|b| b.is_ascii_whitespace())
        .take_while(|b| b.is_ascii_hexdigit())
        .collect();
    if digits.is_empty() {
        return None;
    }
    // strtoull sature à u64::MAX au-delà de 16 chiffres.
    let text = std::str::from_utf8(&digits).ok()?;
    Some(u64::from_str_radix(text, 16).unwrap_or(u64::MAX))
}

/// Lit une réponse de console. `None` si ce n'est pas une réponse HTTP.
pub fn parse_response(addr: Ipv4Addr, data: &[u8]) -> Option<Console> {
    let end = data.iter().position(|&b| b == 0).unwrap_or(data.len());
    let text = std::str::from_utf8(&data[..end]).ok()?;
    let mut lines = text.split('\n').map(|l| l.trim_end_matches('\r'));
    let status = lines.next()?;
    let mut parts = status.splitn(3, ' ');
    if !parts.next()?.starts_with("HTTP/") {
        return None;
    }
    let code: u16 = parts.next()?.trim().parse().ok()?;
    let state = match code {
        200 => State::Ready,
        620 => State::Standby,
        _ => State::Unknown,
    };
    let mut console = Console {
        addr,
        state,
        host_id: String::new(),
        name: String::new(),
        host_type: String::new(),
        system_version: String::new(),
        protocol_version: String::new(),
        request_port: 0,
        running_title_id: None,
        running_app: None,
    };
    for line in lines {
        let Some((key, value)) = line.split_once(':') else { continue };
        let value = value.trim().to_string();
        match key.trim() {
            "host-id" => console.host_id = value,
            "host-name" => console.name = value,
            "host-type" => console.host_type = value,
            "system-version" => console.system_version = value,
            "device-discovery-protocol-version" => console.protocol_version = value,
            "host-request-port" => console.request_port = value.parse().unwrap_or(0),
            "running-app-titleid" if !value.is_empty() => console.running_title_id = Some(value),
            "running-app-name" if !value.is_empty() => console.running_app = Some(value),
            _ => {}
        }
    }
    Some(console)
}

/// Ouvre la prise UDP : un port local entre 9303 et 9319, sinon au hasard,
/// diffusion permise.
fn open_socket() -> io::Result<UdpSocket> {
    let socket = LOCAL_PORTS
        .clone()
        .find_map(|port| UdpSocket::bind((Ipv4Addr::UNSPECIFIED, port)).ok())
        .map(Ok)
        .unwrap_or_else(|| UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0)))?;
    socket.set_broadcast(true)?;
    Ok(socket)
}

/// Cherche les consoles pendant `wait`. Les recherches PS5 et PS4 partent
/// en diffusion (255.255.255.255) et, en plus, en direct vers chaque
/// adresse de `direct` (console connue, ou réseau où la diffusion ne passe
/// pas) ; elles sont renvoyées toutes les 500 ms, car l'UDP peut se perdre.
/// Une console n'apparaît qu'une fois (par `host-id`, à défaut par adresse),
/// avec sa dernière réponse.
pub fn search(wait: Duration, direct: &[Ipv4Addr]) -> io::Result<Vec<Console>> {
    let targets: Vec<Endpoint> = direct.iter().copied().map(Endpoint::direct).collect();
    search_at(wait, true, &targets)
}

/// [`search`] vers des consoles jointes par leurs ports (`targets`), et en
/// diffusion si `broadcast`. Par le relais, l'adresse d'une réponse est
/// celle de la boucle locale : c'est l'identifiant qui compte.
pub fn search_at(wait: Duration, broadcast: bool, targets: &[Endpoint]) -> io::Result<Vec<Console>> {
    let socket = open_socket()?;
    let mut found: Vec<Console> = Vec::new();
    let deadline = Instant::now() + wait;
    let mut next_send = Instant::now();
    let mut buf = [0u8; 1024];
    loop {
        let now = Instant::now();
        if now >= deadline {
            break;
        }
        if now >= next_send {
            send_search(&socket, broadcast, targets);
            next_send = now + Duration::from_millis(500);
        }
        let timeout = deadline.min(next_send).saturating_duration_since(now).max(Duration::from_millis(1));
        socket.set_read_timeout(Some(timeout))?;
        match socket.recv_from(&mut buf) {
            Ok((len, SocketAddr::V4(from))) => {
                if let Some(console) = parse_response(*from.ip(), &buf[..len]) {
                    let same = |c: &Console| {
                        if console.host_id.is_empty() { c.addr == console.addr } else { c.host_id == console.host_id }
                    };
                    match found.iter_mut().find(|c| same(c)) {
                        Some(known) => *known = console,
                        None => {
                            log::info!(
                                "PlayStation : {} « {} » à {} ({:?}, système {})",
                                console.model().name(),
                                console.name,
                                console.addr,
                                console.state,
                                console.system_version
                            );
                            found.push(console);
                        }
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

fn send_search(socket: &UdpSocket, broadcast: bool, targets: &[Endpoint]) {
    let all = broadcast.then(|| Endpoint::direct(Ipv4Addr::BROADCAST));
    for target in all.iter().chain(targets) {
        for model in [Model::Ps5, Model::Ps4] {
            let to = target.search(model);
            if let Err(e) = socket.send_to(&search_packet(model), to) {
                log::debug!("PlayStation : recherche vers {to} refusée : {e}");
            }
        }
    }
}

/// Réveille une console associée, en direct vers son adresse (comme
/// chiaki-ng). `credential` : [`credential_from_regist_key`].
pub fn wake(addr: IpAddr, model: Model, credential: u64) -> io::Result<()> {
    let IpAddr::V4(addr) = addr else {
        return Err(io::Error::new(io::ErrorKind::Unsupported, "les consoles PlayStation ne parlent qu'en IPv4"));
    };
    wake_at(&Endpoint::direct(addr), model, credential)
}

/// [`wake`] vers une console jointe par ses ports (par le relais : le port
/// du tunnel pour le réveil).
pub fn wake_at(target: &Endpoint, model: Model, credential: u64) -> io::Result<()> {
    let socket = open_socket()?;
    socket.send_to(&wake_packet(model, credential), target.search(model))?;
    log::info!("PlayStation : réveil envoyé à {} ({}{})", model.name(), target.ip, if target.relayed { ", par le relais" } else { "" });
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn search_packet_matches_chiaki() {
        assert_eq!(
            search_packet(Model::Ps5),
            b"SRCH * HTTP/1.1\ndevice-discovery-protocol-version:00030010\n\0".to_vec()
        );
        assert_eq!(
            search_packet(Model::Ps4),
            b"SRCH * HTTP/1.1\ndevice-discovery-protocol-version:00020020\n\0".to_vec()
        );
    }

    #[test]
    fn wake_packet_matches_chiaki() {
        assert_eq!(
            wake_packet(Model::Ps5, 305419896),
            b"WAKEUP * HTTP/1.1\nclient-type:vr\nauth-type:R\nmodel:w\napp-type:r\n\
              user-credential:305419896\ndevice-discovery-protocol-version:00030010\n\0"
                .to_vec()
        );
    }

    #[test]
    fn credential_reads_hex_like_strtoull() {
        assert_eq!(credential_from_regist_key(b"12345678\0\0\0\0\0\0\0\0"), Some(0x1234_5678));
        assert_eq!(credential_from_regist_key(b"abcdef01"), Some(0xabcd_ef01));
        assert_eq!(credential_from_regist_key(b"\0\0"), None);
        assert_eq!(credential_from_regist_key(b"ffffffffffffffffff"), Some(u64::MAX));
    }

    #[test]
    fn parses_ps5_standby() {
        let reply = b"HTTP/1.1 620 Server Standby\n\
            host-id:A1B2C3D4E5F6\n\
            host-type:PS5\n\
            host-name:PS5-Salon\n\
            host-request-port:997\n\
            device-discovery-protocol-version:00030010\n\
            system-version:09060000\n\0";
        let c = parse_response(Ipv4Addr::new(192, 168, 1, 50), reply).unwrap();
        assert_eq!(c.state, State::Standby);
        assert_eq!(c.model(), Model::Ps5);
        assert_eq!(c.name, "PS5-Salon");
        assert_eq!(c.request_port, 997);
        assert_eq!(c.target(), Target::Ps5V1);
        assert_eq!(c.running_app, None);
    }

    #[test]
    fn parses_ps4_ready_with_game() {
        let reply = b"HTTP/1.1 200 Ok\r\n\
            host-id:001122334455\r\n\
            host-type:PS4\r\n\
            host-name:PS4\r\n\
            host-request-port:997\r\n\
            device-discovery-protocol-version:00020020\r\n\
            system-version:11000000\r\n\
            running-app-titleid:CUSA00001\r\n\
            running-app-name:Jeu: sous-titre\r\n";
        let c = parse_response(Ipv4Addr::new(10, 0, 0, 2), reply).unwrap();
        assert_eq!(c.state, State::Ready);
        assert_eq!(c.model(), Model::Ps4);
        assert_eq!(c.target(), Target::Ps4V10);
        assert_eq!(c.running_app.as_deref(), Some("Jeu: sous-titre"));
    }

    /// Une « console » sur la boucle locale, à un port choisi : la recherche et
    /// le réveil vont aux ports de l'adresse donnée (par le relais).
    #[test]
    fn search_and_wake_at_given_ports() {
        use super::super::endpoint::Ports;
        let console = UdpSocket::bind("127.0.0.1:0").unwrap();
        let port = console.local_addr().unwrap().port();
        let other = UdpSocket::bind("127.0.0.1:0").unwrap();
        let target = Endpoint { ip: Ipv4Addr::LOCALHOST, ports: Ports { ps5: port, ps4: other.local_addr().unwrap().port(), ..Ports::DIRECT }, relayed: true };
        let t = std::thread::spawn(move || {
            let mut buf = [0u8; 512];
            console.set_read_timeout(Some(Duration::from_secs(3))).unwrap();
            let (n, from) = console.recv_from(&mut buf).unwrap();
            assert_eq!(&buf[..n], &search_packet(Model::Ps5)[..]);
            let reply = b"HTTP/1.1 200 Ok\nhost-id:C0151B2DAE08\nhost-type:PS5\nhost-name:PS5\ndevice-discovery-protocol-version:00030010\n\0";
            console.send_to(reply, from).unwrap();
            // Puis le réveil, au même port.
            loop {
                let (n, _) = console.recv_from(&mut buf).unwrap();
                if buf[..n].starts_with(b"WAKEUP") {
                    return;
                }
            }
        });
        let found = search_at(Duration::from_millis(600), false, &[target]).unwrap();
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].host_id, "C0151B2DAE08");
        assert_eq!(found[0].state, State::Ready);
        wake_at(&target, Model::Ps5, 1).unwrap();
        t.join().unwrap();
        let mut buf = [0u8; 512];
        other.set_read_timeout(Some(Duration::from_millis(50))).unwrap();
        let n = other.recv(&mut buf).unwrap();
        assert_eq!(&buf[..n], &search_packet(Model::Ps4)[..], "PS4 au port de la PS4");
    }

    #[test]
    fn rejects_non_http() {
        assert_eq!(parse_response(Ipv4Addr::LOCALHOST, b"SRCH * HTTP/1.1\n"), None);
        assert_eq!(parse_response(Ipv4Addr::LOCALHOST, b"garbage"), None);
    }
}
