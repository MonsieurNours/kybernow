//! Décodage de l'image par MediaCodec, rendu direct sur la surface de la
//! `SurfaceView` (l'écran la reçoit sans composition).
//!
//! Les paquets arrivent sur le fil réseau de Kyber, qui ne doit jamais
//! bloquer : ils sont recopiés et passés au fil du décodeur (entrée du
//! codec). Un second fil vide la sortie : chaque image décodée part à
//! l'écran ; si plusieurs attendent, seule la plus récente est montrée, les
//! autres sont écartées (comme sur les autres apps).
//!
//! Ce qu'il faut faire de chaque paquet est décidé par la machine du cœur
//! (`frames::FrameFlow`, la même que Windows, Linux et LG) : le jeu de
//! paramètres (SPS/PPS, VPS, en-tête de séquence AV1) gardé et placé devant
//! chaque image clé qui démarre un décodeur (MediaCodec le lit dans le flux,
//! sans `csd-0`, quel que soit le format), l'attente d'une image clé ou d'une
//! image de reprise après une perte, l'image clé redemandée au repos, le
//! format jamais annoncé ouvert d'office. Ici : les étapes exécutées sur
//! MediaCodec et sur PyroWave.
//!
//! PyroWave (`pwv1`, docs/pyrowave-client-android.md) passe à côté de
//! MediaCodec : décodé par Vulkan et présenté sur la même surface par une
//! swapchain (`crate::pyrowave`), sur ce même fil. Chaque image est une image
//! clé : une image incomplète est sautée, jamais d'image clé demandée ; si
//! plusieurs attendent, seule la plus récente est décodée.

use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::mpsc::{channel, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use jni::objects::JValue;
use ndk::media::media_codec::{DequeuedInputBufferResult, DequeuedOutputBufferInfoResult, MediaCodec, MediaCodecDirection};
use ndk::media::media_format::MediaFormat;

use kybernow_core::frames::{FrameFlow, Step};
use kybernow_core::latency;
use kybernow_core::media::{Packet, PacketHandler, FOURCC_AV1, FOURCC_HEVC, FOURCC_PYROWAVE};
use kybernow_core::pyrowave::Config;

use crate::media::MediaHost;
use crate::pyrowave::{PwSession, Shown};

const BUFFER_FLAG_KEY_FRAME: u32 = 1;

pub enum Msg {
    Codec(u32),
    Config(Vec<u8>),
    /// `resume` : image de reprise après une perte (docs/reprise-apres-perte.md).
    Frame { key: bool, pts: u64, data: Vec<u8>, received: i64, resume: Option<u64> },
    Hole,
    /// Des images jetées avant le décodeur (file pleine) : seule une image
    /// clé relance.
    Dropped,
    /// La surface a changé (recréée, ou retirée en arrière-plan).
    Surface,
    Stop,
}

/// Un flux d'image (un par démarrage de l'image).
/// Images en file au plus : au-delà (décodeur qui cale), les suivantes sont
/// jetées et comptées comme un trou, au lieu de remplir la mémoire (AND-26).
const MAX_QUEUED: usize = 120;

pub struct VideoSink {
    tx: Mutex<Sender<Msg>>,
    queued: Arc<AtomicUsize>,
    overflowing: AtomicBool,
    thread: Mutex<Option<JoinHandle<()>>>,
    host: Arc<dyn MediaHost>,
}

impl VideoSink {
    pub fn new(host: Arc<dyn MediaHost>) -> Arc<Self> {
        let (tx, rx) = channel();
        host.set_video_tx(tx.clone());
        let h = host.clone();
        let queued = Arc::new(AtomicUsize::new(0));
        let q = queued.clone();
        let thread = std::thread::Builder::new()
            .name("kybernow-image".into())
            .spawn(move || DecodeLoop::new(h, q).run(rx))
            .ok();
        Arc::new(Self { tx: Mutex::new(tx), queued, overflowing: AtomicBool::new(false), thread: Mutex::new(thread), host })
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
            Packet::Media { is_key, pts, data, resume, .. } => {
                self.host.received_bytes(data.len());
                if self.queued.load(Ordering::Acquire) >= MAX_QUEUED {
                    if !self.overflowing.swap(true, Ordering::AcqRel) {
                        log::warn!("image : {MAX_QUEUED} images en attente, les suivantes sont jetées");
                    }
                    self.host.hole();
                    self.send(Msg::Dropped);
                    return;
                }
                self.overflowing.store(false, Ordering::Release);
                self.queued.fetch_add(1, Ordering::AcqRel);
                self.send(Msg::Frame { key: is_key, pts, data: data.to_vec(), received: latency::now_micros(), resume });
            }
            Packet::Hole { .. } => {
                self.host.hole();
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

/// Le repos du fil de l'image : la machine du cœur redemande alors l'image
/// clé attendue (`FrameFlow::idle`).
const IDLE: Duration = Duration::from_millis(100);

/// Une image à décoder.
struct Media {
    pts: u64,
    key: bool,
    data: Vec<u8>,
    received: i64,
}

struct DecodeLoop {
    host: Arc<dyn MediaHost>,
    /// Ce qu'il faut faire de chaque paquet (`frames::FrameFlow` du cœur).
    flow: FrameFlow,
    decoder: Option<Decoder>,
    received: Received,
    /// PyroWave : la session d'affichage Vulkan (ouverte à la première image),
    /// le dernier paquet de configuration, un échec d'ouverture (réessayé avec
    /// la surface suivante) et le format dont le nom a été donné à Kotlin.
    pw: Option<PwSession>,
    pw_config: Option<Config>,
    pw_failed: bool,
    pw_announced: Option<Config>,
    /// Le repli de la session a été demandé au cœur (une fois : `pyrowave_failed`).
    pw_fallback: bool,
    /// Le dernier changement de surface pris en compte (`SurfaceGate`).
    surface_gen: u64,
    /// Images en file (`VideoSink::queued`).
    queued: Arc<AtomicUsize>,
}

impl DecodeLoop {
    fn new(host: Arc<dyn MediaHost>, queued: Arc<AtomicUsize>) -> Self {
        let flow = FrameFlow::new(host.wanted());
        Self {
            host,
            flow,
            decoder: None,
            received: Arc::new(Mutex::new(VecDeque::with_capacity(64))),
            pw: None,
            pw_config: None,
            pw_failed: false,
            pw_announced: None,
            pw_fallback: false,
            surface_gen: 0,
            queued,
        }
    }

    fn pyrowave(&self) -> bool {
        self.flow.fourcc() == FOURCC_PYROWAVE
    }

    fn run(mut self, rx: Receiver<Msg>) {
        let mut pending: VecDeque<Msg> = VecDeque::new();
        self.surface_gen = self.host.surface_gate().generation();
        loop {
            let m = match pending.pop_front() {
                Some(m) => m,
                None => match rx.recv_timeout(IDLE) {
                    Ok(m) => self.took(m),
                    Err(RecvTimeoutError::Timeout) => {
                        let steps = self.flow.idle(Instant::now());
                        self.apply(steps, None);
                        continue;
                    }
                    Err(RecvTimeoutError::Disconnected) => break,
                },
            };
            // La surface a changé : avant toute image en file, le décodeur la
            // lâche, et le fil principal qui attend dans `surfaceDestroyed` le
            // sait (AND-04). `Msg::Surface` ne sert qu'à réveiller ce fil.
            let g = self.host.surface_gate().generation();
            if g != self.surface_gen {
                self.surface_gen = g;
                self.surface_changed();
                self.host.surface_gate().released(g);
            }
            // PyroWave : seule la plus récente des images en attente est
            // décodée (les autres comptent parmi les images perdues).
            let m = match m {
                Msg::Frame { .. } if self.pyrowave() => {
                    let mut latest = m;
                    while let Ok(next) = rx.try_recv().map(|n| self.took(n)) {
                        if matches!(next, Msg::Frame { .. }) {
                            latest = next;
                        } else {
                            pending.push_back(next);
                            break;
                        }
                    }
                    latest
                }
                m => m,
            };
            // Le format que la session demande (profil changé, repli de
            // PyroWave) : ouvert d'office s'il n'est jamais annoncé.
            self.flow.set_wanted(self.host.wanted());
            match m {
                Msg::Codec(c) => {
                    self.decoder = None;
                    self.pw = None;
                    self.pw_config = None;
                    self.pw_failed = false;
                    self.pw_announced = None;
                    let steps = self.flow.codec(c);
                    self.apply(steps, None);
                }
                Msg::Config(bytes) => {
                    let steps = self.flow.config(&bytes);
                    self.apply(steps, None);
                }
                Msg::Frame { key, pts, data, received, resume } => {
                    let steps = self.flow.frame(key, &data, resume, Instant::now());
                    self.apply(steps, Some(Media { pts, key, data, received }));
                }
                Msg::Hole => {
                    let steps = self.flow.lost();
                    self.apply(steps, None);
                }
                Msg::Dropped => {
                    let steps = self.flow.dropped();
                    self.apply(steps, None);
                }
                Msg::Surface => {}
                Msg::Stop => break,
            }
        }
        self.decoder = None;
        self.pw = None;
        self.host.surface_gate().released(self.host.surface_gate().generation());
    }

    /// Un message sorti de la file : une image de moins en attente.
    fn took(&self, m: Msg) -> Msg {
        if matches!(m, Msg::Frame { .. }) {
            self.queued.fetch_sub(1, Ordering::AcqRel);
        }
        m
    }

    /// Exécute les étapes de la machine du cœur, dans l'ordre.
    fn apply(&mut self, steps: Vec<Step>, media: Option<Media>) {
        let mut media = media;
        for step in steps {
            match step {
                Step::Open(fourcc) if fourcc == FOURCC_PYROWAVE => {
                    // La session d'affichage s'ouvre à la première image (il lui faut la surface).
                    self.pw = None;
                    self.pw_failed = false;
                }
                Step::Open(_) => self.open_decoder(),
                Step::Configure(c) => self.pw_config = Some(c),
                Step::Decode { head, .. } => {
                    let Some(m) = media.take() else { continue };
                    if self.pyrowave() {
                        self.pyrowave_frame(m);
                    } else {
                        self.feed(&head, m);
                    }
                }
                Step::Skip => {}
                Step::Lost => self.host.keyframe_lost(),
                // Sans surface (arrière-plan), rien à demander : la surface
                // revenue redemandera l'image clé (AND-15).
                Step::Keyframe => {
                    if self.host.window().is_some() {
                        self.host.request_keyframe();
                    }
                }
                Step::Recovered { .. } => self.host.keyframe_recovered(),
            }
        }
    }

    /// Surface refaite ou retirée : décodeur (ou swapchain) lâché ; une image
    /// clé redemandée seulement s'il y a de nouveau où l'afficher.
    fn surface_changed(&mut self) {
        self.decoder = None;
        self.pw = None;
        self.pw_failed = false;
        let steps = self.flow.reset(Instant::now());
        self.apply(steps, None);
    }

    /// Le décodeur MediaCodec du format annoncé. Sans surface (arrière-plan),
    /// il se rouvrira sur l'image clé que la surface revenue redemandera ; un
    /// format que l'appareil ne décode pas est écarté jusqu'au suivant.
    fn open_decoder(&mut self) {
        self.decoder = None;
        if self.host.window().is_none() {
            let steps = self.flow.reset(Instant::now());
            self.apply(steps, None);
            return;
        }
        match self.open() {
            Some(d) => self.decoder = Some(d),
            None => self.flow.unavailable(),
        }
    }

    /// Une image PyroWave : session d'affichage ouverte sur la surface au
    /// besoin, décodeur au format du paquet de configuration (ou, faute de lui,
    /// à la taille de la session en SDR), puis décodage et présentation.
    fn pyrowave_frame(&mut self, m: Media) {
        if self.pw.is_none() {
            if self.pw_failed {
                return;
            }
            let Some(window) = self.host.window() else { return };
            match PwSession::open(window) {
                Ok(s) => self.pw = Some(s),
                Err(e) => {
                    log::error!("PyroWave : affichage impossible ({e})");
                    self.failed();
                    return;
                }
            }
        }
        let config = self.pw_config.unwrap_or_else(|| {
            let (w, h) = self.host.video_size();
            crate::pyrowave::fallback(&m.data, w, h)
        });
        let luminance = if config.hdr { self.host.hdr_luminance() } else { None };
        let Some(pw) = self.pw.as_mut() else { return };
        if !pw.configure(config, luminance) {
            log::error!("PyroWave : décodeur impossible pour {}", config.label());
            self.pw = None;
            self.failed();
            return;
        }
        let Some(pw) = self.pw.as_ref() else { return };
        if self.pw_announced != Some(config) {
            self.pw_announced = Some(config);
            let name = pw.describe();
            log::info!("image : décodeur {name} ({} × {})", config.width, config.height);
            self.host.listener().with(|env, obj| {
                let n = env.new_string(&name)?;
                env.call_method(obj, "onDecoder", "(Ljava/lang/String;)V", &[JValue::Object(&n)]).map(|_| ())
            });
            self.host.decoded_size(config.width, config.height);
            self.host
                .listener()
                .void("onVideoSize", "(II)V", &[JValue::Int(config.width as i32), JValue::Int(config.height as i32)]);
        }
        match pw.frame(&m.data) {
            Shown::Shown => {
                // Rendu GPU terminé et présenté : l'écran la montre au balayage suivant.
                let decoded = latency::now_micros();
                self.host.frame_displayed(m.pts, m.received, decoded, decoded + self.host.frame_period_micros());
            }
            // Jamais montrée : comptée parmi les images perdues.
            Shown::Skipped => {}
            Shown::Lost => {
                log::warn!("PyroWave : surface ou GPU perdu, affichage refait à l'image suivante");
                self.pw = None;
            }
        }
    }

    /// PyroWave ne s'ouvre pas sur cet appareil (swapchain, décodeur) : plus
    /// d'essai avec cette surface, et, une fois pour le flux, le repli de la
    /// session (« Image en HEVC : PyroWave n'a pas pu démarrer sur cet
    /// appareil. », docs/link-ui.md, « PyroWave ») : l'image repart dans le
    /// format qu'aurait pris Auto.
    fn failed(&mut self) {
        self.pw_failed = true;
        if !self.pw_fallback {
            self.pw_fallback = true;
            self.host.pyrowave_failed();
        }
    }

    /// Confie l'image à MediaCodec, `head` (les paramètres du flux, devant une
    /// image clé qui démarre le décodeur) d'abord.
    fn feed(&mut self, head: &[u8], m: Media) {
        let Some(d) = &self.decoder else { return };
        let codec = d.codec.clone();
        match codec.0.dequeue_input_buffer(Duration::from_millis(40)) {
            Ok(DequeuedInputBufferResult::Buffer(mut buf)) => {
                let dst = buf.buffer_mut();
                let n = head.len() + m.data.len();
                if n > dst.len() {
                    log::warn!("image : {n} octets pour un tampon de {}", dst.len());
                    // Le tampon est rendu au codec, vide : sinon il serait perdu pour la session.
                    let _ = codec.0.queue_input_buffer(buf, 0, 0, m.pts, 0);
                    let steps = self.flow.dropped();
                    self.apply(steps, None);
                    return;
                }
                // Copie directe dans le tampon du codec (non initialisé, assez grand : vérifié plus haut).
                unsafe {
                    let p = dst.as_mut_ptr() as *mut u8;
                    std::ptr::copy_nonoverlapping(head.as_ptr(), p, head.len());
                    std::ptr::copy_nonoverlapping(m.data.as_ptr(), p.add(head.len()), m.data.len());
                }
                {
                    let mut r = self.received.lock().unwrap();
                    if r.len() == 64 {
                        r.pop_front();
                    }
                    r.push_back((m.pts, m.received));
                }
                let flags = if m.key { BUFFER_FLAG_KEY_FRAME } else { 0 };
                if let Err(e) = codec.0.queue_input_buffer(buf, 0, n, m.pts, flags) {
                    log::warn!("image : envoi au décodeur refusé ({e:?})");
                    let steps = self.flow.failed(Instant::now());
                    self.apply(steps, None);
                    return;
                }
                self.flow.fed(m.pts);
            }
            Ok(DequeuedInputBufferResult::TryAgainLater) => {
                log::warn!("image : décodeur saturé, image écartée");
                let steps = self.flow.dropped();
                self.apply(steps, None);
            }
            Err(e) => {
                log::warn!("image : décodeur en erreur ({e:?}), refait à la prochaine image clé");
                self.decoder = None;
                let steps = self.flow.reset(Instant::now());
                self.apply(steps, None);
            }
        }
    }

    fn open(&mut self) -> Option<Decoder> {
        let window = self.host.window()?;
        let mime = match self.flow.fourcc() {
            FOURCC_HEVC => "video/hevc",
            FOURCC_AV1 => "video/av01",
            _ => "video/avc",
        };
        let (w, h) = self.host.video_size();
        let hdr = self.host.hdr();
        let luminance = if hdr { self.host.hdr_luminance() } else { None };
        let codec = MediaCodec::from_decoder_type(mime)?;
        let configured = codec
            .configure(&format(mime, w, h, hdr, luminance, true), Some(&window), MediaCodecDirection::Decoder)
            .or_else(|_| codec.configure(&format(mime, w, h, hdr, luminance, false), Some(&window), MediaCodecDirection::Decoder));
        if let Err(e) = configured {
            log::error!("image : décodeur {mime} impossible à configurer ({e:?})");
            return None;
        }
        if let Err(e) = codec.start() {
            log::error!("image : décodeur {mime} impossible à démarrer ({e:?})");
            return None;
        }
        let name = codec.name().unwrap_or_default();
        log::info!("image : décodeur {name} ({mime}, {w} × {h}{})", if hdr { ", HDR10" } else { "" });
        self.host.listener().with(|env, obj| {
            let n = env.new_string(&name)?;
            env.call_method(obj, "onDecoder", "(Ljava/lang/String;)V", &[JValue::Object(&n)]).map(|_| ())
        });
        let codec = Arc::new(Codec(codec));
        let stop = Arc::new(AtomicBool::new(false));
        let (c, st, sh, rec) = (codec.clone(), stop.clone(), self.host.clone(), self.received.clone());
        let output = std::thread::Builder::new().name("kybernow-affichage".into()).spawn(move || drain(c, st, sh, rec)).ok();
        Some(Decoder { codec, stop, output })
    }
}

/// Format du décodeur : faible latence (Android 11), priorité temps réel, et
/// les clés des fabricants connues pour couper la remise en ordre des images
/// (Qualcomm, Samsung Exynos, Amlogic). Refusé avec elles : on réessaie sans.
/// En HDR10 (HEVC Main 10 ou AV1 10 bits) : profil HDR10, BT.2020 et PQ
/// annoncés (le flux les porte aussi dans son VUI ou son en-tête de
/// séquence), pour que la surface soit étiquetée HDR10 et que l'écran passe
/// en HDR ; sur un écran qui ne l'est plus, Android ramène l'image au SDR.
/// `luminance` : les métadonnées HDR10 de secours (`hdr-static-info`), que le
/// décodeur ne prend que si le flux n'a pas les siennes.
pub(crate) fn format(mime: &str, w: u32, h: u32, hdr: bool, luminance: Option<kybernow_core::profile::HdrLuminance>, vendor: bool) -> MediaFormat {
    let mut f = MediaFormat::new();
    f.set_str("mime", mime);
    f.set_i32("width", w as i32);
    f.set_i32("height", h as i32);
    f.set_i32("max-width", w.max(3840) as i32);
    f.set_i32("max-height", h.max(2160) as i32);
    f.set_i32("low-latency", 1);
    f.set_i32("priority", 0);
    if hdr && (mime == "video/hevc" || mime == "video/av01") {
        // MediaCodecInfo.CodecProfileLevel.HEVCProfileMain10HDR10 et AV1ProfileMain10HDR10 (0x1000 tous deux) ;
        // MediaFormat.COLOR_STANDARD_BT2020, COLOR_TRANSFER_ST2084, COLOR_RANGE_LIMITED.
        f.set_i32("profile", 0x1000);
        f.set_i32("color-standard", 6);
        f.set_i32("color-transfer", 6);
        f.set_i32("color-range", 2);
        if let Some(l) = luminance {
            f.set_buffer("hdr-static-info", &crate::hdr::static_info(l));
        }
    }
    if vendor {
        f.set_i32("vendor.qti-ext-dec-picture-order.enable", 1);
        f.set_i32("vendor.qti-ext-dec-low-latency.enable", 1);
        f.set_i32("vendor.rtc-ext-dec-low-latency.enable", 1);
        f.set_i32("vendor.low-latency.enable", 1);
    }
    f
}

/// Fil de sortie : montre la plus récente des images décodées.
fn drain(codec: Arc<Codec>, stop: Arc<AtomicBool>, host: Arc<dyn MediaHost>, received: Received) {
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
                        host.skipped(pts);
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
                        host.decoded_size(w, h);
                        host.listener().void("onVideoSize", "(II)V", &[JValue::Int(w as i32), JValue::Int(h as i32)]);
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
            && let Some(rx) = rx
        {
            // Rendu demandé maintenant : l'écran le montre au balayage suivant.
            host.frame_displayed(pts, rx, decoded, latency::now_micros() + host.frame_period_micros());
        }
    }
}
