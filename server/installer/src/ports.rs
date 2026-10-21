//! Choix des ports du stream et de la console.
//!
//! Un port est tiré au hasard dans une plage peu utilisée, puis vérifié :
//! hors des plages que Windows réserve (Hyper-V, WSL, Docker… ; IPv4 comme
//! IPv6), hors des plages d'autres logiciels courants (`RESERVED_RANGES`), et
//! libre en TCP comme en UDP, en IPv4 et en IPv6 (Kyber écoute sur les deux).
//! « Libre » vaut au moment de l'installation : rien ne garantit qu'un autre
//! programme ne prendra pas le port plus tard, d'où les plages connues.

use std::net::{Ipv4Addr, Ipv6Addr, SocketAddr, TcpListener, UdpSocket};

use rand::Rng;

use crate::i18n::t;

/// Plage de tirage : sous la plage dynamique de Windows (49152+), au-dessus des
/// ports classiques. Les ports de Sunshine/Moonlight (47984–48010) sont hors plage.
pub const RANDOM_RANGE: std::ops::RangeInclusive<u16> = 20000..=44999;
pub const KYBER_DEFAULT_PORT: u16 = 8080;

/// Plage de ports que d'autres logiciels prennent d'eux-mêmes, au démarrage
/// ou en cours de partie : libre à l'installation, elle peut ne plus l'être au
/// démarrage suivant. Jamais tirée au hasard, refusée si on la saisit.
pub struct ReservedRange {
    pub first: u16,
    pub last: u16,
    /// Nom du logiciel, tel qu'il apparaît dans le message de refus.
    pub owner: &'static str,
}

/// Plages connues de `RANDOM_RANGE` à laisser tranquilles, une entrée par
/// logiciel avec la raison.
///
/// Les autres ports courants de jeux sont déjà hors de `RANDOM_RANGE` (Discord
/// 50000–65535, Battle.net 1119 et 6881–6999, Sunshine/Moonlight 47984–48010),
/// et Windows déclare lui-même ce que Hyper-V, WSL et Docker réservent
/// (`excluded_ranges`) : rien à lister pour eux.
pub const RESERVED_RANGES: &[ReservedRange] = &[
    // Steam, souvent installé sur ces ordinateurs (KyberNow en emprunte les
    // haut-parleurs virtuels) : trafic des jeux (UDP 27000–27100), téléchargements
    // et serveurs de jeu (27015–27050), Remote Play et Steam Link (27031, 27036,
    // 27037). Les jeux Source et leurs serveurs dédiés prennent aussi 27015.
    ReservedRange { first: 27000, last: 27100, owner: "Steam" },
];

/// Plage réservée qui contient `port`, s'il y en a une.
pub fn reserved_by(port: u16) -> Option<&'static ReservedRange> {
    RESERVED_RANGES.iter().find(|r| (r.first..=r.last).contains(&port))
}

/// Plages que Windows réserve (`netsh int ipv4|ipv6 show excludedportrange`),
/// pour TCP et UDP : Windows tient une liste par famille d'adresses et par
/// protocole, et Kyber écoute en IPv4 comme en IPv6.
pub fn excluded_ranges() -> Vec<(u16, u16)> {
    if !cfg!(windows) {
        return Vec::new();
    }
    let mut ranges = Vec::new();
    for family in ["ipv4", "ipv6"] {
        for proto in ["tcp", "udp"] {
            let arg = format!("protocol={proto}");
            let Ok(out) = crate::sys::run("netsh", &["int", family, "show", "excludedportrange", &arg]) else {
                continue;
            };
            ranges.extend(parse_excluded(&String::from_utf8_lossy(&out.stdout)));
        }
    }
    ranges
}

/// Lit la sortie de `netsh int ipv4|ipv6 show excludedportrange` : une plage
/// par ligne, « début fin » suivi d'une `*` si elle est réservée à la main. Les
/// en-têtes, traduits selon la langue de Windows, n'ont pas deux nombres.
fn parse_excluded(output: &str) -> Vec<(u16, u16)> {
    let mut ranges = Vec::new();
    for line in output.lines() {
        let nums: Vec<u16> = line.split_whitespace().filter_map(|t| t.parse().ok()).collect();
        if let [start, end, ..] = nums[..] {
            if start <= end {
                ranges.push((start, end));
            }
        }
    }
    ranges
}

fn bindable(port: u16) -> bool {
    use std::io::ErrorKind::AddrInUse;
    let v4 = SocketAddr::from((Ipv4Addr::UNSPECIFIED, port));
    let v6 = SocketAddr::from((Ipv6Addr::UNSPECIFIED, port));
    // Chaque test ferme son socket avant le suivant (sinon IPv4 et IPv6
    // peuvent se gêner sur les systèmes en double pile).
    if TcpListener::bind(v4).is_err() || UdpSocket::bind(v4).is_err() {
        return false;
    }
    // IPv6 absent de l'ordinateur : seul un « déjà utilisé » compte.
    let tcp6 = !matches!(TcpListener::bind(v6), Err(e) if e.kind() == AddrInUse);
    let udp6 = !matches!(UdpSocket::bind(v6), Err(e) if e.kind() == AddrInUse);
    tcp6 && udp6
}

pub fn is_free(port: u16, excluded: &[(u16, u16)]) -> bool {
    !excluded.iter().any(|(a, b)| (*a..=*b).contains(&port)) && bindable(port)
}

/// Tire un port libre au hasard, différent de ceux de `avoid`, hors des plages
/// de `RESERVED_RANGES`.
pub fn pick_random(excluded: &[(u16, u16)], avoid: &[u16]) -> u16 {
    let mut rng = rand::rng();
    pick_with(|| rng.random_range(RANDOM_RANGE), |port| is_free(port, excluded), avoid)
}

/// Cœur du tirage, avec le hasard (`draw`) et la vérification d'un port libre
/// (`free`) en paramètres pour pouvoir le tester sans dépendre de la machine.
fn pick_with(mut draw: impl FnMut() -> u16, free: impl Fn(u16) -> bool, avoid: &[u16]) -> u16 {
    for _ in 0..500 {
        let port = draw();
        if !avoid.contains(&port) && reserved_by(port).is_none() && free(port) {
            return port;
        }
    }
    // Plage entièrement occupée : très improbable, on laisse l'utilisateur corriger.
    *RANDOM_RANGE.start()
}

/// Vérifie un port saisi à la main. Renvoie un message lisible en cas de refus.
pub fn validate(port: u16, other: u16, excluded: &[(u16, u16)]) -> Option<String> {
    if port < 1024 {
        return Some(t("ports.range", &[]));
    }
    if port == other {
        return Some(t("ports.samePort", &[]));
    }
    if excluded.iter().any(|(a, b)| (*a..=*b).contains(&port)) {
        return Some(t("ports.reservedByWindows", &[("port", &port)]));
    }
    if let Some(r) = reserved_by(port) {
        return Some(t("ports.reservedBy", &[("port", &port), ("owner", &r.owner), ("first", &r.first), ("last", &r.last)]));
    }
    if !bindable(port) {
        let by = owner(port).map(|p| t("ports.by", &[("process", &p)])).unwrap_or_default();
        return Some(t("ports.inUse", &[("port", &port), ("by", &by)]));
    }
    None
}

/// Programme qui écoute sur un port TCP, si Windows le révèle.
#[cfg(windows)]
pub fn owner(port: u16) -> Option<String> {
    use windows_sys::Win32::Foundation::CloseHandle;
    use windows_sys::Win32::NetworkManagement::IpHelper::{
        GetExtendedTcpTable, MIB_TCPTABLE_OWNER_PID, TCP_TABLE_OWNER_PID_LISTENER,
    };
    use windows_sys::Win32::Networking::WinSock::AF_INET;
    use windows_sys::Win32::System::Threading::{
        OpenProcess, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION, QueryFullProcessImageNameW,
    };

    let pid = unsafe {
        let mut size: u32 = 0;
        GetExtendedTcpTable(std::ptr::null_mut(), &mut size, 0, AF_INET as u32, TCP_TABLE_OWNER_PID_LISTENER, 0);
        if size == 0 {
            return None;
        }
        let mut buf = vec![0u8; size as usize + 64];
        if GetExtendedTcpTable(buf.as_mut_ptr().cast(), &mut size, 0, AF_INET as u32, TCP_TABLE_OWNER_PID_LISTENER, 0) != 0 {
            return None;
        }
        let table = &*(buf.as_ptr() as *const MIB_TCPTABLE_OWNER_PID);
        let rows = std::slice::from_raw_parts(table.table.as_ptr(), table.dwNumEntries as usize);
        rows.iter()
            .find(|r| (((r.dwLocalPort & 0xFF) << 8) | ((r.dwLocalPort >> 8) & 0xFF)) as u16 == port)
            .map(|r| r.dwOwningPid)?
    };
    if pid == 4 {
        return Some(t("ports.windowsHttp", &[]));
    }
    unsafe {
        let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if handle.is_null() {
            return Some(t("ports.process", &[("pid", &pid)]));
        }
        let mut buf = [0u16; 1024];
        let mut len = buf.len() as u32;
        let ok = QueryFullProcessImageNameW(handle, PROCESS_NAME_WIN32, buf.as_mut_ptr(), &mut len);
        CloseHandle(handle);
        if ok == 0 {
            return Some(t("ports.process", &[("pid", &pid)]));
        }
        let path = String::from_utf16_lossy(&buf[..len as usize]);
        Some(path.rsplit('\\').next().unwrap_or(&path).to_string())
    }
}

#[cfg(not(windows))]
pub fn owner(_port: u16) -> Option<String> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    // Sorties de `netsh int ipv4|ipv6 show excludedportrange protocol=tcp`
    // (Windows en anglais, puis en français : seuls les en-têtes changent).
    const NETSH_IPV4_EN: &str = "
Protocol tcp Port Exclusion Ranges

Start Port    End Port
----------    --------
      1723        1723
      5357        5357
     50000       50059     *

* - Administered port exclusions.
";
    const NETSH_IPV6_FR: &str = "
Plages d’exclusion de ports du protocole tcp

Port de départ    Port de fin
--------------    -----------
          2869          2869
         49770         49869
         50000         50059     *

* - Exclusions de ports administrées.
";

    #[test]
    fn random_port_is_in_range_and_free() {
        let p = pick_random(&[], &[]);
        assert!(RANDOM_RANGE.contains(&p));
        assert!(reserved_by(p).is_none());
        assert!(is_free(p, &[]));
    }

    #[test]
    fn validation_messages() {
        assert!(validate(80, 30000, &[]).is_some());
        assert!(validate(30000, 30000, &[]).is_some());
        assert!(validate(30001, 30000, &[(30000, 30100)]).unwrap().contains("réservé"));
        let held = TcpListener::bind("0.0.0.0:0").unwrap();
        let port = held.local_addr().unwrap().port();
        assert!(validate(port, 1, &[]).unwrap().contains("déjà utilisé"));
    }

    #[test]
    fn reserved_ranges_are_well_formed() {
        for (i, r) in RESERVED_RANGES.iter().enumerate() {
            assert!(r.first <= r.last, "{} : plage à l'envers", r.owner);
            assert!(!r.owner.is_empty());
            // Une plage hors de RANDOM_RANGE ne sert à rien : celles-là sont dans la doc.
            assert!(
                RANDOM_RANGE.contains(&r.first) || RANDOM_RANGE.contains(&r.last),
                "{} : hors de la plage de tirage",
                r.owner
            );
            for other in &RESERVED_RANGES[i + 1..] {
                assert!(r.last < other.first || other.last < r.first, "{} et {} se chevauchent", r.owner, other.owner);
            }
        }
    }

    #[test]
    fn steam_range_is_reserved_edges_included() {
        for port in [27000, 27015, 27031, 27036, 27050, 27100] {
            assert_eq!(reserved_by(port).map(|r| r.owner), Some("Steam"), "port {port}");
        }
        for port in [26999, 27101, 20000, 44999] {
            assert!(reserved_by(port).is_none(), "port {port}");
        }
    }

    #[test]
    fn random_draw_skips_reserved_ranges() {
        // Les premiers tirages tombent tous chez Steam : ils sont écartés, même
        // si le port est libre, et le premier tirage hors plage est pris.
        let mut draws = [27000, 27015, 27050, 27100, 31337, 31338].into_iter();
        let port = pick_with(|| draws.next().unwrap(), |_| true, &[]);
        assert_eq!(port, 31337);
    }

    #[test]
    fn random_draw_still_honours_avoid_and_free() {
        let mut draws = [31337, 31338, 31339].into_iter();
        let port = pick_with(|| draws.next().unwrap(), |p| p != 31338, &[31337]);
        assert_eq!(port, 31339);
    }

    #[test]
    fn random_draw_gives_up_on_the_start_of_the_range() {
        // Tout est écarté : on retombe sur le début de la plage, hors de toute plage réservée.
        let port = pick_with(|| 27015, |_| true, &[]);
        assert_eq!(port, *RANDOM_RANGE.start());
        assert!(reserved_by(port).is_none());
    }

    #[test]
    fn typed_port_in_a_reserved_range_is_refused() {
        for port in [27000, 27015, 27100] {
            let message = validate(port, 30000, &[]).expect("port de Steam refusé");
            assert!(message.contains("Steam") && message.contains("27000–27100"), "{message}");
        }
        // Un port juste à côté n'est pas refusé pour cette raison.
        for port in [26999, 27101] {
            assert!(validate(port, 30000, &[]).is_none_or(|m| !m.contains("Steam")), "port {port}");
        }
        // La réservation de Windows passe avant, avec son propre message.
        assert!(validate(27015, 30000, &[(27010, 27020)]).unwrap().contains("Windows"));
    }

    #[test]
    fn netsh_ipv4_output_is_parsed() {
        assert_eq!(parse_excluded(NETSH_IPV4_EN), [(1723, 1723), (5357, 5357), (50000, 50059)]);
    }

    #[test]
    fn netsh_ipv6_output_is_parsed_in_any_language() {
        assert_eq!(parse_excluded(NETSH_IPV6_FR), [(2869, 2869), (49770, 49869), (50000, 50059)]);
    }

    #[test]
    fn netsh_output_without_ranges_or_with_garbage_gives_nothing() {
        assert!(parse_excluded("").is_empty());
        assert!(parse_excluded("Protocol udp Port Exclusion Ranges\n\nStart Port    End Port\n----------    --------\n").is_empty());
        // Plage à l'envers ou hors des ports valides : ignorée.
        assert!(parse_excluded("  5000  4000\n  70000  70010\n").is_empty());
    }

    #[test]
    fn excluded_ranges_from_both_families_all_apply() {
        // Ce que `excluded_ranges` assemble : les plages d'IPv4 et d'IPv6 comptent chacune.
        let mut excluded = parse_excluded(NETSH_IPV4_EN);
        excluded.extend(parse_excluded(NETSH_IPV6_FR));
        assert!(!is_free(2869, &excluded), "plage vue seulement en IPv6");
        assert!(!is_free(5357, &excluded), "plage vue seulement en IPv4");
        assert!(validate(49800, 30000, &excluded).unwrap().contains("Windows"));
    }
}
