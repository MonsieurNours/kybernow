//! Le HDR sur un bureau Wayland (docs/hdr.md, src/native/wayland_hdr.c) : la
//! sonde du compositeur avant SDL, et pour chaque fenêtre de SDL sur Wayland
//! une sous-surface où l'image HDR est dessinée en 10 bits avec sa
//! description d'image (PQ, BT.2020) ; la fenêtre y laisse un trou
//! transparent. Le retour de la surface dit si l'écran de la fenêtre est HDR.
//! Les calculs (courbes, choix du rendu) sont dans `crate::hdr`.

use std::ffi::{c_char, c_int, c_void, CStr};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

use super::sdl;
use super::video::{Format, Frame};
use crate::hdr::{Luminance, Render, StreamLight, Tone, WlDescription, WlOutput, WlProbe};

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct KnWlDesc {
    tf: u32,
    min_lum: u32,
    max_lum: u32,
    reference_lum: u32,
    target_min: u32,
    target_max: u32,
    max_cll: u32,
    max_fall: u32,
}

impl From<KnWlDesc> for WlDescription {
    fn from(d: KnWlDesc) -> Self {
        WlDescription { tf: d.tf, min_lum: d.min_lum, max_lum: d.max_lum, reference_lum: d.reference_lum, target_min: d.target_min, target_max: d.target_max, max_cll: d.max_cll, max_fall: d.max_fall }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct KnWlOutputInfo {
    x: i32,
    y: i32,
    desc: KnWlDesc,
}

#[repr(C)]
#[derive(Clone, Copy, Default, PartialEq)]
struct KnHdrTone {
    mode: c_int,
    src_hi: f32,
    dst_lo: f32,
    dst_hi: f32,
    white: f32,
}

#[repr(C)]
#[derive(Clone, Copy, Default, PartialEq)]
struct KnHdrMeta {
    mastering_min: f32,
    mastering_max: f32,
    max_cll: f32,
    max_fall: f32,
    sdr_white: f32,
}

unsafe extern "C" {
    fn kn_wl_probe(supported: *mut c_int, outs: *mut KnWlOutputInfo, max: c_int) -> c_int;
    fn kn_hdr_surface_new(window: *mut sdl::SDL_Window, require_cm: c_int, err: *mut c_char, errlen: c_int) -> *mut c_void;
    fn kn_hdr_surface_poll(s: *mut c_void, out: *mut KnWlDesc) -> c_int;
    fn kn_hdr_surface_present(s: *mut c_void, format: c_int, width: c_int, height: c_int, planes: *const *const u8, t: *const KnHdrTone, m: *const KnHdrMeta, x: c_int, y: c_int, w: c_int, h: c_int) -> c_int;
    fn kn_hdr_surface_hide(s: *mut c_void);
    fn kn_hdr_surface_bits(s: *mut c_void) -> c_int;
    fn kn_hdr_surface_error(s: *mut c_void) -> *const c_char;
    fn kn_hdr_surface_free(s: *mut c_void);
    #[cfg(test)]
    fn kn_hdr_offscreen(format: c_int, width: c_int, height: c_int, planes: *const *const u8, t: *const KnHdrTone, rgba: *mut u8, err: *mut c_char, errlen: c_int) -> c_int;
}

/// La voie du HDR est ouverte : SDL est sur Wayland, le compositeur décrit une
/// image PQ BT.2020, et la fenêtre de l'app a sa sous-surface.
static ACTIVE: AtomicBool = AtomicBool::new(false);
/// La dernière sonde des sorties (écrans de plus d'une session à plusieurs écrans).
static PROBE: Mutex<Option<WlProbe>> = Mutex::new(None);

pub fn active() -> bool {
    ACTIVE.load(Ordering::Relaxed)
}

pub fn set_active(on: bool) {
    ACTIVE.store(on, Ordering::Relaxed);
}

/// Les sorties relues (hors session, avec les écrans de l'appareil).
pub fn reprobe() {
    if active() {
        *PROBE.lock().unwrap() = probe();
    }
}

pub fn set_probe(p: Option<WlProbe>) {
    *PROBE.lock().unwrap() = p;
}

/// Les valeurs de l'écran de SDL à cette position (cadre logique) s'il est
/// HDR et que la voie du HDR est ouverte.
pub fn screen_at(x: i32, y: i32) -> Option<Luminance> {
    if !active() {
        return None;
    }
    PROBE.lock().unwrap().as_ref().and_then(|p| p.hdr_at(x, y))
}

/// Les valeurs d'un écran pour le cœur (bornées par lui).
pub fn core_luminance(l: Luminance) -> kybernow_core::profile::HdrLuminance {
    kybernow_core::profile::HdrLuminance { max: l.max, full_frame: l.full_frame, min: l.min, sdr_white: l.sdr_white }.clamped()
}

/// Ce que dit le compositeur, sur une connexion à part (avant SDL, puis quand
/// les écrans changent) ; `None` sans Wayland.
pub fn probe() -> Option<WlProbe> {
    let mut supported = 0;
    let mut outs = [KnWlOutputInfo::default(); 8];
    let n = unsafe { kn_wl_probe(&mut supported, outs.as_mut_ptr(), outs.len() as c_int) };
    if n < 0 {
        return None;
    }
    let outputs = outs[..n as usize].iter().map(|o| WlOutput { x: o.x, y: o.y, desc: o.desc.into() }).collect();
    Some(WlProbe { supported: supported != 0, outputs })
}

fn tone(t: Tone) -> KnHdrTone {
    KnHdrTone { mode: t.mode, src_hi: t.src_hi, dst_lo: t.dst_lo, dst_hi: t.dst_hi, white: t.white }
}

fn c_format(f: Format) -> Option<c_int> {
    match f {
        Format::P010 => Some(4),
        Format::I010 => Some(5),
        _ => None,
    }
}

/// La sous-surface HDR d'une fenêtre de SDL sur Wayland.
pub struct HdrSurface {
    raw: *mut c_void,
    /// Ce que l'écran de la fenêtre préfère (dernière description reçue).
    pub preferred: WlDescription,
    /// L'image se voit sur la sous-surface (sinon c'est SDL qui la montre).
    pub shown: bool,
    /// Le rendu de la dernière image (journal quand il change).
    last: Option<Render>,
    failed: bool,
}

impl HdrSurface {
    /// La sous-surface de cette fenêtre ; `None` si elle n'est pas sur Wayland
    /// ou que le compositeur ne décrit pas une image PQ BT.2020.
    pub fn new(window: *mut sdl::SDL_Window) -> Option<Self> {
        Self::open(window, true)
    }

    /// `require_cm` faux (essai) : sans gestion des couleurs aussi.
    fn open(window: *mut sdl::SDL_Window, require_cm: bool) -> Option<Self> {
        let mut err = [0 as c_char; 256];
        let raw = unsafe { kn_hdr_surface_new(window, require_cm as c_int, err.as_mut_ptr(), err.len() as c_int) };
        if raw.is_null() {
            let e = unsafe { CStr::from_ptr(err.as_ptr()) }.to_string_lossy().into_owned();
            log::info!("HDR : pas de sous-surface ({e})");
            return None;
        }
        let mut s = Self { raw, preferred: WlDescription::default(), shown: false, last: None, failed: false };
        s.poll();
        log::info!("HDR : sous-surface prête, écran de la fenêtre {}", describe(&s.preferred));
        Some(s)
    }

    /// Traite ce qui est arrivé ; `true` si l'écran de la fenêtre a changé (HDR, luminosités).
    pub fn poll(&mut self) -> bool {
        let mut d = KnWlDesc::default();
        let changed = unsafe { kn_hdr_surface_poll(self.raw, &mut d) } != 0;
        let desc: WlDescription = d.into();
        let differs = desc != self.preferred;
        self.preferred = desc;
        changed && differs
    }

    /// Les valeurs de l'écran de la fenêtre s'il est HDR.
    pub fn screen(&self) -> Option<Luminance> {
        self.preferred.is_hdr().then(|| self.preferred.luminance())
    }

    /// La sous-surface a renoncé (EGL, shader) : l'image passe par SDL, en SDR.
    pub fn failed(&self) -> bool {
        self.failed
    }

    /// Montre une image 10 bits (P010, I010) en `rect` (pixels de la sortie de
    /// SDL ; `density` : pixels par point de la fenêtre), d'après l'écran de la
    /// fenêtre, les métadonnées du flux et les valeurs données à la Station.
    pub fn present(&mut self, f: &Frame, rect: sdl::SDL_Rect, density: f32, sent: Luminance) -> bool {
        let Some(format) = c_format(f.format) else { return false };
        if self.failed {
            return false;
        }
        let light = StreamLight { max_cll: f64::from(f.max_cll), mastering_max: f64::from(f.mastering_max), mastering_min: f64::from(f.mastering_min) };
        let render = if f.pq { crate::hdr::render_for(self.screen(), light, sent) } else { Render::Pq };
        if self.last != Some(render) {
            log::info!("HDR : image {} × {} ({:?}, {} bits), rendu {render:?}", f.width, f.height, f.format, unsafe { kn_hdr_surface_bits(self.raw) }.max(10));
            self.last = Some(render);
        }
        let t = tone(Tone::of(render));
        // La description de l'image : ce que le flux dit, ramené à l'écran quand l'EETF l'y ramène.
        let (top, cll) = match render {
            Render::Eetf { dst_max, .. } => (dst_max as f32, (light.peak(sent.max) as f32).min(dst_max as f32)),
            _ => (if f.mastering_max > 0.0 { f.mastering_max } else { sent.max as f32 }, f.max_cll),
        };
        let m = KnHdrMeta { mastering_min: f.mastering_min, mastering_max: top, max_cll: cll, max_fall: f.max_fall.min(top), sdr_white: sent.sdr_white as f32 };
        let planes = [f.planes[0].as_ptr(), f.planes[1].as_ptr(), if f.format == Format::I010 { f.planes[2].as_ptr() } else { std::ptr::null() }];
        let d = density.max(0.01);
        let (x, y, w, h) = ((rect.x as f32 / d).round() as c_int, (rect.y as f32 / d).round() as c_int, (rect.w as f32 / d).round() as c_int, (rect.h as f32 / d).round() as c_int);
        let r = unsafe { kn_hdr_surface_present(self.raw, format, f.width as c_int, f.height as c_int, planes.as_ptr(), &t, &m, x, y, w, h) };
        if r < 0 {
            let e = unsafe { CStr::from_ptr(kn_hdr_surface_error(self.raw)) }.to_string_lossy().into_owned();
            log::error!("HDR : image impossible sur la sous-surface ({e}) ; l'image passe par SDL, en SDR");
            self.failed = true;
            self.hide();
            return false;
        }
        self.shown = true;
        true
    }

    /// L'image repasse par SDL (image SDR, fin de session).
    pub fn hide(&mut self) {
        if self.shown {
            unsafe { kn_hdr_surface_hide(self.raw) };
            self.shown = false;
            self.last = None;
        }
    }
}

impl Drop for HdrSurface {
    fn drop(&mut self) {
        unsafe { kn_hdr_surface_free(self.raw) };
    }
}

/// Une mire HDR (P010, PQ, BT.2020) de `w` × `h` : en haut une rampe de gris de 0 à 1 000 cd/m², en bas des barres
/// (blanc à 203 et à 1 000 cd/m², rouge, vert et bleu BT.2020 saturés à 203).
pub fn test_pattern(w: u32, h: u32) -> Frame {
    use crate::hdr::pq_oetf;
    let (w, h) = (w as usize & !1, h as usize & !1);
    let codes = |rgb: [f64; 3]| {
        let y = 0.2627 * rgb[0] + 0.6780 * rgb[1] + 0.0593 * rgb[2];
        let (cb, cr) = ((rgb[2] - y) / 1.8814, (rgb[0] - y) / 1.4746);
        let q = |v: f64| (v.round().clamp(0.0, 1023.0) as u16) << 6;
        (q(64.0 + 876.0 * y), q(512.0 + 896.0 * cb), q(512.0 + 896.0 * cr))
    };
    let bars = [[203.0, 203.0, 203.0], [1000.0, 1000.0, 1000.0], [203.0, 0.0, 0.0], [0.0, 203.0, 0.0], [0.0, 0.0, 203.0]];
    let pixel = |x: usize, y: usize| {
        let nits = if y < h / 2 { [1000.0 * x as f64 / (w - 1).max(1) as f64; 3] } else { bars[(x * bars.len() / w).min(bars.len() - 1)] };
        codes(nits.map(pq_oetf))
    };
    let mut y_plane = Vec::with_capacity(w * h * 2);
    let mut uv = Vec::with_capacity(w * h);
    for y in 0..h {
        for x in 0..w {
            y_plane.extend_from_slice(&pixel(x, y).0.to_le_bytes());
        }
    }
    for y in (0..h).step_by(2) {
        for x in (0..w).step_by(2) {
            let (_, cb, cr) = pixel(x, y);
            uv.extend_from_slice(&cb.to_le_bytes());
            uv.extend_from_slice(&cr.to_le_bytes());
        }
    }
    Frame {
        width: w as u32,
        height: h as u32,
        format: Format::P010,
        planes: [y_plane, uv, Vec::new()],
        pitch: [w * 2, w * 2, 0],
        pts: 0,
        received: 0,
        decoded: 0,
        bt709: false,
        full_range: false,
        pq: true,
        mastering_min: 0.0,
        mastering_max: 1000.0,
        max_cll: 1000.0,
        max_fall: 400.0,
    }
}

/// `kybernow --hdr-test` : une fenêtre SDL sur Wayland, la mire HDR sur sa sous-surface, une bande d'interface
/// semi-transparente par-dessus, pendant `KYBERNOW_HDR_TEST_SECONDS` secondes (20 par défaut). Sans gestion des
/// couleurs (GNOME 46), l'écran est pris pour un écran SDR : la mire passe par la courbe vers le SDR. Ce qui se passe
/// va au journal et à la sortie d'erreur ; l'œil juge l'image (la rampe sous la bande, les barres).
pub fn selftest() {
    sdl::hint("SDL_VIDEODRIVER", "wayland");
    sdl::hint("SDL_VIDEO_EGL_ALLOW_TRANSPARENCY", "1");
    let seconds: u64 = std::env::var("KYBERNOW_HDR_TEST_SECONDS").ok().and_then(|v| v.parse().ok()).unwrap_or(20);
    unsafe {
        if sdl::SDL_Init(sdl::SDL_INIT_VIDEO | sdl::SDL_INIT_EVENTS) != 0 {
            eprintln!("essai HDR : SDL ne démarre pas ({})", sdl::error());
            return;
        }
        sdl::SDL_GL_SetAttribute(sdl::SDL_GL_ALPHA_SIZE, 8);
    }
    eprintln!("essai HDR : SDL {}", sdl::video_driver());
    let title = std::ffi::CString::new("KyberNow Link · essai HDR").unwrap();
    let window = unsafe { sdl::SDL_CreateWindow(title.as_ptr(), sdl::SDL_WINDOWPOS_CENTERED, sdl::SDL_WINDOWPOS_CENTERED, 960, 540, sdl::SDL_WINDOW_SHOWN | sdl::SDL_WINDOW_RESIZABLE | sdl::SDL_WINDOW_ALLOW_HIGHDPI) };
    let renderer = if window.is_null() { std::ptr::null_mut() } else { unsafe { sdl::SDL_CreateRenderer(window, -1, sdl::SDL_RENDERER_ACCELERATED | sdl::SDL_RENDERER_PRESENTVSYNC) } };
    if renderer.is_null() {
        eprintln!("essai HDR : fenêtre impossible ({})", sdl::error());
        return;
    }
    let Some(mut surface) = HdrSurface::open(window, false) else {
        eprintln!("essai HDR : pas de sous-surface (voir le journal)");
        return;
    };
    eprintln!("essai HDR : écran {}", describe(&surface.preferred));
    let frame = test_pattern(1280, 720);
    let started = std::time::Instant::now();
    let mut ev = sdl::SDL_Event::default();
    let mut shown = 0u32;
    while started.elapsed().as_secs() < seconds {
        while unsafe { sdl::SDL_PollEvent(&mut ev) } != 0 {
            if u32::from_ne_bytes([ev.raw[0], ev.raw[1], ev.raw[2], ev.raw[3]]) == sdl::SDL_QUIT {
                return;
            }
        }
        if surface.poll() {
            eprintln!("essai HDR : l'écran est maintenant {}", describe(&surface.preferred));
        }
        let (mut ow, mut oh, mut ww, mut wh) = (0, 0, 0, 0);
        unsafe {
            sdl::SDL_GetRendererOutputSize(renderer, &mut ow, &mut oh);
            sdl::SDL_GetWindowSize(window, &mut ww, &mut wh);
        }
        let rect = super::video::fit((frame.width, frame.height), (ow, oh));
        let density = ow as f32 / ww.max(1) as f32;
        if !surface.present(&frame, rect, density, Luminance::default()) {
            eprintln!("essai HDR : la sous-surface a renoncé (voir le journal)");
            return;
        }
        shown += 1;
        unsafe {
            sdl::SDL_SetRenderDrawBlendMode(renderer, sdl::SDL_BLENDMODE_NONE);
            sdl::SDL_SetRenderDrawColor(renderer, 16, 20, 26, 255);
            sdl::SDL_RenderClear(renderer);
            sdl::SDL_SetRenderDrawColor(renderer, 0, 0, 0, 0);
            sdl::SDL_RenderFillRect(renderer, &rect);
            // Une bande d'interface par-dessus l'image, à moitié transparente, et un carré opaque.
            sdl::SDL_SetRenderDrawBlendMode(renderer, sdl::SDL_BLENDMODE_BLEND);
            sdl::SDL_SetRenderDrawColor(renderer, 9, 13, 18, 160);
            let band = sdl::SDL_Rect { x: 0, y: rect.y, w: ow, h: (64.0 * density) as c_int };
            sdl::SDL_RenderFillRect(renderer, &band);
            sdl::SDL_SetRenderDrawColor(renderer, 154, 140, 255, 255);
            let mark = sdl::SDL_Rect { x: (24.0 * density) as c_int, y: rect.y + (16.0 * density) as c_int, w: (32.0 * density) as c_int, h: (32.0 * density) as c_int };
            sdl::SDL_RenderFillRect(renderer, &mark);
            sdl::SDL_RenderPresent(renderer);
        }
    }
    eprintln!("essai HDR : {shown} images montrées en {seconds} s, sortie en {} bits", unsafe { kn_hdr_surface_bits(surface.raw) });
    drop(surface);
    unsafe {
        sdl::SDL_DestroyRenderer(renderer);
        sdl::SDL_DestroyWindow(window);
    }
}

/// « en HDR (crête 600 cd/m², blanc 203) » / « en SDR », pour le journal.
pub fn describe(d: &WlDescription) -> String {
    if d.is_hdr() {
        let l = d.luminance();
        format!("en HDR (crête {:.0} cd/m², tout l'écran {:.0}, noir {:.4}, blanc {:.0})", l.max, l.full_frame, l.min, l.sdr_white)
    } else {
        format!("en SDR (courbe {})", d.tf)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hdr::{pq_oetf, shade};

    /// Le shader (src/native/hdr_gl.c) contre `hdr::shade`, sur la carte graphique (EGL sans surface de Mesa) :
    /// les trois rendus, en P010 et en I010, à moins de deux niveaux sur 255. Sauté sans EGL utilisable.
    #[test]
    fn le_shader_suit_la_reference() {
        // Une image de 8 × 2 pixels par chroma (la même partout : le filtrage de la chroma ne change rien).
        let code = |n: f64| (64.0 + 876.0 * pq_oetf(n)).round() as u16;
        let ys = [64, code(1.0), code(100.0), code(203.0), code(600.0), code(1000.0), code(4000.0), 940];
        let (w, h) = (8usize, 2usize);
        let tones = [Render::Pq, Render::Eetf { src_max: 4000.0, dst_min: 0.0, dst_max: 1000.0 }, Render::Sdr { white: 203.0 }];
        for (cb, cr) in [(512u16, 512u16), (480, 600), (600, 420), (300, 700)] {
            for planar in [false, true] {
                let luma: Vec<u16> = (0..h).flat_map(|_| ys).collect();
                let n = w / 2 * h / 2;
                let (p0, p1, p2): (Vec<u8>, Vec<u8>, Vec<u8>) = if planar {
                    (luma.iter().flat_map(|v| v.to_le_bytes()).collect(), (0..n).flat_map(|_| cb.to_le_bytes()).collect(), (0..n).flat_map(|_| cr.to_le_bytes()).collect())
                } else {
                    (luma.iter().flat_map(|v| (v << 6).to_le_bytes()).collect(), (0..n).flat_map(|_| [(cb << 6).to_le_bytes(), (cr << 6).to_le_bytes()].concat()).collect(), Vec::new())
                };
                let planes = [p0.as_ptr(), p1.as_ptr(), if planar { p2.as_ptr() } else { std::ptr::null() }];
                for render in tones {
                    let t = tone(Tone::of(render));
                    let mut rgba = vec![0u8; w * h * 4];
                    let mut err = [0 as c_char; 256];
                    let r = unsafe { kn_hdr_offscreen(if planar { 5 } else { 4 }, w as c_int, h as c_int, planes.as_ptr(), &t, rgba.as_mut_ptr(), err.as_mut_ptr(), err.len() as c_int) };
                    let e = unsafe { CStr::from_ptr(err.as_ptr()) }.to_string_lossy().into_owned();
                    if r == -1 {
                        eprintln!("test du shader sauté : {e}");
                        return;
                    }
                    assert_eq!(r, 0, "{e}");
                    for (x, &yv) in ys.iter().enumerate() {
                        let want = shade(yv, cb, cr, &Tone::of(render)).map(|c| c * 255.0);
                        let got = &rgba[x * 4..x * 4 + 3];
                        for c in 0..3 {
                            assert!((f64::from(got[c]) - want[c]).abs() <= 2.0, "{render:?} {} ({cb}, {cr}) pixel {x} composante {c} : {} au lieu de {:.1}", if planar { "I010" } else { "P010" }, got[c], want[c]);
                        }
                    }
                }
            }
        }
    }
}
