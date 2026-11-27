//! Son de la console, d'après `audioreceiver.c` de chiaki-ng.
//!
//! Un paquet de son porte `source` trames Opus consécutives (à partir de
//! son `frame_index`), puis `fec` copies des trames qui les précèdent, en
//! secours si un paquet se perd. Toutes les trames ont la même taille
//! (`unit_size`, 8 bits hauts de `units_in_frame_fec`). Ce module les remet
//! dans l'ordre sans doublon, attend trois trames d'avance au départ, et
//! annonce un trou quand une trame manque alors que les suivantes sont là.

use std::collections::BTreeMap;

use super::takion::AvPacket;

/// Trames d'avance avant la première sortie (`CHIAKI_AUDIO_JITTER_PREFILL`).
const PREFILL: usize = 3;
/// Codec Opus dans les paquets de son.
const CODEC_OPUS: u8 = 5;
const BUFFER_MAX: usize = 32;

/// Ce qui sort, dans l'ordre.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AudioOut {
    /// Une trame Opus.
    Frame(Vec<u8>),
    /// Une trame perdue : à masquer.
    Hole,
}

/// En-tête `OpusHead` pour le moteur de son (famille 0 : mono ou stéréo).
pub fn opus_head(channels: u8, rate: u32) -> Vec<u8> {
    let mut head = b"OpusHead".to_vec();
    head.extend_from_slice(&[1, channels.clamp(1, 2), 0, 0]);
    head.extend_from_slice(&rate.to_le_bytes());
    head.extend_from_slice(&[0, 0, 0]);
    head
}

/// Remet les trames dans l'ordre.
///
/// Les index des trames (16 bits) reviennent à zéro toutes les 65 536 trames
/// (≈ 11 min) : ils sont déroulés sur 64 bits autour d'une référence (la
/// prochaine trame attendue, ou la première reçue avant le départ), pour que
/// la clé d'une trame ne change jamais, même rangée juste avant le passage
/// par zéro.
#[derive(Default)]
pub struct AudioReceiver {
    buffer: BTreeMap<u64, Vec<u8>>,
    /// Prochaine trame à sortir (index déroulé) ; `None` tant que l'avance
    /// n'est pas faite.
    next: Option<u64>,
    /// Référence du déroulement avant le départ.
    anchor: Option<u64>,
    startup: bool,
}

impl AudioReceiver {
    pub fn new() -> Self {
        AudioReceiver { startup: true, ..Default::default() }
    }

    /// Les trames d'un paquet de son (pas de vibrations), et ce qui peut
    /// sortir.
    pub fn push(&mut self, p: &AvPacket) -> Vec<AudioOut> {
        let mut out = Vec::new();
        if p.codec != CODEC_OPUS || p.data.is_empty() {
            return out;
        }
        let source = (p.units_in_frame_fec & 0xf) as usize;
        let fec = ((p.units_in_frame_fec >> 4) & 0xf) as usize;
        let unit = (p.units_in_frame_fec >> 8) as usize;
        if source + fec != p.units_in_frame_total as usize || unit == 0 || p.data.len() != unit * (source + fec) {
            return out;
        }
        if p.frame_index > 1 << 15 {
            self.startup = false;
        }
        for i in 0..source + fec {
            let index = if i < source {
                p.frame_index.wrapping_add(i as u16)
            } else {
                let fi = i - source;
                if self.startup && (p.frame_index as usize + fi) < fec + 1 {
                    continue;
                }
                p.frame_index.wrapping_sub(fec as u16).wrapping_add(fi as u16)
            };
            self.store(index, &p.data[unit * i..unit * (i + 1)]);
        }
        self.drain(&mut out);
        out
    }

    /// Index déroulé : le plus proche de la référence (à ± 32 768 trames).
    fn key(&mut self, index: u16) -> u64 {
        let base = match self.next.or(self.anchor) {
            Some(b) => b,
            None => {
                // Loin de zéro : on peut reculer sans jamais passer sous zéro.
                let b = (1u64 << 32) + u64::from(index);
                self.anchor = Some(b);
                b
            }
        };
        let delta = index.wrapping_sub(base as u16) as i16;
        base.wrapping_add_signed(i64::from(delta))
    }

    fn store(&mut self, index: u16, data: &[u8]) {
        let k = self.key(index);
        if self.next.is_some_and(|n| k < n) {
            return; // déjà sortie ou masquée
        }
        self.buffer.entry(k).or_insert_with(|| data.to_vec());
        while self.buffer.len() > BUFFER_MAX {
            self.buffer.pop_first();
        }
    }

    fn drain(&mut self, out: &mut Vec<AudioOut>) {
        if self.next.is_none() {
            if self.buffer.len() < PREFILL {
                return;
            }
            self.next = self.buffer.keys().next().copied();
        }
        let Some(mut n) = self.next else { return };
        // Trames trop vieilles (ne devrait pas arriver : `store` les écarte).
        while self.buffer.first_key_value().is_some_and(|(&k, _)| k < n) {
            self.buffer.pop_first();
        }
        // Saut de plus que la mémoire : on reprend à la plus ancienne trame
        // gardée, avec un seul trou, au lieu de masquer des milliers de trames.
        if let Some(&first) = self.buffer.keys().next() {
            if first > n + BUFFER_MAX as u64 {
                out.push(AudioOut::Hole);
                n = first;
            }
        }
        loop {
            if let Some(data) = self.buffer.remove(&n) {
                out.push(AudioOut::Frame(data));
                n += 1;
                continue;
            }
            // Trame manquante : masquée quand assez de trames suivantes sont
            // là pour ne plus l'attendre.
            let ahead = self.buffer.range(n + 1..).count();
            let newest = self.buffer.keys().next_back().copied();
            if ahead > 0 && newest.is_some_and(|nk| nk >= n + PREFILL as u64 - 1 || self.buffer.len() < PREFILL) {
                out.push(AudioOut::Hole);
                n += 1;
                continue;
            }
            break;
        }
        self.next = Some(n);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn packet(index: u16, source: &[u8], fec: &[u8], unit: usize) -> AvPacket {
        let mut data = Vec::new();
        for &b in source.iter().chain(fec) {
            data.extend(std::iter::repeat_n(b, unit));
        }
        AvPacket {
            is_video: false,
            is_haptics: false,
            packet_index: index,
            frame_index: index,
            unit_index: 0,
            units_in_frame_total: (source.len() + fec.len()) as u16,
            units_in_frame_fec: ((unit as u16) << 8) | ((fec.len() as u16) << 4) | source.len() as u16,
            codec: CODEC_OPUS,
            adaptive_stream_index: 0,
            key_pos: 0,
            data,
        }
    }

    fn frames(out: &[AudioOut]) -> Vec<u8> {
        out.iter()
            .map(|o| match o {
                AudioOut::Frame(d) => d[0],
                AudioOut::Hole => 0,
            })
            .collect()
    }

    #[test]
    fn in_order_after_prefill() {
        let mut r = AudioReceiver::new();
        let mut out = Vec::new();
        // Une trame par paquet, et la précédente en secours.
        for i in 10u16..20 {
            out.extend(r.push(&packet(i, &[i as u8], &[(i - 1) as u8], 4)));
        }
        assert_eq!(frames(&out), (9u8..20).collect::<Vec<_>>());
    }

    #[test]
    fn lost_packet_recovered_by_fec() {
        let mut r = AudioReceiver::new();
        let mut out = Vec::new();
        for i in 10u16..20 {
            if i == 14 {
                continue;
            }
            out.extend(r.push(&packet(i, &[i as u8], &[(i - 1) as u8], 4)));
        }
        // La 14 revient dans le paquet 15 (copie de secours) : rien ne manque.
        assert_eq!(frames(&out), (9u8..20).collect::<Vec<_>>());
    }

    #[test]
    fn lost_frame_becomes_hole() {
        let mut r = AudioReceiver::new();
        let mut out = Vec::new();
        for i in 10u16..20 {
            if i == 14 || i == 15 {
                continue;
            }
            out.extend(r.push(&packet(i, &[i as u8], &[], 4)));
        }
        assert_eq!(frames(&out), vec![10, 11, 12, 13, 0, 0, 16, 17, 18, 19]);
    }

    #[test]
    fn wraps_around() {
        let mut r = AudioReceiver::new();
        let mut out = Vec::new();
        for i in 0..8u16 {
            let idx = 0xfffc_u16.wrapping_add(i);
            out.extend(r.push(&packet(idx, &[i as u8 + 1], &[], 2)));
        }
        assert_eq!(frames(&out), vec![1, 2, 3, 4, 5, 6, 7, 8]);
    }

    /// COEUR-01 : la trame 0 rangée (en avance) pendant que la prochaine
    /// attendue est 0xffff ne doit pas faire tourner `drain` sans fin.
    #[test]
    fn perte_sur_0xffff() {
        let mut r = AudioReceiver::new();
        let mut out = Vec::new();
        for i in 0..6u16 {
            let idx = 0xfff9_u16.wrapping_add(i); // 0xfff9..0xfffe
            out.extend(r.push(&packet(idx, &[i as u8 + 1], &[], 2)));
        }
        // 0xffff perdue, 0 et 1 arrivent : 0xffff masquée, puis 0 et 1.
        out.extend(r.push(&packet(0, &[7], &[], 2)));
        out.extend(r.push(&packet(1, &[8], &[], 2)));
        out.extend(r.push(&packet(2, &[9], &[], 2)));
        assert_eq!(frames(&out), vec![1, 2, 3, 4, 5, 6, 0, 7, 8, 9]);
        // Et la suite continue normalement, plusieurs tours d'index plus loin.
        let mut n = 0u32;
        for i in 3..200_000u32 {
            let o = r.push(&packet(i as u16, &[1], &[], 2));
            n += o.len() as u32;
        }
        assert_eq!(n, 200_000 - 3);
    }

    #[test]
    fn saut_d_index_borne() {
        let mut r = AudioReceiver::new();
        let mut out = Vec::new();
        for i in 10u16..14 {
            out.extend(r.push(&packet(i, &[1], &[], 2)));
        }
        let before = out.len();
        for i in 10_000u16..10_004 {
            out.extend(r.push(&packet(i, &[2], &[], 2)));
        }
        assert!(out.len() - before <= 6, "{}", out.len() - before);
    }

    #[test]
    fn opus_head_is_parsed_by_the_engine() {
        let h = opus_head(2, 48000);
        let head = crate::media::OpusHead::parse(&h).unwrap();
        assert_eq!((head.channels, head.streams, head.coupled), (2, 1, 1));
    }
}
