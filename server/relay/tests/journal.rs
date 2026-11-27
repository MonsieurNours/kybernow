//! Dépôt d'un journal (docs/relais.md, « Dépôt d'un journal ») sur un vrai relais de la boucle
//! locale : le vecteur d'essai `KNJ1` déposé, la page, l'état vu par l'appareil, l'envoi unique au
//! téléphone, le retrait, et les refus (forme, taille, nombre par adresse).

mod common;

use common::*;
use kybernow_relay::journal;
use kybernow_relay::web::JOURNAL_POLICY;

/// Le vecteur d'essai de docs/relais.md : la sortie `KNJ1` (147 octets).
const KNJ1_OUT: &str = "4b4e4a31a0a1a2a3a4a5a6a7a8a9aaabf993742d45cb02bf609a747d4b30ed2cbb830ee05b7b893aacde15b2ab189573fe5e0fd3e5ef9f6cefce74e527344fadb7edb57a8622d0516c944092f54839820d94362b1182b1213c223754581e9c6d05576a740d0b597a66813296bd8e0fd7133db759f01c0f816a6306e13d075c4a2450614699bf3b2059e75ad4459d934d121ce0";

fn hex(s: &str) -> Vec<u8> {
    (0..s.len()).step_by(2).map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap()).collect()
}

fn json(body: &[u8]) -> serde_json::Value {
    serde_json::from_slice(body).unwrap_or_else(|_| panic!("JSON attendu : {}", String::from_utf8_lossy(body)))
}

#[tokio::test]
async fn depot_page_etat_envoi_unique_et_retrait() {
    let pki = pki();
    let relay = relay(&pki, |_| {}).await;
    let octets = [("content-type", "application/octet-stream")];
    let (status, _, body) = https_bytes(&relay, &pki.roots, "POST", "/v1/journal", &octets, hex(KNJ1_OUT)).await.unwrap();
    assert_eq!(status, 201, "{}", String::from_utf8_lossy(&body));
    let v = json(&body);
    let (id, owner) = (v["id"].as_str().unwrap().to_string(), v["owner"].as_str().unwrap().to_string());
    assert!(journal::is_token(&id) && journal::is_token(&owner), "{v}");
    assert_eq!(v["expires_in"], 3600);

    // La page : son script, sa politique de contenu (la seule page avec un script hors des comptes).
    let page = https(&relay, &pki.roots, DOMAIN, "GET", &format!("/journal/{id}"), &[("accept-language", "fr-FR")]).await.unwrap();
    assert_eq!(page.status, 200);
    assert_eq!(page.headers.get("content-security-policy").unwrap(), JOURNAL_POLICY);
    assert_eq!(page.headers.get("referrer-policy").unwrap(), "no-referrer");
    assert!(page.body.contains("/assets/journal.js?v=") && page.body.contains("Journal de KyberNow Link"), "{}", page.body);
    assert!(!page.body.contains(&owner), "le jeton de l'appareil n'est pas dans la page");
    let other = https(&relay, &pki.roots, DOMAIN, "GET", "/", &[]).await.unwrap();
    assert!(!other.headers.get("content-security-policy").unwrap().to_str().unwrap().contains("script-src"), "les autres pages restent sans script");
    let bad = https(&relay, &pki.roots, DOMAIN, "GET", "/journal/court", &[]).await.unwrap();
    assert_eq!(bad.status, 404);

    // L'appareil suit son dépôt avec son jeton, et seulement avec lui.
    let state = |owner: String| {
        let (relay, roots, id) = (&relay, &pki.roots, id.clone());
        async move {
            let h = [("x-kn-owner", owner.as_str())];
            https_bytes(relay, roots, "GET", &format!("/v1/journal/{id}/state"), &h, Vec::new()).await.unwrap()
        }
    };
    let (s, _, b) = state(owner.clone()).await;
    assert_eq!((s, json(&b)["state"].as_str().map(str::to_string)), (200, Some("waiting".into())));
    let (s, _, b) = state("AAAAAAAAAAAAAAAAAAAAAA".into()).await;
    assert_eq!((s, json(&b)["error"].as_str().map(str::to_string)), (404, Some("gone".into())));

    // Le téléphone le reprend une fois, tel quel.
    let (s, h, b) = https_bytes(&relay, &pki.roots, "GET", &format!("/v1/journal/{id}"), &[], Vec::new()).await.unwrap();
    assert_eq!(s, 200);
    assert_eq!(b.as_ref(), &hex(KNJ1_OUT)[..]);
    assert_eq!(h.get("content-type").unwrap(), "application/octet-stream");
    assert_eq!(h.get("cache-control").unwrap(), "no-store");
    let (s, _, b) = https_bytes(&relay, &pki.roots, "GET", &format!("/v1/journal/{id}"), &[], Vec::new()).await.unwrap();
    assert_eq!((s, json(&b)["error"].as_str().map(str::to_string)), (404, Some("gone".into())), "usage unique");
    let (s, _, b) = state(owner.clone()).await;
    assert_eq!((s, json(&b)["state"].as_str().map(str::to_string)), (200, Some("opened".into())));

    // « Terminé » : retiré, avec le jeton seulement.
    let path = format!("/v1/journal/{id}");
    let (s, _, _) = https_bytes(&relay, &pki.roots, "DELETE", &path, &[("x-kn-owner", "AAAAAAAAAAAAAAAAAAAAAA")], Vec::new()).await.unwrap();
    assert_eq!(s, 404);
    let (s, _, _) = https_bytes(&relay, &pki.roots, "DELETE", &path, &[("x-kn-owner", owner.as_str())], Vec::new()).await.unwrap();
    assert_eq!(s, 204);
    assert_eq!(state(owner.clone()).await.0, 404);

    // Les métriques comptent sans rien nommer.
    let metrics = reqwest_metrics(&relay).await;
    assert!(metrics.contains("relay_journals_total{result=\"stored\"} 1") && metrics.contains("relay_journals_waiting 0"), "{metrics}");
    assert!(!metrics.contains(&id));
}

#[tokio::test]
async fn refus_forme_taille_et_nombre() {
    let pki = pki();
    let relay = relay(&pki, |_| {}).await;
    let post = |body: Vec<u8>, extra: Vec<(&'static str, String)>| {
        let (relay, roots) = (&relay, &pki.roots);
        async move {
            let extra: Vec<(&str, &str)> = extra.iter().map(|(k, v)| (*k, v.as_str())).collect();
            https_bytes(relay, roots, "POST", "/v1/journal", &extra, body).await
        }
    };
    let (s, _, b) = post(b"pas un journal, mais assez long pour la taille".to_vec(), vec![]).await.unwrap();
    assert_eq!((s, json(&b)["error"].as_str().map(str::to_string)), (400, Some("bad_request".into())));
    // Trop grand, annoncé par Content-Length : refusé avant que le corps parte (le relais ne le lit
    // pas ; un client qui l'enverrait quand même verrait la connexion fermée).
    let head = too_large_head(&relay, &pki.roots).await;
    assert!(head.starts_with("HTTP/1.1 413") && head.contains("too_large"), "{head}");
    // Juste à la limite : accepté.
    let mut max = journal::MAGIC.to_vec();
    max.resize(journal::MAX_BYTES, 1);
    assert_eq!(post(max, vec![]).await.unwrap().0, 201);
    // Chaque essai compte : 6 par heure et par adresse, puis 429 avec Retry-After.
    let mut last = 0;
    for _ in 0..journal::PER_ADDRESS {
        last = post(hex(KNJ1_OUT), vec![]).await.unwrap().0;
        if last == 429 {
            break;
        }
    }
    assert_eq!(last, 429);
    let (s, h, b) = post(hex(KNJ1_OUT), vec![]).await.unwrap();
    assert_eq!(s, 429);
    assert!(json(&b)["retry_after"].as_u64().unwrap() > 0);
    assert!(h.get("retry-after").is_some());
    // GET sur la route du dépôt, PUT sur un dépôt : refusés.
    assert_eq!(https_bytes(&relay, &pki.roots, "GET", "/v1/journal", &[], Vec::new()).await.unwrap().0, 405);
    assert_eq!(https_bytes(&relay, &pki.roots, "PUT", "/v1/journal/AAAAAAAAAAAAAAAAAAAAAA", &[], Vec::new()).await.unwrap().0, 405);
}

/// Les en-têtes d'un dépôt annoncé trop grand, sans son corps, et la réponse lue telle quelle.
async fn too_large_head(relay: &kybernow_relay::Relay, roots: &std::sync::Arc<rustls::RootCertStore>) -> String {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let tcp = tokio::net::TcpStream::connect(relay.addr).await.unwrap();
    let mut cfg = rustls::ClientConfig::builder_with_provider(kybernow_tunnel::tls::provider())
        .with_safe_default_protocol_versions()
        .unwrap()
        .with_root_certificates(roots.clone())
        .with_no_client_auth();
    cfg.alpn_protocols = vec![b"http/1.1".to_vec()];
    let name = rustls::pki_types::ServerName::try_from(DOMAIN.to_string()).unwrap();
    let mut tls = tokio_rustls::TlsConnector::from(std::sync::Arc::new(cfg)).connect(name, tcp).await.unwrap();
    let req = format!("POST /v1/journal HTTP/1.1\r\nHost: {DOMAIN}\r\nContent-Type: application/octet-stream\r\nContent-Length: {}\r\n\r\n", journal::MAX_BYTES + 1);
    tls.write_all(req.as_bytes()).await.unwrap();
    let mut out = Vec::new();
    let _ = tokio::time::timeout(std::time::Duration::from_secs(5), tls.read_to_end(&mut out)).await;
    String::from_utf8_lossy(&out).into_owned()
}

/// `/metrics` du relais (sans TLS).
async fn reqwest_metrics(relay: &kybernow_relay::Relay) -> String {
    let addr = relay.metrics_addr.unwrap();
    let tcp = tokio::net::TcpStream::connect(addr).await.unwrap();
    request(tcp, "127.0.0.1", "GET", "/metrics", &[]).await.unwrap().body
}
