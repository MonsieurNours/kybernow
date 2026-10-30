//! KyberNow Share : prêter l'ordinateur (docs/pret.md). L'ami emprunte le PC
//! de l'hôte, qui passe sa session Windows en mode console : statut de
//! l'ordinateur ([`status`]), emprunt par un code ou un compte d'ami
//! ([`borrow`]), attente de l'accord de l'hôte ([`wait`], [`cancel`]),
//! départ ([`leave`]), lien d'invitation ([`parse_invite`]). La session
//! d'emprunt elle-même : [`crate::session::SessionParams::borrower`].
//! L'hôte : la vue du prêt ([`LendView`], `Console::lend_*`, et le champ
//! `lend` de son signe de vie). Hors de chez soi, tout passe par le relais
//! comme le partage ([`crate::relay`]).

use std::time::Duration;

use serde::{Deserialize, Deserializer, Serialize};

use crate::console::{Console, ConsoleError, Mode};
use crate::i18n::{self, Lang, tr};
use crate::relay::{RelayMode, RelayRoute};
use crate::share::{ShareDisplay, ShareKyber, ShareTarget, ShareVia};

/// Sans réponse d'Admin sur le réseau dans ce délai, le relais.
pub const DIRECT_PROBE: Duration = Duration::from_millis(1500);
/// Un emprunt revient (fenêtre fermée, réseau perdu) pendant ce temps par
/// [`borrow`], avec la même demande (même appareil, compte ou code).
pub const COME_BACK: Duration = Duration::from_secs(15 * 60);
/// `borrow` attend sa réponse ce temps au plus : la Station prépare le mode
/// console en 40 s au plus (Steam compris), plus le chemin (relais).
pub const BORROW_WAIT: Duration = Duration::from_secs(50);
/// `wait` : l'attente longue de la Station (30 s), puis, une fois l'hôte
/// d'accord, la préparation du mode console (40 s au plus).
pub const WAIT_WAIT: Duration = Duration::from_secs(80);

// MARK: - L'ordinateur d'un ami

/// L'état de la carte « Ordinateur de {hôte} » (`POST /api/lend/status`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LendAvailability {
    /// « Disponible » : « Emprunter ».
    Free,
    /// « Occupé » : « Demander à {hôte} ».
    Busy,
    /// « Prêté à quelqu'un d'autre ».
    Lent,
    /// « Verrouillé » : la session Windows de l'hôte est verrouillée.
    Locked,
    /// « Prêt coupé » (et tout état inconnu d'une Station plus récente).
    #[default]
    #[serde(other)]
    Off,
}

impl LendAvailability {
    /// L'état de la carte, dans la langue donnée (docs/link-ui.md).
    pub fn label(self, lang: Lang) -> String {
        let key = match self {
            Self::Free => "lend.state_free",
            Self::Busy => "lend.state_busy",
            Self::Lent => "lend.state_lent",
            Self::Locked => "lend.state_locked",
            Self::Off => "lend.state_off",
        };
        tr(lang, key, &[])
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Free => "free",
            Self::Busy => "busy",
            Self::Lent => "lent",
            Self::Locked => "locked",
            Self::Off => "off",
        }
    }

    /// L'inverse de [`Self::as_str`] (`None` : inconnu).
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "free" => Some(Self::Free),
            "busy" => Some(Self::Busy),
            "lent" => Some(Self::Lent),
            "locked" => Some(Self::Locked),
            "off" => Some(Self::Off),
            _ => None,
        }
    }
}

/// Une plage d'un créneau, en secondes Unix.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct LendPeriod {
    #[serde(default)]
    pub start: u64,
    #[serde(default)]
    pub end: u64,
}

/// Le créneau de l'ami : en cours (`now`), et le prochain.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct LendSlot {
    #[serde(default)]
    pub now: bool,
    #[serde(default)]
    pub next: Option<LendPeriod>,
}

/// Réponse de `POST /api/lend/status`.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct LendStatus {
    #[serde(default)]
    pub state: LendAvailability,
    /// Le prénom de l'hôte (« Ordinateur de {hôte} »).
    #[serde(default)]
    pub host: String,
    /// Le nom de l'ordinateur.
    #[serde(default)]
    pub computer: String,
    /// `None` : pas de créneau sur ce compte.
    #[serde(default)]
    pub slot: Option<LendSlot>,
    /// Le prêt en cours est celui de cet ami (`state` `lent`) : il peut y
    /// revenir (« Revenir ») au lieu de lire « Prêté à quelqu'un d'autre ».
    #[serde(default)]
    pub mine: bool,
}

// MARK: - Emprunter

/// Ce qui ouvre le prêt : un code pour une séance, ou un compte d'ami.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum LendCredentials {
    Code { code: String },
    Account { login: String, password: String },
}

impl std::fmt::Debug for LendCredentials {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Code { code } => f.debug_struct("Code").field("code", code).finish(),
            Self::Account { login, .. } => f.debug_struct("Account").field("login", login).field("password", &"[masqué]").finish(),
        }
    }
}

/// Corps de `POST /api/lend/borrow`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BorrowRequest {
    #[serde(flatten)]
    pub credentials: LendCredentials,
    /// Le prénom de l'ami (proposé : celui retenu pour le partage).
    pub name: String,
    /// L'identifiant de l'appareil (`DeviceInfo::id`) : le même revient au prêt.
    pub device: String,
    /// `mac`, `windows`, `linux`, `ios`, `tvos`, `android`, `webos`.
    pub app: String,
    /// L'écran de l'appareil : la Station fait l'écran virtuel à sa taille.
    pub screen: Mode,
    /// Les formats que l'appareil décode (`hevc`, `h264`, `av1`).
    #[serde(default)]
    pub codecs: Vec<String>,
}

/// Le compte Steam du prêt : celui de l'ami, ou celui de l'hôte.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LendSteam {
    /// « Votre compte Steam ».
    Host,
    /// « Son compte Steam » (proposé ; aussi toute valeur inconnue).
    #[default]
    #[serde(other)]
    Friend,
}

impl LendSteam {
    /// Pour les ponts : 0 son compte (l'ami), 1 celui de l'hôte.
    pub fn code(self) -> i32 {
        match self {
            Self::Friend => 0,
            Self::Host => 1,
        }
    }

    pub fn from_code(code: i32) -> Self {
        if code == 1 { Self::Host } else { Self::Friend }
    }
}

/// Réponse `200` de `borrow` (ou de `wait`) : le mode console est prêt.
/// À donner telle quelle à la session d'emprunt
/// ([`crate::session::SessionParams::borrower`]).
#[derive(Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Borrowed {
    /// L'identifiant du prêt (signe de vie, départ).
    #[serde(default)]
    pub lend: String,
    /// Le jeton de l'ami (`x-kybernow-lend`).
    #[serde(default)]
    pub secret: String,
    /// Le prénom de l'hôte (« Prêté par {hôte} »).
    #[serde(default)]
    pub host: String,
    /// Le nom de l'ordinateur.
    #[serde(default)]
    pub computer: String,
    /// Kyber : port, empreinte (obligatoire), jeton renouvelé par `beat`.
    #[serde(default)]
    pub kyber: ShareKyber,
    /// L'écran virtuel fait pour l'appareil.
    #[serde(default)]
    pub display: ShareDisplay,
    /// La fin prévue, en secondes Unix ; `None` : « Jusqu'à ce que {hôte} reprenne ».
    #[serde(default)]
    pub until: Option<u64>,
    #[serde(default)]
    pub steam: LendSteam,
    /// Le signe de vie, toutes les `beat` secondes.
    #[serde(default = "five")]
    pub beat: u64,
    /// Par le relais : la route, gardée pour la session (champ local, pas
    /// d'Admin).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub route: Option<RelayRoute>,
}

fn five() -> u64 {
    5
}

impl std::fmt::Debug for Borrowed {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Borrowed")
            .field("lend", &self.lend)
            .field("secret", &"[masqué]")
            .field("host", &self.host)
            .field("computer", &self.computer)
            .field("kyber", &self.kyber)
            .field("display", &self.display)
            .field("until", &self.until)
            .field("steam", &self.steam)
            .field("beat", &self.beat)
            .field("route", &self.route)
            .finish()
    }
}

impl Borrowed {
    /// {hôte} des textes du prêt : le prénom de l'hôte, sinon l'ordinateur.
    pub fn host_name(&self) -> &str {
        if self.host.trim().is_empty() { &self.computer } else { &self.host }
    }
}

/// Le prêt obtenu : la Station (son adresse, l'empreinte d'Admin épinglée)
/// et sa réponse.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Loan {
    pub target: ShareTarget,
    pub borrowed: Borrowed,
}

/// Une demande en attente de l'accord de l'hôte (« En attente de {hôte}… »,
/// « Annuler ») : à donner à [`wait`] jusqu'à la réponse, ou à [`cancel`].
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PendingBorrow {
    pub target: ShareTarget,
    #[serde(default)]
    pub route: Option<RelayRoute>,
    pub request: String,
    pub secret: String,
    /// Le prénom de l'hôte et le nom de l'ordinateur, quand Admin les donne.
    #[serde(default)]
    pub host: String,
    #[serde(default)]
    pub computer: String,
}

impl std::fmt::Debug for PendingBorrow {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PendingBorrow")
            .field("target", &self.target)
            .field("route", &self.route)
            .field("request", &self.request)
            .field("secret", &"[masqué]")
            .field("host", &self.host)
            .field("computer", &self.computer)
            .finish()
    }
}

/// L'issue de [`borrow`] : le prêt démarre, ou l'hôte doit accepter.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Borrowing {
    Ready(Loan),
    Pending(PendingBorrow),
}

/// Un code de « Rejoindre » qui est un code de prêt
/// ([`crate::share::ShareError::Lend`]) : la Station qui l'a, et le chemin
/// (le relais s'il est venu par lui). L'app emprunte avec [`borrow`] :
/// `borrow(&c.target, c.route.as_ref(), &requête avec LendCredentials::Code)`.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct LendCode {
    pub code: String,
    pub target: ShareTarget,
    #[serde(default)]
    pub route: Option<RelayRoute>,
}

// MARK: - Refus

/// Pourquoi l'emprunt ne se fait pas (docs/link-ui.md, « Refus »).
#[derive(Debug)]
pub enum LendError {
    /// `401 bad_credentials` : « Identifiant ou mot de passe refusé. »
    BadCredentials,
    /// `404 no_code` : « Ce code ne sert plus. »
    NoCode,
    /// `409 lent` : « L'ordinateur est déjà prêté. »
    Lent,
    /// `423 locked` : « L'ordinateur est verrouillé. »
    Locked,
    /// `503 off` : « Le prêt est coupé sur cet ordinateur. »
    Off,
    /// `429 too_many` ; `seconds` : l'attente.
    TooMany { seconds: Option<u64> },
    /// `wait` : `403 refused`, « {hôte} a refusé. »
    Refused,
    /// `wait` : `410 expired`, « {hôte} n'a pas répondu. »
    Expired,
    /// Station sans prêt (avant 0.9.0) : « Le prêt demande KyberNow Station
    /// 0.9.0 ou plus récente. »
    Unsupported,
    /// Le reste : Admin injoignable, certificat changé, relais muet.
    Other(ConsoleError),
}

impl LendError {
    /// Le texte à montrer ; `host` : le prénom de l'hôte quand on le connaît
    /// (refusé, sans réponse).
    pub fn text(&self, lang: Lang, host: &str) -> String {
        let host = host.trim();
        match self {
            Self::BadCredentials => tr(lang, "lend.bad_credentials", &[]),
            Self::NoCode => tr(lang, "lend.no_code", &[]),
            Self::Lent => tr(lang, "lend.lent", &[]),
            Self::Locked => tr(lang, "lend.locked", &[]),
            Self::Off => tr(lang, "lend.off", &[]),
            Self::TooMany { .. } => tr(lang, "share.too_many", &[]),
            Self::Refused if host.is_empty() => tr(lang, "share.refused", &[]),
            Self::Refused => tr(lang, "lend.refused", &[("host", host)]),
            Self::Expired if host.is_empty() => tr(lang, "share.expired", &[]),
            Self::Expired => tr(lang, "lend.expired", &[("host", host)]),
            Self::Unsupported => tr(lang, "lend.unsupported", &[]),
            Self::Other(e) => e.text(lang),
        }
    }

    /// Nom stable pour les ponts : `bad_credentials`, `no_code`, `lent`,
    /// `locked`, `off`, `too_many`, `refused`, `expired`, `unsupported`, `other`.
    pub fn kind(&self) -> &'static str {
        match self {
            Self::BadCredentials => "bad_credentials",
            Self::NoCode => "no_code",
            Self::Lent => "lent",
            Self::Locked => "locked",
            Self::Off => "off",
            Self::TooMany { .. } => "too_many",
            Self::Refused => "refused",
            Self::Expired => "expired",
            Self::Unsupported => "unsupported",
            Self::Other(_) => "other",
        }
    }

    /// Le code des ponts (C, JNI) : 40 à 48 dans l'ordre de [`Self::kind`] ;
    /// `Other` : celui de son erreur de console (voir les ponts).
    pub fn code(&self) -> Option<i32> {
        Some(match self {
            Self::BadCredentials => 40,
            Self::NoCode => 41,
            Self::Lent => 42,
            Self::Locked => 43,
            Self::Off => 44,
            Self::TooMany { .. } => 45,
            Self::Refused => 46,
            Self::Expired => 47,
            Self::Unsupported => 48,
            Self::Other(_) => return None,
        })
    }
}

impl std::fmt::Display for LendError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.text(i18n::current(), ""))
    }
}

impl std::error::Error for LendError {}

impl From<ConsoleError> for LendError {
    fn from(e: ConsoleError) -> Self {
        Self::Other(e)
    }
}

/// `wait` toujours en attente, pour les ponts (C, JNI).
pub const CODE_WAITING: i32 = 49;

/// Un refus d'Admin : `{"error":"<code>"}`, `retry_after` pour `429`.
pub(crate) fn lend_refusal(status: u16, body: &serde_json::Value, retry_after: Option<u64>) -> LendError {
    let code = body.get("error").and_then(|e| e.as_str()).unwrap_or_default();
    match (status, code) {
        (401, _) => LendError::BadCredentials,
        (404, "no_code") => LendError::NoCode,
        // `wait` : une demande que la Station ne connaît plus.
        (404, "unknown") => LendError::Expired,
        // Pas de route du prêt : une Station antérieure.
        (404 | 405, _) => LendError::Unsupported,
        (409, _) => LendError::Lent,
        (423, _) => LendError::Locked,
        (503, "off") => LendError::Off,
        (429, _) => LendError::TooMany { seconds: body.get("retry_after").and_then(|s| s.as_u64()).or(retry_after) },
        (403, "refused") => LendError::Refused,
        (410, _) => LendError::Expired,
        _ => {
            let message = if code.is_empty() { tr(i18n::current(), "console.http_status", &[("status", &status.to_string())]) } else { code.to_string() };
            LendError::Other(ConsoleError::Http { status, message })
        }
    }
}

// MARK: - Pendant le prêt

/// Pourquoi le préavis de 2 minutes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LendNoticeReason {
    /// L'hôte reprend l'ordinateur.
    TakenBack,
    /// La durée du code est passée.
    TimeUp,
    /// Le créneau de l'ami finit.
    SlotEnd,
    /// Chez l'hôte : le créneau d'un ami commence.
    SlotStart,
}

/// Le préavis : sa raison, et l'heure de la fin (secondes Unix).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct LendNotice {
    pub reason: LendNoticeReason,
    #[serde(default)]
    pub at: u64,
}

impl LendNotice {
    /// Le texte de link-ui : chez l'ami, `name` est le prénom de l'hôte
    /// (« {hôte} reprend l'ordinateur dans 2 min : sauvegardez votre
    /// partie. ») ; pour `slot_start`, chez l'hôte, celui de l'ami
    /// (« Créneau de {prénom} : l'ordinateur lui est prêté dans 2 min. »).
    pub fn text(&self, lang: Lang, name: &str) -> String {
        match self.reason {
            LendNoticeReason::TakenBack => tr(lang, "lend.notice_taken_back", &[("host", name)]),
            LendNoticeReason::TimeUp => tr(lang, "lend.notice_time_up", &[]),
            LendNoticeReason::SlotEnd => tr(lang, "lend.notice_slot_end", &[]),
            LendNoticeReason::SlotStart => tr(lang, "lend.notice_slot_start", &[("name", name)]),
        }
    }
}

/// La fin du prêt (`beat`, `end`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LendEnd {
    /// « {hôte} a repris l'ordinateur. »
    TakenBack,
    /// « Le prêt est terminé. » (durée du code passée)
    TimeUp,
    /// « Votre créneau est terminé. »
    SlotEnd,
    /// L'ami a rendu l'ordinateur (« Rendre l'ordinateur », ici ou dans le
    /// lanceur) : une fin voulue, « Vous avez rendu l'ordinateur. ».
    Left,
    /// Le prêt s'est arrêté autrement (agent de console perdu, Station qui
    /// ne le connaît plus) : « Le prêt s'est arrêté. » ; aussi toute fin
    /// inconnue.
    #[serde(other)]
    Stopped,
}

impl LendEnd {
    /// Le texte de link-ui (la carte de fin).
    pub fn text(self, lang: Lang, host: &str) -> String {
        match self {
            Self::TakenBack => tr(lang, "lend.end_taken_back", &[("host", host)]),
            Self::TimeUp => tr(lang, "lend.end_over", &[]),
            Self::SlotEnd => tr(lang, "lend.end_slot_end", &[]),
            Self::Left => tr(lang, "lend.end_left", &[]),
            Self::Stopped => tr(lang, "lend.end_stopped", &[]),
        }
    }

    /// Pour les ponts : 0 repris, 1 durée passée, 2 fin du créneau, 3 rendu, 4 arrêté.
    pub fn code(self) -> i32 {
        match self {
            Self::TakenBack => 0,
            Self::TimeUp => 1,
            Self::SlotEnd => 2,
            Self::Left => 3,
            Self::Stopped => 4,
        }
    }

    /// Une fin que l'ami n'a pas voulue (la session la dit comme un échec).
    pub fn is_failure(self) -> bool {
        self != Self::Left
    }
}

/// Réponse de `POST /api/lend/beat`.
#[derive(Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct LendBeat {
    /// Le jeton de Kyber renouvelé (nul à la fin).
    #[serde(default)]
    pub token: Option<String>,
    #[serde(default)]
    pub expires: u64,
    #[serde(default)]
    pub until: Option<u64>,
    #[serde(default, deserialize_with = "lenient_notice")]
    pub notice: Option<LendNotice>,
    #[serde(default, deserialize_with = "lenient_end")]
    pub end: Option<LendEnd>,
}

impl std::fmt::Debug for LendBeat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LendBeat")
            .field("token", &self.token.as_ref().map(|_| "[masqué]"))
            .field("expires", &self.expires)
            .field("until", &self.until)
            .field("notice", &self.notice)
            .field("end", &self.end)
            .finish()
    }
}

/// Un préavis illisible (raison inconnue) est ignoré, pas le signe de vie.
fn lenient_notice<'de, D: Deserializer<'de>>(d: D) -> Result<Option<LendNotice>, D::Error> {
    let v = Option::<serde_json::Value>::deserialize(d)?;
    Ok(v.filter(|v| v.is_object()).and_then(|v| serde_json::from_value(v).ok()))
}

fn lenient_end<'de, D: Deserializer<'de>>(d: D) -> Result<Option<LendEnd>, D::Error> {
    let v = Option::<serde_json::Value>::deserialize(d)?;
    Ok(v.filter(|v| v.is_string()).and_then(|v| serde_json::from_value(v).ok()))
}

// MARK: - Chez l'hôte

/// L'état du prêt chez l'hôte.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LendHostState {
    /// Le mode console se met en place.
    Preparing,
    /// « Prêté à {prénom} ».
    Lent,
    /// Les apps du prêt se ferment, la session se verrouille.
    Ending,
    /// Rien de prêté (aussi tout état inconnu).
    #[default]
    #[serde(other)]
    Free,
}

/// Comment l'ami est venu.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LendKind {
    /// Un compte d'ami.
    Friend,
    /// Pendant son créneau.
    Slot,
    /// Un code pour une séance (aussi toute valeur inconnue).
    #[default]
    #[serde(other)]
    Code,
}

/// L'ami qui a l'ordinateur.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct LendBorrower {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub kind: LendKind,
    #[serde(default)]
    pub app: String,
    #[serde(default)]
    pub via: ShareVia,
    #[serde(default)]
    pub since: u64,
    #[serde(default)]
    pub until: Option<u64>,
    #[serde(default)]
    pub steam: LendSteam,
    /// Sa session est là (sinon : il peut revenir pendant 15 min).
    #[serde(default)]
    pub connected: bool,
}

/// Une demande d'emprunt, ordinateur utilisé (« {prénom} veut emprunter
/// l'ordinateur. », « Accepter », « Refuser »).
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct LendRequest {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub kind: LendKind,
    #[serde(default)]
    pub app: String,
    #[serde(default)]
    pub via: ShareVia,
    #[serde(default)]
    pub since: u64,
    /// Le compte Steam proposé (celui du compte d'ami, ou du code).
    #[serde(default)]
    pub steam: LendSteam,
    /// La demande vient d'un créneau de l'ami : l'ordinateur lui est prêté
    /// dans 2 min sans réponse (« Créneau de {prénom} : … »).
    #[serde(default)]
    pub slot: bool,
}

/// Le code de séance en cours (« Prêter l'ordinateur… »).
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct LendCodeView {
    #[serde(default)]
    pub code: String,
    #[serde(default)]
    pub link: String,
    #[serde(default)]
    pub web: String,
    /// En secondes ; `None` : « Jusqu'à ce que je reprenne ».
    #[serde(default)]
    pub duration: Option<u64>,
    #[serde(default)]
    pub steam: LendSteam,
    /// Le code ne sert plus après cette heure (24 h).
    #[serde(default)]
    pub expires: u64,
    /// Le relais du code, comme celui de la vue du partage (`connecting`,
    /// `joined`, `unreachable`) ; `None` sans relais réglé, ou illisible.
    #[serde(default, deserialize_with = "crate::share::lenient_relay")]
    pub relay: Option<crate::share::ShareRelay>,
}

impl LendCodeView {
    /// « 123 456 », comme le partage.
    pub fn display_code(&self) -> String {
        crate::share::format_code(&self.code)
    }

    /// Le relais tient le code : il vaut aussi hors de chez soi (`link` et
    /// `web` sont alors ceux du relais), comme [`crate::share::ShareState::relay_joined`].
    pub fn relay_joined(&self) -> bool {
        self.relay.is_some_and(|r| r.state == crate::share::ShareRelayState::Joined)
    }

    /// Le lien du relais se prépare : l'app attend (10 s au plus) avant de
    /// donner celui du réseau, comme [`crate::share::ShareState::relay_connecting`].
    pub fn relay_connecting(&self) -> bool {
        self.relay.is_some_and(|r| r.state == crate::share::ShareRelayState::Connecting)
    }
}

/// La vue du prêt chez l'hôte : `GET /api/lend`, la réponse de chaque
/// `Console::lend_*`, et le champ `lend` du signe de vie de l'hôte.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct LendView {
    /// La Station sait prêter (toujours vrai quand la vue existe).
    #[serde(default)]
    pub available: bool,
    /// Le prêt est activé dans Admin (page « Prêt »).
    #[serde(default)]
    pub enabled: bool,
    /// Le nom de l'ordinateur, et le prénom de l'hôte (le nom complet du
    /// compte Windows de la session, sinon son nom de compte).
    #[serde(default)]
    pub computer: String,
    #[serde(default)]
    pub host: String,
    #[serde(default)]
    pub state: LendHostState,
    #[serde(default)]
    pub borrower: Option<LendBorrower>,
    #[serde(default, deserialize_with = "lenient_notice")]
    pub notice: Option<LendNotice>,
    #[serde(default, deserialize_with = "lenient_requests")]
    pub requests: Vec<LendRequest>,
    #[serde(default)]
    pub code: Option<LendCodeView>,
}

fn lenient_requests<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<LendRequest>, D::Error> {
    let v = Option::<serde_json::Value>::deserialize(d)?;
    Ok(match v {
        Some(serde_json::Value::Array(items)) => items.into_iter().filter_map(|i| serde_json::from_value(i).ok()).collect(),
        _ => Vec::new(),
    })
}

/// Une vue illisible est ignorée, pas tout le signe de vie.
pub(crate) fn lenient_view<'de, D: Deserializer<'de>>(d: D) -> Result<Option<LendView>, D::Error> {
    let v = Option::<serde_json::Value>::deserialize(d)?;
    Ok(v.filter(|v| v.is_object()).and_then(|v| serde_json::from_value(v).ok()))
}

/// `info.lend` (Station 0.9.0 et suivantes) : le prêt existe sur cet
/// ordinateur, et il est activé dans Admin.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct LendInfo {
    #[serde(default)]
    pub available: bool,
    #[serde(default)]
    pub enabled: bool,
}

// MARK: - Lien d'invitation

/// Le lien d'invitation d'un compte d'ami :
/// `kybernow://borrow?host=…&port=…&fp=…&login=…[&relay=…&sid=…]`, ou celui
/// du client web `https://<adresse>:<port>/link/?borrow=<identifiant>` (sans
/// empreinte). L'app ajoute « Ordinateur de {hôte} » à son accueil, à part de
/// ses ordinateurs appairés (identifiant ; mot de passe dans son trousseau).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Invite {
    pub host: String,
    pub port: u16,
    #[serde(default)]
    pub fingerprint: Option<String>,
    pub login: String,
    /// Le relais de la Station, sous sa forme normale.
    #[serde(default)]
    pub relay: Option<String>,
    /// L'identifiant de la Station chez le relais.
    #[serde(default)]
    pub sid: Option<String>,
    /// Le prénom de l'hôte (`name=`), pour « {hôte} vous prête son
    /// ordinateur. » avant le premier statut ; `None` : pas dit par le lien.
    #[serde(default)]
    pub name: Option<String>,
}

impl Invite {
    /// La Station sur son réseau.
    pub fn target(&self) -> ShareTarget {
        ShareTarget { host: self.host.clone(), port: self.port, fingerprint: self.fingerprint.clone() }
    }

    /// Hors de chez soi : le relais, l'identifiant et l'empreinte du lien
    /// (jamais de premier contact par le relais).
    pub fn relay_route(&self) -> Option<RelayRoute> {
        let route = RelayRoute { server: self.relay.clone()?, station: self.sid.clone()?, admin_fp: self.fingerprint.clone()? };
        route.relay_addr().ok()?;
        Some(route)
    }
}

/// Lit un lien d'invitation (voir [`Invite`]) ; `None` s'il n'en est pas un.
pub fn parse_invite(url: &str) -> Option<Invite> {
    use crate::share::{query_pairs, split_authority, strip_brackets};
    let url = url.trim();
    let lower = url.to_ascii_lowercase();
    if lower.starts_with("kybernow://borrow") {
        let rest = &url["kybernow://borrow".len()..];
        let query = rest.trim_start_matches('/').strip_prefix('?')?;
        let pairs = query_pairs(query);
        let get = |k: &str| pairs.iter().find(|(key, _)| key == k).map(|(_, v)| v.trim().to_string()).filter(|v| !v.is_empty());
        let host = strip_brackets(&get("host")?).to_string();
        let port = get("port")?.parse::<u16>().ok().filter(|p| *p > 0)?;
        let login = get("login")?;
        let relay = get("relay").and_then(|r| crate::relay::normalize(&r));
        let sid = get("sid").map(|s| s.to_ascii_lowercase()).filter(|s| kybernow_tunnel::id::is_station_id(s));
        return Some(Invite { host, port, fingerprint: get("fp"), login, relay, sid, name: get("name") });
    }
    // Lien du client web.
    let rest = lower.strip_prefix("https://").map(|_| &url["https://".len()..])?;
    let (authority, path) = rest.split_once('/').unwrap_or((rest, ""));
    let (host, port) = split_authority(authority)?;
    let (page, query) = path.split_once('?')?;
    if !page.trim_end_matches('/').eq_ignore_ascii_case("link") {
        return None;
    }
    let pairs = query_pairs(query.split('#').next().unwrap_or_default());
    let get = |k: &str| pairs.iter().find(|(key, _)| key == k).map(|(_, v)| v.trim().to_string()).filter(|v| !v.is_empty());
    let login = get("borrow")?;
    Some(Invite { host, port, fingerprint: None, login, relay: None, sid: None, name: get("name") })
}

// MARK: - Appels à Admin

/// Ce que répond `borrow` ou `wait`.
#[derive(Debug)]
pub(crate) enum BorrowAnswer {
    Ready(Borrowed),
    Pending { request: String, secret: String, host: String, computer: String },
}

fn borrow_answer(status: u16, v: serde_json::Value) -> Result<BorrowAnswer, LendError> {
    if status == 202 {
        let s = |k: &str| v.get(k).and_then(|x| x.as_str()).unwrap_or_default().to_string();
        return Ok(BorrowAnswer::Pending { request: s("request"), secret: s("secret"), host: s("host"), computer: s("computer") });
    }
    let b: Borrowed = serde_json::from_value(v).map_err(|_| LendError::Unsupported)?;
    // Une page d'Admin d'une Station antérieure (200, sans prêt).
    if b.lend.is_empty() || b.secret.is_empty() {
        return Err(LendError::Unsupported);
    }
    Ok(BorrowAnswer::Ready(b))
}

/// Une réponse d'Admin aux routes de l'ami : la valeur, ou son refus.
fn lend_read(console: &Console, r: Result<ureq::Response, ureq::Error>) -> Result<(u16, serde_json::Value), LendError> {
    match r {
        Ok(resp) => {
            let status = resp.status();
            // Une Station antérieure peut répondre la page d'Admin (200, HTML).
            let v: serde_json::Value = resp.into_json().map_err(|_| LendError::Unsupported)?;
            Ok((status, v))
        }
        Err(ureq::Error::Status(status, resp)) => {
            let retry_after = resp.header("Retry-After").and_then(|h| h.trim().parse().ok());
            let body: serde_json::Value = resp.into_json().unwrap_or(serde_json::Value::Null);
            Err(lend_refusal(status, &body, retry_after))
        }
        Err(e) => Err(LendError::Other(console.read::<serde_json::Value>(Err(e)).err().unwrap_or_else(|| ConsoleError::Invalid(String::new())))),
    }
}

impl Console {
    // KyberNow Share, prêt : l'ami ([`Self::guest`], sans compte).

    /// `POST /api/lend/status` : l'état de l'ordinateur pour ce compte d'ami.
    pub fn lend_status(&self, login: &str, password: &str) -> Result<LendStatus, LendError> {
        let r = self.request("POST", "api/lend/status", Duration::from_secs(6)).send_json(serde_json::json!({ "login": login, "password": password }));
        let (_, v) = lend_read(self, r)?;
        serde_json::from_value(v).map_err(|_| LendError::Unsupported)
    }

    /// `POST /api/lend/borrow` : le prêt démarre (30 s au plus, le temps du
    /// mode console), ou attend l'accord de l'hôte (202).
    pub(crate) fn lend_borrow(&self, req: &BorrowRequest) -> Result<BorrowAnswer, LendError> {
        let r = self.request("POST", "api/lend/borrow", BORROW_WAIT).send_json(req);
        let (status, v) = lend_read(self, r)?;
        borrow_answer(status, v)
    }

    /// `POST /api/lend/wait` : attente longue (30 s), à répéter.
    pub(crate) fn lend_wait(&self, request: &str, secret: &str) -> Result<BorrowAnswer, LendError> {
        let r = self
            .request("POST", "api/lend/wait", WAIT_WAIT)
            .set("x-kybernow-lend", secret)
            .send_json(serde_json::json!({ "request": request }));
        let (status, v) = lend_read(self, r)?;
        borrow_answer(status, v)
    }

    /// `POST /api/lend/beat` : jeton de Kyber renouvelé, préavis, fin.
    pub fn lend_beat(&self, lend: &str, secret: &str) -> crate::console::Result<LendBeat> {
        self.lend_beat_within(lend, secret, Duration::from_secs(10))
    }

    /// [`Self::lend_beat`] avec un autre délai.
    pub fn lend_beat_within(&self, lend: &str, secret: &str, timeout: Duration) -> crate::console::Result<LendBeat> {
        let r = self
            .request("POST", "api/lend/beat", timeout)
            .set("x-kybernow-lend", secret)
            .send_json(serde_json::json!({ "lend": lend }));
        self.read(r)
    }

    /// `POST /api/lend/leave` : « Rendre l'ordinateur » (`{"lend"}`), ou
    /// « Annuler » une demande en attente (`{"request"}`).
    pub(crate) fn lend_leave(&self, field: &str, id: &str, secret: &str) -> crate::console::Result<()> {
        let r = self
            .request("POST", "api/lend/leave", Duration::from_secs(6))
            .set("x-kybernow-lend", secret)
            .send_json(serde_json::json!({ field: id }));
        self.read_unit(r)
    }

    // KyberNow Share, prêt : l'hôte (appareil appairé, comme `share_open`).
    // Chaque appel rend la vue du prêt. Bloquants.

    /// `GET /api/lend`.
    pub fn lend_state(&self) -> crate::console::Result<LendView> {
        let r = self.request("GET", "api/lend", Duration::from_secs(6)).call();
        self.read(r)
    }

    /// « Créer un code » : `duration` en secondes (`None` : « Jusqu'à ce que
    /// je reprenne »), le compte Steam.
    pub fn lend_code(&self, duration: Option<u64>, steam: LendSteam) -> crate::console::Result<LendView> {
        let r = self.request("POST", "api/lend/code", Duration::from_secs(10)).send_json(serde_json::json!({ "duration": duration, "steam": steam }));
        self.read(r)
    }

    /// « Annuler le code ».
    pub fn lend_cancel_code(&self) -> crate::console::Result<LendView> {
        let r = self.request("DELETE", "api/lend/code", Duration::from_secs(6)).call();
        self.read(r)
    }

    /// « Accepter » (`accept`, avec le compte Steam) ou « Refuser » une demande.
    pub fn lend_answer(&self, request: &str, accept: bool, steam: LendSteam) -> crate::console::Result<LendView> {
        let path = format!("api/lend/requests/{}", crate::console::path_segment(request));
        let r = self.request("POST", &path, Duration::from_secs(10)).send_json(serde_json::json!({ "accept": accept, "steam": steam }));
        self.read(r)
    }

    /// « Reprendre l'ordinateur » : préavis de 2 min (`now` faux), ou
    /// « Reprendre tout de suite ».
    pub fn lend_take_back(&self, now: bool) -> crate::console::Result<LendView> {
        let r = self.request("POST", "api/lend/take-back", Duration::from_secs(10)).send_json(serde_json::json!({ "now": now }));
        self.read(r)
    }
}

// MARK: - L'ami : emprunter

/// Où joindre Admin : par le relais seul (une Station venue par le relais,
/// `port` 0), sur le réseau seul (sans route), ou sur le réseau si Admin
/// répond en 1,5 s (même par une erreur), sinon par le relais. Rend la
/// console, la Station à retenir et la route prise.
fn reach(target: &ShareTarget, route: Option<&RelayRoute>) -> (Console, ShareTarget, Option<RelayRoute>) {
    let direct = || Console::guest(&target.host, target.port, target.fingerprint.clone());
    let relay = |r: &RelayRoute| (crate::share::relay_console(r), crate::share::relay_target(r), Some(r.clone()));
    let Some(r) = route else { return (direct(), target.clone(), None) };
    if target.port == 0 || target.host.is_empty() || RelayMode::current() == RelayMode::Force {
        return relay(r);
    }
    if RelayMode::current() == RelayMode::Off {
        return (direct(), target.clone(), None);
    }
    let c = direct();
    match c.info_within(DIRECT_PROBE) {
        Ok(_) | Err(ConsoleError::Http { .. } | ConsoleError::CertificateChanged { .. }) => (c, target.clone(), None),
        Err(e) => {
            log::info!("prêt : Admin ne répond pas sur le réseau ({}), par le relais", e.text(Lang::Fr));
            relay(r)
        }
    }
}

/// L'état de l'ordinateur d'un ami (la carte « Ordinateur de {hôte} »), par
/// son compte. `route` : le relais de l'invitation, pour hors de chez soi.
/// Bloquant.
pub fn status(target: &ShareTarget, route: Option<&RelayRoute>, login: &str, password: &str) -> Result<LendStatus, LendError> {
    let (console, _, _) = reach(target, route);
    console.lend_status(login, password)
}

/// « Emprunter » (compte d'ami) ou « Rejoindre avec un code » qui tombe sur
/// un code de prêt ([`LendCode`]) : le prêt démarre ([`Borrowing::Ready`],
/// à ouvrir avec [`crate::session::SessionParams::borrower`]), ou l'hôte
/// doit accepter ([`Borrowing::Pending`] : « En attente de {hôte}… »).
/// Bloquant : jusqu'à 30 s, le temps que la Station passe en mode console.
pub fn borrow(target: &ShareTarget, route: Option<&RelayRoute>, req: &BorrowRequest) -> Result<Borrowing, LendError> {
    let (console, target, route) = reach(target, route);
    log::info!("prêt : demande {}{}", match req.credentials {
        LendCredentials::Code { .. } => "par un code",
        LendCredentials::Account { .. } => "par un compte d'ami",
    }, if route.is_some() { ", par le relais" } else { "" });
    let answer = console.lend_borrow(req)?;
    Ok(borrowing(&console, target, route, answer))
}

fn borrowing(console: &Console, target: ShareTarget, route: Option<RelayRoute>, answer: BorrowAnswer) -> Borrowing {
    // L'empreinte d'Admin vue pendant l'appel : la session l'épingle.
    let target = ShareTarget { fingerprint: target.fingerprint.clone().or_else(|| console.fingerprint_seen()), ..target };
    match answer {
        BorrowAnswer::Ready(mut borrowed) => {
            borrowed.route = route;
            Borrowing::Ready(Loan { target, borrowed })
        }
        BorrowAnswer::Pending { request, secret, host, computer } => Borrowing::Pending(PendingBorrow { target, route, request, secret, host, computer }),
    }
}

fn pending_console(pending: &PendingBorrow) -> Console {
    match &pending.route {
        Some(r) => crate::share::relay_console(r),
        None => Console::guest(&pending.target.host, pending.target.port, pending.target.fingerprint.clone()),
    }
}

/// L'hôte a-t-il répondu ? Appel tenu 30 s : `Ok(None)` toujours en
/// attente (rappeler), `Ok(Some(prêt))` accepté ; refus :
/// [`LendError::Refused`], [`LendError::Expired`]. Bloquant.
pub fn wait(pending: &PendingBorrow) -> Result<Option<Loan>, LendError> {
    let console = pending_console(pending);
    match console.lend_wait(&pending.request, &pending.secret)? {
        BorrowAnswer::Pending { .. } => Ok(None),
        answer @ BorrowAnswer::Ready(_) => match borrowing(&console, pending.target.clone(), pending.route.clone(), answer) {
            Borrowing::Ready(loan) => Ok(Some(loan)),
            Borrowing::Pending(_) => Ok(None),
        },
    }
}

/// « Annuler » une demande en attente.
pub fn cancel(pending: &PendingBorrow) {
    if let Err(e) = pending_console(pending).lend_leave("request", &pending.request, &pending.secret) {
        log::warn!("prêt : demande pas annulée ({})", e.text(Lang::Fr));
    }
}

/// La console du prêt : sur le réseau, ou par le relais s'il y est venu.
pub(crate) fn loan_console(target: &ShareTarget, borrowed: &Borrowed) -> Console {
    Console::guest(&target.host, target.port, target.fingerprint.clone()).with_relay(borrowed.route.clone())
}

/// « Rendre l'ordinateur » hors d'une session (la session, elle :
/// [`crate::session::Session::give_back`]). Bloquant.
pub fn leave(loan: &Loan) -> Result<(), LendError> {
    loan_console(&loan.target, &loan.borrowed).lend_leave("lend", &loan.borrowed.lend, &loan.borrowed.secret).map_err(LendError::Other)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn etats_et_statut() {
        let s: LendStatus = serde_json::from_str(r#"{"state":"busy","host":"Matthieu","computer":"PC-SALON","slot":{"now":false,"next":{"start":100,"end":200}}}"#).unwrap();
        assert_eq!((s.state, s.mine), (LendAvailability::Busy, false), "`mine` absent : faux");
        let lent: LendStatus = serde_json::from_str(r#"{"state":"lent","mine":true,"host":"Matthieu","computer":"PC-SALON","slot":null}"#).unwrap();
        assert!(lent.mine, "le prêt en cours est le sien");
        assert_eq!(s.slot.unwrap().next, Some(LendPeriod { start: 100, end: 200 }));
        let s: LendStatus = serde_json::from_str(r#"{"state":"plus-tard","host":"M","slot":null}"#).unwrap();
        assert_eq!((s.state, s.slot), (LendAvailability::Off, None), "un état inconnu : prêt coupé");
        assert_eq!(LendAvailability::Free.label(Lang::Fr), "Disponible");
        assert_eq!(LendAvailability::Lent.label(Lang::En), "Lent to someone else");
        for a in [LendAvailability::Free, LendAvailability::Busy, LendAvailability::Lent, LendAvailability::Locked, LendAvailability::Off] {
            assert_eq!(LendAvailability::parse(a.as_str()), Some(a));
        }
    }

    #[test]
    fn demande_d_emprunt() {
        let screen = Mode { width: 2560, height: 1440, refresh: 120 };
        let code = BorrowRequest { credentials: LendCredentials::Code { code: "123456".into() }, name: "Léa".into(), device: "d1".into(), app: "mac".into(), screen, codecs: vec!["hevc".into()] };
        assert_eq!(
            serde_json::to_value(&code).unwrap(),
            serde_json::json!({ "code": "123456", "name": "Léa", "device": "d1", "app": "mac", "screen": { "width": 2560, "height": 1440, "refresh": 120 }, "codecs": ["hevc"] })
        );
        let account: BorrowRequest = serde_json::from_value(serde_json::json!({ "login": "lea", "password": "pw", "name": "Léa", "device": "d1", "app": "ios", "screen": { "width": 1, "height": 2, "refresh": 60 } })).unwrap();
        assert_eq!(account.credentials, LendCredentials::Account { login: "lea".into(), password: "pw".into() });
        assert!(!format!("{account:?}").contains("pw"), "mot de passe masqué");
    }

    #[test]
    fn reponses_de_borrow() {
        let ready = borrow_answer(200, serde_json::json!({
            "lend": "l1", "secret": "S", "host": "Matthieu", "computer": "PC-SALON",
            "kyber": { "port": 24812, "fingerprint": "sha256:kk", "token": "JWT", "expires": 120 },
            "display": { "id": 7, "width": 2560, "height": 1440, "refresh": 120 }, "until": null, "steam": "host", "beat": 5
        }))
        .unwrap();
        let BorrowAnswer::Ready(b) = ready else { panic!() };
        assert_eq!((b.lend.as_str(), b.kyber.port, b.display.id, b.until, b.steam, b.beat), ("l1", 24812, 7, None, LendSteam::Host, 5));
        assert!(!format!("{b:?}").contains("\"S\"") && !format!("{b:?}").contains("JWT"));
        let pending = borrow_answer(202, serde_json::json!({ "request": "r1", "secret": "S2", "host": "Matthieu" })).unwrap();
        assert!(matches!(pending, BorrowAnswer::Pending { ref request, ref host, .. } if request == "r1" && host == "Matthieu"));
        assert!(matches!(borrow_answer(200, serde_json::json!({ "ok": true })), Err(LendError::Unsupported)), "Station antérieure");
        let refusal = |status, error: &str| lend_refusal(status, &serde_json::json!({ "error": error }), None).kind();
        assert_eq!(refusal(401, "bad_credentials"), "bad_credentials");
        assert_eq!(refusal(404, "no_code"), "no_code");
        assert_eq!(refusal(404, "x"), "unsupported");
        assert_eq!(refusal(409, "lent"), "lent");
        assert_eq!(refusal(423, "locked"), "locked");
        assert_eq!(refusal(503, "off"), "off");
        assert_eq!(refusal(403, "refused"), "refused");
        assert_eq!(refusal(410, "expired"), "expired");
        assert_eq!(refusal(404, "unknown"), "expired", "wait : une demande que la Station ne connaît plus");
        assert!(matches!(lend_refusal(429, &serde_json::json!({ "error": "too_many", "retry_after": 30 }), Some(5)), LendError::TooMany { seconds: Some(30) }));
        assert!(matches!(lend_refusal(500, &serde_json::Value::Null, None), LendError::Other(ConsoleError::Http { status: 500, .. })));
    }

    #[test]
    fn textes_et_codes() {
        assert_eq!(LendError::BadCredentials.text(Lang::Fr, ""), "Identifiant ou mot de passe refusé.");
        assert_eq!(LendError::NoCode.text(Lang::En, ""), "This code is no longer valid.");
        assert_eq!(LendError::Lent.text(Lang::Fr, ""), "L'ordinateur est déjà prêté.");
        assert_eq!(LendError::Locked.text(Lang::Fr, ""), "L'ordinateur est verrouillé.");
        assert_eq!(LendError::Off.text(Lang::En, ""), "Lending is off on this computer.");
        assert_eq!(LendError::Refused.text(Lang::Fr, "Matthieu"), "Matthieu a refusé.");
        assert_eq!(LendError::Expired.text(Lang::En, "Matthieu"), "Matthieu didn't answer.");
        assert_eq!(LendError::Refused.text(Lang::Fr, " "), "L'hôte a refusé.");
        assert_eq!(LendError::Unsupported.text(Lang::Fr, ""), "Le prêt demande KyberNow Station 0.9.0 ou plus récente.");
        let codes: Vec<Option<i32>> = [LendError::BadCredentials, LendError::NoCode, LendError::Lent, LendError::Locked, LendError::Off, LendError::TooMany { seconds: None }, LendError::Refused, LendError::Expired, LendError::Unsupported]
            .iter()
            .map(LendError::code)
            .collect();
        assert_eq!(codes, (40..=48).map(Some).collect::<Vec<_>>());
        let n = |reason| LendNotice { reason, at: 0 };
        assert_eq!(n(LendNoticeReason::TakenBack).text(Lang::Fr, "Matthieu"), "Matthieu reprend l'ordinateur dans 2 min : sauvegardez votre partie.");
        assert_eq!(n(LendNoticeReason::TimeUp).text(Lang::En, "M"), "The loan ends in 2 min: save your game.");
        assert_eq!(n(LendNoticeReason::SlotEnd).text(Lang::Fr, "M"), "Votre créneau se termine dans 2 min : sauvegardez votre partie.");
        assert_eq!(n(LendNoticeReason::SlotStart).text(Lang::Fr, "Léa"), "Créneau de Léa : l'ordinateur lui est prêté dans 2 min.");
        assert_eq!(LendEnd::TakenBack.text(Lang::Fr, "Matthieu"), "Matthieu a repris l'ordinateur.");
        assert_eq!(LendEnd::TimeUp.text(Lang::Fr, "M"), "Le prêt est terminé.");
        assert_eq!(LendEnd::SlotEnd.text(Lang::En, "M"), "Your time slot is over.");
        assert_eq!(LendEnd::Left.text(Lang::Fr, "M"), "Vous avez rendu l'ordinateur.");
        assert_eq!(LendEnd::Left.text(Lang::En, "M"), "You gave back the computer.");
        assert_eq!(LendEnd::Stopped.text(Lang::Fr, "M"), "Le prêt s'est arrêté.");
        assert_eq!(LendEnd::Stopped.text(Lang::En, "M"), "The loan has stopped.");
        assert_eq!([LendEnd::TakenBack, LendEnd::TimeUp, LendEnd::SlotEnd, LendEnd::Left, LendEnd::Stopped].map(LendEnd::code), [0, 1, 2, 3, 4]);
        assert!(!LendEnd::Left.is_failure() && LendEnd::TakenBack.is_failure());
    }

    #[test]
    fn signe_de_vie() {
        let b: LendBeat = serde_json::from_str(r#"{"token":"T","expires":120,"until":1000,"notice":{"reason":"taken_back","at":1120},"end":null}"#).unwrap();
        assert_eq!((b.token.as_deref(), b.until, b.notice, b.end), (Some("T"), Some(1000), Some(LendNotice { reason: LendNoticeReason::TakenBack, at: 1120 }), None));
        let b: LendBeat = serde_json::from_str(r#"{"token":null,"notice":{"reason":"bientot"},"end":"slot_end"}"#).unwrap();
        assert_eq!((b.notice, b.end), (None, Some(LendEnd::SlotEnd)), "préavis illisible ignoré");
        let b: LendBeat = serde_json::from_str(r#"{"end":"autre"}"#).unwrap();
        assert_eq!(b.end, Some(LendEnd::Stopped), "fin inconnue : arrêté");
    }

    #[test]
    fn vue_de_l_hote() {
        let v: LendView = serde_json::from_str(
            r#"{"enabled":true,"state":"lent","borrower":{"name":"Léa","kind":"slot","app":"ios","via":"relay","since":10,"until":null,"steam":"friend","connected":true},
               "notice":null,"requests":[{"id":"r1","name":"Tom","kind":"friend","app":"web","via":"lan","since":20,"steam":"host","slot":true},{"oups":1}],
               "available":true,"computer":"PC-SALON","host":"Matthieu",
               "code":{"code":"123456","link":"kybernow://join?x","web":"https://h/link/?join=123456","duration":7200,"steam":"host","expires":99}}"#,
        )
        .unwrap();
        assert_eq!(v.state, LendHostState::Lent);
        let b = v.borrower.as_ref().unwrap();
        assert_eq!((b.kind, b.via, b.connected), (LendKind::Slot, ShareVia::Relay, true));
        assert_eq!(v.requests.len(), 2, "une demande illisible garde ses champs par défaut");
        assert_eq!((v.requests[0].steam, v.requests[0].slot), (LendSteam::Host, true));
        assert_eq!((v.requests[1].steam, v.requests[1].slot), (LendSteam::Friend, false));
        assert_eq!((v.available, v.computer.as_str(), v.host.as_str()), (true, "PC-SALON", "Matthieu"));
        assert_eq!(v.code.as_ref().unwrap().display_code(), "123 456");
        let empty: LendView = serde_json::from_str("{}").unwrap();
        assert_eq!(empty, LendView::default());
        // Le relais du code de séance, comme celui du partage.
        let code = |relay: &str| serde_json::from_str::<LendCodeView>(&format!(r#"{{"code":"123456","relay":{relay}}}"#)).unwrap();
        assert!(code(r#"{"state":"joined"}"#).relay_joined() && !code(r#"{"state":"joined"}"#).relay_connecting());
        assert!(code(r#"{"state":"connecting"}"#).relay_connecting());
        let unreachable = code(r#"{"state":"unreachable"}"#);
        assert!(!unreachable.relay_joined() && !unreachable.relay_connecting() && unreachable.relay.is_some());
        assert_eq!(code(r#"{"state":"plus-tard"}"#).relay.map(|r| r.state), Some(crate::share::ShareRelayState::Unreachable), "inconnu : injoignable");
        assert_eq!((code("null").relay, code(r#""oups""#).relay), (None, None), "sans relais, ou illisible");
    }

    #[test]
    fn invitations() {
        let i = parse_invite("kybernow://borrow?host=192.168.1.20&port=24811&fp=sha256%3Aaa&login=lea&relay=Share.Kybernow.app%3A443&sid=ABCDEFGHIJKLMNOPQRSTUVWXYZ").unwrap();
        assert_eq!((i.host.as_str(), i.port, i.fingerprint.as_deref(), i.login.as_str()), ("192.168.1.20", 24811, Some("sha256:aa"), "lea"));
        assert_eq!((i.relay.as_deref(), i.sid.as_deref()), (Some("share.kybernow.app"), Some("abcdefghijklmnopqrstuvwxyz")));
        assert_eq!(i.relay_route(), Some(RelayRoute { server: "share.kybernow.app".into(), station: "abcdefghijklmnopqrstuvwxyz".into(), admin_fp: "sha256:aa".into() }));
        assert_eq!(i.target(), ShareTarget { host: "192.168.1.20".into(), port: 24811, fingerprint: Some("sha256:aa".into()) });
        let i = parse_invite("kybernow://borrow/?login=tom&port=9443&host=[fe80::1]").unwrap();
        assert_eq!((i.host.as_str(), i.fingerprint.clone(), i.relay_route()), ("fe80::1", None, None), "sans empreinte : jamais par le relais");
        let i = parse_invite("https://192.168.1.20:24811/link/?borrow=lea").unwrap();
        assert_eq!((i.host.as_str(), i.port, i.login.as_str(), i.fingerprint), ("192.168.1.20", 24811, "lea", None));
        assert_eq!(parse_invite("kybernow://borrow?host=h&port=1"), None, "sans identifiant");
        // Le prénom de l'hôte, s'il est dit (encodé), dans les deux formes.
        assert_eq!(parse_invite("kybernow://borrow?host=h&port=1&login=lea&name=Ma%C3%ABl%20Dupont").unwrap().name.as_deref(), Some("Maël Dupont"));
        assert_eq!(parse_invite("https://h:1/link/?borrow=lea&name=Ma%C3%ABl").unwrap().name.as_deref(), Some("Maël"));
        assert_eq!(parse_invite("kybernow://borrow?host=h&port=1&login=lea&name=").unwrap().name, None, "vide : pas de prénom");
        assert_eq!(i.name, None);
        let json = serde_json::to_value(parse_invite("kybernow://borrow?host=h&port=1&login=lea&name=Ma%C3%ABl").unwrap()).unwrap();
        assert_eq!(json["name"], "Maël", "dans le JSON des ponts");
        let old: Invite = serde_json::from_value(serde_json::json!({ "host": "h", "port": 1, "login": "lea" })).unwrap();
        assert_eq!(old.name, None, "une invitation gardée avant le prénom se relit");
        assert_eq!(parse_invite("kybernow://join?host=h&port=1&code=123456"), None);
        assert_eq!(parse_invite("https://h:1/link/?join=123456"), None);
    }
}

/// Le prêt de bout en bout, sur la boucle locale : un faux relais et un faux
/// Admin ([`crate::relay::essai`]) avec le vrai tunnel.
#[cfg(test)]
mod essais_par_le_relais {
    use super::*;
    use crate::share::{JoinRequest, ShareError, join_anywhere};
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    const BORROWED: &str = r#"{"lend":"l1","secret":"S","host":"Matthieu","computer":"PC-SALON",
        "kyber":{"port":24812,"fingerprint":"sha256:kk","token":"jwt","expires":120},
        "display":{"id":7,"width":2560,"height":1440,"refresh":120},"until":null,"steam":"friend","beat":5}"#;

    #[test]
    fn emprunt_par_un_code_hors_de_chez_soi() {
        let (waits, leaves, statuses) = (Arc::new(AtomicUsize::new(0)), Arc::new(AtomicUsize::new(0)), Arc::new(AtomicUsize::new(0)));
        let (w, l, s) = (waits.clone(), leaves.clone(), statuses.clone());
        let bench = crate::relay::essai::start(Arc::new(move |method, path| match (method, path) {
            // « Rejoindre avec un code » : c'est un code de prêt.
            ("POST", "/api/share/join") => (404, r#"{"error":"no_share","lend":true}"#.into()),
            ("POST", "/api/lend/borrow") => (202, r#"{"request":"r1","secret":"S2","host":"Matthieu"}"#.into()),
            // L'hôte accepte au deuxième appel.
            ("POST", "/api/lend/wait") if w.fetch_add(1, Ordering::SeqCst) == 0 => (202, r#"{"pending":true}"#.into()),
            ("POST", "/api/lend/wait") => (200, BORROWED.into()),
            ("POST", "/api/lend/leave") => {
                l.fetch_add(1, Ordering::SeqCst);
                (200, "{}".into())
            }
            ("POST", "/api/lend/status") if s.fetch_add(1, Ordering::SeqCst) == 0 => (401, r#"{"error":"bad_credentials"}"#.into()),
            ("POST", "/api/lend/status") => (200, r#"{"state":"free","host":"Matthieu","computer":"PC-SALON","slot":null}"#.into()),
            ("POST", "/api/lend/beat") => (200, r#"{"token":"jwt-2","expires":120,"until":null,"notice":null,"end":null}"#.into()),
            _ => (404, r#"{"error":"inconnu"}"#.into()),
        }));
        let req = JoinRequest { code: "123456".into(), name: "Léa".into(), device: "mac".into(), app: "mac".into(), ..JoinRequest::default() };
        // Personne sur le réseau ; le relais connaît le code ; la Station dit « prêt ».
        let e = join_anywhere(&[], &[bench.addr()], &req, None).unwrap_err();
        let ShareError::Lend(code) = e else { panic!("un code de prêt attendu : {e:?}") };
        let route = bench.route();
        assert_eq!((code.code.as_str(), code.route.as_ref()), ("123456", Some(&route)));
        assert_eq!((code.target.host.as_str(), code.target.port, code.target.fingerprint.as_deref()), (bench.station.as_str(), 0, Some(bench.admin_fp.as_str())));
        assert_eq!(ShareError::Lend(code.clone()).kind(), "lend");
        // Emprunter avec ce code, par le même chemin : l'hôte doit accepter.
        let breq = BorrowRequest {
            credentials: LendCredentials::Code { code: code.code.clone() },
            name: "Léa".into(),
            device: "mac".into(),
            app: "mac".into(),
            screen: Mode { width: 2560, height: 1440, refresh: 120 },
            codecs: vec!["hevc".into()],
        };
        let Borrowing::Pending(pending) = borrow(&code.target, code.route.as_ref(), &breq).unwrap() else { panic!("en attente de l'hôte") };
        assert_eq!((pending.request.as_str(), pending.host.as_str(), pending.route.as_ref()), ("r1", "Matthieu", Some(&route)));
        assert!(!format!("{pending:?}").contains("S2"), "jeton masqué");
        // Toujours en attente, puis accepté : la session passera par le relais.
        assert_eq!(wait(&pending).unwrap(), None);
        let loan = wait(&pending).unwrap().expect("accepté");
        assert_eq!((loan.borrowed.lend.as_str(), loan.borrowed.route.as_ref()), ("l1", Some(&route)));
        let device = crate::console::DeviceInfo { id: "mac".into(), name: "Mac".into(), kind: "mac".into(), os: "macOS".into(), app: "t".into() };
        let params = crate::session::SessionParams::borrower(&loan, device, crate::profile::StreamSettings::for_profile(crate::profile::Profile::Jeu, crate::profile::Capabilities::default()), 2);
        assert_eq!(params.route, crate::session::Route::Relay(route.clone()));
        assert_eq!((params.console_fingerprint.as_deref(), params.clipboard), (Some(bench.admin_fp.as_str()), false));
        // Le signe de vie, par le relais.
        let beat = loan_console(&loan.target, &loan.borrowed).lend_beat("l1", "S").unwrap();
        assert_eq!(beat.token.as_deref(), Some("jwt-2"));
        // « Annuler » une demande, « Rendre l'ordinateur » : par le relais aussi.
        cancel(&pending);
        leave(&loan).unwrap();
        assert_eq!(leaves.load(Ordering::SeqCst), 2);
        // La carte d'un compte d'ami : mot de passe refusé, puis disponible.
        let relay_only = crate::share::relay_target(&route);
        assert!(matches!(status(&relay_only, Some(&route), "lea", "faux"), Err(LendError::BadCredentials)));
        let s = status(&relay_only, Some(&route), "lea", "bon").unwrap();
        assert_eq!((s.state, s.host.as_str()), (LendAvailability::Free, "Matthieu"));
    }

    #[test]
    fn station_sans_pret() {
        let bench = crate::relay::essai::start(Arc::new(|_, _| (404, r#"{"error":"inconnu"}"#.into())));
        let route = bench.route();
        let e = status(&crate::share::relay_target(&route), Some(&route), "lea", "pw").unwrap_err();
        assert_eq!(e.kind(), "unsupported", "Station antérieure : pas de route du prêt");
    }
}
