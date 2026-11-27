//! Image et son de la session.
//!
//! - Image : les paquets du correctif 0001 (Annex B) vont au décodeur du
//!   téléviseur (NDL), depuis un fil à part (le fil réseau de Kyber ne bloque
//!   jamais), au rythme du flux d'images du cœur (`frames::FrameFlow`) : le
//!   jeu de paramètres (SPS/PPS, VPS, en-tête AV1) placé devant chaque image
//!   clé ; après une perte, rien avant une image clé ou une image de reprise,
//!   redemandée par le cœur ; un format jamais annoncé, ouvert d'office.
//! - HDR : lu dans le jeu de paramètres et l'image clé qui chargent le
//!   décodeur (`crate::hdr`) ; un flux HDR charge NDL avec ses métadonnées
//!   HDR10, et le téléviseur passe en mode HDR ; un flux SDR, jamais.
//! - Son : Opus décodé par le moteur du cœur (tampon adaptatif, masquage,
//!   mixage en stéréo), tiré par le rappel de SDL (PulseAudio du téléviseur) ;
//!   en 5.1, décodé par l'app et donné en PCM 6 voies au plan de son de NDL,
//!   ou encodé en E-AC-3 pour la chaîne Dolby (`esplayer.rs`).

use std::ffi::{c_int, c_void};
use std::sync::mpsc::{channel, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use kybernow_core::audio::{opus, AudioEngine, MonotonicClock, OutputOrder, MAX_FRAMES};
use kybernow_core::frames::{FrameFlow, Step};
use kybernow_core::latency;
use kybernow_core::media::{OpusHead, Packet, PacketHandler, FOURCC_AV1, FOURCC_HEVC};

use super::eac3;
use super::esplayer;
use super::ndl;
use super::sdl;
use super::session::Shared;

enum Msg {
    Codec(u32),
    Config(Vec<u8>),
    /// `resume` : image de reprise après une perte (docs/reprise-apres-perte.md).
    Frame { key: bool, pts: u64, data: Vec<u8>, received: i64, resume: Option<u64> },
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
            Packet::Media { is_key, pts, data, resume, .. } => {
                if let Some(s) = self.shared.session() {
                    s.latency().received_bytes(data.len());
                }
                self.send(Msg::Frame { key: is_key, pts, data: data.to_vec(), received: latency::now_micros(), resume });
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

/// Le fil de l'image : le flux d'images du cœur (`frames::FrameFlow` : paramètres du flux devant l'image clé,
/// attente d'une image clé ou d'une reprise après une perte, format jamais annoncé ouvert d'office, image clé
/// redemandée au plus toutes les 300 ms), exécuté sur le décodeur du téléviseur (NDL).
struct Feeder {
    shared: Arc<Shared>,
    flow: FrameFlow,
    kind: i32,
    /// Chargement de NDL fait par ce fil : s'il change ailleurs (rechargement, repli du son), le décodeur a besoin des
    /// paramètres et d'une image clé.
    generation: Option<u64>,
    /// Le décodeur a été pris par les mires du réglage du HDR : rouvert à la fin des mires.
    reopen: bool,
    failed: bool,
    /// Le chargement en cours est en HDR ; la file de NDL au journal, toutes les 5 s.
    hdr_on: bool,
    queue: crate::journal::Queue,
    /// Ce fil comme propriétaire du décodeur (`ndl::owner`) : il ne décharge que le chargement qu'il tient encore.
    owner: u64,
}

/// Le rythme de `FrameFlow::idle` : en attente d'une image clé sans rien d'avant la perte, elle est redemandée.
const IDLE_EVERY: Duration = Duration::from_millis(100);

impl Feeder {
    fn new(shared: Arc<Shared>) -> Self {
        let flow = FrameFlow::new(shared.wanted_fourcc());
        Self { shared, flow, kind: ndl::VIDEO_H265, generation: None, reopen: false, failed: false, hdr_on: false, queue: crate::journal::Queue::new(Instant::now()), owner: ndl::owner() }
    }

    fn run(mut self, rx: Receiver<Msg>) {
        loop {
            match rx.recv_timeout(IDLE_EVERY) {
                Ok(Msg::Codec(c)) => {
                    let steps = self.flow.codec(c);
                    self.set_kind(c);
                    self.generation = None;
                    self.run_steps(steps, &[], None);
                }
                Ok(Msg::Config(bytes)) => {
                    let steps = self.flow.config(&bytes);
                    self.run_steps(steps, &[], None);
                }
                Ok(Msg::Frame { key, pts, data, received, resume }) => self.frame(key, pts, &data, received, resume),
                Ok(Msg::Hole) => {
                    let steps = self.flow.lost();
                    self.run_steps(steps, &[], None);
                }
                Ok(Msg::Stop) | Err(RecvTimeoutError::Disconnected) => break,
                Err(RecvTimeoutError::Timeout) => {
                    let steps = self.flow.idle(Instant::now());
                    self.run_steps(steps, &[], None);
                }
            }
        }
        // Le décodeur n'est rendu que s'il est encore à ce fil : un fil d'image plus récent (image relancée) l'a peut-être
        // déjà repris (WEBOS-02). La chaîne Dolby d'abord, mise en attente plutôt que déchargée pour de bon : le
        // décodeur de jeu se recharge avant elle, et elle se recharge à la trame suivante si le son continue.
        if ndl::owns(self.owner) {
            esplayer::suspend();
        }
        ndl::unload_if(self.owner);
    }

    /// Le décodeur de NDL de ce format.
    fn set_kind(&mut self, fourcc: u32) {
        self.kind = match fourcc {
            FOURCC_HEVC => ndl::VIDEO_H265,
            FOURCC_AV1 => ndl::VIDEO_AV1,
            _ => ndl::VIDEO_H264,
        };
        self.shared.set_codec(ndl::name(self.kind));
    }

    /// Le HDR de ce flux, lu dans son jeu de paramètres et cette image clé
    /// (`None` : SDR) ; la session dit si elle l'a demandé (`Shared::hdr_expected`).
    fn hdr(&self, head: &[u8], key: &[u8]) -> Option<crate::hdr::HdrMeta> {
        if self.kind == ndl::VIDEO_H264 {
            return None;
        }
        let scan = |b: &[u8]| if self.kind == ndl::VIDEO_AV1 { crate::hdr::scan_av1(b) } else { crate::hdr::scan_hevc(b) };
        let mut seen = scan(head);
        seen.merge(scan(key));
        let expected = self.shared.hdr_expected();
        let hdr = crate::hdr::decide(&seen, self.kind == ndl::VIDEO_H265, expected);
        log::info!(
            "image : flux {} (Main 10 : {}, métadonnées : {}, demandé : {expected})",
            if hdr.is_some() { "HDR" } else { "SDR" },
            seen.main10,
            seen.has_metadata()
        );
        hdr
    }

    /// Le bilan de la file de NDL (images en attente, période de la dalle, durée de l'appel).
    fn log_queue(&mut self, now: Instant) {
        if let Some(line) = self.queue.take(now, self.hdr_on, self.shared.frame_period_micros()) {
            log::info!("{line}");
        }
    }

    /// Le décodeur est à rouvrir (rechargé ailleurs, déchargé par le téléviseur, rendu par les mires) : à l'image clé
    /// suivante, demandée tout de suite.
    fn reset(&mut self, now: Instant) {
        self.generation = None;
        let steps = self.flow.reset(now);
        self.run_steps(steps, &[], None);
    }

    fn frame(&mut self, key: bool, pts: u64, data: &[u8], received: i64, resume: Option<u64>) {
        // Les mires du réglage du HDR tiennent le décodeur : les images de la session attendent, sans redemander
        // d'image clé ; à leur fin, NDL se rouvre à l'image clé suivante.
        if ndl::mires_active() {
            self.reopen = true;
            return;
        }
        let now = Instant::now();
        if std::mem::take(&mut self.reopen) {
            self.reset(now);
        }
        if self.generation.is_some_and(|g| g != ndl::generation()) {
            log::info!("image : décodeur rechargé, image clé redemandée");
            self.reset(now);
        }
        let steps = self.flow.frame(key, data, resume, now);
        self.run_steps(steps, data, Some((pts, received)));
    }

    /// Ce que le flux d'images demande, dans l'ordre ; `data` et `frame` (`pts`, arrivée) : l'image en cours.
    fn run_steps(&mut self, steps: Vec<Step>, data: &[u8], frame: Option<(u64, i64)>) {
        // Le jeu de paramètres qui part devant l'image clé : celui que lit le HDR à l'ouverture.
        let head = steps.iter().find_map(|s| if let Step::Decode { head, .. } = s { Some(head.clone()) } else { None }).unwrap_or_default();
        for step in steps {
            match step {
                Step::Open(fourcc) => {
                    if !self.open(fourcc, &head, data) {
                        return;
                    }
                }
                Step::Decode { head, first } => {
                    if let Some((pts, received)) = frame {
                        self.decode(&head, data, first, pts, received);
                    }
                }
                Step::Skip => {}
                Step::Lost => {
                    if let Some(s) = self.shared.session() {
                        s.keyframe_lost();
                    }
                    self.shared.console_keyframe(true);
                }
                Step::Keyframe => {
                    if let Some(s) = self.shared.session() {
                        s.request_keyframe();
                    }
                    // Une partie sur une console : l'image clé est demandée à la console.
                    self.shared.console_keyframe(true);
                }
                Step::Recovered { .. } => {
                    if let Some(s) = self.shared.session() {
                        s.keyframe_recovered();
                    }
                    self.shared.console_keyframe(false);
                }
                // Pas de PyroWave sur les téléviseurs LG : jamais demandé à l'ordinateur.
                Step::Configure(_) => {}
            }
        }
    }

    /// Ouvre NDL pour ce format, cette taille et ce HDR (`ndl::ensure` le recharge s'ils ont changé) ; faux si
    /// l'image attend ou ne peut pas être montrée.
    fn open(&mut self, fourcc: u32, head: &[u8], key: &[u8]) -> bool {
        self.set_kind(fourcc);
        let (w, h) = self.shared.video_size();
        let hdr = self.hdr(head, key);
        if let Err(e) = ndl::ensure(self.owner, self.kind, w, h, hdr) {
            // Les mires du réglage du HDR tiennent le décodeur, ou NDL refuse ce chargement : l'image attend, et le
            // décodeur se rouvre à l'image clé suivante (un refus est dit une fois).
            if e != ndl::BUSY && !self.failed {
                log::error!("image : {e}");
                self.failed = true;
                self.shared.decoder_failed(&e);
            }
            self.reopen = true;
            return false;
        }
        self.generation = Some(ndl::generation());
        if self.hdr_on != hdr.is_some() {
            // Un bilan est tout SDR ou tout HDR.
            self.log_queue(Instant::now());
            self.hdr_on = hdr.is_some();
        }
        true
    }

    /// Donne l'image à NDL, `head` devant.
    fn decode(&mut self, head: &[u8], data: &[u8], first: bool, pts: u64, received: i64) {
        let buf;
        let bytes: &[u8] = if head.is_empty() {
            data
        } else {
            buf = [head, data].concat();
            &buf
        };
        let call = Instant::now();
        match ndl::play(bytes) {
            Ok(queued) => {
                self.flow.fed(pts);
                let now = Instant::now();
                self.queue.add(queued, now - call);
                if self.queue.due(now) {
                    self.log_queue(now);
                }
                if first {
                    // Une partie sur une console : la première image montrée finit la connexion.
                    self.shared.console_frame_shown();
                }
                // NDL ne dit pas quand l'image est montrée : estimée d'après les
                // images en attente et la fréquence de la dalle.
                let decoded = latency::now_micros();
                let period = self.shared.frame_period_micros();
                let at = decoded + period * (1 + queued.unwrap_or(0).max(0) as i64);
                self.shared.frame_times(decoded - received, at - decoded);
                if let Some(s) = self.shared.session() {
                    s.frame_displayed(pts, received, decoded, at);
                }
            }
            Err(e) if e == ndl::NOT_READY => {
                // Le décodeur attend sa confirmation : une image clé, tout de suite.
                let steps = self.flow.failed(Instant::now());
                self.run_steps(steps, &[], None);
            }
            Err(e) if e == ndl::BUSY => {
                // Les mires tiennent le décodeur : à leur fin, il est déchargé ; rouvert à l'image clé suivante.
                self.reopen = true;
            }
            Err(e) if e == ndl::UNLOADED_BY_TV || e == ndl::NOT_LOADED => {
                // Rouvert à l'image clé suivante, avec les paramètres.
                log::warn!("image : {e} ; rechargement à l'image clé suivante");
                self.reset(Instant::now());
            }
            Err(e) => {
                // Rien d'avant ne suffit plus : une perte, comme un trou du flux.
                log::warn!("image : {e}");
                let steps = self.flow.dropped();
                self.run_steps(steps, &[], None);
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
    /// Pass-Through), si l'ampli l'annonce (le Denon AVC-X3700H annonce
    /// LPCM 2 voies : stéréo). Le décodage par la TV elle-même (plan Opus
    /// 6 voies, essayé le 23/10/2026) sort en stéréo : sa sortie passe par le mixeur.
    NdlPcm,
    /// Opus 5.1 décodé par l'app, encodé en E-AC-3 (32 ms par trame) et donné
    /// à la chaîne audio Dolby (`esplayer.rs`) : le téléviseur le passe tel
    /// quel à l'ampli. L'image reste sur la chaîne de jeu.
    NdlDolby,
    /// Moteur du cœur, sortie SDL en stéréo.
    Sdl,
}

/// Ordre des voies du PCM 6 voies de NDL (G, D, C, LFE, arrière G, arrière
/// D : celui de WAVE et de l'HDMI), depuis celui d'Opus (G, C, D, arrière G,
/// arrière D, LFE).
const PCM_ORDER: [usize; 6] = [0, 2, 1, 5, 3, 4];

/// Le son de la session : 5.1 (PCM au plan de NDL, ou E-AC-3 à la chaîne
/// Dolby) quand il est demandé et que le flux s'y prête, sinon moteur du cœur
/// et sortie SDL en stéréo.
pub struct AudioSink {
    engine: Arc<AudioEngine>,
    device: Mutex<u32>,
    /// Le moteur, prêté au rappel de SDL.
    holder: Mutex<Option<Box<Arc<AudioEngine>>>>,
    surround: bool,
    route: Mutex<Route>,
    /// L'en-tête Opus, pour repasser par SDL si NDL ou la chaîne Dolby refusent le son.
    head: Mutex<Vec<u8>>,
    /// Décodeur 5.1 des routes NDL et Dolby, et ses tampons.
    decoder: Mutex<Option<(opus::Decoder, Vec<f32>, Vec<u8>)>>,
    /// Encodeur E-AC-3 de la route Dolby, son PCM remis dans l'ordre et ses trames (tampons réutilisés, WEBOS-22).
    encoder: Mutex<Option<eac3::Encoder>>,
    ordered: Mutex<Vec<f32>>,
    frames: Mutex<eac3::Frames>,
    /// Son 5.1 demandé : PCM (`false`) ou Dolby (`true`).
    dolby: bool,
    /// Paquets donnés à NDL ou à la chaîne Dolby (journal).
    ndl_packets: std::sync::atomic::AtomicU64,
    /// Le chargement de la chaîne Dolby de ce puits (`esplayer::load`, 0 : aucun) : à sa fin, il ne décharge que le
    /// sien (WEBOS-02).
    dolby_token: std::sync::atomic::AtomicU64,
}

impl AudioSink {
    /// `surround` : le son 5.1 est demandé à l'ordinateur et décodé par l'app ;
    /// `dolby` : encodé en E-AC-3 pour la chaîne Dolby plutôt que donné en PCM à NDL.
    pub fn new(surround: bool, dolby: bool) -> Arc<Self> {
        let engine = Arc::new(AudioEngine::with_order(Arc::new(MonotonicClock::new()), OutputOrder::Windows));
        Arc::new(Self {
            engine,
            device: Mutex::new(0),
            holder: Mutex::new(None),
            surround,
            route: Mutex::new(Route::Pending),
            head: Mutex::new(Vec::new()),
            decoder: Mutex::new(None),
            encoder: Mutex::new(None),
            ordered: Mutex::new(Vec::new()),
            frames: Mutex::new(eac3::Frames::default()),
            dolby,
            ndl_packets: std::sync::atomic::AtomicU64::new(0),
            dolby_token: std::sync::atomic::AtomicU64::new(0),
        })
    }

    /// Les trames E-AC-3 de ce PCM 5.1 (ordre G, D, C, LFE, arrière G, arrière D) à la chaîne Dolby.
    fn feed_dolby(&self, pcm: &[f32]) {
        let mut frames = self.frames.lock().unwrap();
        {
            let mut g = self.encoder.lock().unwrap();
            let Some(enc) = g.as_mut() else { return };
            enc.push(pcm, &mut frames);
        }
        for f in frames.iter() {
            match esplayer::feed(f) {
                Ok(()) => {
                    if self.ndl_packets.fetch_add(1, std::sync::atomic::Ordering::Relaxed) == 0 {
                        log::info!("son : première trame E-AC-3 acceptée par la chaîne Dolby");
                    }
                }
                Err(e) if e == "tampon plein" => log::debug!("son : chaîne Dolby, trame sautée (tampon plein)"),
                // Chargement en cours (en fond, WEBOS-05), ou déchargée par le téléviseur (avec le décodeur d'image) :
                // elle se recharge en fond, après lui ; les trames d'ici là sont sautées.
                Err(e) if e == esplayer::RELOADING_ERR => log::debug!("son : chaîne Dolby, trame sautée (rechargement)"),
                Err(e) => {
                    log::warn!("son : chaîne Dolby : {e} ; stéréo par SDL pour la suite de la session");
                    esplayer::unload_if(self.dolby_token.swap(0, std::sync::atomic::Ordering::AcqRel));
                    self.to_sdl();
                    return;
                }
            }
        }
    }

    /// Décode un paquet (ou masque `conceal` images) en PCM 6 voies dans
    /// l'ordre de NDL, et le donne à NDL (S16LE) ou à l'encodeur E-AC-3.
    fn feed_ndl(&self, data: Option<&[u8]>, conceal: usize) {
        let mut g = self.decoder.lock().unwrap();
        let Some((dec, pcm, bytes)) = g.as_mut() else { return };
        let frames = dec.decode(data, if data.is_some() { MAX_FRAMES } else { conceal.clamp(120, MAX_FRAMES) / 120 * 120 }, pcm);
        if frames == 0 {
            return;
        }
        if self.dolby {
            // Même ordre de voies que le PCM de NDL, en flottant pour l'encodeur.
            let mut ordered = self.ordered.lock().unwrap();
            ordered.clear();
            for f in 0..frames {
                let src = &pcm[f * 6..f * 6 + 6];
                ordered.extend(PCM_ORDER.iter().map(|&i| src[i]));
            }
            drop(g);
            self.feed_dolby(&ordered);
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
                if sent && self.ndl_packets.fetch_add(1, std::sync::atomic::Ordering::Relaxed) == 0 {
                    log::info!("son : premier paquet PCM 5.1 accepté par NDL ; sortie multicanal du téléviseur : {:?}", ndl::multichannel());
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
            Some(d) if self.dolby => {
                let Some(mut enc) = eac3::Encoder::new(6) else {
                    log::warn!("son : encodeur E-AC-3 indisponible ; stéréo par SDL");
                    self.to_sdl();
                    return;
                };
                let Some(silence) = enc.silence() else {
                    log::warn!("son : l'encodeur E-AC-3 ne sort rien ; stéréo par SDL");
                    self.to_sdl();
                    return;
                };
                // Chargée en fond (jusqu'à ~5 s : NDL d'abord, puis l'amorce), pas dans le rappel des paquets
                // (WEBOS-05) : les trames d'ici là sont sautées ; un échec repasse par SDL à la trame suivante.
                let token = esplayer::load(&silence);
                self.dolby_token.store(token, std::sync::atomic::Ordering::Release);
                *self.encoder.lock().unwrap() = Some(enc);
                *self.decoder.lock().unwrap() = Some((d, vec![0.0; MAX_FRAMES * 6], Vec::new()));
                *self.route.lock().unwrap() = Route::NdlDolby;
                log::info!("son : 5.1 décodé par l'app, encodé en E-AC-3 ({} kbit/s) pour le téléviseur (NDL)", eac3::BITRATE / 1000);
            }
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
        self.encoder.lock().unwrap().take();
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

/// « Couper le son » d'une partie sur une console : la sortie SDL rend du silence (le son continue d'être décodé,
/// pour revenir aussitôt).
static MUTED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

pub fn set_muted(on: bool) {
    MUTED.store(on, std::sync::atomic::Ordering::Relaxed);
}

extern "C" fn fill(user: *mut c_void, stream: *mut u8, len: c_int) {
    let n = len.max(0) as usize / 4;
    let out = unsafe { std::slice::from_raw_parts_mut(stream as *mut f32, n) };
    // Une panique du moteur, rattrapée à la frontière C : du silence plutôt que ce que le tampon de SDL contenait.
    let filled = kybernow_core::ffi_guard!(false => {
        let engine = unsafe { &*(user as *const Arc<AudioEngine>) };
        engine.fill(out, n / 2);
        true
    });
    if !filled || MUTED.load(std::sync::atomic::Ordering::Relaxed) {
        out.fill(0.0);
    }
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
                    Route::NdlPcm | Route::NdlDolby => self.feed_ndl(Some(data), 0),
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
                Route::NdlPcm | Route::NdlDolby => self.feed_ndl(None, missing_samples as usize),
                Route::Pending => {}
            },
        }
    }
}

impl Drop for AudioSink {
    fn drop(&mut self) {
        let token = self.dolby_token.swap(0, std::sync::atomic::Ordering::AcqRel);
        if token != 0 {
            esplayer::unload_if(token);
        }
        self.engine.stop();
        let id = *self.device.lock().unwrap();
        if id != 0 {
            unsafe { sdl::SDL_CloseAudioDevice(id) };
        }
        self.holder.lock().unwrap().take();
    }
}
