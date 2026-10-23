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

use kybernow_core::audio::{opus, AudioEngine, MonotonicClock, OutputOrder, MAX_FRAMES};
use kybernow_core::latency;
use kybernow_core::media::{OpusHead, Packet, PacketHandler, FOURCC_AV1, FOURCC_HEVC};

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
            Err(e) if e == ndl::NOT_READY => {
                // Le décodeur attend sa confirmation : on attend l'image clé suivante.
                self.waiting_key = true;
                self.request_keyframe();
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

/// Où part le son.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Route {
    /// En-tête pas encore reçu.
    Pending,
    /// Opus 5.1 décodé par l'app, PCM 6 voies au plan de son de NDL : le PCM
    /// multicanal de la TV vers l'ampli (eARC, Sortie son numérique en
    /// Pass-Through). Le décodage par la TV elle-même (plan Opus 6 voies,
    /// essayé le 23/10/2026) sort en stéréo : sa sortie passe par le mixeur.
    NdlPcm,
    /// Moteur du cœur, sortie SDL en stéréo.
    Sdl,
}

/// Ordre des voies du PCM 6 voies de NDL (G, D, C, LFE, arrière G, arrière
/// D : celui de WAVE et de l'HDMI), depuis celui d'Opus (G, C, D, arrière G,
/// arrière D, LFE).
const PCM_ORDER: [usize; 6] = [0, 2, 1, 5, 3, 4];

/// Le son de la session : 5.1 par NDL quand il est demandé et que le flux
/// s'y prête, sinon moteur du cœur et sortie SDL en stéréo.
pub struct AudioSink {
    engine: Arc<AudioEngine>,
    device: Mutex<u32>,
    /// Le moteur, prêté au rappel de SDL.
    holder: Mutex<Option<Box<Arc<AudioEngine>>>>,
    surround: bool,
    route: Mutex<Route>,
    /// L'en-tête Opus, pour repasser par SDL si NDL refuse le son.
    head: Mutex<Vec<u8>>,
    /// Décodeur 5.1 de la route NDL, et ses tampons.
    decoder: Mutex<Option<(opus::Decoder, Vec<f32>, Vec<u8>)>>,
    /// Paquets donnés à NDL (journal).
    ndl_packets: Mutex<u64>,
}

impl AudioSink {
    /// `surround` : le son 5.1 est demandé à l'ordinateur et confié à NDL.
    pub fn new(surround: bool) -> Arc<Self> {
        let engine = Arc::new(AudioEngine::with_order(Arc::new(MonotonicClock::new()), OutputOrder::Windows));
        Arc::new(Self {
            engine,
            device: Mutex::new(0),
            holder: Mutex::new(None),
            surround,
            route: Mutex::new(Route::Pending),
            head: Mutex::new(Vec::new()),
            decoder: Mutex::new(None),
            ndl_packets: Mutex::new(0),
        })
    }

    /// Décode un paquet (ou masque `conceal` images) en PCM S16LE 6 voies
    /// dans l'ordre de NDL, et le lui donne.
    fn feed_ndl(&self, data: Option<&[u8]>, conceal: usize) {
        let mut g = self.decoder.lock().unwrap();
        let Some((dec, pcm, bytes)) = g.as_mut() else { return };
        let frames = dec.decode(data, if data.is_some() { MAX_FRAMES } else { conceal.clamp(120, MAX_FRAMES) / 120 * 120 }, pcm);
        if frames == 0 {
            return;
        }
        bytes.clear();
        for f in 0..frames {
            let src = &pcm[f * 6..f * 6 + 6];
            for &i in &PCM_ORDER {
                let s = (src[i].clamp(-1.0, 1.0) * 32767.0) as i16;
                bytes.extend_from_slice(&s.to_le_bytes());
            }
        }
        match ndl::play_audio(bytes) {
            Ok(sent) => {
                if sent {
                    let mut n = self.ndl_packets.lock().unwrap();
                    *n += 1;
                    if *n == 1 {
                        log::info!("son : premier paquet PCM 5.1 accepté par NDL ; sortie multicanal du téléviseur : {:?}", ndl::multichannel());
                    }
                }
            }
            Err(e) => {
                drop(g);
                log::warn!("son : {e} ; stéréo par SDL pour la suite de la session");
                self.to_sdl();
            }
        }
    }

    /// Choisit la route d'après l'en-tête du flux.
    fn configure(&self, data: &[u8]) {
        *self.head.lock().unwrap() = data.to_vec();
        let head = OpusHead::parse(data);
        if let Some(h) = &head {
            log::info!("son : en-tête Opus {} voies, {} flux, {} couplés, ordre {:?}", h.channels, h.streams, h.coupled, h.mapping);
        }
        let decoder = match head.as_ref().filter(|h| self.surround && h.channels == 6).map(opus::Decoder::new) {
            Some(Ok(d)) => Some(d),
            Some(Err(e)) => {
                log::warn!("son : décodeur Opus 5.1 impossible ({e})");
                None
            }
            None => None,
        };
        match decoder {
            Some(d) => {
                *self.decoder.lock().unwrap() = Some((d, vec![0.0; MAX_FRAMES * 6], Vec::with_capacity(MAX_FRAMES * 12)));
                *self.route.lock().unwrap() = Route::NdlPcm;
                log::info!("son : 5.1 décodé par l'app, PCM 6 voies au téléviseur (NDL)");
            }
            None => {
                if self.surround {
                    log::warn!("son : 5.1 demandé mais flux pas en 6 voies ; stéréo par SDL");
                }
                self.to_sdl();
            }
        }
    }

    /// Repasse par le moteur du cœur et SDL (stéréo mixée).
    fn to_sdl(&self) {
        *self.route.lock().unwrap() = Route::Sdl;
        self.decoder.lock().unwrap().take();
        let head = self.head.lock().unwrap().clone();
        self.engine.configure(&head);
        self.open();
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
            Packet::Media { is_config: true, data, .. } => self.configure(data),
            Packet::Media { data, .. } => {
                if data.is_empty() {
                    return;
                }
                let route = *self.route.lock().unwrap();
                match route {
                    Route::NdlPcm => self.feed_ndl(Some(data), 0),
                    // Paquet avant l'en-tête : stéréo supposée (comme le moteur).
                    Route::Pending => {
                        *self.route.lock().unwrap() = Route::Sdl;
                        self.open();
                        self.engine.decode(data);
                    }
                    Route::Sdl => self.engine.decode(data),
                }
            }
            Packet::Hole { missing_samples } => match *self.route.lock().unwrap() {
                Route::Sdl => self.engine.conceal(missing_samples as usize),
                Route::NdlPcm => self.feed_ndl(None, missing_samples as usize),
                Route::Pending => {}
            },
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
