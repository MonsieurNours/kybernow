//! Adresses d'Admin tapées, lues dans un lien ou montrées (TRANS-35,
//! TRANS-36) : une seule lecture, juste en IPv6, et une seule adresse
//! d'Admin à ouvrir.

use std::net::Ipv6Addr;

/// Port d'Admin d'une Station quand rien d'autre n'en donne un (ordinateur
/// d'un compte, invitation sans port, ligne de commande).
pub const ADMIN_PORT: u16 = 9443;

/// L'exemple de l'écran d'ajout (« Adresse et port d'Admin, par exemple
/// … »), le même dans toutes les apps.
pub const ADDRESS_EXAMPLE: &str = "192.168.1.20:9443";

/// Une IPv6, avec ou sans zone (`fe80::1%en0`).
fn is_ipv6(host: &str) -> bool {
    let (addr, zone) = match host.split_once('%') {
        Some((a, z)) => (a, Some(z)),
        None => (host, None),
    };
    zone.is_none_or(|z| !z.is_empty() && !z.contains(|c: char| c.is_whitespace() || c == '/')) && addr.parse::<Ipv6Addr>().is_ok()
}

/// Une adresse « hôte:port » tapée ou lue :
///
/// - un nom ou une IPv4, avec ou sans port (`pc.local:9443`, `192.168.1.20`) ;
/// - une IPv6 entre crochets, avec ou sans port (`[fe80::1]:9443`, `[::1]`) ;
/// - une IPv6 nue (`fe80::1`, `fe80::1%en0`) : toute l'adresse est l'hôte,
///   sans port (jamais l'hôte `fe80:` sur le port 1).
///
/// Sans port : `default_port`, ou refusée si `None`. Refusées aussi : un
/// hôte vide, un port nul ou illisible, une IPv4 ou un nom entre crochets,
/// une IPv6 mal écrite, des espaces, `/` ou `@` dans l'hôte. L'hôte est rendu
/// sans crochets.
pub fn parse_address(text: &str, default_port: Option<u16>) -> Option<(String, u16)> {
    let s = text.trim();
    let (host, port) = if let Some(rest) = s.strip_prefix('[') {
        let (h, after) = rest.split_once(']')?;
        if !is_ipv6(h) {
            return None;
        }
        let port = if after.is_empty() { None } else { Some(after.strip_prefix(':')?) };
        (h, port)
    } else if s.matches(':').count() > 1 {
        if !is_ipv6(s) {
            return None;
        }
        (s, None)
    } else {
        match s.split_once(':') {
            Some((h, p)) => (h, Some(p)),
            None => (s, None),
        }
    };
    if host.is_empty() || host.contains(|c: char| c.is_whitespace() || matches!(c, '/' | '@' | '[' | ']')) {
        return None;
    }
    let port = match port {
        Some(p) => p.parse::<u16>().ok().filter(|p| *p > 0)?,
        None => default_port.filter(|p| *p > 0)?,
    };
    Some((host.to_string(), port))
}

/// L'hôte sans ses crochets (`[::1]` → `::1`).
pub fn strip_brackets(host: &str) -> &str {
    let h = host.trim();
    h.strip_prefix('[').and_then(|h| h.strip_suffix(']')).unwrap_or(h)
}

/// L'hôte tel qu'il s'écrit dans une adresse web : une IPv6 entre crochets,
/// une seule fois (`::1` et `[::1]` → `[::1]`), le reste tel quel.
pub fn host_for_url(host: &str) -> String {
    let h = strip_brackets(host);
    if h.contains(':') { format!("[{h}]") } else { h.to_string() }
}

/// Adresse d'Admin à ouvrir dans le navigateur ou à mettre dans un QR code :
/// `https://hôte:port/`, crochets IPv6 compris.
pub fn admin_url(host: &str, port: u16) -> String {
    format!("https://{}:{port}/", host_for_url(host))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adresses_tapees() {
        let p = |s: &str| parse_address(s, None);
        assert_eq!(p("192.168.1.20:24811"), Some(("192.168.1.20".into(), 24811)));
        assert_eq!(p("pc.local:9443"), Some(("pc.local".into(), 9443)));
        assert_eq!(p("[fe80::1]:24811"), Some(("fe80::1".into(), 24811)));
        assert_eq!(p(" [2001:db8::5]:443 "), Some(("2001:db8::5".into(), 443)));
        assert_eq!(p("[fe80::1%en0]:9443"), Some(("fe80::1%en0".into(), 9443)));
        // Sans port, refusées sans port par défaut.
        assert_eq!(p("192.168.1.20"), None);
        assert_eq!(p("fe80::1"), None, "jamais l'hôte « fe80: » sur le port 1");
        assert_eq!(p("[fe80::1]"), None);
        // Mal écrites.
        for bad in [":24811", "[]:24811", "[192.168.1.20]:24811", "fe80::1:24811", "pc.local:0", "pc.local:port", "pc local:9443", "a/b:1", "u@pc:1", "[fe80::1]x", "[fe80::1"] {
            assert_eq!(p(bad), None, "{bad}");
        }
        // Avec un port par défaut.
        let d = |s: &str| parse_address(s, Some(ADMIN_PORT));
        assert_eq!(d("fe80::1"), Some(("fe80::1".into(), 9443)));
        assert_eq!(d("::1"), Some(("::1".into(), 9443)));
        assert_eq!(d("[::1]"), Some(("::1".into(), 9443)));
        assert_eq!(d("192.168.1.20"), Some(("192.168.1.20".into(), 9443)));
        assert_eq!(d("pc.local:37462"), Some(("pc.local".into(), 37462)));
        assert_eq!(parse_address("pc", Some(0)), None);
        assert_eq!(parse_address(ADDRESS_EXAMPLE, None), Some(("192.168.1.20".into(), ADMIN_PORT)));
    }

    #[test]
    fn adresse_d_admin() {
        assert_eq!(admin_url("192.168.1.20", 9443), "https://192.168.1.20:9443/");
        assert_eq!(admin_url("fe80::1", 9443), "https://[fe80::1]:9443/");
        assert_eq!(admin_url("[fe80::1]", 9443), "https://[fe80::1]:9443/", "crochets jamais doublés");
        assert_eq!(admin_url("pc.local", 37462), "https://pc.local:37462/");
        assert_eq!(strip_brackets(" [::1] "), "::1");
        assert_eq!(host_for_url("pc.local"), "pc.local");
    }
}
