//! Routes JSON de l'interface (`/api/*`).

use std::collections::{BTreeMap, HashMap};
use std::net::SocketAddr;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use axum::body::Bytes;
use axum::extract::{ConnectInfo, Path, Query, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post, put};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::audio::AudioSettings;
use crate::display::{Mode, Origin, PolicyChoice, Screen, Trigger, VdSettings};
use crate::access::PendingAccess;
use crate::activity::{self, DeviceInfo, ModeInfo};
use crate::i18n::{self, Msg};
use crate::launch::Launch;
use crate::pairing::{self, RequestError};
use crate::{AppState, auth, gpu, lend_api, logs, preview, schema, service, share_api, station};

pub fn router(state: AppState) -> Router<AppState> {
    Router::new()
        .route("/api/session", get(session))
        .route("/api/setup", post(setup))
        .route("/api/login", post(login))
        .route("/api/logout", post(logout))
        .route("/api/password", post(change_password))
        .route("/api/status", get(status))
        .route("/api/local/state", get(local_state))
        .route("/api/local/end", post(local_end))
        .route("/api/local/pair/{id}/{action}", post(local_pair_answer))
        // KyberNow Share depuis le PC (icône près de l'horloge, ou accueil d'Admin avec sa session).
        .route("/api/local/displays", get(share_api::local_displays))
        .route("/api/local/share", get(share_api::local_get).delete(share_api::local_close))
        .route("/api/local/share/open", post(share_api::local_open))
        .route("/api/local/share/code", post(share_api::local_code))
        .route("/api/local/share/guest/{id}", post(share_api::local_guest_role).delete(share_api::local_guest_remove))
        .route("/api/local/share/guest/{id}/{action}", post(share_api::local_guest_answer))
        // Prêt de l'ordinateur depuis le PC (icône, ou Admin avec sa session).
        .route("/api/local/lend", get(lend_api::local_get))
        .route("/api/local/lend/code", post(lend_api::local_code).delete(lend_api::local_code_cancel))
        .route("/api/local/lend/requests/{id}", post(lend_api::local_answer))
        .route("/api/local/lend/take-back", post(lend_api::local_take_back))
        // L'agent de console du prêt (`kybernow-tray.exe --console`).
        .route("/api/local/lend/console", get(lend_api::console_get))
        .route("/api/local/lend/console/ready", post(lend_api::console_ready))
        .route("/api/local/lend/console/beat", post(lend_api::console_beat))
        .route("/api/local/lend/console/launch", post(lend_api::console_launch))
        .route("/api/local/lend/console/played", post(lend_api::console_played))
        .route("/api/local/lend/console/give-back", post(lend_api::console_give_back))
        .route("/api/local/lend/console/take-back", post(lend_api::console_take_back))
        .route("/api/local/lend/console/ended", post(lend_api::console_ended))
        .route("/api/local/lend/icons/{name}", get(lend_api::console_icon))
        .route("/api/schema", get(schema_route))
        .route("/api/config", get(config).put(update_config))
        .route("/api/config/raw", get(raw_config).put(update_raw_config))
        .route("/api/users", get(users))
        .route("/api/users/{username}", put(upsert_user).delete(delete_user))
        .route("/api/service/{action}", post(service_action))
        .route("/api/logs", get(log_list))
        .route("/api/logs/{name}", get(log_tail))
        .route("/api/display", get(display_status))
        .route("/api/display/settings", put(display_settings))
        .route("/api/display/test", post(display_test))
        .route("/api/display/stop", post(display_stop))
        .route("/api/audio", get(audio_status))
        .route("/api/audio/settings", put(audio_settings))
        .route("/api/update", get(update_status).put(update_channel))
        .route("/api/update/check", post(update_check))
        .route("/api/update/install", post(update_install))
        .route("/api/update/rollback", post(update_rollback))
        // La Station entière (Kyber et Admin), depuis Admin.
        .route("/api/station/restart", post(admin_station_restart))
        .route("/api/devices", get(devices))
        .route("/api/devices/{id}", axum::routing::delete(forget_device))
        .route("/api/devices/{id}/launch", put(set_launch))
        // App ouverte au début des sessions : catalogue du menu Démarrer,
        // icônes, explorateur limité aux dossiers autorisés.
        .route("/api/apps", get(apps))
        .route("/api/apps/refresh", post(apps_refresh))
        .route("/api/apps/icon/{name}", get(app_icon))
        .route("/api/apps/browse", get(apps_browse))
        // Appairage par code (app Apple TV) : la demande et son état sans compte,
        // la réponse depuis la console.
        .route("/api/pair/request", post(pair_request))
        .route("/api/pair/request/{id}", get(pair_status))
        .route("/api/pair/accept/{id}", post(pair_accept))
        .route("/api/pair/refuse/{id}", post(pair_refuse))
        .route("/api/events", get(events))
        .route("/api/session/end", post(end_session))
        .route("/api/preview", get(admin_preview))
        // Client web (/link) : jeton Kyber pour la session de la console.
        .route("/api/web/ticket", get(web_ticket))
        // Lien d'appairage (QR code de la page Appareils Link, session de la console).
        .route("/api/pairing", get(pairing))
        // Hors de chez vous (docs/relais.md) : adresse du relais, accès des
        // appareils, état (session d'Admin, jamais par le relais).
        .route("/api/relay", get(crate::relay::get_relay).put(crate::relay::put_relay))
        // Appels des appareils (apps KyberNow), authentifiés par un compte Kyber.
        .route("/api/stream/info", get(stream_info))
        .route("/api/stream/prepare", post(stream_prepare))
        .route("/api/stream/heartbeat", post(stream_heartbeat))
        .route("/api/stream/foreground", get(stream_foreground))
        .route("/api/stream/preview", get(stream_preview))
        .route("/api/stream/end", post(stream_end))
        // Réglages « Station » des apps : redémarrer, mettre à jour.
        .route("/api/stream/station/restart", post(stream_station_restart))
        .route("/api/stream/station/update", post(stream_station_update))
        // KyberNow Share (docs/kybernow-share.md) : l'hôte (compte de
        // l'appareil en session), puis l'invité (sans compte).
        .route("/api/share", get(share_api::host_get).delete(share_api::host_close))
        .route("/api/share/open", post(share_api::host_open))
        .route("/api/share/code", post(share_api::host_code))
        .route("/api/share/guest/{id}", post(share_api::host_guest_role).delete(share_api::host_guest_remove))
        .route("/api/share/guest/{id}/{action}", post(share_api::host_guest_answer))
        .route("/api/share/join", post(share_api::join))
        .route("/api/share/wait", post(share_api::wait))
        // Prêt de l'ordinateur (docs/pret.md) : l'ami, l'hôte, les réglages.
        .route("/api/lend/status", post(lend_api::status))
        .route("/api/lend/borrow", post(lend_api::borrow))
        .route("/api/lend/wait", post(lend_api::wait))
        .route("/api/lend/beat", post(lend_api::beat))
        .route("/api/lend/leave", post(lend_api::leave))
        .route("/api/lend", get(lend_api::host_get))
        .route("/api/lend/code", post(lend_api::host_code).delete(lend_api::host_code_cancel))
        .route("/api/lend/requests/{id}", post(lend_api::host_answer))
        .route("/api/lend/take-back", post(lend_api::host_take_back))
        .route("/api/lend/settings", get(lend_api::get_settings).put(lend_api::put_settings))
        .route("/api/lend/catalog", get(apps))
        .route("/api/lend/friends", get(lend_api::friends).post(lend_api::friend_create))
        .route("/api/lend/friends/{id}", put(lend_api::friend_update).delete(lend_api::friend_delete))
        .route("/api/lend/friends/{id}/invite", get(lend_api::friend_invite))
        .route("/api/share/beat", post(share_api::beat))
        .route("/api/share/quality", post(share_api::quality))
        .route("/api/share/leave", post(share_api::leave))
        .layer(axum::middleware::from_fn_with_state(state, auth::require_session))
        // La langue des réponses (`Accept-Language`, français sans en-tête),
        // posée avant tout le reste : les refus de `require_session` aussi.
        .layer(axum::middleware::from_fn(i18n::scope))
}

pub struct ApiError(pub(crate) StatusCode, pub(crate) String);

impl ApiError {
    /// Un refus, écrit dans la langue de la requête.
    fn msg(status: StatusCode, msg: Msg) -> Self {
        ApiError(status, msg.text())
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (self.0, Json(json!({ "error": self.1 }))).into_response()
    }
}

impl From<anyhow::Error> for ApiError {
    fn from(e: anyhow::Error) -> Self {
        // Message lisible pour l'interface (dans la langue de la requête),
        // chaîne complète dans le journal (en français).
        tracing::warn!("{e:#}");
        ApiError(StatusCode::BAD_REQUEST, i18n::error_text(i18n::current(), &e))
    }
}

type ApiResult<T> = Result<T, ApiError>;

async fn session(State(s): State<AppState>, headers: HeaderMap, via: Option<axum::Extension<crate::relay::Via>>) -> Json<Value> {
    // Par le relais : un invité du navigateur. Ni nom de l'ordinateur (connu
    // seulement après l'accord de l'hôte), ni fichier du code d'initialisation ;
    // l'image passera en WebSocket.
    if via.is_some() {
        return Json(json!({
            "configured": s.auth.is_configured(),
            "authenticated": false,
            "relay": true,
            "version": crate::update::current(),
        }));
    }
    let authenticated = auth::session_token(&headers).is_some_and(|t| s.auth.session_valid(&t));
    Json(json!({
        "configured": s.auth.is_configured(),
        "authenticated": authenticated,
        "setup_code_file": s.data_dir.join(auth::SETUP_TOKEN_FILE),
        // Déjà annoncé sur le réseau (Bonjour) : l'écran de connexion le nomme.
        "hostname": hostname::get().ok().map(|h| h.to_string_lossy().into_owned()),
        // Contrôle de santé de l'applicateur de mises à jour (sans compte).
        "version": crate::update::current(),
    }))
}

#[derive(Deserialize)]
struct SetupBody {
    code: String,
    password: String,
}

/// Trop d'essais manqués depuis cette adresse : 429 avec le délai d'attente.
fn check_lockout(s: &AppState, peer: SocketAddr) -> ApiResult<()> {
    match s.auth.locked_for(peer.ip()) {
        Some(wait) => Err(ApiError::msg(
            StatusCode::TOO_MANY_REQUESTS,
            Msg::new("api.tooManyTries").with("seconds", wait.as_secs().max(1)),
        )),
        None => Ok(()),
    }
}

async fn setup(
    State(s): State<AppState>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    Json(body): Json<SetupBody>,
) -> ApiResult<Response> {
    check_lockout(&s, peer)?;
    let auth = s.auth.clone();
    let hash = auth.clone().hash_limited(move || auth.setup(&body.code, &body.password)).await;
    let hash = match hash {
        Ok(hash) => hash,
        Err(e) => {
            s.auth.note_failure(peer.ip());
            return Err(e.into());
        }
    };
    s.auth.note_success(peer.ip());
    s.update_settings(|st| st.admin_password_hash = Some(hash))?;
    tracing::info!("mot de passe administrateur défini");
    Ok(with_new_session(&s))
}

#[derive(Deserialize)]
struct LoginBody {
    password: String,
}

async fn login(
    State(s): State<AppState>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    Json(body): Json<LoginBody>,
) -> ApiResult<Response> {
    check_lockout(&s, peer)?;
    // argon2 est volontairement coûteux : hors du fil d'exécution asynchrone,
    // et quelques vérifications à la fois seulement (voir `Auth::hash_limited`).
    let auth = s.auth.clone();
    let ok = auth.clone().hash_limited(move || Ok(auth.verify(&body.password))).await.unwrap_or(false);
    if !ok {
        s.auth.note_failure(peer.ip());
        // Ralentit les essais répétés.
        tokio::time::sleep(Duration::from_millis(800)).await;
        return Err(ApiError::msg(StatusCode::UNAUTHORIZED, Msg::new("api.wrongPassword")));
    }
    s.auth.note_success(peer.ip());
    Ok(with_new_session(&s))
}

fn with_new_session(s: &AppState) -> Response {
    let token = s.auth.open_session();
    (
        [(header::SET_COOKIE, auth::session_cookie(&token, s.secure_cookies))],
        Json(json!({ "ok": true })),
    )
        .into_response()
}

async fn logout(State(s): State<AppState>, headers: HeaderMap) -> Response {
    if let Some(t) = auth::session_token(&headers) {
        s.auth.close_session(&t);
    }
    ([(header::SET_COOKIE, auth::clear_cookie(s.secure_cookies))], Json(json!({ "ok": true })))
        .into_response()
}

#[derive(Deserialize)]
struct PasswordBody {
    current: String,
    new: String,
}

async fn change_password(State(s): State<AppState>, Json(body): Json<PasswordBody>) -> ApiResult<Response> {
    let auth = s.auth.clone();
    let hash = auth.clone().hash_limited(move || auth.change_password(&body.current, &body.new)).await?;
    s.update_settings(|st| st.admin_password_hash = Some(hash))?;
    Ok(with_new_session(&s))
}

async fn status(State(s): State<AppState>) -> ApiResult<Json<Value>> {
    let service_state = s.service.state().unwrap_or(service::ServiceState::Unknown);
    let port = s
        .kyber
        .values()
        .ok()
        .and_then(|v| v.into_iter().find(|f| f.id == "network.port"))
        .and_then(|f| f.value.as_u64())
        .unwrap_or(8080) as u16;
    let reachable = tokio::time::timeout(
        Duration::from_millis(500),
        tokio::net::TcpStream::connect(("127.0.0.1", port)),
    )
    .await
    .is_ok_and(|r| r.is_ok());
    let gpu = tokio::task::spawn_blocking(gpu::status).await.ok().flatten();

    Ok(Json(json!({
        "hostname": hostname::get().ok().map(|h| h.to_string_lossy().into_owned()),
        "os": crate::system::os_name(),
        "lan_ip": crate::tls::lan_ip(),
        "admin_version": env!("CARGO_PKG_VERSION"),
        "kyber_dir": s.kyber_dir,
        "config_path": s.kyber.path(),
        "config_found": s.kyber.path().exists(),
        "service": { "state": service_state, "simulated": s.service.is_simulated() },
        "controller": { "port": port, "reachable": reachable },
        "restart_pending": s.restart_pending.load(Ordering::Relaxed),
        "pending_access": s.pending_access.snapshot(),
        "gpu": gpu,
        "activity": s.activity.summary(),
        "update_available": s.updater.available(),
        "pair_requests": s.pairing.pending(),
        // Ligne « Réveil par le réseau » de la page Système.
        "wake": crate::wake::cards(),
        // KyberNow Share : le partage ouvert (ou null), et s'il est possible.
        "share": share_api::status(&s),
        "share_enabled": share_api::enabled(&s),
    })))
}

/// Les routes `/api/local/*` : depuis l'ordinateur lui-même (loopback, aussi
/// en IPv4 projeté en IPv6) **et** avec le jeton local de la session de
/// l'utilisateur (`x-kyber-local`, voir `local_token.rs`).
fn local_only(s: &AppState, peer: SocketAddr, headers: &HeaderMap) -> ApiResult<()> {
    if !peer.ip().to_canonical().is_loopback() {
        return Err(ApiError::msg(StatusCode::FORBIDDEN, Msg::new("api.localOnly")));
    }
    if !s.local_token.accepts(headers) {
        return Err(ApiError::msg(StatusCode::FORBIDDEN, Msg::new("api.localToken")));
    }
    Ok(())
}

/// État résumé pour l'icône de la zone de notification, réservé à l'ordinateur
/// lui-même : serveur en marche ou non, session d'un appareil en cours ou non.
/// Les demandes d'appairage y sont sans leur code : l'icône renvoie à la
/// console pour le comparer à celui de l'appareil.
async fn local_state(
    State(s): State<AppState>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
) -> ApiResult<Json<Value>> {
    local_only(&s, peer, &headers)?;
    let service_state = s.service.state().unwrap_or(service::ServiceState::Unknown);
    let (d, a) = (s.display.clone(), s.audio.clone());
    let (display, audio) = blocking(move || Ok((d.status()["session"].clone(), a.status()["session"].clone()))).await?;
    let user = display["user"].as_str().map(str::to_owned);
    let requests: Vec<Value> = s
        .pairing
        .pending()
        .into_iter()
        .map(|r| json!({ "id": r.id, "name": r.name, "kind": r.kind, "expires_at": r.expires_at }))
        .collect();
    Ok(Json(json!({
        "service": service_state,
        "session": !display.is_null() || !audio.is_null(),
        "user": user,
        "hostname": hostname::get().ok().map(|h| h.to_string_lossy().into_owned()),
        "address": crate::tls::lan_ip(),
        "console_port": s.listen_port,
        "activity": s.activity.summary(),
        "restart_pending": s.restart_pending.load(Ordering::Relaxed),
        "pending_access": s.pending_access.snapshot(),
        "update_available": s.updater.available(),
        "version": crate::update::current(),
        "pair_requests_count": requests.len(),
        "pair_requests": requests,
        "share": share_api::status(&s),
        "share_enabled": share_api::enabled(&s),
        // Prêt de l'ordinateur : la vue du prêt (icône).
        "lend": lend_api::view(&s),
        // Hors de chez vous : adresse du relais, accès, état (icône).
        "relay": s.relay.brief(&s, &s.settings.lock().unwrap().relay.clone()),
    })))
}

/// Fin de la session en cours, depuis l'icône de la zone de notification
/// (réservé à l'ordinateur lui-même : la personne devant l'écran).
async fn local_end(
    State(s): State<AppState>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
) -> ApiResult<Json<Value>> {
    local_only(&s, peer, &headers)?;
    end_session(State(s)).await
}

/// Réponse à une demande d'appairage depuis le panneau de l'icône (réservé à
/// l'ordinateur lui-même : la personne devant l'écran).
async fn local_pair_answer(
    State(s): State<AppState>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    Path((id, action)): Path<(String, String)>,
) -> ApiResult<Json<Value>> {
    local_only(&s, peer, &headers)?;
    match action.as_str() {
        "accept" => pair_accept(State(s), Path(id)).await,
        "refuse" => pair_refuse(State(s), Path(id)).await,
        _ => Err(ApiError::msg(StatusCode::NOT_FOUND, Msg::new("api.unknownAction").with("action", action))),
    }
}

async fn update_status(State(s): State<AppState>) -> Json<Value> {
    Json(s.updater.status())
}

async fn update_check(State(s): State<AppState>) -> ApiResult<Json<Value>> {
    let u = s.updater.clone();
    Ok(Json(blocking(move || Ok(u.check())).await?))
}

#[derive(Deserialize)]
struct UpdateChannelBody {
    channel: crate::update::Channel,
}

/// Canal des mises à jour (`stable` ou `beta`) : enregistré, puis la
/// dernière version de ce canal est recherchée.
async fn update_channel(State(s): State<AppState>, Json(body): Json<UpdateChannelBody>) -> ApiResult<Json<Value>> {
    s.update_settings(|st| st.update_channel = body.channel)?;
    s.updater.set_channel(body.channel);
    s.activity.event("info", Msg::new("events.channel").with("channel", body.channel.as_str()));
    let u = s.updater.clone();
    Ok(Json(blocking(move || Ok(u.check())).await?))
}

#[derive(Deserialize, Default)]
struct UpdateRunBody {
    /// Installer même si un appareil est en session (sa session sera coupée).
    #[serde(default)]
    force: bool,
}

/// 409 si un appareil est en session, sauf `{"force": true}` : la mise à
/// jour arrête le serveur et coupe sa session.
fn session_guard(s: &AppState, body: &Bytes) -> ApiResult<()> {
    let body: UpdateRunBody = if body.iter().all(u8::is_ascii_whitespace) {
        UpdateRunBody::default()
    } else {
        serde_json::from_slice(body)
            .map_err(|e| ApiError::msg(StatusCode::BAD_REQUEST, Msg::new("api.unreadableBody").with("error", e.to_string())))?
    };
    // L'appareil en session, ou les invités d'un partage.
    match share_api::occupant(s).filter(|_| !body.force) {
        Some(share_api::Occupant::Guests(names)) => Err(ApiError::msg(StatusCode::CONFLICT, Msg::new("update.sessionConflictShare").with("name", names))),
        Some(share_api::Occupant::Borrower(name)) => Err(ApiError::msg(StatusCode::CONFLICT, Msg::new("update.sessionConflictLend").with("name", name))),
        Some(share_api::Occupant::Host(name)) => {
            let message = crate::update::session_conflict(&json!({ "name": name }), false).unwrap_or_else(|| Msg::new("update.sessionConflict"));
            Err(ApiError::msg(StatusCode::CONFLICT, message))
        }
        None => Ok(()),
    }
}

/// 409 pendant un redémarrage de la Station : Admin va s'arrêter, et
/// l'assistant tourne depuis le dossier qu'une mise à jour renomme.
fn not_restarting(s: &AppState) -> ApiResult<()> {
    if s.station.restarting() {
        return Err(ApiError::msg(StatusCode::CONFLICT, Msg::new("station.alreadyRestarting")));
    }
    Ok(())
}

/// Télécharge, vérifie et lance l'installation de la dernière version : le
/// même chemin pour Admin (`/api/update/install`) et pour les apps
/// (`/api/stream/station/update`). Rend la version installée.
async fn launch_install(s: &AppState) -> ApiResult<String> {
    not_restarting(s)?;
    let u = s.updater.clone();
    let version = blocking(move || u.install()).await?;
    tracing::info!("mise à jour {version} lancée");
    Ok(version)
}

/// Télécharge et lance l'installation : la console s'arrête dans la minute
/// et revient dans la nouvelle version. 409 pendant une session, sauf
/// `{"force": true}`.
async fn update_install(State(s): State<AppState>, body: Bytes) -> ApiResult<Json<Value>> {
    session_guard(&s, &body)?;
    let version = launch_install(&s).await?;
    share_api::close_share(&s).await;
    s.activity.event("info", Msg::new("events.installing").with("version", version.as_str()));
    Ok(Json(json!({ "ok": true, "version": version })))
}

/// Remet la version précédente (dossiers `.prev`) : la console s'arrête dans
/// la minute et revient dans cette version. 409 pendant une session, comme
/// l'installation.
async fn update_rollback(State(s): State<AppState>, body: Bytes) -> ApiResult<Json<Value>> {
    session_guard(&s, &body)?;
    not_restarting(&s)?;
    let u = s.updater.clone();
    let version = blocking(move || u.rollback()).await?;
    share_api::close_share(&s).await;
    tracing::info!("retour à la version {version} lancé");
    s.activity.event("info", Msg::new("events.rollingBack").with("version", version.as_str()));
    Ok(Json(json!({ "ok": true, "version": version })))
}

/// Les réglages de Kyber exposés, avec leurs textes dans la langue de la requête.
async fn schema_route() -> Json<Value> {
    let (groups, fields) = i18n::localize_schema(i18n::current(), json!(schema::GROUPS), json!(schema::fields()));
    Json(json!({ "groups": groups, "fields": fields }))
}

async fn config(State(s): State<AppState>) -> ApiResult<Json<Value>> {
    Ok(Json(json!({ "values": s.kyber.values()? })))
}

#[derive(Deserialize)]
struct ConfigBody {
    changes: BTreeMap<String, Option<Value>>,
}

async fn update_config(State(s): State<AppState>, Json(body): Json<ConfigBody>) -> ApiResult<Json<Value>> {
    s.kyber.apply(&body.changes)?;
    s.restart_pending.store(true, Ordering::Relaxed);
    Ok(Json(json!({ "values": s.kyber.values()?, "restart_pending": true })))
}

async fn raw_config(State(s): State<AppState>) -> ApiResult<Json<Value>> {
    Ok(Json(json!({ "text": s.kyber.read_raw()? })))
}

#[derive(Deserialize)]
struct RawBody {
    text: String,
}

async fn update_raw_config(State(s): State<AppState>, Json(body): Json<RawBody>) -> ApiResult<Json<Value>> {
    s.kyber.write_raw(&body.text)?;
    s.restart_pending.store(true, Ordering::Relaxed);
    Ok(Json(json!({ "ok": true, "restart_pending": true })))
}

/// Les comptes Kyber, avec pour chacun l'appareil appairé dont c'est l'accès propre.
fn users_json(s: &AppState) -> ApiResult<Value> {
    let own: HashMap<String, String> = s.activity.own_accounts().into_iter().collect();
    Ok(s.kyber
        .users()?
        .into_iter()
        .map(|u| json!({ "device": own.get(&u.username), "username": u.username, "insecure_default": u.insecure_default }))
        .collect())
}

async fn users(State(s): State<AppState>) -> ApiResult<Json<Value>> {
    Ok(Json(json!({ "users": users_json(&s)? })))
}

#[derive(Deserialize)]
struct UserBody {
    password: String,
}

async fn upsert_user(
    State(s): State<AppState>,
    Path(username): Path<String>,
    Json(body): Json<UserBody>,
) -> ApiResult<Json<Value>> {
    let existed = s.kyber.users()?.iter().any(|u| u.username == username);
    s.kyber.upsert_user(&username, &body.password)?;
    s.restart_pending.store(true, Ordering::Relaxed);
    let key = if existed { "events.passwordChanged" } else { "events.accountAdded" };
    s.activity.event("info", Msg::new(key).with("username", username.as_str()));
    Ok(Json(json!({ "users": users_json(&s)?, "restart_pending": true })))
}

async fn delete_user(State(s): State<AppState>, Path(username): Path<String>) -> ApiResult<Json<Value>> {
    if s.kyber.users()?.len() <= 1 {
        return Err(ApiError::msg(StatusCode::BAD_REQUEST, Msg::new("api.lastUser")));
    }
    s.kyber.delete_user(&username)?;
    s.pending_access.forget(&username);
    s.restart_pending.store(true, Ordering::Relaxed);
    s.activity.event("info", Msg::new("events.accountDeleted").with("username", username.as_str()));
    Ok(Json(json!({ "users": users_json(&s)?, "restart_pending": true })))
}

async fn service_action(State(s): State<AppState>, Path(action): Path<String>) -> ApiResult<Json<Value>> {
    let taken = s.pending_access.snapshot();
    match action.as_str() {
        "start" => s.service.start()?,
        "stop" => s.service.stop()?,
        "restart" => service::restart(s.service.as_ref()).await?,
        _ => return Err(ApiError::msg(StatusCode::NOT_FOUND, Msg::new("api.unknownAction").with("action", action))),
    }
    if action != "stop" {
        // Kyber relit sa configuration en démarrant : comptes compris.
        s.restart_pending.store(false, Ordering::Relaxed);
        s.pending_access.applied(&taken);
    }
    tracing::info!("service kyber : {action}");
    s.activity.event("info", Msg::new(match action.as_str() {
        "start" => "events.serverStarted",
        "stop" => "events.serverStopped",
        _ => "events.serverRestarted",
    }));
    Ok(Json(json!({ "state": s.service.state()? })))
}

async fn log_list(State(s): State<AppState>) -> ApiResult<Json<Value>> {
    Ok(Json(json!({ "files": logs::list(&s.log_sources())? })))
}

#[derive(Deserialize)]
struct TailQuery {
    lines: Option<usize>,
}

async fn log_tail(
    State(s): State<AppState>,
    Path(name): Path<String>,
    Query(q): Query<TailQuery>,
) -> ApiResult<Json<Value>> {
    let lines = q.lines.unwrap_or(500).min(5000);
    let sources = s.log_sources();
    let text = tokio::task::spawn_blocking(move || logs::tail(&sources, &name, lines))
        .await
        .map_err(|e| ApiError(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))??;
    Ok(Json(json!({ "text": text })))
}

/// `f` dans un fil à part, avec la langue de la requête (textes écrits par `f`).
pub(crate) async fn blocking<T: Send + 'static>(f: impl FnOnce() -> anyhow::Result<T> + Send + 'static) -> ApiResult<T> {
    let lang = i18n::current();
    tokio::task::spawn_blocking(move || i18n::with_lang(lang, f))
        .await
        .map_err(|e| ApiError(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .map_err(ApiError::from)
}

async fn display_status(State(s): State<AppState>) -> ApiResult<Json<Value>> {
    let d = s.display.clone();
    Ok(Json(blocking(move || Ok(d.status())).await?))
}

async fn display_settings(State(s): State<AppState>, Json(body): Json<VdSettings>) -> ApiResult<Json<Value>> {
    Mode { width: body.width, height: body.height, refresh: body.refresh }.validate()?;
    s.update_settings(|st| st.virtual_display = body.clone())?;
    *s.display.settings.lock().unwrap() = body.clone();
    if body.trigger == Trigger::Off {
        let d = s.display.clone();
        blocking(move || {
            d.stop("écran virtuel désactivé");
            Ok(())
        })
        .await?;
    }
    display_status(State(s)).await
}

#[derive(Deserialize)]
struct TestBody {
    width: u32,
    height: u32,
    refresh: u32,
    seconds: Option<u64>,
}

async fn display_test(State(s): State<AppState>, Json(body): Json<TestBody>) -> ApiResult<Json<Value>> {
    let mode = Mode { width: body.width, height: body.height, refresh: body.refresh };
    let seconds = body.seconds.unwrap_or(20).clamp(5, 300);
    let d = s.display.clone();
    blocking(move || d.start(Origin::Test, mode, None, None, Some(Duration::from_secs(seconds)), None)).await?;
    display_status(State(s)).await
}

async fn display_stop(State(s): State<AppState>) -> ApiResult<Json<Value>> {
    let d = s.display.clone();
    blocking(move || {
        d.stop("arrêt demandé depuis la console");
        Ok(())
    })
    .await?;
    display_status(State(s)).await
}

async fn audio_status(State(s): State<AppState>) -> ApiResult<Json<Value>> {
    let a = s.audio.clone();
    Ok(Json(blocking(move || Ok(a.status())).await?))
}

async fn audio_settings(State(s): State<AppState>, Json(body): Json<AudioSettings>) -> ApiResult<Json<Value>> {
    s.update_settings(|st| st.virtual_audio = body.clone())?;
    *s.audio.settings.lock().unwrap() = body.clone();
    if body.trigger == Trigger::Off {
        let a = s.audio.clone();
        blocking(move || {
            a.stop("sortie audio virtuelle désactivée");
            Ok(())
        })
        .await?;
    }
    audio_status(State(s)).await
}

/// Identifiant Kyber si l'en-tête `Authorization: Basic` est valide.
pub(crate) fn stream_user(s: &AppState, headers: &HeaderMap) -> ApiResult<String> {
    let bad = || ApiError::msg(StatusCode::UNAUTHORIZED, Msg::new("api.kyberCredentials"));
    // Le client web (/link) passe par la session de la console.
    if !headers.contains_key(header::AUTHORIZATION) && auth::session_token(headers).is_some_and(|t| s.auth.session_valid(&t)) {
        return Ok(web_user(s));
    }
    let value = headers.get(header::AUTHORIZATION).and_then(|v| v.to_str().ok()).ok_or_else(bad)?;
    let encoded = value.strip_prefix("Basic ").ok_or_else(bad)?;
    let decoded = base64_decode(encoded.trim()).ok_or_else(bad)?;
    let (user, pass) = decoded.split_once(':').ok_or_else(bad)?;
    if s.kyber.verify_login(user, pass) { Ok(user.to_string()) } else { Err(bad()) }
}

/// Nom sous lequel le client web se connecte à Kyber : le compte créé à
/// l'installation (`install_account`, mémorisé par `init`), jamais le
/// premier de la liste (qu'un appareil appairé pourrait devenir si
/// l'administrateur retire ou réordonne les comptes).
fn web_user(s: &AppState) -> String {
    if let Some(account) = s.settings.lock().unwrap().install_account.clone() {
        return account;
    }
    s.kyber
        .users()
        .ok()
        .and_then(|users| users.into_iter().next())
        .map(|u| u.username)
        .unwrap_or_else(|| "KyberNow".into())
}

/// Ce dont le client web a besoin pour se connecter à Kyber : un jeton signé
/// avec la clé que Kyber connaît (posée à l'installation), valable le temps
/// d'une session de la console. Jamais mis en cache par le navigateur.
async fn web_ticket(State(s): State<AppState>) -> ApiResult<Response> {
    const LIFETIME: i64 = 12 * 3600;
    let secret = s.kyber.jwt_secret().ok_or_else(|| ApiError::msg(StatusCode::CONFLICT, Msg::new("api.noTokenKey")))?;
    let user = web_user(&s);
    let body = json!({
        "token": crate::kyber_proxy::jwt(&secret, &user, LIFETIME),
        "user": user,
        "expires_in": LIFETIME,
        "kyber_port": s.kyber.port(),
        "hostname": hostname::get().ok().map(|h| h.to_string_lossy().into_owned()),
    });
    Ok(([(header::CACHE_CONTROL, "no-store")], Json(body)).into_response())
}

fn base64_decode(input: &str) -> Option<String> {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut bits = 0u32;
    let mut count = 0;
    let mut out = Vec::new();
    for c in input.bytes().filter(|c| *c != b'=') {
        let v = ALPHABET.iter().position(|a| *a == c)? as u32;
        bits = (bits << 6) | v;
        count += 6;
        if count >= 8 {
            count -= 8;
            out.push((bits >> count) as u8);
            bits &= (1 << count) - 1;
        }
    }
    String::from_utf8(out).ok()
}

#[derive(Deserialize)]
struct PrepareBody {
    width: u32,
    height: u32,
    refresh: u32,
    /// Canaux audio voulus par l'appareil : 2, 6, 8, ou 0 pour garder le son sur l'ordinateur.
    #[serde(default)]
    audio_channels: Option<u16>,
    /// `false` : l'appareil veut les écrans de l'ordinateur tels quels.
    #[serde(default)]
    virtual_display: Option<bool>,
    /// Ce que l'app dit d'elle-même (appareils de la console).
    #[serde(default)]
    device: Option<DeviceInfo>,
    /// Profil choisi dans l'app (« Jeu », « Bureau », « Max »).
    #[serde(default)]
    profile: Option<String>,
    /// Si le mode de l'écran virtuel change en session : `follow` (laisser
    /// faire, profil Jeu) ou `restore` (le remettre, profil Bureau) ;
    /// `default` ou absent : réglage `on_mode_change` de la console.
    #[serde(default)]
    mode_policy: Option<PolicyChoice>,
    /// Multi-écrans (docs/multi-ecrans.md) : un écran virtuel par entrée, de
    /// 1 à 4, placés comme demandé, le principal en (0, 0). `width`,
    /// `height` et `refresh` décrivent alors l'écran principal. Sans effet
    /// sans écran virtuel.
    #[serde(default)]
    screens: Option<Vec<Screen>>,
}

/// Lien d'appairage pour les apps (`kybernow://…`), que la console affiche en
/// lien et en QR code (session de la console requise) : adresse, port et
/// empreinte du certificat, pour une première connexion vérifiée sans question.
async fn pairing(State(s): State<AppState>) -> ApiResult<Json<Value>> {
    let hostname = hostname::get().ok().map(|h| h.to_string_lossy().into_owned()).unwrap_or_default();
    let address = crate::tls::lan_ip().unwrap_or_else(|| "localhost".into());
    let mut link = format!("kybernow://{address}:{}?name={}", s.listen_port, urlencoding::encode(&hostname));
    if let Some(fp) = &s.tls_fingerprint {
        link.push_str("&fp=");
        link.push_str(&urlencoding::encode(fp));
    }
    Ok(Json(json!({
        "link": link,
        "hostname": hostname,
        "address": address,
        "port": s.listen_port,
        "fingerprint": s.tls_fingerprint,
    })))
}

/// Ce que l'app affiche d'un ordinateur : nom, système, carte graphique,
/// possibilités, la Station, et les cartes réseau pour le réveiller.
async fn stream_info(State(s): State<AppState>, headers: HeaderMap) -> ApiResult<Json<Value>> {
    let user = stream_user(&s, &headers)?;
    // Compte créé pour un appareil par l'appairage par code : les apps
    // l'affichent comme « Accès propre à cet appareil ».
    let own_account = s.activity.own_accounts().iter().any(|(u, _)| *u == user);
    let gpu = tokio::task::spawn_blocking(gpu::status).await.ok().flatten();
    let gpu_name = gpu.as_ref().and_then(|g| g.devices.first()).map(|d| d.name.clone());
    // AV1 en NVENC : à partir des cartes Ada (RTX 40) et suivantes.
    let av1 = gpu_name.as_deref().is_some_and(|n| ["RTX 40", "RTX 50", "RTX 2000 Ada", "RTX 4000 Ada", "RTX 5000 Ada", "RTX 6000 Ada", "L4", "L40"].iter().any(|m| n.contains(m)));
    let (d, a) = (s.display.clone(), s.audio.clone());
    let (display, audio) = blocking(move || Ok((d.status(), a.status()))).await?;
    let available = display["driver"]["available"].as_bool().unwrap_or(false);
    let trigger = s.display.settings.lock().unwrap().trigger;
    Ok(Json(json!({
        "hostname": hostname::get().ok().map(|h| h.to_string_lossy().into_owned()),
        "os": crate::system::os_name(),
        "gpu": gpu_name,
        "av1": av1,
        "virtual_display": {
            "available": available,
            "trigger": display["settings"]["trigger"],
            "active": !display["session"].is_null(),
            // Écrans que cette Station sait créer pour une session (multi-écrans).
            "max_screens": max_screens(available, trigger),
        },
        "audio": {
            "speakers": !audio["devices"]["speakers"].is_null(),
            "trigger": audio["settings"]["trigger"],
        },
        "admin_version": env!("CARGO_PKG_VERSION"),
        // KyberNow Share : cette Station le connaît ; coupé avec `multi_client`.
        "share": { "available": true, "enabled": share_api::enabled(&s) },
        "account": { "own_device": own_account },
        "station": station_info(&s),
        // Cartes Ethernet et Wi-Fi pour le réveil par le réseau (`wake.rs`),
        // lues en mémoire ; les apps les gardent pour réveiller l'ordinateur.
        "wake": crate::wake::cards(),
        // Le relais réglé (docs/relais.md, « info ») : absent sans relais.
        "relay": s.relay.info(&s),
        // Prêt de l'ordinateur (docs/pret.md).
        "lend": lend_api::info(&s),
    })))
}

/// Écrans qu'une session peut avoir : 1 sans écran virtuel utilisable
/// (pilote absent, déclenchement coupé), comme une Station antérieure.
fn max_screens(available: bool, trigger: Trigger) -> usize {
    if available && trigger != Trigger::Off { crate::display::MAX_SCREENS } else { 1 }
}

/// La Station pour la rubrique « Station » des réglages des apps : sa
/// version, la mise à jour disponible, un redémarrage en cours. Lu en
/// mémoire seulement : les apps demandent `info` toutes les 8 s.
fn station_info(s: &AppState) -> Value {
    json!({
        "version": crate::update::current(),
        "update": s.updater.brief().json(),
        "restarting": s.station.restarting(),
    })
}

// --- La Station entière : redémarrage et mise à jour --------------------------

/// Réponse à un compte refusé sur les routes de la Station, comme à un mot de
/// passe faux à la connexion d'Admin : ralentit les essais répétés.
const FAILED_TRY_DELAY: Duration = Duration::from_millis(800);

#[derive(Deserialize, Default)]
struct StationBody {
    /// Identifiant de l'appareil qui demande : son nom va au fil d'événements,
    /// il ne donne aucun droit.
    #[serde(default)]
    device_id: Option<String>,
    /// Même si un appareil est en session (sa session sera coupée).
    #[serde(default)]
    force: bool,
}

/// Corps facultatif (`{}` ou rien).
fn station_body(body: &Bytes) -> Result<StationBody, Refusal> {
    if body.iter().all(u8::is_ascii_whitespace) {
        return Ok(StationBody::default());
    }
    serde_json::from_slice(body).map_err(|e| {
        Refusal::new(StatusCode::BAD_REQUEST, "bad_request", Msg::new("api.unreadableBody").with("error", e.to_string()))
    })
}

/// Refus d'une route de la Station : le message (langue de la requête), un
/// code stable que lisent les apps (`code` : `session`, `installing`,
/// `restarting`, `up_to_date`, `requires_setup`, `not_installable`,
/// `too_soon`, `locked`, `unauthorized`, `bad_request`, `failed`) et, selon
/// le cas, l'appareil en session (`device`) ou le délai d'attente en
/// secondes (`retry_after`, aussi en en-tête `Retry-After`).
#[derive(Debug)]
struct Refusal {
    status: StatusCode,
    code: &'static str,
    error: String,
    device: Option<String>,
    retry_after: Option<u64>,
}

impl Refusal {
    fn new(status: StatusCode, code: &'static str, msg: Msg) -> Self {
        Refusal { status, code, error: msg.text(), device: None, retry_after: None }
    }

    fn device(mut self, name: &str) -> Self {
        self.device = Some(name.to_string());
        self
    }

    fn retry_after(mut self, seconds: u64) -> Self {
        self.retry_after = Some(seconds);
        self
    }
}

impl IntoResponse for Refusal {
    fn into_response(self) -> Response {
        let mut body = json!({ "error": self.error, "code": self.code });
        if let Some(device) = &self.device {
            body["device"] = json!(device);
        }
        if let Some(seconds) = self.retry_after {
            body["retry_after"] = json!(seconds);
        }
        let mut res = (self.status, Json(body)).into_response();
        if let Some(seconds) = self.retry_after {
            res.headers_mut().insert(header::RETRY_AFTER, HeaderValue::from(seconds));
        }
        res
    }
}

impl From<ApiError> for Refusal {
    fn from(e: ApiError) -> Self {
        Refusal { status: e.0, code: "failed", error: e.1, device: None, retry_after: None }
    }
}

/// Qui demande un redémarrage ou une mise à jour de la Station.
enum Caller {
    /// Une app KyberNow Link : son compte Kyber.
    Device(String),
    /// Admin lui-même (sa session, sans `Authorization`).
    Admin,
}

/// Le compte de la requête, comme `stream_user`, avec le blocage de la
/// connexion d'Admin après des essais ratés (par adresse, même compteur). Un
/// succès ne remet pas ce compteur à zéro : un compte Kyber ne doit pas
/// servir à effacer les échecs de la connexion d'Admin depuis la même adresse.
/// Le droit ne dépend que du compte : tout appareil appairé peut redémarrer
/// et mettre à jour.
async fn station_caller(s: &AppState, peer: SocketAddr, headers: &HeaderMap) -> Result<Caller, Refusal> {
    if let Some(wait) = s.auth.locked_for(peer.ip()) {
        let seconds = wait.as_secs().max(1);
        return Err(Refusal::new(StatusCode::TOO_MANY_REQUESTS, "locked", Msg::new("api.tooManyTries").with("seconds", seconds)).retry_after(seconds));
    }
    let admin = !headers.contains_key(header::AUTHORIZATION) && auth::session_token(headers).is_some_and(|t| s.auth.session_valid(&t));
    match stream_user(s, headers) {
        Ok(_) if admin => Ok(Caller::Admin),
        Ok(user) => Ok(Caller::Device(user)),
        Err(e) => {
            s.auth.note_failure(peer.ip());
            tokio::time::sleep(FAILED_TRY_DELAY).await;
            Err(Refusal { code: "unauthorized", ..e.into() })
        }
    }
}

/// La ligne du fil : l'appareil (par son identifiant, s'il utilise ce compte),
/// sinon son compte, ou Admin.
fn station_event(s: &AppState, caller: &Caller, device_id: Option<&str>, keys: [&str; 3]) -> Msg {
    let [named, account, admin] = keys;
    match caller {
        Caller::Admin => Msg::new(admin),
        Caller::Device(user) => match device_id.and_then(|id| s.activity.device_name(id, user)) {
            Some(name) => Msg::new(named).with("name", name),
            None => Msg::new(account).with("account", user.as_str()),
        },
    }
}

/// 409 pendant une installation ou un redémarrage déjà lancés.
fn station_busy(s: &AppState, installing: bool) -> Result<(), Refusal> {
    if installing {
        return Err(Refusal::new(StatusCode::CONFLICT, "installing", Msg::new("update.alreadyInstalling")));
    }
    if s.station.restarting() {
        return Err(Refusal::new(StatusCode::CONFLICT, "restarting", Msg::new("station.alreadyRestarting")));
    }
    Ok(())
}

/// Redémarre la Station entière par l'assistant détaché (Admin et Kyber
/// s'arrêtent, puis Kyber et Admin repartent) : 409 pendant une installation,
/// un redémarrage ou une session (sauf `force`), 429 moins d'une minute après
/// le précédent. `event` : la ligne du fil, écrite si le redémarrage part.
fn restart_station(s: &AppState, force: bool, event: Msg) -> Result<(), Refusal> {
    station_busy(s, s.updater.installing())?;
    // L'appareil en session, ou les invités d'un partage (leurs prénoms).
    if !force && let Some(who) = share_api::occupant(s) {
        let key = match who {
            share_api::Occupant::Guests(_) => "station.restartConflictShare",
            share_api::Occupant::Borrower(_) => "station.restartConflictLend",
            share_api::Occupant::Host(_) => "station.restartConflict",
        };
        let msg = Msg::new(key).with("name", who.name());
        return Err(Refusal::new(StatusCode::CONFLICT, "session", msg).device(who.name()));
    }
    match s.station.restart() {
        Ok(()) => {
            tracing::info!("redémarrage de la Station lancé : {event}");
            s.activity.event("info", event);
            Ok(())
        }
        Err(station::Refused::TooSoon(wait)) => {
            let seconds = wait.as_secs().max(1);
            Err(Refusal::new(StatusCode::TOO_MANY_REQUESTS, "too_soon", Msg::new("station.tooSoon").with("seconds", seconds)).retry_after(seconds))
        }
        Err(station::Refused::Failed(e)) => {
            tracing::warn!("redémarrage de la Station : {e:#}");
            Err(Refusal { status: StatusCode::INTERNAL_SERVER_ERROR, code: "failed", error: i18n::error_text(i18n::current(), &e), device: None, retry_after: None })
        }
    }
}

fn accepted(body: Value) -> Response {
    (StatusCode::ACCEPTED, Json(body)).into_response()
}

/// « Redémarrer la Station » d'une app (compte Kyber, ou session d'Admin) :
/// `{"device_id", "force"}` facultatifs, `202 {"ok": true}` avant l'arrêt.
async fn stream_station_restart(
    State(s): State<AppState>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Response, Refusal> {
    let caller = station_caller(&s, peer, &headers).await?;
    let body = station_body(&body)?;
    let keys = ["events.stationRestarted", "events.stationRestartedAccount", "events.stationRestartedAdmin"];
    let event = station_event(&s, &caller, body.device_id.as_deref(), keys);
    restart_station(&s, body.force, event)?;
    // Les invités d'un partage l'apprennent (« arrêté ») avant l'arrêt.
    share_api::close_share(&s).await;
    Ok(accepted(json!({ "ok": true })))
}

/// « Redémarrer la Station » de la page Système d'Admin (sa session).
async fn admin_station_restart(State(s): State<AppState>, body: Bytes) -> Result<Response, Refusal> {
    let body = station_body(&body)?;
    restart_station(&s, body.force, Msg::new("events.stationRestartedAdmin"))?;
    share_api::close_share(&s).await;
    Ok(accepted(json!({ "ok": true })))
}

/// « Mettre à jour la Station » d'une app : la même installation
/// qu'« Installer maintenant » dans Admin. 409 sans version installable
/// depuis un appareil (à jour, installateur requis, paquet refusé), pendant
/// une installation, un redémarrage, ou une session (sauf `force`) ; sinon
/// le paquet est téléchargé et vérifié, l'applicateur lancé, puis
/// `202 {"ok": true, "version"}`.
async fn stream_station_update(
    State(s): State<AppState>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Response, Refusal> {
    let caller = station_caller(&s, peer, &headers).await?;
    let body = station_body(&body)?;
    let brief = s.updater.brief();
    station_busy(&s, brief.installing)?;
    let Some(version) = brief.available.as_deref() else {
        let msg = Msg::new("station.upToDate").with("version", crate::update::current());
        return Err(Refusal::new(StatusCode::CONFLICT, "up_to_date", msg));
    };
    if brief.requires_setup {
        let msg = Msg::new("station.requiresSetup").with("version", version).with("setup", brief.setup);
        return Err(Refusal::new(StatusCode::CONFLICT, "requires_setup", msg));
    }
    if !brief.installable {
        let reason = brief.refusal.clone().unwrap_or_else(|| Msg::new("update.unknownReason"));
        let msg = Msg::new("station.notInstallable").with("version", version).with_msg("reason", reason);
        return Err(Refusal::new(StatusCode::CONFLICT, "not_installable", msg));
    }
    if !body.force && let Some(who) = share_api::occupant(&s) {
        let key = match who {
            share_api::Occupant::Guests(_) => "update.sessionConflictShare",
            share_api::Occupant::Borrower(_) => "update.sessionConflictLend",
            share_api::Occupant::Host(_) => "update.sessionConflictNamed",
        };
        let msg = Msg::new(key).with("name", who.name());
        return Err(Refusal::new(StatusCode::CONFLICT, "session", msg).device(who.name()));
    }
    let installed = launch_install(&s).await?;
    share_api::close_share(&s).await;
    let keys = ["events.stationUpdate", "events.stationUpdateAccount", "events.installing"];
    let event = station_event(&s, &caller, body.device_id.as_deref(), keys).with("version", installed.as_str());
    s.activity.event("info", event);
    Ok(accepted(json!({ "ok": true, "version": installed })))
}

/// Une app annonce son écran avant de se connecter à Kyber.
async fn stream_prepare(
    State(s): State<AppState>,
    headers: HeaderMap,
    via: Option<axum::Extension<crate::relay::Via>>,
    Json(body): Json<PrepareBody>,
) -> ApiResult<Json<Value>> {
    let user = stream_user(&s, &headers)?;
    // Pendant un prêt, pas de session d'un autre appareil (docs/pret.md).
    if let Some(name) = s.lend.borrower() {
        return Err(ApiError::msg(StatusCode::CONFLICT, Msg::new("lend.inUse").with("name", name)));
    }
    let trigger = s.display.settings.lock().unwrap().trigger;
    let virtual_display = trigger != Trigger::Off && body.virtual_display != Some(false);
    // Disposition refusée avant de toucher à quoi que ce soit (son, écrans,
    // session en cours d'un autre appareil).
    if let Some(screens) = body.screens.as_deref().filter(|_| virtual_display) {
        crate::display::validate_screens(screens)?;
    }
    let _preparing = Preparing::new(&s.preparing);
    let port = s.kyber.port();
    let (device_id, device_name) = activity::identity(body.device.as_ref(), &user);
    // Par le relais : ce client joint Kyber, et le fil le dit une fois.
    let relay_session = via.map(|axum::Extension(v)| (v, device_id.clone(), device_name.clone()));
    let relay_ready = |s: &AppState| {
        if let Some((via, id, name)) = &relay_session
            && s.relay.note_session(via, id, name)
        {
            tracing::info!("{name} est en session par le relais");
            s.activity.event("info", Msg::new("events.relaySession").with("name", name.as_str()));
        }
    };

    // Compte accepté pendant la session d'un autre : Kyber ne le connaît pas encore.
    if s.pending_access.waits_for(&user) {
        tracing::info!("{device_name} se connecte avec un compte que Kyber n'a pas encore lu");
        // Port de Kyber : 8080 par défaut (comme `status`).
        grant_pending_access(&s, &user, &device_id, port.unwrap_or(8080)).await?;
    }

    // Reprise de main : un autre appareil est en session. La console lui
    // demande de finir (il l'apprend à son signal de vie suivant, 5 s au plus)
    // et attend qu'il ait rendu la main avant de préparer l'écran du nouveau.
    // L'app ouverte au début de ses sessions (réglée dans Admin), s'il en a une.
    let launch = s.activity.launch_of(&device_id).map(|(l, _)| l);
    if let activity::Claim::Waiting { previous } = s.activity.claim(&device_id, &device_name) {
        tracing::info!("{device_name} prend la main : on attend que {previous} se déconnecte");
        // Si l'arrivant veut la même app que le partant, elle reste ouverte.
        s.launcher.expect_takeover(&device_name, launch.as_ref());
        let deadline = tokio::time::Instant::now() + TAKEOVER_WAIT;
        loop {
            match s.activity.live_device_id() {
                Some(id) if id != device_id => {}
                _ => break,
            }
            if tokio::time::Instant::now() >= deadline {
                // Il ne répond plus : sa session est close, ses écrans et son
                // son seront repris par la préparation qui suit.
                s.activity.drop_live(Msg::new("events.noHandover").with("device", device_name.as_str()));
                break;
            }
            tokio::time::sleep(Duration::from_millis(200)).await;
        }
    }

    // Un partage qui n'est pas celui de cet appareil (lancé depuis le PC, ou
    // celui d'un appareil qui vient de rendre la main) s'arrête : l'écran
    // virtuel de l'arrivant éteindrait celui que regardent les invités.
    if s.share.is_open() && !s.share.hosted_by(&device_id) {
        share_api::close_share(&s).await;
    }

    // Le son d'abord : la sortie d'origine (souvent l'HDMI d'un écran)
    // disparaît quand les écrans physiques s'éteignent.
    let a = s.audio.clone();
    let channels = body.audio_channels;
    let audio = blocking(move || Ok(a.start(Origin::App, channels))).await?;
    // Veille du plein écran (mode jeu de la souris dans l'app).
    s.foreground.start(&s.data_dir);
    let mode_info = ModeInfo { width: body.width, height: body.height, refresh: body.refresh };
    let launch_name = launch.as_ref().map(|l| l.name.clone());
    if !virtual_display {
        s.activity.begin(body.device.clone(), &user, Some(mode_info), body.profile.clone());
        if let Some(l) = launch {
            s.launcher.session_started(device_id, device_name, l, body.profile.clone());
        }
        relay_ready(&s);
        return Ok(Json(json!({
            "virtual_display": false, "audio": audio, "kyber_port": port, "heartbeat_seconds": 5,
            "kyber_fingerprint": crate::kyber_proxy::kyber_fingerprint(&s),
            "launch": launch_name,
        })));
    }
    let d = s.display.clone();
    let (u, id, policy) = (user.clone(), device_id.clone(), body.mode_policy);
    let (applied, screens) = match body.screens.clone() {
        // Multi-écrans : la réponse donne les écrans en place, dans l'ordre de la demande.
        Some(screens) => {
            let started = blocking(move || d.start_screens(Origin::App, screens, Some(u), Some(id), policy)).await?;
            (started.applied, Some(started.screens))
        }
        None => {
            let mode = Mode { width: body.width, height: body.height, refresh: body.refresh };
            (blocking(move || d.start(Origin::App, mode, Some(u), Some(id), None, policy)).await?, None)
        }
    };
    // Modes des écrans pris, le principal en premier (fil d'événements).
    let taken: Vec<ModeInfo> = screens
        .iter()
        .flatten()
        .flatten()
        .map(|s| (s.primary, ModeInfo { width: s.width, height: s.height, refresh: s.refresh }))
        .fold(Vec::new(), |mut all, (primary, m)| {
            if primary { all.insert(0, m) } else { all.push(m) }
            all
        });
    let mode_info = taken.first().copied().unwrap_or(mode_info);
    // La session n'est ouverte qu'une fois l'écran en place : un échec ne
    // laisse pas un appareil « connecté » dans la vue d'ensemble.
    s.activity.begin_screens(body.device.clone(), &user, Some(mode_info), taken, body.profile.clone());
    // Son partage suit son écran principal (recréé, il a pu changer de nom).
    share_api::refresh_host_display(&s, &device_id).await;
    // L'app de l'appareil s'ouvre sur l'écran virtuel, en fond (elle attend
    // le déverrouillage de Windows s'il le faut).
    if let Some(l) = launch {
        s.launcher.session_started(device_id, device_name, l, body.profile.clone());
    }
    relay_ready(&s);
    let mut answer = json!({
        "virtual_display": true, "applied": applied, "audio": audio, "kyber_port": port, "heartbeat_seconds": 5,
        "kyber_fingerprint": crate::kyber_proxy::kyber_fingerprint(&s),
        "launch": launch_name,
    });
    if let Some(screens) = screens {
        answer["screens"] = json!(screens);
    }
    Ok(Json(answer))
}

/// Temps laissé à l'appareil en session pour rendre la main (signal de vie
/// toutes les 5 s, puis sa déconnexion), avant de reprendre l'écran de force.
const TAKEOVER_WAIT: Duration = Duration::from_secs(7);

#[derive(Deserialize, Default)]
struct HeartbeatBody {
    /// Latence médiane des dernières secondes, en millisecondes.
    #[serde(default)]
    latency_ms: Option<f64>,
    #[serde(default)]
    profile: Option<String>,
    /// Identifiant de l'appareil (celui de `device` dans `prepare`), à partir
    /// de 0.2.5 : la console distingue ainsi deux appareils sur le même compte.
    #[serde(default)]
    device_id: Option<String>,
    /// Politique du profil choisi en ce moment (voir `PrepareBody`).
    #[serde(default)]
    mode_policy: Option<PolicyChoice>,
}

/// Signal de vie d'une app, avec sa latence. `end` : la console demande la
/// fin (depuis la console, ou parce qu'un autre appareil prend la main) ;
/// `active` : l'écran virtuel en place est bien celui de cet appareil ;
/// `mode` : mode courant de l'écran virtuel (il a pu changer : un jeu) et
/// `displays` : écrans actifs sur l'ordinateur (null : pas encore relus) ;
/// `mode_policy` : la politique appliquée si le mode change.
async fn stream_heartbeat(
    State(s): State<AppState>,
    headers: HeaderMap,
    via: Option<axum::Extension<crate::relay::Via>>,
    body: Bytes,
) -> ApiResult<Json<Value>> {
    let user = stream_user(&s, &headers)?;
    // Corps facultatif : les apps d'avant 0.2.1 n'en envoient pas.
    let body: HeartbeatBody = serde_json::from_slice(&body).unwrap_or_default();
    let device = body.device_id.as_deref();
    // Par le relais : l'appareil en session reste autorisé à joindre Kyber.
    if let Some(axum::Extension(via)) = &via {
        let id = device.map(str::to_owned).unwrap_or_else(|| activity::identity(None, &user).0);
        if s.activity.live().is_some_and(|l| l.device_id == id) {
            s.relay.grant_device(via, &id);
        }
    }
    s.audio.heartbeat();
    s.foreground.touch();
    let end = s.activity.heartbeat(&user, device, body.latency_ms, body.profile);
    let active = s.display.heartbeat(device, body.mode_policy);
    let observed = if active { s.display.observed() } else { None };
    let mut answer = json!({
        "active": active,
        "end": end,
        "mode": observed.as_ref().map(|o| o.mode).filter(|m| m.width > 0),
        "displays": observed.as_ref().map(|o| o.displays),
        "mode_policy": if active { s.display.policy() } else { None },
    });
    // Le partage de cet appareil (KyberNow Share), absent sans partage.
    if let Some(share) = share_api::for_host(&s, device) {
        answer["share"] = share;
    }
    // Le prêt (code, demandes, prêt en cours), absent quand il n'y a rien à dire.
    if let Some(lend) = lend_api::for_host(&s) {
        answer["lend"] = lend;
    }
    Ok(Json(answer))
}

/// App au premier plan sur l'ordinateur : plein écran, jeu (l'app du Mac
/// l'interroge chaque seconde pour le mode de la souris).
async fn stream_foreground(State(s): State<AppState>, headers: HeaderMap) -> ApiResult<Json<Value>> {
    stream_user(&s, &headers)?;
    Ok(Json(s.foreground.state()))
}

#[derive(Deserialize)]
struct PreviewQuery {
    width: Option<u32>,
}

/// Aperçu de l'écran principal en JPEG (fond de l'accueil des apps), daté
/// par `X-Captured-At`. 503 si l'écran ne peut pas être capturé.
async fn stream_preview(
    State(s): State<AppState>,
    headers: HeaderMap,
    Query(q): Query<PreviewQuery>,
) -> ApiResult<Response> {
    stream_user(&s, &headers)?;
    let width = q.width.unwrap_or(preview::DEFAULT_WIDTH).clamp(preview::MIN_WIDTH, preview::MAX_WIDTH);
    // Pendant une session (la nôtre ou un autre client de Kyber), aucune capture.
    let (d, a) = (s.display.clone(), s.audio.clone());
    let streaming = s.foreground.active()
        || blocking(move || {
            Ok(gpu::encoding()
                || !d.status()["session"].is_null()
                || !a.status()["session"].is_null())
        })
        .await?;
    let shot = s.preview.get(width, streaming).await.map_err(|e| ApiError(StatusCode::SERVICE_UNAVAILABLE, e))?;
    let captured_at = time::OffsetDateTime::from(shot.captured_at)
        .format(&time::format_description::well_known::Rfc3339)
        .unwrap_or_default();
    Ok((
        [
            (header::CONTENT_TYPE, "image/jpeg".to_string()),
            (header::CACHE_CONTROL, "no-store".to_string()),
            (header::HeaderName::from_static("x-captured-at"), captured_at),
        ],
        shot.jpeg,
    )
        .into_response())
}

#[derive(Deserialize, Default)]
struct EndBody {
    /// Latence médiane de toute la session, en millisecondes.
    #[serde(default)]
    latency_ms: Option<f64>,
    /// Identifiant de l'appareil (voir `HeartbeatBody`).
    #[serde(default)]
    device_id: Option<String>,
}

async fn stream_end(State(s): State<AppState>, headers: HeaderMap, body: Bytes) -> ApiResult<Json<Value>> {
    let user = stream_user(&s, &headers)?;
    let body: EndBody = serde_json::from_slice(&body).unwrap_or_default();
    let owned = s.activity.end(&user, body.device_id.as_deref(), body.latency_ms);
    // Un appareil qui a été remplacé par un autre se déconnecte à son tour :
    // il ne doit pas emporter l'écran et le son de celui qui a pris la main.
    if !owned {
        if let Some(live) = s.activity.live() {
            let display_is_mine = body.device_id.as_deref().is_some_and(|d| s.display.owned_by(d));
            if !display_is_mine {
                tracing::info!("fin de session d'un appareil remplacé : l'écran reste à {}", live.name);
                return Ok(Json(json!({ "ok": true, "display_restored": true, "audio_restored": true, "kept_for": live.name })));
            }
        }
    }
    s.foreground.stop();
    let (ended_id, _) = activity::identity(None, &user);
    let ended_id = body.device_id.clone().unwrap_or(ended_id);
    // Par le relais, il ne joint plus Kyber.
    s.relay.revoke_device(&ended_id);
    // Fermer la session de l'hôte arrête son partage : ses invités sont
    // coupés avant que son écran virtuel ne disparaisse.
    if s.share.hosted_by(&ended_id) {
        share_api::close_share(&s).await;
    }
    let (d, a, l) = (s.display.clone(), s.audio.clone(), s.launcher.clone());
    let report = blocking(move || {
        // L'app ouverte pour cet appareil, fermée si l'appareil le demande,
        // avant que ses fenêtres ne migrent vers les écrans d'origine.
        l.session_ended(&ended_id);
        // Les écrans d'abord : la sortie audio d'origine revient avec eux.
        d.stop("l'appareil s'est déconnecté");
        a.stop("l'appareil s'est déconnecté");
        // Ce que l'app affiche à l'utilisateur en fin de session.
        let display = d.status();
        let audio = a.status();
        Ok(json!({
            "ok": true,
            "display_restored": display["session"].is_null(),
            "audio_restored": audio["session"].is_null(),
            "error": display["last_error"].as_str().or(audio["last_error"].as_str()),
        }))
    })
    .await?;
    Ok(Json(report))
}

async fn devices(State(s): State<AppState>) -> Json<Value> {
    Json(json!({ "devices": s.activity.devices() }))
}

#[derive(Deserialize)]
struct LaunchBody {
    /// Le réglage, ou `null` pour ne plus rien ouvrir.
    #[serde(default)]
    launch: Option<Launch>,
}

/// Règle l'app ouverte au début des sessions d'un appareil.
async fn set_launch(State(s): State<AppState>, Path(id): Path<String>, Json(body): Json<LaunchBody>) -> ApiResult<Json<Value>> {
    let launch = match body.launch {
        Some(l) => {
            let launcher = s.launcher.clone();
            Some(blocking(move || launcher.validate(l)).await?)
        }
        None => None,
    };
    let devices = s
        .activity
        .set_launch(&id, launch.clone())
        .ok_or_else(|| ApiError::msg(StatusCode::NOT_FOUND, Msg::new("api.unknownDevice")))?;
    let name = devices.iter().find(|d| d.id == id).map(|d| d.name.clone()).unwrap_or_default();
    match &launch {
        Some(l) => s.activity.event("info", Msg::new("events.launchSet").with("name", name).with("app", l.name.as_str())),
        None => s.activity.event("info", Msg::new("events.launchCleared").with("name", name)),
    }
    Ok(Json(json!({ "devices": devices })))
}

/// Le catalogue des apps de l'ordinateur (menu Démarrer de l'utilisateur,
/// lanceurs détectés, dossiers de l'explorateur), lu la première fois.
async fn apps(State(s): State<AppState>) -> ApiResult<Json<Value>> {
    let launcher = s.launcher.clone();
    let catalogue = blocking(move || {
        let c = launcher.catalogue();
        Ok(if c.refreshed_at == 0 { launcher.refresh() } else { c })
    })
    .await?;
    Ok(Json(serde_json::to_value(catalogue).map_err(anyhow::Error::from)?))
}

/// Relit le catalogue (l'agent tourne dans la session de l'utilisateur).
async fn apps_refresh(State(s): State<AppState>) -> ApiResult<Json<Value>> {
    let launcher = s.launcher.clone();
    let catalogue = blocking(move || Ok(launcher.refresh())).await?;
    Ok(Json(serde_json::to_value(catalogue).map_err(anyhow::Error::from)?))
}

/// L'icône d'une app du catalogue (PNG).
async fn app_icon(State(s): State<AppState>, Path(name): Path<String>) -> Response {
    let Some(path) = s.launcher.icon_path(&name) else {
        return StatusCode::NOT_FOUND.into_response();
    };
    match tokio::fs::read(path).await {
        Ok(bytes) => (
            [(header::CONTENT_TYPE, "image/png".to_string()), (header::CACHE_CONTROL, "private, max-age=3600".to_string())],
            bytes,
        )
            .into_response(),
        Err(_) => StatusCode::NOT_FOUND.into_response(),
    }
}

#[derive(Deserialize)]
struct BrowseQuery {
    path: Option<String>,
}

/// Explorateur « Autre… » : les dossiers autorisés, ou le contenu de l'un d'eux.
async fn apps_browse(State(s): State<AppState>, Query(q): Query<BrowseQuery>) -> ApiResult<Json<Value>> {
    let launcher = s.launcher.clone();
    Ok(Json(blocking(move || launcher.browse(q.path.as_deref())).await?))
}

/// Oublie un appareil. Appairé par code, il perd aussi son accès : son compte
/// propre est supprimé (et sa session, s'il en a une, prend fin).
async fn forget_device(State(s): State<AppState>, Path(id): Path<String>) -> ApiResult<Json<Value>> {
    let live = s.activity.live().is_some_and(|l| l.device_id == id);
    let device = s.activity.forget(&id).ok_or_else(|| ApiError::msg(StatusCode::NOT_FOUND, Msg::new("api.unknownDevice")))?;
    let mut revoked = false;
    if device.own_account && s.kyber.users()?.iter().any(|u| u.username == device.user) {
        s.kyber.delete_user(&device.user)?;
        if live {
            s.activity.request_end();
        }
        s.activity.event("info", Msg::new("events.accessRevoked").with("name", device.name.as_str()).with("account", device.user.as_str()));
        apply_accounts(&s, &device.user, &device.name, false).await;
        revoked = true;
    }
    Ok(Json(json!({
        "devices": s.activity.devices(),
        "revoked": revoked,
        "restart_pending": s.restart_pending.load(Ordering::Relaxed),
        "pending_access": s.pending_access.snapshot(),
    })))
}

/// Intervalle de la surveillance des comptes en attente.
const ACCESS_CHECK: Duration = Duration::from_secs(5);
/// Dans `prepare`, temps laissé à Kyber pour se libérer (l'encodage d'une
/// session qui vient de finir compte encore 15 s) avant d'y renoncer.
const ACCESS_FREE_WAIT: Duration = Duration::from_secs(20);
/// Puis temps laissé à Kyber redémarré pour rouvrir son port.
const KYBER_UP_WAIT: Duration = Duration::from_secs(12);

/// Kyber sert-il quelqu'un d'autre que l'appareil `except` ? Le redémarrer
/// couperait cette connexion (les invités d'un partage comptent).
pub(crate) async fn kyber_busy(s: &AppState, except: Option<&str>) -> bool {
    if s.activity.live().is_some_and(|l| Some(l.device_id.as_str()) != except) {
        return true;
    }
    // Un prêt en cours : l'ami joue.
    if s.lend.loan().is_some() {
        return true;
    }
    if s.share.guest_count() > 0 {
        return true;
    }
    // Une autre préparation en cours : son appareil va se connecter.
    if s.preparing.load(Ordering::SeqCst) > usize::from(except.is_some()) {
        return true;
    }
    let (d, a, except) = (s.display.clone(), s.audio.clone(), except.map(str::to_owned));
    blocking(move || {
        let display = !d.status()["session"].is_null() && !except.as_deref().is_some_and(|e| d.owned_by(e));
        // Le son n'a pas de propriétaire : pour `except`, ce peut être le sien
        // (tentative précédente) ; une autre connexion se voit à l'encodage.
        let audio = except.is_none() && !a.status()["session"].is_null();
        Ok(gpu::encoding() || display || audio)
    })
    .await
    .unwrap_or(true)
}

/// Redémarre Kyber pour qu'il lise les comptes en attente, s'il ne sert
/// personne d'autre que `except`. Vrai si Kyber connaît alors tous les
/// comptes qui attendaient. `context` complète l'événement du fil
/// (« , maintenant que personne n'est connecté »).
async fn apply_pending_access(s: &AppState, except: Option<&str>, context: Option<&str>) -> bool {
    let _one = s.kyber_restart.lock().await;
    let taken = s.pending_access.snapshot();
    if taken.is_empty() {
        return true;
    }
    match s.service.state().ok() {
        Some(service::ServiceState::Running) => {}
        // Arrêté : il lira les comptes à son démarrage.
        Some(service::ServiceState::Stopped | service::ServiceState::NotInstalled) => {
            s.pending_access.applied(&taken);
            return true;
        }
        // En train de démarrer ou de s'arrêter : on ne sait pas ce qu'il a lu.
        _ => return false,
    }
    if kyber_busy(s, except).await {
        return false;
    }
    let why = PendingAccess::reason(&taken);
    match service::restart(s.service.as_ref()).await {
        Ok(()) => {
            s.pending_access.applied(&taken);
            s.restart_pending.store(false, Ordering::Relaxed);
            tracing::info!("Kyber redémarré pour {why}");
            let context = context.map_or(json!(""), |key| Msg::new(key).to_value());
            s.activity.event("info", Msg::new("events.restartedFor").with_msg("why", why).with("context", context));
            true
        }
        Err(e) => {
            tracing::warn!("redémarrage de Kyber pour {why} : {e:#}");
            false
        }
    }
}

/// Un compte d'appareil ajouté ou retiré ne compte pour Kyber qu'à son
/// redémarrage : tout de suite (en arrière-plan) si personne n'est connecté,
/// sinon dès que Kyber redevient libre (`supervise_access`), ou à la première
/// connexion de l'appareil (`stream_prepare`). Vrai si Kyber redémarre maintenant.
async fn apply_accounts(s: &AppState, account: &str, device: &str, grant: bool) -> bool {
    s.pending_access.add(account, device, grant);
    let running = s.service.state().ok() == Some(service::ServiceState::Running);
    if running && kyber_busy(s, None).await {
        let why = PendingAccess::reason(&s.pending_access.snapshot());
        s.activity.event("info", Msg::new("events.restartWaiting").with_msg("why", why));
        return false;
    }
    let s = s.clone();
    tokio::spawn(async move { apply_pending_access(&s, None, None).await });
    running
}

/// Surveillance des comptes en attente : Kyber redémarre dès qu'il ne sert
/// plus personne (fin de session, plus de signal de vie ni d'encodage).
pub fn supervise_access(s: AppState) {
    tokio::spawn(async move {
        let mut tick = tokio::time::interval(ACCESS_CHECK);
        loop {
            tick.tick().await;
            if !s.pending_access.is_empty() {
                apply_pending_access(&s, None, Some("events.contextNobody")).await;
            }
        }
    });
}

/// Compte une préparation de session en cours, le temps de sa vie.
struct Preparing<'a>(&'a AtomicUsize);

impl<'a> Preparing<'a> {
    fn new(count: &'a AtomicUsize) -> Self {
        count.fetch_add(1, Ordering::SeqCst);
        Preparing(count)
    }
}

impl Drop for Preparing<'_> {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::SeqCst);
    }
}

/// Le compte de cet appareil attend que Kyber le lise (accepté pendant la
/// session d'un autre) : Kyber redémarre avant de répondre, si personne
/// d'autre ne s'en sert. Sinon, un message clair plutôt qu'un refus de Kyber.
async fn grant_pending_access(s: &AppState, user: &str, device_id: &str, port: u16) -> ApiResult<()> {
    let busy = |name: Option<String>| {
        let who = name.map_or_else(|| Msg::new("api.currentConnection"), |n| Msg::new("api.sessionOf").with("name", n));
        ApiError::msg(StatusCode::CONFLICT, Msg::new("api.accessPending").with_msg("who", who))
    };
    // L'appareil en session (pas celui-ci), ou les invités d'un partage.
    let other_live = || {
        s.activity
            .live()
            .filter(|l| l.device_id != device_id)
            .map(|l| l.name)
            .or_else(|| share_api::guest_names(s))
    };
    // Ne jamais couper la session d'un autre appareil (ni la lui prendre :
    // Kyber refuserait de toute façon ce compte tant qu'il n'a pas redémarré).
    if let Some(name) = other_live() {
        return Err(busy(Some(name)));
    }
    let deadline = tokio::time::Instant::now() + ACCESS_FREE_WAIT;
    while s.pending_access.waits_for(user) {
        if apply_pending_access(s, Some(device_id), Some("events.contextConnecting")).await {
            break;
        }
        if let Some(name) = other_live() {
            return Err(busy(Some(name)));
        }
        if tokio::time::Instant::now() >= deadline {
            return Err(busy(None));
        }
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
    if s.service.state().ok() != Some(service::ServiceState::Running) {
        return Ok(());
    }
    // Kyber redémarré : son port rouvre en quelques secondes.
    let deadline = tokio::time::Instant::now() + KYBER_UP_WAIT;
    while tokio::time::Instant::now() < deadline {
        let up = tokio::time::timeout(Duration::from_millis(500), tokio::net::TcpStream::connect(("127.0.0.1", port)))
            .await
            .is_ok_and(|r| r.is_ok());
        if up {
            break;
        }
        tokio::time::sleep(Duration::from_millis(300)).await;
    }
    Ok(())
}

/// Supprime les comptes créés pour des demandes acceptées dont l'appareil
/// n'est jamais venu chercher l'accès.
fn delete_orphans(s: &AppState) {
    for user in s.pairing.take_orphans() {
        s.pending_access.forget(&user);
        match s.kyber.delete_user(&user) {
            Ok(()) => tracing::info!("accès jamais livré, compte {user} supprimé"),
            Err(e) => tracing::warn!("suppression du compte {user} : {e:#}"),
        }
    }
}

#[derive(Deserialize)]
struct PairBody {
    code: String,
    device: DeviceInfo,
}

/// Un appareil sans compte demande l'accès avec le code qu'il affiche.
async fn pair_request(
    State(s): State<AppState>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    Json(body): Json<PairBody>,
) -> ApiResult<Json<Value>> {
    delete_orphans(&s);
    let name = body.device.name.clone();
    match s.pairing.request(body.code.trim(), body.device, peer.ip().to_canonical()) {
        Ok(id) => {
            tracing::info!("demande d'appairage de « {} » depuis {}", name.trim(), peer.ip());
            Ok(Json(json!({ "id": id, "expires_in": pairing::REQUEST_TTL.as_secs() })))
        }
        Err(RequestError::Invalid(key)) => Err(ApiError::msg(StatusCode::BAD_REQUEST, Msg::new(key))),
        Err(RequestError::TooMany) => {
            tracing::warn!("demandes d'appairage refusées depuis {} : trop de demandes", peer.ip());
            Err(ApiError::msg(StatusCode::TOO_MANY_REQUESTS, Msg::new("api.pairTooMany")))
        }
    }
}

/// État d'une demande, interrogé par l'appareil. Une demande inconnue (ou dont
/// l'accès a déjà été livré) est « expirée ».
async fn pair_status(State(s): State<AppState>, Path(id): Path<String>) -> Response {
    delete_orphans(&s);
    let body = match s.pairing.status(&id) {
        pairing::Status::Pending => json!({ "status": "pending" }),
        pairing::Status::Refused => json!({ "status": "refused" }),
        pairing::Status::Expired => json!({ "status": "expired" }),
        pairing::Status::Accepted { username, password } => {
            tracing::info!("accès du compte {username} livré à l'appareil");
            json!({ "status": "accepted", "username": username, "password": password })
        }
    };
    ([(header::CACHE_CONTROL, "no-store")], Json(body)).into_response()
}

fn request_gone() -> ApiError {
    ApiError::msg(StatusCode::GONE, Msg::new("api.requestGone"))
}

/// La console accepte : l'appareil reçoit un compte Kyber propre à lui, que
/// la console et Kyber acceptent tous les deux, révocable dans Appareils.
async fn pair_accept(State(s): State<AppState>, Path(id): Path<String>) -> ApiResult<Json<Value>> {
    let device = s.pairing.device(&id).ok_or_else(request_gone)?;
    let taken: Vec<String> = s.kyber.users()?.into_iter().map(|u| u.username).collect();
    let username = pairing::account_name(&device.name, &taken);
    let password = pairing::secret();
    s.kyber.upsert_user(&username, &password)?;
    if !s.pairing.accept(&id, username.clone(), password) {
        let _ = s.kyber.delete_user(&username);
        return Err(request_gone());
    }
    if let Some(old) = s.activity.paired(&device, &username) {
        // Appairé de nouveau : l'accès précédent ne sert plus.
        if let Err(e) = s.kyber.delete_user(&old) {
            tracing::warn!("suppression de l'ancien compte {old} : {e:#}");
        }
    }
    tracing::info!("appairage de « {} » accepté : compte {username}", device.name);
    let restarted = apply_accounts(&s, &username, &device.name, true).await;
    Ok(Json(json!({
        "ok": true,
        "username": username,
        "restarted": restarted,
        "restart_pending": s.restart_pending.load(Ordering::Relaxed),
        "pending_access": s.pending_access.snapshot(),
        "pair_requests": s.pairing.pending(),
        "devices": s.activity.devices(),
    })))
}

async fn pair_refuse(State(s): State<AppState>, Path(id): Path<String>) -> ApiResult<Json<Value>> {
    let name = s.pairing.refuse(&id).ok_or_else(request_gone)?;
    s.activity.event("info", Msg::new("events.pairRefused").with("name", name));
    Ok(Json(json!({ "ok": true, "pair_requests": s.pairing.pending() })))
}

#[derive(Deserialize)]
struct EventsQuery {
    limit: Option<usize>,
}

/// Le fil, du plus récent au plus ancien, chaque phrase écrite dans la langue
/// de la requête (`text`), avec sa clé et ses paramètres (`key`, `params`).
async fn events(State(s): State<AppState>, Query(q): Query<EventsQuery>) -> Json<Value> {
    Json(json!({ "events": s.activity.events(q.limit.unwrap_or(50).min(300), i18n::current()) }))
}

/// Met fin à la session en cours. Une app KyberNow l'apprend à son signal de
/// vie suivant et se déconnecte proprement ; un autre client (navigateur,
/// ligne de commande) est coupé en redémarrant Kyber.
async fn end_session(State(s): State<AppState>) -> ApiResult<Json<Value>> {
    // Un prêt en cours : Admin reprend l'ordinateur tout de suite.
    if let Some(loan) = s.lend.loan() {
        lend_api::finish(&s, &loan.id, crate::lend::Reason::TakenBack).await;
        return Ok(Json(json!({ "ok": true, "method": "lend" })));
    }
    if s.activity.request_end() {
        // Le partage de l'hôte s'arrête avec sa session (`share_api::supervise`).
        return Ok(Json(json!({ "ok": true, "method": "app" })));
    }
    // Un partage sans session (lancé depuis le PC) : ses invités sont
    // prévenus (« arrêté ») et coupés de Kyber, jamais par un redémarrage.
    let shared = share_api::close_share(&s).await.is_some();
    let (d, a) = (s.display.clone(), s.audio.clone());
    let active = blocking(move || Ok(!d.status()["session"].is_null() || !a.status()["session"].is_null())).await?;
    if !active {
        return Ok(Json(json!({ "ok": true, "method": if shared { "share" } else { "none" } })));
    }
    let (d, a) = (s.display.clone(), s.audio.clone());
    blocking(move || {
        d.stop("session terminée depuis la console");
        a.stop("session terminée depuis la console");
        Ok(())
    })
    .await?;
    let taken = s.pending_access.snapshot();
    service::restart(s.service.as_ref()).await?;
    s.pending_access.applied(&taken);
    s.activity.event("info", Msg::new("events.connectionsCut"));
    Ok(Json(json!({ "ok": true, "method": "restart" })))
}

/// Aperçu de l'écran pour la vue d'ensemble de la console.
async fn admin_preview(State(s): State<AppState>, Query(q): Query<PreviewQuery>) -> ApiResult<Response> {
    let width = q.width.unwrap_or(preview::DEFAULT_WIDTH).clamp(preview::MIN_WIDTH, preview::MAX_WIDTH);
    let (d, a) = (s.display.clone(), s.audio.clone());
    let streaming = s.foreground.active()
        || blocking(move || {
            Ok(gpu::encoding()
                || !d.status()["session"].is_null()
                || !a.status()["session"].is_null())
        })
        .await?;
    let shot = s.preview.get(width, streaming).await.map_err(|e| ApiError(StatusCode::SERVICE_UNAVAILABLE, e))?;
    let captured_at = time::OffsetDateTime::from(shot.captured_at)
        .format(&time::format_description::well_known::Rfc3339)
        .unwrap_or_default();
    Ok((
        [
            (header::CONTENT_TYPE, "image/jpeg".to_string()),
            (header::CACHE_CONTROL, "no-store".to_string()),
            (header::HeaderName::from_static("x-captured-at"), captured_at),
        ],
        shot.jpeg,
    )
        .into_response())
}

#[cfg(all(test, not(windows)))]
mod tests {
    use std::path::PathBuf;
    use std::sync::{Arc, Mutex};

    use super::*;
    use crate::service::{ServiceControl, ServiceState};

    /// Faux service Kyber, en marche, qui compte ses arrêts (redémarrages).
    struct FakeKyber {
        running: Mutex<bool>,
        stops: Arc<AtomicUsize>,
    }

    impl ServiceControl for FakeKyber {
        fn state(&self) -> anyhow::Result<ServiceState> {
            Ok(if *self.running.lock().unwrap() { ServiceState::Running } else { ServiceState::Stopped })
        }
        fn start(&self) -> anyhow::Result<()> {
            *self.running.lock().unwrap() = true;
            Ok(())
        }
        fn stop(&self) -> anyhow::Result<()> {
            *self.running.lock().unwrap() = false;
            self.stops.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
    }

    struct Console {
        s: AppState,
        restarts: Arc<AtomicUsize>,
        dir: PathBuf,
    }

    impl Drop for Console {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.dir);
        }
    }

    fn console() -> Console {
        let dir = std::env::temp_dir().join(format!("kn-access-{}-{}", std::process::id(), rand::random::<u64>()));
        std::fs::create_dir_all(&dir).unwrap();
        let restarts = Arc::new(AtomicUsize::new(0));
        let service = FakeKyber { running: Mutex::new(true), stops: restarts.clone() };
        Console { s: AppState::for_tests(dir.clone(), Box::new(service)), restarts, dir }
    }

    fn device(id: &str, name: &str) -> DeviceInfo {
        DeviceInfo { id: id.into(), name: name.into(), kind: None, os: None, app: None }
    }

    fn restarted_event(s: &AppState) -> Option<String> {
        s.activity.events(20, i18n::Lang::Fr).into_iter().map(|e| e.text).find(|t| t.starts_with("Serveur redémarré pour"))
    }

    #[tokio::test]
    async fn accepted_during_a_session_is_applied_when_it_ends() {
        let c = console();
        c.s.activity.begin(Some(device("mac", "Mac")), "mac", None, None);
        assert!(!apply_accounts(&c.s, "apple-tv", "Apple TV", true).await);
        assert!(c.s.pending_access.waits_for("apple-tv"));
        // Tant que le Mac est en session, personne ne redémarre Kyber.
        assert!(!apply_pending_access(&c.s, None, None).await);
        assert_eq!(c.restarts.load(Ordering::SeqCst), 0);

        c.s.activity.end("mac", Some("mac"), None);
        assert!(apply_pending_access(&c.s, None, Some("events.contextNobody")).await);
        assert_eq!(c.restarts.load(Ordering::SeqCst), 1);
        assert!(c.s.pending_access.is_empty());
        assert_eq!(
            restarted_event(&c.s).as_deref(),
            Some("Serveur redémarré pour donner l'accès à Apple TV, maintenant que personne n'est connecté")
        );
        let english = c.s.activity.events(20, i18n::Lang::En).into_iter().map(|e| e.text).find(|t| t.starts_with("Server restarted"));
        assert_eq!(english.as_deref(), Some("Server restarted to give access to Apple TV, now that nobody is connected"));
        // Plus rien n'attend : pas de second redémarrage.
        assert!(apply_pending_access(&c.s, None, None).await);
        assert_eq!(c.restarts.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn accepted_when_free_restarts_right_away() {
        let c = console();
        assert!(apply_accounts(&c.s, "apple-tv", "Apple TV", true).await);
        for _ in 0..100 {
            if c.restarts.load(Ordering::SeqCst) == 1 && c.s.pending_access.is_empty() {
                return;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        panic!("Kyber n'a pas redémarré");
    }

    #[tokio::test]
    async fn another_preparation_holds_the_restart() {
        let c = console();
        c.s.pending_access.add("apple-tv", "Apple TV", true);
        let other = Preparing::new(&c.s.preparing);
        assert!(!apply_pending_access(&c.s, None, None).await);
        drop(other);
        assert!(apply_pending_access(&c.s, None, None).await);
        assert_eq!(c.restarts.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn prepare_restarts_kyber_for_a_pending_account() {
        let c = console();
        let kyber = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = kyber.local_addr().unwrap().port();
        c.s.pending_access.add("apple-tv", "Apple TV", true);
        let _me = Preparing::new(&c.s.preparing);
        if let Err(e) = grant_pending_access(&c.s, "apple-tv", "tv-1", port).await {
            panic!("{}", e.1);
        }
        assert_eq!(c.restarts.load(Ordering::SeqCst), 1);
        assert!(!c.s.pending_access.waits_for("apple-tv"));
        assert_eq!(restarted_event(&c.s).as_deref(), Some("Serveur redémarré pour donner l'accès à Apple TV, qui se connecte"));
    }

    #[tokio::test]
    async fn prepare_never_cuts_another_session() {
        let c = console();
        c.s.activity.begin(Some(device("mac", "Mac")), "mac", None, None);
        c.s.pending_access.add("apple-tv", "Apple TV", true);
        let _me = Preparing::new(&c.s.preparing);
        let err = grant_pending_access(&c.s, "apple-tv", "tv-1", 1).await.unwrap_err();
        assert_eq!(err.0, StatusCode::CONFLICT);
        assert!(err.1.contains("la session de Mac"), "{}", err.1);
        assert_eq!(c.restarts.load(Ordering::SeqCst), 0);
        assert!(c.s.pending_access.waits_for("apple-tv"));
        // Ni la reprise de main : le Mac reste en session.
        assert_eq!(c.s.activity.live_device_id().as_deref(), Some("mac"));
    }

    /// En-têtes d'une app : Basic du compte Kyber `user`.
    fn basic(user: &str, password: &str) -> HeaderMap {
        const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
        let bytes = format!("{user}:{password}").into_bytes();
        let mut out = String::new();
        for chunk in bytes.chunks(3) {
            let n = chunk.iter().enumerate().fold(0u32, |n, (i, b)| n | (u32::from(*b) << (16 - 8 * i)));
            for i in 0..4 {
                out.push(if i <= chunk.len() { ALPHABET[(n >> (18 - 6 * i) & 63) as usize] as char } else { '=' });
            }
        }
        let mut h = HeaderMap::new();
        h.insert(header::AUTHORIZATION, format!("Basic {out}").parse().unwrap());
        h
    }

    /// Console avec un compte Kyber `mac` (mot de passe `mot-de-passe-mac`).
    fn console_with_account() -> Console {
        let c = console();
        std::fs::write(c.dir.join("kyber_config.toml"), "").unwrap();
        c.s.kyber.upsert_user("mac", "mot-de-passe-mac").unwrap();
        assert!(base64_decode(basic("mac", "mot-de-passe-mac")[header::AUTHORIZATION].to_str().unwrap().strip_prefix("Basic ").unwrap()).as_deref() == Some("mac:mot-de-passe-mac"));
        c
    }

    fn peer() -> ConnectInfo<SocketAddr> {
        ConnectInfo("192.168.1.20:50000".parse().unwrap())
    }

    fn texts(s: &AppState, lang: i18n::Lang) -> Vec<String> {
        s.activity.events(20, lang).into_iter().map(|e| e.text).collect()
    }

    async fn restart_from(s: &AppState, headers: HeaderMap, body: &str) -> Result<Response, Refusal> {
        stream_station_restart(State(s.clone()), peer(), headers, Bytes::from(body.to_string())).await
    }

    fn prepare_body(screens: Value) -> Json<PrepareBody> {
        let body = json!({ "width": 3840, "height": 2160, "refresh": 160, "screens": screens, "device": { "id": "mac-1", "name": "Mac" } });
        Json(serde_json::from_value(body).unwrap())
    }

    #[tokio::test]
    async fn prepare_refuses_a_bad_layout_without_touching_anything() {
        let c = console_with_account();
        let bad = [
            // Deux écrans qui se chevauchent.
            json!([{ "width": 3840, "height": 2160, "refresh": 160, "x": 0, "y": 0, "primary": true },
                   { "width": 5120, "height": 2880, "refresh": 60, "x": 1000, "y": 0 }]),
            // Pas d'écran principal.
            json!([{ "width": 3840, "height": 2160, "refresh": 160, "x": 0, "y": 0 }]),
            // Écran séparé des autres.
            json!([{ "width": 3840, "height": 2160, "refresh": 160, "x": 0, "y": 0, "primary": true },
                   { "width": 1920, "height": 1080, "refresh": 60, "x": 5000, "y": 0 }]),
            json!([]),
        ];
        for screens in bad {
            let err = stream_prepare(State(c.s.clone()), basic("mac", "mot-de-passe-mac"), None, prepare_body(screens.clone())).await.unwrap_err();
            assert_eq!(err.0, StatusCode::BAD_REQUEST, "{screens}");
            assert!(!err.1.is_empty());
        }
        // Rien n'a bougé : ni écran virtuel, ni son, ni session.
        assert!(c.s.display.status()["session"].is_null());
        assert!(c.s.audio.status()["session"].is_null());
        assert!(c.s.activity.live().is_none());
        // Erreur dans le format de l'API, dans la langue de la requête.
        let err = i18n::with_lang(i18n::Lang::En, || crate::display::validate_screens(&[]).unwrap_err());
        assert_eq!(ApiError::from(err).into_response().status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn prepare_screens_field_is_optional() {
        // Les apps qui ne gèrent pas le multi-écrans n'envoient pas `screens`.
        let body: PrepareBody = serde_json::from_value(json!({ "width": 1920, "height": 1080, "refresh": 60 })).unwrap();
        assert!(body.screens.is_none());
        let body: PrepareBody = serde_json::from_value(json!({ "width": 3840, "height": 2160, "refresh": 160,
            "screens": [{ "width": 3840, "height": 2160, "refresh": 160, "x": 0, "y": 0, "primary": true },
                        { "width": 3456, "height": 2234, "refresh": 60, "x": -3456, "y": 400 }] })).unwrap();
        let screens = body.screens.unwrap();
        assert!(screens[0].primary && !screens[1].primary);
        assert_eq!((screens[1].x, screens[1].y), (-3456, 400));
        // Réponse : le principal porte `primary`, les autres non.
        assert_eq!(json!(screens[1]), json!({ "width": 3456, "height": 2234, "refresh": 60, "x": -3456, "y": 400 }));
    }

    #[tokio::test]
    async fn info_tells_the_apps_about_the_station() {
        let c = console();
        let info = station_info(&c.s);
        assert_eq!(info["version"], crate::update::current());
        assert_eq!(info["restarting"], false);
        assert_eq!(info["update"], json!({ "available": false, "version": null, "installable": false, "requires_setup": false, "installing": false }));
    }

    #[tokio::test]
    async fn info_says_how_many_displays_the_station_can_create() {
        let c = console_with_account();
        let Json(info) = stream_info(State(c.s.clone()), basic("mac", "mot-de-passe-mac")).await.ok().unwrap();
        assert_eq!(info["virtual_display"]["max_screens"], 4);
        // Écran virtuel inutilisable : un seul écran annoncé.
        assert_eq!(max_screens(false, Trigger::Apps), 1);
        assert_eq!(max_screens(true, Trigger::Off), 1);
        assert_eq!(max_screens(true, Trigger::All), 4);
        c.s.display.settings.lock().unwrap().trigger = Trigger::Off;
        let Json(info) = stream_info(State(c.s.clone()), basic("mac", "mot-de-passe-mac")).await.ok().unwrap();
        assert_eq!(info["virtual_display"]["max_screens"], 1);
    }

    #[tokio::test]
    async fn info_gives_the_cards_to_wake_the_computer() {
        let c = console_with_account();
        let Json(info) = stream_info(State(c.s.clone()), basic("mac", "mot-de-passe-mac")).await.ok().unwrap();
        // Lu en mémoire : vide tant que la relecture de fond n'a pas tourné (jamais en test).
        assert_eq!(info["wake"], json!(crate::wake::cards()));
        assert!(info["wake"].is_array());
    }

    #[tokio::test]
    async fn a_device_restarts_the_station_after_asking_about_the_session() {
        let c = console_with_account();
        c.s.activity.begin(Some(device("mac-1", "Mac de Léa")), "mac", None, None);
        let refused = restart_from(&c.s, basic("mac", "mot-de-passe-mac"), r#"{"device_id": "mac-1"}"#).await.err().unwrap();
        assert_eq!((refused.status, refused.code, refused.device.as_deref()), (StatusCode::CONFLICT, "session", Some("Mac de Léa")));
        assert_eq!(refused.error, "Un appareil est connecté (Mac de Léa) : le redémarrage coupera sa session");
        assert!(!c.s.station.restarting());

        let ok = restart_from(&c.s, basic("mac", "mot-de-passe-mac"), r#"{"device_id": "mac-1", "force": true}"#).await.ok().unwrap();
        assert_eq!(ok.status(), StatusCode::ACCEPTED);
        assert!(c.s.station.restarting());
        assert_eq!(station_info(&c.s)["restarting"], true);
        assert_eq!(texts(&c.s, i18n::Lang::Fr)[0], "Mac de Léa a redémarré la Station");
        assert_eq!(texts(&c.s, i18n::Lang::En)[0], "Mac de Léa restarted the Station");

        // Déjà en cours : ni second redémarrage ni mise à jour.
        let again = restart_from(&c.s, basic("mac", "mot-de-passe-mac"), r#"{"force": true}"#).await.err().unwrap();
        assert_eq!((again.status, again.code), (StatusCode::CONFLICT, "restarting"));
        let update = stream_station_update(State(c.s.clone()), peer(), basic("mac", "mot-de-passe-mac"), Bytes::new()).await.err().unwrap();
        assert_eq!((update.status, update.code), (StatusCode::CONFLICT, "restarting"));
        let install = update_install(State(c.s.clone()), Bytes::new()).await.err().unwrap();
        assert_eq!(install.0, StatusCode::CONFLICT);
    }

    #[tokio::test]
    async fn one_restart_per_minute() {
        let c = console();
        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs();
        std::fs::create_dir_all(c.dir.join("station")).unwrap();
        std::fs::write(c.dir.join("station/dernier-redemarrage"), now.to_string()).unwrap();
        let refused = admin_station_restart(State(c.s.clone()), Bytes::new()).await.err().unwrap();
        assert_eq!((refused.status, refused.code), (StatusCode::TOO_MANY_REQUESTS, "too_soon"));
        let seconds = refused.retry_after.unwrap();
        assert!((55..=60).contains(&seconds), "{seconds}");
        let response = refused.into_response();
        assert_eq!(response.headers()[header::RETRY_AFTER], seconds.to_string().as_str());
        // Une minute plus tard : accepté, et noté comme venant d'Admin.
        std::fs::write(c.dir.join("station/dernier-redemarrage"), (now - 61).to_string()).unwrap();
        assert_eq!(admin_station_restart(State(c.s.clone()), Bytes::new()).await.ok().unwrap().status(), StatusCode::ACCEPTED);
        assert_eq!(texts(&c.s, i18n::Lang::Fr)[0], "Station redémarrée depuis Admin");
    }

    #[tokio::test]
    async fn a_declared_device_id_never_borrows_another_device_name() {
        let c = console_with_account();
        // L'Apple TV du salon s'est connectée avec un autre compte.
        c.s.activity.begin(Some(device("tv-1", "Apple TV du salon")), "salon", None, None);
        c.s.activity.end("salon", Some("tv-1"), None);
        let ok = restart_from(&c.s, basic("mac", "mot-de-passe-mac"), r#"{"device_id": "tv-1"}"#).await.ok().unwrap();
        assert_eq!(ok.status(), StatusCode::ACCEPTED);
        assert_eq!(texts(&c.s, i18n::Lang::Fr)[0], "Un appareil du compte mac a redémarré la Station");
        assert_eq!(texts(&c.s, i18n::Lang::En)[0], "A device on account mac restarted the Station");
    }

    #[tokio::test]
    async fn the_admin_session_also_works_on_the_device_route() {
        let c = console();
        let mut headers = HeaderMap::new();
        headers.insert(header::COOKIE, format!("{}={}", auth::COOKIE, c.s.auth.open_session()).parse().unwrap());
        assert_eq!(restart_from(&c.s, headers, "").await.ok().unwrap().status(), StatusCode::ACCEPTED);
        assert_eq!(texts(&c.s, i18n::Lang::Fr)[0], "Station redémarrée depuis Admin");
    }

    #[tokio::test]
    async fn wrong_accounts_are_locked_out_like_the_admin_login() {
        let c = console_with_account();
        let refused = restart_from(&c.s, basic("mac", "pas-le-bon"), "").await.err().unwrap();
        assert_eq!((refused.status, refused.code), (StatusCode::UNAUTHORIZED, "unauthorized"));
        // Même compteur que la connexion d'Admin, par adresse.
        for _ in 0..4 {
            c.s.auth.note_failure(peer().0.ip());
        }
        let locked = restart_from(&c.s, basic("mac", "mot-de-passe-mac"), "").await.err().unwrap();
        assert_eq!((locked.status, locked.code), (StatusCode::TOO_MANY_REQUESTS, "locked"));
        assert!(locked.retry_after.is_some_and(|s| s > 0));
        let update = stream_station_update(State(c.s.clone()), peer(), basic("mac", "mot-de-passe-mac"), Bytes::new()).await.err().unwrap();
        assert_eq!(update.code, "locked");
        assert!(!c.s.station.restarting());
        // Les autres routes des apps ne sont pas bloquées.
        assert!(stream_user(&c.s, &basic("mac", "mot-de-passe-mac")).is_ok());
    }

    #[tokio::test]
    async fn nothing_to_install_from_a_device() {
        let c = console_with_account();
        let refused = stream_station_update(State(c.s.clone()), peer(), basic("mac", "mot-de-passe-mac"), Bytes::from(r#"{"force": true}"#)).await.err().unwrap();
        assert_eq!((refused.status, refused.code), (StatusCode::CONFLICT, "up_to_date"));
        assert_eq!(refused.error, format!("KyberNow Station {} est à jour : rien à installer", crate::update::current()));
        let body = station_body(&Bytes::from("{pas du json")).err().unwrap();
        assert_eq!((body.status, body.code), (StatusCode::BAD_REQUEST, "bad_request"));
    }

    /// Un partage lancé depuis le PC, avec Léa et Max dedans.
    fn pc_share_with_guests(s: &AppState) {
        let display = crate::share::ShareDisplay { id: 1, index: 1, width: 1920, height: 1080, refresh: 60, name: "\\\\.\\DISPLAY1".into() };
        let code = s.share.open(crate::share::Origin::Local, display).unwrap().0.code;
        let ip: std::net::IpAddr = "192.168.1.30".parse().unwrap();
        s.share.join(&code, "Léa", "ipad", "ios", ip, false).unwrap();
        s.share.join(&code, "Max", "tv", "tvos", ip, false).unwrap();
    }

    #[tokio::test]
    async fn guests_hold_restarts_and_updates() {
        let c = console_with_account();
        pc_share_with_guests(&c.s);
        let refused = restart_from(&c.s, basic("mac", "mot-de-passe-mac"), "").await.err().unwrap();
        assert_eq!((refused.status, refused.code, refused.device.as_deref()), (StatusCode::CONFLICT, "session", Some("Léa et Max")));
        assert_eq!(refused.error, "Un partage est en cours avec Léa et Max : le redémarrage l'arrêtera");
        let install = update_install(State(c.s.clone()), Bytes::new()).await.err().unwrap();
        assert_eq!(install.0, StatusCode::CONFLICT);
        assert!(install.1.contains("Léa et Max"), "{}", install.1);
        // Les comptes en attente attendent aussi.
        assert!(!apply_accounts(&c.s, "apple-tv", "Apple TV", true).await);
        assert_eq!(c.restarts.load(Ordering::SeqCst), 0);
        let err = grant_pending_access(&c.s, "apple-tv", "tv-1", 1).await.unwrap_err();
        assert!(err.1.contains("Léa et Max"), "{}", err.1);
        // Forcé : le partage s'arrête d'abord (les invités l'apprennent).
        let ok = restart_from(&c.s, basic("mac", "mot-de-passe-mac"), r#"{"force": true}"#).await.ok().unwrap();
        assert_eq!(ok.status(), StatusCode::ACCEPTED);
        assert!(!c.s.share.is_open());
    }

    #[tokio::test]
    async fn ending_a_pc_share_never_restarts_kyber() {
        let c = console();
        pc_share_with_guests(&c.s);
        let Json(v) = end_session(State(c.s.clone())).await.ok().unwrap();
        assert_eq!(v["method"], "share");
        assert!(!c.s.share.is_open());
        assert_eq!(c.restarts.load(Ordering::SeqCst), 0);
        // Rien en cours : comme avant.
        let Json(v) = end_session(State(c.s.clone())).await.ok().unwrap();
        assert_eq!(v["method"], "none");
    }

    #[tokio::test]
    async fn the_host_heartbeat_carries_its_share_and_info_announces_it() {
        let c = console_with_account();
        let Json(info) = stream_info(State(c.s.clone()), basic("mac", "mot-de-passe-mac")).await.ok().unwrap();
        assert_eq!(info["share"], json!({ "available": true, "enabled": true }));
        c.s.activity.begin(Some(device("mac-1", "Mac")), "mac", None, None);
        let beat = || stream_heartbeat(State(c.s.clone()), basic("mac", "mot-de-passe-mac"), None, Bytes::from(r#"{"device_id":"mac-1"}"#));
        let Json(v) = beat().await.ok().unwrap();
        assert!(v.get("share").is_none(), "absent sans partage");
        let display = crate::share::ShareDisplay { id: 1, index: 1, width: 1920, height: 1080, refresh: 60, name: String::new() };
        let origin = crate::share::Origin::Host { device_id: "mac-1".into(), user: "mac".into(), name: "Mac".into() };
        c.s.share.open(origin, display).unwrap();
        let Json(v) = beat().await.ok().unwrap();
        assert_eq!((v["share"]["open"].as_bool(), v["share"]["origin"].as_str()), (Some(true), Some("host")));
        // Fermer la session de l'hôte arrête son partage.
        let Json(end) = stream_end(State(c.s.clone()), basic("mac", "mot-de-passe-mac"), Bytes::from(r#"{"device_id":"mac-1"}"#)).await.ok().unwrap();
        assert_eq!(end["ok"], true);
        assert!(!c.s.share.is_open());
    }

    #[tokio::test]
    async fn stopped_kyber_reads_accounts_at_start() {
        let c = console();
        c.s.service.stop().unwrap();
        c.restarts.store(0, Ordering::SeqCst);
        assert!(!apply_accounts(&c.s, "apple-tv", "Apple TV", true).await);
        for _ in 0..100 {
            if c.s.pending_access.is_empty() {
                assert_eq!(c.restarts.load(Ordering::SeqCst), 0);
                return;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        panic!("l'accès attend encore alors que Kyber est arrêté");
    }
}
