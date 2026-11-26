//! Le micro de l'appareil, envoyé à l'ordinateur (docs/micro.md).
//!
//! L'app capte son micro comme son système le permet (traitement voix
//! compris) et donne ses échantillons à [`crate::session::Session::push_microphone`] :
//! flottants mono, à n'importe quelle fréquence. Le cœur les ramène à 48 kHz,
//! les découpe en trames de 10 ms, les encode en Opus (voix, 32 kbit/s,
//! correction d'erreurs intégrée) et les donne à kyclient (correctif 0080).
//!
//! Une app qui a un micro le dit une fois au lancement ([`set_device_has_microphone`]) :
//! le micro est alors proposé à chaque démarrage de l'image, même coupé (rien
//! ne part micro coupé), si bien que l'ordinateur dit tout de suite s'il le
//! prend et que l'allumer en session ne relance rien. L'état arrive à l'app par
//! [`crate::session::Platform::on_microphone`] ; elle capte tant qu'il est
//! [`MicState::On`] ou [`MicState::Waiting`].

use std::ffi::c_void;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use kyclient::uplink::{Uplink, UplinkState};
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
/// Identifiant Kymux du codec.
pub const OPUS: u32 = u32::from_be_bytes(*b"opus");

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
    fn from_reason(reason: &str) -> Self {
        match reason {
            "not_allowed" => Self::NotAllowed,
            "no_device" => Self::NoDevice,
            "old_station" => Self::OldStation,
            _ => Self::Unsupported,
        }
    }

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

/// L'état d'après le choix de l'app et la réponse de l'ordinateur.
pub fn state(enabled: bool, uplink: &UplinkState) -> MicState {
    match uplink {
        UplinkState::Refused(reason) => MicState::Refused(MicRefusal::from_reason(reason)),
        _ if !enabled => MicState::Off,
        UplinkState::Open => MicState::On,
        UplinkState::Idle | UplinkState::Offered => MicState::Waiting,
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
// Le micro d'une session

struct Pipe {
    encoder: Option<Encoder>,
    resampler: Resampler,
    pending: Vec<f32>,
    /// Heure de capture de la prochaine trame, en microsecondes depuis `epoch`.
    next_pts: u64,
    /// L'app a cessé d'envoyer : le paquet vide est parti.
    silent: bool,
}

/// Le micro d'une session : gardé pour toute la session, donné à chaque
/// démarrage de l'image.
pub struct Microphone {
    uplink: Uplink,
    enabled: AtomicBool,
    epoch: Instant,
    pipe: Mutex<Pipe>,
    /// Dernier état dit à l'app.
    told: Mutex<Option<MicState>>,
}

impl Microphone {
    pub fn new() -> Arc<Self> {
        let uplink = Uplink::new();
        uplink.set_codec(OPUS, FRAME as u16);
        Arc::new(Self {
            uplink,
            enabled: AtomicBool::new(false),
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

    /// Le micro à donner à kyclient pour ce démarrage de l'image : `None`
    /// sans micro sur l'appareil.
    pub(crate) fn for_stream(&self) -> Option<Uplink> {
        device_has_microphone().then(|| {
            self.uplink.set_offer(true);
            self.uplink.clone()
        })
    }

    pub fn set_enabled(&self, enabled: bool) {
        if self.enabled.swap(enabled, Ordering::SeqCst) == enabled {
            return;
        }
        log::info!("micro {}", if enabled { "allumé" } else { "coupé" });
        let mut pipe = self.pipe.lock().unwrap();
        pipe.pending.clear();
        if !enabled && !pipe.silent {
            // Silence tout de suite chez l'ordinateur.
            pipe.silent = true;
            self.uplink.send(&[], 0);
        }
    }

    pub fn enabled(&self) -> bool {
        self.enabled.load(Ordering::SeqCst)
    }

    pub fn state(&self) -> MicState {
        state(self.enabled(), &self.uplink.state())
    }

    /// L'état s'il a changé depuis le dernier dit à l'app.
    pub(crate) fn changed(&self) -> Option<MicState> {
        let now = self.state();
        let mut told = self.told.lock().unwrap();
        (*told != Some(now)).then(|| {
            *told = Some(now);
            now
        })
    }

    /// Des échantillons mono captés à `rate` Hz. Jetés micro coupé ou tant
    /// que l'ordinateur ne le prend pas.
    pub fn push(&self, samples: &[f32], rate: u32) {
        if !self.enabled() || !self.uplink.is_open() {
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
            // Reprise : l'heure repart de maintenant, moins ce qui est déjà là.
            pipe.silent = false;
            pipe.next_pts = self.epoch.elapsed().as_micros() as u64;
        }
        pipe.resampler.push(samples, rate, &mut pipe.pending);
        let mut offset = 0;
        while pipe.pending.len() - offset >= FRAME {
            let frame = &pipe.pending[offset..offset + FRAME];
            offset += FRAME;
            if let Some(packet) = pipe.encoder.as_mut().and_then(|e| e.encode(frame)) {
                self.uplink.send(&packet, pipe.next_pts);
            }
            pipe.next_pts += 10_000;
        }
        pipe.pending.drain(..offset);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn letat_suit_le_choix_et_la_reponse() {
        assert_eq!(state(false, &UplinkState::Idle), MicState::Off);
        assert_eq!(state(false, &UplinkState::Open), MicState::Off);
        assert_eq!(state(true, &UplinkState::Offered), MicState::Waiting);
        assert_eq!(state(true, &UplinkState::Open), MicState::On);
        // Refusé : estompé, allumé ou pas.
        assert_eq!(state(false, &UplinkState::Refused("no_device".into())), MicState::Refused(MicRefusal::NoDevice));
        assert_eq!(state(true, &UplinkState::Refused("old_station".into())), MicState::Refused(MicRefusal::OldStation));
        assert_eq!(state(true, &UplinkState::Refused("autre".into())), MicState::Refused(MicRefusal::Unsupported));
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
            // Une seconde en paquets de 10 ms de l'entrée.
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

    #[test]
    fn rien_ne_part_avant_louverture_ni_micro_coupe() {
        let mic = Microphone::new();
        set_device_has_microphone(true);
        assert!(mic.for_stream().is_some());
        mic.set_enabled(true);
        assert_eq!(mic.state(), MicState::Waiting);
        // Pas ouvert : jeté sans rien encoder.
        mic.push(&[0.0; 480], 48_000);
        assert!(mic.pipe.lock().unwrap().encoder.is_none());
        assert_eq!(mic.changed(), Some(MicState::Waiting));
        assert_eq!(mic.changed(), None);
    }
}
