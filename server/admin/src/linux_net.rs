//! Station Linux : les cartes réseau pour le réveil par le réseau
//! (`wake.rs`), comme sous Windows et sur Mac.
//!
//! - Les cartes : `/sys/class/net/<carte>`. Seules comptent celles qui
//!   tiennent à un vrai périphérique (`device`), de type Ethernet (`type` 1),
//!   en service (`operstate` « up ») : ni boucle locale, ni ponts, ni cartes
//!   de conteneurs, de machines virtuelles ou de VPN (aucune n'a de `device`),
//!   ni Bluetooth ni modem (`DEVTYPE` de `uevent`). Wi-Fi : un dossier
//!   `wireless` ou `phy80211`, ou `DEVTYPE=wlan`.
//! - L'adresse matérielle : `address` si elle est celle de la carte
//!   (`addr_assign_type` 0) ; sinon (adresse tirée au hasard par
//!   NetworkManager) l'adresse permanente de `ethtool -P`, la seule qu'un
//!   paquet de réveil atteint.
//! - Les adresses IPv4 et leur préfixe : `ip -4 -o addr show` ; la carte de
//!   la route par défaut : `/proc/net/route`.
//! - `armed` : le réveil par « paquet magique » est allumé sur la carte
//!   (`Wake-on` de `ethtool <carte>` contient `g`). Sans `ethtool`, ou si la
//!   carte ne dit rien (Wi-Fi), inconnu ; une carte que le noyau empêche de
//!   réveiller l'ordinateur (`device/power/wakeup` « disabled ») : non.
//! - La vitesse de la prise : `speed` (Mbit/s) en Ethernet, `tx bitrate` de
//!   `iw dev <carte> link` en Wi-Fi.
//! - Le nom affiché : celui de la puce dans la base PCI (`pci.ids`), suivi
//!   du nom de l'interface.

#![cfg_attr(not(target_os = "linux"), allow(dead_code))]

use std::net::Ipv4Addr;
use std::path::Path;

use crate::wake::{Card, Kind, broadcast};

/// Une interface telle que `/sys/class/net` la décrit.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Iface {
    pub name: String,
    /// `address`, en majuscules (`AA:BB:CC:DD:EE:FF`).
    pub address: String,
    /// `addr_assign_type` 0 : l'adresse gravée dans la carte.
    pub permanent_address: bool,
    pub kind: Option<Kind>,
    /// Rattachée à un périphérique (`device`).
    pub physical: bool,
    pub up: bool,
    /// `speed`, en Mbit/s ; `None` : inconnue (carte sans lien, Wi-Fi).
    pub speed_mbps: Option<u32>,
    /// `device/power/wakeup` : `Some(false)` si le noyau l'interdit.
    pub wakeup_allowed: Option<bool>,
    /// Identifiants PCI (fabricant, puce), pour le nom.
    pub pci: Option<(u16, u16)>,
}

fn read(dir: &Path, file: &str) -> Option<String> {
    std::fs::read_to_string(dir.join(file)).ok().map(|s| s.trim().to_string())
}

/// `0x10de` → 0x10DE.
pub fn hex_id(text: &str) -> Option<u16> {
    u16::from_str_radix(text.trim().trim_start_matches("0x"), 16).ok()
}

/// Lit une interface de `/sys/class/net` (`dir` : `/sys/class/net/enp3s0`).
pub fn read_iface(dir: &Path) -> Option<Iface> {
    let name = dir.file_name()?.to_string_lossy().into_owned();
    let uevent = read(dir, "uevent").unwrap_or_default();
    let devtype = uevent.lines().find_map(|l| l.strip_prefix("DEVTYPE=")).unwrap_or("").to_string();
    let wireless = dir.join("wireless").exists() || dir.join("phy80211").exists() || devtype == "wlan";
    let ethernet_type = read(dir, "type").as_deref() == Some("1");
    let kind = if !ethernet_type || dir.join("bridge").exists() {
        None
    } else if wireless {
        Some(Kind::Wifi)
    } else if devtype.is_empty() {
        Some(Kind::Ethernet)
    } else {
        // bluetooth, wwan, bond, vlan, macvlan…
        None
    };
    let device = dir.join("device");
    let pci = read(&device, "vendor").and_then(|v| hex_id(&v)).zip(read(&device, "device").and_then(|d| hex_id(&d)));
    Some(Iface {
        address: read(dir, "address").unwrap_or_default().to_ascii_uppercase(),
        permanent_address: read(dir, "addr_assign_type").as_deref() == Some("0"),
        kind,
        physical: device.exists(),
        up: read(dir, "operstate").as_deref() == Some("up"),
        speed_mbps: read(dir, "speed").and_then(|s| s.parse::<i64>().ok()).filter(|s| *s > 0 && *s < i64::from(u32::MAX)).map(|s| s as u32),
        wakeup_allowed: read(&device.join("power"), "wakeup").and_then(|w| match w.as_str() {
            "enabled" => Some(true),
            "disabled" => Some(false),
            _ => None,
        }),
        pci,
        name,
    })
}

/// Adresse MAC utilisable : six octets, ni nulle ni de groupe.
pub fn valid_mac(mac: &str) -> bool {
    let bytes: Vec<u8> = mac.split(':').filter_map(|b| u8::from_str_radix(b, 16).ok()).collect();
    bytes.len() == 6 && mac.len() == 17 && bytes != [0; 6] && bytes[0] & 1 == 0
}

/// Adresses IPv4 de `ip -4 -o addr show` : (interface, adresse, préfixe).
pub fn parse_ip_addr(text: &str) -> Vec<(String, Ipv4Addr, u8)> {
    let mut out = Vec::new();
    for line in text.lines() {
        let words: Vec<&str> = line.split_whitespace().collect();
        let Some(i) = words.iter().position(|w| *w == "inet") else { continue };
        let Some(name) = words.get(1) else { continue };
        // « eth0.10@eth0 » : l'interface elle-même.
        let name = name.split('@').next().unwrap_or(name).trim_end_matches(':');
        let Some((ip, prefix)) = words.get(i + 1).and_then(|a| a.split_once('/')) else { continue };
        if let (Ok(ip), Ok(prefix)) = (ip.parse::<Ipv4Addr>(), prefix.parse::<u8>()) {
            out.push((name.to_string(), ip, prefix.min(32)));
        }
    }
    out
}

/// L'interface de la route par défaut de `/proc/net/route` (la plus petite
/// métrique).
pub fn default_route(text: &str) -> Option<String> {
    text.lines()
        .skip(1)
        .filter_map(|l| {
            let w: Vec<&str> = l.split_whitespace().collect();
            // Iface Destination Gateway Flags RefCnt Use Metric Mask …
            (w.len() >= 8 && w[1] == "00000000" && w[7] == "00000000").then(|| (w[6].parse::<u32>().unwrap_or(u32::MAX), w[0].to_string()))
        })
        .min()
        .map(|(_, name)| name)
}

/// `Wake-on` de `ethtool <carte>` : le paquet magique (`g`) réveille-t-il
/// l'ordinateur ? `None` : la carte ne le dit pas.
pub fn parse_wake_on(text: &str) -> Option<bool> {
    let value = text.lines().map(str::trim).find_map(|l| l.strip_prefix("Wake-on:"))?.trim();
    (!value.is_empty()).then(|| value.contains('g'))
}

/// `Permanent address: 3c:7c:3f:12:ab:cd` de `ethtool -P <carte>`.
pub fn parse_permanent(text: &str) -> Option<String> {
    let mac = text.lines().find_map(|l| l.trim().strip_prefix("Permanent address:"))?.trim().to_ascii_uppercase();
    valid_mac(&mac).then_some(mac)
}

/// `tx bitrate: 866.7 MBit/s …` de `iw dev <carte> link`, en Mbit/s.
pub fn parse_iw_bitrate(text: &str) -> Option<u32> {
    let rest = text.lines().find_map(|l| l.trim().strip_prefix("tx bitrate:"))?;
    let rate: f64 = rest.split_whitespace().next()?.parse().ok()?;
    (rate >= 1.0).then(|| rate.round() as u32)
}

/// Ce qu'il faut savoir de chaque carte, lu ailleurs que dans `/sys`.
pub struct Extra<'a> {
    pub addresses: &'a [(String, Ipv4Addr, u8)],
    pub default_if: Option<&'a str>,
    /// Nom de la puce (base PCI).
    pub model: &'a dyn Fn(u16, u16) -> Option<String>,
    /// `Wake-on` de `ethtool`.
    pub wake_on: &'a dyn Fn(&str) -> Option<bool>,
    /// Adresse permanente (`ethtool -P`).
    pub permanent: &'a dyn Fn(&str) -> Option<String>,
    /// Débit du Wi-Fi.
    pub wifi_mbps: &'a dyn Fn(&str) -> Option<u32>,
}

/// Les cartes à donner aux apps : Ethernet avant Wi-Fi, la carte de la route
/// par défaut d'abord, une adresse MAC une seule fois.
pub fn select(ifaces: &[Iface], extra: &Extra) -> Vec<Card> {
    let usable_addr = |name: &str| {
        extra
            .addresses
            .iter()
            .find(|(n, ip, _)| n == name && !ip.is_link_local() && !ip.is_loopback() && !ip.is_unspecified())
            .map(|(_, ip, p)| (*ip, *p))
    };
    let mut usable: Vec<&Iface> = ifaces.iter().filter(|i| i.physical && i.up && i.kind.is_some()).collect();
    usable.sort_by_key(|i| (i.kind, extra.default_if != Some(i.name.as_str()), usable_addr(&i.name).is_none(), i.name.clone()));
    let mut cards: Vec<Card> = Vec::new();
    for i in usable {
        let Some(kind) = i.kind else { continue };
        let mac = if i.permanent_address { Some(i.address.clone()) } else { (extra.permanent)(&i.name).or_else(|| Some(i.address.clone())) };
        let Some(mac) = mac.filter(|m| valid_mac(m)) else { continue };
        if cards.iter().any(|c| c.mac == mac) {
            continue;
        }
        let ip = usable_addr(&i.name);
        let armed = match i.wakeup_allowed {
            Some(false) => Some(false),
            _ => (extra.wake_on)(&i.name),
        };
        let model = i.pci.and_then(|(v, d)| (extra.model)(v, d));
        cards.push(Card {
            mac,
            ipv4: ip.map(|(ip, _)| ip.to_string()),
            broadcast: ip.map(|(ip, prefix)| broadcast(ip, prefix).to_string()),
            kind,
            armed,
            name: match model {
                Some(m) => format!("{m} ({})", i.name),
                None => i.name.clone(),
            },
            link_mbps: match kind {
                Kind::Ethernet => i.speed_mbps,
                Kind::Wifi => (extra.wifi_mbps)(&i.name),
            },
        });
    }
    cards
}

#[cfg(target_os = "linux")]
fn run(program: &str, args: &[&str]) -> String {
    std::process::Command::new(program)
        .args(args)
        .env("LC_ALL", "C")
        .output()
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).into_owned())
        .unwrap_or_default()
}

/// Les cartes de cette Station.
#[cfg(target_os = "linux")]
pub fn cards() -> Vec<Card> {
    let ifaces: Vec<Iface> = std::fs::read_dir("/sys/class/net")
        .map(|d| d.flatten().filter_map(|e| read_iface(&e.path())).collect())
        .unwrap_or_default();
    let addresses = parse_ip_addr(&run("ip", &["-4", "-o", "addr", "show"]));
    let default_if = std::fs::read_to_string("/proc/net/route").ok().and_then(|t| default_route(&t));
    let model = |v: u16, d: u16| crate::linux_gpu::pci_name(v, d);
    let wake_on = |name: &str| parse_wake_on(&run("ethtool", &[name]));
    let permanent = |name: &str| parse_permanent(&run("ethtool", &["-P", name]));
    let wifi_mbps = |name: &str| parse_iw_bitrate(&run("iw", &["dev", name, "link"]));
    select(
        &ifaces,
        &Extra { addresses: &addresses, default_if: default_if.as_deref(), model: &model, wake_on: &wake_on, permanent: &permanent, wifi_mbps: &wifi_mbps },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sysfs(root: &Path, name: &str, files: &[(&str, &str)], dirs: &[&str]) -> std::path::PathBuf {
        let dir = root.join(name);
        std::fs::create_dir_all(&dir).unwrap();
        for d in dirs {
            std::fs::create_dir_all(dir.join(d)).unwrap();
        }
        for (f, v) in files {
            let path = dir.join(f);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, format!("{v}\n")).unwrap();
        }
        dir
    }

    #[test]
    fn reads_sysfs_interfaces() {
        let root = std::env::temp_dir().join(format!("kn-net-{}", rand::random::<u64>()));
        let eth = sysfs(
            &root,
            "enp3s0",
            &[
                ("address", "3c:7c:3f:12:ab:cd"),
                ("addr_assign_type", "0"),
                ("type", "1"),
                ("operstate", "up"),
                ("speed", "2500"),
                ("uevent", "INTERFACE=enp3s0\nIFINDEX=2"),
                ("device/vendor", "0x8086"),
                ("device/device", "0x15f3"),
                ("device/power/wakeup", "enabled"),
            ],
            &[],
        );
        let i = read_iface(&eth).unwrap();
        assert_eq!(i.kind, Some(Kind::Ethernet));
        assert!(i.physical && i.up && i.permanent_address);
        assert_eq!((i.address.as_str(), i.speed_mbps, i.wakeup_allowed, i.pci), ("3C:7C:3F:12:AB:CD", Some(2500), Some(true), Some((0x8086, 0x15F3))));

        let wifi = sysfs(&root, "wlp2s0", &[("address", "aa:bb:cc:00:11:22"), ("addr_assign_type", "3"), ("type", "1"), ("operstate", "up"), ("uevent", "DEVTYPE=wlan"), ("device/vendor", "0x8086")], &["phy80211"]);
        let w = read_iface(&wifi).unwrap();
        assert_eq!((w.kind, w.permanent_address, w.speed_mbps), (Some(Kind::Wifi), false, None));

        // Sans lien : `speed` vaut -1.
        let down = sysfs(&root, "enp4s0", &[("address", "10:20:30:40:50:60"), ("type", "1"), ("operstate", "down"), ("speed", "-1")], &["device"]);
        let d = read_iface(&down).unwrap();
        assert!(!d.up && d.speed_mbps.is_none());

        // Pont, conteneur, Bluetooth, boucle locale.
        let bridge = sysfs(&root, "virbr0", &[("address", "52:54:00:00:00:01"), ("type", "1"), ("operstate", "up")], &["bridge"]);
        assert_eq!(read_iface(&bridge).unwrap().kind, None);
        let veth = sysfs(&root, "veth1234", &[("address", "02:42:ac:11:00:02"), ("type", "1"), ("operstate", "up")], &[]);
        assert!(!read_iface(&veth).unwrap().physical);
        let bt = sysfs(&root, "bnep0", &[("address", "a4:b1:c1:00:11:26"), ("type", "1"), ("operstate", "up"), ("uevent", "DEVTYPE=bluetooth")], &["device"]);
        assert_eq!(read_iface(&bt).unwrap().kind, None);
        let lo = sysfs(&root, "lo", &[("address", "00:00:00:00:00:00"), ("type", "772"), ("operstate", "unknown")], &[]);
        assert_eq!(read_iface(&lo).unwrap().kind, None);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn reads_command_outputs() {
        let ip = "1: lo    inet 127.0.0.1/8 scope host lo\\       valid_lft forever preferred_lft forever\n\
                  2: enp3s0    inet 192.168.1.20/24 brd 192.168.1.255 scope global dynamic noprefixroute enp3s0\\       valid_lft 85000sec\n\
                  3: wlp2s0    inet 192.168.1.21/24 brd 192.168.1.255 scope global wlp2s0\n\
                  5: eth0.10@eth0    inet 10.0.10.2/16 scope global eth0.10\n";
        let a = parse_ip_addr(ip);
        assert_eq!(a.len(), 4);
        assert_eq!(a[1], ("enp3s0".into(), Ipv4Addr::new(192, 168, 1, 20), 24));
        assert_eq!(a[3].0, "eth0.10");

        let route = "Iface\tDestination\tGateway \tFlags\tRefCnt\tUse\tMetric\tMask\t\tMTU\tWindow\tIRTT\n\
                     wlp2s0\t00000000\t0101A8C0\t0003\t0\t0\t600\t00000000\t0\t0\t0\n\
                     enp3s0\t00000000\t0101A8C0\t0003\t0\t0\t100\t00000000\t0\t0\t0\n\
                     enp3s0\t0001A8C0\t00000000\t0001\t0\t0\t100\t00FFFFFF\t0\t0\t0\n";
        assert_eq!(default_route(route).as_deref(), Some("enp3s0"));
        assert_eq!(default_route("Iface\tDestination\n"), None);

        let ethtool = "Settings for enp3s0:\n\tSupported ports: [ TP ]\n\tSupports Wake-on: pumbg\n\tWake-on: g\n\tLink detected: yes\n";
        assert_eq!(parse_wake_on(ethtool), Some(true));
        assert_eq!(parse_wake_on("\tSupports Wake-on: pumbg\n\tWake-on: d\n"), Some(false));
        assert_eq!(parse_wake_on("Settings for wlp2s0:\n\tLink detected: yes\n"), None);

        assert_eq!(parse_permanent("Permanent address: 3c:7c:3f:12:ab:cd\n").as_deref(), Some("3C:7C:3F:12:AB:CD"));
        assert_eq!(parse_permanent("Permanent address: 00:00:00:00:00:00\n"), None);

        let iw = "Connected to 11:22:33:44:55:66 (on wlp2s0)\n\tSSID: Maison\n\trx bitrate: 1200.9 MBit/s 80MHz HE-MCS 11\n\ttx bitrate: 864.8 MBit/s 80MHz HE-MCS 8\n";
        assert_eq!(parse_iw_bitrate(iw), Some(865));
        assert_eq!(parse_iw_bitrate("Not connected.\n"), None);
    }

    #[test]
    fn ethernet_first_with_its_hardware_address() {
        let iface = |name: &str, address: &str, kind: Kind, permanent: bool| Iface {
            name: name.into(),
            address: address.into(),
            permanent_address: permanent,
            kind: Some(kind),
            physical: true,
            up: true,
            speed_mbps: Some(1000),
            wakeup_allowed: None,
            pci: Some((0x8086, 0x15F3)),
        };
        let mut docker = iface("docker0", "02:42:AC:11:00:01", Kind::Ethernet, true);
        docker.physical = false;
        let mut forbidden = iface("enp4s0", "10:20:30:40:50:60", Kind::Ethernet, true);
        forbidden.wakeup_allowed = Some(false);
        let ifaces = vec![
            iface("wlp2s0", "AA:BB:CC:00:11:22", Kind::Wifi, false),
            docker,
            forbidden,
            iface("enp3s0", "3C:7C:3F:12:AB:CD", Kind::Ethernet, true),
        ];
        let addresses = parse_ip_addr("2: enp3s0 inet 192.168.1.20/24 scope global enp3s0\n3: wlp2s0 inet 192.168.1.21/24 scope global wlp2s0\n4: docker0 inet 172.17.0.1/16 scope global docker0\n");
        let model = |_: u16, _: u16| Some("Ethernet Controller I225-V".to_string());
        let wake_on = |name: &str| (name == "enp3s0").then_some(true);
        // Le Wi-Fi a une adresse tirée au hasard : l'adresse permanente.
        let permanent = |name: &str| (name == "wlp2s0").then(|| "A4:FC:14:00:11:22".to_string());
        let wifi = |_: &str| Some(866);
        let extra = Extra { addresses: &addresses, default_if: Some("enp3s0"), model: &model, wake_on: &wake_on, permanent: &permanent, wifi_mbps: &wifi };
        let cards = select(&ifaces, &extra);
        assert_eq!(cards.iter().map(|c| c.mac.as_str()).collect::<Vec<_>>(), ["3C:7C:3F:12:AB:CD", "10:20:30:40:50:60", "A4:FC:14:00:11:22"]);
        assert_eq!(cards[0].name, "Ethernet Controller I225-V (enp3s0)");
        assert_eq!((cards[0].ipv4.as_deref(), cards[0].broadcast.as_deref(), cards[0].armed, cards[0].link_mbps), (Some("192.168.1.20"), Some("192.168.1.255"), Some(true), Some(1000)));
        // Le noyau interdit le réveil à cette carte, sans adresse : en dernier des Ethernet.
        assert_eq!((cards[1].ipv4.as_ref(), cards[1].armed), (None, Some(false)));
        assert_eq!((cards[2].kind, cards[2].armed, cards[2].link_mbps), (Kind::Wifi, None, Some(866)));
    }
}
