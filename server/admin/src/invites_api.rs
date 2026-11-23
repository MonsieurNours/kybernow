//! Les invitations dans Admin et pour les appareils (docs/invitations.md).
//!
//! Admin (session d'Admin) : `GET /api/invites`, `POST /api/invites`,
//! `DELETE /api/invites/{id}`, `POST /api/invites/{id}/password`.
//! Appareils (sans compte, sur le réseau et par le relais) :
//! `POST /api/pair/invite`.

use std::net::SocketAddr;
use std::time::Duration;

use axum::extract::{ConnectInfo, Path, State};
use axum::http::{header, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::activity::DeviceInfo;
use crate::api::{blocking, ApiError, ApiResult};
use crate::i18n::Msg;
use crate::invites::{self, AddError, Invite};
use crate::relay::Via;
use crate::AppState;

/// Attente après un mot de passe faux (en essais, pas de délai).
#[cfg(not(test))]
const FAILED_TRY_DELAY: Duration = Duration::from_millis(800);
#[cfg(test)]
const FAILED_TRY_DELAY: Duration = Duration::ZERO;

/// Échéance des invitations vérifiée toutes les 30 secondes.
const EXPIRY_CHECK: Duration = Duration::from_secs(30);

fn invite_json(s: &AppState, invite: &Invite) -> Value {
    let mut v = invite.json();
    let links = crate::share_api::links(s, &invite.code);
    v["link"] = json!(links.link);
    v["relay"] = links.relay;
    let devices = s.activity.devices();
    v["devices"] = invite
        .devices
        .iter()
        .map(|id| match devices.iter().find(|d| &d.id == id) {
            Some(d) => json!({ "id": d.id, "name": d.name, "kind": d.kind, "last_seen": d.last_seen }),
            None => json!({ "id": id }),
        })
        .collect();
    v
}

fn view(s: &AppState) -> Value {
    let relay = s.settings.lock().unwrap().relay.clone();
    let now = crate::display::now_secs();
    let list: Vec<Value> = s.invites.all().iter().filter(|i| i.active(now)).map(|i| invite_json(s, i)).collect();
    json!({
        "invites": list,
        "max": invites::MAX_INVITES,
        "days": invites::DAYS,
        "relay": { "set": relay.addr().is_some(), "remote_access": relay.remote_access },
    })
}

pub async fn list(State(s): State<AppState>) -> Json<Value> {
    Json(view(&s))
}

#[derive(Deserialize)]
pub struct CreateBody {
    #[serde(default)]
    name: String,
    #[serde(default)]
    days: Option<u64>,
}

/// Nouvelle invitation : le mot de passe n'est rendu qu'ici. Avec un relais
/// réglé, « Accès hors de chez vous » s'active : l'appareil ajouté de loin
/// doit pouvoir se connecter ensuite.
pub async fn create(State(s): State<AppState>, Json(body): Json<CreateBody>) -> ApiResult<Json<Value>> {
    let password = invites::generate_password();
    let p = password.clone();
    let hash = blocking(move || invites::hash_password(&p)).await?;
    let share_code = s.share.snapshot().map(|snap| snap.code);
    let lend_code = s.lend.code_value();
    let invite = s.invites.create(&body.name, body.days.unwrap_or(7), hash, |c| share_code.as_deref() == Some(c) || lend_code.as_deref() == Some(c))?;
    tracing::info!("invitation « {} » créée, valable jusqu'à {}", invite.name, invite.expires);
    s.activity.event("info", Msg::new("events.inviteCreated").with("name", invite.name.as_str()));
    let remote_access_on = turn_on_remote_access(&s)?;
    s.relay.wake();
    let mut answer = view(&s);
    answer["invite"] = invite_json(&s, &invite);
    answer["password"] = json!(password);
    answer["remote_access_turned_on"] = json!(remote_access_on);
    Ok(Json(answer))
}

/// Active « Accès hors de chez vous » si un relais est réglé ; vrai s'il
/// vient de l'être.
fn turn_on_remote_access(s: &AppState) -> ApiResult<bool> {
    let relay = s.settings.lock().unwrap().relay.clone();
    if relay.addr().is_none() || relay.remote_access {
        return Ok(false);
    }
    s.update_settings(|settings| settings.relay.remote_access = true)?;
    tracing::info!("relais : accès hors de chez vous activé pour une invitation");
    s.activity.event("info", Msg::new("events.relayAccessOn"));
    Ok(true)
}

fn unknown() -> ApiError {
    ApiError::msg(StatusCode::NOT_FOUND, Msg::new("invites.unknown"))
}

/// Supprime l'invitation et les appareils qu'elle a ajoutés.
pub async fn delete(State(s): State<AppState>, Path(id): Path<String>) -> ApiResult<Json<Value>> {
    let invite = s.invites.get(&id).ok_or_else(unknown)?;
    remove(&s, &invite, "events.inviteDeleted").await?;
    Ok(Json(view(&s)))
}

async fn remove(s: &AppState, invite: &Invite, event: &str) -> ApiResult<()> {
    for device in &invite.devices {
        if let Err(e) = crate::api::revoke_device(s, device).await {
            tracing::warn!("invitation « {} » : appareil {device} non retiré ({})", invite.name, e.1);
        }
    }
    s.invites.delete(&invite.id)?;
    s.activity.event("info", Msg::new(event).with("name", invite.name.as_str()).with("count", invite.devices.len()));
    s.relay.wake();
    Ok(())
}

/// Un nouveau mot de passe : l'ancien ne sert plus (les appareils déjà
/// ajoutés gardent leur accès).
pub async fn new_password(State(s): State<AppState>, Path(id): Path<String>) -> ApiResult<Json<Value>> {
    s.invites.get(&id).ok_or_else(unknown)?;
    let password = invites::generate_password();
    let p = password.clone();
    let hash = blocking(move || invites::hash_password(&p)).await?;
    let invite = s.invites.set_hash(&id, hash)?.ok_or_else(unknown)?;
    let mut answer = view(&s);
    answer["invite"] = invite_json(&s, &invite);
    answer["password"] = json!(password);
    Ok(Json(answer))
}

/// Les invitations échues tombent, avec les appareils qu'elles ont ajoutés.
pub fn supervise(s: AppState) {
    tokio::spawn(async move {
        let mut tick = tokio::time::interval(EXPIRY_CHECK);
        loop {
            tick.tick().await;
            expire(&s).await;
        }
    });
}

async fn expire(s: &AppState) {
    for invite in s.invites.take_expired() {
        tracing::info!("invitation « {} » échue : {} appareil(s) retiré(s)", invite.name, invite.devices.len());
        for device in &invite.devices {
            if let Err(e) = crate::api::revoke_device(s, device).await {
                tracing::warn!("invitation « {} » : appareil {device} non retiré ({})", invite.name, e.1);
            }
        }
        s.activity.event("info", Msg::new("events.inviteExpired").with("name", invite.name.as_str()).with("count", invite.devices.len()));
        s.relay.wake();
    }
}

// --- L'appareil : POST /api/pair/invite ---------------------------------------------------

#[derive(Deserialize)]
pub struct RedeemBody {
    #[serde(default)]
    code: String,
    #[serde(default)]
    password: String,
    device: DeviceInfo,
}

fn refusal(status: StatusCode, code: &str, msg: Msg) -> Response {
    (status, [(header::CACHE_CONTROL, "no-store")], Json(json!({ "error": code, "message": msg.text() }))).into_response()
}

/// Le code et le mot de passe d'une invitation : l'appareil reçoit un
/// compte Kyber à lui, comme après un appairage accepté, et ce qu'il faut
/// pour garder l'ordinateur (adresse, empreinte d'Admin, relais).
pub async fn redeem(
    State(s): State<AppState>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    via: Option<axum::Extension<Via>>,
    Json(body): Json<RedeemBody>,
) -> Response {
    let ip = peer.ip().to_canonical();
    let Some(invite) = s.invites.by_code(&body.code) else {
        return refusal(StatusCode::NOT_FOUND, "no_invite", Msg::new("invites.noInvite"));
    };
    if let Err(wait) = s.invites.begin_attempt(ip) {
        let mut res = (
            StatusCode::TOO_MANY_REQUESTS,
            Json(json!({ "error": "too_many", "retry_after": wait, "message": Msg::new("invites.tooManyTries").text() })),
        )
            .into_response();
        res.headers_mut().insert(header::RETRY_AFTER, HeaderValue::from(wait));
        return res;
    }
    let (hash, password) = (invite.hash().to_string(), body.password.clone());
    let ok = tokio::task::spawn_blocking(move || invites::verify_password(&hash, &password)).await.unwrap_or(false);
    match s.invites.end_attempt(ip, ok) {
        Some(true) => {
            tracing::warn!("invitations : trop de mots de passe faux, toutes bloquées 10 min");
            s.activity.event("warn", Msg::new("events.invitesLocked"));
        }
        Some(false) => {
            tracing::warn!("invitations : trop de mots de passe faux depuis {ip}, adresse bloquée 10 min");
            s.activity.event("warn", Msg::new("events.invitesLockedAddress").with("address", ip.to_string()));
        }
        None => {}
    }
    if !ok {
        tracing::info!("invitation « {} » : mot de passe faux depuis {ip}", invite.name);
        tokio::time::sleep(FAILED_TRY_DELAY).await;
        return refusal(StatusCode::UNAUTHORIZED, "bad_password", Msg::new("invites.badPassword"));
    }
    let mut device = body.device;
    device.id = device.id.trim().chars().filter(|c| !c.is_control()).take(80).collect();
    device.name = device.name.trim().chars().filter(|c| !c.is_control()).take(60).collect();
    if device.id.is_empty() || device.name.is_empty() {
        return refusal(StatusCode::BAD_REQUEST, "bad_request", Msg::new("pairing.deviceIdentity"));
    }
    match s.invites.add_device(&invite.id, &device.id) {
        Ok(()) => {}
        Err(AddError::Gone) => return refusal(StatusCode::NOT_FOUND, "no_invite", Msg::new("invites.noInvite")),
        Err(AddError::Full) => return refusal(StatusCode::CONFLICT, "full", Msg::new("invites.full").with("max", invites::MAX_DEVICES)),
    }
    let account = match crate::api::create_device_account(&s, &device).await {
        Ok(account) => account,
        Err(e) => {
            s.invites.remove_device(&device.id);
            tracing::warn!("invitation « {} » : compte non créé ({})", invite.name, e.1);
            return refusal(StatusCode::SERVICE_UNAVAILABLE, "unavailable", Msg::new("invites.unavailable"));
        }
    };
    let away = via.is_some();
    tracing::info!(
        "invitation « {} » : « {} » ajouté{} (compte {})",
        invite.name,
        device.name,
        if away { " par le relais" } else { "" },
        account.username
    );
    s.activity.event("info", Msg::new("events.inviteDevice").with("name", device.name.as_str()).with("invite", invite.name.as_str()));
    let answer = json!({
        "username": account.username,
        "password": account.password,
        "hostname": crate::system::computer_name().unwrap_or_default(),
        "address": crate::tls::lan_ip(),
        "port": s.listen_port,
        "fingerprint": s.tls_fingerprint,
        "relay": s.relay.info(&s),
        "invite": { "name": invite.name, "expires": invite.expires },
    });
    ([(header::CACHE_CONTROL, "no-store")], Json(answer)).into_response()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state() -> AppState {
        let dir = std::env::temp_dir().join(format!("kyber-admin-invites-{}", rand::random::<u64>()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("kyber_config.toml"), "").unwrap();
        AppState::for_tests(dir, Box::new(crate::service::Simulated::default()))
    }

    async fn body(res: Response) -> (StatusCode, Value) {
        let status = res.status();
        let bytes = axum::body::to_bytes(res.into_body(), 1 << 20).await.unwrap();
        (status, serde_json::from_slice(&bytes).unwrap_or(Value::Null))
    }

    fn peer(n: u8) -> ConnectInfo<SocketAddr> {
        ConnectInfo(SocketAddr::from(([82, 66, 39, n], 0)))
    }

    fn via() -> Option<axum::Extension<Via>> {
        Some(axum::Extension(Via { kind: kybernow_tunnel::StreamKind::AdminHttp, client: "c1".into() }))
    }

    fn redeem_body(code: &str, password: &str, id: &str) -> Json<RedeemBody> {
        Json(RedeemBody {
            code: code.into(),
            password: password.into(),
            device: DeviceInfo { id: id.into(), name: "iPhone de test".into(), kind: Some("phone".into()), os: Some("iOS 26.1".into()), app: None },
        })
    }

    #[tokio::test]
    async fn une_invitation_ajoute_un_appareil_sans_accord() {
        let s = state();
        let Json(created) = create(State(s.clone()), Json(CreateBody { name: "App Review".into(), days: Some(7) })).await.ok().unwrap();
        let password = created["password"].as_str().unwrap().to_string();
        let code = created["invite"]["code"].as_str().unwrap().to_string();
        assert!(created["invite"]["link"].as_str().unwrap().contains(&format!("&code={code}")));
        assert!(created["invite"].get("hash").is_none());
        assert_eq!(created["invites"].as_array().unwrap().len(), 1);
        // Pas de relais réglé dans les essais : l'accès hors de chez vous ne bouge pas.
        assert!(s.invites.is_invite_code(&code));

        // Code inconnu, puis mot de passe faux.
        let (status, v) = body(redeem(State(s.clone()), peer(1), via(), redeem_body("000000", &password, "iphone-1")).await).await;
        assert_eq!((status, v["error"].as_str()), (StatusCode::NOT_FOUND, Some("no_invite")));
        let (status, v) = body(redeem(State(s.clone()), peer(1), via(), redeem_body(&code, "faux-faux-faux", "iphone-1")).await).await;
        assert_eq!((status, v["error"].as_str()), (StatusCode::UNAUTHORIZED, Some("bad_password")));
        assert!(s.activity.devices().is_empty());

        // Le bon mot de passe, recopié en majuscules avec des espaces.
        let typed = password.to_uppercase().replace('-', " ");
        let (status, v) = body(redeem(State(s.clone()), peer(1), via(), redeem_body(&code, &typed, "iphone-1")).await).await;
        assert_eq!(status, StatusCode::OK, "{v}");
        let user = v["username"].as_str().unwrap().to_string();
        assert!(user.starts_with("iPhone-de-test-"), "{user}");
        assert_eq!(v["password"].as_str().unwrap().len(), 32);
        assert_eq!(v["invite"]["name"], "App Review");
        assert!(s.kyber.users().unwrap().iter().any(|u| u.username == user));
        let devices = s.activity.devices();
        assert_eq!((devices.len(), devices[0].own_account), (1, true));
        assert_eq!(s.invites.of_device("iphone-1").unwrap().name, "App Review");
        let Json(listed) = list(State(s.clone())).await;
        assert_eq!(listed["invites"][0]["devices"][0]["name"], "iPhone de test");

        // Supprimer l'invitation retire l'appareil et son compte.
        let id = created["invite"]["id"].as_str().unwrap().to_string();
        let Json(after) = delete(State(s.clone()), Path(id)).await.ok().unwrap();
        assert!(after["invites"].as_array().unwrap().is_empty());
        assert!(s.activity.devices().is_empty());
        assert!(!s.kyber.users().unwrap().iter().any(|u| u.username == user));
        let (status, _) = body(redeem(State(s.clone()), peer(1), via(), redeem_body(&code, &password, "iphone-2")).await).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn trop_de_mots_de_passe_faux_bloquent_l_adresse() {
        let s = state();
        let Json(created) = create(State(s.clone()), Json(CreateBody { name: "Essai".into(), days: Some(1) })).await.ok().unwrap();
        let password = created["password"].as_str().unwrap().to_string();
        let code = created["invite"]["code"].as_str().unwrap().to_string();
        for _ in 0..5 {
            let (status, _) = body(redeem(State(s.clone()), peer(7), via(), redeem_body(&code, "non", "x")).await).await;
            assert_eq!(status, StatusCode::UNAUTHORIZED);
        }
        // Même le bon mot de passe attend, depuis cette adresse.
        let (status, v) = body(redeem(State(s.clone()), peer(7), via(), redeem_body(&code, &password, "x")).await).await;
        assert_eq!(status, StatusCode::TOO_MANY_REQUESTS, "{v}");
        assert!(v["retry_after"].as_u64().unwrap() > 0);
        let (status, _) = body(redeem(State(s.clone()), peer(8), via(), redeem_body(&code, &password, "y")).await).await;
        assert_eq!(status, StatusCode::OK);
    }

    #[tokio::test]
    async fn nouveau_mot_de_passe() {
        let s = state();
        let Json(created) = create(State(s.clone()), Json(CreateBody { name: "Essai".into(), days: Some(30) })).await.ok().unwrap();
        let (old, code, id) = (
            created["password"].as_str().unwrap().to_string(),
            created["invite"]["code"].as_str().unwrap().to_string(),
            created["invite"]["id"].as_str().unwrap().to_string(),
        );
        let Json(renewed) = new_password(State(s.clone()), Path(id)).await.ok().unwrap();
        let new = renewed["password"].as_str().unwrap().to_string();
        assert_ne!(old, new);
        let (status, _) = body(redeem(State(s.clone()), peer(1), None, redeem_body(&code, &old, "a")).await).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        let (status, _) = body(redeem(State(s.clone()), peer(1), None, redeem_body(&code, &new, "a")).await).await;
        assert_eq!(status, StatusCode::OK);
    }
}
