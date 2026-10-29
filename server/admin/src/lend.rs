//! Prêter l'ordinateur (docs/pret.md) : les réglages (`[lend]` de
//! kyber-admin.toml), les comptes d'amis et leurs créneaux
//! (`lend-friends.json`, mots de passe hachés), le code de séance, les
//! demandes en attente de l'hôte et le prêt en cours.
//!
//! Ce module ne parle ni à Kyber, ni à Windows, ni au réseau : il garde
//! l'état et dit ce qu'il faut faire ; `lend_api.rs` le fait (écran virtuel,
//! agent de console, surveillance des processus, verrouillage). Le temps est
//! passé en paramètre aux fonctions qui en dépendent : les essais le
//! choisissent.

use std::collections::{HashMap, VecDeque};
use std::net::IpAddr;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail};
use rand::Rng;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::display::now_secs;
use crate::i18n::Msg;
use crate::launch::Launch;

/// Signe de vie de l'ami, en secondes (`beat` de la réponse à `borrow`).
pub const BEAT_SECONDS: u64 = 5;
/// Durée de vie du jeton de Kyber de l'ami, renouvelé à chaque signe de vie.
pub const TOKEN_LIFETIME: i64 = 120;
/// Un code de séance non utilisé vaut ce temps.
pub const CODE_TTL: Duration = Duration::from_secs(24 * 3600);
/// Une demande sans réponse de l'hôte expire après ce temps.
pub const REQUEST_TTL: Duration = Duration::from_secs(120);
/// Le préavis avant une reprise ou une fin prévue.
pub const NOTICE: Duration = Duration::from_secs(120);
/// Sans signe de vie de l'ami, le prêt continue ce temps (il peut revenir).
pub const COMEBACK: Duration = Duration::from_secs(15 * 60);
/// Durées proposées pour un code de séance (`null` : jusqu'à la reprise).
pub const DURATIONS: [u64; 3] = [3600, 7200, 14400];
/// Essais de `status` et `borrow` par adresse et par minute, comme le partage.
const TRIES_PER_MINUTE: usize = 10;
/// Mots de passe refusés avant de bloquer un compte d'ami, sur ce temps.
const FRIEND_LOCK_AFTER: usize = 10;
const FRIEND_LOCK_WINDOW: Duration = Duration::from_secs(10 * 60);
const FRIEND_LOCK_FOR: Duration = Duration::from_secs(10 * 60);
/// Une fin reste lisible ce temps au signe de vie suivant de l'ami.
const ENDED_KEEP: Duration = Duration::from_secs(60);
/// Cookie du navigateur de l'ami (client web) ; par le relais, `__Host-`.
pub const COOKIE: &str = "kybernow_lend";
pub const COOKIE_RELAY: &str = "__Host-kybernow_lend";
const MAX_NAME: usize = 40;
const MAX_DEVICE: usize = 80;
const APPS: &[&str] = &["mac", "windows", "linux", "ios", "tvos", "android", "webos", "web"];

fn random_hex(bytes: usize) -> String {
    let mut buf = vec![0u8; bytes];
    rand::rng().fill(&mut buf[..]);
    hex::encode(buf)
}

/// Comparaison à temps constant.
fn same(a: &str, b: &str) -> bool {
    let (a, b) = (a.as_bytes(), b.as_bytes());
    !b.is_empty() && a.len() == b.len() && a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

fn clean(text: &str, max: usize) -> String {
    text.trim().chars().filter(|c| !c.is_control()).take(max).collect::<String>().trim().to_string()
}

// --- Réglages --------------------------------------------------------------------

/// Le compte Steam du prêt.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum Steam {
    /// « Son compte Steam » : Steam se déconnecte du vôtre.
    #[default]
    Friend,
    /// « Votre compte Steam ».
    Host,
}

/// `[lend]` de kyber-admin.toml.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LendSettings {
    /// « Prêter l'ordinateur » : coupé au départ.
    #[serde(default)]
    pub enabled: bool,
    /// Steam parmi les apps permises (en Big Picture).
    #[serde(default = "yes")]
    pub steam: bool,
    /// Les autres apps permises, du catalogue de `launch.rs`.
    #[serde(default)]
    pub apps: Vec<Launch>,
}

fn yes() -> bool {
    true
}

impl Default for LendSettings {
    fn default() -> Self {
        LendSettings { enabled: false, steam: true, apps: Vec::new() }
    }
}

// --- Comptes d'amis et créneaux ---------------------------------------------------

/// Une plage de la semaine, à l'heure locale du PC : `days` de 1 (lundi) à 7
/// (dimanche), `start` < `end` (« 14:00 », « 18:00 »), le même jour.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Slot {
    pub days: Vec<u8>,
    pub start: String,
    pub end: String,
}

/// « 14:00 » → 840 (minutes depuis minuit).
fn minutes(text: &str) -> Option<u32> {
    let (h, m) = text.trim().split_once(':')?;
    if h.len() > 2 || m.len() != 2 {
        return None;
    }
    let (h, m): (u32, u32) = (h.parse().ok()?, m.parse().ok()?);
    match (h, m) {
        (24, 0) => Some(1440),
        (h, m) if h < 24 && m < 60 => Some(h * 60 + m),
        _ => None,
    }
}

impl Slot {
    fn check(&self) -> Result<()> {
        let (Some(start), Some(end)) = (minutes(&self.start), minutes(&self.end)) else {
            bail!(Msg::new("lend.badSlot"));
        };
        if self.days.is_empty() || self.days.iter().any(|d| !(1..=7).contains(d)) || start >= end {
            bail!(Msg::new("lend.badSlot"));
        }
        Ok(())
    }

    fn span(&self) -> (u32, u32) {
        (minutes(&self.start).unwrap_or(0), minutes(&self.end).unwrap_or(0))
    }
}

/// L'heure locale du PC, pour les créneaux : jour (1 = lundi) et minute du
/// jour, avec l'instant Unix correspondant au début de la minute.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LocalTime {
    pub weekday: u8,
    pub minute: u32,
    /// L'instant (secondes Unix) de ce moment.
    pub unix: u64,
}

impl LocalTime {
    /// Maintenant, à l'heure du PC (UTC si Windows ne la donne pas).
    pub fn now() -> LocalTime {
        let now = time::OffsetDateTime::now_local().unwrap_or_else(|_| time::OffsetDateTime::now_utc());
        LocalTime {
            weekday: now.weekday().number_from_monday(),
            minute: u32::from(now.hour()) * 60 + u32::from(now.minute()),
            unix: now.unix_timestamp().max(0) as u64 - u64::from(now.second()),
        }
    }
}

/// Dans un créneau en ce moment : la fin de la plage (secondes Unix).
pub fn slot_now(slots: &[Slot], at: LocalTime) -> Option<u64> {
    slots
        .iter()
        .filter(|s| s.days.contains(&at.weekday))
        .map(Slot::span)
        .filter(|(start, end)| *start <= at.minute && at.minute < *end)
        .map(|(_, end)| at.unix + u64::from(end - at.minute) * 60)
        .max()
}

/// Le prochain créneau à venir (ou en cours) dans les sept jours : début et fin.
pub fn next_slot(slots: &[Slot], at: LocalTime) -> Option<(u64, u64)> {
    let mut best: Option<(u64, u64)> = None;
    for day_offset in 0..8u32 {
        let weekday = ((u32::from(at.weekday) - 1 + day_offset) % 7 + 1) as u8;
        for s in slots.iter().filter(|s| s.days.contains(&weekday)) {
            let (start, end) = s.span();
            // Minutes depuis maintenant jusqu'au début et à la fin.
            let base = i64::from(day_offset) * 1440 - i64::from(at.minute);
            let (to_start, to_end) = (base + i64::from(start), base + i64::from(end));
            if to_end <= 0 {
                continue;
            }
            let start_unix = (at.unix as i64 + to_start.max(0) * 60) as u64;
            let end_unix = (at.unix as i64 + to_end * 60) as u64;
            if best.is_none_or(|(b, _)| start_unix < b) {
                best = Some((start_unix, end_unix));
            }
        }
    }
    best
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Friend {
    pub id: String,
    pub name: String,
    pub login: String,
    /// Empreinte Argon2 du mot de passe, jamais rendue.
    hash: String,
    #[serde(default)]
    pub steam: Steam,
    #[serde(default)]
    pub slots: Vec<Slot>,
    #[serde(default = "yes")]
    pub enabled: bool,
    #[serde(default)]
    pub last_used: Option<u64>,
}

impl Friend {
    /// Ce que voit Admin (jamais le mot de passe).
    pub fn json(&self) -> Value {
        json!({
            "id": self.id,
            "name": self.name,
            "login": self.login,
            "steam": self.steam,
            "slots": self.slots,
            "enabled": self.enabled,
            "last_used": self.last_used,
        })
    }

    pub fn hash(&self) -> &str {
        &self.hash
    }
}

/// Un identifiant d'ami : 3 à 32 caractères `a-z0-9._-`.
pub fn valid_login(login: &str) -> bool {
    (3..=32).contains(&login.len()) && login.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b"._-".contains(&b))
}

/// Empreinte Argon2 d'un mot de passe d'ami (6 caractères au moins).
pub fn hash_friend_password(password: &str) -> Result<String> {
    use argon2::password_hash::{PasswordHasher, SaltString};
    if password.chars().count() < 6 {
        bail!(Msg::new("lend.passwordLength"));
    }
    let salt_bytes: [u8; 16] = rand::random();
    let salt = SaltString::encode_b64(&salt_bytes).map_err(|e| anyhow::anyhow!("{e}"))?;
    Ok(argon2::Argon2::default().hash_password(password.as_bytes(), &salt).map_err(|e| anyhow::anyhow!("{e}"))?.to_string())
}

pub fn verify_friend_password(hash: &str, password: &str) -> bool {
    use argon2::password_hash::{PasswordHash, PasswordVerifier};
    PasswordHash::new(hash).is_ok_and(|parsed| argon2::Argon2::default().verify_password(password.as_bytes(), &parsed).is_ok())
}

/// Ce qu'Admin envoie pour créer ou changer un compte d'ami.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct FriendBody {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub login: Option<String>,
    #[serde(default)]
    pub password: Option<String>,
    #[serde(default)]
    pub steam: Option<Steam>,
    #[serde(default)]
    pub slots: Option<Vec<Slot>>,
    #[serde(default)]
    pub enabled: Option<bool>,
}

/// Les comptes d'amis, gardés dans `lend-friends.json` du dossier de données
/// (réservé à SYSTEM et aux Administrateurs).
pub struct Friends {
    path: PathBuf,
    list: Mutex<Vec<Friend>>,
}

pub const FRIENDS_FILE: &str = "lend-friends.json";

impl Friends {
    pub fn load(data_dir: &Path) -> Friends {
        let path = data_dir.join(FRIENDS_FILE);
        let list = std::fs::read(&path).ok().and_then(|b| serde_json::from_slice::<Vec<Friend>>(&b).ok()).unwrap_or_default();
        Friends { path, list: Mutex::new(list) }
    }

    fn save(&self, list: &[Friend]) -> Result<()> {
        let tmp = self.path.with_extension("json.tmp");
        std::fs::write(&tmp, serde_json::to_vec_pretty(list)?).with_context(|| format!("écriture de {}", tmp.display()))?;
        std::fs::rename(&tmp, &self.path).with_context(|| format!("remplacement de {}", self.path.display()))
    }

    pub fn all(&self) -> Vec<Friend> {
        self.list.lock().unwrap().clone()
    }

    pub fn get(&self, id: &str) -> Option<Friend> {
        self.list.lock().unwrap().iter().find(|f| f.id == id).cloned()
    }

    pub fn by_login(&self, login: &str) -> Option<Friend> {
        let login = login.trim().to_ascii_lowercase();
        self.list.lock().unwrap().iter().find(|f| f.login == login).cloned()
    }

    pub fn any_enabled(&self) -> bool {
        self.list.lock().unwrap().iter().any(|f| f.enabled)
    }

    /// Crée un compte (`hash` déjà calculé, hors du fil asynchrone).
    pub fn create(&self, body: &FriendBody, hash: String) -> Result<Friend> {
        let name = clean(body.name.as_deref().unwrap_or(""), MAX_NAME);
        let login = body.login.as_deref().unwrap_or("").trim().to_ascii_lowercase();
        if name.is_empty() {
            bail!(Msg::new("lend.nameRequired"));
        }
        if !valid_login(&login) {
            bail!(Msg::new("lend.badLogin"));
        }
        let slots = body.slots.clone().unwrap_or_default();
        for s in &slots {
            s.check()?;
        }
        let mut list = self.list.lock().unwrap();
        if list.iter().any(|f| f.login == login) {
            bail!(Msg::new("lend.loginTaken").with("login", login.as_str()));
        }
        let friend = Friend {
            id: random_hex(8),
            name,
            login,
            hash,
            steam: body.steam.unwrap_or_default(),
            slots,
            enabled: body.enabled.unwrap_or(true),
            last_used: None,
        };
        list.push(friend.clone());
        self.save(&list)?;
        Ok(friend)
    }

    /// Change un compte ; `hash` : nouveau mot de passe déjà haché.
    pub fn update(&self, id: &str, body: &FriendBody, hash: Option<String>) -> Result<Option<Friend>> {
        let mut list = self.list.lock().unwrap();
        let taken = |login: &str, list: &[Friend]| list.iter().any(|f| f.login == login && f.id != id);
        let Some(index) = list.iter().position(|f| f.id == id) else { return Ok(None) };
        let mut friend = list[index].clone();
        if let Some(name) = &body.name {
            let name = clean(name, MAX_NAME);
            if name.is_empty() {
                bail!(Msg::new("lend.nameRequired"));
            }
            friend.name = name;
        }
        if let Some(login) = &body.login {
            let login = login.trim().to_ascii_lowercase();
            if !valid_login(&login) {
                bail!(Msg::new("lend.badLogin"));
            }
            if taken(&login, &list) {
                bail!(Msg::new("lend.loginTaken").with("login", login.as_str()));
            }
            friend.login = login;
        }
        if let Some(slots) = &body.slots {
            for s in slots {
                s.check()?;
            }
            friend.slots = slots.clone();
        }
        if let Some(steam) = body.steam {
            friend.steam = steam;
        }
        if let Some(enabled) = body.enabled {
            friend.enabled = enabled;
        }
        if let Some(hash) = hash {
            friend.hash = hash;
        }
        list[index] = friend.clone();
        self.save(&list)?;
        Ok(Some(friend))
    }

    pub fn delete(&self, id: &str) -> Result<Option<Friend>> {
        let mut list = self.list.lock().unwrap();
        let Some(index) = list.iter().position(|f| f.id == id) else { return Ok(None) };
        let friend = list.remove(index);
        self.save(&list)?;
        Ok(Some(friend))
    }

    pub fn touch(&self, id: &str) {
        let mut list = self.list.lock().unwrap();
        if let Some(f) = list.iter_mut().find(|f| f.id == id) {
            f.last_used = Some(now_secs());
            let _ = self.save(&list);
        }
    }
}

// --- Limites --------------------------------------------------------------------------

#[derive(Default)]
struct Strikes {
    failures: VecDeque<Instant>,
    until: Option<Instant>,
}

/// Les essais de l'ami : par adresse (10 par minute) et par compte (10 mots
/// de passe refusés en 10 minutes bloquent le compte 10 minutes).
#[derive(Default)]
pub struct Limits {
    tries: Mutex<HashMap<IpAddr, Vec<Instant>>>,
    accounts: Mutex<HashMap<String, Strikes>>,
}

impl Limits {
    /// Un essai depuis `peer` : `Err(attente en secondes)` au-delà de 10 par minute.
    pub fn count_try(&self, peer: IpAddr, now: Instant) -> Result<(), u64> {
        let mut tries = self.tries.lock().unwrap();
        tries.retain(|_, times| {
            times.retain(|t| now.saturating_duration_since(*t) < Duration::from_secs(60));
            !times.is_empty()
        });
        let times = tries.entry(peer.to_canonical()).or_default();
        if times.len() >= TRIES_PER_MINUTE {
            let oldest = times.iter().min().copied().unwrap_or(now);
            return Err(Duration::from_secs(60).saturating_sub(now.saturating_duration_since(oldest)).as_secs().max(1));
        }
        times.push(now);
        Ok(())
    }

    /// Ce compte est-il bloqué (attente en secondes) ?
    pub fn account_locked(&self, login: &str, now: Instant) -> Option<u64> {
        let accounts = self.accounts.lock().unwrap();
        let until = accounts.get(login)?.until?;
        (until > now).then(|| (until - now).as_secs().max(1))
    }

    /// Un mot de passe refusé pour ce compte.
    pub fn account_failed(&self, login: &str, now: Instant) {
        let mut accounts = self.accounts.lock().unwrap();
        let s = accounts.entry(login.to_string()).or_default();
        while s.failures.front().is_some_and(|t| now.saturating_duration_since(*t) >= FRIEND_LOCK_WINDOW) {
            s.failures.pop_front();
        }
        s.failures.push_back(now);
        if s.failures.len() >= FRIEND_LOCK_AFTER {
            s.until = Some(now + FRIEND_LOCK_FOR);
            s.failures.clear();
            tracing::warn!("prêt : compte d'ami {login} bloqué 10 min après {FRIEND_LOCK_AFTER} mots de passe refusés");
        }
    }
}

// --- Le prêt --------------------------------------------------------------------------

/// Comment l'ami est venu.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    Code,
    Friend,
    Slot,
}

/// Pourquoi un préavis, ou une fin.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Reason {
    /// L'hôte reprend l'ordinateur.
    TakenBack,
    /// Fin de la durée du code.
    TimeUp,
    /// Fin du créneau.
    SlotEnd,
    /// Le créneau d'un ami commence (chez l'hôte).
    SlotStart,
    /// L'ami a rendu l'ordinateur.
    Left,
    /// Arrêté autrement (prêt coupé, ami parti 15 min, agent perdu, redémarrage).
    Stopped,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Notice {
    pub reason: Reason,
    /// Quand (secondes Unix).
    pub at: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Phase {
    /// L'écran virtuel et le mode console se mettent en place.
    Preparing,
    Lent,
    /// Remise en état (agent de console, apps, écran, verrouillage).
    Ending,
}

/// La taille de l'écran de l'ami (son écran virtuel).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Screen {
    pub width: u32,
    pub height: u32,
    #[serde(default = "sixty")]
    pub refresh: u32,
}

fn sixty() -> u32 {
    60
}

impl Default for Screen {
    fn default() -> Self {
        Screen { width: 1920, height: 1080, refresh: 60 }
    }
}

/// Qui emprunte : ce qui sert à le reconnaître quand il revient.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Who {
    pub name: String,
    pub device: String,
    pub app: String,
    pub via_relay: bool,
    pub kind: Kind,
    pub friend: Option<String>,
    pub code: Option<String>,
    pub screen: Screen,
    pub browser: bool,
}

impl Who {
    pub fn new(name: &str, device: &str, app: &str, via_relay: bool, kind: Kind, screen: Option<Screen>) -> Who {
        let app = if APPS.contains(&app) { app.to_string() } else { "other".to_string() };
        let mut screen = screen.unwrap_or_default();
        // Bornes de l'écran virtuel (comme `prepare`) : la validation finale est celle de l'écran.
        screen.width = screen.width.clamp(640, 7680) & !1;
        screen.height = screen.height.clamp(480, 4320) & !1;
        screen.refresh = screen.refresh.clamp(24, 240);
        Who {
            name: clean(name, MAX_NAME),
            device: clean(device, MAX_DEVICE),
            browser: app == "web",
            app,
            via_relay,
            kind,
            friend: None,
            code: None,
            screen,
        }
    }

    /// Le même ami qui revient : même appareil, même compte, ou même code.
    fn same_as(&self, other: &Who) -> bool {
        (!self.device.is_empty() && self.device == other.device)
            || (self.friend.is_some() && self.friend == other.friend)
            || (self.code.is_some() && self.code == other.code)
    }
}

#[derive(Debug, Clone)]
pub struct LendCode {
    pub code: String,
    pub duration: Option<u64>,
    pub steam: Steam,
    pub expires: u64,
    /// Créé par l'appareil en session : vaut accord, sa session se fermera.
    pub from_session: bool,
}

#[derive(Debug, Clone)]
pub struct Request {
    pub id: String,
    secret: String,
    pub who: Who,
    pub since: u64,
    requested: Instant,
    /// La durée et le compte Steam proposés (ceux du code, ou du compte d'ami).
    pub duration: Option<u64>,
    pub until: Option<u64>,
    pub steam: Steam,
    /// Créneau : accordée d'office à ce moment (préavis de 2 min chez l'hôte).
    pub auto_at: Option<Instant>,
    /// L'hôte a répondu oui ; le prêt se prépare.
    pub accepted: bool,
}

impl Request {
    pub fn secret(&self) -> &str {
        &self.secret
    }
}

#[derive(Debug, Clone)]
pub struct Loan {
    pub id: String,
    secret: String,
    /// Cookie du navigateur de l'ami.
    cookie: Option<String>,
    pub who: Who,
    pub since: u64,
    pub until: Option<u64>,
    pub steam: Steam,
    pub phase: Phase,
    pub notice: Option<Notice>,
    /// L'écran virtuel de l'ami (`kn_display`), une fois créé.
    pub display: Option<(u32, Screen)>,
    last_beat: Instant,
    pub connected: bool,
    /// Le mode console est en place (agent prêt).
    pub console_ready: bool,
    pub console_beat: Option<Instant>,
    pub console_restarts: u32,
    /// La demande d'où il vient et son secret (pour `wait`).
    request: Option<(String, String)>,
}

impl Loan {
    pub fn secret(&self) -> &str {
        &self.secret
    }

    pub fn cookie(&self) -> Option<&str> {
        self.cookie.as_deref()
    }

    /// Le sujet de ses jetons de Kyber.
    pub fn kyber_user(&self) -> String {
        format!("kn-lend-{}", self.id)
    }
}

#[derive(Debug, Clone)]
struct Ended {
    secret: String,
    reason: Reason,
    at: Instant,
    /// Une demande refusée ou expirée (pour `wait`).
    request: bool,
}

/// L'état du prêt, en mémoire.
#[derive(Default)]
struct State {
    code: Option<LendCode>,
    requests: Vec<Request>,
    loan: Option<Loan>,
    ended: HashMap<String, Ended>,
}

/// Ce que `wait` apprend d'une demande.
#[derive(Debug, Clone)]
#[allow(clippy::large_enum_variant)] // rendue une fois par appel, jamais gardée
pub enum Wait {
    Pending,
    /// Le prêt est prêt pour elle.
    Lent(Loan),
    Refused,
    Expired,
    Unknown,
}

/// Ce que `beat` rend.
#[derive(Debug, Clone)]
#[allow(clippy::large_enum_variant)] // rendue une fois par appel, jamais gardée
pub enum Beat {
    Live(Loan),
    Ended(Reason),
    Unknown,
}

#[derive(Default)]
pub struct Lending {
    state: Mutex<State>,
    pub limits: Limits,
    /// Réveille les attentes (`borrow`, `wait`) et la veille.
    changed: tokio::sync::Notify,
}

/// Les codes de six chiffres, tirés au hasard.
pub fn new_code() -> String {
    format!("{:06}", rand::rng().random_range(0..1_000_000u32))
}

impl Lending {
    pub fn new() -> Self {
        Self::default()
    }

    /// Une attente à prendre avant de relire l'état.
    pub fn changed(&self) -> tokio::sync::futures::Notified<'_> {
        self.changed.notified()
    }

    pub fn notify(&self) {
        self.changed.notify_waiters();
    }

    pub fn loan(&self) -> Option<Loan> {
        self.state.lock().unwrap().loan.clone()
    }

    pub fn code(&self) -> Option<LendCode> {
        self.state.lock().unwrap().code.clone()
    }

    pub fn requests(&self) -> Vec<Request> {
        self.state.lock().unwrap().requests.clone()
    }

    /// Le code de séance en cours (pour le relais et l'espace des codes).
    pub fn code_value(&self) -> Option<String> {
        self.state.lock().unwrap().code.as_ref().map(|c| c.code.clone())
    }

    /// Ce code est-il celui d'un prêt (code de séance, ou du prêt en cours) ?
    pub fn is_lend_code(&self, code: &str) -> bool {
        let s = self.state.lock().unwrap();
        s.code.as_ref().is_some_and(|c| same(code, &c.code)) || s.loan.as_ref().and_then(|l| l.who.code.as_deref()).is_some_and(|c| same(code, c))
    }

    /// Le nom de l'ami à qui l'ordinateur est prêté (gardes de la Station).
    pub fn borrower(&self) -> Option<String> {
        self.state.lock().unwrap().loan.as_ref().map(|l| l.who.name.clone())
    }

    /// Un code de séance : `taken` dit si un code est déjà pris (partage).
    pub fn create_code(&self, duration: Option<u64>, steam: Steam, from_session: bool, taken: impl Fn(&str) -> bool) -> LendCode {
        let mut code = new_code();
        while taken(&code) {
            code = new_code();
        }
        let lend = LendCode { code, duration, steam, expires: now_secs() + CODE_TTL.as_secs(), from_session };
        self.state.lock().unwrap().code = Some(lend.clone());
        self.notify();
        lend
    }

    /// Le code réservé chez le relais remplace celui du code de séance.
    pub fn replace_code(&self, old: &str, new: &str) -> bool {
        let mut s = self.state.lock().unwrap();
        match s.code.as_mut() {
            Some(c) if c.code == old => {
                c.code = new.to_string();
                true
            }
            _ => false,
        }
    }

    pub fn cancel_code(&self) -> Option<LendCode> {
        let code = self.state.lock().unwrap().code.take();
        self.notify();
        code
    }

    /// L'ami arrive avec un code. `Err(None)` : pas ce code ; sinon le code.
    pub fn take_code(&self, code: &str) -> Option<LendCode> {
        let code: String = code.chars().filter(|c| !c.is_whitespace()).collect();
        let s = self.state.lock().unwrap();
        s.code.as_ref().filter(|c| same(&code, &c.code) && c.expires > now_secs()).cloned()
    }

    /// Le même ami qui revient pendant le prêt (15 min) : son prêt, avec un
    /// nouveau secret (et cookie). `None` : pas lui.
    pub fn resume(&self, who: &Who) -> Option<Loan> {
        let mut s = self.state.lock().unwrap();
        let loan = s.loan.as_mut().filter(|l| l.phase != Phase::Ending && l.who.same_as(who))?;
        loan.secret = random_hex(32);
        loan.cookie = who.browser.then(|| random_hex(32));
        loan.who.via_relay = who.via_relay;
        loan.who.app = who.app.clone();
        loan.who.browser = who.browser;
        loan.last_beat = Instant::now();
        loan.connected = true;
        Some(loan.clone())
    }

    /// Un prêt (ou une reprise de la demande) qui démarre pour `who`. Un code
    /// est consommé ici (une séance).
    pub fn begin(&self, who: Who, until: Option<u64>, steam: Steam, request: Option<(String, String)>) -> Loan {
        let mut s = self.state.lock().unwrap();
        if let Some(code) = &who.code
            && s.code.as_ref().is_some_and(|c| &c.code == code)
        {
            s.code = None;
        }
        let loan = Loan {
            id: random_hex(8),
            secret: random_hex(32),
            cookie: who.browser.then(|| random_hex(32)),
            who,
            since: now_secs(),
            until,
            steam,
            phase: Phase::Preparing,
            notice: None,
            display: None,
            last_beat: Instant::now(),
            connected: true,
            console_ready: false,
            console_beat: None,
            console_restarts: 0,
            request,
        };
        s.loan = Some(loan.clone());
        if let Some((r, _)) = &loan.request {
            s.requests.retain(|q| &q.id != r);
        }
        drop(s);
        self.notify();
        loan
    }

    /// Une demande en attente de l'hôte (ou d'un créneau, accordée à `auto_at`).
    pub fn ask(&self, who: Who, duration: Option<u64>, until: Option<u64>, steam: Steam, auto_at: Option<Instant>) -> Request {
        let mut s = self.state.lock().unwrap();
        // Le même ami qui redemande remplace sa demande.
        s.requests.retain(|r| !r.who.same_as(&who));
        let request = Request {
            id: random_hex(8),
            secret: random_hex(32),
            who,
            since: now_secs(),
            requested: Instant::now(),
            duration,
            until,
            steam,
            auto_at,
            accepted: false,
        };
        s.requests.push(request.clone());
        drop(s);
        self.notify();
        request
    }

    /// L'hôte répond à une demande. `Some(demande)` si elle existait.
    pub fn answer(&self, id: &str, accept: bool, steam: Option<Steam>) -> Option<Request> {
        let mut s = self.state.lock().unwrap();
        let index = s.requests.iter().position(|r| r.id == id && !r.accepted)?;
        if accept {
            let r = &mut s.requests[index];
            r.accepted = true;
            if let Some(steam) = steam {
                r.steam = steam;
            }
            let r = r.clone();
            drop(s);
            self.notify();
            return Some(r);
        }
        let r = s.requests.remove(index);
        s.ended.insert(r.id.clone(), Ended { secret: r.secret.clone(), reason: Reason::Stopped, at: Instant::now(), request: true });
        drop(s);
        self.notify();
        Some(r)
    }

    /// Où en est la demande `id` pour `wait`.
    pub fn wait(&self, id: &str, secret: &str) -> Wait {
        let mut s = self.state.lock().unwrap();
        let now = Instant::now();
        s.ended.retain(|_, e| now.duration_since(e.at) < ENDED_KEEP);
        // La demande a mené au prêt : prêt quand le mode console est en place.
        if let Some(loan) = s.loan.as_ref().filter(|l| l.request.as_ref().is_some_and(|(i, sec)| i == id && same(secret, sec))) {
            return if loan.phase == Phase::Lent { Wait::Lent(loan.clone()) } else { Wait::Pending };
        }
        if s.requests.iter().any(|r| r.id == id && same(secret, &r.secret)) {
            return Wait::Pending;
        }
        match s.ended.get(id).filter(|e| same(secret, &e.secret) && e.request) {
            Some(e) if e.reason == Reason::TimeUp => Wait::Expired,
            Some(_) => Wait::Refused,
            None => Wait::Unknown,
        }
    }

    /// La demande acceptée devient un prêt : son secret reste lisible par `wait`.
    pub fn begin_from_request(&self, id: &str, until: Option<u64>) -> Option<Loan> {
        let request = {
            let s = self.state.lock().unwrap();
            s.requests.iter().find(|r| r.id == id && r.accepted).cloned()
        }?;
        let until = until.or(request.until).or(request.duration.map(|d| now_secs() + d));
        Some(self.begin(request.who.clone(), until, request.steam, Some((request.id.clone(), request.secret.clone()))))
    }

    /// L'ami annule sa demande (`leave` avec le secret de la demande).
    pub fn cancel_request(&self, id: &str, secret: &str) -> bool {
        let mut s = self.state.lock().unwrap();
        let before = s.requests.len();
        s.requests.retain(|r| !(r.id == id && same(secret, &r.secret)));
        let gone = s.requests.len() != before;
        drop(s);
        if gone {
            self.notify();
        }
        gone
    }

    /// L'écran virtuel de l'ami est là.
    pub fn set_display(&self, id: &str, display: u32, screen: Screen) {
        if let Some(l) = self.state.lock().unwrap().loan.as_mut().filter(|l| l.id == id) {
            l.display = Some((display, screen));
        }
    }

    /// Le mode console est en place : le prêt commence.
    pub fn console_ready(&self, id: Option<&str>) -> Option<Loan> {
        let mut s = self.state.lock().unwrap();
        let loan = s.loan.as_mut().filter(|l| id.is_none_or(|i| l.id == i))?;
        loan.console_ready = true;
        loan.console_beat = Some(Instant::now());
        if loan.phase == Phase::Preparing {
            loan.phase = Phase::Lent;
        }
        let loan = loan.clone();
        drop(s);
        self.notify();
        Some(loan)
    }

    pub fn console_beat(&self) {
        if let Some(l) = self.state.lock().unwrap().loan.as_mut() {
            l.console_beat = Some(Instant::now());
        }
    }

    pub fn console_restarted(&self) {
        if let Some(l) = self.state.lock().unwrap().loan.as_mut() {
            l.console_restarts += 1;
            l.console_beat = Some(Instant::now());
        }
    }

    /// Signe de vie de l'ami.
    pub fn beat(&self, id: &str, secret: &str) -> Beat {
        let mut s = self.state.lock().unwrap();
        let now = Instant::now();
        s.ended.retain(|_, e| now.duration_since(e.at) < ENDED_KEEP);
        if let Some(loan) = s.loan.as_mut().filter(|l| l.id == id && same(secret, &l.secret)) {
            loan.last_beat = now;
            loan.connected = true;
            return Beat::Live(loan.clone());
        }
        match s.ended.get(id).filter(|e| same(secret, &e.secret) && !e.request) {
            Some(e) => Beat::Ended(e.reason),
            None => Beat::Unknown,
        }
    }

    /// Préavis de 2 min (reprise, fin prévue), s'il n'y en a pas déjà un plus proche.
    pub fn notice(&self, reason: Reason, at: u64) -> Option<Loan> {
        let mut s = self.state.lock().unwrap();
        let loan = s.loan.as_mut().filter(|l| l.phase != Phase::Ending)?;
        if loan.notice.is_none_or(|n| n.at > at) {
            loan.notice = Some(Notice { reason, at });
        }
        let loan = loan.clone();
        drop(s);
        self.notify();
        Some(loan)
    }

    /// La remise en état commence : `None` si rien à arrêter (ou déjà en cours).
    pub fn ending(&self, id: Option<&str>) -> Option<Loan> {
        let mut s = self.state.lock().unwrap();
        let loan = s.loan.as_mut().filter(|l| id.is_none_or(|i| l.id == i) && l.phase != Phase::Ending)?;
        loan.phase = Phase::Ending;
        let loan = loan.clone();
        drop(s);
        self.notify();
        Some(loan)
    }

    /// Le prêt est fini : la raison reste lisible au signe de vie suivant.
    pub fn ended(&self, id: &str, reason: Reason) -> Option<Loan> {
        let mut s = self.state.lock().unwrap();
        let loan = s.loan.take_if(|l| l.id == id)?;
        s.ended.insert(loan.id.clone(), Ended { secret: loan.secret.clone(), reason, at: Instant::now(), request: false });
        drop(s);
        self.notify();
        Some(loan)
    }

    /// Le cookie d'un navigateur d'ami, tant que le prêt vit.
    pub fn cookie_valid(&self, cookie: &str) -> bool {
        let s = self.state.lock().unwrap();
        s.loan.as_ref().is_some_and(|l| l.phase != Phase::Ending && l.cookie.as_deref().is_some_and(|c| same(cookie, c)))
    }

    /// Le tour de la veille : ce qu'il faut faire maintenant.
    pub fn tick(&self, now: Instant, now_unix: u64) -> Tick {
        let mut s = self.state.lock().unwrap();
        let mut tick = Tick::default();
        // Code de séance échu.
        if s.code.as_ref().is_some_and(|c| c.expires <= now_unix) {
            s.code = None;
            tick.changed = true;
        }
        // Demandes : expirées (2 min), ou accordées d'office (créneau).
        let mut kept = Vec::new();
        for r in std::mem::take(&mut s.requests) {
            if r.accepted {
                kept.push(r);
            } else if r.auto_at.is_some_and(|t| t <= now) {
                tick.auto_accept.push(r.id.clone());
                let mut r = r;
                r.accepted = true;
                kept.push(r);
            } else if r.auto_at.is_none() && now.saturating_duration_since(r.requested) >= REQUEST_TTL {
                s.ended.insert(r.id.clone(), Ended { secret: r.secret.clone(), reason: Reason::TimeUp, at: now, request: true });
                tick.expired.push(r.who.name.clone());
            } else {
                kept.push(r);
            }
        }
        s.requests = kept;
        if let Some(loan) = s.loan.as_mut() {
            // Préparation et prêt : l'écran virtuel et le son sont gardés en vie.
            if loan.phase != Phase::Ending {
                tick.keep_alive = Some(loan.id.clone());
            }
            if loan.phase == Phase::Lent {
                // Fin prévue : préavis 2 min avant.
                if let Some(until) = loan.until
                    && loan.notice.is_none()
                    && now_unix + NOTICE.as_secs() >= until
                {
                    let reason = if loan.who.kind == Kind::Slot { Reason::SlotEnd } else { Reason::TimeUp };
                    loan.notice = Some(Notice { reason, at: until });
                    tick.changed = true;
                }
                if let Some(n) = loan.notice.filter(|n| n.at <= now_unix) {
                    tick.end = Some((loan.id.clone(), n.reason));
                } else if now.saturating_duration_since(loan.last_beat) >= COMEBACK {
                    tick.end = Some((loan.id.clone(), Reason::Stopped));
                }
                // L'ami ne donne plus de nouvelles : son image s'arrête (jeton), le prêt continue.
                if loan.connected && now.saturating_duration_since(loan.last_beat) > crate::activity::HEARTBEAT_GRACE {
                    loan.connected = false;
                    tick.changed = true;
                }
                tick.console_silent =
                    loan.console_ready && loan.console_beat.is_some_and(|b| now.saturating_duration_since(b) > Duration::from_secs(10));
            }
        }
        if tick.changed || !tick.auto_accept.is_empty() || !tick.expired.is_empty() || tick.end.is_some() {
            drop(s);
            self.notify();
        }
        tick
    }

    /// La vue du prêt (contrat, « Échange avec Admin »).
    pub fn view(&self, enabled: bool) -> Value {
        let s = self.state.lock().unwrap();
        let state = match s.loan.as_ref().map(|l| l.phase) {
            None => "free",
            Some(Phase::Preparing) => "preparing",
            Some(Phase::Lent) => "lent",
            Some(Phase::Ending) => "ending",
        };
        let borrower = s.loan.as_ref().map(|l| {
            json!({
                "name": l.who.name,
                "kind": l.who.kind,
                "app": l.who.app,
                "via": if l.who.via_relay { "relay" } else { "lan" },
                "since": l.since,
                "until": l.until,
                "steam": l.steam,
                "connected": l.connected,
            })
        });
        let requests: Vec<Value> = s
            .requests
            .iter()
            .filter(|r| !r.accepted)
            .map(|r| {
                json!({
                    "id": r.id,
                    "name": r.who.name,
                    "kind": if r.who.kind == Kind::Code { "code" } else { "friend" },
                    "app": r.who.app,
                    "via": if r.who.via_relay { "relay" } else { "lan" },
                    "since": r.since,
                    "slot": r.auto_at.is_some(),
                })
            })
            .collect();
        json!({
            "available": true,
            "enabled": enabled,
            "state": state,
            "borrower": borrower,
            "notice": s.loan.as_ref().and_then(|l| l.notice),
            "requests": requests,
        })
    }

    #[cfg(test)]
    fn age_request(&self, id: &str, by: Duration) {
        if let Some(r) = self.state.lock().unwrap().requests.iter_mut().find(|r| r.id == id) {
            r.requested -= by;
        }
    }

    #[cfg(test)]
    fn age_loan_beat(&self, by: Duration) {
        if let Some(l) = self.state.lock().unwrap().loan.as_mut() {
            l.last_beat -= by;
        }
    }
}

/// Ce que la veille doit faire après un tour.
#[derive(Debug, Default)]
pub struct Tick {
    /// Le prêt à finir, et pourquoi.
    pub end: Option<(String, Reason)>,
    /// Demandes accordées d'office (créneau) : préparer leur prêt.
    pub auto_accept: Vec<String>,
    /// Demandes expirées (prénoms, pour le fil).
    pub expired: Vec<String>,
    /// Le prêt en cours : son écran virtuel et son son sont gardés en vie.
    pub keep_alive: Option<String>,
    /// L'agent de console ne donne plus de nouvelles.
    pub console_silent: bool,
    pub changed: bool,
}

/// Le cookie de l'ami d'une requête : celui de sa voie (`__Host-` par le
/// relais), comme le cookie d'un invité du partage.
pub fn cookie(headers: &axum::http::HeaderMap, relay: bool) -> Option<String> {
    let name = if relay { COOKIE_RELAY } else { COOKIE };
    headers
        .get_all(axum::http::header::COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .flat_map(|v| v.split(';'))
        .filter_map(|c| c.trim().split_once('='))
        .find(|(k, _)| *k == name)
        .map(|(_, v)| v.to_string())
}

pub fn set_cookie(value: &str, secure: bool, relay: bool) -> String {
    if relay {
        return format!("{COOKIE_RELAY}={value}; Path=/; HttpOnly; SameSite=Strict; Secure; Max-Age=43200");
    }
    format!("{COOKIE}={value}; Path=/; HttpOnly; SameSite=Strict; Max-Age=43200{}", if secure { "; Secure" } else { "" })
}

pub fn clear_cookie(secure: bool, relay: bool) -> String {
    if relay {
        return format!("{COOKIE_RELAY}=; Path=/; HttpOnly; SameSite=Strict; Secure; Max-Age=0");
    }
    format!("{COOKIE}=; Path=/; HttpOnly; SameSite=Strict; Max-Age=0{}", if secure { "; Secure" } else { "" })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn who(name: &str, device: &str) -> Who {
        Who::new(name, device, "ios", false, Kind::Code, Some(Screen { width: 2560, height: 1440, refresh: 120 }))
    }

    fn at(weekday: u8, hh: u32, mm: u32) -> LocalTime {
        // Un lundi 00:00 fictif à 1_000_000 s, puis jours et minutes.
        LocalTime { weekday, minute: hh * 60 + mm, unix: 1_000_000 + u64::from(weekday - 1) * 86_400 + u64::from(hh * 60 + mm) * 60 }
    }

    fn slot(days: &[u8], start: &str, end: &str) -> Slot {
        Slot { days: days.to_vec(), start: start.into(), end: end.into() }
    }

    #[test]
    fn creneaux_de_la_semaine() {
        let slots = vec![slot(&[3], "14:00", "18:00")];
        // Mercredi 15:30 : dans le créneau, fin à 18:00.
        let now = at(3, 15, 30);
        assert_eq!(slot_now(&slots, now), Some(now.unix + 150 * 60));
        assert_eq!(slot_now(&slots, at(3, 18, 0)), None);
        assert_eq!(slot_now(&slots, at(2, 15, 0)), None);
        // Mardi 10:00 : le prochain est demain 14:00 – 18:00.
        let tue = at(2, 10, 0);
        let (start, end) = next_slot(&slots, tue).unwrap();
        assert_eq!((start - tue.unix) / 60, 28 * 60);
        assert_eq!(end - start, 4 * 3600);
        // Mercredi 19:00 : la semaine suivante.
        let wed = at(3, 19, 0);
        let (start, _) = next_slot(&slots, wed).unwrap();
        assert_eq!((start - wed.unix) / 3600, 7 * 24 - 5);
        // Dans le créneau : le prochain est celui-ci (commencé).
        let (start, end) = next_slot(&slots, now).unwrap();
        assert_eq!((start, end), (now.unix, now.unix + 150 * 60));
        assert!(slot(&[8], "10:00", "11:00").check().is_err());
        assert!(slot(&[1], "11:00", "10:00").check().is_err());
        assert!(slot(&[1], "10:00", "24:00").check().is_ok());
        assert!(slot(&[], "10:00", "11:00").check().is_err());
    }

    #[test]
    fn comptes_d_amis_gardes_sur_disque() {
        let dir = std::env::temp_dir().join(format!("kn-lend-{}", rand::random::<u64>()));
        std::fs::create_dir_all(&dir).unwrap();
        let friends = Friends::load(&dir);
        let hash = hash_friend_password("secret-lea").unwrap();
        let body = FriendBody { name: Some("Léa".into()), login: Some("Lea".into()), slots: Some(vec![slot(&[3], "14:00", "18:00")]), ..Default::default() };
        let lea = friends.create(&body, hash).unwrap();
        assert_eq!(lea.login, "lea");
        assert!(friends.create(&body, "x".into()).is_err(), "identifiant déjà pris");
        assert!(friends.create(&FriendBody { name: Some("X".into()), login: Some("a b".into()), ..Default::default() }, "x".into()).is_err());
        assert!(hash_friend_password("court").is_err());
        let again = Friends::load(&dir);
        let read = again.by_login("LEA").unwrap();
        assert!(verify_friend_password(read.hash(), "secret-lea"));
        assert!(!verify_friend_password(read.hash(), "autre"));
        assert!(read.json().get("hash").is_none());
        let changed = again.update(&lea.id, &FriendBody { steam: Some(Steam::Host), ..Default::default() }, None).unwrap().unwrap();
        assert_eq!(changed.steam, Steam::Host);
        assert!(verify_friend_password(changed.hash(), "secret-lea"), "mot de passe inchangé");
        assert!(again.delete(&lea.id).unwrap().is_some());
        assert!(Friends::load(&dir).all().is_empty());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn un_code_de_seance_sert_une_fois() {
        let lending = Lending::new();
        let code = lending.create_code(Some(7200), Steam::Friend, false, |c| c == "000000");
        assert!(lending.is_lend_code(&code.code));
        assert!(lending.take_code("12").is_none());
        let found = lending.take_code(&code.code).unwrap();
        let mut w = who("Léa", "ipad");
        w.code = Some(found.code.clone());
        let loan = lending.begin(w, Some(now_secs() + 7200), found.steam, None);
        assert!(lending.code().is_none(), "consommé");
        assert!(lending.take_code(&code.code).is_none());
        // Le même code ramène au même prêt pendant 15 min.
        let mut back = who("Léa", "autre-appareil");
        back.code = Some(code.code.clone());
        let resumed = lending.resume(&back).unwrap();
        assert_eq!(resumed.id, loan.id);
        assert_ne!(resumed.secret(), loan.secret());
        assert!(lending.is_lend_code(&code.code));
    }

    #[test]
    fn demande_acceptee_puis_pret() {
        let lending = Lending::new();
        let request = lending.ask(who("Léa", "ipad"), Some(3600), None, Steam::Friend, None);
        assert!(matches!(lending.wait(&request.id, request.secret()), Wait::Pending));
        assert_eq!(lending.view(true)["requests"].as_array().unwrap().len(), 1);
        lending.answer(&request.id, true, Some(Steam::Host)).unwrap();
        let loan = lending.begin_from_request(&request.id, None).unwrap();
        assert_eq!(loan.steam, Steam::Host);
        assert!(loan.until.is_some_and(|u| u >= now_secs() + 3599));
        assert!(matches!(lending.wait(&request.id, request.secret()), Wait::Pending), "le mode console se prépare");
        lending.console_ready(Some(&loan.id)).unwrap();
        match lending.wait(&request.id, request.secret()) {
            Wait::Lent(l) => assert_eq!(l.id, loan.id),
            other => panic!("{other:?}"),
        }
        assert!(matches!(lending.wait(&request.id, "faux"), Wait::Pending | Wait::Unknown));
        assert_eq!(lending.view(true)["state"], "lent");
        assert_eq!(lending.view(true)["borrower"]["name"], "Léa");
    }

    #[test]
    fn demande_refusee_ou_sans_reponse() {
        let lending = Lending::new();
        let r = lending.ask(who("Léa", "ipad"), None, None, Steam::Friend, None);
        lending.answer(&r.id, false, None).unwrap();
        assert!(matches!(lending.wait(&r.id, r.secret()), Wait::Refused));
        let r = lending.ask(who("Max", "tv"), None, None, Steam::Friend, None);
        lending.age_request(&r.id, REQUEST_TTL + Duration::from_secs(1));
        let tick = lending.tick(Instant::now(), now_secs());
        assert_eq!(tick.expired, vec!["Max".to_string()]);
        assert!(matches!(lending.wait(&r.id, r.secret()), Wait::Expired));
        // Un créneau : accordée d'office au bout du préavis.
        let r = lending.ask(who("Léa", "ipad"), None, None, Steam::Friend, Some(Instant::now()));
        let tick = lending.tick(Instant::now(), now_secs());
        assert_eq!(tick.auto_accept, vec![r.id.clone()]);
        assert!(lending.begin_from_request(&r.id, None).is_some());
    }

    #[test]
    fn preavis_puis_fin() {
        let lending = Lending::new();
        let now = now_secs();
        let loan = lending.begin(who("Léa", "ipad"), Some(now + 600), Steam::Friend, None);
        lending.console_ready(Some(&loan.id));
        assert!(lending.tick(Instant::now(), now).end.is_none());
        // Deux minutes avant la fin : le préavis.
        let tick = lending.tick(Instant::now(), now + 600 - 100);
        assert!(tick.changed);
        assert_eq!(lending.loan().unwrap().notice.unwrap().reason, Reason::TimeUp);
        // À la fin : le prêt s'arrête.
        assert_eq!(lending.tick(Instant::now(), now + 600).end, Some((loan.id.clone(), Reason::TimeUp)));
        // La reprise par l'hôte : un préavis plus proche remplace l'autre.
        lending.notice(Reason::TakenBack, now + 120).unwrap();
        assert_eq!(lending.loan().unwrap().notice.unwrap().reason, Reason::TakenBack);
        lending.ending(Some(&loan.id)).unwrap();
        assert!(lending.ending(Some(&loan.id)).is_none(), "une seule remise en état");
        lending.ended(&loan.id, Reason::TakenBack).unwrap();
        assert!(matches!(lending.beat(&loan.id, loan.secret()), Beat::Ended(Reason::TakenBack)));
        assert!(matches!(lending.beat(&loan.id, "faux"), Beat::Unknown));
        assert_eq!(lending.view(true)["state"], "free");
    }

    #[test]
    fn l_ami_peut_revenir_quinze_minutes() {
        let lending = Lending::new();
        let loan = lending.begin(who("Léa", "ipad"), None, Steam::Friend, None);
        lending.console_ready(Some(&loan.id));
        lending.age_loan_beat(Duration::from_secs(60));
        let tick = lending.tick(Instant::now(), now_secs());
        assert!(tick.end.is_none());
        assert!(!lending.loan().unwrap().connected, "son image s'arrête");
        assert!(lending.resume(&who("Léa", "ipad")).is_some());
        lending.age_loan_beat(COMEBACK);
        assert_eq!(lending.tick(Instant::now(), now_secs()).end, Some((loan.id, Reason::Stopped)));
    }

    #[test]
    fn essais_limites() {
        let limits = Limits::default();
        let ip: IpAddr = "203.0.113.4".parse().unwrap();
        let t0 = Instant::now();
        for _ in 0..TRIES_PER_MINUTE {
            limits.count_try(ip, t0).unwrap();
        }
        assert!(limits.count_try(ip, t0).is_err());
        assert!(limits.count_try(ip, t0 + Duration::from_secs(61)).is_ok());
        for _ in 0..FRIEND_LOCK_AFTER {
            limits.account_failed("lea", t0);
        }
        assert!(limits.account_locked("lea", t0).is_some());
        assert!(limits.account_locked("max", t0).is_none());
        assert!(limits.account_locked("lea", t0 + FRIEND_LOCK_FOR).is_none());
    }

    #[test]
    fn cookie_du_navigateur() {
        let lending = Lending::new();
        let mut w = who("Léa", "chrome");
        w.app = "web".into();
        w.browser = true;
        let loan = lending.begin(w, None, Steam::Friend, None);
        let cookie = loan.cookie().unwrap().to_string();
        assert!(lending.cookie_valid(&cookie));
        assert!(set_cookie(&cookie, true, true).starts_with("__Host-kybernow_lend="));
        lending.ending(None);
        assert!(!lending.cookie_valid(&cookie));
    }
}
