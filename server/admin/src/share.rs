//! KyberNow Share, le partage en local (docs/kybernow-share.md) : un partage
//! ouvert, son code à six chiffres, l'écran montré et les invités (rôle,
//! prénom, appareil, dernier signe de vie).
//!
//! L'hôte reste la session d'aujourd'hui (`activity::Live`) : préparation,
//! écran virtuel, reprise de main. Un invité ne passe jamais par `prepare` ni
//! par la reprise de main : il rejoint avec le code (`join`), donne signe de
//! vie toutes les 5 s (`beat`) et part (`leave`). Ses droits sur Kyber
//! viennent d'un jeton court (120 s) qu'Admin signe à chaque signe de vie,
//! avec son rôle et son écran (correctif Kyber 0019).
//!
//! Deux origines : la session d'un appareil (l'hôte partage son écran
//! principal), ou le PC lui-même (l'icône près de l'horloge ou l'accueil
//! d'Admin, sans session, sur un écran physique choisi). Un seul partage à la
//! fois, quatre invités au plus.
//!
//! Ce module ne parle ni à Kyber ni au réseau : il rend ce qu'il faut faire
//! (invités à couper, rôle à pousser), `share_api.rs` le fait.

use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use rand::Rng;
use serde::{Deserialize, Serialize};

use crate::display::now_secs;

/// Invités au plus dans un partage.
pub const MAX_GUESTS: usize = 4;
/// Signe de vie d'un invité, en secondes (`beat` de la réponse à `join`).
pub const BEAT_SECONDS: u64 = 5;
/// Durée de vie du jeton de Kyber d'un invité, renouvelé à chaque signe de vie.
pub const TOKEN_LIFETIME: i64 = 120;
/// Sans signe de vie pendant ce temps, l'invité sort (même délai que l'hôte).
pub const GUEST_GRACE: Duration = crate::activity::HEARTBEAT_GRACE;
/// Essais de `join` par adresse et par minute, comme l'appairage.
const JOIN_TRIES_PER_MINUTE: usize = 10;
/// Un invité retiré ou un partage arrêté : la raison reste lisible ce temps-là
/// au signe de vie suivant de l'invité.
const ENDED_KEEP: Duration = Duration::from_secs(60);
/// Prénom gardé, en caractères.
const MAX_NAME: usize = 40;
const MAX_DEVICE: usize = 80;
/// Apps connues ; une autre valeur devient `other`.
const APPS: &[&str] = &["mac", "windows", "linux", "ios", "tvos", "android", "webos", "web"];
/// Cookie d'un invité du navigateur (client web) : ouvre le relais vers Kyber
/// et `/api/share/*`, rien d'autre.
pub const GUEST_COOKIE: &str = "kybernow_guest";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    /// Voit et entend, n'envoie rien (aucun droit).
    Spectator,
    /// Les manettes : elles deviennent la manette suivante de Windows.
    Player,
    /// Le clavier et la souris, sans les manettes.
    Kbm,
    /// Les deux droits : manettes, clavier et souris.
    PlayerKbm,
}

impl Role {
    pub fn as_str(self) -> &'static str {
        match self {
            Role::Spectator => "spectator",
            Role::Player => "player",
            Role::Kbm => "kbm",
            Role::PlayerKbm => "player_kbm",
        }
    }

    /// Le rôle de ces deux droits, donnés ou repris l'un sans l'autre.
    pub fn from_rights(gamepads: bool, keyboard_mouse: bool) -> Role {
        match (gamepads, keyboard_mouse) {
            (false, false) => Role::Spectator,
            (true, false) => Role::Player,
            (false, true) => Role::Kbm,
            (true, true) => Role::PlayerKbm,
        }
    }

    /// Les manettes sont données.
    pub fn gamepads(self) -> bool {
        matches!(self, Role::Player | Role::PlayerKbm)
    }

    /// Le clavier et la souris sont donnés.
    pub fn keyboard_mouse(self) -> bool {
        matches!(self, Role::Kbm | Role::PlayerKbm)
    }

    /// Un droit au moins : l'invité « joue ».
    pub fn plays(self) -> bool {
        self != Role::Spectator
    }

    /// `next` donne un droit que `self` n'avait pas.
    pub fn gains_in(self, next: Role) -> bool {
        (next.gamepads() && !self.gamepads()) || (next.keyboard_mouse() && !self.keyboard_mouse())
    }
}

/// L'écran montré aux invités.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ShareDisplay {
    /// Identifiant de Kyber (CRC-32 du nom GDI, comme txproto).
    pub id: u32,
    /// Rang dans la liste des écrans du PC (« Écran 1 »), 1 pour l'écran
    /// principal de l'hôte.
    pub index: u32,
    pub width: u32,
    pub height: u32,
    pub refresh: u32,
    /// Nom GDI (`\\.\DISPLAY1`), vide s'il est inconnu.
    #[serde(skip)]
    pub name: String,
}

/// D'où vient le partage.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Origin {
    /// La session d'un appareil : l'hôte.
    Host { device_id: String, user: String, name: String },
    /// Le PC lui-même (icône près de l'horloge, accueil d'Admin).
    Local,
}

impl Origin {
    pub fn kind(&self) -> &'static str {
        match self {
            Origin::Host { .. } => "host",
            Origin::Local => "local",
        }
    }
}

#[derive(Debug, Clone)]
pub struct Guest {
    pub id: String,
    /// Jeton de l'invité (`x-kybernow-guest`), comparé à temps constant.
    secret: String,
    /// Cookie d'un invité du navigateur.
    cookie: Option<String>,
    pub name: String,
    pub device: String,
    pub app: String,
    pub role: Role,
    /// Monte à chaque changement de rôle (`kn_rev` du jeton de Kyber) : un
    /// vieux jeton encore valide ne rend jamais un droit retiré.
    pub rev: u64,
    pub since: u64,
    pub last_beat_secs: u64,
    last_beat: Instant,
}

impl Guest {
    /// Le sujet des jetons de Kyber de cet invité.
    pub fn kyber_user(&self) -> String {
        kyber_user(&self.id)
    }

    pub fn secret(&self) -> &str {
        &self.secret
    }

    pub fn cookie(&self) -> Option<&str> {
        self.cookie.as_deref()
    }
}

/// Le sujet des jetons de Kyber d'un invité.
pub fn kyber_user(guest_id: &str) -> String {
    format!("kn-guest-{guest_id}")
}

/// Pourquoi un invité est sorti, tel que son signe de vie suivant l'apprend.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum End {
    /// L'hôte (ou le PC) a arrêté le partage.
    Stopped,
    /// L'hôte l'a retiré.
    Removed,
}

struct Ended {
    secret: String,
    end: End,
    role: Role,
    at: Instant,
}

/// Le partage ouvert, tel que les routes l'affichent.
#[derive(Debug, Clone)]
pub struct Snapshot {
    pub code: String,
    pub origin: Origin,
    pub display: ShareDisplay,
    pub opened_at: u64,
    pub guests: Vec<Guest>,
}

struct Open {
    code: String,
    origin: Origin,
    display: ShareDisplay,
    opened_at: u64,
    guests: Vec<Guest>,
}

impl Open {
    fn snapshot(&self) -> Snapshot {
        Snapshot {
            code: self.code.clone(),
            origin: self.origin.clone(),
            display: self.display.clone(),
            opened_at: self.opened_at,
            guests: self.guests.clone(),
        }
    }
}

#[derive(Default)]
struct State {
    open: Option<Open>,
    /// Invités sortis sur décision de l'hôte, par identifiant.
    ended: HashMap<String, Ended>,
    /// Heures des derniers essais de `join`, par adresse.
    tries: HashMap<IpAddr, Vec<Instant>>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum OpenError {
    /// Un autre partage est ouvert (l'autre origine).
    Other,
}

#[derive(Debug, PartialEq, Eq)]
pub enum JoinError {
    /// Code ou prénom invalide.
    BadRequest,
    /// Aucun partage avec ce code.
    NoShare,
    /// Quatre invités déjà.
    Full,
    /// Trop d'essais depuis cette adresse : attendre ce nombre de secondes.
    TooMany(u64),
}

/// Ce qu'un invité reçoit en rejoignant.
#[derive(Debug, Clone)]
pub struct Joined {
    pub guest: Guest,
    pub snapshot: Snapshot,
    /// Faux si le même appareil était déjà là (il revient après une coupure).
    pub new: bool,
}

/// Réponse au signe de vie d'un invité.
#[derive(Debug, Clone)]
pub enum Beat {
    Live { guest: Guest, display: ShareDisplay },
    Ended { end: End, role: Role },
}

/// Ce que devient un invité qui part de lui-même (`leave`).
#[derive(Debug)]
pub enum Leave {
    /// Il était dans le partage : à couper de Kyber.
    Left(Guest),
    /// Déjà sorti (retiré, partage arrêté) depuis moins d'une minute : sa
    /// raison est oubliée.
    Ended,
    /// Inconnu : jamais venu, parti, sorti sans nouvelles ou depuis plus
    /// d'une minute, ou mauvais secret.
    Unknown,
}

/// Un changement de rôle : l'invité tel qu'il est maintenant, et son rôle d'avant.
#[derive(Debug, Clone)]
pub struct RoleChange {
    pub guest: Guest,
    pub before: Role,
}

#[derive(Default)]
pub struct Share {
    state: Mutex<State>,
}

fn random_hex(bytes: usize) -> String {
    let mut buf = vec![0u8; bytes];
    rand::rng().fill(&mut buf[..]);
    hex::encode(buf)
}

fn new_code() -> String {
    format!("{:06}", rand::rng().random_range(0..1_000_000u32))
}

/// Comparaison à temps constant.
fn same(a: &str, b: &str) -> bool {
    let (a, b) = (a.as_bytes(), b.as_bytes());
    !b.is_empty() && a.len() == b.len() && a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

fn clean(text: &str, max: usize) -> String {
    text.trim().chars().filter(|c| !c.is_control()).take(max).collect::<String>().trim().to_string()
}

impl Share {
    pub fn new() -> Self {
        Self::default()
    }

    /// Le partage ouvert, s'il y en a un.
    pub fn snapshot(&self) -> Option<Snapshot> {
        self.state.lock().unwrap().open.as_ref().map(Open::snapshot)
    }

    pub fn is_open(&self) -> bool {
        self.state.lock().unwrap().open.is_some()
    }

    /// Un partage lancé depuis le PC est ouvert : pas d'écran virtuel
    /// automatique, pas de changement de la sortie son.
    pub fn local_open(&self) -> bool {
        self.state.lock().unwrap().open.as_ref().is_some_and(|o| o.origin == Origin::Local)
    }

    /// Les invités du partage ouvert.
    pub fn guests(&self) -> Vec<Guest> {
        self.state.lock().unwrap().open.as_ref().map(|o| o.guests.clone()).unwrap_or_default()
    }

    pub fn guest_count(&self) -> usize {
        self.state.lock().unwrap().open.as_ref().map_or(0, |o| o.guests.len())
    }

    /// Ouvre un partage, ou rend celui de la même origine déjà ouvert (son
    /// écran est alors celui donné). Faux en second : déjà ouvert.
    pub fn open(&self, origin: Origin, display: ShareDisplay) -> Result<(Snapshot, bool), OpenError> {
        // Ce qui est rendu est lu sous le même verrou : un « Arrêter » qui
        // passerait entre les deux ne fait pas rendre un partage fermé.
        let mut s = self.state.lock().unwrap();
        match s.open.as_mut() {
            Some(o) if o.origin == origin => {
                // Des invités déjà là regardent l'écran du partage : il
                // ne change plus sous eux.
                if o.guests.is_empty() {
                    o.display = display;
                }
                Ok((o.snapshot(), false))
            }
            Some(_) => Err(OpenError::Other),
            None => {
                let open = Open { code: new_code(), origin, display, opened_at: now_secs(), guests: Vec::new() };
                let snapshot = open.snapshot();
                s.open = Some(open);
                Ok((snapshot, true))
            }
        }
    }

    /// L'écran de l'hôte a changé (son écran virtuel recréé sous un autre
    /// nom) : le partage le suit, les jetons suivants des invités le portent.
    /// Vrai s'il a changé.
    pub fn set_display(&self, display: ShareDisplay) -> bool {
        let mut s = self.state.lock().unwrap();
        match s.open.as_mut() {
            Some(o) if o.display.id != display.id => {
                o.display = display;
                true
            }
            Some(o) => {
                o.display = display;
                false
            }
            None => false,
        }
    }

    /// Arrête le partage : rend les invités à couper (leur signe de vie
    /// suivant apprend « arrêté ») et ce qu'était le partage.
    pub fn close(&self) -> Option<Snapshot> {
        // Les invités rendus (à couper de Kyber) sont exactement ceux que le
        // partage avait en se fermant : un invité qui rejoint en même temps
        // est l'un d'eux, ou trouve le partage fermé.
        let mut s = self.state.lock().unwrap();
        let open = s.open.take()?;
        let snapshot = open.snapshot();
        let now = Instant::now();
        for g in open.guests {
            s.ended.insert(g.id.clone(), Ended { secret: g.secret, end: End::Stopped, role: g.role, at: now });
        }
        Some(snapshot)
    }

    /// Nouveau code ; les invités déjà là restent.
    pub fn renew_code(&self) -> Option<Snapshot> {
        let mut s = self.state.lock().unwrap();
        let open = s.open.as_mut()?;
        let mut code = new_code();
        while code == open.code {
            code = new_code();
        }
        open.code = code;
        Some(open.snapshot())
    }

    /// Change le rôle d'un invité. `None` : invité inconnu.
    pub fn set_role(&self, id: &str, role: Role) -> Option<RoleChange> {
        let mut s = self.state.lock().unwrap();
        let guest = s.open.as_mut()?.guests.iter_mut().find(|g| g.id == id)?;
        let before = guest.role;
        if before != role {
            guest.role = role;
            guest.rev += 1;
        }
        Some(RoleChange { guest: guest.clone(), before })
    }

    /// Retire un invité (son signe de vie suivant apprend « retiré »).
    pub fn remove(&self, id: &str) -> Option<Guest> {
        let mut s = self.state.lock().unwrap();
        let open = s.open.as_mut()?;
        let index = open.guests.iter().position(|g| g.id == id)?;
        let guest = open.guests.remove(index);
        s.ended.insert(
            guest.id.clone(),
            Ended { secret: guest.secret.clone(), end: End::Removed, role: guest.role, at: Instant::now() },
        );
        Some(guest)
    }

    /// Un essai de `join` depuis `peer` : compté (comme l'appairage) avant
    /// toute vérification du code.
    fn count_try(s: &mut State, peer: IpAddr, now: Instant) -> Result<(), JoinError> {
        s.tries.retain(|_, times| {
            times.retain(|t| now.duration_since(*t) < Duration::from_secs(60));
            !times.is_empty()
        });
        let times = s.tries.entry(peer.to_canonical()).or_default();
        if times.len() >= JOIN_TRIES_PER_MINUTE {
            let oldest = times.iter().min().copied().unwrap_or(now);
            let wait = Duration::from_secs(60).saturating_sub(now.duration_since(oldest)).as_secs().max(1);
            return Err(JoinError::TooMany(wait));
        }
        times.push(now);
        Ok(())
    }

    /// Un invité rejoint avec le code. Le même appareil déjà là reprend sa
    /// place (même identifiant, même rôle, nouveau secret).
    pub fn join(&self, code: &str, name: &str, device: &str, app: &str, peer: IpAddr, browser: bool) -> Result<Joined, JoinError> {
        let now = Instant::now();
        let mut s = self.state.lock().unwrap();
        Self::count_try(&mut s, peer, now)?;
        let code: String = code.chars().filter(|c| !c.is_whitespace()).collect();
        let name = clean(name, MAX_NAME);
        if code.len() != 6 || !code.bytes().all(|b| b.is_ascii_digit()) || name.is_empty() {
            return Err(JoinError::BadRequest);
        }
        let device = clean(device, MAX_DEVICE);
        let app = if APPS.contains(&app) { app.to_string() } else { "other".to_string() };
        let open = match s.open.as_mut() {
            Some(o) if same(&code, &o.code) => o,
            _ => return Err(JoinError::NoShare),
        };
        let secret = random_hex(32);
        let cookie = browser.then(|| random_hex(32));
        let existing = (!device.is_empty()).then(|| open.guests.iter_mut().find(|g| g.device == device)).flatten();
        let (guest, new) = match existing {
            Some(g) => {
                g.secret = secret;
                g.cookie = cookie;
                g.name = name;
                g.app = app;
                g.last_beat = now;
                g.last_beat_secs = now_secs();
                (g.clone(), false)
            }
            None => {
                if open.guests.len() >= MAX_GUESTS {
                    return Err(JoinError::Full);
                }
                let g = Guest {
                    id: random_hex(8),
                    secret,
                    cookie,
                    name,
                    device,
                    app,
                    role: Role::Spectator,
                    rev: 1,
                    since: now_secs(),
                    last_beat_secs: now_secs(),
                    last_beat: now,
                };
                open.guests.push(g.clone());
                (g, true)
            }
        };
        // Lu sous le verrou : le partage ne se ferme pas sous l'invité.
        Ok(Joined { guest, snapshot: open.snapshot(), new })
    }

    /// Signe de vie d'un invité. `None` : inconnu (jamais venu, parti, ou
    /// sorti depuis longtemps).
    pub fn beat(&self, id: &str, secret: &str) -> Option<Beat> {
        let mut s = self.state.lock().unwrap();
        let now = Instant::now();
        s.ended.retain(|_, e| now.duration_since(e.at) < ENDED_KEEP);
        if let Some(open) = s.open.as_mut() {
            let display = open.display.clone();
            if let Some(g) = open.guests.iter_mut().find(|g| g.id == id && same(secret, &g.secret)) {
                g.last_beat = now;
                g.last_beat_secs = now_secs();
                return Some(Beat::Live { guest: g.clone(), display });
            }
        }
        let ended = s.ended.get(id).filter(|e| same(secret, &e.secret))?;
        Some(Beat::Ended { end: ended.end, role: ended.role })
    }

    /// L'invité part de lui-même.
    pub fn leave(&self, id: &str, secret: &str) -> Leave {
        let mut s = self.state.lock().unwrap();
        let now = Instant::now();
        s.ended.retain(|_, e| now.duration_since(e.at) < ENDED_KEEP);
        if s.ended.get(id).is_some_and(|e| same(secret, &e.secret)) {
            s.ended.remove(id);
            return Leave::Ended;
        }
        let Some(open) = s.open.as_mut() else { return Leave::Unknown };
        match open.guests.iter().position(|g| g.id == id && same(secret, &g.secret)) {
            Some(index) => Leave::Left(open.guests.remove(index)),
            None => Leave::Unknown,
        }
    }

    /// Les invités sans signe de vie depuis `GUEST_GRACE` : sortis, rendus
    /// pour être coupés de Kyber.
    pub fn expire(&self) -> Vec<Guest> {
        self.expire_at(Instant::now())
    }

    fn expire_at(&self, now: Instant) -> Vec<Guest> {
        let mut s = self.state.lock().unwrap();
        s.ended.retain(|_, e| now.duration_since(e.at) < ENDED_KEEP);
        let Some(open) = s.open.as_mut() else { return Vec::new() };
        let (gone, kept): (Vec<Guest>, Vec<Guest>) =
            open.guests.drain(..).partition(|g| now.saturating_duration_since(g.last_beat) > GUEST_GRACE);
        open.guests = kept;
        gone
    }

    /// Le cookie d'un invité du navigateur encore dans le partage.
    pub fn cookie_valid(&self, cookie: &str) -> bool {
        let s = self.state.lock().unwrap();
        s.open.as_ref().is_some_and(|o| o.guests.iter().any(|g| g.cookie.as_deref().is_some_and(|c| same(cookie, c))))
    }

    /// Le partage est-il celui de cet appareil hôte ?
    pub fn hosted_by(&self, device_id: &str) -> bool {
        let s = self.state.lock().unwrap();
        matches!(s.open.as_ref().map(|o| &o.origin), Some(Origin::Host { device_id: d, .. }) if d == device_id)
    }

    #[cfg(test)]
    fn age_guest(&self, id: &str, by: Duration) {
        let mut s = self.state.lock().unwrap();
        if let Some(g) = s.open.as_mut().and_then(|o| o.guests.iter_mut().find(|g| g.id == id)) {
            g.last_beat -= by;
        }
    }
}

/// Le cookie d'invité d'une requête, s'il y en a un.
pub fn guest_cookie(headers: &axum::http::HeaderMap) -> Option<String> {
    headers
        .get_all(axum::http::header::COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .flat_map(|v| v.split(';'))
        .filter_map(|c| c.trim().split_once('='))
        .find(|(k, _)| *k == GUEST_COOKIE)
        .map(|(_, v)| v.to_string())
}

/// En-tête `Set-Cookie` d'un invité du navigateur (12 h au plus ; il ne vaut
/// que tant que l'invité est dans le partage).
pub fn set_cookie(value: &str, secure: bool) -> String {
    format!("{GUEST_COOKIE}={value}; Path=/; HttpOnly; SameSite=Strict; Max-Age=43200{}", if secure { "; Secure" } else { "" })
}

pub fn clear_cookie(secure: bool) -> String {
    format!("{GUEST_COOKIE}=; Path=/; HttpOnly; SameSite=Strict; Max-Age=0{}", if secure { "; Secure" } else { "" })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn display(id: u32) -> ShareDisplay {
        ShareDisplay { id, index: 1, width: 1920, height: 1080, refresh: 60, name: "\\\\.\\DISPLAY1".into() }
    }

    fn peer(n: u8) -> IpAddr {
        IpAddr::from([192, 168, 1, n])
    }

    fn host() -> Origin {
        Origin::Host { device_id: "mac".into(), user: "matthieu".into(), name: "MacBook".into() }
    }

    #[test]
    fn un_partage_a_la_fois() {
        let share = Share::new();
        let (first, fresh) = share.open(Origin::Local, display(1)).unwrap();
        assert!(fresh && share.local_open());
        assert_eq!(first.code.len(), 6);
        // La même origine : le même partage, le même code.
        let (again, fresh) = share.open(Origin::Local, display(2)).unwrap();
        assert!(!fresh);
        assert_eq!((again.code.as_str(), again.display.id), (first.code.as_str(), 2));
        assert_eq!(share.open(host(), display(3)).unwrap_err(), OpenError::Other);
        assert!(share.close().is_some());
        assert!(!share.is_open());
        assert!(share.open(host(), display(3)).is_ok());
        assert!(share.hosted_by("mac") && !share.hosted_by("tv") && !share.local_open());
    }

    #[test]
    fn rejoindre_avec_le_code() {
        let share = Share::new();
        let code = share.open(Origin::Local, display(7)).unwrap().0.code;
        assert_eq!(share.join("000000x", "Léa", "ipad", "ios", peer(1), false).unwrap_err(), JoinError::BadRequest);
        assert_eq!(share.join(&code, "  ", "ipad", "ios", peer(1), false).unwrap_err(), JoinError::BadRequest);
        let wrong = if code == "111111" { "222222" } else { "111111" };
        assert_eq!(share.join(wrong, "Léa", "ipad", "ios", peer(1), false).unwrap_err(), JoinError::NoShare);
        // « 123 456 » tapé avec l'espace marche aussi.
        let spaced = format!("{} {}", &code[..3], &code[3..]);
        let lea = share.join(&spaced, " Léa\u{7} ", "ipad", "ios", peer(1), false).unwrap();
        assert!(lea.new);
        assert_eq!((lea.guest.name.as_str(), lea.guest.role, lea.guest.app.as_str()), ("Léa", Role::Spectator, "ios"));
        assert_eq!(lea.snapshot.display.id, 7);
        assert!(lea.guest.cookie().is_none());
        // Le même appareil revient : même place, même rôle, nouveau secret.
        share.set_role(&lea.guest.id, Role::Player).unwrap();
        let back = share.join(&code, "Léa", "ipad", "ios", peer(1), false).unwrap();
        assert!(!back.new);
        assert_eq!((back.guest.id.as_str(), back.guest.role), (lea.guest.id.as_str(), Role::Player));
        assert_ne!(back.guest.secret(), lea.guest.secret());
        assert!(share.beat(&lea.guest.id, lea.guest.secret()).is_none(), "l'ancien secret ne vaut plus");
        assert_eq!(share.guest_count(), 1);
        // Une app inconnue devient « other ».
        assert_eq!(share.join(&code, "Max", "tv", "amiga", peer(2), false).unwrap().guest.app, "other");
    }

    #[test]
    fn quatre_invites_au_plus() {
        let share = Share::new();
        let code = share.open(Origin::Local, display(1)).unwrap().0.code;
        for i in 0..4 {
            share.join(&code, "Invité", &format!("app-{i}"), "mac", peer(i), false).unwrap();
        }
        assert_eq!(share.join(&code, "Cinquième", "app-5", "mac", peer(9), false).unwrap_err(), JoinError::Full);
        // Un appareil déjà là revient toujours.
        assert!(share.join(&code, "Invité", "app-0", "mac", peer(0), false).is_ok());
    }

    #[test]
    fn dix_essais_par_minute_et_par_adresse() {
        let share = Share::new();
        let code = share.open(Origin::Local, display(1)).unwrap().0.code;
        let wrong = if code == "999999" { "888888" } else { "999999" };
        for _ in 0..10 {
            assert_eq!(share.join(wrong, "x", "d", "mac", peer(3), false).unwrap_err(), JoinError::NoShare);
        }
        // Le bon code ne passe plus non plus : on ne devine pas en insistant.
        assert!(matches!(share.join(&code, "x", "d", "mac", peer(3), false), Err(JoinError::TooMany(s)) if s > 0 && s <= 60));
        // Une autre adresse n'est pas bloquée ; IPv4 projetée en IPv6 compte comme la même.
        assert!(share.join(&code, "x", "d", "mac", peer(4), false).is_ok());
        let mapped: IpAddr = "::ffff:192.168.1.3".parse().unwrap();
        assert!(matches!(share.join(&code, "x", "d", "mac", mapped, false), Err(JoinError::TooMany(_))));
    }

    #[test]
    fn roles_et_numero_de_revision() {
        let share = Share::new();
        let code = share.open(host(), display(1)).unwrap().0.code;
        let g = share.join(&code, "Léa", "ipad", "ios", peer(1), false).unwrap().guest;
        assert_eq!(g.rev, 1);
        let change = share.set_role(&g.id, Role::Player).unwrap();
        assert_eq!((change.before, change.guest.role, change.guest.rev), (Role::Spectator, Role::Player, 2));
        // Le même rôle : rien ne monte.
        assert_eq!(share.set_role(&g.id, Role::Player).unwrap().guest.rev, 2);
        assert_eq!(share.set_role(&g.id, Role::PlayerKbm).unwrap().guest.rev, 3);
        assert!(share.set_role("inconnu", Role::Player).is_none());
        match share.beat(&g.id, g.secret()).unwrap() {
            Beat::Live { guest, display } => assert_eq!((guest.role, guest.rev, display.id), (Role::PlayerKbm, 3, 1)),
            other => panic!("{other:?}"),
        }
        assert!(share.beat(&g.id, "faux").is_none());
    }

    #[test]
    fn retire_ou_arrete_le_signe_de_vie_suivant_le_dit() {
        let share = Share::new();
        let code = share.open(Origin::Local, display(1)).unwrap().0.code;
        let lea = share.join(&code, "Léa", "ipad", "ios", peer(1), false).unwrap().guest;
        let max = share.join(&code, "Max", "tv", "tvos", peer(2), false).unwrap().guest;
        assert_eq!(share.remove(&lea.id).unwrap().name, "Léa");
        assert!(matches!(share.beat(&lea.id, lea.secret()), Some(Beat::Ended { end: End::Removed, .. })));
        let closed = share.close().unwrap();
        assert_eq!(closed.guests.len(), 1);
        assert!(matches!(share.beat(&max.id, max.secret()), Some(Beat::Ended { end: End::Stopped, .. })));
        assert!(share.beat(&max.id, "faux").is_none());
        // Après la fin, `leave` oublie la raison ; ensuite, l'invité est inconnu.
        assert!(matches!(share.leave(&max.id, "faux"), Leave::Unknown));
        assert!(matches!(share.leave(&max.id, max.secret()), Leave::Ended));
        assert!(share.beat(&max.id, max.secret()).is_none());
        assert!(matches!(share.leave(&max.id, max.secret()), Leave::Unknown));
    }

    #[test]
    fn sans_signe_de_vie_l_invite_sort() {
        let share = Share::new();
        let code = share.open(Origin::Local, display(1)).unwrap().0.code;
        let lea = share.join(&code, "Léa", "ipad", "ios", peer(1), false).unwrap().guest;
        let max = share.join(&code, "Max", "tv", "tvos", peer(2), false).unwrap().guest;
        assert!(share.expire().is_empty());
        share.age_guest(&lea.id, GUEST_GRACE + Duration::from_secs(1));
        let gone = share.expire();
        assert_eq!(gone.iter().map(|g| g.name.as_str()).collect::<Vec<_>>(), ["Léa"]);
        assert!(share.beat(&lea.id, lea.secret()).is_none());
        assert!(share.beat(&max.id, max.secret()).is_some());
        assert!(matches!(share.leave(&max.id, max.secret()), Leave::Left(g) if g.name == "Max"));
        assert!(matches!(share.leave(&lea.id, lea.secret()), Leave::Unknown), "sortie sans nouvelles : inconnue");
        assert_eq!(share.guest_count(), 0);
    }

    #[test]
    fn deux_droits_independants() {
        for (gamepads, kbm, role, name) in [
            (false, false, Role::Spectator, "spectator"),
            (true, false, Role::Player, "player"),
            (false, true, Role::Kbm, "kbm"),
            (true, true, Role::PlayerKbm, "player_kbm"),
        ] {
            assert_eq!(Role::from_rights(gamepads, kbm), role);
            assert_eq!((role.gamepads(), role.keyboard_mouse(), role.as_str()), (gamepads, kbm, name));
            assert_eq!(serde_json::to_value(role).unwrap(), name);
            assert_eq!(serde_json::from_value::<Role>(serde_json::json!(name)).unwrap(), role);
        }
        // Donner l'un sans l'autre, puis le second : un droit gagné à chaque fois.
        assert!(Role::Spectator.gains_in(Role::Kbm));
        assert!(Role::Kbm.gains_in(Role::PlayerKbm));
        assert!(Role::Player.gains_in(Role::Kbm), "changer de droit en gagne un");
        // Reprendre : rien de gagné.
        assert!(!Role::PlayerKbm.gains_in(Role::Player));
        assert!(!Role::Kbm.gains_in(Role::Spectator));
    }

    #[test]
    fn l_ecran_de_l_hote_peut_changer_sous_les_invites() {
        let share = Share::new();
        let code = share.open(host(), display(1)).unwrap().0.code;
        let g = share.join(&code, "Léa", "ipad", "ios", peer(1), false).unwrap().guest;
        // `open` ne change pas l'écran sous des invités ; l'hôte qui se prépare de nouveau, si.
        share.open(host(), display(2)).unwrap();
        assert_eq!(share.snapshot().unwrap().display.id, 1);
        assert!(share.set_display(display(3)));
        assert!(!share.set_display(display(3)));
        assert!(matches!(share.beat(&g.id, g.secret()), Some(Beat::Live { display, .. }) if display.id == 3));
        share.close();
        assert!(!share.set_display(display(4)));
    }

    #[test]
    fn cookie_du_navigateur_et_nouveau_code() {
        let share = Share::new();
        let code = share.open(Origin::Local, display(1)).unwrap().0.code;
        let web = share.join(&code, "Léa", "chrome-1", "web", peer(1), true).unwrap().guest;
        let cookie = web.cookie().unwrap().to_string();
        assert!(share.cookie_valid(&cookie) && !share.cookie_valid("autre") && !share.cookie_valid(""));
        let renewed = share.renew_code().unwrap();
        assert_ne!(renewed.code, code);
        assert_eq!(renewed.guests.len(), 1, "les invités restent");
        assert_eq!(share.join(&code, "Max", "tv", "tvos", peer(2), false).unwrap_err(), JoinError::NoShare);
        share.remove(&web.id);
        assert!(!share.cookie_valid(&cookie));
        let mut headers = axum::http::HeaderMap::new();
        headers.insert(axum::http::header::COOKIE, format!("a=1; {GUEST_COOKIE}=abc").parse().unwrap());
        assert_eq!(guest_cookie(&headers).as_deref(), Some("abc"));
        assert!(set_cookie("abc", true).ends_with("; Secure"));
    }

    /// « Arrêter » pendant qu'un invité rejoint : l'invité accepté est dans ce
    /// que rend `close` (il est coupé de Kyber), ou refusé ; jamais entre les
    /// deux, et `join` ne tombe pas sur un partage fermé sous lui.
    #[test]
    fn arreter_pendant_qu_un_invite_rejoint() {
        let share = std::sync::Arc::new(Share::new());
        for round in 0..3000u32 {
            let code = share.open(Origin::Local, display(1)).unwrap().0.code;
            let barrier = std::sync::Arc::new(std::sync::Barrier::new(2));
            let joiner = {
                let (share, barrier) = (share.clone(), barrier.clone());
                std::thread::spawn(move || {
                    barrier.wait();
                    // Une adresse par tour : la limite d'essais ne s'en mêle pas.
                    let peer = IpAddr::from((round + 1).to_be_bytes());
                    share.join(&code, "Léa", "ipad", "ios", peer, false)
                })
            };
            barrier.wait();
            let closed = share.close().expect("ouvert");
            match joiner.join().expect("join ne panique pas") {
                Ok(joined) => {
                    assert!(closed.guests.iter().any(|g| g.id == joined.guest.id), "tour {round} : invité accepté mais pas coupé");
                    assert!(joined.snapshot.guests.iter().any(|g| g.id == joined.guest.id));
                }
                Err(e) => assert_eq!(e, JoinError::NoShare),
            }
        }
    }
}
