//! Le relais sur le web (docs/relais.md, « Le relais sur le web ») :
//!
//! - `share.<domaine>` : l'accueil de KyberNow Share et la saisie du code (`/`),
//!   `/j/{code}`, `/assets/…` (styles, polices et logo embarqués), `/v1/codes/{code}`,
//!   `/v1/health`, `/privacy` ;
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
                async move { Ok::<_, Infallible>(relay_page(&hub, req, ip)) }
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

fn relay_page(hub: &Hub, req: Request<Incoming>, ip: IpAddr) -> Response<Body> {
    let lang = Lang::of_request(req.uri().query(), req.headers().get(header::ACCEPT_LANGUAGE).and_then(|v| v.to_str().ok()));
    if req.method() != hyper::Method::GET && req.method() != hyper::Method::HEAD {
        return html(StatusCode::METHOD_NOT_ALLOWED, pages::not_found(lang));
    }
    let path = req.uri().path();
    match path {
        "/" => html(StatusCode::OK, pages::home(lang, None)),
        "/v1/health" => json(StatusCode::OK, serde_json::json!({ "ok": true, "version": VERSION })),
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
}
