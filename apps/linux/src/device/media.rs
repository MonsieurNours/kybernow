//! Son de la session : Opus décodé par le moteur du cœur (tampon adaptatif,
//! masquage des pertes, dérive des horloges, mixage vers le bas), tiré par le
//! rappel de SDL (PipeWire ou PulseAudio), aux voies de la sortie : stéréo,
//! 5.1 ou 7.1, dans l'ordre de SDL (celui de Windows : G, D, C, LFE, arrière,
//! côtés).

use std::ffi::{c_int, c_void};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use kybernow_core::audio::{AudioEngine, MonotonicClock, OutputOrder};
use kybernow_core::media::{Packet, PacketHandler};

use super::sdl;

const RATE: i32 = 48_000;

/// Ce que le rappel de SDL reçoit : le moteur et les voies qu'il remplit.
struct Output {
    engine: Arc<AudioEngine>,
    channels: usize,
}

pub struct AudioSink {
    engine: Arc<AudioEngine>,
    device: Mutex<u32>,
    holder: Mutex<Option<Box<Output>>>,
    /// Voies de la sortie (celles demandées à l'ordinateur) ; SDL convertit
    /// vers la carte son si elle en a d'autres.
    channels: u8,
}

/// Le son coupé depuis le menu de session.
pub static MUTED: AtomicBool = AtomicBool::new(false);

impl AudioSink {
    pub fn new(channels: u8) -> Arc<Self> {
        let engine = Arc::new(AudioEngine::with_order(Arc::new(MonotonicClock::new()), OutputOrder::Windows));
        Arc::new(Self { engine, device: Mutex::new(0), holder: Mutex::new(None), channels: channels.max(2) })
    }

    pub fn engine(&self) -> &Arc<AudioEngine> {
        &self.engine
    }

    fn open(&self) {
        let mut dev = self.device.lock().unwrap();
        if *dev != 0 {
            return;
        }
        let holder = Box::new(Output { engine: self.engine.clone(), channels: self.channels as usize });
        let user = &*holder as *const Output as *mut c_void;
        // 256 images (5,3 ms) par rappel : PipeWire tient la cadence.
        let want = sdl::SDL_AudioSpec { freq: RATE, format: sdl::AUDIO_F32SYS, channels: self.channels, silence: 0, samples: 256, padding: 0, size: 0, callback: Some(fill), userdata: user };
        let mut have = sdl::SDL_AudioSpec::empty();
        let id = unsafe { sdl::SDL_OpenAudioDevice(std::ptr::null(), 0, &want, &mut have, 0) };
        if id == 0 {
            log::error!("son : sortie SDL impossible ({})", sdl::error());
            return;
        }
        let ms = have.samples as f64 * 1000.0 / have.freq.max(1) as f64;
        self.engine.set_output_latency_ms(ms * 2.0);
        log::info!("son : SDL ({}) {} voies, {} Hz, {} images par rappel", sdl::audio_driver(), have.channels, have.freq, have.samples);
        *self.holder.lock().unwrap() = Some(holder);
        unsafe { sdl::SDL_PauseAudioDevice(id, 0) };
        *dev = id;
    }
}

extern "C" fn fill(user: *mut c_void, stream: *mut u8, len: c_int) {
    let out_ = unsafe { &*(user as *const Output) };
    let n = len.max(0) as usize / 4;
    let out = unsafe { std::slice::from_raw_parts_mut(stream as *mut f32, n) };
    out_.engine.fill(out, n / out_.channels.max(1));
    if MUTED.load(Ordering::Relaxed) {
        out.fill(0.0);
    }
}

impl PacketHandler for AudioSink {
    fn packet(&self, packet: Packet<'_>) {
        match packet {
            Packet::Codec { .. } => {}
            Packet::Media { is_config: true, data, .. } => {
                self.engine.configure(data);
                self.open();
            }
            Packet::Media { data, .. } => {
                if data.is_empty() {
                    return;
                }
                // Paquet avant l'en-tête : stéréo supposée (comme le moteur).
                self.open();
                self.engine.decode(data);
            }
            Packet::Hole { missing_samples } => self.engine.conceal(missing_samples as usize),
        }
    }
}

impl Drop for AudioSink {
    fn drop(&mut self) {
        self.engine.stop();
        let id = *self.device.lock().unwrap();
        if id != 0 {
            unsafe { sdl::SDL_CloseAudioDevice(id) };
        }
        self.holder.lock().unwrap().take();
    }
}
