//! Le relais sur le web (docs/relais.md, « Le relais sur le web ») :
//!
//! - `share.<domaine>` : l'accueil de KyberNow Share et la saisie du code (`/`),
//!   `/j/{code}`, `/assets/…` (styles, polices et logo embarqués), `/v1/codes/{code}`,
//!   `/v1/health`, `/privacy` ; avec les comptes (`[accounts] enabled`), `/compte/…`
//!   ([`crate::accounts`]), seules pages en POST ; avec eux aussi `/tv`,
//!   `/v1/keys` (clé de signature des jetons), l'API des appareils
//!   (`/v1/devices/…`, `/v1/computers`, `/v1/access`, en JSON) et les textes
//!   juridiques `/conditions` et `/confidentialite` ([`crate::legal`]) ;
//! - `<id>.share.<domaine>` : mandataire HTTP/1.1 (WebSocket compris) vers
//!   Admin de la Station `<id>`, par un flux `admin_http`.
//!
//! Les journaux ne contiennent jamais un code (pas de chemins).

use std::convert::Infallible;
use std::net::IpAddr;
use std::sync::Arc;
use std::time::Duration;

use bytes::Bytes;
use http_body_util::combinators::BoxBody;
use http_body_util::{BodyExt, Full};
use hyper::body::Incoming;
use hyper::header::{self, HeaderValue};
use hyper::service::service_fn;
use hyper::{Request, Response, StatusCode};
use hyper_util::rt::{TokioIo, TokioTimer};
use kybernow_tunnel::id;
use kybernow_tunnel::lookup::is_code;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::sync::Mutex;

use crate::accounts::devices::{self, ApiMethod, ApiRequest};
use crate::accounts::{Accounts, web as account_web};
use crate::assets;
use crate::hub::{Hub, VERSION};
use crate::pages::{self, Lang};

type Body = BoxBody<Bytes, hyper::Error>;

/// À qui s'adresse une connexion TLS du navigateur, d'après le nom demandé.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Site {
    /// `share.<domaine>` : les pages du relais.
    Relay,
    /// `<id>.share.<domaine>` : Admin de cette Station.
    Station(String),
}

/// Le site d'un nom (SNI), ou `None` (connexion fermée).
pub fn site_for(sni: Option<&str>, domain: &str) -> Option<Site> {
    let sni = sni?.trim_end_matches('.').to_ascii_lowercase();
    if sni == domain {
        return Some(Site::Relay);
    }
    let label = sni.strip_suffix(domain)?.strip_suffix('.')?;
    id::is_station_id(label).then(|| Site::Station(label.to_string()))
}

fn full(body: impl Into<Bytes>) -> Body {
    Full::new(body.into()).map_err(|never| match never {}).boxed()
}

fn html(status: StatusCode, page: String) -> Response<Body> {
    let mut r = Response::new(full(page));
    *r.status_mut() = status;
    let h = r.headers_mut();
    h.insert(header::CONTENT_TYPE, HeaderValue::from_static("text/html; charset=utf-8"));
    h.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    h.insert("x-content-type-options", HeaderValue::from_static("nosniff"));
    h.insert("referrer-policy", HeaderValue::from_static("no-referrer"));
    // La langue vient du navigateur (ou de ?lang=) : un cache ne doit pas la mélanger.
    h.insert(header::VARY, HeaderValue::from_static("accept-language"));
    h.insert("content-security-policy", HeaderValue::from_static(PAGE_POLICY));
    r
}

/// La politique de contenu des pages : tout vient du relais (styles, polices, logo, favicon) ;
/// les images `data:` sont celles de la feuille de styles. Aucun script.
pub const PAGE_POLICY: &str =
    "default-src 'none'; style-src 'self' 'unsafe-inline'; font-src 'self'; img-src 'self' data:; form-action 'self'; base-uri 'none'; frame-ancestors 'none'";

/// La politique des pages des comptes : la même, plus le script des passkeys (`/assets/passkey.js`, lot 1b).
pub const ACCOUNT_POLICY: &str = "default-src 'none'; script-src 'self'; style-src 'self' 'unsafe-inline'; font-src 'self'; img-src 'self' data:; form-action 'self'; base-uri 'none'; frame-ancestors 'none'";

/// Un fichier de `/assets/` : l'adresse porte la version des fichiers, la réponse se garde un an.
fn asset(name: &str) -> Option<Response<Body>> {
    let asset = assets::get(name)?;
    let mut r = Response::new(full(Bytes::from_static(asset.body)));
    let h = r.headers_mut();
    h.insert(header::CONTENT_TYPE, HeaderValue::from_static(asset.mime));
    h.insert(header::CACHE_CONTROL, HeaderValue::from_static("public, max-age=31536000, immutable"));
    h.insert("x-content-type-options", HeaderValue::from_static("nosniff"));
    Some(r)
}

fn json(status: StatusCode, value: serde_json::Value) -> Response<Body> {
    let mut r = Response::new(full(value.to_string()));
    *r.status_mut() = status;
    r.headers_mut().insert(header::CONTENT_TYPE, HeaderValue::from_static("application/json"));
    r.headers_mut().insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    r
}

fn redirect(location: &str) -> Response<Body> {
    let mut r = Response::new(full(Bytes::new()));
    *r.status_mut() = StatusCode::FOUND;
    if let Ok(v) = HeaderValue::from_str(location) {
        r.headers_mut().insert(header::LOCATION, v);
    }
    r.headers_mut().insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    r
}

fn retry_after(mut r: Response<Body>, wait: Duration) -> Response<Body> {
    let secs = wait.as_secs().max(1);
    r.headers_mut().insert(header::RETRY_AFTER, HeaderValue::from(secs));
    r
}

/// L'origine d'une Station : `https://<id>.<domaine>[:port]`.
pub fn station_origin(id: &str, domain: &str, port: u16) -> String {
    if port == 443 { format!("https://{id}.{domain}") } else { format!("https://{id}.{domain}:{port}") }
}

/// Sert une connexion TLS du navigateur.
pub async fn serve<S>(hub: Arc<Hub>, io: S, ip: IpAddr, site: Site)
where
    S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    let _guard = hub.web_guard();
    let limits = &hub.cfg.limits;
    let mut builder = hyper::server::conn::http1::Builder::new();
    builder
        .timer(TokioTimer::new())
        .header_read_timeout(Duration::from_secs(limits.header_timeout_secs))
        .max_buf_size(limits.max_header_bytes.max(8192));
    match site {
        Site::Relay => {
            let svc = service_fn(move |req| {
                let hub = hub.clone();
                async move { Ok::<_, Infallible>(with_hsts(relay_page(&hub, req, ip).await)) }
            });
            let _ = builder.serve_connection(TokioIo::new(io), svc).await;
        }
        Site::Station(station) => {
            let backend = Arc::new(Backend { station, browser: id::random_id(), ip, sender: Mutex::new(None) });
            let svc = service_fn(move |req| {
                let (hub, backend) = (hub.clone(), backend.clone());
                async move { Ok::<_, Infallible>(proxy(&hub, &backend, req).await) }
            });
            let _ = builder.serve_connection(TokioIo::new(io), svc).with_upgrades().await;
        }
    }
}

/// HSTS sur les pages du relais (REL-21) : un an, sans les sous-domaines (les pages des Stations
/// passent par leur propre nom, toujours en TLS, mais ce choix-là reste à faire à part).
pub const HSTS: &str = "max-age=31536000";

fn with_hsts(mut r: Response<Body>) -> Response<Body> {
    r.headers_mut().insert(header::STRICT_TRANSPORT_SECURITY, HeaderValue::from_static(HSTS));
    r
}

async fn relay_page(hub: &Hub, req: Request<Incoming>, ip: IpAddr) -> Response<Body> {
    let lang = Lang::of_request(req.uri().query(), req.headers().get(header::ACCEPT_LANGUAGE).and_then(|v| v.to_str().ok()));
    if let Some(accounts) = &hub.accounts
        && account_web::is_account_path(req.uri().path())
    {
        return account_page(accounts.clone(), req, ip, lang).await;
    }
    if let Some(accounts) = &hub.accounts {
        let path = req.uri().path();
        if path == kybernow_tunnel::access::KEYS_PATH && req.method() == hyper::Method::GET {
            return json(StatusCode::OK, serde_json::to_value(accounts.stations.keys.key_set()).expect("JSON"));
        }
        if devices::is_api_path(path) {
            return device_api(accounts.clone(), req, ip).await;
        }
    }
    if req.method() != hyper::Method::GET && req.method() != hyper::Method::HEAD {
        return html(StatusCode::METHOD_NOT_ALLOWED, pages::not_found(lang));
    }
    let path = req.uri().path();
    match path {
        "/" => html(StatusCode::OK, pages::home(lang, None)),
        "/v1/health" => json(StatusCode::OK, serde_json::json!({ "ok": true, "version": VERSION })),
        // Avec les comptes, les textes juridiques du service (brouillons, lot 1b) ; sinon la page de toujours.
        "/conditions" | "/confidentialite" if hub.accounts.is_some() => {
            html(StatusCode::OK, crate::legal::page(path, lang).unwrap_or_else(|| pages::not_found(lang)))
        }
        "/privacy" | "/confidentialite" => html(StatusCode::OK, pages::privacy(lang)),
        "/j" => {
            // Le formulaire de l'accueil : /j?code=123 456
            let code: String = req
                .uri()
                .query()
                .and_then(|q| q.split('&').find_map(|kv| kv.strip_prefix("code=")))
                .unwrap_or("")
                .chars()
                .filter(char::is_ascii_digit)
                .collect();
            if is_code(&code) { join(hub, &code, ip, lang) } else { html(StatusCode::BAD_REQUEST, pages::home(lang, Some(pages::Notice::BadCode))) }
        }
        _ => {
            if let Some(name) = path.strip_prefix("/assets/") {
                return asset(name).unwrap_or_else(|| html(StatusCode::NOT_FOUND, pages::not_found(lang)));
            }
            if let Some(code) = path.strip_prefix("/j/") {
                let code = code.trim_end_matches('/');
                return if is_code(code) { join(hub, code, ip, lang) } else { html(StatusCode::BAD_REQUEST, pages::home(lang, Some(pages::Notice::BadCode))) };
            }
            if let Some(code) = path.strip_prefix("/v1/codes/") {
                if !is_code(code) {
                    hub.metrics.lookup("bad");
                    return json(StatusCode::BAD_REQUEST, serde_json::json!({ "error": "bad_request" }));
                }
                return match hub.lookup_code(code, ip) {
                    Err(wait) => retry_after(
                        json(StatusCode::TOO_MANY_REQUESTS, serde_json::json!({ "error": "too_many", "retry_after": wait.as_secs().max(1) })),
                        wait,
                    ),
                    Ok(None) => json(StatusCode::NOT_FOUND, serde_json::json!({ "error": "unknown_code" })),
                    Ok(Some(found)) => json(StatusCode::OK, serde_json::to_value(found).expect("JSON")),
                };
            }
            html(StatusCode::NOT_FOUND, pages::not_found(lang))
        }
    }
}

/// La taille d'un formulaire des comptes.
pub const MAX_FORM_BYTES: usize = 16 * 1024;
/// Le corps d'une requête arrive dans ce temps, sinon 408 (REL-17).
pub const BODY_TIMEOUT: Duration = Duration::from_secs(10);
/// Une place dans la file des comptes se libère dans ce temps, sinon 503 (REL-17).
pub const GATE_TIMEOUT: Duration = Duration::from_secs(10);

/// L'envoi vient-il d'un autre site ? `Sec-Fetch-Site` (une Station, `<id>.<domaine>`,
/// est du même site mais pas de la même origine), sinon `Origin` (`null` est
/// accepté : avec `Referrer-Policy: no-referrer`, un formulaire de la page l'envoie).
pub fn cross_site(headers: &hyper::HeaderMap) -> bool {
    let get = |h: &str| headers.get(h).and_then(|v| v.to_str().ok());
    if let Some(site) = get("sec-fetch-site") {
        return !matches!(site, "same-origin" | "none");
    }
    match (get("origin"), get("host")) {
        (None | Some("null"), _) => false,
        (Some(origin), Some(host)) => origin != format!("https://{host}"),
        (Some(_), None) => true,
    }
}

/// `/compte/…` : la requête lue (formulaire de 16 Kio au plus), puis servie hors de l'exécuteur.
async fn account_page(accounts: Arc<Accounts>, req: Request<Incoming>, ip: IpAddr, lang: Lang) -> Response<Body> {
    let method = match *req.method() {
        hyper::Method::GET | hyper::Method::HEAD => account_web::Method::Get,
        hyper::Method::POST => account_web::Method::Post,
        _ => account_web::Method::Other,
    };
    let cross = cross_site(req.headers());
    let header = |h: header::HeaderName| req.headers().get(h).and_then(|v| v.to_str().ok()).unwrap_or("").to_string();
    let cookies: Vec<(String, String)> =
        req.headers().get_all(header::COOKIE).iter().filter_map(|v| v.to_str().ok()).flat_map(account_web::parse_cookies).collect();
    let user_agent = header(header::USER_AGENT);
    let content_type = header(header::CONTENT_TYPE);
    let path = req.uri().path().to_string();
    let query = req.uri().query().map(account_web::parse_pairs).unwrap_or_default();
    let form = if method == account_web::Method::Post {
        if !content_type.starts_with("application/x-www-form-urlencoded") {
            return html(StatusCode::UNSUPPORTED_MEDIA_TYPE, pages::not_found(lang));
        }
        match tokio::time::timeout(BODY_TIMEOUT, http_body_util::Limited::new(req.into_body(), MAX_FORM_BYTES).collect()).await {
            Ok(Ok(body)) => account_web::parse_pairs(&String::from_utf8_lossy(&body.to_bytes())),
            Ok(Err(_)) => return html(StatusCode::PAYLOAD_TOO_LARGE, pages::not_found(lang)),
            Err(_) => return html(StatusCode::REQUEST_TIMEOUT, pages::not_answering(lang)),
        }
    } else {
        Vec::new()
    };
    let request = account_web::Request { method, path, query, cookies, form, ip, lang, user_agent, cross_site: cross };
    let Ok(Ok(_permit)) = tokio::time::timeout(GATE_TIMEOUT, accounts.gate.acquire()).await else {
        return html(StatusCode::SERVICE_UNAVAILABLE, pages::not_answering(lang));
    };
    let worker = accounts.clone();
    let reply = match tokio::task::spawn_blocking(move || worker.handle(&request)).await {
        Ok(r) => r,
        Err(_) => return html(StatusCode::INTERNAL_SERVER_ERROR, pages::not_answering(lang)),
    };
    let status = StatusCode::from_u16(reply.status).unwrap_or(StatusCode::OK);
    let mut r = match (reply.location, reply.html, reply.download) {
        // L'export des données (lot 1b) : un fichier JSON à télécharger.
        (None, None, Some(file)) => {
            let mut r = Response::new(full(file.body));
            let h = r.headers_mut();
            h.insert(header::CONTENT_TYPE, HeaderValue::from_static("application/json; charset=utf-8"));
            h.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
            h.insert("x-content-type-options", HeaderValue::from_static("nosniff"));
            if let Ok(v) = HeaderValue::from_str(&format!("attachment; filename=\"{}\"", file.filename)) {
                h.insert(header::CONTENT_DISPOSITION, v);
            }
            r
        }
        (Some(location), _, _) => {
            let mut r = redirect(&location);
            *r.status_mut() = StatusCode::SEE_OTHER;
            r
        }
        (None, page, _) => {
            let mut r = html(status, page.unwrap_or_default());
            r.headers_mut().insert("content-security-policy", HeaderValue::from_static(ACCOUNT_POLICY));
            r
        }
    };
    for c in reply.cookies {
        if let Ok(v) = HeaderValue::from_str(&c) {
            r.headers_mut().append(header::SET_COOKIE, v);
        }
    }
    if let Some(secs) = reply.retry_after {
        r.headers_mut().insert(header::RETRY_AFTER, HeaderValue::from(secs));
    }
    r
}

/// L'API des appareils (`/v1/devices/…`, `/v1/computers`, `/v1/access`, `/v1/settings`) :
/// JSON de 16 Kio au plus, jeton porteur, servie hors de l'exécuteur.
async fn device_api(accounts: Arc<Accounts>, req: Request<Incoming>, ip: IpAddr) -> Response<Body> {
    let method = match *req.method() {
        hyper::Method::GET => ApiMethod::Get,
        hyper::Method::POST => ApiMethod::Post,
        hyper::Method::PUT => ApiMethod::Put,
        _ => return json(StatusCode::METHOD_NOT_ALLOWED, serde_json::json!({ "error": "method_not_allowed" })),
    };
    let bearer = req
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.trim().split_once(' '))
        .filter(|(scheme, _)| scheme.eq_ignore_ascii_case("bearer"))
        .map(|(_, token)| token.trim().to_string());
    let path = req.uri().path().to_string();
    let body = if method != ApiMethod::Get {
        match tokio::time::timeout(BODY_TIMEOUT, http_body_util::Limited::new(req.into_body(), MAX_FORM_BYTES).collect()).await {
            Err(_) => return json(StatusCode::REQUEST_TIMEOUT, serde_json::json!({ "error": "timeout" })),
            Ok(Ok(b)) => {
                let bytes = b.to_bytes();
                if bytes.iter().all(u8::is_ascii_whitespace) {
                    serde_json::Value::Null
                } else {
                    match serde_json::from_slice(&bytes) {
                        Ok(v) => v,
                        Err(_) => return json(StatusCode::BAD_REQUEST, serde_json::json!({ "error": "bad_request" })),
                    }
                }
            }
            Ok(Err(_)) => return json(StatusCode::PAYLOAD_TOO_LARGE, serde_json::json!({ "error": "too_large" })),
        }
    } else {
        serde_json::Value::Null
    };
    let request = ApiRequest { method, path, bearer, body, ip };
    // Sa propre file : les pages (et leurs calculs d'Argon2) ne retiennent pas `/v1/access` (REL-17).
    let Ok(Ok(_permit)) = tokio::time::timeout(GATE_TIMEOUT, accounts.api_gate.acquire()).await else {
        return json(StatusCode::SERVICE_UNAVAILABLE, serde_json::json!({ "error": "unavailable" }));
    };
    let worker = accounts.clone();
    let reply = match tokio::task::spawn_blocking(move || worker.api(&request)).await {
        Ok(r) => r,
        Err(_) => return json(StatusCode::INTERNAL_SERVER_ERROR, serde_json::json!({ "error": "unavailable" })),
    };
    let mut r = json(StatusCode::from_u16(reply.status).unwrap_or(StatusCode::OK), reply.body);
    if let Some(secs) = reply.retry_after {
        r.headers_mut().insert(header::RETRY_AFTER, HeaderValue::from(secs));
    }
    r
}

/// `/j/{code}` : vers la page de la Station, `/link/?join=<code>`.
fn join(hub: &Hub, code: &str, ip: IpAddr, lang: Lang) -> Response<Body> {
    match hub.lookup_code(code, ip) {
        Err(wait) => retry_after(html(StatusCode::TOO_MANY_REQUESTS, pages::home(lang, Some(pages::Notice::TooMany))), wait),
        Ok(None) => html(StatusCode::NOT_FOUND, pages::home(lang, Some(pages::Notice::Unknown))),
        Ok(Some(found)) if !found.online => html(StatusCode::SERVICE_UNAVAILABLE, pages::offline(lang)),
        Ok(Some(found)) => {
            let port = hub.link_port.load(std::sync::atomic::Ordering::Relaxed);
            redirect(&format!("{}/link/?join={code}", station_origin(&found.station, &hub.cfg.domain, port)))
        }
    }
}

/// La connexion d'un navigateur vers sa Station : un flux `admin_http`,
/// ouvert à la première requête et gardé tant qu'il sert.
struct Backend {
    station: String,
    browser: String,
    ip: IpAddr,
    sender: Mutex<Option<hyper::client::conn::http1::SendRequest<Incoming>>>,
}

/// Retire `Domain=` d'un `Set-Cookie` : le cookie reste à l'origine de la Station.
pub fn strip_cookie_domain(value: &str) -> String {
    value
        .split(';')
        .filter(|part| !part.trim().to_ascii_lowercase().starts_with("domain="))
        .collect::<Vec<_>>()
        .join(";")
}

fn is_upgrade(req: &Request<Incoming>) -> bool {
    req.headers().get(header::CONNECTION).and_then(|v| v.to_str().ok()).is_some_and(|v| v.to_ascii_lowercase().contains("upgrade"))
        && req.headers().contains_key(header::UPGRADE)
}

async fn proxy(hub: &Arc<Hub>, backend: &Backend, mut req: Request<Incoming>) -> Response<Body> {
    let lang = Lang::of(req.headers().get(header::ACCEPT_LANGUAGE).and_then(|v| v.to_str().ok()));
    // L'adresse du navigateur voyage dans l'en-tête du flux, pas dans ces en-têtes.
    for h in ["x-forwarded-for", "x-forwarded-host", "x-forwarded-proto", "x-real-ip", "forwarded"] {
        req.headers_mut().remove(h);
    }
    let host = req
        .headers()
        .get(header::HOST)
        .and_then(|v| v.to_str().ok())
        .map(|h| h.to_string())
        .unwrap_or_else(|| format!("{}.{}", backend.station, hub.cfg.domain));
    let upgrade = is_upgrade(&req).then(|| hyper::upgrade::on(&mut req));
    let mut slot = backend.sender.lock().await;
    if slot.as_ref().is_none_or(|s| s.is_closed()) {
        *slot = None;
        let Some(stream) = hub.open_web_stream(&backend.station, &backend.browser, backend.ip, &host).await else {
            return html(StatusCode::SERVICE_UNAVAILABLE, pages::offline(lang));
        };
        let counted = crate::counted::Counted::new(stream, hub.metrics.clone());
        match hyper::client::conn::http1::handshake(TokioIo::new(counted)).await {
            Ok((sender, conn)) => {
                tokio::spawn(conn.with_upgrades());
                *slot = Some(sender);
            }
            Err(_) => return html(StatusCode::BAD_GATEWAY, pages::not_answering(lang)),
        }
    }
    let sender = slot.as_mut().expect("connexion à la Station");
    if sender.ready().await.is_err() {
        *slot = None;
        return html(StatusCode::BAD_GATEWAY, pages::not_answering(lang));
    }
    let mut resp = match sender.send_request(req).await {
        Ok(r) => r,
        Err(_) => {
            *slot = None;
            return html(StatusCode::BAD_GATEWAY, pages::not_answering(lang));
        }
    };
    let cookies: Vec<HeaderValue> = resp
        .headers()
        .get_all(header::SET_COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .filter_map(|v| HeaderValue::from_str(&strip_cookie_domain(v)).ok())
        .collect();
    if !cookies.is_empty() {
        resp.headers_mut().remove(header::SET_COOKIE);
        for c in cookies {
            resp.headers_mut().append(header::SET_COOKIE, c);
        }
    }
    if resp.status() == StatusCode::SWITCHING_PROTOCOLS {
        // La connexion vers la Station sert désormais au WebSocket.
        *slot = None;
        let backend_up = hyper::upgrade::on(&mut resp);
        if let Some(client_up) = upgrade {
            tokio::spawn(async move {
                if let (Ok(a), Ok(b)) = tokio::join!(client_up, backend_up) {
                    let _ = tokio::io::copy_bidirectional(&mut TokioIo::new(a), &mut TokioIo::new(b)).await;
                }
            });
        }
    }
    resp.map(|b| b.boxed())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hsts_sur_les_pages_du_relais() {
        let r = with_hsts(html(StatusCode::OK, String::new()));
        assert_eq!(r.headers().get(header::STRICT_TRANSPORT_SECURITY).unwrap(), "max-age=31536000");
    }

    #[test]
    fn aiguillage_par_le_nom() {
        let id = id::station_id(b"cle");
        assert_eq!(site_for(Some("share.kybernow.app"), "share.kybernow.app"), Some(Site::Relay));
        assert_eq!(site_for(Some("SHARE.kybernow.app."), "share.kybernow.app"), Some(Site::Relay));
        assert_eq!(site_for(Some(&format!("{id}.share.kybernow.app")), "share.kybernow.app"), Some(Site::Station(id.clone())));
        assert_eq!(site_for(Some("www.share.kybernow.app"), "share.kybernow.app"), None);
        assert_eq!(site_for(Some(&format!("a.{id}.share.kybernow.app")), "share.kybernow.app"), None);
        assert_eq!(site_for(Some("kybernow.app"), "share.kybernow.app"), None);
        assert_eq!(site_for(None, "share.kybernow.app"), None);
    }

    #[test]
    fn cookies_sans_domaine() {
        assert_eq!(
            strip_cookie_domain("kybernow_guest=abc; Path=/; Domain=share.kybernow.app; Secure; HttpOnly"),
            "kybernow_guest=abc; Path=/; Secure; HttpOnly"
        );
        assert_eq!(strip_cookie_domain("a=b;domain=.x"), "a=b");
        assert_eq!(strip_cookie_domain("a=b; SameSite=Strict"), "a=b; SameSite=Strict");
    }

    #[test]
    fn origine_d_une_station() {
        assert_eq!(station_origin("abc", "share.kybernow.app", 8443), "https://abc.share.kybernow.app:8443");
        assert_eq!(station_origin("abc", "share.kybernow.app", 443), "https://abc.share.kybernow.app");
    }

    #[test]
    fn formulaires_d_un_autre_site_refuses() {
        let headers = |pairs: &[(&'static str, &'static str)]| {
            let mut h = hyper::HeaderMap::new();
            for (k, v) in pairs {
                h.insert(*k, HeaderValue::from_static(v));
            }
            h
        };
        assert!(!cross_site(&headers(&[("sec-fetch-site", "same-origin")])));
        assert!(!cross_site(&headers(&[("sec-fetch-site", "none")])), "adresse tapée, favori");
        // Admin d'une Station (<id>.share.kybernow.app) est du même site, pas de la même origine.
        assert!(cross_site(&headers(&[("sec-fetch-site", "same-site")])));
        assert!(cross_site(&headers(&[("sec-fetch-site", "cross-site"), ("origin", "https://share.kybernow.app"), ("host", "share.kybernow.app")])));
        assert!(!cross_site(&headers(&[("origin", "https://share.kybernow.app"), ("host", "share.kybernow.app")])));
        assert!(!cross_site(&headers(&[("origin", "null"), ("host", "share.kybernow.app")])));
        assert!(cross_site(&headers(&[("origin", "https://abc.share.kybernow.app"), ("host", "share.kybernow.app")])));
        assert!(!cross_site(&headers(&[])), "sans ces en-têtes, le jeton CSRF suffit");
    }
}
