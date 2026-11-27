//! Image de la session : les paquets du correctif 0001 (Annex B) vont, depuis
//! un fil à part (le fil réseau de Kyber ne bloque jamais), à FFmpeg
//! (`src/native/decoder.c`) : le décodeur matériel (VA-API ; sur le Pi, V4L2)
//! quand la machine sait le format, sinon le
//! processeur. Le jeu de paramètres (SPS/PPS, VPS, en-tête AV1) est placé
//! devant chaque image clé ; après une perte, rien avant l'image clé suivante,
//! redemandée par le cœur. Seule l'image la plus récente est gardée pour le
//! fil principal, qui la montre (`take`) : le retard ne s'accumule pas.
//!
//! HDR (docs/hdr.md) : une image 10 bits (P010 par VA-API, yuv420p10le par le
//! processeur) garde ses 10 bits et ce que le flux dit de sa couleur ; la
//! fenêtre la donne à sa sous-surface HDR (`hdr_wl.rs`). Sans elle, `Screen`
//! la ramène en 8 bits pour SDL (couleurs approchées : un repli).
//!
//! Ce qu'il faut faire de chaque paquet (ouvrir le décodeur, placer les
//! paramètres, attendre l'image clé ou la reprise, dire une perte à la session,
//! ouvrir d'office le format demandé quand Kyber ne l'annonce pas) est la
//! machine du cœur, `frames::FrameFlow` : ce fil exécute ses étapes.
//!
//! PyroWave (docs/pyrowave-client-linux.md, format `pwv1`) : un paquet de
//! configuration `KNPW` (version 1, 16 octets ; version 2, 48 octets avec les
//! métadonnées HDR10 de la capture, demandée par l'app : docs/kyber-changes.md),
//! lu par `pyrowave::Config` du cœur, puis chaque paquet média est une image
//! entière, toujours clé ; `src/native/pyrowave.c` la décode par Vulkan et relit
//! ses plans (I420, I010 en HDR10, BGRA en 4:4:4) dans le tampon de l'image.
//! Une image incomplète ou illisible est sautée : jamais d'image clé demandée.
//! Un décodeur PyroWave qui ne s'ouvre pas sur cet appareil : le cœur relance
//! l'image dans un autre format pour la session (`Session::pyrowave_failed`).

use std::ffi::{c_char, c_int, c_void, CStr};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::mpsc::{channel, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use kybernow_core::frames::{FrameFlow, Step};
use kybernow_core::latency;
use kybernow_core::media::{Packet, PacketHandler, FOURCC_AV1, FOURCC_HEVC, FOURCC_PYROWAVE};
use kybernow_core::pyrowave::Config;

use super::sdl;
use super::session::Shared;
use crate::video_rules::{self, Admit, Backlog, Yuv};

#[repr(C)]
struct KnFrame {
    width: c_int,
    height: c_int,
    format: c_int,
    data: [*mut u8; 3],
    linesize: [c_int; 3],
    pts: i64,
    bt709: c_int,
    full_range: c_int,
    pq: c_int,
    bt2020: c_int,
    mastering_min: f32,
    mastering_max: f32,
    max_cll: f32,
    max_fall: f32,
}

unsafe extern "C" {
    fn kn_log_quiet();
    fn kn_decoder_open(codec: c_int, hw: c_int, threads: c_int, err: *mut c_char, errlen: c_int) -> *mut c_void;
    fn kn_decoder_name(d: *mut c_void) -> *const c_char;
    fn kn_decoder_is_hw(d: *mut c_void) -> c_int;
    fn kn_decoder_frames(d: *mut c_void) -> i64;
    fn kn_decoder_send(d: *mut c_void, data: *const u8, len: c_int, pts: i64) -> c_int;
    fn kn_decoder_send_parts(d: *mut c_void, head: *const u8, head_len: c_int, data: *const u8, len: c_int, pts: i64) -> c_int;
    fn kn_decoder_receive(d: *mut c_void, out: *mut KnFrame) -> c_int;
    fn kn_decoder_close(d: *mut c_void);
    fn kn_pw_open(width: c_int, height: c_int, c444: c_int, hdr: c_int, err: *mut c_char, errlen: c_int) -> *mut c_void;
    fn kn_pw_decode(d: *mut c_void, data: *const u8, len: usize, out: *const *mut u8, stride: *const c_int, bt709: c_int, full: c_int) -> c_int;
    fn kn_pw_close(d: *mut c_void);
}

/// Le format de l'image PyroWave relue : I420 (SDR 4:2:0), I010 (HDR10), BGRA (4:4:4).
fn pw_format(c: &Config) -> Format {
    match (c.chroma444, c.hdr) {
        (true, _) => Format::Bgra,
        (false, true) => Format::I010,
        (false, false) => Format::I420,
    }
}

/// Événement SDL qui réveille la boucle principale : une image est prête.
pub static FRAME_EVENT: AtomicU32 = AtomicU32::new(0);

pub(crate) fn wake_main() {
    let kind = FRAME_EVENT.load(Ordering::Relaxed);
    if kind == 0 {
        return;
    }
    let mut ev = sdl::SDL_Event::default();
    ev.raw[0..4].copy_from_slice(&kind.to_ne_bytes());
    unsafe { sdl::SDL_PushEvent(&mut ev) };
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Format {
    Nv12,
    I420,
    /// 4:4:4 converti en RVB par `decoder.c` (octets B, G, R, A).
    Bgra,
    /// 10 bits, mots de 16 bits petit-boutistes, lignes serrées (largeur × 2
    /// octets) : P010 (luma, chroma entrelacée ; valeurs dans les bits de poids
    /// fort) et I010 (trois plans ; bits de poids faible).
    P010,
    I010,
}

impl Frame {
    /// BT.709 en plage pleine (PyroWave SDR) : SDL 2 ne sait pas le montrer, l'image passe en plage réduite
    /// ici, sur le fil du décodeur (`video_rules`). Une image BGRA (4:4:4 converti) ou 10 bits n'est pas touchée.
    fn narrow_if_needed(&mut self) {
        if !video_rules::needs_narrowing(self.bt709, self.full_range) || !matches!(self.format, Format::Nv12 | Format::I420) {
            return;
        }
        video_rules::narrow_luma(&mut self.planes[0]);
        video_rules::narrow_chroma(&mut self.planes[1]);
        video_rules::narrow_chroma(&mut self.planes[2]);
        self.full_range = false;
    }
}

impl Format {
    pub fn ten_bit(self) -> bool {
        matches!(self, Format::P010 | Format::I010)
    }
}

/// Une image décodée, prête à montrer.
pub struct Frame {
    pub width: u32,
    pub height: u32,
    pub format: Format,
    pub planes: [Vec<u8>; 3],
    pub pitch: [usize; 3],
    pub pts: u64,
    pub received: i64,
    pub decoded: i64,
    pub bt709: bool,
    pub full_range: bool,
    /// Courbe PQ (HDR10) ; métadonnées HDR10 de l'image (cd/m², 0 : absentes),
    /// gardées d'une image clé à la suivante.
    pub pq: bool,
    pub mastering_min: f32,
    pub mastering_max: f32,
    pub max_cll: f32,
    pub max_fall: f32,
}

/// L'image la plus récente, et les tampons à réutiliser.
#[derive(Default)]
pub struct Latest {
    slot: Mutex<Option<Frame>>,
    spare: Mutex<Vec<Frame>>,
    /// Images remplacées avant d'être montrées (écartées).
    pub dropped: AtomicU32,
    /// L'image réduite de l'accueil (dernière image de la session), refaite toutes les 2 s sur le fil du
    /// décodeur, pas sur celui de l'interface (LINAPP-33) ; quand elle a été faite.
    snapshot: Mutex<(Option<Instant>, Option<tiny_skia::Pixmap>)>,
}

impl Latest {
    pub fn take(&self) -> Option<Frame> {
        self.slot.lock().unwrap().take()
    }

    pub fn give_back(&self, f: Frame) {
        let mut s = self.spare.lock().unwrap();
        if s.len() < 3 {
            s.push(f);
        }
    }

    /// L'image réduite la plus récente, si elle a changé depuis la dernière prise.
    pub fn take_snapshot(&self) -> Option<tiny_skia::Pixmap> {
        self.snapshot.lock().unwrap().1.take()
    }

    fn put(&self, f: Frame) {
        let due = self.snapshot.lock().unwrap().0.is_none_or(|t| t.elapsed() > Duration::from_secs(2));
        if due {
            let img = snapshot(&f, 640);
            *self.snapshot.lock().unwrap() = (Some(Instant::now()), img);
        }
        if let Some(old) = self.slot.lock().unwrap().replace(f) {
            self.dropped.fetch_add(1, Ordering::Relaxed);
            self.give_back(old);
        }
    }

    fn buffer(&self) -> Option<Frame> {
        self.spare.lock().unwrap().pop()
    }
}

enum Msg {
    Codec(u32),
    Config(Vec<u8>),
    /// `resume` : image de reprise après une perte (docs/reprise-apres-perte.md).
    Frame { key: bool, pts: u64, data: Vec<u8>, received: i64, resume: Option<u64> },
    Hole,
    /// Des paquets jetés à la réception (file pleine, [`Backlog`]) : comme un trou, sans reprise possible.
    Dropped,
    Stop,
}

/// `kn_decoder_send` : le décodeur n'a pas pris le paquet (tampons pleins), à renvoyer.
const SEND_AGAIN: c_int = 1;

/// Un flux d'image (un par démarrage de l'image, et par écran de l'appareil
/// en multi-écrans : un décodeur et une file d'images chacun).
pub struct VideoSink {
    tx: Mutex<Sender<Msg>>,
    thread: Mutex<Option<JoinHandle<()>>>,
    shared: Arc<Shared>,
    /// Les paquets en attente du fil du décodeur, bornés.
    backlog: Arc<Backlog>,
}

impl VideoSink {
    pub fn new(shared: Arc<Shared>, screen: usize) -> Arc<Self> {
        let (tx, rx) = channel();
        let s = shared.clone();
        let latest = shared.fresh_latest(screen);
        let backlog = Arc::new(Backlog::default());
        let b = backlog.clone();
        let thread = std::thread::Builder::new().name("kybernow-image".into()).spawn(move || Decoder::new(s, latest, b).run(rx)).ok();
        Arc::new(Self { tx: Mutex::new(tx), thread: Mutex::new(thread), shared, backlog })
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
                match self.backlog.admit(is_key) {
                    Admit::Send => self.send(Msg::Frame { key: is_key, pts, data: data.to_vec(), received: latency::now_micros(), resume }),
                    Admit::Drop => {}
                    Admit::DropFirst => {
                        log::warn!("image : le décodage ne suit plus ({} paquets en attente) ; paquets jetés jusqu'à l'image clé", self.backlog.pending());
                        self.send(Msg::Dropped);
                        match self.shared.session() {
                            Some(s) => s.request_keyframe(),
                            None => self.shared.keyframe(true),
                        }
                    }
                }
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

struct Decoder {
    shared: Arc<Shared>,
    /// L'image la plus récente de cet écran, pour le fil principal.
    latest: Arc<Latest>,
    /// Les paquets en attente (décomptés ici, à chaque image prise).
    backlog: Arc<Backlog>,
    /// Ce qu'il faut faire de chaque paquet : la machine du cœur.
    flow: FrameFlow,
    /// Le nom du décodeur relu après la première image (statistiques).
    named: bool,
    /// 0 H.264, 1 HEVC, 2 AV1.
    codec: c_int,
    dec: *mut c_void,
    /// Décodeur matériel refusé ou en échec : le processeur décode.
    software: bool,
    errors: u32,
    /// L'échec d'ouverture a été dit (une fois par format).
    failed: bool,
    /// Les dernières métadonnées HDR10 vues (FFmpeg ne les met qu'à certaines images) : minimum et maximum du
    /// mastering display, MaxCLL, MaxFALL.
    meta: [f32; 4],
    /// PyroWave : la configuration lue, le décodeur (`pyrowave.c`) et celle de son ouverture, les métadonnées
    /// HDR10 de l'en-tête version 2.
    pw_config: Option<Config>,
    pw: *mut c_void,
    pw_open: Option<Config>,
    pw_meta: [f32; 4],
    /// Images sautées (incomplètes, illisibles), pour le journal.
    pw_skipped: u32,
}

/// Une image reçue, telle que le décodeur la prend.
struct Media<'a> {
    pts: u64,
    data: &'a [u8],
    received: i64,
}

/// Sans paquet, la machine du cœur est réveillée à ce rythme (image clé redemandée pendant une attente).
const IDLE: Duration = Duration::from_millis(100);

impl Decoder {
    fn new(shared: Arc<Shared>, latest: Arc<Latest>, backlog: Arc<Backlog>) -> Self {
        unsafe { kn_log_quiet() };
        let flow = FrameFlow::new(shared.wanted());
        Self {
            shared,
            latest,
            backlog,
            flow,
            named: false,
            codec: 1,
            dec: std::ptr::null_mut(),
            software: false,
            errors: 0,
            failed: false,
            meta: [0.0; 4],
            pw_config: None,
            pw: std::ptr::null_mut(),
            pw_open: None,
            pw_meta: [0.0; 4],
            pw_skipped: 0,
        }
    }

    fn run(mut self, rx: Receiver<Msg>) {
        loop {
            let m = match rx.recv_timeout(IDLE) {
                Ok(m) => m,
                Err(RecvTimeoutError::Timeout) => {
                    let steps = self.flow.idle(Instant::now());
                    self.apply(steps, None);
                    continue;
                }
                Err(RecvTimeoutError::Disconnected) => break,
            };
            // Le format que la session demande (profil changé, repli de PyroWave) : ouvert d'office s'il n'est
            // jamais annoncé.
            self.flow.set_wanted(self.shared.wanted());
            let steps = match &m {
                Msg::Codec(c) => {
                    self.announced(*c);
                    self.flow.codec(*c)
                }
                Msg::Config(bytes) => self.flow.config(bytes),
                Msg::Frame { key, data, resume, .. } => {
                    self.backlog.taken();
                    self.flow.frame(*key, data, *resume, Instant::now())
                }
                Msg::Hole => self.flow.lost(),
                Msg::Dropped => self.flow.dropped(),
                Msg::Stop => break,
            };
            let media = match &m {
                Msg::Frame { pts, data, received, .. } => Some(Media { pts: *pts, data, received: *received }),
                _ => None,
            };
            self.apply(steps, media.as_ref());
        }
        self.close();
    }

    /// Kyber annonce un format : il repart de zéro (le repli sur le processeur et l'échec d'ouverture d'avant
    /// valaient pour l'ancien ; un repli en H.264 ne tient pas le HEVC au processeur).
    fn announced(&mut self, fourcc: u32) {
        self.close();
        self.software = false;
        self.failed = false;
        self.meta = [0.0; 4];
        self.pw_config = None;
        self.pw_meta = [0.0; 4];
        if fourcc == FOURCC_PYROWAVE {
            log::info!("image : PyroWave");
        }
    }

    /// Exécute les étapes de la machine du cœur, dans l'ordre.
    fn apply(&mut self, steps: Vec<Step>, media: Option<&Media>) {
        for step in steps {
            match step {
                Step::Open(fourcc) => self.open_for(fourcc),
                Step::Configure(c) => {
                    if self.pw_config != Some(c) {
                        log::info!("image : PyroWave {}", c.label());
                    }
                    // Les métadonnées HDR10 de la capture (en-tête `KNPW` version 2, lues par le cœur).
                    self.pw_meta = pw_meta(&c.hdr10);
                    self.pw_config = Some(c);
                }
                Step::Decode { head, first } => {
                    let Some(m) = media else { continue };
                    if self.flow.fourcc() == FOURCC_PYROWAVE {
                        self.pw_frame(m);
                    } else {
                        self.decode(&head, first, m);
                    }
                }
                Step::Skip => {}
                // Une perte : réparation, puis image clé (une fois par perte) ; l'image revenue.
                Step::Lost => self.shared.keyframe(true),
                Step::Recovered { .. } => self.shared.keyframe(false),
                Step::Keyframe => self.request_keyframe(),
            }
        }
    }

    fn close(&mut self) {
        if !self.dec.is_null() {
            unsafe { kn_decoder_close(self.dec) };
            self.dec = std::ptr::null_mut();
        }
        if !self.pw.is_null() {
            unsafe { kn_pw_close(self.pw) };
            self.pw = std::ptr::null_mut();
        }
        self.pw_open = None;
    }

    /// Un décodeur pour ce format. PyroWave s'ouvre à sa première image (il lui faut la définition).
    fn open_for(&mut self, fourcc: u32) {
        self.close();
        if fourcc == FOURCC_PYROWAVE {
            self.shared.set_codec("PyroWave");
            return;
        }
        self.codec = match fourcc {
            FOURCC_HEVC => 1,
            FOURCC_AV1 => 2,
            _ => 0,
        };
        if let Err(e) = self.open() {
            // Ce format ne s'ouvre pas ici : les images sont écartées jusqu'au prochain (le cœur relance l'image
            // quand rien ne s'affiche).
            self.flow.unavailable();
            if !self.failed {
                log::error!("image : {e}");
                self.failed = true;
                self.shared.decoder_failed(&e);
            }
        }
    }

    /// Une image PyroWave entière : décodée dans un tampon de l'image, ou sautée.
    fn pw_frame(&mut self, m: &Media) {
        if self.pw_config.is_none()
            && let Some(c) = Config::from_sequence_header(m.data)
        {
            log::warn!("image : pas de paquet de configuration PyroWave, définition lue dans le flux ({} × {})", c.width, c.height);
            self.pw_config = Some(c);
        }
        let Some(cfg) = self.pw_config else { return };
        if self.pw.is_null() || self.pw_open != Some(cfg) {
            self.close();
            let mut err = [0 as c_char; 256];
            let d = unsafe { kn_pw_open(cfg.width as c_int, cfg.height as c_int, cfg.chroma444 as c_int, cfg.hdr as c_int, err.as_mut_ptr(), err.len() as c_int) };
            if d.is_null() {
                let e = unsafe { CStr::from_ptr(err.as_ptr()) }.to_string_lossy().into_owned();
                // PyroWave ne s'ouvre pas sur cet appareil : le cœur relance l'image dans le format qu'aurait pris
                // Auto, pour la session (avis `Platform::on_pyrowave_fallback`).
                log::error!("image : PyroWave : {e}");
                self.flow.unavailable();
                self.shared.pyrowave_failed(&e);
                return;
            }
            self.pw = d;
            self.pw_open = Some(cfg);
        }
        let format = pw_format(&cfg);
        let (w, h) = (cfg.width as usize, cfg.height as usize);
        let mut out = self.latest.buffer().unwrap_or_else(|| empty_frame(format));
        // Lignes serrées : I420 et I010 (largeur, puis moitié), BGRA (largeur × 4).
        let (pitch, rows): ([usize; 3], [usize; 3]) = match format {
            Format::Bgra => ([w * 4, 0, 0], [h, 0, 0]),
            Format::I010 => ([w * 2, w, w], [h, h / 2, h / 2]),
            _ => ([w, w / 2, w / 2], [h, h / 2, h / 2]),
        };
        for p in 0..3 {
            out.planes[p].resize(pitch[p] * rows[p], 0);
            out.pitch[p] = pitch[p];
        }
        let ptrs = [out.planes[0].as_mut_ptr(), out.planes[1].as_mut_ptr(), out.planes[2].as_mut_ptr()];
        let strides = pitch.map(|p| p as c_int);
        let r = unsafe { kn_pw_decode(self.pw, m.data.as_ptr(), m.data.len(), ptrs.as_ptr(), strides.as_ptr(), 1, cfg.full_range as c_int) };
        if r != 1 {
            self.pw_skipped += 1;
            if self.pw_skipped <= 3 || self.pw_skipped % 100 == 0 {
                log::warn!("image : PyroWave, image {} sautée ({}, {} en tout)", m.pts, if r == 0 { "incomplète" } else { "illisible" }, self.pw_skipped);
            }
            self.latest.give_back(out);
            return;
        }
        out.width = cfg.width;
        out.height = cfg.height;
        out.format = format;
        out.pts = m.pts;
        out.received = m.received;
        out.decoded = latency::now_micros();
        out.bt709 = !cfg.hdr;
        out.full_range = cfg.full_range;
        out.pq = cfg.hdr;
        out.narrow_if_needed();
        // HDR10 : les métadonnées de l'en-tête version 2 (la capture de la Station) ; rien en version 1.
        [out.mastering_min, out.mastering_max, out.max_cll, out.max_fall] = if cfg.hdr { self.pw_meta } else { [0.0; 4] };
        self.latest.put(out);
        wake_main();
    }

    fn open(&mut self) -> Result<(), String> {
        // Le processeur se partage entre les flux (multi-écrans : un décodeur par écran).
        let streams = self.shared.screens().streams.len().max(1);
        let threads = (std::thread::available_parallelism().map(|n| n.get().min(8)).unwrap_or(4) / streams).max(if streams > 1 { 2 } else { 1 }) as c_int;
        let mut err = [0 as c_char; 256];
        // 0 : processeur ; 1 : VA-API ; 2 : V4L2 (décodeurs du Raspberry Pi).
        let hw = if !self.software && self.shared.hw_for(self.codec) { self.shared.hw_backend } else { 0 };
        let d = unsafe { kn_decoder_open(self.codec, hw, threads, err.as_mut_ptr(), err.len() as c_int) };
        if d.is_null() {
            let e = unsafe { CStr::from_ptr(err.as_ptr()) }.to_string_lossy().into_owned();
            if hw != 0 && self.codec != 2 {
                log::warn!("image : décodeur matériel refusé ({e}) ; le processeur décode");
                self.software = true;
                return self.open();
            }
            return Err(e);
        }
        self.dec = d;
        // Ouvert : un échec suivant se signale à nouveau ; le nom sera relu après la première image.
        self.failed = false;
        self.named = false;
        let name = unsafe { CStr::from_ptr(kn_decoder_name(d)) }.to_string_lossy().into_owned();
        log::info!("image : décodeur {name}");
        self.shared.set_codec(&name);
        Ok(())
    }

    /// Une image clé, tout de suite (la machine du cœur en règle le rythme) : à la session, ou à la partie sur une
    /// console, qui la redemande jusqu'à ce qu'elle arrive.
    fn request_keyframe(&self) {
        match self.shared.session() {
            Some(s) => s.request_keyframe(),
            None => self.shared.keyframe(true),
        }
    }

    /// Une image confiée à FFmpeg, `head` (les paramètres du flux, devant une image clé) d'abord.
    fn decode(&mut self, head: &[u8], first: bool, m: &Media) {
        if self.dec.is_null() {
            return;
        }
        // Les paramètres devant une image clé : copiés avec elle dans le paquet de FFmpeg, sans concaténation.
        // (Quelques dizaines d'octets : copiés à part, l'image ne l'est plus.)
        let send = |dec| unsafe { kn_decoder_send_parts(dec, head.as_ptr(), head.len() as c_int, m.data.as_ptr(), m.data.len() as c_int, m.pts as i64) };
        let mut r = send(self.dec);
        if r == SEND_AGAIN {
            // Tampons d'entrée pleins : les images prêtes sortent d'abord (la plus récente est montrée), puis
            // le même paquet repart.
            self.receive(false, m.pts, m.received);
            if self.dec.is_null() {
                return;
            }
            r = send(self.dec);
        }
        if r == SEND_AGAIN {
            // Toujours refusé : le paquet est perdu. Il ne compte pas comme décodé, et la suite attend une image clé.
            log::warn!("image : paquet {} refusé par le décodeur (tampons pleins)", m.pts);
            let steps = self.flow.failed(Instant::now());
            self.apply(steps, None);
            return;
        }
        if r < 0 {
            self.decode_error(r);
            return;
        }
        self.flow.fed(m.pts);
        self.receive(first, m.pts, m.received);
    }

    /// Les images prêtes du décodeur : la plus récente part à l'écran.
    fn receive(&mut self, first: bool, pts: u64, received: i64) {
        let mut f = KnFrame {
            width: 0,
            height: 0,
            format: 0,
            data: [std::ptr::null_mut(); 3],
            linesize: [0; 3],
            pts: 0,
            bt709: 1,
            full_range: 0,
            pq: 0,
            bt2020: 0,
            mastering_min: 0.0,
            mastering_max: 0.0,
            max_cll: 0.0,
            max_fall: 0.0,
        };
        let got = unsafe { kn_decoder_receive(self.dec, &mut f) };
        if got < 0 {
            self.decode_error(got);
            return;
        }
        if first && got > 0 && !self.named {
            // Le nom vu après la première image (le repli de FFmpeg sur le processeur ne se sait qu'alors).
            self.named = true;
            let name = unsafe { CStr::from_ptr(kn_decoder_name(self.dec)) }.to_string_lossy().into_owned();
            self.shared.set_codec(&name);
        }
        if first {
            self.errors = 0;
        }
        if got == 0 || f.format == 0 {
            return;
        }
        let decoded = latency::now_micros();
        if f.mastering_max > 0.0 || f.max_cll > 0.0 {
            self.meta = [f.mastering_min, f.mastering_max, f.max_cll, f.max_fall];
        }
        let out = self.copy(&f, pts, received, decoded);
        self.latest.put(out);
        wake_main();
    }

    /// Décodeur en erreur : ses images ne sont plus sûres, une image clé tout de suite. Le décodeur matériel qui
    /// échoue en boucle : le processeur prend la suite, rouvert sur l'image clé.
    fn decode_error(&mut self, code: c_int) {
        self.errors += 1;
        if self.errors <= 3 || self.errors % 100 == 0 {
            log::warn!("image : erreur de décodage {code} ({} de suite)", self.errors);
        }
        let now = Instant::now();
        let steps = if self.errors >= 8 && !self.software && unsafe { kn_decoder_is_hw(self.dec) } != 0 && self.codec != 2 {
            log::warn!("image : le décodeur matériel échoue ; le processeur décode");
            self.software = true;
            self.close();
            self.flow.reset(now)
        } else {
            self.flow.failed(now)
        };
        self.apply(steps, None);
    }

    /// Copie l'image dans un tampon réutilisé : le décodeur garde la sienne.
    fn copy(&self, f: &KnFrame, pts: u64, received: i64, decoded: i64) -> Frame {
        let (w, h) = (f.width.max(0) as usize, f.height.max(0) as usize);
        let format = match f.format {
            1 => Format::Nv12,
            3 => Format::Bgra,
            4 => Format::P010,
            5 => Format::I010,
            _ => Format::I420,
        };
        let mut out = self.latest.buffer().unwrap_or_else(|| empty_frame(format));
        let rows = |plane: usize| match (format, plane) {
            (_, 0) => h,
            (Format::Bgra, _) => 0,
            (Format::Nv12 | Format::P010, 1) => h.div_ceil(2),
            (Format::Nv12 | Format::P010, _) => 0,
            (Format::I420 | Format::I010, _) => h.div_ceil(2),
        };
        // 10 bits : lignes serrées (OpenGL ES 2 n'a pas de pas de ligne) : largeur × 2 octets, chroma à moitié
        // (P010 : U et V entrelacés, la largeur de la luma).
        let tight = |plane: usize| match (format, plane) {
            (Format::P010 | Format::I010, 0) => Some(w * 2),
            (Format::P010, 1) => Some(w.div_ceil(2) * 4),
            (Format::I010, _) => Some(w.div_ceil(2) * 2),
            _ => None,
        };
        for p in 0..3 {
            let n = rows(p);
            let pitch = f.linesize[p].max(0) as usize;
            out.planes[p].clear();
            if n == 0 || pitch == 0 || f.data[p].is_null() {
                out.pitch[p] = 0;
                continue;
            }
            let src = unsafe { std::slice::from_raw_parts(f.data[p], pitch * n) };
            match tight(p).filter(|&t| t < pitch) {
                Some(t) => {
                    for row in src.chunks(pitch).take(n) {
                        out.planes[p].extend_from_slice(&row[..t]);
                    }
                    out.pitch[p] = t;
                }
                None => {
                    out.planes[p].extend_from_slice(src);
                    out.pitch[p] = pitch;
                }
            }
        }
        out.width = w as u32;
        out.height = h as u32;
        out.format = format;
        out.pts = pts;
        out.received = received;
        out.decoded = decoded;
        out.bt709 = f.bt709 != 0;
        out.full_range = f.full_range != 0;
        out.pq = f.pq != 0;
        [out.mastering_min, out.mastering_max, out.max_cll, out.max_fall] = self.meta;
        out.narrow_if_needed();
        out
    }
}

fn empty_frame(format: Format) -> Frame {
    Frame {
        width: 0,
        height: 0,
        format,
        planes: [Vec::new(), Vec::new(), Vec::new()],
        pitch: [0; 3],
        pts: 0,
        received: 0,
        decoded: 0,
        bt709: true,
        full_range: false,
        pq: false,
        mastering_min: 0.0,
        mastering_max: 0.0,
        max_cll: 0.0,
        max_fall: 0.0,
    }
}

// SAFETY : le décodeur n'est utilisé que par son fil.
unsafe impl Send for Decoder {}

// MARK: - Test du profil Auto

/// Le temps de décodage d'un extrait du test (docs/profil-auto.md) : 1 s au plus.
const SAMPLE_TIME: Duration = Duration::from_secs(1);

fn blank_frame() -> KnFrame {
    KnFrame {
        width: 0,
        height: 0,
        format: 0,
        data: [std::ptr::null_mut(); 3],
        linesize: [0; 3],
        pts: 0,
        bt709: 1,
        full_range: 0,
        pq: 0,
        bt2020: 0,
        mastering_min: 0.0,
        mastering_max: 0.0,
        max_cll: 0.0,
        max_fall: 0.0,
    }
}

/// Un extrait du test du profil Auto, décodé par le décodeur de la session, aussi vite qu'il les prend, en boucle
/// depuis l'image clé, pendant 1 s au plus : les images décodées par seconde ; `None` si le décodeur le refuse.
/// H.264, HEVC, AV1 : FFmpeg (`decoder.c`), la carte (VA-API) ou les décodeurs du Pi (V4L2) quand la session les
/// prendrait, sinon le processeur, images recopiées de la carte comme en session (le 4:4:4 converti en RVB) ;
/// PyroWave : Vulkan (`pyrowave.c`), plans relus comme en session. Rien n'est affiché.
pub fn decode_sample(s: &kybernow_core::autotest::Sample, m: &crate::app::Machine) -> Option<f64> {
    use kybernow_core::profile::Codec;
    unsafe { kn_log_quiet() };
    if s.packets.is_empty() {
        return None;
    }
    let codec = match s.codec {
        Codec::PyroWave => return pyrowave_sample(s),
        Codec::H264 => 0,
        Codec::Hevc => 1,
        Codec::Av1 => 2,
    };
    // Le décodeur matériel que la session prendrait (`Shared::hw_for`) ; le 4:4:4 seulement si la carte le sait.
    let hw = m.hw_decoders()[codec as usize] && (!s.yuv444 || m.hw_hevc444);
    let backend = m.hw_backend();
    let threads = std::thread::available_parallelism().map(|n| n.get().min(8)).unwrap_or(4) as c_int;
    if hw && let Some(fps) = ffmpeg_sample(s, codec, backend, threads) {
        return Some(fps);
    }
    // Le processeur : pas d'AV1 (FFmpeg est compilé sans décodeur AV1 logiciel).
    if codec == 2 {
        return None;
    }
    ffmpeg_sample(s, codec, 0, threads)
}

/// Les paquets de l'extrait passés à FFmpeg (`hw` : 0 processeur, 1 VA-API, 2 V4L2), en boucle, 1 s au plus.
fn ffmpeg_sample(s: &kybernow_core::autotest::Sample, codec: c_int, hw: c_int, threads: c_int) -> Option<f64> {
    let mut err = [0 as c_char; 256];
    let d = unsafe { kn_decoder_open(codec, hw, threads, err.as_mut_ptr(), err.len() as c_int) };
    if d.is_null() {
        let e = unsafe { CStr::from_ptr(err.as_ptr()) }.to_string_lossy().into_owned();
        log::info!("profil Auto : décodeur refusé ({e})");
        return None;
    }
    struct Close(*mut c_void);
    impl Drop for Close {
        fn drop(&mut self) {
            unsafe { kn_decoder_close(self.0) };
        }
    }
    let _close = Close(d);
    let mut f = blank_frame();
    let (mut i, mut pts, mut errors) = (0usize, 0i64, 0u32);
    let started = Instant::now();
    while started.elapsed() < SAMPLE_TIME {
        let p = &s.packets[i];
        let r = unsafe { kn_decoder_send(d, p.data.as_ptr(), p.data.len() as c_int, pts) };
        if r < 0 || unsafe { kn_decoder_receive(d, &mut f) } < 0 {
            errors += 1;
            // Un décodeur qui échoue en boucle (format refusé par la carte) : pas décodé ici.
            if errors >= 8 && unsafe { kn_decoder_frames(d) } == 0 {
                return None;
            }
        }
        pts += 1;
        i = (i + 1) % s.packets.len();
    }
    let elapsed = started.elapsed().as_secs_f64();
    let frames = unsafe { kn_decoder_frames(d) };
    (frames > 0).then(|| frames as f64 / elapsed)
}

/// L'image intra de l'extrait PyroWave décodée par Vulkan et relue (I420, BGRA en 4:4:4), en boucle, 1 s au plus.
fn pyrowave_sample(s: &kybernow_core::autotest::Sample) -> Option<f64> {
    let cfg = Config::from_sequence_header(&s.packets[0].data).unwrap_or(Config::new(s.yuv444, false, true, s.width, s.height));
    let mut err = [0 as c_char; 256];
    let d = unsafe { kn_pw_open(cfg.width as c_int, cfg.height as c_int, cfg.chroma444 as c_int, 0, err.as_mut_ptr(), err.len() as c_int) };
    if d.is_null() {
        let e = unsafe { CStr::from_ptr(err.as_ptr()) }.to_string_lossy().into_owned();
        log::info!("profil Auto : PyroWave refusé ({e})");
        return None;
    }
    struct Close(*mut c_void);
    impl Drop for Close {
        fn drop(&mut self) {
            unsafe { kn_pw_close(self.0) };
        }
    }
    let _close = Close(d);
    let (w, h) = (cfg.width as usize, cfg.height as usize);
    let (pitch, rows): ([usize; 3], [usize; 3]) = if cfg.chroma444 { ([w * 4, 0, 0], [h, 0, 0]) } else { ([w, w / 2, w / 2], [h, h / 2, h / 2]) };
    let mut planes: [Vec<u8>; 3] = [vec![0; pitch[0] * rows[0]], vec![0; pitch[1] * rows[1]], vec![0; pitch[2] * rows[2]]];
    let ptrs = [planes[0].as_mut_ptr(), planes[1].as_mut_ptr(), planes[2].as_mut_ptr()];
    let strides = pitch.map(|p| p as c_int);
    let (mut i, mut frames, mut errors) = (0usize, 0u64, 0u32);
    let started = Instant::now();
    while started.elapsed() < SAMPLE_TIME {
        let p = &s.packets[i];
        let r = unsafe { kn_pw_decode(d, p.data.as_ptr(), p.data.len(), ptrs.as_ptr(), strides.as_ptr(), 1, cfg.full_range as c_int) };
        if r == 1 {
            frames += 1;
        } else {
            errors += 1;
            if errors >= 8 && frames == 0 {
                return None;
            }
        }
        i = (i + 1) % s.packets.len();
    }
    let elapsed = started.elapsed().as_secs_f64();
    (frames > 0).then(|| frames as f64 / elapsed)
}

/// La conversion YUV de SDL (réglage global) pour la texture qu'on va créer, remplir ou dessiner.
fn set_yuv(yuv: Option<Yuv>) {
    let Some(y) = yuv else { return };
    let mode = match y {
        Yuv::Bt601 => sdl::SDL_YUV_CONVERSION_BT601,
        Yuv::Bt709 => sdl::SDL_YUV_CONVERSION_BT709,
        Yuv::Jpeg => sdl::SDL_YUV_CONVERSION_JPEG,
    };
    unsafe { sdl::SDL_SetYUVConversionMode(mode) };
}

/// La texture de l'image et ce qu'il faut pour la montrer (fil principal).
pub struct Screen {
    texture: *mut sdl::SDL_Texture,
    size: (u32, u32),
    format: Option<Format>,
    /// La conversion YUV de cette texture : SDL la lit à la création (rendu GL) ou à chaque conversion
    /// (rendu logiciel) dans un réglage global ; chaque fenêtre (plusieurs écrans, plusieurs ordinateurs)
    /// le repose donc pour elle avant de créer, d'envoyer et de dessiner.
    yuv: Option<Yuv>,
    pub last: Option<(u32, u32)>,
}

impl Default for Screen {
    fn default() -> Self {
        Self { texture: std::ptr::null_mut(), size: (0, 0), format: None, yuv: None, last: None }
    }
}

impl Screen {
    /// Envoie l'image à la texture (recréée si la taille ou le format change).
    /// Une image 10 bits (sans sous-surface HDR) est d'abord ramenée en 8 bits.
    pub fn upload(&mut self, r: *mut sdl::SDL_Renderer, f: &Frame) -> bool {
        if f.format.ten_bit() {
            return self.upload(r, &eight_bit(f));
        }
        unsafe {
            let yuv = (f.format != Format::Bgra).then(|| video_rules::yuv_for(f.bt709, f.full_range));
            set_yuv(yuv);
            if self.texture.is_null() || self.size != (f.width, f.height) || self.format != Some(f.format) || self.yuv != yuv {
                if !self.texture.is_null() {
                    sdl::SDL_DestroyTexture(self.texture);
                }
                let fmt = match f.format {
                    Format::Nv12 => sdl::SDL_PIXELFORMAT_NV12,
                    Format::I420 => sdl::SDL_PIXELFORMAT_IYUV,
                    // Octets B, G, R, A : ARGB8888 en petit-boutiste.
                    Format::Bgra => sdl::SDL_PIXELFORMAT_ARGB8888,
                    Format::P010 | Format::I010 => unreachable!(),
                };
                self.texture = sdl::SDL_CreateTexture(r, fmt, sdl::SDL_TEXTUREACCESS_STREAMING, f.width as c_int, f.height as c_int);
                if self.texture.is_null() {
                    log::error!("image : texture {} × {} impossible ({})", f.width, f.height, sdl::error());
                    return false;
                }
                self.size = (f.width, f.height);
                self.format = Some(f.format);
                self.yuv = yuv;
                log::info!("image : texture {} × {} ({:?}, {:?})", f.width, f.height, f.format, yuv);
            }
            let ok = match f.format {
                Format::Nv12 => sdl::SDL_UpdateNVTexture(self.texture, std::ptr::null(), f.planes[0].as_ptr(), f.pitch[0] as c_int, f.planes[1].as_ptr(), f.pitch[1] as c_int),
                Format::I420 => sdl::SDL_UpdateYUVTexture(
                    self.texture,
                    std::ptr::null(),
                    f.planes[0].as_ptr(),
                    f.pitch[0] as c_int,
                    f.planes[1].as_ptr(),
                    f.pitch[1] as c_int,
                    f.planes[2].as_ptr(),
                    f.pitch[2] as c_int,
                ),
                Format::Bgra => sdl::SDL_UpdateTexture(self.texture, std::ptr::null(), f.planes[0].as_ptr() as *const c_void, f.pitch[0] as c_int),
                Format::P010 | Format::I010 => unreachable!(),
            };
            self.last = Some((f.width, f.height));
            ok == 0
        }
    }

    /// Dessine l'image, proportions gardées, dans la sortie `(w, h)` ; rend le cadre.
    pub fn draw(&self, r: *mut sdl::SDL_Renderer, out: (i32, i32)) -> Option<sdl::SDL_Rect> {
        if self.texture.is_null() {
            return None;
        }
        let dst = fit(self.size, out);
        set_yuv(self.yuv);
        unsafe { sdl::SDL_RenderCopy(r, self.texture, std::ptr::null(), &dst) };
        Some(dst)
    }

    pub fn reset(&mut self) {
        if !self.texture.is_null() {
            unsafe { sdl::SDL_DestroyTexture(self.texture) };
        }
        *self = Self::default();
    }
}

/// Où poser une image de `size` pixels dans la sortie `out`, proportions gardées, centrée.
pub fn fit(size: (u32, u32), out: (i32, i32)) -> sdl::SDL_Rect {
    let (vw, vh) = (f64::from(size.0.max(1)), f64::from(size.1.max(1)));
    let s = (f64::from(out.0) / vw).min(f64::from(out.1) / vh);
    let (dw, dh) = ((vw * s).round() as i32, (vh * s).round() as i32);
    sdl::SDL_Rect { x: (out.0 - dw) / 2, y: (out.1 - dh) / 2, w: dw, h: dh }
}

/// Le code 10 bits du mot de 16 bits à `off` du plan `p` (P010 : bits de poids fort).
fn code10(f: &Frame, p: usize, off: usize) -> u16 {
    let w = f.planes[p].get(off..off + 2).map_or(0, |b| u16::from_le_bytes([b[0], b[1]]));
    if f.format == Format::P010 { w >> 6 } else { w & 0x3ff }
}

/// Les codes 10 bits (Y', Cb, Cr) du pixel (x, y) d'une image 10 bits.
fn ycbcr10(f: &Frame, x: usize, y: usize) -> (u16, u16, u16) {
    let yy = code10(f, 0, y * f.pitch[0] + x * 2);
    match f.format {
        Format::P010 => {
            let o = (y / 2) * f.pitch[1] + (x / 2) * 4;
            (yy, code10(f, 1, o), code10(f, 1, o + 2))
        }
        _ => {
            let o = (y / 2) * f.pitch[1] + (x / 2) * 2;
            (yy, code10(f, 1, o), code10(f, 2, (y / 2) * f.pitch[2] + (x / 2) * 2))
        }
    }
}

/// Une image 10 bits ramenée en 8 bits pour SDL (NV12 ou I420, codes divisés
/// par 4) : le repli sans sous-surface HDR, couleurs approchées (une image PQ
/// y paraît terne).
fn eight_bit(f: &Frame) -> Frame {
    let (w, h) = (f.width as usize, f.height as usize);
    let (cw, ch) = (w.div_ceil(2), h.div_ceil(2));
    let mut out = Frame {
        width: f.width,
        height: f.height,
        format: if f.format == Format::P010 { Format::Nv12 } else { Format::I420 },
        planes: [Vec::with_capacity(w * h), Vec::new(), Vec::new()],
        pitch: [w, 0, 0],
        pts: f.pts,
        received: f.received,
        decoded: f.decoded,
        bt709: true,
        full_range: false,
        pq: false,
        mastering_min: 0.0,
        mastering_max: 0.0,
        max_cll: 0.0,
        max_fall: 0.0,
    };
    // Par ligne, sans contrôle de bornes par pixel (LINAPP-33) ; une ligne absente (image tronquée) reste noire.
    let p010 = f.format == Format::P010;
    let row = |src: &[u8], pitch: usize, y: usize, n: usize, dst: &mut Vec<u8>| {
        let start = y * pitch;
        match src.get(start..start + n * 2) {
            Some(r) => dst.extend(r.chunks_exact(2).map(|b| {
                let w = u16::from_le_bytes([b[0], b[1]]);
                ((if p010 { w >> 6 } else { w & 0x3ff }) >> 2) as u8
            })),
            None => dst.resize(dst.len() + n, 0),
        }
    };
    for y in 0..h {
        row(&f.planes[0], f.pitch[0], y, w, &mut out.planes[0]);
    }
    if p010 {
        out.pitch[1] = cw * 2;
        out.planes[1].reserve(cw * 2 * ch);
        for y in 0..ch {
            row(&f.planes[1], f.pitch[1], y, cw * 2, &mut out.planes[1]);
        }
    } else {
        out.pitch[1] = cw;
        out.pitch[2] = cw;
        for p in 1..3 {
            out.planes[p].reserve(cw * ch);
            for y in 0..ch {
                row(&f.planes[p], f.pitch[p], y, cw, &mut out.planes[p]);
            }
        }
    }
    out
}

/// Une image réduite à `width` de large, en RVB (BT.709 ou BT.601, plage
/// limitée ou complète, selon l'image ; une image PQ ramenée au SDR, le blanc
/// du bureau à 203 cd/m²), pour l'accueil : la dernière image de la session.
pub fn snapshot(f: &Frame, width: u32) -> Option<tiny_skia::Pixmap> {
    if f.width == 0 || f.height == 0 || f.planes[0].is_empty() {
        return None;
    }
    let w = width.min(f.width);
    let h = (f.height as u64 * w as u64 / f.width as u64).max(1) as u32;
    let mut p = tiny_skia::Pixmap::new(w, h)?;
    let d = p.data_mut();
    let (kr, kb) = if f.bt709 { (0.2126f32, 0.0722f32) } else { (0.299, 0.114) };
    let kg = 1.0 - kr - kb;
    for y in 0..h {
        let sy = (y as u64 * f.height as u64 / h as u64) as usize;
        for x in 0..w {
            let sx = (x as u64 * f.width as u64 / w as u64) as usize;
            let i = ((y * w + x) * 4) as usize;
            if f.format.ten_bit() {
                let (yy, cb, cr) = ycbcr10(f, sx, sy);
                let rgb = if f.pq { crate::hdr::pq_to_srgb8(yy, cb, cr, crate::hdr::Luminance::default().sdr_white) } else { bt709_8(yy, cb, cr) };
                d[i..i + 3].copy_from_slice(&rgb);
                d[i + 3] = 255;
                continue;
            }
            if f.format == Format::Bgra {
                let s = sy * f.pitch[0] + sx * 4;
                d[i] = f.planes[0][s + 2];
                d[i + 1] = f.planes[0][s + 1];
                d[i + 2] = f.planes[0][s];
                d[i + 3] = 255;
                continue;
            }
            let yy = f.planes[0][sy * f.pitch[0] + sx] as f32;
            let (u, v) = match f.format {
                Format::Nv12 => {
                    let i = (sy / 2) * f.pitch[1] + (sx / 2) * 2;
                    (f.planes[1][i] as f32, f.planes[1][i + 1] as f32)
                }
                Format::I420 => (f.planes[1][(sy / 2) * f.pitch[1] + sx / 2] as f32, f.planes[2][(sy / 2) * f.pitch[2] + sx / 2] as f32),
                Format::Bgra | Format::P010 | Format::I010 => unreachable!(),
            };
            let (yn, un, vn) = if f.full_range { (yy / 255.0, (u - 128.0) / 255.0, (v - 128.0) / 255.0) } else { ((yy - 16.0) / 219.0, (u - 128.0) / 224.0, (v - 128.0) / 224.0) };
            let r = yn + 2.0 * (1.0 - kr) * vn;
            let b = yn + 2.0 * (1.0 - kb) * un;
            let g = (yn - kr * r - kb * b) / kg;
            d[i] = (r.clamp(0.0, 1.0) * 255.0) as u8;
            d[i + 1] = (g.clamp(0.0, 1.0) * 255.0) as u8;
            d[i + 2] = (b.clamp(0.0, 1.0) * 255.0) as u8;
            d[i + 3] = 255;
        }
    }
    Some(p)
}

/// Un pixel 10 bits sans PQ (BT.709, plage limitée), en sRGB 8 bits approché.
fn bt709_8(y: u16, cb: u16, cr: u16) -> [u8; 3] {
    let yn = (f32::from(y) - 64.0) / 876.0;
    let (un, vn) = ((f32::from(cb) - 512.0) / 896.0, (f32::from(cr) - 512.0) / 896.0);
    let r = yn + 1.5748 * vn;
    let b = yn + 1.8556 * un;
    let g = (yn - 0.2126 * r - 0.0722 * b) / 0.7152;
    [r, g, b].map(|c| (c.clamp(0.0, 1.0) * 255.0) as u8)
}

/// Les métadonnées HDR10 de l'en-tête `KNPW` version 2 (`Config::hdr10` du cœur) dans l'ordre de l'image HDR :
/// minimum et maximum du mastering display (cd/m²), MaxCLL, MaxFALL ; 0 pour ce que le paquet n'a pas (version 1, SDR,
/// drapeau à 0).
fn pw_meta(h: &kybernow_core::pyrowave::ContentHdr) -> [f32; 4] {
    let (min, max) = h.mastering.map_or((0.0, 0.0), |m| (m.min_nits() as f32, m.max_nits() as f32));
    let (cll, fall) = h.light.map_or((0.0, 0.0), |l| (f32::from(l.max_cll), f32::from(l.max_fall)));
    [min, max, cll, fall]
}

#[cfg(test)]
mod pyrowave_tests {
    use super::*;

    #[test]
    fn format_de_l_image_pyrowave() {
        let c = Config::new(false, true, false, 2560, 1440);
        assert_eq!(pw_format(&c), Format::I010);
        assert_eq!(pw_format(&Config { chroma444: true, hdr: false, ..c }), Format::Bgra);
        assert_eq!(pw_format(&Config { hdr: false, ..c }), Format::I420);
    }

    /// Les métadonnées de l'en-tête version 2, lues par le cœur, dans l'ordre de l'image HDR.
    #[test]
    fn metadonnees_de_l_en_tete_pyrowave_v2() {
        use kybernow_core::pyrowave::{ContentHdr, LightLevel, Mastering};
        let mastering = Mastering { primaries: [35400, 14600, 8500, 39850, 6550, 2300], white: [15635, 16450], max_luminance: 16_000_000, min_luminance: 1 };
        let m = pw_meta(&ContentHdr { mastering: Some(mastering), light: Some(LightLevel { max_cll: 1200, max_fall: 400 }) });
        assert!((m[0] - 0.0001).abs() < 1e-9 && m[1] == 1600.0 && m[2] == 1200.0 && m[3] == 400.0, "{m:?}");
        // Le niveau de lumière absent : MaxCLL et MaxFALL à 0, le mastering display gardé.
        assert_eq!(pw_meta(&ContentHdr { mastering: Some(mastering), light: None })[1..], [1600.0, 0.0, 0.0]);
        // Version 1, SDR : rien.
        assert_eq!(pw_meta(&ContentHdr::default()), [0.0; 4]);
    }
}
