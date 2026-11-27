//! L'adresse d'un relais, `hôte[:port][#sha256:…]` (docs/relais.md), lue et
//! montrée comme le fait le tunnel (`kybernow_tunnel::RelayAddr`, dont le
//! cœur vérifie qu'elle donne les mêmes formes) : sans port, 443, et une
//! adresse au port 443 s'écrit sans lui ; une empreinte épinglée est gardée
//! dans la forme normale, jamais dans ce qu'on montre.

/// Le relais public de KyberNow.
pub const DEFAULT_RELAY: &str = "share.kybernow.app";
/// Le port d'un relais dont l'adresse n'en donne pas.
pub const DEFAULT_PORT: u16 = 443;

/// Une adresse de relais lue.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelayParts {
    /// Nom en minuscules (ou adresse IP, sans crochets).
    pub host: String,
    pub port: u16,
    /// Empreinte épinglée, sous sa forme normale (`sha256:aa:bb:…`).
    pub pin: Option<String>,
}

impl RelayParts {
    fn bracketed_host(&self) -> String {
        if self.host.contains(':') { format!("[{}]", self.host) } else { self.host.clone() }
    }

    /// `hôte`, ou `hôte:port` hors du 443 : ce qu'on montre, et l'autorité
    /// des liens et des pages du relais.
    pub fn web_authority(&self) -> String {
        if self.port == DEFAULT_PORT { self.bracketed_host() } else { format!("{}:{}", self.bracketed_host(), self.port) }
    }

    /// La forme normale, `hôte[:port][#sha256:…]` : deux relais sont le même
    /// si leurs formes normales sont égales.
    pub fn normal(&self) -> String {
        match &self.pin {
            Some(pin) => format!("{}#{pin}", self.web_authority()),
            None => self.web_authority(),
        }
    }
}

/// Lit une adresse de relais (collée avec `https://` et `/` final aussi) ;
/// `None` si elle ne se lit pas.
pub fn parse(addr: &str) -> Option<RelayParts> {
    let s = addr.trim();
    let s = s.strip_prefix("https://").unwrap_or(s).trim_end_matches('/');
    let (main, pin) = match s.split_once('#') {
        Some((m, p)) => (m, Some(super::fingerprint::normalize(p)?)),
        None => (s, None),
    };
    let (host, port) = if let Some(rest) = main.strip_prefix('[') {
        let (h, after) = rest.split_once(']')?;
        let port = match after.strip_prefix(':') {
            Some(p) => p.parse::<u16>().ok()?,
            None if after.is_empty() => DEFAULT_PORT,
            None => return None,
        };
        (h, port)
    } else {
        match main.rsplit_once(':') {
            Some((h, p)) if !h.contains(':') => (h, p.parse::<u16>().ok()?),
            _ => (main, DEFAULT_PORT),
        }
    };
    let host = host.to_ascii_lowercase();
    if host.is_empty() || port == 0 || host.chars().any(|c| c.is_whitespace() || c == '/' || c == '@') {
        return None;
    }
    Some(RelayParts { host, port, pin })
}

/// La forme normale d'une adresse de relais ; `None` si elle ne se lit pas.
pub fn normalize(addr: &str) -> Option<String> {
    parse(addr).map(|r| r.normal())
}

/// Le relais tel qu'on le montre : `share.kybernow.app`,
/// `relais.exemple.org:9443`, sans empreinte ; une adresse illisible, telle
/// quelle sans son empreinte.
pub fn label(addr: &str) -> String {
    match parse(addr) {
        Some(r) => r.web_authority(),
        None => addr.split('#').next().unwrap_or_default().to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formes_du_relais() {
        assert_eq!(normalize("share.kybernow.app").as_deref(), Some(DEFAULT_RELAY));
        for s in ["share.kybernow.app:443", "https://share.kybernow.app/", " SHARE.kybernow.app "] {
            assert_eq!(normalize(s).as_deref(), Some(DEFAULT_RELAY), "{s}");
        }
        assert_eq!(normalize("share.kybernow.app:8443").as_deref(), Some("share.kybernow.app:8443"));
        assert_eq!(normalize("[2001:db8::1]:8443").as_deref(), Some("[2001:db8::1]:8443"));
        assert_eq!(normalize("2001:db8::2").as_deref(), Some("[2001:db8::2]"));
        let fp = format!("sha256:{}", ["ab"; 32].join(":"));
        assert_eq!(normalize(&format!("relais.exemple.org:9443#{}", fp.to_uppercase().replace("SHA256", "sha256"))), Some(format!("relais.exemple.org:9443#{fp}")));
        for bad in ["", "relais:0", "relais#sha256:12", "re lais", "a/b", "u@relais", "[::1]x", "relais:port"] {
            assert_eq!(normalize(bad), None, "{bad}");
        }
        assert_eq!(label(&format!("relais.exemple.org:443#{fp}")), "relais.exemple.org");
        assert_eq!(label("https://relais.exemple.org:9443/"), "relais.exemple.org:9443");
        assert_eq!(label("pas un relais#x"), "pas un relais");
        assert_eq!(label(""), "");
    }
}
