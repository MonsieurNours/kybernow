//! Relais vers kycontroller pour le client web (`/ecran`).
//!
//! Le moteur web de Kyber (`kyclient_wasm`) parle à kycontroller en HTTPS
//! (`/session/login`, `/kymux/start_mux`, `/ws`…) sur l'adresse qu'on lui
//! donne, puis ouvre l'image en WebTransport sur le port de Kyber, vérifié par
//! l'empreinte de son certificat. kycontroller refuse les autres origines et
//! présente son propre certificat : la page passe donc par la console, sur la
//! même origine, et la console relaie ces appels à kycontroller sur cette
//! machine. Les fichiers du moteur viennent du `/webclient/` installé avec
//! Kyber : toujours de la même version que le serveur.

use std::sync::Arc;

use axum::body::Body;
use axum::extract::{Request, State};
use axum::http::{HeaderMap, HeaderName, HeaderValue, Method, StatusCode, Uri, Version, header};
use axum::response::{IntoResponse, Response};
use hyper_util::rt::TokioIo;
use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::crypto::{CryptoProvider, verify_tls12_signature, verify_tls13_signature};
use rustls::pki_types::{CertificateDer, ServerName, UnixTime};
use rustls::{DigitallySignedStruct, SignatureScheme};
use tokio::net::TcpStream;

use crate::AppState;

/// Fichiers du moteur, servis à côté de la page (le worklet audio est chargé
/// par un chemin relatif à la page).
const ENGINE_FILES: &[&str] = &["kyclient_wasm.js", "kyclient_wasm_bg.wasm", "audio_worklet.js", "spinlock.js"];

/// Adresse de kycontroller, pour le développement sur un autre ordinateur
/// (`KYBERNOW_KYBER_ADDR=192.168.1.100:30241`).
const ADDR_ENV: &str = "KYBERNOW_KYBER_ADDR";

/// Chemins de kycontroller utilisés par le moteur web.
pub fn is_kyber_path(path: &str) -> bool {
    matches!(
        path,
        "/session/login"
            | "/session/renew"
            | "/session/logout"
            | "/kymux/start_mux"
            | "/kymux/start_session"
            | "/ws"
            | "/capabilities"
            | "/enumerate_displays"
            | "/stop_streaming"
            | "/set_bitrate"
            | "/force_idr"
    ) || path.starts_with("/websocket/")
}

/// Fichier du moteur demandé par la page (`/ecran/kyclient_wasm.js`).
pub fn engine_file(path: &str) -> Option<&'static str> {
    let name = path.strip_prefix("/ecran/")?;
    ENGINE_FILES.iter().copied().find(|f| *f == name)
}

/// En-têtes d'isolation exigés par le moteur (mémoire partagée entre le
/// worklet audio et la page).
pub fn isolate(headers: &mut HeaderMap) {
    headers.insert(HeaderName::from_static("cross-origin-opener-policy"), HeaderValue::from_static("same-origin"));
    headers.insert(HeaderName::from_static("cross-origin-embedder-policy"), HeaderValue::from_static("require-corp"));
}

fn target(s: &AppState) -> Option<String> {
    if let Ok(addr) = std::env::var(ADDR_ENV)
        && !addr.is_empty()
    {
        return Some(addr);
    }
    s.kyber.port().map(|p| format!("127.0.0.1:{p}"))
}

fn error(status: StatusCode, text: &str) -> Response {
    (status, text.to_string()).into_response()
}

/// Relaie une requête du moteur web vers kycontroller.
pub async fn proxy(State(s): State<AppState>, req: Request) -> Response {
    let Some(addr) = target(&s) else {
        return error(StatusCode::SERVICE_UNAVAILABLE, "port de Kyber inconnu");
    };
    let path = req.uri().path().to_string();
    let upstream_path = match engine_file(&path) {
        Some(file) => format!("/webclient/{file}"),
        None => req.uri().path_and_query().map(|p| p.as_str().to_string()).unwrap_or(path.clone()),
    };
    match forward(&s, &addr, req, &upstream_path).await {
        Ok(mut res) => {
            if engine_file(&path).is_some() {
                isolate(res.headers_mut());
                res.headers_mut().insert(header::CACHE_CONTROL, HeaderValue::from_static("no-cache"));
            }
            res
        }
        Err(e) => {
            tracing::warn!("relais vers Kyber ({addr}{upstream_path}) : {e:#}");
            error(StatusCode::BAD_GATEWAY, "le serveur Kyber ne répond pas")
        }
    }
}

async fn forward(s: &AppState, addr: &str, mut req: Request, upstream_path: &str) -> anyhow::Result<Response> {
    let upgrade = req.headers().contains_key(header::UPGRADE);
    let client_upgrade = upgrade.then(|| hyper::upgrade::on(&mut req));
    let (parts, body) = req.into_parts();

    let mut headers = HeaderMap::new();
    for (name, value) in &parts.headers {
        if is_hop_by_hop(name) && !(upgrade && (name == header::CONNECTION || name == header::UPGRADE)) {
            continue;
        }
        if name == header::HOST {
            continue;
        }
        if name == header::COOKIE {
            // La session de la console reste à la console.
            if let Some(v) = strip_console_cookie(value) {
                headers.append(header::COOKIE, v);
            }
            continue;
        }
        headers.append(name.clone(), value.clone());
    }
    headers.insert(header::HOST, HeaderValue::from_str(addr)?);

    // Le moteur annonce le port qu'il a joint (celui de la console) comme port
    // de l'image : on le remplace par celui de Kyber.
    let body = if upstream_path == "/kymux/start_mux" && parts.method == Method::POST {
        let bytes = axum::body::to_bytes(body, 64 * 1024).await?;
        let mut json: serde_json::Value = serde_json::from_slice(&bytes)?;
        if let Some(port) = s.kyber.port().or_else(|| addr.rsplit(':').next().and_then(|p| p.parse().ok())) {
            json["public_streamer_port"] = port.into();
        }
        let out = serde_json::to_vec(&json)?;
        headers.insert(header::CONTENT_LENGTH, HeaderValue::from(out.len()));
        Body::from(out)
    } else {
        body
    };

    let mut upstream = hyper::Request::builder()
        .method(parts.method.clone())
        .uri(upstream_path.parse::<Uri>()?)
        .version(Version::HTTP_11)
        .body(body)?;
    *upstream.headers_mut() = headers;

    let tcp = TcpStream::connect(addr).await?;
    tcp.set_nodelay(true)?;
    let tls = connector().connect(ServerName::try_from("localhost")?, tcp).await?;
    let (mut sender, conn) = hyper::client::conn::http1::handshake(TokioIo::new(tls)).await?;
    tokio::spawn(async move {
        if let Err(e) = conn.with_upgrades().await {
            tracing::debug!("connexion au serveur Kyber : {e}");
        }
    });

    let mut res = sender.send_request(upstream).await?;

    if res.status() == StatusCode::SWITCHING_PROTOCOLS
        && let Some(client_upgrade) = client_upgrade
    {
        let server_upgrade = hyper::upgrade::on(&mut res);
        tokio::spawn(async move {
            match tokio::try_join!(client_upgrade, server_upgrade) {
                Ok((client, server)) => {
                    let _ = tokio::io::copy_bidirectional(&mut TokioIo::new(client), &mut TokioIo::new(server)).await;
                }
                Err(e) => tracing::debug!("passage en WebSocket : {e}"),
            }
        });
        let (parts, _) = res.into_parts();
        return Ok(Response::from_parts(parts, Body::empty()));
    }

    let (mut parts, body) = res.into_parts();
    let names: Vec<HeaderName> = parts.headers.keys().filter(|n| is_hop_by_hop(n)).cloned().collect();
    for name in names {
        parts.headers.remove(name);
    }
    Ok(Response::from_parts(parts, Body::new(body)))
}

fn is_hop_by_hop(name: &HeaderName) -> bool {
    matches!(
        name.as_str(),
        "connection" | "keep-alive" | "proxy-authenticate" | "proxy-authorization" | "te" | "trailer" | "transfer-encoding" | "upgrade"
    )
}

fn strip_console_cookie(value: &HeaderValue) -> Option<HeaderValue> {
    let text = value.to_str().ok()?;
    let kept: Vec<&str> = text
        .split(';')
        .map(str::trim)
        .filter(|c| !c.is_empty() && !c.starts_with(&format!("{}=", crate::auth::COOKIE)))
        .collect();
    if kept.is_empty() { None } else { HeaderValue::from_str(&kept.join("; ")).ok() }
}

fn connector() -> tokio_rustls::TlsConnector {
    static CONFIG: std::sync::OnceLock<Arc<rustls::ClientConfig>> = std::sync::OnceLock::new();
    let config = CONFIG.get_or_init(|| {
        let provider = Arc::new(rustls::crypto::ring::default_provider());
        let mut config = rustls::ClientConfig::builder_with_provider(provider.clone())
            .with_safe_default_protocol_versions()
            .expect("versions TLS")
            .dangerous()
            .with_custom_certificate_verifier(Arc::new(AnyCertificate(provider)))
            .with_no_client_auth();
        config.alpn_protocols = vec![b"http/1.1".to_vec()];
        Arc::new(config)
    });
    tokio_rustls::TlsConnector::from(config.clone())
}

/// kycontroller présente un certificat auto-signé, sur cette machine : on
/// vérifie la signature de la poignée de main, pas la chaîne.
#[derive(Debug)]
struct AnyCertificate(Arc<CryptoProvider>);

impl ServerCertVerifier for AnyCertificate {
    fn verify_server_cert(
        &self,
        _end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp_response: &[u8],
        _now: UnixTime,
    ) -> Result<ServerCertVerified, rustls::Error> {
        Ok(ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        verify_tls12_signature(message, cert, dss, &self.0.signature_verification_algorithms)
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        verify_tls13_signature(message, cert, dss, &self.0.signature_verification_algorithms)
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.0.signature_verification_algorithms.supported_schemes()
    }
}

/// Jeton JWT (HS256) accepté par Kyber : audience « kyber », sujet, expiration.
pub fn jwt(secret: &str, subject: &str, lifetime_secs: i64) -> String {
    let exp = time::OffsetDateTime::now_utc().unix_timestamp() + lifetime_secs;
    let header = b64url(br#"{"alg":"HS256","typ":"JWT"}"#);
    let claims = b64url(serde_json::json!({ "aud": "kyber", "sub": subject, "exp": exp }).to_string().as_bytes());
    let signing_input = format!("{header}.{claims}");
    let signature = b64url(&hmac_sha256(secret.as_bytes(), signing_input.as_bytes()));
    format!("{signing_input}.{signature}")
}

fn hmac_sha256(key: &[u8], message: &[u8]) -> [u8; 32] {
    use sha2::{Digest, Sha256};
    let mut block = [0u8; 64];
    if key.len() > 64 {
        block[..32].copy_from_slice(&Sha256::digest(key));
    } else {
        block[..key.len()].copy_from_slice(key);
    }
    let mut inner = Sha256::new();
    inner.update(block.map(|b| b ^ 0x36));
    inner.update(message);
    let mut outer = Sha256::new();
    outer.update(block.map(|b| b ^ 0x5c));
    outer.update(inner.finalize());
    outer.finalize().into()
}

fn b64url(input: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let mut out = String::with_capacity(input.len().div_ceil(3) * 4);
    for chunk in input.chunks(3) {
        let n = chunk.iter().enumerate().fold(0u32, |acc, (i, b)| acc | (u32::from(*b) << (16 - 8 * i)));
        let chars = chunk.len() + 1;
        for i in 0..chars {
            out.push(ALPHABET[((n >> (18 - 6 * i)) & 63) as usize] as char);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hmac_matches_rfc4231() {
        // RFC 4231, cas 2.
        let mac = hmac_sha256(b"Jefe", b"what do ya want for nothing?");
        assert_eq!(hex::encode(mac), "5bdcc146bf60754e6a042426089575c75a003f089d2739839dec58b964ec3843");
    }

    #[test]
    fn base64url_without_padding() {
        assert_eq!(b64url(b"f"), "Zg");
        assert_eq!(b64url(b"fo"), "Zm8");
        assert_eq!(b64url(b"foo"), "Zm9v");
        assert_eq!(b64url(&[0xfb, 0xff]), "-_8");
    }

    #[test]
    fn jwt_has_three_parts_and_kyber_audience() {
        let token = jwt("secret", "matthieu", 60);
        let parts: Vec<&str> = token.split('.').collect();
        assert_eq!(parts.len(), 3);
        assert_eq!(parts[0], "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9");
    }

    #[test]
    fn known_paths() {
        assert!(is_kyber_path("/session/login"));
        assert!(is_kyber_path("/websocket/video"));
        assert!(!is_kyber_path("/api/status"));
        assert!(!is_kyber_path("/webclient/"));
        assert_eq!(engine_file("/ecran/kyclient_wasm.js"), Some("kyclient_wasm.js"));
        assert_eq!(engine_file("/ecran/../secret"), None);
    }
}
