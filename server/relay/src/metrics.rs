//! Métriques Prometheus du relais (format texte, écrit à la main) et
//! `/healthz` pour le contrôle de santé de Docker ; sur 127.0.0.1:9464.

use std::collections::BTreeMap;
use std::convert::Infallible;
use std::fmt::Write as _;
use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};

use bytes::Bytes;
use http_body_util::Full;
use hyper::service::service_fn;
use hyper::{Request, Response, StatusCode};
use hyper_util::rt::TokioIo;
use tokio::net::TcpListener;

/// Les trois trajets : paquets UDP, flux TCP des apps, navigateur.
#[derive(Debug, Clone, Copy)]
pub enum Leg {
    Media = 0,
    Stream = 1,
    Web = 2,
}

/// Le sens : vers la Station, vers le client.
#[derive(Debug, Clone, Copy)]
pub enum Dir {
    ToStation = 0,
    ToClient = 1,
}

/// Ce qu'est devenu un paquet relayé.
#[derive(Debug, Clone, Copy)]
pub enum Dg {
    Forwarded = 0,
    Fallback = 1,
    TooLarge = 2,
    Full = 3,
    Closed = 4,
    Unsupported = 5,
    NoRoute = 6,
    Capped = 7,
}

const LEGS: [&str; 3] = ["media", "stream", "web"];
const DIRS: [&str; 2] = ["to_station", "to_client"];
const DGS: [&str; 8] = ["forwarded", "fallback", "dropped_too_large", "dropped_full", "dropped_closed", "dropped_unsupported", "no_route", "capped"];

/// Ce qui se lit au moment de la collecte.
#[derive(Debug, Default, Clone)]
pub struct Gauges {
    pub stations_quic: usize,
    pub stations_tcp: usize,
    pub clients_app: usize,
    pub clients_web: usize,
    pub sessions: usize,
    pub codes: usize,
    pub rtt_station: Option<f64>,
    pub rtt_client: Option<f64>,
    pub cert_expiry: Option<i64>,
}

#[derive(Debug, Default)]
pub struct Metrics {
    bytes: [[AtomicU64; 2]; 3],
    datagrams: [AtomicU64; 8],
    /// Compteurs plus rares : (nom, étiquettes) → valeur.
    counters: Mutex<BTreeMap<(&'static str, String), u64>>,
    /// Débit sortant des 10 dernières secondes, en kbit/s.
    egress_kbps: AtomicU64,
}

impl Metrics {
    pub fn add_bytes(&self, leg: Leg, dir: Dir, n: u64) {
        self.bytes[leg as usize][dir as usize].fetch_add(n, Ordering::Relaxed);
    }

    pub fn datagram(&self, r: Dg) {
        self.datagrams[r as usize].fetch_add(1, Ordering::Relaxed);
    }

    /// Un compteur rare : `labels` déjà écrites (`result="found"`).
    pub fn inc(&self, name: &'static str, labels: String) {
        *self.counters.lock().unwrap().entry((name, labels)).or_insert(0) += 1;
    }

    pub fn limit_hit(&self, limit: &str) {
        self.inc("relay_limit_hits_total", format!("limit=\"{limit}\""));
    }

    pub fn lookup(&self, result: &str) {
        self.inc("relay_code_lookups_total", format!("result=\"{result}\""));
    }

    pub fn handshake(&self, alpn: &str, result: &str) {
        self.inc("relay_handshakes_total", format!("alpn=\"{alpn}\",result=\"{result}\""));
    }

    /// Octets envoyés par le relais (vers les deux bouts), depuis le départ.
    pub fn egress_total(&self) -> u64 {
        self.bytes.iter().flat_map(|l| l.iter()).map(|a| a.load(Ordering::Relaxed)).sum()
    }

    pub fn set_egress_kbps(&self, kbps: u64) {
        self.egress_kbps.store(kbps, Ordering::Relaxed);
    }

    pub fn counter(&self, name: &'static str, labels: &str) -> u64 {
        self.counters.lock().unwrap().get(&(name, labels.to_string())).copied().unwrap_or(0)
    }

    pub fn render(&self, g: &Gauges) -> String {
        let mut o = String::new();
        let gauge = |o: &mut String, name: &str, help: &str| {
            let _ = writeln!(o, "# HELP {name} {help}\n# TYPE {name} gauge");
        };
        gauge(&mut o, "relay_stations_connected", "Stations connectées, par transport.");
        let _ = writeln!(o, "relay_stations_connected{{transport=\"quic\"}} {}", g.stations_quic);
        let _ = writeln!(o, "relay_stations_connected{{transport=\"tcp\"}} {}", g.stations_tcp);
        gauge(&mut o, "relay_clients_connected", "Clients connectés : apps et connexions de navigateur.");
        let _ = writeln!(o, "relay_clients_connected{{kind=\"app\"}} {}", g.clients_app);
        let _ = writeln!(o, "relay_clients_connected{{kind=\"web\"}} {}", g.clients_web);
        gauge(&mut o, "relay_sessions_active", "Apps qui ont au moins un flux UDP ouvert.");
        let _ = writeln!(o, "relay_sessions_active {}", g.sessions);
        gauge(&mut o, "relay_codes_active", "Codes de partage réservés.");
        let _ = writeln!(o, "relay_codes_active {}", g.codes);
        gauge(&mut o, "relay_rtt_seconds", "Aller-retour moyen avec le relais.");
        if let Some(r) = g.rtt_station {
            let _ = writeln!(o, "relay_rtt_seconds{{peer=\"station\"}} {r:.6}");
        }
        if let Some(r) = g.rtt_client {
            let _ = writeln!(o, "relay_rtt_seconds{{peer=\"client\"}} {r:.6}");
        }
        gauge(&mut o, "relay_tls_cert_expiry_seconds", "Secondes avant l'expiration du certificat.");
        if let Some(e) = g.cert_expiry {
            let _ = writeln!(o, "relay_tls_cert_expiry_seconds {e}");
        }
        gauge(&mut o, "relay_egress_mbps", "Débit relayé, moyenne des 10 dernières secondes.");
        let _ = writeln!(o, "relay_egress_mbps {:.3}", self.egress_kbps.load(Ordering::Relaxed) as f64 / 1000.0);
        let _ = writeln!(o, "# HELP relay_bytes_total Octets relayés.\n# TYPE relay_bytes_total counter");
        for (l, leg) in LEGS.iter().enumerate() {
            for (d, dir) in DIRS.iter().enumerate() {
                let _ = writeln!(o, "relay_bytes_total{{leg=\"{leg}\",dir=\"{dir}\"}} {}", self.bytes[l][d].load(Ordering::Relaxed));
            }
        }
        let _ = writeln!(o, "# HELP relay_datagrams_total Paquets UDP relayés.\n# TYPE relay_datagrams_total counter");
        for (i, r) in DGS.iter().enumerate() {
            let _ = writeln!(o, "relay_datagrams_total{{result=\"{r}\"}} {}", self.datagrams[i].load(Ordering::Relaxed));
        }
        let counters = self.counters.lock().unwrap();
        let mut last = "";
        for ((name, labels), v) in counters.iter() {
            if *name != last {
                let _ = writeln!(o, "# TYPE {name} counter");
                last = name;
            }
            let _ = writeln!(o, "{name}{{{labels}}} {v}");
        }
        o
    }
}

/// Sert `/metrics` et `/healthz`.
pub async fn serve(listener: TcpListener, render: Arc<dyn Fn() -> String + Send + Sync>) {
    while let Ok((tcp, _)) = listener.accept().await {
        let render = render.clone();
        tokio::spawn(async move {
            let svc = service_fn(move |req: Request<hyper::body::Incoming>| {
                let render = render.clone();
                async move {
                    let (status, body) = match req.uri().path() {
                        "/metrics" => (StatusCode::OK, render()),
                        "/healthz" => (StatusCode::OK, "ok\n".to_string()),
                        _ => (StatusCode::NOT_FOUND, String::new()),
                    };
                    let mut resp = Response::new(Full::new(Bytes::from(body)));
                    *resp.status_mut() = status;
                    resp.headers_mut().insert("content-type", "text/plain; version=0.0.4; charset=utf-8".parse().unwrap());
                    Ok::<_, Infallible>(resp)
                }
            });
            let _ = hyper::server::conn::http1::Builder::new().serve_connection(TokioIo::new(tcp), svc).await;
        });
    }
}

/// `--healthcheck` : `/healthz` répond-il ? (L'image n'a pas de curl.)
pub fn healthcheck(addr: SocketAddr) -> bool {
    use std::io::{Read, Write};
    let Ok(mut s) = std::net::TcpStream::connect_timeout(&addr, std::time::Duration::from_secs(3)) else { return false };
    let _ = s.set_read_timeout(Some(std::time::Duration::from_secs(3)));
    if s.write_all(b"GET /healthz HTTP/1.0\r\nHost: localhost\r\n\r\n").is_err() {
        return false;
    }
    let mut buf = [0u8; 64];
    let n = s.read(&mut buf).unwrap_or(0);
    std::str::from_utf8(&buf[..n]).is_ok_and(|t| t.starts_with("HTTP/1.1 200") || t.starts_with("HTTP/1.0 200"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_prometheus() {
        let m = Metrics::default();
        m.add_bytes(Leg::Media, Dir::ToClient, 1200);
        m.datagram(Dg::Forwarded);
        m.lookup("found");
        m.lookup("found");
        m.limit_hit("lookup_ip_minute");
        let text = m.render(&Gauges { stations_quic: 2, cert_expiry: Some(86_400), ..Default::default() });
        assert!(text.contains("relay_stations_connected{transport=\"quic\"} 2"));
        assert!(text.contains("relay_bytes_total{leg=\"media\",dir=\"to_client\"} 1200"));
        assert!(text.contains("relay_datagrams_total{result=\"forwarded\"} 1"));
        assert!(text.contains("relay_code_lookups_total{result=\"found\"} 2"));
        assert!(text.contains("relay_limit_hits_total{limit=\"lookup_ip_minute\"} 1"));
        assert!(text.contains("relay_tls_cert_expiry_seconds 86400"));
        assert_eq!(m.egress_total(), 1200);
    }
}
