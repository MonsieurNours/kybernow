//! Où joindre une console : son adresse et les ports de la lecture à
//! distance. Sur le réseau (ou par un VPN), l'adresse de la console et ses
//! vrais ports ([`Endpoint::direct`]) ; par le relais, la boucle locale de
//! l'appareil et un port du tunnel pour chacun ([`super::remote`],
//! docs/relais.md, « Consoles PlayStation »), avec des paquets plus petits
//! (MTU de Takion à 1 200 octets, ce que porte un datagramme du tunnel).

use std::net::{Ipv4Addr, SocketAddrV4};

use serde::{Deserialize, Serialize};

use super::discovery::{Model, PORT_PS4, PORT_PS5};

/// Port de l'association, de la demande de session et du canal de contrôle
/// (TCP), et de la recherche de l'association (UDP).
pub const PORT_RP: u16 = 9295;
/// Port de Takion (UDP).
pub const PORT_TAKION: u16 = 9296;
/// Ports TCP de la PS4 (dialogue de contrôle, `host-request-port`).
pub const PORT_PS4_REQUEST: u16 = 997;
/// MTU de Takion annoncée à la console sur le réseau (valeur de secours de chiaki-ng).
pub const MTU_LAN: u32 = 1454;
/// MTU par le relais : un paquet tient dans un datagramme du tunnel.
pub const MTU_RELAY: u32 = 1200;

/// Les ports, un par service de la console.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Ports {
    /// Recherche et réveil d'une PS5 (UDP 9302).
    pub ps5: u16,
    /// Recherche et réveil d'une PS4 (UDP 987).
    pub ps4: u16,
    /// Association, session, canal de contrôle (TCP 9295) et recherche de
    /// l'association (UDP 9295, même numéro).
    pub rp: u16,
    /// Takion (UDP 9296).
    pub takion: u16,
}

impl Ports {
    /// Les vrais ports, sur le réseau.
    pub const DIRECT: Ports = Ports { ps5: PORT_PS5, ps4: PORT_PS4, rp: PORT_RP, takion: PORT_TAKION };

    /// Le port de recherche et de réveil d'un modèle.
    pub fn search(&self, model: Model) -> u16 {
        match model {
            Model::Ps5 => self.ps5,
            Model::Ps4 => self.ps4,
        }
    }
}

impl Default for Ports {
    fn default() -> Self {
        Ports::DIRECT
    }
}

/// Une console à joindre : adresse, ports, et si l'on passe par le relais.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Endpoint {
    pub ip: Ipv4Addr,
    #[serde(default)]
    pub ports: Ports,
    /// Par le relais (MTU réduite, puce « Par le relais »).
    #[serde(default)]
    pub relayed: bool,
}

impl Endpoint {
    /// La console à son adresse, sur le réseau ou par un VPN.
    pub fn direct(ip: Ipv4Addr) -> Self {
        Endpoint { ip, ports: Ports::DIRECT, relayed: false }
    }

    /// La MTU de Takion à annoncer à la console.
    pub fn mtu(&self) -> u32 {
        if self.relayed { MTU_RELAY } else { MTU_LAN }
    }

    pub fn search(&self, model: Model) -> SocketAddrV4 {
        SocketAddrV4::new(self.ip, self.ports.search(model))
    }

    pub fn rp(&self) -> SocketAddrV4 {
        SocketAddrV4::new(self.ip, self.ports.rp)
    }

    pub fn takion(&self) -> SocketAddrV4 {
        SocketAddrV4::new(self.ip, self.ports.takion)
    }

    /// L'adresse telle que les ponts la passent : une adresse IPv4 (sur le
    /// réseau), ou ce JSON (par le relais).
    pub fn parse(text: &str) -> Option<Endpoint> {
        let text = text.trim();
        if text.starts_with('{') {
            serde_json::from_str(text).ok()
        } else {
            text.parse().ok().map(Endpoint::direct)
        }
    }

    /// Le texte que [`Endpoint::parse`] relit : l'adresse seule sur le réseau.
    pub fn to_text(&self) -> String {
        if !self.relayed && self.ports == Ports::DIRECT {
            self.ip.to_string()
        } else {
            serde_json::to_string(self).unwrap_or_default()
        }
    }
}

impl From<Ipv4Addr> for Endpoint {
    fn from(ip: Ipv4Addr) -> Self {
        Endpoint::direct(ip)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sur_le_reseau_rien_ne_change() {
        let e = Endpoint::direct(Ipv4Addr::new(192, 168, 1, 110));
        assert_eq!(e.search(Model::Ps5).to_string(), "192.168.1.110:9302");
        assert_eq!(e.search(Model::Ps4).to_string(), "192.168.1.110:987");
        assert_eq!(e.rp().to_string(), "192.168.1.110:9295");
        assert_eq!(e.takion().to_string(), "192.168.1.110:9296");
        assert_eq!(e.mtu(), 1454);
        assert_eq!(e.to_text(), "192.168.1.110");
        assert_eq!(Endpoint::parse("192.168.1.110"), Some(e));
        assert_eq!(Endpoint::parse("pas une adresse"), None);
    }

    #[test]
    fn par_le_relais_les_ports_du_tunnel() {
        let e = Endpoint { ip: Ipv4Addr::LOCALHOST, ports: Ports { ps5: 50001, ps4: 50002, rp: 50003, takion: 50004 }, relayed: true };
        assert_eq!(e.mtu(), 1200);
        assert_eq!(e.takion().to_string(), "127.0.0.1:50004");
        let text = e.to_text();
        assert!(text.starts_with('{'), "{text}");
        assert_eq!(Endpoint::parse(&text), Some(e));
    }
}
