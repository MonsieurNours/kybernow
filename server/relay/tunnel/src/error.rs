//! Les erreurs du tunnel, communes au tunnel des apps et à l'agent de la Station.

use std::fmt;
use std::io;

use crate::dial::DialError;
use crate::msg::ErrorCode;

/// Pourquoi le tunnel ne sert pas.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TunnelError {
    /// Le relais ne répond pas (« Le relais ne répond pas. »).
    Unreachable(String),
    /// Le relais refuse (trop de connexions, banni, version…).
    Refused(ErrorCode, Option<String>),
    /// La Station n'est pas connectée au relais (éteinte, en veille, ou rien
    /// n'y demande le relais).
    StationOffline,
    /// La Station refuse ce flux UDP (client pas autorisé à joindre Kyber).
    FlowRefused(String),
    /// Délai dépassé.
    Timeout,
    /// Tunnel fermé.
    Closed,
    Io(String),
}

impl fmt::Display for TunnelError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TunnelError::Unreachable(why) => write!(f, "relais injoignable : {why}"),
            TunnelError::Refused(code, _) => write!(f, "refusé par le relais ({})", code.as_str()),
            TunnelError::StationOffline => f.write_str("la Station n'est pas connectée au relais"),
            TunnelError::FlowRefused(why) => write!(f, "flux refusé par la Station : {why}"),
            TunnelError::Timeout => f.write_str("délai dépassé"),
            TunnelError::Closed => f.write_str("tunnel fermé"),
            TunnelError::Io(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for TunnelError {}

impl From<DialError> for TunnelError {
    fn from(e: DialError) -> Self {
        match e {
            DialError::Unreachable(why) => TunnelError::Unreachable(why),
            DialError::Refused(code, m) => TunnelError::Refused(code, m),
        }
    }
}

impl From<TunnelError> for io::Error {
    fn from(e: TunnelError) -> Self {
        let kind = match e {
            TunnelError::StationOffline | TunnelError::FlowRefused(_) => io::ErrorKind::ConnectionRefused,
            TunnelError::Timeout => io::ErrorKind::TimedOut,
            _ => io::ErrorKind::NotConnected,
        };
        io::Error::new(kind, e.to_string())
    }
}

