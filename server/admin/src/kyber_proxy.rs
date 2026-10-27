//! Relais vers kycontroller pour le client web (`/link`).
//!
//! Le moteur web de Kyber (`kyclient_wasm`) parle à kycontroller en HTTPS
//! (`/session/login`, `/kymux/start_mux`, `/ws`…) sur l'adresse qu'on lui
//! donne, puis ouvre l'image en WebTransport sur le port de Kyber, vérifié par
//! l'empreinte de son certificat. kycontroller refuse les autres origines et
//! présente son propre certificat : la page passe donc par la console, sur la
//! même origine, et la console relaie ces appels à kycontroller sur cette
//! machine. Les fichiers du moteur viennent du `/webclient/` installé avec
//! Kyber : toujours de la même version que le serveur.
//!
//! Le relais n'est pas une porte ouverte sur kycontroller : il exige la
//! session de la console, ou le cookie d'un invité du navigateur encore dans
//! le partage (KyberNow Share, `share.rs`) (le moteur, en même origine,
//! envoie le cookie sur `fetch` et WebSocket ; ces cookies sont retirés avant
//! Kyber, et Kyber impose lui-même les droits d'un invité), n'accepte une
//! origine que si c'est celle de la console elle-même, ne relaie que des
//! chemins connus (rien qui remonte), épingle l'empreinte du certificat de
//! kycontroller, borne les WebSockets relayés et le temps d'attente de Kyber,
//! et réutilise ses connexions HTTP.

use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};

use axum::body::Body;
use axum::extract::{Request, State};
use axum::http::{HeaderMap, HeaderName, HeaderValue, Method, StatusCode, Uri, Version, header};
use axum::response::{IntoResponse, Response};
use hyper::client::conn::http1::SendRequest;
use hyper_util::rt::TokioIo;
use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::crypto::{CryptoProvider, verify_tls12_signature, verify_tls13_signature};
use rustls::pki_types::pem::PemObject;
use rustls::pki_types::{CertificateDer, ServerName, UnixTime};
use rustls::{DigitallySignedStruct, SignatureScheme};
use sha2::{Digest, Sha256};
use tokio::net::TcpStream;
use tokio::sync::Semaphore;

use crate::AppState;

/// Fichiers du moteur, servis à côté de la page (le worklet audio est chargé
/// par un chemin relatif à la page).
const ENGINE_FILES: &[&str] = &["kyclient_wasm.js", "kyclient_wasm_bg.wasm", "audio_worklet.js", "spinlock.js"];

/// Adresse de kycontroller, pour le développement sur un autre ordinateur
/// (`KYBERNOW_KYBER_ADDR=192.168.1.100:30241`).
const ADDR_ENV: &str = "KYBERNOW_KYBER_ADDR";
/// Empreinte du certificat de kycontroller quand il n'est pas sur ce
/// disque (`KYBERNOW_KYBER_FINGERPRINT=sha256:aa:bb:…`, celle qu'affiche
/// Kyber ou `openssl x509 -fingerprint -sha256`).
const FINGERPRINT_ENV: &str = "KYBERNOW_KYBER_FINGERPRINT";

/// Temps laissé à kycontroller pour accepter la connexion et répondre
/// (en-têtes) ; le corps d'une réponse et un WebSocket ouvert n'ont pas de limite.
const UPSTREAM_TIMEOUT: Duration = Duration::from_secs(15);
/// WebSockets relayés en même temps (une session du client web en ouvre
/// deux ou trois : contrôle, image en WebSocket, entrées ; le client web
/// d'Admin et quatre invités du navigateur, avec de la marge).
const MAX_WEBSOCKETS: usize = 24;
/// Connexions HTTP gardées ouvertes vers kycontroller entre deux requêtes.
const POOL_MAX: usize = 4;

static WEBSOCKETS: Semaphore = Semaphore::const_new(MAX_WEBSOCKETS);
static POOL: Mutex<Vec<Idle>> = Mutex::new(Vec::new());
static PIN: Mutex<Option<Pin>> = Mutex::new(None);

/// Connexion HTTP/1.1 vers kycontroller, prête ou en train de finir sa réponse.
struct Idle {
    addr: String,
    sender: SendRequest<Body>,
}

/// Empreinte épinglée et connecteur TLS construit pour elle.
struct Pin {
    /// Chemin et date du certificat lu (relu s'il change), ou l'environnement.
    source: (String, Option<SystemTime>),
    fingerprint: [u8; 32],
    connector: tokio_rustls::TlsConnector,
}

/// Chemins de kycontroller utilisés par le moteur web. Rien d'autre, et
/// rien qui puisse remonter (`..`, `%2e`, `\`) : un chemin n'est relayé que
/// s'il est l'un des chemins connus ou un nom simple sous `/websocket/`.
pub fn is_kyber_path(path: &str) -> bool {
    if !safe_path(path) {
        return false;
    }
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
    ) || path
        .strip_prefix("/websocket/")
        .is_some_and(|name| !name.is_empty() && name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-'))
}

/// Ni `..`, ni `%2e` (point encodé), ni barre oblique inverse, ni segment vide.
fn safe_path(path: &str) -> bool {
    let lower = path.to_ascii_lowercase();
    path.starts_with('/') && !lower.contains("..") && !lower.contains("%2e") && !lower.contains('\\') && !lower.contains("//")
}

/// Fichier du moteur demandé par la page (`/link/kyclient_wasm.js`). L'ancienne
/// adresse (`/ecran/`, 0.5.0 et avant) sert encore une page restée en cache.
pub fn engine_file(path: &str) -> Option<&'static str> {
    let name = path.strip_prefix("/link/").or_else(|| path.strip_prefix("/ecran/"))?;
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
    // La session de la console, comme pour /api : le moteur tourne dans la
    // page, en même origine, et le navigateur joint le cookie. Ou un invité
    // du navigateur, tant qu'il est dans le partage.
    let console = crate::auth::session_token(req.headers()).is_some_and(|t| s.auth.session_valid(&t));
    let guest = !console && crate::share::guest_cookie(req.headers()).is_some_and(|c| s.share.cookie_valid(&c));
    if !console && !guest {
        return error(StatusCode::UNAUTHORIZED, "session de la console requise");
    }
    // Une origine, si le navigateur en donne une, ne peut être que la
    // console elle-même : un autre site n'ouvre pas de WebSocket vers Kyber
    // par ici, même avec le cookie.
    if !origin_is_own(req.headers(), req.uri().authority().map(|a| a.as_str())) {
        return error(StatusCode::FORBIDDEN, "origine refusée");
    }
    let Some(addr) = target(&s) else {
        return error(StatusCode::SERVICE_UNAVAILABLE, "port de Kyber inconnu");
    };
    let connector = match pinned_connector(&s) {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!("relais vers Kyber : {e:#}");
            return error(StatusCode::SERVICE_UNAVAILABLE, "certificat de Kyber introuvable : relais fermé");
        }
    };
    let path = req.uri().path().to_string();
    let upstream_path = match engine_file(&path) {
        Some(file) => format!("/webclient/{file}"),
        None => req.uri().path_and_query().map(|p| p.as_str().to_string()).unwrap_or(path.clone()),
    };
    // Une poignée par WebSocket relayé, rendue quand la copie s'arrête.
    let permit = if req.headers().contains_key(header::UPGRADE) {
        match WEBSOCKETS.try_acquire() {
            Ok(p) => Some(p),
            Err(_) => return error(StatusCode::SERVICE_UNAVAILABLE, "trop de connexions WebSocket relayées"),
        }
    } else {
        None
    };
    match forward(&s, &addr, connector, req, &upstream_path, permit).await {
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

/// Sans `Origin`, ou avec l'origine de la console : même hôte et port que
/// `Host` (HTTP/1.1) ou, à défaut, que l'autorité de la requête (en HTTP/2,
/// que les navigateurs négocient avec la console, `:authority` remplace
/// `Host` et hyper la range dans l'URI), quel que soit le schéma.
fn origin_is_own(headers: &HeaderMap, authority: Option<&str>) -> bool {
    let Some(origin) = headers.get(header::ORIGIN).and_then(|v| v.to_str().ok()) else { return true };
    let Some(host) = headers.get(header::HOST).and_then(|v| v.to_str().ok()).or(authority) else { return false };
    let origin = origin.trim().split_once("://").map(|(_, rest)| rest).unwrap_or(origin.trim());
    !origin.is_empty() && origin.eq_ignore_ascii_case(host.trim())
}

/// En-têtes transmis à kycontroller : sans ceux de la connexion (sauf
/// `Connection` et `Upgrade` d'un WebSocket), sans `Host` (remplacé par
/// l'adresse de Kyber) et sans le cookie de la console.
fn upstream_headers(headers: &HeaderMap, upgrade: bool, addr: &str) -> HeaderMap {
    let mut out = HeaderMap::new();
    for (name, value) in headers {
        if is_hop_by_hop(name) && !(upgrade && (name == header::CONNECTION || name == header::UPGRADE)) {
            continue;
        }
        if name == header::HOST {
            continue;
        }
        if name == header::COOKIE {
            // La session de la console reste à la console.
            if let Some(v) = strip_console_cookie(value) {
                out.append(header::COOKIE, v);
            }
            continue;
        }
        out.append(name.clone(), value.clone());
    }
    if let Ok(host) = HeaderValue::from_str(addr) {
        out.insert(header::HOST, host);
    }
    out
}

async fn forward(
    s: &AppState,
    addr: &str,
    connector: tokio_rustls::TlsConnector,
    mut req: Request,
    upstream_path: &str,
    permit: Option<tokio::sync::SemaphorePermit<'static>>,
) -> anyhow::Result<Response> {
    let upgrade = req.headers().contains_key(header::UPGRADE);
    let client_upgrade = upgrade.then(|| hyper::upgrade::on(&mut req));
    let (parts, body) = req.into_parts();
    let mut headers = upstream_headers(&parts.headers, upgrade, addr);

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

    // Une connexion gardée d'une requête précédente, sinon une neuve ; Kyber
    // a 15 s pour accepter la connexion et répondre.
    let mut sender = match (!upgrade).then(|| take_idle(addr)).flatten() {
        Some(sender) => sender,
        None => tokio::time::timeout(UPSTREAM_TIMEOUT, connect(addr, connector)).await.map_err(|_| anyhow::anyhow!("Kyber ne répond pas en {} s", UPSTREAM_TIMEOUT.as_secs()))??,
    };
    let mut res = tokio::time::timeout(UPSTREAM_TIMEOUT, sender.send_request(upstream))
        .await
        .map_err(|_| anyhow::anyhow!("Kyber ne répond pas en {} s", UPSTREAM_TIMEOUT.as_secs()))??;

    if res.status() == StatusCode::SWITCHING_PROTOCOLS
        && let Some(client_upgrade) = client_upgrade
    {
        let server_upgrade = hyper::upgrade::on(&mut res);
        tokio::spawn(async move {
            let _permit = permit;
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
    drop(permit);
    if !upgrade {
        give_back(addr, sender);
    }

    let (mut parts, body) = res.into_parts();
    let names: Vec<HeaderName> = parts.headers.keys().filter(|n| is_hop_by_hop(n)).cloned().collect();
    for name in names {
        parts.headers.remove(name);
    }
    Ok(Response::from_parts(parts, Body::new(body)))
}

/// Sujet du jeton d'Admin pour les routes `/kybernow/*` de Kyber (correctif 0019).
const ADMIN_SUBJECT: &str = "kybernow-admin";
/// Temps laissé à Kyber pour répondre à un appel d'Admin : la fin d'une
/// session ou l'arrivée d'un invité n'attendent pas un Kyber bloqué.
const ADMIN_CALL_TIMEOUT: Duration = Duration::from_secs(4);

/// Un appel d'Admin à kycontroller (sur cette machine, certificat épinglé) :
/// `/kybernow/*` du correctif 0019 avec un jeton `kn_admin` d'une minute, ou
/// une route sans compte (`/enumerate_displays`). Rend le statut et le JSON
/// de la réponse (`null` si elle n'en est pas).
pub async fn kyber_call(s: &AppState, method: Method, path: &str, body: Option<serde_json::Value>) -> anyhow::Result<(StatusCode, serde_json::Value)> {
    let addr = target(s).ok_or_else(|| anyhow::anyhow!("port de Kyber inconnu"))?;
    let connector = pinned_connector(s)?;
    let mut builder = hyper::Request::builder()
        .method(method)
        .uri(path.parse::<Uri>()?)
        .version(Version::HTTP_11)
        .header(header::HOST, addr.as_str());
    if path.starts_with("/kybernow/") {
        let secret = s.kyber.jwt_secret().ok_or_else(|| anyhow::anyhow!("pas de clé pour les jetons de Kyber"))?;
        let token = jwt_with(&secret, ADMIN_SUBJECT, 60, serde_json::json!({ "kn_admin": true }));
        builder = builder.header(header::AUTHORIZATION, format!("Bearer {token}"));
    }
    let request = match body {
        Some(json) => {
            let bytes = serde_json::to_vec(&json)?;
            builder
                .header(header::CONTENT_TYPE, "application/json")
                .header(header::CONTENT_LENGTH, bytes.len())
                .body(Body::from(bytes))?
        }
        None => builder.body(Body::empty())?,
    };
    let call = async {
        let mut sender = match take_idle(&addr) {
            Some(sender) => sender,
            None => connect(&addr, connector).await?,
        };
        let res = sender.send_request(request).await?;
        give_back(&addr, sender);
        let status = res.status();
        let bytes = axum::body::to_bytes(Body::new(res.into_body()), 256 * 1024).await?;
        let json = serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null);
        anyhow::Ok((status, json))
    };
    tokio::time::timeout(ADMIN_CALL_TIMEOUT, call).await.map_err(|_| anyhow::anyhow!("Kyber ne répond pas en {} s", ADMIN_CALL_TIMEOUT.as_secs()))?
}

/// Nouvelle connexion TCP + TLS (empreinte vérifiée) + HTTP/1.1 vers kycontroller.
async fn connect(addr: &str, connector: tokio_rustls::TlsConnector) -> anyhow::Result<SendRequest<Body>> {
    let tcp = TcpStream::connect(addr).await?;
    tcp.set_nodelay(true)?;
    let tls = connector.connect(ServerName::try_from("localhost")?, tcp).await?;
    let (sender, conn) = hyper::client::conn::http1::handshake(TokioIo::new(tls)).await?;
    tokio::spawn(async move {
        if let Err(e) = conn.with_upgrades().await {
            tracing::debug!("connexion au serveur Kyber : {e}");
        }
    });
    Ok(sender)
}

/// Une connexion gardée, prête (sa réponse précédente entièrement lue).
fn take_idle(addr: &str) -> Option<SendRequest<Body>> {
    let mut pool = POOL.lock().unwrap();
    pool.retain(|i| !i.sender.is_closed());
    let pos = pool.iter().position(|i| i.addr == addr && i.sender.is_ready())?;
    Some(pool.remove(pos).sender)
}

fn give_back(addr: &str, sender: SendRequest<Body>) {
    let mut pool = POOL.lock().unwrap();
    pool.retain(|i| !i.sender.is_closed());
    if !sender.is_closed() && pool.len() < POOL_MAX {
        pool.push(Idle { addr: addr.to_string(), sender });
    }
}

fn is_hop_by_hop(name: &HeaderName) -> bool {
    matches!(
        name.as_str(),
        "connection" | "keep-alive" | "proxy-authenticate" | "proxy-authorization" | "te" | "trailer" | "transfer-encoding" | "upgrade"
    )
}

/// Retire le cookie de la console et celui d'un invité : ils restent à la console.
fn strip_console_cookie(value: &HeaderValue) -> Option<HeaderValue> {
    let text = value.to_str().ok()?;
    let ours = [format!("{}=", crate::auth::COOKIE), format!("{}=", crate::share::GUEST_COOKIE)];
    let kept: Vec<&str> = text
        .split(';')
        .map(str::trim)
        .filter(|c| !c.is_empty() && !ours.iter().any(|o| c.starts_with(o.as_str())))
        .collect();
    if kept.is_empty() { None } else { HeaderValue::from_str(&kept.join("; ")).ok() }
}

/// Connecteur TLS qui n'accepte que le certificat de kycontroller : celui de
/// `kyber\` (`[kycontroller] tls_cert`, sinon `kybertest_cert.pem`, relu s'il
/// change : réinstallation), ou l'empreinte donnée par l'environnement pour
/// un Kyber sur un autre ordinateur.
fn pinned_connector(s: &AppState) -> anyhow::Result<tokio_rustls::TlsConnector> {
    let (source, fingerprint) = match std::env::var(FINGERPRINT_ENV).ok().filter(|v| !v.is_empty()) {
        Some(text) => ((format!("env:{text}"), None), parse_fingerprint(&text).ok_or_else(|| anyhow::anyhow!("{FINGERPRINT_ENV} illisible"))?),
        None => {
            let path = s.kyber.tls_cert_path();
            let modified = std::fs::metadata(&path).and_then(|m| m.modified()).ok();
            let source = (path.display().to_string(), modified);
            let mut pin = PIN.lock().unwrap();
            if let Some(p) = pin.as_ref().filter(|p| p.source == source) {
                return Ok(p.connector.clone());
            }
            let der = CertificateDer::from_pem_file(&path).map_err(|e| anyhow::anyhow!("certificat de Kyber {} : {e}", path.display()))?;
            let fingerprint: [u8; 32] = Sha256::digest(der.as_ref()).into();
            let connector = make_connector(fingerprint);
            *pin = Some(Pin { source, fingerprint, connector: connector.clone() });
            tracing::info!("certificat de Kyber épinglé : sha256:{}", hex::encode(fingerprint));
            return Ok(connector);
        }
    };
    let mut pin = PIN.lock().unwrap();
    if let Some(p) = pin.as_ref().filter(|p| p.source == source && p.fingerprint == fingerprint) {
        return Ok(p.connector.clone());
    }
    let connector = make_connector(fingerprint);
    *pin = Some(Pin { source, fingerprint, connector: connector.clone() });
    tracing::info!("certificat de Kyber épinglé ({FINGERPRINT_ENV}) : sha256:{}", hex::encode(fingerprint));
    Ok(connector)
}

/// Empreinte du certificat de kycontroller au format de kyclient
/// (`sha256:aa:bb:…`), donnée aux apps par `prepare` : elles l'exigent au
/// lieu de retenir le certificat au premier contact. Réinstaller la Station
/// change ce certificat : une app qui aurait retenu le premier refuserait de
/// se connecter (« connexion impossible »), sans recours.
pub fn kyber_fingerprint(s: &AppState) -> Option<String> {
    pinned_connector(s).ok()?;
    let fingerprint = PIN.lock().unwrap().as_ref()?.fingerprint;
    Some(format!("sha256:{}", fingerprint.iter().map(|b| format!("{b:02x}")).collect::<Vec<_>>().join(":")))
}

/// « sha256:aa:bb:… », « aa:bb:… » ou 64 caractères hexadécimaux.
fn parse_fingerprint(text: &str) -> Option<[u8; 32]> {
    let hex: String = text.trim().trim_start_matches("sha256:").chars().filter(|c| *c != ':').collect();
    let bytes = hex::decode(hex).ok()?;
    bytes.try_into().ok()
}

fn make_connector(fingerprint: [u8; 32]) -> tokio_rustls::TlsConnector {
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let mut config = rustls::ClientConfig::builder_with_provider(provider.clone())
        .with_safe_default_protocol_versions()
        .expect("versions TLS")
        .dangerous()
        .with_custom_certificate_verifier(Arc::new(PinnedCertificate { fingerprint, provider }))
        .with_no_client_auth();
    config.alpn_protocols = vec![b"http/1.1".to_vec()];
    tokio_rustls::TlsConnector::from(Arc::new(config))
}

/// kycontroller présente un certificat auto-signé : on n'en accepte qu'un, par
/// son empreinte SHA-256, et on vérifie la signature de la poignée de main.
#[derive(Debug)]
struct PinnedCertificate {
    fingerprint: [u8; 32],
    provider: Arc<CryptoProvider>,
}

impl ServerCertVerifier for PinnedCertificate {
    fn verify_server_cert(
        &self,
        end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp_response: &[u8],
        _now: UnixTime,
    ) -> Result<ServerCertVerified, rustls::Error> {
        let seen: [u8; 32] = Sha256::digest(end_entity.as_ref()).into();
        if seen == self.fingerprint {
            Ok(ServerCertVerified::assertion())
        } else {
            Err(rustls::Error::General(format!("certificat de Kyber inattendu (sha256:{})", hex::encode(seen))))
        }
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        verify_tls12_signature(message, cert, dss, &self.provider.signature_verification_algorithms)
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        verify_tls13_signature(message, cert, dss, &self.provider.signature_verification_algorithms)
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.provider.signature_verification_algorithms.supported_schemes()
    }
}

/// Jeton JWT (HS256) accepté par Kyber : audience « kyber », sujet, expiration.
pub fn jwt(secret: &str, subject: &str, lifetime_secs: i64) -> String {
    jwt_with(secret, subject, lifetime_secs, serde_json::json!({}))
}

/// Comme `jwt`, avec des revendications en plus (`extra`, un objet) : celles
/// de KyberNow Share (`kn_role`, `kn_display`, `kn_rev`, `kn_admin`), que
/// Kyber impose avec le correctif 0019.
pub fn jwt_with(secret: &str, subject: &str, lifetime_secs: i64, extra: serde_json::Value) -> String {
    let exp = time::OffsetDateTime::now_utc().unix_timestamp() + lifetime_secs;
    let header = b64url(br#"{"alg":"HS256","typ":"JWT"}"#);
    let mut claims = serde_json::json!({ "aud": "kyber", "sub": subject, "exp": exp });
    if let (Some(all), Some(more)) = (claims.as_object_mut(), extra.as_object()) {
        for (k, v) in more {
            all.insert(k.clone(), v.clone());
        }
    }
    let claims = b64url(claims.to_string().as_bytes());
    let signing_input = format!("{header}.{claims}");
    let signature = b64url(&hmac_sha256(secret.as_bytes(), signing_input.as_bytes()));
    format!("{signing_input}.{signature}")
}

fn hmac_sha256(key: &[u8], message: &[u8]) -> [u8; 32] {
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
    fn jwt_carries_the_share_claims() {
        let token = jwt_with("secret", "kn-guest-1", 120, serde_json::json!({ "kn_role": "player", "kn_display": 7, "kn_rev": 2 }));
        let part = token.split('.').nth(1).unwrap();
        const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
        let (mut bits, mut count, mut out) = (0u32, 0, Vec::new());
        for c in part.bytes() {
            bits = (bits << 6) | ALPHABET.iter().position(|a| *a == c).unwrap() as u32;
            count += 6;
            if count >= 8 {
                count -= 8;
                out.push((bits >> count) as u8);
                bits &= (1 << count) - 1;
            }
        }
        let claims: serde_json::Value = serde_json::from_slice(&out).unwrap();
        assert_eq!((claims["aud"].as_str(), claims["sub"].as_str()), (Some("kyber"), Some("kn-guest-1")));
        assert_eq!((claims["kn_role"].as_str(), claims["kn_display"].as_u64(), claims["kn_rev"].as_u64()), (Some("player"), Some(7), Some(2)));
        assert!(claims["exp"].as_i64().unwrap() > time::OffsetDateTime::now_utc().unix_timestamp() + 100);
        // Sans revendication en plus : le jeton d'avant, à l'identique.
        assert_eq!(jwt("secret", "matthieu", 60).split('.').count(), 3);
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
        assert!(is_kyber_path("/websocket/inputs_2"));
        assert!(!is_kyber_path("/api/status"));
        assert!(!is_kyber_path("/webclient/"));
        assert!(!is_kyber_path("/session/"));
        assert!(!is_kyber_path("/session/login/../../webclient/index.html"));
        assert!(!is_kyber_path("/websocket/"));
        assert!(!is_kyber_path("/websocket/../session/login"));
        assert!(!is_kyber_path("/websocket/%2e%2e/x"));
        assert!(!is_kyber_path("/websocket/a/b"));
        assert!(!is_kyber_path("/websocket/a%2Eb"));
        assert!(!is_kyber_path("/websocket/a\\b"));
        assert!(!is_kyber_path("/websocket//video"));
        assert_eq!(engine_file("/link/kyclient_wasm.js"), Some("kyclient_wasm.js"));
        assert_eq!(engine_file("/ecran/kyclient_wasm.js"), Some("kyclient_wasm.js"));
        assert_eq!(engine_file("/link/../secret"), None);
        assert_eq!(engine_file("/link/kyclient_wasm.js/.."), None);
    }

    #[test]
    fn headers_sent_to_kyber() {
        let mut h = HeaderMap::new();
        h.insert(header::HOST, "console.local:9443".parse().unwrap());
        h.insert(header::COOKIE, format!("a=1; {}=secret; b=2", crate::auth::COOKIE).parse().unwrap());
        h.insert(header::CONNECTION, "keep-alive".parse().unwrap());
        h.insert(header::UPGRADE, "websocket".parse().unwrap());
        h.insert(header::ORIGIN, "https://console.local:9443".parse().unwrap());
        h.insert(header::AUTHORIZATION, "Bearer x".parse().unwrap());

        let plain = upstream_headers(&h, false, "127.0.0.1:30241");
        assert_eq!(plain.get(header::HOST).unwrap(), "127.0.0.1:30241");
        assert_eq!(plain.get(header::COOKIE).unwrap(), "a=1; b=2");
        assert!(plain.get(header::CONNECTION).is_none());
        assert!(plain.get(header::UPGRADE).is_none());
        assert_eq!(plain.get(header::ORIGIN).unwrap(), "https://console.local:9443");
        assert_eq!(plain.get(header::AUTHORIZATION).unwrap(), "Bearer x");

        let ws = upstream_headers(&h, true, "127.0.0.1:30241");
        assert_eq!(ws.get(header::CONNECTION).unwrap(), "keep-alive");
        assert_eq!(ws.get(header::UPGRADE).unwrap(), "websocket");

        // Le cookie d'un invité du navigateur reste aussi à la console.
        let mut guest = HeaderMap::new();
        guest.insert(header::COOKIE, format!("a=1; {}=secret", crate::share::GUEST_COOKIE).parse().unwrap());
        assert_eq!(upstream_headers(&guest, false, "127.0.0.1:1").get(header::COOKIE).unwrap(), "a=1");

        // Seul le cookie de la console : plus de Cookie du tout.
        let mut only = HeaderMap::new();
        only.insert(header::COOKIE, format!("{}=secret", crate::auth::COOKIE).parse().unwrap());
        assert!(upstream_headers(&only, false, "127.0.0.1:1").get(header::COOKIE).is_none());
    }

    #[test]
    fn origin_must_be_the_console() {
        let mut h = HeaderMap::new();
        assert!(origin_is_own(&h, None), "sans Origin (hors navigateur)");
        h.insert(header::HOST, "console.local:9443".parse().unwrap());
        h.insert(header::ORIGIN, "https://console.local:9443".parse().unwrap());
        assert!(origin_is_own(&h, None));
        h.insert(header::ORIGIN, "https://CONSOLE.local:9443".parse().unwrap());
        assert!(origin_is_own(&h, None));
        h.insert(header::ORIGIN, "https://evil.example".parse().unwrap());
        assert!(!origin_is_own(&h, None));
        h.insert(header::ORIGIN, "https://console.local:9444".parse().unwrap());
        assert!(!origin_is_own(&h, None));
        h.insert(header::ORIGIN, "null".parse().unwrap());
        assert!(!origin_is_own(&h, None));
        let mut no_host = HeaderMap::new();
        no_host.insert(header::ORIGIN, "https://console.local:9443".parse().unwrap());
        assert!(!origin_is_own(&no_host, None), "ni Host ni autorité");
        // HTTP/2 : pas d'en-tête Host, l'autorité est dans l'URI.
        assert!(origin_is_own(&no_host, Some("console.local:9443")));
        assert!(origin_is_own(&no_host, Some("CONSOLE.LOCAL:9443")));
        assert!(!origin_is_own(&no_host, Some("console.local:9444")));
        assert!(!origin_is_own(&no_host, Some("evil.example")));
    }

    #[test]
    fn fingerprint_formats() {
        let hex = "0a".repeat(32);
        assert_eq!(parse_fingerprint(&hex), Some([0x0a; 32]));
        let colons = (0..32).map(|_| "0a").collect::<Vec<_>>().join(":");
        assert_eq!(parse_fingerprint(&format!("sha256:{colons}")), Some([0x0a; 32]));
        assert_eq!(parse_fingerprint(&format!(" {colons} ")), Some([0x0a; 32]));
        assert_eq!(parse_fingerprint("sha256:0a"), None);
        assert_eq!(parse_fingerprint("zz".repeat(32).as_str()), None);
    }

    #[test]
    fn only_the_pinned_certificate_is_accepted() {
        let cert = rcgen::generate_simple_self_signed(vec!["localhost".to_string()]).unwrap();
        let der = cert.cert.der().clone();
        let fingerprint: [u8; 32] = Sha256::digest(der.as_ref()).into();
        let provider = Arc::new(rustls::crypto::ring::default_provider());
        let good = PinnedCertificate { fingerprint, provider: provider.clone() };
        let bad = PinnedCertificate { fingerprint: [0; 32], provider };
        let name = ServerName::try_from("localhost").unwrap();
        assert!(good.verify_server_cert(&der, &[], &name, &[], UnixTime::now()).is_ok());
        assert!(bad.verify_server_cert(&der, &[], &name, &[], UnixTime::now()).is_err());
    }
}
