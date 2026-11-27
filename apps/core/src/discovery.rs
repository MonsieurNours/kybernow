//! Stations trouvées sur le réseau (annonce Bonjour d'Admin,
//! `_kybernow._tcp`) : ce qu'une annonce résolue donne à l'écran d'ajout,
//! lu d'une seule façon (WINAPP-53 ; avant, Windows écartait `kyber=0` et
//! l'IPv6 locale au lien, Linux et LG prenaient la première IPv4 et
//! préfixaient `sha256:` d'eux-mêmes). L'app garde sa bibliothèque mDNS et
//! donne à [`found`] ce qu'elle a résolu.

use std::net::IpAddr;

use serde::{Deserialize, Serialize};

/// Le service annoncé par Admin.
pub const SERVICE: &str = "_kybernow._tcp.local.";

/// Une Station trouvée.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Found {
    /// Le nom complet de l'annonce (stable : clé de la liste, retrait).
    pub id: String,
    /// Le nom de l'ordinateur (`name` de l'annonce, sinon celui de l'instance).
    pub name: String,
    /// L'adresse à joindre (IPv4 de préférence), sans crochets.
    pub host: String,
    /// Le port d'Admin.
    pub port: u16,
    /// L'empreinte d'Admin annoncée, sous la forme `sha256:aa:bb:…` quand
    /// elle se lit.
    pub fingerprint: Option<String>,
}

/// L'adresse à joindre parmi celles de l'annonce : une IPv4 hors de
/// 169.254/16, puis une IPv4, puis une IPv6 qui n'est pas locale au lien
/// (une IPv6 `fe80::` ne fait pas une adresse sans son interface).
fn best_address(addresses: &[IpAddr]) -> Option<IpAddr> {
    let rank = |a: &IpAddr| match a {
        IpAddr::V4(v) if v.is_unspecified() || v.is_loopback() => None,
        IpAddr::V4(v) if v.is_link_local() => Some(1),
        IpAddr::V4(_) => Some(0),
        IpAddr::V6(v) if v.is_unspecified() || v.is_loopback() || (v.segments()[0] & 0xffc0) == 0xfe80 => None,
        IpAddr::V6(_) => Some(2),
    };
    addresses.iter().filter_map(|a| rank(a).map(|r| (r, *a))).min_by_key(|(r, _)| *r).map(|(_, a)| a)
}

/// L'empreinte annoncée : sa forme normale si elle se lit (avec ou sans le
/// préfixe `sha256:`), sinon telle quelle ; vide : aucune.
fn fingerprint(fp: &str) -> Option<String> {
    let t = fp.trim();
    if t.is_empty() {
        return None;
    }
    let normal = kybernow_rules::fingerprint::normalize(t).or_else(|| kybernow_rules::fingerprint::normalize(&format!("sha256:{t}")));
    Some(normal.unwrap_or_else(|| t.to_string()))
}

/// Une annonce résolue : son nom complet, son port, ses adresses et ses
/// champs TXT (`txt("name")`, `txt("fp")`, `txt("kyber")`). `None` : pas
/// une Station à proposer (`kyber=0` : Admin sans Kyber ; aucune adresse
/// joignable ; port nul).
pub fn found(fullname: &str, port: u16, addresses: &[IpAddr], txt: impl Fn(&str) -> Option<String>) -> Option<Found> {
    if txt("kyber").as_deref().map(str::trim) == Some("0") || port == 0 {
        return None;
    }
    let host = best_address(addresses)?.to_string();
    let instance = fullname.strip_suffix(SERVICE).unwrap_or(fullname).trim_end_matches('.');
    let instance = if instance.is_empty() { "Station" } else { instance };
    let name = txt("name").map(|n| n.trim().to_string()).filter(|n| !n.is_empty()).unwrap_or_else(|| instance.to_string());
    Some(Found { id: fullname.to_string(), name, host, port, fingerprint: txt("fp").as_deref().and_then(fingerprint) })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn annonce_lue_d_une_seule_facon() {
        let hex = "ab".repeat(32);
        let normal = format!("sha256:{}", ["ab"; 32].join(":"));
        let txt = |name: Option<&'static str>, fp: Option<String>, kyber: Option<&'static str>| {
            move |k: &str| match k {
                "name" => name.map(str::to_string),
                "fp" => fp.clone(),
                "kyber" => kyber.map(str::to_string),
                _ => None,
            }
        };
        let v6: IpAddr = "fe80::1".parse().unwrap();
        let g6: IpAddr = "2001:db8::5".parse().unwrap();
        let ll4: IpAddr = "169.254.3.4".parse().unwrap();
        let v4: IpAddr = "192.168.1.20".parse().unwrap();
        let full = "PC-SALON._kybernow._tcp.local.";
        // IPv4 d'abord ; empreinte sans préfixe mise en forme ; nom de l'annonce.
        let f = found(full, 9443, &[v6, g6, ll4, v4], txt(Some("PC du salon"), Some(hex.clone()), None)).unwrap();
        assert_eq!((f.host.as_str(), f.port, f.name.as_str(), f.fingerprint.as_deref()), ("192.168.1.20", 9443, "PC du salon", Some(normal.as_str())));
        // Sans nom : celui de l'instance ; l'IPv6 locale au lien n'est jamais prise.
        let f = found(full, 9443, &[v6, g6], txt(None, Some(normal.to_uppercase().replace("SHA256", "sha256")), None)).unwrap();
        assert_eq!((f.name.as_str(), f.host.as_str(), f.fingerprint.as_deref()), ("PC-SALON", "2001:db8::5", Some(normal.as_str())));
        assert_eq!(found(full, 9443, &[v6], txt(None, None, None)), None);
        assert_eq!(found(full, 9443, &[ll4], txt(None, None, None)).map(|f| f.host), Some("169.254.3.4".into()));
        // Admin sans Kyber, port nul : pas proposée. Empreinte illisible : telle quelle.
        assert_eq!(found(full, 9443, &[v4], txt(None, None, Some("0"))), None);
        assert_eq!(found(full, 0, &[v4], txt(None, None, None)), None);
        assert_eq!(found(full, 9443, &[v4], txt(None, Some("abc".into()), Some("1"))).and_then(|f| f.fingerprint), Some("abc".into()));
        assert_eq!(found(full, 9443, &[v4], txt(None, Some(" ".into()), None)).and_then(|f| f.fingerprint), None);
    }
}
