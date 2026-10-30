//! Routes du prêt (docs/pret.md, « Échange avec Admin ») et sa mise en
//! œuvre : préparation (session de l'hôte fermée, partage arrêté, écran
//! virtuel à la taille de l'ami, agent de console, surveillance des
//! processus), signes de vie, préavis, fin (remise en état, verrouillage).
//!
//! - l'ami (sans compte) : `POST /api/lend/status|borrow|wait|beat|leave` ;
//! - l'hôte : `GET /api/lend`, `POST|DELETE /api/lend/code`, `POST
//!   /api/lend/requests/{id}`, `POST /api/lend/take-back` (un appareil
//!   appairé, ou la session d'Admin), et les mêmes sous `/api/local/lend…`
//!   (l'icône, ou la session d'Admin) ;
//! - les réglages (session d'Admin) : `GET|PUT /api/lend/settings`,
//!   `/api/lend/friends…` ;
//! - l'agent de console (`kybernow-tray.exe --console`, jeton local) :
//!   `/api/local/lend/console…`.

use std::net::SocketAddr;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use axum::Json;
use axum::body::Bytes;
use axum::extract::{ConnectInfo, Path, State};
use axum::http::{HeaderMap, HeaderValue, Method, StatusCode, header};
use axum::response::{IntoResponse, Response};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::AppState;
use crate::api::{ApiError, blocking, stream_user};
use crate::display::{Mode, Origin, now_secs};
use crate::i18n::Msg;
use crate::lend::{self, Beat, Kind, LocalTime, Reason, Screen, Steam, Wait, Who};
use crate::lend_last;
use crate::lend_sys::{self, SessionState};
use crate::relay::Via;

/// En-tête du secret de l'ami (et de sa demande).
const LEND_HEADER: &str = "x-kybernow-lend";
/// Une attente de `wait` tenue au plus ce temps, puis `202`.
const WAIT_HOLD: Duration = Duration::from_secs(30);
/// Le mode console a ce temps pour se mettre en place (basculer Steam sur
/// le compte de l'ami prend jusqu'à 25 s)...
const READY_WAIT: Duration = Duration::from_secs(35);
/// ... sans que toute la préparation passe ce temps : le cœur attend la
/// réponse de `borrow` 45 s.
const PREPARE_WAIT: Duration = Duration::from_secs(40);
/// La fin vue par l'ami : le lanceur la dit (« Vous avez rendu l'ordinateur. »,
/// puis « À bientôt, {prénom} ») avant que son image s'arrête ; ses entrées
/// sont déjà coupées.
#[cfg(not(test))]
const ENDING_SHOW: Duration = Duration::from_millis(5000);
#[cfg(test)]
const ENDING_SHOW: Duration = Duration::ZERO;
/// L'agent de console a ce temps pour se remettre en état à la fin (fenêtres
/// 10 s, Steam jusqu'à 25 s).
const ENDED_WAIT: Duration = Duration::from_secs(45);
/// Une entrée physique plus récente que ça : quelqu'un est devant l'ordinateur.
const PRESENT: u64 = 5 * 60;
/// Un invité coupé de Kyber ne revient pas avec un vieux jeton pendant ce temps.
const KYBER_BAN_SECONDS: u64 = lend::TOKEN_LIFETIME as u64 + 10;

// --- Refus ----------------------------------------------------------------------------

/// Refus des routes de l'hôte et des réglages : `{"error": <texte>, "code"}`.
pub struct LendError {
    status: StatusCode,
    code: &'static str,
    error: String,
}

impl LendError {
    fn new(status: StatusCode, code: &'static str, msg: Msg) -> Self {
        LendError { status, code, error: msg.text() }
    }
}

impl IntoResponse for LendError {
    fn into_response(self) -> Response {
        (self.status, Json(json!({ "error": self.error, "code": self.code }))).into_response()
    }
}

impl From<ApiError> for LendError {
    fn from(e: ApiError) -> Self {
        let code = match e.0 {
            StatusCode::UNAUTHORIZED => "unauthorized",
            StatusCode::FORBIDDEN => "forbidden",
            StatusCode::BAD_REQUEST => "bad_request",
            _ => "failed",
        };
        LendError { status: e.0, code, error: e.1 }
    }
}

impl From<anyhow::Error> for LendError {
    fn from(e: anyhow::Error) -> Self {
        ApiError::from(e).into()
    }
}

type LendResult<T> = Result<T, LendError>;

/// Refus de l'ami, comme le contrat les nomme : `{"error": "<code>"}`.
fn friend_error(status: StatusCode, code: &str) -> Response {
    (status, [(header::CACHE_CONTROL, "no-store")], Json(json!({ "error": code }))).into_response()
}

fn too_many(wait: u64) -> Response {
    let mut res = (StatusCode::TOO_MANY_REQUESTS, Json(json!({ "error": "too_many", "retry_after": wait }))).into_response();
    res.headers_mut().insert(header::RETRY_AFTER, HeaderValue::from(wait));
    res
}

// --- Réglages et état -------------------------------------------------------------------

pub fn settings(s: &AppState) -> lend::LendSettings {
    s.settings.lock().unwrap().lend.clone()
}

pub fn enabled(s: &AppState) -> bool {
    s.settings.lock().unwrap().lend.enabled
}

fn hostname() -> String {
    hostname::get().ok().map(|h| h.to_string_lossy().into_owned()).unwrap_or_default()
}

/// Le prénom de l'hôte (compte de la session, relu toutes les 10 minutes),
/// sinon le nom de l'ordinateur.
fn host_name() -> String {
    static CACHE: Mutex<Option<(String, Instant)>> = Mutex::new(None);
    if let Some((name, at)) = CACHE.lock().unwrap().clone()
        && at.elapsed() < Duration::from_secs(600)
    {
        return name;
    }
    let name = lend_sys::host_name().unwrap_or_else(hostname);
    *CACHE.lock().unwrap() = Some((name.clone(), Instant::now()));
    name
}

/// `info.lend`.
pub fn info(s: &AppState) -> Value {
    json!({ "available": true, "enabled": enabled(s) })
}

/// La vue du prêt (contrat), avec le nom de l'ordinateur et de l'hôte.
pub fn view(s: &AppState) -> Value {
    let mut v = s.lend.view(enabled(s));
    v["computer"] = json!(hostname());
    v["host"] = json!(host_name());
    v["code"] = match s.lend.code() {
        Some(c) => {
            let links = crate::share_api::links(s, &c.code);
            json!({
                "code": c.code,
                "link": links.link,
                "web": links.web,
                "web_local": links.web_local,
                "relay": links.relay,
                "duration": c.duration,
                "steam": c.steam,
                "expires": c.expires,
                "from_session": c.from_session,
            })
        }
        None => Value::Null,
    };
    v
}

/// La vue, pour le signe de vie de l'hôte : seulement s'il y a quelque chose à dire.
pub fn for_host(s: &AppState) -> Option<Value> {
    let busy = s.lend.code().is_some() || s.lend.loan().is_some() || !s.lend.requests().is_empty();
    busy.then(|| view(s))
}

// --- Kyber ------------------------------------------------------------------------------

fn kyber_token(s: &AppState, loan: &lend::Loan) -> Option<String> {
    let secret = s.kyber.jwt_secret()?;
    let display = loan.display.map(|(id, _)| id)?;
    let claims = json!({ "kn_role": "player_kbm", "kn_display": display, "kn_rev": 1 });
    Some(crate::kyber_proxy::jwt_with(&secret, &loan.kyber_user(), lend::TOKEN_LIFETIME, claims))
}

/// Plus d'entrées pour l'ami (rôle spectateur, révision 2 : un jeton renouvelé
/// ne les rend pas), l'image continue.
async fn kyber_spectator(s: &AppState, loan: &lend::Loan) {
    let body = json!({ "user": loan.kyber_user(), "role": "spectator", "rev": 2 });
    if let Err(e) = crate::kyber_proxy::kyber_call(s, Method::POST, "/kybernow/role", Some(body)).await {
        tracing::warn!("prêt : entrées de {} non coupées : {e:#}", loan.who.name);
    }
}

async fn kyber_close(s: &AppState, loan: &lend::Loan) {
    let body = json!({ "user": loan.kyber_user(), "ban_seconds": KYBER_BAN_SECONDS });
    match crate::kyber_proxy::kyber_call(s, Method::POST, "/kybernow/close", Some(body)).await {
        Ok((status, _)) if status.is_success() => tracing::info!("prêt : {} coupé de Kyber", loan.who.name),
        Ok((status, _)) => tracing::warn!("prêt : Kyber refuse de couper {} ({status})", loan.who.name),
        Err(e) => tracing::warn!("prêt : {} non coupé de Kyber : {e:#}", loan.who.name),
    }
}

/// La réponse `200` de `borrow` et `wait` : jeton de Kyber, écran, fin ;
/// le cookie d'un navigateur ; par le relais, `"relay": true` et ce client
/// autorisé à joindre Kyber.
fn loan_answer(s: &AppState, loan: &lend::Loan, via: Option<&Via>) -> Response {
    let Some(token) = kyber_token(s, loan) else {
        tracing::warn!("prêt : pas de clé pour les jetons de Kyber, ou pas d'écran virtuel");
        return friend_error(StatusCode::SERVICE_UNAVAILABLE, "off");
    };
    if let Some(via) = via {
        s.relay.grant_guest(via, &loan.id);
    }
    let (id, screen) = loan.display.unwrap_or((0, loan.who.screen));
    let mut answer = json!({
        "lend": loan.id,
        "secret": loan.secret(),
        "host": host_name(),
        "computer": hostname(),
        "kyber": {
            "port": s.kyber.port(),
            "fingerprint": crate::kyber_proxy::kyber_fingerprint(s),
            "token": token,
            "expires": lend::TOKEN_LIFETIME,
        },
        "display": { "id": id, "width": screen.width, "height": screen.height, "refresh": screen.refresh },
        "until": loan.until,
        "steam": loan.steam,
        "beat": lend::BEAT_SECONDS,
    });
    if via.is_some() {
        answer["relay"] = json!(true);
    }
    let mut res = ([(header::CACHE_CONTROL, "no-store")], Json(answer)).into_response();
    if let Some(cookie) = loan.cookie()
        && let Ok(value) = HeaderValue::from_str(&lend::set_cookie(cookie, s.secure_cookies, via.is_some()))
    {
        res.headers_mut().insert(header::SET_COOKIE, value);
    }
    res
}

// --- Préparer, finir ------------------------------------------------------------------------

/// La surveillance des processus du prêt en cours, et ses règles.
static WATCH: Mutex<Option<(lend_sys::Watch, lend_sys::Rules)>> = Mutex::new(None);

/// Le prêt existe encore et ne finit pas (repris, rendu, prêt coupé pendant
/// sa préparation : elle s'arrête là).
fn alive(s: &AppState, id: &str) -> bool {
    s.lend.loan().is_some_and(|l| l.id == id && l.phase != lend::Phase::Ending)
}

/// Repère d'un prêt en cours, dans le dossier de données : un Admin arrêté
/// en plein prêt le retrouve à son démarrage ([`recover`]).
const ACTIVE_FILE: &str = "lend-active";
#[cfg(not(test))]
const RECOVER_TRIES: u32 = 30;
#[cfg(test)]
const RECOVER_TRIES: u32 = 1;

/// Un prêt arrêté pendant sa préparation, et déjà remis en état : son repère part.
fn abandoned(s: &AppState, id: &str) -> bool {
    if s.lend.loan().is_none_or(|l| l.id != id) {
        let _ = std::fs::remove_file(s.data_dir.join(ACTIVE_FILE));
    }
    false
}

/// Au démarrage d'Admin, un prêt laissé en plein milieu (Admin arrêté ou
/// tombé) : la session se verrouille et l'ami est coupé de Kyber ; l'agent de
/// console, lui, se remet en état en trouvant Admin sans prêt.
pub async fn recover(s: &AppState) {
    let path = s.data_dir.join(ACTIVE_FILE);
    if !path.exists() {
        return;
    }
    tracing::warn!("prêt : Admin redémarré en plein prêt : session verrouillée, ami coupé de Kyber");
    if let Err(e) = tokio::task::spawn_blocking(lend_sys::lock_session).await.unwrap_or_else(|e| Err(anyhow::anyhow!("{e}"))) {
        tracing::warn!("prêt : session non verrouillée : {e:#}");
    }
    // Kyber démarre peut-être encore : jusqu'à une minute.
    for attempt in 0..RECOVER_TRIES {
        if let Ok((status, v)) = crate::kyber_proxy::kyber_call(s, Method::GET, "/kybernow/sessions", None).await
            && status.is_success()
        {
            let mut users: Vec<String> = v["sessions"].as_array().into_iter().flatten().filter_map(|x| x["user"].as_str()).filter(|u| u.starts_with("kn-lend-")).map(str::to_owned).collect();
            users.dedup();
            for user in users {
                let body = json!({ "user": user, "ban_seconds": KYBER_BAN_SECONDS });
                let _ = crate::kyber_proxy::kyber_call(s, Method::POST, "/kybernow/close", Some(body)).await;
            }
            break;
        }
        if attempt + 1 < RECOVER_TRIES {
            tokio::time::sleep(Duration::from_secs(2)).await;
        }
    }
    let _ = std::fs::remove_file(&path);
}

/// La dernière fois qu'une entrée venait de loin (session d'un appareil,
/// partage, prêt) : relevé chaque seconde par la surveillance du prêt.
static REMOTE_SEEN: Mutex<Option<Instant>> = Mutex::new(None);

/// L'ordinateur est-il utilisé (session d'un appareil, partage, quelqu'un
/// devant depuis moins de 5 minutes) ? `GetLastInputInfo` compte aussi
/// l'entrée injectée par Kyber : une entrée d'avant la fin de la dernière
/// session à distance n'est pas quelqu'un devant le PC.
async fn in_use(s: &AppState) -> bool {
    if s.activity.live().is_some() || s.share.is_open() {
        return true;
    }
    let remote_ago = REMOTE_SEEN.lock().unwrap().map(|t| t.elapsed().as_secs());
    tokio::task::spawn_blocking(lend_sys::idle_secs).await.ok().flatten().is_some_and(|idle| present(idle, remote_ago))
}

/// Quelqu'un devant le PC : une entrée de moins de 5 minutes, plus récente
/// (de 2 s au moins) que la fin de la dernière session à distance.
fn present(idle: u64, remote_ago: Option<u64>) -> bool {
    idle < PRESENT && remote_ago.is_none_or(|ago| idle + 2 < ago)
}

/// Prépare le prêt : la session de l'hôte se ferme, un partage s'arrête,
/// l'écran virtuel de l'ami se crée, le mode console se met en place.
/// Rend vrai une fois le prêt commencé.
async fn prepare(s: &AppState, loan: lend::Loan) -> bool {
    tracing::info!("prêt : préparation pour {} ({:?})", loan.who.name, loan.who.kind);
    let begun = tokio::time::Instant::now();
    if !alive(s, &loan.id) {
        return false;
    }
    let _ = std::fs::write(s.data_dir.join(ACTIVE_FILE), &loan.id);
    // La session de l'hôte : on lui demande de finir (7 s), puis on la ferme.
    if s.activity.live().is_some() {
        s.activity.request_end();
        let deadline = Instant::now() + Duration::from_secs(7);
        while s.activity.live().is_some() && Instant::now() < deadline {
            tokio::time::sleep(Duration::from_millis(200)).await;
        }
        if s.activity.live().is_some() {
            s.activity.drop_live(Msg::new("events.lendTookOver"));
        }
    }
    if s.share.is_open() {
        crate::share_api::close_share(s).await;
    }
    // L'écran virtuel à la taille de l'ami, et le son.
    let screen = loan.who.screen;
    let (d, a, id) = (s.display.clone(), s.audio.clone(), loan.id.clone());
    let started = blocking(move || {
        d.stop("prêt de l'ordinateur");
        let mode = Mode { width: screen.width, height: screen.height, refresh: screen.refresh };
        d.start_keeping_others(mode, Some(id))?;
        a.start(Origin::App, Some(2));
        Ok(d.primary_screen())
    })
    .await;
    let display = match started {
        Ok(Some((name, mode))) if !name.is_empty() => (crate::display::kyber_display_id(&name), Screen { width: mode.width, height: mode.height, refresh: mode.refresh }),
        Ok(_) if cfg!(not(windows)) => (1, screen),
        Ok(_) => {
            tracing::warn!("prêt : écran virtuel sans nom");
            return false;
        }
        Err(e) => {
            tracing::warn!("prêt : écran virtuel impossible : {}", e.1);
            return false;
        }
    };
    // Arrêté pendant que l'écran se créait : la remise en état est peut-être
    // déjà passée, l'écran et le son se retirent ici.
    if !alive(s, &loan.id) {
        let (d, a) = (s.display.clone(), s.audio.clone());
        let _ = blocking(move || {
            d.stop("prêt arrêté pendant sa préparation");
            a.stop("prêt arrêté pendant sa préparation");
            Ok(())
        })
        .await;
        return abandoned(s, &loan.id);
    }
    s.lend.set_display(&loan.id, display.0, display.1);
    // Le mode console : l'agent dans la session de l'utilisateur, la surveillance par Admin.
    let settings = settings(s);
    let exes: Vec<String> = settings.apps.iter().filter_map(|a| a.exe.clone()).collect();
    let kyber_dir = s.kyber_dir.clone();
    let watch = tokio::task::spawn_blocking(move || {
        let rules = lend_sys::rules(&exes, lend_sys::steam_dir(), &kyber_dir);
        lend_sys::watch(rules.clone()).map(|w| (w, rules))
    })
    .await
    .ok()
    .flatten();
    {
        // Vérifié sous le verrou de WATCH : la remise en état le prend après
        // avoir fait finir le prêt, une surveillance ne lui échappe jamais.
        let mut slot = WATCH.lock().unwrap();
        if !alive(s, &loan.id) {
            return abandoned(s, &loan.id); // `watch` lâché : sa surveillance s'arrête.
        }
        *slot = watch;
    }
    if cfg!(windows) {
        if !alive(s, &loan.id) {
            return abandoned(s, &loan.id);
        }
        if let Err(e) = tokio::task::spawn_blocking(lend_sys::spawn_console).await.unwrap_or_else(|e| Err(anyhow::anyhow!("{e}"))) {
            tracing::warn!("prêt : agent de console impossible : {e:#}");
            return false;
        }
        let deadline = (tokio::time::Instant::now() + READY_WAIT).min(begun + PREPARE_WAIT);
        loop {
            let changed = s.lend.changed();
            tokio::pin!(changed);
            changed.as_mut().enable();
            match s.lend.loan() {
                Some(l) if l.id == loan.id && l.phase == lend::Phase::Lent => break,
                Some(l) if l.id == loan.id => {}
                _ => return false,
            }
            if tokio::time::timeout_at(deadline, changed).await.is_err() {
                tracing::warn!("prêt : le mode console ne s'est pas mis en place à temps");
                return false;
            }
        }
    } else {
        // Développement : pas d'agent, le mode console est tenu pour prêt.
        s.lend.console_ready(Some(&loan.id));
    }
    if let Some(friend) = &loan.who.friend {
        s.friends.touch(friend);
    }
    s.activity.event("info", Msg::new("events.lendStarted").with("name", loan.who.name.as_str()));
    s.relay.wake();
    true
}

/// Prépare en tâche de fond ; un échec arrête le prêt.
fn spawn_prepare(s: &AppState, loan: lend::Loan) {
    let s = s.clone();
    tokio::spawn(async move {
        let id = loan.id.clone();
        if !prepare(&s, loan).await {
            finish(&s, &id, Reason::Stopped).await;
        }
    });
}

/// Finit le prêt : préavis passé ou reprise immédiate. Une seule remise en
/// état à la fois (les autres appels ne font rien).
pub async fn finish(s: &AppState, id: &str, reason: Reason) {
    let Some(loan) = s.lend.ending(Some(id), reason) else { return };
    tracing::info!("prêt de {} : fin ({reason:?})", loan.who.name);
    // L'ami connecté voit la fin dans le lanceur, sans plus rien envoyer.
    if loan.console_ready && loan.connected && loan.display.is_some() {
        kyber_spectator(s, &loan).await;
        tokio::time::sleep(ENDING_SHOW).await;
    }
    s.relay.revoke_guest(&loan.id);
    kyber_close(s, &loan).await;
    // L'agent de console se remet en état (apps fermées, Steam, fenêtres).
    if cfg!(windows) && loan.console_ready {
        let deadline = tokio::time::Instant::now() + ENDED_WAIT;
        while tokio::time::Instant::now() < deadline && CONSOLE_ENDED.lock().unwrap().as_deref() != Some(loan.id.as_str()) {
            tokio::time::sleep(Duration::from_millis(250)).await;
        }
    }
    // Ce qui reste des apps du prêt, l'écran virtuel, le son, puis le verrouillage.
    let watch = WATCH.lock().unwrap().take();
    let (d, a) = (s.display.clone(), s.audio.clone());
    let _ = blocking(move || {
        if let Some((w, rules)) = watch {
            let closed = w.stop_and_close(&rules);
            if closed > 0 {
                tracing::info!("prêt : {closed} processus fermés");
            }
        }
        d.stop("fin du prêt");
        a.stop("fin du prêt");
        if let Err(e) = lend_sys::lock_session() {
            tracing::warn!("prêt : session non verrouillée : {e:#}");
        }
        Ok(())
    })
    .await;
    s.lend.ended(&loan.id, reason);
    let _ = std::fs::remove_file(s.data_dir.join(ACTIVE_FILE));
    match reason {
        Reason::Left => s.activity.event("info", Msg::new("events.lendGivenBack").with("name", loan.who.name.as_str())),
        Reason::TakenBack => s.activity.event("info", Msg::new("events.lendTakenBack")),
        _ => tracing::info!("prêt de {} terminé", loan.who.name),
    }
    s.relay.wake();
}

fn spawn_finish(s: &AppState, id: String, reason: Reason) {
    let s = s.clone();
    tokio::spawn(async move { finish(&s, &id, reason).await });
}

/// Le prêt dont l'agent de console a fini sa remise en état.
static CONSOLE_ENDED: Mutex<Option<String>> = Mutex::new(None);

// --- L'ami -------------------------------------------------------------------------------

#[derive(Deserialize)]
pub struct StatusBody {
    login: String,
    password: String,
}

/// Le compte d'ami de `login`/`password`, ou le refus à rendre.
async fn friend_of(s: &AppState, login: &str, password: &str) -> Result<lend::Friend, Response> {
    let login = login.trim().to_ascii_lowercase();
    if let Some(wait) = s.lend.limits.account_locked(&login, Instant::now()) {
        return Err(too_many(wait));
    }
    let friend = s.friends.by_login(&login).filter(|f| f.enabled);
    let hash = friend.as_ref().map(|f| f.hash().to_string());
    let password = password.to_string();
    // Toujours un calcul argon2, même sans compte : la réponse ne dit pas s'il existe.
    let ok = s
        .auth
        .clone()
        .hash_limited(move || {
            let reference = hash.unwrap_or_else(|| "$argon2id$v=19$m=19456,t=2,p=1$YWFhYWFhYWFhYWFhYWFhYQ$AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA".into());
            Ok(lend::verify_friend_password(&reference, &password))
        })
        .await
        .unwrap_or(false);
    match friend {
        Some(f) if ok => Ok(f),
        _ => {
            s.lend.limits.account_failed(&login, Instant::now());
            Err(friend_error(StatusCode::UNAUTHORIZED, "bad_credentials"))
        }
    }
}

/// `POST /api/lend/status` : l'état de la carte « Ordinateur de {hôte} ».
pub async fn status(State(s): State<AppState>, ConnectInfo(peer): ConnectInfo<SocketAddr>, Json(body): Json<StatusBody>) -> Response {
    if let Err(wait) = s.lend.limits.count_try(peer.ip(), Instant::now()) {
        return too_many(wait);
    }
    let friend = match friend_of(&s, &body.login, &body.password).await {
        Ok(f) => f,
        Err(res) => return res,
    };
    let session = tokio::task::spawn_blocking(lend_sys::session_state).await.unwrap_or(SessionState::None);
    // `mine` : le prêt en cours est le sien (il peut y revenir).
    let mine = s.lend.loan().is_some_and(|l| l.who.friend.as_deref() == Some(friend.id.as_str()));
    let state = if !enabled(&s) || session == SessionState::None {
        "off"
    } else if s.lend.loan().is_some() {
        "lent"
    } else if session == SessionState::Locked {
        "locked"
    } else if in_use(&s).await {
        "busy"
    } else {
        "free"
    };
    let at = LocalTime::now();
    let slot = (!friend.slots.is_empty()).then(|| {
        json!({
            "now": lend::slot_now(&friend.slots, at).is_some(),
            "next": lend::next_slot(&friend.slots, at).map(|(start, end)| json!({ "start": start, "end": end })),
        })
    });
    let host = tokio::task::spawn_blocking(host_name).await.unwrap_or_default();
    let answer = json!({ "state": state, "mine": mine, "host": host, "computer": hostname(), "slot": slot });
    ([(header::CACHE_CONTROL, "no-store")], Json(answer)).into_response()
}

#[derive(Deserialize)]
pub struct BorrowBody {
    #[serde(default)]
    code: Option<String>,
    #[serde(default)]
    login: Option<String>,
    #[serde(default)]
    password: Option<String>,
    #[serde(default)]
    name: String,
    #[serde(default)]
    device: String,
    #[serde(default)]
    app: String,
    #[serde(default)]
    screen: Option<Screen>,
}

/// `POST /api/lend/borrow` : emprunter, avec un code ou un compte d'ami.
pub async fn borrow(
    State(s): State<AppState>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    via: Option<axum::Extension<Via>>,
    Json(body): Json<BorrowBody>,
) -> Response {
    let via = via.map(|axum::Extension(v)| v);
    if let Err(wait) = s.lend.limits.count_try(peer.ip(), Instant::now()) {
        return too_many(wait);
    }
    let mut who = Who::new(&body.name, &body.device, &body.app, via.is_some(), Kind::Code, body.screen);
    let (until, steam, from_session);
    let mut slot = false;
    if let Some(code) = body.code.as_deref().map(|c| c.chars().filter(|c| !c.is_whitespace()).collect::<String>()) {
        who.code = Some(code.clone());
        // Le même ami qui revient pendant le prêt (15 min), avec son code.
        if let Some(res) = come_back(&s, &who, via.as_ref()).await {
            return res;
        }
        let Some(c) = s.lend.take_code(&code) else { return friend_error(StatusCode::NOT_FOUND, "no_code") };
        until = c.duration.map(|d| now_secs() + d);
        steam = c.steam;
        from_session = c.from_session;
    } else {
        let friend = match friend_of(&s, body.login.as_deref().unwrap_or(""), body.password.as_deref().unwrap_or("")).await {
            Ok(f) => f,
            Err(res) => return res,
        };
        who.kind = Kind::Friend;
        who.friend = Some(friend.id.clone());
        who.name = friend.name.clone();
        if let Some(res) = come_back(&s, &who, via.as_ref()).await {
            return res;
        }
        match lend::slot_now(&friend.slots, LocalTime::now()) {
            Some(end) => {
                who.kind = Kind::Slot;
                until = Some(end);
                slot = true;
            }
            None => until = None,
        }
        steam = friend.steam;
        from_session = false;
    }
    if who.name.is_empty() {
        return friend_error(StatusCode::BAD_REQUEST, "bad_request");
    }
    if !enabled(&s) {
        return friend_error(StatusCode::SERVICE_UNAVAILABLE, "off");
    }
    if s.lend.loan().is_some() {
        return friend_error(StatusCode::CONFLICT, "lent");
    }
    match tokio::task::spawn_blocking(lend_sys::session_state).await.unwrap_or(SessionState::None) {
        SessionState::None => return friend_error(StatusCode::SERVICE_UNAVAILABLE, "off"),
        SessionState::Locked => return friend_error(StatusCode::LOCKED, "locked"),
        SessionState::Open => {}
    }
    // Ordinateur utilisé : l'accord de l'hôte, sauf un code créé depuis sa
    // session (il vaut accord) ; un créneau passe avant, après 2 min de préavis.
    if in_use(&s).await && !from_session {
        let auto_at = slot.then(|| Instant::now() + lend::NOTICE);
        let name = who.name.clone();
        let request = s.lend.ask(who, None, until, steam, auto_at);
        if slot {
            s.activity.event("info", Msg::new("events.lendSlotStart").with("name", name.as_str()));
        } else {
            s.activity.event("info", Msg::new("events.lendRequest").with("name", name.as_str()));
        }
        tracing::info!("prêt : {name} demande l'ordinateur");
        // `host` : « En attente de {hôte}… », même venu par un code.
        let host = tokio::task::spawn_blocking(host_name).await.unwrap_or_default();
        let answer = json!({ "request": request.id, "secret": request.secret(), "host": host, "computer": hostname() });
        return (StatusCode::ACCEPTED, [(header::CACHE_CONTROL, "no-store")], Json(answer)).into_response();
    }
    // Deux emprunts en même temps : un seul prêt.
    let Some(loan) = s.lend.begin(who, until, steam, None) else { return friend_error(StatusCode::CONFLICT, "lent") };
    if prepare(&s, loan.clone()).await {
        match s.lend.loan().filter(|l| l.id == loan.id) {
            Some(l) => loan_answer(&s, &l, via.as_ref()),
            None => friend_error(StatusCode::SERVICE_UNAVAILABLE, "off"),
        }
    } else {
        finish(&s, &loan.id, Reason::Stopped).await;
        friend_error(StatusCode::SERVICE_UNAVAILABLE, "off")
    }
}

/// Le même ami qui revient (15 min) : son prêt, s'il a la même preuve (le
/// code, ou son compte vérifié) et le même appareil, ou s'il n'était plus
/// connecté. Encore en préparation : il attend qu'elle finisse.
async fn come_back(s: &AppState, who: &Who, via: Option<&Via>) -> Option<Response> {
    if let Some(id) = s.lend.preparing_for(who) {
        let deadline = tokio::time::Instant::now() + PREPARE_WAIT;
        loop {
            let changed = s.lend.changed();
            tokio::pin!(changed);
            changed.as_mut().enable();
            match s.lend.loan() {
                Some(l) if l.id == id && l.phase == lend::Phase::Preparing => {}
                _ => break,
            }
            if tokio::time::timeout_at(deadline, changed).await.is_err() {
                return Some(friend_error(StatusCode::SERVICE_UNAVAILABLE, "off"));
            }
        }
    }
    let loan = s.lend.resume(who)?;
    Some(loan_answer(s, &loan, via))
}

#[derive(Deserialize)]
pub struct WaitBody {
    #[serde(default)]
    request: Option<String>,
    #[serde(default)]
    lend: Option<String>,
}

fn lend_secret(headers: &HeaderMap) -> String {
    headers.get(LEND_HEADER).and_then(|v| v.to_str().ok()).unwrap_or_default().trim().to_string()
}

/// `POST /api/lend/wait` : la demande attend l'accord de l'hôte, puis la
/// préparation (30 s au plus par appel, à répéter).
pub async fn wait(State(s): State<AppState>, headers: HeaderMap, via: Option<axum::Extension<Via>>, Json(body): Json<WaitBody>) -> Response {
    let via = via.map(|axum::Extension(v)| v);
    let secret = lend_secret(&headers);
    let id = body.request.or(body.lend).unwrap_or_default();
    let deadline = tokio::time::Instant::now() + WAIT_HOLD;
    loop {
        let changed = s.lend.changed();
        tokio::pin!(changed);
        changed.as_mut().enable();
        match s.lend.wait(&id, &secret) {
            Wait::Lent(loan) => return loan_answer(&s, &loan, via.as_ref()),
            Wait::Pending => {}
            Wait::Refused => return friend_error(StatusCode::FORBIDDEN, "refused"),
            Wait::Expired => return friend_error(StatusCode::GONE, "expired"),
            Wait::Unknown => return friend_error(StatusCode::NOT_FOUND, "unknown"),
        }
        if tokio::time::timeout_at(deadline, changed).await.is_err() {
            return (StatusCode::ACCEPTED, [(header::CACHE_CONTROL, "no-store")], Json(json!({ "pending": true }))).into_response();
        }
    }
}

#[derive(Deserialize)]
pub struct LendBody {
    #[serde(default)]
    lend: Option<String>,
    #[serde(default)]
    request: Option<String>,
}

/// `POST /api/lend/beat` : signe de vie de l'ami, jeton renouvelé, préavis, fin.
pub async fn beat(State(s): State<AppState>, headers: HeaderMap, via: Option<axum::Extension<Via>>, Json(body): Json<LendBody>) -> Response {
    let answer = match s.lend.beat(body.lend.as_deref().unwrap_or(""), &lend_secret(&headers)) {
        Beat::Unknown => return friend_error(StatusCode::NOT_FOUND, "unknown"),
        Beat::Ended(reason) => json!({ "token": null, "expires": 0, "until": null, "notice": null, "end": end_name(reason) }),
        Beat::Live(loan) => {
            if let Some(axum::Extension(via)) = &via {
                s.relay.grant_guest(via, &loan.id);
            }
            json!({
                "token": kyber_token(&s, &loan),
                "expires": lend::TOKEN_LIFETIME,
                "until": loan.until,
                "notice": loan.notice,
                "end": null,
            })
        }
    };
    ([(header::CACHE_CONTROL, "no-store")], Json(answer)).into_response()
}

/// La fin, telle que l'ami la lit.
fn end_name(reason: Reason) -> &'static str {
    match reason {
        Reason::TakenBack => "taken_back",
        Reason::TimeUp => "time_up",
        Reason::SlotEnd => "slot_end",
        Reason::Left => "left",
        Reason::SlotStart | Reason::Stopped => "stopped",
    }
}

/// `POST /api/lend/leave` : « Rendre l'ordinateur », ou annuler une demande.
pub async fn leave(State(s): State<AppState>, headers: HeaderMap, via: Option<axum::Extension<Via>>, Json(body): Json<LendBody>) -> Response {
    let secret = lend_secret(&headers);
    let mut res = if let Some(request) = body.request.as_deref().filter(|r| s.lend.cancel_request(r, &secret)) {
        tracing::info!("prêt : demande {request} annulée");
        Json(json!({ "ok": true })).into_response()
    } else {
        let id = body.lend.unwrap_or_default();
        match s.lend.loan().filter(|l| l.id == id && lend::same(&secret, l.secret())) {
            Some(loan) => {
                spawn_finish(&s, loan.id.clone(), Reason::Left);
                Json(json!({ "ok": true })).into_response()
            }
            None => friend_error(StatusCode::NOT_FOUND, "unknown"),
        }
    };
    if lend::cookie(&headers, via.is_some()).is_some()
        && let Ok(value) = HeaderValue::from_str(&lend::clear_cookie(s.secure_cookies, via.is_some()))
    {
        res.headers_mut().insert(header::SET_COOKIE, value);
    }
    res
}

// --- L'hôte -------------------------------------------------------------------------------

/// L'hôte : un appareil appairé (compte Basic), ou la session d'Admin.
fn host(s: &AppState, headers: &HeaderMap) -> LendResult<()> {
    stream_user(s, headers).map(|_| ()).map_err(LendError::from)
}

/// L'icône (boucle locale, jeton local), ou la session d'Admin.
fn local_or_admin(s: &AppState, peer: SocketAddr, headers: &HeaderMap) -> LendResult<()> {
    if crate::auth::session_token(headers).is_some_and(|t| s.auth.session_valid(&t)) {
        return Ok(());
    }
    if !peer.ip().to_canonical().is_loopback() {
        return Err(LendError::new(StatusCode::FORBIDDEN, "forbidden", Msg::new("api.localOnly")));
    }
    if !s.local_token.accepts(headers) {
        return Err(LendError::new(StatusCode::FORBIDDEN, "forbidden", Msg::new("api.localToken")));
    }
    Ok(())
}

#[derive(Deserialize, Default)]
pub struct CodeBody {
    #[serde(default)]
    duration: Option<u64>,
    #[serde(default)]
    steam: Option<Steam>,
    #[serde(default)]
    device_id: Option<String>,
}

fn create_code(s: &AppState, body: CodeBody, user: Option<&str>) -> LendResult<Json<Value>> {
    if !enabled(s) {
        return Err(LendError::new(StatusCode::CONFLICT, "disabled", Msg::new("lend.disabled")));
    }
    if s.lend.loan().is_some() {
        return Err(LendError::new(StatusCode::CONFLICT, "lent", Msg::new("lend.inUse").with("name", s.lend.borrower().unwrap_or_default())));
    }
    if body.duration.is_some_and(|d| !lend::DURATIONS.contains(&d)) {
        return Err(LendError::new(StatusCode::BAD_REQUEST, "bad_request", Msg::new("lend.badDuration")));
    }
    // Un code créé par l'appareil en session vaut accord : sa session se fermera.
    let from_session = s.activity.live().is_some_and(|l| match &body.device_id {
        Some(id) => &l.device_id == id,
        None => user.is_some_and(|u| u == l.user),
    });
    let share_code = s.share.snapshot().map(|snap| snap.code);
    let previous = s.lend.code_value();
    s.lend.create_code(body.duration, body.steam.unwrap_or_default(), from_session, |c| share_code.as_deref() == Some(c) || previous.as_deref() == Some(c));
    s.relay.wake();
    Ok(Json(view(s)))
}

fn cancel_code(s: &AppState) -> Json<Value> {
    if s.lend.cancel_code().is_some() {
        s.relay.wake();
    }
    Json(view(s))
}

#[derive(Deserialize)]
pub struct AnswerBody {
    accept: bool,
    #[serde(default)]
    steam: Option<Steam>,
}

fn answer_request(s: &AppState, id: &str, body: AnswerBody) -> LendResult<Json<Value>> {
    let request = s.lend.answer(id, body.accept, body.steam).ok_or_else(|| LendError::new(StatusCode::NOT_FOUND, "unknown_request", Msg::new("lend.unknownRequest")))?;
    if body.accept {
        if s.lend.loan().is_some() {
            return Err(LendError::new(StatusCode::CONFLICT, "lent", Msg::new("lend.inUse").with("name", s.lend.borrower().unwrap_or_default())));
        }
        tracing::info!("prêt : demande de {} acceptée", request.who.name);
        if let Some(loan) = s.lend.begin_from_request(&request.id, None) {
            spawn_prepare(s, loan);
        }
    } else {
        tracing::info!("prêt : demande de {} refusée", request.who.name);
        s.activity.event("info", Msg::new("events.lendRefused").with("name", request.who.name.as_str()));
    }
    Ok(Json(view(s)))
}

#[derive(Deserialize, Default)]
pub struct TakeBackBody {
    #[serde(default)]
    now: bool,
}

fn take_back(s: &AppState, body: TakeBackBody) -> LendResult<Json<Value>> {
    let loan = s.lend.loan().ok_or_else(|| LendError::new(StatusCode::NOT_FOUND, "no_lend", Msg::new("lend.noLend")))?;
    if body.now {
        spawn_finish(s, loan.id, Reason::TakenBack);
    } else {
        s.lend.notice(Reason::TakenBack, now_secs() + lend::NOTICE.as_secs());
    }
    Ok(Json(view(s)))
}

fn json_body<T: for<'a> Deserialize<'a> + Default>(body: &Bytes) -> LendResult<T> {
    if body.iter().all(u8::is_ascii_whitespace) {
        return Ok(T::default());
    }
    serde_json::from_slice(body).map_err(|e| LendError::new(StatusCode::BAD_REQUEST, "bad_request", Msg::new("api.unreadableBody").with("error", e.to_string())))
}

pub async fn host_get(State(s): State<AppState>, headers: HeaderMap) -> LendResult<Json<Value>> {
    host(&s, &headers)?;
    Ok(Json(view(&s)))
}

pub async fn host_code(State(s): State<AppState>, headers: HeaderMap, body: Bytes) -> LendResult<Json<Value>> {
    let user = stream_user(&s, &headers).map_err(LendError::from)?;
    create_code(&s, json_body(&body)?, Some(&user))
}

pub async fn host_code_cancel(State(s): State<AppState>, headers: HeaderMap) -> LendResult<Json<Value>> {
    host(&s, &headers)?;
    Ok(cancel_code(&s))
}

pub async fn host_answer(State(s): State<AppState>, headers: HeaderMap, Path(id): Path<String>, Json(body): Json<AnswerBody>) -> LendResult<Json<Value>> {
    host(&s, &headers)?;
    answer_request(&s, &id, body)
}

pub async fn host_take_back(State(s): State<AppState>, headers: HeaderMap, body: Bytes) -> LendResult<Json<Value>> {
    host(&s, &headers)?;
    take_back(&s, json_body(&body)?)
}

pub async fn local_get(State(s): State<AppState>, ConnectInfo(peer): ConnectInfo<SocketAddr>, headers: HeaderMap) -> LendResult<Json<Value>> {
    local_or_admin(&s, peer, &headers)?;
    Ok(Json(view(&s)))
}

pub async fn local_code(State(s): State<AppState>, ConnectInfo(peer): ConnectInfo<SocketAddr>, headers: HeaderMap, body: Bytes) -> LendResult<Json<Value>> {
    local_or_admin(&s, peer, &headers)?;
    create_code(&s, json_body(&body)?, None)
}

pub async fn local_code_cancel(State(s): State<AppState>, ConnectInfo(peer): ConnectInfo<SocketAddr>, headers: HeaderMap) -> LendResult<Json<Value>> {
    local_or_admin(&s, peer, &headers)?;
    Ok(cancel_code(&s))
}

pub async fn local_answer(
    State(s): State<AppState>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(body): Json<AnswerBody>,
) -> LendResult<Json<Value>> {
    local_or_admin(&s, peer, &headers)?;
    answer_request(&s, &id, body)
}

pub async fn local_take_back(State(s): State<AppState>, ConnectInfo(peer): ConnectInfo<SocketAddr>, headers: HeaderMap, body: Bytes) -> LendResult<Json<Value>> {
    local_or_admin(&s, peer, &headers)?;
    take_back(&s, json_body(&body)?)
}

// --- Réglages (session d'Admin) -------------------------------------------------------------

#[derive(Deserialize)]
pub struct SettingsBody {
    #[serde(default)]
    enabled: Option<bool>,
    #[serde(default)]
    steam: Option<bool>,
    #[serde(default)]
    apps: Option<Vec<crate::launch::Launch>>,
}

fn settings_json(s: &AppState) -> Value {
    let settings = settings(s);
    json!({ "enabled": settings.enabled, "apps": settings.apps, "steam": settings.steam, "steam_found": lend_sys::steam_dir().is_some() || cfg!(not(windows)) })
}

pub async fn get_settings(State(s): State<AppState>) -> Json<Value> {
    Json(settings_json(&s))
}

pub async fn put_settings(State(s): State<AppState>, Json(body): Json<SettingsBody>) -> LendResult<Json<Value>> {
    let apps = match body.apps {
        Some(apps) => {
            let launcher = s.launcher.clone();
            Some(blocking(move || apps.into_iter().map(|a| launcher.validate(a)).collect::<anyhow::Result<Vec<_>>>()).await?)
        }
        None => None,
    };
    let before = enabled(&s);
    s.update_settings(|settings| {
        if let Some(on) = body.enabled {
            settings.lend.enabled = on;
        }
        if let Some(steam) = body.steam {
            settings.lend.steam = steam;
        }
        if let Some(apps) = apps {
            settings.lend.apps = apps;
        }
    })?;
    let after = enabled(&s);
    if before != after {
        s.activity.event("info", Msg::new(if after { "events.lendOn" } else { "events.lendOff" }));
        if !after {
            // Prêt coupé : code effacé, demandes refusées, prêt arrêté.
            s.lend.cancel_code();
            for r in s.lend.requests() {
                s.lend.answer(&r.id, false, None);
            }
            if let Some(loan) = s.lend.loan() {
                spawn_finish(&s, loan.id, Reason::Stopped);
            }
        }
        s.relay.wake();
    }
    Ok(Json(settings_json(&s)))
}

pub async fn friends(State(s): State<AppState>) -> Json<Value> {
    Json(Value::Array(s.friends.all().iter().map(lend::Friend::json).collect()))
}

async fn hash(s: &AppState, password: String) -> LendResult<String> {
    Ok(s.auth.clone().hash_limited(move || lend::hash_friend_password(&password)).await?)
}

pub async fn friend_create(State(s): State<AppState>, Json(body): Json<lend::FriendBody>) -> LendResult<Json<Value>> {
    let Some(password) = body.password.clone() else {
        return Err(LendError::new(StatusCode::BAD_REQUEST, "bad_request", Msg::new("lend.passwordLength")));
    };
    let hash = hash(&s, password).await?;
    let friend = s.friends.create(&body, hash)?;
    s.activity.event("info", Msg::new("events.lendFriendAdded").with("name", friend.name.as_str()));
    s.relay.wake();
    Ok(Json(friend.json()))
}

pub async fn friend_update(State(s): State<AppState>, Path(id): Path<String>, Json(body): Json<lend::FriendBody>) -> LendResult<Json<Value>> {
    let hash = match body.password.clone().filter(|p| !p.is_empty()) {
        Some(p) => Some(hash(&s, p).await?),
        None => None,
    };
    let friend = s.friends.update(&id, &body, hash)?.ok_or_else(|| LendError::new(StatusCode::NOT_FOUND, "unknown_friend", Msg::new("lend.unknownFriend")))?;
    // Compte coupé : son prêt en cours s'arrête, sa demande part.
    if !friend.enabled {
        if let Some(loan) = s.lend.loan().filter(|l| l.who.friend.as_deref() == Some(friend.id.as_str())) {
            spawn_finish(&s, loan.id, Reason::Stopped);
        }
        s.lend.drop_requests_of(&friend.id);
    }
    Ok(Json(friend.json()))
}

pub async fn friend_delete(State(s): State<AppState>, Path(id): Path<String>) -> LendResult<Json<Value>> {
    let friend = s.friends.delete(&id)?.ok_or_else(|| LendError::new(StatusCode::NOT_FOUND, "unknown_friend", Msg::new("lend.unknownFriend")))?;
    // Sa dernière partie est oubliée ; son prêt en cours s'arrête.
    s.last_played.forget(&lend_last::friend_key(&friend.id));
    if let Some(loan) = s.lend.loan().filter(|l| l.who.friend.as_deref() == Some(friend.id.as_str())) {
        spawn_finish(&s, loan.id, Reason::Stopped);
    }
    Ok(Json(json!({ "ok": true })))
}

/// `GET /api/lend/friends/{id}/invite` : le lien des apps et l'adresse du navigateur.
pub async fn friend_invite(State(s): State<AppState>, Path(id): Path<String>) -> LendResult<Json<Value>> {
    let friend = s.friends.get(&id).ok_or_else(|| LendError::new(StatusCode::NOT_FOUND, "unknown_friend", Msg::new("lend.unknownFriend")))?;
    let address = crate::tls::lan_ip().unwrap_or_else(|| "localhost".into());
    let port = s.listen_port;
    let mut link = format!("kybernow://borrow?host={address}&port={port}");
    if let Some(fp) = &s.tls_fingerprint {
        link.push_str(&format!("&fp={}", urlencoding::encode(fp)));
    }
    // Le prénom de l'hôte : « {hôte} vous prête son ordinateur. » avant tout essai du compte.
    let host = tokio::task::spawn_blocking(host_name).await.unwrap_or_default();
    let (login, name) = (urlencoding::encode(&friend.login), urlencoding::encode(&host));
    link.push_str(&format!("&login={login}"));
    let scheme = if s.secure_cookies { "https" } else { "http" };
    let mut web = format!("{scheme}://{address}:{port}/link/?borrow={login}");
    if let Some(addr) = s.settings.lock().unwrap().relay.addr()
        && let Some(id) = s.relay.id()
    {
        link.push_str(&format!("&relay={}&sid={id}", urlencoding::encode(&addr.to_string())));
        web = format!("https://{id}.{}/link/?borrow={login}", addr.web_authority());
    }
    if !host.is_empty() {
        link.push_str(&format!("&name={name}"));
        web.push_str(&format!("&name={name}"));
    }
    Ok(Json(json!({ "link": link, "web": web })))
}

// --- L'agent de console (jeton local) ---------------------------------------------------------

fn console_only(s: &AppState, peer: SocketAddr, headers: &HeaderMap) -> LendResult<()> {
    if !peer.ip().to_canonical().is_loopback() || !s.local_token.accepts(headers) {
        return Err(LendError::new(StatusCode::FORBIDDEN, "forbidden", Msg::new("api.localToken")));
    }
    Ok(())
}

/// Les apps du lanceur : Steam (s'il est permis), puis les apps choisies.
fn console_apps(s: &AppState) -> Vec<Value> {
    let settings = settings(s);
    let mut apps = Vec::new();
    if settings.steam {
        apps.push(json!({ "id": "steam", "name": "Steam", "icon": null, "kind": "uri", "target": "steam://open/bigpicture", "exe": null }));
    }
    for a in &settings.apps {
        let id = a.icon.as_deref().map(|i| i.trim_end_matches(".png").to_string()).unwrap_or_else(|| crate::launch::icon_name(&a.target).trim_end_matches(".png").to_string());
        apps.push(json!({
            "id": id,
            "name": a.name,
            "icon": a.icon.as_ref().map(|i| format!("/api/local/lend/icons/{i}")),
            "kind": a.kind,
            "target": a.target,
            "exe": a.exe,
        }));
    }
    apps
}

/// `GET /api/local/lend/console` : ce que l'agent de console doit faire.
pub async fn console_get(State(s): State<AppState>, ConnectInfo(peer): ConnectInfo<SocketAddr>, headers: HeaderMap) -> LendResult<Json<Value>> {
    console_only(&s, peer, &headers)?;
    let Some(loan) = s.lend.loan() else { return Ok(Json(json!({ "phase": "idle" }))) };
    // L'écran virtuel de l'ami est le principal, en (0, 0).
    let display = s.display.primary_screen().map(|(name, mode)| json!({ "gdi_name": name, "x": 0, "y": 0, "width": mode.width, "height": mode.height }));
    let lang = crate::i18n::current().code();
    Ok(Json(json!({
        "phase": loan.phase,
        "lend": loan.id,
        "borrower": loan.who.name,
        "host": host_name(),
        "computer": hostname(),
        "since": loan.since,
        "until": loan.until,
        "notice": loan.notice,
        // L'ami a donné signe de vie : son image arrive (« Bonne partie »).
        "connected": loan.beaten && loan.connected,
        "end": loan.end_reason.map(end_name),
        "steam": loan.steam,
        "display": if loan.display.is_some() { display.unwrap_or(Value::Null) } else { Value::Null },
        "apps": console_apps(&s),
        "last": last_tile(&s, &loan).await,
        "steam_exe": null,
        "lang": lang,
    })))
}

/// La grande tuile « Dernière partie » de cet ami, si elle est encore permise.
async fn last_tile(s: &AppState, loan: &lend::Loan) -> Value {
    let Some(played) = lend_last::key_of(&loan.who).and_then(|k| s.last_played.get(&k)) else { return Value::Null };
    let (apps, steam) = (console_apps(s), settings(s).steam);
    tokio::task::spawn_blocking(move || {
        let root = lend_last::steam_root();
        lend_last::tile(&played, &apps, steam, |appid| root.as_deref().and_then(|r| lend_last::steam_game(r, appid))).unwrap_or(Value::Null)
    })
    .await
    .unwrap_or(Value::Null)
}

/// Note la dernière partie de l'ami du prêt en cours.
fn remember(s: &AppState, played: lend_last::Played) {
    if let Some(key) = s.lend.loan().and_then(|l| lend_last::key_of(&l.who)) {
        s.last_played.set(&key, played);
    }
}

/// Un jeu Steam installé, Steam permis.
async fn allowed_steam_game(s: &AppState, appid: u32) -> Option<lend_last::SteamGame> {
    if !settings(s).steam {
        return None;
    }
    tokio::task::spawn_blocking(move || lend_last::steam_root().and_then(|r| lend_last::steam_game(&r, appid))).await.ok().flatten()
}

#[derive(Deserialize)]
pub struct PlayedBody {
    #[serde(default)]
    steam: u32,
}

/// `POST /api/local/lend/console/played` `{"steam": <appid>}` : l'agent a vu
/// l'ami jouer à ce jeu Steam (`RunningAppID`) ; il devient sa dernière partie.
pub async fn console_played(State(s): State<AppState>, ConnectInfo(peer): ConnectInfo<SocketAddr>, headers: HeaderMap, Json(body): Json<PlayedBody>) -> LendResult<Json<Value>> {
    console_only(&s, peer, &headers)?;
    if body.steam > 0 && allowed_steam_game(&s, body.steam).await.is_some() {
        remember(&s, lend_last::Played::Steam { appid: body.steam });
    }
    Ok(Json(json!({ "ok": true })))
}

pub async fn console_ready(State(s): State<AppState>, ConnectInfo(peer): ConnectInfo<SocketAddr>, headers: HeaderMap) -> LendResult<Json<Value>> {
    console_only(&s, peer, &headers)?;
    s.lend.console_ready(None);
    Ok(Json(json!({ "ok": true })))
}

pub async fn console_beat(State(s): State<AppState>, ConnectInfo(peer): ConnectInfo<SocketAddr>, headers: HeaderMap) -> LendResult<Json<Value>> {
    console_only(&s, peer, &headers)?;
    s.lend.console_beat();
    Ok(Json(json!({ "ok": true })))
}

#[derive(Deserialize)]
pub struct LaunchBody {
    id: String,
}

/// Une tuile du lanceur : Admin ouvre l'app permise dans la session.
pub async fn console_launch(State(s): State<AppState>, ConnectInfo(peer): ConnectInfo<SocketAddr>, headers: HeaderMap, Json(body): Json<LaunchBody>) -> LendResult<Json<Value>> {
    console_only(&s, peer, &headers)?;
    let settings = settings(&s);
    let launch = if let Some(appid) = lend_last::steam_appid(&body.id) {
        // « Reprendre {jeu} » : un jeu Steam installé, Steam permis.
        let game = allowed_steam_game(&s, appid).await.ok_or_else(|| LendError::new(StatusCode::NOT_FOUND, "unknown_app", Msg::new("lend.unknownApp")))?;
        crate::launch::Launch {
            kind: crate::launch::LaunchKind::Uri,
            target: format!("steam://rungameid/{appid}"),
            name: game.name,
            icon: None,
            exe: None,
            close: None,
            close_at_end: true,
            skip_desktop: false,
        }
    } else if body.id == "steam" && settings.steam {
        crate::launch::Launch {
            kind: crate::launch::LaunchKind::Uri,
            target: "steam://open/bigpicture".into(),
            name: "Steam".into(),
            icon: None,
            exe: None,
            close: Some("steam://close/bigpicture".into()),
            close_at_end: true,
            skip_desktop: false,
        }
    } else {
        settings
            .apps
            .into_iter()
            .find(|a| a.icon.as_deref().map(|i| i.trim_end_matches(".png")) == Some(body.id.as_str()) || crate::launch::icon_name(&a.target).trim_end_matches(".png") == body.id)
            .ok_or_else(|| LendError::new(StatusCode::NOT_FOUND, "unknown_app", Msg::new("lend.unknownApp")))?
    };
    blocking(move || crate::launch::launch_now(&launch)).await?;
    // La dernière partie : l'app ou le jeu lancé (pas Steam lui-même).
    if let Some(appid) = lend_last::steam_appid(&body.id) {
        remember(&s, lend_last::Played::Steam { appid });
    } else if body.id != "steam" {
        remember(&s, lend_last::Played::App { id: body.id.clone() });
    }
    Ok(Json(json!({ "ok": true })))
}

pub async fn console_give_back(State(s): State<AppState>, ConnectInfo(peer): ConnectInfo<SocketAddr>, headers: HeaderMap) -> LendResult<Json<Value>> {
    console_only(&s, peer, &headers)?;
    if let Some(loan) = s.lend.loan() {
        spawn_finish(&s, loan.id, Reason::Left);
    }
    Ok(Json(json!({ "ok": true })))
}

pub async fn console_take_back(State(s): State<AppState>, ConnectInfo(peer): ConnectInfo<SocketAddr>, headers: HeaderMap, body: Bytes) -> LendResult<Json<Value>> {
    console_only(&s, peer, &headers)?;
    take_back(&s, json_body(&body)?)
}

pub async fn console_ended(State(s): State<AppState>, ConnectInfo(peer): ConnectInfo<SocketAddr>, headers: HeaderMap) -> LendResult<Json<Value>> {
    console_only(&s, peer, &headers)?;
    if let Some(loan) = s.lend.loan() {
        *CONSOLE_ENDED.lock().unwrap() = Some(loan.id);
    }
    Ok(Json(json!({ "ok": true })))
}

/// L'icône d'une app permise, pour le lanceur.
pub async fn console_icon(State(s): State<AppState>, ConnectInfo(peer): ConnectInfo<SocketAddr>, headers: HeaderMap, Path(name): Path<String>) -> Response {
    if let Err(e) = console_only(&s, peer, &headers) {
        return e.into_response();
    }
    if let Some(appid) = lend_last::steam_image_appid(&name) {
        return match allowed_steam_game(&s, appid).await.and_then(|g| g.image).and_then(|p| std::fs::read(p).ok()) {
            Some(bytes) => ([(header::CONTENT_TYPE, "image/jpeg"), (header::CACHE_CONTROL, "max-age=3600")], bytes).into_response(),
            None => StatusCode::NOT_FOUND.into_response(),
        };
    }
    match s.launcher.icon_path(&name).and_then(|p| std::fs::read(p).ok()) {
        Some(bytes) => ([(header::CONTENT_TYPE, "image/png"), (header::CACHE_CONTROL, "max-age=3600")], bytes).into_response(),
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

// --- Veille -------------------------------------------------------------------------------------

/// Chaque seconde : codes et demandes échus, créneaux accordés, préavis,
/// fins, écran virtuel gardé en vie, agent de console surveillé, veille de
/// Windows empêchée tant que le prêt est activé.
pub fn supervise(s: AppState) {
    let r = s.clone();
    tokio::spawn(async move { recover(&r).await });
    tokio::spawn(async move {
        let mut tick = tokio::time::interval(Duration::from_secs(1));
        loop {
            tick.tick().await;
            supervise_tick(&s).await;
        }
    });
}

async fn supervise_tick(s: &AppState) {
    lend_sys::keep_awake(enabled(s));
    if s.activity.live().is_some() || s.share.is_open() || s.lend.loan().is_some() {
        *REMOTE_SEEN.lock().unwrap() = Some(Instant::now());
    }
    let tick = s.lend.tick(Instant::now(), now_secs());
    for name in &tick.expired {
        tracing::info!("prêt : demande de {name} sans réponse");
    }
    for id in &tick.auto_accept {
        if s.lend.loan().is_none()
            && let Some(loan) = s.lend.begin_from_request(id, None)
        {
            tracing::info!("prêt : créneau de {}, l'ordinateur lui est prêté", loan.who.name);
            spawn_prepare(s, loan);
        }
    }
    if let Some(id) = &tick.keep_alive {
        s.display.heartbeat(Some(id), None);
        s.audio.heartbeat();
    }
    if tick.console_silent
        && let Some(loan) = s.lend.loan()
    {
        if loan.console_restarts == 0 {
            tracing::warn!("prêt : l'agent de console ne répond plus, relancé");
            s.lend.console_restarted();
            let _ = tokio::task::spawn_blocking(lend_sys::spawn_console).await;
        } else {
            tracing::warn!("prêt : l'agent de console ne répond plus, prêt arrêté");
            spawn_finish(s, loan.id, Reason::Stopped);
        }
    }
    // La session Windows verrouillée en plein prêt (Ctrl+Alt+Suppr au clavier
    // du PC, agent tombé) : le prêt s'arrête.
    if let Some(loan) = s.lend.loan().filter(|l| l.phase == lend::Phase::Lent)
        && tokio::task::spawn_blocking(lend_sys::session_state).await.is_ok_and(|st| st == SessionState::Locked)
    {
        tracing::warn!("prêt : session Windows verrouillée, prêt arrêté");
        spawn_finish(s, loan.id, Reason::Stopped);
    }
    let ending = tick.end.is_some();
    if let Some((id, reason)) = tick.end {
        spawn_finish(s, id, reason);
    }
    if tick.changed || !tick.auto_accept.is_empty() || ending {
        s.relay.wake();
    }
}

#[cfg(all(test, not(windows)))]
mod tests {
    use super::*;

    fn station() -> (AppState, std::path::PathBuf) {
        let dir = std::env::temp_dir().join(format!("kn-lend-api-{}", rand::random::<u64>()));
        std::fs::create_dir_all(&dir).unwrap();
        let s = AppState::for_tests(dir.clone(), Box::new(crate::service::Simulated::default()));
        std::fs::write(dir.join("kyber_config.toml"), "").unwrap();
        s.kyber.set_jwt_secret("cle-de-test").unwrap();
        s.settings.lock().unwrap().lend.enabled = true;
        (s, dir)
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

    fn borrow_code(code: &str, name: &str, device: &str) -> Json<BorrowBody> {
        Json(serde_json::from_value(json!({ "code": code, "name": name, "device": device, "app": "ios", "screen": { "width": 2560, "height": 1440, "refresh": 120 } })).unwrap())
    }

    fn secret_headers(secret: &str) -> HeaderMap {
        let mut h = HeaderMap::new();
        h.insert(LEND_HEADER, secret.parse().unwrap());
        h
    }

    #[tokio::test]
    async fn un_pret_laisse_par_un_admin_arrete() {
        let (s, dir) = station();
        let Json(view) = local_code(State(s.clone()), local(), admin(&s), Bytes::new()).await.ok().unwrap();
        let code = view["code"]["code"].as_str().unwrap().to_string();
        let (status, lent) = body(borrow(State(s.clone()), peer(50), None, borrow_code(&code, "Léa", "ipad")).await).await;
        assert_eq!(status, StatusCode::OK, "{lent}");
        // Pendant le prêt, le repère est là ; à la fin, il part.
        assert!(dir.join(ACTIVE_FILE).exists());
        finish(&s, lent["lend"].as_str().unwrap(), Reason::Left).await;
        assert!(!dir.join(ACTIVE_FILE).exists());
        // Admin arrêté en plein prêt : au démarrage, session verrouillée, ami coupé, repère parti.
        std::fs::write(dir.join(ACTIVE_FILE), "l1").unwrap();
        recover(&s).await;
        assert!(!dir.join(ACTIVE_FILE).exists());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn quelqu_un_devant_le_pc() {
        // Aucune session à distance connue : l'entrée de moins de 5 min compte.
        assert!(present(10, None));
        assert!(!present(PRESENT, None));
        // La session de l'hôte finie il y a 60 s, dernière entrée il y a 70 s : la sienne, à distance.
        assert!(!present(70, Some(60)));
        // Une entrée après la fin de la session : quelqu'un devant le PC.
        assert!(present(20, Some(60)));
    }

    #[tokio::test]
    async fn la_derniere_partie_de_l_ami_au_lanceur() {
        let (s, dir) = station();
        s.settings.lock().unwrap().lend.apps = vec![crate::launch::Launch {
            kind: crate::launch::LaunchKind::Path,
            target: "D:\\Jeux\\Celeste\\Celeste.exe".into(),
            name: "Celeste".into(),
            icon: Some("a1b2c3d4e5f60718.png".into()),
            exe: None,
            close: None,
            close_at_end: true,
            skip_desktop: false,
        }];
        let lend_once = |device: &'static str| {
            let s = s.clone();
            async move {
                let Json(view) = local_code(State(s.clone()), local(), admin(&s), Bytes::new()).await.ok().unwrap();
                let code = view["code"]["code"].as_str().unwrap().to_string();
                let (status, lent) = body(borrow(State(s.clone()), peer(40), None, borrow_code(&code, "Léa", device)).await).await;
                assert_eq!(status, StatusCode::OK, "{lent}");
                s.lend.loan().unwrap()
            }
        };
        // La première fois : rien.
        let loan = lend_once("ipad-lea").await;
        assert!(last_tile(&s, &loan).await.is_null());
        // L'app lancée devient sa dernière partie (pas la tuile Steam).
        remember(&s, lend_last::Played::App { id: "a1b2c3d4e5f60718".into() });
        let tile = last_tile(&s, &loan).await;
        assert_eq!((tile["id"].as_str(), tile["name"].as_str()), (Some("a1b2c3d4e5f60718"), Some("Celeste")));
        finish(&s, &loan.id, Reason::Left).await;
        // Le même appareil revient avec un autre code : la grande tuile ; un autre appareil : rien.
        let loan = lend_once("ipad-lea").await;
        assert_eq!(last_tile(&s, &loan).await["name"], "Celeste");
        finish(&s, &loan.id, Reason::Left).await;
        let loan = lend_once("tv-max").await;
        assert!(last_tile(&s, &loan).await.is_null());
        finish(&s, &loan.id, Reason::Left).await;
        // L'app retirée des apps permises : plus de grande tuile.
        s.settings.lock().unwrap().lend.apps.clear();
        let loan = lend_once("ipad-lea").await;
        assert!(last_tile(&s, &loan).await.is_null());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[tokio::test]
    async fn un_code_de_seance_puis_rendre_l_ordinateur() {
        let (s, dir) = station();
        let Json(view) = local_code(State(s.clone()), local(), admin(&s), Bytes::from(r#"{"duration":7200,"steam":"host"}"#)).await.ok().unwrap();
        let code = view["code"]["code"].as_str().unwrap().to_string();
        assert_eq!(view["code"]["duration"], 7200);
        assert!(view["code"]["web"].as_str().unwrap().ends_with(&format!("/link/?join={code}")));
        // Un code faux, puis le bon.
        let (status, v) = body(borrow(State(s.clone()), peer(20), None, borrow_code("000000", "Léa", "ipad")).await).await;
        assert_eq!((status, v["error"].clone()), (StatusCode::NOT_FOUND, json!("no_code")));
        let (status, lent) = body(borrow(State(s.clone()), peer(20), None, borrow_code(&code, "Léa", "ipad")).await).await;
        assert_eq!(status, StatusCode::OK, "{lent}");
        assert_eq!(lent["steam"], "host");
        assert!(lent["until"].as_u64().unwrap() >= now_secs() + 7199);
        assert_eq!(lent["display"]["width"], 2560);
        assert!(lent["kyber"]["token"].is_string());
        assert_eq!(s.lend.view(true)["state"], "lent");
        assert!(s.lend.code().is_none(), "le code sert une fois");
        // Pendant le prêt : pas de nouveau code, et un autre ami trouve l'ordinateur prêté.
        assert!(local_code(State(s.clone()), local(), admin(&s), Bytes::new()).await.is_err());
        let (status, v) = body(borrow(State(s.clone()), peer(21), None, borrow_code("123456", "Max", "tv")).await).await;
        assert!(matches!(status, StatusCode::NOT_FOUND | StatusCode::CONFLICT), "{v}");
        // Signe de vie, puis « Rendre l'ordinateur ».
        let (id, secret) = (lent["lend"].as_str().unwrap().to_string(), lent["secret"].as_str().unwrap().to_string());
        let (status, b) = body(beat(State(s.clone()), secret_headers(&secret), None, Json(LendBody { lend: Some(id.clone()), request: None })).await).await;
        assert_eq!(status, StatusCode::OK);
        assert!(b["token"].is_string() && b["end"].is_null());
        let res = leave(State(s.clone()), secret_headers(&secret), None, Json(LendBody { lend: Some(id.clone()), request: None })).await;
        assert_eq!(res.status(), StatusCode::OK);
        for _ in 0..50 {
            if s.lend.loan().is_none() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        let (_, b) = body(beat(State(s.clone()), secret_headers(&secret), None, Json(LendBody { lend: Some(id), request: None })).await).await;
        assert_eq!(b["end"], "left");
        assert!(s.activity.events(5, crate::i18n::Lang::Fr).iter().any(|e| e.text.contains("Léa")));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[tokio::test]
    async fn un_compte_d_ami_et_l_accord_de_l_hote() {
        let (s, dir) = station();
        let friend = s.friends.create(&lend::FriendBody { name: Some("Léa".into()), login: Some("lea".into()), ..Default::default() }, lend::hash_friend_password("secret-lea").unwrap()).unwrap();
        let login = |password: &str| Json(serde_json::from_value::<BorrowBody>(json!({ "login": "lea", "password": password, "name": "x", "device": "ipad", "app": "ios" })).unwrap());
        let (status, v) = body(borrow(State(s.clone()), peer(30), None, login("faux")).await).await;
        assert_eq!((status, v["error"].clone()), (StatusCode::UNAUTHORIZED, json!("bad_credentials")));
        // L'état de la carte.
        let (status, st) = body(super::status(State(s.clone()), peer(30), Json(StatusBody { login: "lea".into(), password: "secret-lea".into() })).await).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(st["state"], "free");
        // L'ordinateur utilisé (un appareil en session) : il faut l'accord de l'hôte.
        s.activity.begin(Some(crate::activity::DeviceInfo { id: "mac-1".into(), name: "MacBook".into(), kind: Some("mac".into()), os: None, app: None }), "mac", None, None);
        let (status, asked) = body(borrow(State(s.clone()), peer(30), None, login("secret-lea")).await).await;
        assert_eq!(status, StatusCode::ACCEPTED, "{asked}");
        assert!(asked["host"].is_string() && asked["computer"].is_string(), "{asked}");
        let (request, secret) = (asked["request"].as_str().unwrap().to_string(), asked["secret"].as_str().unwrap().to_string());
        assert_eq!(view(&s)["requests"][0]["name"], "Léa");
        // L'hôte refuse.
        let Json(_) = local_answer(State(s.clone()), local(), admin(&s), Path(request.clone()), Json(AnswerBody { accept: false, steam: None })).await.ok().unwrap();
        let (status, v) = body(wait(State(s.clone()), secret_headers(&secret), None, Json(WaitBody { request: Some(request), lend: None })).await).await;
        assert_eq!((status, v["error"].clone()), (StatusCode::FORBIDDEN, json!("refused")));
        // Une nouvelle demande, que l'ami annule (« Annuler ») : l'hôte ne la voit plus.
        let (status, asked) = body(borrow(State(s.clone()), peer(30), None, login("secret-lea")).await).await;
        assert_eq!(status, StatusCode::ACCEPTED, "{asked}");
        let (request, secret) = (asked["request"].as_str().unwrap().to_string(), asked["secret"].as_str().unwrap().to_string());
        let (status, _) = body(leave(State(s.clone()), secret_headers("faux"), None, Json(LendBody { lend: None, request: Some(request.clone()) })).await).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        let (status, _) = body(leave(State(s.clone()), secret_headers(&secret), None, Json(LendBody { lend: None, request: Some(request.clone()) })).await).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(view(&s)["requests"].as_array().map(Vec::len), Some(0));
        let (status, _) = body(wait(State(s.clone()), secret_headers(&secret), None, Json(WaitBody { request: Some(request), lend: None })).await).await;
        assert_ne!(status, StatusCode::OK);
        assert_eq!(friend.login, "lea");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[tokio::test]
    async fn le_code_de_pret_dit_le_relais_et_l_invitation_le_prenom() {
        let (s, dir) = station();
        let Json(v) = local_code(State(s.clone()), local(), admin(&s), Bytes::new()).await.ok().unwrap();
        let code = v["code"]["code"].as_str().unwrap().to_string();
        // Relais réglé (le public, par défaut), agent pas encore joint : `connecting`, liens du réseau.
        assert_eq!(v["code"]["relay"]["state"], "connecting");
        assert_eq!(v["code"]["web"], v["code"]["web_local"]);
        assert!(v["code"]["web_local"].as_str().unwrap().ends_with(&format!("/link/?join={code}")));
        assert!(!v["code"]["link"].as_str().unwrap().contains("&relay="));
        // Pas de relais : `null`.
        s.settings.lock().unwrap().relay.server = Some(String::new());
        assert!(view(&s)["code"]["relay"].is_null());
        // L'invitation d'un ami porte son identifiant et le prénom de l'hôte.
        let friend = s.friends.create(&lend::FriendBody { name: Some("Léa".into()), login: Some("lea".into()), ..Default::default() }, lend::hash_friend_password("secret-lea").unwrap()).unwrap();
        let Json(invite) = friend_invite(State(s.clone()), Path(friend.id.clone())).await.ok().unwrap();
        let name = format!("&name={}", urlencoding::encode(&host_name()));
        let (link, web) = (invite["link"].as_str().unwrap(), invite["web"].as_str().unwrap());
        assert!(link.starts_with("kybernow://borrow?") && link.contains("&login=lea") && link.ends_with(&name), "{link}");
        assert!(web.contains("/link/?borrow=lea&name=") && web.ends_with(&name), "{web}");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[tokio::test]
    async fn pret_coupe_ou_deja_prete() {
        let (s, dir) = station();
        s.settings.lock().unwrap().lend.enabled = false;
        assert!(local_code(State(s.clone()), local(), admin(&s), Bytes::new()).await.is_err());
        s.settings.lock().unwrap().lend.enabled = true;
        let Json(view) = local_code(State(s.clone()), local(), admin(&s), Bytes::new()).await.ok().unwrap();
        let code = view["code"]["code"].as_str().unwrap().to_string();
        assert!(s.lend.is_lend_code(&code));
        let Json(view) = local_code_cancel(State(s.clone()), local(), admin(&s)).await.ok().unwrap();
        assert!(view["code"].is_null());
        // Hors de la boucle locale et sans session d'Admin : refusé.
        assert!(local_get(State(s.clone()), peer(5), HeaderMap::new()).await.is_err());
        let _ = std::fs::remove_dir_all(dir);
    }
}
