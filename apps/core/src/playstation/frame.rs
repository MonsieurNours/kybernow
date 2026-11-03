//! Assemblage des images, d'après `videoreceiver.c` et `frameprocessor.c`
//! de chiaki-ng.
//!
//! Une image arrive en unités (paquets Takion) : `k` unités de données
//! puis `m` de correction. Chaque unité de données commence par 2 octets
//! (gros-boutiste) : le remplissage ajouté pour qu'elles aient toutes la
//! même taille, que la première unité ajoute à la sienne pour donner la
//! taille commune. L'image est la suite des unités de données sans ces 2
//! octets : du H.264 ou du H.265 en Annex B, prêt pour le décodeur.
//!
//! Une image est rendue dès que ses unités de données sont là, ou que
//! données et correction suffisent (correction d'erreurs, [`super::fec`]).
//! Les images sautées sont signalées ([`Assembled::missing`]) : la session
//! le dit à la console (CORRUPTFRAME), qui renvoie une image de référence.

use super::fec;
use super::takion::AvPacket;

const UNIT_SLOTS_MAX: usize = 512;

/// Une image assemblée.
#[derive(Debug, Clone)]
pub struct Frame {
    pub index: u16,
    /// Annex B, en-tête vidéo (VPS/SPS/PPS) devant la première image et à
    /// chaque changement de définition.
    pub data: Vec<u8>,
    /// Retrouvée par la correction d'erreurs.
    pub recovered: bool,
    /// Index de la définition (`adaptive_stream_index`).
    pub profile: u8,
}

/// Ce que donne un paquet.
#[derive(Debug, Default)]
pub struct Assembled {
    pub frame: Option<Frame>,
    /// Images manquantes ou perdues, de `.0` à `.1` comprises.
    pub missing: Option<(u16, u16)>,
}

struct Building {
    index: u16,
    profile: u8,
    k: usize,
    m: usize,
    unit_size: usize,
    units: Vec<Option<Vec<u8>>>,
    data_received: usize,
    fec_received: usize,
}

/// Assemble les images d'un flux.
pub struct Assembler {
    headers: Vec<Vec<u8>>,
    profile_sent: Option<u8>,
    current: Option<Building>,
    /// Dernière image commencée, et dernière rendue entière.
    last_started: Option<u16>,
    last_complete: Option<u16>,
}

fn seq16_lt(a: u16, b: u16) -> bool {
    a != b && ((a < b && b - a < 0x8000) || (a > b && a - b > 0x8000))
}

impl Assembler {
    /// `headers` : l'en-tête vidéo de chaque définition, dans l'ordre de
    /// STREAMINFO.
    pub fn new(headers: Vec<Vec<u8>>) -> Self {
        Assembler { headers, profile_sent: None, current: None, last_started: None, last_complete: None }
    }

    /// Ajoute un paquet d'image.
    pub fn push(&mut self, p: &AvPacket) -> Assembled {
        let mut out = Assembled::default();
        if let Some(cur) = self.last_started {
            if seq16_lt(p.frame_index, cur) {
                return out; // reste d'une image passée
            }
        }
        if (p.adaptive_stream_index as usize) >= self.headers.len() {
            log::warn!("PlayStation : définition {} inconnue", p.adaptive_stream_index);
            return out;
        }
        if self.last_started != Some(p.frame_index) {
            // Image suivante : l'ancienne, si elle n'est pas sortie, est
            // rendue telle quelle (ou perdue).
            if let Some(b) = self.current.take() {
                out.frame = self.flush(b);
            }
            let expected = self.last_complete.map(|c| c.wrapping_add(1));
            if let Some(exp) = expected {
                if seq16_lt(exp, p.frame_index) {
                    out.missing = Some((exp, p.frame_index.wrapping_sub(1)));
                }
            }
            self.last_started = Some(p.frame_index);
            self.current = Self::start(p);
        }
        let ready = match self.current.as_mut() {
            Some(b) => {
                b.put(p);
                b.data_received + b.fec_received >= b.k || p.unit_index + 1 == p.units_in_frame_total
            }
            None => false,
        };
        if ready {
            if let Some(b) = self.current.take() {
                let frame = self.flush(b);
                // Deux images au plus par paquet : on garde la plus récente
                // (la précédente était incomplète et a déjà été signalée).
                if frame.is_some() {
                    out.frame = frame;
                }
            }
        }
        out
    }

    fn start(p: &AvPacket) -> Option<Building> {
        let total = p.units_in_frame_total as usize;
        let fec = p.units_in_frame_fec as usize;
        if total < fec || total == 0 {
            return None;
        }
        let k = total - fec;
        let m = fec.max(1);
        let mut unit_size = p.data.len();
        if (p.unit_index as usize) < k {
            if p.data.len() < 2 {
                return None;
            }
            unit_size += u16::from_be_bytes([p.data[0], p.data[1]]) as usize;
        }
        if k + m > UNIT_SLOTS_MAX || unit_size == 0 {
            return None;
        }
        Some(Building {
            index: p.frame_index,
            profile: p.adaptive_stream_index,
            k,
            m,
            unit_size,
            units: vec![None; k + m],
            data_received: 0,
            fec_received: 0,
        })
    }

    /// Rend l'image si elle est entière ou réparable.
    fn flush(&mut self, b: Building) -> Option<Frame> {
        let Building { index, profile, k, m, unit_size, mut units, data_received, fec_received } = b;
        let mut recovered = false;
        if data_received < k {
            if data_received + fec_received < k {
                log::debug!("PlayStation : image {index} incomplète ({data_received}+{fec_received}/{k}), perdue");
                return None;
            }
            let erased: Vec<usize> = (0..k + m).filter(|&i| units[i].is_none()).collect();
            let mut full: Vec<Vec<u8>> = units
                .iter()
                .map(|u| {
                    let mut v = u.clone().unwrap_or_default();
                    v.resize(unit_size, 0);
                    v
                })
                .collect();
            if !fec::decode(&mut full, unit_size, k, m, &erased) {
                log::debug!("PlayStation : correction d'erreurs impossible sur l'image {index}");
                return None;
            }
            // Taille réelle de chaque unité retrouvée : taille commune moins
            // son remplissage.
            for i in 0..k {
                if units[i].is_none() {
                    let pad = u16::from_be_bytes([full[i][0], full[i][1]]) as usize;
                    if pad >= unit_size {
                        return None;
                    }
                    full[i].truncate(unit_size - pad);
                    units[i] = Some(std::mem::take(&mut full[i]));
                }
            }
            recovered = true;
        }
        let mut data = Vec::new();
        if self.profile_sent != Some(profile) {
            data.extend_from_slice(&self.headers[profile as usize]);
            self.profile_sent = Some(profile);
        }
        for u in units.iter().take(k).flatten() {
            if u.len() >= 2 {
                data.extend_from_slice(&u[2..]);
            }
        }
        self.last_complete = Some(index);
        Some(Frame { index, data, recovered, profile })
    }
}

impl Building {
    fn put(&mut self, p: &AvPacket) {
        let i = p.unit_index as usize;
        if i >= self.units.len() || p.data.is_empty() || p.data.len() > self.unit_size || self.units[i].is_some() {
            return;
        }
        self.units[i] = Some(p.data.clone());
        if i < self.k {
            self.data_received += 1;
        } else {
            self.fec_received += 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Découpe `payload` comme la console : `k` unités de même taille
    /// (2 octets de remplissage en tête), plus `m` de correction.
    fn split(index: u16, payload: &[u8], k: usize, m: usize) -> Vec<AvPacket> {
        let chunk = payload.len().div_ceil(k);
        let size = chunk + 2;
        let mut data: Vec<Vec<u8>> = payload
            .chunks(chunk)
            .map(|c| {
                let pad = (size - 2 - c.len()) as u16;
                let mut u = pad.to_be_bytes().to_vec();
                u.extend_from_slice(c);
                u
            })
            .collect();
        assert_eq!(data.len(), k);
        let padded: Vec<Vec<u8>> = data
            .iter()
            .map(|u| {
                let mut v = u.clone();
                v.resize(size, 0);
                v
            })
            .collect();
        data.extend(fec::encode(&padded, size, m));
        data.into_iter()
            .enumerate()
            .map(|(i, d)| AvPacket {
                is_video: true,
                is_haptics: false,
                packet_index: i as u16,
                frame_index: index,
                unit_index: i as u16,
                units_in_frame_total: (k + m) as u16,
                units_in_frame_fec: m as u16,
                codec: 0,
                adaptive_stream_index: 0,
                key_pos: 0,
                data: d,
            })
            .collect()
    }

    #[test]
    fn assembles_with_header_once() {
        let mut a = Assembler::new(vec![b"HDR".to_vec()]);
        let f1: Vec<u8> = (0..100).collect();
        let mut frames = Vec::new();
        for p in split(1, &f1, 3, 1).iter().take(3) {
            frames.extend(a.push(p).frame);
        }
        assert_eq!(frames.len(), 1);
        assert_eq!(&frames[0].data[..3], b"HDR");
        assert_eq!(&frames[0].data[3..], &f1[..]);
        let f2: Vec<u8> = (0..40).rev().collect();
        for p in split(2, &f2, 2, 1).iter().take(2) {
            frames.extend(a.push(p).frame);
        }
        assert_eq!(frames[1].data, f2);
        assert!(!frames[1].recovered);
    }

    #[test]
    fn recovers_lost_units() {
        let mut a = Assembler::new(vec![Vec::new()]);
        let f: Vec<u8> = (0..250u16).map(|x| (x * 7) as u8).collect();
        let packets = split(1, &f, 5, 2);
        let mut out = None;
        for (i, p) in packets.iter().enumerate() {
            if i == 0 || i == 3 {
                continue;
            }
            if let Some(fr) = a.push(p).frame {
                out = Some(fr);
            }
        }
        let fr = out.unwrap();
        assert!(fr.recovered);
        assert_eq!(fr.data, f);
    }

    #[test]
    fn reports_missing_frames() {
        let mut a = Assembler::new(vec![Vec::new()]);
        for p in split(1, &[1, 2, 3, 4], 1, 1).iter().take(1) {
            a.push(p);
        }
        let r = a.push(&split(4, &[5, 6], 1, 1)[0]);
        assert_eq!(r.missing, Some((2, 3)));
        assert_eq!(r.frame.unwrap().data, vec![5, 6]);
    }

    #[test]
    fn too_many_losses_drop_the_frame() {
        let mut a = Assembler::new(vec![Vec::new()]);
        assert!(a.push(&split(1, &[7], 1, 1)[0]).frame.is_some());
        let packets = split(2, &(0..90).collect::<Vec<u8>>(), 3, 1);
        assert!(a.push(&packets[1]).frame.is_none());
        // Image suivante : la 2, irréparable, est perdue et signalée.
        let r = a.push(&split(3, &[9], 1, 1)[0]);
        assert_eq!(r.frame.unwrap().data, vec![9]);
        assert_eq!(r.missing, Some((2, 2)));
    }
}
