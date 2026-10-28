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
#[cfg(test)]
use std::sync::Arc;
use std::time::Duration;

use serde::{Deserialize, Deserializer, Serialize};

use crate::console::{Console, ConsoleError, Info};
use crate::i18n::{self, Lang, tr};
use crate::input::InputRights;
use crate::profile::{Capabilities, Codec};
use crate::relay::{RelayAddr, RelayError, RelayRoute};
use crate::screens::DeviceScreen;

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
    /// L'image qu'il reçoit (Station 0.8 avec le correctif 0020 ; absente
    /// sinon), que l'hôte voit sans la changer.
    #[serde(default, deserialize_with = "lenient_image")]
    pub image: Option<ShareImage>,
}

/// Une image illisible est ignorée, pas tout ce qui la porte.
fn lenient_image<'de, D: Deserializer<'de>>(d: D) -> Result<Option<ShareImage>, D::Error> {
    let v = Option::<serde_json::Value>::deserialize(d)?;
    Ok(v.filter(|v| v.is_object()).and_then(|v| serde_json::from_value(v).ok()))
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
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct JoinRequest {
    /// Six chiffres ([`normalize_code`]).
    pub code: String,
    /// Le prénom de l'invité (proposé : le nom de l'appareil).
    pub name: String,
    /// L'identifiant de l'appareil (`DeviceInfo::id`).
    pub device: String,
    /// `mac`, `windows`, `linux`, `ios`, `tvos`, `android`, `webos`.
    pub app: String,
    /// « Qualité de l'image » choisie (retenue par l'app ; « Auto » par défaut).
    #[serde(default)]
    pub quality: ShareQuality,
    /// Pour « Auto » : ce que l'appareil sait afficher et décoder
    /// ([`auto_quality`]).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auto: Option<ShareAuto>,
}

// MARK: - Qualité de l'image d'un invité

/// La « Qualité de l'image » qu'un invité demande (docs/link-ui.md). L'image
/// reçue n'est jamais au-dessus de celle de l'hôte ; la Station (Kyber,
/// correctif 0020) décide et le dit ([`ShareImage`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum ShareQuality {
    /// D'après l'écran de l'appareil et ce que son décodeur suit ([`ShareAuto`]).
    #[default]
    Auto,
    /// « Comme l'hôte ».
    Host,
    /// « 4K · 60 Hz ».
    P2160_60,
    /// « 1080p · 60 Hz ».
    P1080_60,
    /// « 720p · 30 Hz ».
    P720_30,
}

impl ShareQuality {
    /// Dans l'ordre du choix (et des menus).
    pub const ALL: [ShareQuality; 5] = [Self::Auto, Self::Host, Self::P2160_60, Self::P1080_60, Self::P720_30];

    /// Nom dans l'échange avec Admin : `auto`, `host`, `2160p60`, `1080p60`, `720p30`.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Host => "host",
            Self::P2160_60 => "2160p60",
            Self::P1080_60 => "1080p60",
            Self::P720_30 => "720p30",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "auto" => Some(Self::Auto),
            "host" => Some(Self::Host),
            "2160p60" => Some(Self::P2160_60),
            "1080p60" => Some(Self::P1080_60),
            "720p30" => Some(Self::P720_30),
            _ => None,
        }
    }

    /// Pour les ponts C et JNI : 0 Auto, 1 comme l'hôte, 2 1080p60, 3 720p30,
    /// 4 2160p60 (ajouté après : les codes ne suivent pas l'ordre de [`Self::ALL`]).
    pub fn code(self) -> i32 {
        match self {
            Self::Auto => 0,
            Self::Host => 1,
            Self::P1080_60 => 2,
            Self::P720_30 => 3,
            Self::P2160_60 => 4,
        }
    }

    pub fn from_code(code: i32) -> Option<Self> {
        Self::ALL.into_iter().find(|q| q.code() == code)
    }

    /// Le libellé du choix : « Auto », « Comme l'hôte », « 4K · 60 Hz »,
    /// « 1080p · 60 Hz », « 720p · 30 Hz ».
    pub fn label(self, lang: Lang) -> String {
        match self {
            Self::Auto => tr(lang, "share.quality_auto", &[]),
            Self::Host => tr(lang, "share.quality_host", &[]),
            Self::P2160_60 => quality_label(2160, 60),
            Self::P1080_60 => quality_label(1080, 60),
            Self::P720_30 => quality_label(720, 30),
        }
    }
}

/// « 1080p · 60 Hz » ; 2160 lignes se disent « 4K » (« 4K · 60 Hz »).
fn quality_label(height: u32, fps: u32) -> String {
    match height {
        2160 => format!("4K · {fps} Hz"),
        h => format!("{h}p · {fps} Hz"),
    }
}

impl Serialize for ShareQuality {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(self.as_str())
    }
}

/// Une qualité inconnue (Station plus récente) se lit « Auto ».
impl<'de> Deserialize<'de> for ShareQuality {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = Option::<String>::deserialize(d)?;
        Ok(s.as_deref().and_then(Self::parse).unwrap_or_default())
    }
}

/// Pour « Auto », ce que l'appareil sait afficher et décoder (`auto` de
/// `join`) : la Station en tire l'image de l'invité.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct ShareAuto {
    /// Définition au plus, en pixels, en paysage.
    #[serde(default)]
    pub width: u32,
    #[serde(default)]
    pub height: u32,
    /// Images par seconde au plus.
    #[serde(default)]
    pub fps: u32,
    /// Les formats que l'appareil décode (`h264`, `hevc`, `av1`).
    #[serde(default)]
    pub codecs: Vec<String>,
    /// Il décode le HEVC 4:4:4.
    #[serde(default)]
    pub yuv444: bool,
}

/// Ce que le décodeur de l'appareil sait faire, pour « Auto » : ses formats,
/// et la définition et la fréquence qu'il suit (0 : rien de connu au-delà de
/// l'écran).
/// En JSON (ponts) : `{"hevc", "hevc444", "av1", "max_width", "max_height",
/// "max_fps"}`, tous facultatifs.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct DecoderCaps {
    #[serde(flatten)]
    pub capabilities: Capabilities,
    pub max_width: u32,
    pub max_height: u32,
    pub max_fps: u32,
}

impl From<Capabilities> for DecoderCaps {
    fn from(capabilities: Capabilities) -> Self {
        Self { capabilities, ..Self::default() }
    }
}

/// « Auto » : la définition et la fréquence au plus celles de l'écran de
/// l'appareil (`screen` : sa définition native et sa fréquence) et de ce que
/// son décodeur suit, en paysage ; jamais au-dessus de l'image de l'hôte
/// quand on la connaît (`host`, l'écran partagé : en cours de session). Les
/// formats sont ceux du décodeur (H.264 toujours). Ce qu'on envoie à la
/// Station avec `quality: auto`.
pub fn auto_quality(screen: &DeviceScreen, decoder: &DecoderCaps, host: Option<&ShareDisplay>) -> ShareAuto {
    let limit = |v: u32, max: u32| if max > 0 { v.min(max) } else { v };
    let (long, short) = (screen.pixel_width.max(screen.pixel_height), screen.pixel_width.min(screen.pixel_height));
    let (max_long, max_short) = (decoder.max_width.max(decoder.max_height), decoder.max_width.min(decoder.max_height));
    let (mut width, mut height) = (limit(long, max_long), limit(short, max_short));
    let mut fps = limit(screen.refresh.max(1), decoder.max_fps);
    if let Some(h) = host.filter(|h| h.width > 0 && h.height > 0) {
        width = width.min(h.width.max(h.height));
        height = height.min(h.width.min(h.height));
        if h.refresh > 0 {
            fps = fps.min(h.refresh);
        }
    }
    let c = decoder.capabilities;
    let mut codecs = vec!["h264".to_string()];
    if c.hevc {
        codecs.push("hevc".into());
    }
    if c.av1 {
        codecs.push("av1".into());
    }
    ShareAuto { width, height, fps, codecs, yuv444: c.hevc && c.hevc444 }
}

/// Pourquoi l'image reçue est celle-là.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ShareImageReason {
    /// La qualité demandée.
    Asked,
    /// Bornée par l'image de l'hôte (jamais au-dessus).
    Capped,
    /// Plus d'image à part possible : l'image ouverte la plus proche, plus légère.
    Limit,
    /// En retard sur l'image de l'hôte : un cran plus bas.
    Lag,
}

impl ShareImageReason {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Asked => "asked",
            Self::Capped => "capped",
            Self::Limit => "limit",
            Self::Lag => "lag",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "asked" => Some(Self::Asked),
            "capped" => Some(Self::Capped),
            "limit" => Some(Self::Limit),
            "lag" => Some(Self::Lag),
            _ => None,
        }
    }
}

impl Serialize for ShareImageReason {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for ShareImageReason {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = String::deserialize(d)?;
        Self::parse(&s).ok_or_else(|| serde::de::Error::custom("raison inconnue"))
    }
}

/// Une valeur d'`image` : `null` tant que Kyber ne sait rien ; une valeur
/// d'un autre type (Station plus récente) se lit `None` aussi.
fn lenient<'de, D: Deserializer<'de>, T: serde::de::DeserializeOwned>(d: D) -> Result<Option<T>, D::Error> {
    let v = Option::<serde_json::Value>::deserialize(d)?;
    Ok(v.and_then(|v| serde_json::from_value(v).ok()))
}

/// L'image qu'un invité reçoit (`image` de `join`, du signe de vie et de
/// `POST /api/share/quality` ; `guests[].image` chez l'hôte). Toujours un
/// objet ; `quality` est la qualité demandée (« Auto » compris), les autres
/// champs sont `None` tant que Kyber ne sait rien (Admin les relit toutes
/// les 2 s au plus).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct ShareImage {
    /// La qualité demandée.
    #[serde(default)]
    pub quality: ShareQuality,
    #[serde(default, deserialize_with = "lenient")]
    pub width: Option<u32>,
    #[serde(default, deserialize_with = "lenient")]
    pub height: Option<u32>,
    #[serde(default, deserialize_with = "lenient")]
    pub fps: Option<u32>,
    #[serde(default, deserialize_with = "lenient")]
    pub codec: Option<String>,
    /// L'invité partage l'image de l'hôte (`Some(false)` : une image à part,
    /// à lui ou à ceux de même qualité ; `None` : pas encore su, compté comme
    /// l'image de l'hôte).
    #[serde(default, deserialize_with = "lenient")]
    pub follows_host: Option<bool>,
    #[serde(default, deserialize_with = "lenient")]
    pub reason: Option<ShareImageReason>,
}

impl ShareImage {
    /// Définition et fréquence reçues, quand Kyber les connaît.
    pub fn size(&self) -> Option<(u32, u32, u32)> {
        match (self.width, self.height, self.fps) {
            (Some(w), Some(h), Some(f)) if w > 0 && h > 0 && f > 0 => Some((w, h, f)),
            _ => None,
        }
    }

    /// L'invité reçoit une image à part (su : `follows_host` faux).
    pub fn own(&self) -> bool {
        self.follows_host == Some(false)
    }

    /// La qualité reçue, telle que la barre et le panneau de l'hôte la
    /// montrent : « 1080p · 60 Hz » ; `None` tant que Kyber ne la connaît pas.
    pub fn label(&self) -> Option<String> {
        self.size().map(|(w, h, f)| quality_label(h.min(w), f))
    }

    pub fn codec(&self) -> Option<Codec> {
        self.codec.as_deref().and_then(Codec::parse)
    }

    /// L'avis bref à montrer quand l'image devient celle-ci (`previous` :
    /// celle d'avant, `None` au départ), seulement sur ce qui est su :
    /// « Image allégée pour suivre. » quand elle descend vraiment (raison
    /// `lag`, et moins de pixels par seconde qu'avant, les deux tailles
    /// connues) ; « L'ordinateur ne peut pas faire une image de plus : vous
    /// recevez {qualité}. » (raison `limit`, taille connue, pas déjà dit
    /// pour cette image) ; rien sinon.
    pub fn notice(&self, previous: Option<&ShareImage>, lang: Lang) -> Option<String> {
        let (w, h, f) = self.size()?;
        let rate = |(w, h, f): (u32, u32, u32)| u64::from(w) * u64::from(h) * u64::from(f);
        match self.reason? {
            ShareImageReason::Lag => {
                let before = previous.and_then(ShareImage::size)?;
                (rate((w, h, f)) < rate(before)).then(|| tr(lang, "share.image_lag", &[]))
            }
            ShareImageReason::Limit => {
                let said = previous.is_some_and(|p| p.reason == Some(ShareImageReason::Limit) && p.size() == Some((w, h, f)));
                (!said).then(|| tr(lang, "share.image_limit", &[("quality", &quality_label(h.min(w), f))]))
            }
            _ => None,
        }
    }
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
    /// L'image que l'invité recevra, si la Station la dit déjà.
    #[serde(default, deserialize_with = "lenient_image")]
    pub image: Option<ShareImage>,
    /// Gardé par le cœur, pas dans la réponse d'Admin : le `auto` envoyé à
    /// `join` (ce que l'appareil sait afficher et décoder), que
    /// [`crate::session::Session::set_share_quality`] renvoie avec « Auto »
    /// (sans lui, Admin lit « Auto » comme l'image de l'hôte). [`join`] le
    /// met ici ; il voyage avec `joined` jusqu'à la session.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auto: Option<ShareAuto>,
    /// Gardé par le cœur, pas dans la réponse d'Admin : par où l'invité est
    /// entré, quand c'est par le relais ([`join_anywhere`], [`wait`]) ; sa
    /// session y passe aussi (Admin et Kyber par le tunnel).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub route: Option<RelayRoute>,
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
            .field("image", &self.image)
            .field("auto", &self.auto)
            .field("route", &self.route)
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
    /// L'image que l'invité reçoit maintenant.
    #[serde(default, deserialize_with = "lenient_image")]
    pub image: Option<ShareImage>,
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
            .field("image", &self.image)
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
    /// Par le relais : l'hôte a refusé (« L'hôte a refusé. » ; aussi un
    /// appareil refusé il y a moins de 10 minutes).
    Refused,
    /// Par le relais : pas de réponse de l'hôte en 120 s (« L'hôte n'a pas
    /// répondu. »).
    Expired,
    /// Par le relais : trois demandes attendent déjà l'accord de l'hôte.
    PendingFull,
    /// Le reste : Admin injoignable, certificat changé, réponse illisible,
    /// relais muet (`ConsoleError::Relay`).
    Other(ConsoleError),
}

impl ShareError {
    pub fn text(&self, lang: Lang) -> String {
        match self {
            Self::NoShare => tr(lang, "share.no_share", &[]),
            Self::Full => tr(lang, "share.full", &[]),
            Self::TooManyTries { .. } => tr(lang, "share.too_many", &[]),
            Self::Unreadable { format } => tr(lang, "share.unreadable", &[("format", format)]),
            Self::Refused => tr(lang, "share.refused", &[]),
            Self::Expired => tr(lang, "share.expired", &[]),
            Self::PendingFull => tr(lang, "share.pending_full", &[]),
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
            Self::Refused => "refused",
            Self::Expired => "expired",
            Self::PendingFull => "pending_full",
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

/// L'appareil sait-il lire l'image de l'hôte telle quelle ? `codec` : celui
/// de la réponse de `join` (absent : l'image n'a pas encore démarré, rien à
/// vérifier). Un non n'empêche plus d'entrer ([`guest_format`] : image à
/// part) ; « Comme l'hôte » et la barre peuvent s'en servir.
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

/// Le format qu'un invité demande à Kyber : celui de l'image de l'hôte
/// (`codec`, `yuv444` de `join`) quand l'appareil le lit (en 4:2:0 s'il ne
/// lit pas son 4:4:4 : Kyber lui fait alors une image à part) ; sinon un
/// format qu'il lit (HEVC, puis AV1, H.264 sinon), dans lequel Kyber lui fait
/// une image à part. `None` : l'image de l'hôte n'a pas démarré (le format
/// de l'app, en 4:2:0). [`ShareError::Unreadable`] seulement si l'appareil
/// ne lit aucun format possible.
pub fn guest_format(codec: Option<&str>, yuv444: bool, caps: Capabilities) -> Result<Option<(Codec, bool)>, ShareError> {
    let Some(raw) = codec.map(str::trim).filter(|c| !c.is_empty()) else { return Ok(None) };
    let reads = |c: Codec| match c {
        Codec::H264 => true,
        Codec::Hevc => caps.hevc,
        Codec::Av1 => caps.av1,
    };
    if let Some(c) = Codec::parse(raw).filter(|c| reads(*c)) {
        return Ok(Some((c, yuv444 && c == Codec::Hevc && caps.hevc444)));
    }
    match [Codec::Hevc, Codec::Av1, Codec::H264].into_iter().find(|c| reads(*c)) {
        Some(c) => Ok(Some((c, false))),
        None => Err(ShareError::Unreadable { format: Codec::parse(raw).map_or_else(|| raw.to_ascii_uppercase(), |c| c.label().to_string()) }),
    }
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
    /// Le relais de la Station (`relay=`, relais joint quand le lien a été fait).
    #[serde(default)]
    pub relay: Option<String>,
    /// L'identifiant de la Station chez ce relais (`sid=`).
    #[serde(default)]
    pub sid: Option<String>,
}

impl JoinLink {
    pub fn target(&self) -> ShareTarget {
        ShareTarget { host: self.host.clone(), port: self.port, fingerprint: self.fingerprint.clone() }
    }

    /// Par le relais, de bout en bout : la Station `sid`, Admin épinglé sur
    /// l'empreinte du lien (`fp`, venue de la Station : le relais ne peut pas
    /// s'interposer). `None` sans relais, identifiant ou empreinte.
    pub fn relay_route(&self) -> Option<RelayRoute> {
        let route = RelayRoute { server: self.relay.clone()?, station: self.sid.clone()?, admin_fp: self.fingerprint.clone()? };
        route.relay_addr().ok().map(|_| route)
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
        let relay = get("relay").map(|r| r.trim().to_string()).filter(|r| r.parse::<RelayAddr>().is_ok());
        let sid = get("sid").map(|s| s.trim().to_ascii_lowercase()).filter(|s| kybernow_tunnel::id::is_station_id(s));
        return Some(JoinLink { host, port, fingerprint, code, relay, sid });
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
    Some(JoinLink { host, port, fingerprint: None, code, relay: None, sid: None })
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

/// [`join`], puis la vérification du format avec ce que l'appareil décode
/// ([`guest_format`]) : un format de l'hôte qu'il ne lit pas n'empêche plus
/// d'entrer (Kyber lui fait une image à part dans un format qu'il lit) ;
/// seul un appareil qui ne lit aucun format possible repart tout de suite
/// (`share_leave`), avec [`ShareError::Unreadable`].
pub fn join_checked(targets: &[ShareTarget], req: &JoinRequest, caps: Capabilities) -> Result<Found, ShareError> {
    let found = join(targets, req)?;
    if let Err(e) = guest_format(found.joined.codec.as_deref(), found.joined.yuv444, caps) {
        leave(&found);
        return Err(e);
    }
    Ok(found)
}

/// L'invité repart avant d'avoir ouvert sa session (format illisible,
/// l'utilisateur annule) ; la session, elle, part seule.
pub fn leave(found: &Found) {
    let t = &found.target;
    let console = Console::guest(&t.host, t.port, t.fingerprint.clone()).with_relay(found.joined.route.clone());
    if let Err(e) = console.share_leave(&found.joined.guest, &found.joined.secret) {
        log::warn!("partage : départ non confirmé ({})", e.text(Lang::Fr));
    }
}

// MARK: - Rejoindre hors de chez soi (docs/relais.md)

/// Une demande par le relais, en attente de l'accord de l'hôte (« En
/// attente de l'accord de l'hôte… ») : à donner à [`wait`] jusqu'à la
/// réponse, ou à [`cancel`] (« Annuler »).
#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PendingJoin {
    pub route: RelayRoute,
    pub guest: String,
    pub secret: String,
    /// Sans réponse de l'hôte en ce temps (120 s), la demande expire.
    pub expires: u64,
    /// Le `auto` envoyé à `join`, gardé pour la session.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auto: Option<ShareAuto>,
}

impl Joining {
    /// La demande en attente, s'il y en a une.
    pub fn pending(self) -> Option<PendingJoin> {
        match self {
            Joining::Pending(p) => Some(p),
            Joining::Joined(_) => None,
        }
    }
}

impl std::fmt::Debug for PendingJoin {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PendingJoin").field("route", &self.route).field("guest", &self.guest).field("secret", &"[masqué]").field("expires", &self.expires).finish()
    }
}

/// L'issue de « Rejoindre » : l'invité entre, ou attend l'accord de l'hôte
/// (venu par le relais). En JSON (ponts) : `{"joined": <Found>}` ou
/// `{"pending": <PendingJoin>}`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Joining {
    Joined(Found),
    Pending(PendingJoin),
}

/// Une Station du réseau qui répond dans ce délai est prise avant le relais.
pub const LOCAL_FIRST: Duration = Duration::from_millis(1500);

/// « Rejoindre » sur le réseau et hors de chez soi : le code part aux
/// Stations du réseau ([`join`]) **et en même temps** aux relais (`GET
/// /v1/codes/{code}`, le relais par défaut et ceux des ordinateurs que l'app
/// connaît, `info.relay.server`). Une Station du réseau qui l'accepte en
/// 1,5 s est prise ; sinon la Station que donne un relais, rejointe de bout
/// en bout par le tunnel, Admin épinglé sur l'empreinte que le relais
/// atteste (signature vérifiée) : l'invité entre, ou, d'ordinaire, attend
/// l'accord de l'hôte ([`Joining::Pending`]). Le format de l'image est
/// vérifié avec `caps` (comme [`join_checked`]). Bloquant (8 s au plus,
/// sans compter l'attente de l'hôte).
pub fn join_anywhere(targets: &[ShareTarget], relays: &[RelayAddr], req: &JoinRequest, caps: Option<Capabilities>) -> Result<Joining, ShareError> {
    let (t, r) = (targets.to_vec(), req.clone());
    let lookups: Vec<Box<dyn FnOnce() -> Result<Option<(RelayAddr, crate::relay::CodeLookup)>, RelayError> + Send>> = unique_relays(relays)
        .into_iter()
        .map(|relay| {
            let code = req.code.clone();
            Box::new(move || crate::relay::lookup(&relay, &code).map(|f| f.map(|f| (relay.clone(), f)))) as Box<dyn FnOnce() -> _ + Send>
        })
        .collect();
    let joining = join_anywhere_with(move || join(&t, &r), lookups, |route| relay_join(route, req), LOCAL_FIRST)?;
    checked(joining, caps)
}

/// Le lien d'un partage : la Station du réseau d'abord (`host`, `port`,
/// `fp`, 1,5 s), puis, si le lien porte `relay` et `sid`, par le relais de
/// bout en bout (Admin épinglé sur `fp`).
pub fn join_link(link: &JoinLink, req: &JoinRequest, caps: Option<Capabilities>) -> Result<Joining, ShareError> {
    let (target, r) = (link.target(), req.clone());
    let route = link.relay_route();
    let joining = join_link_with(move || join(&[target], &r), route, |route| relay_join(route, req), LOCAL_FIRST)?;
    checked(joining, caps)
}

/// L'hôte a-t-il répondu ? Un appel tenu 25 s au plus : `Ok(None)` (toujours
/// en attente : rappeler), `Ok(Some(found))` (accepté : la session d'invité
/// peut s'ouvrir, par le relais), ou le refus ([`ShareError::Refused`],
/// [`ShareError::Expired`], [`ShareError::NoShare`] si le partage s'arrête).
/// Le format est vérifié avec `caps` (l'invité repart s'il ne lit rien).
pub fn wait(pending: &PendingJoin, caps: Option<Capabilities>) -> Result<Option<Found>, ShareError> {
    let console = relay_console(&pending.route);
    match console.share_wait(&pending.guest, &pending.secret)? {
        JoinAnswerRef::Pending { .. } => Ok(None),
        JoinAnswerRef::Joined(mut joined) => {
            joined.route = Some(pending.route.clone());
            joined.auto = pending.auto.clone();
            let found = Found { target: relay_target(&pending.route), joined };
            match checked(Joining::Joined(found), caps)? {
                Joining::Joined(f) => Ok(Some(f)),
                Joining::Pending(_) => Ok(None),
            }
        }
    }
}

/// « Annuler » une demande en attente (`share_leave`).
pub fn cancel(pending: &PendingJoin) {
    if let Err(e) = relay_console(&pending.route).share_leave(&pending.guest, &pending.secret) {
        log::warn!("partage : demande pas annulée ({})", e.text(Lang::Fr));
    }
}

use crate::console::JoinAnswer as JoinAnswerRef;

/// Les relais à interroger, sans doublon (le relais par défaut si la liste est vide).
fn unique_relays(relays: &[RelayAddr]) -> Vec<RelayAddr> {
    let mut out: Vec<RelayAddr> = Vec::new();
    for r in relays.iter().cloned().chain(relays.is_empty().then(RelayAddr::public)) {
        if !out.iter().any(|o| o.host == r.host && o.port == r.port) {
            out.push(r);
        }
    }
    out
}

/// La console vers Admin d'une Station par le relais, sans compte.
fn relay_console(route: &RelayRoute) -> Console {
    Console::guest(&route.station, 0, Some(route.admin_fp.clone())).with_relay(Some(route.clone()))
}

/// La « Station » d'une session venue par le relais : son identifiant, son
/// empreinte (l'adresse ne sert pas : tout passe par le tunnel).
fn relay_target(route: &RelayRoute) -> ShareTarget {
    ShareTarget { host: route.station.clone(), port: 0, fingerprint: Some(route.admin_fp.clone()) }
}

/// `join` de bout en bout par le relais.
fn relay_join(route: RelayRoute, req: &JoinRequest) -> Result<Joining, ShareError> {
    log::info!("partage : code envoyé par le relais {} à {}", route.server, route.station);
    match relay_console(&route).share_join_answer(req)? {
        JoinAnswerRef::Joined(mut joined) => {
            joined.auto = req.auto.clone();
            joined.route = Some(route.clone());
            Ok(Joining::Joined(Found { target: relay_target(&route), joined }))
        }
        JoinAnswerRef::Pending { guest, secret, expires } => Ok(Joining::Pending(PendingJoin { route, guest, secret, expires, auto: req.auto.clone() })),
    }
}

/// Le format vérifié (comme [`join_checked`]) pour un invité qui entre.
fn checked(joining: Joining, caps: Option<Capabilities>) -> Result<Joining, ShareError> {
    let (Joining::Joined(found), Some(caps)) = (&joining, caps) else { return Ok(joining) };
    if let Err(e) = guest_format(found.joined.codec.as_deref(), found.joined.yuv444, caps) {
        leave(found);
        return Err(e);
    }
    Ok(joining)
}

type Lookup = Box<dyn FnOnce() -> Result<Option<(RelayAddr, crate::relay::CodeLookup)>, RelayError> + Send>;

enum Heard {
    Local(Result<Found, ShareError>),
    Lookup(Result<Option<(RelayAddr, crate::relay::CodeLookup)>, RelayError>),
}

/// [`join_anywhere`] avec les appels substitués (tests).
fn join_anywhere_with(
    local: impl FnOnce() -> Result<Found, ShareError> + Send + 'static,
    lookups: Vec<Lookup>,
    relay_join: impl FnOnce(RelayRoute) -> Result<Joining, ShareError>,
    local_first: Duration,
) -> Result<Joining, ShareError> {
    let (tx, rx) = mpsc::channel();
    let started = std::time::Instant::now();
    {
        let tx = tx.clone();
        let _ = std::thread::Builder::new().name("kybernow-partage".into()).spawn(move || {
            let _ = tx.send(Heard::Local(local()));
        });
    }
    let mut pending_lookups = lookups.len();
    for lookup in lookups {
        let tx = tx.clone();
        let _ = std::thread::Builder::new().name("kybernow-relais-code".into()).spawn(move || {
            let _ = tx.send(Heard::Lookup(lookup()));
        });
    }
    drop(tx);
    let overall = started + JOIN_TIMEOUT + Duration::from_secs(2);
    let mut local_error: Option<ShareError> = None;
    let mut local_done = false;
    let mut remote: Option<(RelayAddr, crate::relay::CodeLookup)> = None;
    let mut relay_error: Option<RelayError> = None;
    loop {
        // Une Station du réseau a répondu non, ou 1,5 s ont passé : la Station du relais.
        if remote.is_some() && (local_done || std::time::Instant::now() >= started + local_first) {
            break;
        }
        if local_done && pending_lookups == 0 {
            break;
        }
        let until = if remote.is_some() { started + local_first } else { overall };
        match rx.recv_timeout(until.saturating_duration_since(std::time::Instant::now())) {
            Ok(Heard::Local(Ok(found))) => return Ok(Joining::Joined(found)),
            Ok(Heard::Local(Err(e))) => {
                local_done = true;
                local_error = Some(e);
            }
            Ok(Heard::Lookup(r)) => {
                pending_lookups -= 1;
                match r {
                    Ok(Some(found)) if remote.is_none() => remote = Some(found),
                    Ok(_) => {}
                    Err(e) => relay_error = Some(e),
                }
            }
            Err(_) if remote.is_some() => break,
            Err(_) => {
                local_done = true;
                pending_lookups = 0;
            }
        }
    }
    if let Some((relay, found)) = remote {
        // La Station de ce code n'est pas chez le relais en ce moment.
        if !found.online {
            return Err(ShareError::Other(ConsoleError::Relay(RelayError::StationOffline)));
        }
        let route = RelayRoute { server: relay.to_string(), station: found.station, admin_fp: found.admin_fp };
        return relay_join(route);
    }
    // Rien hors de chez soi : la réponse du réseau, sinon pourquoi le relais n'a pas aidé.
    match (local_error, relay_error) {
        (Some(e @ (ShareError::Full | ShareError::TooManyTries { .. })), _) => Err(e),
        (_, Some(RelayError::TooMany { seconds })) => Err(ShareError::TooManyTries { seconds }),
        (Some(ShareError::NoShare) | None, Some(e @ RelayError::Unreachable(_))) => Err(ShareError::Other(ConsoleError::Relay(e))),
        (Some(e), _) => Err(e),
        (None, _) => Err(ShareError::NoShare),
    }
}

/// [`join_link`] avec les appels substitués (tests).
fn join_link_with(
    local: impl FnOnce() -> Result<Found, ShareError> + Send + 'static,
    route: Option<RelayRoute>,
    relay_join: impl FnOnce(RelayRoute) -> Result<Joining, ShareError>,
    local_first: Duration,
) -> Result<Joining, ShareError> {
    let (tx, rx) = mpsc::channel();
    let _ = std::thread::Builder::new().name("kybernow-partage".into()).spawn(move || {
        let _ = tx.send(local());
    });
    let Some(route) = route else {
        return rx.recv_timeout(JOIN_TIMEOUT + Duration::from_secs(2)).unwrap_or(Err(ShareError::NoShare)).map(Joining::Joined);
    };
    match rx.recv_timeout(local_first) {
        Ok(Ok(found)) => Ok(Joining::Joined(found)),
        // La Station du lien dit non sur le réseau (complet, trop d'essais) : c'est sa réponse.
        Ok(Err(e @ (ShareError::Full | ShareError::TooManyTries { .. }))) => Err(e),
        _ => relay_join(route),
    }
}

/// Corps de `POST /api/share/quality` : avec « Auto », le `auto` mesuré par
/// l'appareil (sans lui, Admin lit « Auto » comme l'image de l'hôte).
pub(crate) fn quality_body(guest: &str, quality: ShareQuality, auto: Option<&ShareAuto>) -> serde_json::Value {
    let mut body = serde_json::json!({ "guest": guest, "quality": quality.as_str() });
    if quality == ShareQuality::Auto
        && let Some(a) = auto
    {
        body["auto"] = serde_json::to_value(a).unwrap_or_default();
    }
    body
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
            Ok(Ok(mut found)) => {
                // Le `auto` mesuré par l'appareil, renvoyé plus tard avec « Auto ».
                found.joined.auto = req.auto.clone();
                return Ok(found);
            }
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
    fn qualites() {
        for (s, q, code, fr, en) in [
            ("auto", ShareQuality::Auto, 0, "Auto", "Auto"),
            ("host", ShareQuality::Host, 1, "Comme l'hôte", "Same as host"),
            ("2160p60", ShareQuality::P2160_60, 4, "4K · 60 Hz", "4K · 60 Hz"),
            ("1080p60", ShareQuality::P1080_60, 2, "1080p · 60 Hz", "1080p · 60 Hz"),
            ("720p30", ShareQuality::P720_30, 3, "720p · 30 Hz", "720p · 30 Hz"),
        ] {
            assert_eq!((q.as_str(), ShareQuality::parse(s), q.code(), ShareQuality::from_code(code)), (s, Some(q), code, Some(q)));
            assert_eq!(serde_json::to_value(q).unwrap(), serde_json::json!(s));
            assert_eq!((q.label(Lang::Fr), q.label(Lang::En)), (fr.to_string(), en.to_string()));
        }
        assert_eq!(serde_json::from_value::<ShareQuality>(serde_json::json!("4k120")).unwrap(), ShareQuality::Auto, "inconnue : Auto");
        assert_eq!(ShareQuality::from_code(5), None);
        // L'ordre des menus : 4K entre « Comme l'hôte » et 1080p ; les codes des ponts gardent leur sens.
        assert_eq!(ShareQuality::ALL.map(ShareQuality::code), [0, 1, 4, 2, 3]);
        // Dans `join` : « Auto » par défaut, `auto` seulement s'il est donné.
        let req = JoinRequest { code: "123456".into(), name: "Léa".into(), device: "ipad".into(), app: "ios".into(), ..JoinRequest::default() };
        let v = serde_json::to_value(&req).unwrap();
        assert_eq!((v["quality"].clone(), v.get("auto").is_none()), (serde_json::json!("auto"), true));
        let auto = ShareAuto { width: 2556, height: 1179, fps: 60, codecs: vec!["h264".into(), "hevc".into()], yuv444: false };
        let v = serde_json::to_value(JoinRequest { quality: ShareQuality::Auto, auto: Some(auto), ..req.clone() }).unwrap();
        assert_eq!(v["auto"], serde_json::json!({ "width": 2556, "height": 1179, "fps": 60, "codecs": ["h264", "hevc"], "yuv444": false }));
        let v = serde_json::to_value(JoinRequest { quality: ShareQuality::P720_30, ..req }).unwrap();
        assert_eq!(v["quality"], serde_json::json!("720p30"));
        // Les ponts donnent le JSON sans les champs nouveaux : « Auto ».
        let old: JoinRequest = serde_json::from_str(r#"{"code":"123456","name":"Léa","device":"d","app":"mac"}"#).unwrap();
        assert_eq!((old.quality, old.auto), (ShareQuality::Auto, None));
    }

    fn screen(w: u32, h: u32, hz: u32) -> DeviceScreen {
        DeviceScreen { x: 0.0, y: 0.0, width: f64::from(w) / 3.0, height: f64::from(h) / 3.0, pixel_width: w, pixel_height: h, refresh: hz, primary: true }
    }

    #[test]
    fn qualite_changee_en_session() {
        let auto = ShareAuto { width: 2556, height: 1179, fps: 60, codecs: vec!["h264".into()], yuv444: false };
        assert_eq!(
            quality_body("g1", ShareQuality::Auto, Some(&auto)),
            serde_json::json!({ "guest": "g1", "quality": "auto", "auto": { "width": 2556, "height": 1179, "fps": 60, "codecs": ["h264"], "yuv444": false } })
        );
        assert_eq!(quality_body("g1", ShareQuality::Auto, None), serde_json::json!({ "guest": "g1", "quality": "auto" }), "rien de mesuré : l'image de l'hôte");
        assert_eq!(quality_body("g1", ShareQuality::P720_30, Some(&auto)), serde_json::json!({ "guest": "g1", "quality": "720p30" }), "seulement avec « Auto »");
    }

    #[test]
    fn qualite_auto() {
        let hevc = Capabilities { hevc: true, hevc444: false, av1: false };
        // Un iPhone (écran en portrait, 120 Hz), décodeur qui suit la 4K à 60 Hz : en paysage.
        let iphone = screen(1179, 2556, 120);
        let decoder = DecoderCaps { capabilities: hevc, max_width: 3840, max_height: 2160, max_fps: 60 };
        let a = auto_quality(&iphone, &decoder, None);
        assert_eq!((a.width, a.height, a.fps, a.yuv444), (2556, 1179, 60, false));
        assert_eq!(a.codecs, ["h264", "hevc"]);
        // Jamais au-dessus de l'image de l'hôte, quand on la connaît.
        let host = ShareDisplay { id: 7, width: 1920, height: 1080, refresh: 50 };
        let a = auto_quality(&iphone, &decoder, Some(&host));
        assert_eq!((a.width, a.height, a.fps), (1920, 1080, 50), "des bornes : la Station garde les proportions de l'image de l'hôte");
        // Un téléviseur 4K dont le décodeur suit 1080p ; sans limite connue : l'écran.
        let tv = screen(3840, 2160, 60);
        let a = auto_quality(&tv, &DecoderCaps { capabilities: Capabilities::default(), max_width: 1920, max_height: 1080, max_fps: 0 }, None);
        assert_eq!((a.width, a.height, a.fps, a.codecs.len()), (1920, 1080, 60, 1));
        let a = auto_quality(&tv, &DecoderCaps::from(Capabilities { hevc: true, hevc444: true, av1: true }), None);
        assert_eq!((a.width, a.height, a.fps, a.yuv444), (3840, 2160, 60, true));
        assert_eq!(a.codecs, ["h264", "hevc", "av1"]);
        // Le décodeur en JSON, pour les ponts.
        let d: DecoderCaps = serde_json::from_str(r#"{"hevc":true,"max_width":1920,"max_height":1080}"#).unwrap();
        assert_eq!(d, DecoderCaps { capabilities: Capabilities { hevc: true, ..Capabilities::default() }, max_width: 1920, max_height: 1080, max_fps: 0 });
    }

    #[test]
    fn image_recue() {
        let i: ShareImage = serde_json::from_str(r#"{"quality":"1080p60","width":1920,"height":1080,"fps":60,"codec":"hevc","follows_host":false,"reason":"asked"}"#).unwrap();
        assert_eq!((i.quality, i.label(), i.codec(), i.own(), i.reason), (ShareQuality::P1080_60, Some("1080p · 60 Hz".to_string()), Some(Codec::Hevc), true, Some(ShareImageReason::Asked)));
        // Tant que Kyber ne sait rien : tout à `null`, sauf la qualité demandée.
        let unknown: ShareImage =
            serde_json::from_str(r#"{"quality":"auto","width":null,"height":null,"fps":null,"codec":null,"follows_host":null,"reason":null}"#).unwrap();
        assert_eq!(unknown, ShareImage { quality: ShareQuality::Auto, ..ShareImage::default() });
        assert_eq!((unknown.size(), unknown.label(), unknown.own()), (None, None, false), "compté comme l'image de l'hôte");
        let odd: ShareImage = serde_json::from_str(r#"{"reason":"autre","quality":"8k","width":"1920","fps":60}"#).unwrap();
        assert_eq!((odd.reason, odd.quality, odd.width, odd.fps), (None, ShareQuality::Auto, None, Some(60)));
        // Dans le signe de vie, `join` et la liste de l'hôte ; pas un objet : ignorée.
        let b: ShareBeat = serde_json::from_str(r#"{"role":"spectator","image":{"quality":"auto","width":1280,"height":720,"fps":30,"reason":"lag"}}"#).unwrap();
        assert_eq!(b.image.map(|i| (i.height, i.reason)), Some((Some(720), Some(ShareImageReason::Lag))));
        let b: ShareBeat = serde_json::from_str(r#"{"role":"spectator","image":{"quality":"720p30","width":null,"height":null,"fps":null}}"#).unwrap();
        assert_eq!(b.image.map(|i| (i.quality, i.size())), Some((ShareQuality::P720_30, None)));
        let b: ShareBeat = serde_json::from_str(r#"{"role":"spectator","image":"720p"}"#).unwrap();
        assert!(b.image.is_none());
        let j: ShareJoined = serde_json::from_str(r#"{"guest":"g","secret":"s","image":{"width":1920,"height":1080,"fps":60,"follows_host":true}}"#).unwrap();
        assert!(j.image.is_some_and(|i| i.follows_host == Some(true)));
        let g: ShareGuest = serde_json::from_str(r#"{"id":"g","name":"Léa","image":{"quality":"host","width":1920,"height":1080,"fps":60}}"#).unwrap();
        assert_eq!(g.image.and_then(|i| i.label()), Some("1080p · 60 Hz".into()));
        let g: ShareGuest = serde_json::from_str(r#"{"id":"g","name":"Léa","image":[]}"#).unwrap();
        assert!(g.image.is_none() && g.name == "Léa");
        // Une image de 3840 × 2160 se lit « 4K » ; les autres, par leurs lignes.
        let at = |w, h, f| ShareImage { width: Some(w), height: Some(h), fps: Some(f), ..ShareImage::default() }.label();
        assert_eq!(at(3840, 2160, 60).as_deref(), Some("4K · 60 Hz"));
        assert_eq!(at(3840, 2160, 120).as_deref(), Some("4K · 120 Hz"), "4K de l'hôte à 120 Hz");
        assert_eq!(at(5120, 2880, 60).as_deref(), Some("2880p · 60 Hz"));
        assert_eq!(at(2560, 1440, 144).as_deref(), Some("1440p · 144 Hz"));
        let i: ShareImage = serde_json::from_str(r#"{"quality":"2160p60","width":3840,"height":2160,"fps":60,"reason":"asked"}"#).unwrap();
        assert_eq!((i.quality, i.label().as_deref()), (ShareQuality::P2160_60, Some("4K · 60 Hz")));
    }

    #[test]
    fn avis_de_l_image() {
        let img = |h: u32, fps: u32, reason| ShareImage { width: Some(h * 16 / 9), height: Some(h), fps: Some(fps), reason: Some(reason), ..ShareImage::default() };
        let full = img(1080, 60, ShareImageReason::Asked);
        let lag = img(720, 30, ShareImageReason::Lag);
        assert_eq!(lag.notice(Some(&full), Lang::Fr).as_deref(), Some("Image allégée pour suivre."));
        assert_eq!(lag.notice(Some(&full), Lang::En).as_deref(), Some("Picture lightened to keep up."));
        // Seulement sur une vraie baisse connue.
        let unknown = ShareImage::default();
        assert_eq!(lag.notice(None, Lang::Fr), None, "rien d'avant");
        assert_eq!(lag.notice(Some(&unknown), Lang::Fr), None, "taille d'avant inconnue");
        assert_eq!(img(1080, 60, ShareImageReason::Lag).notice(Some(&lag), Lang::Fr), None, "pas une baisse");
        assert_eq!(ShareImage { fps: None, ..lag.clone() }.notice(Some(&full), Lang::Fr), None, "taille reçue inconnue");
        assert!(img(540, 30, ShareImageReason::Lag).notice(Some(&lag), Lang::Fr).is_some(), "encore un cran plus bas");
        let limit = img(1080, 60, ShareImageReason::Limit);
        assert_eq!(limit.notice(None, Lang::Fr).as_deref(), Some("L'ordinateur ne peut pas faire une image de plus : vous recevez 1080p · 60 Hz."));
        assert_eq!(limit.notice(Some(&unknown), Lang::En).as_deref(), Some("The computer can't make another picture: you get 1080p · 60 Hz."));
        assert_eq!(limit.notice(Some(&ShareImage { codec: Some("h264".into()), ..limit.clone() }), Lang::Fr), None, "déjà dit pour cette image");
        assert_eq!(ShareImage { width: None, ..limit }.notice(None, Lang::Fr), None);
        // Rien à dire : demandée, bornée par l'hôte, raison inconnue.
        assert_eq!(full.notice(None, Lang::Fr), None);
        assert_eq!(img(2160, 120, ShareImageReason::Capped).notice(Some(&lag), Lang::Fr), None);
        assert_eq!(ShareImage { reason: None, ..lag.clone() }.notice(Some(&full), Lang::Fr), None);
    }

    #[test]
    fn format_demande_a_kyber() {
        let hevc = Capabilities { hevc: true, hevc444: false, av1: false };
        let hevc444 = Capabilities { hevc: true, hevc444: true, av1: false };
        let all = Capabilities { hevc: true, hevc444: true, av1: true };
        let h264 = Capabilities::default();
        assert_eq!(guest_format(None, false, h264).unwrap(), None, "image de l'hôte pas encore démarrée");
        assert_eq!(guest_format(Some("hevc"), true, hevc444).unwrap(), Some((Codec::Hevc, true)));
        assert_eq!(guest_format(Some("hevc"), true, hevc).unwrap(), Some((Codec::Hevc, false)), "4:2:0 sans le 4:4:4");
        assert_eq!(guest_format(Some("av1"), false, all).unwrap(), Some((Codec::Av1, false)));
        assert_eq!(guest_format(Some("av1"), false, hevc).unwrap(), Some((Codec::Hevc, false)), "un format lisible");
        assert_eq!(guest_format(Some("hevc"), false, h264).unwrap(), Some((Codec::H264, false)), "H.264 sinon");
        assert_eq!(guest_format(Some("vp9"), false, h264).unwrap(), Some((Codec::H264, false)), "format inconnu de l'hôte");
        assert_eq!(guest_format(Some("h264"), true, all).unwrap(), Some((Codec::H264, false)));
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
        assert_eq!(l, JoinLink { host: "192.168.1.20".into(), port: 24811, fingerprint: Some("sha256:aa:bb".into()), code: "123456".into(), relay: None, sid: None });
        assert_eq!(l.relay_route(), None, "sans relais : sur le réseau seulement");
        assert_eq!(l.target(), ShareTarget { host: "192.168.1.20".into(), port: 24811, fingerprint: Some("sha256:aa:bb".into()) });
        // Empreinte non encodée, ordre quelconque, IPv6 entre crochets.
        let l = parse_link("kybernow://join/?code=123%20456&fp=sha256:cc&port=9443&host=[fe80::1]").unwrap();
        assert_eq!((l.host.as_str(), l.port, l.fingerprint.as_deref(), l.code.as_str()), ("fe80::1", 9443, Some("sha256:cc"), "123456"));
        // Lien du client web : sans empreinte.
        let l = parse_link("https://192.168.1.20:24811/link/?join=123456").unwrap();
        assert_eq!(l, JoinLink { host: "192.168.1.20".into(), port: 24811, fingerprint: None, code: "123456".into(), relay: None, sid: None });
        // Hors de chez soi : `relay` (avec son empreinte éventuelle, encodée) et `sid`.
        let l = parse_link("kybernow://join?host=192.168.1.20&port=24811&fp=sha256%3Aaa&code=123456&relay=share.kybernow.app%3A8443&sid=ABCDEFGHIJKLMNOPQRSTUVWXYZ").unwrap();
        assert_eq!((l.relay.as_deref(), l.sid.as_deref()), (Some("share.kybernow.app:8443"), Some("abcdefghijklmnopqrstuvwxyz")));
        assert_eq!(
            l.relay_route(),
            Some(RelayRoute { server: "share.kybernow.app:8443".into(), station: "abcdefghijklmnopqrstuvwxyz".into(), admin_fp: "sha256:aa".into() }),
            "de bout en bout : Admin épinglé sur l'empreinte du lien"
        );
        let l = parse_link("kybernow://join?host=h&port=1&code=123456&relay=relais.exemple.org%3A9443%23sha256%3AAB%3AAB%3AAB%3AAB%3AAB%3AAB%3AAB%3AAB%3AAB%3AAB%3AAB%3AAB%3AAB%3AAB%3AAB%3AAB%3AAB%3AAB%3AAB%3AAB%3AAB%3AAB%3AAB%3AAB%3AAB%3AAB%3AAB%3AAB%3AAB%3AAB%3AAB%3AAB&sid=abcdefghijklmnopqrstuvwxyz").unwrap();
        assert_eq!(l.relay.as_deref(), Some("relais.exemple.org:9443#sha256:AB:AB:AB:AB:AB:AB:AB:AB:AB:AB:AB:AB:AB:AB:AB:AB:AB:AB:AB:AB:AB:AB:AB:AB:AB:AB:AB:AB:AB:AB:AB:AB"), "relais chez soi, épinglé");
        assert_eq!(l.relay_route(), None, "sans `fp`, pas par le relais : jamais de premier contact");
        let l = parse_link("kybernow://join?host=h&port=1&fp=sha256:aa&code=123456&relay=share.kybernow.app&sid=pas-un-id").unwrap();
        assert_eq!((l.sid.clone(), l.relay_route()), (None, None), "identifiant illisible : écarté");
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
        JoinRequest { code: "123456".into(), name: "Léa".into(), device: "ipad-1".into(), app: "ios".into(), ..JoinRequest::default() }
    }

    fn station_code(online: bool) -> crate::relay::CodeLookup {
        crate::relay::CodeLookup { station: "abcdefghijklmnopqrstuvwxyz".into(), station_key: "k".into(), admin_fp: "sha256:ad".into(), sig: "s".into(), online }
    }

    fn found(host: &str) -> Found {
        Found { target: target(host), joined: ShareJoined { guest: "g".into(), secret: "s".into(), ..ShareJoined::default() } }
    }

    fn pending(route: RelayRoute) -> Joining {
        Joining::Pending(PendingJoin { route, guest: "g".into(), secret: "s".into(), expires: 120, auto: None })
    }

    fn lookup_of(r: Result<Option<crate::relay::CodeLookup>, RelayError>, delay: Duration) -> Lookup {
        Box::new(move || {
            std::thread::sleep(delay);
            r.map(|f| f.map(|f| (RelayAddr::public(), f)))
        })
    }

    const FAST: Duration = Duration::from_millis(80);

    #[test]
    fn rejoindre_sur_le_reseau_et_hors_de_chez_soi() {
        let never = |_: RelayRoute| -> Result<Joining, ShareError> { panic!("pas par le relais") };
        // Sur le réseau, en moins de 1,5 s : pris, même si le relais connaît le code.
        let j = join_anywhere_with(|| Ok(found("pc")), vec![lookup_of(Ok(Some(station_code(true))), Duration::ZERO)], never, FAST).unwrap();
        assert!(matches!(j, Joining::Joined(f) if f.target.host == "pc"));
        // Personne sur le réseau : la Station du relais, de bout en bout, en attente de l'hôte.
        let seen = std::sync::Mutex::new(None);
        let j = join_anywhere_with(
            || Err(ShareError::NoShare),
            vec![lookup_of(Ok(None), Duration::ZERO), lookup_of(Ok(Some(station_code(true))), Duration::from_millis(20))],
            |route| {
                *seen.lock().unwrap() = Some(route.clone());
                Ok(pending(route))
            },
            FAST,
        )
        .unwrap();
        assert!(matches!(j, Joining::Pending(_)));
        assert_eq!(
            seen.into_inner().unwrap(),
            Some(RelayRoute { server: "share.kybernow.app:8443".into(), station: "abcdefghijklmnopqrstuvwxyz".into(), admin_fp: "sha256:ad".into() }),
            "Admin épinglé sur l'empreinte attestée"
        );
        // Le réseau lent : 1,5 s puis le relais, sans attendre la fin du réseau.
        let started = std::time::Instant::now();
        let slow_local = || {
            std::thread::sleep(Duration::from_millis(600));
            Ok(found("pc"))
        };
        let j = join_anywhere_with(slow_local, vec![lookup_of(Ok(Some(station_code(true))), Duration::ZERO)], |r| Ok(pending(r)), FAST).unwrap();
        assert!(matches!(j, Joining::Pending(_)) && started.elapsed() < Duration::from_millis(500));
        // La Station du code n'est pas chez le relais en ce moment.
        let e = join_anywhere_with(|| Err(ShareError::NoShare), vec![lookup_of(Ok(Some(station_code(false))), Duration::ZERO)], never, FAST).unwrap_err();
        assert!(matches!(e, ShareError::Other(ConsoleError::Relay(RelayError::StationOffline))), "{e:?}");
        // Le relais ne connaît pas le code non plus : « Aucun partage… ».
        let e = join_anywhere_with(|| Err(ShareError::NoShare), vec![lookup_of(Ok(None), Duration::ZERO)], never, FAST).unwrap_err();
        assert!(matches!(e, ShareError::NoShare));
        // Relais muet, rien sur le réseau : c'est le relais qu'on dit.
        let e = join_anywhere_with(|| Err(ShareError::NoShare), vec![lookup_of(Err(RelayError::Unreachable("x".into())), Duration::ZERO)], never, FAST).unwrap_err();
        assert!(matches!(e, ShareError::Other(ConsoleError::Relay(RelayError::Unreachable(_)))));
        // Le partage du réseau est complet : sa réponse, pas celle du relais.
        let e = join_anywhere_with(|| Err(ShareError::Full), vec![lookup_of(Err(RelayError::Unreachable("x".into())), Duration::ZERO)], never, FAST).unwrap_err();
        assert!(matches!(e, ShareError::Full));
        // Trop de recherches au relais.
        let e = join_anywhere_with(|| Err(ShareError::NoShare), vec![lookup_of(Err(RelayError::TooMany { seconds: Some(60) }), Duration::ZERO)], never, FAST).unwrap_err();
        assert!(matches!(e, ShareError::TooManyTries { seconds: Some(60) }));
        // Relais par défaut quand l'app n'en connaît aucun ; pas de doublon.
        assert_eq!(unique_relays(&[]), vec![RelayAddr::public()]);
        assert_eq!(unique_relays(&[RelayAddr::public(), "share.kybernow.app".parse().unwrap()]).len(), 1);
    }

    #[test]
    fn rejoindre_par_un_lien() {
        let route = RelayRoute { server: "share.kybernow.app:8443".into(), station: "abcdefghijklmnopqrstuvwxyz".into(), admin_fp: "sha256:ad".into() };
        // Sur le réseau d'abord.
        let j = join_link_with(|| Ok(found("pc")), Some(route.clone()), |_| panic!("pas par le relais"), FAST).unwrap();
        assert!(matches!(j, Joining::Joined(_)));
        // L'ordinateur n'est pas sur ce réseau : le relais, vers `sid`, épinglé sur `fp`.
        let offline = || -> Result<Found, ShareError> { Err(ShareError::Other(ConsoleError::Network("hors ligne".into()))) };
        let j = join_link_with(offline, Some(route.clone()), |r| {
            assert_eq!(r, route);
            Ok(pending(r))
        }, FAST)
        .unwrap();
        assert!(matches!(j, Joining::Pending(_)));
        // Le partage du lien, complet sur le réseau : sa réponse.
        assert!(matches!(join_link_with(|| Err(ShareError::Full), Some(route.clone()), |_| panic!(), FAST), Err(ShareError::Full)));
        // Lien sans relais : la réponse du réseau seulement.
        assert!(matches!(join_link_with(offline, None, |_| panic!(), FAST), Err(ShareError::Other(_))));
    }

    #[test]
    fn demande_en_attente_en_json() {
        let route = RelayRoute { server: "share.kybernow.app:8443".into(), station: "abcdefghijklmnopqrstuvwxyz".into(), admin_fp: "sha256:ad".into() };
        let p = PendingJoin { route, guest: "g1".into(), secret: "s3cr3t".into(), expires: 120, auto: None };
        let v = serde_json::to_value(Joining::Pending(p.clone())).unwrap();
        assert_eq!(v["pending"]["guest"], "g1");
        assert_eq!(serde_json::from_value::<Joining>(v).unwrap().pending(), Some(p.clone()));
        assert!(!format!("{p:?}").contains("s3cr3t"));
        let v = serde_json::to_value(Joining::Joined(found("pc"))).unwrap();
        assert_eq!(v["joined"]["target"]["host"], "pc");
        // La route d'une session venue par le relais voyage avec `joined`.
        let mut j = ShareJoined::default();
        j.route = p.route.clone().into();
        let again: ShareJoined = serde_json::from_value(serde_json::to_value(&j).unwrap()).unwrap();
        assert_eq!(again.route, Some(p.route));
        assert!(serde_json::to_value(ShareJoined::default()).unwrap().get("route").is_none());
    }

    #[test]
    fn partage_hors_de_chez_soi_par_le_relais() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        let waits = Arc::new(AtomicUsize::new(0));
        let left = Arc::new(AtomicUsize::new(0));
        let (w, l) = (waits.clone(), left.clone());
        let bench = crate::relay::essai::start(Arc::new(move |method, path| match (method, path) {
            ("POST", "/api/share/join") => (202, r#"{"pending":true,"guest":"g1","secret":"s3cr3t","expires":120}"#.into()),
            // L'hôte accepte au deuxième appel.
            ("POST", "/api/share/wait") if w.fetch_add(1, Ordering::SeqCst) == 0 => (202, r#"{"pending":true}"#.into()),
            ("POST", "/api/share/wait") => (
                200,
                r#"{"guest":"g1","secret":"s3cr3t","role":"spectator","host":"Léa","computer":"PC","relay":true,
                   "kyber":{"port":24812,"fingerprint":"sha256:kk","token":"jwt","expires":120},
                   "display":{"id":7,"width":1920,"height":1080,"refresh":60},"codec":"hevc","yuv444":false,"beat":5}"#
                    .into(),
            ),
            ("POST", "/api/share/leave") => {
                l.fetch_add(1, Ordering::SeqCst);
                (200, "{}".into())
            }
            _ => (404, r#"{"error":"no_share"}"#.into()),
        }));
        let auto = ShareAuto { width: 2556, height: 1179, fps: 60, codecs: vec!["h264".into()], yuv444: false };
        let req = JoinRequest { code: "123456".into(), name: "Léa".into(), device: "iphone".into(), app: "ios".into(), auto: Some(auto.clone()), ..JoinRequest::default() };
        // Personne sur le réseau : le relais connaît le code, la demande attend l'hôte.
        let joining = join_anywhere(&[], &[bench.addr()], &req, None).unwrap();
        let pending = joining.pending().expect("en attente de l'accord de l'hôte");
        assert_eq!((pending.guest.as_str(), pending.route.station.as_str(), pending.route.admin_fp.as_str()), ("g1", bench.code.station.as_str(), bench.admin_fp.as_str()));
        // Toujours en attente, puis accepté : la session passera par le relais.
        assert!(wait(&pending, None).unwrap().is_none());
        let found = wait(&pending, Some(Capabilities { hevc: true, ..Capabilities::default() })).unwrap().expect("accepté");
        assert_eq!(found.joined.route.as_ref(), Some(&pending.route));
        assert_eq!(found.joined.auto, Some(auto), "le `auto` mesuré reste pour la session");
        assert_eq!((found.target.host.as_str(), found.target.fingerprint.as_deref()), (bench.station.as_str(), Some(bench.admin_fp.as_str())));
        let params = crate::session::SessionParams::guest(&found, crate::console::DeviceInfo { id: "iphone".into(), name: "iPhone".into(), kind: "ios".into(), os: "iOS".into(), app: "t".into() }, crate::profile::StreamSettings::for_profile(crate::profile::Profile::Jeu, Capabilities::default()), Capabilities::default(), 2);
        assert_eq!(params.route, crate::session::Route::Relay(pending.route.clone()));
        // Annuler une demande, quitter le partage : par le relais aussi.
        cancel(&pending);
        leave(&found);
        assert_eq!(left.load(Ordering::SeqCst), 2);
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
        assert_eq!(found.joined.auto, None, "rien envoyé pour « Auto »");
        // Le `auto` envoyé à `join` reste avec la réponse, pour la session.
        let auto = ShareAuto { width: 2556, height: 1179, fps: 60, codecs: vec!["h264".into(), "hevc".into()], yuv444: false };
        let with_auto = JoinRequest { auto: Some(auto.clone()), ..request() };
        let found = join_with(&[target("b")], &with_auto, call).unwrap();
        assert_eq!(found.joined.auto.as_ref(), Some(&auto));
        let again: ShareJoined = serde_json::from_value(serde_json::to_value(&found.joined).unwrap()).unwrap();
        assert_eq!(again.auto, Some(auto), "et voyage dans le JSON des ponts");
        assert!(serde_json::to_value(ShareJoined::default()).unwrap().get("auto").is_none());
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
