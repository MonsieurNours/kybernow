//! La clé d'Admin à distance (docs/relais.md, « Clé d'Admin à distance », STWIN-17) : une passkey
//! enregistrée **auprès de la Station elle-même**, sur son nom de bout en bout
//! (`<id>.station.share.kybernow.app`), que la Station vérifie à chaque ouverture d'Admin à distance,
//! en plus du jeton `via: "relay"` du relais.
//!
//! Le relais signe ce jeton seul ; une clé que la Station vérifie elle-même, sur un nom dont le relais
//! n'a pas le certificat, est ce qu'un relais compromis ne peut pas fabriquer :
//!
//! - **Ajout, sur place seulement** : Admin, ouvert sur le réseau (jamais par le relais), tire un code à
//!   usage unique (10 minutes) et donne `https://<nom>/api/login/key/add#code=…` (QR code dans
//!   l'interface). La page, servie par la Station de bout en bout, crée la passkey (WebAuthn, `rp.id` =
//!   ce nom, vérification de l'utilisateur exigée) ; le code n'est jamais envoyé au relais en clair (il
//!   voyage dans le fragment, puis dans le TLS de la Station).
//! - **Ouverture à distance** : le jeton du relais vérifié (droit Administrer, passkey du compte, `jti`
//!   lié au navigateur), la Station demande en plus sa clé (`navigator.credentials.get`, défi tiré par
//!   elle) ; la session ne s'ouvre qu'avec les deux.
//!
//! Seulement sur le chemin de bout en bout (`browser_tls`) : sur l'ancien chemin, déchiffré par le relais,
//! une clé ne protégerait rien (le relais voit la session) ; une Station qui a été de bout en bout le
//! refuse déjà (`remote_admin_e2e`).

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use axum::Json;
use axum::extract::{Path as UrlPath, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{IntoResponse, Response};
use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use kybernow_tunnel::StreamKind;
use kybernow_tunnel::webauthn::{self, Assertion, Rp, StoredKey};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::AppState;
use crate::i18n::Msg;
use crate::relay::Via;

/// Le fichier des clés, dans le dossier des données d'Admin.
pub const FILE: &str = "relay/remote-keys.json";
/// Une Station garde 10 clés au plus.
pub const MAX_KEYS: usize = 10;
/// Un lien d'ajout vaut 10 minutes, une fois.
pub const ENROLL_TTL: Duration = Duration::from_secs(10 * 60);
/// Une ouverture attend la clé 2 minutes au plus.
pub const LOGIN_TTL: Duration = Duration::from_secs(2 * 60);
/// Ajouts et ouvertures en cours, au plus.
const MAX_PENDING: usize = 16;
/// Le cookie d'une ouverture qui attend la clé (`__Host-` : ce nom seul, `Path=/`).
pub const LOGIN_COOKIE: &str = "__Host-kyber_admin_key";
/// Le délai de la cérémonie côté navigateur.
const CEREMONY_MS: u64 = 120_000;

/// Une clé d'Admin à distance.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Key {
    /// L'identifiant de la passkey (base64url).
    pub id: String,
    /// L'appareil qui l'a ajoutée (« iPhone », « Mac »…), lu dans son navigateur.
    pub name: String,
    /// Le nom de bout en bout pour lequel elle vaut (`rp.id`).
    pub rp: String,
    pub alg: i64,
    /// La clé publique (base64url), sous la forme de `kybernow_tunnel::webauthn::PublicKey`.
    pub public_key: String,
    pub counter: i64,
    /// Secondes Unix.
    pub added: u64,
    #[serde(default)]
    pub used: Option<u64>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct Stored {
    /// L'identifiant WebAuthn de l'utilisateur (16 octets, base64url), le même pour toutes les clés :
    /// ajouter de nouveau sur le même authentificateur remplace la clé au lieu d'en créer une autre.
    #[serde(default)]
    user: String,
    #[serde(default)]
    keys: Vec<Key>,
}

struct Enroll {
    until: Instant,
    challenge: Option<[u8; 32]>,
}

struct Pending {
    until: Instant,
    challenge: [u8; 32],
    /// Le compte KyberNow (identifiant public) et son nom, d'après le jeton du relais.
    account: String,
    name: String,
}

/// Les clés de la Station, et les ajouts et ouvertures en cours (en mémoire).
pub struct RemoteKeys {
    path: PathBuf,
    stored: Mutex<Stored>,
    enrolls: Mutex<HashMap<String, Enroll>>,
    pending: Mutex<HashMap<String, Pending>>,
}

/// Pourquoi une clé est refusée.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refusal {
    /// Code ou ouverture inconnus, déjà servis ou échus.
    Expired,
    /// Pas sur le nom de bout en bout de la Station.
    WrongPath,
    /// La réponse du navigateur ne se vérifie pas.
    Invalid,
    /// Déjà [`MAX_KEYS`] clés.
    Full,
    /// Aucune clé pour ce nom.
    NoKey,
}

impl Refusal {
    pub fn code(self) -> &'static str {
        match self {
            Refusal::Expired => "expired",
            Refusal::WrongPath => "wrong_path",
            Refusal::Invalid => "invalid",
            Refusal::Full => "full",
            Refusal::NoKey => "no_key",
        }
    }

    fn msg(self, adding: bool) -> Msg {
        Msg::new(match (self, adding) {
            (Refusal::Expired, true) => "remoteKey.addExpired",
            (Refusal::Expired, false) => "remoteKey.loginExpired",
            (Refusal::WrongPath, _) => "remoteKey.wrongPath",
            (Refusal::Invalid, true) => "remoteKey.addFailed",
            (Refusal::Invalid, false) => "remoteKey.loginFailed",
            (Refusal::Full, _) => "remoteKey.full",
            (Refusal::NoKey, _) => "remoteKey.none",
        })
    }

    fn status(self) -> StatusCode {
        match self {
            Refusal::Expired => StatusCode::GONE,
            Refusal::Full => StatusCode::CONFLICT,
            _ => StatusCode::FORBIDDEN,
        }
    }
}

fn b64(bytes: &[u8]) -> String {
    URL_SAFE_NO_PAD.encode(bytes)
}

fn unb64(s: &str) -> Option<Vec<u8>> {
    URL_SAFE_NO_PAD.decode(s.trim()).ok()
}

fn now_secs() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

/// 16 octets au hasard, en base64url.
fn token() -> String {
    b64(&rand::random::<[u8; 16]>())
}

/// Garde au plus [`MAX_PENDING`] entrées encore valables (la plus ancienne part).
fn trim<T>(map: &mut HashMap<String, T>, until: impl Fn(&T) -> Instant) {
    let now = Instant::now();
    map.retain(|_, v| until(v) > now);
    while map.len() >= MAX_PENDING {
        let Some(oldest) = map.iter().min_by_key(|(_, v)| until(v)).map(|(k, _)| k.clone()) else { break };
        map.remove(&oldest);
    }
}

/// L'appareil d'après le `User-Agent` : ce que l'hôte reconnaîtra dans la liste.
pub fn device_of(user_agent: &str) -> String {
    let ua = user_agent.to_ascii_lowercase();
    let name = if ua.contains("iphone") {
        "iPhone"
    } else if ua.contains("ipad") {
        "iPad"
    } else if ua.contains("android") {
        "Android"
    } else if ua.contains("cros") {
        "Chromebook"
    } else if ua.contains("macintosh") || ua.contains("mac os") {
        "Mac"
    } else if ua.contains("windows") {
        "Windows"
    } else if ua.contains("linux") {
        "Linux"
    } else {
        return Msg::new("remoteKey.browser").text();
    };
    name.to_string()
}

impl RemoteKeys {
    pub fn load(data_dir: &Path) -> RemoteKeys {
        let path = data_dir.join(FILE);
        let mut stored: Stored = crate::store::load_json(&path);
        if unb64(&stored.user).is_none_or(|u| u.len() != 16) {
            stored.user = token();
        }
        RemoteKeys { path, stored: Mutex::new(stored), enrolls: Mutex::new(HashMap::new()), pending: Mutex::new(HashMap::new()) }
    }

    fn save(&self, stored: &Stored) {
        let bytes = match serde_json::to_vec_pretty(stored) {
            Ok(b) => b,
            Err(e) => return tracing::warn!("clés d'Admin à distance illisibles : {e}"),
        };
        if let Some(dir) = self.path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        if let Err(e) = crate::durable::write(&self.path, bytes) {
            tracing::warn!("clés d'Admin à distance : {} pas écrit : {e}", self.path.display());
        }
    }

    /// Les clés, pour `GET /api/relay` : `current` dit si elle vaut pour le nom de bout en bout du moment.
    pub fn view(&self, rp: Option<&str>) -> Value {
        let stored = self.stored.lock().unwrap();
        let keys: Vec<Value> = stored
            .keys
            .iter()
            .map(|k| json!({ "id": k.id, "name": k.name, "added": k.added, "used": k.used, "current": rp == Some(k.rp.as_str()) }))
            .collect();
        Value::Array(keys)
    }

    /// Une clé au moins pour ce nom ?
    pub fn has_keys(&self, rp: &str) -> bool {
        self.stored.lock().unwrap().keys.iter().any(|k| k.rp == rp)
    }

    /// Retire une clé ; faux si elle est inconnue.
    pub fn remove(&self, id: &str) -> Option<Key> {
        let mut stored = self.stored.lock().unwrap();
        let at = stored.keys.iter().position(|k| k.id == id)?;
        let key = stored.keys.remove(at);
        self.save(&stored);
        Some(key)
    }

    // --- Ajout ---

    /// Un code d'ajout, valable [`ENROLL_TTL`], une fois.
    pub fn new_enroll(&self) -> String {
        let code = token();
        let mut enrolls = self.enrolls.lock().unwrap();
        trim(&mut enrolls, |e| e.until);
        enrolls.insert(code.clone(), Enroll { until: Instant::now() + ENROLL_TTL, challenge: None });
        code
    }

    /// Les options de `navigator.credentials.create()` pour ce code (le défi tiré ici et gardé).
    pub fn enroll_options(&self, code: &str, rp: &str, station: &str) -> Result<Value, Refusal> {
        let mut enrolls = self.enrolls.lock().unwrap();
        let e = enrolls.get_mut(code).filter(|e| e.until > Instant::now()).ok_or(Refusal::Expired)?;
        let challenge = rand::random::<[u8; 32]>();
        e.challenge = Some(challenge);
        let stored = self.stored.lock().unwrap();
        if stored.keys.len() >= MAX_KEYS {
            return Err(Refusal::Full);
        }
        let exclude: Vec<Value> = stored.keys.iter().filter(|k| k.rp == rp).map(|k| json!({ "type": "public-key", "id": k.id })).collect();
        let label = Msg::new("remoteKey.userName").with("station", station).text();
        Ok(json!({
            "rp": { "id": rp, "name": "KyberNow Admin" },
            "user": { "id": stored.user, "name": label, "displayName": label },
            "challenge": b64(&challenge),
            "pubKeyCredParams": [
                { "type": "public-key", "alg": webauthn::ES256 },
                { "type": "public-key", "alg": webauthn::ED25519 },
                { "type": "public-key", "alg": webauthn::RS256 }
            ],
            "authenticatorSelection": { "residentKey": "preferred", "userVerification": "required" },
            "attestation": "none",
            "timeout": CEREMONY_MS,
            "excludeCredentials": exclude,
        }))
    }

    /// Vérifie la passkey créée et la garde ; le code ne sert plus.
    pub fn enroll_finish(&self, code: &str, rp: &Rp, client_data: &[u8], attestation: &[u8], name: &str) -> Result<Key, Refusal> {
        let challenge = {
            let mut enrolls = self.enrolls.lock().unwrap();
            let e = enrolls.remove(code).filter(|e| e.until > Instant::now()).ok_or(Refusal::Expired)?;
            e.challenge.ok_or(Refusal::Expired)?
        };
        let got = webauthn::client_data(rp, client_data, "webauthn.create").map_err(|_| Refusal::Invalid)?;
        if !webauthn::ct_eq(got.as_bytes(), b64(&challenge).as_bytes()) {
            return Err(Refusal::Invalid);
        }
        let reg = webauthn::verify_registration(rp, attestation).map_err(|_| Refusal::Invalid)?;
        let key = Key {
            id: b64(&reg.credential),
            name: name.chars().filter(|c| !c.is_control()).take(40).collect(),
            rp: rp.id.clone(),
            alg: reg.alg,
            public_key: b64(&reg.public_key),
            counter: reg.counter,
            added: now_secs(),
            used: None,
        };
        let mut stored = self.stored.lock().unwrap();
        stored.keys.retain(|k| k.id != key.id);
        if stored.keys.len() >= MAX_KEYS {
            return Err(Refusal::Full);
        }
        stored.keys.push(key.clone());
        self.save(&stored);
        Ok(key)
    }

    // --- Ouverture ---

    /// Une ouverture qui attend la clé, après le jeton du relais ; rend son identifiant (cookie).
    pub fn begin_login(&self, account: &str, name: &str) -> String {
        let id = token();
        let mut pending = self.pending.lock().unwrap();
        trim(&mut pending, |p| p.until);
        pending.insert(id.clone(), Pending { until: Instant::now() + LOGIN_TTL, challenge: rand::random(), account: account.into(), name: name.into() });
        id
    }

    /// Les options de `navigator.credentials.get()` pour cette ouverture.
    pub fn login_options(&self, pending: &str, rp: &str) -> Result<Value, Refusal> {
        let challenge = self.pending.lock().unwrap().get(pending).filter(|p| p.until > Instant::now()).map(|p| p.challenge).ok_or(Refusal::Expired)?;
        let allow: Vec<Value> = self.stored.lock().unwrap().keys.iter().filter(|k| k.rp == rp).map(|k| json!({ "type": "public-key", "id": k.id })).collect();
        if allow.is_empty() {
            return Err(Refusal::NoKey);
        }
        Ok(json!({ "rpId": rp, "challenge": b64(&challenge), "allowCredentials": allow, "userVerification": "required", "timeout": CEREMONY_MS }))
    }

    /// Vérifie la clé ; rend le compte et le nom du jeton du relais. L'ouverture ne sert qu'une fois.
    pub fn finish_login(&self, pending: &str, rp: &Rp, a: &Assertion) -> Result<(String, String), Refusal> {
        let p = self.pending.lock().unwrap().remove(pending).filter(|p| p.until > Instant::now()).ok_or(Refusal::Expired)?;
        let got = webauthn::client_data(rp, &a.client_data, "webauthn.get").map_err(|_| Refusal::Invalid)?;
        if !webauthn::ct_eq(got.as_bytes(), b64(&p.challenge).as_bytes()) {
            return Err(Refusal::Invalid);
        }
        let mut stored = self.stored.lock().unwrap();
        let user = unb64(&stored.user).unwrap_or_default();
        let id = b64(&a.credential);
        let key = stored.keys.iter_mut().find(|k| k.id == id && k.rp == rp.id).ok_or(Refusal::Invalid)?;
        let public_key = unb64(&key.public_key).ok_or(Refusal::Invalid)?;
        let counter = webauthn::verify_assertion(rp, &StoredKey { alg: key.alg, public_key: &public_key, counter: key.counter }, &user, a).map_err(|_| Refusal::Invalid)?;
        key.counter = counter;
        key.used = Some(now_secs());
        self.save(&stored);
        Ok((p.account, p.name))
    }
}

// --- Routes ------------------------------------------------------------------------

/// Le nom de bout en bout de la Station, s'il est prêt (certificat valable pour le nom annoncé).
pub fn e2e_name(s: &AppState) -> Option<String> {
    s.relay.tls.ready().then(|| s.relay.tls.name()).flatten()
}

/// La requête arrive-t-elle de bout en bout, sur ce nom ? Rend l'identité WebAuthn de la Station.
fn rp_for(s: &AppState, via: Option<&Via>, headers: &HeaderMap) -> Result<Rp, Refusal> {
    let name = e2e_name(s).ok_or(Refusal::WrongPath)?;
    let host = headers.get(header::HOST).and_then(|h| h.to_str().ok()).unwrap_or("").trim().to_ascii_lowercase();
    if via.map(|v| v.kind) != Some(StreamKind::BrowserTls) || host != name {
        return Err(Refusal::WrongPath);
    }
    if s.settings.lock().unwrap().relay.e2e_suspended.is_some() {
        return Err(Refusal::WrongPath);
    }
    Ok(Rp { origin: format!("https://{name}"), id: name })
}

fn refused(r: Refusal, adding: bool) -> Response {
    (r.status(), Json(json!({ "error": r.msg(adding).text(), "code": r.code() }))).into_response()
}

/// `POST /api/relay/keys` (session d'Admin, sur le réseau seulement) : un lien d'ajout.
pub async fn create(State(s): State<AppState>, via: Option<axum::Extension<Via>>) -> Response {
    if via.is_some() {
        return (StatusCode::FORBIDDEN, Json(json!({ "error": Msg::new("remoteKey.onSite").text(), "code": "network_only" }))).into_response();
    }
    let Some(name) = e2e_name(&s) else {
        return (StatusCode::CONFLICT, Json(json!({ "error": Msg::new("remoteKey.notReady").text(), "code": "not_ready" }))).into_response();
    };
    let code = s.accounts.remote_keys.new_enroll();
    tracing::info!("relais : lien d'ajout d'une clé d'Admin à distance tiré");
    Json(json!({ "url": format!("https://{name}/api/login/key/add#code={code}"), "expires": ENROLL_TTL.as_secs() })).into_response()
}

/// `DELETE /api/relay/keys/{id}` (session d'Admin) : retire une clé ; la vue du relais.
pub async fn delete(State(s): State<AppState>, UrlPath(id): UrlPath<String>) -> Response {
    let Some(key) = s.accounts.remote_keys.remove(&id) else {
        return (StatusCode::NOT_FOUND, Json(json!({ "error": Msg::new("remoteKey.unknown").text(), "code": "unknown" }))).into_response();
    };
    tracing::info!("relais : clé d'Admin à distance retirée ({})", key.name);
    s.activity.event("info", Msg::new("events.remoteKeyRemoved").with("name", key.name));
    Json(s.relay.status(&s)).into_response()
}

#[derive(Deserialize)]
pub struct CodeBody {
    #[serde(default)]
    code: String,
}

/// `POST /api/login/key/add/options {code}` (de bout en bout) : les options de création.
pub async fn add_options(State(s): State<AppState>, via: Option<axum::Extension<Via>>, headers: HeaderMap, Json(body): Json<CodeBody>) -> Response {
    let rp = match rp_for(&s, via.as_deref(), &headers) {
        Ok(rp) => rp,
        Err(r) => return refused(r, true),
    };
    let station = crate::system::computer_name().unwrap_or_default();
    match s.accounts.remote_keys.enroll_options(body.code.trim(), &rp.id, &station) {
        Ok(options) => Json(options).into_response(),
        Err(r) => refused(r, true),
    }
}

#[derive(Deserialize)]
pub struct AddBody {
    #[serde(default)]
    code: String,
    #[serde(default)]
    client_data: String,
    #[serde(default)]
    attestation: String,
}

/// `POST /api/login/key/add {code, client_data, attestation}` (de bout en bout) : garde la clé.
pub async fn add(State(s): State<AppState>, via: Option<axum::Extension<Via>>, headers: HeaderMap, Json(body): Json<AddBody>) -> Response {
    let rp = match rp_for(&s, via.as_deref(), &headers) {
        Ok(rp) => rp,
        Err(r) => return refused(r, true),
    };
    let (Some(client), Some(object)) = (unb64(&body.client_data), unb64(&body.attestation)) else {
        return refused(Refusal::Invalid, true);
    };
    let name = device_of(headers.get(header::USER_AGENT).and_then(|v| v.to_str().ok()).unwrap_or(""));
    match s.accounts.remote_keys.enroll_finish(body.code.trim(), &rp, &client, &object, &name) {
        Ok(key) => {
            tracing::info!("relais : clé d'Admin à distance ajoutée ({})", key.name);
            s.activity.event("info", Msg::new("events.remoteKeyAdded").with("name", key.name.clone()));
            Json(json!({ "ok": true, "name": key.name })).into_response()
        }
        Err(r) => {
            tracing::warn!("relais : clé d'Admin à distance refusée ({})", r.code());
            refused(r, true)
        }
    }
}

/// `GET /api/login/key/add` (de bout en bout) : la page d'ajout ; le code est dans le fragment.
pub async fn add_page(State(s): State<AppState>, via: Option<axum::Extension<Via>>, headers: HeaderMap) -> Response {
    if let Err(r) = rp_for(&s, via.as_deref(), &headers) {
        return page(&Msg::new("remoteKey.addTitle").text(), &r.msg(true).text(), None, r.status());
    }
    page(&Msg::new("remoteKey.addTitle").text(), &Msg::new("remoteKey.addText").text(), Some(Ceremony::Add), StatusCode::OK)
}

/// L'ouverture à distance attend la clé : la page de la cérémonie et le cookie de cette ouverture.
/// Appelé par le retour de « Ouvrir Admin » une fois le jeton du relais vérifié.
pub fn login_page(s: &AppState, account: &str, name: &str, clear: String) -> Response {
    let Some(rp) = e2e_name(s) else {
        return with_cookies(page(&Msg::new("remoteKey.loginTitle").text(), &Refusal::WrongPath.msg(false).text(), None, StatusCode::FORBIDDEN), vec![clear]);
    };
    if !s.accounts.remote_keys.has_keys(&rp) {
        tracing::info!("comptes : Admin à distance refusé, aucune clé d'Admin à distance sur cette Station");
        return with_cookies(page(&Msg::new("remoteKey.loginTitle").text(), &Refusal::NoKey.msg(false).text(), None, StatusCode::FORBIDDEN), vec![clear]);
    }
    let pending = s.accounts.remote_keys.begin_login(account, name);
    let cookie = format!("{LOGIN_COOKIE}={pending}; Path=/; Secure; HttpOnly; SameSite=Strict; Max-Age={}", LOGIN_TTL.as_secs());
    with_cookies(page(&Msg::new("remoteKey.loginTitle").text(), &Msg::new("remoteKey.loginText").text(), Some(Ceremony::Login), StatusCode::OK), vec![clear, cookie])
}

fn pending_of(headers: &HeaderMap) -> Option<String> {
    headers
        .get_all(header::COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .flat_map(|v| v.split(';'))
        .filter_map(|c| c.trim().split_once('='))
        .find(|(k, _)| *k == LOGIN_COOKIE)
        .map(|(_, v)| v.to_string())
}

/// `POST /api/login/key/options` (de bout en bout, cookie de l'ouverture) : les options de la clé.
pub async fn login_options(State(s): State<AppState>, via: Option<axum::Extension<Via>>, headers: HeaderMap) -> Response {
    let rp = match rp_for(&s, via.as_deref(), &headers) {
        Ok(rp) => rp,
        Err(r) => return refused(r, false),
    };
    let Some(pending) = pending_of(&headers) else { return refused(Refusal::Expired, false) };
    match s.accounts.remote_keys.login_options(&pending, &rp.id) {
        Ok(options) => Json(options).into_response(),
        Err(r) => refused(r, false),
    }
}

#[derive(Deserialize)]
pub struct LoginBody {
    #[serde(default)]
    id: String,
    #[serde(default)]
    client_data: String,
    #[serde(default)]
    authenticator: String,
    #[serde(default)]
    signature: String,
    #[serde(default)]
    user: String,
}

/// `POST /api/login/key {id, client_data, authenticator, signature, user}` (de bout en bout, cookie de
/// l'ouverture) : la clé vérifiée, la session d'Admin à distance s'ouvre.
pub async fn login(State(s): State<AppState>, via: Option<axum::Extension<Via>>, headers: HeaderMap, Json(body): Json<LoginBody>) -> Response {
    let clear = format!("{LOGIN_COOKIE}=; Path=/; Secure; HttpOnly; SameSite=Strict; Max-Age=0");
    let rp = match rp_for(&s, via.as_deref(), &headers) {
        Ok(rp) => rp,
        Err(r) => return with_cookies(refused(r, false), vec![clear]),
    };
    if let Err((code, msg)) = crate::relay::remote_admin_for(&s, StreamKind::BrowserTls) {
        return with_cookies((StatusCode::FORBIDDEN, Json(json!({ "error": msg.text(), "code": code }))).into_response(), vec![clear]);
    }
    let Some(pending) = pending_of(&headers) else { return refused(Refusal::Expired, false) };
    let parts = (unb64(&body.id), unb64(&body.client_data), unb64(&body.authenticator), unb64(&body.signature));
    let (Some(credential), Some(client_data), Some(authenticator), Some(signature)) = parts else {
        return with_cookies(refused(Refusal::Invalid, false), vec![clear]);
    };
    let a = Assertion { credential, client_data, authenticator, signature, user: unb64(&body.user).unwrap_or_default() };
    match s.accounts.remote_keys.finish_login(&pending, &rp, &a) {
        Ok((account, name)) => {
            tracing::info!("comptes : Admin ouvert à distance avec KyberNow et la clé d'Admin à distance");
            s.activity.event("warn", Msg::new("events.accountAdminRemote").with("name", name.clone()));
            s.relay.admin_opened(&account, &name);
            let session = s.auth.open_remote_session(&name, true);
            with_cookies(Json(json!({ "ok": true })).into_response(), vec![crate::auth::remote_session_cookie(&session), clear])
        }
        Err(r) => {
            tracing::warn!("comptes : clé d'Admin à distance refusée ({})", r.code());
            with_cookies(refused(r, false), vec![clear])
        }
    }
}

fn with_cookies(mut res: Response, cookies: Vec<String>) -> Response {
    for c in cookies {
        if let Ok(v) = header::HeaderValue::from_str(&c) {
            res.headers_mut().append(header::SET_COOKIE, v);
        }
    }
    res
}

// --- Pages --------------------------------------------------------------------------

/// La cérémonie d'une page.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Ceremony {
    Add,
    Login,
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;").replace('\'', "&#39;")
}

/// Le script des deux cérémonies : base64url ↔ octets, `navigator.credentials`, puis le POST.
const SCRIPT: &str = r#"(function(){var d=document.body.dataset,k=d.kind,st=document.getElementById('st'),b=document.getElementById('go');
function u8(s){s=s.replace(/-/g,'+').replace(/_/g,'/');s+='==='.slice((s.length+3)%4);var x=atob(s),o=new Uint8Array(x.length);for(var i=0;i<x.length;i++)o[i]=x.charCodeAt(i);return o;}
function s64(b){b=new Uint8Array(b);var x='';for(var i=0;i<b.length;i++)x+=String.fromCharCode(b[i]);return btoa(x).replace(/\+/g,'-').replace(/\//g,'_').replace(/=+$/,'');}
function say(t,bad){st.textContent=t;st.className=bad?'bad':'';}
function post(p,o){return fetch(p,{method:'POST',credentials:'same-origin',headers:{'content-type':'application/json','x-kyber-admin':'1'},body:JSON.stringify(o)}).then(function(r){return r.json().then(function(j){if(!r.ok)throw new Error(j.error||d.failed);return j;});});}
var code=new URLSearchParams(location.hash.slice(1)).get('code')||'';if(location.hash)history.replaceState(null,'',location.pathname);
if(!window.PublicKeyCredential){say(d.unsupported,true);b.hidden=true;return;}
if(k==='add'&&!code){say(d.expired,true);b.hidden=true;return;}
b.onclick=function(){b.disabled=true;say(d.wait,false);
(k==='add'?post('/api/login/key/add/options',{code:code}).then(function(o){o.challenge=u8(o.challenge);o.user.id=u8(o.user.id);o.excludeCredentials.forEach(function(c){c.id=u8(c.id);});
return navigator.credentials.create({publicKey:o});}).then(function(c){return post('/api/login/key/add',{code:code,client_data:s64(c.response.clientDataJSON),attestation:s64(c.response.attestationObject)});}).then(function(){say(d.done,false);b.hidden=true;document.getElementById('home').hidden=false;})
:post('/api/login/key/options',{}).then(function(o){o.challenge=u8(o.challenge);o.allowCredentials.forEach(function(c){c.id=u8(c.id);});return navigator.credentials.get({publicKey:o});}).then(function(c){var r=c.response;
return post('/api/login/key',{id:s64(c.rawId),client_data:s64(r.clientDataJSON),authenticator:s64(r.authenticatorData),signature:s64(r.signature),user:r.userHandle?s64(r.userHandle):''});}).then(function(){location.replace('/');}))
.catch(function(e){b.disabled=false;say(e&&e.name==='NotAllowedError'?d.cancelled:(e&&e.message)||d.failed,true);});};})();"#;

/// La politique de contenu des deux pages : leur script seul (par son condensé), leurs requêtes vers
/// la Station, rien d'autre.
fn policy() -> String {
    let hash = base64::engine::general_purpose::STANDARD.encode(ring::digest::digest(&ring::digest::SHA256, SCRIPT.as_bytes()));
    format!("default-src 'none'; script-src 'sha256-{hash}'; style-src 'unsafe-inline'; connect-src 'self'; img-src 'none'; form-action 'none'; base-uri 'none'; frame-ancestors 'none'")
}

/// Une petite page servie par la Station (de bout en bout) : le titre, le texte et, s'il y a lieu, le
/// bouton de la cérémonie et son script. Rien d'ailleurs ; les textes sont dans la langue d'Admin.
fn page(title: &str, text: &str, ceremony: Option<Ceremony>, status: StatusCode) -> Response {
    let lang = crate::i18n::current().code();
    let t = |k: &str| esc(&Msg::new(k).text());
    let (data, button, script) = match ceremony {
        Some(c) => {
            let (kind, go, done) = match c {
                Ceremony::Add => ("add", t("remoteKey.addButton"), t("remoteKey.added")),
                Ceremony::Login => ("login", t("remoteKey.loginButton"), String::new()),
            };
            let failed = if c == Ceremony::Add { t("remoteKey.addFailed") } else { t("remoteKey.loginFailed") };
            let data = format!(
                " data-kind=\"{kind}\" data-wait=\"{}\" data-done=\"{done}\" data-failed=\"{failed}\" data-cancelled=\"{}\" data-expired=\"{}\" data-unsupported=\"{}\"",
                t("remoteKey.waiting"),
                t("remoteKey.cancelled"),
                t("remoteKey.addExpired"),
                t("remoteKey.unsupported"),
            );
            let button = format!("<p><button id=\"go\" type=\"button\">{go}</button></p><p id=\"home\" hidden><a href=\"/\">{}</a></p>", t("remoteKey.openAdmin"));
            (data, button, format!("<script>{SCRIPT}</script>"))
        }
        None => (String::new(), format!("<p><a href=\"/\">{}</a></p>", t("remoteKey.openAdmin")), String::new()),
    };
    let body = format!(
        "<!doctype html><html lang=\"{lang}\"><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width, initial-scale=1\"><meta name=\"robots\" content=\"noindex\"><meta name=\"referrer\" content=\"no-referrer\"><title>{title}</title>\
<style>body{{font:16px/1.5 system-ui,sans-serif;max-width:34em;margin:10vh auto;padding:0 20px;color:#E3EDF0;background:#05070A}}h1{{font-size:1.4em}}button{{font:inherit;font-weight:600;padding:.7em 1.4em;border:0;border-radius:10px;background:#9A8CFF;color:#05070A;cursor:pointer}}button:disabled{{opacity:.6}}a{{color:#E3EDF0}}.bad{{color:#FF8C8C}}</style></head>\
<body{data}><h1>{title}</h1><p>{text}</p><p id=\"st\" role=\"status\"></p>{button}{script}</body></html>",
        title = esc(title),
        text = esc(text),
    );
    let mut res = (status, [(header::CONTENT_TYPE, "text/html; charset=utf-8"), (header::CACHE_CONTROL, "no-store")], body).into_response();
    if let Ok(v) = header::HeaderValue::from_str(&policy()) {
        res.headers_mut().insert(header::CONTENT_SECURITY_POLICY, v);
    }
    res
}

#[cfg(test)]
mod tests {
    use super::*;
    use kybernow_tunnel::webauthn::testing::Authenticator;

    const NAME: &str = "abc.station.share.kybernow.app";

    fn keys() -> (RemoteKeys, PathBuf) {
        let dir = std::env::temp_dir().join(format!("kyber-admin-cles-{}", rand::random::<u64>()));
        std::fs::create_dir_all(&dir).unwrap();
        (RemoteKeys::load(&dir), dir)
    }

    fn rp() -> Rp {
        Rp { id: NAME.into(), origin: format!("https://{NAME}") }
    }

    /// Ajoute la clé d'un authentificateur logiciel ; rend la clé gardée.
    fn add(k: &RemoteKeys, auth: &Authenticator) -> Result<Key, Refusal> {
        let code = k.new_enroll();
        let options = k.enroll_options(&code, NAME, "PC du salon")?;
        let (client, object) = auth.create(options["challenge"].as_str().unwrap());
        k.enroll_finish(&code, &rp(), &client, &object, "iPhone")
    }

    #[test]
    fn ajout_une_fois_par_code_puis_garde_sur_le_disque() {
        let (k, dir) = keys();
        let auth = Authenticator::new(webauthn::ES256, &format!("https://{NAME}"));
        let code = k.new_enroll();
        let options = k.enroll_options(&code, NAME, "PC du salon").unwrap();
        assert_eq!(options["rp"]["id"], NAME);
        assert_eq!(options["authenticatorSelection"]["userVerification"], "required");
        assert_eq!(options["attestation"], "none");
        let (client, object) = auth.create(options["challenge"].as_str().unwrap());
        // Un autre défi que celui tiré : refusé, et le code a servi.
        let (other_client, _) = auth.create("pas-le-defi");
        assert_eq!(k.enroll_finish(&code, &rp(), &other_client, &object, "iPhone"), Err(Refusal::Invalid));
        assert_eq!(k.enroll_finish(&code, &rp(), &client, &object, "iPhone"), Err(Refusal::Expired));
        assert!(!k.has_keys(NAME));
        // Le bon parcours.
        let key = add(&k, &auth).unwrap();
        assert_eq!((key.rp.as_str(), key.name.as_str(), key.alg), (NAME, "iPhone", webauthn::ES256));
        assert!(k.has_keys(NAME) && !k.has_keys("autre.station.share.kybernow.app"));
        // Relue depuis le disque, avec le même identifiant d'utilisateur.
        let again = RemoteKeys::load(&dir);
        assert!(again.has_keys(NAME));
        assert_eq!(again.stored.lock().unwrap().user, k.stored.lock().unwrap().user);
        assert_eq!(again.view(Some(NAME))[0]["current"], true);
        assert_eq!(again.view(Some("autre"))[0]["current"], false);
        // Une passkey créée pour un autre nom (le relais, par exemple) : refusée.
        let relay_key = Authenticator::new(webauthn::ES256, "https://share.kybernow.app");
        assert_eq!(add(&k, &relay_key), Err(Refusal::Invalid));
        // Un code inconnu.
        assert_eq!(k.enroll_options("inconnu", NAME, "PC").unwrap_err(), Refusal::Expired);
        // Retirée.
        assert_eq!(k.remove(&key.id).map(|k| k.name), Some("iPhone".into()));
        assert!(!k.has_keys(NAME) && k.remove(&key.id).is_none());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn ouverture_avec_la_cle_seulement() {
        let (k, dir) = keys();
        let mut auth = Authenticator::new(webauthn::ES256, &format!("https://{NAME}"));
        add(&k, &auth).unwrap();
        let pending = k.begin_login("acc_lea", "Léa Martin");
        let options = k.login_options(&pending, NAME).unwrap();
        assert_eq!(options["allowCredentials"].as_array().unwrap().len(), 1);
        let a = auth.get(options["challenge"].as_str().unwrap());
        assert_eq!(k.finish_login(&pending, &rp(), &a), Ok(("acc_lea".into(), "Léa Martin".into())));
        // Une fois seulement.
        assert_eq!(k.finish_login(&pending, &rp(), &a), Err(Refusal::Expired));
        // Une autre clé (un relais qui en fabriquerait une) : refusée.
        let pending = k.begin_login("acc_lea", "Léa Martin");
        let challenge = k.login_options(&pending, NAME).unwrap()["challenge"].as_str().unwrap().to_string();
        let mut forged = Authenticator::new(webauthn::ES256, &format!("https://{NAME}"));
        assert_eq!(k.finish_login(&pending, &rp(), &forged.get(&challenge)), Err(Refusal::Invalid));
        // La bonne clé, mais pour un autre défi (rejouée) : refusée.
        let pending = k.begin_login("acc_lea", "Léa Martin");
        let _ = k.login_options(&pending, NAME).unwrap();
        assert_eq!(k.finish_login(&pending, &rp(), &auth.get("ancien-defi")), Err(Refusal::Invalid));
        // Sans clé pour ce nom : rien à demander.
        let pending = k.begin_login("acc_lea", "Léa Martin");
        assert_eq!(k.login_options(&pending, "autre.station.share.kybernow.app").unwrap_err(), Refusal::NoKey);
        // Le compteur et l'heure d'usage sont gardés.
        assert!(k.stored.lock().unwrap().keys[0].used.is_some());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn plafond_et_noms() {
        let (k, dir) = keys();
        for _ in 0..MAX_KEYS {
            add(&k, &Authenticator::new(webauthn::ED25519, &format!("https://{NAME}"))).unwrap();
        }
        let code = k.new_enroll();
        assert_eq!(k.enroll_options(&code, NAME, "PC").unwrap_err(), Refusal::Full);
        assert_eq!(device_of("Mozilla/5.0 (iPhone; CPU iPhone OS 18_0 like Mac OS X)"), "iPhone");
        assert_eq!(device_of("Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7)"), "Mac");
        assert_eq!(device_of("Mozilla/5.0 (Windows NT 10.0; Win64; x64)"), "Windows");
        assert_eq!(device_of("Mozilla/5.0 (Linux; Android 15; Pixel 9)"), "Android");
        // Les codes et ouvertures en cours restent bornés.
        for _ in 0..100 {
            k.new_enroll();
            k.begin_login("a", "b");
        }
        assert!(k.enrolls.lock().unwrap().len() <= MAX_PENDING && k.pending.lock().unwrap().len() <= MAX_PENDING);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn la_page_ne_charge_rien_d_ailleurs() {
        for bad in ["http:", "https:", "eval(", "innerHTML", "localStorage", "document.cookie", "import "] {
            assert!(!SCRIPT.contains(bad), "{bad}");
        }
        assert!(SCRIPT.contains("history.replaceState(null,'',location.pathname)"), "le code quitte l'adresse");
        assert!(SCRIPT.contains("'x-kyber-admin':'1'"));
    }
}

/// Le parcours entier par le routeur que sert l'agent du tunnel (garde du relais comprise) : ajout sur
/// place, puis ouverture à distance de bout en bout avec le jeton du relais **et** la clé.
#[cfg(all(test, not(windows)))]
mod parcours {
    use std::net::{IpAddr, Ipv6Addr, SocketAddr};

    use axum::body::Body;
    use axum::extract::ConnectInfo;
    use axum::http::Method;
    use kybernow_tunnel::StationIdentity;
    use kybernow_tunnel::access::{Claims, Names, Right};
    use kybernow_tunnel::webauthn::testing::Authenticator;

    use super::*;

    struct Call {
        router: axum::Router,
        host: String,
    }

    impl Call {
        async fn send(&self, method: Method, path: &str, kind: StreamKind, cookie: Option<&str>, body: Option<Value>) -> (StatusCode, HeaderMap, String) {
            let mut req = axum::http::Request::builder().method(method).uri(path).header("x-kyber-admin", "1").header(header::HOST, &self.host);
            req = req.header(header::USER_AGENT, "Mozilla/5.0 (iPhone; CPU iPhone OS 18_0 like Mac OS X)");
            if let Some(c) = cookie {
                req = req.header(header::COOKIE, c);
            }
            let body = match body {
                Some(b) => {
                    req = req.header(header::CONTENT_TYPE, "application/json");
                    Body::from(b.to_string())
                }
                None => Body::empty(),
            };
            let mut req = req.body(body).unwrap();
            req.extensions_mut().insert(ConnectInfo(SocketAddr::new(IpAddr::V6(Ipv6Addr::new(0x100, 0, 0, 0, 0, 0, 0, 1)), 0)));
            req.extensions_mut().insert(Via { kind, client: "c".into() });
            let res = tower::ServiceExt::oneshot(self.router.clone(), req).await.unwrap();
            let (status, headers) = (res.status(), res.headers().clone());
            let body = String::from_utf8(axum::body::to_bytes(res.into_body(), 1 << 20).await.unwrap().to_vec()).unwrap();
            (status, headers, body)
        }
    }

    /// Le cookie `nom=valeur` posé par une réponse.
    fn cookie(headers: &HeaderMap, name: &str) -> Option<String> {
        headers.get_all(header::SET_COOKIE).iter().filter_map(|v| v.to_str().ok()).find(|c| c.starts_with(&format!("{name}="))).map(|c| c.split(';').next().unwrap().to_string())
    }

    /// Ouverture à distance : le départ, puis le retour avec le jeton du relais (`via: "relay"`), puis la
    /// clé de `phone` (aucune : on s'arrête à la page qui la demande).
    async fn open(call: &Call, s: &AppState, signer: &kybernow_tunnel::access::Signer, name: &str, phone: Option<&mut Authenticator>) -> (StatusCode, String, Option<String>) {
        let e2e = StreamKind::BrowserTls;
        let (status, headers, _) = call.send(Method::GET, "/api/login/kybernow/start", e2e, None, None).await;
        assert_eq!(status, StatusCode::SEE_OTHER);
        let nonce_cookie = cookie(&headers, "__Host-kyber_admin_kn").unwrap();
        let nonce = nonce_cookie.split_once('=').unwrap().1.to_string();
        let names = Names { account: "Léa Martin".into(), email: None, device: "Navigateur".into() };
        let mut claims = Claims::issue("share.kybernow.app", &s.relay.id().unwrap(), "acc_lea", "web", [Right::Admin].into_iter().collect(), names, now_secs());
        claims.jti = nonce;
        claims.via = Some("relay".into());
        let token = signer.sign(&claims);
        let (status, headers, page) = call.send(Method::GET, &format!("/api/login/kybernow?token={token}"), e2e, Some(&nonce_cookie), None).await;
        // Le jeton seul n'ouvre rien : pas de session, la page demande la clé.
        assert!(cookie(&headers, crate::auth::REMOTE_COOKIE).is_none(), "le jeton du relais suffit encore");
        let Some(phone) = phone else { return (status, page, None) };
        assert_eq!(status, StatusCode::OK, "{page}");
        assert!(page.contains("data-kind=\"login\""));
        let pending = cookie(&headers, LOGIN_COOKIE).unwrap();
        let (status, _, options) = call.send(Method::POST, "/api/login/key/options", e2e, Some(&pending), Some(json!({}))).await;
        assert_eq!(status, StatusCode::OK, "{options}");
        let options: Value = serde_json::from_str(&options).unwrap();
        assert_eq!(options["rpId"], name);
        let a = phone.get(options["challenge"].as_str().unwrap());
        let body = json!({ "id": b64(&a.credential), "client_data": b64(&a.client_data), "authenticator": b64(&a.authenticator), "signature": b64(&a.signature), "user": "" });
        let (status, headers, body) = call.send(Method::POST, "/api/login/key", e2e, Some(&pending), Some(body)).await;
        (status, body, cookie(&headers, crate::auth::REMOTE_COOKIE))
    }

    #[tokio::test]
    async fn ajout_sur_place_puis_ouverture_avec_jeton_et_cle() {
        // Une Station de bout en bout : son certificat pour `<id>.station.share.kybernow.app`.
        let dir = std::env::temp_dir().join(format!("kyber-admin-cle-{}", rand::random::<u64>()));
        let id = StationIdentity::load_or_create(&dir.join("relay").join("station.key")).unwrap().id().to_string();
        let name = format!("{id}.station.share.kybernow.app");
        let key = rcgen::KeyPair::generate_for(&rcgen::PKCS_ECDSA_P256_SHA256).unwrap();
        let cert = rcgen::CertificateParams::new(vec![name.clone()]).unwrap().self_signed(&key).unwrap();
        std::fs::create_dir_all(dir.join("relay").join("tls")).unwrap();
        std::fs::write(dir.join("relay").join("tls").join("cert.pem"), format!("{}{}", key.serialize_pem(), cert.pem())).unwrap();
        let s = AppState::for_tests(dir.clone(), Box::new(crate::service::Simulated::default()));
        s.relay.tls.observe(true, Some(name.clone()), true);
        assert_eq!(e2e_name(&s).as_deref(), Some(name.as_str()));
        let signer = crate::accounts::attach_for_tests(&s);
        {
            let mut st = s.settings.lock().unwrap();
            st.account.accept = true;
            st.relay.remote_access = true;
            st.relay.remote_admin = true;
        }
        let call = Call { router: crate::relay::router(s.clone()), host: name.clone() };
        let e2e = StreamKind::BrowserTls;

        // Le lien d'ajout : sur place seulement.
        let (status, ..) = call.send(Method::POST, "/api/relay/keys", e2e, Some(&format!("{}={}", crate::auth::REMOTE_COOKIE, s.auth.open_remote_session("Léa", true))), None).await;
        assert_eq!(status, StatusCode::FORBIDDEN, "jamais par le relais, même avec une session à distance");
        let res = create(State(s.clone()), None).await;
        assert_eq!(res.status(), StatusCode::OK);
        let link: Value = serde_json::from_slice(&axum::body::to_bytes(res.into_body(), 4096).await.unwrap()).unwrap();
        let url = link["url"].as_str().unwrap();
        let code = url.strip_prefix(&format!("https://{name}/api/login/key/add#code=")).expect(url).to_string();
        s.auth.close_remote_sessions();

        // La page d'ajout, de bout en bout seulement, avec sa politique de contenu.
        let (status, headers, page) = call.send(Method::GET, "/api/login/key/add", e2e, None, None).await;
        assert_eq!(status, StatusCode::OK, "{page}");
        assert!(headers[header::CONTENT_SECURITY_POLICY].to_str().unwrap().contains("script-src 'sha256-"));
        assert!(page.contains("data-kind=\"add\""));
        assert_eq!(call.send(Method::GET, "/api/login/key/add", StreamKind::AdminHttp, None, None).await.0, StatusCode::FORBIDDEN, "pas sur le chemin déchiffré");

        // La passkey créée pour le nom de la Station.
        let mut phone = Authenticator::new(webauthn::ES256, &format!("https://{name}"));
        let (status, _, options) = call.send(Method::POST, "/api/login/key/add/options", e2e, None, Some(json!({ "code": code }))).await;
        assert_eq!(status, StatusCode::OK, "{options}");
        let options: Value = serde_json::from_str(&options).unwrap();
        assert_eq!(options["rp"]["id"], name.as_str());
        let (client, object) = phone.create(options["challenge"].as_str().unwrap());
        let added = json!({ "code": code, "client_data": b64(&client), "attestation": b64(&object) });
        let (status, _, body) = call.send(Method::POST, "/api/login/key/add", e2e, None, Some(added.clone())).await;
        assert_eq!(status, StatusCode::OK, "{body}");
        assert!(body.contains("\"iPhone\""));
        assert_eq!(call.send(Method::POST, "/api/login/key/add", e2e, None, Some(added)).await.0, StatusCode::GONE, "le code a servi");
        let view = s.relay.status(&s);
        assert_eq!((view["remote_keys"][0]["name"].as_str(), view["remote_keys"][0]["current"].as_bool()), (Some("iPhone"), Some(true)), "{view}");

        // Avec la bonne clé : la session s'ouvre, et sert de bout en bout.
        let (status, body, session) = open(&call, &s, &signer, &name, Some(&mut phone)).await;
        assert_eq!(status, StatusCode::OK, "{body}");
        let session = session.expect("session d'Admin à distance");
        assert_eq!(call.send(Method::GET, "/api/relay", e2e, Some(&session), None).await.0, StatusCode::OK);
        assert_eq!(s.auth.remote_names(), vec!["Léa Martin".to_string()]);
        // Un relais compromis qui forge le jeton n'a pas la clé : une passkey à lui est refusée.
        let mut forged = Authenticator::new(webauthn::ES256, &format!("https://{name}"));
        let (status, _, session) = open(&call, &s, &signer, &name, Some(&mut forged)).await;
        assert_eq!((status, session), (StatusCode::FORBIDDEN, None));
        // Sans clé sur la Station : refusé, avec la marche à suivre.
        let id = s.relay.status(&s)["remote_keys"][0]["id"].as_str().unwrap().to_string();
        assert_eq!(delete(State(s.clone()), UrlPath(id)).await.status(), StatusCode::OK);
        let (status, page, _) = open(&call, &s, &signer, &name, None).await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert!(page.contains(&Msg::new("remoteKey.none").text().replace('\'', "&#39;")), "{page}");
        let _ = std::fs::remove_dir_all(dir);
    }
}
