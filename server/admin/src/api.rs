//! Routes JSON de l'interface (`/api/*`).

use std::collections::BTreeMap;
use std::net::SocketAddr;
use std::sync::atomic::Ordering;
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
use crate::display::{Mode, Origin, Trigger, VdSettings};
use crate::activity::{DeviceInfo, ModeInfo};
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
        .route("/api/update", get(update_status))
        .route("/api/update/check", post(update_check))
        .route("/api/update/install", post(update_install))
        .route("/api/devices", get(devices))
        .route("/api/devices/{id}", axum::routing::delete(forget_device))
        .route("/api/events", get(events))
        .route("/api/session/end", post(end_session))
        .route("/api/preview", get(admin_preview))
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
    }))
}

#[derive(Deserialize)]
struct SetupBody {
    code: String,
    password: String,
}

async fn setup(State(s): State<AppState>, Json(body): Json<SetupBody>) -> ApiResult<Response> {
    let hash = s.auth.setup(&body.code, &body.password)?;
    s.update_settings(|st| st.admin_password_hash = Some(hash))?;
    tracing::info!("mot de passe administrateur défini");
    Ok(with_new_session(&s))
}

#[derive(Deserialize)]
struct LoginBody {
    password: String,
}

async fn login(State(s): State<AppState>, Json(body): Json<LoginBody>) -> ApiResult<Response> {
    // argon2 est volontairement coûteux : hors du fil d'exécution asynchrone.
    let auth = s.auth.clone();
    let ok = tokio::task::spawn_blocking(move || auth.verify(&body.password))
        .await
        .unwrap_or(false);
    if !ok {
        // Ralentit les essais répétés.
        tokio::time::sleep(Duration::from_millis(800)).await;
        return Err(ApiError(StatusCode::UNAUTHORIZED, "mot de passe incorrect".into()));
    }
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
    let hash = s.auth.change_password(&body.current, &body.new)?;
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

async fn update_status(State(s): State<AppState>) -> Json<Value> {
    Json(s.updater.status())
}

async fn update_check(State(s): State<AppState>) -> ApiResult<Json<Value>> {
    let u = s.updater.clone();
    Ok(Json(blocking(move || Ok(u.check())).await?))
}

/// Télécharge et lance l'installation : la console s'arrête dans la minute
/// et revient dans la nouvelle version.
async fn update_install(State(s): State<AppState>) -> ApiResult<Json<Value>> {
    let u = s.updater.clone();
    let version = blocking(move || u.install()).await?;
    tracing::info!("mise à jour {version} lancée");
    s.activity.event("info", format!("Installation de KyberNow {version} lancée"));
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

async fn users(State(s): State<AppState>) -> ApiResult<Json<Value>> {
    Ok(Json(json!({ "users": s.kyber.users()? })))
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
    Ok(Json(json!({ "users": s.kyber.users()?, "restart_pending": true })))
}

async fn delete_user(State(s): State<AppState>, Path(username): Path<String>) -> ApiResult<Json<Value>> {
    if s.kyber.users()?.len() <= 1 {
        return Err(ApiError(StatusCode::BAD_REQUEST, "il doit rester au moins un utilisateur".into()));
    }
    s.kyber.delete_user(&username)?;
    s.restart_pending.store(true, Ordering::Relaxed);
    s.activity.event("info", format!("Compte {username} supprimé"));
    Ok(Json(json!({ "users": s.kyber.users()?, "restart_pending": true })))
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
    blocking(move || d.start(Origin::Test, mode, None, Some(Duration::from_secs(seconds)))).await?;
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
    let value = headers.get(header::AUTHORIZATION).and_then(|v| v.to_str().ok()).ok_or_else(bad)?;
    let encoded = value.strip_prefix("Basic ").ok_or_else(bad)?;
    let decoded = base64_decode(encoded.trim()).ok_or_else(bad)?;
    let (user, pass) = decoded.split_once(':').ok_or_else(bad)?;
    if s.kyber.verify_login(user, pass) { Ok(user.to_string()) } else { Err(bad()) }
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
    let port = s
        .kyber
        .values()
        .ok()
        .and_then(|v| v.into_iter().find(|f| f.id == "network.port"))
        .and_then(|f| f.value.as_u64());
    // Le son d'abord : la sortie d'origine (souvent l'HDMI d'un écran)
    // disparaît quand les écrans physiques s'éteignent.
    let a = s.audio.clone();
    let channels = body.audio_channels;
    let audio = blocking(move || Ok(a.start(Origin::App, channels))).await?;
    // Veille du plein écran (mode jeu de la souris dans l'app).
    s.foreground.start(&s.data_dir);
    let mode_info = ModeInfo { width: body.width, height: body.height, refresh: body.refresh };
    s.activity.begin(body.device.clone(), &user, Some(mode_info), body.profile.clone());
    if trigger == Trigger::Off || body.virtual_display == Some(false) {
        return Ok(Json(json!({ "virtual_display": false, "audio": audio, "kyber_port": port, "heartbeat_seconds": 5 })));
    }
    let mode = Mode { width: body.width, height: body.height, refresh: body.refresh };
    let d = s.display.clone();
    let applied = blocking(move || d.start(Origin::App, mode, Some(user), None)).await?;
    Ok(Json(json!({ "virtual_display": true, "applied": applied, "audio": audio, "kyber_port": port, "heartbeat_seconds": 5 })))
}

#[derive(Deserialize, Default)]
struct HeartbeatBody {
    /// Latence médiane des dernières secondes, en millisecondes.
    #[serde(default)]
    latency_ms: Option<f64>,
    #[serde(default)]
    profile: Option<String>,
}

/// Signal de vie d'une app, avec sa latence. `end` : la console demande la fin.
async fn stream_heartbeat(State(s): State<AppState>, headers: HeaderMap, body: Bytes) -> ApiResult<Json<Value>> {
    let user = stream_user(&s, &headers)?;
    // Corps facultatif : les apps d'avant 0.2.1 n'en envoient pas.
    let body: HeartbeatBody = serde_json::from_slice(&body).unwrap_or_default();
    s.audio.heartbeat();
    s.foreground.touch();
    let end = s.activity.heartbeat(&user, body.latency_ms, body.profile);
    Ok(Json(json!({ "active": s.display.heartbeat(), "end": end })))
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
            Ok(gpu::encoder_sessions().unwrap_or(0) > 0
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
}

async fn stream_end(State(s): State<AppState>, headers: HeaderMap, body: Bytes) -> ApiResult<Json<Value>> {
    let user = stream_user(&s, &headers)?;
    let body: EndBody = serde_json::from_slice(&body).unwrap_or_default();
    s.activity.end(&user, body.latency_ms);
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

async fn forget_device(State(s): State<AppState>, Path(id): Path<String>) -> ApiResult<Json<Value>> {
    if !s.activity.forget(&id) {
        return Err(ApiError(StatusCode::NOT_FOUND, "appareil inconnu".into()));
    }
    Ok(Json(json!({ "devices": s.activity.devices() })))
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
            Ok(gpu::encoder_sessions().unwrap_or(0) > 0
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
