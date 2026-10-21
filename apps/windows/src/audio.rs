//! Son : Opus décodé par libopus dès qu'il arrive (masquage des pertes
//! compris), joué par WASAPI en mode partagé, piloté par événement. Une petite
//! réserve (30 ms) absorbe la gigue du réseau ; au-delà de 100 ms, le plus
//! ancien est jeté pour ne pas laisser le son prendre du retard.

use std::collections::VecDeque;
use std::ffi::c_void;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use windows::Win32::Foundation::{CloseHandle, HANDLE, WAIT_OBJECT_0};
use windows::Win32::Media::Audio::*;
use windows::Win32::Media::KernelStreaming::WAVE_FORMAT_EXTENSIBLE;
use windows::Win32::Media::Multimedia::KSDATAFORMAT_SUBTYPE_IEEE_FLOAT;
use windows::Win32::System::Com::{CLSCTX_ALL, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx, CoTaskMemFree};
use windows::Win32::System::Threading::{CreateEventW, WaitForSingleObject};

use kybernow_core::media::{OpusHead, Packet, PacketHandler};

const RATE: u32 = 48_000;
const TARGET_MS: usize = 30;
const MAX_MS: usize = 100;

unsafe extern "C" {
    fn opus_multistream_decoder_create(
        fs: i32,
        channels: i32,
        streams: i32,
        coupled: i32,
        mapping: *const u8,
        error: *mut i32,
    ) -> *mut c_void;
    fn opus_multistream_decode_float(
        st: *mut c_void,
        data: *const u8,
        len: i32,
        pcm: *mut f32,
        frame_size: i32,
        decode_fec: i32,
    ) -> i32;
    fn opus_multistream_decoder_destroy(st: *mut c_void);
}

/// Ordre des voies Opus (Vorbis) → ordre de Windows (WAVEFORMATEXTENSIBLE).
fn reorder(channels: usize) -> Vec<usize> {
    match channels {
        3 => vec![0, 2, 1],
        6 => vec![0, 2, 1, 5, 3, 4],
        8 => vec![0, 2, 1, 7, 5, 6, 3, 4],
        n => (0..n).collect(),
    }
}

fn channel_mask(channels: u32) -> u32 {
    match channels {
        1 => 0x4,
        2 => 0x3,
        6 => 0x3F,
        8 => 0x63F,
        _ => 0,
    }
}

struct Opus {
    st: *mut c_void,
    channels: usize,
    order: Vec<usize>,
}

unsafe impl Send for Opus {}

impl Opus {
    fn new(head: &OpusHead) -> Option<Self> {
        let mut err = 0;
        let st = unsafe {
            opus_multistream_decoder_create(
                RATE as i32,
                head.channels as i32,
                head.streams as i32,
                head.coupled as i32,
                head.mapping.as_ptr(),
                &mut err,
            )
        };
        if st.is_null() || err != 0 {
            log::error!("libopus : décodeur refusé ({err})");
            return None;
        }
        Some(Self {
            st,
            channels: head.channels as usize,
            order: reorder(head.channels as usize),
        })
    }

    /// `None` en entrée : masque une perte de `frames` échantillons.
    fn decode(&mut self, data: Option<&[u8]>, frames: usize, out: &mut Vec<f32>) {
        let mut pcm = vec![0f32; frames * self.channels];
        let (ptr, len) = match data {
            Some(d) => (d.as_ptr(), d.len() as i32),
            None => (std::ptr::null(), 0),
        };
        let n = unsafe { opus_multistream_decode_float(self.st, ptr, len, pcm.as_mut_ptr(), frames as i32, 0) };
        if n <= 0 {
            return;
        }
        for frame in pcm.chunks_exact(self.channels).take(n as usize) {
            for &i in &self.order {
                out.push(frame[i]);
            }
        }
    }
}

impl Drop for Opus {
    fn drop(&mut self) {
        unsafe { opus_multistream_decoder_destroy(self.st) };
    }
}

struct Ring {
    samples: Mutex<VecDeque<f32>>,
    channels: AtomicU32,
}

pub struct AudioHandler {
    opus: Mutex<Option<Opus>>,
    ring: Arc<Ring>,
    stop: Arc<AtomicBool>,
}

impl AudioHandler {
    fn push(&self, pcm: &[f32]) {
        let ch = self.ring.channels.load(Ordering::Relaxed).max(1) as usize;
        let mut q = self.ring.samples.lock().unwrap();
        q.extend(pcm.iter().copied());
        let max = RATE as usize * MAX_MS / 1000 * ch;
        if q.len() > max {
            let target = RATE as usize * TARGET_MS / 1000 * ch;
            let drop = (q.len() - target) / ch * ch;
            q.drain(..drop);
        }
    }

    pub fn stop(&self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

impl PacketHandler for AudioHandler {
    fn packet(&self, p: Packet<'_>) {
        let mut pcm = Vec::with_capacity(960 * 8);
        {
            let mut opus = self.opus.lock().unwrap();
            match p {
                Packet::Media { is_config: true, data, .. } => {
                    let head = OpusHead::parse(data).unwrap_or_else(OpusHead::stereo);
                    log::info!("son : {} voies", head.channels);
                    *opus = Opus::new(&head);
                    self.ring.channels.store(head.channels as u32, Ordering::Relaxed);
                    self.ring.samples.lock().unwrap().clear();
                    return;
                }
                Packet::Media { data, .. } => {
                    if opus.is_none() {
                        *opus = Opus::new(&OpusHead::stereo());
                        self.ring.channels.store(2, Ordering::Relaxed);
                    }
                    if let Some(o) = opus.as_mut() {
                        o.decode(Some(data), 5760, &mut pcm);
                    }
                }
                Packet::Hole { missing_samples } => {
                    // Masquage par tranches de 120 échantillons, 960 au plus
                    // par appel, 200 ms au plus.
                    let Some(o) = opus.as_mut() else { return };
                    let mut left = (missing_samples as usize).min(RATE as usize / 5) / 120 * 120;
                    while left > 0 {
                        let n = left.min(960);
                        o.decode(None, n, &mut pcm);
                        left -= n;
                    }
                }
                Packet::Codec { .. } => return,
            }
        }
        if !pcm.is_empty() {
            self.push(&pcm);
        }
    }
}

/// Voies de la sortie son par défaut (2, 6 ou 8), à demander à la console.
pub fn default_output_channels() -> u16 {
    unsafe {
        let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
        let Ok(enumerator) = CoCreateInstance::<_, IMMDeviceEnumerator>(&MMDeviceEnumerator, None, CLSCTX_ALL) else {
            return 2;
        };
        let Ok(device) = enumerator.GetDefaultAudioEndpoint(eRender, eConsole) else { return 0 };
        let Ok(client) = device.Activate::<IAudioClient>(CLSCTX_ALL, None) else { return 2 };
        let Ok(mix) = client.GetMixFormat() else { return 2 };
        let ch = (*mix).nChannels;
        CoTaskMemFree(Some(mix as *const c_void));
        match ch {
            0..=2 => 2,
            3..=6 => 6,
            _ => 8,
        }
    }
}

pub fn spawn() -> Arc<AudioHandler> {
    let ring = Arc::new(Ring {
        samples: Mutex::new(VecDeque::new()),
        channels: AtomicU32::new(2),
    });
    let stop = Arc::new(AtomicBool::new(false));
    let handler = Arc::new(AudioHandler {
        opus: Mutex::new(None),
        ring: ring.clone(),
        stop: stop.clone(),
    });
    std::thread::Builder::new()
        .name("kybernow-audio".into())
        .spawn(move || {
            unsafe { let _ = CoInitializeEx(None, COINIT_MULTITHREADED); }
            while !stop.load(Ordering::Relaxed) {
                if let Err(e) = unsafe { play(&ring, &stop) } {
                    log::warn!("son : {e}");
                    std::thread::sleep(Duration::from_millis(500));
                }
            }
        })
        .expect("fil du son");
    handler
}

/// Joue jusqu'à l'arrêt, une erreur de la sortie ou un changement de voies.
unsafe fn play(ring: &Ring, stop: &AtomicBool) -> windows::core::Result<()> {
    unsafe {
        let channels = ring.channels.load(Ordering::Relaxed).max(1);
        let enumerator: IMMDeviceEnumerator = CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)?;
        let device = enumerator.GetDefaultAudioEndpoint(eRender, eConsole)?;
        let client: IAudioClient = device.Activate(CLSCTX_ALL, None)?;
        let fmt = WAVEFORMATEXTENSIBLE {
            Format: WAVEFORMATEX {
                wFormatTag: WAVE_FORMAT_EXTENSIBLE as u16,
                nChannels: channels as u16,
                nSamplesPerSec: RATE,
                nAvgBytesPerSec: RATE * 4 * channels,
                nBlockAlign: (4 * channels) as u16,
                wBitsPerSample: 32,
                cbSize: 22,
            },
            Samples: WAVEFORMATEXTENSIBLE_0 { wValidBitsPerSample: 32 },
            dwChannelMask: channel_mask(channels),
            SubFormat: KSDATAFORMAT_SUBTYPE_IEEE_FLOAT,
        };
        // Windows convertit fréquence et voies vers la sortie si besoin.
        client.Initialize(
            AUDCLNT_SHAREMODE_SHARED,
            AUDCLNT_STREAMFLAGS_EVENTCALLBACK | AUDCLNT_STREAMFLAGS_AUTOCONVERTPCM | AUDCLNT_STREAMFLAGS_SRC_DEFAULT_QUALITY,
            0,
            0,
            &fmt as *const _ as *const WAVEFORMATEX,
            None,
        )?;
        let event: HANDLE = CreateEventW(None, false, false, None)?;
        client.SetEventHandle(event)?;
        let render: IAudioRenderClient = client.GetService()?;
        let size = client.GetBufferSize()?;
        log::info!("son : WASAPI, {channels} voies, tampon de {} ms", size * 1000 / RATE);
        client.Start()?;
        let target = RATE as usize * TARGET_MS / 1000 * channels as usize;
        let mut buffering = true;
        let result = loop {
            if stop.load(Ordering::Relaxed) || ring.channels.load(Ordering::Relaxed) != channels {
                break Ok(());
            }
            if WaitForSingleObject(event, 200) != WAIT_OBJECT_0 {
                continue;
            }
            let padding = match client.GetCurrentPadding() {
                Ok(p) => p,
                Err(e) => break Err(e),
            };
            let avail = size.saturating_sub(padding);
            if avail == 0 {
                continue;
            }
            let buf = match render.GetBuffer(avail) {
                Ok(b) => b as *mut f32,
                Err(e) => break Err(e),
            };
            let out = std::slice::from_raw_parts_mut(buf, (avail * channels) as usize);
            let mut q = ring.samples.lock().unwrap();
            if buffering && q.len() >= target {
                buffering = false;
            }
            let mut flags = 0u32;
            if buffering {
                flags = AUDCLNT_BUFFERFLAGS_SILENT.0 as u32;
            } else {
                let n = out.len().min(q.len());
                for (o, s) in out.iter_mut().zip(q.drain(..n)) {
                    *o = s;
                }
                if n < out.len() {
                    // Plus rien : silence, et on refait une petite réserve.
                    out[n..].fill(0.0);
                    buffering = true;
                }
            }
            drop(q);
            if let Err(e) = render.ReleaseBuffer(avail, flags) {
                break Err(e);
            }
        };
        let _ = client.Stop();
        let _ = CloseHandle(event);
        result
    }
}
