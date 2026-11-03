//! Début d'une session de lecture à distance, d'après `session.c` et
//! `ctrl.c` de chiaki-ng (réseau local seulement, sans les serveurs de Sony).
//!
//! 1. **Demande de session** ([`request_session`]) : `GET /sie/ps5/rp/sess/init`
//!    sur le port 9295 (TCP), avec la clé d'association en clair
//!    (`RP-Registkey`). La console répond un `RP-Nonce` (16 octets en base64),
//!    ou sa version du protocole si la nôtre ne lui convient pas : on
//!    redemande alors avec la sienne, comme chiaki-ng.
//! 2. **Canal de contrôle** ([`Ctrl::connect`]) : `GET /sie/ps5/rp/sess/ctrl`
//!    sur le même port, en gardant la connexion. Les en-têtes `RP-Auth`,
//!    `RP-Did`, `RP-OSType`, `RP-StartBitrate` et `RP-StreamingType` sont
//!    chiffrés avec les clés de session ([`RpCrypt::auth`] : `nonce` et
//!    `RP-Key`), chacun avec le compteur suivant. Ensuite, des messages : en-tête
//!    de 8 octets (taille gros-boutiste sur 4 octets, type sur 2, 2 octets
//!    nuls) puis le contenu chiffré. La console envoie l'identifiant de session
//!    (`0x33`) puis des battements de cœur (`0xfe`) auxquels on répond (`0x1fe`).
//!
//! L'identifiant de session sert ensuite au transport de l'image (Takion),
//! qui n'est pas encore écrit.

use std::io::{self, Read, Write};
use std::net::{Ipv4Addr, Shutdown, SocketAddr, SocketAddrV4, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use base64::Engine;

use super::discovery::Target;
use super::regist::{self, Registered};
use super::rpcrypt::{self, KEY_SIZE, RpCrypt};

/// Port de la demande de session et du canal de contrôle.
pub const PORT: u16 = 9295;

const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
const RESPONSE_TIMEOUT: Duration = Duration::from_secs(5);

/// Taille maximale d'un identifiant de session, zéro final compris.
const SESSION_ID_SIZE_MAX: usize = 80;

/// `RP-OSType` envoyé par chiaki-ng (celui de l'app de Sony pour Windows).
const OS_TYPE: &str = "Win10.0.0";

/// Types de messages du canal de contrôle (`ctrl.c`).
pub mod msg {
    pub const SESSION_ID: u16 = 0x33;
    pub const HEARTBEAT_REQ: u16 = 0xfe;
    pub const HEARTBEAT_REP: u16 = 0x1fe;
    pub const LOGIN_PIN_REQ: u16 = 0x4;
    pub const LOGIN_PIN_REP: u16 = 0x8004;
    pub const LOGIN: u16 = 0x5;
    pub const GOTO_BED: u16 = 0x50;
    pub const ENABLE_DUALSENSE_FEATURES: u16 = 0x13;
    pub const DUALSENSE_CONNECT: u16 = 0x11;
    pub const GO_HOME: u16 = 0x14;
    pub const DISPLAY_A: u16 = 0x1;
    pub const DISPLAY_B: u16 = 0x16;
    pub const MIC_TOGGLE: u16 = 0x36;
    pub const DISPLAY_DEVICES: u16 = 0x910;
    /// Clavier de la console (docs/playstation.md, « Clavier de la
    /// console ») : activé au début de la session, puis ouvert et fermé par
    /// la console, le texte allant dans les deux sens.
    pub const KEYBOARD_ENABLE: u16 = 0xd;
    pub const KEYBOARD_ENABLE_TOGGLE: u16 = 0x20;
    pub const KEYBOARD_OPEN: u16 = 0x21;
    pub const KEYBOARD_CLOSE_REMOTE: u16 = 0x22;
    pub const KEYBOARD_TEXT_CHANGE_REQ: u16 = 0x23;
    pub const KEYBOARD_TEXT_CHANGE_RES: u16 = 0x24;
    pub const KEYBOARD_CLOSE_REQ: u16 = 0x25;
}

/// Format d'image demandé à une PS5 (`RP-StreamingType`). La PS4 ne connaît
/// que le H.264.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Codec {
    H264,
    H265,
    H265Hdr,
}

impl Codec {
    fn streaming_type(self) -> u32 {
        match self {
            Codec::H264 => 1,
            Codec::H265 => 2,
            Codec::H265Hdr => 3,
        }
    }
}

/// Pourquoi la session n'a pas pu démarrer.
#[derive(Debug)]
pub enum SessionError {
    /// La console refuse : `RP-Application-Reason` ([`regist::REASON_IN_USE`] :
    /// déjà utilisée par un autre appareil).
    Rejected { status: u16, reason: Option<u32> },
    /// Versions du protocole incompatibles, même après une nouvelle demande.
    VersionMismatch(Option<String>),
    /// Console associée illisible (clés absentes).
    BadRegistration,
    InvalidResponse(String),
    Network(io::Error),
}

impl std::fmt::Display for SessionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SessionError::Rejected { status, reason } => {
                write!(f, "la console refuse la session (HTTP {status}")?;
                if let Some(r) = reason {
                    write!(f, ", raison {r:#x} : {}", regist::reason_text(*r))?;
                }
                write!(f, ")")
            }
            SessionError::VersionMismatch(v) => write!(f, "version du protocole refusée (console : {})", v.as_deref().unwrap_or("?")),
            SessionError::BadRegistration => write!(f, "association illisible"),
            SessionError::InvalidResponse(why) => write!(f, "réponse illisible : {why}"),
            SessionError::Network(e) => write!(f, "réseau : {e}"),
        }
    }
}

impl std::error::Error for SessionError {}

impl From<io::Error> for SessionError {
    fn from(e: io::Error) -> Self {
        SessionError::Network(e)
    }
}

/// Clés d'une console associée, telles que la session les emploie.
#[derive(Clone)]
pub struct Keys {
    /// Clé d'association sur 16 octets, complétée par des zéros
    /// (`rp_regist_key` de chiaki-ng).
    pub regist_key: [u8; KEY_SIZE],
    /// `RP-Key`, le `morning` de [`RpCrypt::auth`].
    pub morning: [u8; KEY_SIZE],
}

impl Keys {
    pub fn from_registered(r: &Registered) -> Option<Self> {
        let raw = hex_decode(&r.regist_key)?;
        if raw.is_empty() || raw.len() > KEY_SIZE {
            return None;
        }
        let mut regist_key = [0u8; KEY_SIZE];
        regist_key[..raw.len()].copy_from_slice(&raw);
        Some(Self { regist_key, morning: r.rp_key_bytes()? })
    }

    /// `RP-Registkey` : les octets de la clé jusqu'au premier nul, en
    /// hexadécimal minuscule.
    fn regist_key_hex(&self) -> String {
        let end = self.regist_key.iter().position(|&b| b == 0).unwrap_or(KEY_SIZE);
        self.regist_key[..end].iter().map(|b| format!("{b:02x}")).collect()
    }
}

impl std::fmt::Debug for Keys {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Keys(..)")
    }
}

fn hex_decode(text: &str) -> Option<Vec<u8>> {
    let text = text.trim();
    if text.len() % 2 != 0 {
        return None;
    }
    (0..text.len()).step_by(2).map(|i| u8::from_str_radix(&text[i..i + 2], 16).ok()).collect()
}

fn session_path(target: Target) -> &'static str {
    match target {
        Target::Ps4V8 | Target::Ps4V9 => "/sce/rp/session",
        t if rpcrypt::is_ps5(t) => "/sie/ps5/rp/sess/init",
        _ => "/sie/ps4/rp/sess/init",
    }
}

fn ctrl_path(target: Target) -> &'static str {
    match target {
        Target::Ps4V8 | Target::Ps4V9 => "/sce/rp/session/ctrl",
        t if rpcrypt::is_ps5(t) => "/sie/ps5/rp/sess/ctrl",
        _ => "/sie/ps4/rp/sess/ctrl",
    }
}

/// La cible d'après le `RP-Version` de la console (`chiaki_rp_version_parse`).
pub(crate) fn parse_rp_version(version: &str, ps5: bool) -> Option<Target> {
    match (ps5, version.trim()) {
        (true, "1.0") => Some(Target::Ps5V1),
        (false, "8.0") => Some(Target::Ps4V8),
        (false, "9.0") => Some(Target::Ps4V9),
        (false, "10.0") => Some(Target::Ps4V10),
        // chiaki-ng : une PS4 qui annonce 5.0 accepte 9.0.
        (false, "5.0") => Some(Target::Ps4V9),
        _ => None,
    }
}

/// Texte de la demande de session.
pub fn session_request(target: Target, host: Ipv4Addr, keys: &Keys) -> String {
    format!(
        "GET {} HTTP/1.1\r\n\
         Host: {host}:{PORT}\r\n\
         User-Agent: remoteplay Windows\r\n\
         Connection: close\r\n\
         Content-Length: 0\r\n\
         RP-Registkey: {}\r\n\
         Rp-Version: {}\r\n\
         \r\n",
        session_path(target),
        keys.regist_key_hex(),
        regist::rp_version(target).unwrap_or("")
    )
}

/// Une réponse HTTP lue jusqu'à la ligne vide : statut, en-têtes, et ce qui
/// est arrivé après.
struct HttpHead {
    status: u16,
    headers: Vec<(String, String)>,
    rest: Vec<u8>,
}

impl HttpHead {
    fn get(&self, key: &str) -> Option<&str> {
        self.headers.iter().find(|(k, _)| k.eq_ignore_ascii_case(key)).map(|(_, v)| v.as_str())
    }
}

fn read_head(stream: &mut TcpStream, timeout: Duration) -> Result<HttpHead, SessionError> {
    stream.set_read_timeout(Some(timeout))?;
    let deadline = Instant::now() + timeout;
    let mut data = Vec::new();
    let mut chunk = [0u8; 1024];
    let end = loop {
        if let Some(pos) = data.windows(4).position(|w| w == b"\r\n\r\n") {
            break pos + 4;
        }
        if Instant::now() >= deadline || data.len() > 8192 {
            return Err(SessionError::InvalidResponse("en-tête HTTP incomplet".into()));
        }
        let n = stream.read(&mut chunk)?;
        if n == 0 {
            return Err(SessionError::InvalidResponse("connexion fermée avant la réponse".into()));
        }
        data.extend_from_slice(&chunk[..n]);
    };
    let head = String::from_utf8_lossy(&data[..end]).into_owned();
    log::debug!("PlayStation : réponse {head:?}");
    let mut lines = head.split("\r\n");
    let status_line = lines.next().unwrap_or_default();
    let status = status_line
        .split(' ')
        .nth(1)
        .and_then(|s| s.parse().ok())
        .ok_or_else(|| SessionError::InvalidResponse(format!("statut illisible : {status_line}")))?;
    let headers = lines
        .filter_map(|l| {
            let (k, v) = l.split_once(':')?;
            Some((k.trim().to_string(), v.trim().to_string()))
        })
        .collect();
    Ok(HttpHead { status, headers, rest: data[end..].to_vec() })
}

fn reason(head: &HttpHead) -> Option<u32> {
    head.get("RP-Application-Reason").and_then(|v| u32::from_str_radix(v.trim_start_matches("0x"), 16).ok())
}

/// Demande une session ; rend la cible acceptée par la console et son
/// `nonce`. Une version refusée est redemandée avec celle de la console.
pub fn request_session(addr: Ipv4Addr, target: Target, keys: &Keys) -> Result<(Target, [u8; KEY_SIZE]), SessionError> {
    let ps5 = rpcrypt::is_ps5(target);
    let mut target = target;
    for _ in 0..3 {
        let mut stream = TcpStream::connect_timeout(&SocketAddr::V4(SocketAddrV4::new(addr, PORT)), CONNECT_TIMEOUT)?;
        stream.set_write_timeout(Some(RESPONSE_TIMEOUT))?;
        stream.write_all(session_request(target, addr, keys).as_bytes())?;
        let head = read_head(&mut stream, RESPONSE_TIMEOUT)?;
        let _ = stream.shutdown(Shutdown::Both);
        if head.status == 200 {
            if let Some(nonce) = head.get("RP-Nonce") {
                let bytes = base64::engine::general_purpose::STANDARD
                    .decode(nonce)
                    .map_err(|_| SessionError::InvalidResponse("RP-Nonce illisible".into()))?;
                let nonce: [u8; KEY_SIZE] = bytes.try_into().map_err(|_| SessionError::InvalidResponse("RP-Nonce de mauvaise taille".into()))?;
                log::info!("PlayStation : session accordée ({:?})", target);
                return Ok((target, nonce));
            }
        }
        let why = reason(&head);
        let server_version = head.get("RP-Version").map(str::to_string);
        let mismatch = matches!(why, Some(regist::REASON_RP_VERSION) | Some(0x80108bff));
        if mismatch {
            if let Some(v) = server_version.as_deref().and_then(|v| parse_rp_version(v, ps5)) {
                if v != target {
                    log::info!("PlayStation : la console veut la version {:?}, nouvelle demande", server_version);
                    target = v;
                    continue;
                }
            }
            return Err(SessionError::VersionMismatch(server_version));
        }
        return Err(SessionError::Rejected { status: head.status, reason: why });
    }
    Err(SessionError::VersionMismatch(None))
}

/// `RP-Did` : identifiant de l'appareil, 32 octets dont 16 au hasard
/// (préfixe et suffixe fixes de chiaki-ng). Une session, un tirage.
pub fn random_did() -> [u8; 32] {
    let mut did = [0u8; 32];
    did[..10].copy_from_slice(&[0x00, 0x18, 0x00, 0x00, 0x00, 0x07, 0x00, 0x40, 0x00, 0x80]);
    let _ = getrandom::getrandom(&mut did[10..26]);
    did
}

/// Ce que le canal de contrôle annonce à l'app.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CtrlEvent {
    /// L'identifiant de session (`0x33`) : la suite (Takion) peut démarrer.
    SessionId(String),
    /// La console demande le code de connexion de son compte (réglage de la
    /// console) : pas encore géré.
    LoginPinRequested,
    /// Résultat de la connexion au compte (`0x5`) : `true` si accepté.
    Login(bool),
    /// La console ne peut pas afficher ce qu'elle montre (contenu protégé),
    /// ou de nouveau (`false`), d'après `0x1` et `0x16` comme chiaki-ng.
    CantDisplay(bool),
    /// La console ouvre son clavier à l'écran (`0x21`), avec le texte déjà
    /// dans son champ (souvent vide).
    KeyboardOpen(String),
    /// Le texte du champ de la console (`0x24`, réponse à
    /// [`Ctrl::keyboard_set_text`] ou changé par la console) : il remplace
    /// celui de l'app.
    KeyboardText(String),
    /// La console a refermé son clavier (`0x22`).
    KeyboardClosed,
    /// Message d'un autre type, déchiffré.
    Other(u16, Vec<u8>),
    /// Canal fermé (par la console, le réseau ou [`Ctrl::stop`]).
    Closed(String),
}

struct Writer {
    stream: TcpStream,
    counter: u64,
}

/// Le canal de contrôle ouvert ; un fil lit les messages et répond aux
/// battements de cœur.
pub struct Ctrl {
    writer: Arc<Mutex<Writer>>,
    crypt: RpCrypt,
    stop: Arc<AtomicBool>,
    /// Type de console annoncé (`RP-Server-Type` : 0 PS4, 1 PS4 Pro, 2 PS5).
    pub server_type: Option<u8>,
    /// `RP-Prohibit` : la console n'autorise pas l'affichage.
    pub prohibited: bool,
    thread: Option<std::thread::JoinHandle<()>>,
    /// Compteur des textes envoyés au clavier de la console (croissant, le
    /// premier vaut 1).
    keyboard_counter: std::sync::atomic::AtomicU32,
}

impl Ctrl {
    /// Ouvre le canal de contrôle. `events` reçoit l'identifiant de session
    /// et la suite.
    pub fn connect(addr: Ipv4Addr, target: Target, nonce: &[u8; KEY_SIZE], keys: &Keys, codec: Codec) -> Result<(Self, Receiver<CtrlEvent>), SessionError> {
        let crypt = RpCrypt::auth(target, nonce, &keys.morning);
        let (request, counter) = ctrl_request(&crypt, target, addr, keys, &random_did(), codec);
        let mut stream = TcpStream::connect_timeout(&SocketAddr::V4(SocketAddrV4::new(addr, PORT)), CONNECT_TIMEOUT)?;
        stream.set_nodelay(true)?;
        stream.set_write_timeout(Some(RESPONSE_TIMEOUT))?;
        stream.write_all(request.as_bytes())?;
        let head = read_head(&mut stream, RESPONSE_TIMEOUT)?;
        if head.status != 200 {
            return Err(SessionError::Rejected { status: head.status, reason: reason(&head) });
        }
        let mut remote_counter = 0u64;
        let server_type = head.get("RP-Server-Type").and_then(|v| {
            let mut bytes = base64::engine::general_purpose::STANDARD.decode(v).ok()?;
            if bytes.len() != KEY_SIZE {
                return None;
            }
            crypt.decrypt(remote_counter, &mut bytes);
            remote_counter += 1;
            Some(bytes[0])
        });
        let prohibited = head.get("RP-Prohibit").is_some_and(|v| v.trim() == "1");
        log::info!("PlayStation : canal de contrôle ouvert (type de console {server_type:?}, affichage interdit : {prohibited})");

        let (tx, rx) = mpsc::channel();
        let reader = stream.try_clone()?;
        let writer = Arc::new(Mutex::new(Writer { stream, counter }));
        let stop = Arc::new(AtomicBool::new(false));
        let thread = {
            let writer = writer.clone();
            let crypt = crypt.clone();
            let stop = stop.clone();
            let rest = head.rest;
            std::thread::Builder::new()
                .name("ps-ctrl".into())
                .spawn(move || read_loop(reader, rest, remote_counter, crypt, writer, stop, tx))?
        };
        Ok((Ctrl { writer, crypt, stop, server_type, prohibited, thread: Some(thread), keyboard_counter: Default::default() }, rx))
    }

    /// Les clés de session, dont le flux a besoin pour masquer sa « launch
    /// spec ».
    pub fn crypt(&self) -> &RpCrypt {
        &self.crypt
    }

    /// Envoie un message chiffré.
    pub fn send(&self, kind: u16, payload: &[u8]) -> io::Result<()> {
        send_message(&self.writer, &self.crypt, kind, payload)
    }

    /// Ce que chiaki-ng envoie après l'identifiant de session : les fonctions
    /// de la DualSense, le clavier de la console, l'état du micro (deux fois,
    /// `toggle_microphone(false)`), aucun écran de plus.
    ///
    /// Le clavier : `0xd` (16 octets, la « signature » de chiaki-ng, qui ne
    /// sait pas ce qu'elle porte) puis `0x20` (1). chiaki-ng l'écrit mais ne
    /// l'allume pas ; pas encore essayé ici avec la console.
    pub fn enable_features(&self, dualsense: bool, keyboard: bool) -> io::Result<()> {
        if dualsense {
            self.send(msg::ENABLE_DUALSENSE_FEATURES, &[0x00, 0x40, 0x00])?;
            let mut connect = [0u8; 16];
            connect[..13].copy_from_slice(&[0xa0, 0xab, 0x51, 0xbd, 0xd1, 0x7e, 0x00, 0x00, 0xff, 0xff, 0xff, 0xff, 0xff]);
            self.send(msg::DUALSENSE_CONNECT, &connect)?;
        }
        if keyboard {
            self.send(msg::KEYBOARD_ENABLE, &KEYBOARD_SIGNATURE)?;
            self.send(msg::KEYBOARD_ENABLE_TOGGLE, &[1])?;
        }
        self.send(msg::MIC_TOGGLE, &[0, 1, 1, 89])?;
        self.send(msg::MIC_TOGGLE, &[0, 1, 1, 89])?;
        self.send(msg::DISPLAY_DEVICES, &[0, 0, 0, 0])
    }

    /// Met la console en mode repos.
    pub fn goto_bed(&self) -> io::Result<()> {
        self.send(msg::GOTO_BED, &[])
    }

    /// Le texte entier du champ de la console (à chaque changement), avec le
    /// compteur suivant.
    pub fn keyboard_set_text(&self, text: &str) -> io::Result<()> {
        let counter = self.keyboard_counter.fetch_add(1, Ordering::SeqCst).wrapping_add(1);
        self.send(msg::KEYBOARD_TEXT_CHANGE_REQ, &keyboard_text_request(counter, text))
    }

    /// Entrée : le texte est validé, la console ferme son clavier.
    pub fn keyboard_accept(&self) -> io::Result<()> {
        self.send(msg::KEYBOARD_CLOSE_REQ, &keyboard_close_request(true))
    }

    /// Échap : le texte est refusé, la console ferme son clavier.
    pub fn keyboard_reject(&self) -> io::Result<()> {
        self.send(msg::KEYBOARD_CLOSE_REQ, &keyboard_close_request(false))
    }

    /// Ferme le canal.
    pub fn stop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Ok(w) = self.writer.lock() {
            let _ = w.stream.shutdown(Shutdown::Both);
        }
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

impl Drop for Ctrl {
    fn drop(&mut self) {
        self.stop();
    }
}

/// Texte de la demande du canal de contrôle et le compteur local après les
/// en-têtes chiffrés.
pub(crate) fn ctrl_request(crypt: &RpCrypt, target: Target, addr: Ipv4Addr, keys: &Keys, did: &[u8; 32], codec: Codec) -> (String, u64) {
    let b64 = |b: &[u8]| base64::engine::general_purpose::STANDARD.encode(b);
    let mut counter = 0u64;
    let mut enc = |data: &[u8]| {
        let mut buf = data.to_vec();
        crypt.encrypt(counter, &mut buf);
        counter += 1;
        b64(&buf)
    };
    let auth = enc(&keys.regist_key);
    let did = enc(did);
    let mut os = OS_TYPE.as_bytes().to_vec();
    os.push(0);
    let os = enc(&os);
    let bitrate = (rpcrypt::target_code(target) >= rpcrypt::target_code(Target::Ps4V10)).then(|| enc(&[0u8; 4]));
    let ps5 = rpcrypt::is_ps5(target);
    let streaming = ps5.then(|| enc(&codec.streaming_type().to_le_bytes()));
    let mut text = format!(
        "GET {} HTTP/1.1\r\n\
         Host: {addr}:{PORT}\r\n\
         User-Agent: remoteplay Windows\r\n\
         Connection: keep-alive\r\n\
         Content-Length: 0\r\n\
         RP-Auth: {auth}\r\n\
         RP-Version: {}\r\n\
         RP-Did: {did}\r\n\
         RP-ControllerType: 3\r\n\
         RP-ClientType: 11\r\n\
         RP-OSType: {os}\r\n\
         RP-ConPath: 1\r\n",
        ctrl_path(target),
        regist::rp_version(target).unwrap_or("")
    );
    if ps5 {
        // Sans lui, une PS5 envoie certains effets de gâchette sous une forme
        // que la manette n'applique pas (chiaki-ng).
        text.push_str("RP-PadProcNo: 3\r\n");
    }
    if let Some(b) = bitrate {
        text.push_str(&format!("RP-StartBitrate: {b}\r\n"));
    }
    if let Some(s) = streaming {
        text.push_str(&format!("RP-StreamingType: {s}\r\n"));
    }
    text.push_str("\r\n");
    (text, counter)
}

fn send_message(writer: &Mutex<Writer>, crypt: &RpCrypt, kind: u16, payload: &[u8]) -> io::Result<()> {
    let mut w = writer.lock().map_err(|_| io::Error::other("canal de contrôle empoisonné"))?;
    let mut frame = Vec::with_capacity(8 + payload.len());
    frame.extend_from_slice(&(payload.len() as u32).to_be_bytes());
    frame.extend_from_slice(&kind.to_be_bytes());
    frame.extend_from_slice(&[0, 0]);
    if !payload.is_empty() {
        let mut enc = payload.to_vec();
        crypt.encrypt(w.counter, &mut enc);
        w.counter += 1;
        frame.extend_from_slice(&enc);
    }
    w.stream.write_all(&frame)
}

fn read_loop(
    mut reader: TcpStream,
    mut buf: Vec<u8>,
    mut remote_counter: u64,
    crypt: RpCrypt,
    writer: Arc<Mutex<Writer>>,
    stop: Arc<AtomicBool>,
    tx: Sender<CtrlEvent>,
) {
    let _ = reader.set_read_timeout(Some(Duration::from_millis(500)));
    let mut chunk = [0u8; 2048];
    let mut display = DisplayState::default();
    let reason = loop {
        while buf.len() >= 8 {
            let size = u32::from_be_bytes([buf[0], buf[1], buf[2], buf[3]]) as usize;
            if size > 64 * 1024 {
                let _ = tx.send(CtrlEvent::Closed(format!("message trop grand ({size} octets)")));
                return;
            }
            if buf.len() < 8 + size {
                break;
            }
            let kind = u16::from_be_bytes([buf[4], buf[5]]);
            let mut payload = buf[8..8 + size].to_vec();
            buf.drain(..8 + size);
            if !payload.is_empty() {
                crypt.decrypt(remote_counter, &mut payload);
                remote_counter += 1;
            }
            match kind {
                msg::HEARTBEAT_REQ => {
                    log::debug!("PlayStation : battement de cœur");
                    if let Err(e) = send_message(&writer, &crypt, msg::HEARTBEAT_REP, &[]) {
                        log::warn!("PlayStation : réponse au battement de cœur impossible : {e}");
                    }
                }
                msg::SESSION_ID => {
                    let _ = tx.send(match parse_session_id(&payload) {
                        Some(id) => CtrlEvent::SessionId(id),
                        None => {
                            log::warn!("PlayStation : identifiant de session illisible, identifiant de secours");
                            CtrlEvent::SessionId(fallback_session_id())
                        }
                    });
                }
                msg::LOGIN_PIN_REQ => {
                    let _ = tx.send(CtrlEvent::LoginPinRequested);
                }
                msg::LOGIN => {
                    let _ = tx.send(CtrlEvent::Login(payload.first() == Some(&0)));
                }
                msg::KEYBOARD_OPEN => match parse_keyboard_open(&payload) {
                    Some(text) => {
                        log::info!("PlayStation : la console ouvre son clavier ({} caractères déjà)", text.chars().count());
                        let _ = tx.send(CtrlEvent::KeyboardOpen(text));
                    }
                    None => log::warn!("PlayStation : ouverture du clavier illisible ({} octets)", payload.len()),
                },
                msg::KEYBOARD_TEXT_CHANGE_RES => match parse_keyboard_text(&payload) {
                    Some(text) => {
                        let _ = tx.send(CtrlEvent::KeyboardText(text));
                    }
                    None => log::warn!("PlayStation : texte du clavier illisible ({} octets)", payload.len()),
                },
                msg::KEYBOARD_CLOSE_REMOTE => {
                    log::info!("PlayStation : la console ferme son clavier");
                    let _ = tx.send(CtrlEvent::KeyboardClosed);
                }
                msg::DISPLAY_A | msg::DISPLAY_B => {
                    if let Some(cant) = display.update(kind, &payload) {
                        let _ = tx.send(CtrlEvent::CantDisplay(cant));
                    }
                }
                other => {
                    log::debug!("PlayStation : message de contrôle {other:#x} ({} octets)", payload.len());
                    let _ = tx.send(CtrlEvent::Other(other, payload));
                }
            }
        }
        if stop.load(Ordering::SeqCst) {
            break "arrêt demandé".to_string();
        }
        match reader.read(&mut chunk) {
            Ok(0) if stop.load(Ordering::SeqCst) => break "arrêt demandé".to_string(),
            Ok(0) => break "fermé par la console".to_string(),
            Ok(n) => buf.extend_from_slice(&chunk[..n]),
            Err(e) if matches!(e.kind(), io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut) => {}
            Err(e) => break format!("réseau : {e}"),
        }
    };
    log::info!("PlayStation : canal de contrôle fermé ({reason})");
    let _ = tx.send(CtrlEvent::Closed(reason));
}

/// Messages `0x1` et `0x16` (`ctrl_message_received_displaya`/`b`) : la
/// console prévient (A = 1), puis bloque l'image (B, sauf `01 ff`) ;
/// A = 0 sans blocage en cours, ou B = `01 ff`, la libère.
#[derive(Default)]
pub(crate) struct DisplayState {
    cant_a: bool,
    cant_b: bool,
}

impl DisplayState {
    /// Le changement à annoncer, s'il y en a un.
    pub(crate) fn update(&mut self, kind: u16, payload: &[u8]) -> Option<bool> {
        let b0 = *payload.first()?;
        let b1 = payload.get(1).copied();
        if kind == msg::DISPLAY_A {
            if b0 == 1 {
                self.cant_a = true;
            } else if b0 == 0 && !self.cant_b {
                self.cant_a = false;
                return Some(false);
            }
            return None;
        }
        let release = b0 == 1 && b1 == Some(0xff);
        let mut change = None;
        if self.cant_a && !release && !self.cant_b {
            self.cant_b = true;
            change = Some(true);
        }
        if self.cant_b && release {
            self.cant_b = false;
        }
        change
    }
}

/// « Signature » de l'activation du clavier (`ctrl_enable_features` de
/// chiaki-ng, sens inconnu).
const KEYBOARD_SIGNATURE: [u8; 16] = [0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x05, 0xae, 0, 0, 0, 0, 0, 0, 0, 0];

/// En-tête de `KEYBOARD_OPEN` : 28 octets inconnus, puis la taille du texte
/// (gros-boutiste), puis le texte (UTF-8, sans zéro final).
const KEYBOARD_OPEN_HEAD: usize = 0x1c + 4;
/// En-tête de `KEYBOARD_TEXT_CHANGE_REQ` : compteur, taille, 8 + 16 octets
/// nuls, taille encore (tout en gros-boutiste), puis le texte.
const KEYBOARD_TEXT_REQ_HEAD: usize = 4 + 4 + 8 + 16 + 4;
/// En-tête de `KEYBOARD_TEXT_CHANGE_RES` : compteur, inconnu, taille,
/// inconnu, 16 octets, inconnu, taille encore, puis le texte.
const KEYBOARD_TEXT_RES_HEAD: usize = 4 + 4 + 4 + 4 + 16 + 4 + 4;

/// Le texte qui suit un en-tête de `head` octets dont la taille du texte est
/// à `len_at` ; un texte plus court que sa taille annoncée est refusé, un
/// texte qui n'est pas de l'UTF-8 valide est lu au mieux.
fn keyboard_payload_text(payload: &[u8], head: usize, len_at: usize) -> Option<String> {
    if payload.len() < head {
        return None;
    }
    let len = u32::from_be_bytes(payload[len_at..len_at + 4].try_into().ok()?) as usize;
    let text = payload.get(head..head.checked_add(len)?)?;
    // chiaki-ng lit une chaîne C : ce qui suit un zéro est ignoré.
    let text = text.split(|&b| b == 0).next().unwrap_or_default();
    Some(String::from_utf8_lossy(text).into_owned())
}

/// Le texte déjà dans le champ quand la console ouvre son clavier.
pub(crate) fn parse_keyboard_open(payload: &[u8]) -> Option<String> {
    keyboard_payload_text(payload, KEYBOARD_OPEN_HEAD, 0x1c)
}

/// Le texte du champ de la console (`KEYBOARD_TEXT_CHANGE_RES`).
pub(crate) fn parse_keyboard_text(payload: &[u8]) -> Option<String> {
    keyboard_payload_text(payload, KEYBOARD_TEXT_RES_HEAD, 8)
}

/// `KEYBOARD_TEXT_CHANGE_REQ` : le texte entier, avec son compteur.
pub(crate) fn keyboard_text_request(counter: u32, text: &str) -> Vec<u8> {
    let bytes = text.as_bytes();
    let len = (bytes.len() as u32).to_be_bytes();
    let mut p = vec![0u8; KEYBOARD_TEXT_REQ_HEAD];
    p[0..4].copy_from_slice(&counter.to_be_bytes());
    p[4..8].copy_from_slice(&len);
    p[32..36].copy_from_slice(&len);
    p.extend_from_slice(bytes);
    p
}

/// `KEYBOARD_CLOSE_REQ` : `00 00 00 00` valide, `00 00 00 01` refuse.
pub(crate) fn keyboard_close_request(accept: bool) -> [u8; 4] {
    [0, 0, 0, if accept { 0 } else { 1 }]
}

/// L'identifiant de session (`ctrl_message_received_session_id`) : un
/// octet de taille (`0x4a` en général), puis 24 caractères alphanumériques
/// au moins.
pub(crate) fn parse_session_id(payload: &[u8]) -> Option<String> {
    let id = payload.get(1..)?;
    if id.len() < 24 || id.len() >= SESSION_ID_SIZE_MAX - 1 || !id.iter().all(u8::is_ascii_alphanumeric) {
        return None;
    }
    String::from_utf8(id.to_vec()).ok()
}

/// Identifiant de secours de chiaki-ng : l'heure en secondes, puis 48 octets
/// au hasard en base64.
fn fallback_session_id() -> String {
    let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    let mut rand = [0u8; 48];
    let _ = getrandom::getrandom(&mut rand);
    format!("{secs}{}", base64::engine::general_purpose::STANDARD.encode(rand))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn keys() -> Keys {
        let mut regist_key = [0u8; KEY_SIZE];
        regist_key[..8].copy_from_slice(b"1a2b3c4d");
        Keys { regist_key, morning: [7u8; KEY_SIZE] }
    }

    #[test]
    fn session_request_ps5() {
        let text = session_request(Target::Ps5V1, Ipv4Addr::new(192, 168, 1, 110), &keys());
        assert_eq!(
            text,
            "GET /sie/ps5/rp/sess/init HTTP/1.1\r\nHost: 192.168.1.110:9295\r\nUser-Agent: remoteplay Windows\r\n\
             Connection: close\r\nContent-Length: 0\r\nRP-Registkey: 3161326233633464\r\nRp-Version: 1.0\r\n\r\n"
        );
    }

    #[test]
    fn keys_from_registered() {
        let r: Registered = serde_json::from_value(serde_json::json!({
            "target": "ps5_v1", "nickname": "x", "mac": "C0151B2DAE08",
            "regist_key": "3161326233633464", "rp_key": "00112233445566778899aabbccddeeff", "rp_key_type": 2
        }))
        .unwrap();
        let k = Keys::from_registered(&r).unwrap();
        assert_eq!(&k.regist_key[..8], b"1a2b3c4d");
        assert_eq!(&k.regist_key[8..], &[0u8; 8]);
        assert_eq!(k.regist_key_hex(), "3161326233633464");
        assert_eq!(k.morning[15], 0xff);
    }

    #[test]
    fn ctrl_request_ps5_headers_decrypt() {
        let nonce = [3u8; KEY_SIZE];
        let k = keys();
        let crypt = RpCrypt::auth(Target::Ps5V1, &nonce, &k.morning);
        let did = random_did();
        let (text, counter) = ctrl_request(&crypt, Target::Ps5V1, Ipv4Addr::new(10, 0, 0, 1), &k, &did, Codec::H265);
        assert_eq!(counter, 5);
        assert!(text.starts_with("GET /sie/ps5/rp/sess/ctrl HTTP/1.1\r\n"));
        assert!(text.contains("RP-PadProcNo: 3\r\n"));
        assert!(text.ends_with("\r\n\r\n"));
        let header = |name: &str| -> Vec<u8> {
            let line = text.lines().find(|l| l.starts_with(name)).unwrap();
            base64::engine::general_purpose::STANDARD.decode(line[name.len() + 2..].trim()).unwrap()
        };
        let dec = |mut b: Vec<u8>, c: u64| {
            crypt.decrypt(c, &mut b);
            b
        };
        assert_eq!(dec(header("RP-Auth"), 0), k.regist_key.to_vec());
        assert_eq!(dec(header("RP-Did"), 1), did.to_vec());
        assert_eq!(dec(header("RP-OSType"), 2), b"Win10.0.0\0".to_vec());
        assert_eq!(dec(header("RP-StartBitrate"), 3), vec![0; 4]);
        assert_eq!(dec(header("RP-StreamingType"), 4), vec![2, 0, 0, 0]);
    }

    #[test]
    fn ctrl_request_ps4_pre10_has_no_bitrate() {
        let k = keys();
        let crypt = RpCrypt::auth(Target::Ps4V9, &[1u8; KEY_SIZE], &k.morning);
        let (text, counter) = ctrl_request(&crypt, Target::Ps4V9, Ipv4Addr::LOCALHOST, &k, &random_did(), Codec::H264);
        assert_eq!(counter, 3);
        assert!(text.starts_with("GET /sce/rp/session/ctrl HTTP/1.1"));
        assert!(!text.contains("RP-StartBitrate") && !text.contains("RP-StreamingType") && !text.contains("PadProcNo"));
    }

    #[test]
    fn session_ids() {
        let mut p = vec![0x4a];
        p.extend_from_slice(b"AbCdEfGhIjKlMnOpQrStUvWx0123");
        assert_eq!(parse_session_id(&p).as_deref(), Some("AbCdEfGhIjKlMnOpQrStUvWx0123"));
        assert_eq!(parse_session_id(&[0x4a, b'a', b'b']), None);
        let mut bad = vec![0x4a];
        bad.extend_from_slice(b"AbCdEfGhIjKlMnOpQrStUvWx-123");
        assert_eq!(parse_session_id(&bad), None);
        assert!(fallback_session_id().len() > 64);
    }

    #[test]
    fn display_states() {
        let mut d = DisplayState::default();
        assert_eq!(d.update(msg::DISPLAY_A, &[0]), Some(false));
        assert_eq!(d.update(msg::DISPLAY_B, &[0, 0]), None);
        assert_eq!(d.update(msg::DISPLAY_A, &[1]), None);
        assert_eq!(d.update(msg::DISPLAY_B, &[0, 0]), Some(true));
        assert_eq!(d.update(msg::DISPLAY_B, &[0, 0]), None);
        assert_eq!(d.update(msg::DISPLAY_A, &[0]), None);
        assert_eq!(d.update(msg::DISPLAY_B, &[1, 0xff]), None);
        assert_eq!(d.update(msg::DISPLAY_A, &[0]), Some(false));
    }

    #[test]
    fn keyboard_messages() {
        // Ouverture : 28 octets inconnus, taille, texte.
        let mut open = vec![0xaau8; 0x1c];
        open.extend_from_slice(&5u32.to_be_bytes());
        open.extend_from_slice("héé".as_bytes());
        assert_eq!(parse_keyboard_open(&open).as_deref(), Some("héé"));
        // Sans texte.
        let mut empty = vec![0u8; 0x1c];
        empty.extend_from_slice(&0u32.to_be_bytes());
        assert_eq!(parse_keyboard_open(&empty).as_deref(), Some(""));
        // Trop court, ou texte plus court que sa taille : refusé.
        assert_eq!(parse_keyboard_open(&[0; 10]), None);
        let mut short = vec![0u8; 0x1c];
        short.extend_from_slice(&9u32.to_be_bytes());
        short.extend_from_slice(b"abc");
        assert_eq!(parse_keyboard_open(&short), None);
        // Taille énorme : pas de dépassement.
        let mut huge = vec![0u8; 0x1c];
        huge.extend_from_slice(&u32::MAX.to_be_bytes());
        assert_eq!(parse_keyboard_open(&huge), None);

        // Demande de changement : compteur, taille, 24 octets nuls, taille, texte.
        let req = keyboard_text_request(3, "Ça va");
        assert_eq!(req.len(), 36 + 6);
        assert_eq!(&req[0..4], &[0, 0, 0, 3]);
        assert_eq!(&req[4..8], &[0, 0, 0, 6]);
        assert!(req[8..32].iter().all(|&b| b == 0));
        assert_eq!(&req[32..36], &[0, 0, 0, 6]);
        assert_eq!(&req[36..], "Ça va".as_bytes());
        assert_eq!(keyboard_text_request(1, "").len(), 36);

        // Réponse de la console : taille à l'octet 8, texte après 40 octets.
        let mut res = vec![0u8; 40];
        res[0..4].copy_from_slice(&7u32.to_be_bytes());
        res[8..12].copy_from_slice(&4u32.to_be_bytes());
        res[36..40].copy_from_slice(&4u32.to_be_bytes());
        res.extend_from_slice(b"kyb!");
        assert_eq!(parse_keyboard_text(&res).as_deref(), Some("kyb!"));
        assert_eq!(parse_keyboard_text(&res[..39]), None);
        // Un zéro dans le texte le termine (chaîne C de chiaki-ng).
        let mut nul = vec![0u8; 40];
        nul[8..12].copy_from_slice(&3u32.to_be_bytes());
        nul.extend_from_slice(b"a\0b");
        assert_eq!(parse_keyboard_text(&nul).as_deref(), Some("a"));

        assert_eq!(keyboard_close_request(true), [0, 0, 0, 0]);
        assert_eq!(keyboard_close_request(false), [0, 0, 0, 1]);
    }

    #[test]
    fn rp_versions() {
        assert_eq!(parse_rp_version("1.0", true), Some(Target::Ps5V1));
        assert_eq!(parse_rp_version("10.0", false), Some(Target::Ps4V10));
        assert_eq!(parse_rp_version("5.0", false), Some(Target::Ps4V9));
        assert_eq!(parse_rp_version("2.0", true), None);
    }
}
