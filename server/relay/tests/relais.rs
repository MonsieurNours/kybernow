//! Le relais entier sur la boucle locale : une fausse Station (agent du
//! tunnel), une fausse app (tunnel et points d'écoute locaux), des échos TCP
//! et UDP, le repli TCP, les autorisations, les codes et l'attente avec un
//! faux Admin, la page du relais et le mandataire du navigateur.

mod common;

use std::time::Duration;

use common::*;
use kybernow_tunnel::client::{ClientConfig, Tunnel};
use kybernow_tunnel::lookup::CodeLookup;
use kybernow_tunnel::{StreamKind, Transport, TunnelError};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpStream, UdpSocket};
use tokio::time::timeout;

const T: Duration = Duration::from_secs(20);

fn client_cfg(relay: &kybernow_relay::Relay, pki: &Pki, station: &str, force_tcp: bool) -> ClientConfig {
    ClientConfig { relay: relay_addr(relay), station: station.into(), roots: Some(pki.roots.clone()), force_tcp, version: None }
}

/// Écho TCP puis UDP par le point d'écoute Kyber du tunnel (même port).
async fn echo_through(tunnel: &Tunnel) {
    let kyber = tunnel.listen_kyber().await.unwrap();
    let mut tcp = TcpStream::connect(kyber.addr()).await.unwrap();
    let big: Vec<u8> = (0..200_000u32).map(|i| (i % 251) as u8).collect();
    tcp.write_all(&big).await.unwrap();
    let mut back = vec![0u8; big.len()];
    timeout(T, tcp.read_exact(&mut back)).await.expect("écho TCP").unwrap();
    assert_eq!(back, big);
    let udp = UdpSocket::bind("127.0.0.1:0").await.unwrap();
    udp.connect(kyber.addr()).await.unwrap();
    for i in 0..20u8 {
        let packet = vec![i; 1200];
        udp.send(&packet).await.unwrap();
        let mut buf = [0u8; 2048];
        let n = timeout(T, udp.recv(&mut buf)).await.expect("écho UDP").unwrap();
        assert_eq!(&buf[..n], &packet[..]);
    }
}

async fn tcp_et_udp(station_tcp: bool, client_tcp: bool) {
    let pki = pki();
    let relay = relay(&pki, |_| {}).await;
    let station = fake_station(relay_addr(&relay), pki.roots.clone(), station_tcp, Policy::default());
    station.joined().await;
    assert_eq!(station.agent.transport(), Some(if station_tcp { Transport::Tcp } else { Transport::Quic }));
    let tunnel = Tunnel::open(client_cfg(&relay, &pki, station.id(), client_tcp)).await.unwrap();
    assert!(tunnel.wait_station(T).await);
    echo_through(&tunnel).await;
    let stats = tunnel.stats();
    assert!(stats.connected && stats.station_online);
    assert_eq!(stats.transport, Some(if client_tcp { Transport::Tcp } else { Transport::Quic }));
    // L'en-tête vient du relais : l'identifiant du client et son adresse.
    let headers = station.headers.lock().unwrap().clone();
    let h = headers.iter().find(|h| h.kind == StreamKind::KyberTcp).expect("flux Kyber");
    assert_eq!(h.client, tunnel.client_id());
    assert_eq!(h.ip.to_string(), "127.0.0.1");
    let metrics = relay.hub.metrics.render(&relay.hub.gauges());
    assert!(metrics.contains("relay_datagrams_total{result=\"forwarded\"}"));
    assert!(!metrics.contains("relay_datagrams_total{result=\"forwarded\"} 0"), "{metrics}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn tcp_et_udp_par_quic() {
    timeout(T * 2, tcp_et_udp(false, false)).await.expect("délai");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn tcp_et_udp_par_le_repli_tcp() {
    timeout(T * 2, tcp_et_udp(true, true)).await.expect("délai");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn station_en_tcp_app_en_quic() {
    timeout(T * 2, tcp_et_udp(true, false)).await.expect("délai");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn autorisations_refusees_par_la_station() {
    timeout(T * 2, async {
        let pki = pki();
        let relay = relay(&pki, |_| {}).await;
        let policy = Policy { flows: false, kyber_tcp: false, ..Policy::default() };
        let station = fake_station(relay_addr(&relay), pki.roots.clone(), false, policy);
        station.joined().await;
        let tunnel = Tunnel::open(client_cfg(&relay, &pki, station.id(), false)).await.unwrap();
        assert!(tunnel.wait_station(T).await);
        match tunnel.open_flow(1).await {
            Err(TunnelError::FlowRefused(why)) => assert_eq!(why, "not_authorized"),
            other => panic!("flux accepté : {:?}", other.err()),
        }
        // Le flux TCP vers Kyber est lâché par la Station : fermé chez l'app.
        let kyber = tunnel.listen_kyber().await.unwrap();
        let mut tcp = TcpStream::connect(kyber.addr()).await.unwrap();
        let _ = tcp.write_all(b"bonjour").await;
        let mut buf = [0u8; 16];
        let n = timeout(T, tcp.read(&mut buf)).await.expect("fermeture").unwrap_or(0);
        assert_eq!(n, 0);
    })
    .await
    .expect("délai");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn station_absente_puis_connectee() {
    timeout(T * 2, async {
        let pki = pki();
        let relay = relay(&pki, |_| {}).await;
        let identity = std::sync::Arc::new(kybernow_tunnel::StationIdentity::generate().unwrap());
        let tunnel = Tunnel::open(client_cfg(&relay, &pki, identity.id(), false)).await.unwrap();
        assert!(!tunnel.station_online());
        assert_eq!(tunnel.open_stream(kybernow_tunnel::Service::AdminTls).await.err(), Some(TunnelError::StationOffline));
        assert_eq!(tunnel.presence(&[identity.id().to_string()]).await.unwrap(), vec![false]);
        let station = fake_station_with(identity.clone(), relay_addr(&relay), pki.roots.clone(), false, Policy::default());
        station.joined().await;
        assert!(tunnel.wait_station(T).await, "le relais prévient l'app");
        assert_eq!(tunnel.presence(&[identity.id().to_string()]).await.unwrap(), vec![true]);
        let alone = kybernow_tunnel::client::presence(&relay_addr(&relay), Some(pki.roots.clone()), &[identity.id().to_string()]).await.unwrap();
        assert_eq!(alone, vec![true]);
        echo_through(&tunnel).await;
    })
    .await
    .expect("délai");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn reconnexion_de_la_station() {
    timeout(T * 3, async {
        let pki = pki();
        let relay = relay(&pki, |_| {}).await;
        let identity = std::sync::Arc::new(kybernow_tunnel::StationIdentity::generate().unwrap());
        let station = fake_station_with(identity.clone(), relay_addr(&relay), pki.roots.clone(), false, Policy::default());
        station.joined().await;
        let code = station.agent.reserve(Some("424242")).await.unwrap();
        assert_eq!(code, "424242");
        let other = fake_station(relay_addr(&relay), pki.roots.clone(), false, Policy::default());
        other.joined().await;
        let tunnel = Tunnel::open(client_cfg(&relay, &pki, identity.id(), false)).await.unwrap();
        assert!(tunnel.wait_station(T).await);
        let kyber = tunnel.listen_kyber().await.unwrap();
        let udp = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        udp.connect(kyber.addr()).await.unwrap();
        let mut buf = [0u8; 64];
        udp.send(b"avant").await.unwrap();
        let n = timeout(T, udp.recv(&mut buf)).await.unwrap().unwrap();
        assert_eq!(&buf[..n], b"avant");
        // La Station part (Admin redémarre) puis revient avec la même clé.
        drop(station);
        let deadline = tokio::time::Instant::now() + T;
        while tunnel.station_online() {
            assert!(tokio::time::Instant::now() < deadline, "le relais n'a pas vu partir la Station");
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        // Pendant la coupure, le code reste à la Station partie (120 s).
        assert_ne!(other.agent.reserve(Some("424242")).await.unwrap(), "424242");
        let station = fake_station_with(identity.clone(), relay_addr(&relay), pki.roots.clone(), false, Policy::default());
        station.joined().await;
        assert!(tunnel.wait_station(T).await);
        // Le même port local : le flux est rouvert de lui-même.
        let mut got = None;
        for _ in 0..50 {
            udp.send(b"apres").await.unwrap();
            if let Ok(Ok(n)) = timeout(Duration::from_millis(200), udp.recv(&mut buf)).await {
                got = Some(buf[..n].to_vec());
                break;
            }
        }
        assert_eq!(got.as_deref(), Some(&b"apres"[..]));
        // Revenue, elle reprend son code.
        assert_eq!(station.agent.reserve(Some("424242")).await.unwrap(), "424242");
    })
    .await
    .expect("délai");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn codes_puis_attente_avec_un_faux_admin() {
    timeout(T * 2, async {
        let pki = pki();
        let relay = relay(&pki, |_| {}).await;
        let station = fake_station(relay_addr(&relay), pki.roots.clone(), false, Policy::default());
        station.joined().await;
        let code = station.agent.reserve(None).await.unwrap();
        // Une autre Station ne prend pas ce code.
        let other = fake_station(relay_addr(&relay), pki.roots.clone(), false, Policy::default());
        other.joined().await;
        assert_ne!(other.agent.reserve(Some(&code)).await.unwrap(), code);
        // L'app cherche le code chez le relais.
        let r = https(&relay, &pki.roots, DOMAIN, "GET", &format!("/v1/codes/{code}"), &[]).await.unwrap();
        assert_eq!(r.status, 200, "{}", r.body);
        let found: CodeLookup = serde_json::from_str(&r.body).unwrap();
        assert_eq!(found.station, station.id());
        assert_eq!(found.admin_fp, station.admin_fp);
        assert!(found.online && found.verify(), "clé et signature de la Station");
        // Tunnel, puis join et wait de bout en bout, Admin épinglé.
        let tunnel = Tunnel::open(client_cfg(&relay, &pki, &found.station, false)).await.unwrap();
        assert!(tunnel.wait_station(T).await);
        let admin = tunnel.listen_admin().await.unwrap();
        let join = admin_request(admin.addr(), &found.admin_fp, "/api/share/join").await.unwrap();
        assert_eq!(join.status, 202);
        assert!(join.body.contains("\"pending\":true"));
        let wait = admin_request(admin.addr(), &found.admin_fp, "/api/share/wait").await.unwrap();
        assert_eq!(wait.status, 202);
        let wait = admin_request(admin.addr(), &found.admin_fp, "/api/share/wait").await.unwrap();
        assert_eq!(wait.status, 200);
        assert!(wait.body.contains("\"relay\":true"));
        // Une empreinte fausse : le TLS de bout en bout refuse.
        assert!(admin_request(admin.addr(), &kybernow_tunnel::id::fingerprint(b"autre"), "/api/share/join").await.is_err());
        let h = station.headers.lock().unwrap().iter().find(|h| h.kind == StreamKind::AdminTls).cloned().unwrap();
        assert_eq!(h.client, tunnel.client_id());
        // Le code libéré n'est plus trouvé.
        station.agent.release(&code);
        tokio::time::sleep(Duration::from_millis(200)).await;
        let r = https(&relay, &pki.roots, DOMAIN, "GET", &format!("/v1/codes/{code}"), &[]).await.unwrap();
        assert_eq!(r.status, 404);
        assert!(r.body.contains("unknown_code"));
    })
    .await
    .expect("délai");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn attente_refusee() {
    timeout(T * 2, async {
        let pki = pki();
        let relay = relay(&pki, |_| {}).await;
        let station = fake_station(relay_addr(&relay), pki.roots.clone(), true, Policy { wait_status: 403, ..Policy::default() });
        station.joined().await;
        let tunnel = Tunnel::open(client_cfg(&relay, &pki, station.id(), true)).await.unwrap();
        assert!(tunnel.wait_station(T).await);
        let admin = tunnel.listen_admin().await.unwrap();
        assert_eq!(admin_request(admin.addr(), &station.admin_fp, "/api/share/join").await.unwrap().status, 202);
        assert_eq!(admin_request(admin.addr(), &station.admin_fp, "/api/share/wait").await.unwrap().status, 202);
        let refused = admin_request(admin.addr(), &station.admin_fp, "/api/share/wait").await.unwrap();
        assert_eq!(refused.status, 403);
        assert!(refused.body.contains("refused"));
    })
    .await
    .expect("délai");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn pages_du_relais() {
    timeout(T * 2, async {
        let pki = pki();
        let relay = relay(&pki, |_| {}).await;
        let station = fake_station(relay_addr(&relay), pki.roots.clone(), false, Policy::default());
        station.joined().await;
        let code = station.agent.reserve(Some("135790")).await.unwrap();
        let home = https(&relay, &pki.roots, DOMAIN, "GET", "/", &[("accept-language", "fr-FR,fr;q=0.9")]).await.unwrap();
        assert_eq!(home.status, 200);
        assert!(home.body.contains("Découvrir KyberNow"));
        assert!(home.body.contains("Rejoindre avec un code"));
        assert!(home.body.contains(r#"<form method="get" action="/j">"#));
        let home_en = https(&relay, &pki.roots, DOMAIN, "GET", "/", &[("accept-language", "en-GB")]).await.unwrap();
        assert!(home_en.body.contains("Discover KyberNow"));
        assert!(home_en.body.contains("Join with a code"));
        // Le choix du visiteur (?lang=) passe avant le navigateur ; les réponses disent qu'elles varient.
        let chosen = https(&relay, &pki.roots, DOMAIN, "GET", "/?lang=en", &[("accept-language", "fr-FR")]).await.unwrap();
        assert!(chosen.body.contains("Join with a code") && chosen.body.contains(r#"<html lang="en">"#));
        assert_eq!(home.headers.get("vary").unwrap(), "accept-language");
        assert_eq!(home.headers.get("cache-control").unwrap(), "no-store");
        // Pas de cookie, pas de script : la politique de contenu ne laisse venir que le relais.
        assert!(home.headers.get("set-cookie").is_none());
        let policy = home.headers.get("content-security-policy").unwrap().to_str().unwrap();
        for part in ["default-src 'none'", "style-src 'self' 'unsafe-inline'", "font-src 'self'", "img-src 'self' data:", "form-action 'self'", "frame-ancestors 'none'"] {
            assert!(policy.contains(part), "politique : {part}");
        }
        assert!(!policy.contains("script-src"), "aucun script : default-src 'none' les refuse");
        assert!(!home.body.contains("<script"));
        let j = https(&relay, &pki.roots, DOMAIN, "GET", &format!("/j/{code}"), &[]).await.unwrap();
        assert_eq!(j.status, 302);
        let port = relay.addr.port();
        assert_eq!(
            j.headers.get("location").unwrap().to_str().unwrap(),
            format!("https://{}.{DOMAIN}:{port}/link/?join={code}", station.id())
        );
        let form = https(&relay, &pki.roots, DOMAIN, "GET", "/j?code=135+790", &[]).await.unwrap();
        assert_eq!(form.status, 302, "le formulaire accepte « 135 790 »");
        let unknown = https(&relay, &pki.roots, DOMAIN, "GET", "/j/000000", &[("accept-language", "fr")]).await.unwrap();
        assert_eq!(unknown.status, 404);
        assert!(unknown.body.contains("Aucun partage avec ce code"));
        let bad = https(&relay, &pki.roots, DOMAIN, "GET", "/v1/codes/12a456", &[]).await.unwrap();
        assert_eq!(bad.status, 400);
        let health = https(&relay, &pki.roots, DOMAIN, "GET", "/v1/health", &[]).await.unwrap();
        assert_eq!(health.status, 200);
        assert!(health.body.contains("\"ok\":true"));
        assert_eq!(https(&relay, &pki.roots, DOMAIN, "GET", "/privacy", &[]).await.unwrap().status, 200);
        // Un nom qui n'est ni le relais ni une Station : connexion fermée.
        assert!(https(&relay, &pki.roots, &format!("www.{DOMAIN}"), "GET", "/", &[]).await.is_err());
        // Les métriques ne contiennent jamais un code.
        let metrics = relay.hub.metrics.render(&relay.hub.gauges());
        assert!(!metrics.contains(&code));
        assert!(metrics.contains("relay_code_lookups_total{result=\"found\"}"));
        let addr = relay.metrics_addr.unwrap();
        assert!(kybernow_relay::metrics::healthcheck(addr));
    })
    .await
    .expect("délai");
}

/// Les fichiers que l'accueil cite (`/assets/…?v=…`) : servis par le relais, gardés un an.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn fichiers_de_la_page_d_accueil() {
    timeout(T * 2, async {
        let pki = pki();
        let relay = relay(&pki, |_| {}).await;
        let home = https(&relay, &pki.roots, DOMAIN, "GET", "/", &[]).await.unwrap();
        let mut urls: Vec<String> = home
            .body
            .split('"')
            .filter(|s| s.starts_with("/assets/"))
            .map(str::to_string)
            .collect();
        urls.sort();
        urls.dedup();
        assert_eq!(urls.len(), 4, "feuille de styles, police, logo et favicon : {urls:?}");
        for url in &urls {
            let name = url.trim_start_matches("/assets/").split('?').next().unwrap();
            let r = https(&relay, &pki.roots, DOMAIN, "GET", url, &[]).await.unwrap();
            assert_eq!(r.status, 200, "{url}");
            let asset = kybernow_relay::assets::get(name).unwrap();
            assert_eq!(r.headers.get("content-type").unwrap(), asset.mime, "{url}");
            assert_eq!(r.headers.get("cache-control").unwrap(), "public, max-age=31536000, immutable", "{url}");
            assert_eq!(r.headers.get("x-content-type-options").unwrap(), "nosniff");
            assert_eq!(r.headers.get("content-length").unwrap().to_str().unwrap(), asset.body.len().to_string(), "{url}");
            assert!(r.headers.get("set-cookie").is_none());
        }
        // La feuille de styles : les jetons de la charte, la mise en page, les polices du relais avec la version.
        let css_url = urls.iter().find(|u| u.starts_with("/assets/share.css")).unwrap();
        let css = https(&relay, &pki.roots, DOMAIN, "GET", css_url, &[]).await.unwrap();
        assert!(css.body.contains("--kn-signal") && css.body.contains(".hero"));
        assert!(css.body.contains(&format!("/assets/archivo.woff2?v={}", kybernow_relay::assets::version())));
        assert!(css.headers.get("content-type").unwrap().to_str().unwrap().starts_with("text/css"));
        // Les polices sont du woff2.
        let font = https(&relay, &pki.roots, DOMAIN, "GET", "/assets/martian-mono.woff2", &[]).await.unwrap();
        assert_eq!(font.headers.get("content-type").unwrap(), "font/woff2");
        assert!(font.body.starts_with("wOF2"));
        // Un fichier qui n'existe pas, ou un chemin qui sort du dossier : la page « introuvable », sans fichier.
        for path in ["/assets/inconnu.css", "/assets/", "/assets/../src/web.rs", "/assets/share.css/x"] {
            let r = https(&relay, &pki.roots, DOMAIN, "GET", path, &[("accept-language", "fr")]).await.unwrap();
            assert_eq!(r.status, 404, "{path}");
            assert!(r.body.contains("Page introuvable"), "{path}");
        }
        // Les fichiers se lisent aussi par HEAD, jamais par POST.
        let head = https(&relay, &pki.roots, DOMAIN, "HEAD", css_url, &[]).await.unwrap();
        assert_eq!(head.status, 200);
        let post = https(&relay, &pki.roots, DOMAIN, "POST", css_url, &[]).await.unwrap();
        assert_eq!(post.status, 405);
        // Les autres pages ont la même politique et le même style.
        for path in ["/privacy", "/j/000000", "/j?code=12", "/rien"] {
            let r = https(&relay, &pki.roots, DOMAIN, "GET", path, &[]).await.unwrap();
            assert!(r.body.contains("/assets/share.css?v="), "{path}");
            assert_eq!(r.headers.get("content-security-policy").unwrap().to_str().unwrap(), kybernow_relay::web::PAGE_POLICY, "{path}");
        }
        // Ni les métriques ni la santé n'ont de politique de page : ce sont des réponses JSON.
        let health = https(&relay, &pki.roots, DOMAIN, "GET", "/v1/health", &[]).await.unwrap();
        assert!(health.headers.get("content-security-policy").is_none());
    })
    .await
    .expect("délai");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn mandataire_du_navigateur() {
    timeout(T * 2, async {
        let pki = pki();
        let relay = relay(&pki, |_| {}).await;
        let station = fake_station(relay_addr(&relay), pki.roots.clone(), false, Policy::default());
        station.joined().await;
        let host = format!("{}.{DOMAIN}", station.id());
        let r = https(&relay, &pki.roots, &host, "GET", "/link/?join=123456", &[("x-forwarded-for", "6.6.6.6"), ("forwarded", "for=6.6.6.6")])
            .await
            .unwrap();
        assert_eq!(r.status, 200, "{}", r.body);
        let echoed: serde_json::Value = serde_json::from_str(&r.body).unwrap();
        assert_eq!(echoed["path"], "/link/");
        assert_eq!(echoed["headers"]["host"], host, "Host passé tel quel");
        assert!(echoed["headers"].get("x-forwarded-for").is_none());
        assert!(echoed["headers"].get("forwarded").is_none());
        let cookies: Vec<&str> = r.headers.get_all("set-cookie").iter().map(|v| v.to_str().unwrap()).collect();
        assert_eq!(cookies.len(), 2);
        assert!(cookies.iter().all(|c| !c.to_ascii_lowercase().contains("domain=")), "{cookies:?}");
        let h = station.headers.lock().unwrap().iter().find(|h| h.kind == StreamKind::AdminHttp).cloned().unwrap();
        assert_eq!(h.host.as_deref(), Some(host.as_str()));
        assert_eq!(h.ip.to_string(), "127.0.0.1");
        assert_eq!(h.client.len(), 16);
        // WebSocket : la connexion passe en écho après 101.
        let tcp = TcpStream::connect(relay.addr).await.unwrap();
        let mut cfg = rustls::ClientConfig::builder_with_provider(kybernow_tunnel::tls::provider())
            .with_safe_default_protocol_versions()
            .unwrap()
            .with_root_certificates(pki.roots.clone())
            .with_no_client_auth();
        cfg.alpn_protocols = vec![b"http/1.1".to_vec()];
        let mut tls = tokio_rustls::TlsConnector::from(std::sync::Arc::new(cfg))
            .connect(rustls::pki_types::ServerName::try_from(host.clone()).unwrap(), tcp)
            .await
            .unwrap();
        let req = format!(
            "GET /ws HTTP/1.1\r\nHost: {host}\r\nConnection: Upgrade\r\nUpgrade: websocket\r\nSec-WebSocket-Version: 13\r\nSec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\n\r\n"
        );
        tls.write_all(req.as_bytes()).await.unwrap();
        let mut head = Vec::new();
        let mut byte = [0u8; 1];
        while !head.ends_with(b"\r\n\r\n") {
            timeout(T, tls.read_exact(&mut byte)).await.expect("réponse 101").unwrap();
            head.push(byte[0]);
        }
        assert!(String::from_utf8_lossy(&head).starts_with("HTTP/1.1 101"), "{}", String::from_utf8_lossy(&head));
        tls.write_all(b"trame").await.unwrap();
        let mut back = [0u8; 5];
        timeout(T, tls.read_exact(&mut back)).await.expect("écho WebSocket").unwrap();
        assert_eq!(&back, b"trame");
        // Une Station absente : la page « ordinateur injoignable ».
        let absent = format!("{}.{DOMAIN}", kybernow_tunnel::id::station_id(b"absente"));
        let r = https(&relay, &pki.roots, &absent, "GET", "/link/", &[("accept-language", "fr")]).await.unwrap();
        assert_eq!(r.status, 503);
        assert!(r.body.contains("éteint ou en veille"));
    })
    .await
    .expect("délai");
}

/// UDP perdu en route (un relais joignable seulement en TCP) : après 3 s
/// sans réponse de QUIC, la Station et l'app passent au repli TCP.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn repli_tcp_quand_l_udp_ne_passe_pas() {
    timeout(T * 2, async {
        let pki = pki();
        let relay = relay(&pki, |_| {}).await;
        // Un faux relais : TCP vers le vrai, UDP avalé sans réponse.
        let tcp = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = tcp.local_addr().unwrap().port();
        let _udp_sink = std::net::UdpSocket::bind(("127.0.0.1", port)).unwrap();
        let target = relay.addr;
        tokio::spawn(async move {
            while let Ok((mut a, _)) = tcp.accept().await {
                tokio::spawn(async move {
                    if let Ok(mut b) = TcpStream::connect(target).await {
                        let _ = tokio::io::copy_bidirectional(&mut a, &mut b).await;
                    }
                });
            }
        });
        let addr = kybernow_tunnel::RelayAddr { host: "localhost".into(), port, pin: None };
        let station = fake_station(addr.clone(), pki.roots.clone(), false, Policy::default());
        station.joined().await;
        assert_eq!(station.agent.transport(), Some(Transport::Tcp));
        let cfg = ClientConfig { relay: addr, station: station.id().into(), roots: Some(pki.roots.clone()), force_tcp: false, version: None };
        let tunnel = Tunnel::open(cfg).await.unwrap();
        assert!(tunnel.wait_station(T).await);
        assert_eq!(tunnel.stats().transport, Some(Transport::Tcp));
        echo_through(&tunnel).await;
    })
    .await
    .expect("délai");
}

/// Le relais redémarre : la Station et l'app se reconnectent d'elles-mêmes,
/// et le même port local de Kyber sert de nouveau.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn le_relais_redemarre() {
    timeout(T * 3, async {
        let pki = pki();
        let first = relay(&pki, |_| {}).await;
        let listen = first.addr;
        let station = fake_station(relay_addr(&first), pki.roots.clone(), false, Policy::default());
        station.joined().await;
        let tunnel = Tunnel::open(client_cfg(&first, &pki, station.id(), false)).await.unwrap();
        assert!(tunnel.wait_station(T).await);
        let kyber = tunnel.listen_kyber().await.unwrap();
        let udp = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        udp.connect(kyber.addr()).await.unwrap();
        let mut buf = [0u8; 64];
        udp.send(b"un").await.unwrap();
        let n = timeout(T, udp.recv(&mut buf)).await.unwrap().unwrap();
        assert_eq!(&buf[..n], b"un");
        drop(first);
        // Même port : on attend qu'il se libère.
        let second = loop {
            let mut cfg = kybernow_relay::config::Config {
                domain: DOMAIN.into(),
                listen,
                cert: pki.cert.clone(),
                key: pki.key.clone(),
                ..Default::default()
            };
            cfg.metrics.listen = None;
            match kybernow_relay::start(cfg).await {
                Ok(r) => break r,
                Err(_) => tokio::time::sleep(Duration::from_millis(100)).await,
            }
        };
        station.joined().await;
        let mut got = None;
        for _ in 0..150 {
            udp.send(b"deux").await.unwrap();
            if let Ok(Ok(n)) = timeout(Duration::from_millis(200), udp.recv(&mut buf)).await {
                got = Some(buf[..n].to_vec());
                break;
            }
        }
        assert_eq!(got.as_deref(), Some(&b"deux"[..]));
        assert!(tunnel.stats().connected);
        drop(second);
    })
    .await
    .expect("délai");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn recherches_de_code_limitees() {
    timeout(T * 2, async {
        let pki = pki();
        let relay = relay(&pki, |_| {}).await;
        for _ in 0..10 {
            assert_eq!(https(&relay, &pki.roots, DOMAIN, "GET", "/v1/codes/111111", &[]).await.unwrap().status, 404);
        }
        let r = https(&relay, &pki.roots, DOMAIN, "GET", "/v1/codes/111111", &[]).await.unwrap();
        assert_eq!(r.status, 429);
        assert!(r.body.contains("too_many"));
        assert!(r.headers.get("retry-after").is_some());
        let page = https(&relay, &pki.roots, DOMAIN, "GET", "/j/111111", &[("accept-language", "fr")]).await.unwrap();
        assert_eq!(page.status, 429);
        assert!(page.body.contains("Trop d'essais de code"));
    })
    .await
    .expect("délai");
}

/// Un client occupé : une connexion vers Kyber relayée, gardée ouverte
/// (l'écho a répondu : le relais la compte).
async fn busy(tunnel: &Tunnel) -> (kybernow_tunnel::client::LocalListener, TcpStream) {
    let kyber = tunnel.listen_kyber().await.unwrap();
    let mut tcp = TcpStream::connect(kyber.addr()).await.unwrap();
    tcp.write_all(b"x").await.unwrap();
    let mut one = [0u8; 1];
    timeout(T, tcp.read_exact(&mut one)).await.expect("écho").unwrap();
    (kyber, tcp)
}

fn limit_hits(relay: &kybernow_relay::Relay, limit: &str) -> u64 {
    let metrics = relay.hub.metrics.render(&relay.hub.gauges());
    let key = format!("relay_limit_hits_total{{limit=\"{limit}\"}} ");
    metrics.lines().find_map(|l| l.strip_prefix(&key)).and_then(|n| n.trim().parse().ok()).unwrap_or(0)
}

fn too_many(r: Result<Tunnel, TunnelError>) {
    match r {
        Err(TunnelError::Refused(code, _)) => assert_eq!(code, kybernow_tunnel::msg::ErrorCode::TooMany),
        other => panic!("client accepté : {:?}", other.err()),
    }
}

/// Station pleine : le plus ancien client inactif cède sa place ; si tous
/// sont occupés, le nouveau est refusé (`too_many`).
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn station_pleine_le_plus_ancien_inactif_cede_sa_place() {
    timeout(T * 2, async {
        let pki = pki();
        let relay = relay(&pki, |c| {
            c.limits.clients_per_station = 2;
            c.limits.clients_per_station_per_ip = 8;
        })
        .await;
        let station = fake_station(relay_addr(&relay), pki.roots.clone(), false, Policy::default());
        station.joined().await;
        let a = Tunnel::open(client_cfg(&relay, &pki, station.id(), false)).await.unwrap();
        let _a_busy = busy(&a).await;
        let b = Tunnel::open(client_cfg(&relay, &pki, station.id(), false)).await.unwrap();
        // Un troisième : b, inactif, lui cède sa place ; a, occupé, la garde.
        let c = Tunnel::open(client_cfg(&relay, &pki, station.id(), false)).await.unwrap();
        assert_eq!(limit_hits(&relay, "client_yielded"), 1);
        b.close();
        let _c_busy = busy(&c).await;
        assert!(a.stats().connected);
        // Tous occupés : refusé.
        too_many(Tunnel::open(client_cfg(&relay, &pki, station.id(), false)).await);
        assert!(limit_hits(&relay, "clients_per_station") >= 1);
    })
    .await
    .expect("délai");
}

/// Deux clients d'une même Station par adresse ; un troisième n'entre qu'à
/// la place d'un inactif de la même adresse.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn deux_clients_par_station_et_par_adresse() {
    timeout(T * 2, async {
        let pki = pki();
        let relay = relay(&pki, |_| {}).await;
        assert_eq!(relay.hub.cfg.limits.clients_per_station_per_ip, 2);
        let station = fake_station(relay_addr(&relay), pki.roots.clone(), false, Policy::default());
        station.joined().await;
        let a = Tunnel::open(client_cfg(&relay, &pki, station.id(), false)).await.unwrap();
        let a_busy = busy(&a).await;
        let b = Tunnel::open(client_cfg(&relay, &pki, station.id(), false)).await.unwrap();
        let _b_busy = busy(&b).await;
        too_many(Tunnel::open(client_cfg(&relay, &pki, station.id(), false)).await);
        assert!(limit_hits(&relay, "clients_per_station_ip") >= 1);
        // a n'a plus rien en cours : il cède sa place au suivant.
        drop(a_busy);
        tokio::time::sleep(Duration::from_millis(300)).await;
        let _c = Tunnel::open(client_cfg(&relay, &pki, station.id(), false)).await.unwrap();
        assert_eq!(limit_hits(&relay, "client_yielded"), 1);
        a.close();
    })
    .await
    .expect("délai");
}

/// Un client sans flux ni connexion pendant `client_idle_secs` est renvoyé ;
/// un client occupé reste.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn un_client_inactif_est_renvoye() {
    timeout(T * 2, async {
        let pki = pki();
        let relay = relay(&pki, |c| c.limits.client_idle_secs = 1).await;
        let station = fake_station(relay_addr(&relay), pki.roots.clone(), false, Policy::default());
        station.joined().await;
        let idle = Tunnel::open(client_cfg(&relay, &pki, station.id(), false)).await.unwrap();
        let working = Tunnel::open(client_cfg(&relay, &pki, station.id(), false)).await.unwrap();
        let _working_busy = busy(&working).await;
        assert_eq!(relay.hub.gauges().clients_app, 2);
        tokio::time::sleep(Duration::from_millis(1300)).await;
        relay.hub.sweep_idle_clients();
        assert_eq!(limit_hits(&relay, "client_idle"), 1);
        assert_eq!(relay.hub.gauges().clients_app, 1);
        idle.close();
        // L'occupé reste, même après le délai.
        tokio::time::sleep(Duration::from_millis(1300)).await;
        relay.hub.sweep_idle_clients();
        assert_eq!(limit_hits(&relay, "client_idle"), 1);
        assert!(working.stats().connected);
    })
    .await
    .expect("délai");
}

/// Des Stations (des clés neuves, gratuites) en rafale depuis une même
/// adresse : au-delà de `stations_per_minute_per_ip`, refusées (`too_many`),
/// même si elles se succèdent sans jamais dépasser quatre à la fois ; sans
/// quoi chacune garderait ses codes 120 s et une seule adresse finirait par
/// tenir une bonne part des codes.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn stations_en_rafale_depuis_une_adresse() {
    timeout(T * 2, async {
        let pki = pki();
        let relay = relay(&pki, |c| c.limits.stations_per_minute_per_ip = 2).await;
        let a = fake_station(relay_addr(&relay), pki.roots.clone(), false, Policy::default());
        a.joined().await;
        drop(a);
        let b = fake_station(relay_addr(&relay), pki.roots.clone(), false, Policy::default());
        b.joined().await;
        let c = fake_station(relay_addr(&relay), pki.roots.clone(), false, Policy::default());
        let mut rx = c.agent.subscribe();
        let state = timeout(T, rx.wait_for(|s| !matches!(s, kybernow_tunnel::AgentState::Connecting))).await.expect("réponse du relais").unwrap().clone();
        assert_eq!(state, kybernow_tunnel::AgentState::Unreachable { reason: "too_many".into() });
    })
    .await
    .expect("délai");
}

/// Un bout qui envoie des `ping` (ou des `status`) sans jamais lire les
/// réponses : la file de ses messages ne grossit pas sans fin dans le
/// relais, il est coupé. N'importe qui peut ouvrir une telle connexion
/// (sans Station, sans compte).
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn un_bout_qui_ne_lit_pas_ses_reponses_est_coupe() {
    use kybernow_tunnel::frame::{encode, read_msg};
    use kybernow_tunnel::msg::{Hello, Msg, Role};
    use kybernow_tunnel::quic::ALPN_CTL;
    timeout(T * 2, async {
        let pki = pki();
        let relay = relay(&pki, |_| {}).await;
        // Une app « à la main » : une petite fenêtre de réception, jamais relue.
        let tls = kybernow_tunnel::tls::client_config(&relay_addr(&relay), Some(pki.roots.clone()), ALPN_CTL, None).unwrap();
        let mut cfg = quinn::ClientConfig::new(std::sync::Arc::new(quinn::crypto::rustls::QuicClientConfig::try_from(tls).unwrap()));
        let mut transport = quinn::TransportConfig::default();
        transport.stream_receive_window(quinn::VarInt::from_u32(2048));
        transport.receive_window(quinn::VarInt::from_u32(4096));
        cfg.transport_config(std::sync::Arc::new(transport));
        let ep = quinn::Endpoint::client("127.0.0.1:0".parse().unwrap()).unwrap();
        let conn = ep.connect_with(cfg, relay.addr, "localhost").unwrap().await.unwrap();
        let (mut send, mut recv) = conn.open_bi().await.unwrap();
        let hello = Hello {
            v: 1,
            role: Role::Client,
            station: None,
            token: Some(kybernow_tunnel::id::new_token()),
            admin_fp: None,
            sig: None,
            codes: vec![],
            version: None,
        };
        send.write_all(&encode(&Msg::Hello(hello))).await.unwrap();
        assert!(matches!(read_msg(&mut recv).await.unwrap(), Some(Msg::Welcome(_))));
        // Des `ping` par milliers, aucune réponse lue ; le flux reste ouvert.
        let flood = tokio::spawn(async move {
            for n in 0..50_000u64 {
                if send.write_all(&encode(&Msg::Ping { n })).await.is_err() {
                    return;
                }
            }
            std::future::pending::<()>().await;
            drop(send);
        });
        let closed = timeout(Duration::from_secs(10), conn.closed()).await;
        assert!(
            matches!(&closed, Ok(quinn::ConnectionError::ApplicationClosed(c)) if c.reason.as_ref() == b"messages non lus"),
            "la connexion qui ne lit pas ses réponses reste ouverte : {closed:?}"
        );
        flood.abort();
    })
    .await
    .expect("délai");
}
