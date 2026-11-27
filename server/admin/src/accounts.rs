//! Comptes KyberNow côté Station (docs/comptes.md, « Les quatre garde-fous de
//! la Station ») : le compte rattaché et la clé du relais épinglée,
//! l'interrupteur « Accepter les comptes KyberNow », la vérification des
//! jetons d'accès (`Authorization: KyberNow <jeton>`, format dans
//! `kybernow_tunnel::access`) et la liste locale des comptes et appareils
//! entrés par un jeton, avec leur blocage.
//!
//! Le relais décide de l'accès ; la Station garde le dernier mot :
//! interrupteur coupé, Station pas rattachée, jeton mal signé, échu, pour une
//! autre Station ou déjà rejoué, compte ou appareil bloqué ici : refusé, même
//! si le relais a dit oui.
//!
//! Le rattachement (docs/comptes.md, « Rattacher une Station ») passe par la
//! connexion de contrôle de l'agent du relais : « Se connecter »
//! (`POST /api/account/attach`) demande un code (`attach_request`) et rend la
//! page de la console à ouvrir ; la personne y confirme, le relais envoie
//! `attached` ([`on_attached`] : [`pin`], « Accepter les comptes KyberNow » et
//! « Accès hors de chez vous » allumés). Un `attached` que personne n'a
//! demandé ici (pas de « Se connecter » depuis 15 minutes) n'est jamais pris,
//! sauf pour mettre à jour les clés du même rattachement. « Détacher » le dit
//! au relais ; détachée dans la console, le relais envoie `detached`
//! ([`on_detached`]). Le mode d'essai ([`TEST_ENV`]) épingle une clé locale, et
//! `kyber-admin account-token` signe des jetons avec elle.

use std::collections::HashMap;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail};
use axum::Json;
use axum::extract::{ConnectInfo, Path as UrlPath, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{IntoResponse, Response};
use kybernow_tunnel::TunnelError;
use kybernow_tunnel::access::{self, AccessError, Claims, KeySet, PublicKey, Right, Rights, Use, Verifier};
use kybernow_tunnel::msg::{Attached, ErrorCode};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use crate::AppState;
use crate::display::now_secs;
use crate::i18n::Msg;

/// La liste locale, dans le dossier de données (réservé à SYSTEM et aux Administrateurs).
pub const FILE: &str = "account-access.json";
/// Mode d'essai : `KYBERNOW_ACCOUNT_TEST=1` au démarrage d'Admin épingle une
/// clé d'essai (`account-test.key`) au relais [`TEST_RELAY`], tant que la
/// Station n'est rattachée à aucun relais.
pub const TEST_ENV: &str = "KYBERNOW_ACCOUNT_TEST";
/// Le relais du mode d'essai (une adresse qui ne mène nulle part).
pub const TEST_RELAY: &str = "essai.kybernow.invalid";
/// La clé d'essai (PKCS#8 en DER), dans le dossier de données.
pub const TEST_KEY_FILE: &str = "account-test.key";
/// Durée de vie d'un jeton de Kyber donné à un compte qui ouvre une session :
/// celle du jeton d'accès, 2 minutes (Kyber ne le lit qu'à la connexion ; le
/// signe de vie, toutes les 5 secondes, en donne un neuf).
pub const KYBER_TOKEN_LIFETIME: i64 = access::LIFETIME_SECS as i64;
/// Une entrée vue de nouveau est réécrite sur le disque au plus toutes les 5 min.
const SAVE_SEEN_EVERY: u64 = 5 * 60;
/// Un appel arrivé par le relais l'est encore si son appel vérifié par la
/// garde date de moins que ceci (la garde vérifie juste avant la route).
const RELAY_MARK: Duration = Duration::from_secs(2);
/// Entrées gardées au plus (les plus anciennes non bloquées partent d'abord).
const MAX_ENTRIES: usize = 500;

// --- Réglages : `[account]` de kyber-admin.toml ------------------------------------------

/// À qui la Station est rattachée : une personne ou une organisation.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OwnerKind {
    #[default]
    Person,
    Organization,
}

/// Le propriétaire, tel que le relais l'a dit au rattachement.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Owner {
    #[serde(default)]
    pub kind: OwnerKind,
    /// L'identifiant du compte (ou de l'organisation) chez le relais.
    pub id: String,
    /// Son nom (« Les Lignes Claires »), ou celui de la personne.
    #[serde(default)]
    pub name: String,
    /// L'adresse e-mail du compte (une personne).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
}

/// Le compte rattaché (`[account]` de kyber-admin.toml).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AccountSettings {
    /// Le relais du rattachement (forme normale de son adresse) ; absent : pas rattachée.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub relay: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub owner: Option<Owner>,
    /// Les clés du relais épinglées au rattachement (deux pendant une rotation).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub keys: Vec<PublicKey>,
    /// Date du rattachement (secondes Unix).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pinned_at: Option<u64>,
    /// « Accepter les comptes KyberNow ».
    #[serde(default)]
    pub accept: bool,
    /// Rattachée par le mode d'essai.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub test: bool,
    /// Lot 5 : un jeton d'enrôlement d'organisation (`kne_…`), déposé par
    /// l'installateur en silencieux (`/enroll=`) ; présenté au relais dès qu'il
    /// est joint, puis effacé (enrôlée, ou refusé pour de bon).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enroll: Option<String>,
}

impl AccountSettings {
    /// Rattachée : un relais, un propriétaire et au moins une clé lisible.
    pub fn attached(&self) -> bool {
        self.relay.is_some() && self.owner.is_some() && self.keys.iter().any(PublicKey::is_valid)
    }
}

// --- Liste locale ------------------------------------------------------------------

/// Un compte et un appareil entrés par un jeton.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Entry {
    /// Identifiant local (d'après le compte et l'appareil).
    pub id: String,
    pub account: String,
    pub device: String,
    /// Le nom de la personne.
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
    pub device_name: String,
    /// Les droits du dernier jeton.
    pub rights: Rights,
    pub first_seen: u64,
    pub last_seen: u64,
    /// Par où le dernier appel est venu : `lan` ou `relay`.
    pub via: String,
    /// Bloqué ici (date), cet appareil seulement.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub blocked: Option<u64>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct Stored {
    #[serde(default)]
    entries: Vec<Entry>,
    /// Comptes bloqués en entier (tous leurs appareils) : identifiant → date.
    #[serde(default)]
    blocked_accounts: HashMap<String, u64>,
}

/// L'identifiant local d'un compte et d'un appareil : 8 octets de SHA-256 en base32.
pub fn entry_id(account: &str, device: &str) -> String {
    let mut h = Sha256::new();
    h.update(b"kn-access/1");
    h.update(account.as_bytes());
    h.update([0]);
    h.update(device.as_bytes());
    kybernow_tunnel::id::base32_lower(&h.finalize()[..8])
}

/// Le compte Kyber sous lequel un compte KyberNow ouvre sa session (jeton de
/// Kyber, jamais un compte écrit dans la configuration de Kyber).
pub fn kyber_user(account: &str) -> String {
    let mut h = Sha256::new();
    h.update(b"kn-account/1");
    h.update(account.as_bytes());
    format!("kn-account-{}", kybernow_tunnel::id::base32_lower(&h.finalize()[..10]))
}

/// La liste locale et le vérificateur des jetons.
pub struct Accounts {
    path: PathBuf,
    stored: Mutex<Stored>,
    /// Dernière écriture de chaque entrée vue (secondes), pour ne pas
    /// réécrire le fichier à chaque signe de vie.
    saved_seen: Mutex<HashMap<String, u64>>,
    /// Dernier appel vérifié par la garde du relais, par entrée.
    relay_marks: Mutex<HashMap<String, Instant>>,
    verifier: Mutex<Option<Arc<Verifier>>>,
    /// Lot 4 : « Se connecter avec KyberNow » en cours, par `nonce` (et leur échéance).
    web_logins: Mutex<HashMap<String, Instant>>,
}

impl Accounts {
    pub fn load(data_dir: &Path) -> Accounts {
        let path = data_dir.join(FILE);
        let stored: Stored = crate::store::load_json(&path);
        Accounts {
            path,
            stored: Mutex::new(stored),
            saved_seen: Mutex::new(HashMap::new()),
            relay_marks: Mutex::new(HashMap::new()),
            verifier: Mutex::new(None),
            web_logins: Mutex::new(HashMap::new()),
        }
    }

    fn save(&self, stored: &Stored) {
        let bytes = match serde_json::to_vec_pretty(stored) {
            Ok(b) => b,
            Err(e) => return tracing::warn!("comptes : liste locale illisible : {e}"),
        };
        if let Err(e) = crate::durable::write(&self.path, bytes) {
            tracing::warn!("comptes : {} pas écrit : {e}", self.path.display());
        }
    }

    pub fn entries(&self) -> Vec<Entry> {
        self.stored.lock().unwrap().entries.clone()
    }

    pub fn account_blocked(&self, account: &str) -> Option<u64> {
        self.stored.lock().unwrap().blocked_accounts.get(account).copied()
    }

    /// Bloqué ici : l'appareil, ou le compte entier.
    pub fn is_blocked(&self, account: &str, device: &str) -> bool {
        let s = self.stored.lock().unwrap();
        let id = entry_id(account, device);
        s.blocked_accounts.contains_key(account) || s.entries.iter().any(|e| e.id == id && e.blocked.is_some())
    }

    /// Bloque ou débloque une entrée (`whole` : tout son compte). `None` : inconnue.
    pub fn set_blocked(&self, id: &str, blocked: bool, whole: bool) -> Option<Entry> {
        let mut s = self.stored.lock().unwrap();
        let now = now_secs();
        let entry = s.entries.iter_mut().find(|e| e.id == id)?;
        let account = entry.account.clone();
        if !whole || blocked {
            entry.blocked = blocked.then_some(now);
        }
        let entry = entry.clone();
        if whole {
            if blocked {
                s.blocked_accounts.insert(account, now);
            } else {
                // Débloquer le compte débloque aussi chacun de ses appareils.
                s.blocked_accounts.remove(&account);
                for e in s.entries.iter_mut().filter(|e| e.account == account) {
                    e.blocked = None;
                }
            }
        }
        let entry = s.entries.iter().find(|e| e.id == entry.id).cloned();
        self.save(&s);
        entry
    }

    /// Note un appel accepté. Rend vrai pour un appareil jamais vu
    /// (« Avis à chaque nouvel appareil »).
    fn note(&self, claims: &Claims, via_relay: bool) -> (String, bool) {
        let id = entry_id(&claims.account, &claims.device);
        let now = now_secs();
        let via = if via_relay { "relay" } else { "lan" };
        let mut s = self.stored.lock().unwrap();
        let (changed, new) = match s.entries.iter_mut().find(|e| e.id == id) {
            Some(e) => {
                let changed = e.rights != claims.rights
                    || e.via != via
                    || e.name != claims.name.account
                    || e.device_name != claims.name.device
                    || e.email != claims.name.email;
                e.rights = claims.rights.clone();
                e.via = via.into();
                e.name = claims.name.account.clone();
                e.email = claims.name.email.clone();
                e.device_name = claims.name.device.clone();
                e.last_seen = now;
                (changed, false)
            }
            None => {
                if s.entries.len() >= MAX_ENTRIES {
                    // La plus ancienne non bloquée part.
                    if let Some(i) = s.entries.iter().enumerate().filter(|(_, e)| e.blocked.is_none()).min_by_key(|(_, e)| e.last_seen).map(|(i, _)| i) {
                        s.entries.remove(i);
                    }
                }
                s.entries.push(Entry {
                    id: id.clone(),
                    account: claims.account.clone(),
                    device: claims.device.clone(),
                    name: claims.name.account.clone(),
                    email: claims.name.email.clone(),
                    device_name: claims.name.device.clone(),
                    rights: claims.rights.clone(),
                    first_seen: now,
                    last_seen: now,
                    via: via.into(),
                    blocked: None,
                });
                (true, true)
            }
        };
        let mut saved = self.saved_seen.lock().unwrap();
        let stale = saved.get(&id).is_none_or(|t| now.saturating_sub(*t) >= SAVE_SEEN_EVERY);
        if changed || stale {
            saved.insert(id.clone(), now);
            self.save(&s);
        }
        (id, new)
    }

    /// La garde du relais vient de vérifier un jeton de cette entrée.
    fn mark_relay(&self, id: &str) {
        let mut marks = self.relay_marks.lock().unwrap();
        let now = Instant::now();
        marks.retain(|_, t| now.duration_since(*t) < RELAY_MARK);
        marks.insert(id.to_string(), now);
    }

    fn relay_marked(&self, id: &str) -> bool {
        self.relay_marks.lock().unwrap().get(id).is_some_and(|t| t.elapsed() < RELAY_MARK)
    }

    /// Détachée : la liste est vidée.
    fn clear(&self) {
        let mut s = self.stored.lock().unwrap();
        *s = Stored::default();
        self.save(&s);
        self.saved_seen.lock().unwrap().clear();
    }

    /// Le vérificateur, construit d'après les réglages à la première demande.
    fn verifier(&self, s: &AppState) -> Option<Arc<Verifier>> {
        let mut v = self.verifier.lock().unwrap();
        if v.is_none() {
            let settings = s.settings.lock().unwrap().account.clone();
            if settings.attached() {
                let station = s.relay.id()?;
                *v = Some(Arc::new(Verifier::new(settings.relay.as_deref().unwrap_or_default(), &station, &settings.keys)));
            }
        }
        v.clone()
    }

    /// Réglages changés (rattachement, détachement) : le vérificateur sera refait.
    fn reset_verifier(&self) {
        *self.verifier.lock().unwrap() = None;
    }

    /// Oublie de temps en temps les `jti` échus.
    pub fn prune(&self) {
        if let Some(v) = self.verifier.lock().unwrap().as_ref() {
            v.prune(now_secs());
        }
    }
}

// --- Rattachement ------------------------------------------------------------------

/// Épingle le rattachement : le relais, le propriétaire, la ou les clés
/// publiées par le relais (`GET /v1/keys`). Appelé par le rattachement
/// (dialogue avec le relais, à venir) ou par le mode d'essai. Active
/// « Accepter les comptes KyberNow » (docs/comptes.md, « Rattacher une
/// Station ») ; Admin permet de le couper.
pub fn pin(s: &AppState, relay: &str, owner: Owner, keys: &KeySet) -> Result<()> {
    let keys = keys.valid();
    if keys.is_empty() {
        bail!("aucune clé du relais lisible");
    }
    if owner.id.trim().is_empty() {
        bail!("propriétaire sans identifiant");
    }
    let relay = crate::relay::normalize_server(relay).map_err(|m| anyhow::anyhow!(m))?;
    if relay.is_empty() {
        bail!("relais vide");
    }
    let name = owner.email.clone().filter(|e| !e.is_empty()).unwrap_or_else(|| owner.name.clone());
    let test = relay == TEST_RELAY;
    s.update_settings(|st| {
        st.account = AccountSettings { relay: Some(relay.clone()), owner: Some(owner), keys, pinned_at: Some(now_secs()), accept: true, test, enroll: None };
    })?;
    s.accounts.reset_verifier();
    tracing::info!("comptes : Station rattachée à {name} (relais {relay})");
    s.activity.event("info", Msg::new("events.accountAttached").with("name", name));
    Ok(())
}

/// Détache la Station : réglages effacés, interrupteur coupé, liste vidée.
pub fn detach(s: &AppState) -> Result<()> {
    s.update_settings(|st| st.account = AccountSettings::default())?;
    s.accounts.reset_verifier();
    s.accounts.clear();
    s.auth.close_remote_sessions();
    tracing::info!("comptes : Station détachée");
    s.activity.event("info", Msg::new("events.accountDetached"));
    Ok(())
}

/// Le relais réglé dans Admin (« Hors de chez vous ») est-il celui du rattachement ?
fn same_relay(s: &AppState, relay: &str) -> bool {
    s.settings.lock().unwrap().relay.addr().is_some_and(|a| a.web_authority() == relay)
}

/// `attached` reçu du relais (rattachement confirmé dans la console, ou
/// réponse à `attach_status` à chaque connexion).
pub fn on_attached(s: &AppState, a: Attached) {
    let relay = match crate::relay::normalize_server(&a.relay) {
        Ok(r) if !r.is_empty() => r,
        _ => return tracing::warn!("comptes : rattachement ignoré (relais illisible)"),
    };
    let kind = if a.owner.kind == "organization" { OwnerKind::Organization } else { OwnerKind::Person };
    let owner = Owner { kind, id: a.owner.id, name: a.owner.name, email: a.owner.email.filter(|e| !e.is_empty()) };
    let keys = KeySet { keys: a.keys };
    let current = s.settings.lock().unwrap().account.clone();
    let asked = s.relay.attaching();
    let same = current.attached() && !current.test && current.relay.as_deref() == Some(relay.as_str()) && current.owner.as_ref().map(|o| &o.id) == Some(&owner.id);
    if same && !asked {
        // Le même rattachement : les clés (rotation) et les noms suivent le relais.
        let valid = keys.valid();
        if valid.is_empty() || (valid == current.keys && current.owner.as_ref() == Some(&owner)) {
            return;
        }
        if s.update_settings(|st| {
            st.account.keys = valid;
            st.account.owner = Some(owner);
        })
        .is_ok()
        {
            s.accounts.reset_verifier();
            tracing::info!("comptes : clés du relais {relay} mises à jour");
        }
        return;
    }
    let moved = current.attached() && !current.test && current.relay.as_deref() == Some(relay.as_str()) && !keys.valid().is_empty() && keys.valid() == current.keys;
    if moved && !asked {
        // Déplacé dans la console (docs/comptes.md, « Déplacer un ordinateur ») :
        // même relais, mêmes clés déjà épinglées, autre propriétaire. Seul le
        // propriétaire affiché change ; les clés et l'accès restent.
        if s.update_settings(|st| st.account.owner = Some(owner)).is_ok() {
            tracing::info!("comptes : nouveau propriétaire (ordinateur déplacé dans la console du relais {relay})");
        }
        return;
    }
    if !asked {
        // Personne n'a demandé de rattachement ici : jamais pris.
        tracing::warn!("comptes : rattachement non demandé, ignoré (relais {relay})");
        return;
    }
    if let Err(e) = pin(s, &relay, owner, &keys) {
        return tracing::warn!("comptes : rattachement refusé : {e:#}");
    }
    s.relay.attach_done();
    // « Accès hors de chez vous » s'allume avec le rattachement (Admin permet de le couper).
    let was_on = s.settings.lock().unwrap().relay.remote_access;
    if !was_on && s.update_settings(|st| st.relay.remote_access = true).is_ok() {
        tracing::info!("relais : accès hors de chez vous activé (rattachement)");
        s.activity.event("info", Msg::new("events.relayAccessOn"));
    }
    s.relay.wake();
}

/// `detached` reçu du relais : détachée dans la console (ou plus connue du relais).
pub fn on_detached(s: &AppState) {
    let current = s.settings.lock().unwrap().account.clone();
    if !current.attached() || current.test || !current.relay.as_deref().is_some_and(|r| same_relay(s, r)) {
        return;
    }
    tracing::info!("comptes : détachée dans la console du relais");
    if let Err(e) = detach(s) {
        tracing::warn!("comptes : détachement : {e:#}");
    }
}

/// `POST /api/account/attach` (« Se connecter ») : un code de rattachement
/// demandé au relais, et la page de sa console à ouvrir dans le navigateur.
pub async fn attach(State(s): State<AppState>) -> Response {
    let refuse = |status: StatusCode, code: &str, msg: Msg| (status, Json(json!({ "error": msg.text(), "code": code }))).into_response();
    if s.settings.lock().unwrap().relay.addr().is_none() {
        return refuse(StatusCode::CONFLICT, "no_relay", Msg::new("accounts.noRelay"));
    }
    let name = crate::system::computer_name().unwrap_or_default();
    match s.relay.attach_code(&name).await {
        Ok(code) if code.url.starts_with("https://") => {
            tracing::info!("comptes : rattachement demandé au relais");
            Json(json!({ "url": code.url, "expires": code.expires.as_secs() })).into_response()
        }
        Ok(_) => {
            s.relay.attach_done();
            refuse(StatusCode::BAD_GATEWAY, "relay_unreachable", Msg::new("accounts.relayUnreachable"))
        }
        // Un relais sans comptes refuse ; un relais d'avant les comptes ne répond pas.
        Err(TunnelError::Refused(ErrorCode::NoAccounts, _)) | Err(TunnelError::Timeout) => {
            s.relay.attach_done();
            refuse(StatusCode::CONFLICT, "no_accounts", Msg::new("accounts.noAccounts"))
        }
        Err(e) => {
            tracing::info!("comptes : rattachement impossible : {e}");
            s.relay.attach_done();
            refuse(StatusCode::SERVICE_UNAVAILABLE, "relay_unreachable", Msg::new("accounts.relayUnreachable"))
        }
    }
}

/// Lot 5 : présente le jeton d'enrôlement déposé par l'installateur
/// (`[account] enroll`), jusqu'à ce que le relais réponde : enrôlée (rattachée
/// à l'organisation, comptes acceptés, accès hors de chez vous actif), ou
/// refusé pour de bon (jeton inconnu, échu, révoqué, épuisé). Sans relais
/// réglé, ou relais injoignable : nouvel essai, de plus en plus espacé.
pub fn start_enroll(s: &AppState) {
    let token = s.settings.lock().unwrap().account.enroll.clone().filter(|t| !t.trim().is_empty());
    let Some(token) = token else { return };
    let s = s.clone();
    tokio::spawn(async move {
        let mut wait = Duration::from_secs(5);
        loop {
            if s.settings.lock().unwrap().relay.addr().is_some() {
                let name = crate::system::computer_name().unwrap_or_default();
                match s.relay.enroll(token.trim(), &name).await {
                    Ok(attached) => {
                        let org = attached.owner.name.clone();
                        on_attached(&s, attached);
                        let _ = s.update_settings(|st| st.account.enroll = None);
                        tracing::info!("comptes : Station enrôlée dans l'organisation {org}");
                        s.activity.event("info", Msg::new("events.accountEnrolled").with("org", org.as_str()));
                        return;
                    }
                    Err(TunnelError::Refused(ErrorCode::EnrollRefused, why)) | Err(TunnelError::Refused(ErrorCode::NoAccounts, why)) => {
                        let why = why.unwrap_or_default();
                        let _ = s.update_settings(|st| st.account.enroll = None);
                        tracing::warn!("comptes : jeton d'enrôlement refusé ({why})");
                        s.activity.event("warn", Msg::new("events.accountEnrollRefused").with("reason", why.as_str()));
                        return;
                    }
                    Err(e) => tracing::info!("comptes : enrôlement remis à plus tard ({e})"),
                }
            }
            tokio::time::sleep(wait).await;
            wait = (wait * 2).min(Duration::from_secs(600));
        }
    });
}

// --- Routes locales de l'installateur ---------------------------------------------------
//
// `/api/local/account…` : depuis l'ordinateur lui-même, avec le jeton local
// (docs/comptes.md, « Dans les installateurs »). L'installateur de la Station
// y règle le relais choisi, lance le rattachement (la page du relais s'ouvre
// dans sa fenêtre) ou l'enrôlement d'une installation silencieuse. Une Station
// déjà rattachée ne se rattache pas ailleurs par ces routes : c'est Admin
// (« Détacher ») qui le décide.

/// La vue de `GET /api/account`, avec le relais réglé et « Accès hors de chez vous ».
pub fn local_view(s: &AppState) -> Value {
    let mut v = view(s);
    let relay = s.settings.lock().unwrap().relay.clone();
    v["server"] = json!(relay.server());
    v["remote_access"] = json!(relay.remote_access);
    v
}

/// Depuis l'ordinateur lui-même, avec le jeton local (comme toutes les routes `/api/local/*`).
fn local_guard(s: &AppState, peer: SocketAddr, headers: &HeaderMap) -> Result<(), Response> {
    crate::api::local_only(s, peer, headers).map_err(IntoResponse::into_response)
}

/// Rattacher la Station ou changer son relais : l'installateur seulement
/// (jeton lisible par un administrateur, voir `local_token`), jamais un
/// compte Windows standard qui a le jeton local de sa session.
fn installer_guard(s: &AppState, peer: SocketAddr, headers: &HeaderMap) -> Result<(), Response> {
    local_guard(s, peer, headers)?;
    if !s.local_token.accepts_installer(headers) {
        return Err(refuse(StatusCode::FORBIDDEN, "installer_only", Msg::new("accounts.installerOnly")));
    }
    Ok(())
}

fn refuse(status: StatusCode, code: &str, msg: Msg) -> Response {
    (status, Json(json!({ "error": msg.text(), "code": code }))).into_response()
}

/// Règle le relais de la Station (« Hors de chez vous ») avant un rattachement
/// ou un enrôlement ; refusé si elle est déjà rattachée (pas en mode d'essai).
fn set_relay_locally(s: &AppState, relay: Option<&str>) -> Result<(), Response> {
    let current = s.settings.lock().unwrap().account.clone();
    if current.attached() && !current.test {
        return Err(refuse(StatusCode::CONFLICT, "already_attached", Msg::new("accounts.alreadyAttached")));
    }
    let Some(relay) = relay else { return Ok(()) };
    let server = crate::relay::normalize_server(relay).map_err(|msg| refuse(StatusCode::BAD_REQUEST, "bad_relay", msg))?;
    if server.is_empty() {
        return Err(refuse(StatusCode::CONFLICT, "no_relay", Msg::new("accounts.noRelay")));
    }
    let before = s.settings.lock().unwrap().relay.server();
    if before != server {
        s.update_settings(|st| st.relay.server = if server == crate::relay::DEFAULT_SERVER { None } else { Some(server.clone()) })
            .map_err(|e| crate::api::ApiError::from(e).into_response())?;
        tracing::info!("relais : adresse réglée à « {server} » (installateur)");
        s.relay.wake();
    }
    Ok(())
}

#[derive(Deserialize, Default)]
pub struct LocalRelayBody {
    /// Le relais choisi dans l'installateur ; absent : celui qui est réglé.
    #[serde(default)]
    relay: Option<String>,
}

/// `GET /api/local/account` : où en est le rattachement.
pub async fn local_get(State(s): State<AppState>, ConnectInfo(peer): ConnectInfo<SocketAddr>, headers: HeaderMap) -> Response {
    if let Err(r) = local_guard(&s, peer, &headers) {
        return r;
    }
    Json(local_view(&s)).into_response()
}

/// `PUT /api/local/account/relay {relay}` : le relais choisi, sans rattacher (« Plus tard »).
pub async fn local_relay(State(s): State<AppState>, ConnectInfo(peer): ConnectInfo<SocketAddr>, headers: HeaderMap, Json(body): Json<LocalRelayBody>) -> Response {
    if let Err(r) = installer_guard(&s, peer, &headers) {
        return r;
    }
    if body.relay.is_none() {
        return refuse(StatusCode::BAD_REQUEST, "bad_relay", Msg::new("accounts.noRelay"));
    }
    match set_relay_locally(&s, body.relay.as_deref()) {
        Ok(()) => Json(local_view(&s)).into_response(),
        Err(r) => r,
    }
}

/// `POST /api/local/account/attach {relay?}` : comme « Se connecter » dans
/// Admin, la page du relais à ouvrir (`{url, expires}`), pour la fenêtre de l'installateur.
pub async fn local_attach(State(s): State<AppState>, ConnectInfo(peer): ConnectInfo<SocketAddr>, headers: HeaderMap, Json(body): Json<LocalRelayBody>) -> Response {
    if let Err(r) = installer_guard(&s, peer, &headers) {
        return r;
    }
    if let Err(r) = set_relay_locally(&s, body.relay.as_deref()) {
        return r;
    }
    attach(State(s)).await
}

/// `DELETE /api/local/account/attach` : l'installateur abandonne (« Plus tard », « Retour »).
pub async fn local_cancel(State(s): State<AppState>, ConnectInfo(peer): ConnectInfo<SocketAddr>, headers: HeaderMap) -> Response {
    if let Err(r) = local_guard(&s, peer, &headers) {
        return r;
    }
    if !s.settings.lock().unwrap().account.attached() {
        s.relay.attach_done();
    }
    Json(local_view(&s)).into_response()
}

#[derive(Deserialize)]
pub struct EnrollBody {
    /// Le jeton d'enrôlement donné par la console de l'organisation.
    token: String,
    #[serde(default)]
    relay: Option<String>,
}

/// Forme d'un jeton d'enrôlement acceptée avant de le passer au relais : il
/// ne voyage que dans un message JSON, mais un jeton collé de travers
/// (espaces, guillemets, retour à la ligne) se voit tout de suite.
pub fn enroll_token_ok(token: &str) -> bool {
    (8..=256).contains(&token.len()) && token.bytes().all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-' | b'.' | b'~'))
}

/// `POST /api/local/account/enroll {token, relay?}` : installation silencieuse
/// d'une flotte (`/enroll=<jeton>`). Le jeton part au relais par la connexion
/// de contrôle (`enroll`) ; sa réponse `attached` est épinglée comme un
/// rattachement confirmé dans la console.
pub async fn local_enroll(State(s): State<AppState>, ConnectInfo(peer): ConnectInfo<SocketAddr>, headers: HeaderMap, Json(body): Json<EnrollBody>) -> Response {
    if let Err(r) = installer_guard(&s, peer, &headers) {
        return r;
    }
    let token = body.token.trim();
    if !enroll_token_ok(token) {
        return refuse(StatusCode::BAD_REQUEST, "bad_token", Msg::new("accounts.badEnrollToken"));
    }
    if let Err(r) = set_relay_locally(&s, body.relay.as_deref()) {
        return r;
    }
    if s.settings.lock().unwrap().relay.addr().is_none() {
        return refuse(StatusCode::CONFLICT, "no_relay", Msg::new("accounts.noRelay"));
    }
    let name = crate::system::computer_name().unwrap_or_default();
    let answer = s.relay.enroll(token, &name).await;
    let refused = |status, code, key| {
        s.relay.attach_done();
        refuse(status, code, Msg::new(key))
    };
    match answer {
        Ok(a) => {
            on_attached(&s, a);
            s.relay.attach_done();
            if s.settings.lock().unwrap().account.attached() {
                tracing::info!("comptes : Station enrôlée");
                Json(local_view(&s)).into_response()
            } else {
                refuse(StatusCode::BAD_GATEWAY, "relay_unreachable", Msg::new("accounts.relayUnreachable"))
            }
        }
        Err(TunnelError::Refused(ErrorCode::EnrollRefused, why)) => {
            tracing::info!("comptes : jeton d'enrôlement refusé par le relais ({})", why.unwrap_or_default());
            refused(StatusCode::FORBIDDEN, "bad_token", "accounts.badEnrollToken")
        }
        // Un relais sans comptes refuse ; un relais d'avant l'enrôlement ne répond pas.
        Err(TunnelError::Refused(ErrorCode::NoAccounts, _)) | Err(TunnelError::Timeout) => {
            refused(StatusCode::CONFLICT, "no_accounts", "accounts.noAccounts")
        }
        Err(e) => {
            tracing::info!("comptes : enrôlement impossible : {e}");
            refused(StatusCode::SERVICE_UNAVAILABLE, "relay_unreachable", "accounts.relayUnreachable")
        }
    }
}

/// Le mode d'essai est-il demandé (`KYBERNOW_ACCOUNT_TEST` à `1`, `true`, `oui`, `yes`) ?
pub fn test_mode_wanted(value: Option<&str>) -> bool {
    value.is_some_and(|v| matches!(v.trim().to_ascii_lowercase().as_str(), "1" | "true" | "oui" | "yes" | "on"))
}

/// La clé d'essai, lue ou créée dans le dossier de données.
pub fn test_signer(data_dir: &Path) -> Result<access::Signer> {
    let path = data_dir.join(TEST_KEY_FILE);
    let der = match std::fs::read(&path) {
        Ok(der) => der,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            let der = access::Signer::generate_pkcs8().map_err(|e| anyhow::anyhow!("clé d'essai : {e}"))?;
            crate::durable::write(&path, &der).with_context(|| format!("écriture de {}", path.display()))?;
            der
        }
        Err(e) => return Err(e).with_context(|| format!("lecture de {}", path.display())),
    };
    access::Signer::from_pkcs8(&der).map_err(|e| anyhow::anyhow!("clé d'essai illisible : {e}"))
}

/// Au démarrage, avec [`TEST_ENV`] : épingle la clé d'essai si la Station
/// n'est rattachée à aucun relais (un vrai rattachement n'est jamais remplacé).
pub fn start_test_mode(s: &AppState) {
    if !test_mode_wanted(std::env::var(TEST_ENV).ok().as_deref()) {
        return;
    }
    let current = s.settings.lock().unwrap().account.clone();
    if current.attached() && !current.test {
        tracing::warn!("comptes : mode d'essai demandé, mais la Station est rattachée à {} : ignoré", current.relay.unwrap_or_default());
        return;
    }
    let signer = match test_signer(&s.data_dir) {
        Ok(signer) => signer,
        Err(e) => return tracing::warn!("comptes : mode d'essai impossible : {e:#}"),
    };
    let keys = KeySet { keys: vec![signer.public_key().clone()] };
    if current.test && current.keys == keys.keys {
        return;
    }
    let owner = Owner { kind: OwnerKind::Person, id: "essai".into(), name: "Essai".into(), email: Some("essai@kybernow.invalid".into()) };
    match pin(s, TEST_RELAY, owner, &keys) {
        Ok(()) => tracing::warn!("comptes : MODE D'ESSAI, clé {} épinglée (relais {TEST_RELAY})", signer.public_key().kid),
        Err(e) => tracing::warn!("comptes : mode d'essai impossible : {e:#}"),
    }
}

/// `kyber-admin account-token` : un jeton signé par la clé d'essai, pour cette Station.
pub fn test_token(data_dir: &Path, rights: &str, account: &str, device: &str, name: &str, device_name: &str) -> Result<String> {
    let signer = test_signer(data_dir)?;
    let station = kybernow_tunnel::StationIdentity::load_or_create(&data_dir.join("relay").join("station.key"))
        .context("identité de la Station")?
        .id()
        .to_string();
    let mut parsed = Vec::new();
    for word in rights.split(',').map(str::trim).filter(|w| !w.is_empty()) {
        parsed.push(Right::parse(word).with_context(|| format!("droit inconnu : {word} (session, lend, share, admin)"))?);
    }
    let names = access::Names { account: name.into(), email: None, device: device_name.into() };
    let claims = Claims::issue(TEST_RELAY, &station, account, device, Rights::new(parsed), names, now_secs());
    Ok(signer.sign(&claims))
}

// --- Vérification ------------------------------------------------------------------

/// Un appel accepté : ce que dit le jeton, et son entrée dans la liste.
#[derive(Debug, Clone)]
pub struct Grant {
    pub claims: Claims,
    pub entry: String,
}

impl Grant {
    /// Le compte Kyber de sa session (jeton de Kyber).
    pub fn kyber_user(&self) -> String {
        kyber_user(&self.claims.account)
    }

    /// « Léa Martin · iPad de Léa », pour le fil.
    pub fn label(&self) -> String {
        let (who, device) = (self.claims.name.account.trim(), self.claims.name.device.trim());
        match (who.is_empty(), device.is_empty()) {
            (false, false) => format!("{who} · {device}"),
            (false, true) => who.to_string(),
            (true, false) => device.to_string(),
            (true, true) => self.claims.account.clone(),
        }
    }

    /// L'identifiant d'appareil de sa session (activité, écran virtuel, partage).
    pub fn device_id(&self) -> String {
        format!("kn-{}", self.entry)
    }
}

/// Pourquoi un jeton est refusé par la Station.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Denied {
    /// « Accepter les comptes KyberNow » est coupé.
    Off,
    /// La Station n'est rattachée à aucun relais.
    NotAttached,
    /// Le jeton ne tient pas (signature, échéance, Station, `jti`…).
    Token(AccessError),
    /// Le compte ou l'appareil est bloqué ici.
    Blocked,
    /// Le jeton n'a pas ce droit.
    Right(Right),
}

impl Denied {
    pub fn status(self) -> StatusCode {
        match self {
            Denied::Token(_) => StatusCode::UNAUTHORIZED,
            _ => StatusCode::FORBIDDEN,
        }
    }

    /// Le mot rendu aux apps (`code`).
    pub fn code(self) -> &'static str {
        match self {
            Denied::Off => "accounts_off",
            Denied::NotAttached => "not_attached",
            Denied::Token(AccessError::Expired) => "token_expired",
            Denied::Token(_) => "bad_token",
            Denied::Blocked => "blocked",
            Denied::Right(_) => "no_right",
        }
    }

    pub fn msg(self) -> Msg {
        match self {
            Denied::Off => Msg::new("accounts.off"),
            Denied::NotAttached => Msg::new("accounts.notAttached"),
            Denied::Token(AccessError::Expired) => Msg::new("accounts.expired"),
            Denied::Token(e) => Msg::new("accounts.badToken").with("reason", e.code()),
            Denied::Blocked => Msg::new("accounts.blocked"),
            Denied::Right(r) => Msg::new("accounts.noRight").with("right", r.as_str()),
        }
    }

    pub fn response(self) -> Response {
        (self.status(), Json(json!({ "error": self.msg().text(), "code": self.code() }))).into_response()
    }
}

/// Le jeton d'un en-tête `Authorization: KyberNow <jeton>`, s'il y en a un.
pub fn bearer(headers: &HeaderMap) -> Option<&str> {
    headers.get(header::AUTHORIZATION).and_then(|v| v.to_str().ok()).and_then(access::from_authorization)
}

/// Vérifie le jeton de la requête : `None` sans jeton KyberNow (l'appel
/// suit son chemin d'avant), sinon l'accord ou le refus. `need` : le droit
/// exigé (aucun : la garde du relais, chaque route vérifie ensuite le sien).
/// `relay_guard` : appelé par la garde du relais (l'appel est noté « hors de
/// chez vous »).
pub fn authorize(s: &AppState, headers: &HeaderMap, need: Option<Right>, use_: Use, relay_guard: bool) -> Option<Result<Grant, Denied>> {
    let token = bearer(headers)?;
    Some(check(s, token, need, use_, relay_guard))
}

fn check(s: &AppState, token: &str, need: Option<Right>, use_: Use, relay_guard: bool) -> Result<Grant, Denied> {
    if !s.settings.lock().unwrap().account.accept {
        return Err(Denied::Off);
    }
    let verifier = s.accounts.verifier(s).ok_or(Denied::NotAttached)?;
    let claims = verifier.verify(token, now_secs(), use_).map_err(|e| {
        tracing::debug!("comptes : jeton refusé ({e})");
        Denied::Token(e)
    })?;
    if s.accounts.is_blocked(&claims.account, &claims.device) {
        tracing::info!("comptes : {} refusé (bloqué ici)", claims.name.device);
        return Err(Denied::Blocked);
    }
    if let Some(right) = need
        && !claims.rights.has(right)
    {
        return Err(Denied::Right(right));
    }
    let id = entry_id(&claims.account, &claims.device);
    let via_relay = relay_guard || s.accounts.relay_marked(&id);
    let (entry, new) = s.accounts.note(&claims, via_relay);
    if relay_guard {
        s.accounts.mark_relay(&entry);
    }
    let grant = Grant { claims, entry };
    if new {
        // Garde-fou 4 : la première venue d'un appareil se voit dans le fil.
        tracing::info!("comptes : nouvel appareil, {}", grant.label());
        s.activity.event("info", Msg::new("events.accountNewDevice").with("name", grant.label()));
    }
    Ok(grant)
}

// --- Admin sur le réseau : « Se connecter avec KyberNow » (lot 4) -------------------

/// Le cookie du `nonce` d'une connexion avec KyberNow en cours (`SameSite=Lax` :
/// le retour depuis la console du relais est une navigation d'un autre site).
pub const WEB_LOGIN_COOKIE: &str = "kyber_admin_kn";
/// Une connexion avec KyberNow vaut 10 minutes.
const WEB_LOGIN_TTL: Duration = Duration::from_secs(10 * 60);
/// Connexions avec KyberNow en cours, au plus.
const MAX_WEB_LOGINS: usize = 64;

impl Accounts {
    /// Un nouveau `nonce` (16 octets en base32), gardé 10 minutes.
    fn new_web_login(&self) -> String {
        let nonce = kybernow_tunnel::id::base32_lower(&rand::random::<[u8; 16]>());
        let mut list = self.web_logins.lock().unwrap();
        let now = Instant::now();
        list.retain(|_, exp| *exp > now);
        if list.len() >= MAX_WEB_LOGINS
            && let Some(oldest) = list.iter().min_by_key(|(_, e)| **e).map(|(n, _)| n.clone())
        {
            list.remove(&oldest);
        }
        list.insert(nonce.clone(), now + WEB_LOGIN_TTL);
        nonce
    }

    /// Prend le `nonce` (une seule fois), s'il vaut encore.
    fn take_web_login(&self, nonce: &str) -> bool {
        self.web_logins.lock().unwrap().remove(nonce).is_some_and(|exp| exp > Instant::now())
    }
}

/// « Admin à distance » vaut-il (docs/comptes.md, « Admin depuis KyberNow Share ») : allumé, l'accès hors de
/// chez vous aussi, et la connexion avec KyberNow possible.
pub fn remote_admin_on(s: &AppState) -> bool {
    let relay = s.settings.lock().unwrap().relay.clone();
    relay.remote_admin && relay.remote_access && web_login_available(s)
}

/// Le cookie du `nonce` par le relais : `__Host-`, donc sur tout le site (`Path=/`).
const REMOTE_LOGIN_COOKIE: &str = "__Host-kyber_admin_kn";

#[derive(Deserialize)]
pub struct WebLoginBegin {
    #[serde(default)]
    lang: Option<String>,
}

/// `GET /api/login/kybernow/start` : le départ depuis la console du relais (« Ouvrir Admin »), sur le
/// réseau ou à distance. Tire le `nonce` et renvoie vers « Ouvrir Admin de {ordinateur} ? » chez le relais.
pub async fn web_login_begin(
    State(s): State<AppState>,
    via: Option<axum::Extension<crate::relay::Via>>,
    headers: HeaderMap,
    uri: axum::http::Uri,
    axum::extract::Query(q): axum::extract::Query<WebLoginBegin>,
) -> Response {
    let remote = via.is_some();
    if remote && !remote_admin_on(&s) {
        return web_login_page(&Msg::new("accounts.remoteAdminOff").text(), false, vec![]);
    }
    if !web_login_available(&s) {
        return web_login_page(&Msg::new("accounts.webLoginOff").text(), false, vec![]);
    }
    // L'adresse d'Admin telle que le navigateur la voit (le relais passe `Host` tel quel).
    let host = browser_host(&headers, &uri);
    if host.is_empty() || host.len() > 200 || host.contains(['/', '?', '#', '@', ' ']) {
        return web_login_page(&Msg::new("accounts.badOrigin").text(), false, vec![]);
    }
    let scheme = if remote || s.secure_cookies { "https" } else { "http" };
    let origin = format!("{scheme}://{host}");
    let relay = s.settings.lock().unwrap().account.relay.clone().unwrap_or_default();
    let (Some(console), Some(station)) = (console_url(&relay), s.relay.id()) else {
        return web_login_page(&Msg::new("accounts.webLoginOff").text(), false, vec![]);
    };
    let nonce = s.accounts.new_web_login();
    let lang = if q.lang.as_deref() == Some("en") { "en" } else { "fr" };
    let url = format!("{console}/ouvrir-admin?station={station}&nonce={nonce}&retour={}&lang={lang}", pct(&origin));
    let cookie = if remote {
        format!("{REMOTE_LOGIN_COOKIE}={nonce}; Path=/; Secure; HttpOnly; SameSite=Lax; Max-Age={}", WEB_LOGIN_TTL.as_secs())
    } else {
        format!("{WEB_LOGIN_COOKIE}={nonce}; Path=/api/login/kybernow; HttpOnly; SameSite=Lax; Max-Age={}{}", WEB_LOGIN_TTL.as_secs(), if s.secure_cookies { "; Secure" } else { "" })
    };
    tracing::info!("comptes : connexion avec KyberNow demandée depuis la console ({})", if remote { "à distance" } else { "sur le réseau" });
    let mut res = (StatusCode::SEE_OTHER, [(header::LOCATION, url.as_str()), (header::CACHE_CONTROL, "no-store")]).into_response();
    if let Ok(v) = header::HeaderValue::from_str(&cookie) {
        res.headers_mut().append(header::SET_COOKIE, v);
    }
    res
}

/// L'adresse d'Admin telle que le navigateur l'a appelée : l'en-tête `Host`
/// (HTTP/1.1), sinon l'autorité de la requête (HTTP/2 : `:authority`, sans
/// `Host`, ce que fait Safari sur iPhone en HTTPS).
fn browser_host(headers: &HeaderMap, uri: &axum::http::Uri) -> String {
    headers
        .get(header::HOST)
        .and_then(|h| h.to_str().ok())
        .map(str::to_string)
        .or_else(|| uri.authority().map(|a| a.as_str().to_string()))
        .unwrap_or_default()
        .trim()
        .to_string()
}

/// La connexion avec KyberNow est-elle possible (rattachée, comptes acceptés, pas en mode d'essai) ?
pub fn web_login_available(s: &AppState) -> bool {
    let a = s.settings.lock().unwrap().account.clone();
    a.attached() && a.accept && !a.test && a.relay.as_deref().and_then(console_url).is_some()
}

/// Encode une valeur pour une adresse.
fn pct(s: &str) -> String {
    s.bytes()
        .map(|b| if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~') { (b as char).to_string() } else { format!("%{b:02X}") })
        .collect()
}

#[derive(Deserialize)]
pub struct WebLoginBody {
    /// L'adresse d'Admin vue par le navigateur (`https://192.168.1.20:8443`).
    origin: String,
    #[serde(default)]
    lang: Option<String>,
}

/// `POST /api/login/kybernow` (sans session, sur le réseau seulement) : la page
/// de la console du relais à ouvrir, et le cookie du `nonce`.
pub async fn web_login_start(State(s): State<AppState>, via: Option<axum::Extension<crate::relay::Via>>, Json(body): Json<WebLoginBody>) -> Response {
    let refuse = |status: StatusCode, code: &str, msg: Msg| (status, Json(json!({ "error": msg.text(), "code": code }))).into_response();
    if via.is_some() {
        return refuse(StatusCode::FORBIDDEN, "network_only", Msg::new("accounts.networkOnly"));
    }
    if !web_login_available(&s) {
        return refuse(StatusCode::CONFLICT, "unavailable", Msg::new("accounts.webLoginOff"));
    }
    let origin = body.origin.trim().trim_end_matches('/').to_string();
    if !(origin.starts_with("https://") || origin.starts_with("http://")) || origin.len() > 200 || origin[8..].contains(['/', '?', '#', '@', ' ']) {
        return refuse(StatusCode::BAD_REQUEST, "bad_origin", Msg::new("accounts.badOrigin"));
    }
    let relay = s.settings.lock().unwrap().account.relay.clone().unwrap_or_default();
    let Some(console) = console_url(&relay) else {
        return refuse(StatusCode::CONFLICT, "unavailable", Msg::new("accounts.webLoginOff"));
    };
    let Some(station) = s.relay.id() else {
        return refuse(StatusCode::CONFLICT, "unavailable", Msg::new("accounts.webLoginOff"));
    };
    let nonce = s.accounts.new_web_login();
    let lang = if body.lang.as_deref() == Some("en") { "en" } else { "fr" };
    let url = format!("{console}/ouvrir-admin?station={station}&nonce={nonce}&retour={}&lang={lang}", pct(&origin));
    let cookie = format!(
        "{WEB_LOGIN_COOKIE}={nonce}; Path=/api/login/kybernow; HttpOnly; SameSite=Lax; Max-Age={}{}",
        WEB_LOGIN_TTL.as_secs(),
        if s.secure_cookies { "; Secure" } else { "" }
    );
    tracing::info!("comptes : connexion avec KyberNow demandée");
    ([(header::SET_COOKIE, cookie)], Json(json!({ "url": url, "expires": WEB_LOGIN_TTL.as_secs() }))).into_response()
}

/// Le `nonce` du cookie de ce navigateur (`name` : sur le réseau, ou par le relais).
fn web_login_cookie(headers: &HeaderMap, name: &str) -> Option<String> {
    headers
        .get_all(header::COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .flat_map(|v| v.split(';'))
        .filter_map(|c| c.trim().split_once('='))
        .find(|(k, _)| *k == name)
        .map(|(_, v)| v.to_string())
}

#[derive(Deserialize)]
pub struct WebLoginReturn {
    #[serde(default)]
    token: String,
}

/// Une petite page : vers l'accueil d'Admin (une navigation de ce site, pour
/// que le cookie `SameSite=Strict` de la session parte), ou le refus.
fn web_login_page(text: &str, ok: bool, cookies: Vec<String>) -> Response {
    let refresh = if ok { "<meta http-equiv=\"refresh\" content=\"0;url=/\">" } else { "" };
    let body = format!(
        "<!doctype html><html><head><meta charset=\"utf-8\"><meta name=\"robots\" content=\"noindex\">{refresh}<title>KyberNow Admin</title></head><body><p>{}</p><p><a href=\"/\">Admin</a></p></body></html>",
        text.replace('&', "&amp;").replace('<', "&lt;")
    );
    let status = if ok { StatusCode::OK } else { StatusCode::FORBIDDEN };
    let mut res = (status, [(header::CONTENT_TYPE, "text/html; charset=utf-8"), (header::CACHE_CONTROL, "no-store")], body).into_response();
    for c in cookies {
        if let Ok(v) = header::HeaderValue::from_str(&c) {
            res.headers_mut().append(header::SET_COOKIE, v);
        }
    }
    res
}

/// `GET /api/login/kybernow?token=…` : le retour depuis la console du relais.
/// Le jeton (droit Administrer, usage unique) doit porter en `jti` le `nonce`
/// de ce navigateur ; alors la session d'Admin s'ouvre.
pub async fn web_login_return(
    State(s): State<AppState>,
    via: Option<axum::Extension<crate::relay::Via>>,
    headers: HeaderMap,
    axum::extract::Query(q): axum::extract::Query<WebLoginReturn>,
) -> Response {
    let remote = via.is_some();
    let clear = if remote {
        format!("{REMOTE_LOGIN_COOKIE}=; Path=/; Secure; HttpOnly; SameSite=Lax; Max-Age=0")
    } else {
        format!("{WEB_LOGIN_COOKIE}=; Path=/api/login/kybernow; HttpOnly; SameSite=Lax; Max-Age=0{}", if s.secure_cookies { "; Secure" } else { "" })
    };
    // Par le relais : seulement avec « Admin à distance » (docs/comptes.md, « Admin depuis KyberNow Share »).
    if remote && !remote_admin_on(&s) {
        return web_login_page(&Msg::new("accounts.remoteAdminOff").text(), false, vec![clear]);
    }
    let cookie_name = if remote { REMOTE_LOGIN_COOKIE } else { WEB_LOGIN_COOKIE };
    let Some(nonce) = web_login_cookie(&headers, cookie_name).filter(|n| s.accounts.take_web_login(n)) else {
        return web_login_page(&Msg::new("accounts.webLoginExpired").text(), false, vec![clear]);
    };
    let grant = match check(&s, q.token.trim(), Some(Right::Admin), Use::Once, remote) {
        // À distance, le jeton dit que le relais a vu une passkey (`via: "relay"`).
        Ok(g) if g.claims.jti == nonce && (!remote || g.claims.via.as_deref() == Some("relay")) => g,
        Ok(_) => return web_login_page(&Msg::new("accounts.webLoginExpired").text(), false, vec![clear]),
        Err(d) => return web_login_page(&d.msg().text(), false, vec![clear]),
    };
    if remote {
        let name = if grant.claims.name.account.is_empty() { grant.label() } else { grant.claims.name.account.clone() };
        tracing::info!("comptes : Admin ouvert à distance avec KyberNow ({})", grant.label());
        s.activity.event("warn", Msg::new("events.accountAdminRemote").with("name", name.clone()));
        s.relay.admin_opened(&grant.claims.account, &name);
        let session = s.auth.open_remote_session(&name);
        return web_login_page("…", true, vec![crate::auth::remote_session_cookie(&session), clear]);
    }
    tracing::info!("comptes : Admin ouvert avec KyberNow ({})", grant.label());
    s.activity.event("info", Msg::new("events.accountAdminLogin").with("name", grant.label()));
    let session = s.auth.open_session();
    web_login_page("…", true, vec![crate::auth::session_cookie(&session, s.secure_cookies), clear])
}

// --- Routes : session d'Admin ------------------------------------------------------

/// L'adresse de la console du relais (« Gérer dans la console »).
fn console_url(relay: &str) -> Option<String> {
    let host = relay.split('#').next().unwrap_or_default().trim();
    (!host.is_empty() && relay != TEST_RELAY).then(|| format!("https://{host}/compte"))
}

pub fn view(s: &AppState) -> Value {
    let a = s.settings.lock().unwrap().account.clone();
    let attached = a.attached();
    let keys: Vec<Value> = a.keys.iter().map(|k| json!({ "kid": k.kid, "fingerprint": k.fingerprint() })).collect();
    json!({
        "attached": attached,
        "relay": a.relay,
        "owner": a.owner,
        "keys": keys,
        "pinned_at": a.pinned_at,
        "accept": a.accept,
        "test": a.test,
        "station": s.relay.id(),
        "console": a.relay.as_deref().and_then(console_url),
        // « Se connecter » demande le code au relais réglé dans « Hors de chez vous ».
        "can_attach": s.settings.lock().unwrap().relay.addr().is_some(),
        // « Se connecter » attend la confirmation dans la console.
        "attaching": !attached && s.relay.attaching(),
    })
}

pub async fn get(State(s): State<AppState>) -> Json<Value> {
    Json(view(&s))
}

#[derive(Deserialize, Default)]
pub struct AccountBody {
    /// « Accepter les comptes KyberNow ».
    #[serde(default)]
    accept: Option<bool>,
    /// Détacher la Station.
    #[serde(default)]
    detach: bool,
    /// Abandonner « Se connecter » (le code du relais échoit de lui-même).
    #[serde(default)]
    cancel_attach: bool,
}

pub async fn put(State(s): State<AppState>, Json(body): Json<AccountBody>) -> Response {
    if body.cancel_attach {
        s.relay.attach_done();
    }
    if body.detach {
        let was = s.settings.lock().unwrap().account.clone();
        if let Err(e) = detach(&s) {
            return crate::api::ApiError::from(e).into_response();
        }
        // Détachée des deux côtés : le relais l'apprend (le mode d'essai n'a pas de relais).
        if was.attached() && !was.test {
            s.relay.detach_at_relay();
        } else {
            s.relay.attach_done();
        }
        return Json(view(&s)).into_response();
    }
    if let Some(on) = body.accept {
        let attached = s.settings.lock().unwrap().account.attached();
        if on && !attached {
            return (StatusCode::CONFLICT, Json(json!({ "error": Msg::new("accounts.notAttached").text(), "code": "not_attached" }))).into_response();
        }
        let before = s.settings.lock().unwrap().account.accept;
        if let Err(e) = s.update_settings(|st| st.account.accept = on) {
            return crate::api::ApiError::from(e).into_response();
        }
        if before != on {
            tracing::info!("comptes : comptes KyberNow {}", if on { "acceptés" } else { "refusés" });
            s.activity.event("info", Msg::new(if on { "events.accountsOn" } else { "events.accountsOff" }));
        }
    }
    Json(view(&s)).into_response()
}

fn entry_json(s: &AppState, e: &Entry, owner: Option<&str>) -> Value {
    let account_blocked = s.accounts.account_blocked(&e.account);
    json!({
        "id": e.id,
        "account": e.account,
        "device": e.device,
        "name": e.name,
        "email": e.email,
        "device_name": e.device_name,
        "rights": e.rights,
        "first_seen": e.first_seen,
        "last_seen": e.last_seen,
        "via": e.via,
        "blocked": e.blocked.or(account_blocked),
        "account_blocked": account_blocked.is_some(),
        "owner": owner == Some(e.account.as_str()),
    })
}

pub fn access_view(s: &AppState) -> Value {
    let owner = s.settings.lock().unwrap().account.owner.as_ref().map(|o| o.id.clone());
    let mut entries = s.accounts.entries();
    // Les dernières venues d'abord, les bloqués à la fin.
    entries.sort_by_key(|e| (e.blocked.is_some() || s.accounts.account_blocked(&e.account).is_some(), std::cmp::Reverse(e.last_seen)));
    let list: Vec<Value> = entries.iter().map(|e| entry_json(s, e, owner.as_deref())).collect();
    json!({ "entries": list })
}

pub async fn get_access(State(s): State<AppState>) -> Json<Value> {
    Json(access_view(&s))
}

#[derive(Deserialize)]
pub struct BlockBody {
    #[serde(default = "yes")]
    blocked: bool,
    /// Tout le compte (tous ses appareils), pas seulement cet appareil.
    #[serde(default)]
    account: bool,
}

fn yes() -> bool {
    true
}

pub async fn block(State(s): State<AppState>, UrlPath(id): UrlPath<String>, Json(body): Json<BlockBody>) -> Response {
    let Some(entry) = s.accounts.set_blocked(&id, body.blocked, body.account) else {
        return (StatusCode::NOT_FOUND, Json(json!({ "error": Msg::new("accounts.unknownEntry").text() }))).into_response();
    };
    let label = if entry.device_name.is_empty() { entry.name.clone() } else { format!("{} · {}", entry.name, entry.device_name) };
    let key = match (body.blocked, body.account) {
        (true, false) => "events.accountBlocked",
        (true, true) => "events.accountBlockedAll",
        (false, _) => "events.accountUnblocked",
    };
    tracing::info!("comptes : {label} {}", if body.blocked { "bloqué" } else { "débloqué" });
    s.activity.event("info", Msg::new(key).with("name", if body.account { entry.name.clone() } else { label }));
    // Un appareil bloqué en session par le relais ne joint plus Kyber.
    if body.blocked {
        s.relay.revoke_device(&format!("kn-{}", entry.id));
    }
    Json(access_view(&s)).into_response()
}

/// Pour les essais des autres modules : rattache la Station à une clé neuve.
#[cfg(test)]
pub fn attach_for_tests(s: &AppState) -> access::Signer {
    let signer = access::Signer::from_pkcs8(&access::Signer::generate_pkcs8().unwrap()).unwrap();
    let owner = Owner { kind: OwnerKind::Person, id: "acc_owner".into(), name: "Matthieu".into(), email: None };
    pin(s, "share.kybernow.app", owner, &KeySet { keys: vec![signer.public_key().clone()] }).unwrap();
    signer
}

/// Un en-tête `Authorization: KyberNow <jeton>` pour cette Station.
#[cfg(test)]
pub fn token_headers(s: &AppState, signer: &access::Signer, account: &str, device: &str, rights: &[Right]) -> HeaderMap {
    let names = access::Names { account: "Léa Martin".into(), email: None, device: "iPad de Léa".into() };
    let claims = Claims::issue("share.kybernow.app", &s.relay.id().unwrap(), account, device, rights.iter().copied().collect(), names, now_secs());
    let mut h = HeaderMap::new();
    h.insert(header::AUTHORIZATION, format!("KyberNow {}", signer.sign(&claims)).parse().unwrap());
    h
}

#[cfg(test)]
mod tests {
    use super::*;
    use access::{Names, Signer};

    fn state() -> (AppState, PathBuf) {
        let dir = std::env::temp_dir().join(format!("kyber-admin-accounts-{}", rand::random::<u64>()));
        std::fs::create_dir_all(&dir).unwrap();
        (AppState::for_tests(dir.clone(), crate::service::platform()), dir)
    }

    fn signer() -> Signer {
        Signer::from_pkcs8(&Signer::generate_pkcs8().unwrap()).unwrap()
    }

    #[test]
    fn adresse_vue_par_le_navigateur_en_http1_et_http2() {
        let mut h = HeaderMap::new();
        h.insert(header::HOST, "192.168.1.100:35555".parse().unwrap());
        assert_eq!(browser_host(&h, &"/api/login/kybernow/start".parse().unwrap()), "192.168.1.100:35555");
        // HTTP/2 (Safari sur iPhone) : pas de `Host`, l'autorité de la requête.
        let uri: axum::http::Uri = "https://192.168.1.100:35555/api/login/kybernow/start?lang=fr".parse().unwrap();
        assert_eq!(browser_host(&HeaderMap::new(), &uri), "192.168.1.100:35555");
        assert_eq!(browser_host(&HeaderMap::new(), &"/api/login/kybernow/start".parse().unwrap()), "");
    }

    fn owner() -> Owner {
        Owner { kind: OwnerKind::Person, id: "acc_owner".into(), name: "Matthieu".into(), email: Some("m@exemple.fr".into()) }
    }

    fn attach(s: &AppState, signer: &Signer) {
        pin(s, "share.kybernow.app", owner(), &KeySet { keys: vec![signer.public_key().clone()] }).unwrap();
    }

    fn token(s: &AppState, signer: &Signer, account: &str, device: &str, rights: &[Right]) -> String {
        let names = Names { account: "Léa Martin".into(), email: None, device: "iPad de Léa".into() };
        let claims = Claims::issue("share.kybernow.app", &s.relay.id().unwrap(), account, device, rights.iter().copied().collect(), names, now_secs());
        signer.sign(&claims)
    }

    fn headers(token: &str) -> HeaderMap {
        let mut h = HeaderMap::new();
        h.insert(header::AUTHORIZATION, format!("KyberNow {token}").parse().unwrap());
        h
    }

    #[test]
    fn sans_jeton_rien_ne_change() {
        let (s, dir) = state();
        assert!(authorize(&s, &HeaderMap::new(), Some(Right::Session), Use::Reusable, false).is_none());
        let mut basic = HeaderMap::new();
        basic.insert(header::AUTHORIZATION, "Basic YTpi".parse().unwrap());
        assert!(authorize(&s, &basic, Some(Right::Session), Use::Reusable, false).is_none());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn pas_rattachee_puis_interrupteur() {
        let (s, dir) = state();
        let k = signer();
        // Pas rattachée : l'interrupteur ne s'allume pas.
        assert!(!view(&s)["attached"].as_bool().unwrap());
        let t = token(&s, &k, "acc_lea", "dev_1", &[Right::Session]);
        assert_eq!(authorize(&s, &headers(&t), Some(Right::Session), Use::Reusable, false).unwrap().unwrap_err(), Denied::Off);
        attach(&s, &k);
        let v = view(&s);
        assert_eq!((v["attached"].as_bool(), v["accept"].as_bool(), v["relay"].as_str()), (Some(true), Some(true), Some("share.kybernow.app")));
        assert_eq!(v["console"], "https://share.kybernow.app/compte");
        assert!(v["keys"][0]["fingerprint"].as_str().unwrap().starts_with("sha256:"));
        let grant = authorize(&s, &headers(&t), Some(Right::Session), Use::Reusable, false).unwrap().unwrap();
        assert_eq!(grant.label(), "Léa Martin · iPad de Léa");
        assert!(grant.kyber_user().starts_with("kn-account-"));
        // Coupé : plus rien.
        s.update_settings(|st| st.account.accept = false).unwrap();
        assert_eq!(authorize(&s, &headers(&t), Some(Right::Session), Use::Reusable, false).unwrap().unwrap_err(), Denied::Off);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn mauvaise_cle_droits_et_blocage() {
        let (s, dir) = state();
        let (k, other) = (signer(), signer());
        attach(&s, &k);
        // Signé par une autre clé que celle épinglée.
        let forged = token(&s, &other, "acc_lea", "dev_1", &[Right::Session]);
        assert_eq!(authorize(&s, &headers(&forged), None, Use::Reusable, false).unwrap().unwrap_err(), Denied::Token(AccessError::Signature));
        // Le droit demandé manque.
        let t = token(&s, &k, "acc_lea", "dev_1", &[Right::Share]);
        assert_eq!(authorize(&s, &headers(&t), Some(Right::Session), Use::Reusable, false).unwrap().unwrap_err(), Denied::Right(Right::Session));
        let g = authorize(&s, &headers(&t), Some(Right::Share), Use::Reusable, true).unwrap().unwrap();
        // Dans la liste, venu par le relais.
        let list = access_view(&s);
        assert_eq!(list["entries"].as_array().unwrap().len(), 1);
        assert_eq!(list["entries"][0]["via"], "relay");
        assert_eq!(list["entries"][0]["rights"], json!(["share"]));
        // Bloqué ici : refusé même avec un jeton neuf.
        s.accounts.set_blocked(&g.entry, true, false).unwrap();
        let fresh = token(&s, &k, "acc_lea", "dev_1", &[Right::Share]);
        assert_eq!(authorize(&s, &headers(&fresh), Some(Right::Share), Use::Reusable, false).unwrap().unwrap_err(), Denied::Blocked);
        // Un autre appareil du même compte passe…
        let t2 = token(&s, &k, "acc_lea", "dev_2", &[Right::Share]);
        let g2 = authorize(&s, &headers(&t2), Some(Right::Share), Use::Reusable, false).unwrap().unwrap();
        // …jusqu'à ce que le compte entier soit bloqué.
        s.accounts.set_blocked(&g2.entry, true, true).unwrap();
        let t3 = token(&s, &k, "acc_lea", "dev_3", &[Right::Share]);
        assert_eq!(authorize(&s, &headers(&t3), Some(Right::Share), Use::Reusable, false).unwrap().unwrap_err(), Denied::Blocked);
        let v = access_view(&s);
        assert!(v["entries"].as_array().unwrap().iter().all(|e| e["account_blocked"] == true));
        // Débloquer le compte débloque ses appareils.
        s.accounts.set_blocked(&g.entry, false, true).unwrap();
        assert!(!s.accounts.is_blocked("acc_lea", "dev_1"));
        assert!(!s.accounts.is_blocked("acc_lea", "dev_3"));
        // Relue depuis le disque.
        let again = Accounts::load(&dir);
        assert_eq!(again.entries().len(), 2);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn nouvel_appareil_dans_le_fil_et_detacher() {
        let (s, dir) = state();
        let k = signer();
        attach(&s, &k);
        let t = token(&s, &k, "acc_lea", "dev_1", &[Right::Session]);
        authorize(&s, &headers(&t), Some(Right::Session), Use::Reusable, false).unwrap().unwrap();
        authorize(&s, &headers(&t), Some(Right::Session), Use::Reusable, false).unwrap().unwrap();
        let events = s.activity.events(10, crate::i18n::Lang::Fr);
        let news = events.iter().filter(|e| serde_json::to_string(e).unwrap().contains("première fois")).count();
        assert_eq!(news, 1);
        detach(&s).unwrap();
        assert!(!view(&s)["attached"].as_bool().unwrap());
        assert!(s.accounts.entries().is_empty());
        assert_eq!(authorize(&s, &headers(&t), Some(Right::Session), Use::Reusable, false).unwrap().unwrap_err(), Denied::Off);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn jeton_d_essai() {
        let (s, dir) = state();
        assert!(test_mode_wanted(Some("1")) && test_mode_wanted(Some("oui")) && !test_mode_wanted(Some("0")) && !test_mode_wanted(None));
        let signer = test_signer(&dir).unwrap();
        // La même clé relue.
        assert_eq!(test_signer(&dir).unwrap().public_key(), signer.public_key());
        pin(&s, TEST_RELAY, Owner { id: "essai".into(), ..Owner::default() }, &KeySet { keys: vec![signer.public_key().clone()] }).unwrap();
        assert!(view(&s)["test"].as_bool().unwrap());
        assert!(view(&s)["console"].is_null());
        let t = test_token(&dir, "session, admin", "acc_x", "dev_x", "Essai", "Mon téléphone").unwrap();
        let g = authorize(&s, &headers(&t), Some(Right::Admin), Use::Once, false).unwrap().unwrap();
        assert!(g.claims.rights.has(Right::Session));
        // Consommé une fois pour redémarrer : il ne sert plus.
        assert!(authorize(&s, &headers(&t), Some(Right::Session), Use::Reusable, false).unwrap().is_err());
        assert!(test_token(&dir, "tout", "a", "d", "n", "d").is_err());
        let _ = std::fs::remove_dir_all(dir);
    }

    fn attached_msg(signer: &Signer, owner_id: &str) -> Attached {
        Attached {
            req: None,
            relay: "share.kybernow.app".into(),
            account: owner_id.into(),
            owner: kybernow_tunnel::msg::AttachOwner { kind: "person".into(), id: owner_id.into(), name: "Léa".into(), email: Some("lea@exemple.fr".into()) },
            keys: vec![signer.public_key().clone()],
        }
    }

    #[test]
    fn rattachement_par_le_relais() {
        let (s, dir) = state();
        let k = signer();
        let v = view(&s);
        assert_eq!((v["can_attach"].as_bool(), v["attaching"].as_bool()), (Some(true), Some(false)), "relais public réglé par défaut");
        // Un `attached` que personne n'a demandé : jamais pris.
        on_attached(&s, attached_msg(&k, "acc_lea"));
        assert!(!view(&s)["attached"].as_bool().unwrap());
        // « Se connecter », puis la confirmation dans la console.
        s.relay.begin_attach();
        assert!(view(&s)["attaching"].as_bool().unwrap());
        on_attached(&s, attached_msg(&k, "acc_lea"));
        let v = view(&s);
        assert_eq!((v["attached"].as_bool(), v["accept"].as_bool(), v["attaching"].as_bool()), (Some(true), Some(true), Some(false)));
        assert_eq!(v["owner"]["email"], "lea@exemple.fr");
        assert!(s.settings.lock().unwrap().relay.remote_access, "accès hors de chez vous allumé");
        assert!(!s.relay.attaching());
        // Les jetons signés par la clé reçue passent.
        let t = token(&s, &k, "acc_lea", "dev_1", &[Right::Session]);
        assert!(authorize(&s, &headers(&t), Some(Right::Session), Use::Reusable, false).unwrap().is_ok());
        // À la reconnexion : nouvelles clés du même rattachement (rotation), sans rien rallumer.
        s.update_settings(|st| st.account.accept = false).unwrap();
        let k2 = signer();
        let mut rotated = attached_msg(&k, "acc_lea");
        rotated.keys.push(k2.public_key().clone());
        on_attached(&s, rotated);
        let v = view(&s);
        assert_eq!(v["keys"].as_array().unwrap().len(), 2);
        assert_eq!(v["accept"].as_bool(), Some(false), "l'interrupteur coupé le reste");
        // Un autre propriétaire sans « Se connecter » : ignoré.
        on_attached(&s, attached_msg(&k2, "acc_bob"));
        assert_eq!(view(&s)["owner"]["id"], "acc_lea");
        // Déplacé dans la console : même relais, mêmes clés épinglées, autre propriétaire : pris.
        let mut moved = attached_msg(&k, "org_atelier");
        moved.keys.push(k2.public_key().clone());
        moved.owner.kind = "organization".into();
        on_attached(&s, moved);
        let v = view(&s);
        assert_eq!(v["owner"]["id"], "org_atelier");
        assert_eq!(v["keys"].as_array().unwrap().len(), 2, "les clés ne changent pas");
        // Détachée dans la console : `detached`.
        on_detached(&s);
        assert!(!view(&s)["attached"].as_bool().unwrap());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[tokio::test]
    async fn detacher_le_dit_au_relais_et_se_connecter_sans_relais() {
        let (s, dir) = state();
        let k = signer();
        s.relay.begin_attach();
        on_attached(&s, attached_msg(&k, "acc_lea"));
        // Relais pas joint (essais) : le détachement attend la prochaine connexion.
        let res = put(State(s.clone()), Json(AccountBody { accept: None, detach: true, cancel_attach: false })).await;
        assert_eq!(res.status(), StatusCode::OK);
        assert!(s.relay.detach_pending());
        assert!(!view(&s)["attached"].as_bool().unwrap());
        // Le mode d'essai n'a pas de relais : rien à lui dire.
        let (t, dir2) = state();
        pin(&t, TEST_RELAY, Owner { id: "essai".into(), ..Owner::default() }, &KeySet { keys: vec![k.public_key().clone()] }).unwrap();
        put(State(t.clone()), Json(AccountBody { accept: None, detach: true, cancel_attach: false })).await;
        assert!(!t.relay.detach_pending());
        // Pas de relais réglé : « Se connecter » le dit.
        t.update_settings(|st| st.relay.server = Some(String::new())).unwrap();
        assert!(!view(&t)["can_attach"].as_bool().unwrap());
        let res = super::attach(State(t.clone())).await;
        assert_eq!(res.status(), StatusCode::CONFLICT);
        let body = axum::body::to_bytes(res.into_body(), 1 << 16).await.unwrap();
        assert_eq!(serde_json::from_slice::<Value>(&body).unwrap()["code"], "no_relay");
        let _ = std::fs::remove_dir_all(dir);
        let _ = std::fs::remove_dir_all(dir2);
    }

    /// Lot 4 : « Se connecter avec KyberNow », sur le réseau seulement.
    #[tokio::test]
    async fn se_connecter_avec_kybernow_sur_le_reseau() {
        let (s, dir) = state();
        let k = signer();
        let body = || Json(WebLoginBody { origin: "https://192.168.1.20:8443/".into(), lang: None });
        // Pas rattachée : pas de connexion avec KyberNow.
        assert!(!web_login_available(&s));
        assert_eq!(web_login_start(State(s.clone()), None, body()).await.status(), StatusCode::CONFLICT);
        attach(&s, &k);
        assert!(web_login_available(&s));
        // Par le relais : jamais.
        let via = Some(axum::Extension(crate::relay::Via { kind: kybernow_tunnel::StreamKind::AdminHttp, client: "c1".into() }));
        assert_eq!(web_login_start(State(s.clone()), via, body()).await.status(), StatusCode::FORBIDDEN);
        let res = web_login_start(State(s.clone()), None, body()).await;
        assert_eq!(res.status(), StatusCode::OK);
        let cookie = res.headers().get(header::SET_COOKIE).unwrap().to_str().unwrap().to_string();
        assert!(cookie.contains("SameSite=Lax") && cookie.contains("HttpOnly"));
        let nonce = cookie.split(';').next().unwrap().split_once('=').unwrap().1.to_string();
        let json: Value = serde_json::from_slice(&axum::body::to_bytes(res.into_body(), 1 << 16).await.unwrap()).unwrap();
        let url = json["url"].as_str().unwrap();
        assert!(url.starts_with("https://share.kybernow.app/compte/ouvrir-admin?station="), "{url}");
        assert!(url.contains(&format!("nonce={nonce}")) && url.contains("retour=https%3A%2F%2F192.168.1.20%3A8443"));
        // Le jeton du relais : droit Administrer, `jti` = le `nonce` de ce navigateur.
        let signed = |jti: &str, rights: &[Right]| {
            let names = Names { account: "Léa Martin".into(), email: None, device: "Navigateur".into() };
            let mut claims = Claims::issue("share.kybernow.app", &s.relay.id().unwrap(), "acc_lea", "web", rights.iter().copied().collect(), names, now_secs());
            claims.jti = jti.to_string();
            k.sign(&claims)
        };
        let with_cookie = |n: &str| {
            let mut h = HeaderMap::new();
            h.insert(header::COOKIE, format!("{WEB_LOGIN_COOKIE}={n}").parse().unwrap());
            h
        };
        let back = |token: String, h: HeaderMap| web_login_return(State(s.clone()), None, h, axum::extract::Query(WebLoginReturn { token }));
        // Sans le droit Administrer : refusé (et le `nonce` a servi).
        assert_eq!(back(signed(&nonce, &[Right::Session]), with_cookie(&nonce)).await.status(), StatusCode::FORBIDDEN);
        // Un autre `nonce` que celui du navigateur : refusé.
        let res = web_login_start(State(s.clone()), None, body()).await;
        let nonce = res.headers().get(header::SET_COOKIE).unwrap().to_str().unwrap().split(';').next().unwrap().split_once('=').unwrap().1.to_string();
        assert_eq!(back(signed("autre-nonce", &[Right::Admin]), with_cookie(&nonce)).await.status(), StatusCode::FORBIDDEN);
        // Le bon : la session d'Admin s'ouvre, une seule fois.
        let res = web_login_start(State(s.clone()), None, body()).await;
        let nonce = res.headers().get(header::SET_COOKIE).unwrap().to_str().unwrap().split(';').next().unwrap().split_once('=').unwrap().1.to_string();
        let admin_token = signed(&nonce, &[Right::Session, Right::Admin]);
        let res = back(admin_token.clone(), with_cookie(&nonce)).await;
        assert_eq!(res.status(), StatusCode::OK);
        let session = res.headers().get_all(header::SET_COOKIE).iter().map(|v| v.to_str().unwrap().to_string()).find(|c| c.starts_with(crate::auth::COOKIE)).unwrap();
        let session = session.split(';').next().unwrap().split_once('=').unwrap().1.to_string();
        assert!(s.auth.session_valid(&session));
        assert_eq!(back(admin_token, with_cookie(&nonce)).await.status(), StatusCode::FORBIDDEN, "une seule fois");
        // Le prêt (lot 4) : le droit Prêt du jeton remplace le compte d'ami.
        let lend = token(&s, &k, "acc_lea", "dev_1", &[Right::Lend]);
        let b = crate::lend_api::account_borrower_for_tests(&s, &headers(&lend)).unwrap().unwrap();
        assert_eq!(b, "Léa Martin");
        let session_only = token(&s, &k, "acc_lea", "dev_1", &[Right::Session]);
        assert_eq!(crate::lend_api::account_borrower_for_tests(&s, &headers(&session_only)).unwrap().unwrap_err(), StatusCode::FORBIDDEN);
        assert!(crate::lend_api::account_borrower_for_tests(&s, &HeaderMap::new()).is_none(), "sans jeton : le compte d'ami");
        // Interrupteur coupé : plus de connexion avec KyberNow.
        s.update_settings(|st| st.account.accept = false).unwrap();
        assert!(!web_login_available(&s));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn epinglage_refuse_sans_cle() {
        let (s, dir) = state();
        assert!(pin(&s, "share.kybernow.app", owner(), &KeySet::default()).is_err());
        let k = signer();
        let keys = KeySet { keys: vec![k.public_key().clone()] };
        assert!(pin(&s, "share.kybernow.app", Owner::default(), &keys).is_err());
        assert!(pin(&s, "", owner(), &keys).is_err());
        assert!(!s.settings.lock().unwrap().account.attached());
        let _ = std::fs::remove_dir_all(dir);
    }

    // --- Routes locales de l'installateur ---

    /// L'ordinateur lui-même.
    fn here() -> ConnectInfo<SocketAddr> {
        ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 50000)))
    }

    /// L'en-tête du jeton local, tiré s'il ne l'est pas encore.
    fn local(s: &AppState) -> HeaderMap {
        let file = s.data_dir.join(crate::local_token::FILE);
        if !file.exists() {
            s.local_token.renew();
        }
        let mut h = HeaderMap::new();
        h.insert(crate::local_token::HEADER, std::fs::read_to_string(file).unwrap().trim().parse().unwrap());
        let installer = s.data_dir.join(crate::local_token::INSTALLER_FILE);
        h.insert(crate::local_token::INSTALLER_HEADER, std::fs::read_to_string(installer).unwrap().trim().parse().unwrap());
        h
    }

    /// Un compte Windows standard a le jeton local de sa session, pas celui
    /// de l'installateur : il ne rattache pas la Station ni ne change son relais.
    #[tokio::test]
    async fn le_jeton_de_la_session_ne_suffit_pas_pour_rattacher() {
        let (s, _dir) = state();
        let mut session = local(&s);
        session.remove(crate::local_token::INSTALLER_HEADER);
        let (status, v) = body(local_relay(State(s.clone()), here(), session.clone(), Json(LocalRelayBody { relay: Some("relais.pirate.example".into()) })).await).await;
        assert_eq!((status, v["code"].as_str()), (StatusCode::FORBIDDEN, Some("installer_only")));
        let (status, v) = body(local_attach(State(s.clone()), here(), session.clone(), Json(LocalRelayBody::default())).await).await;
        assert_eq!((status, v["code"].as_str()), (StatusCode::FORBIDDEN, Some("installer_only")));
        let (status, v) = body(local_enroll(State(s.clone()), here(), session.clone(), Json(EnrollBody { token: "kne_7Qm2xR9pLa4V".into(), relay: None })).await).await;
        assert_eq!((status, v["code"].as_str()), (StatusCode::FORBIDDEN, Some("installer_only")));
        // L'état reste lisible avec le jeton local seul.
        let (status, _) = body(local_get(State(s.clone()), here(), session).await).await;
        assert_eq!(status, StatusCode::OK);
        assert_ne!(s.settings.lock().unwrap().relay.server(), "relais.pirate.example");
    }

    #[tokio::test]
    async fn les_routes_de_l_installateur_demandent_l_ordinateur_et_le_jeton_local() {
        let (s, _dir) = state();
        let far = ConnectInfo(SocketAddr::from(([192, 168, 1, 20], 50000)));
        let (status, _) = body(local_get(State(s.clone()), far, local(&s)).await).await;
        assert_eq!(status, StatusCode::FORBIDDEN, "depuis le réseau");
        let (status, _) = body(local_enroll(State(s.clone()), here(), HeaderMap::new(), Json(EnrollBody { token: "kne_7Qm2xR9pLa4V".into(), relay: None })).await).await;
        assert_eq!(status, StatusCode::FORBIDDEN, "sans jeton local");
        let (status, _) = body(local_attach(State(s.clone()), far, local(&s), Json(LocalRelayBody::default())).await).await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        let (status, v) = body(local_get(State(s.clone()), here(), local(&s)).await).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(v["attached"], false);
    }

    async fn body(r: Response) -> (StatusCode, Value) {
        let status = r.status();
        let bytes = axum::body::to_bytes(r.into_body(), 1 << 20).await.unwrap();
        (status, serde_json::from_slice(&bytes).unwrap_or(Value::Null))
    }

    #[test]
    fn forme_du_jeton_d_enrolement() {
        assert!(enroll_token_ok("kne_7Qm2xR9pLa4V"));
        assert!(!enroll_token_ok("court"));
        assert!(!enroll_token_ok("kne_7Qm2 xR9pLa4V"));
        assert!(!enroll_token_ok("\"kne_7Qm2xR9pLa4V\""));
        assert!(!enroll_token_ok(&"a".repeat(300)));
    }

    #[tokio::test]
    async fn l_installateur_regle_le_relais_tant_que_la_station_n_est_pas_rattachee() {
        let (s, _dir) = state();
        let (status, v) = body(local_relay(State(s.clone()), here(), local(&s), Json(LocalRelayBody { relay: Some("https://relais.exemple.org:9443/".into()) })).await).await;
        assert_eq!(status, StatusCode::OK, "{v}");
        assert_eq!(v["server"], "relais.exemple.org:9443");
        assert_eq!(v["attached"], false);
        let (status, v) = body(local_relay(State(s.clone()), here(), local(&s), Json(LocalRelayBody { relay: Some("pas une adresse".into()) })).await).await;
        assert_eq!((status, v["code"].as_str()), (StatusCode::BAD_REQUEST, Some("bad_relay")));
        // Rattachée : ni autre relais, ni autre rattachement, ni enrôlement par l'installateur.
        attach(&s, &signer());
        let (status, v) = body(local_relay(State(s.clone()), here(), local(&s), Json(LocalRelayBody { relay: Some("share.kybernow.app".into()) })).await).await;
        assert_eq!((status, v["code"].as_str()), (StatusCode::CONFLICT, Some("already_attached")));
        let (status, v) = body(local_attach(State(s.clone()), here(), local(&s), Json(LocalRelayBody::default())).await).await;
        assert_eq!((status, v["code"].as_str()), (StatusCode::CONFLICT, Some("already_attached")));
        let (status, v) = body(local_enroll(State(s.clone()), here(), local(&s), Json(EnrollBody { token: "kne_7Qm2xR9pLa4V".into(), relay: None })).await).await;
        assert_eq!((status, v["code"].as_str()), (StatusCode::CONFLICT, Some("already_attached")));
        assert_eq!(s.settings.lock().unwrap().relay.server(), "relais.exemple.org:9443");
    }

    #[tokio::test]
    async fn sans_relais_ni_jeton_lisible_rien_ne_part() {
        let (s, _dir) = state();
        let (status, v) = body(local_enroll(State(s.clone()), here(), local(&s), Json(EnrollBody { token: "kne 7Q".into(), relay: None })).await).await;
        assert_eq!((status, v["code"].as_str()), (StatusCode::BAD_REQUEST, Some("bad_token")));
        let (status, v) = body(local_enroll(State(s.clone()), here(), local(&s), Json(EnrollBody { token: "kne_7Qm2xR9pLa4V".into(), relay: Some(" ".into()) })).await).await;
        assert_eq!((status, v["code"].as_str()), (StatusCode::CONFLICT, Some("no_relay")));
        assert!(!s.relay.attaching());
    }

    /// L'enrôlement de bout en bout : la route locale, l'agent du relais, le
    /// relais simulé qui répond `attached` (ou refuse le jeton), l'épinglage.
    #[tokio::test(flavor = "multi_thread")]
    async fn enrolement_avec_un_relais_simule() {
        use kybernow_tunnel::msg::{AttachOwner, Msg};
        use kybernow_tunnel::testing::FakeRelay;

        let key = signer();
        let public = key.public_key().clone();
        let name = std::sync::Arc::new(Mutex::new(String::new()));
        let relay = {
            let name = name.clone();
            let authority = std::sync::Arc::new(Mutex::new(String::new()));
            let a2 = authority.clone();
            let relay = FakeRelay::start(move |m| match m {
                Msg::Enroll { req, token, name: computer } if token == "kne_bon_jeton" => {
                    *name.lock().unwrap() = computer.clone();
                    Some(Msg::Attached(Attached {
                        req: Some(*req),
                        relay: a2.lock().unwrap().clone(),
                        account: "acc_lea".into(),
                        owner: AttachOwner { kind: "organization".into(), id: "org_1".into(), name: "Les Lignes Claires".into(), email: None },
                        keys: vec![public.clone()],
                    }))
                }
                Msg::Enroll { req, .. } => Some(Msg::error(Some(*req), ErrorCode::EnrollRefused, Some("expired".into()))),
                _ => None,
            })
            .await;
            *authority.lock().unwrap() = relay.addr.web_authority();
            relay
        };

        let (s, _dir) = state();
        let s = s.with_tls_fingerprint(&format!("sha256:{}", ["ab"; 32].join(":")));
        crate::relay::supervise(s.clone(), axum::Router::new());
        let addr = relay.addr.to_string();

        let refused = local_enroll(State(s.clone()), here(), local(&s), Json(EnrollBody { token: "kne_mauvais_jeton".into(), relay: Some(addr.clone()) })).await;
        let status = refused.status();
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert!(!s.settings.lock().unwrap().account.attached());
        assert!(!s.relay.attaching());

        let ok = local_enroll(State(s.clone()), here(), local(&s), Json(EnrollBody { token: "kne_bon_jeton".into(), relay: Some(addr.clone()) })).await;
        assert_eq!(ok.status(), StatusCode::OK);
        let settings = s.settings.lock().unwrap().clone();
        assert!(settings.account.attached());
        assert_eq!(settings.account.relay.as_deref(), Some(relay.addr.web_authority().as_str()));
        assert_eq!(settings.account.owner.as_ref().map(|o| o.kind), Some(OwnerKind::Organization));
        assert!(settings.account.accept, "comptes acceptés");
        assert!(settings.relay.remote_access, "accès hors de chez vous activé");
        assert_eq!(*name.lock().unwrap(), crate::system::computer_name().unwrap_or_default());
        let v = local_view(&s);
        assert_eq!(v["owner"]["name"], "Les Lignes Claires");
        assert_eq!(v["remote_access"], true);
    }

}
