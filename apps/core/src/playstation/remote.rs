//! Une console jointe hors de chez soi, par une Station de son réseau et le
//! relais (docs/playstation.md, « Hors de chez soi » ; docs/relais.md,
//! « Consoles PlayStation »).
//!
//! [`Remote::open`] : un signe de vie à Admin par le relais
//! (`POST /api/consoles/beat`, qui autorise ce client du relais 30 s et dit
//! si la Station voit la console), puis un point d'écoute du tunnel sur la
//! boucle locale pour chaque port de la lecture à distance
//! ([`crate::relay::RelayTunnel::console_addr`]). L'[`Endpoint`] qui en sort
//! sert à la recherche, au réveil, à l'association et à la partie comme une
//! adresse sur le réseau. Tant que le `Remote` vit, le signe de vie repart
//! toutes les 10 s et le tunnel reste ouvert.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;

use super::discovery::{PORT_PS4, PORT_PS5};
use super::endpoint::{Endpoint, PORT_RP, PORT_TAKION, Ports};
use crate::console::{Console, ConsoleError, StationConsole};
use crate::relay::{RelayError, RelayTunnel, RouteInfo};

/// Le signe de vie repart à cet intervalle (l'autorisation d'Admin dure 30 s).
pub const BEAT_EVERY: Duration = Duration::from_secs(10);

/// Pourquoi la console n'est pas jointe par cette Station.
#[derive(Debug)]
pub enum RemoteError {
    /// La console d'Admin n'a pas de route par le relais.
    NotRelayed,
    /// La Station ne voit pas (ou plus) cette console sur son réseau.
    NotSeen,
    /// Admin ou le relais refuse, ou ne répond pas (accès coupé, Station
    /// absente du relais, compte refusé, Station d'avant les consoles : `404`).
    Console(ConsoleError),
}

impl std::fmt::Display for RemoteError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RemoteError::NotRelayed => f.write_str("pas de route par le relais"),
            RemoteError::NotSeen => f.write_str("la Station ne voit pas cette console"),
            RemoteError::Console(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for RemoteError {}

impl From<ConsoleError> for RemoteError {
    fn from(e: ConsoleError) -> Self {
        RemoteError::Console(e)
    }
}

impl From<RelayError> for RemoteError {
    fn from(e: RelayError) -> Self {
        RemoteError::Console(ConsoleError::Relay(e))
    }
}

struct Beat {
    stop: Mutex<bool>,
    wake: Condvar,
}

/// Une console jointe par une Station ; le lâcher arrête le signe de vie.
pub struct Remote {
    tunnel: RelayTunnel,
    endpoint: Endpoint,
    host_id: String,
    seen: Arc<Mutex<Option<StationConsole>>>,
    alive: Arc<AtomicBool>,
    beat: Arc<Beat>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl Remote {
    /// `console` : Admin de la Station **par le relais**
    /// ([`Console::with_relay`]) ; `host_id` : l'identifiant de la console.
    /// Bloquant (quelques secondes au plus) : hors du fil de l'interface.
    pub fn open(console: &Console, host_id: &str) -> Result<Remote, RemoteError> {
        let host_id = host_id.trim().to_ascii_uppercase();
        let tunnel = console.relay_tunnel().ok_or(RemoteError::NotRelayed)??;
        let beat_console = console.relay_twin().ok_or(RemoteError::NotRelayed)?;
        let list = console.consoles_beat()?;
        let seen = list.into_iter().find(|c| c.host_id.eq_ignore_ascii_case(&host_id)).ok_or(RemoteError::NotSeen)?;
        let ports = Ports {
            ps5: tunnel.console_addr(&host_id, PORT_PS5)?.port(),
            ps4: tunnel.console_addr(&host_id, PORT_PS4)?.port(),
            rp: tunnel.console_addr(&host_id, PORT_RP)?.port(),
            takion: tunnel.console_addr(&host_id, PORT_TAKION)?.port(),
        };
        let endpoint = Endpoint { ip: std::net::Ipv4Addr::LOCALHOST, ports, relayed: true };
        log::info!("PlayStation : console « {} » par la Station {} ({:?})", seen.name, tunnel.route().station, seen.state);
        let seen = Arc::new(Mutex::new(Some(seen)));
        let alive = Arc::new(AtomicBool::new(true));
        let beat = Arc::new(Beat { stop: Mutex::new(false), wake: Condvar::new() });
        let thread = {
            let (seen, alive, beat, host_id) = (seen.clone(), alive.clone(), beat.clone(), host_id.clone());
            std::thread::Builder::new()
                .name("ps-relais".into())
                .spawn(move || beat_loop(beat_console, &host_id, &seen, &alive, &beat))
                .ok()
        };
        Ok(Remote { tunnel, endpoint, host_id, seen, alive, beat, thread })
    }

    /// Où joindre la console : la boucle locale et les ports du tunnel.
    pub fn endpoint(&self) -> Endpoint {
        self.endpoint
    }

    pub fn host_id(&self) -> &str {
        &self.host_id
    }

    /// La console telle que la Station la voit au dernier signe de vie
    /// (`state` : `ready`, `standby`) ; `None` si elle ne la voit plus.
    pub fn seen(&self) -> Option<StationConsole> {
        self.seen.lock().unwrap().clone()
    }

    /// Le dernier signe de vie a abouti (Admin autorise encore ce client).
    pub fn alive(&self) -> bool {
        self.alive.load(Ordering::Relaxed)
    }

    /// Le chemin, pour la puce « Par le relais » et la ligne « Chemin ».
    pub fn route_info(&self) -> RouteInfo {
        self.tunnel.route_info()
    }

    pub fn tunnel(&self) -> &RelayTunnel {
        &self.tunnel
    }
}

impl Drop for Remote {
    fn drop(&mut self) {
        *self.beat.stop.lock().unwrap() = true;
        self.beat.wake.notify_all();
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

fn beat_loop(console: Console, host_id: &str, seen: &Mutex<Option<StationConsole>>, alive: &AtomicBool, beat: &Beat) {
    loop {
        {
            let stop = beat.stop.lock().unwrap();
            let (stop, _) = beat.wake.wait_timeout_while(stop, BEAT_EVERY, |s| !*s).unwrap();
            if *stop {
                return;
            }
        }
        match console.consoles_beat() {
            Ok(list) => {
                alive.store(true, Ordering::Relaxed);
                *seen.lock().unwrap() = list.into_iter().find(|c| c.host_id.eq_ignore_ascii_case(host_id));
            }
            Err(e) => {
                alive.store(false, Ordering::Relaxed);
                log::warn!("PlayStation : signe de vie à la Station refusé : {e}");
            }
        }
    }
}

/// Une console par une Station et le relais, sur la boucle locale : le faux
/// relais de `relay::essai` joue aussi la Station, qui fait suivre les vrais
/// ports de la console vers une fausse console (recherche, réveil,
/// association, session, Takion).
#[cfg(test)]
mod tests {
    use std::net::{SocketAddr, TcpListener, UdpSocket};
    use std::sync::Arc;
    use std::sync::mpsc;
    use std::time::Duration;

    use super::*;
    use crate::playstation::discovery::{self, Model, State};
    use crate::relay::essai::{CONSOLE, start_with};

    /// TCP puis UDP sur le même numéro de la boucle locale.
    fn pair() -> (TcpListener, UdpSocket, SocketAddr) {
        loop {
            let tcp = TcpListener::bind("127.0.0.1:0").unwrap();
            let addr = tcp.local_addr().unwrap();
            if let Ok(udp) = UdpSocket::bind(addr) {
                return (tcp, udp, addr);
            }
        }
    }

    fn console_list(state: &str) -> String {
        format!(r#"{{"consoles":[{{"host_id":"{CONSOLE}","name":"PS5 de l'essai","model":"ps5","state":"{state}","seen":1}}]}}"#)
    }

    #[test]
    fn une_console_par_la_station() {
        // La fausse console : recherche et réveil (9302), association et session (9295), Takion (9296).
        let search = UdpSocket::bind("127.0.0.1:0").unwrap();
        let (rp_tcp, rp_udp, rp) = pair();
        let takion = UdpSocket::bind("127.0.0.1:0").unwrap();
        let (woken_tx, woken) = mpsc::channel();
        let search_addr = search.local_addr().unwrap();
        std::thread::spawn(move || {
            let mut buf = [0u8; 2048];
            while let Ok((n, from)) = search.recv_from(&mut buf) {
                if buf[..n].starts_with(b"SRCH") {
                    let reply = format!("HTTP/1.1 620 Server Standby\nhost-id:{CONSOLE}\nhost-type:PS5\nhost-name:PS5 de l'essai\ndevice-discovery-protocol-version:00030010\n\0");
                    let _ = search.send_to(reply.as_bytes(), from);
                } else if buf[..n].starts_with(b"WAKEUP") {
                    let _ = woken_tx.send(());
                }
            }
        });
        std::thread::spawn(move || {
            let mut buf = [0u8; 2048];
            while let Ok((n, from)) = rp_udp.recv_from(&mut buf) {
                if buf[..n].starts_with(b"SRC3") {
                    let _ = rp_udp.send_to(b"RES3", from);
                }
            }
        });
        std::thread::spawn(move || {
            for mut c in rp_tcp.incoming().flatten() {
                std::thread::spawn(move || {
                    let _ = std::io::copy(&mut c.try_clone().unwrap(), &mut c);
                });
            }
        });
        let takion_addr = takion.local_addr().unwrap();
        std::thread::spawn(move || {
            let mut buf = [0u8; 4096];
            while let Ok((n, from)) = takion.recv_from(&mut buf) {
                let _ = takion.send_to(&buf[..n], from);
            }
        });
        let bench = start_with(
            Arc::new(|method, path| match (method, path) {
                ("POST", "/api/consoles/beat") | ("GET", "/api/consoles") => (200, console_list("standby")),
                _ => (404, r#"{"error":"inconnu"}"#.into()),
            }),
            vec![(PORT_PS5, search_addr), (PORT_RP, rp), (PORT_TAKION, takion_addr)],
        );
        let admin = Console::new("192.0.2.1", 9, "u", "p", None).with_relay(Some(bench.route()));
        assert_eq!(admin.consoles().unwrap()[0].host_id, CONSOLE, "GET /api/consoles par le relais");

        // Sans relais, pas de Remote ; console que la Station ne voit pas : refusée.
        assert!(matches!(Remote::open(&Console::new("192.0.2.1", 9, "u", "p", None), CONSOLE), Err(RemoteError::NotRelayed)));
        assert!(matches!(Remote::open(&admin, "AAAAAAAAAAAA"), Err(RemoteError::NotSeen)));

        let remote = Remote::open(&admin, &CONSOLE.to_ascii_lowercase()).expect("console par la Station");
        let at = remote.endpoint();
        assert!(at.relayed && at.ip.is_loopback());
        assert_eq!(at.mtu(), 1200, "des paquets qui tiennent dans un datagramme du tunnel");
        assert_eq!(remote.seen().map(|c| c.state), Some("standby".into()));
        assert!(remote.route_info().via_relay());

        // État et réveil par les ports du tunnel.
        let found = discovery::search_at(Duration::from_millis(800), false, &[at]).unwrap();
        assert_eq!(found.iter().map(|c| (c.host_id.as_str(), c.state)).collect::<Vec<_>>(), vec![(CONSOLE, State::Standby)]);
        discovery::wake_at(&at, Model::Ps5, 7).unwrap();
        woken.recv_timeout(Duration::from_secs(5)).expect("réveil arrivé à la console");

        // La session (TCP 9295) et la recherche de l'association (UDP 9295), même numéro.
        use std::io::{Read, Write};
        let mut tcp = std::net::TcpStream::connect(SocketAddr::V4(at.rp())).unwrap();
        tcp.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
        tcp.write_all(b"GET /sie/ps5/rp/sess/init").unwrap();
        let mut back = [0u8; 25];
        tcp.read_exact(&mut back).unwrap();
        assert_eq!(&back, b"GET /sie/ps5/rp/sess/init");
        let udp = UdpSocket::bind("127.0.0.1:0").unwrap();
        udp.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
        udp.connect(SocketAddr::V4(at.rp())).unwrap();
        udp.send(b"SRC3\0").unwrap();
        let mut buf = [0u8; 2048];
        let n = udp.recv(&mut buf).unwrap();
        assert_eq!(&buf[..n], b"RES3");

        // Takion : des paquets de 1 200 octets passent dans les deux sens.
        let t = UdpSocket::bind("127.0.0.1:0").unwrap();
        t.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
        t.connect(SocketAddr::V4(at.takion())).unwrap();
        for i in 0..5u8 {
            let packet = vec![i; 1200];
            t.send(&packet).unwrap();
            let n = t.recv(&mut buf).unwrap();
            assert_eq!(&buf[..n], &packet[..]);
        }

        // Un port que la Station ne fait pas suivre : refusé, rien ne revient.
        let ps4 = UdpSocket::bind("127.0.0.1:0").unwrap();
        ps4.set_read_timeout(Some(Duration::from_millis(500))).unwrap();
        ps4.connect(SocketAddr::V4(at.search(Model::Ps4))).unwrap();
        ps4.send(&discovery::search_packet(Model::Ps4)).unwrap();
        assert!(ps4.recv(&mut buf).is_err());
        drop(remote);
    }

    /// Avec une vraie console (pas lancé d'office : `cargo test -p
    /// kybernow-core vraie_console -- --ignored --nocapture`, avec `PS_STATE`,
    /// l'état de `examples/playstation.rs`, et `PS_ADDR`, l'adresse de la
    /// console) : le faux relais fait suivre les ports de la lecture à
    /// distance vers la console, comme une Station ; l'état, le réveil et la
    /// partie passent par le tunnel, MTU de 1 200 octets. Rien n'est écrit
    /// des clés ni des identifiants.
    #[test]
    #[ignore]
    fn vraie_console_par_le_relais() {
        use crate::media::{Packet, PacketHandler};
        use crate::playstation::play::{Play, PlayOptions, PlayState};
        use crate::playstation::regist::Registered;
        use std::sync::atomic::AtomicU64;
        let (Ok(state), Ok(addr)) = (std::env::var("PS_STATE"), std::env::var("PS_ADDR")) else {
            eprintln!("PS_STATE et PS_ADDR absents : essai sauté");
            return;
        };
        let addr: std::net::Ipv4Addr = addr.parse().expect("PS_ADDR");
        let state: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(state).expect("PS_STATE")).expect("état");
        let direct = discovery::search(Duration::from_secs(2), &[addr]).expect("recherche");
        let console = direct.into_iter().find(|c| c.addr == addr).expect("console muette sur le réseau");
        let host_id = console.host_id.to_ascii_uppercase();
        let registered: Registered = state["consoles"]
            .as_object()
            .and_then(|m| m.iter().find(|(k, _)| k.eq_ignore_ascii_case(&host_id)))
            .and_then(|(_, v)| serde_json::from_value(v.clone()).ok())
            .expect("console pas associée dans PS_STATE");
        let list = format!(r#"{{"consoles":[{{"host_id":"{host_id}","name":"PS5","model":"ps5","state":"ready","seen":1}}]}}"#);
        let to = |p: u16| SocketAddr::new(std::net::IpAddr::V4(addr), p);
        let bench = crate::relay::essai::start_console(
            Arc::new(move |method, path| match (method, path) {
                ("POST", "/api/consoles/beat") | ("GET", "/api/consoles") => (200, list.clone()),
                _ => (404, "{}".into()),
            }),
            &host_id,
            vec![(PORT_PS5, to(PORT_PS5)), (PORT_PS4, to(PORT_PS4)), (PORT_RP, to(PORT_RP)), (PORT_TAKION, to(PORT_TAKION))],
        );
        let admin = Console::new("192.0.2.1", 9, "u", "p", None).with_relay(Some(bench.route()));
        let remote = Remote::open(&admin, &host_id).expect("console par la Station");
        let at = remote.endpoint();
        let seen = discovery::search_at(Duration::from_millis(1500), false, &[at]).expect("recherche par le relais");
        let seen = seen.into_iter().find(|c| c.host_id.eq_ignore_ascii_case(&host_id)).expect("état par le relais");
        eprintln!("état par le relais : {:?}", seen.state);
        if seen.state == State::Standby {
            discovery::wake_at(&at, Model::Ps5, registered.wake_credential().expect("clé de réveil")).unwrap();
            let deadline = std::time::Instant::now() + Duration::from_secs(60);
            loop {
                let s = discovery::search_at(Duration::from_millis(1500), false, &[at]).unwrap_or_default();
                if s.iter().any(|c| c.state == State::Ready) {
                    break;
                }
                assert!(std::time::Instant::now() < deadline, "la console ne se réveille pas par le relais");
            }
            eprintln!("réveillée par le relais");
        }
        struct Count(Arc<AtomicU64>, Arc<AtomicU64>);
        impl PacketHandler for Count {
            fn packet(&self, p: Packet<'_>) {
                if let Packet::Media { data, .. } = p {
                    self.0.fetch_add(1, Ordering::Relaxed);
                    self.1.fetch_max(data.len() as u64, Ordering::Relaxed);
                }
            }
        }
        let (frames, biggest) = (Arc::new(AtomicU64::new(0)), Arc::new(AtomicU64::new(0)));
        let (states_tx, states) = mpsc::channel::<String>();
        let states_tx = std::sync::Mutex::new(states_tx);
        let mut play = Play::start(
            at,
            &registered,
            PlayOptions { settings: Default::default(), hevc_decoder: true },
            Arc::new(Count(frames.clone(), biggest.clone())),
            None,
            Box::new(move |s: PlayState| {
                let _ = states_tx.lock().unwrap().send(format!("{s:?}"));
            }),
            Box::new(|_| {}),
        );
        let deadline = std::time::Instant::now() + Duration::from_secs(40);
        while frames.load(Ordering::Relaxed) < 300 && std::time::Instant::now() < deadline {
            if let Ok(s) = states.recv_timeout(Duration::from_millis(200)) {
                eprintln!("partie : {s}");
                assert!(!s.starts_with("Ended"), "partie finie par le relais : {s}");
            }
        }
        let stats = play.stats();
        // La console était en mode repos : elle y retourne (`PS_REST=1` aussi).
        if seen.state == State::Standby || std::env::var("PS_REST").is_ok_and(|v| v == "1") {
            play.goto_bed();
            std::thread::sleep(Duration::from_secs(2));
            eprintln!("console remise en mode repos par le relais");
        }
        play.stop();
        eprintln!(
            "par le relais : {} images, la plus grande {} octets, {:?}, chemin {:?}",
            frames.load(Ordering::Relaxed),
            biggest.load(Ordering::Relaxed),
            stats,
            remote.route_info().transport
        );
        assert!(frames.load(Ordering::Relaxed) >= 300, "moins de 300 images en 40 s");
    }
}
