//! Les cartes réseau de l'ordinateur pour le réveil par le réseau
//! (Wake-on-LAN) : `wake` de `GET /api/stream/info` (les apps les gardent pour
//! réveiller l'ordinateur une fois éteint) et de `GET /api/status` (ligne
//! « Réveil par le réseau » de la page Système).
//!
//! Seules les cartes physiques en service comptent : Ethernet d'abord, Wi-Fi
//! ensuite, jamais les cartes virtuelles (Hyper-V, WSL, VPN, machines
//! virtuelles), Bluetooth ni la boucle locale. Pour chacune : son adresse MAC,
//! son adresse IPv4, l'adresse de diffusion de son sous-réseau (d'après le
//! préfixe), sa sorte et `armed`, ce que Windows en permet (la carte est-elle
//! autorisée à réveiller l'ordinateur, `powercfg /devicequery wake_armed`).
//!
//! Un fil dédié relit les cartes toutes les minutes (`GetIfTable2` pour les
//! cartes et leur nature, `GetAdaptersAddresses` pour leurs adresses) et
//! `powercfg` toutes les 5 minutes ; `info` et `status` ne lisent que la
//! mémoire (les apps demandent `info` toutes les 8 s). Hors Windows
//! (développement), une carte Ethernet simulée, à l'adresse du Mac.

// Hors Windows, la lecture des réponses de Windows n'est que testée.
#![cfg_attr(not(windows), allow(dead_code))]

use std::net::Ipv4Addr;
use std::sync::Mutex;
use std::time::Duration;

use serde::Serialize;

/// Cartes relues ce délai après la lecture précédente.
const CARDS_EVERY: Duration = Duration::from_secs(60);
/// Autorisations de réveil relues ce délai après la lecture précédente
/// (`powercfg` est un programme à lancer).
const ARMED_EVERY: Duration = Duration::from_secs(300);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    // Dans l'ordre où les apps les essaient.
    Ethernet,
    Wifi,
}

/// Une carte telle que les apps la gardent (`kybernow_core::wake::WakeTarget`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Card {
    /// `AA:BB:CC:DD:EE:FF`.
    pub mac: String,
    pub ipv4: Option<String>,
    /// Adresse de diffusion du sous-réseau de `ipv4`.
    pub broadcast: Option<String>,
    pub kind: Kind,
    /// Windows permet à la carte de réveiller l'ordinateur ; `None` : inconnu.
    pub armed: Option<bool>,
    /// Nom de la carte (« Intel(R) Ethernet Connection I219-V »).
    pub name: String,
}

static CARDS: Mutex<Vec<Card>> = Mutex::new(Vec::new());

/// Les cartes, telles que relues en dernier (vide avant la première lecture).
pub fn cards() -> Vec<Card> {
    CARDS.lock().unwrap().clone()
}

/// Lance la relecture de fond (au démarrage d'Admin).
pub fn start() {
    let spawned = std::thread::Builder::new().name("cartes-reseau".into()).spawn(|| {
        let mut armed: Option<Vec<String>> = None;
        let mut armed_at: Option<std::time::Instant> = None;
        loop {
            if armed_at.is_none_or(|t| t.elapsed() >= ARMED_EVERY) {
                armed = platform::armed();
                armed_at = Some(std::time::Instant::now());
            }
            let cards = platform::cards(armed.as_deref());
            let mut current = CARDS.lock().unwrap();
            if *current != cards {
                let list: Vec<String> = cards
                    .iter()
                    .map(|c| format!("{}{} {}{}", if c.name.is_empty() { String::new() } else { format!("{} ", c.name) }, c.mac, c.ipv4.as_deref().unwrap_or("sans IPv4"), match c.armed {
                        Some(true) => ", réveil permis",
                        Some(false) => ", réveil non permis",
                        None => "",
                    }))
                    .collect();
                tracing::info!("cartes réseau pour le réveil : {}", if list.is_empty() { "aucune".to_string() } else { list.join(" ; ") });
                *current = cards;
            }
            drop(current);
            std::thread::sleep(CARDS_EVERY);
        }
    });
    if let Err(e) = spawned {
        tracing::warn!("cartes réseau : relecture impossible ({e})");
    }
}

/// Adresse de diffusion de `ip/prefix` (192.168.1.20/24 → 192.168.1.255).
pub fn broadcast(ip: Ipv4Addr, prefix: u8) -> Ipv4Addr {
    let host_bits = 32 - u32::from(prefix.min(32));
    let mask = if host_bits == 32 { 0 } else { u32::MAX << host_bits };
    Ipv4Addr::from(u32::from(ip) | !mask)
}

fn format_mac(mac: &[u8]) -> String {
    mac.iter().map(|b| format!("{b:02X}")).collect::<Vec<_>>().join(":")
}

/// Une interface réseau telle que Windows la décrit.
#[derive(Debug, Clone, Default)]
struct Adapter {
    /// Identifiant de l'interface (LUID), pour rattacher ses adresses.
    luid: u64,
    /// Description de la carte (celle du Gestionnaire de périphériques).
    name: String,
    mac: Vec<u8>,
    /// `IF_TYPE_*` : 6 Ethernet, 71 Wi-Fi.
    if_type: u32,
    /// `NDIS_PHYSICAL_MEDIUM` ; `None` : inconnu.
    medium: Option<i32>,
    /// Un connecteur physique (`ConnectorPresent`, comme
    /// `Get-NetAdapter -Physical`) ; `None` : inconnu.
    connector: Option<bool>,
    /// Interface de filtre posée sur une carte (QoS, WFP…).
    filter: bool,
    up: bool,
    /// Adresses IPv4 et longueur de leur préfixe.
    ipv4: Vec<(Ipv4Addr, u8)>,
    /// Une passerelle par défaut : la carte du réseau local.
    gateway: bool,
}

const IF_TYPE_ETHERNET: u32 = 6;
const IF_TYPE_WIFI: u32 = 71;
const MEDIUM_WIRELESS_LAN: i32 = 1;
const MEDIUM_NATIVE_802_11: i32 = 9;
const MEDIUM_BLUETOOTH: i32 = 10;

/// Mots des cartes virtuelles que Windows décrit parfois comme physiques.
const VIRTUAL_NAMES: [&str; 16] = [
    "virtual", "hyper-v", "vethernet", "wsl", "vpn", "tap-", "wintun", "wireguard", "tailscale", "zerotier", "vmware",
    "virtualbox", "bluetooth", "loopback", "npcap", "rndis",
];

impl Adapter {
    fn mac6(&self) -> Option<[u8; 6]> {
        let mac: [u8; 6] = self.mac.as_slice().try_into().ok()?;
        // Ni adresse nulle ni adresse de groupe.
        (mac != [0; 6] && mac[0] & 1 == 0).then_some(mac)
    }

    fn kind(&self) -> Option<Kind> {
        match (self.if_type, self.medium) {
            (_, Some(MEDIUM_BLUETOOTH)) => None,
            (IF_TYPE_WIFI, _) | (IF_TYPE_ETHERNET, Some(MEDIUM_NATIVE_802_11 | MEDIUM_WIRELESS_LAN)) => Some(Kind::Wifi),
            (IF_TYPE_ETHERNET, _) => Some(Kind::Ethernet),
            _ => None,
        }
    }

    /// Carte physique en service, Ethernet ou Wi-Fi.
    fn usable(&self) -> bool {
        let name = self.name.to_lowercase();
        self.up
            && !self.filter
            && self.connector != Some(false)
            && self.kind().is_some()
            && self.mac6().is_some()
            && !VIRTUAL_NAMES.iter().any(|w| name.contains(w))
    }

    /// Première adresse IPv4 utilisable (pas celles que Windows se donne
    /// faute de réseau, 169.254.x.x).
    fn address(&self) -> Option<(Ipv4Addr, u8)> {
        self.ipv4.iter().copied().find(|(a, _)| !a.is_link_local() && !a.is_loopback() && !a.is_unspecified())
    }
}

/// Lettres et chiffres d'un nom de périphérique, en minuscules : `powercfg`
/// écrit dans la page de code de la console, qui abîme les lettres hors ASCII
/// (« ® »), et compare ainsi un nom à lui-même.
fn normalized(name: &str) -> String {
    name.chars().filter(char::is_ascii_alphanumeric).map(|c| c.to_ascii_lowercase()).collect()
}

/// Périphériques de la sortie de `powercfg /devicequery wake_armed`, un par
/// ligne (« NONE », ou sa traduction, quand il n'y en a aucun : il ne
/// ressemble à aucune carte).
fn parse_armed(output: &str) -> Vec<String> {
    output.lines().map(normalized).filter(|l| !l.is_empty()).collect()
}

/// Les cartes à donner aux apps, dans l'ordre où les essayer : Ethernet avant
/// Wi-Fi, la carte de la passerelle par défaut d'abord, une adresse MAC une
/// seule fois. `armed` : les périphériques autorisés à réveiller l'ordinateur
/// (`None` : inconnu).
fn select(adapters: &[Adapter], armed: Option<&[String]>) -> Vec<Card> {
    let mut usable: Vec<&Adapter> = adapters.iter().filter(|a| a.usable()).collect();
    // Une carte reliée à un commutateur virtuel (Hyper-V, externe) n'a pas
    // d'adresse : la carte virtuelle de l'ordinateur, qui porte la même adresse
    // MAC, a la sienne.
    let address = |a: &Adapter| {
        a.address().or_else(|| adapters.iter().filter(|o| o.mac == a.mac && o.up).find_map(Adapter::address))
    };
    let gateway = |a: &Adapter| a.gateway || adapters.iter().any(|o| o.mac == a.mac && o.up && o.gateway);
    usable.sort_by_key(|a| (a.kind(), !gateway(a), address(a).is_none()));
    let mut cards: Vec<Card> = Vec::new();
    for a in usable {
        let (Some(mac), Some(kind)) = (a.mac6(), a.kind()) else { continue };
        let mac = format_mac(&mac);
        if cards.iter().any(|c| c.mac == mac) {
            continue;
        }
        let ip = address(a);
        let name = a.name.trim().to_string();
        cards.push(Card {
            mac,
            ipv4: ip.map(|(ip, _)| ip.to_string()),
            broadcast: ip.map(|(ip, prefix)| broadcast(ip, prefix).to_string()),
            kind,
            armed: armed.map(|list| list.contains(&normalized(&name))),
            name,
        });
    }
    cards
}

#[cfg(windows)]
mod platform {
    use std::net::Ipv4Addr;
    use std::os::windows::process::CommandExt;

    use windows_sys::Win32::Foundation::{ERROR_BUFFER_OVERFLOW, NO_ERROR};
    use windows_sys::Win32::NetworkManagement::IpHelper::{
        FreeMibTable, GAA_FLAG_INCLUDE_GATEWAYS, GAA_FLAG_SKIP_ANYCAST, GAA_FLAG_SKIP_DNS_SERVER, GAA_FLAG_SKIP_FRIENDLY_NAME,
        GAA_FLAG_SKIP_MULTICAST, GetAdaptersAddresses, GetIfTable2, IP_ADAPTER_ADDRESSES_LH, MIB_IF_TABLE2,
    };
    use windows_sys::Win32::NetworkManagement::Ndis::IfOperStatusUp;
    use windows_sys::Win32::Networking::WinSock::{AF_INET, SOCKADDR_IN};

    use super::{Adapter, Card, parse_armed, select};

    const CREATE_NO_WINDOW: u32 = 0x0800_0000;

    pub fn cards(armed: Option<&[String]>) -> Vec<Card> {
        let mut adapters = interfaces();
        attach_addresses(&mut adapters);
        select(&adapters, armed)
    }

    /// Les périphériques autorisés à réveiller l'ordinateur ; `None` si
    /// `powercfg` ne répond pas.
    pub fn armed() -> Option<Vec<String>> {
        let root = std::env::var_os("SystemRoot").map(std::path::PathBuf::from).unwrap_or_else(|| r"C:\Windows".into());
        let out = std::process::Command::new(root.join("System32").join("powercfg.exe"))
            .args(["/devicequery", "wake_armed"])
            .creation_flags(CREATE_NO_WINDOW)
            .output();
        match out {
            Ok(o) if o.status.success() => Some(parse_armed(&String::from_utf8_lossy(&o.stdout))),
            Ok(o) => {
                tracing::info!("powercfg /devicequery wake_armed : {}", o.status);
                None
            }
            Err(e) => {
                tracing::info!("powercfg introuvable : {e}");
                None
            }
        }
    }

    fn wide(s: &[u16]) -> String {
        let len = s.iter().position(|&c| c == 0).unwrap_or(s.len());
        String::from_utf16_lossy(&s[..len])
    }

    /// Toutes les interfaces, cartes reliées à un commutateur virtuel comprises
    /// (`GetAdaptersAddresses` ne les voit pas : elles n'ont pas de pile IP).
    fn interfaces() -> Vec<Adapter> {
        let mut table: *mut MIB_IF_TABLE2 = std::ptr::null_mut();
        if unsafe { GetIfTable2(&mut table) } != NO_ERROR || table.is_null() {
            return Vec::new();
        }
        let rows = unsafe { std::slice::from_raw_parts((*table).Table.as_ptr(), (*table).NumEntries as usize) };
        let adapters = rows
            .iter()
            .map(|r| {
                let flags = r.InterfaceAndOperStatusFlags._bitfield;
                let len = (r.PhysicalAddressLength as usize).min(r.PhysicalAddress.len());
                Adapter {
                    luid: unsafe { r.InterfaceLuid.Value },
                    name: wide(&r.Description),
                    mac: r.PhysicalAddress[..len].to_vec(),
                    if_type: r.Type,
                    medium: Some(r.PhysicalMediumType),
                    // HardwareInterface : bit 0, FilterInterface : bit 1, ConnectorPresent : bit 2.
                    connector: Some(flags & 0b100 != 0),
                    filter: flags & 0b10 != 0,
                    up: r.OperStatus == IfOperStatusUp,
                    ..Adapter::default()
                }
            })
            .collect();
        unsafe { FreeMibTable(table as *const _) };
        adapters
    }

    /// Les adresses IPv4 (et leur préfixe) et la passerelle de chaque carte.
    fn attach_addresses(adapters: &mut Vec<Adapter>) {
        let flags = GAA_FLAG_SKIP_ANYCAST | GAA_FLAG_SKIP_MULTICAST | GAA_FLAG_SKIP_DNS_SERVER | GAA_FLAG_SKIP_FRIENDLY_NAME | GAA_FLAG_INCLUDE_GATEWAYS;
        // Tampon aligné pour IP_ADAPTER_ADDRESSES_LH ; agrandi si Windows le demande.
        let mut size: u32 = 16 * 1024;
        let mut buf: Vec<u64> = Vec::new();
        let mut status = ERROR_BUFFER_OVERFLOW;
        for _ in 0..3 {
            buf = vec![0u64; (size as usize).div_ceil(8)];
            status = unsafe { GetAdaptersAddresses(u32::from(AF_INET), flags, std::ptr::null(), buf.as_mut_ptr().cast(), &mut size) };
            if status != ERROR_BUFFER_OVERFLOW {
                break;
            }
        }
        if status != NO_ERROR {
            return;
        }
        let mut p = buf.as_ptr() as *const IP_ADAPTER_ADDRESSES_LH;
        while !p.is_null() {
            let a = unsafe { &*p };
            let mut ipv4 = Vec::new();
            let mut u = a.FirstUnicastAddress;
            while !u.is_null() {
                let addr = unsafe { &*u };
                let sa = addr.Address.lpSockaddr;
                if !sa.is_null() && unsafe { (*sa).sa_family } == AF_INET {
                    let sin = unsafe { &*(sa as *const SOCKADDR_IN) };
                    let ip = Ipv4Addr::from(u32::from_be(unsafe { sin.sin_addr.S_un.S_addr }));
                    ipv4.push((ip, addr.OnLinkPrefixLength));
                }
                u = addr.Next;
            }
            let luid = unsafe { a.Luid.Value };
            let gateway = !a.FirstGatewayAddress.is_null();
            match adapters.iter_mut().find(|x| x.luid == luid) {
                Some(x) => {
                    x.ipv4 = ipv4;
                    x.gateway = gateway;
                }
                // Absente de GetIfTable2 (ne devrait pas arriver) : prise telle quelle.
                None => {
                    let len = (a.PhysicalAddressLength as usize).min(a.PhysicalAddress.len());
                    let name = if a.Description.is_null() {
                        String::new()
                    } else {
                        unsafe { String::from_utf16_lossy(std::slice::from_raw_parts(a.Description, (0..).take_while(|&i| *a.Description.add(i) != 0).count())) }
                    };
                    adapters.push(Adapter {
                        luid,
                        name,
                        mac: a.PhysicalAddress[..len].to_vec(),
                        if_type: a.IfType,
                        up: a.OperStatus == IfOperStatusUp,
                        ipv4,
                        gateway,
                        ..Adapter::default()
                    });
                }
            }
            p = a.Next;
        }
    }
}

/// Hors Windows (Mac de développement) : une carte Ethernet simulée, à
/// l'adresse de l'ordinateur, que Windows laisserait réveiller l'ordinateur.
#[cfg(not(windows))]
mod platform {
    use std::net::Ipv4Addr;

    use super::{Card, Kind, broadcast};

    /// Adresse administrée localement : aucune carte ne la porte.
    const SIMULATED_MAC: &str = "02:4B:4E:00:00:01";

    pub fn cards(_armed: Option<&[String]>) -> Vec<Card> {
        let ip = crate::tls::lan_ip().and_then(|a| a.parse::<Ipv4Addr>().ok());
        vec![Card {
            mac: SIMULATED_MAC.into(),
            ipv4: ip.map(|a| a.to_string()),
            broadcast: ip.map(|a| broadcast(a, 24).to_string()),
            kind: Kind::Ethernet,
            armed: Some(true),
            name: String::new(),
        }]
    }

    pub fn armed() -> Option<Vec<String>> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn adapter(name: &str, mac: [u8; 6], if_type: u32, medium: i32, ip: Option<(&str, u8)>) -> Adapter {
        Adapter {
            luid: u64::from(mac[5]),
            name: name.into(),
            mac: mac.to_vec(),
            if_type,
            medium: Some(medium),
            connector: Some(true),
            filter: false,
            up: true,
            ipv4: ip.map(|(a, p)| vec![(a.parse().unwrap(), p)]).unwrap_or_default(),
            gateway: ip.is_some(),
        }
    }

    const ETH: [u8; 6] = [0x3C, 0x7C, 0x3F, 0x12, 0xAB, 0xCD];
    const WIFI: [u8; 6] = [0xA4, 0xB1, 0xC1, 0x00, 0x11, 0x22];

    #[test]
    fn diffusion_du_sous_reseau() {
        assert_eq!(broadcast("192.168.1.20".parse().unwrap(), 24), Ipv4Addr::new(192, 168, 1, 255));
        assert_eq!(broadcast("10.0.37.4".parse().unwrap(), 16), Ipv4Addr::new(10, 0, 255, 255));
        assert_eq!(broadcast("172.16.5.9".parse().unwrap(), 20), Ipv4Addr::new(172, 16, 15, 255));
        assert_eq!(broadcast("192.168.1.20".parse().unwrap(), 32), Ipv4Addr::new(192, 168, 1, 20));
        assert_eq!(broadcast("192.168.1.20".parse().unwrap(), 0), Ipv4Addr::BROADCAST);
    }

    #[test]
    fn ethernet_puis_wifi_sans_cartes_virtuelles() {
        let mut hyperv = adapter("Hyper-V Virtual Ethernet Adapter", [0x00, 0x15, 0x5D, 0x01, 0x02, 0x03], 6, 0, Some(("172.24.80.1", 20)));
        hyperv.connector = Some(false);
        let mut filter = adapter("Intel(R) Ethernet Connection (7) I219-V-WFP Native MAC Layer LightWeight Filter-0000", ETH, 6, 14, None);
        filter.filter = true;
        let mut down = adapter("Realtek PCIe 2.5GbE Family Controller", [0x10, 0x20, 0x30, 0x40, 0x50, 0x60], 6, 14, None);
        down.up = false;
        let adapters = vec![
            adapter("Intel(R) Wi-Fi 6 AX201 160MHz", WIFI, 71, 9, Some(("192.168.1.21", 24))),
            hyperv,
            adapter("Bluetooth Device (Personal Area Network)", [0xA4, 0xB1, 0xC1, 0x00, 0x11, 0x26], 6, 10, None),
            adapter("TAP-Windows Adapter V9", [0x00, 0xFF, 0x11, 0x22, 0x33, 0x44], 6, 0, Some(("10.8.0.2", 24))),
            adapter("Software Loopback Interface 1", [0; 6], 24, 0, Some(("127.0.0.1", 8))),
            filter,
            down,
            adapter("Intel(R) Ethernet Connection (7) I219-V", ETH, 6, 14, Some(("192.168.1.20", 24))),
        ];
        let armed = parse_armed("HID Keyboard Device\r\nIntel(R) Ethernet Connection (7) I219-V\r\nHID-compliant mouse\r\n");
        let cards = select(&adapters, Some(&armed));
        assert_eq!(
            cards,
            vec![
                Card {
                    mac: "3C:7C:3F:12:AB:CD".into(),
                    ipv4: Some("192.168.1.20".into()),
                    broadcast: Some("192.168.1.255".into()),
                    kind: Kind::Ethernet,
                    armed: Some(true),
                    name: "Intel(R) Ethernet Connection (7) I219-V".into(),
                },
                Card {
                    mac: "A4:B1:C1:00:11:22".into(),
                    ipv4: Some("192.168.1.21".into()),
                    broadcast: Some("192.168.1.255".into()),
                    kind: Kind::Wifi,
                    armed: Some(false),
                    name: "Intel(R) Wi-Fi 6 AX201 160MHz".into(),
                },
            ]
        );
        // `powercfg` muet : autorisation inconnue.
        assert!(select(&adapters, None).iter().all(|c| c.armed.is_none()));
        // Le JSON des apps.
        assert_eq!(
            serde_json::to_value(&cards[1]).unwrap(),
            serde_json::json!({ "mac": "A4:B1:C1:00:11:22", "ipv4": "192.168.1.21", "broadcast": "192.168.1.255", "kind": "wifi", "armed": false, "name": "Intel(R) Wi-Fi 6 AX201 160MHz" })
        );
    }

    #[test]
    fn carte_reliee_a_un_commutateur_virtuel() {
        // La carte physique sans adresse, la carte virtuelle de l'ordinateur avec
        // la même adresse MAC et l'adresse du réseau local.
        let mut vswitch = adapter("Hyper-V Virtual Ethernet Adapter #2", ETH, 6, 0, Some(("192.168.1.20", 24)));
        vswitch.connector = Some(false);
        let physical = adapter("Intel(R) Ethernet Connection (7) I219-V", ETH, 6, 14, None);
        let cards = select(&[vswitch, physical], None);
        assert_eq!(cards.len(), 1);
        assert_eq!(cards[0].name, "Intel(R) Ethernet Connection (7) I219-V");
        assert_eq!((cards[0].ipv4.as_deref(), cards[0].broadcast.as_deref()), (Some("192.168.1.20"), Some("192.168.1.255")));
    }

    #[test]
    fn adresse_que_windows_se_donne_ecartee() {
        // 169.254.x.x : pas de réseau sur cette carte (câble branché sur rien).
        let lonely = adapter("Killer E2600 Gigabit Ethernet Controller", ETH, 6, 14, Some(("169.254.12.7", 16)));
        let cards = select(&[lonely], Some(&[]));
        assert_eq!((cards[0].ipv4.as_ref(), cards[0].broadcast.as_ref(), cards[0].armed), (None, None, Some(false)));
    }

    #[test]
    fn la_carte_de_la_passerelle_d_abord() {
        let mut second = adapter("Realtek PCIe 2.5GbE Family Controller", [0x10, 0x20, 0x30, 0x40, 0x50, 0x60], 6, 14, Some(("10.0.0.5", 24)));
        second.gateway = false;
        let first = adapter("Intel(R) Ethernet Controller I225-V", ETH, 6, 14, Some(("192.168.1.20", 24)));
        let cards = select(&[second, first], None);
        assert_eq!(cards.iter().map(|c| c.name.as_str()).collect::<Vec<_>>(), ["Intel(R) Ethernet Controller I225-V", "Realtek PCIe 2.5GbE Family Controller"]);
    }

    #[test]
    fn sortie_de_powercfg() {
        // Page de code de la console : « ® » abîmé, comparé sans lui.
        let armed = parse_armed("Intel\u{FFFD} Ethernet Connection I219-V\r\n\r\nNONE\r\n");
        assert_eq!(armed, vec!["intelethernetconnectioni219v".to_string(), "none".to_string()]);
        assert!(armed.contains(&normalized("Intel® Ethernet Connection I219-V")));
        assert!(!armed.contains(&normalized("Intel® Ethernet Connection I219-V #2")));
        assert!(parse_armed("").is_empty());
    }
}
