//! Association d'une console par le code à 8 chiffres, d'après `regist.c` de
//! chiaki-ng (sans la variante par les serveurs de Sony).
//!
//! 1. **Recherche** : `SRC3` (PS5) ou `SRC2` (PS4) en UDP vers le port 9295 de
//!    la console, qui répond `RES3` / `RES2` tant que son écran « Associer un
//!    appareil » est ouvert. Sinon elle refuse (port fermé) ou se tait.
//! 2. **Demande** : en TCP sur le même port, un en-tête HTTP
//!    (`POST /sie/ps5/rp/sess/rgst`) puis 0x1e0 octets de remplissage qui
//!    portent `ambassador` masqué, puis des en-têtes chiffrés (type de client
//!    et identifiant du compte PSN).
//! 3. **Réponse** : `200` et un corps chiffré avec les mêmes clés : la clé de
//!    réveil (`PS5-RegistKey`), la clé de session (`RP-Key`), l'adresse
//!    matérielle de la console et son nom. Sinon un code d'erreur dans
//!    `RP-Application-Reason` (code faux, compte refusé…).
//!
//! Le résultat, [`Registered`], est ce que l'app garde pour chaque console.

use std::io::{self, Read, Write};
use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4, TcpStream, UdpSocket};
use std::time::{Duration, Instant};

use base64::Engine;
use serde::{Deserialize, Serialize};

use super::discovery::Target;
use super::rpcrypt::{self, KEY_SIZE, RpCrypt};

/// Port de l'association, en UDP (recherche) puis en TCP (demande).
pub const PORT: u16 = 9295;

/// Attente de la réponse à la recherche, puis de la réponse à la demande.
const SEARCH_TIMEOUT: Duration = Duration::from_secs(3);
const RESPONSE_TIMEOUT: Duration = Duration::from_secs(3);
/// La PS4 n'accepte pas la demande tout de suite après la recherche.
const AFTER_SEARCH: Duration = Duration::from_millis(100);

/// Taille du remplissage avant les en-têtes chiffrés.
const INNER_HEADER_OFF: usize = 0x1e0;

/// `Client-Type` des PS4 8.0 et suivantes et des PS5 (empreinte fixe de
/// l'app de Sony, reprise de chiaki-ng).
const CLIENT_TYPE: &str = "dabfa2ec873de5839bee8d3f4c0239c4282c07c25c6077a2931afcf0adc0d34f";
const CLIENT_TYPE_PS4_PRE10: &str = "Windows";

/// Raisons d'échec que la console donne dans `RP-Application-Reason`.
pub const REASON_REGIST_FAILED: u32 = 0x80108b09;
pub const REASON_INVALID_PSN_ID: u32 = 0x80108b02;
pub const REASON_IN_USE: u32 = 0x80108b10;
pub const REASON_CRASH: u32 = 0x80108b15;
pub const REASON_RP_VERSION: u32 = 0x80108b11;

/// Identifiant du compte PSN : 8 octets, le `user_id` décimal du compte en
/// petit-boutiste (voir `psn::account_id`). Les PS4 avant 7.0 veulent à la
/// place le nom en ligne (`OnlineId`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PsnId {
    AccountId([u8; 8]),
    OnlineId(String),
}

impl PsnId {
    /// D'après le `user_id` décimal que donnent les serveurs de Sony.
    pub fn from_user_id(user_id: u64) -> Self {
        PsnId::AccountId(user_id.to_le_bytes())
    }

    /// D'après l'identifiant en base64 (12 caractères) qu'affichent
    /// chiaki-ng et ses outils : la saisie à la main, en secours.
    pub fn from_base64(text: &str) -> Option<Self> {
        let bytes = base64::engine::general_purpose::STANDARD.decode(text.trim()).ok()?;
        Some(PsnId::AccountId(bytes.try_into().ok()?))
    }

    /// L'identifiant en base64, s'il s'agit d'un `AccountId`.
    pub fn to_base64(&self) -> Option<String> {
        match self {
            PsnId::AccountId(id) => Some(base64::engine::general_purpose::STANDARD.encode(id)),
            PsnId::OnlineId(_) => None,
        }
    }
}

/// Une console associée, telle que l'app la garde (JSON stable).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Registered {
    /// `PS5` ou `PS4`, et la version du protocole de session au moment de
    /// l'association.
    pub target: TargetName,
    /// `PS5-Nickname` / `PS4-Nickname`.
    pub nickname: String,
    /// `PS5-Mac` / `PS4-Mac`, 12 chiffres hexadécimaux : c'est aussi le
    /// `host-id` de la découverte, qui relie la console trouvée à celle-ci.
    pub mac: String,
    /// `PS5-RegistKey` en hexadécimal, tel que reçu : sert au réveil
    /// ([`Registered::wake_credential`]) et à la session.
    pub regist_key: String,
    /// `RP-Key` en hexadécimal (16 octets) : le `morning` de la session.
    pub rp_key: String,
    /// `RP-KeyType`.
    pub rp_key_type: u32,
    /// `AP-*` : réseau Wi-Fi direct de la console, s'il y en a un.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub ap_ssid: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub ap_bssid: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub ap_key: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub ap_name: String,
}

/// [`Target`] sous une forme stable pour le JSON.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TargetName {
    Ps4V8,
    Ps4V9,
    Ps4V10,
    Ps5V1,
}

impl From<TargetName> for Target {
    fn from(t: TargetName) -> Self {
        match t {
            TargetName::Ps4V8 => Target::Ps4V8,
            TargetName::Ps4V9 => Target::Ps4V9,
            TargetName::Ps4V10 => Target::Ps4V10,
            TargetName::Ps5V1 => Target::Ps5V1,
        }
    }
}

impl TargetName {
    /// La cible à associer : les inconnues prennent la version la plus
    /// récente de leur famille, comme chiaki-ng.
    pub fn from_target(t: Target) -> Self {
        match t {
            Target::Ps4V8 => TargetName::Ps4V8,
            Target::Ps4V9 => TargetName::Ps4V9,
            Target::Ps4V10 | Target::Ps4Unknown => TargetName::Ps4V10,
            Target::Ps5V1 | Target::Ps5Unknown => TargetName::Ps5V1,
        }
    }
}

impl Registered {
    /// La clé de réveil (`user-credential`), voir
    /// [`super::discovery::credential_from_regist_key`].
    pub fn wake_credential(&self) -> Option<u64> {
        let bytes = parse_hex(&self.regist_key)?;
        super::discovery::credential_from_regist_key(&bytes)
    }

    /// `RP-Key` en octets.
    pub fn rp_key_bytes(&self) -> Option<[u8; KEY_SIZE]> {
        parse_hex(&self.rp_key)?.try_into().ok()
    }
}

/// Pourquoi l'association n'a pas abouti.
#[derive(Debug)]
pub enum RegistError {
    /// La console n'a pas répondu à la recherche : son écran « Associer un
    /// appareil » n'est pas ouvert, ou elle est injoignable.
    NoAnswer,
    /// La console a fermé la porte à la recherche (même cause, réponse
    /// immédiate).
    Refused,
    /// La console a refusé la demande : `RP-Application-Reason`
    /// ([`REASON_REGIST_FAILED`] : code faux ou expiré).
    Rejected { status: u16, reason: Option<u32> },
    /// Réponse illisible.
    InvalidResponse(String),
    Network(io::Error),
}

impl std::fmt::Display for RegistError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RegistError::NoAnswer => write!(f, "la console ne répond pas (écran « Associer un appareil » fermé ?)"),
            RegistError::Refused => write!(f, "la console refuse la recherche (écran « Associer un appareil » fermé ?)"),
            RegistError::Rejected { status, reason } => {
                write!(f, "la console refuse l'association (HTTP {status}")?;
                if let Some(r) = reason {
                    write!(f, ", raison {r:#x} : {}", reason_text(*r))?;
                }
                write!(f, ")")
            }
            RegistError::InvalidResponse(why) => write!(f, "réponse illisible : {why}"),
            RegistError::Network(e) => write!(f, "réseau : {e}"),
        }
    }
}

impl std::error::Error for RegistError {}

impl From<io::Error> for RegistError {
    fn from(e: io::Error) -> Self {
        RegistError::Network(e)
    }
}

/// Texte d'une raison d'échec, pour le journal.
pub fn reason_text(reason: u32) -> &'static str {
    match reason {
        REASON_REGIST_FAILED => "association refusée, code sans doute faux ou expiré",
        REASON_INVALID_PSN_ID => "identifiant PSN refusé",
        REASON_IN_USE => "lecture à distance déjà en cours",
        REASON_CRASH => "la lecture à distance de la console a planté",
        REASON_RP_VERSION => "version du protocole refusée",
        _ => "inconnue",
    }
}

/// `RP-Version` (`chiaki_rp_version_string`).
pub(crate) fn rp_version(target: Target) -> Option<&'static str> {
    match target {
        Target::Ps4V8 => Some("8.0"),
        Target::Ps4V9 => Some("9.0"),
        Target::Ps4V10 => Some("10.0"),
        Target::Ps5V1 => Some("1.0"),
        _ => None,
    }
}

fn request_path(target: Target) -> &'static str {
    match target {
        Target::Ps5Unknown | Target::Ps5V1 => "/sie/ps5/rp/sess/rgst",
        Target::Ps4V8 | Target::Ps4V9 => "/sce/rp/regist",
        _ => "/sie/ps4/rp/sess/rgst",
    }
}

/// En-tête HTTP de la demande (`request_header_format`), tel quel : le
/// ` HTTP/1.1` répété de la première ligne et l'hôte fixe `10.0.2.15`
/// viennent de l'app de Sony.
pub fn request_header(target: Target, payload_len: usize) -> String {
    let mut head = format!(
        "POST {} HTTP/1.1\r\n HTTP/1.1\r\n\
         HOST: 10.0.2.15\r\n\
         User-Agent: remoteplay Windows\r\n\
         Connection: close\r\n\
         Content-Length: {payload_len}\r\n",
        request_path(target)
    );
    if rpcrypt::target_code(target) >= rpcrypt::target_code(Target::Ps4V9) {
        if let Some(v) = rp_version(target) {
            head.push_str(&format!("RP-Version: {v}\r\n"));
        }
    }
    head.push_str("\r\n");
    head
}

/// Corps de la demande et les clés de l'échange
/// (`chiaki_regist_request_payload_format`). `None` si la cible ou
/// l'identifiant ne conviennent pas.
pub fn request_payload(target: Target, ambassador: &[u8; KEY_SIZE], id: &PsnId, pin: u32) -> Option<(Vec<u8>, RpCrypt)> {
    let mut buf = vec![b'A'; INNER_HEADER_OFF];
    let pre10 = rpcrypt::is_ps4_pre10(target);
    let crypt = if pre10 {
        let crypt = RpCrypt::regist_ps4_pre10(ambassador, pin);
        buf[0x11c..0x12c].copy_from_slice(&rpcrypt::aeropause_ps4_pre10(crypt.ambassador()));
        crypt
    } else {
        let key_0_off = (buf[0x18d] & 0x1f) as usize;
        let key_1_off = (buf[0] >> 3) as usize;
        let crypt = RpCrypt::regist(target, ambassador, key_0_off, pin)?;
        let aeropause = rpcrypt::aeropause(target, key_1_off, crypt.ambassador())?;
        buf[0xc7..0xcf].copy_from_slice(&aeropause[8..]);
        buf[0x191..0x199].copy_from_slice(&aeropause[..8]);
        crypt
    };
    // Après 8.0, chiaki-ng ignore le nom en ligne : il faut l'identifiant.
    let inner = match id {
        PsnId::OnlineId(name) if pre10 => format!("Client-Type: Windows\r\nNp-Online-Id: {name}\r\n"),
        PsnId::OnlineId(_) => return None,
        PsnId::AccountId(account) => format!(
            "Client-Type: {}\r\nNp-AccountId: {}\r\n",
            if pre10 { CLIENT_TYPE_PS4_PRE10 } else { CLIENT_TYPE },
            base64::engine::general_purpose::STANDARD.encode(account)
        ),
    };
    let mut inner = inner.into_bytes();
    crypt.encrypt(0, &mut inner);
    buf.extend_from_slice(&inner);
    Some((buf, crypt))
}

/// Associe la console à `addr` avec le code `pin` affiché par elle.
/// Bloque quelques secondes au plus : à appeler hors du fil de l'interface.
pub fn regist(addr: Ipv4Addr, target: Target, id: &PsnId, pin: u32) -> Result<Registered, RegistError> {
    let target: Target = TargetName::from_target(target).into();
    let mut ambassador = [0u8; KEY_SIZE];
    getrandom::getrandom(&mut ambassador).map_err(|e| io::Error::other(e.to_string()))?;
    let (payload, crypt) = request_payload(target, &ambassador, id, pin)
        .ok_or_else(|| RegistError::InvalidResponse("identifiant PSN ou cible inutilisable".into()))?;
    let header = request_header(target, payload.len());

    let console = search(addr, rpcrypt::is_ps5(target))?;
    std::thread::sleep(AFTER_SEARCH);

    let mut stream = TcpStream::connect_timeout(&console, RESPONSE_TIMEOUT)?;
    stream.set_write_timeout(Some(RESPONSE_TIMEOUT))?;
    stream.write_all(header.as_bytes())?;
    stream.write_all(&payload)?;
    log::info!("PlayStation : demande d'association envoyée à {console}");

    let (status, headers, mut body) = read_response(&mut stream)?;
    if status != 200 {
        let reason = header_value(&headers, "RP-Application-Reason").and_then(|v| u32::from_str_radix(v.trim_start_matches("0x"), 16).ok());
        return Err(RegistError::Rejected { status, reason });
    }
    crypt.decrypt(0, &mut body);
    parse_registered(target, &body)
}

/// La recherche `SRC3` / `SRC2` ; rend l'adresse qui a répondu (port 9295).
fn search(addr: Ipv4Addr, ps5: bool) -> Result<SocketAddr, RegistError> {
    let (src, res): (&[u8], &[u8]) = if ps5 { (b"SRC3\0", b"RES3") } else { (b"SRC2\0", b"RES2") };
    let socket = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0))?;
    socket.connect(SocketAddrV4::new(addr, PORT))?;
    socket.send(src)?;
    let deadline = Instant::now() + SEARCH_TIMEOUT;
    let mut buf = [0u8; 256];
    loop {
        let left = deadline.saturating_duration_since(Instant::now());
        if left.is_zero() {
            return Err(RegistError::NoAnswer);
        }
        socket.set_read_timeout(Some(left))?;
        match socket.recv(&mut buf) {
            Ok(n) if buf[..n].starts_with(res) => return Ok(socket.peer_addr()?),
            Ok(_) => {}
            Err(e) if e.kind() == io::ErrorKind::ConnectionRefused => return Err(RegistError::Refused),
            Err(e) if matches!(e.kind(), io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut) => return Err(RegistError::NoAnswer),
            Err(e) => return Err(e.into()),
        }
    }
}

/// Lit une réponse HTTP : statut, en-têtes, corps (`Content-Length`).
fn read_response(stream: &mut TcpStream) -> Result<(u16, Vec<(String, String)>, Vec<u8>), RegistError> {
    stream.set_read_timeout(Some(RESPONSE_TIMEOUT))?;
    let deadline = Instant::now() + RESPONSE_TIMEOUT;
    let mut data = Vec::new();
    let mut chunk = [0u8; 1500];
    let header_end = loop {
        if let Some(pos) = data.windows(4).position(|w| w == b"\r\n\r\n") {
            break pos + 4;
        }
        if Instant::now() >= deadline || data.len() > 8192 {
            return Err(RegistError::InvalidResponse("en-tête HTTP incomplet".into()));
        }
        let n = stream.read(&mut chunk)?;
        if n == 0 {
            return Err(RegistError::InvalidResponse("connexion fermée avant la réponse".into()));
        }
        data.extend_from_slice(&chunk[..n]);
    };
    let head = String::from_utf8_lossy(&data[..header_end]).into_owned();
    log::debug!("PlayStation : réponse d'association : {:?} (+{} octets)", head, data.len() - header_end);
    let mut lines = head.split("\r\n");
    let status_line = lines.next().unwrap_or_default();
    let status: u16 = status_line
        .split(' ')
        .nth(1)
        .and_then(|s| s.parse().ok())
        .ok_or_else(|| RegistError::InvalidResponse(format!("statut illisible : {status_line}")))?;
    let headers = parse_headers(lines.collect::<Vec<_>>().join("\n").as_bytes());
    if status != 200 {
        return Ok((status, headers, Vec::new()));
    }
    // Sans `Content-Length` lisible, le corps va jusqu'à la fermeture
    // (`Connection: close`).
    let length: Option<usize> = header_value(&headers, "Content-Length")
        .and_then(|v| v.trim().parse().ok())
        .filter(|&n| n > 0 && n <= 4096);
    let mut body = data[header_end..].to_vec();
    while length.is_none_or(|l| body.len() < l) {
        if Instant::now() >= deadline || body.len() > 4096 {
            if length.is_none() && !body.is_empty() {
                break;
            }
            return Err(RegistError::InvalidResponse("corps incomplet".into()));
        }
        let n = match stream.read(&mut chunk) {
            Ok(n) => n,
            Err(e) if length.is_none() && !body.is_empty() && matches!(e.kind(), io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut) => 0,
            Err(e) => return Err(e.into()),
        };
        if n == 0 {
            if length.is_none() && !body.is_empty() {
                break;
            }
            return Err(RegistError::InvalidResponse("corps incomplet".into()));
        }
        body.extend_from_slice(&chunk[..n]);
    }
    if let Some(l) = length {
        body.truncate(l);
    }
    Ok((status, headers, body))
}

/// En-têtes `clé: valeur` séparés par `\r\n` ou `\n` (`chiaki_http_header_parse` :
/// une espace au plus après les deux-points ; arrêt au premier octet nul).
fn parse_headers(data: &[u8]) -> Vec<(String, String)> {
    let end = data.iter().position(|&b| b == 0).unwrap_or(data.len());
    String::from_utf8_lossy(&data[..end])
        .split(['\r', '\n'])
        .filter_map(|line| {
            let (k, v) = line.split_once(':')?;
            Some((k.to_string(), v.strip_prefix(' ').unwrap_or(v).to_string()))
        })
        .filter(|(k, _)| !k.is_empty())
        .collect()
}

fn header_value<'a>(headers: &'a [(String, String)], key: &str) -> Option<&'a str> {
    headers.iter().find(|(k, _)| k.eq_ignore_ascii_case(key)).map(|(_, v)| v.as_str())
}

fn parse_hex(text: &str) -> Option<Vec<u8>> {
    let text = text.trim();
    if text.len() % 2 != 0 {
        return None;
    }
    (0..text.len()).step_by(2).map(|i| u8::from_str_radix(&text[i..i + 2], 16).ok()).collect()
}

/// Le corps déchiffré de la réponse (`regist_parse_response_payload`).
pub(crate) fn parse_registered(target: Target, body: &[u8]) -> Result<Registered, RegistError> {
    let headers = parse_headers(body);
    let ps5 = rpcrypt::is_ps5(target);
    let get = |key: &str| header_value(&headers, key).map(str::to_string);
    let prefix = if ps5 { "PS5" } else { "PS4" };
    let regist_key = get(&format!("{prefix}-RegistKey"))
        .filter(|k| parse_hex(k).is_some_and(|b| !b.is_empty() && b.len() <= KEY_SIZE))
        .ok_or_else(|| RegistError::InvalidResponse("clé d'association absente".into()))?;
    let rp_key = get("RP-Key")
        .filter(|k| parse_hex(k).is_some_and(|b| b.len() == KEY_SIZE))
        .ok_or_else(|| RegistError::InvalidResponse("clé de session absente".into()))?;
    let mac = get(&format!("{prefix}-Mac"))
        .filter(|m| parse_hex(m).is_some_and(|b| b.len() == 6))
        .ok_or_else(|| RegistError::InvalidResponse("adresse matérielle absente".into()))?;
    for (k, v) in &headers {
        if k == "RP-SupportCmd" {
            log::info!("PlayStation : RP-SupportCmd {v}");
        }
    }
    Ok(Registered {
        target: TargetName::from_target(target),
        nickname: get(&format!("{prefix}-Nickname")).unwrap_or_default(),
        mac: mac.to_ascii_uppercase(),
        regist_key,
        rp_key,
        rp_key_type: get("RP-KeyType").and_then(|v| v.trim().parse().ok()).unwrap_or(0),
        ap_ssid: get("AP-Ssid").unwrap_or_default(),
        ap_bssid: get("AP-Bssid").unwrap_or_default(),
        ap_key: get("AP-Key").unwrap_or_default(),
        ap_name: get("AP-Name").unwrap_or_default(),
    })
}

#[cfg(test)]
mod tests {
    use super::super::test_vectors::*;
    use super::*;

    #[test]
    fn payload_ps4_pre10_matches_chiaki() {
        let id = PsnId::OnlineId("ChiakiNanami1337".into());
        let (payload, _) = request_payload(Target::Ps4V9, &REGIST_AMBASSADOR, &id, 13374201).unwrap();
        assert_eq!(payload, REGIST_REQUEST_PAYLOAD_PS4_PRE10_EXPECTED.to_vec());
    }

    #[test]
    fn payload_ps5_layout() {
        let id = PsnId::from_user_id(0x0123_4567_89ab_cdef);
        let (payload, crypt) = request_payload(Target::Ps5V1, &REGIST_AMBASSADOR, &id, 12345678).unwrap();
        // Remplissage, puis `ambassador` masqué aux deux endroits.
        let aeropause = rpcrypt::aeropause(Target::Ps5V1, 8, &REGIST_AMBASSADOR).unwrap();
        assert_eq!(&payload[0xc7..0xcf], &aeropause[8..]);
        assert_eq!(&payload[0x191..0x199], &aeropause[..8]);
        assert!(payload[..0xc7].iter().all(|&b| b == b'A'));
        // Les en-têtes se déchiffrent avec les mêmes clés.
        let mut inner = payload[INNER_HEADER_OFF..].to_vec();
        crypt.decrypt(0, &mut inner);
        let text = String::from_utf8(inner).unwrap();
        assert_eq!(text, format!("Client-Type: {CLIENT_TYPE}\r\nNp-AccountId: 782riWdFIwE=\r\n"));
        // Le code est mêlé à la fin de `bright`.
        let plain = RpCrypt::regist(Target::Ps5V1, &REGIST_AMBASSADOR, 1, 0).unwrap();
        let pin = 12345678u32.to_be_bytes();
        for i in 0..4 {
            assert_eq!(crypt.bright()[0xc + i], plain.bright()[0xc + i] ^ pin[i]);
        }
    }

    #[test]
    fn header_ps5() {
        assert_eq!(
            request_header(Target::Ps5V1, 0x216),
            "POST /sie/ps5/rp/sess/rgst HTTP/1.1\r\n HTTP/1.1\r\nHOST: 10.0.2.15\r\nUser-Agent: remoteplay Windows\r\n\
             Connection: close\r\nContent-Length: 534\r\nRP-Version: 1.0\r\n\r\n"
        );
    }

    #[test]
    fn psn_id_base64() {
        let id = PsnId::from_user_id(0x0123_4567_89ab_cdef);
        assert_eq!(id.to_base64().as_deref(), Some("782riWdFIwE="));
        assert_eq!(PsnId::from_base64(" 782riWdFIwE= "), Some(id));
        assert_eq!(PsnId::from_base64("abc"), None);
    }

    #[test]
    fn parses_reply() {
        let body = b"AP-Ssid: PS5-123\r\nPS5-Nickname: PS5 de Matthieu\r\nPS5-RegistKey: 3132333435363738\r\n\
            RP-KeyType: 2\r\nRP-Key: 00112233445566778899aabbccddeeff\r\nPS5-Mac: c0151b2dae08\r\nRP-SupportCmd: 7\r\n";
        let r = parse_registered(Target::Ps5V1, body).unwrap();
        assert_eq!(r.nickname, "PS5 de Matthieu");
        assert_eq!(r.mac, "C0151B2DAE08");
        assert_eq!(r.rp_key_type, 2);
        assert_eq!(r.wake_credential(), Some(0x1234_5678));
        assert_eq!(r.rp_key_bytes().unwrap()[15], 0xff);
        let json = serde_json::to_string(&r).unwrap();
        assert_eq!(serde_json::from_str::<Registered>(&json).unwrap(), r);
    }

    #[test]
    fn content_length_with_spaces() {
        // Réponse réelle d'une PS5 Pro (système 14.20) : plusieurs espaces.
        let headers = parse_headers(b"Connection: close\nRP-Feature: 3\nContent-Length:        203\n");
        let length: Option<usize> = header_value(&headers, "Content-Length").and_then(|v| v.trim().parse().ok());
        assert_eq!(length, Some(203));
    }

    #[test]
    fn reply_without_key_is_refused() {
        let body = b"PS5-Nickname: x\r\nPS5-Mac: c0151b2dae08\r\n";
        assert!(matches!(parse_registered(Target::Ps5V1, body), Err(RegistError::InvalidResponse(_))));
    }
}
