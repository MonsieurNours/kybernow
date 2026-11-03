//! Envoi des comptes PlayStation et de leurs associations d'un appareil à
//! l'autre, par le réseau (docs/playstation.md, « Plusieurs appareils ») :
//! une télé sans navigateur reçoit ce qu'un Mac, un PC ou un téléphone a déjà.
//!
//! - L'appareil qui attend ([`Receiver`]) écoute en TCP sur un port au hasard
//!   et répond aux recherches en UDP sur le port [`PORT`] : `KNPS-HANDOFF?` →
//!   `{"name","platform","port"}`. Il affiche un code à 6 chiffres.
//! - L'appareil qui envoie trouve les appareils en attente ([`discover`]),
//!   puis [`send`] le paquet avec le code tapé.
//! - Poignée de main SPAKE2 (Ed25519) sur le code : le code ne circule
//!   jamais, et un code faux ne révèle rien (un seul essai par connexion).
//!   Puis confirmation des clés (HMAC-SHA256) et le paquet en AES-256-GCM.
//! - Après 3 codes faux, nouveau code ; plus d'écoute après 5 minutes.
//!
//! Le paquet est un JSON opaque pour le cœur (son format est dans le contrat) :
//! les apps le font et le lisent.
//!
//! Trames TCP : `[longueur, 4 octets gros-boutistes][contenu]`.
//! 1. envoyeur → message SPAKE2 A ; 2. receveur → message SPAKE2 B ;
//! 3. envoyeur → HMAC(clé, "kn-handoff sender") ; 4. receveur → HMAC(clé,
//! "kn-handoff receiver"), ou une trame vide si le code est faux ;
//! 5. envoyeur → nonce (12 octets) + paquet chiffré ; 6. receveur → `ok`.

use std::io::{self, Read, Write};
use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4, TcpListener, TcpStream, UdpSocket};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver as Rx, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Nonce};
use hmac::{Hmac, Mac};
use sha2::{Digest, Sha256};
use spake2::{Ed25519Group, Identity, Password, Spake2};

/// Port UDP des recherches.
pub const PORT: u16 = 9340;
const QUERY: &[u8] = b"KNPS-HANDOFF?";
const ID_SENDER: &[u8] = b"kn-handoff sender";
const ID_RECEIVER: &[u8] = b"kn-handoff receiver";
/// Plus d'écoute après ce délai.
pub const LIFETIME: Duration = Duration::from_secs(5 * 60);
/// Codes faux avant un nouveau code.
pub const TRIES: u32 = 3;
/// Taille maximale d'une trame (le paquet tient en quelques Ko).
const FRAME_MAX: usize = 1 << 20;
const IO_TIMEOUT: Duration = Duration::from_secs(10);

/// Un appareil en attente d'un envoi.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Waiting {
    pub name: String,
    pub platform: String,
    pub addr: SocketAddr,
}

/// Ce que dit l'appareil qui attend.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    /// Le paquet reçu (JSON).
    Received(String),
    /// Trois codes faux : voici le nouveau.
    NewCode(String),
    /// Cinq minutes passées : plus d'écoute.
    Expired,
}

/// Pourquoi un envoi n'a pas abouti.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SendError {
    /// Code faux.
    Refused,
    /// L'appareil ne répond plus.
    Unreachable(String),
}

impl std::fmt::Display for SendError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SendError::Refused => write!(f, "code refusé"),
            SendError::Unreachable(why) => write!(f, "injoignable : {why}"),
        }
    }
}

/// Un code à 6 chiffres au hasard.
fn new_code() -> String {
    let mut b = [0u8; 4];
    let _ = getrandom::getrandom(&mut b);
    format!("{:06}", u32::from_le_bytes(b) % 1_000_000)
}

/// L'appareil qui attend un envoi.
pub struct Receiver {
    code: Arc<Mutex<String>>,
    port: u16,
    stop: Arc<AtomicBool>,
    events: Rx<Event>,
}

impl Receiver {
    /// Commence à attendre : `name` (le nom de l'appareil, montré par celui
    /// qui envoie) et `platform` (`tvos`, `webos`, `windows`…).
    pub fn start(name: &str, platform: &str) -> io::Result<Receiver> {
        Self::start_on(name, platform, PORT)
    }

    /// Comme [`Receiver::start`], sur un autre port de recherche (essais).
    pub fn start_on(name: &str, platform: &str, udp_port: u16) -> io::Result<Receiver> {
        let listener = TcpListener::bind((Ipv4Addr::UNSPECIFIED, 0))?;
        listener.set_nonblocking(true)?;
        let port = listener.local_addr()?.port();
        let code = Arc::new(Mutex::new(new_code()));
        let stop = Arc::new(AtomicBool::new(false));
        let (tx, events) = mpsc::channel();
        // Les recherches : le port peut être pris (une autre app) ; on attend
        // quand même, l'envoi direct reste possible.
        let udp = bind_udp(udp_port);
        if let Some(udp) = udp {
            let answer = serde_json::json!({ "name": name, "platform": platform, "port": port }).to_string();
            let stop = stop.clone();
            std::thread::Builder::new().name("ps-handoff-udp".into()).spawn(move || answer_searches(udp, answer, stop))?;
        } else {
            log::warn!("PlayStation : port {udp_port} des recherches pris, envoi direct seulement");
        }
        {
            let code = code.clone();
            let stop = stop.clone();
            std::thread::Builder::new().name("ps-handoff".into()).spawn(move || accept_loop(listener, code, stop, tx))?;
        }
        log::info!("PlayStation : en attente d'un envoi (port {port})");
        Ok(Receiver { code, port, stop, events })
    }

    /// Le code à afficher.
    pub fn code(&self) -> String {
        self.code.lock().unwrap().clone()
    }

    /// Port TCP d'écoute.
    pub fn port(&self) -> u16 {
        self.port
    }

    /// Le prochain évènement, s'il en vient un dans le délai.
    pub fn next(&self, timeout: Duration) -> Option<Event> {
        match self.events.recv_timeout(timeout) {
            Ok(e) => Some(e),
            Err(RecvTimeoutError::Timeout) => None,
            Err(RecvTimeoutError::Disconnected) => Some(Event::Expired),
        }
    }

    pub fn stop(&self) {
        self.stop.store(true, Ordering::SeqCst);
    }
}

impl Drop for Receiver {
    fn drop(&mut self) {
        self.stop();
    }
}

fn bind_udp(port: u16) -> Option<UdpSocket> {
    let s = socket2::Socket::new(socket2::Domain::IPV4, socket2::Type::DGRAM, Some(socket2::Protocol::UDP)).ok()?;
    let _ = s.set_reuse_address(true);
    s.bind(&SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, port).into()).ok()?;
    let u: UdpSocket = s.into();
    u.set_read_timeout(Some(Duration::from_millis(250))).ok()?;
    Some(u)
}

fn answer_searches(udp: UdpSocket, answer: String, stop: Arc<AtomicBool>) {
    let started = Instant::now();
    let mut buf = [0u8; 64];
    while !stop.load(Ordering::SeqCst) && started.elapsed() < LIFETIME {
        if let Ok((n, from)) = udp.recv_from(&mut buf) {
            if &buf[..n] == QUERY {
                let _ = udp.send_to(answer.as_bytes(), from);
            }
        }
    }
}

fn accept_loop(listener: TcpListener, code: Arc<Mutex<String>>, stop: Arc<AtomicBool>, tx: Sender<Event>) {
    let started = Instant::now();
    let mut failures = 0;
    loop {
        if stop.load(Ordering::SeqCst) {
            return;
        }
        if started.elapsed() >= LIFETIME {
            stop.store(true, Ordering::SeqCst);
            let _ = tx.send(Event::Expired);
            return;
        }
        match listener.accept() {
            Ok((stream, from)) => {
                let current = code.lock().unwrap().clone();
                match receive_one(stream, &current) {
                    Ok(Some(bundle)) => {
                        log::info!("PlayStation : paquet reçu de {}", from.ip());
                        let _ = tx.send(Event::Received(bundle));
                    }
                    Ok(None) => {
                        failures += 1;
                        log::info!("PlayStation : code faux depuis {} ({failures}/{TRIES})", from.ip());
                        if failures >= TRIES {
                            failures = 0;
                            let next = new_code();
                            *code.lock().unwrap() = next.clone();
                            let _ = tx.send(Event::NewCode(next));
                        }
                    }
                    Err(e) => log::info!("PlayStation : envoi interrompu depuis {} ({e})", from.ip()),
                }
            }
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => std::thread::sleep(Duration::from_millis(100)),
            Err(e) => {
                log::warn!("PlayStation : écoute de l'envoi arrêtée ({e})");
                return;
            }
        }
    }
}

fn read_frame(s: &mut TcpStream) -> io::Result<Vec<u8>> {
    let mut len = [0u8; 4];
    s.read_exact(&mut len)?;
    let n = u32::from_be_bytes(len) as usize;
    if n > FRAME_MAX {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "trame trop grande"));
    }
    let mut buf = vec![0u8; n];
    s.read_exact(&mut buf)?;
    Ok(buf)
}

fn write_frame(s: &mut TcpStream, data: &[u8]) -> io::Result<()> {
    s.write_all(&(data.len() as u32).to_be_bytes())?;
    s.write_all(data)?;
    s.flush()
}

fn confirm(key: &[u8], label: &[u8]) -> Vec<u8> {
    let mut mac = <Hmac<Sha256> as Mac>::new_from_slice(key).expect("clé HMAC");
    mac.update(label);
    mac.finalize().into_bytes().to_vec()
}

fn cipher(key: &[u8]) -> Aes256Gcm {
    // La clé de SPAKE2 (32 octets) passée par SHA-256 avec une étiquette.
    let mut h = Sha256::new();
    h.update(b"kn-handoff aes");
    h.update(key);
    Aes256Gcm::new_from_slice(&h.finalize()).expect("clé AES")
}

/// Une connexion de l'envoyeur : le paquet, `None` pour un code faux.
fn receive_one(mut s: TcpStream, code: &str) -> io::Result<Option<String>> {
    s.set_nonblocking(false)?;
    s.set_read_timeout(Some(IO_TIMEOUT))?;
    s.set_write_timeout(Some(IO_TIMEOUT))?;
    let a = read_frame(&mut s)?;
    let (state, b) = Spake2::<Ed25519Group>::start_b(&Password::new(code.as_bytes()), &Identity::new(ID_SENDER), &Identity::new(ID_RECEIVER));
    write_frame(&mut s, &b)?;
    let Ok(key) = state.finish(&a) else { return Ok(None) };
    let their = read_frame(&mut s)?;
    if their != confirm(&key, ID_SENDER) {
        let _ = write_frame(&mut s, &[]);
        return Ok(None);
    }
    write_frame(&mut s, &confirm(&key, ID_RECEIVER))?;
    let sealed = read_frame(&mut s)?;
    if sealed.len() < 12 {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "paquet trop court"));
    }
    let plain = cipher(&key)
        .decrypt(Nonce::from_slice(&sealed[..12]), &sealed[12..])
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "paquet illisible"))?;
    let text = String::from_utf8(plain).map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "paquet pas en UTF-8"))?;
    write_frame(&mut s, b"ok")?;
    Ok(Some(text))
}

/// Envoie le paquet (JSON) à un appareil en attente, avec le code tapé.
/// Bloque quelques secondes au plus : hors du fil de l'interface.
pub fn send(to: SocketAddr, code: &str, bundle: &str) -> Result<(), SendError> {
    let net = |e: io::Error| SendError::Unreachable(e.to_string());
    let mut s = TcpStream::connect_timeout(&to, Duration::from_secs(3)).map_err(net)?;
    s.set_read_timeout(Some(IO_TIMEOUT)).map_err(net)?;
    s.set_write_timeout(Some(IO_TIMEOUT)).map_err(net)?;
    let code: String = code.chars().filter(char::is_ascii_digit).collect();
    let (state, a) = Spake2::<Ed25519Group>::start_a(&Password::new(code.as_bytes()), &Identity::new(ID_SENDER), &Identity::new(ID_RECEIVER));
    write_frame(&mut s, &a).map_err(net)?;
    let b = read_frame(&mut s).map_err(net)?;
    let key = state.finish(&b).map_err(|_| SendError::Refused)?;
    write_frame(&mut s, &confirm(&key, ID_SENDER)).map_err(net)?;
    let theirs = read_frame(&mut s).map_err(net)?;
    if theirs != confirm(&key, ID_RECEIVER) {
        return Err(SendError::Refused);
    }
    let mut nonce = [0u8; 12];
    let _ = getrandom::getrandom(&mut nonce);
    let sealed = cipher(&key)
        .encrypt(Nonce::from_slice(&nonce), bundle.as_bytes())
        .map_err(|_| SendError::Unreachable("chiffrement".into()))?;
    let mut frame = nonce.to_vec();
    frame.extend_from_slice(&sealed);
    write_frame(&mut s, &frame).map_err(net)?;
    match read_frame(&mut s) {
        Ok(ok) if ok == b"ok" => Ok(()),
        Ok(_) => Err(SendError::Unreachable("réponse inattendue".into())),
        Err(e) => Err(net(e)),
    }
}

/// Les appareils du réseau qui attendent un envoi (diffusion, et `direct` :
/// adresses à interroger en plus, par un VPN par exemple).
pub fn discover(wait: Duration, direct: &[Ipv4Addr]) -> io::Result<Vec<Waiting>> {
    discover_on(wait, direct, PORT)
}

/// Comme [`discover`], sur un autre port (essais).
pub fn discover_on(wait: Duration, direct: &[Ipv4Addr], port: u16) -> io::Result<Vec<Waiting>> {
    let s = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0))?;
    s.set_broadcast(true)?;
    let mut targets = vec![Ipv4Addr::BROADCAST];
    targets.extend_from_slice(direct);
    let mut found: Vec<Waiting> = Vec::new();
    let deadline = Instant::now() + wait;
    let mut next_send = Instant::now();
    let mut buf = [0u8; 512];
    while Instant::now() < deadline {
        if Instant::now() >= next_send {
            for t in &targets {
                let _ = s.send_to(QUERY, (*t, port));
            }
            next_send = Instant::now() + Duration::from_millis(400);
        }
        let left = deadline.min(next_send).saturating_duration_since(Instant::now()).max(Duration::from_millis(1));
        s.set_read_timeout(Some(left))?;
        let Ok((n, from)) = s.recv_from(&mut buf) else { continue };
        let Ok(v) = serde_json::from_slice::<serde_json::Value>(&buf[..n]) else { continue };
        let (Some(name), Some(platform), Some(tcp)) = (v["name"].as_str(), v["platform"].as_str(), v["port"].as_u64()) else { continue };
        let addr = SocketAddr::new(from.ip(), tcp as u16);
        if !found.iter().any(|w| w.addr == addr) {
            found.push(Waiting { name: name.to_string(), platform: platform.to_string(), addr });
        }
    }
    Ok(found)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn local(r: &Receiver) -> SocketAddr {
        SocketAddr::new(Ipv4Addr::LOCALHOST.into(), r.port())
    }

    #[test]
    fn bundle_arrives_with_the_right_code() {
        let r = Receiver::start_on("Salon", "tvos", 0).unwrap();
        let code = r.code();
        assert_eq!(code.len(), 6);
        // Espaces et groupes tolérés à la saisie.
        let typed = format!("{} {}", &code[..3], &code[3..]);
        send(local(&r), &typed, r#"{"accounts":[],"consoles":[]}"#).unwrap();
        assert_eq!(r.next(Duration::from_secs(2)), Some(Event::Received(r#"{"accounts":[],"consoles":[]}"#.into())));
    }

    #[test]
    fn wrong_codes_reveal_nothing_and_renew_the_code() {
        let r = Receiver::start_on("Salon", "webos", 0).unwrap();
        let code = r.code();
        let wrong = if code == "000000" { "111111" } else { "000000" };
        for _ in 0..TRIES {
            assert_eq!(send(local(&r), wrong, "{}"), Err(SendError::Refused));
        }
        match r.next(Duration::from_secs(2)) {
            Some(Event::NewCode(next)) => assert_eq!(next, r.code()),
            other => panic!("attendu un nouveau code, reçu {other:?}"),
        }
        // L'ancien code ne vaut plus (sauf hasard d'un tirage identique).
        if r.code() != code {
            assert_eq!(send(local(&r), &code, "{}"), Err(SendError::Refused));
        }
        send(local(&r), &r.code(), "{\"ok\":1}").unwrap();
        loop {
            match r.next(Duration::from_secs(2)) {
                Some(Event::Received(b)) => {
                    assert_eq!(b, "{\"ok\":1}");
                    break;
                }
                Some(Event::NewCode(_)) => continue,
                other => panic!("attendu le paquet, reçu {other:?}"),
            }
        }
    }

    #[test]
    fn waiting_device_answers_searches() {
        // Port d'essai, interrogé en direct (la diffusion n'atteint pas
        // toujours la boucle locale).
        let port = 19_340 + (std::process::id() % 500) as u16;
        let r = Receiver::start_on("Salon", "tvos", port).unwrap();
        let found = discover_on(Duration::from_millis(600), &[Ipv4Addr::LOCALHOST], port).unwrap();
        let w = found.iter().find(|w| w.addr.port() == r.port()).expect("appareil trouvé");
        assert_eq!((w.name.as_str(), w.platform.as_str()), ("Salon", "tvos"));
    }

    #[test]
    fn unreachable_device() {
        // Un port fermé de la boucle locale.
        let free = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap().local_addr().unwrap();
        assert!(matches!(send(free, "123456", "{}"), Err(SendError::Unreachable(_))));
    }
}
