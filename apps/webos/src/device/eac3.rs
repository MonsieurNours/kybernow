//! Encodeur E-AC-3 (Dolby Digital Plus) : libavcodec derrière `src/eac3_shim.c`.
//! Le son 5.1 décodé par l'app en ressort en trames de 1536 images (32 ms),
//! que le téléviseur laisse passer telles quelles vers l'ampli.

use std::ffi::{c_int, c_void};

unsafe extern "C" {
    fn kn_eac3_frame_size() -> c_int;
    fn kn_eac3_open(channels: c_int, bitrate: c_int, plus: c_int) -> *mut c_void;
    fn kn_eac3_encode(e: *mut c_void, interleaved: *const f32, out: *mut u8, cap: c_int) -> c_int;
    fn kn_eac3_close(e: *mut c_void);
}

/// Débit du 5.1 vers un ampli : 640 kbit/s, le plus haut de l'AC-3, que tout ampli Dolby accepte.
pub const BITRATE: i32 = 640_000;

pub struct Encoder {
    handle: *mut c_void,
    channels: usize,
    frame: usize,
    /// Images en attente (entrelacées, ordre G, D, C, LFE, arrière G, arrière D).
    pending: Vec<f32>,
    out: Vec<u8>,
}

unsafe impl Send for Encoder {}

impl Encoder {
    /// `channels` : 6 (5.1) ou 2. `None` si libavcodec refuse.
    pub fn new(channels: usize) -> Option<Self> {
        let handle = unsafe { kn_eac3_open(channels as c_int, BITRATE, 1) };
        if handle.is_null() {
            return None;
        }
        let frame = unsafe { kn_eac3_frame_size() } as usize;
        Some(Self { handle, channels, frame, pending: Vec::with_capacity(frame * channels * 2), out: vec![0; 8192] })
    }

    /// Images par trame (1536).
    pub fn frame_size(&self) -> usize {
        self.frame
    }

    /// Ajoute des images entrelacées ; rend chaque trame encodée complète.
    pub fn push(&mut self, interleaved: &[f32]) -> Vec<Vec<u8>> {
        self.pending.extend_from_slice(interleaved);
        let mut frames = Vec::new();
        let need = self.frame * self.channels;
        while self.pending.len() >= need {
            let n = unsafe { kn_eac3_encode(self.handle, self.pending.as_ptr(), self.out.as_mut_ptr(), self.out.len() as c_int) };
            self.pending.drain(..need);
            if n > 0 {
                frames.push(self.out[..n as usize].to_vec());
            }
        }
        frames
    }

    /// Une trame de silence (pour amorcer le décodeur du téléviseur).
    pub fn silence(&mut self) -> Option<Vec<u8>> {
        let zeros = vec![0.0f32; self.frame * self.channels];
        // L'encodeur garde une trame d'avance : deux pour en obtenir une.
        let mut frames = self.push(&zeros);
        if frames.is_empty() {
            frames = self.push(&zeros);
        }
        self.pending.clear();
        frames.pop()
    }
}

impl Drop for Encoder {
    fn drop(&mut self) {
        unsafe { kn_eac3_close(self.handle) };
    }
}
