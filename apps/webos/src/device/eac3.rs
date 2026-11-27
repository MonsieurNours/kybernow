//! Encodeur E-AC-3 (Dolby Digital Plus) : libavcodec derrière `src/eac3_shim.c`.
//! Le son 5.1 décodé par l'app en ressort en trames de 1536 images (32 ms),
//! que le téléviseur laisse passer telles quelles vers l'ampli.

use std::ffi::{c_int, c_void};

unsafe extern "C" {
    fn kn_eac3_frame_size(e: *const c_void) -> c_int;
    fn kn_eac3_open(channels: c_int, bitrate: c_int, plus: c_int) -> *mut c_void;
    fn kn_eac3_encode(e: *mut c_void, interleaved: *const f32, out: *mut u8, cap: c_int) -> c_int;
    fn kn_eac3_close(e: *mut c_void);
}

/// Débit du 5.1 vers un ampli : 640 kbit/s, le plus haut de l'AC-3, que tout ampli Dolby accepte.
pub const BITRATE: i32 = 640_000;

/// Les trames encodées d'un appel à `push`, dans un tampon réutilisé (WEBOS-22) : les octets bout à bout, et la
/// fin de chaque trame.
#[derive(Default)]
pub struct Frames {
    bytes: Vec<u8>,
    ends: Vec<usize>,
}

impl Frames {
    pub fn clear(&mut self) {
        self.bytes.clear();
        self.ends.clear();
    }

    pub fn is_empty(&self) -> bool {
        self.ends.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = &[u8]> {
        let mut start = 0;
        self.ends.iter().map(move |&end| {
            let f = &self.bytes[start..end];
            start = end;
            f
        })
    }

    fn push(&mut self, frame: &[u8]) {
        self.bytes.extend_from_slice(frame);
        self.ends.push(self.bytes.len());
    }
}

pub struct Encoder {
    handle: *mut c_void,
    channels: usize,
    frame: usize,
    /// Images en attente (entrelacées, ordre G, D, C, LFE, arrière G, arrière D), à partir de `start`.
    pending: Vec<f32>,
    start: usize,
    out: Vec<u8>,
    /// Trames perdues par une erreur de l'encodeur (journal : la première, puis toutes les 1000).
    errors: u64,
}

unsafe impl Send for Encoder {}

impl Encoder {
    /// `channels` : 6 (5.1) ou 2. `None` si libavcodec refuse.
    pub fn new(channels: usize) -> Option<Self> {
        let handle = unsafe { kn_eac3_open(channels as c_int, BITRATE, 1) };
        if handle.is_null() {
            return None;
        }
        // La taille lue sur l'encodeur ouvert (WEBOS-18) : c'est elle qu'il lit à chaque trame.
        let frame = unsafe { kn_eac3_frame_size(handle) };
        if frame <= 0 {
            unsafe { kn_eac3_close(handle) };
            return None;
        }
        let frame = frame as usize;
        Some(Self { handle, channels, frame, pending: Vec::with_capacity(frame * channels * 2), start: 0, out: vec![0; 8192], errors: 0 })
    }

    /// Ajoute des images entrelacées ; chaque trame encodée complète va dans `frames` (vidé d'abord).
    pub fn push(&mut self, interleaved: &[f32], frames: &mut Frames) {
        frames.clear();
        self.pending.extend_from_slice(interleaved);
        let need = self.frame * self.channels;
        while self.pending.len() - self.start >= need {
            let n = unsafe { kn_eac3_encode(self.handle, self.pending[self.start..].as_ptr(), self.out.as_mut_ptr(), self.out.len() as c_int) };
            self.start += need;
            if n > 0 {
                frames.push(&self.out[..n as usize]);
            } else if n < 0 {
                self.errors += 1;
                if self.errors == 1 || self.errors % 1000 == 0 {
                    log::warn!("son : encodeur E-AC-3, trame perdue (erreur {n}, {} en tout)", self.errors);
                }
            }
        }
        // Le reste ramené au début, sans réallouer.
        if self.start > 0 {
            self.pending.drain(..self.start);
            self.start = 0;
        }
    }

    /// Une trame de silence (pour amorcer le décodeur du téléviseur).
    pub fn silence(&mut self) -> Option<Vec<u8>> {
        let zeros = vec![0.0f32; self.frame * self.channels];
        let mut frames = Frames::default();
        // L'encodeur garde une trame d'avance : deux pour en obtenir une.
        self.push(&zeros, &mut frames);
        if frames.is_empty() {
            self.push(&zeros, &mut frames);
        }
        self.pending.clear();
        self.start = 0;
        frames.iter().last().map(<[u8]>::to_vec)
    }
}

impl Drop for Encoder {
    fn drop(&mut self) {
        unsafe { kn_eac3_close(self.handle) };
    }
}
