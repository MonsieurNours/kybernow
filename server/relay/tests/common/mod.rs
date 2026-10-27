//! Outils des essais du relais sur la boucle locale : une autorité d'essai,
//! un relais, une fausse Station (agent du tunnel, faux Admin, écho UDP et
//! TCP), un client HTTPS.

#![allow(dead_code)]

use std::collections::HashMap;
use std::convert::Infallible;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};

use bytes::Bytes;
use http_body_util::{BodyExt, Full};
use hyper::service::service_fn;
use hyper::{Request, Response, StatusCode};
use hyper_util::rt::TokioIo;
use kybernow_relay::config::Config;
use kybernow_relay::{Relay, start};
use kybernow_tunnel::station::{Agent, AgentConfig, Incoming};
use kybernow_tunnel::{RelayAddr, StationIdentity, StreamHeader, StreamKind, id};
use rustls::RootCertStore;
use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer, ServerName, UnixTime};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpStream, UdpSocket};
use tokio::task::JoinHandle;

pub const DOMAIN: &str = "share.test";

pub struct Pki {
    pub roots: Arc<RootCertStore>,
    pub dir: PathBuf,
    pub cert: PathBuf,
    pub key: PathBuf,
}

/// Une autorité d'essai et le certificat du relais (`share.test`,
/// `*.share.test`, `localhost`), écrits dans un dossier temporaire.
pub fn pki() -> Pki {
    let dir = std::env::temp_dir().join(format!("kn-relay-essai-{}", id::random_id()));
    std::fs::create_dir_all(&dir).unwrap();
    let ca_key = rcgen::KeyPair::generate().unwrap();
    let mut ca_params = rcgen::CertificateParams::new(Vec::<String>::new()).unwrap();
    ca_params.is_ca = rcgen::IsCa::Ca(rcgen::BasicConstraints::Unconstrained);
    ca_params.distinguished_name.push(rcgen::DnType::CommonName, "Autorite d'essai du relais");
    let ca = ca_params.self_signed(&ca_key).unwrap();
    let key = rcgen::KeyPair::generate().unwrap();
    let leaf = rcgen::CertificateParams::new(vec![DOMAIN.into(), format!("*.{DOMAIN}"), "localhost".into()])
        .unwrap()
        .signed_by(&key, &ca, &ca_key)
        .unwrap();
    let (cert, keyf) = (dir.join("relay.crt"), dir.join("relay.key"));
    std::fs::write(&cert, format!("{}{}", leaf.pem(), ca.pem())).unwrap();
    std::fs::write(&keyf, key.serialize_pem()).unwrap();
    let mut roots = RootCertStore::empty();
    roots.add(ca.der().clone()).unwrap();
    Pki { roots: Arc::new(roots), dir, cert, key: keyf }
}

/// Un relais sur 127.0.0.1, port libre.
pub async fn relay(pki: &Pki, tweak: impl FnOnce(&mut Config)) -> Relay {
    let mut cfg = Config {
        domain: DOMAIN.into(),
        listen: "127.0.0.1:0".parse().unwrap(),
        cert: pki.cert.clone(),
        key: pki.key.clone(),
        ..Config::default()
    };
    cfg.metrics.listen = Some("127.0.0.1:0".parse().unwrap());
    tweak(&mut cfg);
    start(cfg).await.unwrap()
}

pub fn relay_addr(relay: &Relay) -> RelayAddr {
    RelayAddr { host: "localhost".into(), port: relay.addr.port(), pin: None }
}

/// Ce que la fausse Station laisse passer et répond.
#[derive(Clone)]
pub struct Policy {
    pub flows: bool,
    pub kyber_tcp: bool,
    /// Réponse au deuxième `wait` : 200 (accepté) ou 403 (refusé).
    pub wait_status: u16,
}

impl Default for Policy {
    fn default() -> Self {
        Policy { flows: true, kyber_tcp: true, wait_status: 200 }
    }
}

pub struct FakeStation {
    pub agent: Agent,
    pub identity: Arc<StationIdentity>,
    pub admin_fp: String,
    pub headers: Arc<Mutex<Vec<StreamHeader>>>,
    pub udp_echo: SocketAddr,
    tasks: Vec<JoinHandle<()>>,
}

impl Drop for FakeStation {
    fn drop(&mut self) {
        for t in &self.tasks {
            t.abort();
        }
    }
}

/// Le certificat du faux Admin (signé par lui-même, épinglé par l'app).
pub struct AdminCert {
    pub der: CertificateDer<'static>,
    pub key: Vec<u8>,
    pub fp: String,
}

pub fn admin_cert() -> AdminCert {
    let key = rcgen::KeyPair::generate().unwrap();
    let cert = rcgen::CertificateParams::new(vec!["kybernow-admin".into()]).unwrap().self_signed(&key).unwrap();
    let der = cert.der().clone();
    AdminCert { fp: id::fingerprint(&der), der, key: key.serialize_der() }
}

async fn udp_echo() -> (SocketAddr, JoinHandle<()>) {
    let sock = UdpSocket::bind("127.0.0.1:0").await.unwrap();
    let addr = sock.local_addr().unwrap();
    let task = tokio::spawn(async move {
        let mut buf = vec![0u8; 65_536];
        while let Ok((n, from)) = sock.recv_from(&mut buf).await {
            let _ = sock.send_to(&buf[..n], from).await;
        }
    });
    (addr, task)
}

type Resp = Response<Full<Bytes>>;

fn resp(status: u16, body: serde_json::Value) -> Resp {
    let mut r = Response::new(Full::new(Bytes::from(body.to_string())));
    *r.status_mut() = StatusCode::from_u16(status).unwrap();
    r
}

/// Le faux Admin derrière le relais : `join` → 202, `wait` → 202 puis
/// `wait_status` ; `/link/` renvoie les en-têtes reçus et pose un cookie ;
/// `/ws` passe en WebSocket (écho).
async fn fake_admin(mut req: Request<hyper::body::Incoming>, waits: Arc<AtomicU32>, policy: Policy) -> Result<Resp, Infallible> {
    let path = req.uri().path().to_string();
    Ok(match path.as_str() {
        "/api/share/join" => resp(202, serde_json::json!({ "pending": true, "guest": "g1", "secret": "s1", "expires": 120 })),
        "/api/share/wait" => {
            if waits.fetch_add(1, Ordering::Relaxed) == 0 {
                resp(202, serde_json::json!({ "pending": true }))
            } else if policy.wait_status == 200 {
                resp(200, serde_json::json!({ "guest": "g1", "host": "Léa", "relay": true, "kyber": { "port": 12345 } }))
            } else {
                resp(403, serde_json::json!({ "error": "refused" }))
            }
        }
        "/ws" => {
            let up = hyper::upgrade::on(&mut req);
            tokio::spawn(async move {
                if let Ok(up) = up.await {
                    let mut io = TokioIo::new(up);
                    let mut buf = [0u8; 1024];
                    while let Ok(n) = io.read(&mut buf).await {
                        if n == 0 || io.write_all(&buf[..n]).await.is_err() {
                            break;
                        }
                    }
                }
            });
            let mut r = Response::new(Full::new(Bytes::new()));
            *r.status_mut() = StatusCode::SWITCHING_PROTOCOLS;
            r.headers_mut().insert("connection", "upgrade".parse().unwrap());
            r.headers_mut().insert("upgrade", "websocket".parse().unwrap());
            r
        }
        _ => {
            let headers: HashMap<String, String> =
                req.headers().iter().map(|(k, v)| (k.as_str().to_string(), v.to_str().unwrap_or("").to_string())).collect();
            let mut r = resp(200, serde_json::json!({ "path": path, "headers": headers }));
            r.headers_mut().append("set-cookie", format!("kybernow_guest=abc; Path=/; Domain={DOMAIN}; Secure; HttpOnly").parse().unwrap());
            r.headers_mut().append("set-cookie", "autre=1; domain=.share.test; Path=/".parse().unwrap());
            r
        }
    })
}

/// Une fausse Station : l'agent du tunnel et ce qu'Admin ferait des flux.
pub fn fake_station(relay: RelayAddr, roots: Arc<RootCertStore>, force_tcp: bool, policy: Policy) -> FakeStation {
    fake_station_with(Arc::new(StationIdentity::generate().unwrap()), relay, roots, force_tcp, policy)
}

pub fn fake_station_with(identity: Arc<StationIdentity>, relay: RelayAddr, roots: Arc<RootCertStore>, force_tcp: bool, policy: Policy) -> FakeStation {
    let admin = admin_cert();
    let (agent, mut incoming) = Agent::start(AgentConfig {
        relay,
        identity: identity.clone(),
        admin_fp: admin.fp.clone(),
        version: "essai".into(),
        roots: Some(roots),
        force_tcp,
    });
    let headers = Arc::new(Mutex::new(Vec::new()));
    let std_echo = std::net::UdpSocket::bind("127.0.0.1:0").unwrap();
    let udp_echo_addr = std_echo.local_addr().unwrap();
    std_echo.set_nonblocking(true).unwrap();
    let echo_task = tokio::spawn(async move {
        let sock = UdpSocket::from_std(std_echo).unwrap();
        let mut buf = vec![0u8; 65_536];
        while let Ok((n, from)) = sock.recv_from(&mut buf).await {
            let _ = sock.send_to(&buf[..n], from).await;
        }
    });
    let tls = {
        let mut cfg = rustls::ServerConfig::builder_with_provider(kybernow_tunnel::tls::provider())
            .with_safe_default_protocol_versions()
            .unwrap()
            .with_no_client_auth()
            .with_single_cert(vec![admin.der.clone()], PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(admin.key.clone())))
            .unwrap();
        cfg.alpn_protocols = vec![b"http/1.1".to_vec()];
        tokio_rustls::TlsAcceptor::from(Arc::new(cfg))
    };
    let waits = Arc::new(AtomicU32::new(0));
    let seen = headers.clone();
    let task = tokio::spawn(async move {
        while let Some(ev) = incoming.recv().await {
            match ev {
                Incoming::Stream { header, mut stream } => {
                    seen.lock().unwrap().push(header.clone());
                    let (tls, waits, policy) = (tls.clone(), waits.clone(), policy.clone());
                    tokio::spawn(async move {
                        match header.kind {
                            StreamKind::KyberTcp if policy.kyber_tcp => {
                                let mut buf = vec![0u8; 16 * 1024];
                                while let Ok(n) = stream.read(&mut buf).await {
                                    if n == 0 || stream.write_all(&buf[..n]).await.is_err() {
                                        break;
                                    }
                                }
                                let _ = stream.shutdown().await;
                            }
                            StreamKind::KyberTcp => drop(stream),
                            StreamKind::AdminTls => {
                                if let Ok(tls) = tls.accept(stream).await {
                                    let svc = service_fn(move |req| fake_admin(req, waits.clone(), policy.clone()));
                                    let _ = hyper::server::conn::http1::Builder::new().serve_connection(TokioIo::new(tls), svc).await;
                                }
                            }
                            StreamKind::AdminHttp => {
                                let svc = service_fn(move |req| fake_admin(req, waits.clone(), policy.clone()));
                                let _ = hyper::server::conn::http1::Builder::new().serve_connection(TokioIo::new(stream), svc).with_upgrades().await;
                            }
                        }
                    });
                }
                Incoming::Flow(req) => {
                    if policy.flows {
                        req.accept("127.0.0.1:0".parse().unwrap(), udp_echo_addr).unwrap();
                    } else {
                        req.refuse("not_authorized");
                    }
                }
                Incoming::ClientGone { .. } => {}
            }
        }
    });
    FakeStation { agent, identity, admin_fp: admin.fp, headers, udp_echo: udp_echo_addr, tasks: vec![task, echo_task] }
}

impl FakeStation {
    pub fn id(&self) -> &str {
        self.identity.id()
    }

    pub async fn joined(&self) {
        let mut rx = self.agent.subscribe();
        tokio::time::timeout(std::time::Duration::from_secs(10), rx.wait_for(|s| s.is_joined())).await.expect("Station jointe").unwrap();
    }
}

pub async fn udp_echo_server() -> (SocketAddr, JoinHandle<()>) {
    udp_echo().await
}

/// Une réponse HTTP lue en entier.
pub struct Http {
    pub status: u16,
    pub headers: hyper::HeaderMap,
    pub body: String,
}

/// Une requête HTTPS au relais, avec ce nom (SNI et `Host`).
pub async fn https(relay: &Relay, roots: &Arc<RootCertStore>, name: &str, method: &str, path: &str, extra: &[(&str, &str)]) -> std::io::Result<Http> {
    let tcp = TcpStream::connect(relay.addr).await?;
    let mut cfg = rustls::ClientConfig::builder_with_provider(kybernow_tunnel::tls::provider())
        .with_safe_default_protocol_versions()
        .unwrap()
        .with_root_certificates(roots.clone())
        .with_no_client_auth();
    cfg.alpn_protocols = vec![b"http/1.1".to_vec()];
    let tls = tokio_rustls::TlsConnector::from(Arc::new(cfg)).connect(ServerName::try_from(name.to_string()).unwrap(), tcp).await?;
    request(tls, name, method, path, extra).await
}

pub async fn request<S>(io: S, host: &str, method: &str, path: &str, extra: &[(&str, &str)]) -> std::io::Result<Http>
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send + 'static,
{
    let (mut sender, conn) = hyper::client::conn::http1::handshake(TokioIo::new(io)).await.map_err(std::io::Error::other)?;
    tokio::spawn(conn);
    let mut b = Request::builder().method(method).uri(path).header("host", host);
    for (k, v) in extra {
        b = b.header(*k, *v);
    }
    let resp = sender.send_request(b.body(Full::new(Bytes::from_static(b"{}"))).unwrap()).await.map_err(std::io::Error::other)?;
    let status = resp.status().as_u16();
    let headers = resp.headers().clone();
    let body = resp.into_body().collect().await.map_err(std::io::Error::other)?.to_bytes();
    Ok(Http { status, headers, body: String::from_utf8_lossy(&body).into_owned() })
}

/// L'app vers Admin : TLS épinglé sur l'empreinte (le nom n'est pas vérifié).
#[derive(Debug)]
struct Pin(String);

impl ServerCertVerifier for Pin {
    fn verify_server_cert(&self, ee: &CertificateDer<'_>, _: &[CertificateDer<'_>], _: &ServerName<'_>, _: &[u8], _: UnixTime) -> Result<ServerCertVerified, rustls::Error> {
        if id::fingerprint(ee) == self.0 {
            Ok(ServerCertVerified::assertion())
        } else {
            Err(rustls::Error::General("empreinte".into()))
        }
    }

    fn verify_tls12_signature(&self, m: &[u8], c: &CertificateDer<'_>, d: &rustls::DigitallySignedStruct) -> Result<HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls12_signature(m, c, d, &kybernow_tunnel::tls::provider().signature_verification_algorithms)
    }

    fn verify_tls13_signature(&self, m: &[u8], c: &CertificateDer<'_>, d: &rustls::DigitallySignedStruct) -> Result<HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls13_signature(m, c, d, &kybernow_tunnel::tls::provider().signature_verification_algorithms)
    }

    fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> {
        kybernow_tunnel::tls::provider().signature_verification_algorithms.supported_schemes()
    }
}

/// Une requête à Admin par le point d'écoute local du tunnel, épinglée.
pub async fn admin_request(admin: SocketAddr, fp: &str, path: &str) -> std::io::Result<Http> {
    let tcp = TcpStream::connect(admin).await?;
    let cfg = rustls::ClientConfig::builder_with_provider(kybernow_tunnel::tls::provider())
        .with_safe_default_protocol_versions()
        .unwrap()
        .dangerous()
        .with_custom_certificate_verifier(Arc::new(Pin(fp.to_string())))
        .with_no_client_auth();
    let tls = tokio_rustls::TlsConnector::from(Arc::new(cfg)).connect(ServerName::try_from("127.0.0.1").unwrap(), tcp).await?;
    request(tls, "127.0.0.1", "POST", path, &[("content-type", "application/json")]).await
}
