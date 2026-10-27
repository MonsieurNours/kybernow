//! TCP 8443 : le relais lit le début de la négociation TLS. ALPN `kn-tcp/1` :
//! le repli du tunnel (yamux) ; nom `share.<domaine>` : ses pages ; nom
//! `<id>.share.<domaine>` : le navigateur vers la Station ; sinon, fermé.

use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use kybernow_tunnel::quic::ALPN_TCP;
use kybernow_tunnel::transport::{Conn, YamuxHandle};
use kybernow_tunnel::{id, tls};
use tokio::net::{TcpListener, TcpStream};
use tokio::time::timeout;
use tokio_util::compat::TokioAsyncReadCompatExt;

use crate::certs::CertStore;
use crate::hub::Hub;
use crate::limits::{ConnCounter, ConnGuard};
use crate::web::{self, Site};

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
        let Some(guard) = conns.try_acquire(ip, hub.cfg.limits.tcp_per_ip) else {
            hub.metrics.limit_hit("tcp_per_ip");
            continue;
        };
        tokio::spawn(handle(hub.clone(), tcp, peer, ip, configs.clone(), guard));
    }
}

async fn handle(hub: Arc<Hub>, tcp: TcpStream, peer: SocketAddr, ip: IpAddr, configs: Arc<TlsConfigs>, _guard: ConnGuard) {
    let _ = tcp.set_nodelay(true);
    let deadline = Duration::from_secs(hub.cfg.limits.header_timeout_secs);
    let acceptor = tokio_rustls::LazyConfigAcceptor::new(rustls::server::Acceptor::default(), tcp);
    let Ok(Ok(start)) = timeout(deadline, acceptor).await else { return };
    let hello = start.client_hello();
    let tunnel = hello.alpn().is_some_and(|mut a| a.any(|p| p == ALPN_TCP));
    let site = web::site_for(hello.server_name(), &hub.cfg.domain);
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
