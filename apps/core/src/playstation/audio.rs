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

fn seq16_lt(a: u16, b: u16) -> bool {
    a != b && ((a < b && b - a < 0x8000) || (a > b && a - b > 0x8000))
}

/// Remet les trames dans l'ordre.
#[derive(Default)]
pub struct AudioReceiver {
    buffer: BTreeMap<u32, Vec<u8>>,
    /// Prochaine trame à sortir ; `None` tant que l'avance n'est pas faite.
    next: Option<u16>,
    startup: bool,
    started: bool,
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

    /// Clé de tri : index déroulé autour de la prochaine trame attendue.
    fn key(&self, index: u16) -> u32 {
        match self.next {
            Some(n) => (n as u32).wrapping_add(index.wrapping_sub(n) as i16 as i32 as u32) ^ 0x8000_0000,
            None => index as u32 ^ 0x8000_0000,
        }
    }

    fn store(&mut self, index: u16, data: &[u8]) {
        if let Some(n) = self.next {
            if seq16_lt(index, n) {
                return; // déjà sortie ou masquée
            }
        }
        let k = self.key(index);
        self.buffer.entry(k).or_insert_with(|| data.to_vec());
        while self.buffer.len() > BUFFER_MAX {
            self.buffer.pop_first();
        }
    }

    fn drain(&mut self, out: &mut Vec<AudioOut>) {
        if !self.started {
            if self.buffer.len() < PREFILL {
                return;
            }
            let (&k, _) = self.buffer.iter().next().unwrap();
            self.next = Some((k ^ 0x8000_0000) as u16);
            self.started = true;
            // Clés recalculées autour du départ.
            let old = std::mem::take(&mut self.buffer);
            for (k, v) in old {
                let idx = (k ^ 0x8000_0000) as u16;
                let nk = self.key(idx);
                self.buffer.insert(nk, v);
            }
        }
        loop {
            let Some(n) = self.next else { return };
            let k = self.key(n);
            if let Some(data) = self.buffer.remove(&k) {
                out.push(AudioOut::Frame(data));
                self.next = Some(n.wrapping_add(1));
                continue;
            }
            // Trame manquante : masquée quand assez de trames suivantes sont
            // là pour ne plus l'attendre.
            let ahead = self.buffer.keys().filter(|&&b| b > k).count();
            let newest = self.buffer.keys().next_back().copied();
            if ahead > 0 && newest.is_some_and(|nk| nk >= k + PREFILL as u32 - 1 || self.buffer.len() < PREFILL) {
                out.push(AudioOut::Hole);
                self.next = Some(n.wrapping_add(1));
                continue;
            }
            return;
        }
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

    #[test]
    fn opus_head_is_parsed_by_the_engine() {
        let h = opus_head(2, 48000);
        let head = crate::media::OpusHead::parse(&h).unwrap();
        assert_eq!((head.channels, head.streams, head.coupled), (2, 1, 1));
    }
}
