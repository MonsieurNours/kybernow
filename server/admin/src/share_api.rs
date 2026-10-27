//! Routes de KyberNow Share (docs/kybernow-share.md, « Échange avec Admin ») :
//!
//! - l'hôte (compte de l'appareil, comme `prepare` ; seulement l'appareil en
//!   session) : `POST /api/share/open`, `GET /api/share`, `POST
//!   /api/share/code`, `POST|DELETE /api/share/guest/{id}`, `DELETE /api/share` ;
//! - le PC lui-même (l'icône près de l'horloge avec son jeton local, ou
//!   l'accueil d'Admin avec sa session) : `GET /api/local/displays` et les
//!   mêmes routes sous `/api/local/share…` ;
//! - l'invité (sans compte) : `POST /api/share/join` (limité comme
//!   l'appairage), `POST /api/share/beat`, `POST /api/share/leave`.
//!
//! L'état vit dans `share.rs` ; ici, les droits, les jetons de Kyber des
//! invités, les appels à Kyber (correctif 0019 : fermer la session d'un
//! invité, changer son rôle, lire le format de l'image) et le fil d'Admin.

use std::net::SocketAddr;
use std::time::Duration;

use axum::extract::{ConnectInfo, Path, Query, State};
use axum::http::{HeaderMap, HeaderValue, Method, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::Deserialize;
use serde_json::{Value, json};

use crate::AppState;
use crate::activity::Live;
use crate::api::{ApiError, blocking, stream_user};
use crate::display::{PhysicalDisplay, kyber_display_id};
use crate::i18n::{self, Msg};
use crate::share::{self, Beat, Guest, JoinError, Origin, Role, ShareDisplay, Snapshot};

/// En-tête du secret d'un invité.
const GUEST_HEADER: &str = "x-kybernow-guest";
/// Un invité coupé de Kyber ne peut pas y revenir avec un vieux jeton
/// pendant ce temps (sa durée de vie, plus une marge).
const KYBER_BAN_SECONDS: u64 = share::TOKEN_LIFETIME as u64 + 10;

// --- Refus ---------------------------------------------------------------------

/// Refus des routes de l'hôte et du PC : le message (langue de la requête) et
/// un code stable (`disabled`, `session`, `busy`, `not_in_session`,
/// `unknown_display`, `unknown_guest`, `no_share`, `other_share`,
/// `unauthorized`, `forbidden`, `bad_request`, `failed`).
pub struct ShareError {
    status: StatusCode,
    code: &'static str,
    error: String,
}

impl ShareError {
    fn new(status: StatusCode, code: &'static str, msg: Msg) -> Self {
        ShareError { status, code, error: msg.text() }
    }
}

impl IntoResponse for ShareError {
    fn into_response(self) -> Response {
        (self.status, Json(json!({ "error": self.error, "code": self.code }))).into_response()
    }
}

impl From<ApiError> for ShareError {
    fn from(e: ApiError) -> Self {
        let code = match e.0 {
            StatusCode::UNAUTHORIZED => "unauthorized",
            StatusCode::FORBIDDEN => "forbidden",
            StatusCode::BAD_REQUEST => "bad_request",
            _ => "failed",
        };
        ShareError { status: e.0, code, error: e.1 }
    }
}

type ShareResult<T> = Result<T, ShareError>;

/// Refus des routes de l'invité, comme le contrat les nomme : `{"error": "<code>"}`.
fn guest_error(status: StatusCode, code: &str) -> Response {
    (status, Json(json!({ "error": code }))).into_response()
}

// --- Ce que voient l'hôte, le PC et Admin ---------------------------------------

fn hostname() -> String {
    hostname::get().ok().map(|h| h.to_string_lossy().into_owned()).unwrap_or_default()
}

/// Le partage de `multi_client` (réglage de Kyber, vrai par défaut) :
/// sans lui, Kyber n'accepte qu'un appareil à la fois.
pub fn enabled(s: &AppState) -> bool {
    s.kyber
        .values()
        .ok()
        .and_then(|v| v.into_iter().find(|f| f.id == "network.multi_client"))
        .and_then(|f| f.value.as_bool())
        .unwrap_or(true)
}

fn label(index: u32) -> String {
    Msg::new("share.display").with("n", index).text()
}

/// Lien pour les apps et adresse du client web (docs/kybernow-share.md, « Lien »).
fn links(s: &AppState, code: &str) -> (String, String) {
    let address = crate::tls::lan_ip().unwrap_or_else(|| "localhost".into());
    let port = s.listen_port;
    let mut link = format!("kybernow://join?host={address}&port={port}");
    if let Some(fp) = &s.tls_fingerprint {
        link.push_str("&fp=");
        link.push_str(&urlencoding::encode(fp));
    }
    link.push_str(&format!("&code={code}"));
    let scheme = if s.secure_cookies { "https" } else { "http" };
    (link, format!("{scheme}://{address}:{port}/link/?join={code}"))
}

fn host_name(snapshot: &Snapshot) -> String {
    match &snapshot.origin {
        Origin::Host { name, .. } => name.clone(),
        Origin::Local => hostname(),
    }
}

fn guest_json(g: &Guest) -> Value {
    json!({
        "id": g.id,
        "name": g.name,
        "app": g.app,
        "device": g.device,
        "role": g.role,
        "since": g.since,
        "last_beat": g.last_beat_secs,
        "image": image_json(g),
    })
}

// --- L'image d'un invité (correctif Kyber 0020) ----------------------------------

/// Ce que l'appareil sait afficher et décoder, pour « Auto » (`join`).
#[derive(Debug, Clone, Default, Deserialize)]
pub struct AutoCaps {
    #[serde(default)]
    width: u32,
    #[serde(default)]
    height: u32,
    #[serde(default)]
    fps: u32,
    /// Lus par Kyber dans la demande d'image du client (format, 4:4:4) : ici
    /// seulement acceptés.
    #[serde(default)]
    #[allow(dead_code)]
    codecs: Vec<String>,
    #[serde(default)]
    #[allow(dead_code)]
    yuv444: bool,
}

/// Le préréglage que Kyber reçoit (`kn_quality`). « Auto » : l'image de
/// l'hôte si l'appareil couvre son écran et suit sa fréquence ; sinon
/// 1080p60 s'il affiche au moins 1080p à 60 Hz ; sinon 720p30. Sans ce que
/// l'appareil sait (app d'avant 0.8.0) : l'image de l'hôte, comme avant.
/// Le format et le 4:4:4 restent à Kyber (demande d'image du client).
pub fn resolve_quality(quality: share::Quality, auto: Option<&AutoCaps>, display: &ShareDisplay) -> share::Preset {
    use share::{Preset, Quality};
    match quality {
        Quality::Host => Preset::Host,
        Quality::P1080 => Preset::P1080,
        Quality::P720 => Preset::P720,
        Quality::Auto => {
            let Some(a) = auto.filter(|a| a.width > 0 && a.height > 0) else { return Preset::Host };
            let (long, short) = (a.width.max(a.height), a.width.min(a.height));
            let (host_long, host_short) = (display.width.max(display.height), display.width.min(display.height));
            if long >= host_long && short >= host_short && a.fps >= display.refresh.max(1) {
                Preset::Host
            } else if long >= 1920 && short >= 1080 && a.fps >= 60 {
                Preset::P1080
            } else {
                Preset::P720
            }
        }
    }
}

/// Les images des sessions de Kyber (`/kybernow/sessions`, 0020), par
/// sujet, relues toutes les 2 s au plus pendant un partage avec invités.
static IMAGES: std::sync::Mutex<Option<(std::time::Instant, std::collections::HashMap<String, Value>)>> = std::sync::Mutex::new(None);
const IMAGES_MAX_AGE: Duration = Duration::from_secs(2);

/// Relit les images des invités dans Kyber (sans 0020 : rien).
async fn refresh_images(s: &AppState) {
    let Ok((status, v)) = crate::kyber_proxy::kyber_call(s, Method::GET, "/kybernow/sessions", None).await else { return };
    if !status.is_success() {
        return;
    }
    let mut images = std::collections::HashMap::new();
    for session in v["sessions"].as_array().into_iter().flatten() {
        let (Some(user), image) = (session["user"].as_str(), &session["image"]) else { continue };
        // Deux sessions d'un même invité (une qui remplace l'autre) : celle
        // qui a une image et qui diffuse.
        if image.is_object() && (session["streaming"].as_bool() == Some(true) || !images.contains_key(user)) {
            images.insert(user.to_string(), image.clone());
        }
    }
    *IMAGES.lock().unwrap() = Some((std::time::Instant::now(), images));
}

async fn fresh_images(s: &AppState) {
    let stale = IMAGES.lock().unwrap().as_ref().is_none_or(|(at, _)| at.elapsed() > IMAGES_MAX_AGE);
    if stale {
        refresh_images(s).await;
    }
}

/// L'image d'un invité (`image` du signe de vie, `guests[].image` de la
/// vue de l'hôte) : la qualité demandée, puis ce que Kyber en a fait (`null`
/// tant qu'il ne le sait pas).
fn image_json(g: &Guest) -> Value {
    let known = IMAGES.lock().unwrap().as_ref().and_then(|(_, m)| m.get(&g.kyber_user()).cloned()).unwrap_or(Value::Null);
    json!({
        "quality": g.quality.as_str(),
        "width": known["width"],
        "height": known["height"],
        "fps": known["fps"],
        "codec": known["codec"],
        "follows_host": known["follows_host"],
        "reason": known["reason"],
    })
}

/// La qualité d'un invité, poussée tout de suite à Kyber (il relance
/// l'image de l'invité seulement, et la retient 10 min : à une nouvelle
/// connexion, il la préfère à celle du jeton). `false` : Kyber ne l'a pas
/// prise.
async fn kyber_quality(s: &AppState, guest: &Guest) -> bool {
    let body = json!({ "user": guest.kyber_user(), "quality": guest.preset.as_str() });
    match crate::kyber_proxy::kyber_call(s, Method::POST, "/kybernow/quality", Some(body)).await {
        Ok((status, _)) if status.is_success() => true,
        Ok((status, _)) => {
            tracing::warn!("partage : qualité de {} refusée par Kyber ({status}) ; correctif 0020 absent ?", guest.name);
            false
        }
        Err(e) => {
            tracing::warn!("partage : qualité de {} non transmise à Kyber : {e:#}", guest.name);
            false
        }
    }
}

/// La vue d'un partage ouvert (`ShareView` des interfaces) ; elle contient
/// ce que donne `open` dans le contrat (`code`, `link`, `web`, `display`).
pub fn view(s: &AppState, snapshot: &Snapshot) -> Value {
    let (link, web) = links(s, &snapshot.code);
    let d = &snapshot.display;
    json!({
        "open": true,
        "origin": snapshot.origin.kind(),
        "host": host_name(snapshot),
        "code": snapshot.code,
        "link": link,
        "web": web,
        "display": {
            "id": d.id,
            "index": d.index,
            "label": label(d.index),
            "width": d.width,
            "height": d.height,
            "refresh": d.refresh,
        },
        "guests": snapshot.guests.iter().map(guest_json).collect::<Vec<_>>(),
        "max_guests": share::MAX_GUESTS,
        "opened_at": snapshot.opened_at,
    })
}

/// Le partage ouvert, ou `null` (`/api/status`, `/api/local/state`).
pub fn status(s: &AppState) -> Value {
    s.share.snapshot().map_or(Value::Null, |snap| view(s, &snap))
}

fn current_view(s: &AppState) -> Value {
    s.share.snapshot().map_or_else(|| json!({ "open": false }), |snap| view(s, &snap))
}

/// Le partage, pour le signe de vie de l'appareil `device_id` s'il en est l'hôte.
pub fn for_host(s: &AppState, device_id: Option<&str>) -> Option<Value> {
    let device_id = device_id?;
    s.share.hosted_by(device_id).then(|| current_view(s))
}

/// Qui occupe l'ordinateur, tel que les gardes (redémarrage, mise à jour,
/// comptes en attente) le nomment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Occupant {
    /// L'appareil en session.
    Host(String),
    /// Les invités d'un partage sans session (leurs prénoms, « Léa et Max »).
    Guests(String),
}

impl Occupant {
    pub fn name(&self) -> &str {
        match self {
            Occupant::Host(name) | Occupant::Guests(name) => name,
        }
    }
}

/// L'appareil en session, sinon les invités d'un partage.
pub fn occupant(s: &AppState) -> Option<Occupant> {
    if let Some(live) = s.activity.live() {
        return Some(Occupant::Host(live.name));
    }
    guest_names(s).map(Occupant::Guests)
}

/// Les prénoms des invités (« Léa et Max »), s'il y en a.
pub fn guest_names(s: &AppState) -> Option<String> {
    let names: Vec<String> = s.share.guests().into_iter().map(|g| g.name).collect();
    (!names.is_empty()).then(|| i18n::list(i18n::current(), &names))
}

// --- Kyber (correctif 0019) ------------------------------------------------------

/// Le jeton de Kyber d'un invité : son rôle et son écran, 120 s.
fn guest_token(s: &AppState, guest: &Guest, display: u32) -> Option<String> {
    let secret = s.kyber.jwt_secret()?;
    let claims = json!({ "kn_role": guest.role.as_str(), "kn_display": display, "kn_rev": guest.rev, "kn_quality": guest.preset.as_str() });
    Some(crate::kyber_proxy::jwt_with(&secret, &guest.kyber_user(), share::TOKEN_LIFETIME, claims))
}

/// Ferme tout de suite les sessions de Kyber d'un invité, et lui refuse son
/// vieux jeton le temps qu'il expire.
async fn kyber_close(s: &AppState, guest: &Guest) {
    let body = json!({ "user": guest.kyber_user(), "ban_seconds": KYBER_BAN_SECONDS });
    match crate::kyber_proxy::kyber_call(s, Method::POST, "/kybernow/close", Some(body)).await {
        Ok((status, v)) if status.is_success() => {
            tracing::info!("partage : {} coupé de Kyber ({} session(s))", guest.name, v["closed"].as_u64().unwrap_or(0))
        }
        Ok((status, _)) => tracing::warn!("partage : Kyber refuse de couper {} ({status}) ; correctif 0019 absent ?", guest.name),
        Err(e) => tracing::warn!("partage : {} non coupé de Kyber : {e:#}", guest.name),
    }
}

/// Le nouveau rôle d'un invité, appliqué tout de suite par Kyber (sinon à
/// son prochain renouvellement de jeton).
async fn kyber_role(s: &AppState, guest: &Guest) {
    let body = json!({ "user": guest.kyber_user(), "role": guest.role.as_str(), "rev": guest.rev });
    match crate::kyber_proxy::kyber_call(s, Method::POST, "/kybernow/role", Some(body)).await {
        Ok((status, _)) if status.is_success() => {}
        Ok((status, _)) => tracing::warn!("partage : rôle de {} refusé par Kyber ({status})", guest.name),
        Err(e) => tracing::warn!("partage : rôle de {} non transmis à Kyber : {e:#}", guest.name),
    }
}

/// Le format de l'image en cours sur cet écran (`h264`, `hevc`, `av1`) et
/// si elle est en 4:4:4 ; `None` si l'image n'a pas encore démarré.
async fn kyber_stream(s: &AppState, display: u32) -> Option<(String, bool)> {
    let (status, v) = crate::kyber_proxy::kyber_call(s, Method::GET, "/kybernow/sessions", None).await.ok()?;
    if !status.is_success() {
        return None;
    }
    // L'image de l'hôte seulement (0020 donne `kind` ; 0019 n'a qu'elle).
    let stream = v["streams"]
        .as_array()?
        .iter()
        .find(|st| st["display"].as_u64() == Some(u64::from(display)) && st["kind"].as_str().is_none_or(|k| k == "host"))?;
    Some((stream["codec"].as_str()?.to_string(), stream["yuv444"].as_bool().unwrap_or(false)))
}

/// Les écrans que Kyber sait capturer (sa liste, sans compte), s'il répond.
async fn kyber_display_ids(s: &AppState) -> Option<Vec<u32>> {
    let (status, v) = crate::kyber_proxy::kyber_call(s, Method::GET, "/enumerate_displays", None).await.ok()?;
    if !status.is_success() {
        return None;
    }
    Some(v.as_array()?.iter().filter_map(|d| d["id"].as_u64().map(|id| id as u32)).collect())
}

// --- Ouvrir, arrêter ----------------------------------------------------------------

/// Les écrans du PC dans l'ordre du partage, avec leur identifiant de Kyber
/// et leur rang (« Écran 1 »). Seuls ceux que Kyber capture, s'il le dit.
async fn displays(s: &AppState) -> ShareResult<Vec<(PhysicalDisplay, u32, u32)>> {
    let d = s.display.clone();
    let list = blocking(move || d.displays()).await?;
    let known = kyber_display_ids(s).await.unwrap_or_default();
    let capturable: Vec<PhysicalDisplay> = list.iter().filter(|d| known.contains(&kyber_display_id(&d.name))).cloned().collect();
    // Kyber ne dit rien, ou rien ne correspond (sa liste a pu vieillir) : tous.
    let list = if capturable.is_empty() { list } else { capturable };
    Ok(list.into_iter().enumerate().map(|(i, d)| {
        let id = kyber_display_id(&d.name);
        (d, id, i as u32 + 1)
    }).collect())
}

/// L'écran principal de la session de l'hôte : l'écran virtuel principal,
/// sinon l'écran principal de Windows.
async fn host_display(s: &AppState) -> ShareResult<ShareDisplay> {
    if let Some((name, mode)) = s.display.primary_screen().filter(|(n, _)| !n.is_empty()) {
        return Ok(ShareDisplay { id: kyber_display_id(&name), index: 1, width: mode.width, height: mode.height, refresh: mode.refresh, name });
    }
    let list = displays(s).await?;
    let (d, id, _) = list.into_iter().next().ok_or_else(|| ShareError::new(StatusCode::CONFLICT, "unknown_display", Msg::new("share.unknownDisplay")))?;
    Ok(ShareDisplay { id, index: 1, width: d.width, height: d.height, refresh: d.refresh, name: d.name })
}

/// L'hôte s'est préparé de nouveau (`prepare` du même appareil) : son écran
/// principal a pu changer d'identifiant ; le partage le suit.
pub async fn refresh_host_display(s: &AppState, device_id: &str) {
    if !s.share.hosted_by(device_id) {
        return;
    }
    if let Ok(screen) = host_display(s).await
        && s.share.set_display(screen.clone())
    {
        tracing::info!("partage : l'écran de l'hôte est maintenant {} ({})", screen.id, screen.name);
    }
}

/// Les écrans et le son suivent le partage du PC (pas d'écran virtuel
/// automatique, pas de sortie son changée tant qu'il est ouvert) : réglés
/// d'après le partage tel qu'il est, jamais d'après ce qu'on croit avoir
/// fait (une ouverture et un arrêt qui se croisent ne les laissent pas
/// faux ; la veille les relit chaque seconde).
fn sync_sharing(s: &AppState) {
    let local = s.share.local_open();
    s.display.set_sharing(local);
    s.audio.set_sharing(local);
}

fn opened(s: &AppState, snapshot: &Snapshot) {
    sync_sharing(s);
    s.activity.event("info", Msg::new("events.shareOpened").with("host", host_name(snapshot)).with("n", snapshot.display.index));
}

/// Arrête le partage ouvert : les invités sont coupés de Kyber (leur signe de
/// vie suivant dit « arrêté »). Rend ce qu'il était.
pub async fn close_share(s: &AppState) -> Option<Snapshot> {
    let snapshot = s.share.close()?;
    sync_sharing(s);
    // Tous les invités à la fois : un Kyber lent ne fait attendre qu'une fois.
    let mut kicks = tokio::task::JoinSet::new();
    for g in snapshot.guests.clone() {
        let s = s.clone();
        kicks.spawn(async move { kyber_close(&s, &g).await });
    }
    while kicks.join_next().await.is_some() {}
    tracing::info!("partage arrêté ({} invité(s))", snapshot.guests.len());
    s.activity.event("info", Msg::new("events.shareStopped"));
    Some(snapshot)
}

/// Un invité sort (parti, retiré, sans nouvelles) : coupé de Kyber, et le fil le dit.
async fn guest_gone(s: &AppState, guest: &Guest) {
    kyber_close(s, guest).await;
    s.activity.event("info", Msg::new("events.shareLeft").with("name", guest.name.as_str()));
}

fn no_share() -> ShareError {
    ShareError::new(StatusCode::NOT_FOUND, "no_share", Msg::new("share.noShare"))
}

fn unknown_guest() -> ShareError {
    ShareError::new(StatusCode::NOT_FOUND, "unknown_guest", Msg::new("share.unknownGuest"))
}

/// Corps de `POST …/guest/{id}`.
#[derive(Deserialize)]
pub struct RoleBody {
    role: Role,
}

async fn change_role(s: &AppState, id: &str, role: Role) -> ShareResult<Json<Value>> {
    let change = s.share.set_role(id, role).ok_or_else(unknown_guest)?;
    if change.before != role {
        kyber_role(s, &change.guest).await;
        let g = &change.guest;
        tracing::info!("partage : {} passe de {} à {}", g.name, change.before.as_str(), role.as_str());
        // « joue avec vous » dès qu'un droit est donné (manettes, ou clavier
        // et souris) ; « regarde votre écran » quand il n'en reste aucun.
        if change.before.gains_in(role) {
            s.activity.event("info", Msg::new("events.sharePlaying").with("name", g.name.as_str()));
        } else if !role.plays() {
            s.activity.event("info", Msg::new("events.shareWatching").with("name", g.name.as_str()));
        }
    }
    Ok(Json(current_view(s)))
}

async fn remove_guest(s: &AppState, id: &str) -> ShareResult<Json<Value>> {
    let guest = s.share.remove(id).ok_or_else(unknown_guest)?;
    tracing::info!("partage : {} retiré", guest.name);
    guest_gone(s, &guest).await;
    Ok(Json(current_view(s)))
}

async fn new_code(s: &AppState) -> ShareResult<Json<Value>> {
    let snapshot = s.share.renew_code().ok_or_else(no_share)?;
    Ok(Json(view(s, &snapshot)))
}

// --- L'hôte -------------------------------------------------------------------------

/// Corps facultatif des routes de l'hôte : l'identifiant de l'appareil, pour
/// distinguer deux appareils sur le même compte (comme le signe de vie).
#[derive(Deserialize, Default)]
struct HostBody {
    #[serde(default)]
    device_id: Option<String>,
}

#[derive(Deserialize, Default)]
pub struct HostQuery {
    #[serde(default)]
    device_id: Option<String>,
}

fn host_body(body: &axum::body::Bytes) -> HostBody {
    serde_json::from_slice(body).unwrap_or_default()
}

/// L'appareil en session qui appelle (compte Kyber, ou la session d'Admin
/// pour le client web). Les autres : 409 `not_in_session`.
fn host_session(s: &AppState, headers: &HeaderMap, device_id: Option<&str>) -> ShareResult<Live> {
    let user = stream_user(s, headers)?;
    s.activity
        .live()
        .filter(|l| match device_id {
            Some(id) => l.device_id == id,
            None => l.user == user,
        })
        .ok_or_else(|| ShareError::new(StatusCode::CONFLICT, "not_in_session", Msg::new("share.notInSession")))
}

/// Le partage de l'hôte qui appelle : 404 `no_share` s'il n'en a pas d'ouvert.
fn host_share(s: &AppState, live: &Live) -> ShareResult<()> {
    if s.share.hosted_by(&live.device_id) { Ok(()) } else { Err(no_share()) }
}

/// `POST /api/share/open` : l'hôte partage son écran principal.
pub async fn host_open(State(s): State<AppState>, headers: HeaderMap, body: axum::body::Bytes) -> ShareResult<Json<Value>> {
    let body = host_body(&body);
    let live = host_session(&s, &headers, body.device_id.as_deref())?;
    if !enabled(&s) {
        return Err(ShareError::new(StatusCode::CONFLICT, "disabled", Msg::new("share.disabled")));
    }
    let display = host_display(&s).await?;
    let origin = Origin::Host { device_id: live.device_id.clone(), user: live.user.clone(), name: live.name.clone() };
    let (snapshot, fresh) = match s.share.open(origin.clone(), display.clone()) {
        Ok(opened) => opened,
        // Le partage d'un autre (un appareil remplacé, le PC) : celui de
        // l'appareil en session passe avant.
        Err(_) => {
            close_share(&s).await;
            s.share.open(origin, display).map_err(|_| ShareError::new(StatusCode::CONFLICT, "other_share", Msg::new("share.otherShare")))?
        }
    };
    if fresh {
        tracing::info!("partage ouvert par {} (écran {})", live.name, snapshot.display.id);
        opened(&s, &snapshot);
    }
    Ok(Json(view(&s, &snapshot)))
}

/// `GET /api/share` : le partage de l'hôte, ou `{"open": false}`.
pub async fn host_get(State(s): State<AppState>, headers: HeaderMap, Query(q): Query<HostQuery>) -> ShareResult<Json<Value>> {
    let live = host_session(&s, &headers, q.device_id.as_deref())?;
    Ok(Json(if s.share.hosted_by(&live.device_id) { current_view(&s) } else { json!({ "open": false }) }))
}

/// `POST /api/share/code` : nouveau code.
pub async fn host_code(State(s): State<AppState>, headers: HeaderMap, body: axum::body::Bytes) -> ShareResult<Json<Value>> {
    let live = host_session(&s, &headers, host_body(&body).device_id.as_deref())?;
    host_share(&s, &live)?;
    new_code(&s).await
}

/// `POST /api/share/guest/{id}` `{"role"}` : rôle d'un invité.
pub async fn host_guest_role(State(s): State<AppState>, headers: HeaderMap, Path(id): Path<String>, body: axum::body::Bytes) -> ShareResult<Json<Value>> {
    let value: Value = serde_json::from_slice(&body).map_err(|e| ShareError::new(StatusCode::BAD_REQUEST, "bad_request", Msg::new("api.unreadableBody").with("error", e.to_string())))?;
    let device_id = value["device_id"].as_str().map(str::to_owned);
    let live = host_session(&s, &headers, device_id.as_deref())?;
    host_share(&s, &live)?;
    let RoleBody { role } = serde_json::from_value(value).map_err(|e| ShareError::new(StatusCode::BAD_REQUEST, "bad_request", Msg::new("api.unreadableBody").with("error", e.to_string())))?;
    change_role(&s, &id, role).await
}

/// `DELETE /api/share/guest/{id}` : retirer un invité.
pub async fn host_guest_remove(State(s): State<AppState>, headers: HeaderMap, Path(id): Path<String>, Query(q): Query<HostQuery>) -> ShareResult<Json<Value>> {
    let live = host_session(&s, &headers, q.device_id.as_deref())?;
    host_share(&s, &live)?;
    remove_guest(&s, &id).await
}

/// `DELETE /api/share` : arrêter le partage.
pub async fn host_close(State(s): State<AppState>, headers: HeaderMap, Query(q): Query<HostQuery>) -> ShareResult<Json<Value>> {
    let live = host_session(&s, &headers, q.device_id.as_deref())?;
    host_share(&s, &live)?;
    close_share(&s).await;
    Ok(Json(json!({ "open": false })))
}

// --- Le PC lui-même (icône, accueil d'Admin) ------------------------------------------

/// L'icône près de l'horloge (depuis l'ordinateur, avec le jeton local de la
/// session de l'utilisateur), ou Admin (sa session).
fn local_or_admin(s: &AppState, peer: SocketAddr, headers: &HeaderMap) -> ShareResult<()> {
    if crate::auth::session_token(headers).is_some_and(|t| s.auth.session_valid(&t)) {
        return Ok(());
    }
    if !peer.ip().to_canonical().is_loopback() {
        return Err(ShareError::new(StatusCode::FORBIDDEN, "forbidden", Msg::new("api.localOnly")));
    }
    if !s.local_token.accepts(headers) {
        return Err(ShareError::new(StatusCode::FORBIDDEN, "forbidden", Msg::new("api.localToken")));
    }
    Ok(())
}

/// `GET /api/local/displays` : les écrans du PC, l'écran principal d'abord.
pub async fn local_displays(State(s): State<AppState>, ConnectInfo(peer): ConnectInfo<SocketAddr>, headers: HeaderMap) -> ShareResult<Json<Value>> {
    local_or_admin(&s, peer, &headers)?;
    let list: Vec<Value> = displays(&s)
        .await?
        .into_iter()
        .map(|(d, id, index)| {
            json!({
                "id": id,
                "name": d.name,
                "index": index,
                "label": label(index),
                "width": d.width,
                "height": d.height,
                "refresh": d.refresh,
                "primary": d.primary,
                "x": d.x,
                "y": d.y,
            })
        })
        .collect();
    Ok(Json(Value::Array(list)))
}

#[derive(Deserialize)]
pub struct LocalOpenBody {
    display: u32,
}

/// Aucun appareil en session ni en train de se préparer (son écran virtuel
/// éteindrait l'écran partagé) : sinon `session` (avec son nom) ou `busy`.
fn device_free(s: &AppState) -> ShareResult<()> {
    if let Some(live) = s.activity.live() {
        return Err(ShareError::new(StatusCode::CONFLICT, "session", Msg::new("share.inSession").with("name", live.name)));
    }
    if s.preparing.load(std::sync::atomic::Ordering::SeqCst) > 0 {
        return Err(ShareError::new(StatusCode::CONFLICT, "busy", Msg::new("share.busy")));
    }
    Ok(())
}

/// `POST /api/local/share/open` `{"display": <id>}` : partager un écran du PC,
/// sans session. Refusé pendant la session d'un appareil (son écran virtuel
/// éteint ceux du PC) ou une autre connexion qui a pris les écrans ou le son.
pub async fn local_open(
    State(s): State<AppState>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    Json(body): Json<LocalOpenBody>,
) -> ShareResult<Json<Value>> {
    local_or_admin(&s, peer, &headers)?;
    if !enabled(&s) {
        return Err(ShareError::new(StatusCode::CONFLICT, "disabled", Msg::new("share.disabled")));
    }
    device_free(&s)?;
    let (d, a) = (s.display.clone(), s.audio.clone());
    let taken = blocking(move || Ok(!d.status()["session"].is_null() || !a.status()["session"].is_null())).await?;
    if taken {
        return Err(ShareError::new(StatusCode::CONFLICT, "busy", Msg::new("share.busy")));
    }
    let (chosen, id, index) = displays(&s)
        .await?
        .into_iter()
        .find(|(_, id, _)| *id == body.display)
        .ok_or_else(|| ShareError::new(StatusCode::NOT_FOUND, "unknown_display", Msg::new("share.unknownDisplay")))?;
    let display = ShareDisplay { id, index, width: chosen.width, height: chosen.height, refresh: chosen.refresh, name: chosen.name };
    let (snapshot, fresh) =
        s.share.open(Origin::Local, display).map_err(|_| ShareError::new(StatusCode::CONFLICT, "other_share", Msg::new("share.otherShare")))?;
    // La lecture des écrans prend du temps (Kyber, l'agent de la session de
    // l'écran) : un appareil a pu commencer à se préparer entre-temps, après
    // avoir vu qu'aucun partage n'était ouvert. Revu une fois ouvert : l'un
    // des deux voit toujours l'autre.
    if fresh && let Err(refusal) = device_free(&s) {
        close_share(&s).await;
        return Err(refusal);
    }
    if fresh {
        tracing::info!("partage de l'écran {} ({}) ouvert depuis le PC", snapshot.display.index, snapshot.display.name);
        opened(&s, &snapshot);
    }
    Ok(Json(view(&s, &snapshot)))
}

/// `GET /api/local/share` : le partage en cours (de l'hôte ou du PC), ou `{"open": false}`.
pub async fn local_get(State(s): State<AppState>, ConnectInfo(peer): ConnectInfo<SocketAddr>, headers: HeaderMap) -> ShareResult<Json<Value>> {
    local_or_admin(&s, peer, &headers)?;
    Ok(Json(current_view(&s)))
}

pub async fn local_code(State(s): State<AppState>, ConnectInfo(peer): ConnectInfo<SocketAddr>, headers: HeaderMap) -> ShareResult<Json<Value>> {
    local_or_admin(&s, peer, &headers)?;
    new_code(&s).await
}

pub async fn local_guest_role(
    State(s): State<AppState>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(body): Json<RoleBody>,
) -> ShareResult<Json<Value>> {
    local_or_admin(&s, peer, &headers)?;
    change_role(&s, &id, body.role).await
}

pub async fn local_guest_remove(
    State(s): State<AppState>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> ShareResult<Json<Value>> {
    local_or_admin(&s, peer, &headers)?;
    remove_guest(&s, &id).await
}

/// `DELETE /api/local/share` : arrêter le partage en cours (celui d'un hôte aussi).
pub async fn local_close(State(s): State<AppState>, ConnectInfo(peer): ConnectInfo<SocketAddr>, headers: HeaderMap) -> ShareResult<Json<Value>> {
    local_or_admin(&s, peer, &headers)?;
    close_share(&s).await;
    Ok(Json(json!({ "open": false })))
}

// --- L'invité -------------------------------------------------------------------------

#[derive(Deserialize)]
pub struct JoinBody {
    code: String,
    name: String,
    #[serde(default)]
    device: String,
    #[serde(default)]
    app: String,
    /// « Qualité de l'image » (absent : « Auto »).
    #[serde(default)]
    quality: share::Quality,
    /// Pour « Auto » : ce que l'appareil sait afficher et décoder.
    #[serde(default)]
    auto: Option<AutoCaps>,
}

/// `POST /api/share/join` : rejoindre avec le code (sans compte ; 10 essais
/// par minute et par adresse, comme l'appairage).
pub async fn join(State(s): State<AppState>, ConnectInfo(peer): ConnectInfo<SocketAddr>, Json(body): Json<JoinBody>) -> Response {
    let browser = body.app == "web";
    let joined = match s.share.join(&body.code, &body.name, &body.device, &body.app, peer.ip(), browser) {
        Ok(joined) => joined,
        Err(JoinError::BadRequest) => return guest_error(StatusCode::BAD_REQUEST, "bad_request"),
        Err(JoinError::NoShare) => return guest_error(StatusCode::NOT_FOUND, "no_share"),
        Err(JoinError::Full) => return guest_error(StatusCode::CONFLICT, "full"),
        Err(JoinError::TooMany(wait)) => {
            tracing::warn!("partage : essais refusés depuis {} (trop d'essais)", peer.ip());
            let mut res = (StatusCode::TOO_MANY_REQUESTS, Json(json!({ "error": "too_many", "retry_after": wait }))).into_response();
            res.headers_mut().insert(header::RETRY_AFTER, HeaderValue::from(wait));
            return res;
        }
    };
    let (guest, snapshot) = (joined.guest, joined.snapshot);
    let display = &snapshot.display;
    let preset = resolve_quality(body.quality, body.auto.as_ref(), display);
    let guest = s.share.set_quality(&guest.id, None, body.quality, preset).unwrap_or(guest);
    let Some(token) = guest_token(&s, &guest, display.id) else {
        // Sans clé de jetons, Kyber ne le laisserait pas entrer.
        let _ = s.share.leave(&guest.id, guest.secret());
        tracing::warn!("partage : pas de clé pour les jetons de Kyber, {} ne peut pas entrer", guest.name);
        return guest_error(StatusCode::SERVICE_UNAVAILABLE, "unavailable");
    };
    if joined.new {
        tracing::info!("partage : {} rejoint ({}, {})", guest.name, guest.app, peer.ip());
        s.activity.event("info", Msg::new("events.shareWatching").with("name", guest.name.as_str()));
    } else {
        // Le même appareil qui revient : Kyber retient la qualité poussée en
        // cours de partage (et un cran descendu pour retard), qu'il
        // préférerait à celle de ce jeton ; il prend celle d'aujourd'hui.
        kyber_quality(&s, &guest).await;
    }
    let stream = kyber_stream(&s, display.id).await;
    let answer = json!({
        "guest": guest.id,
        "secret": guest.secret(),
        "role": guest.role,
        "host": host_name(&snapshot),
        "computer": hostname(),
        "kyber": {
            "port": s.kyber.port(),
            "fingerprint": crate::kyber_proxy::kyber_fingerprint(&s),
            "token": token,
            "expires": share::TOKEN_LIFETIME,
        },
        "display": { "id": display.id, "width": display.width, "height": display.height, "refresh": display.refresh },
        "codec": stream.as_ref().map(|(codec, _)| codec.clone()),
        "yuv444": stream.is_some_and(|(_, yuv444)| yuv444),
        "beat": share::BEAT_SECONDS,
    });
    let mut res = ([(header::CACHE_CONTROL, "no-store")], Json(answer)).into_response();
    if let Some(cookie) = guest.cookie()
        && let Ok(value) = HeaderValue::from_str(&share::set_cookie(cookie, s.secure_cookies))
    {
        res.headers_mut().insert(header::SET_COOKIE, value);
    }
    res
}

#[derive(Deserialize)]
pub struct GuestBody {
    guest: String,
}

fn guest_secret(headers: &HeaderMap) -> String {
    headers.get(GUEST_HEADER).and_then(|v| v.to_str().ok()).unwrap_or_default().trim().to_string()
}

/// `POST /api/share/beat` : signe de vie de l'invité, toutes les 5 s ; il
/// rend un jeton de Kyber renouvelé et, s'il est sorti, pourquoi.
pub async fn beat(State(s): State<AppState>, headers: HeaderMap, Json(body): Json<GuestBody>) -> Response {
    fresh_images(&s).await;
    let answer = match s.share.beat(&body.guest, &guest_secret(&headers)) {
        None => return guest_error(StatusCode::NOT_FOUND, "unknown_guest"),
        // `display` en plus du contrat : l'écran du partage peut changer
        // (écran virtuel de l'hôte recréé) ; le jeton porte toujours le bon.
        Some(Beat::Live { guest, display }) => match guest_token(&s, &guest, display.id) {
            Some(token) => json!({
                "role": guest.role,
                "token": token,
                "expires": share::TOKEN_LIFETIME,
                "end": null,
                "display": { "id": display.id, "width": display.width, "height": display.height, "refresh": display.refresh },
                "image": image_json(&guest),
            }),
            None => return guest_error(StatusCode::SERVICE_UNAVAILABLE, "unavailable"),
        },
        Some(Beat::Ended { end, role }) => json!({ "role": role, "token": null, "expires": 0, "end": end }),
    };
    ([(header::CACHE_CONTROL, "no-store")], Json(answer)).into_response()
}

#[derive(Deserialize)]
pub struct QualityBody {
    guest: String,
    quality: share::Quality,
    #[serde(default)]
    auto: Option<AutoCaps>,
}

/// `POST /api/share/quality` : l'invité change sa « Qualité de l'image » ;
/// Kyber relance son image seulement. Rend l'`image` du signe de vie. Un
/// autre préréglage que Kyber ne prend pas : `503 unavailable`, rien ne
/// change (ni le jeton, ni ce que l'invité reçoit), il peut réessayer.
pub async fn quality(State(s): State<AppState>, headers: HeaderMap, Json(body): Json<QualityBody>) -> Response {
    let secret = guest_secret(&headers);
    let Some(display) = s.share.snapshot().map(|snap| snap.display) else {
        return guest_error(StatusCode::NOT_FOUND, "unknown_guest");
    };
    let before = s.share.guests().into_iter().find(|g| g.id == body.guest).map(|g| (g.quality, g.preset));
    let preset = resolve_quality(body.quality, body.auto.as_ref(), &display);
    let Some(guest) = s.share.set_quality(&body.guest, Some(&secret), body.quality, preset) else {
        return guest_error(StatusCode::NOT_FOUND, "unknown_guest");
    };
    if let Some((quality_before, preset_before)) = before
        && preset_before != preset
    {
        tracing::info!("partage : {} demande {} ({})", guest.name, body.quality.as_str(), preset.as_str());
        if !kyber_quality(&s, &guest).await {
            // Gardée, elle partirait dans le jeton sans que Kyber relance
            // l'image, et la même demande ne serait plus jamais poussée.
            let _ = s.share.set_quality(&body.guest, Some(&secret), quality_before, preset_before);
            return guest_error(StatusCode::SERVICE_UNAVAILABLE, "unavailable");
        }
    }
    refresh_images(&s).await;
    ([(header::CACHE_CONTROL, "no-store")], Json(image_json(&guest))).into_response()
}

/// `POST /api/share/leave` : l'invité quitte le partage. Un invité inconnu
/// (comme pour `beat`) : 404 `unknown_guest` ; le cookie du navigateur est
/// effacé dans tous les cas.
pub async fn leave(State(s): State<AppState>, headers: HeaderMap, Json(body): Json<GuestBody>) -> Response {
    let mut res = match s.share.leave(&body.guest, &guest_secret(&headers)) {
        share::Leave::Left(guest) => {
            tracing::info!("partage : {} part", guest.name);
            guest_gone(&s, &guest).await;
            Json(json!({ "ok": true })).into_response()
        }
        share::Leave::Ended => Json(json!({ "ok": true })).into_response(),
        share::Leave::Unknown => guest_error(StatusCode::NOT_FOUND, "unknown_guest"),
    };
    if share::guest_cookie(&headers).is_some()
        && let Ok(value) = HeaderValue::from_str(&share::clear_cookie(s.secure_cookies))
    {
        res.headers_mut().insert(header::SET_COOKIE, value);
    }
    res
}

// --- Veille ----------------------------------------------------------------------------

/// Chaque seconde, avec un partage ouvert : les invités sans nouvelles depuis
/// 20 s sortent ; le partage d'un hôte s'arrête quand sa session finit
/// (déconnexion, reprise de main par un autre appareil, plus de signe de vie).
pub fn supervise(s: AppState) {
    tokio::spawn(async move {
        let mut tick = tokio::time::interval(Duration::from_secs(1));
        loop {
            tick.tick().await;
            supervise_tick(&s).await;
        }
    });
}

/// Un tour de la veille.
async fn supervise_tick(s: &AppState) {
    sync_sharing(s);
    let Some(snapshot) = s.share.snapshot() else { return };
    // Les images des invités (vue de l'hôte, signe de vie), relues dans
    // Kyber toutes les 2 s au plus.
    if !snapshot.guests.is_empty() {
        fresh_images(s).await;
    }
    for guest in s.share.expire() {
        tracing::info!("partage : {} ne donne plus de nouvelles", guest.name);
        guest_gone(s, &guest).await;
    }
    match snapshot.origin {
        Origin::Host { device_id, name, .. } if s.activity.live().is_none_or(|l| l.device_id != device_id) => {
            tracing::info!("partage de {name} arrêté : sa session est finie");
            close_share(s).await;
        }
        // Une session d'appareil a commencé quand même (sa préparation a
        // regardé juste avant que le PC ouvre son partage) : son écran
        // virtuel éteindrait celui que regardent les invités.
        Origin::Local => {
            if let Some(live) = s.activity.live() {
                tracing::info!("partage du PC arrêté : {} est en session", live.name);
                close_share(s).await;
            }
        }
        Origin::Host { .. } => {}
    }
}

#[cfg(all(test, not(windows)))]
mod tests {
    use std::path::PathBuf;
    use std::sync::Mutex;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    use axum::body::Bytes;

    use super::*;
    use crate::activity::DeviceInfo;
    use crate::service::{ServiceControl, ServiceState};

    struct Running(Mutex<bool>, Arc<AtomicUsize>);

    impl ServiceControl for Running {
        fn state(&self) -> anyhow::Result<ServiceState> {
            Ok(if *self.0.lock().unwrap() { ServiceState::Running } else { ServiceState::Stopped })
        }
        fn start(&self) -> anyhow::Result<()> {
            *self.0.lock().unwrap() = true;
            Ok(())
        }
        fn stop(&self) -> anyhow::Result<()> {
            *self.0.lock().unwrap() = false;
            self.1.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
    }

    struct Station {
        s: AppState,
        dir: PathBuf,
        restarts: Arc<AtomicUsize>,
    }

    impl Drop for Station {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.dir);
        }
    }

    /// Une Station de test : un compte Kyber `mac`, une clé de jetons.
    fn station() -> Station {
        let dir = std::env::temp_dir().join(format!("kn-share-{}-{}", std::process::id(), rand::random::<u64>()));
        std::fs::create_dir_all(&dir).unwrap();
        let restarts = Arc::new(AtomicUsize::new(0));
        let s = AppState::for_tests(dir.clone(), Box::new(Running(Mutex::new(true), restarts.clone())));
        std::fs::write(dir.join("kyber_config.toml"), "").unwrap();
        s.kyber.upsert_user("mac", "mot-de-passe-mac").unwrap();
        s.kyber.set_jwt_secret("cle-de-test").unwrap();
        Station { s, dir, restarts }
    }

    fn basic(user: &str, password: &str) -> HeaderMap {
        use std::io::Write;
        let mut raw = Vec::new();
        write!(raw, "{user}:{password}").unwrap();
        const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
        let mut out = String::new();
        for chunk in raw.chunks(3) {
            let n = chunk.iter().enumerate().fold(0u32, |n, (i, b)| n | (u32::from(*b) << (16 - 8 * i)));
            for i in 0..4 {
                out.push(if i <= chunk.len() { ALPHABET[(n >> (18 - 6 * i) & 63) as usize] as char } else { '=' });
            }
        }
        let mut h = HeaderMap::new();
        h.insert(header::AUTHORIZATION, format!("Basic {out}").parse().unwrap());
        h
    }

    fn peer(n: u8) -> ConnectInfo<SocketAddr> {
        ConnectInfo(SocketAddr::from(([192, 168, 1, n], 50000)))
    }

    fn local() -> ConnectInfo<SocketAddr> {
        ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 50000)))
    }

    fn admin(s: &AppState) -> HeaderMap {
        let mut h = HeaderMap::new();
        h.insert(header::COOKIE, format!("{}={}", crate::auth::COOKIE, s.auth.open_session()).parse().unwrap());
        h
    }

    async fn body(res: Response) -> (StatusCode, Value) {
        let status = res.status();
        let bytes = axum::body::to_bytes(res.into_body(), 1 << 20).await.unwrap();
        (status, serde_json::from_slice(&bytes).unwrap_or(Value::Null))
    }

    fn join_body(code: &str, name: &str, device: &str, app: &str) -> Json<JoinBody> {
        Json(serde_json::from_value(json!({ "code": code, "name": name, "device": device, "app": app })).unwrap())
    }

    fn guest_headers(secret: &str) -> HeaderMap {
        let mut h = HeaderMap::new();
        h.insert(GUEST_HEADER, secret.parse().unwrap());
        h
    }

    /// Décode les revendications d'un jeton (sans vérifier la signature).
    fn claims(token: &str) -> Value {
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
        serde_json::from_slice(&out).unwrap()
    }

    fn mac() -> DeviceInfo {
        DeviceInfo { id: "mac-1".into(), name: "MacBook de Léa".into(), kind: Some("mac".into()), os: None, app: None }
    }

    fn texts(s: &AppState) -> Vec<String> {
        s.activity.events(30, i18n::Lang::Fr).into_iter().map(|e| e.text).collect()
    }

    #[tokio::test]
    async fn l_hote_partage_son_ecran_et_l_invite_rejoint() {
        let st = station();
        let s = &st.s;
        // Pas en session : refusé.
        let refused = host_open(State(s.clone()), basic("mac", "mot-de-passe-mac"), Bytes::new()).await.err().unwrap();
        assert_eq!((refused.status, refused.code), (StatusCode::CONFLICT, "not_in_session"));
        s.activity.begin(Some(mac()), "mac", None, None);
        let Json(opened) = host_open(State(s.clone()), basic("mac", "mot-de-passe-mac"), Bytes::from(r#"{"device_id":"mac-1"}"#)).await.ok().unwrap();
        assert_eq!(opened["origin"], "host");
        assert_eq!(opened["host"], "MacBook de Léa");
        let code = opened["code"].as_str().unwrap().to_string();
        assert_eq!(code.len(), 6);
        assert!(opened["link"].as_str().unwrap().starts_with("kybernow://join?host="));
        assert!(opened["link"].as_str().unwrap().ends_with(&format!("&code={code}")));
        assert!(opened["web"].as_str().unwrap().ends_with(&format!("/link/?join={code}")));
        // Sans écran virtuel : l'écran principal de Windows (simulé : DISPLAY1 en 3840 × 2160).
        assert_eq!(opened["display"]["id"], kyber_display_id("\\\\.\\DISPLAY1"));
        assert_eq!(opened["display"]["label"], "Écran 1");
        assert!(texts(s)[0].contains("partage l'écran 1"), "{:?}", texts(s));
        // Le signe de vie de l'hôte porte le partage ; pas celui d'un autre appareil.
        assert!(for_host(s, Some("mac-1")).is_some() && for_host(s, Some("tv")).is_none());

        let (status, joined) = body(join(State(s.clone()), peer(20), join_body(&code, "Léa", "ipad-1", "ios")).await).await;
        assert_eq!(status, StatusCode::OK, "{joined}");
        assert_eq!(joined["role"], "spectator");
        assert_eq!(joined["host"], "MacBook de Léa");
        assert_eq!(joined["kyber"]["expires"], 120);
        assert_eq!(joined["beat"], 5);
        assert_eq!(joined["display"]["width"], 3840);
        assert!(joined["codec"].is_null(), "Kyber absent : format inconnu");
        let token = claims(joined["kyber"]["token"].as_str().unwrap());
        assert_eq!(token["aud"], "kyber");
        assert_eq!(token["kn_role"], "spectator");
        assert_eq!(token["kn_display"], opened["display"]["id"]);
        assert_eq!(token["kn_rev"], 1);
        assert!(token["sub"].as_str().unwrap().starts_with("kn-guest-"));
        assert!(texts(s)[0] == "Léa regarde votre écran");

        // L'hôte la fait jouer, puis lui donne le clavier et la souris.
        let guest = joined["guest"].as_str().unwrap().to_string();
        let secret = joined["secret"].as_str().unwrap().to_string();
        let role = |r: &str| Bytes::from(json!({ "role": r, "device_id": "mac-1" }).to_string());
        let Json(v) = host_guest_role(State(s.clone()), basic("mac", "mot-de-passe-mac"), Path(guest.clone()), role("player")).await.ok().unwrap();
        assert_eq!(v["guests"][0]["role"], "player");
        assert_eq!(texts(s)[0], "Léa joue avec vous");
        let _ = host_guest_role(State(s.clone()), basic("mac", "mot-de-passe-mac"), Path(guest.clone()), role("player_kbm")).await.ok().unwrap();
        let (_, beat_v) = body(beat(State(s.clone()), guest_headers(&secret), Json(GuestBody { guest: guest.clone() })).await).await;
        assert_eq!(beat_v["role"], "player_kbm");
        assert_eq!(beat_v["display"]["id"], opened["display"]["id"]);
        assert!(beat_v["end"].is_null());
        let renewed = claims(beat_v["token"].as_str().unwrap());
        assert_eq!((renewed["kn_role"].as_str(), renewed["kn_rev"].as_u64()), (Some("player_kbm"), Some(3)));
        // Un mauvais secret : inconnu.
        let (status, _) = body(beat(State(s.clone()), guest_headers("faux"), Json(GuestBody { guest: guest.clone() })).await).await;
        assert_eq!(status, StatusCode::NOT_FOUND);

        // L'hôte se déconnecte : le partage s'arrête, l'invité l'apprend.
        assert!(s.activity.end("mac", Some("mac-1"), None));
        assert!(s.activity.live().is_none());
        close_share(s).await;
        let (_, ended) = body(beat(State(s.clone()), guest_headers(&secret), Json(GuestBody { guest })).await).await;
        assert_eq!(ended["end"], "stopped");
        assert!(ended["token"].is_null());
        assert_eq!(texts(s)[0], "Partage arrêté");
    }

    #[tokio::test]
    async fn manettes_et_clavier_souris_donnes_l_un_sans_l_autre() {
        let st = station();
        let s = &st.s;
        let Json(opened) = local_open(State(s.clone()), peer(5), admin(s), Json(LocalOpenBody { display: kyber_display_id("\\\\.\\DISPLAY1") })).await.ok().unwrap();
        let code = opened["code"].as_str().unwrap();
        let (_, joined) = body(join(State(s.clone()), peer(20), join_body(code, "Léa", "ipad", "ios")).await).await;
        let (guest, secret) = (joined["guest"].as_str().unwrap().to_string(), joined["secret"].as_str().unwrap().to_string());
        let set = |r: &'static str| local_guest_role(State(s.clone()), peer(5), admin(s), Path(guest.clone()), Json(serde_json::from_value(json!({ "role": r })).unwrap()));
        let token_role = || async {
            let (_, b) = body(beat(State(s.clone()), guest_headers(&secret), Json(GuestBody { guest: guest.clone() })).await).await;
            (b["role"].as_str().unwrap().to_string(), claims(b["token"].as_str().unwrap())["kn_role"].as_str().unwrap().to_string())
        };
        // Le clavier et la souris d'abord, sans les manettes.
        let Json(v) = set("kbm").await.ok().unwrap();
        assert_eq!(v["guests"][0]["role"], "kbm");
        assert_eq!(token_role().await, ("kbm".to_string(), "kbm".to_string()));
        assert_eq!(texts(s)[0], "Léa joue avec vous");
        // Puis les manettes : un droit de plus, le fil le dit encore.
        let _ = set("player_kbm").await.ok().unwrap();
        assert_eq!(texts(s)[0], "Léa joue avec vous");
        assert_eq!(texts(s)[1], "Léa joue avec vous");
        // Le clavier et la souris repris : il reste les manettes, rien au fil.
        let _ = set("player").await.ok().unwrap();
        assert_eq!(texts(s).iter().filter(|t| t.starts_with("Léa")).count(), 3);
        assert_eq!(token_role().await.1, "player");
        // Plus aucun droit.
        let _ = set("spectator").await.ok().unwrap();
        assert_eq!(texts(s)[0], "Léa regarde votre écran");
    }

    fn caps(width: u32, height: u32, fps: u32) -> AutoCaps {
        AutoCaps { width, height, fps, codecs: vec!["h264".into()], yuv444: false }
    }

    #[test]
    fn auto_choisit_d_apres_l_ecran_et_la_frequence() {
        use share::{Preset, Quality};
        let host_4k160 = ShareDisplay { id: 1, index: 1, width: 3840, height: 2160, refresh: 160, name: String::new() };
        let host_1080 = ShareDisplay { id: 1, index: 1, width: 1920, height: 1080, refresh: 60, name: String::new() };
        // iPhone dans Safari (écran × densité, 60 Hz au plus) : 1080p60.
        assert_eq!(resolve_quality(Quality::Auto, Some(&caps(1170, 2532, 60)), &host_4k160), Preset::P1080);
        // Un téléviseur 4K à 60 Hz face à un hôte à 160 Hz : 1080p60, pas l'image de l'hôte.
        assert_eq!(resolve_quality(Quality::Auto, Some(&caps(3840, 2160, 60)), &host_4k160), Preset::P1080);
        // Un appareil qui couvre l'écran et suit sa fréquence : l'image de l'hôte.
        assert_eq!(resolve_quality(Quality::Auto, Some(&caps(5120, 2880, 165)), &host_4k160), Preset::Host);
        assert_eq!(resolve_quality(Quality::Auto, Some(&caps(1920, 1080, 60)), &host_1080), Preset::Host);
        // Petit écran, ou moins de 60 Hz : 720p30.
        assert_eq!(resolve_quality(Quality::Auto, Some(&caps(1334, 750, 60)), &host_4k160), Preset::P720);
        assert_eq!(resolve_quality(Quality::Auto, Some(&caps(1920, 1080, 30)), &host_4k160), Preset::P720);
        // Sans ce que sait l'appareil (app d'avant 0.8.0) : comme avant, l'image de l'hôte.
        assert_eq!(resolve_quality(Quality::Auto, None, &host_4k160), Preset::Host);
        // Un choix explicite passe tel quel.
        assert_eq!(resolve_quality(Quality::Host, Some(&caps(640, 480, 30)), &host_4k160), Preset::Host);
        assert_eq!(resolve_quality(Quality::P720, None, &host_1080), Preset::P720);
    }

    #[tokio::test]
    async fn la_qualite_va_dans_le_jeton_et_le_signe_de_vie() {
        let st = station();
        let s = &st.s;
        let Json(opened) = local_open(State(s.clone()), peer(5), admin(s), Json(LocalOpenBody { display: kyber_display_id("\\\\.\\DISPLAY1") })).await.ok().unwrap();
        let code = opened["code"].as_str().unwrap();
        // L'écran simulé est en 3840 × 2160 à 144 Hz ; un iPhone en « Auto ».
        let request = json!({ "code": code, "name": "Léa", "device": "iphone", "app": "web", "quality": "auto", "auto": { "width": 1170, "height": 2532, "fps": 60, "codecs": ["h264", "hevc"], "yuv444": false } });
        let (status, joined) = body(join(State(s.clone()), peer(20), Json(serde_json::from_value(request).unwrap())).await).await;
        assert_eq!(status, StatusCode::OK, "{joined}");
        assert_eq!(claims(joined["kyber"]["token"].as_str().unwrap())["kn_quality"], "1080p60");
        let (guest, secret) = (joined["guest"].as_str().unwrap().to_string(), joined["secret"].as_str().unwrap().to_string());
        let (_, beat_v) = body(beat(State(s.clone()), guest_headers(&secret), Json(GuestBody { guest: guest.clone() })).await).await;
        // Kyber absent : la qualité demandée, le reste encore inconnu.
        assert_eq!(beat_v["image"]["quality"], "auto");
        assert!(beat_v["image"]["width"].is_null() && beat_v["image"]["reason"].is_null());
        assert_eq!(claims(beat_v["token"].as_str().unwrap())["kn_quality"], "1080p60");
        // La vue de l'hôte donne la même image.
        let Json(v) = local_get(State(s.clone()), peer(5), admin(s)).await.ok().unwrap();
        assert_eq!(v["guests"][0]["image"]["quality"], "auto");
        // Changer de qualité en cours : Kyber (absent ici) ne la prend pas,
        // rien ne change (ni le jeton ni l'image lue), l'invité peut réessayer.
        let quality = |q: &str, secret: &str| quality(State(s.clone()), guest_headers(secret), Json(serde_json::from_value(json!({ "guest": guest, "quality": q })).unwrap()));
        let (status, refused) = body(quality("720p30", &secret).await).await;
        assert_eq!((status, refused["error"].as_str()), (StatusCode::SERVICE_UNAVAILABLE, Some("unavailable")));
        let (_, beat_v) = body(beat(State(s.clone()), guest_headers(&secret), Json(GuestBody { guest: guest.clone() })).await).await;
        assert_eq!(claims(beat_v["token"].as_str().unwrap())["kn_quality"], "1080p60");
        assert_eq!(beat_v["image"]["quality"], "auto");
        // Le même préréglage (rien à relancer chez Kyber) : pris tel quel.
        let (status, image) = body(quality("1080p60", &secret).await).await;
        assert_eq!((status, image["quality"].as_str()), (StatusCode::OK, Some("1080p60")));
        let (_, beat_v) = body(beat(State(s.clone()), guest_headers(&secret), Json(GuestBody { guest: guest.clone() })).await).await;
        assert_eq!(claims(beat_v["token"].as_str().unwrap())["kn_quality"], "1080p60");
        let (status, _) = body(quality("host", "faux").await).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        // Sans `quality` (app d'avant 0.8.0) : « Auto » sans rien savoir, l'image de l'hôte.
        let (_, old) = body(join(State(s.clone()), peer(21), join_body(code, "Max", "tv", "tvos")).await).await;
        assert_eq!(claims(old["kyber"]["token"].as_str().unwrap())["kn_quality"], "host");
    }

    #[tokio::test]
    async fn rejoindre_limite_complet_et_codes() {
        let st = station();
        let s = &st.s;
        let (status, v) = body(join(State(s.clone()), peer(1), join_body("123456", "Léa", "ipad", "ios")).await).await;
        assert_eq!((status, v["error"].as_str()), (StatusCode::NOT_FOUND, Some("no_share")));
        let Json(opened) = local_open(State(s.clone()), local_with_token(s), local_token_headers(s), Json(LocalOpenBody { display: kyber_display_id("\\\\.\\DISPLAY2") })).await.ok().unwrap();
        assert_eq!((opened["origin"].as_str(), opened["display"]["index"].as_u64()), (Some("local"), Some(2)));
        assert_eq!(opened["display"]["label"], "Écran 2");
        let code = opened["code"].as_str().unwrap().to_string();
        for i in 0..4 {
            let (status, _) = body(join(State(s.clone()), peer(10 + i), join_body(&code, "Invité", &format!("d{i}"), "mac")).await).await;
            assert_eq!(status, StatusCode::OK);
        }
        let (status, v) = body(join(State(s.clone()), peer(30), join_body(&code, "Cinq", "d5", "mac")).await).await;
        assert_eq!((status, v["error"].as_str()), (StatusCode::CONFLICT, Some("full")));
        let (status, v) = body(join(State(s.clone()), peer(30), join_body("12", "Cinq", "d5", "mac")).await).await;
        assert_eq!((status, v["error"].as_str()), (StatusCode::BAD_REQUEST, Some("bad_request")));
        // Dix essais par minute et par adresse.
        for _ in 0..8 {
            body(join(State(s.clone()), peer(31), join_body("000000", "x", "x", "mac")).await).await;
        }
        for _ in 0..2 {
            assert_eq!(join(State(s.clone()), peer(31), join_body("000000", "x", "x", "mac")).await.status(), StatusCode::NOT_FOUND);
        }
        // Le onzième essai, même avec le bon code : refusé.
        let res = join(State(s.clone()), peer(31), join_body(&code, "x", "x", "mac")).await;
        assert_eq!(res.status(), StatusCode::TOO_MANY_REQUESTS);
        assert!(res.headers().contains_key(header::RETRY_AFTER));
        // Nouveau code : l'ancien ne vaut plus, les invités restent.
        let Json(renewed) = local_code(State(s.clone()), local_with_token(s), local_token_headers(s)).await.ok().unwrap();
        assert_ne!(renewed["code"], opened["code"]);
        assert_eq!(renewed["guests"].as_array().unwrap().len(), 4);
        // Retirer, arrêter.
        let first = renewed["guests"][0]["id"].as_str().unwrap().to_string();
        let Json(after) = local_guest_remove(State(s.clone()), local_with_token(s), local_token_headers(s), Path(first)).await.ok().unwrap();
        assert_eq!(after["guests"].as_array().unwrap().len(), 3);
        assert_eq!(texts(s)[0], "Invité a quitté le partage");
        let Json(closed) = local_close(State(s.clone()), local_with_token(s), local_token_headers(s)).await.ok().unwrap();
        assert_eq!(closed, json!({ "open": false }));
        assert!(!s.share.is_open());
    }

    fn local_with_token(s: &AppState) -> ConnectInfo<SocketAddr> {
        s.local_token.renew();
        local()
    }

    fn local_token_headers(s: &AppState) -> HeaderMap {
        let token = std::fs::read_to_string(s.data_dir.join(crate::local_token::FILE)).unwrap();
        let mut h = HeaderMap::new();
        h.insert(crate::local_token::HEADER, token.trim().parse().unwrap());
        h
    }

    #[tokio::test]
    async fn routes_locales_jeton_ou_session_d_admin() {
        let st = station();
        let s = &st.s;
        // Depuis le réseau sans session d'Admin : refusé.
        let refused = local_displays(State(s.clone()), peer(5), HeaderMap::new()).await.err().unwrap();
        assert_eq!((refused.status, refused.code), (StatusCode::FORBIDDEN, "forbidden"));
        // Depuis l'ordinateur sans jeton : refusé.
        let refused = local_displays(State(s.clone()), local(), HeaderMap::new()).await.err().unwrap();
        assert_eq!(refused.status, StatusCode::FORBIDDEN);
        // Avec le jeton de l'icône.
        let Json(list) = local_displays(State(s.clone()), local_with_token(s), local_token_headers(s)).await.ok().unwrap();
        let list = list.as_array().unwrap().clone();
        assert_eq!(list.len(), 2);
        // Le principal d'abord, numéroté 1.
        assert_eq!((list[0]["primary"].as_bool(), list[0]["label"].as_str(), list[0]["width"].as_u64()), (Some(true), Some("Écran 1"), Some(3840)));
        assert_eq!(list[1]["label"], "Écran 2");
        assert_eq!(list[0]["id"], kyber_display_id("\\\\.\\DISPLAY1"));
        // Avec la session d'Admin, depuis le réseau.
        assert!(local_displays(State(s.clone()), peer(5), admin(s)).await.is_ok());
        // Une session d'appareil en cours : le PC ne partage pas un de ses écrans.
        s.activity.begin(Some(mac()), "mac", None, None);
        let refused = local_open(State(s.clone()), peer(5), admin(s), Json(LocalOpenBody { display: kyber_display_id("\\\\.\\DISPLAY1") })).await.err().unwrap();
        assert_eq!((refused.status, refused.code), (StatusCode::CONFLICT, "session"));
        assert!(refused.error.contains("MacBook de Léa"));
        s.activity.end("mac", Some("mac-1"), None);
        let refused = local_open(State(s.clone()), peer(5), admin(s), Json(LocalOpenBody { display: 42 })).await.err().unwrap();
        assert_eq!((refused.status, refused.code), (StatusCode::NOT_FOUND, "unknown_display"));
        // Partage coupé (`multi_client = false`).
        s.kyber.apply(&[("network.multi_client".to_string(), Some(json!(false)))].into_iter().collect()).unwrap();
        let refused = local_open(State(s.clone()), peer(5), admin(s), Json(LocalOpenBody { display: kyber_display_id("\\\\.\\DISPLAY1") })).await.err().unwrap();
        assert_eq!((refused.status, refused.code), (StatusCode::CONFLICT, "disabled"));
        assert_eq!(refused.error, "Le partage est coupé sur cet ordinateur (Admin › Appareils Link).");
    }

    #[tokio::test]
    async fn le_partage_du_pc_garde_les_ecrans_et_le_son() {
        let st = station();
        let s = &st.s;
        s.display.settings.lock().unwrap().trigger = crate::display::Trigger::All;
        let _ = local_open(State(s.clone()), peer(5), admin(s), Json(LocalOpenBody { display: kyber_display_id("\\\\.\\DISPLAY1") })).await.ok().unwrap();
        assert!(s.share.local_open());
        assert!(s.display.sharing() && s.audio.sharing());
        close_share(s).await;
        assert!(!s.display.sharing() && !s.audio.sharing());
        assert_eq!(st.restarts.load(Ordering::SeqCst), 0);
    }

    /// Une session qui commence pendant que le PC ouvre son partage (le
    /// partage lit les écrans, Kyber, l'agent : plusieurs secondes) : le
    /// partage ne reste pas ouvert sous l'écran virtuel de l'appareil.
    #[tokio::test]
    async fn une_session_qui_commence_pendant_le_partage_du_pc() {
        let st = station();
        let s = &st.s;
        let screen = || Json(LocalOpenBody { display: kyber_display_id("\\\\.\\DISPLAY1") });
        // Un appareil se prépare (`prepare` en cours) : pas de partage du PC.
        s.preparing.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let refused = local_open(State(s.clone()), peer(5), admin(s), screen()).await.err().unwrap();
        assert_eq!((refused.status, refused.code), (StatusCode::CONFLICT, "busy"));
        assert!(!s.share.is_open() && !s.display.sharing() && !s.audio.sharing());
        s.preparing.fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
        // Partage ouvert, puis la session de l'appareil commence quand même
        // (sa préparation a regardé avant l'ouverture) : la veille l'arrête.
        let Json(opened) = local_open(State(s.clone()), peer(5), admin(s), screen()).await.ok().unwrap();
        let code = opened["code"].as_str().unwrap();
        let (_, joined) = body(join(State(s.clone()), peer(20), join_body(code, "Léa", "ipad", "ios")).await).await;
        s.activity.begin(Some(mac()), "mac", None, None);
        supervise_tick(s).await;
        assert!(!s.share.is_open(), "le partage du PC ne survit pas à une session d'appareil");
        assert!(!s.display.sharing() && !s.audio.sharing());
        let secret = joined["secret"].as_str().unwrap();
        let (_, ended) = body(beat(State(s.clone()), guest_headers(secret), Json(GuestBody { guest: joined["guest"].as_str().unwrap().into() })).await).await;
        assert_eq!(ended["end"], "stopped");
    }

    /// Les réglages « partage du PC » de l'écran et du son suivent le partage,
    /// même si une ouverture et un arrêt se croisent.
    #[tokio::test]
    async fn ecran_et_son_suivent_le_partage_du_pc() {
        let st = station();
        let s = &st.s;
        // Restés allumés sans partage (ouverture et arrêt croisés) : rendus.
        s.display.set_sharing(true);
        s.audio.set_sharing(true);
        supervise_tick(s).await;
        assert!(!s.display.sharing() && !s.audio.sharing());
        // Éteints sous un partage du PC ouvert : rallumés.
        let _ = local_open(State(s.clone()), peer(5), admin(s), Json(LocalOpenBody { display: kyber_display_id("\\\\.\\DISPLAY1") })).await.ok().unwrap();
        s.display.set_sharing(false);
        s.audio.set_sharing(false);
        supervise_tick(s).await;
        assert!(s.display.sharing() && s.audio.sharing());
    }

    #[tokio::test]
    async fn l_invite_part_et_le_cookie_du_navigateur() {
        let st = station();
        let s = &st.s;
        let Json(opened) = local_open(State(s.clone()), peer(5), admin(s), Json(LocalOpenBody { display: kyber_display_id("\\\\.\\DISPLAY1") })).await.ok().unwrap();
        let code = opened["code"].as_str().unwrap();
        let res = join(State(s.clone()), peer(20), join_body(code, "Max", "chrome", "web")).await;
        let cookie = res.headers()[header::SET_COOKIE].to_str().unwrap().to_string();
        assert!(cookie.starts_with("kybernow_guest=") && cookie.contains("HttpOnly") && cookie.contains("SameSite=Strict"));
        let (_, joined) = body(res).await;
        let value = cookie.trim_start_matches("kybernow_guest=").split(';').next().unwrap().to_string();
        assert!(s.share.cookie_valid(&value));
        let mut h = guest_headers(joined["secret"].as_str().unwrap());
        h.insert(header::COOKIE, format!("kybernow_guest={value}").parse().unwrap());
        let res = leave(State(s.clone()), h, Json(GuestBody { guest: joined["guest"].as_str().unwrap().into() })).await;
        assert!(res.headers()[header::SET_COOKIE].to_str().unwrap().contains("Max-Age=0"));
        assert_eq!(res.status(), StatusCode::OK);
        assert!(!s.share.cookie_valid(&value));
        assert_eq!(texts(s)[0], "Max a quitté le partage");
        assert_eq!(s.share.guest_count(), 0);
        // Reparti, ou un mauvais secret : inconnu (404 `unknown_guest`, comme
        // `beat`) ; le cookie est quand même effacé.
        let mut h = guest_headers(joined["secret"].as_str().unwrap());
        h.insert(header::COOKIE, format!("kybernow_guest={value}").parse().unwrap());
        let res = leave(State(s.clone()), h, Json(GuestBody { guest: joined["guest"].as_str().unwrap().into() })).await;
        assert!(res.headers()[header::SET_COOKIE].to_str().unwrap().contains("Max-Age=0"));
        let (status, v) = body(res).await;
        assert_eq!((status, v["error"].as_str()), (StatusCode::NOT_FOUND, Some("unknown_guest")));
        // Retiré depuis moins d'une minute : connu, il part sans erreur.
        let (_, lea) = body(join(State(s.clone()), peer(21), join_body(code, "Léa", "ipad", "ios")).await).await;
        let id = lea["guest"].as_str().unwrap().to_string();
        let _ = local_guest_remove(State(s.clone()), peer(5), admin(s), Path(id.clone())).await.ok().unwrap();
        let res = leave(State(s.clone()), guest_headers(lea["secret"].as_str().unwrap()), Json(GuestBody { guest: id })).await;
        assert_eq!(res.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn les_gardes_voient_les_invites() {
        let st = station();
        let s = &st.s;
        assert_eq!(occupant(s), None);
        let Json(opened) = local_open(State(s.clone()), peer(5), admin(s), Json(LocalOpenBody { display: kyber_display_id("\\\\.\\DISPLAY1") })).await.ok().unwrap();
        let code = opened["code"].as_str().unwrap();
        body(join(State(s.clone()), peer(20), join_body(code, "Léa", "ipad", "ios")).await).await;
        body(join(State(s.clone()), peer(21), join_body(code, "Max", "tv", "tvos")).await).await;
        assert_eq!(occupant(s), Some(Occupant::Guests("Léa et Max".into())));
        // Les comptes en attente ne redémarrent pas Kyber sous les invités.
        assert!(crate::api::kyber_busy(s, None).await);
        assert_eq!(st.restarts.load(Ordering::SeqCst), 0);
    }
}
