//! TCP, sur le port du relais (8443 par défaut) : le relais lit le début de la
//! négociation TLS (le `ClientHello`) sans y répondre, puis :
//!
//! - ALPN `kn-tcp/1` : le repli du tunnel (yamux) ;
//! - nom `share.<domaine>` : ses pages ;
//! - nom `<id>.<tls_zone>` (`station.<domaine>`) : le TLS passe **tel quel**
//!   jusqu'à la Station, qui tient sa clé (flux `browser_tls`, docs/relais.md,
//!   « Navigateur chiffré de bout en bout ») ; le relais ne déchiffre rien ;
//! - nom `<id>.share.<domaine>` : le navigateur vers la Station, déchiffré ici
//!   (flux `admin_http`, Stations d'avant la 0.11.8) ;
//! - sinon, fermé.

use std::io;
use std::net::{IpAddr, SocketAddr};
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};
use std::time::Duration;

use anyhow::Result;
use kybernow_tunnel::quic::ALPN_TCP;
use kybernow_tunnel::transport::{Conn, YamuxHandle};
use kybernow_tunnel::{id, tls};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, ReadBuf};
use tokio::net::{TcpListener, TcpStream};
use tokio::time::timeout;
use tokio_util::compat::TokioAsyncReadCompatExt;

use crate::certs::CertStore;
use crate::hub::{ADMIN_STREAM_IDLE, Hub, copy_until_idle};
use crate::limits::{ConnCounter, ConnGuard};
use crate::web::{self, Site};

/// Le `ClientHello` le plus long qu'on attende (une extension de clé
/// post-quantique le porte vers 2 Kio ; un enregistrement TLS fait 16 Kio).
const MAX_HELLO: usize = 32 * 1024;

/// Les deux configurations TLS sur TCP : tunnel et navigateur.
pub struct TlsConfigs {
    tunnel: Arc<rustls::ServerConfig>,
    web: Arc<rustls::ServerConfig>,
}

impl TlsConfigs {
    pub fn new(certs: Arc<CertStore>) -> Result<TlsConfigs> {
        let mut tunnel = rustls::ServerConfig::builder_with_provider(tls::provider())
            .with_protocol_versions(&[&rustls::version::TLS13])?
            .with_client_cert_verifier(tls::AnyClientCert::new())
            .with_cert_resolver(certs.clone());
        tunnel.alpn_protocols = vec![ALPN_TCP.to_vec()];
        // Les navigateurs anciens (TLS 1.2) passent aussi.
        let mut web = rustls::ServerConfig::builder_with_provider(tls::provider())
            .with_safe_default_protocol_versions()?
            .with_no_client_auth()
            .with_cert_resolver(certs);
        web.alpn_protocols = vec![b"http/1.1".to_vec()];
        Ok(TlsConfigs { tunnel: Arc::new(tunnel), web: Arc::new(web) })
    }
}

pub async fn run(hub: Arc<Hub>, listener: TcpListener, configs: Arc<TlsConfigs>) {
    let conns = ConnCounter::default();
    loop {
        let Ok((tcp, peer)) = listener.accept().await else {
            tokio::time::sleep(Duration::from_millis(50)).await;
            continue;
        };
        let ip = peer.ip().to_canonical();
        if hub.banlist.contains(ip) {
            hub.metrics.limit_hit("banned");
            continue;
        }
        if conns.total() >= hub.cfg.limits.tcp_total {
            hub.metrics.limit_hit("tcp_total");
            continue;
        }
        let Some(guard) = conns.try_acquire(ip, hub.cfg.limits.tcp_per_ip) else {
            hub.metrics.limit_hit("tcp_per_ip");
            continue;
        };
        tokio::spawn(handle(hub.clone(), tcp, peer, ip, configs.clone(), guard));
    }
}

/// Ce que dit le `ClientHello` : le nom présenté et les ALPN proposés.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hello {
    pub sni: Option<String>,
    pub alpn: Vec<Vec<u8>>,
}

/// Lit le `ClientHello` sans répondre : ce qu'il dit, et les octets lus (à
/// rejouer devant la négociation, ou à faire passer tels quels). `None` :
/// connexion fermée, pas du TLS, ou trop long.
pub async fn read_hello<S: AsyncRead + Unpin>(io: &mut S) -> Option<(Hello, Vec<u8>)> {
    let mut acceptor = rustls::server::Acceptor::default();
    let mut raw = Vec::new();
    let mut chunk = [0u8; 4096];
    loop {
        let n = io.read(&mut chunk).await.ok()?;
        if n == 0 || raw.len() + n > MAX_HELLO {
            return None;
        }
        raw.extend_from_slice(&chunk[..n]);
        let mut rest = &chunk[..n];
        while !rest.is_empty() {
            if acceptor.read_tls(&mut rest).ok()? == 0 {
                return None;
            }
        }
        match acceptor.accept() {
            Ok(Some(accepted)) => {
                let hello = accepted.client_hello();
                let sni = hello.server_name().map(str::to_string);
                let alpn = hello.alpn().map(|a| a.map(<[u8]>::to_vec).collect()).unwrap_or_default();
                return Some((Hello { sni, alpn }, raw));
            }
            Ok(None) => {}
            Err(_) => return None,
        }
    }
}

/// Un flux dont la lecture commence par des octets déjà lus.
pub struct Replay<S> {
    head: Vec<u8>,
    pos: usize,
    inner: S,
}

impl<S> Replay<S> {
    pub fn new(head: Vec<u8>, inner: S) -> Self {
        Replay { head, pos: 0, inner }
    }
}

impl<S: AsyncRead + Unpin> AsyncRead for Replay<S> {
    fn poll_read(mut self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &mut ReadBuf<'_>) -> Poll<io::Result<()>> {
        if self.pos < self.head.len() {
            let n = (self.head.len() - self.pos).min(buf.remaining());
            let start = self.pos;
            buf.put_slice(&self.head[start..start + n]);
            self.pos += n;
            if self.pos == self.head.len() {
                self.head = Vec::new();
                self.pos = 0;
            }
            return Poll::Ready(Ok(()));
        }
        Pin::new(&mut self.inner).poll_read(cx, buf)
    }
}

impl<S: AsyncWrite + Unpin> AsyncWrite for Replay<S> {
    fn poll_write(mut self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &[u8]) -> Poll<io::Result<usize>> {
        Pin::new(&mut self.inner).poll_write(cx, buf)
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.inner).poll_flush(cx)
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.inner).poll_shutdown(cx)
    }
}

async fn handle(hub: Arc<Hub>, mut tcp: TcpStream, peer: SocketAddr, ip: IpAddr, configs: Arc<TlsConfigs>, _guard: ConnGuard) {
    let _ = tcp.set_nodelay(true);
    let deadline = Duration::from_secs(hub.cfg.limits.header_timeout_secs);
    let Ok(Some((hello, raw))) = timeout(deadline, read_hello(&mut tcp)).await else { return };
    let tunnel = hello.alpn.iter().any(|p| p == ALPN_TCP);
    let site = if tunnel { None } else { web::site_for(hello.sni.as_deref(), &hub.cfg.domain, hub.tls_zone().as_deref()) };
    if let Some(Site::Passthrough(station)) = site {
        return passthrough(hub, tcp, raw, ip, &station, hello.sni.unwrap_or_default()).await;
    }
    // Le reste se négocie ici : le `ClientHello` déjà lu est rejoué devant.
    let io = Replay::new(raw, tcp);
    let acceptor = tokio_rustls::LazyConfigAcceptor::new(rustls::server::Acceptor::default(), io);
    let Ok(Ok(start)) = timeout(deadline, acceptor).await else { return };
    if tunnel {
        let Ok(Ok(tls)) = timeout(deadline, start.into_stream(configs.tunnel.clone())).await else {
            return hub.metrics.handshake("kn-tcp/1", "error");
        };
        hub.metrics.handshake("kn-tcp/1", "ok");
        let spki = tls.get_ref().1.peer_certificates().and_then(|c| id::spki_of_cert(c.first()?));
        let conn = Conn::Yamux(YamuxHandle::start(tls.compat(), yamux::Mode::Server, peer));
        hub.serve_ctl(conn, spki, ip).await;
        return;
    }
    let Some(site) = site else {
        return hub.metrics.handshake("http", "refused");
    };
    let Ok(Ok(tls)) = timeout(deadline, start.into_stream(configs.web.clone())).await else {
        return hub.metrics.handshake("http", "error");
    };
    hub.metrics.handshake("http", "ok");
    if let Site::Station(s) = &site {
        tracing::debug!(station = %s, "navigateur");
    }
    web::serve(hub, tls, ip, site).await;
}

/// `<id>.<tls_zone>` : un flux `browser_tls` vers la Station, où passent les
/// octets tels quels, `ClientHello` compris. Station absente, ou qui ne sert
/// pas ces flux (d'avant la 0.11.8) : fermé, faute de certificat pour ce nom.
async fn passthrough(hub: Arc<Hub>, mut tcp: TcpStream, raw: Vec<u8>, ip: IpAddr, station: &str, host: String) {
    let _guard = hub.web_guard();
    let browser = id::random_id();
    let Some(stream) = hub.open_tls_stream(station, &browser, ip, &host.trim_end_matches('.').to_ascii_lowercase()).await else {
        return hub.metrics.handshake("browser_tls", "refused");
    };
    hub.metrics.handshake("browser_tls", "ok");
    tracing::debug!(station = %station, "navigateur de bout en bout");
    let mut to_station = crate::counted::Counted::new(stream, hub.metrics.clone());
    if to_station.write_all(&raw).await.is_err() {
        return;
    }
    drop(raw);
    copy_until_idle(&mut tcp, &mut to_station, Some(ADMIN_STREAM_IDLE)).await;
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Un vrai `ClientHello` (rustls client) : le nom et l'ALPN se lisent, et les
    /// octets lus sont exactement ceux envoyés jusque-là.
    #[tokio::test]
    async fn lecture_du_client_hello() {
        let (mut client_side, mut relay_side) = tokio::io::duplex(64 * 1024);
        let mut cfg = rustls::ClientConfig::builder_with_provider(tls::provider())
            .with_safe_default_protocol_versions()
            .unwrap()
            .with_root_certificates(rustls::RootCertStore::empty())
            .with_no_client_auth();
        cfg.alpn_protocols = vec![b"acme-tls/1".to_vec()];
        let mut conn = rustls::ClientConnection::new(Arc::new(cfg), "abc.station.share.test".try_into().unwrap()).unwrap();
        let mut sent = Vec::new();
        conn.write_tls(&mut sent).unwrap();
        client_side.write_all(&sent).await.unwrap();
        let (hello, raw) = read_hello(&mut relay_side).await.unwrap();
        assert_eq!(hello.sni.as_deref(), Some("abc.station.share.test"));
        assert_eq!(hello.alpn, vec![b"acme-tls/1".to_vec()]);
        assert_eq!(raw, sent);
        // Pas du TLS : rien.
        let (mut c, mut r) = tokio::io::duplex(1024);
        c.write_all(b"GET / HTTP/1.1\r\nHost: x\r\n\r\n").await.unwrap();
        assert!(read_hello(&mut r).await.is_none());
    }

    #[tokio::test]
    async fn rejoue_avant_le_reste() {
        let (mut a, b) = tokio::io::duplex(1024);
        a.write_all(b" suite").await.unwrap();
        drop(a);
        let mut r = Replay::new(b"debut".to_vec(), b);
        let mut all = String::new();
        r.read_to_string(&mut all).await.unwrap();
        assert_eq!(all, "debut suite");
    }
}
