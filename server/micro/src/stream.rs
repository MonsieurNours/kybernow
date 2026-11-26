//! Ce que kycontroller passe sur l'entrée standard (flux montant commun,
//! correctif 0085, `kycontroller/src/uplink.rs`), et son décodage.
//!
//! Chaque paquet est précédé de sa longueur (u32, gros-boutiste) ; son
//! premier octet dit lequel : `START` 0x01 + JSON (`{"codec": "opus",
//! "rate": 48000, "channels": 1, "frame": 480, …}`), `MEDIA` 0x02 + drapeaux
//! (u8) + heure de capture en µs (u64, gros-boutiste) + paquet Opus,
//! `PAUSE` 0x04 (l'appareil a coupé son micro).

use std::ffi::c_void;
use std::io::{self, Read};

use crate::RATE;

#[derive(Debug, PartialEq, Eq)]
pub enum Record {
    /// Le format : échantillons par paquet et voies (seul Opus est pris).
    Start { opus: bool, frame: u16, channels: u8 },
    Media { pts: u64, packet: Vec<u8> },
    Silence,
    /// Un paquet d'un autre type : ignoré.
    Other,
}

/// Le plus gros paquet accepté (une trame Opus fait au plus 1 275 octets).
const MAX_PACKET: usize = 64 * 1024;

/// Le prochain paquet ; `Ok(None)` à la fin de l'entrée.
pub fn read(input: &mut impl Read) -> io::Result<Option<Record>> {
    let mut len = [0u8; 4];
    match input.read_exact(&mut len) {
        Ok(()) => {}
        Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => return Ok(None),
        Err(e) => return Err(e),
    }
    let len = u32::from_be_bytes(len) as usize;
    if len > MAX_PACKET {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "paquet trop gros"));
    }
    let mut body = vec![0u8; len];
    input.read_exact(&mut body)?;
    Ok(Some(match body.first() {
        Some(0x01) => {
            let json: serde_json::Value = serde_json::from_slice(&body[1..]).unwrap_or_default();
            Record::Start {
                opus: json["codec"].as_str() == Some("opus"),
                frame: json["frame"].as_u64().unwrap_or(480) as u16,
                channels: json["channels"].as_u64().unwrap_or(1) as u8,
            }
        }
        Some(0x02) if body.len() >= 10 => Record::Media {
            pts: u64::from_be_bytes(body[2..10].try_into().unwrap()),
            packet: body[10..].to_vec(),
        },
        Some(0x04) => Record::Silence,
        _ => Record::Other,
    }))
}

unsafe extern "C" {
    fn opus_decoder_create(fs: i32, channels: i32, error: *mut i32) -> *mut c_void;
    fn opus_decode_float(st: *mut c_void, data: *const u8, len: i32, pcm: *mut f32, frame_size: i32, decode_fec: i32) -> i32;
    fn opus_decoder_destroy(st: *mut c_void);
}

/// Décodeur Opus mono à 48 kHz, avec ce qui répare les pertes : la trame
/// manquante juste avant un paquet est refaite par sa correction d'erreurs,
/// les autres sont masquées (PLC), trois au plus (30 ms) ; au-delà, on
/// reprend au paquet suivant (le tampon de gigue joue du silence).
pub struct Decoder {
    st: *mut c_void,
    frame: usize,
    /// Heure attendue du prochain paquet.
    next_pts: Option<u64>,
    pub concealed: u64,
    pub recovered: u64,
}

unsafe impl Send for Decoder {}

const MAX_CONCEAL: u64 = 3;

impl Decoder {
    pub fn new(frame: usize) -> Result<Self, i32> {
        let mut err = 0;
        let st = unsafe { opus_decoder_create(RATE as i32, 1, &mut err) };
        if st.is_null() || err != 0 {
            return Err(err);
        }
        Ok(Self {
            st,
            frame,
            next_pts: None,
            concealed: 0,
            recovered: 0,
        })
    }

    fn decode_into(&mut self, packet: Option<&[u8]>, fec: bool, out: &mut Vec<f32>) {
        let mut pcm = vec![0f32; self.frame];
        let (ptr, len) = match packet {
            Some(p) => (p.as_ptr(), p.len() as i32),
            None => (std::ptr::null(), 0),
        };
        let n = unsafe { opus_decode_float(self.st, ptr, len, pcm.as_mut_ptr(), self.frame as i32, i32::from(fec)) };
        if n > 0 {
            out.extend_from_slice(&pcm[..n as usize]);
        }
    }

    /// Le son d'un paquet capturé à `pts` µs, précédé de ce qui répare les
    /// trames perdues juste avant.
    pub fn decode(&mut self, pts: u64, packet: &[u8], out: &mut Vec<f32>) {
        let period = (self.frame as u64 * 1_000_000) / u64::from(RATE);
        if let Some(expected) = self.next_pts
            && pts > expected
        {
            let missing = (pts - expected + period / 2) / period;
            if (1..=MAX_CONCEAL).contains(&missing) {
                for _ in 1..missing {
                    self.decode_into(None, false, out);
                    self.concealed += 1;
                }
                // La dernière trame perdue, d'après la correction de ce paquet.
                self.decode_into(Some(packet), true, out);
                self.recovered += 1;
            }
        }
        if self.next_pts.is_some_and(|e| pts + period / 2 < e) {
            // Un paquet en retard, déjà remplacé : ignoré.
            return;
        }
        self.decode_into(Some(packet), false, out);
        self.next_pts = Some(pts + period);
    }

    /// Reprise après une coupure : pas de trou à réparer.
    pub fn reset(&mut self) {
        self.next_pts = None;
    }
}

impl Drop for Decoder {
    fn drop(&mut self) {
        unsafe { opus_decoder_destroy(self.st) };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    unsafe extern "C" {
        fn opus_encoder_create(fs: i32, channels: i32, application: i32, error: *mut i32) -> *mut c_void;
        fn opus_encode_float(st: *mut c_void, pcm: *const f32, frame_size: i32, data: *mut u8, max_bytes: i32) -> i32;
        fn opus_encoder_destroy(st: *mut c_void);
    }

    fn packets(n: usize) -> Vec<Vec<u8>> {
        let mut err = 0;
        let enc = unsafe { opus_encoder_create(48_000, 1, 2048, &mut err) };
        let out = (0..n)
            .map(|k| {
                let pcm: Vec<f32> = (0..480).map(|i| (((k * 480 + i) as f32) * 0.03).sin() * 0.3).collect();
                let mut data = vec![0u8; 1500];
                let len = unsafe { opus_encode_float(enc, pcm.as_ptr(), 480, data.as_mut_ptr(), 1500) };
                data.truncate(len as usize);
                data
            })
            .collect();
        unsafe { opus_encoder_destroy(enc) };
        out
    }

    #[test]
    fn lit_les_paquets_de_kycontroller() {
        let start = br#"{"codec":"opus","rate":48000,"channels":1,"frame":480}"#;
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&((start.len() + 1) as u32).to_be_bytes());
        bytes.push(0x01);
        bytes.extend_from_slice(start);
        bytes.extend_from_slice(&[0, 0, 0, 12, 0x02, 0, 0, 0, 0, 0, 0, 0, 0x27, 0x10, 9, 8]);
        bytes.extend_from_slice(&[0, 0, 0, 1, 0x04]);
        let mut input = &bytes[..];
        assert_eq!(read(&mut input).unwrap(), Some(Record::Start { opus: true, frame: 480, channels: 1 }));
        assert_eq!(read(&mut input).unwrap(), Some(Record::Media { pts: 10_000, packet: vec![9, 8] }));
        assert_eq!(read(&mut input).unwrap(), Some(Record::Silence));
        assert_eq!(read(&mut input).unwrap(), None);
    }

    #[test]
    fn une_perte_est_reparee_sans_decalage() {
        let p = packets(5);
        let mut d = Decoder::new(480).unwrap();
        let mut out = Vec::new();
        d.decode(0, &p[0], &mut out);
        d.decode(10_000, &p[1], &mut out);
        // p[2] perdu.
        d.decode(30_000, &p[3], &mut out);
        d.decode(40_000, &p[4], &mut out);
        assert_eq!(out.len(), 5 * 480, "chaque trame a sa place");
        assert_eq!(d.recovered, 1);
        // Un paquet en retard, déjà refait : ignoré.
        d.decode(20_000, &p[2], &mut out);
        assert_eq!(out.len(), 5 * 480);
    }

    #[test]
    fn un_long_trou_nest_pas_masque() {
        let p = packets(2);
        let mut d = Decoder::new(480).unwrap();
        let mut out = Vec::new();
        d.decode(0, &p[0], &mut out);
        d.decode(500_000, &p[1], &mut out);
        assert_eq!(out.len(), 2 * 480);
    }
}
