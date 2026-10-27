//! Réglages QUIC du tunnel (docs/relais.md, « Connexions ») : MTU de départ
//! 1280, recherche jusqu'à 1472, coupure après 30 s ; `kn-ctl/1` en Cubic,
//! `kn-media/1` en contrôle permissif avec une file de datagrammes bornée.

use std::net::{Ipv4Addr, Ipv6Addr, SocketAddr};
use std::sync::Arc;
use std::time::Duration;

use quinn::{IdleTimeout, MtuDiscoveryConfig, TransportConfig, VarInt};

use crate::congestion::PermissiveFactory;

pub const ALPN_CTL: &[u8] = b"kn-ctl/1";
pub const ALPN_MEDIA: &[u8] = b"kn-media/1";
pub const ALPN_TCP: &[u8] = b"kn-tcp/1";

/// MTU de départ (charge UDP), de quoi passer un paquet de 1200 octets.
pub const INITIAL_MTU: u16 = 1280;
/// Plafond de la recherche de MTU.
pub const MTU_UPPER_BOUND: u16 = 1472;
/// Coupure sans nouvelles.
pub const IDLE_TIMEOUT: Duration = Duration::from_secs(30);
/// Signe de vie de la Station.
pub const STATION_KEEP_ALIVE: Duration = Duration::from_secs(10);
/// Signe de vie d'une app.
pub const CLIENT_KEEP_ALIVE: Duration = Duration::from_secs(5);
/// File d'envoi des datagrammes : au-delà, les plus anciens sont jetés.
pub const DATAGRAM_SEND_BUFFER: usize = 1024 * 1024;
/// File de réception des datagrammes.
pub const DATAGRAM_RECEIVE_BUFFER: usize = 4 * 1024 * 1024;

/// Les deux connexions QUIC d'un bout.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Ctl,
    Media,
}

fn base(keep_alive: Duration) -> TransportConfig {
    let mut t = TransportConfig::default();
    t.max_idle_timeout(Some(IdleTimeout::try_from(IDLE_TIMEOUT).expect("30 s tient dans un VarInt")));
    t.keep_alive_interval(Some(keep_alive));
    t.initial_mtu(INITIAL_MTU);
    let mut mtu = MtuDiscoveryConfig::default();
    mtu.upper_bound(MTU_UPPER_BOUND);
    t.mtu_discovery_config(Some(mtu));
    t
}

/// Réglages d'une connexion du côté qui se connecte (Station, app).
pub fn transport(kind: Kind, keep_alive: Duration) -> TransportConfig {
    let mut t = base(keep_alive);
    match kind {
        Kind::Ctl => {
            t.max_concurrent_bidi_streams(VarInt::from_u32(256));
            t.max_concurrent_uni_streams(VarInt::from_u32(0));
            t.datagram_receive_buffer_size(None);
            // Une image en WebSocket (navigateur, repli de kyclient) passe par
            // ces flux : de quoi tenir ~200 Mbit/s à 40 ms d'aller-retour.
            t.stream_receive_window(VarInt::from_u32(4 * 1024 * 1024));
            t.receive_window(VarInt::from_u32(16 * 1024 * 1024));
            t.send_window(16 * 1024 * 1024);
        }
        Kind::Media => {
            t.congestion_controller_factory(Arc::new(PermissiveFactory));
            t.max_concurrent_bidi_streams(VarInt::from_u32(0));
            // Les paquets de secours (règle de taille) : un flux par paquet.
            t.max_concurrent_uni_streams(VarInt::from_u32(256));
            t.datagram_receive_buffer_size(Some(DATAGRAM_RECEIVE_BUFFER));
            t.datagram_send_buffer_size(DATAGRAM_SEND_BUFFER);
        }
    }
    t
}

/// Configuration client quinn d'après une configuration rustls (TLS 1.3).
pub fn client_config(tls: rustls::ClientConfig, kind: Kind, keep_alive: Duration) -> Result<quinn::ClientConfig, String> {
    let crypto = quinn::crypto::rustls::QuicClientConfig::try_from(tls).map_err(|e| e.to_string())?;
    let mut cfg = quinn::ClientConfig::new(Arc::new(crypto));
    cfg.transport_config(Arc::new(transport(kind, keep_alive)));
    Ok(cfg)
}

/// Un point d'envoi QUIC pour joindre cette adresse (même famille d'adresses).
pub fn client_endpoint(target: SocketAddr) -> std::io::Result<quinn::Endpoint> {
    let local: SocketAddr = match target {
        SocketAddr::V4(_) => (Ipv4Addr::UNSPECIFIED, 0).into(),
        SocketAddr::V6(_) => (Ipv6Addr::UNSPECIFIED, 0).into(),
    };
    quinn::Endpoint::client(local)
}

#[cfg(feature = "server")]
pub use server::{alpn, incoming_config, peer_station_id, server_transport};

#[cfg(feature = "server")]
mod server {
    use super::*;
    use crate::congestion::{CcMode, SwitchableFactory};

    /// Réglages des connexions entrantes du relais : l'ALPN n'est connu
    /// qu'après la négociation, donc tout est permis (flux et datagrammes),
    /// et le contrôle de congestion bascule en permissif par `mode`.
    pub fn server_transport(mode: CcMode) -> TransportConfig {
        // Le relais ne fait pas de signe de vie lui-même : chaque bout en fait.
        let mut t = base(Duration::from_secs(15));
        t.congestion_controller_factory(Arc::new(SwitchableFactory { mode }));
        t.max_concurrent_bidi_streams(VarInt::from_u32(256));
        t.max_concurrent_uni_streams(VarInt::from_u32(256));
        t.datagram_receive_buffer_size(Some(DATAGRAM_RECEIVE_BUFFER));
        t.datagram_send_buffer_size(DATAGRAM_SEND_BUFFER);
        t.stream_receive_window(VarInt::from_u32(4 * 1024 * 1024));
        t.receive_window(VarInt::from_u32(16 * 1024 * 1024));
        t.send_window(16 * 1024 * 1024);
        t
    }

    /// L'ALPN négocié d'une connexion.
    pub fn alpn(conn: &quinn::Connection) -> Option<Vec<u8>> {
        conn.handshake_data()?.downcast::<quinn::crypto::rustls::HandshakeData>().ok()?.protocol
    }

    /// L'identifiant de Station du certificat client, s'il y en a un.
    pub fn peer_station_id(conn: &quinn::Connection) -> Option<String> {
        let certs = conn.peer_identity()?.downcast::<Vec<rustls::pki_types::CertificateDer<'static>>>().ok()?;
        crate::id::station_id_of_cert(certs.first()?)
    }

    /// La configuration d'une connexion entrante, avec son propre [`CcMode`].
    pub fn incoming_config(base: &quinn::ServerConfig) -> (quinn::ServerConfig, CcMode) {
        let mode = CcMode::new();
        let mut cfg = base.clone();
        cfg.transport = Arc::new(server_transport(mode.clone()));
        (cfg, mode)
    }
}
