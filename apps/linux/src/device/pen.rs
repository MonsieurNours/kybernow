//! Le stylet d'une tablette graphique (docs/link-ui.md, « Stylet » ;
//! docs/stylet.md), lu à côté de SDL par `src/native/pen.c` (XInput2 sous
//! X11, `tablet-v2` sous Wayland).
//!
//! Chaque état du stylet part à l'ordinateur par
//! [`kybernow_core::input::InputPipeline::pen_on`] ; une Station qui ne
//! connaît pas le stylet le reçoit en souris, comme avant :
//! - X11 : SDL reçoit déjà le stylet en souris (le pointeur du serveur X) ;
//!   tant que le stylet part en stylet, ces mouvements et ces clics ne vont
//!   pas à l'ordinateur ([`claimed`]), l'interface de l'app les reçoit ;
//! - Wayland : le compositeur n'envoie plus le stylet en souris à une app qui
//!   lit les tablettes ; l'app le rejoue en souris dans SDL
//!   ([`PEN_WHICH`]), pour son interface et, en souris, pour l'ordinateur.

use std::ffi::c_void;
use std::sync::atomic::{AtomicPtr, AtomicU64, Ordering};
use std::time::{Duration, Instant};

use super::sdl;
use kybernow_core::input::PenInput;

#[repr(C)]
#[derive(Clone, Copy, Default, Debug)]
pub struct PenEvent {
    pub window_id: u32,
    pub x: f32,
    pub y: f32,
    pub pressure: f32,
    pub tilt_x: f32,
    pub tilt_y: f32,
    pub rotation: f32,
    pub in_range: u8,
    pub contact: u8,
    pub eraser: u8,
    pub barrel: u8,
    pub has_tilt: u8,
    pub has_rotation: u8,
}

impl PenEvent {
    pub fn input(&self) -> PenInput {
        PenInput {
            in_range: self.in_range != 0,
            contact: self.contact != 0,
            pressure: self.pressure,
            tilt: (self.has_tilt != 0).then_some((self.tilt_x, self.tilt_y)),
            rotation: (self.has_rotation != 0).then_some(self.rotation),
            eraser: self.eraser != 0,
            barrel: self.barrel != 0,
        }
    }
}

unsafe extern "C" {
    fn kn_pen_open(window: *mut sdl::SDL_Window) -> *mut c_void;
    fn kn_pen_set_wake(p: *mut c_void, kind: u32);
    fn kn_pen_watch(p: *mut c_void, window: *mut sdl::SDL_Window, on: i32);
    fn kn_pen_poll(p: *mut c_void, out: *mut PenEvent) -> i32;
    fn kn_x_init_threads() -> i32;
}

/// Xlib sûr en plusieurs fils (stylet X11 : fil X et fil principal sur la même connexion) ; à appeler au
/// lancement, avant SDL_Init. Sans libX11 (Wayland seul, KMS/DRM), rien.
pub fn x_init_threads() {
    if std::env::var_os("DISPLAY").is_some() {
        unsafe { kn_x_init_threads() };
    }
}

/// `which` des événements de souris rejoués pour le stylet (Wayland).
pub const PEN_WHICH: u32 = 0xFFFF_FFFE;

static PEN: AtomicPtr<c_void> = AtomicPtr::new(std::ptr::null_mut());
static WAYLAND: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
/// Jusqu'à quand (en µs depuis [`epoch`]) le stylet tient la souris de l'ordinateur.
static CLAIMED_UNTIL: AtomicU64 = AtomicU64::new(0);

fn epoch() -> Instant {
    static E: std::sync::OnceLock<Instant> = std::sync::OnceLock::new();
    *E.get_or_init(Instant::now)
}

/// Ouvre la lecture du stylet, d'après la première fenêtre de l'app ; le
/// fil X11 réveille la boucle par `wake` (un événement SDL).
pub fn open(window: *mut sdl::SDL_Window, wake: u32) {
    // Dépannage : `KYBERNOW_NO_PEN=1` laisse le stylet à SDL (une souris).
    if std::env::var_os("KYBERNOW_NO_PEN").is_some_and(|v| v == "1") {
        log::info!("stylet : coupé (KYBERNOW_NO_PEN)");
        return;
    }
    let p = unsafe { kn_pen_open(window) };
    if p.is_null() {
        log::info!("stylet : pas de tablette lisible (ni X11 ni Wayland)");
        return;
    }
    WAYLAND.store(sdl_is_wayland(), Ordering::Relaxed);
    unsafe { kn_pen_set_wake(p, wake) };
    PEN.store(p, Ordering::SeqCst);
    log::info!("stylet : lu par {}", if WAYLAND.load(Ordering::Relaxed) { "Wayland (tablet-v2)" } else { "X11 (XInput2)" });
    watch(window, true);
}

fn sdl_is_wayland() -> bool {
    let name = unsafe { sdl::SDL_GetCurrentVideoDriver() };
    !name.is_null() && unsafe { std::ffi::CStr::from_ptr(name) }.to_bytes() == b"wayland"
}

/// Suit une fenêtre de l'app (créée), ou plus (détruite).
pub fn watch(window: *mut sdl::SDL_Window, on: bool) {
    let p = PEN.load(Ordering::SeqCst);
    if !p.is_null() && !window.is_null() {
        unsafe { kn_pen_watch(p, window, on as i32) };
    }
}

/// Les états du stylet arrivés depuis le dernier tour.
pub fn poll() -> Vec<PenEvent> {
    let p = PEN.load(Ordering::SeqCst);
    let mut out = Vec::new();
    if p.is_null() {
        return out;
    }
    let mut ev = PenEvent::default();
    while unsafe { kn_pen_poll(p, &mut ev) } != 0 {
        SEEN.store(true, Ordering::Relaxed);
        // Dans le journal : chaque approche et chaque départ du stylet.
        let near = ev.in_range != 0;
        if NEAR.swap(near, Ordering::Relaxed) != near {
            if near {
                log::info!(
                    "stylet : près de la fenêtre {} ({}{}{})",
                    ev.window_id,
                    if ev.eraser != 0 { "gomme" } else { "pointe" },
                    if ev.has_tilt != 0 { ", inclinaison" } else { "" },
                    if ev.has_rotation != 0 { ", rotation" } else { "" }
                );
            } else {
                log::info!("stylet : parti");
            }
        }
        if ev.contact != 0 && ev.in_range != 0 && !TOUCHED.swap(true, Ordering::Relaxed) {
            log::info!("stylet : premier contact (pression {:.2})", ev.pressure);
        }
        out.push(ev);
    }
    out
}

static NEAR: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
/// Un stylet s'est déjà approché d'une fenêtre de l'app depuis son lancement.
static SEEN: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// L'appareil a un stylet, pour la ligne « Stylet » du menu « Souris » (`Session::pen_choice`) : une tablette dont le
/// stylet s'est déjà approché d'une fenêtre de l'app depuis son lancement (l'app ne lit les tablettes qu'à travers
/// les états du stylet : une tablette branchée mais jamais approchée ne compte pas encore ; la session, elle, compte
/// le stylet dès qu'il a servi).
pub fn device_has_pen() -> bool {
    SEEN.load(Ordering::Relaxed)
}
static TOUCHED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Le compositeur n'envoie plus le stylet en souris : l'app le rejoue.
pub fn replays_mouse() -> bool {
    WAYLAND.load(Ordering::Relaxed)
}

/// Le stylet est parti en stylet : la souris que SDL en tire ne va pas à
/// l'ordinateur, tant qu'il est près (et un instant après, le temps que les
/// derniers mouvements du serveur X arrivent).
pub fn claim(in_range: bool) {
    let grace = if in_range { Duration::from_millis(600) } else { Duration::from_millis(100) };
    CLAIMED_UNTIL.store((epoch().elapsed() + grace).as_micros() as u64, Ordering::Relaxed);
}

/// Voir [`claim`].
pub fn claimed() -> bool {
    epoch().elapsed().as_micros() as u64 <= CLAIMED_UNTIL.load(Ordering::Relaxed)
}

/// Wayland : le stylet rejoué en souris dans SDL (mouvement, et le clic de
/// la pointe ou du bouton quand il change).
pub fn replay(ev: &PenEvent, was_contact: bool, was_barrel: bool) {
    let (x, y) = (ev.x.round() as i32, ev.y.round() as i32);
    let mut m = sdl::SDL_Event::default();
    m.raw[0..4].copy_from_slice(&sdl::SDL_MOUSEMOTION.to_ne_bytes());
    m.raw[8..12].copy_from_slice(&ev.window_id.to_ne_bytes());
    m.raw[12..16].copy_from_slice(&PEN_WHICH.to_ne_bytes());
    m.raw[20..24].copy_from_slice(&x.to_ne_bytes());
    m.raw[24..28].copy_from_slice(&y.to_ne_bytes());
    unsafe { sdl::SDL_PushEvent(&mut m) };
    let contact = ev.in_range != 0 && ev.contact != 0;
    let barrel = ev.in_range != 0 && ev.barrel != 0;
    for (now, was, button) in [(contact, was_contact, 1u8), (barrel, was_barrel, 3u8)] {
        if now == was {
            continue;
        }
        let mut b = sdl::SDL_Event::default();
        let kind = if now { sdl::SDL_MOUSEBUTTONDOWN } else { sdl::SDL_MOUSEBUTTONUP };
        b.raw[0..4].copy_from_slice(&kind.to_ne_bytes());
        b.raw[8..12].copy_from_slice(&ev.window_id.to_ne_bytes());
        b.raw[12..16].copy_from_slice(&PEN_WHICH.to_ne_bytes());
        b.raw[16] = button;
        b.raw[17] = now as u8;
        b.raw[18] = 1;
        b.raw[20..24].copy_from_slice(&x.to_ne_bytes());
        b.raw[24..28].copy_from_slice(&y.to_ne_bytes());
        unsafe { sdl::SDL_PushEvent(&mut b) };
    }
}

/// Ce qui est parti au dernier état : à l'ordinateur (près de la surface)
/// et, sous Wayland, rejoué en souris (pointe, bouton).
#[derive(Default)]
struct Last {
    sent_in_range: bool,
    replay_contact: bool,
    replay_barrel: bool,
}

static LAST: std::sync::Mutex<Last> = std::sync::Mutex::new(Last { sent_in_range: false, replay_contact: false, replay_barrel: false });

/// Envoie l'état à l'ordinateur (`false` : il part en souris). Un stylet
/// déjà parti n'est pas redit à chaque mouvement.
pub fn send(p: &kybernow_core::input::InputPipeline, screen: usize, x: i16, y: i16, input: &PenInput) -> bool {
    let mut last = LAST.lock().unwrap();
    if !input.in_range && !last.sent_in_range {
        return p.pen_available();
    }
    let sent = p.pen_on(Some(screen), x, y, input);
    last.sent_in_range = sent && input.in_range;
    sent
}

/// Wayland : rejoue l'état en souris dans SDL (voir [`replay`]).
pub fn replay_if_needed(ev: &PenEvent) {
    if !replays_mouse() {
        return;
    }
    let mut last = LAST.lock().unwrap();
    replay(ev, last.replay_contact, last.replay_barrel);
    last.replay_contact = ev.in_range != 0 && ev.contact != 0;
    last.replay_barrel = ev.in_range != 0 && ev.barrel != 0;
}
