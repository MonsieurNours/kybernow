//! Tunnel du relais de KyberNow (docs/relais.md) : ce que le relais, Admin et
//! le cœur des apps partagent.
//!
//! - [`msg`] et [`frame`] : les messages (JSON, versionnés) et leurs trames,
//!   les datagrammes `[flux][paquet]` ;
//! - [`id`] : identifiant de la Station (clé publique), du client (jeton),
//!   empreintes ;
//! - [`addr`] : l'adresse d'un relais, `hôte:port#sha256:…` ;
//! - [`tls`], [`quic`], [`transport`] : QUIC (`kn-ctl/1`, `kn-media/1`) et
//!   TLS sur TCP avec yamux (`kn-tcp/1`) sous une même forme, règle de
//!   taille, contrôle de congestion permissif ;
//! - [`client`] (fonction `client`) : le tunnel d'une app et ses points
//!   d'écoute sur la boucle locale ;
//! - [`station`] et [`identity`] (fonction `station`) : l'agent de la Station ;
//! - [`lookup`] : la réponse de `GET /v1/codes/{code}` et la signature de
//!   l'empreinte d'Admin.

pub mod addr;
pub mod congestion;
#[cfg_attr(not(all(feature = "client", feature = "station")), allow(dead_code))]
mod dial;
pub mod error;
pub mod frame;
pub mod id;
pub mod lookup;
pub mod msg;
pub mod quic;
pub mod tls;
pub mod transport;

#[cfg(feature = "client")]
pub mod client;
#[cfg(feature = "station")]
pub mod identity;
#[cfg(feature = "station")]
pub mod station;

pub use addr::{DEFAULT_PORT, DEFAULT_RELAY, RelayAddr};
pub use dial::{DialError, QUIC_DIAL_TIMEOUT, QUIC_RETRY_EVERY, backoff};
pub use error::TunnelError;
pub use msg::{Msg, Service, StreamHeader, StreamKind};
pub use transport::{RelayStream, Transport};

#[cfg(feature = "client")]
pub use client::{ClientConfig, Flow, LocalListener, Tunnel, TunnelStats};
#[cfg(feature = "station")]
pub use identity::StationIdentity;
#[cfg(feature = "station")]
pub use station::{Agent, AgentConfig, AgentState, FlowRequest, Incoming};

/// Version du protocole du tunnel (`hello.v`, `welcome.v`).
pub const PROTOCOL_VERSION: u32 = 1;
