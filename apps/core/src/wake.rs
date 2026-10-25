//! Réveil d'un ordinateur par le réseau (Wake-on-LAN) : le paquet magique
//! (6 octets `FF`, puis 16 fois l'adresse MAC de la carte) part en UDP, ports
//! 9 et 7, vers l'adresse de diffusion du sous-réseau de chaque carte, vers
//! 255.255.255.255 et, en direct, vers la dernière adresse IPv4 connue de
//! chaque carte. Une erreur sur une adresse n'arrête rien : le réveil est
//! envoyé dès qu'un paquet est parti.
//!
//! Les cibles viennent d'Admin (`wake` de `GET /api/stream/info`, voir
//! [`crate::console::Info::wake`]) : les cartes Ethernet et Wi-Fi en service
//! de l'ordinateur, Ethernet d'abord. Les apps les gardent pour chaque
//! ordinateur, telles quelles ([`WakeTarget`] est un JSON stable), pour le
//! réveiller une fois éteint ; elles remplacent les cibles gardées par celles
//! d'un `info` réussi quand la liste n'est pas vide.
//!
//! **iPhone, iPad et Apple TV** : envoyer vers une adresse de diffusion
//! demande l'autorisation `com.apple.developer.networking.multicast`,
//! qu'Apple accorde sur demande. Sans elle, l'envoi en diffusion échoue
//! (souvent `EHOSTUNREACH`) ; l'envoi direct passe, mais ne réveille que si
//! le réseau sait encore joindre l'adresse (cache ARP du routeur, carte en
//! veille qui répond encore à l'ARP). L'envoi direct est donc toujours tenté
//! en plus de la diffusion, et un refus de la seule diffusion n'est pas un
//! échec. Sur Mac, pas de restriction (l'autorisation « Réseau local » suffit,
//! comme pour joindre Admin).

use std::collections::BTreeSet;
use std::net::{Ipv4Addr, UdpSocket};

use serde::{Deserialize, Serialize};

use crate::i18n::{self, Lang, tr};

/// Ports du paquet magique : 9 (discard), l'usage, et 7 (echo), que
/// certaines cartes et certains routeurs attendent.
pub const PORTS: [u16; 2] = [9, 7];

/// Longueur du paquet magique : 6 octets `FF` et 16 fois l'adresse MAC.
pub const PACKET_LEN: usize = 6 + 16 * 6;

/// Une carte réseau de l'ordinateur à réveiller, telle qu'Admin la donne et
/// que les apps la gardent. Tous les champs sauf `mac` sont facultatifs : une
/// app peut aussi en écrire une à partir d'une adresse saisie
/// ([`WakeTarget::from_mac`]).
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct WakeTarget {
    /// Adresse matérielle de la carte, `AA:BB:CC:DD:EE:FF`.
    pub mac: String,
    /// Dernière adresse IPv4 connue de la carte (envoi direct).
    #[serde(default)]
    pub ipv4: Option<String>,
    /// Adresse de diffusion de son sous-réseau (192.168.1.255 pour
    /// 192.168.1.20/24) ; à défaut, celle du /24 de `ipv4`.
    #[serde(default)]
    pub broadcast: Option<String>,
    /// `ethernet` ou `wifi` (vide : inconnu).
    #[serde(default)]
    pub kind: String,
    /// Windows permet à cette carte de réveiller l'ordinateur (réglage de la
    /// carte dans le Gestionnaire de périphériques) ; `None` : inconnu.
    #[serde(default)]
    pub armed: Option<bool>,
    /// Nom de la carte (« Intel(R) Ethernet Connection I219-V »), vide s'il
    /// est inconnu.
    #[serde(default)]
    pub name: String,
}

impl WakeTarget {
    /// Une cible à partir d'une adresse MAC saisie (`3c:7c:3f:12:ab:cd`,
    /// `3C-7C-3F-12-AB-CD`, `3c7c.3f12.abcd`…) et, si on la connaît,
    /// de l'adresse IPv4 de l'ordinateur ; `None` si la MAC est illisible.
    pub fn from_mac(mac: &str, ipv4: Option<&str>) -> Option<Self> {
        let mac = parse_mac(mac)?;
        Some(Self {
            mac: format_mac(mac),
            ipv4: ipv4.and_then(|a| a.trim().parse::<Ipv4Addr>().ok()).map(|a| a.to_string()),
            ..Self::default()
        })
    }
}

/// Lecture tolérante d'une liste de cibles (`wake` d'`info`) : `null`, une
/// valeur qui n'est pas une liste ou une cible illisible ne font pas échouer
/// la lecture de toute la réponse d'Admin (l'ordinateur passerait pour hors
/// ligne) ; elles sont simplement ignorées.
pub(crate) fn lenient<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Vec<WakeTarget>, D::Error> {
    let value = Option::<serde_json::Value>::deserialize(d)?;
    Ok(from_value(value.unwrap_or_default()))
}

fn from_value(value: serde_json::Value) -> Vec<WakeTarget> {
    let items = match value {
        serde_json::Value::Array(items) => items,
        // Un objet : une cible seule, ou une réponse d'`info` entière.
        serde_json::Value::Object(mut o) => match o.remove("wake") {
            Some(wake) => return from_value(wake),
            None => vec![serde_json::Value::Object(o)],
        },
        _ => Vec::new(),
    };
    items.into_iter().filter_map(|v| serde_json::from_value(v).ok()).collect()
}

/// Les cibles d'un texte JSON, pour les ponts des apps : une liste de cibles
/// (celle d'`info`, gardée par l'app), une cible seule, ou une réponse
/// d'`info` entière ; vide si rien n'est lisible.
pub fn targets_from_json(text: &str) -> Vec<WakeTarget> {
    serde_json::from_str(text).map(from_value).unwrap_or_default()
}

/// L'adresse MAC d'un texte : 12 chiffres hexadécimaux, séparés ou non par
/// `:`, `-`, `.` ou des espaces. `None` pour une adresse nulle ou de groupe
/// (premier octet impair, comme `FF:FF:FF:FF:FF:FF`) : aucune carte ne la porte.
pub fn parse_mac(text: &str) -> Option<[u8; 6]> {
    let digits: Vec<u8> = text
        .chars()
        .filter(|c| !matches!(c, ':' | '-' | '.') && !c.is_whitespace())
        .map(|c| c.to_digit(16).map(|d| d as u8))
        .collect::<Option<_>>()?;
    if digits.len() != 12 {
        return None;
    }
    let mut mac = [0u8; 6];
    for (i, pair) in digits.chunks(2).enumerate() {
        mac[i] = pair[0] << 4 | pair[1];
    }
    (mac != [0; 6] && mac[0] & 1 == 0).then_some(mac)
}

/// `AA:BB:CC:DD:EE:FF`.
pub fn format_mac(mac: [u8; 6]) -> String {
    mac.iter().map(|b| format!("{b:02X}")).collect::<Vec<_>>().join(":")
}

/// Le paquet magique d'une carte.
pub fn magic_packet(mac: [u8; 6]) -> [u8; PACKET_LEN] {
    let mut packet = [0xFFu8; PACKET_LEN];
    for chunk in packet[6..].chunks_exact_mut(6) {
        chunk.copy_from_slice(&mac);
    }
    packet
}

/// Adresse de diffusion du sous-réseau de `ip` de longueur de préfixe
/// `prefix` (192.168.1.20/24 → 192.168.1.255) ; un préfixe de plus de 32 compte
/// pour 32 (l'adresse elle-même).
pub fn broadcast(ip: Ipv4Addr, prefix: u8) -> Ipv4Addr {
    let host_bits = 32 - u32::from(prefix.min(32));
    let mask = if host_bits == 32 { 0 } else { u32::MAX << host_bits };
    Ipv4Addr::from(u32::from(ip) | !mask)
}

fn ipv4(text: Option<&str>) -> Option<Ipv4Addr> {
    text.and_then(|t| t.trim().parse::<Ipv4Addr>().ok())
        .filter(|a| !a.is_unspecified() && !a.is_loopback() && !a.is_multicast())
}

/// Où partent les paquets, sans doublon : la diffusion de chaque sous-réseau
/// (celle d'Admin, sinon le /24 de l'adresse connue), 255.255.255.255, puis
/// l'adresse connue de chaque carte, en direct.
pub fn destinations(targets: &[WakeTarget]) -> Vec<Ipv4Addr> {
    let mut out: Vec<Ipv4Addr> = Vec::new();
    let mut push = |a: Ipv4Addr| {
        if !out.contains(&a) {
            out.push(a);
        }
    };
    for t in targets {
        let known = ipv4(t.ipv4.as_deref());
        let subnet = t
            .broadcast
            .as_deref()
            .and_then(|b| b.trim().parse::<Ipv4Addr>().ok())
            .filter(|b| !b.is_unspecified())
            .or_else(|| known.map(|a| broadcast(a, 24)));
        if let Some(b) = subnet {
            push(b);
        }
    }
    push(Ipv4Addr::BROADCAST);
    for t in targets {
        if let Some(a) = ipv4(t.ipv4.as_deref()) {
            push(a);
        }
    }
    out
}

/// Échec d'un réveil.
#[derive(Debug)]
pub enum WakeError {
    /// Aucune adresse MAC lisible parmi les cibles : l'app ne connaît pas la
    /// carte réseau de l'ordinateur.
    NoTarget,
    /// Aucun paquet n'est parti (la dernière erreur du système).
    Network(std::io::Error),
}

impl WakeError {
    /// Le message dans une langue (`Display` : celle de l'app).
    pub fn text(&self, lang: Lang) -> String {
        match self {
            Self::NoTarget => tr(lang, "wake.no_target", &[]),
            Self::Network(e) => tr(lang, "wake.network", &[("error", &e.to_string())]),
        }
    }

    /// Nom stable de l'échec, pour les ponts : `no_target` ou `network`.
    pub fn kind(&self) -> &'static str {
        match self {
            Self::NoTarget => "no_target",
            Self::Network(_) => "network",
        }
    }
}

impl std::fmt::Display for WakeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.text(i18n::current()))
    }
}

impl std::error::Error for WakeError {}

/// Envoie le paquet magique de chaque carte des cibles vers chaque adresse
/// de [`destinations`], ports 9 et 7 ; rend le nombre de paquets partis.
/// Les refus par adresse sont ignorés (journal) tant qu'au moins un paquet
/// part : sur iPhone, iPad et Apple TV sans l'autorisation multicast, seule la
/// diffusion est refusée et l'envoi direct passe. Appel rapide (pas d'attente
/// réseau), mais à faire hors du fil de l'interface comme tout envoi.
pub fn send(targets: &[WakeTarget]) -> Result<usize, WakeError> {
    let macs: BTreeSet<[u8; 6]> = targets.iter().filter_map(|t| parse_mac(&t.mac)).collect();
    if macs.is_empty() {
        return Err(WakeError::NoTarget);
    }
    let destinations = destinations(targets);
    let socket = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0)).map_err(WakeError::Network)?;
    if let Err(e) = socket.set_broadcast(true) {
        log::info!("réveil : diffusion refusée par le système ({e}), envoi direct seulement");
    }
    let mut sent = 0usize;
    let mut refused: Vec<String> = Vec::new();
    let mut last = None;
    for mac in &macs {
        let packet = magic_packet(*mac);
        for address in &destinations {
            for port in PORTS {
                match socket.send_to(&packet, (*address, port)) {
                    Ok(_) => sent += 1,
                    Err(e) => {
                        let line = format!("{address} ({e})");
                        if !refused.contains(&line) {
                            refused.push(line);
                        }
                        last = Some(e);
                    }
                }
            }
        }
    }
    let cards: Vec<String> = macs.iter().map(|m| format_mac(*m)).collect();
    if refused.is_empty() {
        log::info!("réveil de {} : {sent} paquets envoyés", cards.join(", "));
    } else {
        // Sur iPhone, iPad et Apple TV, une diffusion refusée (`EHOSTUNREACH`)
        // signale l'autorisation multicast absente.
        log::info!("réveil de {} : {sent} paquets envoyés, refusés vers {}", cards.join(", "), refused.join(", "));
    }
    if sent == 0 {
        return Err(WakeError::Network(last.unwrap_or_else(|| std::io::Error::other("aucune adresse"))));
    }
    Ok(sent)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn target(mac: &str, ipv4: Option<&str>, broadcast: Option<&str>) -> WakeTarget {
        WakeTarget {
            mac: mac.into(),
            ipv4: ipv4.map(Into::into),
            broadcast: broadcast.map(Into::into),
            kind: "ethernet".into(),
            ..WakeTarget::default()
        }
    }

    #[test]
    fn paquet_magique() {
        let mac = [0x3C, 0x7C, 0x3F, 0x12, 0xAB, 0xCD];
        let packet = magic_packet(mac);
        assert_eq!(packet.len(), 102);
        assert_eq!(&packet[..6], &[0xFF; 6]);
        for i in 0..16 {
            assert_eq!(&packet[6 + 6 * i..12 + 6 * i], &mac, "répétition {i}");
        }
    }

    #[test]
    fn adresse_mac() {
        let mac = Some([0x3C, 0x7C, 0x3F, 0x12, 0xAB, 0xCD]);
        for text in ["3C:7C:3F:12:AB:CD", "3c-7c-3f-12-ab-cd", "3c7c.3f12.abcd", "3C7C3F12ABCD", " 3c:7c:3f:12:ab:cd\n", "3c 7c 3f 12 ab cd"] {
            assert_eq!(parse_mac(text), mac, "{text:?}");
        }
        // Trop court, trop long, pas hexadécimal, nulle, de groupe.
        for text in ["", "3C:7C:3F:12:AB", "3C:7C:3F:12:AB:CD:EF", "3C:7C:3F:12:AB:CG", "00:00:00:00:00:00", "FF:FF:FF:FF:FF:FF", "01:00:5E:00:00:01"] {
            assert_eq!(parse_mac(text), None, "{text:?}");
        }
        assert_eq!(format_mac([0x3C, 0x7C, 0x3F, 0x12, 0xAB, 0xCD]), "3C:7C:3F:12:AB:CD");
        assert_eq!(format_mac(parse_mac("aa:bb:cc:dd:ee:ff").unwrap()), "AA:BB:CC:DD:EE:FF");
    }

    #[test]
    fn adresse_de_diffusion() {
        let ip = Ipv4Addr::new(192, 168, 1, 20);
        assert_eq!(broadcast(ip, 24), Ipv4Addr::new(192, 168, 1, 255));
        assert_eq!(broadcast(Ipv4Addr::new(10, 0, 37, 4), 16), Ipv4Addr::new(10, 0, 255, 255));
        assert_eq!(broadcast(Ipv4Addr::new(172, 16, 5, 9), 20), Ipv4Addr::new(172, 16, 15, 255));
        assert_eq!(broadcast(ip, 30), Ipv4Addr::new(192, 168, 1, 23));
        assert_eq!(broadcast(ip, 32), ip);
        assert_eq!(broadcast(ip, 40), ip);
        assert_eq!(broadcast(ip, 0), Ipv4Addr::BROADCAST);
    }

    #[test]
    fn destinations_sans_doublon() {
        let targets = [
            target("3C:7C:3F:12:AB:CD", Some("192.168.1.20"), Some("192.168.1.255")),
            // Wi-Fi sur le même réseau : même diffusion.
            target("A4:B1:C1:00:11:22", Some("192.168.1.21"), Some("192.168.1.255")),
            // Sans diffusion donnée : le /24 de l'adresse.
            target("A4:B1:C1:00:11:23", Some("10.0.5.7"), None),
            // Carte sans adresse (reliée à un commutateur virtuel) : diffusion générale seulement.
            target("A4:B1:C1:00:11:24", None, None),
        ];
        let ips: Vec<String> = destinations(&targets).iter().map(|a| a.to_string()).collect();
        assert_eq!(ips, ["192.168.1.255", "10.0.5.255", "255.255.255.255", "192.168.1.20", "192.168.1.21", "10.0.5.7"]);
        // Adresses inutilisables écartées.
        let odd = [target("3C:7C:3F:12:AB:CD", Some("0.0.0.0"), Some("pas une adresse")), target("3C:7C:3F:12:AB:CE", Some("127.0.0.1"), None)];
        assert_eq!(destinations(&odd), vec![Ipv4Addr::BROADCAST]);
    }

    #[test]
    fn cible_saisie() {
        let t = WakeTarget::from_mac("3c-7c-3f-12-ab-cd", Some(" 192.168.1.20 ")).unwrap();
        assert_eq!(t.mac, "3C:7C:3F:12:AB:CD");
        assert_eq!(t.ipv4.as_deref(), Some("192.168.1.20"));
        assert_eq!(WakeTarget::from_mac("3c-7c-3f-12-ab-cd", Some("bureau.local")).unwrap().ipv4, None);
        assert_eq!(WakeTarget::from_mac("3c-7c", None), None);
    }

    #[test]
    fn json_stable() {
        let t = WakeTarget {
            mac: "3C:7C:3F:12:AB:CD".into(),
            ipv4: Some("192.168.1.20".into()),
            broadcast: Some("192.168.1.255".into()),
            kind: "ethernet".into(),
            armed: Some(true),
            name: "Intel(R) Ethernet Connection I219-V".into(),
        };
        let json = serde_json::to_value(&t).unwrap();
        assert_eq!(
            json,
            serde_json::json!({ "mac": "3C:7C:3F:12:AB:CD", "ipv4": "192.168.1.20", "broadcast": "192.168.1.255", "kind": "ethernet", "armed": true, "name": "Intel(R) Ethernet Connection I219-V" })
        );
        assert_eq!(serde_json::from_value::<WakeTarget>(json).unwrap(), t);
        // Champs absents ou inconnus (Station plus récente) : valeurs par défaut.
        let partial: WakeTarget = serde_json::from_str(r#"{"mac":"3C:7C:3F:12:AB:CD","plus_tard":1}"#).unwrap();
        assert_eq!(partial, WakeTarget { mac: "3C:7C:3F:12:AB:CD".into(), ..WakeTarget::default() });
    }

    #[test]
    fn json_des_ponts() {
        let list = r#"[{"mac":"3C:7C:3F:12:AB:CD","ipv4":"192.168.1.20"},{"pas":"une cible"},{"mac":"A4:B1:C1:00:11:22"}]"#;
        assert_eq!(targets_from_json(list).len(), 2);
        assert_eq!(targets_from_json(r#"{"mac":"3C:7C:3F:12:AB:CD"}"#).len(), 1);
        assert_eq!(targets_from_json(r#"{"hostname":"Bureau","wake":[{"mac":"3C:7C:3F:12:AB:CD"}]}"#).len(), 1);
        assert!(targets_from_json("pas du json").is_empty());
        assert!(targets_from_json("null").is_empty());
    }

    #[test]
    fn sans_cible() {
        assert!(matches!(send(&[]), Err(WakeError::NoTarget)));
        assert!(matches!(send(&[target("pas une mac", Some("192.168.1.20"), None)]), Err(WakeError::NoTarget)));
        assert_eq!(WakeError::NoTarget.kind(), "no_target");
        assert!(!WakeError::NoTarget.text(Lang::Fr).starts_with("wake."));
        assert!(!WakeError::NoTarget.text(Lang::En).starts_with("wake."));
    }

    /// Envoi réel sur le réseau local, vers une carte fictive (adresse
    /// administrée localement, qu'aucune carte ne porte) :
    /// `cargo test -p kybernow-core wake -- --ignored --nocapture`.
    #[test]
    #[ignore = "envoie des paquets sur le réseau local"]
    fn envoi_reel() {
        let local = UdpSocket::bind("0.0.0.0:0")
            .and_then(|s| s.connect("192.0.2.1:9").and_then(|_| s.local_addr()))
            .map(|a| a.ip().to_string())
            .ok();
        let ip = local.as_deref().and_then(|a| a.parse::<Ipv4Addr>().ok());
        let t = WakeTarget {
            mac: "02:4B:4E:00:00:01".into(),
            ipv4: ip.map(|a| a.to_string()),
            broadcast: ip.map(|a| broadcast(a, 24).to_string()),
            kind: "ethernet".into(),
            ..WakeTarget::default()
        };
        let sent = send(&[t]).expect("au moins un paquet parti");
        println!("réveil : {sent} paquets partis (adresse locale {local:?})");
        assert!(sent >= 2);
    }
}
