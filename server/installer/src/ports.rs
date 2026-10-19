//! Choix des ports du stream et de la console.
//!
//! Un port est tiré au hasard dans une plage peu utilisée, puis vérifié :
//! hors des plages que Windows réserve (Hyper-V, WSL, Docker…), et libre en
//! TCP comme en UDP, en IPv4 et en IPv6 (Kyber écoute sur les deux).

use std::net::{Ipv4Addr, Ipv6Addr, SocketAddr, TcpListener, UdpSocket};

use rand::Rng;

/// Plage de tirage : sous la plage dynamique de Windows (49152+), au-dessus des
/// ports classiques. Les ports de Sunshine/Moonlight (47984–48010) sont hors plage.
pub const RANDOM_RANGE: std::ops::RangeInclusive<u16> = 20000..=44999;
pub const KYBER_DEFAULT_PORT: u16 = 8080;

/// Plages réservées par Windows (`netsh int ipv4 show excludedportrange`).
pub fn excluded_ranges() -> Vec<(u16, u16)> {
    if !cfg!(windows) {
        return Vec::new();
    }
    let mut ranges = Vec::new();
    for proto in ["tcp", "udp"] {
        let arg = format!("protocol={proto}");
        let Ok(out) = crate::sys::run("netsh", &["int", "ipv4", "show", "excludedportrange", &arg]) else {
            continue;
        };
        for line in String::from_utf8_lossy(&out.stdout).lines() {
            let nums: Vec<u16> = line.split_whitespace().filter_map(|t| t.parse().ok()).collect();
            if let [start, end, ..] = nums[..] {
                if start <= end {
                    ranges.push((start, end));
                }
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

/// Tire un port libre au hasard, différent de ceux de `avoid`.
pub fn pick_random(excluded: &[(u16, u16)], avoid: &[u16]) -> u16 {
    let mut rng = rand::rng();
    for _ in 0..500 {
        let port = rng.random_range(RANDOM_RANGE);
        if !avoid.contains(&port) && is_free(port, excluded) {
            return port;
        }
    }
    // Plage entièrement occupée : très improbable, on laisse l'utilisateur corriger.
    *RANDOM_RANGE.start()
}

/// Vérifie un port saisi à la main. Renvoie un message lisible en cas de refus.
pub fn validate(port: u16, other: u16, excluded: &[(u16, u16)]) -> Option<String> {
    if port < 1024 {
        return Some("Choisissez un port entre 1024 et 65535.".into());
    }
    if port == other {
        return Some("Le stream et la console ont besoin de deux ports différents.".into());
    }
    if excluded.iter().any(|(a, b)| (*a..=*b).contains(&port)) {
        return Some(format!("Le port {port} est réservé par Windows."));
    }
    if !bindable(port) {
        let who = owner(port).map(|p| format!(" par {p}")).unwrap_or_default();
        return Some(format!("Le port {port} est déjà utilisé{who}."));
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
        return Some("Windows (service HTTP du système)".into());
    }
    unsafe {
        let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if handle.is_null() {
            return Some(format!("le processus {pid}"));
        }
        let mut buf = [0u16; 1024];
        let mut len = buf.len() as u32;
        let ok = QueryFullProcessImageNameW(handle, PROCESS_NAME_WIN32, buf.as_mut_ptr(), &mut len);
        CloseHandle(handle);
        if ok == 0 {
            return Some(format!("le processus {pid}"));
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

    #[test]
    fn random_port_is_in_range_and_free() {
        let p = pick_random(&[], &[]);
        assert!(RANDOM_RANGE.contains(&p));
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
}
