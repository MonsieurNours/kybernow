//! libopus, décodeur multistream (stéréo, 5.1, 7.1) : liaisons C minimales.
//! L'encodeur ne sert qu'aux tests.

use std::ffi::c_void;

use super::{MAX_FRAMES, SAMPLE_RATE};
use crate::media::OpusHead;

unsafe extern "C" {
    fn opus_multistream_decoder_create(fs: i32, channels: i32, streams: i32, coupled: i32, mapping: *const u8, error: *mut i32) -> *mut c_void;
    fn opus_multistream_decode_float(st: *mut c_void, data: *const u8, len: i32, pcm: *mut f32, frame_size: i32, decode_fec: i32) -> i32;
    fn opus_multistream_decoder_destroy(st: *mut c_void);
    #[cfg(test)]
    fn opus_multistream_surround_encoder_create(
        fs: i32,
        channels: i32,
        mapping_family: i32,
        streams: *mut i32,
        coupled: *mut i32,
        mapping: *mut u8,
        application: i32,
        error: *mut i32,
    ) -> *mut c_void;
    #[cfg(test)]
    fn opus_multistream_encode_float(st: *mut c_void, pcm: *const f32, frame_size: i32, data: *mut u8, max_bytes: i32) -> i32;
    #[cfg(test)]
    fn opus_multistream_encoder_destroy(st: *mut c_void);
}

/// Décodeur multistream pour un flux donné par son en-tête.
pub struct Decoder {
    st: *mut c_void,
    channels: usize,
}

unsafe impl Send for Decoder {}

impl Decoder {
    /// `Err(code)` : erreur libopus.
    pub fn new(head: &OpusHead) -> Result<Self, i32> {
        let mut err = 0;
        let st = unsafe {
            opus_multistream_decoder_create(
                SAMPLE_RATE as i32,
                i32::from(head.channels),
                i32::from(head.streams),
                i32::from(head.coupled),
                head.mapping.as_ptr(),
                &mut err,
            )
        };
        if st.is_null() || err != 0 {
            return Err(err);
        }
        Ok(Self {
            st,
            channels: usize::from(head.channels),
        })
    }

    pub fn channels(&self) -> usize {
        self.channels
    }

    /// Décode un paquet dans `out` (images entrelacées, `MAX_FRAMES × voies`
    /// au moins) ; `None` : une trame de masquage de `frames` images (multiple
    /// de 120, 2,5 ms). Rend le nombre d'images écrites, 0 en cas d'erreur.
    pub fn decode(&mut self, data: Option<&[u8]>, frames: usize, out: &mut [f32]) -> usize {
        let frames = frames.min(MAX_FRAMES).min(out.len() / self.channels);
        let (ptr, len) = match data {
            Some(d) => (d.as_ptr(), d.len() as i32),
            None => (std::ptr::null(), 0),
        };
        let n = unsafe { opus_multistream_decode_float(self.st, ptr, len, out.as_mut_ptr(), frames as i32, 0) };
        if n > 0 { n as usize } else { 0 }
    }
}

impl Drop for Decoder {
    fn drop(&mut self) {
        unsafe { opus_multistream_decoder_destroy(self.st) };
    }
}

/// Encodeur pour les tests : un flux comme celui du serveur (famille 0 en
/// stéréo, famille 1 au-delà), et son en-tête `OpusHead`.
#[cfg(test)]
pub struct Encoder {
    st: *mut c_void,
    channels: usize,
    pub head: Vec<u8>,
}

#[cfg(test)]
impl Encoder {
    pub fn new(channels: usize) -> Self {
        let mut err = 0;
        let (mut streams, mut coupled) = (0i32, 0i32);
        let mut mapping = vec![0u8; channels];
        let family: i32 = if channels > 2 { 1 } else { 0 };
        let st = unsafe {
            opus_multistream_surround_encoder_create(
                SAMPLE_RATE as i32,
                channels as i32,
                family,
                &mut streams,
                &mut coupled,
                mapping.as_mut_ptr(),
                2051, // OPUS_APPLICATION_RESTRICTED_LOWDELAY
                &mut err,
            )
        };
        assert!(!st.is_null() && err == 0, "encodeur Opus : erreur {err}");
        let mut head = b"OpusHead".to_vec();
        head.extend_from_slice(&[1, channels as u8, 0, 0]);
        head.extend_from_slice(&(SAMPLE_RATE as u32).to_le_bytes());
        head.extend_from_slice(&[0, 0, family as u8]);
        if family != 0 {
            head.extend_from_slice(&[streams as u8, coupled as u8]);
            head.extend_from_slice(&mapping);
        }
        Self { st, channels, head }
    }

    /// Encode `frames` images entrelacées.
    pub fn encode(&mut self, pcm: &[f32]) -> Vec<u8> {
        let frames = pcm.len() / self.channels;
        let mut packet = vec![0u8; 4000];
        let len = unsafe { opus_multistream_encode_float(self.st, pcm.as_ptr(), frames as i32, packet.as_mut_ptr(), 4000) };
        assert!(len > 0, "encodage Opus : erreur {len}");
        packet.truncate(len as usize);
        packet
    }
}

#[cfg(test)]
impl Drop for Encoder {
    fn drop(&mut self) {
        unsafe { opus_multistream_encoder_destroy(self.st) };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aller_retour_stereo_et_5_1() {
        for channels in [2usize, 6, 8] {
            let mut enc = Encoder::new(channels);
            let head = OpusHead::parse(&enc.head).expect("en-tête");
            assert_eq!(usize::from(head.channels), channels);
            let mut dec = Decoder::new(&head).expect("décodeur");
            let mut pcm = vec![0f32; 960 * channels];
            let mut out = vec![0f32; MAX_FRAMES * channels];
            let mut phase = 0f64;
            // Quelques paquets pour que l'encodeur se cale, puis une sinusoïde reconnaissable.
            for _ in 0..5 {
                for i in 0..960 {
                    let v = (phase.sin() * 0.2) as f32;
                    phase += 2.0 * std::f64::consts::PI * 440.0 / SAMPLE_RATE as f64;
                    for c in 0..channels {
                        pcm[i * channels + c] = v;
                    }
                }
                let packet = enc.encode(&pcm);
                assert_eq!(dec.decode(Some(&packet), MAX_FRAMES, &mut out), 960, "{channels} voies : 960 images décodées");
            }
            let rms = (out[..960 * channels].iter().map(|v| v * v).sum::<f32>() / (960 * channels) as f32).sqrt();
            assert!((rms - 0.141).abs() < 0.03, "{channels} voies : niveau décodé {rms}");
            assert_eq!(dec.decode(None, 480, &mut out), 480, "trame de masquage de 10 ms");
            assert_eq!(dec.decode(None, 100, &mut out), 0, "pas un multiple de 2,5 ms : refusé");
        }
    }
}
