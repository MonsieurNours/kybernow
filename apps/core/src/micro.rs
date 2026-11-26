//! Le micro de l'appareil, envoyé à l'ordinateur (docs/micro.md).
//!
//! L'app capte son micro comme son système le permet (traitement voix
//! compris) et donne ses échantillons à [`crate::session::Session::push_microphone`] :
//! flottants mono, à n'importe quelle fréquence. Le cœur les ramène à 48 kHz,
//! les découpe en trames de 10 ms, les encode en Opus (voix, 32 kbit/s,
//! correction d'erreurs intégrée) et les envoie sur le flux montant commun
//! (correctif 0085, type `microphone`, paquets START/MEDIA/PAUSE).
//!
//! Une app qui a un micro le dit une fois au lancement ([`set_device_has_microphone`]) :
//! le micro est alors demandé à chaque démarrage de l'image, même coupé (rien
//! ne part micro coupé), si bien que l'ordinateur dit tout de suite s'il le
//! prend et que l'allumer en session ne relance rien. L'état arrive à l'app par
//! [`crate::session::Platform::on_microphone`] ; elle capte tant qu'il est
//! [`MicState::On`] ou [`MicState::Waiting`].

use std::ffi::c_void;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use kyclient::uplink::Uplink;
use serde::Serialize;

use crate::i18n;

fn t(key: &str, params: &[(&str, &str)]) -> String {
    i18n::tr(i18n::current(), key, params)
}

/// 48 kHz, trames de 10 ms.
pub const SAMPLE_RATE: u32 = 48_000;
pub const FRAME: usize = 480;
/// Débit de l'encodeur, en bit/s.
pub const BITRATE: i32 = 32_000;
/// Pertes prévues pour la correction d'erreurs d'Opus, en %.
pub const EXPECTED_LOSS: i32 = 10;

static HAS_MICROPHONE: AtomicBool = AtomicBool::new(false);

/// L'appareil a un micro (Mac, iPhone, iPad, Android, Windows, Linux, web ;
/// jamais l'Apple TV ni les téléviseurs) : le micro est proposé aux
/// ordinateurs. Coupé par défaut : une app qui ne l'appelle pas ne propose
/// rien, comme avant.
pub fn set_device_has_microphone(has: bool) {
    HAS_MICROPHONE.store(has, Ordering::Relaxed);
}

pub fn device_has_microphone() -> bool {
    HAS_MICROPHONE.load(Ordering::Relaxed)
}

/// Pourquoi l'ordinateur ne prend pas le micro.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MicRefusal {
    /// Invité d'un partage que l'hôte ne laisse pas parler.
    NotAllowed,
    /// Pas de micro virtuel sur l'ordinateur.
    NoDevice,
    /// La Station ne peut pas le prendre.
    Unsupported,
    /// Station d'avant le micro.
    OldStation,
}

impl MicRefusal {
    /// Le texte de link-ui.md (« Micro »), dans la langue de l'app.
    pub fn text(self) -> String {
        t(
            match self {
                Self::NotAllowed => "micro.not_allowed",
                Self::NoDevice => "micro.no_device",
                Self::Unsupported => "micro.unsupported",
                Self::OldStation => "micro.old_station",
            },
            &[],
        )
    }
}

/// Ce que montrent la puce et la tuile « Micro ».
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "state", content = "reason", rename_all = "snake_case")]
pub enum MicState {
    /// Coupé (ou pas de micro sur l'appareil) : l'app ne capte pas.
    Off,
    /// Allumé, la réponse de l'ordinateur pas encore venue : l'app capte
    /// (les échantillons sont jetés jusqu'à l'ouverture).
    Waiting,
    /// Allumé et pris : l'app capte, le son part.
    On,
    /// L'ordinateur ne le prend pas : puce estompée avec la raison.
    Refused(MicRefusal),
}

impl MicState {
    /// L'app doit capter.
    pub fn capturing(self) -> bool {
        matches!(self, Self::On | Self::Waiting)
    }

    /// JSON pour les ponts : `{"state":"refused","reason":"no_device","text":"…"}`.
    pub fn to_json(self) -> String {
        let mut value = serde_json::to_value(self).unwrap_or_default();
        if let (Self::Refused(r), Some(map)) = (self, value.as_object_mut()) {
            map.insert("text".into(), r.text().into());
        }
        value.to_string()
    }
}

// ---------------------------------------------------------------------------
// libopus

unsafe extern "C" {
    fn opus_encoder_create(fs: i32, channels: i32, application: i32, error: *mut i32) -> *mut c_void;
    fn opus_encode_float(st: *mut c_void, pcm: *const f32, frame_size: i32, data: *mut u8, max_bytes: i32) -> i32;
    fn opus_encoder_ctl(st: *mut c_void, request: i32, ...) -> i32;
    fn opus_encoder_destroy(st: *mut c_void);
}

const OPUS_APPLICATION_VOIP: i32 = 2048;
const OPUS_SET_BITRATE_REQUEST: i32 = 4002;
const OPUS_SET_INBAND_FEC_REQUEST: i32 = 4012;
const OPUS_SET_PACKET_LOSS_PERC_REQUEST: i32 = 4014;
const OPUS_SET_DTX_REQUEST: i32 = 4016;
const OPUS_SET_SIGNAL_REQUEST: i32 = 4024;
const OPUS_SIGNAL_VOICE: i32 = 3001;
const OPUS_SET_COMPLEXITY_REQUEST: i32 = 4010;

/// Encodeur Opus mono, 48 kHz, voix.
pub struct Encoder {
    st: *mut c_void,
}

unsafe impl Send for Encoder {}

impl Encoder {
    pub fn new() -> Result<Self, i32> {
        let mut err = 0;
        let st = unsafe { opus_encoder_create(SAMPLE_RATE as i32, 1, OPUS_APPLICATION_VOIP, &mut err) };
        if st.is_null() || err != 0 {
            return Err(err);
        }
        unsafe {
            opus_encoder_ctl(st, OPUS_SET_BITRATE_REQUEST, BITRATE);
            opus_encoder_ctl(st, OPUS_SET_INBAND_FEC_REQUEST, 1i32);
            opus_encoder_ctl(st, OPUS_SET_PACKET_LOSS_PERC_REQUEST, EXPECTED_LOSS);
            opus_encoder_ctl(st, OPUS_SET_DTX_REQUEST, 0i32);
            opus_encoder_ctl(st, OPUS_SET_SIGNAL_REQUEST, OPUS_SIGNAL_VOICE);
            opus_encoder_ctl(st, OPUS_SET_COMPLEXITY_REQUEST, 8i32);
        }
        Ok(Self { st })
    }

    /// Une trame de [`FRAME`] échantillons ; `None` : erreur de libopus.
    pub fn encode(&mut self, frame: &[f32]) -> Option<Vec<u8>> {
        debug_assert_eq!(frame.len(), FRAME);
        let mut packet = vec![0u8; 1500];
        let len = unsafe { opus_encode_float(self.st, frame.as_ptr(), FRAME as i32, packet.as_mut_ptr(), packet.len() as i32) };
        if len <= 0 {
            return None;
        }
        packet.truncate(len as usize);
        Some(packet)
    }
}

impl Drop for Encoder {
    fn drop(&mut self) {
        unsafe { opus_encoder_destroy(self.st) };
    }
}

// ---------------------------------------------------------------------------
// Fréquence

/// Ramène un flux mono à 48 kHz, par interpolation linéaire, en gardant la
/// phase d'un appel à l'autre. Suffisant pour la voix (16, 44,1 ou 48 kHz) ;
/// au-dessus de 48 kHz, une moyenne glissante évite le repliement.
#[derive(Default)]
pub struct Resampler {
    rate: u32,
    /// Position du prochain échantillon de sortie, en échantillons d'entrée.
    pos: f64,
    last: f32,
}

impl Resampler {
    pub fn push(&mut self, input: &[f32], rate: u32, out: &mut Vec<f32>) {
        if rate == 0 {
            return;
        }
        if rate != self.rate {
            *self = Self { rate, ..Default::default() };
        }
        if rate == SAMPLE_RATE {
            out.extend_from_slice(input);
            return;
        }
        let step = f64::from(rate) / f64::from(SAMPLE_RATE);
        // Au-dessus de 48 kHz : moyenne de `ratio` échantillons d'abord.
        let ratio = (step.floor() as usize).max(1);
        let smoothed: Vec<f32>;
        let input = if ratio > 1 {
            smoothed = input
                .iter()
                .enumerate()
                .map(|(i, _)| {
                    let from = i.saturating_sub(ratio - 1);
                    input[from..=i].iter().sum::<f32>() / (i - from + 1) as f32
                })
                .collect();
            &smoothed[..]
        } else {
            input
        };
        // `pos` est relatif au début de `input` ; l'indice -1 désigne `last`,
        // le dernier échantillon de l'appel précédent.
        let n = input.len() as f64;
        let last = self.last;
        let at = |k: isize| if k < 0 { last } else { input[k as usize] };
        while self.pos.floor() + 1.0 <= n - 1.0 {
            let i = self.pos.floor() as isize;
            let frac = (self.pos - i as f64) as f32;
            let (a, b) = (at(i), at(i + 1));
            out.push(a + (b - a) * frac);
            self.pos += step;
        }
        if let Some(&l) = input.last() {
            self.last = l;
        }
        self.pos -= input.len() as f64;
    }
}

// ---------------------------------------------------------------------------
// Le micro d'une session, sur le flux montant commun (correctif 0085)

/// Le type du flux montant du micro (kyclient `Client::uplink`).
pub const KIND: &str = "microphone";

/// Paquets du flux montant (correctif 0085, docs/webcam.md « Le flux montant »).
const START: u8 = 0x01;
const MEDIA: u8 = 0x02;
const PAUSE: u8 = 0x04;
const STATUS: u8 = 0x10;

struct Pipe {
    encoder: Option<Encoder>,
    resampler: Resampler,
    pending: Vec<f32>,
    /// Heure de capture de la prochaine trame, en microsecondes depuis `epoch`.
    next_pts: u64,
    /// Rien n'est parti depuis `START` ou `PAUSE` : l'heure repart de maintenant.
    silent: bool,
}

/// Où en est le flux de ce démarrage de l'image.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Link {
    /// Pas d'image en cours.
    NoStream,
    /// Image en cours, l'ordinateur n'a pas proposé de flux du micro.
    NotOffered,
    /// Flux là, `START` pas encore envoyé (micro coupé) ou réponse attendue.
    Offered,
    /// L'ordinateur a répondu `started`.
    Started,
    /// L'ordinateur a refusé (`failed`, `busy`).
    Failed(MicRefusal),
}

/// Le micro d'une session : gardé pour toute la session ; chaque démarrage
/// de l'image lui donne le flux montant proposé par l'ordinateur.
pub struct Microphone {
    uplink: Mutex<Option<Uplink>>,
    link: Mutex<Link>,
    enabled: AtomicBool,
    /// Invité d'un partage ou ami qui emprunte : un flux pas proposé veut
    /// dire « pas le droit », pas « Station d'avant ».
    guest: AtomicBool,
    /// Le nom de l'appareil, dans `START`.
    device: Mutex<String>,
    epoch: Instant,
    pipe: Mutex<Pipe>,
    /// Dernier état dit à l'app.
    told: Mutex<Option<MicState>>,
}

impl Microphone {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            uplink: Mutex::new(None),
            link: Mutex::new(Link::NoStream),
            enabled: AtomicBool::new(false),
            guest: AtomicBool::new(false),
            device: Mutex::new(String::new()),
            epoch: Instant::now(),
            pipe: Mutex::new(Pipe {
                encoder: None,
                resampler: Resampler::default(),
                pending: Vec::new(),
                next_pts: 0,
                silent: true,
            }),
            told: Mutex::new(None),
        })
    }

    pub(crate) fn set_identity(&self, device: &str, guest: bool) {
        *self.device.lock().unwrap() = device.to_owned();
        self.guest.store(guest, Ordering::Relaxed);
    }

    /// Le flux de ce démarrage de l'image : `Some` proposé, `None` pas
    /// proposé ; `stream` faux : plus d'image. Allumé, `START` repart sur le
    /// nouveau flux.
    pub(crate) fn set_uplink(&self, uplink: Option<Uplink>, stream: bool) {
        let offered = uplink.is_some();
        *self.uplink.lock().unwrap() = uplink;
        *self.link.lock().unwrap() = match (stream, offered) {
            (false, _) => Link::NoStream,
            (true, false) => Link::NotOffered,
            (true, true) => Link::Offered,
        };
        if offered && self.enabled() {
            self.send_start();
        }
    }

    fn send_start(&self) {
        let settings = serde_json::json!({
            "codec": "opus",
            "rate": SAMPLE_RATE,
            "channels": 1,
            "frame": FRAME,
            "bitrate": BITRATE,
            "device": *self.device.lock().unwrap(),
        });
        let mut packet = vec![START];
        packet.extend_from_slice(settings.to_string().as_bytes());
        self.pipe.lock().unwrap().silent = true;
        if let Some(u) = self.uplink.lock().unwrap().as_ref() {
            u.try_send(packet);
        }
    }

    fn send(&self, packet: Vec<u8>) -> bool {
        match self.uplink.lock().unwrap().as_ref() {
            Some(u) => matches!(u.try_send(packet), kyclient::uplink::UplinkSend::Sent),
            None => false,
        }
    }

    pub fn set_enabled(&self, enabled: bool) {
        if self.enabled.swap(enabled, Ordering::SeqCst) == enabled {
            return;
        }
        log::info!("micro {}", if enabled { "allumé" } else { "coupé" });
        self.pipe.lock().unwrap().pending.clear();
        if enabled {
            if self.uplink.lock().unwrap().is_some() {
                self.send_start();
            }
        } else {
            // Silence tout de suite chez l'ordinateur ; l'aide reste prête.
            self.send(vec![PAUSE]);
            self.pipe.lock().unwrap().silent = true;
        }
    }

    pub fn enabled(&self) -> bool {
        self.enabled.load(Ordering::SeqCst)
    }

    pub fn state(&self) -> MicState {
        let link = *self.link.lock().unwrap();
        match link {
            Link::Failed(r) => MicState::Refused(r),
            Link::NotOffered if self.guest.load(Ordering::Relaxed) => MicState::Refused(MicRefusal::NotAllowed),
            Link::NotOffered => MicState::Refused(MicRefusal::OldStation),
            _ if !self.enabled() => MicState::Off,
            Link::Started => MicState::On,
            Link::NoStream | Link::Offered => MicState::Waiting,
        }
    }

    /// Lit les réponses de l'ordinateur (`STATUS`), puis l'état s'il a
    /// changé depuis le dernier dit à l'app.
    pub(crate) fn changed(&self) -> Option<MicState> {
        loop {
            let packet = self.uplink.lock().unwrap().as_ref().and_then(|u| u.try_recv());
            let Some(packet) = packet else { break };
            if let Some(link) = parse_status(&packet) {
                *self.link.lock().unwrap() = link;
            }
        }
        let now = self.state();
        let mut told = self.told.lock().unwrap();
        (*told != Some(now)).then(|| {
            *told = Some(now);
            now
        })
    }

    /// Des échantillons mono captés à `rate` Hz. Jetés micro coupé ou tant
    /// que l'ordinateur ne l'a pas pris.
    pub fn push(&self, samples: &[f32], rate: u32) {
        if !self.enabled() || *self.link.lock().unwrap() != Link::Started {
            return;
        }
        let mut pipe = self.pipe.lock().unwrap();
        let pipe = &mut *pipe;
        if pipe.encoder.is_none() {
            match Encoder::new() {
                Ok(e) => pipe.encoder = Some(e),
                Err(code) => {
                    log::warn!("micro : encodeur Opus refusé ({code})");
                    return;
                }
            }
        }
        if pipe.silent {
            pipe.silent = false;
            pipe.next_pts = self.epoch.elapsed().as_micros() as u64;
        }
        pipe.resampler.push(samples, rate, &mut pipe.pending);
        let mut offset = 0;
        while pipe.pending.len() - offset >= FRAME {
            let frame = &pipe.pending[offset..offset + FRAME];
            offset += FRAME;
            if let Some(opus) = pipe.encoder.as_mut().and_then(|e| e.encode(frame)) {
                let mut packet = Vec::with_capacity(10 + opus.len());
                packet.push(MEDIA);
                packet.push(0);
                packet.extend_from_slice(&pipe.next_pts.to_be_bytes());
                packet.extend_from_slice(&opus);
                // File pleine : la trame est perdue, l'ordinateur la masque.
                let _ = self.send(packet);
            }
            pipe.next_pts += 10_000;
        }
        pipe.pending.drain(..offset);
    }
}

/// Une réponse de l'ordinateur (`STATUS` + JSON) : ce qu'elle change.
fn parse_status(packet: &[u8]) -> Option<Link> {
    if packet.first() != Some(&STATUS) {
        return None;
    }
    let json: serde_json::Value = serde_json::from_slice(&packet[1..]).ok()?;
    match json.get("state")?.as_str()? {
        "started" => Some(Link::Started),
        "stopped" => Some(Link::Offered),
        "busy" => Some(Link::Failed(MicRefusal::Unsupported)),
        "failed" => Some(Link::Failed(match json.get("reason").and_then(|r| r.as_str()) {
            Some("no_device") => MicRefusal::NoDevice,
            Some("not_allowed") => MicRefusal::NotAllowed,
            _ => MicRefusal::Unsupported,
        })),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn status(json: &str) -> Vec<u8> {
        let mut p = vec![STATUS];
        p.extend_from_slice(json.as_bytes());
        p
    }

    #[test]
    fn letat_suit_le_choix_et_les_reponses() {
        let mic = Microphone::new();
        assert_eq!(mic.state(), MicState::Off, "pas d'image, coupé");
        let (uplink, mut peer) = Uplink::pair(KIND, 24);
        mic.set_uplink(Some(uplink), true);
        assert_eq!(mic.state(), MicState::Off);
        mic.set_enabled(true);
        assert_eq!(mic.state(), MicState::Waiting);
        let start = peer.sent.try_recv().unwrap();
        assert_eq!(start[0], START);
        let json: serde_json::Value = serde_json::from_slice(&start[1..]).unwrap();
        assert_eq!((json["codec"].as_str(), json["frame"].as_u64()), (Some("opus"), Some(480)));
        peer.answer.send(status(r#"{"state":"started"}"#)).unwrap();
        assert_eq!(mic.changed(), Some(MicState::On));
        mic.push(&[0.1; 960], 48_000);
        assert_eq!(peer.sent.try_recv().unwrap()[0], MEDIA);
        assert!(peer.sent.try_recv().is_ok(), "deux trames de 10 ms");
        mic.set_enabled(false);
        assert_eq!(peer.sent.try_recv().unwrap(), vec![PAUSE]);
        assert_eq!(mic.changed(), Some(MicState::Off));
    }

    #[test]
    fn pas_propose_hote_ou_invite() {
        let mic = Microphone::new();
        mic.set_uplink(None, true);
        assert_eq!(mic.state(), MicState::Refused(MicRefusal::OldStation));
        mic.set_identity("iPad", true);
        assert_eq!(mic.state(), MicState::Refused(MicRefusal::NotAllowed));
    }

    #[test]
    fn micro_virtuel_absent() {
        let mic = Microphone::new();
        let (uplink, peer) = Uplink::pair(KIND, 24);
        mic.set_uplink(Some(uplink), true);
        mic.set_enabled(true);
        peer.answer.send(status(r#"{"state":"failed","reason":"no_device"}"#)).unwrap();
        assert_eq!(mic.changed(), Some(MicState::Refused(MicRefusal::NoDevice)));
    }

    #[test]
    fn json_des_ponts() {
        assert_eq!(MicState::On.to_json(), r#"{"state":"on"}"#);
        let refused: serde_json::Value = serde_json::from_str(&MicState::Refused(MicRefusal::NotAllowed).to_json()).unwrap();
        assert_eq!(refused["state"], "refused");
        assert_eq!(refused["reason"], "not_allowed");
        assert!(refused["text"].as_str().unwrap().len() > 5);
    }

    #[test]
    fn frequences_ramenees_a_48_khz() {
        for rate in [16_000u32, 44_100, 48_000, 96_000] {
            let mut r = Resampler::default();
            let mut out = Vec::new();
            let chunk = (rate / 100) as usize;
            for n in 0..100 {
                let input: Vec<f32> = (0..chunk).map(|i| (((n * chunk + i) as f32) * 0.01).sin()).collect();
                r.push(&input, rate, &mut out);
            }
            let diff = (out.len() as i64 - 48_000).abs();
            // L'interpolation garde un échantillon d'entrée en réserve (3 à 16 kHz).
            assert!(diff <= 4, "{rate} Hz : {} échantillons", out.len());
        }
    }

    #[test]
    fn encodeur_voix_10_ms() {
        let mut e = Encoder::new().unwrap();
        let frame: Vec<f32> = (0..FRAME).map(|i| (i as f32 * 0.05).sin() * 0.3).collect();
        let packet = e.encode(&frame).unwrap();
        assert!(!packet.is_empty() && packet.len() < 200);
    }
}
