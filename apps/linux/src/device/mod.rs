//! L'app sur Linux : la fenêtre SDL (bureau : sans bordure, barre de titre
//! dessinée par l'app ; console : plein écran sous gamescope), la boucle des
//! événements (clavier, souris, pavés tactiles, manettes), l'interface, les
//! tâches de fond et la session. Pendant la session, l'image de l'ordinateur
//! est une texture YUV ; l'interface (barre, menu, capsules) est posée
//! par-dessus, avec le pointeur de l'ordinateur.

mod jobs;
mod log_file;
mod media;
mod pads;
pub mod sdl;
mod session;
mod system;
mod video;

use std::ffi::{c_int, c_void, CString};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Sender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use kybernow_core::console::{DeviceInfo, Mode};
use kybernow_core::input::{GamepadRumble, MouseMode, MouseModeRules};
use kybernow_core::kynput_rs::MouseButtonType;
use kybernow_core::latency;
use kybernow_core::session::Status;

use crate::app::{Action, AddStep, App, Found, HostLive, Layout, Nav, Online, Phase, Screen, SessionUi, Stats, Target};
use crate::i18n::{self, tr};
use crate::model::{now_secs, LastSession, Store};
use crate::ui::{self, canvas::Canvas};

/// Ce qui revient à la boucle principale (fils de fond, rappels du cœur).
pub enum Event {
    HostInfo { id: String, online: bool, os: Option<String>, gpu: Option<String>, av1: bool },
    Preview { id: String, jpeg: Vec<u8> },
    Found(Vec<Found>),
    Probed { address: String, port: u16, fingerprint: Result<String, String> },
    PairAsked { name: String },
    PairAccepted(crate::model::Host),
    PairFailed(String),
    Status(Status),
    Mode(Mode),
    Second { latency_ms: Option<f64>, fps: f64, mbps: f64, lost: u32, codec: String, audio_ms: Option<f64> },
    Notice(String),
    Pointer { position: Option<(f64, f64)>, hidden: bool, shape: Option<(u32, u32, u32, u32, Vec<u32>)> },
    Rumble(GamepadRumble),
    Combo,
    InputReady,
    GameFullscreen(bool),
    DecoderFailed(String),
    Update(Result<Option<kybernow_core::update::Available>, String>),
}

/// Règles de la souris Auto : le jeu doit aussi cacher son pointeur (comme Mac et Windows).
const MOUSE_RULES: MouseModeRules = MouseModeRules { needs_hidden_pointer: true };

/// Zones de la fenêtre sans bordure (bureau), en coordonnées de la fenêtre,
/// lues par le test de SDL (déplacer par la barre de titre, bords pour redimensionner).
#[derive(Default)]
struct HitAreas {
    enabled: bool,
    drag: (i32, i32, i32, i32),
    size: (i32, i32),
}

static HIT: Mutex<HitAreas> = Mutex::new(HitAreas { enabled: false, drag: (0, 0, 0, 0), size: (0, 0) });

extern "C" fn hit_test(_win: *mut sdl::SDL_Window, p: *const sdl::SDL_Point, _data: *mut c_void) -> c_int {
    let Ok(h) = HIT.try_lock() else { return sdl::HITTEST_NORMAL };
    if !h.enabled {
        return sdl::HITTEST_NORMAL;
    }
    let (x, y) = unsafe { ((*p).x, (*p).y) };
    let (w, hh) = h.size;
    let b = 6;
    let (l, r, t, bo) = (x < b, x >= w - b, y < b, y >= hh - b);
    match (l, r, t, bo) {
        (true, _, true, _) => return sdl::HITTEST_RESIZE_TOPLEFT,
        (_, true, true, _) => return sdl::HITTEST_RESIZE_TOPRIGHT,
        (true, _, _, true) => return sdl::HITTEST_RESIZE_BOTTOMLEFT,
        (_, true, _, true) => return sdl::HITTEST_RESIZE_BOTTOMRIGHT,
        (true, _, _, _) => return sdl::HITTEST_RESIZE_LEFT,
        (_, true, _, _) => return sdl::HITTEST_RESIZE_RIGHT,
        (_, _, true, _) => return sdl::HITTEST_RESIZE_TOP,
        (_, _, _, true) => return sdl::HITTEST_RESIZE_BOTTOM,
        _ => {}
    }
    let (dx, dy, dw, dh) = h.drag;
    if x >= dx && x < dx + dw && y >= dy && y < dy + dh { sdl::HITTEST_DRAGGABLE } else { sdl::HITTEST_NORMAL }
}

struct Win {
    window: *mut sdl::SDL_Window,
    renderer: *mut sdl::SDL_Renderer,
    ui: *mut sdl::SDL_Texture,
    ui_size: (u32, u32),
    /// Le pointeur de l'ordinateur dessiné par l'app (souris des jeux, console).
    cursor: *mut sdl::SDL_Texture,
    cursor_size: (u32, u32, u32, u32),
    cursor_pixels: Vec<u32>,
    /// Le pointeur du système à la forme de celui de l'ordinateur (bureau).
    sys_cursor: *mut sdl::SDL_Cursor,
    screen: video::Screen,
    premul: c_int,
    vsync: bool,
    /// Taille de la fenêtre (coordonnées des événements) et de la sortie (pixels).
    win_size: (i32, i32),
    out_size: (i32, i32),
    /// Où l'image est posée dans la sortie (pixels).
    video_rect: Option<sdl::SDL_Rect>,
}

impl Win {
    fn sizes(&mut self) {
        unsafe {
            let (mut w, mut h) = (0, 0);
            sdl::SDL_GetWindowSize(self.window, &mut w, &mut h);
            self.win_size = (w.max(1), h.max(1));
            let (mut pw, mut ph) = (0, 0);
            sdl::SDL_GetRendererOutputSize(self.renderer, &mut pw, &mut ph);
            self.out_size = (pw.max(1), ph.max(1));
        }
    }

    /// Pixels réels par coordonnée de la fenêtre (écrans HiDPI).
    fn density(&self) -> f32 {
        self.out_size.0 as f32 / self.win_size.0 as f32
    }

    fn set_vsync(&mut self, on: bool) {
        if self.vsync != on {
            unsafe { sdl::SDL_RenderSetVSync(self.renderer, on as c_int) };
            self.vsync = on;
            log::info!("affichage : {}", if on { "synchronisé" } else { "immédiat (fréquence variable si l'écran la permet)" });
        }
    }
}

/// État de la session en cours.
struct Live {
    handle: session::Handle,
    /// L'ordinateur de la session (la dernière session y est enregistrée).
    host: String,
    pointer: Option<(f64, f64)>,
    pointer_hidden: bool,
    started: Instant,
    latencies: Vec<f64>,
    mbps: Vec<f64>,
    mode: String,
    game_fullscreen: bool,
    /// Souris choisie pour la session (barre, raccourci) ; sinon le réglage.
    mouse: MouseMode,
    /// Saisie de texte (clavier de Steam) : les caractères partent en Unicode.
    typing: bool,
    /// Échap tenu (long : rend la souris).
    escape_down: Option<Instant>,
    /// La barre du bureau doit se cacher à cet instant.
    bar_hide: Option<Instant>,
    decoded: Option<(u32, u32)>,
    last_frame: Option<Instant>,
    /// Image clé de la dernière image montrée (latence).
    shown: Option<(u64, i64, i64)>,
    /// Dernière image de la session, réduite (accueil).
    snapshot: Option<tiny_skia::Pixmap>,
    snapshot_at: Option<Instant>,
}

/// Coordonnées de la fenêtre → points de l'interface.
fn to_ui(win: &Win, cv: &Canvas, x: i32, y: i32) -> (f32, f32) {
    let d = win.density() / cv.k;
    (x as f32 * d, y as f32 * d)
}

/// L'échelle de l'interface : pixels par point.
fn ui_scale(layout: Layout, out: (i32, i32), density: f32) -> f32 {
    match layout {
        Layout::Desktop => density.max(1.0),
        Layout::Handheld => (out.1 as f32 / 800.0).min(out.0 as f32 / 1100.0).max(0.5),
        Layout::Tv => (out.1 as f32 / 1080.0).min(out.0 as f32 / 1700.0).max(0.5),
    }
}

fn device_info(app: &App) -> DeviceInfo {
    let kind = match app.layout {
        Layout::Desktop => "computer",
        Layout::Handheld => "handheld",
        Layout::Tv => "tv",
    };
    DeviceInfo {
        id: app.store.device_id.clone(),
        name: app.device_name.clone(),
        kind: kind.into(),
        os: app.machine.os.clone(),
        app: format!("KyberNow Link {} (Linux)", env!("CARGO_PKG_VERSION")),
    }
}

fn set_language(app: &App) {
    i18n::set_english(i18n::resolve(&app.store.app.language, &sdl::locale()));
    kybernow_core::i18n::set_current(if i18n::english() { kybernow_core::i18n::Lang::En } else { kybernow_core::i18n::Lang::Fr });
}

pub fn run(args: &[String]) {
    log_file::init();
    log::info!("KyberNow Link {} (Linux, {}) démarre", env!("CARGO_PKG_VERSION"), std::env::consts::ARCH);

    // Classe de la fenêtre = identifiant de l'app : rattachée à son icône et
    // à son fichier .desktop (variables lues par SDL à la création de la fenêtre).
    unsafe {
        std::env::set_var("SDL_VIDEO_X11_WMCLASS", crate::APP_ID);
        std::env::set_var("SDL_VIDEO_WAYLAND_WMCLASS", crate::APP_ID);
    }
    sdl::hint("SDL_APP_NAME", "KyberNow Link");
    sdl::hint("SDL_VIDEO_X11_WMCLASS", crate::APP_ID);
    sdl::hint("SDL_VIDEO_WAYLAND_WMCLASS", crate::APP_ID);
    sdl::hint("SDL_VIDEO_ALLOW_SCREENSAVER", "1");
    sdl::hint("SDL_RENDER_SCALE_QUALITY", "1");
    sdl::hint("SDL_MOUSE_FOCUS_CLICKTHROUGH", "1");
    sdl::hint("SDL_GRAB_KEYBOARD", "1");
    sdl::hint("SDL_JOYSTICK_ALLOW_BACKGROUND_EVENTS", "0");
    if let Ok(v) = std::env::var("KYBERNOW_VIDEO") {
        sdl::hint("SDL_VIDEODRIVER", &v);
    }
    unsafe {
        if sdl::SDL_Init(sdl::SDL_INIT_VIDEO | sdl::SDL_INIT_AUDIO | sdl::SDL_INIT_GAMECONTROLLER | sdl::SDL_INIT_EVENTS) != 0 {
            log::error!("SDL : {}", sdl::error());
            eprintln!("KyberNow Link : SDL ne démarre pas ({})", sdl::error());
            return;
        }
    }
    log::info!("SDL : vidéo {}, son {}", sdl::video_driver(), sdl::audio_driver());

    let store = Store::load();
    let ui_arg = args.iter().position(|a| a == "--ui").and_then(|i| args.get(i + 1)).map(String::as_str);
    let display = unsafe {
        let mut m = sdl::SDL_DisplayMode::default();
        if sdl::SDL_GetCurrentDisplayMode(0, &mut m) == 0 { (m.w.max(1) as u32, m.h.max(1) as u32) } else { (1280, 800) }
    };
    let layout = system::layout(&store.app.interface, ui_arg, display);
    log::info!("interface : {layout:?} (écran {} × {}, gamescope {})", display.0, display.1, system::gamescope());

    // La fenêtre.
    let title = CString::new("KyberNow Link").unwrap();
    let (ww, wh, flags) = match layout {
        Layout::Desktop => {
            let w = 1280.min((display.0 as f32 * 0.9) as i32);
            let h = 800.min((display.1 as f32 * 0.9) as i32);
            (w, h, sdl::SDL_WINDOW_SHOWN | sdl::SDL_WINDOW_RESIZABLE | sdl::SDL_WINDOW_BORDERLESS | sdl::SDL_WINDOW_ALLOW_HIGHDPI)
        }
        _ => (display.0 as i32, display.1 as i32, sdl::SDL_WINDOW_SHOWN | sdl::SDL_WINDOW_FULLSCREEN_DESKTOP | sdl::SDL_WINDOW_ALLOW_HIGHDPI),
    };
    let window = unsafe { sdl::SDL_CreateWindow(title.as_ptr(), sdl::SDL_WINDOWPOS_CENTERED, sdl::SDL_WINDOWPOS_CENTERED, ww, wh, flags) };
    if window.is_null() {
        log::error!("SDL : fenêtre impossible ({})", sdl::error());
        return;
    }
    let renderer = unsafe { sdl::SDL_CreateRenderer(window, -1, sdl::SDL_RENDERER_ACCELERATED | sdl::SDL_RENDERER_PRESENTVSYNC) };
    if renderer.is_null() {
        log::error!("SDL : rendu impossible ({})", sdl::error());
        return;
    }
    unsafe {
        let mut info: sdl::SDL_RendererInfo = std::mem::zeroed();
        if sdl::SDL_GetRendererInfo(renderer, &mut info) == 0 && !info.name.is_null() {
            log::info!("rendu : {}", std::ffi::CStr::from_ptr(info.name).to_string_lossy());
        }
        sdl::SDL_SetWindowMinimumSize(window, 960, 600);
        sdl::SDL_SetWindowHitTest(window, Some(hit_test), std::ptr::null_mut());
    }
    // Interface en RGBA prémultiplié (tiny-skia) : source ONE, destination 1 − alpha.
    // SDL_BlendFactor : 2 = ONE, 6 = ONE_MINUS_SRC_ALPHA ; SDL_BlendOperation : 1 = ADD.
    let premul = unsafe { sdl::SDL_ComposeCustomBlendMode(2, 6, 1, 2, 6, 1) };
    let mut win = Win {
        window,
        renderer,
        ui: std::ptr::null_mut(),
        ui_size: (0, 0),
        cursor: std::ptr::null_mut(),
        cursor_size: (0, 0, 0, 0),
        cursor_pixels: Vec::new(),
        sys_cursor: std::ptr::null_mut(),
        screen: video::Screen::default(),
        premul,
        vsync: true,
        win_size: (ww, wh),
        out_size: (ww, wh),
        video_rect: None,
    };
    win.sizes();
    video::FRAME_EVENT.store(unsafe { sdl::SDL_RegisterEvents(1) }, Ordering::Relaxed);

    // L'app.
    let mut app = App::new(store, layout);
    set_language(&app);
    app.machine = system::machine(window);
    app.device_name = system::device_name(&app.machine);
    app.net = system::net();
    app.reduce_motion = system::reduce_motion();
    app.launched = Instant::now();
    for h in app.store.hosts.clone() {
        let last = Store::image_path(&h.id).with_extension("png");
        if app.store.app.keep_last_image
            && let Some(img) = std::fs::read(&last).ok().and_then(|b| ui::canvas::decode_png(&b))
        {
            app.images.insert(h.id.clone(), img);
            app.image_is_last.insert(h.id.clone(), true);
        } else if let Some(img) = std::fs::read(Store::image_path(&h.id)).ok().and_then(|b| ui::canvas::decode_jpeg(&b)) {
            app.images.insert(h.id.clone(), img);
            app.image_is_last.insert(h.id.clone(), false);
        }
    }
    if !layout.console() {
        app.go(Screen::Home);
    }

    let (tx, rx) = channel::<Event>();
    jobs::discover(tx.clone());
    let mut last_update = Instant::now();
    app.update = crate::app::UpdateState::Checking;
    jobs::check_update(app.store.app.beta, tx.clone());
    let mut pads = pads::Pads::default();
    let mut cv = Canvas::new();
    let mut live: Option<Live> = None;
    let mut pairing: Vec<Arc<AtomicBool>> = Vec::new();
    let mut last_poll = Instant::now() - Duration::from_secs(60);
    let mut last_net = Instant::now();
    let mut last_clip = Instant::now();
    let mut dirty = true;
    let mut present = true;
    let mut quit = false;

    while !quit {
        let streaming = live.is_some() && app.session.as_ref().is_some_and(|s| matches!(s.phase, Phase::Streaming | Phase::Reconnecting { .. }));
        // Animations : lancement, passage, capsules, ajout, écran des manettes.
        let animating = app.screen == Screen::Launch
            || app.session.as_ref().is_some_and(|s| matches!(s.phase, Phase::Preparing | Phase::Connecting | Phase::FirstImages | Phase::Ending) || ui::passage_opening(s))
            || app.toast.as_ref().is_some_and(|(_, t)| t.elapsed().as_secs_f32() < 4.2)
            || app.session.as_ref().and_then(|s| s.notice.as_ref()).is_some_and(|(_, t)| t.elapsed().as_secs_f32() < 4.2)
            || app.screen == Screen::Gamepads
            || app.screen == Screen::Add
            || pads.mouse_active();
        let timeout = if animating { 16 } else if streaming { 100 } else { 50 };
        let mut ev = sdl::SDL_Event::default();
        let mut actions: Vec<Action> = Vec::new();
        if unsafe { sdl::SDL_WaitEventTimeout(&mut ev, timeout) } != 0 {
            loop {
                handle_sdl(&ev, &mut app, &mut pads, live.as_mut(), &mut win, &cv, &mut actions, &mut dirty, &mut present, &mut quit);
                if unsafe { sdl::SDL_PollEvent(&mut ev) } == 0 {
                    break;
                }
            }
        }
        // Manette en souris : déplacement continu.
        if let Some(l) = live.as_ref()
            && app.session.as_ref().is_some_and(|s| s.pad_mouse && !s.menu)
            && pads.mouse_step(&l.handle.shared)
        {
            present = true;
        }
        if let Some(n) = pads.repeat_nav() {
            actions.extend(app.nav(n));
            dirty = true;
        }
        while let Ok(e) = rx.try_recv() {
            if matches!(e, Event::Pointer { .. }) {
                present = true;
            } else {
                dirty = true;
            }
            handle_event(e, &mut app, &mut pads, &mut live, &mut win, &mut actions, &mut pairing);
        }
        for a in actions {
            perform(a, &mut app, &mut pads, &mut live, &mut win, &tx, &mut pairing, &mut quit);
            dirty = true;
        }

        // Lancement fini : l'accueil.
        if app.screen == Screen::Launch && app.launched.elapsed().as_secs_f32() >= if app.reduce_motion { ui::LAUNCH_REDUCED } else { ui::LAUNCH } {
            app.launch_done();
            dirty = true;
        }

        // Fin de session : l'accueil, la dernière session enregistrée.
        if live.is_some() && matches!(app.session.as_ref().map(|s| &s.phase), Some(Phase::Ended { .. }) | None) {
            let l = live.take().unwrap();
            finish(&mut app, &mut win, l);
            last_poll = Instant::now() - Duration::from_secs(60);
            dirty = true;
        }

        if let Some(l) = live.as_mut() {
            session_tick(&mut app, l, &mut win, &mut dirty, &mut present);
            // Presse-papiers : le texte du système, et ce que l'ordinateur envoie.
            if let Some(c) = &l.handle.shared.clipboard {
                if let Some(t) = c.outgoing.lock().unwrap().take() {
                    sdl::set_clipboard(&t);
                }
                if last_clip.elapsed() > Duration::from_millis(500) {
                    last_clip = Instant::now();
                    c.system_text(sdl::clipboard());
                }
            }
        }

        // Sondage d'Admin toutes les 8 s, hors session ; le réseau et la batterie toutes les 5 s.
        if live.is_none() && last_poll.elapsed() > Duration::from_secs(8) && matches!(app.screen, Screen::Home | Screen::Settings | Screen::Launch) {
            last_poll = Instant::now();
            for h in &app.store.hosts {
                jobs::poll(h.clone(), !app.images.contains_key(&h.id), tx.clone());
            }
        }
        if live.is_none() && last_update.elapsed() > Duration::from_secs(6 * 3600) {
            last_update = Instant::now();
            jobs::check_update(app.store.app.beta, tx.clone());
        }
        if last_net.elapsed() > Duration::from_secs(5) && live.is_none() {
            last_net = Instant::now();
            let n = system::net();
            let b = sdl::power();
            if n != app.net || b != app.machine.battery {
                app.net = n;
                app.machine.battery = b;
                dirty = true;
            }
        }

        // Une image décodée attend : elle part à la texture.
        let mut new_frame = None;
        if let Some(l) = live.as_mut()
            && let Some(f) = l.handle.shared.latest.take()
        {
            // Une image toutes les 2 s, réduite, pour l'accueil (dernière image de la session).
            if l.snapshot_at.is_none_or(|t| t.elapsed() > Duration::from_secs(2)) {
                l.snapshot_at = Some(Instant::now());
                l.snapshot = video::snapshot(&f, 640);
            }
            if win.screen.upload(win.renderer, &f) {
                if l.decoded != Some((f.width, f.height)) {
                    l.decoded = Some((f.width, f.height));
                    l.handle.session.decoded_size(f.width, f.height);
                }
                new_frame = Some((f.pts, f.received, f.decoded));
                present = true;
            }
            l.handle.shared.latest.give_back(f);
        }

        if animating {
            dirty = true;
        }
        let show_ui = app.screen != Screen::Session || ui::session_overlay_visible(&app);
        if dirty {
            win.sizes();
            let k = ui_scale(app.layout, win.out_size, win.density());
            cv.resize(win.out_size.0 as u32, win.out_size.1 as u32, k);
            if show_ui {
                app.pads = pads.infos();
                app.hits = ui::draw(&app, &mut cv);
                upload_ui(&mut win, &cv);
            } else {
                app.hits.clear();
            }
            update_hit_areas(&app, &win, &cv);
            dirty = false;
            present = true;
        }
        if present {
            draw_frame(&mut win, show_ui, live.as_ref(), &app);
            present = false;
            if let (Some((pts, received, decoded)), Some(l)) = (new_frame, live.as_mut()) {
                let at = latency::now_micros();
                l.handle.session.frame_displayed(pts, received, decoded, at);
                l.last_frame = Some(Instant::now());
                l.shown = Some((pts, received, decoded));
            }
        }
    }

    if let Some(l) = live.take() {
        l.handle.session.stop();
    }
    for c in pairing.drain(..) {
        c.store(true, Ordering::Relaxed);
    }
    app.store.save();
    unsafe { sdl::SDL_Quit() };
    log::info!("KyberNow Link s'arrête");
}

fn upload_ui(win: &mut Win, cv: &Canvas) {
    let (w, h) = (cv.pix.width(), cv.pix.height());
    unsafe {
        if win.ui.is_null() || win.ui_size != (w, h) {
            if !win.ui.is_null() {
                sdl::SDL_DestroyTexture(win.ui);
            }
            win.ui = sdl::SDL_CreateTexture(win.renderer, sdl::SDL_PIXELFORMAT_ABGR8888, sdl::SDL_TEXTUREACCESS_STREAMING, w as c_int, h as c_int);
            if win.ui.is_null() {
                log::error!("interface : texture {w} × {h} impossible ({})", sdl::error());
                return;
            }
            // Le mélange prémultiplié ; refusé par le pilote : mélange ordinaire.
            if sdl::SDL_SetTextureBlendMode(win.ui, win.premul) != 0 {
                log::warn!("interface : mélange prémultiplié refusé ({}), mélange ordinaire", sdl::error());
                win.premul = sdl::SDL_BLENDMODE_BLEND;
                sdl::SDL_SetTextureBlendMode(win.ui, win.premul);
            }
            win.ui_size = (w, h);
            log::info!("interface : texture {w} × {h}");
        }
        if sdl::SDL_UpdateTexture(win.ui, std::ptr::null(), cv.bytes().as_ptr() as *const c_void, (w * 4) as c_int) != 0 {
            log::warn!("interface : envoi de la texture impossible ({})", sdl::error());
        }
    }
}

fn update_hit_areas(app: &App, win: &Win, cv: &Canvas) {
    let mut h = HIT.lock().unwrap();
    h.size = win.win_size;
    h.enabled = app.layout == Layout::Desktop && !app.fullscreen && !app.maximized_or_session();
    if let Some(r) = ui::title_drag_area(app, cv.w) {
        let d = cv.k / win.density();
        h.drag = ((r.x * d) as i32, (r.y * d) as i32, (r.w * d) as i32, (r.h * d) as i32);
    } else {
        h.drag = (0, 0, 0, 0);
    }
}

impl App {
    /// Pendant la session (hors menus), la fenêtre ne se déplace pas par l'image.
    fn maximized_or_session(&self) -> bool {
        self.screen == Screen::Session
    }
}

/// Compose la fenêtre : l'image de l'ordinateur pendant la session,
/// l'interface si elle est montrée, le pointeur de l'ordinateur.
fn draw_frame(win: &mut Win, show_ui: bool, live: Option<&Live>, app: &App) {
    unsafe {
        sdl::SDL_SetRenderDrawColor(win.renderer, 0, 0, 0, 255);
        sdl::SDL_RenderClear(win.renderer);
        let in_session = app.screen == Screen::Session && app.session.as_ref().is_some_and(|s| matches!(s.phase, Phase::Streaming | Phase::Reconnecting { .. } | Phase::Ending));
        win.video_rect = if in_session { win.screen.draw(win.renderer, win.out_size) } else { None };
        if show_ui && !win.ui.is_null() {
            sdl::SDL_RenderCopy(win.renderer, win.ui, std::ptr::null(), std::ptr::null());
        }
        // Le pointeur de l'ordinateur, dessiné par l'app : souris des jeux
        // (menus d'un jeu), console (pavés tactiles, manette en souris).
        let s = app.session.as_ref();
        let draw_cursor = s.is_some_and(|s| !s.menu && !s.keys && s.phase == Phase::Streaming && (s.relative || s.pad_mouse || app.layout.console()));
        if let (true, Some(l), Some(dst)) = (draw_cursor, live, win.video_rect)
            && !win.cursor.is_null()
            && (!l.pointer_hidden || s.is_some_and(|s| s.pad_mouse))
            && let Some((x, y)) = l.pointer
        {
            let (hw, hh) = l.handle.shared.host_size();
            let (cw, ch, hx, hy) = win.cursor_size;
            let sx = dst.w as f64 / hw.max(1) as f64;
            let sy = dst.h as f64 / hh.max(1) as f64;
            let r = sdl::SDL_Rect {
                x: dst.x + ((x - hx as f64) * sx) as i32,
                y: dst.y + ((y - hy as f64) * sy) as i32,
                w: ((cw as f64 * sx).max(12.0)) as i32,
                h: ((ch as f64 * sy).max(12.0)) as i32,
            };
            sdl::SDL_RenderCopy(win.renderer, win.cursor, std::ptr::null(), &r);
        }
        sdl::SDL_RenderPresent(win.renderer);
    }
}

/// Ce qui se règle à chaque tour pendant la session : souris des jeux ou du
/// bureau, barre du bureau, curseur du système, image absente.
fn session_tick(app: &mut App, l: &mut Live, win: &mut Win, dirty: &mut bool, present: &mut bool) {
    let Some(s) = app.session.as_mut() else { return };
    let streaming = matches!(s.phase, Phase::Streaming);
    // Souris : Auto suit le jeu en plein écran et le pointeur caché.
    let state = l.handle.shared.pointer.state();
    let visible_for = {
        use kybernow_core::input::Clock;
        state.visible_for(l.handle.shared.clock.now())
    };
    let current = s.relative;
    let wanted = streaming && !s.menu && !s.keys && !s.bar_menu.is_some() && MOUSE_RULES.wants_relative(l.mouse, l.game_fullscreen, state.remote_hidden, visible_for, current);
    if wanted != current {
        s.relative = wanted;
        l.handle.shared.relative.store(wanted, Ordering::Relaxed);
        if let Some(p) = l.handle.shared.current_input() {
            p.set_relative(wanted);
        }
        unsafe { sdl::SDL_SetRelativeMouseMode(wanted as c_int) };
        log::info!("souris : {}", if wanted { "jeu (relative)" } else { "bureau" });
        if wanted && app.layout == Layout::Desktop {
            s.notice = Some((tr("Souris des jeux · Échap longtemps ou Ctrl+Alt+Maj+M pour rendre la souris", "Game mouse · hold Esc or Ctrl+Alt+Shift+M to release the mouse").into(), Instant::now()));
        }
        *dirty = true;
    }
    // Pointeur du système : forme de l'ordinateur (bureau, souris du bureau),
    // caché ailleurs (l'app dessine celui de l'ordinateur).
    // Bureau, souris du bureau : toujours visible. Windows masque son pointeur
    // par intermittence (sans souris branchée, à l'arrêt) ; le suivre ferait
    // perdre le curseur au repos.
    let show_sys = !streaming || s.menu || s.keys || s.bar || (app.layout == Layout::Desktop && !s.relative);
    unsafe {
        sdl::SDL_ShowCursor(show_sys as c_int);
        if !win.sys_cursor.is_null() && streaming && !s.menu && !s.keys && !s.bar {
            sdl::SDL_SetCursor(win.sys_cursor);
        } else {
            sdl::SDL_SetCursor(sdl::SDL_GetDefaultCursor());
        }
    }
    // Barre du bureau : cachée quand le pointeur l'a quittée depuis 400 ms.
    if let Some(at) = l.bar_hide
        && Instant::now() >= at
    {
        l.bar_hide = None;
        if s.bar_menu.is_none() {
            s.bar = false;
            *dirty = true;
        }
    }
    // Échap tenu une seconde : la souris des jeux est rendue.
    if let Some(t) = l.escape_down
        && t.elapsed() > Duration::from_millis(1000)
        && s.relative
    {
        l.escape_down = None;
        l.mouse = MouseMode::Desktop;
        s.notice = Some((tr("Souris du bureau", "Desktop mouse").into(), Instant::now()));
        *dirty = true;
    }
    let _ = present;
}

fn host_point(win: &Win, l: &Live, x: i32, y: i32) -> Option<(i16, i16)> {
    let dst = win.video_rect?;
    let d = win.density();
    let (px, py) = (x as f32 * d, y as f32 * d);
    let (hw, hh) = l.handle.shared.host_size();
    let hx = ((px - dst.x as f32) / dst.w.max(1) as f32 * hw as f32).clamp(0.0, hw as f32 - 1.0);
    let hy = ((py - dst.y as f32) / dst.h.max(1) as f32 * hh as f32).clamp(0.0, hh as f32 - 1.0);
    Some((hx as i16, hy as i16))
}

#[allow(clippy::too_many_arguments)]
fn handle_sdl(ev: &sdl::SDL_Event, app: &mut App, pads: &mut pads::Pads, live: Option<&mut Live>, win: &mut Win, cv: &Canvas, actions: &mut Vec<Action>, dirty: &mut bool, present: &mut bool, quit: &mut bool) {
    let kind = ev.kind();
    if kind == video::FRAME_EVENT.load(Ordering::Relaxed) {
        return;
    }
    let in_game = app.screen == Screen::Session && app.session.as_ref().is_some_and(|s| !s.menu && !s.keys && matches!(s.phase, Phase::Streaming | Phase::Reconnecting { .. }));
    let input = live.as_ref().and_then(|l| l.handle.shared.current_input());
    match kind {
        sdl::SDL_QUIT => {
            if live.is_some() {
                actions.push(Action::Disconnect);
            }
            *quit = true;
        }
        sdl::SDL_WINDOWEVENT => {
            let (e, _, _) = ev.window();
            match e {
                sdl::SDL_WINDOWEVENT_FOCUS_LOST => {
                    if let Some(p) = &input {
                        p.release_all();
                    }
                }
                sdl::SDL_WINDOWEVENT_LEAVE => {
                    if app.layout == Layout::Desktop {
                        app.pointer = None;
                    }
                }
                sdl::SDL_WINDOWEVENT_MAXIMIZED => app.maximized = true,
                sdl::SDL_WINDOWEVENT_RESTORED => app.maximized = false,
                _ => {}
            }
            *dirty = true;
        }
        sdl::SDL_RENDER_TARGETS_RESET | sdl::SDL_RENDER_DEVICE_RESET => {
            win.ui_size = (0, 0);
            win.screen.reset();
            *dirty = true;
        }
        sdl::SDL_KEYDOWN | sdl::SDL_KEYUP => {
            let down = kind == sdl::SDL_KEYDOWN;
            let (sc, repeat, mods) = ev.key();
            let app_combo = mods & sdl::KMOD_CTRL != 0 && mods & sdl::KMOD_ALT != 0 && mods & sdl::KMOD_SHIFT != 0;
            // Raccourcis de l'app : Ctrl+Alt+Maj + D (se déconnecter), F (plein écran), M (souris).
            if app_combo && matches!(sc, sdl::SC_D | sdl::SC_F | sdl::SC_M) {
                if down && !repeat {
                    match sc {
                        sdl::SC_D if live.is_some() => actions.push(Action::Disconnect),
                        sdl::SC_F => actions.push(Action::ToggleFullscreen),
                        sdl::SC_M if live.is_some() => actions.push(Action::ToggleMouse),
                        _ => {}
                    }
                    if let Some(p) = &input {
                        p.release_all();
                    }
                }
                return;
            }
            if in_game {
                let Some(l) = live else { return };
                if sc == sdl::SC_ESCAPE {
                    l.escape_down = if down { Some(l.escape_down.unwrap_or_else(Instant::now)) } else { None };
                }
                let Some(p) = input else { return };
                // Saisie au clavier de Steam : les caractères partent en texte.
                if l.typing && !matches!(sc, sdl::SC_RETURN | sdl::SC_BACKSPACE | sdl::SC_ESCAPE | sdl::SC_TAB | sdl::SC_UP | sdl::SC_DOWN | sdl::SC_LEFT | sdl::SC_RIGHT | sdl::SC_KP_ENTER) {
                    return;
                }
                if !repeat && let Some(w) = pads::win_scancode(sc) {
                    p.key(w, down);
                }
                return;
            }
            if !down {
                return;
            }
            let nav = match sc {
                sdl::SC_UP => Some(Nav::Up),
                sdl::SC_DOWN => Some(Nav::Down),
                sdl::SC_LEFT => Some(Nav::Left),
                sdl::SC_RIGHT => Some(Nav::Right),
                sdl::SC_RETURN | sdl::SC_KP_ENTER => Some(Nav::Ok),
                sdl::SC_ESCAPE | sdl::SC_AC_BACK => Some(Nav::Back),
                sdl::SC_BACKSPACE => Some(Nav::Erase),
                sdl::SC_TAB => Some(if mods & sdl::KMOD_SHIFT != 0 { Nav::Left } else { Nav::Right }),
                sdl::SC_F11 => {
                    actions.push(Action::ToggleFullscreen);
                    None
                }
                _ => None,
            };
            if let Some(n) = nav {
                actions.extend(app.nav(n));
                *dirty = true;
            }
        }
        sdl::SDL_TEXTINPUT => {
            let text = ev.text();
            if in_game {
                if let (Some(l), Some(p)) = (live, &input)
                    && l.typing
                {
                    p.type_text(&text);
                }
                return;
            }
            if app.screen == Screen::Add && app.add.step == AddStep::Address {
                for c in text.chars() {
                    if c.is_ascii_alphanumeric() || matches!(c, '.' | ':' | '-' | '[' | ']') {
                        actions.extend(app.nav(Nav::Char(c)));
                    }
                }
                *dirty = true;
            }
        }
        sdl::SDL_MOUSEMOTION => {
            let (x, y, xr, yr, _) = ev.motion();
            if in_game {
                let Some(l) = live else { return };
                let s = app.session.as_mut().unwrap();
                if s.relative {
                    if let Some(p) = &input {
                        p.mouse_move(xr.clamp(-32768, 32767) as i16, yr.clamp(-32768, 32767) as i16);
                    }
                    return;
                }
                // Barre du bureau : le pointeur touche le haut de la fenêtre.
                if app.layout == Layout::Desktop {
                    let (ux, uy) = to_ui(win, cv, x, y);
                    if uy < 6.0 && !s.bar {
                        s.bar = true;
                        l.bar_hide = None;
                        *dirty = true;
                    }
                    if s.bar {
                        let over = app.hits.iter().any(|(_, r)| r.contains(ux, uy)) || uy < 58.0 || s.bar_menu.is_some();
                        if over {
                            l.bar_hide = None;
                            if app.pointer_moved(ux, uy) {
                                *dirty = true;
                            }
                            return;
                        } else if l.bar_hide.is_none() {
                            l.bar_hide = Some(Instant::now() + Duration::from_millis(400));
                        }
                    }
                }
                if let (Some(p), Some((hx, hy))) = (&input, host_point(win, l, x, y)) {
                    p.mouse(hx, hy);
                    l.pointer = Some((hx as f64, hy as f64));
                    *present = true;
                }
                return;
            }
            let (ux, uy) = to_ui(win, cv, x, y);
            if app.pointer_moved(ux, uy) {
                *dirty = true;
            }
        }
        sdl::SDL_MOUSEBUTTONDOWN | sdl::SDL_MOUSEBUTTONUP => {
            let down = kind == sdl::SDL_MOUSEBUTTONDOWN;
            let (b, x, y, which) = ev.button();
            let touch = which == u32::MAX;
            if in_game {
                let s = app.session.as_ref().unwrap();
                let (ux, uy) = to_ui(win, cv, x, y);
                // Un toucher en haut de l'écran (Deck) ouvre le menu.
                if touch && down && app.layout.console() && uy < cv.h * 0.06 {
                    actions.extend(app.open_menu());
                    *dirty = true;
                    return;
                }
                if s.bar && app.hits.iter().any(|(_, r)| r.contains(ux, uy)) {
                    if down && b == 1 {
                        actions.extend(app.pointer_click(ux, uy));
                        *dirty = true;
                    }
                    return;
                }
                if s.bar_menu.is_some() && down {
                    if let Some(s) = app.session.as_mut() {
                        s.bar_menu = None;
                    }
                    *dirty = true;
                    return;
                }
                if let Some(p) = input {
                    if !s.relative
                        && let Some(l) = live
                        && let Some((hx, hy)) = host_point(win, l, x, y)
                    {
                        p.mouse(hx, hy);
                    }
                    let button = match b {
                        3 => MouseButtonType::Right,
                        2 => MouseButtonType::Middle,
                        4 => MouseButtonType::Side,
                        5 => MouseButtonType::Extra,
                        _ => MouseButtonType::Left,
                    };
                    p.mouse_button(button, down);
                }
                return;
            }
            if down && b == 1 {
                let (ux, uy) = to_ui(win, cv, x, y);
                // Double clic sur la barre de titre : agrandir.
                if ev.u8(18) == 2 && ui::title_drag_area(app, cv.w).is_some_and(|r| r.contains(ux, uy)) {
                    actions.push(Action::Maximize);
                    return;
                }
                app.pointer = Some((ux, uy));
                actions.extend(app.pointer_click(ux, uy));
                *dirty = true;
            }
        }
        sdl::SDL_MOUSEWHEEL => {
            let (wx, wy) = ev.wheel();
            if in_game {
                if let Some(p) = input {
                    p.wheel(-wx, -wy);
                }
                return;
            }
            if app.screen == Screen::Settings && app.layout == Layout::Desktop {
                app.wheel(wy);
            } else if wy != 0.0 {
                actions.extend(app.nav(if wy > 0.0 { Nav::Up } else { Nav::Down }));
            }
            *dirty = true;
        }
        sdl::SDL_CONTROLLERDEVICEADDED => {
            let (index, _, _) = ev.pad();
            if let Some(slot) = pads.add(index) {
                if let Some(p) = &input {
                    let (v, pr) = pads.ids(slot);
                    p.gamepad_plug(slot as u32, Some(v), Some(pr));
                }
                if let Some(s) = app.session.as_mut() {
                    s.notice = Some((format!("{} {}{} : {}", tr("Manette", "Controller"), tr("J", "P"), slot + 1, pads.name(slot)), Instant::now()));
                }
                *dirty = true;
            }
        }
        sdl::SDL_CONTROLLERDEVICEREMOVED => {
            let (instance, _, _) = ev.pad();
            if let Some(slot) = pads.remove(instance) {
                if let Some(p) = &input {
                    p.gamepad_unplug(slot as u32);
                }
                *dirty = true;
            }
        }
        sdl::SDL_CONTROLLERBUTTONDOWN | sdl::SDL_CONTROLLERBUTTONUP | sdl::SDL_CONTROLLERAXISMOTION => {
            let (instance, which, value) = ev.pad();
            let Some(slot) = pads.slot_of(instance) else { return };
            let is_button = kind != sdl::SDL_CONTROLLERAXISMOTION;
            let down = kind == sdl::SDL_CONTROLLERBUTTONDOWN;
            pads.update(slot, is_button, which, down, value);
            if app.screen == Screen::Gamepads {
                *dirty = true;
            }
            if app.screen == Screen::Launch && is_button && down {
                app.launch_done();
                *dirty = true;
                return;
            }
            let s = app.session.as_ref();
            if in_game && s.is_some_and(|s| s.pad_mouse) {
                if let Some(p) = &input {
                    pads.mouse_input(p, is_button, which, down, value);
                    if is_button {
                        p.gamepad_button(slot as u32, pads::button(which), down);
                    }
                }
                return;
            }
            if in_game {
                if let Some(p) = &input {
                    if is_button {
                        if let Some(b) = pads::core_button(which) {
                            p.gamepad_button(slot as u32, b, down);
                        }
                    } else if let Some(a) = pads::core_axis(which) {
                        p.gamepad_axis(slot as u32, a, value);
                    }
                }
                return;
            }
            // Hors jeu : les relâchements vont quand même au suivi de la combinaison.
            if is_button
                && !down
                && let Some(p) = &input
                && let Some(b) = pads::core_button(which)
            {
                p.combo().button(slot as u32, b, false);
            }
            if let Some(n) = pads.nav(is_button, which, down, value) {
                actions.extend(app.nav(n));
                *dirty = true;
            }
        }
        _ => {}
    }
}

#[allow(clippy::too_many_arguments)]
fn handle_event(e: Event, app: &mut App, pads: &mut pads::Pads, live: &mut Option<Live>, win: &mut Win, actions: &mut Vec<Action>, pairing: &mut Vec<Arc<AtomicBool>>) {
    match e {
        Event::HostInfo { id, online, os, gpu, av1 } => {
            let entry = app.live.entry(id.clone()).or_insert_with(HostLive::default);
            entry.online = if online { Online::Online } else { Online::Offline };
            if online && let Some(h) = app.store.host_mut(&id) {
                let changed = h.os != os || h.gpu != gpu || h.av1 != av1;
                h.os = os.or(h.os.take());
                h.gpu = gpu.or(h.gpu.take());
                h.av1 = av1;
                if changed {
                    app.store.save();
                }
            }
        }
        Event::Preview { id, jpeg } => {
            if let Some(img) = ui::canvas::decode_jpeg(&jpeg) {
                let _ = std::fs::write(Store::image_path(&id), &jpeg);
                app.images.insert(id.clone(), img);
                app.image_is_last.insert(id, false);
            }
        }
        Event::Found(list) => {
            app.found = list;
            if app.screen == Screen::Add && app.add.step == AddStep::Code && !app.add.code.is_empty() && app.new_found().iter().any(|f| !app.add.asked.iter().any(|(_, a)| *a == f.address)) {
                actions.push(Action::StartPairing);
            }
        }
        Event::Probed { address, port, fingerprint } => {
            app.add.busy = false;
            match fingerprint {
                Ok(fp) => {
                    app.add.step = AddStep::Verify { address, port, fingerprint: fp };
                    app.add.message = None;
                    app.go(Screen::Add);
                }
                Err(e) => app.add.message = Some(e),
            }
        }
        Event::PairAsked { name } => {
            app.add.message = Some(format!("{} {name} : {}", tr("Code présenté à", "Code shown to"), tr("acceptez-le dans Admin", "accept it in Admin")));
        }
        Event::PairAccepted(host) => {
            for c in pairing.drain(..) {
                c.store(true, Ordering::Relaxed);
            }
            let name = host.name.clone();
            let id = app.store.put(host);
            app.store.selected = Some(id);
            app.store.save();
            app.toast(format!("{name} {}", tr("est ajouté", "added")));
            app.go(Screen::Home);
        }
        Event::PairFailed(msg) => app.add.message = Some(msg),
        Event::Status(st) => {
            let Some(s) = app.session.as_mut() else { return };
            s.phase = match st {
                Status::Preparing => Phase::Preparing,
                Status::Connecting => Phase::Connecting,
                Status::FirstImages => Phase::FirstImages,
                Status::Streaming => {
                    if s.phase != Phase::Streaming {
                        app.focus = Target::Menu(crate::app::MenuItem::Resume);
                    }
                    s.opened_at.get_or_insert_with(Instant::now);
                    Phase::Streaming
                }
                Status::Reconnecting { attempt, max } => Phase::Reconnecting { attempt, max },
                Status::Ending => {
                    s.ending_since.get_or_insert_with(Instant::now);
                    Phase::Ending
                }
                Status::Ended { error, .. } => {
                    if let Some(l) = live.as_ref()
                        && let Some(p) = l.handle.shared.current_input()
                    {
                        p.release_all();
                    }
                    Phase::Ended { error }
                }
            };
            if let Phase::Ended { error: None } = s.phase {
                app.session = None;
                app.go(Screen::Home);
            } else if let Phase::Ended { .. } = s.phase {
                app.screen = Screen::Session;
                app.focus = Target::Back;
            }
        }
        Event::Mode(m) => {
            if let Some(l) = live.as_mut() {
                l.mode = format!("{} × {}", m.width, m.height);
            }
        }
        Event::Second { latency_ms, fps, mbps, lost, codec, audio_ms } => {
            if let Some(l) = live.as_mut() {
                // Toutes les 10 s au journal : ce qu'on voit dans la barre.
                if l.started.elapsed().as_secs() % 10 == 0 {
                    log::info!("session : latence {:?} ms, {fps:.0} i/s, {mbps:.1} Mbit/s, {lost} perdues, {codec}", latency_ms.map(|v| (v * 10.0).round() / 10.0));
                }
                if let Some(v) = latency_ms {
                    l.latencies.push(v);
                }
                if fps > 0.0 {
                    l.mbps.push(mbps);
                }
                if let Some(s) = app.session.as_mut() {
                    let mut history = s.stats.as_ref().map(|s| s.history.clone()).unwrap_or_default();
                    if let Some(v) = latency_ms {
                        history.push(v);
                        if history.len() > 30 {
                            history.remove(0);
                        }
                    }
                    s.stats = Some(Stats { latency_ms, fps, mbps, lost, mode: l.mode.clone(), decoder: codec, audio_ms, history });
                }
            }
        }
        Event::Notice(n) => {
            if let Some(s) = app.session.as_mut() {
                s.notice = Some((n, Instant::now()));
            }
        }
        Event::Pointer { position, hidden, shape } => {
            if let Some(l) = live.as_mut() {
                l.pointer = position;
                l.pointer_hidden = hidden;
            }
            if let Some((w, h, hx, hy, px)) = shape
                && w > 0
                && h > 0
            {
                set_cursor_shape(win, live.as_ref(), w, h, hx, hy, px);
            }
        }
        Event::Rumble(r) => pads.rumble(r.id as usize, r.large_motor, r.small_motor, r.duration_ms),
        Event::Combo => actions.extend(app.open_menu()),
        Event::InputReady => {
            if let Some(l) = live.as_ref()
                && let Some(p) = l.handle.shared.current_input()
            {
                if !pads.slots().is_empty() {
                    log::info!("manettes : {} annoncée(s) à l'ordinateur", pads.slots().len());
                }
                for slot in pads.slots() {
                    let (v, pr) = pads.ids(slot);
                    p.gamepad_plug(slot as u32, Some(v), Some(pr));
                }
            }
        }
        Event::GameFullscreen(g) => {
            if let Some(l) = live.as_mut() {
                l.game_fullscreen = g;
            }
        }
        Event::DecoderFailed(e) => {
            if let Some(s) = app.session.as_mut() {
                s.notice = Some((format!("{} ({e})", tr("Décodeur indisponible", "Decoder unavailable")), Instant::now()));
            }
        }
        Event::Update(r) => {
            app.update = match r {
                Ok(Some(a)) => {
                    log::info!("mise à jour : version {} ({}) disponible, {}", a.version, a.channel.as_str(), a.page);
                    crate::app::UpdateState::Available { version: a.version, channel: a.channel.as_str().into(), page: a.page }
                }
                Ok(None) => crate::app::UpdateState::UpToDate,
                Err(e) => {
                    log::warn!("mise à jour : {e}");
                    crate::app::UpdateState::Failed(e)
                }
            };
        }
    }
}

/// La forme du pointeur de l'ordinateur : une texture (dessinée par l'app)
/// et un pointeur du système à l'échelle de l'image (bureau).
fn set_cursor_shape(win: &mut Win, live: Option<&Live>, w: u32, h: u32, hx: u32, hy: u32, px: Vec<u32>) {
    unsafe {
        if !win.cursor.is_null() {
            sdl::SDL_DestroyTexture(win.cursor);
        }
        win.cursor = sdl::SDL_CreateTexture(win.renderer, sdl::SDL_PIXELFORMAT_ARGB8888, sdl::SDL_TEXTUREACCESS_STREAMING, w as c_int, h as c_int);
        sdl::SDL_SetTextureBlendMode(win.cursor, win.premul);
        sdl::SDL_UpdateTexture(win.cursor, std::ptr::null(), px.as_ptr() as *const c_void, (w * 4) as c_int);
        win.cursor_size = (w, h, hx, hy);
        // Pointeur du système : à l'échelle de l'image (pixels de l'écran par
        // pixel de l'ordinateur, ramenés aux coordonnées de la fenêtre).
        let scale = match (live, win.video_rect) {
            (Some(l), Some(dst)) => (dst.w as f32 / l.handle.shared.host_size().0.max(1) as f32 / win.density()).clamp(0.5, 3.0),
            _ => 1.0,
        };
        let (sw, sh) = (((w as f32 * scale).round() as u32).max(1), ((h as f32 * scale).round() as u32).max(1));
        win.cursor_pixels = scale_argb(&px, w, h, sw, sh);
        let surf = sdl::SDL_CreateRGBSurfaceWithFormatFrom(win.cursor_pixels.as_mut_ptr() as *mut c_void, sw as c_int, sh as c_int, 32, (sw * 4) as c_int, sdl::SDL_PIXELFORMAT_ARGB8888);
        if !surf.is_null() {
            let c = sdl::SDL_CreateColorCursor(surf, (hx as f32 * scale) as c_int, (hy as f32 * scale) as c_int);
            sdl::SDL_FreeSurface(surf);
            if !c.is_null() {
                if !win.sys_cursor.is_null() {
                    sdl::SDL_SetCursor(sdl::SDL_GetDefaultCursor());
                    sdl::SDL_FreeCursor(win.sys_cursor);
                }
                win.sys_cursor = c;
            }
        }
    }
}

/// Mise à l'échelle au plus proche voisin (pointeurs : bords nets).
fn scale_argb(px: &[u32], w: u32, h: u32, sw: u32, sh: u32) -> Vec<u32> {
    let mut out = vec![0u32; (sw * sh) as usize];
    for y in 0..sh {
        let sy = (y * h / sh).min(h - 1);
        for x in 0..sw {
            let sx = (x * w / sw).min(w - 1);
            out[(y * sw + x) as usize] = px.get((sy * w + sx) as usize).copied().unwrap_or(0);
        }
    }
    out
}

/// Affichage de la session : immédiat (fréquence variable) ou synchronisé.
fn wants_vsync(app: &App, host: &crate::model::Host) -> bool {
    match host.settings.display.as_str() {
        "vsync" => true,
        "immediate" | "vrr" => false,
        _ => app.machine.vrr != Some(true),
    }
}

#[allow(clippy::too_many_arguments)]
fn perform(a: Action, app: &mut App, pads: &mut pads::Pads, live: &mut Option<Live>, win: &mut Win, tx: &Sender<Event>, pairing: &mut Vec<Arc<AtomicBool>>, quit: &mut bool) {
    match a {
        Action::Save => app.store.save(),
        Action::Quit => {
            if let Some(l) = live.as_ref() {
                l.handle.session.stop();
            }
            *quit = true;
        }
        Action::Language => set_language(app),
        Action::Interface => {
            let display = app.machine.display;
            let layout = system::layout(&app.store.app.interface, None, display);
            if layout != app.layout {
                app.layout = layout;
                app.keyboard_focus = layout.console();
                let full = layout.console();
                unsafe { sdl::SDL_SetWindowFullscreen(win.window, if full { sdl::SDL_WINDOW_FULLSCREEN_DESKTOP } else { 0 }) };
                app.fullscreen = full;
                app.open_settings(true, Screen::Home);
            }
        }
        Action::CheckUpdate => {
            app.update = crate::app::UpdateState::Checking;
            jobs::check_update(app.store.app.beta, tx.clone());
        }
        Action::OpenUrl(url) => open_url(&url),
        Action::Minimize => unsafe { sdl::SDL_MinimizeWindow(win.window) },
        Action::Maximize => unsafe {
            if sdl::SDL_GetWindowFlags(win.window) & sdl::SDL_WINDOW_MAXIMIZED != 0 {
                sdl::SDL_RestoreWindow(win.window);
                app.maximized = false;
            } else {
                sdl::SDL_MaximizeWindow(win.window);
                app.maximized = true;
            }
        },
        Action::ToggleFullscreen => {
            if app.layout.console() {
                return;
            }
            app.fullscreen = !app.fullscreen;
            unsafe {
                sdl::SDL_SetWindowFullscreen(win.window, if app.fullscreen { sdl::SDL_WINDOW_FULLSCREEN_DESKTOP } else { 0 });
                // En plein écran pendant la session, les raccourcis du système vont à l'ordinateur.
                sdl::SDL_SetWindowKeyboardGrab(win.window, (app.fullscreen && live.is_some()) as c_int);
            }
        }
        Action::ToggleMouse => {
            if let (Some(l), Some(s)) = (live.as_mut(), app.session.as_ref()) {
                l.mouse = if s.relative { MouseMode::Desktop } else { MouseMode::Game };
            }
        }
        Action::OpenKeyboard => {
            unsafe { sdl::SDL_StartTextInput() };
            if let Some(l) = live.as_mut() {
                l.typing = app.layout.console();
            }
            if system::gamescope() {
                open_url("steam://open/keyboard");
            }
        }
        Action::Connect { host, profile } => {
            if live.is_some() {
                return;
            }
            let Some(h) = app.store.host(&host).cloned() else { return };
            let mode = app.session_mode(&h);
            let codec = app.codec_for(&h);
            let device = device_info(app);
            let light = h.settings.battery_light && app.machine.on_battery();
            let relative = h.settings.mouse == "game";
            let handle = session::start(&h, profile, codec, mode, &app.machine, light, relative, device, tx.clone());
            let mbps = session::settings(&h, profile, codec, mode, &app.machine, light).bitrate(mode.0, mode.1, mode.2) / 1_000_000;
            let codec_label = match codec {
                "h264" => "H.264",
                "av1" => "AV1",
                _ => "HEVC",
            };
            app.session = Some(SessionUi {
                host: h.id.clone(),
                name: h.name.clone(),
                profile,
                phase: Phase::Preparing,
                menu: false,
                keys: false,
                stats: None,
                show_stats: false,
                pad_mouse: false,
                muted: media::MUTED.load(Ordering::Relaxed),
                light,
                relative,
                bar: false,
                bar_menu: None,
                notice: None,
                started: Instant::now(),
                ending_since: None,
                opened_at: None,
                summary: format!("{} · {} × {} · {} Hz · {codec_label} · {mbps} Mbit/s", ui::profile_label(profile), mode.0, mode.1, mode.2),
            });
            app.screen = Screen::Session;
            app.focus = Target::CancelConnect;
            let vsync = wants_vsync(app, &h);
            win.set_vsync(vsync);
            unsafe {
                sdl::SDL_DisableScreenSaver();
                if app.fullscreen || app.layout.console() {
                    sdl::SDL_SetWindowKeyboardGrab(win.window, 1);
                }
            }
            *live = Some(Live {
                handle,
                host: h.id.clone(),
                pointer: None,
                pointer_hidden: true,
                started: Instant::now(),
                latencies: Vec::new(),
                mbps: Vec::new(),
                mode: format!("{} × {}", mode.0, mode.1),
                game_fullscreen: false,
                mouse: match h.settings.mouse.as_str() {
                    "desktop" => MouseMode::Desktop,
                    "game" => MouseMode::Game,
                    _ => MouseMode::Auto,
                },
                typing: false,
                escape_down: None,
                bar_hide: None,
                decoded: None,
                last_frame: None,
                shown: None,
                snapshot: None,
                snapshot_at: None,
            });
            log::info!("session : {} ({}, {} × {} à {} Hz, {codec}, {})", h.name, profile.id(), mode.0, mode.1, mode.2, if vsync { "synchronisé" } else { "immédiat" });
        }
        Action::CancelConnect | Action::Disconnect => {
            if let Some(l) = live.as_ref() {
                if let Some(p) = l.handle.shared.current_input() {
                    p.release_all();
                }
                l.handle.session.stop();
            }
            if let Some(s) = app.session.as_mut() {
                s.menu = false;
                s.keys = false;
                s.bar = false;
                s.bar_menu = None;
                s.phase = Phase::Ending;
                s.ending_since.get_or_insert_with(Instant::now);
            }
            // L'accueil qui revient montre déjà la dernière image de la session.
            if let (Some(l), Some(s)) = (live.as_ref(), app.session.as_ref())
                && app.store.app.keep_last_image
                && let Some(img) = l.snapshot.clone()
            {
                app.images.insert(s.host.clone(), img);
                app.image_is_last.insert(s.host.clone(), true);
            }
            unsafe {
                sdl::SDL_SetRelativeMouseMode(0);
                sdl::SDL_ShowCursor(1);
                sdl::SDL_SetCursor(sdl::SDL_GetDefaultCursor());
            }
        }
        Action::SetProfile(p) => {
            if let (Some(l), Some(s)) = (live.as_ref(), app.session.as_ref())
                && let Some(h) = app.store.host(&s.host)
            {
                let mode = app.session_mode(h);
                let codec = app.codec_for(h);
                l.handle.session.set_settings(session::settings(h, p, codec, mode, &app.machine, s.light));
            }
        }
        Action::Light(on) => {
            if let (Some(l), Some(s)) = (live.as_ref(), app.session.as_ref())
                && let Some(h) = app.store.host(&s.host)
            {
                let (w, hh, fps) = app.session_mode(h);
                let fps = if on { fps.min(60) } else { fps };
                let codec = app.codec_for(h);
                l.handle.session.set_settings(session::settings(h, s.profile, codec, (w, hh, fps), &app.machine, on));
                l.handle.session.set_mode(Mode { width: w, height: hh, refresh: fps });
            }
        }
        Action::Mute(on) => media::MUTED.store(on, Ordering::Relaxed),
        Action::PadMouse(on) => pads.set_mouse(on),
        Action::ReleaseAll => {
            if let Some(l) = live.as_ref()
                && let Some(p) = l.handle.shared.current_input()
            {
                p.release_all();
                for slot in pads.slots() {
                    p.gamepad_axes_rest(slot as u32);
                }
            }
            if let Some(l) = live.as_mut() {
                l.typing = false;
            }
        }
        Action::Shortcut(keys) => {
            if let Some(l) = live.as_ref()
                && let Some(p) = l.handle.shared.current_input()
            {
                for k in &keys {
                    p.key(*k, true);
                }
                for k in keys.iter().rev() {
                    p.key(*k, false);
                }
            }
        }
        Action::StartPairing => {
            let targets: Vec<jobs::Target> = app
                .new_found()
                .iter()
                .filter(|f| !app.add.asked.iter().any(|(_, a)| *a == f.address))
                .map(|f| jobs::Target { name: f.name.clone(), address: f.address.clone(), port: f.port, fingerprint: f.fingerprint.clone() })
                .collect();
            if targets.is_empty() {
                if !app.add.asked.is_empty() {
                    return;
                }
                app.add.message = Some(tr("Aucune Station trouvée pour l'instant : le code sera présenté dès qu'elle apparaît, ou ajoutez-la par adresse.", "No Station found yet: the code is shown as soon as one appears, or add it by address.").into());
                return;
            }
            app.add.asked.extend(targets.iter().map(|t| (t.name.clone(), t.address.clone())));
            let cancel = Arc::new(AtomicBool::new(false));
            jobs::pair(targets, app.add.code.clone(), device_info(app), cancel.clone(), tx.clone());
            pairing.push(cancel);
        }
        Action::PairAddress { address, port, fingerprint } => {
            let cancel = Arc::new(AtomicBool::new(false));
            app.add.asked.push((address.clone(), address.clone()));
            let t = jobs::Target { name: address.clone(), address, port, fingerprint: Some(fingerprint) };
            jobs::pair(vec![t], app.add.code.clone(), device_info(app), cancel.clone(), tx.clone());
            pairing.push(cancel);
        }
        Action::StopPairing => {
            for c in pairing.drain(..) {
                c.store(true, Ordering::Relaxed);
            }
        }
        Action::Probe { address, port } => jobs::probe(address, port, tx.clone()),
        Action::Wake(id) => {
            let ok = app.store.host(&id).and_then(|h| h.mac.clone()).is_some_and(|m| jobs::wake(&m));
            app.toast(if ok { tr("Réveil envoyé : l'ordinateur démarre…", "Wake sent: the computer is starting…") } else { tr("Réveil impossible", "Could not wake") });
        }
        Action::Rumble { pad, low, high, triggers } => pads.test_rumble(pad, low, high, triggers),
    }
}

/// Ouvre une adresse avec l'app du système (portail en Flatpak).
fn open_url(url: &str) {
    match std::process::Command::new("xdg-open").arg(url).spawn() {
        Ok(_) => log::info!("ouverture de {url}"),
        Err(e) => log::warn!("ouverture de {url} impossible ({e})"),
    }
}

/// Fin de session : la dernière session (latence médiane, durée) pour l'accueil.
fn finish(app: &mut App, win: &mut Win, l: Live) {
    // L'interface a pu oublier la session (fin sans erreur) : l'ordinateur vient de `Live`.
    let host = Some(l.host.clone());
    if app.session.as_ref().is_some_and(|s| !matches!(s.phase, Phase::Ended { error: Some(_) })) {
        app.session = None;
        if app.screen == Screen::Session {
            app.go(Screen::Home);
        }
    }
    unsafe {
        sdl::SDL_SetRelativeMouseMode(0);
        sdl::SDL_ShowCursor(1);
        sdl::SDL_SetCursor(sdl::SDL_GetDefaultCursor());
        sdl::SDL_SetWindowKeyboardGrab(win.window, 0);
        sdl::SDL_EnableScreenSaver();
        sdl::SDL_StopTextInput();
    }
    win.set_vsync(true);
    win.screen.reset();
    let mut lat = l.latencies.clone();
    lat.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let seconds = l.started.elapsed().as_secs();
    log::info!("session : {} s, {} secondes mesurées", seconds, lat.len());
    let Some(id) = host else { return };
    // La dernière session : latence si l'ordinateur l'a mesurée, sinon durée et mode seuls.
    let mbps = if l.mbps.is_empty() { 0.0 } else { l.mbps.iter().sum::<f64>() / l.mbps.len() as f64 };
    let codec = l.handle.shared.codec();
    let (median, p95) = if lat.is_empty() { (0.0, 0.0) } else { (lat[lat.len() / 2], lat[((lat.len() as f64 * 0.95) as usize).min(lat.len() - 1)]) };
    if seconds >= 3
        && let Some(h) = app.store.host_mut(&id)
    {
        h.last = Some(LastSession { median, p95, seconds, mode: format!("{} · {codec}", l.mode), mbps, at: now_secs() });
    }
    app.store.save();
    if app.store.app.keep_last_image
        && let Some(img) = l.snapshot.clone()
    {
        if let Some(png) = ui::canvas::encode_png(&img) {
            let _ = std::fs::write(Store::image_path(&id).with_extension("png"), png);
        }
        app.images.insert(id.clone(), img);
        app.image_is_last.insert(id, true);
    } else {
        // L'aperçu sera repris d'Admin au prochain sondage.
        app.images.remove(&id);
    }
}
