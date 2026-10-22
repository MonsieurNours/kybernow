//! Décodage de l'image par MediaCodec, rendu direct sur la surface de la
//! `SurfaceView` (l'écran la reçoit sans composition).
//!
//! Les paquets arrivent sur le fil réseau de Kyber, qui ne doit jamais
//! bloquer : ils sont recopiés et passés au fil du décodeur (entrée du
//! codec). Un second fil vide la sortie : chaque image décodée part à
//! l'écran ; si plusieurs attendent, seule la plus récente est montrée, les
//! autres sont écartées (comme sur les autres apps).
//!
//! Le jeu de paramètres (SPS/PPS, VPS, en-tête de séquence AV1) est gardé et
//! placé devant chaque image clé qui démarre un décodeur : MediaCodec le lit
//! dans le flux, sans `csd-0`, quel que soit le format. Une perte fait
//! attendre la prochaine image clé (redemandée par le cœur).

use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use jni::objects::JValue;
use ndk::media::media_codec::{DequeuedInputBufferResult, DequeuedOutputBufferInfoResult, MediaCodec, MediaCodecDirection};
use ndk::media::media_format::MediaFormat;

use kybernow_core::latency;
use kybernow_core::media::{Packet, PacketHandler, FOURCC_AV1, FOURCC_HEVC};

use crate::session::Shared;

const BUFFER_FLAG_KEY_FRAME: u32 = 1;

pub enum Msg {
    Codec(u32),
    Config(Vec<u8>),
    Frame { key: bool, pts: u64, data: Vec<u8>, received: i64 },
    Hole,
    /// La surface a changé (recréée, ou retirée en arrière-plan).
    Surface,
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
        *shared.video_tx.lock().unwrap() = Some(tx.clone());
        let s = shared.clone();
        let thread = std::thread::Builder::new()
            .name("kybernow-image".into())
            .spawn(move || DecodeLoop::new(s).run(rx))
            .ok();
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

/// Le codec, partagé entre le fil d'entrée et celui de sortie (MediaCodec
/// le permet : une file d'entrée et une file de sortie indépendantes).
struct Codec(MediaCodec);
unsafe impl Send for Codec {}
unsafe impl Sync for Codec {}

struct Decoder {
    codec: Arc<Codec>,
    stop: Arc<AtomicBool>,
    output: Option<JoinHandle<()>>,
    config: Vec<u8>,
}

impl Drop for Decoder {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(t) = self.output.take() {
            let _ = t.join();
        }
        let _ = self.codec.0.stop();
    }
}

/// Instant de réception de chaque image envoyée au décodeur (latence).
type Received = Arc<Mutex<VecDeque<(u64, i64)>>>;

struct DecodeLoop {
    shared: Arc<Shared>,
    fourcc: u32,
    config: Vec<u8>,
    /// Le dernier paquet reçu était un jeu de paramètres (les suivants s'y ajoutent).
    in_config: bool,
    decoder: Option<Decoder>,
    /// Après une perte ou un décodeur refait : rien avant une image clé.
    waiting_key: bool,
    lost_reported: bool,
    last_request: Option<Instant>,
    received: Received,
}

impl DecodeLoop {
    fn new(shared: Arc<Shared>) -> Self {
        Self {
            shared,
            fourcc: 0,
            config: Vec::new(),
            in_config: false,
            decoder: None,
            waiting_key: true,
            lost_reported: false,
            last_request: None,
            received: Arc::new(Mutex::new(VecDeque::with_capacity(64))),
        }
    }

    fn run(mut self, rx: Receiver<Msg>) {
        while let Ok(m) = rx.recv() {
            match m {
                Msg::Codec(c) => {
                    self.fourcc = c;
                    self.decoder = None;
                    self.config.clear();
                    self.waiting_key = true;
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
                    self.frame(key, pts, data, received);
                }
                Msg::Hole => self.lost(),
                Msg::Surface => {
                    self.decoder = None;
                    self.waiting_key = true;
                    self.request_keyframe();
                }
                Msg::Stop => break,
            }
        }
        self.decoder = None;
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

    fn frame(&mut self, key: bool, pts: u64, data: Vec<u8>, received: i64) {
        let stale = self.decoder.as_ref().is_some_and(|d| key && !self.config.is_empty() && d.config != self.config);
        if self.decoder.is_none() || stale {
            if !key {
                self.request_keyframe();
                return;
            }
            self.decoder = None;
            match self.open() {
                Some(d) => self.decoder = Some(d),
                None => return,
            }
        }
        if self.waiting_key && !key {
            return;
        }
        let first = self.waiting_key;
        let Some(d) = &self.decoder else { return };
        let codec = d.codec.clone();
        // Image clé qui (re)démarre le décodeur : le jeu de paramètres devant.
        let prefix: &[u8] = if key && first { &self.config } else { &[] };
        match codec.0.dequeue_input_buffer(Duration::from_millis(40)) {
            Ok(DequeuedInputBufferResult::Buffer(mut buf)) => {
                let dst = buf.buffer_mut();
                let n = prefix.len() + data.len();
                if n > dst.len() {
                    log::warn!("image : {n} octets pour un tampon de {}", dst.len());
                    // Le tampon est rendu au codec, vide : sinon il serait perdu pour la session.
                    let _ = codec.0.queue_input_buffer(buf, 0, 0, pts, 0);
                    self.lost();
                    return;
                }
                // Copie directe dans le tampon du codec (non initialisé, assez grand : vérifié plus haut).
                unsafe {
                    let p = dst.as_mut_ptr() as *mut u8;
                    std::ptr::copy_nonoverlapping(prefix.as_ptr(), p, prefix.len());
                    std::ptr::copy_nonoverlapping(data.as_ptr(), p.add(prefix.len()), data.len());
                }
                {
                    let mut r = self.received.lock().unwrap();
                    if r.len() == 64 {
                        r.pop_front();
                    }
                    r.push_back((pts, received));
                }
                let flags = if key { BUFFER_FLAG_KEY_FRAME } else { 0 };
                if let Err(e) = codec.0.queue_input_buffer(buf, 0, n, pts, flags) {
                    log::warn!("image : envoi au décodeur refusé ({e:?})");
                    self.lost();
                    return;
                }
                if key && first {
                    self.waiting_key = false;
                    if self.lost_reported {
                        self.lost_reported = false;
                        if let Some(s) = self.shared.session() {
                            s.keyframe_recovered();
                        }
                    }
                }
            }
            Ok(DequeuedInputBufferResult::TryAgainLater) => {
                log::warn!("image : décodeur saturé, image écartée");
                self.lost();
            }
            Err(e) => {
                log::warn!("image : décodeur en erreur ({e:?}), refait à la prochaine image clé");
                self.decoder = None;
                self.lost();
            }
        }
    }

    fn open(&mut self) -> Option<Decoder> {
        let window = self.shared.window.lock().unwrap().clone()?;
        let mime = match self.fourcc {
            FOURCC_HEVC => "video/hevc",
            FOURCC_AV1 => "video/av01",
            _ => "video/avc",
        };
        let (w, h) = self.shared.video_size();
        let codec = MediaCodec::from_decoder_type(mime)?;
        let configured = codec
            .configure(&format(mime, w, h, true), Some(&window), MediaCodecDirection::Decoder)
            .or_else(|_| codec.configure(&format(mime, w, h, false), Some(&window), MediaCodecDirection::Decoder));
        if let Err(e) = configured {
            log::error!("image : décodeur {mime} impossible à configurer ({e:?})");
            return None;
        }
        if let Err(e) = codec.start() {
            log::error!("image : décodeur {mime} impossible à démarrer ({e:?})");
            return None;
        }
        let name = codec.name().unwrap_or_default();
        log::info!("image : décodeur {name} ({mime}, {w} × {h})");
        self.shared.listener.with(|env, obj| {
            let n = env.new_string(&name)?;
            env.call_method(obj, "onDecoder", "(Ljava/lang/String;)V", &[JValue::Object(&n)]).map(|_| ())
        });
        self.waiting_key = true;
        let codec = Arc::new(Codec(codec));
        let stop = Arc::new(AtomicBool::new(false));
        let (c, st, sh, rec) = (codec.clone(), stop.clone(), self.shared.clone(), self.received.clone());
        let output = std::thread::Builder::new().name("kybernow-affichage".into()).spawn(move || drain(c, st, sh, rec)).ok();
        Some(Decoder { codec, stop, output, config: self.config.clone() })
    }
}

/// Format du décodeur : faible latence (Android 11), priorité temps réel, et
/// les clés des fabricants connues pour couper la remise en ordre des images
/// (Qualcomm, Samsung Exynos, Amlogic). Refusé avec elles : on réessaie sans.
fn format(mime: &str, w: u32, h: u32, vendor: bool) -> MediaFormat {
    let mut f = MediaFormat::new();
    f.set_str("mime", mime);
    f.set_i32("width", w as i32);
    f.set_i32("height", h as i32);
    f.set_i32("max-width", w.max(3840) as i32);
    f.set_i32("max-height", h.max(2160) as i32);
    f.set_i32("low-latency", 1);
    f.set_i32("priority", 0);
    if vendor {
        f.set_i32("vendor.qti-ext-dec-picture-order.enable", 1);
        f.set_i32("vendor.qti-ext-dec-low-latency.enable", 1);
        f.set_i32("vendor.rtc-ext-dec-low-latency.enable", 1);
        f.set_i32("vendor.low-latency.enable", 1);
    }
    f
}

/// Fil de sortie : montre la plus récente des images décodées.
fn drain(codec: Arc<Codec>, stop: Arc<AtomicBool>, shared: Arc<Shared>, received: Received) {
    let c = &codec.0;
    while !stop.load(Ordering::Acquire) {
        let mut latest = None;
        let mut timeout = Duration::from_millis(20);
        loop {
            match c.dequeue_output_buffer(timeout) {
                Ok(DequeuedOutputBufferInfoResult::Buffer(b)) => {
                    if let Some(old) = latest.replace(b) {
                        let pts = old.info().presentation_time_us() as u64;
                        let _ = c.release_output_buffer(old, false);
                        if let Some(s) = shared.session() {
                            s.latency().skipped(pts);
                        }
                    }
                    timeout = Duration::ZERO;
                }
                Ok(DequeuedOutputBufferInfoResult::OutputFormatChanged) => {
                    let f = c.output_format();
                    let (w, h) = match f.rect("crop") {
                        Some((l, t, r, b)) => ((r - l + 1).max(0) as u32, (b - t + 1).max(0) as u32),
                        None => (f.i32("width").unwrap_or(0) as u32, f.i32("height").unwrap_or(0) as u32),
                    };
                    if w > 0 && h > 0 {
                        if let Some(s) = shared.session() {
                            s.decoded_size(w, h);
                        }
                        shared.listener.void("onVideoSize", "(II)V", &[JValue::Int(w as i32), JValue::Int(h as i32)]);
                    }
                }
                Ok(DequeuedOutputBufferInfoResult::OutputBuffersChanged) => {}
                Ok(DequeuedOutputBufferInfoResult::TryAgainLater) => break,
                Err(e) => {
                    if !stop.load(Ordering::Acquire) {
                        log::warn!("image : sortie du décodeur en erreur ({e:?})");
                        std::thread::sleep(Duration::from_millis(5));
                    }
                    break;
                }
            }
        }
        let Some(b) = latest else { continue };
        let pts = b.info().presentation_time_us() as u64;
        let decoded = latency::now_micros();
        let rx = {
            let mut r = received.lock().unwrap();
            let at = r.iter().position(|(p, _)| *p == pts);
            at.map(|i| {
                let v = r[i].1;
                r.drain(..=i);
                v
            })
        };
        if c.release_output_buffer(b, true).is_ok()
            && let (Some(s), Some(rx)) = (shared.session(), rx)
        {
            // Rendu demandé maintenant : l'écran le montre au balayage suivant.
            s.frame_displayed(pts, rx, decoded, latency::now_micros() + shared.frame_period_micros());
        }
    }
}
