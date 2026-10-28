//! Se connecter au relais (Station et app) : QUIC d'abord (`kn-ctl/1`, puis
//! `kn-media/1` après `welcome`), repli TLS sur TCP (`kn-tcp/1` et yamux)
//! si QUIC ne répond pas en 3 s (docs/relais.md, « Connexions »).

use std::fmt;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use rustls::RootCertStore;
use tokio::io::{AsyncWriteExt, ReadHalf};
use tokio::net::TcpStream;
use tokio::sync::mpsc;
use tokio::time::timeout;
use tokio_util::compat::TokioAsyncReadCompatExt;

use crate::addr::RelayAddr;
use crate::frame::{encode, read_msg, write_msg};
use crate::msg::{ErrorCode, ErrorMsg, Hello, Msg, Welcome};
use crate::quic::{self, ALPN_CTL, ALPN_MEDIA, ALPN_TCP, Kind};
use crate::tls::{self, ClientAuth};
use crate::transport::{AbortOnDrop, Conn, MediaReceiver, MediaSender, RelayStream, Transport, YamuxHandle, media_over_stream};

/// QUIC qui ne répond pas dans ce délai : repli TCP.
pub const QUIC_DIAL_TIMEOUT: Duration = Duration::from_secs(3);
/// Sur le repli TCP, QUIC est retenté à ce rythme (connexion au repos).
pub const QUIC_RETRY_EVERY: Duration = Duration::from_secs(5 * 60);
const TCP_DIAL_TIMEOUT: Duration = Duration::from_secs(5);
const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(10);

/// Pourquoi la connexion au relais n'a pas abouti.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DialError {
    /// Pas de réponse (nom introuvable, réseau, délai, certificat).
    Unreachable(String),
    /// Le relais a répondu par un refus.
    Refused(ErrorCode, Option<String>),
}

impl fmt::Display for DialError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DialError::Unreachable(why) => write!(f, "relais injoignable : {why}"),
            DialError::Refused(code, Some(m)) => write!(f, "refusé par le relais ({}) : {m}", code.as_str()),
            DialError::Refused(code, None) => write!(f, "refusé par le relais ({})", code.as_str()),
        }
    }
}

impl std::error::Error for DialError {}

fn unreachable(e: impl fmt::Display) -> DialError {
    DialError::Unreachable(e.to_string())
}

pub(crate) struct DialConfig {
    pub relay: RelayAddr,
    pub roots: Option<Arc<RootCertStore>>,
    pub auth: Option<ClientAuth>,
    pub keep_alive: Duration,
    pub hello: Hello,
    /// Ouvrir aussi la connexion des paquets.
    pub with_media: bool,
    pub force_tcp: bool,
}

/// Une connexion établie : flux, messages, paquets.
pub(crate) struct Session {
    pub conn: Conn,
    pub ctl_tx: mpsc::UnboundedSender<Msg>,
    pub ctl_rx: ReadHalf<RelayStream>,
    pub media: Option<(MediaSender, MediaReceiver)>,
    pub welcome: Welcome,
    pub transport: Transport,
    pub _endpoint: Option<quinn::Endpoint>,
    pub _writer: AbortOnDrop,
}

impl Session {
    pub fn close(&self, reason: &str) {
        self.conn.close(reason);
    }
}

async fn resolve(relay: &RelayAddr) -> Result<Vec<SocketAddr>, DialError> {
    let mut addrs: Vec<SocketAddr> = tokio::net::lookup_host(relay.host_port()).await.map_err(unreachable)?.collect();
    // IPv4 d'abord : un IPv6 cassé chez l'utilisateur est plus courant que l'inverse.
    addrs.sort_by_key(|a| a.is_ipv6());
    if addrs.is_empty() {
        return Err(DialError::Unreachable(format!("{} : aucune adresse", relay.host)));
    }
    Ok(addrs)
}

/// Se connecte au relais, QUIC d'abord puis repli TCP.
pub(crate) async fn dial(cfg: &DialConfig) -> Result<Session, DialError> {
    let addrs = resolve(&cfg.relay).await?;
    if !cfg.force_tcp {
        match dial_quic(cfg, addrs[0]).await {
            Ok(s) => return Ok(s),
            Err(e @ DialError::Refused(..)) => return Err(e),
            Err(e) => log::info!("relais {} : QUIC sans réponse ({e}), repli TCP", cfg.relay),
        }
    }
    let mut last = DialError::Unreachable("aucune adresse".into());
    for addr in addrs {
        match dial_tcp(cfg, addr).await {
            Ok(s) => return Ok(s),
            Err(e @ DialError::Refused(..)) => return Err(e),
            Err(e) => last = e,
        }
    }
    Err(last)
}

/// QUIC seulement (retour depuis le repli TCP).
pub(crate) async fn dial_quic_only(cfg: &DialConfig) -> Result<Session, DialError> {
    let addrs = resolve(&cfg.relay).await?;
    dial_quic(cfg, addrs[0]).await
}

async fn handshake(ctl: &mut RelayStream, hello: &Hello) -> Result<Welcome, DialError> {
    write_msg(ctl, &Msg::Hello(hello.clone())).await.map_err(unreachable)?;
    match timeout(HANDSHAKE_TIMEOUT, read_msg(ctl)).await {
        Err(_) => Err(DialError::Unreachable("pas de réponse à hello".into())),
        Ok(Err(e)) => Err(unreachable(e)),
        Ok(Ok(None)) => Err(DialError::Unreachable("connexion fermée avant welcome".into())),
        Ok(Ok(Some(Msg::Welcome(w)))) => Ok(w),
        Ok(Ok(Some(Msg::Error(ErrorMsg { code, message, .. })))) => Err(DialError::Refused(code, message)),
        Ok(Ok(Some(other))) => Err(DialError::Unreachable(format!("réponse inattendue : {}", other.kind()))),
    }
}

fn media_hello(hello: &Hello) -> Msg {
    Msg::Media { token: hello.token.clone() }
}

fn start_writer(ctl: RelayStream) -> (ReadHalf<RelayStream>, mpsc::UnboundedSender<Msg>, AbortOnDrop) {
    let (read, mut write) = tokio::io::split(ctl);
    let (tx, mut rx) = mpsc::unbounded_channel::<Msg>();
    let task = tokio::spawn(async move {
        while let Some(m) = rx.recv().await {
            if write_msg(&mut write, &m).await.is_err() {
                break;
            }
        }
        let _ = write.shutdown().await;
    });
    (read, tx, AbortOnDrop(task))
}

async fn dial_quic(cfg: &DialConfig, addr: SocketAddr) -> Result<Session, DialError> {
    let endpoint = quic::client_endpoint(addr).map_err(unreachable)?;
    let tls_ctl = tls::client_config(&cfg.relay, cfg.roots.clone(), ALPN_CTL, cfg.auth.as_ref()).map_err(unreachable)?;
    let ctl_cfg = quic::client_config(tls_ctl, Kind::Ctl, cfg.keep_alive).map_err(DialError::Unreachable)?;
    let connecting = endpoint.connect_with(ctl_cfg, addr, &cfg.relay.host).map_err(unreachable)?;
    let conn = match timeout(QUIC_DIAL_TIMEOUT, connecting).await {
        Err(_) => return Err(DialError::Unreachable("QUIC : pas de réponse en 3 s".into())),
        Ok(r) => r.map_err(unreachable)?,
    };
    let (send, recv) = conn.open_bi().await.map_err(unreachable)?;
    let mut ctl = RelayStream::quic(send, recv);
    let welcome = handshake(&mut ctl, &cfg.hello).await?;
    let media = if cfg.with_media {
        let tls_media = tls::client_config(&cfg.relay, cfg.roots.clone(), ALPN_MEDIA, cfg.auth.as_ref()).map_err(unreachable)?;
        let media_cfg = quic::client_config(tls_media, Kind::Media, cfg.keep_alive).map_err(DialError::Unreachable)?;
        let connecting = endpoint.connect_with(media_cfg, addr, &cfg.relay.host).map_err(unreachable)?;
        let m = match timeout(HANDSHAKE_TIMEOUT, connecting).await {
            Err(_) => return Err(DialError::Unreachable("kn-media/1 : pas de réponse".into())),
            Ok(r) => r.map_err(unreachable)?,
        };
        let mut uni = m.open_uni().await.map_err(unreachable)?;
        uni.write_all(&encode(&media_hello(&cfg.hello))).await.map_err(unreachable)?;
        let _ = uni.finish();
        Some((MediaSender::quic(m.clone()), MediaReceiver::quic(m)))
    } else {
        None
    };
    let (ctl_rx, ctl_tx, writer) = start_writer(ctl);
    Ok(Session {
        conn: Conn::Quic(conn),
        ctl_tx,
        ctl_rx,
        media,
        welcome,
        transport: Transport::Quic,
        _endpoint: Some(endpoint),
        _writer: writer,
    })
}

async fn dial_tcp(cfg: &DialConfig, addr: SocketAddr) -> Result<Session, DialError> {
    let tcp = match timeout(TCP_DIAL_TIMEOUT, TcpStream::connect(addr)).await {
        Err(_) => return Err(DialError::Unreachable("TCP : pas de réponse".into())),
        Ok(r) => r.map_err(unreachable)?,
    };
    let _ = tcp.set_nodelay(true);
    let tls_cfg = tls::client_config(&cfg.relay, cfg.roots.clone(), ALPN_TCP, cfg.auth.as_ref()).map_err(unreachable)?;
    let name = tls::server_name(&cfg.relay).map_err(unreachable)?;
    let connector = tokio_rustls::TlsConnector::from(Arc::new(tls_cfg));
    let tls = match timeout(HANDSHAKE_TIMEOUT, connector.connect(name, tcp)).await {
        Err(_) => return Err(DialError::Unreachable("TLS : pas de réponse".into())),
        Ok(r) => r.map_err(unreachable)?,
    };
    let conn = Conn::Yamux(YamuxHandle::start(tls.compat(), yamux::Mode::Client, addr));
    let mut ctl = conn.open().await.map_err(unreachable)?;
    let welcome = handshake(&mut ctl, &cfg.hello).await?;
    let media = if cfg.with_media {
        let mut m = conn.open().await.map_err(unreachable)?;
        write_msg(&mut m, &media_hello(&cfg.hello)).await.map_err(unreachable)?;
        Some(media_over_stream(m))
    } else {
        None
    };
    let (ctl_rx, ctl_tx, writer) = start_writer(ctl);
    Ok(Session { conn, ctl_tx, ctl_rx, media, welcome, transport: Transport::Tcp, _endpoint: None, _writer: writer })
}

/// Repli TCP : signe de vie toutes les 5 s et mesure de l'aller-retour
/// (`ping`/`pong`, `n` = microsecondes depuis le départ) ; sans `pong`
/// pendant 30 s, la connexion est coupée.
pub(crate) struct Pinger {
    conn: Conn,
    epoch: std::time::Instant,
    last_pong: Arc<std::sync::atomic::AtomicU64>,
    _task: AbortOnDrop,
}

impl Pinger {
    pub fn start(conn: Conn, ctl: mpsc::UnboundedSender<Msg>) -> Pinger {
        use std::sync::atomic::Ordering;
        let epoch = std::time::Instant::now();
        let last_pong = Arc::new(std::sync::atomic::AtomicU64::new(0));
        let (c, lp) = (conn.clone(), last_pong.clone());
        let task = tokio::spawn(async move {
            let mut tick = tokio::time::interval(Duration::from_secs(5));
            loop {
                tick.tick().await;
                let now = epoch.elapsed().as_micros() as u64;
                if now.saturating_sub(lp.load(Ordering::Relaxed)) > 30_000_000 {
                    c.close("pas de pong");
                    return;
                }
                if ctl.send(Msg::Ping { n: now }).is_err() {
                    return;
                }
            }
        });
        Pinger { conn, epoch, last_pong, _task: AbortOnDrop(task) }
    }

    pub fn pong(&self, n: u64) {
        let now = self.epoch.elapsed().as_micros() as u64;
        if n <= now {
            self.conn.set_rtt(Duration::from_micros(now - n));
            self.last_pong.store(now, std::sync::atomic::Ordering::Relaxed);
        }
    }
}

/// Attente avant le prochain essai : 1, 2, 4, 8, 16 puis 30 s, à ±20 %.
pub fn backoff(attempt: u32) -> Duration {
    let base = [1u64, 2, 4, 8, 16, 30][attempt.min(5) as usize] as f64;
    let r = crate::id::random_bytes::<2>();
    let jitter = (u16::from_be_bytes(r) as f64 / u16::MAX as f64) * 0.4 - 0.2;
    Duration::from_secs_f64(base * (1.0 + jitter))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn attente_entre_les_essais() {
        for (attempt, base) in [(0u32, 1.0f64), (1, 2.0), (4, 16.0), (5, 30.0), (40, 30.0)] {
            let d = backoff(attempt).as_secs_f64();
            assert!(d >= base * 0.8 - 1e-6 && d <= base * 1.2 + 1e-6, "{attempt} : {d}");
        }
    }
}
