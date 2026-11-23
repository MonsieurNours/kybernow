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
//! Le rattachement lui-même (dialogue avec le relais, `POST
//! /v1/stations/attach`) viendra avec le relais : il appellera [`pin`]. En
//! attendant, le mode d'essai ([`TEST_ENV`]) épingle une clé locale, et
//! `kyber-admin account-token` signe des jetons avec elle.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail};
use axum::Json;
use axum::extract::{Path as UrlPath, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{IntoResponse, Response};
use kybernow_tunnel::access::{self, AccessError, Claims, KeySet, PublicKey, Right, Rights, Use, Verifier};
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
/// celle du jeton d'accès (Kyber ne le lit qu'à la connexion).
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
}

impl Accounts {
    pub fn load(data_dir: &Path) -> Accounts {
        let path = data_dir.join(FILE);
        let stored = std::fs::read(&path).ok().and_then(|b| serde_json::from_slice::<Stored>(&b).ok()).unwrap_or_default();
        Accounts {
            path,
            stored: Mutex::new(stored),
            saved_seen: Mutex::new(HashMap::new()),
            relay_marks: Mutex::new(HashMap::new()),
            verifier: Mutex::new(None),
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
        st.account = AccountSettings { relay: Some(relay.clone()), owner: Some(owner), keys, pinned_at: Some(now_secs()), accept: true, test };
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
    tracing::info!("comptes : Station détachée");
    s.activity.event("info", Msg::new("events.accountDetached"));
    Ok(())
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
        // Le relais ne propose pas encore le rattachement : « Se connecter » attend.
        "can_attach": false,
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
}

pub async fn put(State(s): State<AppState>, Json(body): Json<AccountBody>) -> Response {
    if body.detach {
        if let Err(e) = detach(&s) {
            return crate::api::ApiError::from(e).into_response();
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
}
