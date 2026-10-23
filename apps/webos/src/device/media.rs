//! Image et son de la session.
//!
//! - Image : les paquets du correctif 0001 (Annex B) vont au décodeur du
//!   téléviseur (NDL), depuis un fil à part (le fil réseau de Kyber ne bloque
//!   jamais). Le jeu de paramètres (SPS/PPS, VPS, en-tête AV1) est placé devant
//!   chaque image clé qui (re)démarre le décodeur ; après une perte, rien
//!   avant l'image clé suivante, redemandée par le cœur.
//! - Son : Opus décodé par le moteur du cœur (tampon adaptatif, masquage,
//!   mixage en stéréo), tiré par le rappel de SDL (PulseAudio du téléviseur).

use std::ffi::{c_int, c_void};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use kybernow_core::audio::{AudioEngine, MonotonicClock, OutputOrder};
use kybernow_core::latency;
use kybernow_core::media::{Packet, PacketHandler, FOURCC_AV1, FOURCC_HEVC};

use super::ndl;
use super::sdl;
use super::session::Shared;

enum Msg {
    Codec(u32),
    Config(Vec<u8>),
    Frame { key: bool, pts: u64, data: Vec<u8>, received: i64 },
    Hole,
    Stop,
}

/// Un flux d'image (un par démarrage de l'image).
pub struct VideoSink {
    tx: Mutex<Sender<Msg>>,
    thread: Mutex<Option<JoinHandle<()>>>,
    shared: Arc<Shared>,
}

impl VideoSink {
    pub fn new(shared: Arc<Shared>) -> Arc<Self> {
        let (tx, rx) = channel();
        let s = shared.clone();
        let thread = std::thread::Builder::new().name("kybernow-image".into()).spawn(move || Feeder::new(s).run(rx)).ok();
        Arc::new(Self { tx: Mutex::new(tx), thread: Mutex::new(thread), shared })
    }

    fn send(&self, m: Msg) {
        let _ = self.tx.lock().unwrap().send(m);
    }
}

impl PacketHandler for VideoSink {
    fn packet(&self, packet: Packet<'_>) {
        match packet {
            Packet::Codec { codec } => self.send(Msg::Codec(codec)),
            Packet::Media { is_config: true, data, .. } => self.send(Msg::Config(data.to_vec())),
            Packet::Media { is_key, pts, data, .. } => {
                if let Some(s) = self.shared.session() {
                    s.latency().received_bytes(data.len());
                }
                self.send(Msg::Frame { key: is_key, pts, data: data.to_vec(), received: latency::now_micros() });
            }
            Packet::Hole { .. } => {
                if let Some(s) = self.shared.session() {
                    s.latency().hole();
                }
                self.send(Msg::Hole);
            }
        }
    }
}

impl Drop for VideoSink {
    fn drop(&mut self) {
        self.send(Msg::Stop);
        if let Some(t) = self.thread.lock().unwrap().take() {
            let _ = t.join();
        }
    }
}

struct Feeder {
    shared: Arc<Shared>,
    kind: i32,
    config: Vec<u8>,
    in_config: bool,
    loaded_config: Option<Vec<u8>>,
    waiting_key: bool,
    lost_reported: bool,
    last_request: Option<Instant>,
    failed: bool,
}

impl Feeder {
    fn new(shared: Arc<Shared>) -> Self {
        Self { shared, kind: ndl::VIDEO_H265, config: Vec::new(), in_config: false, loaded_config: None, waiting_key: true, lost_reported: false, last_request: None, failed: false }
    }

    fn run(mut self, rx: Receiver<Msg>) {
        while let Ok(m) = rx.recv() {
            match m {
                Msg::Codec(c) => {
                    self.kind = match c {
                        FOURCC_HEVC => ndl::VIDEO_H265,
                        FOURCC_AV1 => ndl::VIDEO_AV1,
                        _ => ndl::VIDEO_H264,
                    };
                    self.loaded_config = None;
                    self.config.clear();
                    self.waiting_key = true;
                    self.shared.set_codec(ndl::name(self.kind));
                }
                Msg::Config(bytes) => {
                    if !self.in_config {
                        self.config.clear();
                    }
                    self.config.extend_from_slice(&bytes);
                    self.in_config = true;
                }
                Msg::Frame { key, pts, data, received } => {
                    self.in_config = false;
                    self.frame(key, pts, &data, received);
                }
                Msg::Hole => self.lost(),
                Msg::Stop => break,
            }
        }
        ndl::unload();
    }

    fn request_keyframe(&mut self) {
        if self.last_request.is_some_and(|t| t.elapsed() < Duration::from_millis(300)) {
            return;
        }
        self.last_request = Some(Instant::now());
        if let Some(s) = self.shared.session() {
            s.request_keyframe();
        }
    }

    fn lost(&mut self) {
        self.waiting_key = true;
        if !self.lost_reported {
            self.lost_reported = true;
            if let Some(s) = self.shared.session() {
                s.keyframe_lost();
            }
        }
    }

    fn frame(&mut self, key: bool, pts: u64, data: &[u8], received: i64) {
        // Nouveau jeu de paramètres sur une image clé : le décodeur est refait.
        let stale = key && !self.config.is_empty() && self.loaded_config.as_ref().is_some_and(|c| *c != self.config);
        if self.loaded_config.is_none() || stale {
            if !key {
                self.request_keyframe();
                return;
            }
            let (w, h) = self.shared.video_size();
            if let Err(e) = ndl::ensure(self.kind, w, h) {
                if !self.failed {
                    log::error!("image : {e}");
                    self.failed = true;
                    self.shared.decoder_failed(&e);
                }
                return;
            }
            self.loaded_config = Some(self.config.clone());
            self.waiting_key = true;
        }
        if self.waiting_key && !key {
            return;
        }
        let first = self.waiting_key;
        let buf;
        let bytes: &[u8] = if key && first && !self.config.is_empty() {
            buf = [self.config.as_slice(), data].concat();
            &buf
        } else {
            data
        };
        match ndl::play(bytes) {
            Ok(queued) => {
                if key && first {
                    self.waiting_key = false;
                    if self.lost_reported {
                        self.lost_reported = false;
                        if let Some(s) = self.shared.session() {
                            s.keyframe_recovered();
                        }
                    }
                }
                // NDL ne dit pas quand l'image est montrée : estimée d'après les
                // images en attente et la fréquence de la dalle.
                let decoded = latency::now_micros();
                let period = self.shared.frame_period_micros();
                let at = decoded + period * (1 + queued.unwrap_or(0).max(0) as i64);
                if let Some(s) = self.shared.session() {
                    s.frame_displayed(pts, received, decoded, at);
                }
            }
            Err(e) => {
                log::warn!("image : {e}");
                self.lost();
            }
        }
    }
}

// MARK: - Son

const RATE: i32 = 48_000;

/// Le son de la session : moteur du cœur, sortie SDL en stéréo.
pub struct AudioSink {
    engine: Arc<AudioEngine>,
    device: Mutex<u32>,
    /// Le moteur, prêté au rappel de SDL.
    holder: Mutex<Option<Box<Arc<AudioEngine>>>>,
}

impl AudioSink {
    pub fn new() -> Arc<Self> {
        let engine = Arc::new(AudioEngine::with_order(Arc::new(MonotonicClock::new()), OutputOrder::Windows));
        Arc::new(Self { engine, device: Mutex::new(0), holder: Mutex::new(None) })
    }

    fn open(&self) {
        let mut dev = self.device.lock().unwrap();
        if *dev != 0 {
            return;
        }
        let holder = Box::new(self.engine.clone());
        let user = &*holder as *const Arc<AudioEngine> as *mut c_void;
        let want = sdl::SDL_AudioSpec {
            freq: RATE,
            format: sdl::AUDIO_F32SYS,
            channels: 2,
            silence: 0,
            samples: 512,
            padding: 0,
            size: 0,
            callback: Some(fill),
            userdata: user,
        };
        let mut have = sdl::SDL_AudioSpec { freq: 0, format: 0, channels: 0, silence: 0, samples: 0, padding: 0, size: 0, callback: None, userdata: std::ptr::null_mut() };
        let id = unsafe { sdl::SDL_OpenAudioDevice(std::ptr::null(), 0, &want, &mut have, 0) };
        if id == 0 {
            log::error!("son : sortie SDL impossible ({})", sdl::error());
            return;
        }
        let ms = have.samples as f64 * 1000.0 / have.freq.max(1) as f64;
        self.engine.set_output_latency_ms(ms * 2.0);
        log::info!("son : SDL {} voies, {} Hz, {} images par rappel", have.channels, have.freq, have.samples);
        *self.holder.lock().unwrap() = Some(holder);
        unsafe { sdl::SDL_PauseAudioDevice(id, 0) };
        *dev = id;
    }
}

extern "C" fn fill(user: *mut c_void, stream: *mut u8, len: c_int) {
    let engine = unsafe { &*(user as *const Arc<AudioEngine>) };
    let n = len.max(0) as usize / 4;
    let out = unsafe { std::slice::from_raw_parts_mut(stream as *mut f32, n) };
    engine.fill(out, n / 2);
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
                if !data.is_empty() {
                    self.engine.decode(data);
                }
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
