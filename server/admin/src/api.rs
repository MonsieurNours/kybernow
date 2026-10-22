//! Routes JSON de l'interface (`/api/*`).

use std::collections::{BTreeMap, HashMap};
use std::net::SocketAddr;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use axum::body::Bytes;
use axum::extract::{ConnectInfo, Path, Query, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post, put};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::audio::AudioSettings;
use crate::display::{Mode, Origin, PolicyChoice, Trigger, VdSettings};
use crate::access::PendingAccess;
use crate::activity::{self, DeviceInfo, ModeInfo};
use crate::i18n::{self, Msg};
use crate::launch::Launch;
use crate::pairing::{self, RequestError};
use crate::{AppState, auth, gpu, logs, preview, schema, service};

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
        // Client web (/ecran) : jeton Kyber pour la session de la console.
        .route("/api/web/ticket", get(web_ticket))
        // Appels des appareils (apps KyberNow), authentifiés par un compte Kyber.
        .route("/api/pairing", get(pairing))
        .route("/api/stream/info", get(stream_info))
        .route("/api/stream/prepare", post(stream_prepare))
        .route("/api/stream/heartbeat", post(stream_heartbeat))
        .route("/api/stream/foreground", get(stream_foreground))
        .route("/api/stream/preview", get(stream_preview))
        .route("/api/stream/end", post(stream_end))
        .layer(axum::middleware::from_fn_with_state(state, auth::require_session))
        // La langue des réponses (`Accept-Language`, français sans en-tête),
        // posée avant tout le reste : les refus de `require_session` aussi.
        .layer(axum::middleware::from_fn(i18n::scope))
}

pub struct ApiError(StatusCode, String);

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

async fn session(State(s): State<AppState>, headers: HeaderMap) -> Json<Value> {
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
    match crate::update::session_conflict(&s.activity.summary()["live"], body.force) {
        Some(message) => Err(ApiError::msg(StatusCode::CONFLICT, message)),
        None => Ok(()),
    }
}

/// Télécharge et lance l'installation : la console s'arrête dans la minute
/// et revient dans la nouvelle version. 409 pendant une session, sauf
/// `{"force": true}`.
async fn update_install(State(s): State<AppState>, body: Bytes) -> ApiResult<Json<Value>> {
    session_guard(&s, &body)?;
    let u = s.updater.clone();
    let version = blocking(move || u.install()).await?;
    tracing::info!("mise à jour {version} lancée");
    s.activity.event("info", Msg::new("events.installing").with("version", version.as_str()));
    Ok(Json(json!({ "ok": true, "version": version })))
}

/// Remet la version précédente (dossiers `.prev`) : la console s'arrête dans
/// la minute et revient dans cette version. 409 pendant une session, comme
/// l'installation.
async fn update_rollback(State(s): State<AppState>, body: Bytes) -> ApiResult<Json<Value>> {
    session_guard(&s, &body)?;
    let u = s.updater.clone();
    let version = blocking(move || u.rollback()).await?;
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
async fn blocking<T: Send + 'static>(f: impl FnOnce() -> anyhow::Result<T> + Send + 'static) -> ApiResult<T> {
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
fn stream_user(s: &AppState, headers: &HeaderMap) -> ApiResult<String> {
    let bad = || ApiError::msg(StatusCode::UNAUTHORIZED, Msg::new("api.kyberCredentials"));
    // Le client web (/ecran) passe par la session de la console.
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
}

/// Lien d'appairage pour les apps (aussi affiché en QR code par la console) :
/// adresse, port et empreinte du certificat, pour une première connexion
/// vérifiée sans question.
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

/// Ce que l'app affiche d'un ordinateur : nom, système, carte graphique, possibilités.
async fn stream_info(State(s): State<AppState>, headers: HeaderMap) -> ApiResult<Json<Value>> {
    stream_user(&s, &headers)?;
    let gpu = tokio::task::spawn_blocking(gpu::status).await.ok().flatten();
    let gpu_name = gpu.as_ref().and_then(|g| g.devices.first()).map(|d| d.name.clone());
    // AV1 en NVENC : à partir des cartes Ada (RTX 40) et suivantes.
    let av1 = gpu_name.as_deref().is_some_and(|n| ["RTX 40", "RTX 50", "RTX 2000 Ada", "RTX 4000 Ada", "RTX 5000 Ada", "RTX 6000 Ada", "L4", "L40"].iter().any(|m| n.contains(m)));
    let (d, a) = (s.display.clone(), s.audio.clone());
    let (display, audio) = blocking(move || Ok((d.status(), a.status()))).await?;
    Ok(Json(json!({
        "hostname": hostname::get().ok().map(|h| h.to_string_lossy().into_owned()),
        "os": crate::system::os_name(),
        "gpu": gpu_name,
        "av1": av1,
        "virtual_display": {
            "available": display["driver"]["available"].as_bool().unwrap_or(false),
            "trigger": display["settings"]["trigger"],
            "active": !display["session"].is_null(),
        },
        "audio": {
            "speakers": !audio["devices"]["speakers"].is_null(),
            "trigger": audio["settings"]["trigger"],
        },
        "admin_version": env!("CARGO_PKG_VERSION"),
    })))
}

/// Une app annonce son écran avant de se connecter à Kyber.
async fn stream_prepare(
    State(s): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<PrepareBody>,
) -> ApiResult<Json<Value>> {
    let user = stream_user(&s, &headers)?;
    let _preparing = Preparing::new(&s.preparing);
    let trigger = s.display.settings.lock().unwrap().trigger;
    let port = s.kyber.port();
    let (device_id, device_name) = activity::identity(body.device.as_ref(), &user);

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

    // Le son d'abord : la sortie d'origine (souvent l'HDMI d'un écran)
    // disparaît quand les écrans physiques s'éteignent.
    let a = s.audio.clone();
    let channels = body.audio_channels;
    let audio = blocking(move || Ok(a.start(Origin::App, channels))).await?;
    // Veille du plein écran (mode jeu de la souris dans l'app).
    s.foreground.start(&s.data_dir);
    let mode_info = ModeInfo { width: body.width, height: body.height, refresh: body.refresh };
    let launch_name = launch.as_ref().map(|l| l.name.clone());
    if trigger == Trigger::Off || body.virtual_display == Some(false) {
        s.activity.begin(body.device.clone(), &user, Some(mode_info), body.profile.clone());
        if let Some(l) = launch {
            s.launcher.session_started(device_id, device_name, l, body.profile.clone());
        }
        return Ok(Json(json!({
            "virtual_display": false, "audio": audio, "kyber_port": port, "heartbeat_seconds": 5,
            "kyber_fingerprint": crate::kyber_proxy::kyber_fingerprint(&s),
            "launch": launch_name,
        })));
    }
    let mode = Mode { width: body.width, height: body.height, refresh: body.refresh };
    let d = s.display.clone();
    let (u, id, policy) = (user.clone(), device_id.clone(), body.mode_policy);
    let applied = blocking(move || d.start(Origin::App, mode, Some(u), Some(id), None, policy)).await?;
    // La session n'est ouverte qu'une fois l'écran en place : un échec ne
    // laisse pas un appareil « connecté » dans la vue d'ensemble.
    s.activity.begin(body.device.clone(), &user, Some(mode_info), body.profile.clone());
    // L'app de l'appareil s'ouvre sur l'écran virtuel, en fond (elle attend
    // le déverrouillage de Windows s'il le faut).
    if let Some(l) = launch {
        s.launcher.session_started(device_id, device_name, l, body.profile.clone());
    }
    Ok(Json(json!({
        "virtual_display": true, "applied": applied, "audio": audio, "kyber_port": port, "heartbeat_seconds": 5,
        "kyber_fingerprint": crate::kyber_proxy::kyber_fingerprint(&s),
        "launch": launch_name,
    })))
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
async fn stream_heartbeat(State(s): State<AppState>, headers: HeaderMap, body: Bytes) -> ApiResult<Json<Value>> {
    let user = stream_user(&s, &headers)?;
    // Corps facultatif : les apps d'avant 0.2.1 n'en envoient pas.
    let body: HeartbeatBody = serde_json::from_slice(&body).unwrap_or_default();
    let device = body.device_id.as_deref();
    s.audio.heartbeat();
    s.foreground.touch();
    let end = s.activity.heartbeat(&user, device, body.latency_ms, body.profile);
    let active = s.display.heartbeat(device, body.mode_policy);
    let observed = if active { s.display.observed() } else { None };
    Ok(Json(json!({
        "active": active,
        "end": end,
        "mode": observed.as_ref().map(|o| o.mode).filter(|m| m.width > 0),
        "displays": observed.as_ref().map(|o| o.displays),
        "mode_policy": if active { s.display.policy() } else { None },
    })))
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
    let (d, a, l) = (s.display.clone(), s.audio.clone(), s.launcher.clone());
    let (ended_id, _) = activity::identity(None, &user);
    let ended_id = body.device_id.clone().unwrap_or(ended_id);
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
/// couperait cette connexion.
async fn kyber_busy(s: &AppState, except: Option<&str>) -> bool {
    if s.activity.live().is_some_and(|l| Some(l.device_id.as_str()) != except) {
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
/// personne d'autre que `except`. Vrai si Kyber connaît désormais tous les
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
    let other_live = || s.activity.live().filter(|l| l.device_id != device_id);
    // Ne jamais couper la session d'un autre appareil (ni la lui prendre :
    // Kyber refuserait de toute façon ce compte tant qu'il n'a pas redémarré).
    if let Some(live) = other_live() {
        return Err(busy(Some(live.name)));
    }
    let deadline = tokio::time::Instant::now() + ACCESS_FREE_WAIT;
    while s.pending_access.waits_for(user) {
        if apply_pending_access(s, Some(device_id), Some("events.contextConnecting")).await {
            break;
        }
        if let Some(live) = other_live() {
            return Err(busy(Some(live.name)));
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
    if s.activity.request_end() {
        return Ok(Json(json!({ "ok": true, "method": "app" })));
    }
    let (d, a) = (s.display.clone(), s.audio.clone());
    let active = blocking(move || Ok(!d.status()["session"].is_null() || !a.status()["session"].is_null())).await?;
    if !active {
        return Ok(Json(json!({ "ok": true, "method": "none" })));
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
