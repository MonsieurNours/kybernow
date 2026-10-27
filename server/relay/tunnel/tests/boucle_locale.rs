//! Les transports du tunnel sur la boucle locale : QUIC (flux, datagrammes,
//! règle de taille, certificat client de la Station, relais épinglé) et le
//! repli TLS sur TCP avec yamux. Le relais entier, l'agent de la Station et
//! le tunnel des apps sont essayés ensemble dans `server/relay/tests`.

use std::net::SocketAddr;
use std::sync::Arc;

use bytes::Bytes;
use kybernow_tunnel::frame::{encode_datagram, read_msg, write_msg};
use kybernow_tunnel::msg::{Msg, Service};
use kybernow_tunnel::quic::{self, ALPN_CTL, ALPN_MEDIA, ALPN_TCP, CLIENT_KEEP_ALIVE, Kind};
use kybernow_tunnel::tls::{self, AnyClientCert, ClientAuth};
use kybernow_tunnel::transport::{Conn, DropReason, MediaReceiver, MediaSender, Sent, YamuxHandle, media_over_stream};
use kybernow_tunnel::{RelayAddr, StationIdentity, id};
use rustls::RootCertStore;
use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio_util::compat::TokioAsyncReadCompatExt;

struct Pki {
    roots: Arc<RootCertStore>,
    chain: Vec<CertificateDer<'static>>,
    key: Vec<u8>,
}

impl Pki {
    fn new() -> Pki {
        let ca_key = rcgen::KeyPair::generate().unwrap();
        let mut ca_params = rcgen::CertificateParams::new(Vec::<String>::new()).unwrap();
        ca_params.is_ca = rcgen::IsCa::Ca(rcgen::BasicConstraints::Unconstrained);
        ca_params.distinguished_name.push(rcgen::DnType::CommonName, "Autorite d'essai");
        let ca = ca_params.self_signed(&ca_key).unwrap();
        let key = rcgen::KeyPair::generate().unwrap();
        let leaf = rcgen::CertificateParams::new(vec!["relay.test".into()]).unwrap().signed_by(&key, &ca, &ca_key).unwrap();
        let mut roots = RootCertStore::empty();
        roots.add(ca.der().clone()).unwrap();
        Pki { roots: Arc::new(roots), chain: vec![leaf.der().clone()], key: key.serialize_der() }
    }

    fn server(&self, alpn: &[&[u8]]) -> rustls::ServerConfig {
        let mut sc = rustls::ServerConfig::builder_with_provider(tls::provider())
            .with_protocol_versions(&[&rustls::version::TLS13])
            .unwrap()
            .with_client_cert_verifier(AnyClientCert::new())
            .with_single_cert(self.chain.clone(), PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(self.key.clone())))
            .unwrap();
        sc.alpn_protocols = alpn.iter().map(|a| a.to_vec()).collect();
        sc
    }
}

fn relay(port: u16) -> RelayAddr {
    RelayAddr { host: "relay.test".into(), port, pin: None }
}

async fn quic_server(pki: &Pki) -> (quinn::Endpoint, quinn::ServerConfig) {
    let qsc = quinn::crypto::rustls::QuicServerConfig::try_from(pki.server(&[ALPN_CTL, ALPN_MEDIA])).unwrap();
    let base = quinn::ServerConfig::with_crypto(Arc::new(qsc));
    let ep = quinn::Endpoint::server(base.clone(), "127.0.0.1:0".parse().unwrap()).unwrap();
    (ep, base)
}

async fn accept(ep: &quinn::Endpoint, base: &quinn::ServerConfig) -> (quinn::Connection, kybernow_tunnel::congestion::CcMode) {
    let incoming = ep.accept().await.unwrap();
    let (cfg, mode) = quic::incoming_config(base);
    (incoming.accept_with(Arc::new(cfg)).unwrap().await.unwrap(), mode)
}

async fn connect(relay: &RelayAddr, addr: SocketAddr, roots: Option<Arc<RootCertStore>>, alpn: &[u8], kind: Kind, auth: Option<&ClientAuth>) -> Result<quinn::Connection, String> {
    let tls = tls::client_config(relay, roots, alpn, auth).map_err(|e| e.to_string())?;
    let cfg = quic::client_config(tls, kind, CLIENT_KEEP_ALIVE)?;
    let ep = quic::client_endpoint(addr).map_err(|e| e.to_string())?;
    ep.connect_with(cfg, addr, &relay.host).map_err(|e| e.to_string())?.await.map_err(|e| e.to_string())
}

#[tokio::test]
async fn quic_flux_messages_et_octets() {
    let pki = Pki::new();
    let (ep, base) = quic_server(&pki).await;
    let addr = ep.local_addr().unwrap();
    let server = tokio::spawn(async move {
        let (c, _) = accept(&ep, &base).await;
        assert_eq!(quic::alpn(&c).as_deref(), Some(ALPN_CTL));
        assert_eq!(quic::peer_station_id(&c), None, "une app n'a pas de certificat");
        let conn = Conn::Quic(c);
        let mut s = conn.accept().await.unwrap();
        let open = read_msg(&mut s).await.unwrap().unwrap();
        assert_eq!(open, Msg::Open { service: Service::AdminTls });
        // Écho jusqu'à la fermeture du côté client (demi-fermeture).
        let mut data = Vec::new();
        s.read_to_end(&mut data).await.unwrap();
        s.write_all(&data).await.unwrap();
        s.shutdown().await.unwrap();
        conn.closed().await;
    });
    let c = connect(&relay(addr.port()), addr, Some(pki.roots.clone()), ALPN_CTL, Kind::Ctl, None).await.unwrap();
    let conn = Conn::Quic(c);
    let mut s = conn.open().await.unwrap();
    write_msg(&mut s, &Msg::Open { service: Service::AdminTls }).await.unwrap();
    let big = vec![7u8; 300_000];
    s.write_all(&big).await.unwrap();
    s.shutdown().await.unwrap();
    let mut back = Vec::new();
    s.read_to_end(&mut back).await.unwrap();
    assert_eq!(back, big);
    assert!(conn.rtt().is_some());
    conn.close("fin");
    let _ = server.await;
}

#[tokio::test]
async fn quic_datagrammes_et_regle_de_taille() {
    let pki = Pki::new();
    let (ep, base) = quic_server(&pki).await;
    let addr = ep.local_addr().unwrap();
    let server = tokio::spawn(async move {
        let (c, mode) = accept(&ep, &base).await;
        assert_eq!(quic::alpn(&c).as_deref(), Some(ALPN_MEDIA));
        mode.set_permissive();
        // Renvoie chaque paquet sur le flux suivant ; les gros paquets de
        // l'essai sont annoncés comme « petits » pour passer par le secours.
        let tx = MediaSender::quic(c.clone());
        let mut rx = MediaReceiver::quic(c.clone());
        while let Some((flow, payload)) = rx.recv().await {
            tx.send_datagram(encode_datagram(flow + 1, &payload), payload.len().min(1200));
        }
    });
    let c = connect(&relay(addr.port()), addr, Some(pki.roots.clone()), ALPN_MEDIA, Kind::Media, None).await.unwrap();
    let tx = MediaSender::quic(c.clone());
    let mut rx = MediaReceiver::quic(c.clone());
    assert!(tx.max_datagram_size().unwrap() >= 1202, "un paquet de 1200 octets tient au départ");
    assert_eq!(tx.send(1, b"bonjour"), Sent::Datagram);
    let (flow, payload) = rx.recv().await.unwrap();
    assert_eq!((flow, &payload[..]), (2, &b"bonjour"[..]));
    assert_eq!(tx.send(300, &[1u8; 1200]), Sent::Datagram);
    let (flow, payload) = rx.recv().await.unwrap();
    assert_eq!((flow, payload.len()), (301, 1200));
    // Plus grand que le plafond de la recherche de MTU : un paquet de 1200
    // octets au plus part dans un flux de secours, plus grand il est jeté.
    let huge = encode_datagram(9, &[3u8; 1500]);
    assert_eq!(tx.send_datagram(huge.clone(), 1200), Sent::Fallback);
    let (flow, payload) = rx.recv().await.unwrap();
    assert_eq!((flow, payload.len()), (10, 1500));
    assert_eq!(tx.send_datagram(huge, 1500), Sent::Dropped(DropReason::TooLarge));
    c.close(0u32.into(), b"fin");
    let _ = server.await;
}

#[tokio::test]
async fn station_reconnue_a_son_certificat_client() {
    let pki = Pki::new();
    let (ep, base) = quic_server(&pki).await;
    let addr = ep.local_addr().unwrap();
    let identity = StationIdentity::generate().unwrap();
    let expected = identity.id().to_string();
    let server = tokio::spawn(async move {
        let (c, _) = accept(&ep, &base).await;
        quic::peer_station_id(&c)
    });
    let c = connect(&relay(addr.port()), addr, Some(pki.roots.clone()), ALPN_CTL, Kind::Ctl, Some(&identity.client_auth())).await.unwrap();
    assert_eq!(server.await.unwrap().as_deref(), Some(expected.as_str()));
    c.close(0u32.into(), b"");
}

#[tokio::test]
async fn relais_epingle_par_son_empreinte() {
    let pki = Pki::new();
    let pin = id::fingerprint(&pki.chain[0]);
    let (ep, base) = quic_server(&pki).await;
    let addr = ep.local_addr().unwrap();
    tokio::spawn(async move {
        loop {
            let incoming = ep.accept().await.unwrap();
            let (cfg, _) = quic::incoming_config(&base);
            if let Ok(c) = incoming.accept_with(Arc::new(cfg)) {
                tokio::spawn(async move {
                    if let Ok(c) = c.await {
                        c.closed().await;
                    }
                });
            }
        }
    });
    // Nom faux, pas d'autorités : seule l'empreinte compte.
    let pinned: RelayAddr = format!("127.0.0.1:{}#{pin}", addr.port()).parse().unwrap();
    let c = connect(&pinned, addr, None, ALPN_CTL, Kind::Ctl, None).await.unwrap();
    c.close(0u32.into(), b"");
    let wrong: RelayAddr = format!("127.0.0.1:{}#{}", addr.port(), id::fingerprint(b"autre")).parse().unwrap();
    assert!(connect(&wrong, addr, None, ALPN_CTL, Kind::Ctl, None).await.is_err());
    // Sans empreinte ni autorité d'essai, l'autorité d'essai n'est pas reconnue.
    assert!(connect(&relay(addr.port()), addr, None, ALPN_CTL, Kind::Ctl, None).await.is_err());
}

#[tokio::test]
async fn repli_tls_sur_tcp_avec_yamux() {
    let pki = Pki::new();
    let identity = StationIdentity::generate().unwrap();
    let expected = identity.id().to_string();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let acceptor = tokio_rustls::TlsAcceptor::from(Arc::new(pki.server(&[ALPN_TCP])));
    let server = tokio::spawn(async move {
        let (tcp, peer) = listener.accept().await.unwrap();
        let tls = acceptor.accept(tcp).await.unwrap();
        let (_, session) = tls.get_ref();
        assert_eq!(session.alpn_protocol(), Some(ALPN_TCP));
        let station = session.peer_certificates().and_then(|c| id::station_id_of_cert(c.first()?));
        let conn = Conn::Yamux(YamuxHandle::start(tls.compat(), yamux::Mode::Server, peer));
        // Premier flux : les messages ; deuxième : les paquets.
        let mut ctl = conn.accept().await.unwrap();
        let hello = read_msg(&mut ctl).await.unwrap().unwrap();
        write_msg(&mut ctl, &hello).await.unwrap();
        let mut media = conn.accept().await.unwrap();
        assert_eq!(read_msg(&mut media).await.unwrap(), Some(Msg::Media { token: None }));
        let (tx, mut rx) = media_over_stream(media);
        let (flow, payload) = rx.recv().await.unwrap();
        assert_eq!(tx.send(flow * 2, &payload), Sent::Datagram);
        // Le relais ouvre aussi des flux vers la Station.
        let mut s = conn.open().await.unwrap();
        write_msg(&mut s, &Msg::Ping { n: 5 }).await.unwrap();
        let mut rest = Vec::new();
        s.read_to_end(&mut rest).await.unwrap();
        assert_eq!(rest, b"pong");
        conn.closed().await;
        station
    });
    let tcp = tokio::net::TcpStream::connect(addr).await.unwrap();
    let cfg = tls::client_config(&relay(addr.port()), Some(pki.roots.clone()), ALPN_TCP, Some(&identity.client_auth())).unwrap();
    let tls = tokio_rustls::TlsConnector::from(Arc::new(cfg)).connect(tls::server_name(&relay(addr.port())).unwrap(), tcp).await.unwrap();
    let conn = Conn::Yamux(YamuxHandle::start(tls.compat(), yamux::Mode::Client, addr));
    let mut ctl = conn.open().await.unwrap();
    write_msg(&mut ctl, &Msg::Ping { n: 1 }).await.unwrap();
    assert_eq!(read_msg(&mut ctl).await.unwrap(), Some(Msg::Ping { n: 1 }));
    let mut media = conn.open().await.unwrap();
    write_msg(&mut media, &Msg::Media { token: None }).await.unwrap();
    let (tx, mut rx) = media_over_stream(media);
    assert_eq!(tx.send(21, b"paquet"), Sent::Datagram);
    let (flow, payload) = rx.recv().await.unwrap();
    assert_eq!((flow, payload), (42, Bytes::from_static(b"paquet")));
    let mut s = conn.accept().await.unwrap();
    assert_eq!(read_msg(&mut s).await.unwrap(), Some(Msg::Ping { n: 5 }));
    s.write_all(b"pong").await.unwrap();
    s.shutdown().await.unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    conn.close("fin");
    assert_eq!(server.await.unwrap().as_deref(), Some(expected.as_str()));
}
