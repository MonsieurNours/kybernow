//! L'adresse d'un relais : `hôte[:port][#sha256:…]` (docs/relais.md).
//! Sans port : 8443. Avec une empreinte : le certificat du relais est vérifié
//! par elle (relais installé chez soi, certificat signé par lui-même), et
//! son nom n'est pas vérifié.

use std::fmt;
use std::str::FromStr;

use crate::id::normalize_fingerprint;

/// Le relais public de KyberNow.
pub const DEFAULT_RELAY: &str = "share.kybernow.app:8443";
/// Le port du relais, en TCP et en UDP.
pub const DEFAULT_PORT: u16 = 8443;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RelayAddr {
    /// Nom (ou adresse IP, sans crochets).
    pub host: String,
    pub port: u16,
    /// Empreinte du certificat du relais (`sha256:aa:bb:…`), si elle est épinglée.
    pub pin: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BadRelayAddr(pub String);

impl fmt::Display for BadRelayAddr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "adresse de relais illisible : {}", self.0)
    }
}

impl std::error::Error for BadRelayAddr {}

impl RelayAddr {
    /// Le relais public.
    pub fn public() -> RelayAddr {
        DEFAULT_RELAY.parse().expect("adresse du relais public")
    }

    /// `hôte:port`, avec des crochets pour une adresse IPv6.
    pub fn authority(&self) -> String {
        if self.host.contains(':') { format!("[{}]:{}", self.host, self.port) } else { format!("{}:{}", self.host, self.port) }
    }
}

impl Default for RelayAddr {
    fn default() -> Self {
        RelayAddr::public()
    }
}

impl FromStr for RelayAddr {
    type Err = BadRelayAddr;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let bad = || BadRelayAddr(s.to_string());
        let s = s.trim();
        // Une adresse collée avec son schéma reste lisible.
        let s = s.strip_prefix("https://").unwrap_or(s).trim_end_matches('/');
        let (main, pin) = match s.split_once('#') {
            Some((m, p)) => (m, Some(normalize_fingerprint(p).ok_or_else(bad)?)),
            None => (s, None),
        };
        let (host, port) = if let Some(rest) = main.strip_prefix('[') {
            let (h, after) = rest.split_once(']').ok_or_else(bad)?;
            let port = match after.strip_prefix(':') {
                Some(p) => p.parse::<u16>().map_err(|_| bad())?,
                None if after.is_empty() => DEFAULT_PORT,
                None => return Err(bad()),
            };
            (h.to_string(), port)
        } else {
            match main.rsplit_once(':') {
                Some((h, p)) if !h.contains(':') => (h.to_string(), p.parse::<u16>().map_err(|_| bad())?),
                Some(_) => (main.to_string(), DEFAULT_PORT),
                None => (main.to_string(), DEFAULT_PORT),
            }
        };
        let host = host.to_ascii_lowercase();
        if host.is_empty() || port == 0 || host.chars().any(|c| c.is_whitespace() || c == '/' || c == '@') {
            return Err(bad());
        }
        Ok(RelayAddr { host, port, pin })
    }
}

impl fmt::Display for RelayAddr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.authority())?;
        if let Some(pin) = &self.pin {
            write!(f, "#{pin}")?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adresses() {
        let a: RelayAddr = "share.kybernow.app:8443".parse().unwrap();
        assert_eq!((a.host.as_str(), a.port, a.pin.as_deref()), ("share.kybernow.app", 8443, None));
        assert_eq!(a.to_string(), DEFAULT_RELAY);
        let b: RelayAddr = "Relais.Exemple.org".parse().unwrap();
        assert_eq!(b.authority(), "relais.exemple.org:8443");
        let fp = crate::id::fingerprint(b"x");
        let c: RelayAddr = format!("relais.exemple.org:9443#{fp}").parse().unwrap();
        assert_eq!(c.pin.as_deref(), Some(fp.as_str()));
        assert_eq!(c.to_string(), format!("relais.exemple.org:9443#{fp}"));
        let d: RelayAddr = "[2001:db8::1]:8443".parse().unwrap();
        assert_eq!(d.host, "2001:db8::1");
        assert_eq!(d.authority(), "[2001:db8::1]:8443");
        let e: RelayAddr = "2001:db8::2".parse().unwrap();
        assert_eq!(e.port, 8443);
        assert!("".parse::<RelayAddr>().is_err());
        assert!("relais:0".parse::<RelayAddr>().is_err());
        assert!("relais#sha256:12".parse::<RelayAddr>().is_err());
        assert_eq!(RelayAddr::default(), a);
    }
}
