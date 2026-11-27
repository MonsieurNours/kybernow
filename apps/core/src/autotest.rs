//! Profil « Auto » (docs/profil-auto.md) : le test des conditions, lancé à la
//! demande, et les réglages qu'il propose.
//!
//! Quatre étapes ([`Step`]) : la liaison (câble, Wi-Fi, relais), la latence au
//! repos (`GET /api/stream/ping`), le débit vers l'appareil (la rafale de la Station,
//! `GET /api/stream/probe`, avec la latence en charge) et le décodage de
//! l'appareil (des extraits vidéo embarqués, décodés par l'app). Puis
//! [`decide`] choisit le format, la définition, la fréquence, le 4:4:4, le
//! débit et le transport, chacun avec sa raison. Rien n'est appliqué ici :
//! l'app montre le résumé et garde le résultat ([`AutoResult`]) si
//! l'utilisateur l'applique.

use std::io::Read;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use crate::console::{Console, ConsoleError, EncoderAnswer, EncoderQuery, Mode};
use crate::i18n::{Lang, tr};
use crate::profile::{Capabilities, Codec, PYROWAVE_KYBER, Profile, StreamSettings};

/// Une étape du test, dans l'ordre.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Step {
    Link,
    Latency,
    Bandwidth,
    Decode,
    Encoder,
}

impl Step {
    pub const ALL: [Step; 5] = [Step::Link, Step::Latency, Step::Bandwidth, Step::Decode, Step::Encoder];

    pub fn key(self) -> &'static str {
        match self {
            Self::Link => "link",
            Self::Latency => "latency",
            Self::Bandwidth => "bandwidth",
            Self::Decode => "decode",
            Self::Encoder => "encoder",
        }
    }

    /// « Liaison », « Latence au repos », « Débit vers cet appareil »,
    /// « Décodage de cet appareil », « Encodeur de l'ordinateur ».
    pub fn title(self, lang: Lang) -> String {
        tr(lang, &format!("autotest.step.{}", self.key()), &[])
    }
}

/// Où en est une étape.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "state", content = "text", rename_all = "lowercase")]
pub enum StepState {
    Waiting,
    Running,
    /// Cochée, avec sa mesure (« 2,1 ms · gigue 0,4 ms »).
    Done(String),
    /// Passée, avec sa raison.
    Skipped(String),
}

/// L'avancement du test, donné à l'app à chaque changement.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Progress {
    pub steps: Vec<StepProgress>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StepProgress {
    pub step: Step,
    pub title: String,
    #[serde(flatten)]
    pub state: StepState,
}

impl Progress {
    fn new(lang: Lang) -> Self {
        Self { steps: Step::ALL.iter().map(|&step| StepProgress { step, title: step.title(lang), state: StepState::Waiting }).collect() }
    }

    fn set(&mut self, step: Step, state: StepState) {
        if let Some(s) = self.steps.iter_mut().find(|s| s.step == step) {
            s.state = state;
        }
    }
}

/// Comment l'appareil joint l'ordinateur.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Link {
    Cable,
    Wifi,
    Relay,
}

impl Link {
    pub fn text(self, lang: Lang) -> String {
        tr(
            lang,
            match self {
                Self::Cable => "autotest.link.cable",
                Self::Wifi => "autotest.link.wifi",
                Self::Relay => "autotest.link.relay",
            },
            &[],
        )
    }

    /// Débit supposé sans mesure (Station d'avant), en Mbit/s, pour la 4K à
    /// 60 Hz : câble 300, Wi-Fi 80, relais 25.
    fn assumed_mbps(self) -> f64 {
        match self {
            Self::Cable => 300.0,
            Self::Wifi => 80.0,
            Self::Relay => 25.0,
        }
    }

    /// Part du débit mesuré gardée pour l'image : câble 60 %, sinon 50 %.
    fn share(self) -> f64 {
        match self {
            Self::Cable => 0.6,
            _ => 0.5,
        }
    }
}

/// Latence au repos, en millisecondes.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Latency {
    pub min: f64,
    pub median: f64,
    /// Écart entre le 9ᵉ et le 1ᵉʳ décile.
    pub jitter: f64,
}

/// La rafale de la Station.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Bandwidth {
    pub mbps: f64,
    /// Hausse de la latence pendant la rafale (médiane en charge moins
    /// médiane au repos), en millisecondes ; `None` sans mesure.
    pub loaded_delta: Option<f64>,
}

/// L'encodeur de l'ordinateur essayé par le test (docs/profil-auto.md,
/// « L'encodeur de l'ordinateur ») : le débit le plus haut accepté, par
/// écran, à ce format et à ce mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct EncoderMeasure {
    #[serde(with = "codec_key")]
    pub codec: Codec,
    pub yuv444: bool,
    pub width: u32,
    pub height: u32,
    pub refresh: u32,
    /// Mbit/s acceptés.
    pub mbps: u32,
    /// Un débit plus haut a été refusé : `mbps` est la limite de l'encodeur
    /// (sinon, le premier essai a passé : la limite est au-dessus).
    pub limited: bool,
}

/// Images par seconde décodées par l'app pour un extrait (en 4K).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct DecodeResult {
    #[serde(with = "codec_key")]
    pub codec: Codec,
    pub yuv444: bool,
    pub fps: f64,
}

mod codec_key {
    use super::Codec;
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S: Serializer>(c: &Codec, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(c.key())
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Codec, D::Error> {
        let s = String::deserialize(d)?;
        Codec::parse(&s).ok_or_else(|| serde::de::Error::custom(format!("format inconnu : {s}")))
    }
}

/// Les mesures du test.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Measures {
    pub link: Link,
    pub latency: Option<Latency>,
    /// `None` : Station d'avant (pas de rafale).
    pub bandwidth: Option<Bandwidth>,
    pub decode: Vec<DecodeResult>,
    /// `None` : pas essayé (PyroWave, Station d'avant, sonde impossible).
    #[serde(default)]
    pub encoder: Option<EncoderMeasure>,
}

/// Ce que l'app dit de l'appareil et de l'ordinateur pour le test.
#[derive(Debug, Clone, PartialEq)]
pub struct Device {
    /// L'écran de la session (celui de l'appareil).
    pub screen: Mode,
    pub caps: Capabilities,
    /// Le choix « HDR » de l'app pour cet ordinateur (gardé tel quel).
    pub hdr: bool,
    /// Le réseau du moment (nom donné par l'app : nom du Wi-Fi, « Ethernet »),
    /// gardé avec le résultat ; le relais s'appelle `relay`.
    pub network: Option<String>,
    /// Relié par câble au réseau de l'ordinateur.
    pub wired: bool,
}

/// Ce que l'ordinateur propose.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Station {
    pub name: String,
    pub pyrowave: bool,
    /// « Débit maximal » de PyroWave, en Mbit/s.
    pub pyrowave_max: Option<u32>,
    /// La Station sait essayer son encodeur (`GET /api/stream/encoder`).
    pub encoder_check: bool,
}

/// Pourquoi un réglage a été choisi.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Reason {
    PyroWaveCarried,
    PyroWaveNeedsCable,
    PyroWaveNotCarried,
    PyroWaveNotOffered,
    PyroWaveTooSlow,
    PyroWaveNotDecoded,
    /// HEVC, AV1 ou H.264 sans PyroWave en jeu (appareil qui ne le décode pas,
    /// app sans PyroWave).
    BestFormat,
    MatchesDisplay,
    DecodeLimited,
    BitrateLimited,
    Yuv444Allowed,
    Yuv444TooTight,
    Yuv444NotDecoded,
    Yuv444Hdr,
    Measured,
    AssumedFromLink,
    /// Le débit borné par l'encodeur de l'ordinateur (« {nom} encode jusqu'à {n} Mbit/s »).
    EncoderLimited,
    Steady,
    Unsteady,
    Adaptive,
}

/// Un réglage proposé, tel que le résumé le montre.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Choice {
    /// `codec`, `mode`, `yuv444`, `bitrate`, `transport`, `adaptive`.
    pub key: String,
    pub title: String,
    pub value: String,
    pub reason: String,
}

/// Le résultat du test : les mesures et les réglages proposés.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AutoResult {
    /// Date du test (secondes depuis 1970).
    pub at: u64,
    /// Le réseau du test (`Device::network`, `relay` par le relais).
    pub network: Option<String>,
    pub measures: Measures,
    /// Format choisi ; `pyrowave` : PyroWave par câble, `mbps` restant le
    /// débit de HEVC ([`crate::profile::pyrowave_format`]).
    #[serde(with = "codec_key")]
    pub codec: Codec,
    pub yuv444: bool,
    pub width: u32,
    pub height: u32,
    pub refresh: u32,
    /// Débit de base, calé sur la 4K à 60 Hz comme celui des profils.
    pub mbps: u32,
    /// Débit visé à la définition choisie, en Mbit/s (pour le résumé).
    pub mode_mbps: u32,
    pub reliable: bool,
    pub reasons: Reasons,
    /// Le nom de l'ordinateur (textes).
    #[serde(default)]
    pub station: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Reasons {
    pub codec: Reason,
    pub mode: Reason,
    pub yuv444: Reason,
    pub bitrate: Reason,
    pub transport: Reason,
    /// Le décodage limité à tant d'images par seconde en 4K (raison
    /// `decode_limited`).
    #[serde(default)]
    pub decode_fps: Option<f64>,
}

impl AutoResult {
    /// Les réglages de session d'Auto ([`StreamSettings`], profil `auto`),
    /// avec le HDR et le câble donnés par l'app.
    pub fn settings(&self, hdr: bool, wired: bool) -> StreamSettings {
        StreamSettings {
            profile: Some(Profile::Auto),
            codec: self.codec,
            yuv444: self.yuv444 && !hdr,
            mbps: self.mbps,
            reliable: self.reliable,
            hdr,
            wired,
            adaptive: true,
            encoder_max: self.encoder_max(),
        }
    }

    /// Le plafond de l'encodeur trouvé par le test (Mbit/s par écran) : un
    /// débit plus haut a été refusé.
    pub fn encoder_max(&self) -> Option<u32> {
        self.measures.encoder.filter(|e| e.limited).map(|e| e.mbps)
    }

    /// Le débit accepté du premier coup (Mbit/s par écran) : la limite de
    /// l'encodeur est au-dessus.
    pub fn encoder_ok(&self) -> Option<u32> {
        self.measures.encoder.filter(|e| !e.limited).map(|e| e.mbps)
    }

    /// Le plafond de l'encodeur à garder pour cet ordinateur une fois ce
    /// résultat appliqué, d'après celui qu'il avait (`previous`) : la limite
    /// trouvée ; sinon un plafond gardé plus bas que le débit accepté du
    /// premier coup est retiré ; sinon celui d'avant.
    /// (`rules::limits::next_encoder_max`, la règle que LG prend aussi sans le cœur.)
    pub fn next_encoder_max(&self, previous: Option<u32>) -> Option<u32> {
        kybernow_rules::limits::next_encoder_max(self.encoder_max(), self.encoder_ok(), previous)
    }

    /// Le test a-t-il été fait sur ce réseau ? (`network` : celui du moment,
    /// `relay` par le relais.) Un réseau inconnu des deux côtés compte comme
    /// le même.
    pub fn same_network(&self, network: Option<&str>) -> bool {
        self.network.as_deref() == network
    }

    /// Le résumé : une ligne par réglage, valeur puis raison.
    pub fn choices(&self, lang: Lang) -> Vec<Choice> {
        let name = self.station.as_str();
        let t = |k: &str| tr(lang, k, &[]);
        let reason = |r: Reason| -> String {
            let fps = self.reasons.decode_fps.map(|f| format!("{}", f.round() as u32)).unwrap_or_default();
            let measured = self.measures.bandwidth.map(|b| format!("{}", b.mbps.round() as u32)).unwrap_or_default();
            let encoder = self.measures.encoder.map(|e| e.mbps.to_string()).unwrap_or_default();
            tr(lang, &format!("autotest.reason.{}", key(r)), &[("name", name), ("fps", &fps), ("measured", &measured), ("n", &encoder)])
        };
        let yes_no = |b: bool| t(if b { "autotest.on" } else { "autotest.off" });
        vec![
            Choice { key: "codec".into(), title: t("autotest.choice.codec"), value: self.codec.label().into(), reason: reason(self.reasons.codec) },
            Choice {
                key: "mode".into(),
                title: t("autotest.choice.mode"),
                value: tr(lang, "autotest.mode", &[("w", &self.width.to_string()), ("h", &self.height.to_string()), ("hz", &self.refresh.to_string())]),
                reason: reason(self.reasons.mode),
            },
            Choice { key: "yuv444".into(), title: t("autotest.choice.yuv444"), value: yes_no(self.yuv444), reason: reason(self.reasons.yuv444) },
            Choice {
                key: "bitrate".into(),
                title: t("autotest.choice.bitrate"),
                value: tr(lang, "autotest.mbps", &[("n", &self.mode_mbps.to_string())]),
                reason: reason(self.reasons.bitrate),
            },
            Choice {
                key: "transport".into(),
                title: t("autotest.choice.transport"),
                value: t(if self.reliable { "autotest.reliable" } else { "autotest.fast" }),
                reason: reason(self.reasons.transport),
            },
            Choice { key: "adaptive".into(), title: t("autotest.choice.adaptive"), value: yes_no(true), reason: reason(Reason::Adaptive) },
        ]
    }
}

fn key(r: Reason) -> String {
    serde_json::to_value(r).ok().and_then(|v| v.as_str().map(str::to_string)).unwrap_or_default()
}

/// Débit des profils et de Max, calé sur la 4K à 60 Hz.
const MAX_MBPS: f64 = 300.0;
const MIN_MBPS: f64 = 10.0;
/// Correction d'erreurs : 30 % du débit en plus.
const FEC: f64 = 1.3;
/// Marge du décodage : 20 % d'images par seconde en plus de la fréquence.
const DECODE_MARGIN: f64 = 1.2;
const UHD: f64 = 3840.0 * 2160.0;

/// La règle des profils (`StreamSettings::bitrate_for_pixel_rate`) : un débit
/// calé sur la 4K à 60 Hz, mis à l'échelle des pixels par seconde.
fn scale(m: Mode) -> f64 {
    crate::profile::pixel_rate_scale(f64::from(m.width) * f64::from(m.height) * f64::from(m.refresh.max(1)))
}

/// Images par seconde d'un format à une définition, d'après l'extrait en 4K
/// (le décodage suit le nombre de pixels). `None` : pas d'essai.
fn decode_fps(decode: &[DecodeResult], codec: Codec, yuv444: bool, m: Mode) -> Option<f64> {
    let r = decode.iter().find(|d| d.codec == codec && d.yuv444 == yuv444)?;
    Some(r.fps * UHD / (f64::from(m.width) * f64::from(m.height)).max(1.0))
}

/// Le décodage tient-il la fréquence ? Sans essai : oui pour les formats
/// matériels que l'app dit décoder, non pour PyroWave.
fn decodes(decode: &[DecodeResult], codec: Codec, yuv444: bool, m: Mode) -> bool {
    match decode_fps(decode, codec, yuv444, m) {
        Some(fps) => fps >= f64::from(m.refresh) * DECODE_MARGIN,
        None => codec != Codec::PyroWave,
    }
}

/// Les modes essayés, du meilleur au plus modeste : l'écran, sa fréquence
/// ramenée à 120 puis 60 Hz, puis la définition ramenée à 1440p et 1080p
/// (même rapport de côtés).
fn modes(screen: Mode) -> Vec<Mode> {
    let mut out: Vec<Mode> = Vec::new();
    let mut push = |m: Mode| {
        if !out.contains(&m) {
            out.push(m);
        }
    };
    push(screen);
    for hz in [120, 60] {
        if screen.refresh > hz {
            push(Mode { refresh: hz, ..screen });
        }
    }
    let hz = screen.refresh.min(60);
    for h in [1440u32, 1080] {
        if screen.height > h {
            let w = ((f64::from(screen.width) * f64::from(h) / f64::from(screen.height)) / 2.0).round() as u32 * 2;
            push(Mode { width: w, height: h, refresh: hz });
        }
    }
    out
}

/// Les réglages proposés pour ces mesures (docs/profil-auto.md, « Les choix »).
pub fn decide(m: &Measures, d: &Device, st: &Station, now: u64) -> AutoResult {
    let measured = m.bandwidth.map(|b| b.mbps);
    // Budget de l'image, en Mbit/s, à la définition de la session.
    let budget = |mode: Mode| match measured {
        Some(t) => t * m.link.share() / FEC,
        None => m.link.assumed_mbps() * scale(mode),
    };

    // Format : PyroWave d'abord, s'il tient à la définition de l'écran.
    let screen = d.screen;
    let pw_possible = PYROWAVE_KYBER && st.pyrowave && d.caps.pyrowave;
    let pw_budget = |mode: Mode| {
        let b = budget(mode);
        st.pyrowave_max.map_or(b, |cap| b.min(f64::from(cap)))
    };
    let pw_needed = |mode: Mode, yuv444: bool| f64::from(crate::pyrowave::needed_mbps(&[mode], yuv444));
    let pyrowave_reason = if !PYROWAVE_KYBER || !d.caps.pyrowave {
        None
    } else if !st.pyrowave {
        Some(Reason::PyroWaveNotOffered)
    } else if m.link != Link::Cable {
        Some(Reason::PyroWaveNeedsCable)
    } else if measured.is_some() && pw_budget(screen) < pw_needed(screen, false) {
        Some(Reason::PyroWaveNotCarried)
    } else if measured.is_none() {
        // Sans mesure, PyroWave n'est pas pris : rien ne dit que le réseau le porte.
        Some(Reason::PyroWaveNotCarried)
    } else if decode_fps(&m.decode, Codec::PyroWave, false, screen).is_none() {
        Some(Reason::PyroWaveNotDecoded)
    } else if !decodes(&m.decode, Codec::PyroWave, false, screen) {
        Some(Reason::PyroWaveTooSlow)
    } else {
        None
    };
    let pyrowave = pw_possible && pyrowave_reason.is_none();

    // Le format de repli (et celui de la session sans PyroWave).
    let fallback = crate::profile::pyrowave_fallback(d.caps);
    let codec_reason = if pyrowave { Reason::PyroWaveCarried } else { pyrowave_reason.unwrap_or(Reason::BestFormat) };

    // Définition et fréquence : le premier mode que le décodage tient.
    let candidates = modes(screen);
    let (mode, mode_reason) = if pyrowave {
        (screen, Reason::MatchesDisplay)
    } else {
        match candidates.iter().copied().find(|&mm| decodes(&m.decode, fallback, false, mm)) {
            Some(mm) if mm == screen => (mm, Reason::MatchesDisplay),
            Some(mm) => (mm, Reason::DecodeLimited),
            None => (*candidates.last().unwrap_or(&screen), Reason::DecodeLimited),
        }
    };
    let decode_limit = decode_fps(&m.decode, if pyrowave { Codec::PyroWave } else { fallback }, false, Mode { width: 3840, height: 2160, refresh: 60 });

    // Débit de HEVC (AV1, H.264) à ce mode : le budget, au plus celui de Max.
    let h264 = if fallback == Codec::H264 { 1.5 } else { 1.0 };
    let max_at_mode = MAX_MBPS * h264 * scale(mode);
    let b = budget(mode);
    let uncapped = b.clamp(MIN_MBPS, max_at_mode.max(MIN_MBPS));
    // Au plus ce que l'encodeur de l'ordinateur a accepté à ce format et à ce mode.
    let encoder = m.encoder.filter(|e| {
        !pyrowave && e.codec == fallback && (e.width, e.height, e.refresh) == (mode.width, mode.height, mode.refresh)
    });
    let mode_mbps_hevc = encoder.map_or(uncapped, |e| uncapped.min(f64::from(e.mbps)));
    let bitrate_reason = if encoder.is_some_and(|e| e.limited && f64::from(e.mbps) < uncapped) {
        Reason::EncoderLimited
    } else if measured.is_some() {
        Reason::Measured
    } else {
        Reason::AssumedFromLink
    };

    // 4:4:4 : décodé, sans HDR, et le budget couvre le surcoût.
    let (yuv444, yuv444_reason) = if d.hdr {
        (false, Reason::Yuv444Hdr)
    } else if pyrowave {
        if !decodes(&m.decode, Codec::PyroWave, true, mode) || decode_fps(&m.decode, Codec::PyroWave, true, mode).is_none() {
            (false, Reason::Yuv444NotDecoded)
        } else if pw_budget(mode) < pw_needed(mode, true) {
            (false, Reason::Yuv444TooTight)
        } else {
            (true, Reason::Yuv444Allowed)
        }
    } else if fallback != Codec::Hevc || !d.caps.hevc444 || !decodes(&m.decode, Codec::Hevc, true, mode) {
        (false, Reason::Yuv444NotDecoded)
    } else if b < mode_mbps_hevc * 4.0 / 3.0 || b < 40.0 * scale(mode) {
        (false, Reason::Yuv444TooTight)
    } else {
        (true, Reason::Yuv444Allowed)
    };

    // Transport : Fiable sur un réseau irrégulier.
    let jitter = m.latency.map_or(0.0, |l| l.jitter);
    let loaded = m.bandwidth.and_then(|b| b.loaded_delta).unwrap_or(0.0);
    let reliable = jitter > 10.0 || loaded > 50.0;

    let mode_mbps = if pyrowave {
        let need = pw_needed(mode, yuv444);
        let cap = pw_budget(mode);
        need.max(f64::from(crate::profile::PYROWAVE_MBPS) * scale(mode) * if yuv444 { 1.5 } else { 1.0 }).min(cap.max(need))
    } else {
        mode_mbps_hevc
    };

    AutoResult {
        at: now,
        network: d.network.clone(),
        measures: m.clone(),
        codec: if pyrowave { Codec::PyroWave } else { fallback },
        yuv444,
        width: mode.width,
        height: mode.height,
        refresh: mode.refresh,
        mbps: (mode_mbps_hevc / scale(mode)).round().clamp(5.0, 4294.0) as u32,
        mode_mbps: mode_mbps.round() as u32,
        reliable,
        reasons: Reasons {
            codec: codec_reason,
            mode: mode_reason,
            yuv444: yuv444_reason,
            bitrate: bitrate_reason,
            transport: if reliable { Reason::Unsteady } else { Reason::Steady },
            decode_fps: (mode_reason == Reason::DecodeLimited).then_some(decode_limit).flatten(),
        },
        station: st.name.clone(),
    }
}

// ---------------------------------------------------------------- Extraits

/// Un extrait vidéo du test de décodage : des paquets comme ceux de Kyber
/// ([`crate::media::Packet::Media`]), en 3840 × 2160.
#[derive(Debug, Clone)]
pub struct Sample {
    pub codec: Codec,
    pub yuv444: bool,
    pub width: u32,
    pub height: u32,
    pub packets: Vec<SamplePacket>,
}

#[derive(Debug, Clone)]
pub struct SamplePacket {
    pub is_key: bool,
    pub data: Vec<u8>,
}

/// Lit un extrait au format `KNS1` (apps/core/scripts/samples.sh) : `KNS1`,
/// FourCC (u32), drapeaux (u8, 1 : 4:4:4), largeur et hauteur (u16), nombre
/// de paquets (u16), puis pour chaque paquet : drapeaux (u8, 1 : image clé),
/// taille (u32) et données. Entiers en petit-boutiste.
pub fn parse_sample(bytes: &[u8]) -> Option<Sample> {
    let mut r = bytes;
    let mut take = |n: usize| -> Option<&[u8]> {
        if r.len() < n {
            return None;
        }
        let (a, b) = r.split_at(n);
        r = b;
        Some(a)
    };
    if take(4)? != b"KNS1" {
        return None;
    }
    let fourcc = u32::from_le_bytes(take(4)?.try_into().ok()?);
    let flags = take(1)?[0];
    let width = u32::from(u16::from_le_bytes(take(2)?.try_into().ok()?));
    let height = u32::from(u16::from_le_bytes(take(2)?.try_into().ok()?));
    let count = u16::from_le_bytes(take(2)?.try_into().ok()?);
    let codec = [Codec::H264, Codec::Hevc, Codec::Av1, Codec::PyroWave].into_iter().find(|c| c.fourcc() == fourcc)?;
    let mut packets = Vec::with_capacity(count.into());
    for _ in 0..count {
        let f = take(1)?[0];
        let len = u32::from_le_bytes(take(4)?.try_into().ok()?) as usize;
        packets.push(SamplePacket { is_key: f & 1 == 1, data: take(len)?.to_vec() });
    }
    Some(Sample { codec, yuv444: flags & 1 == 1, width, height, packets })
}

/// Les extraits embarqués, dans l'ordre du test.
pub fn samples() -> Vec<Sample> {
    [
        &include_bytes!("../samples/hevc-420.kns")[..],
        &include_bytes!("../samples/hevc-444.kns")[..],
        &include_bytes!("../samples/av1-420.kns")[..],
        &include_bytes!("../samples/h264-420.kns")[..],
        &include_bytes!("../samples/pyrowave-420.kns")[..],
        &include_bytes!("../samples/pyrowave-444.kns")[..],
    ]
    .into_iter()
    .filter_map(parse_sample)
    .collect()
}

/// Les extraits que cet appareil peut essayer (`caps`).
pub fn samples_for(caps: Capabilities) -> Vec<Sample> {
    samples()
        .into_iter()
        .filter(|s| {
            s.codec.decodable(caps)
                && (!s.yuv444 || match s.codec {
                    Codec::Hevc => caps.hevc444,
                    Codec::PyroWave => caps.pyrowave,
                    _ => false,
                })
                && (s.codec != Codec::PyroWave || PYROWAVE_KYBER)
        })
        .collect()
}

// ---------------------------------------------------------------- Le test

/// Pourquoi le test s'arrête.
#[derive(Debug)]
pub enum AutoError {
    /// L'utilisateur l'a arrêté.
    Cancelled,
    /// La Station ne répond pas.
    Unreachable(ConsoleError),
}

impl AutoError {
    pub fn text(&self, lang: Lang, name: &str) -> String {
        match self {
            Self::Cancelled => tr(lang, "autotest.cancelled", &[]),
            Self::Unreachable(_) => tr(lang, "autotest.unreachable", &[("name", name)]),
        }
    }
}

/// Le décodeur de l'app : décode l'extrait aussi vite que possible, pendant
/// 1 s au plus, et rend les images par seconde (`None` : pas su le décoder).
pub trait SampleDecoder {
    fn decode(&mut self, sample: &Sample) -> Option<f64>;
}

impl<F: FnMut(&Sample) -> Option<f64>> SampleDecoder for F {
    fn decode(&mut self, sample: &Sample) -> Option<f64> {
        self(sample)
    }
}

const PINGS: usize = 20;
const PROBE_MS: u32 = 4000;

fn percentile(sorted: &[f64], p: f64) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }
    let i = ((sorted.len() - 1) as f64 * p).round() as usize;
    sorted[i.min(sorted.len() - 1)]
}

/// Latence au repos d'après des allers-retours (millisecondes).
pub fn latency_of(samples: &[f64]) -> Option<Latency> {
    if samples.is_empty() {
        return None;
    }
    let mut s = samples.to_vec();
    s.sort_by(f64::total_cmp);
    Some(Latency { min: s[0], median: percentile(&s, 0.5), jitter: percentile(&s, 0.9) - percentile(&s, 0.1) })
}

/// Débit d'une rafale lue : octets reçus par quart de seconde
/// (`windows`), médiane de la seconde moitié, en Mbit/s.
pub fn bandwidth_of(windows: &[u64], window: Duration) -> Option<f64> {
    if windows.len() < 2 {
        return None;
    }
    let mut half: Vec<f64> = windows[windows.len() / 2..].iter().map(|b| *b as f64 * 8.0 / window.as_secs_f64() / 1e6).collect();
    half.sort_by(f64::total_cmp);
    Some(percentile(&half, 0.5))
}

fn fmt_ms(lang: Lang, v: f64) -> String {
    let s = format!("{v:.1}");
    if lang == Lang::Fr { s.replace('.', ",") } else { s }
}

/// Mène le test : `progress` est appelé à chaque changement d'étape ;
/// `cancel` l'arrête (entre deux mesures). Bloquant (une dizaine de
/// secondes) : à lancer sur un fil à part.
pub fn run(
    console: Arc<Console>,
    device: &Device,
    decoder: &mut dyn SampleDecoder,
    progress: &mut dyn FnMut(&Progress),
    cancel: &AtomicBool,
    lang: Lang,
) -> Result<AutoResult, AutoError> {
    let mut p = Progress::new(lang);
    let check = |cancel: &AtomicBool| if cancel.load(Ordering::Relaxed) { Err(AutoError::Cancelled) } else { Ok(()) };

    // L'ordinateur : PyroWave, son nom.
    let info = console.info().map_err(AutoError::Unreachable)?;
    let station = Station {
        name: info.hostname.clone(),
        pyrowave: info.pyrowave_offered(),
        pyrowave_max: info.pyrowave.as_ref().and_then(|p| p.max_mbps),
        encoder_check: info.encoder_check,
    };

    // 1. Liaison.
    let link = if console.relay().is_some() {
        Link::Relay
    } else if device.wired {
        Link::Cable
    } else {
        Link::Wifi
    };
    p.set(Step::Link, StepState::Done(link.text(lang)));
    p.set(Step::Latency, StepState::Running);
    progress(&p);
    check(cancel)?;

    // 2. Latence au repos.
    let mut rtts = Vec::with_capacity(PINGS);
    let mut has_ping = true;
    for _ in 0..PINGS {
        check(cancel)?;
        let started = Instant::now();
        match console.ping() {
            Ok(Some(ms)) => rtts.push(ms),
            Ok(None) => {
                has_ping = false;
                break;
            }
            Err(e) => return Err(AutoError::Unreachable(e)),
        }
        if let Some(rest) = Duration::from_millis(100).checked_sub(started.elapsed()) {
            std::thread::sleep(rest);
        }
    }
    if !has_ping {
        // Station d'avant : l'annonce sert d'aller-retour (moins précis).
        for _ in 0..5 {
            let started = Instant::now();
            console.info().map_err(AutoError::Unreachable)?;
            rtts.push(started.elapsed().as_secs_f64() * 1000.0);
        }
    }
    let latency = latency_of(&rtts);
    if let Some(l) = latency {
        p.set(
            Step::Latency,
            StepState::Done(tr(lang, "autotest.latency", &[("ms", &fmt_ms(lang, l.median)), ("jitter", &fmt_ms(lang, l.jitter))])),
        );
    }
    p.set(Step::Bandwidth, StepState::Running);
    progress(&p);
    check(cancel)?;

    // 3. Débit : la rafale, avec des allers-retours en même temps.
    let bandwidth = match console.burst(PROBE_MS) {
        Ok(None) => None,
        Err(e) => return Err(AutoError::Unreachable(e)),
        Ok(Some(mut reader)) => {
            let stop = Arc::new(AtomicBool::new(false));
            let pinger = has_ping.then(|| {
                let (c, stop) = (console.clone(), stop.clone());
                std::thread::spawn(move || {
                    let mut out = Vec::new();
                    std::thread::sleep(Duration::from_millis(500));
                    while !stop.load(Ordering::Relaxed) {
                        if let Ok(Some(ms)) = c.ping() {
                            out.push(ms);
                        }
                        std::thread::sleep(Duration::from_millis(150));
                    }
                    out
                })
            });
            let window = Duration::from_millis(250);
            let mut windows: Vec<u64> = Vec::new();
            let mut buf = vec![0u8; 256 * 1024];
            let started = Instant::now();
            loop {
                if cancel.load(Ordering::Relaxed) {
                    stop.store(true, Ordering::Relaxed);
                    return Err(AutoError::Cancelled);
                }
                match reader.read(&mut buf) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => {
                        let i = (started.elapsed().as_millis() / window.as_millis()) as usize;
                        if windows.len() <= i {
                            windows.resize(i + 1, 0);
                        }
                        windows[i] += n as u64;
                    }
                }
            }
            stop.store(true, Ordering::Relaxed);
            let loaded = pinger.and_then(|h| h.join().ok()).unwrap_or_default();
            let loaded_delta = latency_of(&loaded).zip(latency).map(|(l, idle)| (l.median - idle.median).max(0.0));
            bandwidth_of(&windows, window).map(|mbps| Bandwidth { mbps, loaded_delta })
        }
    };
    match bandwidth {
        Some(b) => {
            let delta = b.loaded_delta.map(|d| format!("{}", d.round() as i64)).unwrap_or_else(|| "?".into());
            p.set(Step::Bandwidth, StepState::Done(tr(lang, "autotest.bandwidth", &[("mbps", &format!("{}", b.mbps.round() as u64)), ("delta", &delta)])));
        }
        None => p.set(Step::Bandwidth, StepState::Skipped(tr(lang, "autotest.bandwidth_old", &[("name", &station.name)]))),
    }
    p.set(Step::Decode, StepState::Running);
    progress(&p);
    check(cancel)?;

    // 4. Décodage.
    let mut decode = Vec::new();
    for s in samples_for(device.caps) {
        check(cancel)?;
        if let Some(fps) = decoder.decode(&s) {
            decode.push(DecodeResult { codec: s.codec, yuv444: s.yuv444, fps });
        }
    }
    let summary: Vec<String> = [Codec::PyroWave, Codec::Hevc, Codec::Av1, Codec::H264]
        .iter()
        .filter_map(|&c| decode.iter().filter(|d| d.codec == c).map(|d| d.fps).reduce(f64::max).map(|fps| (c, fps)))
        .map(|(c, fps)| tr(lang, "autotest.decode", &[("codec", c.label()), ("fps", &format!("{}", fps.round() as u32))]))
        .collect();
    p.set(
        Step::Decode,
        if summary.is_empty() { StepState::Skipped(tr(lang, "autotest.decode_none", &[])) } else { StepState::Done(summary.join(" · ")) },
    );
    p.set(Step::Encoder, StepState::Running);
    progress(&p);
    check(cancel)?;

    // 5. L'encodeur de l'ordinateur : les choix d'abord, puis leur débit essayé.
    let network = if link == Link::Relay { Some("relay".to_string()) } else { device.network.clone() };
    let device = Device { network, ..device.clone() };
    let mut measures = Measures { link, latency, bandwidth, decode, encoder: None };
    let first = decide(&measures, &device, &station, crate::account::now_secs());
    let name = station.name.as_str();
    let state = if first.codec == Codec::PyroWave {
        StepState::Skipped(tr(lang, "autotest.encoder_pyrowave", &[]))
    } else if !station.encoder_check {
        StepState::Skipped(tr(lang, "autotest.encoder_old", &[("name", name)]))
    } else {
        let query = EncoderQuery {
            codec: first.codec,
            width: first.width,
            height: first.height,
            fps: first.refresh,
            mbps: first.mode_mbps,
            yuv444: first.yuv444,
            hdr: device.hdr,
        };
        match search_encoder(&console, query, cancel)? {
            Some(e) => {
                measures.encoder = Some(e);
                let key = if e.limited { "autotest.encoder_limited" } else { "autotest.encoder_ok" };
                StepState::Done(tr(lang, key, &[("n", &e.mbps.to_string()), ("codec", e.codec.label())]))
            }
            None => StepState::Skipped(tr(lang, "autotest.encoder_unknown", &[("name", name)])),
        }
    };
    p.set(Step::Encoder, state);
    progress(&p);
    Ok(decide(&measures, &device, &station, crate::account::now_secs()))
}

/// Pas de la descente et débit le plus bas essayé (Mbit/s par écran).
const ENCODER_STEP: u32 = 50;

/// Le débit le plus haut que l'encodeur accepte, en partant de `q.mbps` et
/// en descendant de 50 Mbit/s à chaque refus (50 au plus bas). `None` : la
/// Station n'a pas pu essayer (ou répond 404). Refusé jusqu'à 50 compris :
/// 50, limité (la session redescendra encore si besoin).
fn search_encoder(console: &Console, mut q: EncoderQuery, cancel: &AtomicBool) -> Result<Option<EncoderMeasure>, AutoError> {
    let mut limited = false;
    loop {
        if cancel.load(Ordering::Relaxed) {
            return Err(AutoError::Cancelled);
        }
        let answer = match console.encoder_check(&q) {
            Ok(a) => a,
            Err(e) => {
                log::info!("profil Auto : sonde de l'encodeur impossible ({e})");
                return Ok(None);
            }
        };
        let measure = |mbps, limited| EncoderMeasure { codec: q.codec, yuv444: q.yuv444, width: q.width, height: q.height, refresh: q.fps, mbps, limited };
        match answer {
            Some(EncoderAnswer::Accepted) => return Ok(Some(measure(q.mbps, limited))),
            Some(EncoderAnswer::Refused(why)) => {
                log::info!("profil Auto : {} à {} Mbit/s refusé par l'encodeur ({why})", q.codec.label(), q.mbps);
                limited = true;
                match next_step(q.mbps) {
                    Some(n) => q.mbps = n,
                    None => return Ok(Some(measure(q.mbps.min(ENCODER_STEP), true))),
                }
            }
            Some(EncoderAnswer::Unknown(why)) => {
                log::info!("profil Auto : l'ordinateur n'a pas pu essayer son encodeur ({why})");
                return Ok(None);
            }
            None => return Ok(None),
        }
    }
}

/// Le débit suivant de la descente : 50 Mbit/s de moins, 50 au plus bas ;
/// `None` une fois 50 essayé.
fn next_step(mbps: u32) -> Option<u32> {
    (mbps > ENCODER_STEP).then(|| mbps.saturating_sub(ENCODER_STEP).max(ENCODER_STEP))
}

#[cfg(test)]
mod tests;
