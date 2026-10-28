//! QUIC sur le port UDP du relais (8443 par défaut) : `kn-ctl/1` et `kn-media/1`. Les limites passent avant
//! la négociation (bannissement, connexions par adresse, négociations par
//! minute) ; sous charge, le relais demande une preuve d'adresse (Retry).

use std::net::{IpAddr, SocketAddr};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use kybernow_tunnel::quic::{ALPN_CTL, ALPN_MEDIA, alpn, incoming_config};
use kybernow_tunnel::transport::Conn;
use kybernow_tunnel::{id, tls};
use rustls::pki_types::CertificateDer;
use tokio::time::timeout;

use crate::certs::CertStore;
use crate::hub::Hub;
use crate::limits::{Bucket, Buckets, ConnCounter, ConnGuard};

/// Configuration QUIC du relais : son certificat, le certificat client facultatif.
pub fn server_config(certs: Arc<CertStore>) -> Result<quinn::ServerConfig> {
    let mut tls = rustls::ServerConfig::builder_with_provider(tls::provider())
        .with_protocol_versions(&[&rustls::version::TLS13])?
        .with_client_cert_verifier(tls::AnyClientCert::new())
        .with_cert_resolver(certs);
    tls.alpn_protocols = vec![ALPN_CTL.to_vec(), ALPN_MEDIA.to_vec()];
    let crypto = quinn::crypto::rustls::QuicServerConfig::try_from(tls).context("configuration QUIC")?;
    Ok(quinn::ServerConfig::with_crypto(Arc::new(crypto)))
}

/// Le socket UDP du relais, avec de grands tampons (8 Mio, si le système le permet).
pub fn bind_udp(addr: SocketAddr) -> std::io::Result<std::net::UdpSocket> {
    use socket2::{Domain, Protocol, Socket, Type};
    let sock = Socket::new(Domain::for_address(addr), Type::DGRAM, Some(Protocol::UDP))?;
    if addr.is_ipv6() {
        let _ = sock.set_only_v6(false);
    }
    let _ = sock.set_recv_buffer_size(8 << 20);
    let _ = sock.set_send_buffer_size(8 << 20);
    sock.bind(&addr.into())?;
    Ok(sock.into())
}

pub fn endpoint(socket: std::net::UdpSocket, cfg: quinn::ServerConfig) -> std::io::Result<quinn::Endpoint> {
    quinn::Endpoint::new(quinn::EndpointConfig::default(), Some(cfg), socket, Arc::new(quinn::TokioRuntime))
}

fn peer_spki(conn: &quinn::Connection) -> Option<Vec<u8>> {
    let certs = conn.peer_identity()?.downcast::<Vec<CertificateDer<'static>>>().ok()?;
    id::spki_of_cert(certs.first()?)
}

pub async fn run(hub: Arc<Hub>, endpoint: quinn::Endpoint, base: quinn::ServerConfig) {
    let l = hub.cfg.limits.clone();
    let conns = ConnCounter::default();
    let per_ip: Mutex<Buckets<IpAddr>> = Mutex::new(Buckets::new(l.quic_handshakes_per_minute_per_ip, Duration::from_secs(60)));
    let mut load = Bucket::new(l.retry_above_per_second, Duration::from_secs(1), Instant::now());
    let mut last_sweep = Instant::now();
    while let Some(incoming) = endpoint.accept().await {
        let ip = incoming.remote_address().ip().to_canonical();
        let now = Instant::now();
        if now.duration_since(last_sweep) > Duration::from_secs(60) {
            per_ip.lock().unwrap().sweep(now);
            last_sweep = now;
        }
        if hub.banlist.contains(ip) {
            hub.metrics.limit_hit("banned");
            incoming.refuse();
            continue;
        }
        if load.take(now).is_err() && !incoming.remote_address_validated() && incoming.may_retry() {
            hub.metrics.handshake("quic", "retry");
            let _ = incoming.retry();
            continue;
        }
        if conns.total() >= l.quic_total {
            hub.metrics.limit_hit("quic_total");
            incoming.refuse();
            continue;
        }
        if per_ip.lock().unwrap().take(&ip, now).is_err() {
            hub.metrics.limit_hit("quic_handshakes_ip");
            incoming.refuse();
            continue;
        }
        let Some(guard) = conns.try_acquire(ip, l.quic_per_ip) else {
            hub.metrics.limit_hit("quic_per_ip");
            incoming.refuse();
            continue;
        };
        let (cfg, mode) = incoming_config(&base);
        match incoming.accept_with(Arc::new(cfg)) {
            Ok(connecting) => {
                tokio::spawn(handle(hub.clone(), connecting, mode, ip, guard));
            }
            Err(_) => hub.metrics.handshake("quic", "error"),
        }
    }
}

async fn drain_ctl(conn: quinn::Connection) {
    loop {
        tokio::select! {
            uni = conn.accept_uni() => match uni {
                Ok(mut r) => {
                    let _ = r.stop(0u32.into());
                }
                Err(_) => return,
            },
            d = conn.read_datagram() => if d.is_err() {
                return;
            },
        }
    }
}

async fn handle(hub: Arc<Hub>, connecting: quinn::Connecting, mode: kybernow_tunnel::congestion::CcMode, ip: IpAddr, _guard: ConnGuard) {
    let conn = match timeout(Duration::from_secs(10), connecting).await {
        Ok(Ok(c)) => c,
        _ => return hub.metrics.handshake("quic", "error"),
    };
    let spki = peer_spki(&conn);
    match alpn(&conn).as_deref() {
        Some(ALPN_CTL) => {
            hub.metrics.handshake("kn-ctl/1", "ok");
            // Les réglages permettent tout avant l'ALPN : sur `kn-ctl/1`, les
            // flux unidirectionnels et les datagrammes sont refusés ou jetés,
            // pour qu'ils ne s'entassent pas en mémoire.
            let drain = tokio::spawn(drain_ctl(conn.clone()));
            hub.serve_ctl(Conn::Quic(conn), spki, ip).await;
            drain.abort();
        }
        Some(ALPN_MEDIA) => {
            mode.set_permissive();
            hub.metrics.handshake("kn-media/1", "ok");
            hub.serve_media_quic(conn, spki).await;
        }
        _ => conn.close(0u32.into(), b"alpn"),
    }
}
