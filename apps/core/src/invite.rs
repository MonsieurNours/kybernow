//! Invitations : ajouter un ordinateur de loin (docs/invitations.md).
//!
//! « Rejoindre avec un code » reconnaît un code d'invitation
//! ([`crate::share::ShareError::Invite`], `404 {"error":"no_share",
//! "invite":true}`) : l'app demande alors le mot de passe de l'invitation et
//! appelle [`redeem`], qui rend l'ordinateur à garder (compte propre à cet
//! appareil, adresse, empreinte d'Admin, relais), comme après un appairage
//! accepté. Personne n'a à l'accepter sur l'ordinateur.

use serde::{Deserialize, Serialize};
use std::time::Duration;

use crate::console::{self, Console, ConsoleError, DeviceInfo};
use crate::i18n::{self, tr, Lang};
use crate::relay::{RelayInfo, RelayRoute};
use crate::share::ShareTarget;

/// Délai de `POST /api/pair/invite` (le mot de passe est vérifié par
/// Argon2, puis le compte créé).
const REDEEM_TIMEOUT: Duration = Duration::from_secs(15);

/// Un code d'invitation, tel que « Rejoindre » l'a trouvé : la Station qui
/// l'a (sur le réseau, ou par le relais), et le nom de l'ordinateur pour
/// l'écran du mot de passe (« Ajouter {ordinateur} »).
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct InviteCode {
    pub code: String,
    pub target: ShareTarget,
    #[serde(default)]
    pub route: Option<RelayRoute>,
    /// Nom de l'ordinateur (`computer` de la réponse) ; vide pour une
    /// Station qui ne le dit pas.
    #[serde(default)]
    pub computer: String,
}

/// L'ordinateur ajouté : à garder comme un ordinateur appairé.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Invited {
    /// Nom de l'ordinateur (« PC du salon »).
    #[serde(default)]
    pub hostname: String,
    /// Adresse d'Admin sur le réseau de l'ordinateur, et son port.
    #[serde(default)]
    pub address: String,
    #[serde(default)]
    pub port: u16,
    /// Empreinte d'Admin, à épingler.
    #[serde(default)]
    pub fingerprint: Option<String>,
    /// Le compte propre à cet appareil.
    pub username: String,
    pub password: String,
    /// Le relais de la Station (`info.relay`), pour l'ouvrir hors de chez soi.
    #[serde(default)]
    pub relay: Option<RelayInfo>,
    /// Nom de l'invitation et son échéance (secondes Unix de la Station) :
    /// l'accès tombe avec elle.
    #[serde(default)]
    pub invite_name: String,
    #[serde(default)]
    pub expires: u64,
    /// Ajouté par le relais (et non sur le réseau).
    #[serde(default)]
    pub via_relay: bool,
}

/// Pourquoi l'ordinateur n'est pas ajouté (docs/link-ui.md, « Invitations »).
#[derive(Debug)]
pub enum InviteError {
    /// `401 bad_password` : « Mot de passe refusé. »
    BadPassword,
    /// `404 no_invite` : « Ce code ne sert plus. »
    NoInvite,
    /// `409 full` : « Cette invitation a déjà ajouté tous ses appareils. »
    Full,
    /// `429` : « Trop d'essais : réessayez dans {n} min. »
    TooManyTries { seconds: Option<u64> },
    /// Par le relais, `403 remote_access_off` : accès hors de chez vous coupé.
    AccessOff,
    /// Le reste : Admin injoignable, relais muet, réponse illisible.
    Other(ConsoleError),
}

impl InviteError {
    /// `computer` : le nom de l'ordinateur, pour l'accès coupé.
    pub fn text(&self, lang: Lang, computer: &str) -> String {
        match self {
            Self::BadPassword => tr(lang, "invite.bad_password", &[]),
            Self::NoInvite => tr(lang, "invite.no_invite", &[]),
            Self::Full => tr(lang, "invite.full", &[]),
            Self::TooManyTries { seconds } => {
                let minutes = seconds.map(|s| s.div_ceil(60).max(1)).unwrap_or(10);
                tr(lang, "invite.too_many", &[("minutes", &minutes.to_string())])
            }
            Self::AccessOff if computer.is_empty() => tr(lang, "relay.access_off", &[]),
            Self::AccessOff => tr(lang, "relay.access_off_named", &[("name", computer)]),
            Self::Other(e) => e.text(lang),
        }
    }

    /// Nom stable pour les ponts : `bad_password`, `no_invite`, `full`,
    /// `too_many`, `access_off`, `other`.
    pub fn kind(&self) -> &'static str {
        match self {
            Self::BadPassword => "bad_password",
            Self::NoInvite => "no_invite",
            Self::Full => "full",
            Self::TooManyTries { .. } => "too_many",
            Self::AccessOff => "access_off",
            Self::Other(_) => "other",
        }
    }

    /// Code des ponts (C, JNI) : 60 et suivants.
    pub fn code(&self) -> i32 {
        match self {
            Self::BadPassword => 60,
            Self::NoInvite => 61,
            Self::Full => 62,
            Self::TooManyTries { .. } => 63,
            Self::AccessOff => 64,
            Self::Other(_) => 65,
        }
    }
}

impl std::fmt::Display for InviteError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.text(i18n::current(), ""))
    }
}

impl std::error::Error for InviteError {}

impl From<ConsoleError> for InviteError {
    fn from(e: ConsoleError) -> Self {
        Self::Other(e)
    }
}

/// Un refus d'Admin.
pub(crate) fn invite_refusal(status: u16, body: &serde_json::Value, retry_after: Option<u64>) -> InviteError {
    let code = body.get("error").and_then(|e| e.as_str()).unwrap_or_default();
    match (status, code) {
        (401, _) => InviteError::BadPassword,
        (404, _) => InviteError::NoInvite,
        (409, "full") => InviteError::Full,
        (429, _) => InviteError::TooManyTries { seconds: body.get("retry_after").and_then(|s| s.as_u64()).or(retry_after) },
        (403, "remote_access_off") => InviteError::AccessOff,
        _ => {
            let message = body
                .get("message")
                .and_then(|m| m.as_str())
                .map(str::to_owned)
                .unwrap_or_else(|| if code.is_empty() { tr(i18n::current(), "console.http_status", &[("status", &status.to_string())]) } else { code.to_string() });
            InviteError::Other(ConsoleError::Http { status, message })
        }
    }
}

/// La réponse de `POST /api/pair/invite`.
#[derive(Deserialize)]
struct Answer {
    username: String,
    password: String,
    #[serde(default)]
    hostname: String,
    #[serde(default)]
    address: Option<String>,
    #[serde(default)]
    port: u16,
    #[serde(default)]
    fingerprint: Option<String>,
    #[serde(default)]
    relay: Option<RelayInfo>,
    #[serde(default)]
    invite: Option<AnswerInvite>,
}

#[derive(Deserialize, Default)]
struct AnswerInvite {
    #[serde(default)]
    name: String,
    #[serde(default)]
    expires: u64,
}

impl Console {
    /// `POST /api/pair/invite` : le code et le mot de passe d'une invitation.
    pub(crate) fn pair_invite(&self, code: &str, password: &str, device: &DeviceInfo) -> Result<serde_json::Value, InviteError> {
        let r = self
            .request("POST", "api/pair/invite", REDEEM_TIMEOUT)
            .send_json(serde_json::json!({ "code": code, "password": password, "device": device }));
        match self.answer(r).map_err(InviteError::Other)? {
            console::Answer::Ok(resp) => resp.into_json().map_err(|e| InviteError::Other(ConsoleError::Invalid(e.to_string()))),
            console::Answer::Refused(r) => Err(invite_refusal(r.status, &r.body, r.retry_after)),
        }
    }
}

/// Ajoute l'ordinateur de ce code d'invitation avec son mot de passe, par
/// le même chemin que « Rejoindre » (le réseau, ou le relais de bout en
/// bout, Admin épinglé). Bloquant (15 s au plus).
pub fn redeem(code: &InviteCode, password: &str, device: &DeviceInfo) -> Result<Invited, InviteError> {
    let console = match &code.route {
        Some(route) => crate::share::relay_console(route),
        None => Console::guest(&code.target.host, code.target.port, code.target.fingerprint.clone()),
    };
    log::info!("invitation : mot de passe envoyé{}", if code.route.is_some() { " par le relais" } else { "" });
    let v = console.pair_invite(&code.code, password, device)?;
    let answer: Answer = serde_json::from_value(v).map_err(|e| InviteError::Other(ConsoleError::Invalid(e.to_string())))?;
    let seen = code.target.fingerprint.clone().or_else(|| console.fingerprint_seen());
    Ok(invited(code, answer, seen))
}

fn invited(code: &InviteCode, a: Answer, seen: Option<String>) -> Invited {
    let via_relay = code.route.is_some();
    // Sur le réseau, l'adresse où Admin a répondu vaut mieux que celle qu'il croit avoir.
    let (address, port) = if via_relay || code.target.host.is_empty() {
        (a.address.unwrap_or_default(), a.port)
    } else {
        (code.target.host.clone(), code.target.port)
    };
    let hostname = if a.hostname.is_empty() { code.computer.clone() } else { a.hostname };
    let invite = a.invite.unwrap_or_default();
    Invited {
        hostname,
        address,
        port,
        fingerprint: a.fingerprint.filter(|f| !f.is_empty()).or(seen),
        username: a.username,
        password: a.password,
        relay: a.relay,
        invite_name: invite.name,
        expires: invite.expires,
        via_relay,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn refus() {
        let r = |status, body: serde_json::Value| invite_refusal(status, &body, Some(120)).kind();
        assert_eq!(r(401, serde_json::json!({"error":"bad_password"})), "bad_password");
        assert_eq!(r(404, serde_json::json!({"error":"no_invite"})), "no_invite");
        assert_eq!(r(409, serde_json::json!({"error":"full"})), "full");
        assert_eq!(r(403, serde_json::json!({"error":"remote_access_off"})), "access_off");
        assert_eq!(r(500, serde_json::json!({})), "other");
        let e = invite_refusal(429, &serde_json::json!({"error":"too_many","retry_after":540}), None);
        assert!(matches!(e, InviteError::TooManyTries { seconds: Some(540) }));
        assert_eq!(e.text(Lang::Fr, ""), "Trop d'essais : réessayez dans 9 min.");
        assert_eq!(InviteError::AccessOff.text(Lang::En, "Living room PC"), "Living room PC cannot be reached away from home (access turned off in Admin).");
    }

    fn answer() -> Answer {
        serde_json::from_value(serde_json::json!({
            "username": "iPhone-ab12", "password": "secret", "hostname": "PC du salon",
            "address": "192.168.1.20", "port": 47990, "fingerprint": "sha256:aa",
            "relay": {"server": "share.kybernow.app", "id": "abcd", "remote_access": true},
            "invite": {"name": "App Review", "expires": 1_800_000_000u64}
        }))
        .unwrap()
    }

    #[test]
    fn par_le_relais_l_adresse_vient_de_la_station() {
        let route = RelayRoute { server: "share.kybernow.app".into(), station: "abcd".into(), admin_fp: "sha256:aa".into() };
        let code = InviteCode { code: "123456".into(), target: crate::share::relay_target(&route), route: Some(route), computer: String::new() };
        let i = invited(&code, answer(), None);
        assert_eq!((i.address.as_str(), i.port, i.via_relay), ("192.168.1.20", 47990, true));
        assert_eq!(i.relay.unwrap().id, "abcd");
        assert_eq!((i.invite_name.as_str(), i.hostname.as_str()), ("App Review", "PC du salon"));
    }

    #[test]
    fn sur_le_reseau_l_adresse_qui_a_repondu() {
        let code = InviteCode { code: "123456".into(), target: ShareTarget { host: "10.0.0.5".into(), port: 47990, fingerprint: None }, route: None, computer: "PC".into() };
        let mut a = answer();
        a.fingerprint = None;
        let i = invited(&code, a, Some("sha256:vu".into()));
        assert_eq!((i.address.as_str(), i.via_relay), ("10.0.0.5", false));
        assert_eq!(i.fingerprint.as_deref(), Some("sha256:vu"));
    }
}
