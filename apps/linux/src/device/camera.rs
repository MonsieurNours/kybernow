//! La caméra de l'appareil vers l'ordinateur (docs/webcam.md, règles dans docs/link-ui.md, « Caméra de
//! l'appareil ») : la webcam et l'encodeur sont dans `src/native/camera.c` (V4L2, FFmpeg : VA-API ou V4L2
//! M2M) ; ici, le fil qui donne chaque image au cœur (`CameraLink::frame`), la liste des webcams et la
//! webcam choisie, gardée pour l'appareil.

use std::ffi::{CStr, CString, c_char, c_int, c_void};
use std::sync::atomic::{AtomicBool, AtomicPtr, Ordering};
use std::sync::{Arc, OnceLock};
use std::thread::JoinHandle;
use std::time::Instant;

use kybernow_core::camera::{CameraCodec, CameraLink, CameraResolution, CameraSettings, FPS, FrameOutcome};

unsafe extern "C" {
    fn kn_camera_list(out: *mut c_char, size: c_int) -> c_int;
    fn kn_camera_encoder_available() -> c_int;
    fn kn_cam_open(path: *const c_char, w: c_int, h: c_int, bitrate: c_int, out_w: *mut c_int, out_h: *mut c_int, error: *mut c_int) -> *mut c_void;
    fn kn_cam_next(c: *mut c_void, data: *mut *const u8, size: *mut c_int, pts_us: *mut i64, key: *mut c_int) -> c_int;
    fn kn_cam_force_key(c: *mut c_void);
    fn kn_cam_set_bitrate(c: *mut c_void, bitrate: c_int);
    fn kn_cam_close(c: *mut c_void);
}

/// Les webcams de l'appareil : chemin (`/dev/videoN`) et nom.
pub fn cameras() -> Vec<(String, String)> {
    let mut buf = vec![0u8; 8192];
    unsafe { kn_camera_list(buf.as_mut_ptr() as *mut c_char, buf.len() as c_int) };
    let text = unsafe { CStr::from_ptr(buf.as_ptr() as *const c_char) }.to_string_lossy().into_owned();
    text.lines().filter_map(|l| l.split_once('\t').map(|(p, n)| (p.to_owned(), n.trim().to_owned()))).collect()
}

/// L'appareil sait envoyer sa caméra : une webcam et un encodeur H.264 matériel (vérifié une fois).
pub fn available() -> bool {
    static ENCODER: OnceLock<bool> = OnceLock::new();
    !cameras().is_empty() && *ENCODER.get_or_init(|| unsafe { kn_camera_encoder_available() } != 0)
}

fn choice_path() -> Option<std::path::PathBuf> {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(std::path::PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| std::path::PathBuf::from(h).join(".config")))?;
    Some(base.join("kybernow").join("camera.txt"))
}

/// La webcam choisie dans le menu, gardée pour l'appareil (pas par ordinateur).
pub fn chosen() -> Option<String> {
    std::fs::read_to_string(choice_path()?).ok().map(|s| s.trim().to_owned()).filter(|s| !s.is_empty())
}

pub fn choose(path: &str) {
    if let Some(p) = choice_path() {
        if let Some(dir) = p.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let _ = std::fs::write(p, path);
    }
}

/// Pourquoi la webcam s'arrête : prise par une autre app, partie (débranchée), pas d'encodeur.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Problem {
    InUse,
    Unplugged,
    NoEncoder,
}

/// L'envoi en cours : le fil de la webcam, arrêté et attendu quand on le lâche.
pub struct Sender {
    stop: Arc<AtomicBool>,
    key: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
    /// La webcam (son nom pour les avis).
    pub name: String,
}

impl Sender {
    /// Ouvre la webcam `path` (nulle : la choisie, sinon la première) et l'envoie ; `problem` (du fil de la
    /// webcam) dit pourquoi elle s'arrête en route.
    pub fn start(link: Arc<CameraLink>, path: Option<String>, resolution: CameraResolution, device: String, problem: impl Fn(Problem) + Send + 'static) -> Sender {
        let list = cameras();
        let wanted = path.or_else(chosen).filter(|p| list.iter().any(|(q, _)| q == p));
        let (path, name) = match wanted.and_then(|p| list.iter().find(|(q, _)| *q == p).cloned()).or_else(|| list.first().cloned()) {
            Some(c) => c,
            None => (String::new(), String::new()),
        };
        let stop = Arc::new(AtomicBool::new(false));
        let key = Arc::new(AtomicBool::new(true));
        let (s, k) = (stop.clone(), key.clone());
        let thread = std::thread::Builder::new()
            .name("kybernow-camera".into())
            .spawn(move || {
                if let Err(p) = run(&link, &path, resolution, &device, &s, &k) {
                    problem(p);
                }
            })
            .ok();
        Sender { stop, key, thread, name }
    }

    pub fn force_keyframe(&self) {
        self.key.store(true, Ordering::Relaxed);
    }
}

impl Drop for Sender {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

struct Cam(AtomicPtr<c_void>);

impl Drop for Cam {
    fn drop(&mut self) {
        unsafe { kn_cam_close(self.0.load(Ordering::Relaxed)) };
    }
}

fn run(link: &CameraLink, path: &str, resolution: CameraResolution, device: &str, stop: &AtomicBool, key: &AtomicBool) -> Result<(), Problem> {
    if path.is_empty() {
        return Err(Problem::Unplugged);
    }
    let (w, h) = resolution.size();
    let mut bitrate = link.target_bitrate();
    let (mut ow, mut oh, mut error) = (0, 0, 0);
    let cpath = CString::new(path).map_err(|_| Problem::Unplugged)?;
    let raw = unsafe { kn_cam_open(cpath.as_ptr(), w as c_int, h as c_int, bitrate as c_int, &mut ow, &mut oh, &mut error) };
    if raw.is_null() {
        return Err(match error {
            1 => Problem::InUse,
            3 => Problem::NoEncoder,
            _ => Problem::Unplugged,
        });
    }
    let cam = Cam(AtomicPtr::new(raw));
    let c = cam.0.load(Ordering::Relaxed);
    let settings = CameraSettings { codec: CameraCodec::H264, width: ow as u32, height: oh as u32, fps: FPS, resolution, device: device.to_owned() };
    if !link.start(settings) {
        return Ok(());
    }
    log::info!("caméra : {path}, {ow} × {oh}");
    let mut last_rate = Instant::now();
    while !stop.load(Ordering::Relaxed) {
        if key.swap(false, Ordering::Relaxed) {
            unsafe { kn_cam_force_key(c) };
        }
        if last_rate.elapsed().as_secs() >= 1 {
            last_rate = Instant::now();
            let target = link.target_bitrate();
            if target != bitrate {
                bitrate = target;
                unsafe { kn_cam_set_bitrate(c, bitrate as c_int) };
            }
        }
        let (mut data, mut size, mut pts, mut k) = (std::ptr::null(), 0, 0i64, 0);
        match unsafe { kn_cam_next(c, &mut data, &mut size, &mut pts, &mut k) } {
            1 => {
                let bytes = unsafe { std::slice::from_raw_parts(data, size.max(0) as usize) };
                if link.frame(bytes, pts.max(0) as u64, k != 0) == FrameOutcome::WantKeyframe {
                    key.store(true, Ordering::Relaxed);
                }
            }
            0 => {}
            _ => return Err(Problem::Unplugged),
        }
    }
    Ok(())
}
