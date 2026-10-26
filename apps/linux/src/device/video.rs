//! Image de la session : les paquets du correctif 0001 (Annex B) vont, depuis
//! un fil à part (le fil réseau de Kyber ne bloque jamais), à FFmpeg
//! (`src/native/decoder.c`) : le décodeur matériel (VA-API ; sur le Pi, V4L2)
//! quand la machine sait le format, sinon le
//! processeur. Le jeu de paramètres (SPS/PPS, VPS, en-tête AV1) est placé
//! devant chaque image clé ; après une perte, rien avant l'image clé suivante,
//! redemandée par le cœur. Seule l'image la plus récente est gardée pour le
//! fil principal, qui la montre (`take`) : le retard ne s'accumule pas.

use std::ffi::{c_char, c_int, c_void, CStr};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use kybernow_core::latency;
use kybernow_core::media::{Packet, PacketHandler, FOURCC_AV1, FOURCC_HEVC};

use super::sdl;
use super::session::Shared;

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
}

unsafe extern "C" {
    fn kn_log_quiet();
    fn kn_decoder_open(codec: c_int, hw: c_int, threads: c_int, err: *mut c_char, errlen: c_int) -> *mut c_void;
    fn kn_decoder_name(d: *mut c_void) -> *const c_char;
    fn kn_decoder_is_hw(d: *mut c_void) -> c_int;
    fn kn_decoder_send(d: *mut c_void, data: *const u8, len: c_int, pts: i64) -> c_int;
    fn kn_decoder_receive(d: *mut c_void, out: *mut KnFrame) -> c_int;
    fn kn_decoder_close(d: *mut c_void);
}

/// Événement SDL qui réveille la boucle principale : une image est prête.
pub static FRAME_EVENT: AtomicU32 = AtomicU32::new(0);

fn wake_main() {
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
}

/// L'image la plus récente, et les tampons à réutiliser.
#[derive(Default)]
pub struct Latest {
    slot: Mutex<Option<Frame>>,
    spare: Mutex<Vec<Frame>>,
    /// Images remplacées avant d'être montrées (écartées).
    pub dropped: AtomicU32,
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

    fn put(&self, f: Frame) {
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
    Frame { key: bool, pts: u64, data: Vec<u8>, received: i64 },
    Hole,
    Stop,
}

/// Un flux d'image (un par démarrage de l'image, et par écran de l'appareil
/// en multi-écrans : un décodeur et une file d'images chacun).
pub struct VideoSink {
    tx: Mutex<Sender<Msg>>,
    thread: Mutex<Option<JoinHandle<()>>>,
    shared: Arc<Shared>,
}

impl VideoSink {
    pub fn new(shared: Arc<Shared>, screen: usize) -> Arc<Self> {
        let (tx, rx) = channel();
        let s = shared.clone();
        let latest = shared.fresh_latest(screen);
        let thread = std::thread::Builder::new().name("kybernow-image".into()).spawn(move || Decoder::new(s, latest).run(rx)).ok();
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

struct Decoder {
    shared: Arc<Shared>,
    /// L'image la plus récente de cet écran, pour le fil principal.
    latest: Arc<Latest>,
    /// 0 H.264, 1 HEVC, 2 AV1.
    codec: c_int,
    config: Vec<u8>,
    in_config: bool,
    loaded_config: Option<Vec<u8>>,
    dec: *mut c_void,
    /// Décodeur matériel refusé ou en échec : le processeur décode.
    software: bool,
    waiting_key: bool,
    lost_reported: bool,
    last_request: Option<Instant>,
    errors: u32,
    failed: bool,
}

impl Decoder {
    fn new(shared: Arc<Shared>, latest: Arc<Latest>) -> Self {
        unsafe { kn_log_quiet() };
        Self {
            shared,
            latest,
            codec: 1,
            config: Vec::new(),
            in_config: false,
            loaded_config: None,
            dec: std::ptr::null_mut(),
            software: false,
            waiting_key: true,
            lost_reported: false,
            last_request: None,
            errors: 0,
            failed: false,
        }
    }

    fn run(mut self, rx: Receiver<Msg>) {
        while let Ok(m) = rx.recv() {
            match m {
                Msg::Codec(c) => {
                    self.codec = match c {
                        FOURCC_HEVC => 1,
                        FOURCC_AV1 => 2,
                        _ => 0,
                    };
                    self.close();
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
                    self.frame(key, pts, &data, received);
                }
                Msg::Hole => self.lost(),
                Msg::Stop => break,
            }
        }
        self.close();
    }

    fn close(&mut self) {
        if !self.dec.is_null() {
            unsafe { kn_decoder_close(self.dec) };
            self.dec = std::ptr::null_mut();
        }
        self.loaded_config = None;
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
        let name = unsafe { CStr::from_ptr(kn_decoder_name(d)) }.to_string_lossy().into_owned();
        log::info!("image : décodeur {name}");
        self.shared.set_codec(&name);
        Ok(())
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
        if self.dec.is_null() || stale {
            if !key {
                self.request_keyframe();
                return;
            }
            self.close();
            if let Err(e) = self.open() {
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
            self.request_keyframe();
            return;
        }
        let first = self.waiting_key;
        let buf;
        let bytes: &[u8] = if key && !self.config.is_empty() {
            buf = [self.config.as_slice(), data].concat();
            &buf
        } else {
            data
        };
        let r = unsafe { kn_decoder_send(self.dec, bytes.as_ptr(), bytes.len() as c_int, pts as i64) };
        if r < 0 {
            self.decode_error(r);
            return;
        }
        let mut f = KnFrame { width: 0, height: 0, format: 0, data: [std::ptr::null_mut(); 3], linesize: [0; 3], pts: 0, bt709: 1, full_range: 0 };
        let got = unsafe { kn_decoder_receive(self.dec, &mut f) };
        if got < 0 {
            self.decode_error(got);
            return;
        }
        if key && first {
            self.waiting_key = false;
            self.errors = 0;
            if self.lost_reported {
                self.lost_reported = false;
                if let Some(s) = self.shared.session() {
                    s.keyframe_recovered();
                }
            }
        }
        if got == 0 || f.format == 0 {
            return;
        }
        let decoded = latency::now_micros();
        let out = self.copy(&f, pts, received, decoded);
        self.latest.put(out);
        wake_main();
    }

    fn decode_error(&mut self, code: c_int) {
        self.errors += 1;
        if self.errors <= 3 || self.errors % 100 == 0 {
            log::warn!("image : erreur de décodage {code} ({} de suite)", self.errors);
        }
        // Le décodeur matériel qui échoue en boucle : le processeur prend la suite.
        if self.errors >= 8 && !self.software && unsafe { kn_decoder_is_hw(self.dec) } != 0 && self.codec != 2 {
            log::warn!("image : le décodeur matériel échoue ; le processeur décode");
            self.software = true;
            self.close();
        }
        self.lost();
        self.request_keyframe();
    }

    /// Copie l'image dans un tampon réutilisé : le décodeur garde la sienne.
    fn copy(&self, f: &KnFrame, pts: u64, received: i64, decoded: i64) -> Frame {
        let (w, h) = (f.width.max(0) as usize, f.height.max(0) as usize);
        let format = match f.format {
            1 => Format::Nv12,
            3 => Format::Bgra,
            _ => Format::I420,
        };
        let mut out = self.latest.buffer().unwrap_or_else(|| Frame {
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
        });
        let rows = |plane: usize| match (format, plane) {
            (_, 0) => h,
            (Format::Bgra, _) => 0,
            (Format::Nv12, 1) => h.div_ceil(2),
            (Format::Nv12, _) => 0,
            (Format::I420, _) => h.div_ceil(2),
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
            out.planes[p].extend_from_slice(src);
            out.pitch[p] = pitch;
        }
        out.width = w as u32;
        out.height = h as u32;
        out.format = format;
        out.pts = pts;
        out.received = received;
        out.decoded = decoded;
        out.bt709 = f.bt709 != 0;
        out.full_range = f.full_range != 0;
        out
    }
}

// SAFETY : le décodeur n'est utilisé que par son fil.
unsafe impl Send for Decoder {}

/// La texture de l'image et ce qu'il faut pour la montrer (fil principal).
pub struct Screen {
    texture: *mut sdl::SDL_Texture,
    size: (u32, u32),
    format: Option<Format>,
    pub last: Option<(u32, u32)>,
}

impl Default for Screen {
    fn default() -> Self {
        Self { texture: std::ptr::null_mut(), size: (0, 0), format: None, last: None }
    }
}

impl Screen {
    /// Envoie l'image à la texture (recréée si la taille ou le format change).
    pub fn upload(&mut self, r: *mut sdl::SDL_Renderer, f: &Frame) -> bool {
        unsafe {
            if self.texture.is_null() || self.size != (f.width, f.height) || self.format != Some(f.format) {
                if !self.texture.is_null() {
                    sdl::SDL_DestroyTexture(self.texture);
                }
                let fmt = match f.format {
                    Format::Nv12 => sdl::SDL_PIXELFORMAT_NV12,
                    Format::I420 => sdl::SDL_PIXELFORMAT_IYUV,
                    // Octets B, G, R, A : ARGB8888 en petit-boutiste.
                    Format::Bgra => sdl::SDL_PIXELFORMAT_ARGB8888,
                };
                self.texture = sdl::SDL_CreateTexture(r, fmt, sdl::SDL_TEXTUREACCESS_STREAMING, f.width as c_int, f.height as c_int);
                if self.texture.is_null() {
                    log::error!("image : texture {} × {} impossible ({})", f.width, f.height, sdl::error());
                    return false;
                }
                self.size = (f.width, f.height);
                self.format = Some(f.format);
                sdl::SDL_SetYUVConversionMode(if f.full_range { sdl::SDL_YUV_CONVERSION_JPEG } else if f.bt709 { sdl::SDL_YUV_CONVERSION_BT709 } else { sdl::SDL_YUV_CONVERSION_BT601 });
                log::info!("image : texture {} × {} ({:?})", f.width, f.height, f.format);
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
        let (vw, vh) = (self.size.0 as f64, self.size.1 as f64);
        let s = (out.0 as f64 / vw).min(out.1 as f64 / vh);
        let (dw, dh) = ((vw * s).round() as i32, (vh * s).round() as i32);
        let dst = sdl::SDL_Rect { x: (out.0 - dw) / 2, y: (out.1 - dh) / 2, w: dw, h: dh };
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

/// Une image réduite à `width` de large, en RVB (BT.709 ou BT.601, plage
/// limitée ou complète, selon l'image), pour l'accueil : la dernière image de
/// la session.
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
                Format::Bgra => unreachable!(),
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
