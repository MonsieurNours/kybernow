//! Caméra de l'appareil vers l'ordinateur (docs/webcam.md, règles dans
//! docs/link-ui.md, « Caméra de l'appareil »).
//!
//! L'app capture et encode (H.264 ou HEVC, matériel) ; le cœur porte le
//! reste : le flux montant de Kyber (correctif 0085, `kyclient::uplink`), les
//! paquets START / MEDIA / PAUSE / STOP, les réponses de l'ordinateur, la
//! reprise après une relance de l'image, les images lâchées quand le réseau
//! sature (jamais d'attente : la souris passe avant), le débit visé et la
//! ligne des statistiques.
//!
//! Une [`CameraLink`] par session ([`crate::session::Session::camera`]).
//! L'app l'appelle de n'importe quel fil :
//! - [`CameraLink::offered`] : l'ordinateur propose la caméra pour cette
//!   image (sinon l'entrée « Caméra » est absente) ; changement annoncé par
//!   `Platform::on_camera_offer`.
//! - [`CameraLink::start`] avec [`CameraSettings`] (« Allumer la caméra »,
//!   ou une autre caméra choisie : START de nouveau, sans éteindre), puis
//!   [`CameraLink::frame`] pour chaque image encodée, [`CameraLink::pause`]
//!   (app en arrière-plan) et [`CameraLink::stop`].
//! - [`CameraLink::poll`] : ce que l'ordinateur a répondu ([`CameraEvent`]).
//! - [`CameraLink::target_bitrate`] : le débit à donner à l'encodeur.

use std::collections::VecDeque;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use kyclient::uplink::{Uplink, UplinkSend};
use serde::{Deserialize, Serialize};

use crate::i18n::{self, Lang};

/// Le nom du flux montant de la caméra (Kyber, correctif 0085).
pub const KIND: &str = "camera";

pub const START: u8 = 0x01;
pub const MEDIA: u8 = 0x02;
pub const STOP: u8 = 0x03;
pub const PAUSE: u8 = 0x04;
pub const STATUS: u8 = 0x10;

/// Drapeaux d'un paquet MEDIA : image clé ; bits 1 et 2, les quarts de tour
/// (sens horaire) qui redressent l'image (Android encode l'image du capteur
/// telle quelle : l'ordinateur la tourne).
pub const FLAG_KEY: u8 = 1;
pub const ROTATION_SHIFT: u8 = 1;

/// Images par seconde, toujours.
pub const FPS: u32 = 30;

/// Définition de la caméra (réglage de l'ordinateur « Définition de la
/// caméra »).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum CameraResolution {
    #[default]
    #[serde(rename = "720p")]
    P720,
    #[serde(rename = "1080p")]
    P1080,
}

impl CameraResolution {
    pub fn size(self) -> (u32, u32) {
        match self {
            CameraResolution::P720 => (1280, 720),
            CameraResolution::P1080 => (1920, 1080),
        }
    }

    /// « 720p », « 1080p » (le réglage).
    pub fn label(self) -> &'static str {
        match self {
            CameraResolution::P720 => "720p",
            CameraResolution::P1080 => "1080p",
        }
    }

    /// Débit de départ, en bit/s : 1,5 Mbit/s en 720p, 4 Mbit/s en 1080p ;
    /// la moitié par le relais.
    pub fn base_bitrate(self, relay: bool) -> u32 {
        let b = match self {
            CameraResolution::P720 => 1_500_000,
            CameraResolution::P1080 => 4_000_000,
        };
        if relay { b / 2 } else { b }
    }

    pub fn from_label(s: &str) -> CameraResolution {
        if s == "1080p" { CameraResolution::P1080 } else { CameraResolution::P720 }
    }
}

/// Format de la caméra envoyée.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CameraCodec {
    H264,
    Hevc,
}

impl CameraCodec {
    pub fn label(self) -> &'static str {
        match self {
            CameraCodec::H264 => "H.264",
            CameraCodec::Hevc => "HEVC",
        }
    }
}

/// Ce que l'app envoie au départ (paquet START).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CameraSettings {
    pub codec: CameraCodec,
    /// Taille des images encodées (peut être plus petite que la définition
    /// demandée : une caméra qui ne la sait pas envoie la plus proche en
    /// dessous).
    pub width: u32,
    pub height: u32,
    pub fps: u32,
    /// La définition que l'ordinateur montre (720p ou 1080p).
    pub resolution: CameraResolution,
    /// Le nom de l'appareil (« iPhone de Marie ») : l'ordinateur le rend à
    /// un autre appareil qui voudrait envoyer la sienne.
    pub device: String,
}

/// Réponse de l'ordinateur, ou état du flux.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CameraEvent {
    /// La caméra part : le voyant s'allume.
    Started,
    /// Un autre appareil envoie déjà la sienne (son nom, vide s'il ne l'a
    /// pas donné).
    Busy { device: String },
    /// L'ordinateur n'a pas pu l'ouvrir (caméra virtuelle absente, en panne).
    Failed { reason: String },
    /// Arrêtée (après [`CameraLink::stop`]).
    Stopped,
    /// L'ordinateur demande une image clé : la prochaine image doit l'être.
    KeyframeWanted,
    /// L'ordinateur la propose (`true`) ou plus (`false`, image relancée ou
    /// arrêtée). Allumée, elle repart d'elle-même quand le flux revient.
    Offer(bool),
}

/// Ce qu'est devenue une image donnée à [`CameraLink::frame`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameOutcome {
    Sent,
    /// Lâchée (réseau saturé, ou en attente d'une image clé) : la prochaine
    /// doit être une image clé.
    WantKeyframe,
    /// Caméra éteinte, ou plus de flux : rien n'est parti.
    Off,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum State {
    Off,
    /// START envoyé, en attente de `started`.
    Starting,
    On,
    Paused,
}

/// Mesures de la dernière seconde (ligne « Caméra » des statistiques).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct CameraStats {
    pub sending: bool,
    pub paused: bool,
    pub width: u32,
    pub height: u32,
    pub fps: u32,
    pub codec: Option<CameraCodec>,
    /// Débit envoyé sur la dernière seconde, en bit/s.
    pub bitrate: u32,
    /// Images lâchées depuis le départ.
    pub dropped: u64,
}

struct Inner {
    uplink: Option<Uplink>,
    state: State,
    settings: Option<CameraSettings>,
    relay: bool,
    /// Une image clé est attendue avant de renvoyer des images.
    want_key: bool,
    /// Débit visé, en bit/s, ajusté selon les images lâchées.
    target: u32,
    last_drop: Option<Instant>,
    last_raise: Instant,
    /// (instant, octets) des paquets partis dans la dernière seconde.
    sent: VecDeque<(Instant, usize)>,
    frames: VecDeque<Instant>,
    dropped: u64,
    events: VecDeque<CameraEvent>,
}

/// La caméra d'une session.
pub struct CameraLink {
    inner: Mutex<Inner>,
}

impl Default for CameraLink {
    fn default() -> Self {
        Self::new()
    }
}

/// Le débit ne descend pas sous ce plancher.
const MIN_BITRATE: u32 = 300_000;
/// Sans image lâchée pendant ce temps, le débit remonte d'un cran.
const RAISE_AFTER: Duration = Duration::from_secs(5);

impl CameraLink {
    pub fn new() -> Self {
        CameraLink {
            inner: Mutex::new(Inner {
                uplink: None,
                state: State::Off,
                settings: None,
                relay: false,
                want_key: false,
                target: CameraResolution::P720.base_bitrate(false),
                last_drop: None,
                last_raise: Instant::now(),
                sent: VecDeque::new(),
                frames: VecDeque::new(),
                dropped: 0,
                events: VecDeque::new(),
            }),
        }
    }

    /// Le flux de la caméra de l'image en cours (le cœur l'appelle à chaque
    /// départ et arrêt de l'image). Allumée, la caméra repart sur le
    /// nouveau flux (START de nouveau, puis une image clé).
    pub(crate) fn set_uplink(&self, uplink: Option<Uplink>) -> bool {
        let mut i = self.inner.lock().unwrap();
        let was = i.uplink.is_some();
        let now = uplink.is_some();
        i.uplink = uplink;
        if now && matches!(i.state, State::Starting | State::On | State::Paused) {
            if let Some(s) = i.settings.clone() {
                let paused = i.state == State::Paused;
                i.state = State::Starting;
                i.want_key = true;
                Self::send_start(&mut i, &s);
                if paused {
                    Self::send(&mut i, vec![PAUSE]);
                    i.state = State::Paused;
                }
            }
        }
        if was != now {
            i.events.push_back(CameraEvent::Offer(now));
        }
        was != now
    }

    pub(crate) fn set_relay(&self, relay: bool) {
        let mut i = self.inner.lock().unwrap();
        if i.relay != relay {
            i.relay = relay;
            let resolution = i.settings.as_ref().map(|s| s.resolution).unwrap_or_default();
            i.target = resolution.base_bitrate(relay);
        }
    }

    /// L'ordinateur propose la caméra pour cette image.
    pub fn offered(&self) -> bool {
        let i = self.inner.lock().unwrap();
        i.uplink.as_ref().is_some_and(|u| !u.is_closed())
    }

    /// La caméra est allumée (en cours d'allumage, envoyée ou en pause).
    pub fn is_on(&self) -> bool {
        self.inner.lock().unwrap().state != State::Off
    }

    /// Envoyée (voyant allumé) : l'ordinateur l'a acceptée.
    pub fn is_sending(&self) -> bool {
        matches!(self.inner.lock().unwrap().state, State::On | State::Paused)
    }

    /// Allume la caméra, ou change de caméra ou de définition sans
    /// l'éteindre. `false` si l'ordinateur ne la propose pas.
    pub fn start(&self, settings: CameraSettings) -> bool {
        let mut i = self.inner.lock().unwrap();
        if i.uplink.is_none() {
            return false;
        }
        let first = i.state == State::Off;
        if first || i.settings.as_ref().map(|s| s.resolution) != Some(settings.resolution) {
            i.target = settings.resolution.base_bitrate(i.relay);
            i.last_drop = None;
            i.last_raise = Instant::now();
        }
        if first {
            i.dropped = 0;
            i.sent.clear();
            i.frames.clear();
        }
        i.want_key = true;
        if first {
            i.state = State::Starting;
        }
        i.settings = Some(settings.clone());
        Self::send_start(&mut i, &settings)
    }

    /// Une image encodée (Annex B, paramètres avec chaque image clé),
    /// horodatée en microsecondes.
    pub fn frame(&self, data: &[u8], pts_us: u64, key: bool) -> FrameOutcome {
        self.frame_rotated(data, pts_us, key, 0)
    }

    /// Comme [`Self::frame`], avec les quarts de tour (0 à 3, sens horaire)
    /// que l'ordinateur applique pour redresser l'image.
    pub fn frame_rotated(&self, data: &[u8], pts_us: u64, key: bool, quarter_turns: u8) -> FrameOutcome {
        let mut i = self.inner.lock().unwrap();
        match i.state {
            State::Off => return FrameOutcome::Off,
            State::Paused => i.state = State::On,
            State::Starting | State::On => {}
        }
        if i.uplink.is_none() {
            return FrameOutcome::Off;
        }
        if i.want_key && !key {
            return FrameOutcome::WantKeyframe;
        }
        let mut payload = Vec::with_capacity(data.len() + 10);
        payload.push(MEDIA);
        payload.push((if key { FLAG_KEY } else { 0 }) | ((quarter_turns & 3) << ROTATION_SHIFT));
        payload.extend_from_slice(&pts_us.to_be_bytes());
        payload.extend_from_slice(data);
        let len = payload.len();
        match Self::send(&mut i, payload) {
            UplinkSend::Sent => {
                if key {
                    i.want_key = false;
                }
                let now = Instant::now();
                i.sent.push_back((now, len));
                i.frames.push_back(now);
                Self::trim(&mut i, now);
                Self::maybe_raise(&mut i, now);
                FrameOutcome::Sent
            }
            UplinkSend::Full => {
                // Le réseau ne suit pas : on lâche, on baisse, et on repart
                // d'une image clé (les suivantes en dépendraient).
                let now = Instant::now();
                i.dropped += 1;
                i.want_key = true;
                if i.last_drop.is_none_or(|t| now.duration_since(t) > Duration::from_millis(500)) {
                    i.target = (i.target / 4 * 3).max(MIN_BITRATE);
                }
                i.last_drop = Some(now);
                i.last_raise = now;
                FrameOutcome::WantKeyframe
            }
            UplinkSend::Closed => {
                i.want_key = true;
                FrameOutcome::Off
            }
        }
    }

    /// L'app passe en arrière-plan : l'ordinateur garde la dernière image.
    /// La prochaine image (au retour) repart d'une image clé.
    pub fn pause(&self) {
        let mut i = self.inner.lock().unwrap();
        if matches!(i.state, State::On | State::Starting) {
            Self::send(&mut i, vec![PAUSE]);
            i.state = State::Paused;
            i.want_key = true;
        }
    }

    /// « Éteindre la caméra » (ou fin de session, droit repris).
    pub fn stop(&self) {
        let mut i = self.inner.lock().unwrap();
        if i.state != State::Off {
            Self::send(&mut i, vec![STOP]);
        }
        i.state = State::Off;
        i.settings = None;
        i.want_key = false;
        i.sent.clear();
        i.frames.clear();
    }

    /// Ce qui est arrivé depuis le dernier appel.
    pub fn poll(&self) -> Vec<CameraEvent> {
        let mut i = self.inner.lock().unwrap();
        let mut packets = Vec::new();
        if let Some(u) = &i.uplink {
            while let Some(p) = u.try_recv() {
                packets.push(p);
            }
        }
        for p in packets {
            if let Some(e) = parse_status(&p) {
                match &e {
                    CameraEvent::Started => {
                        if i.state == State::Starting {
                            i.state = State::On;
                        }
                    }
                    CameraEvent::Busy { .. } | CameraEvent::Failed { .. } => {
                        i.state = State::Off;
                        i.settings = None;
                    }
                    CameraEvent::KeyframeWanted => i.want_key = true,
                    _ => {}
                }
                i.events.push_back(e);
            }
        }
        i.events.drain(..).collect()
    }

    /// Le débit à donner à l'encodeur, en bit/s.
    pub fn target_bitrate(&self) -> u32 {
        self.inner.lock().unwrap().target
    }

    pub fn stats(&self) -> CameraStats {
        let mut i = self.inner.lock().unwrap();
        let now = Instant::now();
        Self::trim(&mut i, now);
        let bytes: usize = i.sent.iter().map(|(_, n)| n).sum();
        let s = i.settings.clone();
        CameraStats {
            sending: matches!(i.state, State::On | State::Paused),
            paused: i.state == State::Paused,
            width: s.as_ref().map_or(0, |s| s.width),
            height: s.as_ref().map_or(0, |s| s.height),
            fps: i.frames.len() as u32,
            codec: s.as_ref().map(|s| s.codec),
            bitrate: u32::try_from(bytes * 8).unwrap_or(u32::MAX),
            dropped: i.dropped,
        }
    }

    fn send(i: &mut Inner, payload: Vec<u8>) -> UplinkSend {
        match &i.uplink {
            Some(u) => u.try_send(payload),
            None => UplinkSend::Closed,
        }
    }

    fn send_start(i: &mut Inner, s: &CameraSettings) -> bool {
        let mut payload = vec![START];
        payload.extend_from_slice(serde_json::to_string(s).unwrap_or_default().as_bytes());
        // START ne doit pas se perdre : la file vient d'être vidée par une
        // image lâchée au pire ; un refus ici, c'est un flux fermé.
        Self::send(i, payload) == UplinkSend::Sent
    }

    fn trim(i: &mut Inner, now: Instant) {
        let second = Duration::from_secs(1);
        while i.sent.front().is_some_and(|(t, _)| now.duration_since(*t) > second) {
            i.sent.pop_front();
        }
        while i.frames.front().is_some_and(|t| now.duration_since(*t) > second) {
            i.frames.pop_front();
        }
    }

    fn maybe_raise(i: &mut Inner, now: Instant) {
        let Some(s) = &i.settings else { return };
        let base = s.resolution.base_bitrate(i.relay);
        if i.target < base && now.duration_since(i.last_raise) >= RAISE_AFTER {
            i.target = (i.target + i.target / 10).min(base);
            i.last_raise = now;
        }
    }
}

/// Un paquet STATUS de l'ordinateur.
pub fn parse_status(packet: &[u8]) -> Option<CameraEvent> {
    if packet.first() != Some(&STATUS) {
        return None;
    }
    let v: serde_json::Value = serde_json::from_slice(&packet[1..]).ok()?;
    if v.get("keyframe").and_then(|k| k.as_bool()) == Some(true) {
        return Some(CameraEvent::KeyframeWanted);
    }
    let text = |k: &str| v.get(k).and_then(|x| x.as_str()).unwrap_or_default().to_owned();
    match v.get("state")?.as_str()? {
        "started" => Some(CameraEvent::Started),
        "busy" => Some(CameraEvent::Busy { device: text("device") }),
        "failed" => Some(CameraEvent::Failed { reason: text("reason") }),
        "stopped" => Some(CameraEvent::Stopped),
        _ => None,
    }
}

/// La ligne « Caméra » des statistiques : « 1280 × 720 · 30 i/s · H.264 ·
/// 1,4 Mbit/s », « En pause » ; `None` caméra éteinte.
pub fn stats_line(stats: &CameraStats, lang: Lang) -> Option<String> {
    if !stats.sending {
        return None;
    }
    if stats.paused {
        return Some(i18n::tr(lang, "camera.paused", &[]));
    }
    let mbps = stats.bitrate as f64 / 1_000_000.0;
    let mbps = match lang {
        Lang::Fr => format!("{mbps:.1}").replace('.', ","),
        Lang::En => format!("{mbps:.1}"),
    };
    let fps = match lang {
        Lang::Fr => "i/s",
        Lang::En => "fps",
    };
    Some(format!(
        "{} × {} · {} {} · {} · {} Mbit/s",
        stats.width,
        stats.height,
        stats.fps,
        fps,
        stats.codec.map_or("", |c| c.label()),
        mbps
    ))
}

/// Le texte de l'avis d'un refus, d'après docs/link-ui.md ; `computer` est
/// le nom de l'ordinateur.
pub fn event_notice(event: &CameraEvent, computer: &str, lang: Lang) -> Option<String> {
    match event {
        CameraEvent::Busy { device } if !device.is_empty() => {
            Some(i18n::tr(lang, "camera.busy", &[("device", device), ("name", computer)]))
        }
        CameraEvent::Busy { .. } => Some(i18n::tr(lang, "camera.busy_unnamed", &[("name", computer)])),
        CameraEvent::Failed { .. } => Some(i18n::tr(lang, "camera.failed", &[("name", computer)])),
        _ => None,
    }
}

/// Les avis de l'appareil (docs/link-ui.md), par leur nom.
pub fn device_notice(which: DeviceNotice, camera: &str, lang: Lang) -> String {
    match which {
        DeviceNotice::Denied => i18n::tr(lang, "camera.denied", &[]),
        DeviceNotice::InUse => i18n::tr(lang, "camera.in_use", &[]),
        DeviceNotice::Unplugged => i18n::tr(lang, "camera.unplugged", &[("camera", camera)]),
        DeviceNotice::NoEncoder => i18n::tr(lang, "camera.no_encoder", &[]),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceNotice {
    Denied,
    InUse,
    Unplugged,
    NoEncoder,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn settings(resolution: CameraResolution) -> CameraSettings {
        let (width, height) = resolution.size();
        CameraSettings {
            codec: CameraCodec::H264,
            width,
            height,
            fps: FPS,
            resolution,
            device: "iPhone".into(),
        }
    }

    #[test]
    fn statuses() {
        let p = |s: &str| {
            let mut v = vec![STATUS];
            v.extend_from_slice(s.as_bytes());
            parse_status(&v)
        };
        assert_eq!(p(r#"{"state":"started"}"#), Some(CameraEvent::Started));
        assert_eq!(p(r#"{"state":"busy","device":"Pixel"}"#), Some(CameraEvent::Busy { device: "Pixel".into() }));
        assert_eq!(p(r#"{"state":"failed","reason":"sink"}"#), Some(CameraEvent::Failed { reason: "sink".into() }));
        assert_eq!(p(r#"{"keyframe":true}"#), Some(CameraEvent::KeyframeWanted));
        assert_eq!(p(r#"{"other":1}"#), None);
        assert_eq!(parse_status(&[MEDIA, 0]), None);
    }

    #[test]
    fn nothing_without_the_computer() {
        let c = CameraLink::new();
        assert!(!c.offered());
        assert!(!c.start(settings(CameraResolution::P720)));
        assert_eq!(c.frame(&[0, 0, 1], 0, true), FrameOutcome::Off);
        assert!(!c.is_on());
    }

    #[test]
    fn bitrates() {
        assert_eq!(CameraResolution::P720.base_bitrate(false), 1_500_000);
        assert_eq!(CameraResolution::P1080.base_bitrate(false), 4_000_000);
        assert_eq!(CameraResolution::P1080.base_bitrate(true), 2_000_000);
        assert_eq!(CameraResolution::from_label("1080p"), CameraResolution::P1080);
        assert_eq!(CameraResolution::from_label("x"), CameraResolution::P720);
        assert_eq!(serde_json::to_string(&CameraResolution::P1080).unwrap(), "\"1080p\"");
    }

    #[test]
    fn stats_lines() {
        let mut s = CameraStats { sending: true, width: 1280, height: 720, fps: 30, codec: Some(CameraCodec::H264), bitrate: 1_400_000, ..Default::default() };
        assert_eq!(stats_line(&s, Lang::Fr).unwrap(), "1280 × 720 · 30 i/s · H.264 · 1,4 Mbit/s");
        assert_eq!(stats_line(&s, Lang::En).unwrap(), "1280 × 720 · 30 fps · H.264 · 1.4 Mbit/s");
        s.paused = true;
        assert_eq!(stats_line(&s, Lang::Fr).unwrap(), "En pause");
        assert_eq!(stats_line(&CameraStats::default(), Lang::Fr), None);
    }

    fn answer(peer: &kyclient::uplink::UplinkPeer, json: &str) {
        let mut v = vec![STATUS];
        v.extend_from_slice(json.as_bytes());
        peer.answer.send(v).unwrap();
    }

    #[test]
    fn start_frames_and_stop() {
        let c = CameraLink::new();
        let (up, mut peer) = Uplink::pair(KIND, 4);
        assert!(c.set_uplink(Some(up)));
        assert_eq!(c.poll(), vec![CameraEvent::Offer(true)]);
        assert!(c.offered());
        assert!(c.start(settings(CameraResolution::P720)));
        let start = peer.sent.try_recv().unwrap();
        assert_eq!(start[0], START);
        let json: serde_json::Value = serde_json::from_slice(&start[1..]).unwrap();
        assert_eq!(json["codec"], "h264");
        assert_eq!(json["resolution"], "720p");
        assert_eq!(json["device"], "iPhone");
        assert!(c.is_on() && !c.is_sending());
        // Une image qui n'est pas clé attend la première image clé.
        assert_eq!(c.frame(&[1], 0, false), FrameOutcome::WantKeyframe);
        assert_eq!(c.frame(&[0, 0, 1, 0x65], 33_000, true), FrameOutcome::Sent);
        let media = peer.sent.try_recv().unwrap();
        assert_eq!(media[0], MEDIA);
        assert_eq!(media[1], FLAG_KEY);
        assert_eq!(u64::from_be_bytes(media[2..10].try_into().unwrap()), 33_000);
        assert_eq!(&media[10..], &[0, 0, 1, 0x65]);
        answer(&peer, r#"{"state":"started"}"#);
        assert_eq!(c.poll(), vec![CameraEvent::Started]);
        assert!(c.is_sending());
        assert_eq!(c.frame(&[2], 66_000, false), FrameOutcome::Sent);
        assert_eq!(peer.sent.try_recv().unwrap()[0], MEDIA);
        c.pause();
        assert_eq!(peer.sent.try_recv().unwrap(), vec![PAUSE]);
        // Au retour, l'envoi repart d'une image clé.
        assert_eq!(c.frame(&[3], 99_000, false), FrameOutcome::WantKeyframe);
        assert_eq!(c.frame(&[4], 99_000, true), FrameOutcome::Sent);
        c.stop();
        assert!(!c.is_on());
        assert_eq!(c.frame(&[3], 0, true), FrameOutcome::Off);
    }

    #[test]
    fn rotation_rides_in_the_flags() {
        let c = CameraLink::new();
        let (up, mut peer) = Uplink::pair(KIND, 8);
        c.set_uplink(Some(up));
        assert!(c.start(settings(CameraResolution::P720)));
        peer.sent.try_recv().unwrap();
        assert_eq!(c.frame_rotated(&[9], 0, true, 3), FrameOutcome::Sent);
        let media = peer.sent.try_recv().unwrap();
        assert_eq!(media[1], FLAG_KEY | (3 << ROTATION_SHIFT));
    }

    #[test]
    fn a_full_queue_drops_and_lowers() {
        let c = CameraLink::new();
        let (up, mut peer) = Uplink::pair(KIND, 2);
        c.set_uplink(Some(up));
        assert!(c.start(settings(CameraResolution::P1080)));
        assert_eq!(c.target_bitrate(), 4_000_000);
        // START occupe une place ; une image clé la seconde ; puis plein.
        assert_eq!(c.frame(&[0], 0, true), FrameOutcome::Sent);
        assert_eq!(c.frame(&[1], 1, false), FrameOutcome::WantKeyframe);
        assert_eq!(c.target_bitrate(), 3_000_000);
        assert_eq!(c.stats().dropped, 1);
        // La file se vide : seule une image clé repart.
        while peer.sent.try_recv().is_ok() {}
        assert_eq!(c.frame(&[2], 2, false), FrameOutcome::WantKeyframe);
        assert_eq!(c.frame(&[3], 3, true), FrameOutcome::Sent);
        assert_eq!(c.frame(&[4], 4, false), FrameOutcome::Sent);
    }

    #[test]
    fn busy_and_failed_turn_it_off() {
        let c = CameraLink::new();
        let (up, peer) = Uplink::pair(KIND, 8);
        c.set_uplink(Some(up));
        c.poll();
        assert!(c.start(settings(CameraResolution::P720)));
        answer(&peer, r#"{"state":"busy","device":"Pixel"}"#);
        assert_eq!(c.poll(), vec![CameraEvent::Busy { device: "Pixel".into() }]);
        assert!(!c.is_on());
        assert!(c.start(settings(CameraResolution::P720)));
        answer(&peer, r#"{"state":"failed","reason":"sink"}"#);
        assert_eq!(c.poll(), vec![CameraEvent::Failed { reason: "sink".into() }]);
        assert!(!c.is_on());
    }

    #[test]
    fn a_new_stream_starts_it_again() {
        let c = CameraLink::new();
        let (up, mut peer) = Uplink::pair(KIND, 8);
        c.set_uplink(Some(up));
        assert_eq!(c.poll(), vec![CameraEvent::Offer(true)]);
        assert!(c.start(settings(CameraResolution::P720)));
        assert_eq!(c.frame(&[0], 0, true), FrameOutcome::Sent);
        // L'image est relancée : plus de flux, puis un nouveau.
        assert!(c.set_uplink(None));
        assert!(!c.offered() && c.is_on());
        let (up2, mut peer2) = Uplink::pair(KIND, 8);
        assert!(c.set_uplink(Some(up2)));
        assert_eq!(c.poll(), vec![CameraEvent::Offer(false), CameraEvent::Offer(true)]);
        assert_eq!(peer2.sent.try_recv().unwrap()[0], START);
        assert_eq!(c.frame(&[1], 1, false), FrameOutcome::WantKeyframe);
        assert_eq!(c.frame(&[2], 2, true), FrameOutcome::Sent);
        while peer.sent.try_recv().is_ok() {}
    }

    #[test]
    fn notices() {
        let busy = CameraEvent::Busy { device: "Pixel 9".into() };
        assert_eq!(event_notice(&busy, "Atelier", Lang::Fr).unwrap(), "Pixel 9 envoie déjà sa caméra à Atelier.");
        assert_eq!(event_notice(&busy, "Atelier", Lang::En).unwrap(), "Pixel 9 is already sending its camera to Atelier.");
        assert_eq!(
            event_notice(&CameraEvent::Failed { reason: "sink".into() }, "Atelier", Lang::Fr).unwrap(),
            "Atelier n'a pas pu ouvrir sa caméra virtuelle."
        );
        assert_eq!(event_notice(&CameraEvent::Started, "Atelier", Lang::Fr), None);
        assert_eq!(
            device_notice(DeviceNotice::Unplugged, "Logitech C920", Lang::Fr),
            "Logitech C920 n'est plus branchée."
        );
    }
}
