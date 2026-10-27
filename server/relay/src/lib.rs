//! Le relais de KyberNow (docs/relais.md) : partage et accès à son ordinateur
//! hors de chez soi. Un seul port, en TCP et en UDP.
//!
//! - [`hub`] : Stations, clients, messages, flux TCP et UDP, codes ;
//! - `quic` et `tcp` : les connexions entrantes et leurs limites ;
//! - [`web`] et [`pages`] : la page du relais et le mandataire du navigateur ;
//! - [`certs`] : le certificat relu à chaud ; [`metrics`] : Prometheus.

pub mod certs;
pub mod codes;
pub mod config;
mod counted;
pub mod hub;
pub mod limits;
pub mod metrics;
pub mod pages;
mod quic;
mod tcp;
pub mod web;

use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use tokio::net::TcpListener;
use tokio::task::JoinHandle;

use crate::certs::CertStore;
use crate::config::Config;
use crate::hub::Hub;
use crate::limits::Banlist;

/// Un relais lancé ; il s'arrête quand on le lâche.
pub struct Relay {
    /// L'adresse liée (TCP et UDP).
    pub addr: SocketAddr,
    pub metrics_addr: Option<SocketAddr>,
    pub hub: Arc<Hub>,
    endpoint: quinn::Endpoint,
    tasks: Vec<JoinHandle<()>>,
}

impl Drop for Relay {
    fn drop(&mut self) {
        self.endpoint.close(0u32.into(), b"arret");
        for t in &self.tasks {
            t.abort();
        }
    }
}

/// TCP et UDP sur le même port (port 0 : un port libre dans les deux).
fn bind(listen: SocketAddr) -> Result<(std::net::TcpListener, std::net::UdpSocket)> {
    for _ in 0..32 {
        let tcp = std::net::TcpListener::bind(listen).with_context(|| format!("TCP {listen}"))?;
        let addr = tcp.local_addr()?;
        match quic::bind_udp(addr) {
            Ok(udp) => {
                tcp.set_nonblocking(true)?;
                return Ok((tcp, udp));
            }
            Err(e) if listen.port() != 0 => return Err(e).with_context(|| format!("UDP {addr}")),
            Err(_) => continue,
        }
    }
    anyhow::bail!("pas de port libre en TCP et en UDP")
}

/// Lance le relais.
pub async fn start(cfg: Config) -> Result<Relay> {
    let _ = rustls::crypto::ring::default_provider().install_default();
    let cfg = Arc::new(cfg);
    let certs = CertStore::new(cfg.cert.clone(), cfg.key.clone());
    let banlist = Banlist::new(cfg.banlist.clone());
    let hub = Hub::new(cfg.clone(), certs.clone(), banlist.clone());
    let (tcp, udp) = bind(cfg.listen)?;
    let addr = tcp.local_addr()?;
    hub.link_port.store(cfg.link_port(addr.port()), Ordering::Relaxed);

    let quic_cfg = quic::server_config(certs.clone())?;
    let endpoint = quic::endpoint(udp, quic_cfg.clone())?;
    let tls = Arc::new(tcp::TlsConfigs::new(certs.clone())?);
    let mut tasks = vec![
        tokio::spawn(quic::run(hub.clone(), endpoint.clone(), quic_cfg)),
        tokio::spawn(tcp::run(hub.clone(), TcpListener::from_std(tcp)?, tls)),
        certs.watch(),
    ];
    {
        let (hub, banlist) = (hub.clone(), banlist.clone());
        tasks.push(tokio::spawn(async move {
            let mut tick = tokio::time::interval(Duration::from_secs(30));
            loop {
                tick.tick().await;
                banlist.reload_if_changed();
                hub.sweep();
            }
        }));
    }
    tasks.push(tokio::spawn(egress_monitor(hub.clone())));
    let metrics_addr = match cfg.metrics.listen {
        Some(m) => {
            let listener = TcpListener::bind(m).await.with_context(|| format!("métriques {m}"))?;
            let bound = listener.local_addr()?;
            let h = hub.clone();
            let render: Arc<dyn Fn() -> String + Send + Sync> = Arc::new(move || h.metrics.render(&h.gauges()));
            tasks.push(tokio::spawn(metrics::serve(listener, render)));
            Some(bound)
        }
        None => None,
    };
    tracing::info!(%addr, domain = %cfg.domain, "relais en écoute (TCP et UDP)");
    Ok(Relay { addr, metrics_addr, hub, endpoint, tasks })
}

/// Débit relayé sur 10 s ; un avertissement au-delà de `egress_alert_mbps`.
async fn egress_monitor(hub: Arc<Hub>) {
    let mut tick = tokio::time::interval(Duration::from_secs(10));
    let (mut last_bytes, mut last_at) = (hub.metrics.egress_total(), Instant::now());
    let mut last_alert: Option<Instant> = None;
    loop {
        tick.tick().await;
        let (bytes, now) = (hub.metrics.egress_total(), Instant::now());
        let secs = now.duration_since(last_at).as_secs_f64().max(0.001);
        let kbps = ((bytes - last_bytes) as f64 * 8.0 / 1000.0 / secs) as u64;
        hub.metrics.set_egress_kbps(kbps);
        let alert = hub.cfg.egress_alert_mbps;
        if alert > 0 && kbps / 1000 >= alert && last_alert.is_none_or(|t| now.duration_since(t) > Duration::from_secs(60)) {
            tracing::warn!(mbps = kbps / 1000, seuil = alert, "débit relayé au-dessus du seuil d'alerte");
            last_alert = Some(now);
        }
        (last_bytes, last_at) = (bytes, now);
    }
}
