//! La DualSense pendant une partie (docs/playstation.md, « Vibrations HD »
//! et « Intensités de la DualSense ») : vibrations HD vers la carte son de
//! la manette branchée en USB, intensités des vibrations et des gâchettes
//! adaptatives.
//!
//! **Vibrations HD.** Quand la manette est une DualSense, la console envoie
//! ses vibrations comme un son : PCM 16 bits stéréo à 3 kHz, une trame de
//! 30 échantillons (10 ms, 120 octets) par unité des paquets `is_haptics`
//! (`streamsession.cpp` de chiaki-ng). Une DualSense (ou DualSense Edge)
//! branchée en USB est aussi une carte son à quatre voies dont les voies 3
//! et 4 tiennent les moteurs : la trame y est jouée, rééchantillonnée à
//! 48 kHz (×16, interpolation linéaire). Le cœur prépare les échantillons
//! dans un anneau ([`HdHaptics`], un seul pour l'app : une seule manette
//! joue les vibrations HD), l'app ouvre la sortie de la manette et la
//! signale ([`HdHaptics::set_output`]) ; son fil temps réel tire les
//! échantillons ([`HdHaptics::fill`]). Sans cette sortie (Bluetooth, autre
//! manette), les vibrations simples tirées du même flux, comme avant.
//!
//! **Intensités.** De 0 à 100 % par console ([`Intensities`]), changées
//! tout de suite pendant une partie : les vibrations (HD et simples) sont
//! multipliées par la leur ; pour les gâchettes, la force de chaque effet
//! ([`scale_trigger`]) : 0 % rend la gâchette libre.

use std::sync::atomic::{AtomicBool, AtomicU8, AtomicU32, Ordering};
use std::sync::{Arc, OnceLock};

use crate::audio::ring::Ring;

use super::takion::AvPacket;

/// Fréquence des vibrations HD envoyées par la console.
pub const HAPTICS_RATE: u32 = 3_000;
/// Fréquence de la carte son de la manette.
pub const OUTPUT_RATE: u32 = 48_000;
/// Rééchantillonnage : 16 échantillons de sortie par échantillon reçu.
const UPSAMPLE: usize = (OUTPUT_RATE / HAPTICS_RATE) as usize;
/// Avance avant de jouer (au départ et après un trou) : 20 ms, deux trames.
const PREFILL_FRAMES: usize = OUTPUT_RATE as usize / 50;
/// Au-delà de 60 ms d'avance (réseau qui a calé puis tout rendu d'un coup),
/// le plus ancien est jeté pour revenir à l'avance de départ : les
/// vibrations ne traînent pas derrière le jeu.
const MOST_QUEUED_FRAMES: usize = OUTPUT_RATE as usize * 60 / 1000;

/// Intensités de la DualSense d'une partie, en pour cent (0 à 100).
#[derive(Debug)]
pub struct Intensities {
    rumble: AtomicU8,
    triggers: AtomicU8,
    /// Change à chaque nouvelle valeur : la partie repose alors les
    /// derniers effets reçus de la console.
    generation: AtomicU32,
}

impl Intensities {
    pub fn new(rumble: u8, triggers: u8) -> Self {
        Intensities { rumble: AtomicU8::new(rumble.min(100)), triggers: AtomicU8::new(triggers.min(100)), generation: AtomicU32::new(0) }
    }

    pub fn set(&self, rumble: u8, triggers: u8) {
        self.rumble.store(rumble.min(100), Ordering::Relaxed);
        self.triggers.store(triggers.min(100), Ordering::Relaxed);
        self.generation.fetch_add(1, Ordering::Release);
    }

    pub fn rumble(&self) -> u8 {
        self.rumble.load(Ordering::Relaxed)
    }

    pub fn triggers(&self) -> u8 {
        self.triggers.load(Ordering::Relaxed)
    }

    pub fn generation(&self) -> u32 {
        self.generation.load(Ordering::Acquire)
    }
}

/// Une force de 0 à `max` multipliée par `pct` pour cent, arrondie.
fn scale(v: u32, pct: u8) -> u32 {
    (v * pct.min(100) as u32 + 50) / 100
}

/// Vibration simple (0 à 65 535) à l'intensité choisie.
pub fn scale_rumble(v: u16, pct: u8) -> u16 {
    scale(v as u32, pct) as u16
}

// Types d'effets des gâchettes de la DualSense, tels que la console les
// envoie (`RP-PadProcNo: 3`) : ceux du rapport de sortie de la manette.
// Disposition des paramètres d'après le générateur d'effets de Nielk1
// (`TriggerEffectGenerator`, MIT), relue sur les rapports de chiaki-ng.
const EFFECT_OFF: u8 = 0x05;
/// Ancien « résistance » : position, force sur 8 bits.
const SIMPLE_FEEDBACK: u8 = 0x01;
/// Ancien « arme » : début, fin, force sur 8 bits.
const SIMPLE_WEAPON: u8 = 0x02;
/// Ancienne vibration : fréquence, amplitude sur 8 bits, position.
const SIMPLE_VIBRATION: u8 = 0x06;
/// Variantes bornées : force de 0 à 10.
const LIMITED_FEEDBACK: u8 = 0x11;
const LIMITED_WEAPON: u8 = 0x12;
/// Résistance par zones : zones actives (10 bits), puis une force de 3 bits
/// par zone (valeur + 1 : 1 à 8).
const FEEDBACK: u8 = 0x21;
/// Arc : zones de début et de fin, force et force du « claquement » (3 bits
/// chacune, valeur + 1).
const BOW: u8 = 0x22;
/// Galop : des durées, pas de force.
const GALLOPING: u8 = 0x23;
/// Arme : zones de début et de fin, force sur 3 bits (valeur + 1).
const WEAPON: u8 = 0x25;
/// Vibration par zones : zones actives, amplitude de 3 bits par zone
/// (valeur + 1), fréquence.
const VIBRATION: u8 = 0x26;
/// Machine : zones, deux amplitudes de 3 bits (0 à 7), fréquence, période.
const MACHINE: u8 = 0x27;

/// Un champ de 3 bits qui code une force de 1 à 8 (valeur + 1), multiplié :
/// `None` si la force tombe à 0.
fn scale_minus_one(field: u8, pct: u8) -> Option<u8> {
    let s = scale(field as u32 + 1, pct);
    (s > 0).then(|| (s - 1).min(7) as u8)
}

/// Effet d'une gâchette (`kind` puis 10 octets, tels que la console les
/// envoie) à l'intensité choisie : la force de l'effet multipliée ; 0 %, ou
/// une force qui tombe à 0, rend la gâchette libre (`0x05`). Effet inconnu
/// ou sans force : tel quel (libre à 0 %).
pub fn scale_trigger(kind: u8, params: &[u8; 10], pct: u8) -> (u8, [u8; 10]) {
    let off = (EFFECT_OFF, [0u8; 10]);
    if pct >= 100 || kind == EFFECT_OFF {
        return (kind, *params);
    }
    if pct == 0 {
        return off;
    }
    let mut p = *params;
    match kind {
        // Les paramètres commencent après le type : `p[0]` est l'octet 1 de
        // l'effet.
        SIMPLE_FEEDBACK | SIMPLE_VIBRATION => p[1] = scale(p[1] as u32, pct) as u8,
        SIMPLE_WEAPON => p[2] = scale(p[2] as u32, pct) as u8,
        LIMITED_FEEDBACK => p[1] = scale(p[1].min(10) as u32, pct) as u8,
        LIMITED_WEAPON => p[2] = scale(p[2].min(10) as u32, pct) as u8,
        FEEDBACK | VIBRATION => {
            // Zones actives (octets 1-2), forces de 3 bits (octets 3-6).
            let mut active = u16::from_le_bytes([p[0], p[1]]);
            let mut forces = u32::from_le_bytes([p[2], p[3], p[4], p[5]]);
            for zone in 0..10 {
                if active & (1 << zone) == 0 {
                    continue;
                }
                let field = ((forces >> (3 * zone)) & 7) as u8;
                forces &= !(7 << (3 * zone));
                match scale_minus_one(field, pct) {
                    Some(v) => forces |= (v as u32) << (3 * zone),
                    None => active &= !(1 << zone),
                }
            }
            if active == 0 {
                return off;
            }
            p[..2].copy_from_slice(&active.to_le_bytes());
            p[2..6].copy_from_slice(&forces.to_le_bytes());
        }
        WEAPON => match scale_minus_one(p[2] & 7, pct) {
            Some(v) => p[2] = (p[2] & !7) | v,
            None => return off,
        },
        BOW => {
            let pair = u16::from_le_bytes([p[2], p[3]]);
            let (Some(strength), Some(snap)) = (scale_minus_one((pair & 7) as u8, pct), scale_minus_one(((pair >> 3) & 7) as u8, pct)) else {
                return off;
            };
            let pair = (pair & !0x3f) | strength as u16 | (snap as u16) << 3;
            p[2..4].copy_from_slice(&pair.to_le_bytes());
        }
        MACHINE => {
            let a = scale((p[2] & 7) as u32, pct) as u8;
            let b = scale(((p[2] >> 3) & 7) as u32, pct) as u8;
            if a == 0 && b == 0 {
                return off;
            }
            p[2] = (p[2] & !0x3f) | a | b << 3;
        }
        GALLOPING => {}
        _ => {}
    }
    (kind, p)
}

/// Trames de vibrations HD d'un paquet, dans l'ordre, sans celles déjà
/// vues (le paquet porte `source` trames à partir de son `frame_index`).
#[derive(Default)]
pub(crate) struct HapticsFrames {
    last: Option<u16>,
}

impl HapticsFrames {
    pub(crate) fn push<'a>(&mut self, p: &'a AvPacket) -> Vec<&'a [u8]> {
        let source = (p.units_in_frame_fec & 0xf) as usize;
        let unit = (p.units_in_frame_fec >> 8) as usize;
        if unit == 0 || p.data.len() < unit * source {
            return Vec::new();
        }
        let mut out = Vec::new();
        for i in 0..source {
            let index = p.frame_index.wrapping_add(i as u16);
            if let Some(last) = self.last {
                let d = index.wrapping_sub(last);
                if d == 0 || d >= 0x8000 {
                    continue;
                }
            }
            self.last = Some(index);
            out.push(&p.data[unit * i..unit * (i + 1)]);
        }
        out
    }
}

/// Rééchantillonne une trame (PCM 16 bits stéréo à 3 kHz) vers 48 kHz,
/// stéréo entrelacée, à l'intensité `gain` (0 à 1) : interpolation linéaire
/// depuis le dernier échantillon de la trame précédente (`last`), sans
/// marche entre deux trames.
pub(crate) fn upsample(pcm: &[u8], gain: f32, last: &mut [f32; 2], out: &mut Vec<f32>) {
    for f in pcm.chunks_exact(4) {
        let next = [
            i16::from_le_bytes([f[0], f[1]]) as f32 / 32768.0 * gain,
            i16::from_le_bytes([f[2], f[3]]) as f32 / 32768.0 * gain,
        ];
        for k in 1..=UPSAMPLE {
            let t = k as f32 / UPSAMPLE as f32;
            out.push(last[0] + (next[0] - last[0]) * t);
            out.push(last[1] + (next[1] - last[1]) * t);
        }
        *last = next;
    }
}

/// Sortie des vibrations HD : l'anneau entre la partie (producteur) et le
/// fil temps réel de la carte son de la manette (consommateur).
pub struct HdHaptics {
    /// Stéréo entrelacée à 48 kHz ; une seconde.
    ring: Ring,
    /// L'app a ouvert la carte son d'une DualSense branchée en USB.
    output: AtomicBool,
    /// Consommateur seul : l'avance est faite, la sortie joue.
    playing: AtomicBool,
}

impl HdHaptics {
    fn new() -> Self {
        HdHaptics { ring: Ring::new(OUTPUT_RATE as usize * 2), output: AtomicBool::new(false), playing: AtomicBool::new(false) }
    }

    /// L'app a ouvert (`true`) ou fermé la sortie de la manette (débranchée,
    /// fin de la partie).
    pub fn set_output(&self, on: bool) {
        if self.output.swap(on, Ordering::SeqCst) != on {
            log::info!("PlayStation : vibrations HD {}", if on { "sur la carte son de la DualSense (USB)" } else { "coupées (pas de DualSense en USB) : vibrations simples" });
        }
    }

    /// Une DualSense en USB joue les vibrations HD.
    pub fn has_output(&self) -> bool {
        self.output.load(Ordering::SeqCst)
    }

    /// Producteur seul : des échantillons de [`upsample`].
    pub(crate) fn push(&self, samples: &[f32]) {
        self.ring.push(samples);
    }

    /// Fil temps réel de la sortie : `frames` images de `channels` voies
    /// dans `out` ; les vibrations (gauche, droite) sur les voies `first` et
    /// `first + 1`, du silence ailleurs (et sans avance, ou après un trou).
    pub fn fill(&self, out: &mut [f32], frames: usize, channels: usize, first: usize) {
        let n = (frames * channels).min(out.len());
        out[..n].fill(0.0);
        if channels == 0 || first + 1 >= channels {
            return;
        }
        let frames = n / channels;
        let queued = self.ring.len() / 2;
        // Trop d'avance : retour à celle de départ.
        if queued > MOST_QUEUED_FRAMES {
            self.ring.advance((queued - PREFILL_FRAMES) * 2);
        }
        let queued = self.ring.len() / 2;
        if !self.playing.load(Ordering::Relaxed) {
            if queued < PREFILL_FRAMES {
                return;
            }
            self.playing.store(true, Ordering::Relaxed);
        }
        let take = frames.min(queued);
        let mut pair = [0.0f32; 2];
        for i in 0..take {
            if !self.ring.peek(&mut pair) {
                break;
            }
            self.ring.advance(2);
            out[i * channels + first] = pair[0];
            out[i * channels + first + 1] = pair[1];
        }
        // Plus rien : la prochaine reprise refait l'avance.
        if take < frames {
            self.playing.store(false, Ordering::Relaxed);
        }
    }
}

/// La sortie des vibrations HD de l'app.
pub fn hd_haptics() -> &'static Arc<HdHaptics> {
    static OUT: OnceLock<Arc<HdHaptics>> = OnceLock::new();
    OUT.get_or_init(|| Arc::new(HdHaptics::new()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rumble_scaling() {
        assert_eq!(scale_rumble(65_535, 100), 65_535);
        assert_eq!(scale_rumble(65_535, 50), 32_768);
        assert_eq!(scale_rumble(1000, 0), 0);
        assert_eq!(scale_rumble(1000, 200), 1000, "au plus 100 %");
    }

    #[test]
    fn triggers_full_and_off() {
        let p = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10];
        assert_eq!(scale_trigger(FEEDBACK, &p, 100), (FEEDBACK, p));
        assert_eq!(scale_trigger(FEEDBACK, &p, 0), (EFFECT_OFF, [0; 10]));
        assert_eq!(scale_trigger(GALLOPING, &p, 50), (GALLOPING, p), "sans force : tel quel");
        assert_eq!(scale_trigger(GALLOPING, &p, 0).0, EFFECT_OFF);
    }

    /// Résistance de la zone 3 à la fin, force 8 (champ 7) : la moitié
    /// donne 4 (champ 3).
    #[test]
    fn feedback_zones() {
        let mut active = 0u16;
        let mut forces = 0u32;
        for zone in 3..10 {
            active |= 1 << zone;
            forces |= 7 << (3 * zone);
        }
        let mut p = [0u8; 10];
        p[..2].copy_from_slice(&active.to_le_bytes());
        p[2..6].copy_from_slice(&forces.to_le_bytes());
        let (kind, q) = scale_trigger(FEEDBACK, &p, 50);
        assert_eq!(kind, FEEDBACK);
        assert_eq!(u16::from_le_bytes([q[0], q[1]]), active);
        let f = u32::from_le_bytes([q[2], q[3], q[4], q[5]]);
        for zone in 3..10 {
            assert_eq!((f >> (3 * zone)) & 7, 3, "zone {zone}");
        }
        // Force 1 (champ 0) à 10 % : plus rien, gâchette libre.
        let mut weak = [0u8; 10];
        weak[..2].copy_from_slice(&0b11u16.to_le_bytes());
        assert_eq!(scale_trigger(FEEDBACK, &weak, 10).0, EFFECT_OFF);
    }

    #[test]
    fn weapon_bow_machine_simple() {
        // Arme : zones 2 et 6, force 8 → 4 à 50 %.
        let mut w = [0u8; 10];
        w[..2].copy_from_slice(&((1u16 << 2) | (1 << 6)).to_le_bytes());
        w[2] = 7;
        let (_, q) = scale_trigger(WEAPON, &w, 50);
        assert_eq!((q[0], q[1], q[2]), (w[0], w[1], 3));
        // Arc : force 8, claquement 4 → 4 et 2.
        let mut b = [0u8; 10];
        b[2] = 7 | (3 << 3);
        let (_, q) = scale_trigger(BOW, &b, 50);
        assert_eq!(q[2], 3 | (1 << 3));
        // Machine : amplitudes 6 et 2 → 3 et 1 ; fréquence gardée.
        let mut m = [0u8; 10];
        m[2] = 6 | (2 << 3);
        m[3] = 40;
        let (_, q) = scale_trigger(MACHINE, &m, 50);
        assert_eq!((q[2], q[3]), (3 | (1 << 3), 40));
        // Anciens effets : la force sur 8 bits.
        let (_, q) = scale_trigger(SIMPLE_WEAPON, &[10, 100, 200, 0, 0, 0, 0, 0, 0, 0], 50);
        assert_eq!(&q[..3], &[10, 100, 100]);
        let (_, q) = scale_trigger(SIMPLE_FEEDBACK, &[5, 255, 0, 0, 0, 0, 0, 0, 0, 0], 30);
        assert_eq!(&q[..2], &[5, 77]);
    }

    #[test]
    fn upsampling() {
        let mut pcm = Vec::new();
        for _ in 0..30 {
            pcm.extend_from_slice(&16384i16.to_le_bytes());
            pcm.extend_from_slice(&(-16384i16).to_le_bytes());
        }
        let mut last = [0.0; 2];
        let mut out = Vec::new();
        upsample(&pcm, 1.0, &mut last, &mut out);
        // 30 échantillons à 3 kHz → 480 à 48 kHz, stéréo.
        assert_eq!(out.len(), 480 * 2);
        // Montée depuis 0 sur les 16 premiers, puis constant.
        assert!((out[0] - 0.5 / 16.0).abs() < 1e-6);
        assert!((out[2 * 15] - 0.5).abs() < 1e-6 && (out[2 * 15 + 1] + 0.5).abs() < 1e-6);
        assert!(out[32..].chunks(2).all(|s| (s[0] - 0.5).abs() < 1e-6 && (s[1] + 0.5).abs() < 1e-6));
        assert_eq!(last, [0.5, -0.5]);
        // À 50 %.
        out.clear();
        upsample(&pcm, 0.5, &mut last, &mut out);
        assert!((out.last().unwrap() + 0.25).abs() < 1e-6);
    }

    #[test]
    fn output_fill() {
        let out = HdHaptics::new();
        let mut buf = vec![1.0f32; 64 * 4];
        // Rien en attente : silence partout.
        out.fill(&mut buf, 64, 4, 2);
        assert!(buf.iter().all(|&s| s == 0.0));
        // Moins que l'avance de départ : silence encore.
        out.push(&vec![0.25; 100 * 2]);
        out.fill(&mut buf, 64, 4, 2);
        assert!(buf.iter().all(|&s| s == 0.0));
        out.push(&vec![0.25; PREFILL_FRAMES * 2]);
        out.fill(&mut buf, 64, 4, 2);
        for f in buf.chunks(4) {
            assert_eq!(f, &[0.0, 0.0, 0.25, 0.25], "voies 3 et 4 seulement");
        }
        // Trop d'avance : ramenée à celle de départ.
        out.push(&vec![0.5; OUTPUT_RATE as usize / 10 * 2]);
        out.fill(&mut buf, 64, 4, 2);
        assert_eq!(out.ring.len() / 2, PREFILL_FRAMES - 64);
        // Sortie à deux voies (carte des voies faite par l'app) : 0 et 1.
        let mut two = vec![0.0f32; 8 * 2];
        out.fill(&mut two, 8, 2, 0);
        assert!(two.iter().all(|&s| s == 0.5));
        // Voies impossibles : silence, sans paniquer.
        out.fill(&mut two, 8, 2, 1);
        assert!(two.iter().all(|&s| s == 0.0));
    }

    #[test]
    fn frames_in_order_without_duplicates() {
        let packet = |index: u16, frames: u8| AvPacket {
            is_video: false,
            is_haptics: true,
            packet_index: 0,
            frame_index: index,
            unit_index: 0,
            units_in_frame_total: frames as u16,
            units_in_frame_fec: (120 << 8) | frames as u16,
            codec: 0,
            adaptive_stream_index: 0,
            key_pos: 0,
            data: (0..frames).flat_map(|i| vec![i; 120]).collect(),
        };
        let mut f = HapticsFrames::default();
        let p = packet(10, 2);
        let a = f.push(&p);
        assert_eq!(a.len(), 2);
        assert_eq!((a[0][0], a[1][0]), (0, 1));
        // Le même paquet encore, puis un plus ancien : rien.
        assert!(f.push(&p).is_empty());
        assert!(f.push(&packet(9, 1)).is_empty());
        // Le suivant, qui recouvre la trame 11.
        let n = packet(11, 2);
        let b = f.push(&n);
        assert_eq!(b.len(), 1);
        assert_eq!(b[0][0], 1);
    }
}
