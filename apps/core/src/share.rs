//! KyberNow Share, le partage en local (docs/kybernow-share.md, textes dans
//! docs/link-ui.md, « Partage ») : ce que le cœur échange avec Admin pour un
//! hôte et pour un invité, le lien `kybernow://join?…`, le code à six
//! chiffres, la vérification du format de l'image, et la recherche de la
//! Station qui partage parmi celles du réseau ([`join`]).
//!
//! Les routes elles-mêmes sont des méthodes de [`Console`] : côté hôte
//! (`share_open`, `share_state`, `share_code`, `share_guest`, `share_kick`,
//! `share_close`, avec le compte de l'appareil) et côté invité (`share_join`,
//! `share_beat`, `share_leave`, sans compte : [`Console::guest`]). La session
//! d'un invité ([`crate::session::SessionRole::Guest`]) s'en sert seule.

use std::sync::mpsc;
use std::time::Duration;

use serde::{Deserialize, Deserializer, Serialize};

use crate::console::{Console, ConsoleError, Info};
use crate::i18n::{self, Lang, tr};
use crate::input::InputRights;
use crate::profile::{Capabilities, Codec};

// MARK: - Rôle

/// Le rôle d'un invité, que la Station impose (correctif 0019 de Kyber) :
/// deux droits indépendants, les manettes et le clavier et la souris, que
/// l'hôte donne ou reprend chacun sans toucher à l'autre
/// ([`Self::with_pads`], [`Self::with_kbm`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum ShareRole {
    /// Aucun droit : voit et entend, n'envoie rien (le rôle d'arrivée).
    #[default]
    Spectator,
    /// Les manettes : elles deviennent la manette suivante de Windows.
    Player,
    /// Le clavier et la souris, sans les manettes.
    Kbm,
    /// Les manettes, et le clavier et la souris.
    PlayerKbm,
}

impl ShareRole {
    /// Nom dans l'échange avec Admin : `spectator`, `player`, `kbm`, `player_kbm`.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Spectator => "spectator",
            Self::Player => "player",
            Self::Kbm => "kbm",
            Self::PlayerKbm => "player_kbm",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "spectator" => Some(Self::Spectator),
            "player" => Some(Self::Player),
            "kbm" => Some(Self::Kbm),
            "player_kbm" => Some(Self::PlayerKbm),
            _ => None,
        }
    }

    /// Le rôle de ces deux droits.
    pub fn of(pads: bool, kbm: bool) -> Self {
        match (pads, kbm) {
            (false, false) => Self::Spectator,
            (true, false) => Self::Player,
            (false, true) => Self::Kbm,
            (true, true) => Self::PlayerKbm,
        }
    }

    /// Les manettes de l'invité partent vers l'ordinateur.
    pub fn pads(self) -> bool {
        matches!(self, Self::Player | Self::PlayerKbm)
    }

    /// Le clavier et la souris de l'invité partent vers l'ordinateur.
    pub fn kbm(self) -> bool {
        matches!(self, Self::Kbm | Self::PlayerKbm)
    }

    /// Un droit au moins (manettes, ou clavier et souris) : l'invité a une
    /// chaîne d'entrées, et la barre le dit « Joueur » (« Spectateur » sinon,
    /// docs/link-ui.md).
    pub fn plays(self) -> bool {
        self.pads() || self.kbm()
    }

    /// « Donner les manettes » (`true`) ou « Reprendre les manettes »
    /// (`false`), le clavier et la souris tels quels.
    pub fn with_pads(self, on: bool) -> Self {
        Self::of(on, self.kbm())
    }

    /// « Donner le clavier et la souris » ou « Reprendre le clavier et la
    /// souris », les manettes telles quelles.
    pub fn with_kbm(self, on: bool) -> Self {
        Self::of(self.pads(), on)
    }

    /// Ce que la chaîne d'entrées d'un invité de ce rôle laisse partir
    /// (jamais le presse-papiers).
    pub fn rights(self) -> InputRights {
        InputRights { gamepads: self.pads(), keyboard_mouse: self.kbm(), clipboard: false }
    }

    /// Pour les ponts C et JNI : 0 spectateur, 1 manettes, 2 manettes avec
    /// le clavier et la souris, 3 le clavier et la souris seuls.
    pub fn code(self) -> i32 {
        match self {
            Self::Spectator => 0,
            Self::Player => 1,
            Self::PlayerKbm => 2,
            Self::Kbm => 3,
        }
    }

    /// L'inverse de [`Self::code`].
    pub fn from_code(code: i32) -> Option<Self> {
        match code {
            0 => Some(Self::Spectator),
            1 => Some(Self::Player),
            2 => Some(Self::PlayerKbm),
            3 => Some(Self::Kbm),
            _ => None,
        }
    }
}

impl Serialize for ShareRole {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(self.as_str())
    }
}

/// Un rôle inconnu (Station plus récente) compte comme spectateur : rien ne
/// part de l'appareil.
impl<'de> Deserialize<'de> for ShareRole {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = Option::<String>::deserialize(d)?;
        Ok(s.as_deref().and_then(Self::parse).unwrap_or_default())
    }
}

// MARK: - Ce que la Station annonce

/// `info.share` (Station 0.8.0 et suivantes).
#[derive(Debug, Clone, Copy, Deserialize, Serialize, Default, PartialEq, Eq)]
pub struct ShareInfo {
    #[serde(default)]
    pub available: bool,
    /// Faux si `multi_client` de Kyber est coupé.
    #[serde(default)]
    pub enabled: bool,
}

/// Ce que « Partager » peut faire avec cet ordinateur (docs/link-ui.md).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShareAvailability {
    /// Station antérieure (pas de `share` dans `info`) : « Le partage
    /// demande KyberNow Station 0.8.0 ou plus récente. », « Partager » inactif.
    Unsupported,
    /// Partage coupé sur l'ordinateur : « Le partage est coupé sur cet
    /// ordinateur (Admin › Appareils Link). »
    Disabled,
    Available,
}

impl ShareAvailability {
    pub fn of(info: &Info) -> Self {
        match info.share {
            None => Self::Unsupported,
            Some(s) if !s.available => Self::Unsupported,
            Some(s) if !s.enabled => Self::Disabled,
            Some(_) => Self::Available,
        }
    }

    /// Pour les ponts : 0 Station trop ancienne, 1 partage coupé, 2 disponible.
    pub fn code(self) -> i32 {
        match self {
            Self::Unsupported => 0,
            Self::Disabled => 1,
            Self::Available => 2,
        }
    }

    /// Le texte à montrer, `None` quand le partage est disponible.
    pub fn text(self, lang: Lang) -> Option<String> {
        match self {
            Self::Unsupported => Some(tr(lang, "share.unsupported", &[])),
            Self::Disabled => Some(tr(lang, "share.disabled", &[])),
            Self::Available => None,
        }
    }
}

// MARK: - Hôte

/// L'écran partagé, tel qu'Admin le nomme.
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct ShareScreen {
    #[serde(default)]
    pub id: u32,
    /// « Écran 1 ».
    #[serde(default)]
    pub label: String,
}

/// Réponse de `POST /api/share/open` et `POST /api/share/code`.
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct ShareOpened {
    /// Six chiffres ([`format_code`] pour l'afficher).
    #[serde(default)]
    pub code: String,
    /// `kybernow://join?host=…&port=…&fp=…&code=…` (QR code, « Copier le lien »).
    #[serde(default)]
    pub link: String,
    /// `https://<hôte>:<port>/link/?join=…` : le client web en invité.
    #[serde(default)]
    pub web: String,
    #[serde(default)]
    pub display: Option<ShareScreen>,
}

/// Un invité, dans la liste de l'hôte.
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct ShareGuest {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub name: String,
    /// `mac`, `windows`, `linux`, `ios`, `tvos`, `android`, `webos`, `web`.
    #[serde(default)]
    pub app: String,
    #[serde(default)]
    pub role: ShareRole,
    /// Arrivée, telle que la Station la donne (horodatage numérique ; absent
    /// s'il est illisible).
    #[serde(default, deserialize_with = "lenient_i64")]
    pub since: Option<i64>,
}

/// Le partage ouvert : `GET /api/share`, et le champ `share` du signe de vie
/// de l'hôte ([`crate::session::Platform::on_guests`]).
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct ShareState {
    #[serde(default)]
    pub open: bool,
    #[serde(default)]
    pub code: String,
    #[serde(default)]
    pub link: String,
    #[serde(default)]
    pub web: String,
    #[serde(default)]
    pub display: Option<ShareScreen>,
    /// Quatre au plus, dans l'ordre d'arrivée.
    #[serde(default, deserialize_with = "lenient_guests")]
    pub guests: Vec<ShareGuest>,
}

/// Quatre invités au plus (la Station refuse le cinquième).
pub const MAX_GUESTS: usize = 4;

fn lenient_i64<'de, D: Deserializer<'de>>(d: D) -> Result<Option<i64>, D::Error> {
    let v = Option::<serde_json::Value>::deserialize(d)?;
    Ok(v.and_then(|v| v.as_i64().or_else(|| v.as_f64().map(|f| f as i64))))
}

/// Un invité illisible est écarté, pas toute la liste.
fn lenient_guests<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<ShareGuest>, D::Error> {
    let v = Option::<serde_json::Value>::deserialize(d)?;
    Ok(match v {
        Some(serde_json::Value::Array(items)) => items.into_iter().filter_map(|g| serde_json::from_value(g).ok()).collect(),
        _ => Vec::new(),
    })
}

/// Le champ `share` du signe de vie de l'hôte : illisible, il est ignoré
/// (le signe de vie, lui, doit toujours être lu : il porte la fin demandée).
pub(crate) fn lenient_state<'de, D: Deserializer<'de>>(d: D) -> Result<Option<ShareState>, D::Error> {
    let v = Option::<serde_json::Value>::deserialize(d)?;
    Ok(v.filter(|v| v.is_object()).and_then(|v| serde_json::from_value(v).ok()))
}

// MARK: - Invité

/// Corps de `POST /api/share/join`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct JoinRequest {
    /// Six chiffres ([`normalize_code`]).
    pub code: String,
    /// Le prénom de l'invité (proposé : le nom de l'appareil).
    pub name: String,
    /// L'identifiant de l'appareil (`DeviceInfo::id`).
    pub device: String,
    /// `mac`, `windows`, `linux`, `ios`, `tvos`, `android`, `webos`.
    pub app: String,
}

/// Connexion à Kyber d'un invité.
#[derive(Clone, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct ShareKyber {
    #[serde(default)]
    pub port: u16,
    /// Empreinte du certificat de Kyber : obligatoire pour un invité (aucun
    /// premier contact).
    #[serde(default)]
    pub fingerprint: Option<String>,
    /// Jeton d'accès (JWT, revendications `kn_role` et `kn_display`),
    /// renouvelé par chaque signe de vie.
    #[serde(default)]
    pub token: String,
    /// Durée du jeton, en secondes.
    #[serde(default)]
    pub expires: u64,
}

impl std::fmt::Debug for ShareKyber {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ShareKyber")
            .field("port", &self.port)
            .field("fingerprint", &self.fingerprint)
            .field("token", &"[masqué]")
            .field("expires", &self.expires)
            .finish()
    }
}

/// L'écran de l'ordinateur que l'invité reçoit.
#[derive(Debug, Clone, Copy, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct ShareDisplay {
    /// Identifiant d'écran de Kyber.
    #[serde(default)]
    pub id: u32,
    #[serde(default)]
    pub width: u32,
    #[serde(default)]
    pub height: u32,
    #[serde(default)]
    pub refresh: u32,
}

/// Réponse de `POST /api/share/join`, à donner telle quelle à la session de
/// l'invité ([`crate::session::GuestParams::joined`]).
#[derive(Clone, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct ShareJoined {
    /// L'identifiant de l'invité (signe de vie, départ).
    #[serde(default)]
    pub guest: String,
    /// Son jeton (en-tête `x-kybernow-guest`).
    #[serde(default)]
    pub secret: String,
    #[serde(default)]
    pub role: ShareRole,
    /// {hôte} des textes : le prénom de l'hôte, ou le nom de l'ordinateur
    /// pour un partage lancé depuis le PC.
    #[serde(default)]
    pub host: String,
    /// Le nom de l'ordinateur (« Invité sur {nom} »).
    #[serde(default)]
    pub computer: String,
    #[serde(default)]
    pub kyber: ShareKyber,
    #[serde(default)]
    pub display: ShareDisplay,
    /// Format de l'image en cours (`h264`, `hevc`, `av1`) ; absent si elle
    /// n'a pas encore démarré.
    #[serde(default)]
    pub codec: Option<String>,
    #[serde(default)]
    pub yuv444: bool,
    /// Signe de vie toutes les `beat` secondes.
    #[serde(default = "five")]
    pub beat: u64,
}

fn five() -> u64 {
    5
}

impl ShareJoined {
    /// Le format de l'image, s'il est connu et lisible par le cœur.
    pub fn codec(&self) -> Option<Codec> {
        self.codec.as_deref().and_then(Codec::parse)
    }

    /// {hôte} des textes : le prénom de l'hôte, sinon le nom de l'ordinateur.
    pub fn host_name(&self) -> &str {
        if self.host.trim().is_empty() { &self.computer } else { &self.host }
    }
}

impl std::fmt::Debug for ShareJoined {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ShareJoined")
            .field("guest", &self.guest)
            .field("secret", &"[masqué]")
            .field("role", &self.role)
            .field("host", &self.host)
            .field("computer", &self.computer)
            .field("kyber", &self.kyber)
            .field("display", &self.display)
            .field("codec", &self.codec)
            .field("yuv444", &self.yuv444)
            .field("beat", &self.beat)
            .finish()
    }
}

/// Réponse de `POST /api/share/beat`.
#[derive(Clone, Default, Deserialize, PartialEq, Eq)]
pub struct ShareBeat {
    #[serde(default)]
    pub role: ShareRole,
    /// Jeton de Kyber renouvelé (nul avec une fin).
    #[serde(default)]
    pub token: Option<String>,
    #[serde(default)]
    pub expires: Option<u64>,
    /// L'écran partagé, tel qu'il est maintenant (l'hôte a pu le refaire :
    /// autre identifiant) ; le jeton qui l'accompagne porte son `kn_display`.
    #[serde(default)]
    pub display: Option<ShareDisplay>,
    /// Fin du partage pour cet invité.
    #[serde(default, deserialize_with = "lenient_end")]
    pub end: Option<ShareEnd>,
}

impl std::fmt::Debug for ShareBeat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ShareBeat")
            .field("role", &self.role)
            .field("token", &self.token.as_ref().map(|_| "[masqué]"))
            .field("expires", &self.expires)
            .field("display", &self.display)
            .field("end", &self.end)
            .finish()
    }
}

/// Pourquoi la session d'un invité finit, quand c'est le partage qui la
/// finit ([`crate::session::Platform::on_share_end`]) : jamais « Reprendre
/// la main ».
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ShareEnd {
    /// « {hôte} a arrêté le partage. »
    Stopped,
    /// « {hôte} vous a retiré du partage. »
    Removed,
    /// « Cet appareil ne lit pas l'image de cet ordinateur ({format}). »,
    /// avant toute image.
    Unreadable { format: String },
}

impl ShareEnd {
    /// Pour les ponts : 0 arrêté, 1 retiré, 2 format illisible.
    pub fn code(&self) -> i32 {
        match self {
            Self::Stopped => 0,
            Self::Removed => 1,
            Self::Unreadable { .. } => 2,
        }
    }

    pub fn kind(&self) -> &'static str {
        match self {
            Self::Stopped => "stopped",
            Self::Removed => "removed",
            Self::Unreadable { .. } => "unreadable",
        }
    }

    /// Le texte de la fin (docs/link-ui.md), `host` : {hôte}.
    pub fn text(&self, lang: Lang, host: &str) -> String {
        match self {
            Self::Stopped => tr(lang, "share.stopped", &[("host", host)]),
            Self::Removed => tr(lang, "share.removed", &[("host", host)]),
            Self::Unreadable { format } => tr(lang, "share.unreadable", &[("format", format)]),
        }
    }
}

/// `end` du signe de vie : `stopped`, `removed` ; une autre valeur (Station
/// plus récente) finit aussi le partage, comme un arrêt.
fn lenient_end<'de, D: Deserializer<'de>>(d: D) -> Result<Option<ShareEnd>, D::Error> {
    let v = Option::<serde_json::Value>::deserialize(d)?;
    Ok(match v {
        None | Some(serde_json::Value::Null) | Some(serde_json::Value::Bool(false)) => None,
        Some(serde_json::Value::String(s)) if s == "removed" => Some(ShareEnd::Removed),
        Some(serde_json::Value::String(s)) if s.is_empty() => None,
        Some(_) => Some(ShareEnd::Stopped),
    })
}

// MARK: - Refus

/// Échec de « Rejoindre ».
#[derive(Debug)]
pub enum ShareError {
    /// Aucune Station n'a ce code (404 `no_share`, ou personne n'a répondu) :
    /// « Aucun partage avec ce code sur le réseau. »
    NoShare,
    /// 409 `full` : « Ce partage est complet (quatre invités au plus). »
    Full,
    /// 429 (10 essais par minute et par adresse) ; `seconds` : l'attente.
    TooManyTries { seconds: Option<u64> },
    /// L'appareil ne lit pas le format de l'image de l'hôte.
    Unreadable { format: String },
    /// Le reste : Admin injoignable, certificat changé, réponse illisible.
    Other(ConsoleError),
}

impl ShareError {
    pub fn text(&self, lang: Lang) -> String {
        match self {
            Self::NoShare => tr(lang, "share.no_share", &[]),
            Self::Full => tr(lang, "share.full", &[]),
            Self::TooManyTries { .. } => tr(lang, "share.too_many", &[]),
            Self::Unreadable { format } => tr(lang, "share.unreadable", &[("format", format)]),
            Self::Other(e) => e.text(lang),
        }
    }

    /// Nom stable pour les ponts : `no_share`, `full`, `too_many`,
    /// `unreadable`, `other`.
    pub fn kind(&self) -> &'static str {
        match self {
            Self::NoShare => "no_share",
            Self::Full => "full",
            Self::TooManyTries { .. } => "too_many",
            Self::Unreadable { .. } => "unreadable",
            Self::Other(_) => "other",
        }
    }
}

impl std::fmt::Display for ShareError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.text(i18n::current()))
    }
}

impl std::error::Error for ShareError {}

// MARK: - Format de l'image

/// L'appareil sait-il lire l'image de l'hôte ? `codec` : celui de la réponse
/// de `join` (absent : l'image n'a pas encore démarré, rien à vérifier).
/// Rend le format lu, ou [`ShareError::Unreadable`] avec son nom
/// (« HEVC », « HEVC 4:4:4 », « AV1 »).
pub fn check_codec(codec: Option<&str>, yuv444: bool, caps: Capabilities) -> Result<Option<Codec>, ShareError> {
    let Some(raw) = codec.map(str::trim).filter(|c| !c.is_empty()) else { return Ok(None) };
    let Some(codec) = Codec::parse(raw) else {
        return Err(ShareError::Unreadable { format: raw.to_ascii_uppercase() });
    };
    let readable = match codec {
        Codec::H264 => true,
        Codec::Hevc => caps.hevc && (!yuv444 || caps.hevc444),
        Codec::Av1 => caps.av1,
    };
    if readable {
        return Ok(Some(codec));
    }
    let format = if codec == Codec::Hevc && yuv444 && caps.hevc { format!("{} 4:4:4", codec.label()) } else { codec.label().to_string() };
    Err(ShareError::Unreadable { format })
}

// MARK: - Code et lien

/// Les six chiffres d'un code tapé (« 123 456 », « 123-456 ») ; `None` s'il
/// n'y en a pas exactement six.
pub fn normalize_code(text: &str) -> Option<String> {
    let digits: String = text.chars().filter(|c| !c.is_whitespace() && *c != '-' && *c != '.').collect();
    (digits.len() == 6 && digits.chars().all(|c| c.is_ascii_digit())).then_some(digits)
}

/// Le code à afficher : « 123 456 » (tel quel s'il n'a pas six chiffres).
pub fn format_code(code: &str) -> String {
    match normalize_code(code) {
        Some(d) => format!("{} {}", &d[..3], &d[3..]),
        None => code.to_string(),
    }
}

/// Une Station à qui envoyer le code : l'adresse et le port d'Admin, et son
/// empreinte quand on la connaît (ordinateur de l'appareil, lien).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShareTarget {
    pub host: String,
    pub port: u16,
    #[serde(default)]
    pub fingerprint: Option<String>,
}

/// Le lien d'un partage : `kybernow://join?host=…&port=…&fp=…&code=…`, ou
/// celui du client web `https://<adresse>:<port>/link/?join=…` (sans
/// empreinte : le certificat est accepté au premier contact).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JoinLink {
    pub host: String,
    pub port: u16,
    pub fingerprint: Option<String>,
    pub code: String,
}

impl JoinLink {
    pub fn target(&self) -> ShareTarget {
        ShareTarget { host: self.host.clone(), port: self.port, fingerprint: self.fingerprint.clone() }
    }
}

/// Lit un lien de partage (ouvert, scanné, collé) ; `None` s'il n'en est pas un.
pub fn parse_link(url: &str) -> Option<JoinLink> {
    let url = url.trim();
    let lower = url.to_ascii_lowercase();
    if let Some(rest) = lower.strip_prefix("kybernow://join").map(|_| &url["kybernow://join".len()..]) {
        let query = rest.trim_start_matches('/').strip_prefix('?')?;
        let pairs = query_pairs(query);
        let get = |k: &str| pairs.iter().find(|(key, _)| key == k).map(|(_, v)| v.clone());
        let host = strip_brackets(&get("host")?).to_string();
        let port = get("port")?.parse::<u16>().ok().filter(|p| *p > 0)?;
        let code = normalize_code(&get("code")?)?;
        let fingerprint = get("fp").map(|f| f.trim().to_string()).filter(|f| !f.is_empty());
        if host.is_empty() {
            return None;
        }
        return Some(JoinLink { host, port, fingerprint, code });
    }
    // Lien du client web.
    let rest = lower.strip_prefix("https://").map(|_| &url["https://".len()..])?;
    let (authority, path) = rest.split_once('/').unwrap_or((rest, ""));
    let (host, port) = split_authority(authority)?;
    let query = path.split_once('?').map(|(p, q)| (p.trim_end_matches('/'), q))?;
    if !query.0.eq_ignore_ascii_case("link") {
        return None;
    }
    let pairs = query_pairs(query.1.split('#').next().unwrap_or_default());
    let code = normalize_code(&pairs.iter().find(|(k, _)| k == "join")?.1)?;
    Some(JoinLink { host, port, fingerprint: None, code })
}

fn strip_brackets(host: &str) -> &str {
    host.strip_prefix('[').and_then(|h| h.strip_suffix(']')).unwrap_or(host)
}

/// `adresse:port`, une adresse IPv6 entre crochets.
fn split_authority(authority: &str) -> Option<(String, u16)> {
    let (host, port) = if let Some(rest) = authority.strip_prefix('[') {
        let (h, after) = rest.split_once(']')?;
        (h, after.strip_prefix(':')?)
    } else {
        authority.rsplit_once(':')?
    };
    let port = port.parse::<u16>().ok().filter(|p| *p > 0)?;
    (!host.is_empty()).then(|| (host.to_string(), port))
}

fn query_pairs(query: &str) -> Vec<(String, String)> {
    query
        .split('&')
        .filter(|p| !p.is_empty())
        .map(|p| {
            let (k, v) = p.split_once('=').unwrap_or((p, ""));
            (percent_decode(k), percent_decode(v))
        })
        .collect()
}

fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'%' if i + 2 < bytes.len() => {
                let hex = std::str::from_utf8(&bytes[i + 1..i + 3]).ok().and_then(|h| u8::from_str_radix(h, 16).ok());
                match hex {
                    Some(b) => {
                        out.push(b);
                        i += 3;
                    }
                    None => {
                        out.push(b'%');
                        i += 1;
                    }
                }
            }
            b'+' => {
                out.push(b' ');
                i += 1;
            }
            b => {
                out.push(b);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

// MARK: - Rejoindre

/// La Station qui a accepté le code, et sa réponse.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Found {
    /// Son adresse, avec l'empreinte d'Admin vue pendant l'appel (la session
    /// de l'invité l'épingle : `SessionParams::console_fingerprint`).
    pub target: ShareTarget,
    pub joined: ShareJoined,
}

/// Délai d'un essai auprès d'une Station.
const JOIN_TIMEOUT: Duration = Duration::from_secs(6);

/// Envoie le code à chacune de ces Stations à la fois (celles que l'app
/// trouve sur le réseau, comme pour l'appairage, ou celle du lien) et rend
/// la première qui l'accepte. Sinon : [`ShareError::Full`] si la Station du
/// code est complète, [`ShareError::TooManyTries`] si une Station a limité
/// les essais, [`ShareError::NoShare`] si aucune n'a ce code (ou si aucune
/// ne répond, ou qu'il n'y en a pas) ; une seule Station essayée : son
/// erreur telle quelle (Admin injoignable, certificat changé). Bloquant
/// (6 s au plus).
pub fn join(targets: &[ShareTarget], req: &JoinRequest) -> Result<Found, ShareError> {
    join_with(targets, req, |t, r| {
        let console = Console::guest(&t.host, t.port, t.fingerprint.clone());
        let joined = console.share_join(r)?;
        let fingerprint = t.fingerprint.clone().or_else(|| console.fingerprint_seen());
        Ok(Found { target: ShareTarget { fingerprint, ..t.clone() }, joined })
    })
}

/// [`join`], puis la vérification du format de l'image avec ce que
/// l'appareil décode ([`check_codec`]) : s'il ne le lit pas, l'invité repart
/// tout de suite (`share_leave`) et l'appel rend [`ShareError::Unreadable`].
pub fn join_checked(targets: &[ShareTarget], req: &JoinRequest, caps: Capabilities) -> Result<Found, ShareError> {
    let found = join(targets, req)?;
    if let Err(e) = check_codec(found.joined.codec.as_deref(), found.joined.yuv444, caps) {
        leave(&found);
        return Err(e);
    }
    Ok(found)
}

/// L'invité repart avant d'avoir ouvert sa session (format illisible,
/// l'utilisateur annule) ; la session, elle, part seule.
pub fn leave(found: &Found) {
    let t = &found.target;
    let console = Console::guest(&t.host, t.port, t.fingerprint.clone());
    if let Err(e) = console.share_leave(&found.joined.guest, &found.joined.secret) {
        log::warn!("partage : départ non confirmé ({})", e.text(Lang::Fr));
    }
}

/// [`join`] avec l'appel substitué (tests).
fn join_with<F>(targets: &[ShareTarget], req: &JoinRequest, call: F) -> Result<Found, ShareError>
where
    F: Fn(&ShareTarget, &JoinRequest) -> Result<Found, ShareError> + Send + Sync + 'static + Clone,
{
    let mut seen: Vec<&ShareTarget> = Vec::new();
    for t in targets {
        if !seen.iter().any(|s| s.host == t.host && s.port == t.port) {
            seen.push(t);
        }
    }
    if seen.is_empty() {
        return Err(ShareError::NoShare);
    }
    let single = seen.len() == 1;
    let (tx, rx) = mpsc::channel();
    for t in &seen {
        let (t, r, call, tx) = ((*t).clone(), req.clone(), call.clone(), tx.clone());
        let _ = std::thread::Builder::new().name("kybernow-partage".into()).spawn(move || {
            let _ = tx.send(call(&t, &r));
        });
    }
    drop(tx);
    let deadline = std::time::Instant::now() + JOIN_TIMEOUT + Duration::from_secs(1);
    let mut errors: Vec<ShareError> = Vec::new();
    while errors.len() < seen.len() {
        match rx.recv_timeout(deadline.saturating_duration_since(std::time::Instant::now())) {
            Ok(Ok(found)) => return Ok(found),
            Ok(Err(e)) => errors.push(e),
            Err(_) => break,
        }
    }
    if let Some(i) = errors.iter().position(|e| matches!(e, ShareError::Full)) {
        return Err(errors.swap_remove(i));
    }
    if let Some(i) = errors.iter().position(|e| matches!(e, ShareError::TooManyTries { .. })) {
        return Err(errors.swap_remove(i));
    }
    if single && let Some(e) = errors.pop() {
        return Err(e);
    }
    Err(ShareError::NoShare)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roles() {
        for (s, r, code) in [("spectator", ShareRole::Spectator, 0), ("player", ShareRole::Player, 1), ("player_kbm", ShareRole::PlayerKbm, 2), ("kbm", ShareRole::Kbm, 3)] {
            assert_eq!(ShareRole::parse(s), Some(r));
            assert_eq!(serde_json::to_value(r).unwrap(), serde_json::json!(s));
            assert_eq!(serde_json::from_value::<ShareRole>(serde_json::json!(s)).unwrap(), r);
            assert_eq!((r.code(), ShareRole::from_code(code)), (code, Some(r)));
            assert_eq!(ShareRole::of(r.pads(), r.kbm()), r);
        }
        assert_eq!(ShareRole::from_code(4), None);
        // Inconnu ou absent : spectateur, rien ne part.
        assert_eq!(serde_json::from_value::<ShareRole>(serde_json::json!("admin")).unwrap(), ShareRole::Spectator);
        assert_eq!(serde_json::from_value::<ShareRole>(serde_json::Value::Null).unwrap(), ShareRole::Spectator);
        assert_eq!(ShareRole::Spectator.rights(), InputRights::NONE);
        assert_eq!(ShareRole::Player.rights(), InputRights { gamepads: true, keyboard_mouse: false, clipboard: false });
        assert_eq!(ShareRole::PlayerKbm.rights(), InputRights { gamepads: true, keyboard_mouse: true, clipboard: false });
        assert_eq!(ShareRole::Kbm.rights(), InputRights { gamepads: false, keyboard_mouse: true, clipboard: false }, "le clavier et la souris sans les manettes");
        // « Joueur » : un droit au moins.
        assert!(!ShareRole::Spectator.plays() && ShareRole::Player.plays() && ShareRole::Kbm.plays() && ShareRole::PlayerKbm.plays());
        assert_eq!(
            [ShareRole::Spectator, ShareRole::Player, ShareRole::Kbm, ShareRole::PlayerKbm].map(|r| (r.pads(), r.kbm())),
            [(false, false), (true, false), (false, true), (true, true)]
        );
        // Deux droits indépendants : chacun donné ou repris sans toucher à l'autre.
        assert_eq!(ShareRole::Spectator.with_kbm(true), ShareRole::Kbm);
        assert_eq!(ShareRole::Kbm.with_pads(true), ShareRole::PlayerKbm);
        assert_eq!(ShareRole::PlayerKbm.with_pads(false), ShareRole::Kbm);
        assert_eq!(ShareRole::PlayerKbm.with_kbm(false), ShareRole::Player);
        assert_eq!(ShareRole::Player.with_pads(false), ShareRole::Spectator);
        assert_eq!(ShareRole::Kbm.with_kbm(false), ShareRole::Spectator);
        assert_eq!(ShareRole::Player.with_pads(true), ShareRole::Player);
    }

    #[test]
    fn reponse_de_join() {
        let body = r#"{"guest":"g1","secret":"s3cr3t","role":"spectator","host":"Matthieu","computer":"PC-ATELIER",
            "kyber":{"port":12345,"fingerprint":"sha256:aa:bb","token":"eyJ.x.y","expires":120},
            "display":{"id":2221634456,"width":1920,"height":1080,"refresh":60},"codec":"hevc","yuv444":false,"beat":5}"#;
        let j: ShareJoined = serde_json::from_str(body).unwrap();
        assert_eq!((j.guest.as_str(), j.role, j.host_name()), ("g1", ShareRole::Spectator, "Matthieu"));
        assert_eq!(j.kyber.port, 12345);
        assert_eq!(j.display, ShareDisplay { id: 2_221_634_456, width: 1920, height: 1080, refresh: 60 });
        assert_eq!(j.codec(), Some(Codec::Hevc));
        // Repassée telle quelle par les ponts.
        let again: ShareJoined = serde_json::from_value(serde_json::to_value(&j).unwrap()).unwrap();
        assert!(again == j);
        // Ni le jeton ni le secret dans les journaux.
        let debug = format!("{j:?}");
        assert!(!debug.contains("s3cr3t") && !debug.contains("eyJ"), "{debug}");
        // Image pas encore démarrée, partage lancé depuis le PC.
        let j: ShareJoined = serde_json::from_str(r#"{"guest":"g","secret":"s","role":"player","host":"","computer":"Bureau","codec":null}"#).unwrap();
        assert_eq!((j.codec(), j.host_name(), j.beat), (None, "Bureau", 5));
    }

    #[test]
    fn signe_de_vie_de_l_invite() {
        let b: ShareBeat = serde_json::from_str(r#"{"role":"player_kbm","token":"t","expires":120,"end":null}"#).unwrap();
        assert_eq!((b.role, b.end.clone(), b.token.as_deref()), (ShareRole::PlayerKbm, None, Some("t")));
        let b: ShareBeat = serde_json::from_str(r#"{"role":"spectator","end":"stopped"}"#).unwrap();
        assert_eq!(b.end, Some(ShareEnd::Stopped));
        let b: ShareBeat = serde_json::from_str(r#"{"role":"kbm","token":"t"}"#).unwrap();
        assert_eq!(b.role, ShareRole::Kbm, "le clavier et la souris seuls");
        let b: ShareBeat = serde_json::from_str(r#"{"role":"spectator","end":"removed"}"#).unwrap();
        assert_eq!(b.end, Some(ShareEnd::Removed));
        // Une fin inconnue finit quand même le partage.
        let b: ShareBeat = serde_json::from_str(r#"{"end":"closed"}"#).unwrap();
        assert_eq!(b.end, Some(ShareEnd::Stopped));
        // Pendant 60 s après un retrait : la fin, sans jeton.
        let b: ShareBeat = serde_json::from_str(r#"{"role":"player","token":null,"expires":0,"end":"removed"}"#).unwrap();
        assert_eq!((b.end, b.token, b.expires), (Some(ShareEnd::Removed), None, Some(0)));
        // L'écran partagé, toujours donné.
        let b: ShareBeat = serde_json::from_str(r#"{"role":"spectator","token":"t","display":{"id":9,"width":2560,"height":1440,"refresh":144}}"#).unwrap();
        assert_eq!(b.display, Some(ShareDisplay { id: 9, width: 2560, height: 1440, refresh: 144 }));
    }

    #[test]
    fn textes_des_fins() {
        assert_eq!(ShareEnd::Stopped.text(Lang::Fr, "Léa"), "Léa a arrêté le partage.");
        assert_eq!(ShareEnd::Removed.text(Lang::Fr, "Léa"), "Léa vous a retiré du partage.");
        assert_eq!(ShareEnd::Stopped.text(Lang::En, "Léa"), "Léa stopped sharing.");
        assert_eq!(ShareEnd::Removed.text(Lang::En, "Léa"), "Léa removed you from the share.");
        let u = ShareEnd::Unreadable { format: "HEVC".into() };
        assert_eq!(u.text(Lang::Fr, "Léa"), "Cet appareil ne lit pas l'image de cet ordinateur (HEVC).");
        assert_eq!(u.text(Lang::En, "Léa"), "This device cannot play this computer's picture (HEVC).");
        assert_eq!(ShareError::NoShare.text(Lang::Fr), "Aucun partage avec ce code sur le réseau.");
        assert_eq!(ShareError::Full.text(Lang::En), "This share is full (four guests at most).");
        assert_eq!(ShareAvailability::Unsupported.text(Lang::Fr).unwrap(), "Le partage demande KyberNow Station 0.8.0 ou plus récente.");
        assert_eq!(ShareAvailability::Disabled.text(Lang::En).unwrap(), "Sharing is turned off on this computer (Admin › Link devices).");
    }

    #[test]
    fn partage_annonce_par_info() {
        let old: Info = serde_json::from_str(r#"{"hostname":"Bureau"}"#).unwrap();
        assert_eq!(ShareAvailability::of(&old), ShareAvailability::Unsupported);
        let off: Info = serde_json::from_str(r#"{"share":{"available":true,"enabled":false}}"#).unwrap();
        assert_eq!(ShareAvailability::of(&off), ShareAvailability::Disabled);
        let on: Info = serde_json::from_str(r#"{"share":{"available":true,"enabled":true}}"#).unwrap();
        assert_eq!(ShareAvailability::of(&on), ShareAvailability::Available);
        assert_eq!(serde_json::to_value(&on).unwrap()["share"], serde_json::json!({ "available": true, "enabled": true }));
    }

    #[test]
    fn liste_des_invites() {
        let s: ShareState = serde_json::from_str(
            r#"{"open":true,"code":"123456","link":"kybernow://join?host=a&port=1&code=123456","web":"https://a:1/link/?join=123456",
            "display":{"id":7,"label":"Écran 1"},
            "guests":[{"id":"g1","name":"Léa","app":"ios","role":"spectator","since":1761500000},
                      {"id":"g2","name":"Tom","app":"webos","role":"player","since":"hier"},
                      42]}"#,
        )
        .unwrap();
        assert_eq!(s.guests.len(), 2, "une entrée illisible est écartée");
        assert_eq!((s.guests[0].role, s.guests[0].since), (ShareRole::Spectator, Some(1_761_500_000)));
        assert_eq!((s.guests[1].role, s.guests[1].since), (ShareRole::Player, None), "horodatage illisible");
        assert_eq!(s.display.as_ref().unwrap().label, "Écran 1");
    }

    #[test]
    fn formats_lisibles() {
        let all = Capabilities { hevc: true, hevc444: true, av1: true };
        let h264_only = Capabilities::default();
        assert_eq!(check_codec(None, false, h264_only).unwrap(), None, "image pas encore démarrée");
        assert_eq!(check_codec(Some("h264"), false, h264_only).unwrap(), Some(Codec::H264));
        assert_eq!(check_codec(Some("hevc"), true, all).unwrap(), Some(Codec::Hevc));
        let e = check_codec(Some("hevc"), false, h264_only).unwrap_err();
        assert!(matches!(&e, ShareError::Unreadable { format } if format == "HEVC"), "{e:?}");
        let no444 = Capabilities { hevc: true, hevc444: false, av1: false };
        assert!(matches!(check_codec(Some("hevc"), true, no444), Err(ShareError::Unreadable { format }) if format == "HEVC 4:4:4"));
        assert!(matches!(check_codec(Some("av1"), false, no444), Err(ShareError::Unreadable { format }) if format == "AV1"));
        assert!(matches!(check_codec(Some("vp9"), false, all), Err(ShareError::Unreadable { format }) if format == "VP9"));
    }

    #[test]
    fn codes() {
        assert_eq!(normalize_code("123 456").as_deref(), Some("123456"));
        assert_eq!(normalize_code(" 123-456 ").as_deref(), Some("123456"));
        assert_eq!(normalize_code("12345"), None);
        assert_eq!(normalize_code("1234567"), None);
        assert_eq!(normalize_code("12a456"), None);
        assert_eq!(format_code("123456"), "123 456");
        assert_eq!(format_code("12"), "12");
    }

    #[test]
    fn liens() {
        let l = parse_link("kybernow://join?host=192.168.1.20&port=24811&fp=sha256%3Aaa%3Abb&code=123456").unwrap();
        assert_eq!(l, JoinLink { host: "192.168.1.20".into(), port: 24811, fingerprint: Some("sha256:aa:bb".into()), code: "123456".into() });
        assert_eq!(l.target(), ShareTarget { host: "192.168.1.20".into(), port: 24811, fingerprint: Some("sha256:aa:bb".into()) });
        // Empreinte non encodée, ordre quelconque, IPv6 entre crochets.
        let l = parse_link("kybernow://join/?code=123%20456&fp=sha256:cc&port=9443&host=[fe80::1]").unwrap();
        assert_eq!((l.host.as_str(), l.port, l.fingerprint.as_deref(), l.code.as_str()), ("fe80::1", 9443, Some("sha256:cc"), "123456"));
        // Lien du client web : sans empreinte.
        let l = parse_link("https://192.168.1.20:24811/link/?join=123456").unwrap();
        assert_eq!(l, JoinLink { host: "192.168.1.20".into(), port: 24811, fingerprint: None, code: "123456".into() });
        let l = parse_link("https://[fe80::1]:9443/link?join=654321#x").unwrap();
        assert_eq!((l.host.as_str(), l.port, l.code.as_str()), ("fe80::1", 9443, "654321"));
        for bad in [
            "kybernow://join?host=a&port=0&code=123456",
            "kybernow://join?host=a&port=1&code=12345",
            "kybernow://join?port=1&code=123456",
            "kybernow://pair?host=a&port=1&code=123456",
            "https://a:1/admin/?join=123456",
            "https://a/link/?join=123456",
            "bonjour",
        ] {
            assert_eq!(parse_link(bad), None, "{bad}");
        }
        assert_eq!(percent_decode("a%2"), "a%2");
        assert_eq!(percent_decode("%zz"), "%zz");
    }

    fn target(host: &str) -> ShareTarget {
        ShareTarget { host: host.into(), port: 9443, fingerprint: None }
    }

    fn request() -> JoinRequest {
        JoinRequest { code: "123456".into(), name: "Léa".into(), device: "ipad-1".into(), app: "ios".into() }
    }

    #[test]
    fn la_station_du_code_repond() {
        let call = |t: &ShareTarget, _: &JoinRequest| match t.host.as_str() {
            "b" => Ok(Found { target: ShareTarget { fingerprint: Some("sha256:bb".into()), ..t.clone() }, joined: ShareJoined::default() }),
            "c" => Err(ShareError::Other(ConsoleError::Network("hors ligne".into()))),
            _ => Err(ShareError::NoShare),
        };
        let found = join_with(&[target("a"), target("b"), target("c"), target("a")], &request(), call).unwrap();
        assert_eq!(found.target.host, "b");
        assert_eq!(found.target.fingerprint.as_deref(), Some("sha256:bb"), "l'empreinte d'Admin vue pendant l'appel");
        // Personne n'a ce code, ou personne à qui demander.
        assert!(matches!(join_with(&[target("a"), target("c")], &request(), call), Err(ShareError::NoShare)));
        assert!(matches!(join_with(&[], &request(), call), Err(ShareError::NoShare)));
        // La Station du code est complète, ou une Station limite les essais.
        let full = |t: &ShareTarget, _: &JoinRequest| if t.host == "b" { Err(ShareError::Full) } else { Err(ShareError::TooManyTries { seconds: Some(30) }) };
        assert!(matches!(join_with(&[target("a"), target("b")], &request(), full), Err(ShareError::Full)));
        assert!(matches!(join_with(&[target("a")], &request(), full), Err(ShareError::TooManyTries { seconds: Some(30) })));
        // Une seule Station (le lien) : son erreur telle quelle.
        let offline = |_: &ShareTarget, _: &JoinRequest| Err(ShareError::Other(ConsoleError::Network("hors ligne".into())));
        assert!(matches!(join_with(&[target("a")], &request(), offline), Err(ShareError::Other(_))));
    }
}
