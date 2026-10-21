//! Routes JSON de l'interface (`/api/*`).

use std::collections::{BTreeMap, HashMap};
use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, Ordering};
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
use crate::activity::{self, DeviceInfo, ModeInfo};
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
}

pub struct ApiError(StatusCode, String);

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (self.0, Json(json!({ "error": self.1 }))).into_response()
    }
}

impl From<anyhow::Error> for ApiError {
    fn from(e: anyhow::Error) -> Self {
        // Message lisible pour l'interface, chaîne complète dans le journal.
        tracing::warn!("{e:#}");
        ApiError(StatusCode::BAD_REQUEST, format!("{e:#}"))
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
        Some(wait) => Err(ApiError(
            StatusCode::TOO_MANY_REQUESTS,
            format!("trop d'essais : réessayez dans {} s", wait.as_secs().max(1)),
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
        return Err(ApiError(StatusCode::UNAUTHORIZED, "mot de passe incorrect".into()));
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
        "gpu": gpu,
        "activity": s.activity.summary(),
        "update_available": s.updater.available(),
        "pair_requests": s.pairing.pending(),
    })))
}

/// État résumé pour l'icône de la zone de notification, réservé à l'ordinateur
/// lui-même : serveur en marche ou non, session d'un appareil en cours ou non.
async fn local_state(State(s): State<AppState>, ConnectInfo(peer): ConnectInfo<SocketAddr>) -> ApiResult<Json<Value>> {
    if !peer.ip().is_loopback() {
        return Err(ApiError(StatusCode::FORBIDDEN, "réservé à l'ordinateur local".into()));
    }
    let service_state = s.service.state().unwrap_or(service::ServiceState::Unknown);
    let (d, a) = (s.display.clone(), s.audio.clone());
    let (display, audio) = blocking(move || Ok((d.status()["session"].clone(), a.status()["session"].clone()))).await?;
    let user = display["user"].as_str().map(str::to_owned);
    Ok(Json(json!({
        "service": service_state,
        "session": !display.is_null() || !audio.is_null(),
        "user": user,
        "hostname": hostname::get().ok().map(|h| h.to_string_lossy().into_owned()),
        "address": crate::tls::lan_ip(),
        "console_port": s.listen_port,
        "activity": s.activity.summary(),
        "restart_pending": s.restart_pending.load(Ordering::Relaxed),
        "update_available": s.updater.available(),
        "version": crate::update::current(),
        "pair_requests": s.pairing.pending(),
    })))
}

/// Fin de la session en cours, depuis l'icône de la zone de notification
/// (réservé à l'ordinateur lui-même : la personne devant l'écran).
async fn local_end(State(s): State<AppState>, ConnectInfo(peer): ConnectInfo<SocketAddr>) -> ApiResult<Json<Value>> {
    if !peer.ip().is_loopback() {
        return Err(ApiError(StatusCode::FORBIDDEN, "réservé à l'ordinateur local".into()));
    }
    end_session(State(s)).await
}

/// Réponse à une demande d'appairage depuis le panneau de l'icône (réservé à
/// l'ordinateur lui-même : la personne devant l'écran).
async fn local_pair_answer(
    State(s): State<AppState>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    Path((id, action)): Path<(String, String)>,
) -> ApiResult<Json<Value>> {
    if !peer.ip().is_loopback() {
        return Err(ApiError(StatusCode::FORBIDDEN, "réservé à l'ordinateur local".into()));
    }
    match action.as_str() {
        "accept" => pair_accept(State(s), Path(id)).await,
        "refuse" => pair_refuse(State(s), Path(id)).await,
        _ => Err(ApiError(StatusCode::NOT_FOUND, format!("action inconnue : {action}"))),
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
    s.activity.event("info", format!("Mises à jour : canal {}", body.channel.as_str()));
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
        serde_json::from_slice(body).map_err(|e| ApiError(StatusCode::BAD_REQUEST, format!("corps illisible : {e}")))?
    };
    match crate::update::session_conflict(&s.activity.summary()["live"], body.force) {
        Some(message) => Err(ApiError(StatusCode::CONFLICT, message)),
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
    s.activity.event("info", format!("Installation de KyberNow {version} lancée"));
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
    s.activity.event("info", format!("Retour à KyberNow {version} lancé"));
    Ok(Json(json!({ "ok": true, "version": version })))
}

async fn schema_route() -> Json<Value> {
    Json(json!({ "groups": schema::GROUPS, "fields": schema::fields() }))
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
    s.activity.event("info", if existed { format!("Mot de passe du compte {username} changé") } else { format!("Compte {username} ajouté") });
    Ok(Json(json!({ "users": users_json(&s)?, "restart_pending": true })))
}

async fn delete_user(State(s): State<AppState>, Path(username): Path<String>) -> ApiResult<Json<Value>> {
    if s.kyber.users()?.len() <= 1 {
        return Err(ApiError(StatusCode::BAD_REQUEST, "il doit rester au moins un utilisateur".into()));
    }
    s.kyber.delete_user(&username)?;
    s.restart_pending.store(true, Ordering::Relaxed);
    s.activity.event("info", format!("Compte {username} supprimé"));
    Ok(Json(json!({ "users": users_json(&s)?, "restart_pending": true })))
}

async fn service_action(State(s): State<AppState>, Path(action): Path<String>) -> ApiResult<Json<Value>> {
    match action.as_str() {
        "start" => s.service.start()?,
        "stop" => s.service.stop()?,
        "restart" => service::restart(s.service.as_ref()).await?,
        _ => return Err(ApiError(StatusCode::NOT_FOUND, format!("action inconnue : {action}"))),
    }
    if action != "stop" {
        s.restart_pending.store(false, Ordering::Relaxed);
    }
    tracing::info!("service kyber : {action}");
    s.activity.event("info", match action.as_str() {
        "start" => "Serveur démarré depuis la console",
        "stop" => "Serveur arrêté depuis la console",
        _ => "Serveur redémarré depuis la console",
    });
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

async fn blocking<T: Send + 'static>(f: impl FnOnce() -> anyhow::Result<T> + Send + 'static) -> ApiResult<T> {
    tokio::task::spawn_blocking(f)
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
    let bad = || ApiError(StatusCode::UNAUTHORIZED, "identifiants Kyber requis".into());
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
/// l'installation (celui de la console).
fn web_user(s: &AppState) -> String {
    s.kyber
        .users()
        .ok()
        .and_then(|users| users.into_iter().next())
        .map(|u| u.username)
        .unwrap_or_else(|| "KyberNow".into())
}

/// Ce dont le client web a besoin pour se connecter à Kyber : un jeton signé
/// avec la clé que Kyber connaît (posée à l'installation), valable le temps
/// d'une session de la console.
async fn web_ticket(State(s): State<AppState>) -> ApiResult<Json<Value>> {
    const LIFETIME: i64 = 12 * 3600;
    let secret = s.kyber.jwt_secret().ok_or_else(|| {
        ApiError(
            StatusCode::CONFLICT,
            "Kyber n'a pas de clé pour les jetons de la console. Réinstallez KyberNow Server pour la créer.".into(),
        )
    })?;
    let user = web_user(&s);
    Ok(Json(json!({
        "token": crate::kyber_proxy::jwt(&secret, &user, LIFETIME),
        "user": user,
        "expires_in": LIFETIME,
        "kyber_port": s.kyber.port(),
        "hostname": hostname::get().ok().map(|h| h.to_string_lossy().into_owned()),
    })))
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
    let trigger = s.display.settings.lock().unwrap().trigger;
    let port = s.kyber.port();
    let (device_id, device_name) = activity::identity(body.device.as_ref(), &user);

    // Reprise de main : un autre appareil est en session. La console lui
    // demande de finir (il l'apprend à son signal de vie suivant, 5 s au plus)
    // et attend qu'il ait rendu la main avant de préparer l'écran du nouveau.
    if let activity::Claim::Waiting { previous } = s.activity.claim(&device_id, &device_name) {
        tracing::info!("{device_name} prend la main : on attend que {previous} se déconnecte");
        let deadline = tokio::time::Instant::now() + TAKEOVER_WAIT;
        loop {
            match s.activity.live_device_id() {
                Some(id) if id != device_id => {}
                _ => break,
            }
            if tokio::time::Instant::now() >= deadline {
                // Il ne répond plus : sa session est close, ses écrans et son
                // son seront repris par la préparation qui suit.
                s.activity.drop_live(&format!("n'a pas rendu la main à {device_name}"));
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
    if trigger == Trigger::Off || body.virtual_display == Some(false) {
        s.activity.begin(body.device.clone(), &user, Some(mode_info), body.profile.clone());
        return Ok(Json(json!({ "virtual_display": false, "audio": audio, "kyber_port": port, "heartbeat_seconds": 5 })));
    }
    let mode = Mode { width: body.width, height: body.height, refresh: body.refresh };
    let d = s.display.clone();
    let (u, id, policy) = (user.clone(), device_id.clone(), body.mode_policy);
    let applied = blocking(move || d.start(Origin::App, mode, Some(u), Some(id), None, policy)).await?;
    // La session n'est ouverte qu'une fois l'écran en place : un échec ne
    // laisse pas un appareil « connecté » dans la vue d'ensemble.
    s.activity.begin(body.device.clone(), &user, Some(mode_info), body.profile.clone());
    Ok(Json(json!({ "virtual_display": true, "applied": applied, "audio": audio, "kyber_port": port, "heartbeat_seconds": 5 })))
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
    let (d, a) = (s.display.clone(), s.audio.clone());
    let report = blocking(move || {
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

/// Oublie un appareil. Appairé par code, il perd aussi son accès : son compte
/// propre est supprimé (et sa session, s'il en a une, prend fin).
async fn forget_device(State(s): State<AppState>, Path(id): Path<String>) -> ApiResult<Json<Value>> {
    let live = s.activity.live().is_some_and(|l| l.device_id == id);
    let device = s.activity.forget(&id).ok_or_else(|| ApiError(StatusCode::NOT_FOUND, "appareil inconnu".into()))?;
    let mut revoked = false;
    if device.own_account && s.kyber.users()?.iter().any(|u| u.username == device.user) {
        s.kyber.delete_user(&device.user)?;
        if live {
            s.activity.request_end();
        }
        s.activity.event("info", format!("Accès de {} retiré (compte {} supprimé)", device.name, device.user));
        apply_accounts(&s, &format!("retirer l'accès de {}", device.name)).await;
        revoked = true;
    }
    Ok(Json(json!({
        "devices": s.activity.devices(),
        "revoked": revoked,
        "restart_pending": s.restart_pending.load(Ordering::Relaxed),
    })))
}

/// Un compte ajouté ou retiré ne compte pour Kyber qu'à son redémarrage :
/// tout de suite (en arrière-plan) si personne n'est connecté, sinon plus tard
/// (bandeau de la console). Vrai si Kyber redémarre.
async fn apply_accounts(s: &AppState, why: &str) -> bool {
    static RESTARTING: AtomicBool = AtomicBool::new(false);
    if s.service.state().ok() != Some(service::ServiceState::Running) {
        // Arrêté (ou déjà en train de redémarrer) : il lira les comptes à son démarrage.
        return RESTARTING.load(Ordering::Relaxed);
    }
    let (d, a) = (s.display.clone(), s.audio.clone());
    let busy = s.activity.live().is_some()
        || blocking(move || Ok(gpu::encoding() || !d.status()["session"].is_null() || !a.status()["session"].is_null()))
            .await
            .unwrap_or(true);
    if busy {
        s.restart_pending.store(true, Ordering::Relaxed);
        return false;
    }
    if RESTARTING.swap(true, Ordering::SeqCst) {
        return true;
    }
    let (s, why) = (s.clone(), why.to_string());
    tokio::spawn(async move {
        match service::restart(s.service.as_ref()).await {
            Ok(()) => {
                s.restart_pending.store(false, Ordering::Relaxed);
                s.activity.event("info", format!("Serveur redémarré pour {why}"));
            }
            Err(e) => {
                tracing::warn!("redémarrage de Kyber pour {why} : {e:#}");
                s.restart_pending.store(true, Ordering::Relaxed);
            }
        }
        RESTARTING.store(false, Ordering::SeqCst);
    });
    true
}

/// Supprime les comptes créés pour des demandes acceptées dont l'appareil
/// n'est jamais venu chercher l'accès.
fn delete_orphans(s: &AppState) {
    for user in s.pairing.take_orphans() {
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
        Err(RequestError::Invalid(message)) => Err(ApiError(StatusCode::BAD_REQUEST, message.into())),
        Err(RequestError::TooMany) => {
            tracing::warn!("demandes d'appairage refusées depuis {} : trop de demandes", peer.ip());
            Err(ApiError(StatusCode::TOO_MANY_REQUESTS, "trop de demandes d'appairage : réessayez dans une minute".into()))
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
    ApiError(StatusCode::GONE, "cette demande a expiré : l'appareil en enverra une nouvelle avec un autre code".into())
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
    let restarted = apply_accounts(&s, &format!("donner l'accès à {}", device.name)).await;
    Ok(Json(json!({
        "ok": true,
        "username": username,
        "restarted": restarted,
        "restart_pending": s.restart_pending.load(Ordering::Relaxed),
        "pair_requests": s.pairing.pending(),
        "devices": s.activity.devices(),
    })))
}

async fn pair_refuse(State(s): State<AppState>, Path(id): Path<String>) -> ApiResult<Json<Value>> {
    let name = s.pairing.refuse(&id).ok_or_else(request_gone)?;
    s.activity.event("info", format!("Accès refusé à {name} (appairage par code)"));
    Ok(Json(json!({ "ok": true, "pair_requests": s.pairing.pending() })))
}

#[derive(Deserialize)]
struct EventsQuery {
    limit: Option<usize>,
}

async fn events(State(s): State<AppState>, Query(q): Query<EventsQuery>) -> Json<Value> {
    Json(json!({ "events": s.activity.events(q.limit.unwrap_or(50).min(300)) }))
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
    service::restart(s.service.as_ref()).await?;
    s.activity.event("info", "Connexions coupées depuis la console (serveur redémarré)");
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
