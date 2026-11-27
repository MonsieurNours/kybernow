//! Comptes KyberNow dans les apps (docs/comptes.md, lot 3 ; textes dans
//! docs/link-ui.md, « Comptes KyberNow »).
//!
//! Un appareil se connecte à un compte d'un relais sans jamais voir le mot de
//! passe, de deux façons :
//!
//! - **fenêtre du système** ([`sign_in`], [`finish_sign_in`]) : l'app ouvre la
//!   page de connexion du relais (`/compte/connexion?appareil=…&retour=
//!   kybernow://compte&etat=…`) dans `ASWebAuthenticationSession`, un onglet
//!   personnalisé ou le navigateur ; le relais revient à
//!   `kybernow://compte?code=…&etat=…`, et le code s'échange contre la session
//!   d'appareil (`POST /v1/devices/token`) ;
//! - **code** pour les téléviseurs et le salon ([`tv_code`], [`tv_poll`],
//!   [`tv_wait`]) : un code de huit signes et un QR code, approuvés depuis le
//!   téléphone ; l'app demande toutes les 5 secondes (`POST /v1/devices/poll`).
//!
//! La session d'appareil ([`Account`], son `token` compris) est gardée par
//! l'app dans le coffre de son système (Trousseau, Keystore, Gestionnaire
//! d'identification, Secret Service) : le cœur ne l'écrit nulle part. Elle
//! donne la liste des ordinateurs du compte ([`computers`], avec l'empreinte
//! d'Admin signée par la Station quand elle est connectée) et, pour ouvrir
//! l'un d'eux, un jeton d'accès de 2 minutes ([`StationAccess`],
//! `POST /v1/access`), redemandé au bout de 90 secondes et présenté à Admin
//! (`Authorization: KyberNow <jeton>`) sur le réseau comme par le relais.
//! L'app peut être connectée à plusieurs comptes, sur plusieurs relais ; un
//! même ordinateur n'apparaît qu'une fois à l'accueil ([`merge`]).
//!
//! Les réglages de chaque ordinateur du compte suivent la personne d'un
//! appareil à l'autre ([`settings`] : [`fetch_settings`], [`put_settings`]).
//!
//! Ce module ne dépend que de `serde`, `ureq`, `kybernow-tunnel` et des
//! catalogues du cœur ([`crate::i18n`]), et [`settings`] des réglages de
//! session ([`crate::profile`]) : ses essais tournent sans Kyber.

use std::fmt;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::i18n::{self, Lang, tr};

pub use kybernow_tunnel::{DEFAULT_RELAY, RelayAddr};

/// Le retour de la fenêtre du système : le relais n'en accepte pas d'autre.
pub const RETURN_URI: &str = "kybernow://compte";
/// Le schéma à donner à `ASWebAuthenticationSession` (`callbackURLScheme`).
pub const RETURN_SCHEME: &str = "kybernow";
/// Un jeton d'accès est redemandé passé ce temps au plus (5 minutes pour un
/// relais d'avant la 0.10) ; un jeton plus court l'est plus tôt, à
/// [`EXPIRY_MARGIN`] de son échéance (2 minutes : au bout de 90 secondes).
pub const RENEW_AFTER: Duration = Duration::from_secs(4 * 60);
/// Marge avant l'échéance donnée par le relais (horloges, sommeil de
/// l'appareil, appels en cours). 30 secondes : un jeton de 2 minutes sert 90
/// secondes, et la Station tolère encore une minute d'écart d'horloge.
const EXPIRY_MARGIN: u64 = 30;
/// Délai d'un appel au relais.
const CALL_TIMEOUT: Duration = Duration::from_secs(8);
/// Demande d'un code de téléviseur : toutes les 5 secondes au moins.
const MIN_POLL: Duration = Duration::from_secs(5);
/// Nom de l'appareil quand l'app n'en donne pas.
const DEFAULT_DEVICE_NAME: &str = "KyberNow Link";

// MARK: - Comptes

/// La personne du compte, telle que le relais la donne.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct AccountUser {
    /// Identifiant public du compte chez ce relais (`acc_…`).
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub email: String,
}

/// Cet appareil chez le relais (« Appareils connectés » de la page du compte).
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct AccountDevice {
    pub id: String,
    #[serde(default)]
    pub name: String,
}

/// Un compte connecté sur cet appareil. À garder **entier** dans le coffre du
/// système : `token` est la session d'appareil (jeton porteur de 256 bits,
/// sans échéance, fermée par « Se déconnecter », depuis la page du compte, ou
/// par un nouveau mot de passe). Jamais dans un fichier en clair, jamais dans
/// les journaux (`Debug` le masque).
#[derive(Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Account {
    /// Le relais, sous sa forme normale (`share.kybernow.app`,
    /// `relais.exemple.org:9443`, empreinte `#sha256:…` gardée).
    pub relay: String,
    pub token: String,
    pub account: AccountUser,
    pub device: AccountDevice,
}

impl fmt::Debug for Account {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Account")
            .field("relay", &self.relay)
            .field("token", &"…")
            .field("account", &self.account)
            .field("device", &self.device)
            .finish()
    }
}

impl Account {
    /// Ce qui distingue ce compte des autres sur l'appareil : le relais et
    /// l'identifiant du compte (deux relais ont leurs propres comptes).
    pub fn key(&self) -> String {
        account_key(&self.relay, &self.account.id)
    }

    /// Ce que l'app affiche, sans le jeton.
    pub fn summary(&self) -> AccountSummary {
        AccountSummary {
            key: self.key(),
            relay: self.relay.clone(),
            relay_label: relay_label(&self.relay),
            account: self.account.clone(),
            device: self.device.clone(),
            console: console_url(&self.relay),
        }
    }

    fn addr(&self) -> Result<RelayAddr, AccountError> {
        relay_addr(&self.relay)
    }
}

fn account_key(relay: &str, account: &str) -> String {
    format!("{relay}|{account}")
}

/// Un compte connecté, sans son jeton : pour les listes de l'app (réglages
/// « Comptes », pastilles d'origine) et les journaux.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct AccountSummary {
    pub key: String,
    pub relay: String,
    /// Le relais à montrer : hôte (et port hors du 443), sans empreinte.
    pub relay_label: String,
    pub account: AccountUser,
    pub device: AccountDevice,
    /// La page du compte sur le relais (« Gérer le compte »).
    pub console: String,
}

/// Le relais d'une adresse tapée ou gardée, sous sa forme normale.
pub fn relay_addr(relay: &str) -> Result<RelayAddr, AccountError> {
    let relay = relay.trim();
    let relay = if relay.is_empty() { DEFAULT_RELAY } else { relay };
    relay.parse::<RelayAddr>().map_err(|_| AccountError::BadRelay)
}

/// La forme normale d'une adresse de relais (« Un autre relais… ») ;
/// [`AccountError::BadRelay`] si elle ne se lit pas. Vide : `share.kybernow.app`.
pub fn normalize_relay(relay: &str) -> Result<String, AccountError> {
    relay_addr(relay).map(|a| a.to_string())
}

/// Le relais tel qu'on le montre : `share.kybernow.app`, `relais.exemple.org:9443`.
pub fn relay_label(relay: &str) -> String {
    kybernow_rules::relay::label(relay)
}

/// La page du compte sur ce relais (`https://share.kybernow.app/compte`) :
/// créer, supprimer le compte, changer le mot de passe s'y font, pas dans
/// l'app (App Store, règle 5.1.1(v)).
pub fn console_url(relay: &str) -> String {
    format!("https://{}/compte", relay_label(relay))
}

// MARK: - Erreurs

/// Pourquoi un appel au relais n'aboutit pas (docs/link-ui.md, « Comptes
/// KyberNow »).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AccountError {
    /// `401 bad_session` : la session de cet appareil est fermée (« Déconnecter »
    /// sur la page du compte, nouveau mot de passe, compte supprimé). L'app
    /// oublie le compte et le dit.
    SignedOut,
    /// `404 unknown_station` : l'ordinateur n'est plus au compte.
    UnknownComputer,
    /// Code de téléviseur refusé depuis le téléphone (« Ce n'est pas moi »).
    Denied,
    /// Code de téléviseur échu (10 minutes) ou déjà servi : l'app en demande un autre.
    Expired,
    /// Retour de la fenêtre du système refusé : code échu ou déjà échangé,
    /// `etat` qui n'est pas celui de la demande.
    BadCode,
    /// « Annuler » dans la page du relais, ou la fenêtre fermée.
    Cancelled,
    /// `429` : trop de codes demandés ou de codes faux.
    TooMany { seconds: Option<u64> },
    /// Le relais n'a pas de comptes (relais auto-hébergé sans `[accounts]`,
    /// ou d'avant les comptes).
    NoAccounts,
    /// Adresse du relais illisible.
    BadRelay,
    /// Le relais ne répond pas.
    Unreachable(String),
    /// Réponse inattendue.
    Other(String),
}

impl AccountError {
    pub fn text(&self, lang: Lang) -> String {
        match self {
            Self::SignedOut => tr(lang, "account.signed_out", &[]),
            Self::UnknownComputer => tr(lang, "account.unknown_computer", &[]),
            Self::Denied => tr(lang, "account.denied", &[]),
            Self::Expired => tr(lang, "account.expired", &[]),
            Self::BadCode => tr(lang, "account.bad_code", &[]),
            Self::Cancelled => tr(lang, "account.cancelled", &[]),
            Self::TooMany { seconds } => {
                let minutes = seconds.map(|s| s.div_ceil(60).max(1)).unwrap_or(15);
                tr(lang, "account.too_many", &[("minutes", &minutes.to_string())])
            }
            Self::NoAccounts => tr(lang, "account.no_accounts", &[]),
            Self::BadRelay => tr(lang, "account.bad_relay", &[]),
            Self::Unreachable(_) => tr(lang, "account.unreachable", &[]),
            Self::Other(e) => tr(lang, "account.other", &[("error", e)]),
        }
    }

    /// Nom stable pour les ponts.
    pub fn kind(&self) -> &'static str {
        match self {
            Self::SignedOut => "signed_out",
            Self::UnknownComputer => "unknown_computer",
            Self::Denied => "denied",
            Self::Expired => "expired",
            Self::BadCode => "bad_code",
            Self::Cancelled => "cancelled",
            Self::TooMany { .. } => "too_many",
            Self::NoAccounts => "no_accounts",
            Self::BadRelay => "bad_relay",
            Self::Unreachable(_) => "unreachable",
            Self::Other(_) => "other",
        }
    }

    /// Code des ponts (C, JNI) : 70 et suivants.
    pub fn code(&self) -> i32 {
        match self {
            Self::SignedOut => 70,
            Self::UnknownComputer => 71,
            Self::Denied => 72,
            Self::Expired => 73,
            Self::BadCode => 74,
            Self::Cancelled => 75,
            Self::TooMany { .. } => 76,
            Self::NoAccounts => 77,
            Self::BadRelay => 78,
            Self::Unreachable(_) => 79,
            Self::Other(_) => 80,
        }
    }
}

impl fmt::Display for AccountError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.text(i18n::current()))
    }
}

impl std::error::Error for AccountError {}

// MARK: - Appels au relais

/// Une réponse de l'API des appareils : le statut, le corps JSON (`Null`
/// s'il ne se lit pas : un relais sans comptes répond une page), `Retry-After`.
#[derive(Debug, Clone, PartialEq)]
pub struct Reply {
    pub status: u16,
    pub body: Value,
    pub retry_after: Option<u64>,
}

impl Reply {
    /// Une réponse HTTP lue une fois pour toutes (COEUR-38 : Admin, relais,
    /// comptes) : statut, corps JSON (nul s'il ne se lit pas), en-tête
    /// `Retry-After` en secondes.
    pub(crate) fn read(resp: ureq::Response) -> Reply {
        let status = resp.status();
        let retry_after = resp.header("Retry-After").and_then(|h| h.trim().parse().ok());
        let body = resp.into_json::<Value>().unwrap_or(Value::Null);
        Reply { status, body, retry_after }
    }
}

/// Le transport vers le relais : HTTPS en production ([`UreqHttp`]), un faux
/// relais dans les essais.
pub trait RelayHttp: Send + Sync {
    /// `path` commence par `/v1/` ; `bearer` : la session d'appareil.
    fn call(&self, relay: &RelayAddr, method: &str, path: &str, bearer: Option<&str>, body: Option<&Value>) -> Result<Reply, String>;
}

/// HTTPS vers le relais, avec la même vérification du certificat que le
/// tunnel (autorités publiques, ou empreinte épinglée d'un relais chez soi).
#[derive(Default)]
pub struct UreqHttp;

impl RelayHttp for UreqHttp {
    fn call(&self, relay: &RelayAddr, method: &str, path: &str, bearer: Option<&str>, body: Option<&Value>) -> Result<Reply, String> {
        let agent = crate::relay::relay_agent(relay, Duration::from_secs(5), CALL_TIMEOUT)?;
        let url = format!("https://{}{path}", relay.web_authority());
        let mut req = agent.request(method, &url).set("Accept-Language", i18n::current().code()).set("Accept", "application/json");
        if let Some(b) = bearer {
            req = req.set("Authorization", &format!("Bearer {b}"));
        }
        let r = match body {
            Some(b) => req.send_json(b),
            None => req.call(),
        };
        match r {
            Ok(resp) | Err(ureq::Error::Status(_, resp)) => Ok(Reply::read(resp)),
            Err(e) => Err(e.to_string()),
        }
    }
}

/// Le transport de tous les appels (remplaçable dans les essais du cœur).
fn http() -> Arc<dyn RelayHttp> {
    static HTTP: OnceLock<Arc<dyn RelayHttp>> = OnceLock::new();
    HTTP.get_or_init(|| Arc::new(UreqHttp)).clone()
}

fn call(http: &dyn RelayHttp, relay: &RelayAddr, method: &str, path: &str, bearer: Option<&str>, body: Option<&Value>) -> Result<Reply, AccountError> {
    http.call(relay, method, path, bearer, body).map_err(|e| {
        log::warn!("comptes : relais {} injoignable ({e})", relay.web_authority());
        AccountError::Unreachable(e)
    })
}

/// Un refus du relais. `bearer` : l'appel portait une session d'appareil (un
/// `401` la dit fermée).
fn refusal(r: &Reply, bearer: bool) -> AccountError {
    let code = r.body.get("error").and_then(Value::as_str).unwrap_or_default();
    // Un relais sans comptes ne connaît pas ces routes : une page, pas du JSON.
    if r.body.is_null() && matches!(r.status, 404 | 405) {
        return AccountError::NoAccounts;
    }
    match (r.status, code) {
        (401, _) if bearer => AccountError::SignedOut,
        (404, "unknown_station") => AccountError::UnknownComputer,
        (404, "unknown_code") | (410, _) => AccountError::Expired,
        (403, "denied") => AccountError::Denied,
        (400, "bad_code") => AccountError::BadCode,
        (429, _) => AccountError::TooMany { seconds: r.body.get("retry_after").and_then(Value::as_u64).or(r.retry_after) },
        (s, "") => AccountError::Other(format!("HTTP {s}")),
        (s, c) => AccountError::Other(format!("HTTP {s}, {c}")),
    }
}

/// La session d'appareil que rend le relais (`/v1/devices/token`, `/poll`).
fn opened(relay: &RelayAddr, body: &Value) -> Result<Account, AccountError> {
    #[derive(Deserialize)]
    struct Opened {
        token: String,
        account: AccountUser,
        #[serde(default)]
        device: AccountDevice,
    }
    let o: Opened = serde_json::from_value(body.clone()).map_err(|e| AccountError::Other(e.to_string()))?;
    if o.token.is_empty() || o.account.id.is_empty() {
        return Err(AccountError::Other(String::from("session d'appareil vide")));
    }
    log::info!("comptes : connecté à {} sur {} ({})", o.account.id, relay.web_authority(), o.device.name);
    Ok(Account { relay: relay.to_string(), token: o.token, account: o.account, device: o.device })
}

fn device_name(name: &str) -> String {
    let n: String = name.chars().filter(|c| !c.is_control()).collect::<String>().trim().chars().take(64).collect();
    if n.is_empty() { DEFAULT_DEVICE_NAME.to_string() } else { n }
}

// MARK: - Fenêtre du système

/// Une connexion par la fenêtre du système : l'adresse à ouvrir, et `state`
/// à retrouver au retour (`etat`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SignIn {
    pub relay: String,
    pub url: String,
    pub state: String,
    /// `kybernow` : le schéma du retour (`ASWebAuthenticationSession`).
    pub callback_scheme: String,
    /// PKCE (RFC 7636) : le vérificateur, gardé par l'app avec le reste et
    /// rendu à l'échange du code ; seul son défi (SHA-256) part dans
    /// l'adresse. Une autre app qui intercepterait le retour
    /// `kybernow://compte?code=…` ne peut pas échanger le code. Absent (un
    /// `SignIn` gardé par une app d'avant) : rien n'est envoyé.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub verifier: String,
}

/// L'adresse de la page de connexion du relais pour cet appareil
/// (`device_name` : « iPad de Léa », montré dans « Connecter KyberNow Link sur
/// {appareil} ? » et « Appareils connectés »). `relay` vide : le relais public.
pub fn sign_in(relay: &str, device_name: &str) -> Result<SignIn, AccountError> {
    let addr = relay_addr(relay)?;
    let state = new_state();
    let verifier = new_verifier();
    // Un relais d'avant PKCE ignore `code_challenge` et `code_verifier`.
    let url = format!(
        "https://{}/compte/connexion?appareil={}&retour={}&etat={state}&code_challenge={}&code_challenge_method=S256",
        addr.web_authority(),
        encode(&self::device_name(device_name)),
        encode(RETURN_URI),
        pkce_challenge(&verifier),
    );
    Ok(SignIn { relay: addr.to_string(), url, state, callback_scheme: RETURN_SCHEME.to_string(), verifier })
}

/// Le retour de la fenêtre du système (`kybernow://compte?code=…&etat=…`, ou
/// `?erreur=annule`) : le code est échangé contre la session d'appareil.
/// Bloquant (8 s au plus).
pub fn finish_sign_in(sign_in: &SignIn, callback: &str) -> Result<Account, AccountError> {
    finish_sign_in_with(&*http(), sign_in, callback)
}

fn finish_sign_in_with(http: &dyn RelayHttp, sign_in: &SignIn, callback: &str) -> Result<Account, AccountError> {
    let query = callback_query(callback).ok_or(AccountError::BadCode)?;
    let get = |k: &str| query.iter().find(|(n, _)| n == k).map(|(_, v)| v.as_str());
    if get("etat").unwrap_or_default() != sign_in.state {
        log::warn!("comptes : retour de la fenêtre du système avec un autre état, écarté");
        return Err(AccountError::BadCode);
    }
    if get("erreur").is_some() {
        return Err(AccountError::Cancelled);
    }
    let code = get("code").filter(|c| !c.is_empty() && c.len() <= 128).ok_or(AccountError::BadCode)?;
    let addr = relay_addr(&sign_in.relay)?;
    let mut body = json!({ "code": code });
    if !sign_in.verifier.is_empty() {
        body["code_verifier"] = json!(sign_in.verifier);
    }
    let r = call(http, &addr, "POST", "/v1/devices/token", None, Some(&body))?;
    match r.status {
        200 => opened(&addr, &r.body),
        _ => Err(refusal(&r, false)),
    }
}

/// Est-ce un retour de la fenêtre du système (`kybernow://compte…`) ? Pour
/// les apps qui reçoivent tous les liens `kybernow://` au même endroit.
pub fn is_callback(url: &str) -> bool {
    callback_query(url).is_some()
}

/// Les paramètres d'un retour `kybernow://compte[/]?…`, décodés.
fn callback_query(url: &str) -> Option<Vec<(String, String)>> {
    let url = url.trim();
    let (head, query) = url.split_once('?').unwrap_or((url, ""));
    let head = head.trim_end_matches('/');
    if !head.eq_ignore_ascii_case(RETURN_URI) {
        return None;
    }
    let query = query.split('#').next().unwrap_or_default();
    Some(crate::url::query_pairs(query))
}

/// `etat` : 16 octets au hasard, en hexadécimal (le relais garde 128 signes
/// d'adresse au plus).
fn new_state() -> String {
    let mut b = [0u8; 16];
    if getrandom::getrandom(&mut b).is_err() {
        // Sans hasard du système, l'heure : `etat` lie seulement le retour à la demande.
        let n = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_nanos();
        b.copy_from_slice(&n.to_le_bytes());
    }
    b.iter().map(|x| format!("{x:02x}")).collect()
}

/// Vérificateur PKCE : 32 octets au hasard en base64url sans remplissage
/// (43 signes, RFC 7636).
fn new_verifier() -> String {
    use base64::Engine;
    let mut b = [0u8; 32];
    if getrandom::getrandom(&mut b).is_err() {
        // Sans hasard du système : l'heure et l'état, encore uniques.
        let n = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_nanos();
        b[..16].copy_from_slice(&n.to_le_bytes());
    }
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(b)
}

/// Défi PKCE (`S256`) : base64url sans remplissage du SHA-256 du vérificateur.
fn pkce_challenge(verifier: &str) -> String {
    use base64::Engine;
    use sha2::{Digest, Sha256};
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()))
}

/// Encodage d'un paramètre d'adresse (RFC 3986, caractères non réservés gardés).
fn encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~') {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

// MARK: - Code de téléviseur

/// Un code de téléviseur (`POST /v1/devices/code`) : à afficher avec un QR
/// code de `url_complete`, et `url` à taper sur le téléphone.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TvCode {
    pub relay: String,
    /// `XXXX-XXXX`.
    pub code: String,
    /// Le jeton de la demande (secret : seul cet appareil le connaît).
    pub poll: String,
    /// Validité du code, en secondes (600).
    pub expires: u64,
    /// Intervalle des demandes, en secondes (5).
    pub interval: u64,
    /// `https://share.kybernow.app/tv` : la page où taper le code.
    pub url: String,
    /// La même avec le code : le QR code.
    pub url_complete: String,
    /// `share.kybernow.app/tv`, à écrire à l'écran.
    pub url_label: String,
    /// Quand le code a été donné (secondes Unix de l'appareil) : pour « Le code
    /// change dans {n} min. ».
    #[serde(default)]
    pub issued_at: u64,
}

impl TvCode {
    /// Minutes avant l'échéance (au moins 1 tant qu'il vaut), arrondies au-dessus.
    pub fn minutes_left(&self, now: u64) -> u64 {
        let end = self.issued_at + self.expires;
        if now >= end { 0 } else { (end - now).div_ceil(60).max(1) }
    }

    /// Le code n'est plus bon : en demander un autre.
    pub fn expired(&self, now: u64) -> bool {
        now >= self.issued_at + self.expires
    }
}

/// Demande un code pour cet appareil (`device_name` : « Apple TV du salon »,
/// montré dans « Connecter {appareil} ? »). Bloquant.
pub fn tv_code(relay: &str, device_name: &str) -> Result<TvCode, AccountError> {
    tv_code_with(&*http(), relay, device_name)
}

fn tv_code_with(http: &dyn RelayHttp, relay: &str, name: &str) -> Result<TvCode, AccountError> {
    let addr = relay_addr(relay)?;
    let r = call(http, &addr, "POST", "/v1/devices/code", None, Some(&json!({ "name": device_name(name) })))?;
    if r.status != 200 {
        return Err(refusal(&r, false));
    }
    let s = |k: &str| r.body.get(k).and_then(Value::as_str).unwrap_or_default().to_string();
    let n = |k: &str, d: u64| r.body.get(k).and_then(Value::as_u64).unwrap_or(d);
    let (code, poll, url) = (s("code"), s("poll"), s("url"));
    if code.is_empty() || poll.is_empty() {
        return Err(AccountError::Other(String::from("code absent")));
    }
    let url = if url.is_empty() { format!("https://{}/tv", addr.web_authority()) } else { url };
    let url_complete = match s("url_complete") {
        c if c.is_empty() => format!("https://{}/compte/appareil?code={code}", addr.web_authority()),
        c => c,
    };
    let url_label = url.trim_start_matches("https://").trim_start_matches("http://").trim_end_matches('/').to_string();
    Ok(TvCode { relay: addr.to_string(), code, poll, expires: n("expires", 600), interval: n("interval", 5).max(1), url, url_complete, url_label, issued_at: now_secs() })
}

/// La réponse d'une demande.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TvPoll {
    /// Pas encore approuvé : redemander dans `interval` secondes.
    Pending,
    /// Approuvé sur le téléphone : la session d'appareil.
    Approved(Account),
}

/// Une demande (`POST /v1/devices/poll`). Refus : [`AccountError::Denied`]
/// (« Ce n'est pas moi »), [`AccountError::Expired`] (en demander un autre),
/// [`AccountError::TooMany`] (attendre). Bloquant.
pub fn tv_poll(code: &TvCode) -> Result<TvPoll, AccountError> {
    tv_poll_with(&*http(), code)
}

fn tv_poll_with(http: &dyn RelayHttp, code: &TvCode) -> Result<TvPoll, AccountError> {
    let addr = relay_addr(&code.relay)?;
    let r = call(http, &addr, "POST", "/v1/devices/poll", None, Some(&json!({ "poll": code.poll })))?;
    match r.status {
        200 => opened(&addr, &r.body).map(TvPoll::Approved),
        202 => Ok(TvPoll::Pending),
        _ => Err(refusal(&r, false)),
    }
}

/// Attend l'approbation du code (une demande toutes les `interval` secondes,
/// 5 au moins), jusqu'à son échéance ou `cancel` : la session d'appareil, ou
/// le refus. Un relais muet un moment ne l'arrête pas (le code vaut 10 minutes).
/// Bloquant : sur un fil à part.
pub fn tv_wait(code: &TvCode, cancel: &AtomicBool) -> Result<Account, AccountError> {
    tv_wait_with(&*http(), code, cancel, Duration::from_millis(250), MIN_POLL)
}

fn tv_wait_with(http: &dyn RelayHttp, code: &TvCode, cancel: &AtomicBool, tick: Duration, min: Duration) -> Result<Account, AccountError> {
    let start = Instant::now();
    let life = Duration::from_secs(code.expires.max(1));
    let mut wait = Duration::from_secs(code.interval).max(min);
    loop {
        // Attendre l'intervalle, en regardant « Annuler ».
        let until = Instant::now() + wait;
        while Instant::now() < until {
            if cancel.load(Ordering::Relaxed) {
                return Err(AccountError::Cancelled);
            }
            std::thread::sleep(tick.min(until.saturating_duration_since(Instant::now())));
        }
        if cancel.load(Ordering::Relaxed) {
            return Err(AccountError::Cancelled);
        }
        if start.elapsed() >= life {
            return Err(AccountError::Expired);
        }
        wait = Duration::from_secs(code.interval).max(min);
        match tv_poll_with(http, code) {
            Ok(TvPoll::Approved(a)) => return Ok(a),
            Ok(TvPoll::Pending) => {}
            Err(AccountError::TooMany { seconds }) => wait = Duration::from_secs(seconds.unwrap_or(30)).max(min),
            Err(AccountError::Unreachable(e)) => log::info!("comptes : code de téléviseur, relais muet ({e}), on redemande"),
            Err(e) => return Err(e),
        }
    }
}

// MARK: - Se déconnecter

/// « Se déconnecter » : la session d'appareil est fermée chez le relais
/// (`POST /v1/devices/revoke`). Déjà fermée (`401`) : c'est fait. L'app
/// oublie le compte même si le relais ne répond pas. Bloquant.
pub fn sign_out(account: &Account) -> Result<(), AccountError> {
    sign_out_with(&*http(), account)
}

fn sign_out_with(http: &dyn RelayHttp, account: &Account) -> Result<(), AccountError> {
    let addr = account.addr()?;
    let r = call(http, &addr, "POST", "/v1/devices/revoke", Some(&account.token), Some(&json!({})))?;
    match r.status {
        200 | 401 => {
            log::info!("comptes : déconnecté de {} sur {}", account.account.id, addr.web_authority());
            Ok(())
        }
        _ => Err(refusal(&r, true)),
    }
}

// MARK: - Ordinateurs du compte

/// Le propriétaire d'un ordinateur : la personne, ou l'organisation (lot 5).
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Owner {
    /// `person` ou `organization`.
    #[serde(default)]
    pub kind: String,
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub name: String,
}

/// Un ordinateur du compte (`GET /v1/computers`).
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct AccountComputer {
    /// Le compte qui le donne ([`Account::key`]).
    pub account: String,
    /// Le relais du compte (forme normale) : celui de la Station.
    pub relay: String,
    /// L'identifiant de la Station chez le relais.
    pub station: String,
    pub name: String,
    /// `view`, `session`, `lend`, `share`, `admin` (un mot inconnu est gardé, sans effet).
    #[serde(default)]
    pub rights: Vec<String>,
    #[serde(default)]
    pub owner: Owner,
    /// Rattaché à ce compte (et non ouvert par quelqu'un d'autre).
    #[serde(default)]
    pub mine: bool,
    /// Connecté au relais en ce moment.
    #[serde(default)]
    pub online: bool,
    /// L'empreinte d'Admin, vérifiée (signée par la Station) ; gardée par
    /// l'app d'une liste à l'autre ([`remember_fingerprints`]) : le relais ne
    /// la donne que pour une Station connectée.
    #[serde(default)]
    pub admin_fp: Option<String>,
    #[serde(default)]
    pub attached_at: i64,
}

impl AccountComputer {
    /// Le compte a ce droit sur l'ordinateur.
    pub fn can(&self, right: &str) -> bool {
        self.rights.iter().any(|r| r == right)
    }

    /// « Regarder » est proposé (droit `view`, docs/comptes.md) : sur la carte
    /// à la place de « Se connecter » quand c'est le seul accès au bureau
    /// ([`Self::watch_only`]), sinon dans le menu de l'ordinateur. Il passe
    /// par [`crate::share::watch_account`].
    pub fn can_watch(&self) -> bool {
        self.can("view")
    }

    /// Le droit `view` sans `session` : la carte propose « Regarder » à la
    /// place de « Se connecter » (docs/link-ui.md, « Comptes KyberNow »).
    pub fn watch_only(&self) -> bool {
        self.can("view") && !self.can("session")
    }

    /// La pastille d'origine (docs/link-ui.md) : « Compte » (avec le relais
    /// quand l'appareil est connecté à plusieurs relais), le nom de
    /// l'organisation, « Ouvert par {prénom} ».
    pub fn origin(&self, lang: Lang, several_relays: bool) -> String {
        if self.owner.kind == "organization" && !self.owner.name.is_empty() {
            return self.owner.name.clone();
        }
        if !self.mine && !self.owner.name.is_empty() {
            let first = self.owner.name.split_whitespace().next().unwrap_or(&self.owner.name);
            return tr(lang, "account.origin_shared", &[("name", first)]);
        }
        if several_relays {
            tr(lang, "account.origin_account_relay", &[("relay", &relay_label(&self.relay))])
        } else {
            tr(lang, "account.origin_account", &[])
        }
    }
}

/// La pastille d'un ordinateur appairé sur cet appareil (montrée seulement
/// quand un compte est connecté).
pub fn origin_paired(lang: Lang) -> String {
    tr(lang, "account.origin_paired", &[])
}

/// Les ordinateurs du compte. [`AccountError::SignedOut`] : la session de
/// l'appareil est fermée, l'app oublie le compte. Bloquant.
pub fn computers(account: &Account) -> Result<Vec<AccountComputer>, AccountError> {
    computers_with(&*http(), account)
}

fn computers_with(http: &dyn RelayHttp, account: &Account) -> Result<Vec<AccountComputer>, AccountError> {
    let addr = account.addr()?;
    let r = call(http, &addr, "GET", "/v1/computers", Some(&account.token), None)?;
    if r.status != 200 {
        return Err(refusal(&r, true));
    }
    let list = r.body.get("computers").and_then(Value::as_array).cloned().unwrap_or_default();
    Ok(list.iter().filter_map(|c| computer(account, c)).collect())
}

fn computer(account: &Account, c: &Value) -> Option<AccountComputer> {
    let s = |k: &str| c.get(k).and_then(Value::as_str).unwrap_or_default().to_string();
    let station = s("station");
    if !kybernow_tunnel::id::is_station_id(&station) {
        return None;
    }
    // L'empreinte n'est prise que si la Station l'a signée (la clé donne l'identifiant).
    let (fp, key, sig) = (s("admin_fp"), s("station_key"), s("sig"));
    let admin_fp = if fp.is_empty() {
        None
    } else if kybernow_tunnel::lookup::verify_admin_fp(&station, &key, &fp, &sig) {
        Some(fp)
    } else {
        log::warn!("comptes : empreinte d'Admin de {station} dont la signature ne tient pas, écartée");
        None
    };
    Some(AccountComputer {
        account: account.key(),
        relay: account.relay.clone(),
        name: s("name"),
        rights: c.get("rights").and_then(|r| serde_json::from_value(r.clone()).ok()).unwrap_or_default(),
        owner: c.get("owner").and_then(|o| serde_json::from_value(o.clone()).ok()).unwrap_or_default(),
        mine: c.get("mine").and_then(Value::as_bool).unwrap_or(false),
        online: c.get("online").and_then(Value::as_bool).unwrap_or(false),
        attached_at: c.get("attached_at").and_then(Value::as_i64).unwrap_or(0),
        admin_fp,
        station,
    })
}

/// Garde les empreintes d'Admin déjà vues (le relais ne les donne que pour une
/// Station connectée) : `fresh`, la liste qui arrive ; `known`, celle gardée.
pub fn remember_fingerprints(fresh: &mut [AccountComputer], known: &[AccountComputer]) {
    for c in fresh.iter_mut().filter(|c| c.admin_fp.is_none()) {
        c.admin_fp = known.iter().find(|k| k.relay == c.relay && k.station == c.station).and_then(|k| k.admin_fp.clone());
    }
}

// MARK: - Un ordinateur une seule fois

/// Un ordinateur appairé tel que l'app le garde : son identifiant chez l'app,
/// l'empreinte d'Admin épinglée, et son identifiant de Station chez le relais
/// (`info.relay.id`) s'il est connu.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Paired {
    pub id: String,
    #[serde(default)]
    pub fingerprint: Option<String>,
    #[serde(default)]
    pub station: Option<String>,
}

/// L'accueil, un ordinateur une fois : les appairés restent (l'app les ouvre
/// comme avant ; `linked` donne le compte qui les a aussi, pour la pastille),
/// et `added` les ordinateurs des comptes qui ne sont pas déjà là, sans
/// doublon d'un compte à l'autre.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Merged {
    /// (identifiant de l'appairé, l'ordinateur du compte qui est le même).
    pub linked: Vec<(String, AccountComputer)>,
    pub added: Vec<AccountComputer>,
}

/// Même empreinte d'Admin (la règle commune, MAC-20 : sans égard au préfixe
/// `sha256:`, aux deux-points, aux espaces ni à la casse), ou même Station
/// chez le même relais.
fn same(fp: Option<&str>, station: Option<&str>, c: &AccountComputer) -> bool {
    let by_fp = kybernow_rules::fingerprint::same_opt(fp, c.admin_fp.as_deref());
    let by_station = matches!(station, Some(s) if !s.is_empty() && s == c.station);
    by_fp || by_station
}

pub fn merge(paired: &[Paired], computers: &[AccountComputer]) -> Merged {
    let mut out = Merged::default();
    for c in computers {
        if let Some(p) = paired.iter().find(|p| same(p.fingerprint.as_deref(), p.station.as_deref(), c)) {
            if !out.linked.iter().any(|(id, _)| id == &p.id) {
                out.linked.push((p.id.clone(), c.clone()));
            }
            continue;
        }
        if out.added.iter().any(|a| same(a.admin_fp.as_deref(), Some(&a.station), c)) {
            continue;
        }
        out.added.push(c.clone());
    }
    out
}

// MARK: - Jeton d'accès

/// Ce qu'il faut pour ouvrir un ordinateur du compte : le compte et la
/// Station (`SessionParams::account`, ponts).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AccessParams {
    pub account: Account,
    pub station: String,
}

impl AccessParams {
    /// Pour ouvrir cet ordinateur du compte.
    pub fn new(account: &Account, computer: &AccountComputer) -> Self {
        Self { account: account.clone(), station: computer.station.clone() }
    }
}

struct Cached {
    token: String,
    at: Instant,
    /// Échéance donnée par le relais (secondes Unix).
    expires: u64,
    renew: Duration,
}

/// Les jetons d'accès d'un appareil pour une Station (`POST /v1/access`),
/// valables 2 minutes (5 avec un relais plus ancien) : le même sert à tous les
/// appels jusqu'à 30 secondes de son échéance (4 minutes au plus), puis un
/// autre est demandé ([`Self::token`]) ; redémarrer ou mettre à jour
/// la Station en prend un à part, qui ne sert qu'une fois ([`Self::single_use`]).
pub struct StationAccess {
    account: Account,
    station: String,
    http: Arc<dyn RelayHttp>,
    renew: Duration,
    cache: Mutex<Option<Cached>>,
    /// Une demande de jeton à la fois (jamais sous le verrou `cache` : les
    /// autres appels continuent avec le jeton en cours, COEUR-34).
    fetching: Mutex<()>,
    /// Un renouvellement en avance tourne sur un fil à part.
    refreshing: std::sync::atomic::AtomicBool,
    me: std::sync::Weak<StationAccess>,
}

impl fmt::Debug for StationAccess {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("StationAccess").field("account", &self.account.key()).field("station", &self.station).finish()
    }
}

impl StationAccess {
    pub fn new(params: AccessParams) -> Arc<Self> {
        Self::with_http(params, http(), RENEW_AFTER)
    }

    pub fn with_http(params: AccessParams, http: Arc<dyn RelayHttp>, renew: Duration) -> Arc<Self> {
        Arc::new_cyclic(|me| Self {
            account: params.account,
            station: params.station,
            http,
            renew,
            cache: Mutex::new(None),
            fetching: Mutex::new(()),
            refreshing: std::sync::atomic::AtomicBool::new(false),
            me: me.clone(),
        })
    }

    pub fn account(&self) -> &Account {
        &self.account
    }

    pub fn station(&self) -> &str {
        &self.station
    }

    /// Le jeton en cours. Passé son délai de renouvellement, il sert encore
    /// (il vaut au moins 30 secondes de plus) pendant qu'un neuf est demandé
    /// sur un fil à part : aucun appel n'attend le relais (COEUR-34). Sans
    /// jeton valable (premier appel, appareil qui a dormi), bloquant, une
    /// demande à la fois.
    pub fn token(&self) -> Result<String, AccountError> {
        if let Some(t) = self.cached_token() {
            return Ok(t);
        }
        let _one = self.fetching.lock().unwrap_or_else(|e| e.into_inner());
        // Un autre appel vient peut-être d'en obtenir un.
        if let Some(t) = self.cached_token() {
            return Ok(t);
        }
        let fresh = self.fetch()?;
        let token = fresh.token.clone();
        *self.cache.lock().unwrap_or_else(|e| e.into_inner()) = Some(fresh);
        Ok(token)
    }

    /// Le jeton gardé s'il vaut encore ; renouvelé en avance s'il le faut.
    fn cached_token(&self) -> Option<String> {
        let cache = self.cache.lock().unwrap_or_else(|e| e.into_inner());
        let c = cache.as_ref().filter(|c| now_secs() + EXPIRY_MARGIN < c.expires)?;
        let token = c.token.clone();
        let stale = c.at.elapsed() >= c.renew;
        drop(cache);
        if stale {
            self.refresh_in_background();
        }
        Some(token)
    }

    fn refresh_in_background(&self) {
        if self.refreshing.swap(true, std::sync::atomic::Ordering::SeqCst) {
            return;
        }
        let Some(me) = self.me.upgrade() else {
            self.refreshing.store(false, std::sync::atomic::Ordering::SeqCst);
            return;
        };
        let spawned = std::thread::Builder::new().name("kybernow-jeton".into()).spawn(move || {
            {
                let _one = me.fetching.lock().unwrap_or_else(|e| e.into_inner());
                match me.fetch() {
                    Ok(fresh) => *me.cache.lock().unwrap_or_else(|e| e.into_inner()) = Some(fresh),
                    Err(e) => log::warn!("comptes : jeton d'accès pas renouvelé en avance ({})", e.kind()),
                }
            }
            me.refreshing.store(false, std::sync::atomic::Ordering::SeqCst);
        });
        if spawned.is_err() {
            self.refreshing.store(false, std::sync::atomic::Ordering::SeqCst);
        }
    }

    /// Un jeton neuf, pas gardé : Admin consomme celui qui redémarre ou met à
    /// jour la Station.
    pub fn single_use(&self) -> Result<String, AccountError> {
        self.fetch().map(|c| c.token)
    }

    /// Admin a refusé le jeton (`401 token_expired`, `bad_token`) : le suivant
    /// sera neuf.
    pub fn invalidate(&self) {
        *self.cache.lock().unwrap_or_else(|e| e.into_inner()) = None;
    }

    fn fetch(&self) -> Result<Cached, AccountError> {
        let addr = self.account.addr()?;
        let r = call(&*self.http, &addr, "POST", "/v1/access", Some(&self.account.token), Some(&json!({ "station": self.station })))?;
        if r.status != 200 {
            let e = refusal(&r, true);
            log::warn!("comptes : pas de jeton d'accès pour {} ({})", self.station, e.kind());
            return Err(e);
        }
        let token = r.body.get("token").and_then(Value::as_str).unwrap_or_default().to_string();
        if token.is_empty() {
            return Err(AccountError::Other(String::from("jeton absent")));
        }
        let lifetime = r.body.get("lifetime").and_then(Value::as_u64).unwrap_or(300);
        // Échéance sur l'horloge de l'appareil, d'après la durée de vie : celle
        // du relais (`expires`) ne vaut que si les deux horloges s'accordent ;
        // un appareil en avance de plus de 90 s (Raspberry Pi sans horloge
        // sauvegardée) redemandait un jeton à chaque appel. L'horloge murale
        // compte le sommeil de l'appareil, contrairement à `Instant`.
        let expires = now_secs() + lifetime;
        // Redemandé 30 secondes avant son échéance (90 secondes pour un jeton de 2 minutes), 4 minutes au plus.
        let renew = self.renew.min(Duration::from_secs(lifetime.saturating_sub(EXPIRY_MARGIN)));
        log::debug!("comptes : jeton d'accès pour {} ({} s)", self.station, lifetime);
        Ok(Cached { token, at: Instant::now(), expires, renew })
    }
}

// MARK: - Réglages synchronisés

#[path = "account/settings.rs"]
pub mod settings;

pub use settings::{AccountSync, SyncedSettings, SyncedValues, fetch_settings, put_settings};

/// L'interrupteur des réglages de l'app, section « Comptes » (allumé par
/// défaut, gardé sur l'appareil) : « Synchroniser les réglages des
/// ordinateurs » et son aide.
pub fn sync_settings_texts(lang: Lang) -> (String, String) {
    (tr(lang, "account.sync_settings", &[]), tr(lang, "account.sync_settings_help", &[]))
}

/// L'heure de l'appareil, en secondes Unix.
pub fn now_secs() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

// Chemin explicite : le module se compile aussi seul (essais sans Kyber).
#[cfg(test)]
#[path = "account/tests.rs"]
mod tests;
