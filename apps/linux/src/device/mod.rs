//! L'app sur Linux : la fenêtre SDL (bureau : sans bordure, barre de titre
//! dessinée par l'app ; console : plein écran sous gamescope), la boucle des
//! événements (clavier, souris, pavés tactiles, manettes), l'interface, les
//! tâches de fond et la session. Pendant la session, l'image de l'ordinateur
//! est une texture YUV ; l'interface (barre, menu, capsules) est posée
//! par-dessus, avec le pointeur de l'ordinateur.

mod chime;
mod jobs;
mod log_file;
mod media;
mod os;
mod pads;
pub mod sdl;
mod screens;
mod session;
mod system;
mod video;

use std::collections::HashMap;
use std::ffi::{c_int, c_void, CString};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Sender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use kybernow_core::console::{DeviceInfo, Mode};
use kybernow_core::input::{GamepadRumble, MouseMode, MouseModeRules};
use kybernow_core::kynput_rs::MouseButtonType;
use kybernow_core::latency;
use kybernow_core::profile::{Capabilities, StreamSettings};
use kybernow_core::session::Status;

use crate::app::{Action, AddStep, App, Found, HostLive, Layout, Nav, Online, OsAction, Phase, Screen, SessionUi, Stats, Target};
use crate::displays::{self, Display};
use crate::i18n::{self, tr};
use crate::model::{now_secs, LastSession, Store};
use crate::ui::{self, canvas::Canvas};

/// Ce qui revient à la boucle principale (fils de fond, rappels du cœur).
pub enum Event {
    HostInfo {
        id: String,
        online: bool,
        os: Option<String>,
        gpu: Option<String>,
        av1: bool,
        virtual_display: Option<bool>,
        /// Écrans que la Station sait créer (multi-écrans ; 1 : une Station antérieure).
        max_screens: Option<usize>,
        station: Option<crate::app::StationLive>,
        /// Les cartes réseau de l'ordinateur (vide : Station 0.4.9, ou rien de neuf).
        wake: Vec<crate::model::WakeCard>,
        own_device: Option<bool>,
    },
    /// Le réveil est parti (au moins un paquet), ou pourquoi rien n'est parti.
    Woken { id: String, result: Result<(), String> },
    /// La réponse d'Admin à un redémarrage ou une mise à jour de la Station.
    StationReply { host: String, kind: crate::app::StationKind, result: Result<Option<String>, crate::app::StationRefusal> },
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
    /// Le pointeur de l'ordinateur sur l'écran `screen` de l'appareil (multi-écrans :
    /// une couche par écran ; la forme est celle de l'écran où il se trouve).
    Pointer { screen: usize, position: Option<(f64, f64)>, hidden: bool, shape: Option<(u32, u32, u32, u32, Vec<u32>)> },
    /// Les écrans pris par la session ont changé (au démarrage de l'image, ou l'un d'eux
    /// a changé de taille) : les fenêtres suivent.
    Screens,
    Rumble(GamepadRumble),
    Combo,
    InputReady,
    GameFullscreen(bool),
    DecoderFailed(String),
    Update(Result<Option<kybernow_core::update::Available>, String>),
    /// KyberNow Link OS.
    Bt(Vec<crate::app::BtDevice>),
    Wifi(Result<Vec<crate::app::WifiNet>, String>),
    WifiDone(Result<String, String>),
    OsStatus(crate::app::OsInfo),
    OsApplied(Result<(), String>),
    OsDownload(u64, u64),
    OsDownloaded(Result<std::path::PathBuf, String>),
    /// Ce qui se prépare au lancement : avancement, réseau, Bluetooth.
    OsLoading(f32, bool, bool),
    OsReady,
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

/// Une fenêtre SDL de l'app, avec son rendu, sa texture d'interface, sa
/// texture d'image et son dessin. La fenêtre de l'app est la première ; une
/// session à plusieurs écrans en ouvre une de plus par écran de l'appareil
/// (docs/multi-ecrans.md), chacune plein écran.
struct Win {
    window: *mut sdl::SDL_Window,
    renderer: *mut sdl::SDL_Renderer,
    /// Identifiant SDL de la fenêtre : celui des événements qui la visent.
    id: u32,
    /// L'écran de l'appareil dont cette fenêtre montre le flux (indice dans la
    /// liste regroupée de `screens::Detected`).
    screen: usize,
    /// Le dessin de l'interface de cette fenêtre (sa taille, son échelle).
    cv: Canvas,
    ui: *mut sdl::SDL_Texture,
    ui_size: (u32, u32),
    /// La texture ne porte que le trait de la barre, pour cette taille et cette échelle.
    hint_key: Option<(u32, u32, u32)>,
    /// Le pointeur de l'ordinateur dessiné par l'app (souris des jeux, console).
    cursor: *mut sdl::SDL_Texture,
    cursor_size: (u32, u32, u32, u32),
    cursor_pixels: Vec<u32>,
    /// Le pointeur du système à la forme de celui de l'ordinateur (bureau).
    sys_cursor: *mut sdl::SDL_Cursor,
    /// La texture de l'image de l'ordinateur.
    video: video::Screen,
    premul: c_int,
    vsync: bool,
    /// Taille de la fenêtre (coordonnées des événements) et de la sortie (pixels).
    win_size: (i32, i32),
    out_size: (i32, i32),
    /// Où l'image est posée dans la sortie (pixels).
    video_rect: Option<sdl::SDL_Rect>,
    /// Elle doit être présentée au prochain tour (image reçue, pointeur dessiné).
    present: bool,
}

impl Win {
    fn new(window: *mut sdl::SDL_Window, renderer: *mut sdl::SDL_Renderer, premul: c_int, screen: usize, size: (i32, i32)) -> Self {
        let mut w = Win {
            window,
            renderer,
            id: unsafe { sdl::SDL_GetWindowID(window) },
            screen,
            cv: Canvas::new(),
            ui: std::ptr::null_mut(),
            ui_size: (0, 0),
            hint_key: None,
            cursor: std::ptr::null_mut(),
            cursor_size: (0, 0, 0, 0),
            cursor_pixels: Vec::new(),
            sys_cursor: std::ptr::null_mut(),
            video: video::Screen::default(),
            premul,
            vsync: true,
            win_size: size,
            out_size: size,
            video_rect: None,
            present: true,
        };
        w.sizes();
        w
    }

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

    /// L'échelle de l'interface dans cette fenêtre : pixels par point.
    fn k(&self, layout: Layout) -> f32 {
        ui_scale(layout, self.out_size, self.density())
    }

    /// La largeur de la fenêtre, en points de l'interface.
    fn width_points(&self, layout: Layout) -> f32 {
        self.out_size.0 as f32 / self.k(layout)
    }

    /// La fenêtre a le clavier (focus d'entrée de SDL).
    fn focused(&self) -> bool {
        unsafe { sdl::SDL_GetWindowFlags(self.window) & sdl::SDL_WINDOW_INPUT_FOCUS != 0 }
    }

    /// L'image de l'écran de cette fenêtre est arrivée.
    fn has_frame(&self) -> bool {
        self.video.last.is_some()
    }

    fn set_vsync(&mut self, on: bool) {
        if self.vsync != on {
            unsafe { sdl::SDL_RenderSetVSync(self.renderer, on as c_int) };
            self.vsync = on;
            log::info!("affichage : {}", if on { "synchronisé" } else { "immédiat (fréquence variable si l'écran la permet)" });
        }
    }

    /// Ferme la fenêtre et ce qui lui appartient.
    fn destroy(mut self) {
        self.video.reset();
        unsafe {
            if !self.cursor.is_null() {
                sdl::SDL_DestroyTexture(self.cursor);
            }
            if !self.ui.is_null() {
                sdl::SDL_DestroyTexture(self.ui);
            }
            if !self.sys_cursor.is_null() {
                sdl::SDL_SetCursor(sdl::SDL_GetDefaultCursor());
                sdl::SDL_FreeCursor(self.sys_cursor);
            }
            sdl::SDL_DestroyRenderer(self.renderer);
            sdl::SDL_DestroyWindow(self.window);
        }
    }
}

/// Les fenêtres de l'app : la sienne, puis celles d'une session à plusieurs écrans.
struct Wins {
    /// La fenêtre de l'app avant qu'une session à plusieurs écrans la passe en plein écran ;
    /// remise comme elle était quand la session est finie et que son message est lu.
    prev: Option<PrevWindow>,
    list: Vec<Win>,
}

impl Wins {
    fn main(&mut self) -> &mut Win {
        &mut self.list[0]
    }

    fn index_of(&self, id: u32) -> Option<usize> {
        self.list.iter().position(|w| w.id == id)
    }

    /// La fenêtre qui montre le flux de cet écran de l'appareil.
    fn index_of_screen(&self, screen: usize) -> Option<usize> {
        self.list.iter().position(|w| w.screen == screen)
    }

    /// Une des fenêtres de l'app a le clavier.
    fn any_focused(&self) -> bool {
        self.list.iter().any(Win::focused)
    }

    /// Chaque fenêtre redessine son image et son interface.
    fn invalidate(&mut self) {
        for w in &mut self.list {
            w.ui_size = (0, 0);
            w.video.reset();
        }
    }

    /// La fin de session ferme les fenêtres de plus.
    fn close_extras(&mut self) {
        while self.list.len() > 1 {
            if let Some(w) = self.list.pop() {
                w.destroy();
            }
        }
    }

    /// Ferme celles des fenêtres de plus que `stale` désigne (leur écran n'est plus au même endroit).
    fn close_extras_where(&mut self, stale: impl Fn(&Win) -> bool) {
        let mut i = 1;
        while i < self.list.len() {
            if stale(&self.list[i]) {
                self.list.remove(i).destroy();
            } else {
                i += 1;
            }
        }
    }

    /// Les images reçues sont oubliées (la connexion reprend à sa première étape).
    fn forget_frames(&mut self) {
        for w in &mut self.list {
            w.video.reset();
            w.hint_key = None;
            w.present = true;
        }
    }
}

/// Une fenêtre plein écran de plus, sur l'écran de l'appareil `display` (multi-écrans).
/// Même titre que celle de l'app. Elle s'affiche tout de suite (le fond de la connexion).
fn create_extra(display: &Display, screen: usize, premul: c_int, grab_keyboard: bool) -> Option<Win> {
    let title = CString::new("KyberNow Link").unwrap();
    let at = sdl::windowpos_centered_display(display.sdl);
    let flags = sdl::SDL_WINDOW_SHOWN | sdl::SDL_WINDOW_FULLSCREEN_DESKTOP | sdl::SDL_WINDOW_ALLOW_HIGHDPI;
    let window = unsafe { sdl::SDL_CreateWindow(title.as_ptr(), at, at, display.w.max(1), display.h.max(1), flags) };
    if window.is_null() {
        log::warn!("écran {} : fenêtre impossible ({})", display.sdl, sdl::error());
        return None;
    }
    // Sans attendre la synchro verticale : plusieurs fenêtres présentées l'une après l'autre
    // en attendraient chacune une (l'écran principal suit le réglage « Affichage des images »).
    let renderer = unsafe { sdl::SDL_CreateRenderer(window, -1, sdl::SDL_RENDERER_ACCELERATED) };
    if renderer.is_null() {
        log::warn!("écran {} : rendu impossible ({})", display.sdl, sdl::error());
        unsafe { sdl::SDL_DestroyWindow(window) };
        return None;
    }
    unsafe { sdl::SDL_SetWindowKeyboardGrab(window, grab_keyboard as c_int) };
    let mut w = Win::new(window, renderer, premul, screen, (display.w.max(1), display.h.max(1)));
    w.vsync = false;
    log::info!("écran {} ({}) : fenêtre ouverte, {} × {}", screen, display.name, w.out_size.0, w.out_size.1);
    Some(w)
}

/// Met la fenêtre de l'app en plein écran sur l'écran SDL `index`.
fn place_fullscreen(window: *mut sdl::SDL_Window, index: i32) {
    let at = sdl::windowpos_centered_display(index);
    unsafe {
        sdl::SDL_SetWindowFullscreen(window, 0);
        sdl::SDL_SetWindowPosition(window, at, at);
        sdl::SDL_SetWindowFullscreen(window, sdl::SDL_WINDOW_FULLSCREEN_DESKTOP);
    }
}

/// Les écrans de l'appareil qui ont une fenêtre, d'après le plan : ce que la
/// session ouvre dès la connexion (l'écran principal en premier).
fn plan_screens(plan: &[kybernow_core::screens::PlannedScreen]) -> Vec<usize> {
    plan.iter().map(|p| p.device).collect()
}

/// Ferme et ouvre les fenêtres de plus pour qu'il y en ait une par écran voulu
/// (`wanted`, l'écran principal compris), et que celle de l'app montre l'écran
/// `primary`. Le reste ne bouge pas.
fn sync_windows(wins: &mut Wins, wanted: &[usize], primary: usize, devices: &screens::Detected, grab_keyboard: bool) {
    wins.list[0].screen = primary;
    let mut i = 1;
    while i < wins.list.len() {
        let s = wins.list[i].screen;
        if s == primary || !wanted.contains(&s) {
            wins.list.remove(i).destroy();
        } else {
            i += 1;
        }
    }
    let premul = wins.list[0].premul;
    for &s in wanted {
        if s == primary || wins.index_of_screen(s).is_some() {
            continue;
        }
        if let Some(d) = devices.grouped.get(s)
            && let Some(w) = create_extra(d, s, premul, grab_keyboard)
        {
            wins.list.push(w);
        }
    }
}

/// La fenêtre de l'app avant qu'une session à plusieurs écrans la passe en plein
/// écran sur l'écran principal : la fin de session la remet comme elle était.
struct PrevWindow {
    fullscreen: bool,
    maximized: bool,
    position: (i32, i32),
    size: (i32, i32),
}

/// État de la session en cours.
struct Live {
    handle: session::Handle,
    /// L'ordinateur de la session (la dernière session y est enregistrée).
    host: String,
    /// Le pointeur de l'ordinateur sur chaque écran de l'appareil : position
    /// (pixels de son écran) et caché ; la fenêtre de l'écran ne dessine que le sien.
    pointers: HashMap<usize, (Option<(f64, f64)>, bool)>,
    started: Instant,
    latencies: Vec<f64>,
    mbps: Vec<f64>,
    /// Définition (« 1920 × 1080 »), fréquence et format de la session, pour la dernière session.
    mode: String,
    hz: u32,
    codec: &'static str,
    game_fullscreen: bool,
    /// Souris choisie pour la session (barre, raccourci) ; sinon le réglage.
    mouse: MouseMode,
    /// Saisie de texte (clavier de Steam) : les caractères partent en Unicode.
    typing: bool,
    /// Échap tenu (long : rend la souris).
    escape_down: Option<Instant>,
    /// La barre du bureau doit se cacher à cet instant.
    bar_hide: Option<Instant>,
    /// Le pointeur est contre le bord haut : la barre du bureau s'ouvre à cet instant s'il y reste.
    bar_arm: Option<Instant>,
    /// Multi-écrans : la fenêtre où ce geste a lieu, celle où la barre est ouverte,
    /// celle que le pointeur survole (indices dans `Wins::list`).
    bar_arm_win: usize,
    bar_win: usize,
    hover: usize,
    decoded: Option<(u32, u32)>,
    last_frame: Option<Instant>,
    /// Image clé de la dernière image montrée (latence).
    shown: Option<(u64, i64, i64)>,
    /// Dernière image de la session, réduite (accueil).
    snapshot: Option<tiny_skia::Pixmap>,
    snapshot_at: Option<Instant>,
    /// Multi-écrans : la session a été ouverte avec les écrans de l'appareil
    /// (interface de bureau) ; le cœur décide du nombre d'écrans.
    multi: bool,
    /// Les écrans de l'appareil que la session connaît.
    devices: screens::Detected,
    /// Ce que le cœur a reçu au départ : les écrans et ce qui décide de leur nombre (réglage
    /// « Écrans », définition sur mesure, ce que la Station crée) ; le réglage changé pendant la
    /// session vaut pour la suivante.
    screens_base: Option<kybernow_core::screens::Screens>,
    /// Les réglages du flux tels que le cœur les a (départ, profil changé, « Alléger ») : le
    /// nombre d'écrans que le cœur en tire (`screens::session_plan`) est refait ici à
    /// l'identique, pour ce que la session attend avant son annonce (`Event::Screens`).
    settings: StreamSettings,
    /// L'écran virtuel demandé au départ de la session (réglage de l'ordinateur).
    virtual_display: bool,
    /// Écrans pris par la session, pour la dernière session (1 hors multi-écrans) : ceux que
    /// le cœur a annoncés, ceux qu'il n'a pas pu créer n'y sont pas (comme sur Mac).
    screens: usize,
    /// Les écrans de l'appareil que l'ordinateur n'a pas pu créer (leur fenêtre le dit).
    not_created: Vec<usize>,
    /// L'écran où la fenêtre de l'app a été mise en plein écran (multi-écrans).
    placed: Option<usize>,
    /// Les écrans de l'appareil relus (toutes les secondes), et un changement vu depuis
    /// un instant : appliqué quand il se stabilise (un branchement en fait plusieurs).
    rescan_at: Instant,
    changed: Option<(screens::Detected, Instant)>,
}

/// Coordonnées de la fenêtre → points de l'interface.
fn to_ui(win: &Win, layout: Layout, x: i32, y: i32) -> (f32, f32) {
    let d = win.density() / win.k(layout);
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
    // Plusieurs fenêtres plein écran (multi-écrans) : le clavier passe de l'une à l'autre sans que SDL les réduise.
    sdl::hint("SDL_VIDEO_MINIMIZE_ON_FOCUS_LOSS", "0");
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
    let os = os::is_os();
    let layout = if os {
        // KyberNow Link OS : le premier démarrage se fait au salon ; ensuite l'usage choisi.
        if !store.app.os_setup_done || store.app.interface != "desktop" { Layout::Tv } else { Layout::Desktop }
    } else {
        system::layout(&store.app.interface, ui_arg, display)
    };
    log::info!("interface : {layout:?} (écran {} × {}, gamescope {})", display.0, display.1, system::gamescope());

    // La fenêtre.
    let title = CString::new("KyberNow Link").unwrap();
    let (ww, wh, flags) = match (layout, os) {
        (Layout::Desktop, false) => {
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
    // Les écrans de l'appareil (multi-écrans) : celui de la fenêtre est « Cet écran ».
    let mut devices = screens::detect();
    let first_screen = devices.current(window);
    let mut wins = Wins { prev: None, list: vec![Win::new(window, renderer, premul, first_screen, (ww, wh))] };
    log_devices(&devices);
    video::FRAME_EVENT.store(unsafe { sdl::SDL_RegisterEvents(1) }, Ordering::Relaxed);

    // L'app.
    let mut app = App::new(store, layout);
    if os {
        app.os = Some(os::info());
        app.screen = Screen::Launch;
        app.fullscreen = false;
    }
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
    if !layout.console() && !os {
        app.go(Screen::Home);
    }

    let (tx, rx) = channel::<Event>();
    jobs::discover(tx.clone());
    let mut last_update = Instant::now();
    app.update = crate::app::UpdateState::Checking;
    jobs::check_update(app.store.app.beta, tx.clone());
    let mut pads = pads::Pads::default();
    let mut live: Option<Live> = None;
    // Les écrans de l'appareil relus toutes les deux secondes hors session (branchement, fenêtre déplacée).
    let mut last_scan = Instant::now();
    let mut pairing: Vec<Arc<AtomicBool>> = Vec::new();
    let mut last_poll = Instant::now() - Duration::from_secs(60);
    let mut last_station_poll = Instant::now();
    let mut last_wake_poll = Instant::now();
    let mut last_net = Instant::now();
    // KyberNow Link OS : le son de l'allumage, ce qui se prépare, le nom CEC, l'état A/B.
    let mut chime: Option<u32> = None;
    let mut start_down: Option<Instant> = None;
    let mut os_committed = false;
    if os {
        app.os_loading = Some((tr("réseau · bluetooth", "network · bluetooth").into(), 0.0));
        chime = chime::play();
        os::readiness(tx.clone());
        os::status(tx.clone());
        if app.store.app.os_cec {
            os::ctl(&["cec-name"]);
        }
    }
    let mut last_clip = Instant::now();
    let mut dirty = true;
    let mut present = true;
    let mut quit = false;

    while !quit {
        let streaming = live.is_some() && app.session.as_ref().is_some_and(|s| matches!(s.phase, Phase::Streaming | Phase::Reconnecting { .. }));
        // Animations : lancement, passage, capsules, ajout, écran des manettes.
        let animating = app.screen == Screen::Launch
            || app.session.as_ref().is_some_and(|s| matches!(s.phase, Phase::Preparing | Phase::Connecting | Phase::StationRestarting | Phase::FirstImages | Phase::Ending))
            || app.waking_host().is_some() && app.screen == Screen::Home
            || app.station_op.as_ref().is_some_and(|o| o.busy()) && app.screen == Screen::Settings
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
                handle_sdl(&ev, &mut app, &mut pads, live.as_mut(), &mut wins, &mut actions, &mut dirty, &mut quit, &mut start_down);
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
            if let Event::Pointer { screen, .. } = &e {
                // Le pointeur n'est dessiné que dans la fenêtre de son écran.
                match wins.index_of_screen(*screen) {
                    Some(i) => wins.list[i].present = true,
                    None => present = true,
                }
            } else {
                dirty = true;
            }
            handle_event(e, &mut app, &mut pads, &mut live, &mut wins, &mut actions, &mut pairing);
        }
        for a in actions.drain(..) {
            perform(a, &mut app, &mut pads, &mut live, &mut wins, &tx, &mut pairing, &mut quit);
            dirty = true;
        }

        // Lancement fini : l'accueil.
        if app.screen == Screen::Launch && app.os_loading.is_none() && app.launched.elapsed().as_secs_f32() >= if app.reduce_motion { ui::LAUNCH_REDUCED } else { ui::LAUNCH } {
            app.launch_done();
            dirty = true;
            if let Some(dev) = chime.take() {
                std::thread::spawn(move || {
                    std::thread::sleep(Duration::from_secs(4));
                    chime::stop(dev);
                });
            }
            if app.screen == Screen::Setup {
                app.setup.bt_listening = os::listen(true, tx.clone());
            } else if os && !os_committed {
                // L'accueil s'affiche : un essai A/B en cours devient le défaut.
                os_committed = true;
                os::commit();
            }
        }
        // Le premier démarrage finit quand l'ordinateur est ajouté (ou passé).
        if app.setup.active && app.screen == Screen::Home {
            actions.extend(app.finish_setup());
        }
        for a in actions.drain(..) {
            perform(a, &mut app, &mut pads, &mut live, &mut wins, &tx, &mut pairing, &mut quit);
            dirty = true;
        }

        // Fin de session : l'accueil, la dernière session enregistrée.
        if live.is_some() && matches!(app.session.as_ref().map(|s| &s.phase), Some(Phase::Ended { .. }) | None) {
            let l = live.take().unwrap();
            finish(&mut app, &mut wins, l);
            last_poll = Instant::now() - Duration::from_secs(60);
            last_scan = Instant::now() - Duration::from_secs(60);
            dirty = true;
        }
        // La session à plusieurs écrans est finie et son message lu : la fenêtre de l'app redevient ce qu'elle était.
        if live.is_none() && app.session.is_none() && wins.prev.is_some() {
            restore_main(&mut app, &mut wins);
            dirty = true;
        }

        // Les écrans de l'appareil ont peut-être changé (événement de SDL) : relus tout de suite.
        let rescan_now = RESCAN.swap(false, Ordering::Relaxed);
        if let Some(l) = live.as_mut() {
            if rescan_now {
                l.rescan_at = Instant::now() - Duration::from_secs(10);
            }
            session_tick(&mut app, l, &mut wins, &mut dirty, &mut present);
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

        // Un réveil : l'ordinateur sondé toutes les 2 s, 2 min au plus, jusqu'à ce qu'il réponde.
        if let Some(id) = app.waking_host()
            && last_wake_poll.elapsed() > Duration::from_secs(2)
            && let Some(h) = app.store.host(id)
        {
            last_wake_poll = Instant::now();
            jobs::poll(h.clone(), false, tx.clone());
        }
        // La Station redémarre ou se met à jour : sondée toutes les 2 s jusqu'à son retour.
        if let Some(id) = app.station_waiting()
            && last_station_poll.elapsed() > Duration::from_secs(2)
            && let Some(h) = app.store.host(id)
        {
            last_station_poll = Instant::now();
            jobs::poll(h.clone(), false, tx.clone());
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
            // La température de la puce (Pi), arrondie au degré pour ne pas redessiner sans cesse.
            let t = if app.machine.pi.is_some() { system::temperature().map(|t| t.round()) } else { None };
            if n != app.net || b != app.machine.battery || t != app.machine.temp_c {
                app.net = n;
                app.machine.battery = b;
                app.machine.temp_c = t;
                dirty = true;
            }
        }

        // Les écrans de l'appareil hors session : relus toutes les deux secondes, ou à un événement de SDL.
        if live.is_none() && (rescan_now || last_scan.elapsed() > Duration::from_secs(2)) {
            last_scan = Instant::now();
            if refresh_devices(&mut app, &mut devices, &mut wins) {
                dirty = true;
            }
        }
        // Multi-écrans : ce que la session prendra pour l'ordinateur choisi (accueil, réglages).
        if live.is_none() && app.multi_screens() {
            let plan = selected_plan(&app, &devices, wins.list[0].window);
            if plan != app.screens_plan {
                app.screens_plan = plan;
                dirty = true;
            }
        }

        // Les images décodées qui attendent partent à la texture de leur fenêtre.
        let mut new_frame = None;
        if let Some(l) = live.as_mut() {
            let primary = l.handle.shared.primary();
            for w in wins.list.iter_mut() {
                let latest = l.handle.shared.latest_for(w.screen);
                let Some(f) = latest.take() else { continue };
                // La latence, l'image réduite de l'accueil et la taille décodée : l'écran principal seulement.
                let is_primary = w.screen == primary;
                if is_primary && l.snapshot_at.is_none_or(|t| t.elapsed() > Duration::from_secs(2)) {
                    // Une image toutes les 2 s, réduite, pour l'accueil (dernière image de la session).
                    l.snapshot_at = Some(Instant::now());
                    l.snapshot = video::snapshot(&f, 640);
                }
                let first = !w.has_frame();
                if w.video.upload(w.renderer, &f) {
                    if is_primary {
                        if l.decoded != Some((f.width, f.height)) {
                            l.decoded = Some((f.width, f.height));
                            l.handle.session.decoded_size(f.width, f.height);
                        }
                        new_frame = Some((f.pts, f.received, f.decoded));
                    }
                    w.present = true;
                    // La première image de cet écran : sa fenêtre passe du fond de la connexion à l'image.
                    if first {
                        dirty = true;
                    }
                }
                latest.give_back(f);
            }
        }

        if animating {
            dirty = true;
        }
        // Ce que montre chaque fenêtre : l'interface (ou l'image seule), et si seul le trait de
        // la barre se voit : il ne change pas, on ne le redessine ni ne le renvoie à chaque
        // seconde, et seul son bandeau se pose sur l'image.
        let views: Vec<(ui::View, bool, bool)> = (0..wins.list.len())
            .map(|wi| {
                let view = view_of(&app, live.as_ref(), &wins.list[wi], wi);
                (view, app.screen != Screen::Session || ui::overlay_visible(&app, view), ui::hint_only(&app, view))
            })
            .collect();
        let hits_win = hits_window(&app, live.as_ref());
        let hover = live.as_ref().map_or(0, |l| l.hover);
        for (wi, (view, show_ui, hint_only)) in views.iter().copied().enumerate() {
            // La texture porte autre chose (une capsule qui vient de passer…) : le trait y revient.
            let redraw = dirty || (hint_only && wins.list[wi].hint_key.is_none());
            if !redraw {
                continue;
            }
            let win = &mut wins.list[wi];
            win.sizes();
            let k = win.k(app.layout);
            win.cv.resize(win.out_size.0 as u32, win.out_size.1 as u32, k);
            if show_ui {
                let key = (win.cv.pix.width(), win.cv.pix.height(), win.cv.k.to_bits());
                if !(hint_only && win.hint_key == Some(key) && win.ui_size == (key.0, key.1)) {
                    app.pads = pads.infos();
                    // Le pointeur n'est que dans une fenêtre : les autres n'en montrent pas le survol.
                    let pointer = app.pointer;
                    if wi != hover {
                        app.pointer = None;
                    }
                    let hits = ui::draw_view(&app, &mut win.cv, view);
                    app.pointer = pointer;
                    if wi == hits_win {
                        app.hits = hits;
                    }
                    upload_ui(win);
                    win.hint_key = hint_only.then_some(key);
                }
            } else if wi == hits_win {
                app.hits.clear();
            }
            win.present = true;
        }
        if dirty {
            update_hit_areas(&app, &wins.list[0]);
            dirty = false;
        }
        // Les autres fenêtres d'abord (sans attendre la synchro verticale), celle de l'app
        // ensuite : c'est elle qui suit le réglage « Affichage des images ».
        let order: Vec<usize> = (1..wins.list.len()).chain(0..1).collect();
        for wi in order {
            if !(present || wins.list[wi].present) {
                continue;
            }
            let (_, show_ui, hint_only) = views[wi];
            let win = &mut wins.list[wi];
            let strip = hint_only.then(|| (ui::HINT_STRIP * win.cv.k).ceil() as c_int);
            draw_frame(win, show_ui, strip, live.as_ref(), &app);
            win.present = false;
            if wi == 0
                && let (Some((pts, received, decoded)), Some(l)) = (new_frame, live.as_mut())
            {
                let at = latency::now_micros();
                l.handle.session.frame_displayed(pts, received, decoded, at);
                l.last_frame = Some(Instant::now());
                l.shown = Some((pts, received, decoded));
            }
        }
        present = false;
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

fn upload_ui(win: &mut Win) {
    let (w, h) = (win.cv.pix.width(), win.cv.pix.height());
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
        if sdl::SDL_UpdateTexture(win.ui, std::ptr::null(), win.cv.bytes().as_ptr() as *const c_void, (w * 4) as c_int) != 0 {
            log::warn!("interface : envoi de la texture impossible ({})", sdl::error());
        }
    }
}

/// Les zones de la fenêtre de l'app (déplacer, redimensionner) lues par le test de SDL.
fn update_hit_areas(app: &App, win: &Win) {
    let cv = &win.cv;
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
/// l'interface si elle est montrée (`strip` : seulement ce bandeau du haut,
/// en pixels), le pointeur de l'ordinateur.
fn draw_frame(win: &mut Win, show_ui: bool, strip: Option<c_int>, live: Option<&Live>, app: &App) {
    unsafe {
        sdl::SDL_SetRenderDrawColor(win.renderer, 0, 0, 0, 255);
        sdl::SDL_RenderClear(win.renderer);
        let in_session = app.screen == Screen::Session && app.session.as_ref().is_some_and(|s| matches!(s.phase, Phase::Streaming | Phase::Reconnecting { .. } | Phase::Ending));
        win.video_rect = if in_session { win.video.draw(win.renderer, win.out_size) } else { None };
        if show_ui && !win.ui.is_null() {
            match strip {
                // La texture a la taille de la sortie : le bandeau se pose où il est.
                Some(h) => {
                    let r = sdl::SDL_Rect { x: 0, y: 0, w: win.ui_size.0 as c_int, h: h.min(win.ui_size.1 as c_int) };
                    sdl::SDL_RenderCopy(win.renderer, win.ui, &r, &r);
                }
                None => {
                    sdl::SDL_RenderCopy(win.renderer, win.ui, std::ptr::null(), std::ptr::null());
                }
            }
        }
        // Le pointeur de l'ordinateur, dessiné par l'app : souris des jeux
        // (menus d'un jeu), console (pavés tactiles, manette en souris).
        let s = app.session.as_ref();
        let draw_cursor = s.is_some_and(|s| !s.menu && !s.keys && s.phase == Phase::Streaming && (s.relative || s.pad_mouse || app.layout.console()));
        // Chaque fenêtre ne dessine que le pointeur annoncé sur son écran (multi-écrans).
        let (pointer, pointer_hidden) = live.and_then(|l| l.pointers.get(&win.screen)).copied().unwrap_or((None, true));
        if let (true, Some(l), Some(dst)) = (draw_cursor, live, win.video_rect)
            && !win.cursor.is_null()
            && (!pointer_hidden || s.is_some_and(|s| s.pad_mouse))
            && let Some((x, y)) = pointer
        {
            let (hw, hh) = l.handle.shared.host_size_of(win.screen);
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
/// bureau, barre du bureau, curseur du système, écrans de l'appareil.
fn session_tick(app: &mut App, l: &mut Live, wins: &mut Wins, dirty: &mut bool, present: &mut bool) {
    let layout = app.layout;
    let Some(s) = app.session.as_mut() else { return };
    let streaming = matches!(s.phase, Phase::Streaming);
    // Souris : Auto suit le jeu en plein écran et le pointeur caché (sur tous les écrans).
    let (remote_hidden, visible_for) = l.handle.shared.pointer_summary();
    let current = s.relative;
    let wanted = streaming && !s.menu && !s.keys && !s.bar_menu.is_some() && MOUSE_RULES.wants_relative(l.mouse, l.game_fullscreen, remote_hidden, visible_for, current);
    if wanted != current {
        s.relative = wanted;
        l.handle.shared.relative.store(wanted, Ordering::Relaxed);
        if let Some(p) = l.handle.shared.current_input() {
            p.set_relative(wanted);
        }
        unsafe { sdl::SDL_SetRelativeMouseMode(wanted as c_int) };
        log::info!("souris : {}", if wanted { "jeu (relative)" } else { "bureau" });
        // En souris des jeux, le pointeur est capturé : la barre ne vient plus à lui.
        if wanted {
            l.bar_arm = None;
        }
        // Pas à l'arrivée de l'image (rien par-dessus elle) : seulement quand le jeu change de mode ensuite.
        if wanted && layout == Layout::Desktop && s.opened_at.is_some_and(|t| t.elapsed() > Duration::from_secs(3)) {
            s.notice = Some((tr("Souris des jeux · Échap longtemps ou Ctrl+Alt+Maj+M pour rendre la souris", "Game mouse · hold Esc or Ctrl+Alt+Shift+M to release the mouse").into(), Instant::now()));
        }
        *dirty = true;
    }
    // Pointeur du système : forme de l'ordinateur (bureau, souris du bureau),
    // caché ailleurs (l'app dessine celui de l'ordinateur).
    // Bureau, souris du bureau : toujours visible. Windows masque son pointeur
    // par intermittence (sans souris branchée, à l'arrêt) ; le suivre ferait
    // perdre le curseur au repos.
    let show_sys = !streaming || s.menu || s.keys || s.bar || (layout == Layout::Desktop && !s.relative);
    // Multi-écrans : le pointeur du système prend la forme que l'ordinateur a annoncée, dans la fenêtre survolée.
    let hovered = l.hover.min(wins.list.len() - 1);
    unsafe {
        sdl::SDL_ShowCursor(show_sys as c_int);
        if !wins.list[hovered].sys_cursor.is_null() && streaming && !s.menu && !s.keys && !s.bar {
            sdl::SDL_SetCursor(wins.list[hovered].sys_cursor);
        } else {
            sdl::SDL_SetCursor(sdl::SDL_GetDefaultCursor());
        }
    }
    // Barre du bureau, ouverture : le pointeur est resté un quart de seconde
    // contre le bord haut, dans la zone de sa largeur ; la fenêtre a toujours le
    // clavier, la souris est celle du bureau, aucun bouton n'est tenu (une fenêtre
    // de l'ordinateur glissée vers le haut) ; un passage rapide n'ouvre rien.
    // Plusieurs écrans : sur l'écran où le geste a lieu.
    if let Some(at) = l.bar_arm
        && Instant::now() >= at
    {
        l.bar_arm = None;
        let wi = l.bar_arm_win.min(wins.list.len() - 1);
        let wpts = wins.list[wi].width_points(layout);
        let here = l.hover == wi && app.pointer.is_some_and(|(ux, uy)| ui::at_bar_edge(&mut wins.list[wi].cv, s, wpts, ux, uy));
        if layout == Layout::Desktop && matches!(s.phase, Phase::Streaming | Phase::Reconnecting { .. }) && !s.menu && !s.keys && !s.bar && !s.relative && here && wins.any_focused() && !sdl::mouse_held() {
            s.bar = true;
            l.bar_win = wi;
            l.bar_hide = None;
            // La fenêtre où la barre s'ouvre devient la fenêtre active (le clavier y va).
            if !wins.list[wi].focused() {
                unsafe { sdl::SDL_RaiseWindow(wins.list[wi].window) };
            }
            *dirty = true;
        }
    }
    // Fermeture : 400 ms après que le pointeur a quitté la barre, jamais pendant
    // un de ses menus. Un menu qui se ferme pointeur dehors, ou un pointeur sorti
    // de la fenêtre sans mouvement, arment le même délai.
    if s.bar && s.bar_menu.is_none() && l.bar_hide.is_none() {
        let bw = l.bar_win.min(wins.list.len() - 1);
        let wpts = wins.list[bw].width_points(layout);
        let over = l.hover == bw && app.pointer.is_some_and(|(ux, uy)| ui::over_bar(&mut wins.list[bw].cv, s, wpts, &app.hits, ux, uy));
        if !over {
            l.bar_hide = Some(Instant::now() + Duration::from_millis(400));
        }
    }
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

    // Multi-écrans : les écrans de l'appareil relus toutes les secondes (branché, débranché, disposition
    // ou recopie changée) ; un changement qui tient 0,6 s refait la session avec les nouveaux écrans.
    if l.multi && l.rescan_at.elapsed() > Duration::from_secs(1) {
        l.rescan_at = Instant::now();
        let d = screens::detect();
        if displays::same_layout(&d.grouped, &l.devices.grouped) {
            l.changed = None;
        } else if l.changed.as_ref().is_none_or(|(c, _)| !displays::same_layout(&c.grouped, &d.grouped)) {
            l.changed = Some((d, Instant::now()));
        }
    }
    if l.changed.as_ref().is_some_and(|(_, since)| since.elapsed() > Duration::from_millis(600))
        && let Some((d, _)) = l.changed.take()
    {
        apply_devices(app, l, wins, d);
        *dirty = true;
    }
}

/// Les écrans de l'appareil ont changé en session : le cœur les reçoit
/// (`Session::set_screens`) et, si son plan change, refait la session : la
/// connexion reprend à sa première étape sur l'écran principal. Le cœur est la
/// seule source du plan : les fenêtres de plus s'ouvrent et se ferment à ce qu'il
/// annonce (`follow_screens`). Un plan qui ne change pas (écran branché que la
/// session ne prend pas, fréquence changée, portable posé sur un dock en Jeu…) ne
/// touche ni aux fenêtres, ni aux images, ni au pointeur : le cœur ne refait rien et
/// aucune image clé n'arriverait pour remplacer ce qu'on aurait oublié.
fn apply_devices(app: &mut App, l: &mut Live, wins: &mut Wins, d: screens::Detected) {
    log::info!("écrans de l'appareil changés : {} écran(s)", d.grouped.len());
    log_devices(&d);
    let old = std::mem::replace(&mut l.devices, d);
    let Some(h) = app.store.host(&l.host).cloned() else { return };
    let Some(base) = l.screens_base.clone() else { return };
    let before = plan_of(l);
    let current = l.devices.current(wins.list[0].window);
    // Ce qui décide du nombre d'écrans (le réglage « Écrans », une définition sur mesure, ce que la
    // Station crée) est celui du départ de la session : il ne change qu'à la session suivante.
    let screens = session_screens(app, &h, &l.devices, current, &base);
    l.screens_base = Some(screens.clone());
    let after = plan_of(l);
    l.handle.session.set_screens(screens);
    if before == after {
        log::info!("écrans de l'appareil changés : le plan de la session ne change pas");
        return;
    }
    // Plan différent : le cœur refait la session (« Préparation » efface les images, puis ses écrans
    // arrivent). Les fenêtres de plus dont l'écran n'est plus au même endroit se ferment tout de suite
    // (leur indice a pu changer de sens) ; celles qui manquent s'ouvrent à l'annonce du cœur.
    wins.close_extras_where(|w| !displays::same_screen(&old.grouped, &l.devices.grouped, w.screen));
    l.placed = None;
    l.not_created.clear();
}

/// Les écrans de l'appareil au format du cœur, avec ce qui décide de leur nombre tel que la session
/// l'a reçu au départ (`base`) : le réglage « Écrans » changé dans les réglages pendant la session
/// vaut pour la suivante.
fn session_screens(app: &App, h: &crate::model::Host, devices: &screens::Detected, current: usize, base: &kybernow_core::screens::Screens) -> kybernow_core::screens::Screens {
    kybernow_core::screens::Screens { list: screens::device_screens(devices, h, app.machine.on_battery()), current, ..base.clone() }
}

/// « Cet écran » de la machine (définition, fréquence, nom) : celui de l'appareil qui porte la fenêtre.
fn set_machine_screen(app: &mut App, devices: &screens::Detected, screen: usize) {
    if let Some(d) = devices.grouped.get(screen) {
        app.machine.display = (d.pixel_w, d.pixel_h);
        app.machine.refresh = d.refresh;
        app.machine.display_name = d.name.clone();
    }
}

/// Ce que la Station dit de ses écrans pour cet ordinateur : ce qu'elle sait créer, et son écran
/// virtuel (inconnu tant qu'elle n'a pas répondu : une Station antérieure, un seul écran).
fn station_screens(app: &App, h: &crate::model::Host) -> screens::StationScreens {
    screens::StationScreens { max_screens: app.live.get(&h.id).and_then(|l| l.max_screens).unwrap_or(1), virtual_display_available: app.virtual_display_available(h) }
}

/// Les réglages du flux que la session enverra pour ce profil, avec l'écran et la batterie
/// d'à présent : le même objet va à `session::start` et au calcul du nombre d'écrans, qui est celui
/// du cœur (accueil, réglages et session).
fn stream_settings(app: &App, h: &crate::model::Host, profile: crate::model::Profile, light: bool) -> StreamSettings {
    session::settings(h, profile, app.codec_for(h), app.session_mode(h), &app.machine, light)
}

/// Les écrans virtuels que le cœur demande en ce moment, ou `None` : une session à un écran. Son
/// calcul (`screens::session_plan_for`) sur ce qu'il a reçu : écrans, réglages du flux, écran virtuel.
fn plan_of(l: &Live) -> Option<Vec<kybernow_core::screens::PlannedScreen>> {
    screens::session_plan(l.screens_base.as_ref()?, &l.settings, l.virtual_display)
}

/// Le cœur reçoit de nouveaux réglages du flux (profil choisi dans la barre ou le menu, « Alléger ») :
/// il en tire tout de suite le nombre d'écrans et refait la session s'il change. Ce que la machine
/// décode (4:4:4 remis à zéro en choisissant Jeu) décide aussi du « Personnalisé » du nombre d'écrans :
/// s'il a changé, le cœur le reçoit avec les écrans. Une session à un écran n'envoie que les réglages.
fn push_settings(l: &mut Live, settings: StreamSettings, capabilities: Capabilities) {
    if let Some(base) = l.screens_base.as_mut()
        && base.capabilities != Some(capabilities)
    {
        base.capabilities = Some(capabilities);
        l.handle.session.set_screens(base.clone());
    }
    l.settings = settings.clone();
    l.handle.session.set_settings(settings);
}

/// La fenêtre de l'app passe en plein écran sur l'écran `primary` de l'appareil : une session à
/// plusieurs écrans l'y met (l'écran principal porte la connexion), et une session qui redevient à
/// un écran l'y laisse ou la met sur l'écran du flux (« Cet écran » du cœur). La fin de session la
/// remet comme elle était.
fn take_over_main_window(app: &mut App, l: &mut Live, wins: &mut Wins, primary: usize) {
    let window = wins.list[0].window;
    if wins.prev.is_none() {
        let (mut x, mut y, mut w, mut h) = (0, 0, 0, 0);
        unsafe {
            sdl::SDL_GetWindowPosition(window, &mut x, &mut y);
            sdl::SDL_GetWindowSize(window, &mut w, &mut h);
        }
        let flags = unsafe { sdl::SDL_GetWindowFlags(window) };
        wins.prev = Some(PrevWindow { fullscreen: app.fullscreen, maximized: flags & sdl::SDL_WINDOW_MAXIMIZED != 0, position: (x, y), size: (w, h) });
    }
    if l.placed != Some(primary) {
        place_fullscreen(window, l.devices.sdl_index(primary));
        l.placed = Some(primary);
    }
    app.fullscreen = true;
    unsafe { sdl::SDL_SetWindowKeyboardGrab(window, 1) };
}

/// Le cœur annonce les écrans de la session (au démarrage de l'image, ou l'un d'eux a changé de
/// taille) : il est la seule source du plan, les fenêtres le suivent. Une fenêtre par écran pris ou
/// raté (leur fenêtre le dit) ; une session à un écran (un seul flux et aucun écran raté : une Station
/// qui n'en crée qu'un, l'écran principal seul accepté, un profil à un écran) n'a que la fenêtre de
/// l'app, qui montre ce flux : jamais de fenêtre de plus, sans message, comme sur Mac.
fn follow_screens(app: &mut App, l: &mut Live, wins: &mut Wins) {
    let st = l.handle.shared.screens();
    let Some(primary) = st.streams.first().map(|s| s.screen) else { return };
    // Les écrans pris : le même compte partout (barre, dernière session), sans ceux qu'il n'a pas pu créer.
    l.screens = st.streams.len().max(1);
    l.not_created = st.not_created.clone();
    let mut wanted: Vec<usize> = st.streams.iter().map(|s| s.screen).collect();
    wanted.extend(st.not_created.iter().copied());
    if let Some(s) = app.session.as_mut() {
        s.screens = l.screens;
        s.expected_screens = wanted.len().max(1);
    }
    if l.multi {
        // La fenêtre de l'app est en plein écran sur l'écran principal du flux tant qu'il y a plusieurs
        // écrans, et s'ils redeviennent un seul (profil changé) : elle montre alors l'écran du flux, celui
        // que le cœur a pris comme « Cet écran » (sa définition, son mode).
        if wanted.len() >= 2 || wins.prev.is_some() {
            take_over_main_window(app, l, wins, primary);
        }
        sync_windows(wins, &wanted, primary, &l.devices, true);
    }
    log::info!("écrans de la session : {} pris, {} non créé(s)", st.streams.len(), st.not_created.len());
}

/// Les écrans de l'appareil au journal (une ligne par écran).
fn log_devices(d: &screens::Detected) {
    for (i, s) in d.grouped.iter().enumerate() {
        log::info!(
            "écran {i}{} : {} {} × {} à ({}, {}), {} × {} px (échelle {:.2}) à {} Hz",
            if i == 0 { " (principal)" } else { "" },
            s.name,
            s.w,
            s.h,
            s.x,
            s.y,
            s.pixel_w,
            s.pixel_h,
            s.scale(),
            s.refresh
        );
    }
    if d.all.len() > d.grouped.len() {
        log::info!("{} écran(s) en recopie regroupé(s)", d.all.len() - d.grouped.len());
    }
}

/// Hors session : les écrans de l'appareil relus. Rend vrai quand l'interface doit se redessiner.
/// « Cet écran » (celui de la fenêtre) donne la définition et la fréquence de la machine.
fn refresh_devices(app: &mut App, devices: &mut screens::Detected, wins: &mut Wins) -> bool {
    let d = screens::detect();
    let mut changed = false;
    if !displays::same_layout(&d.grouped, &devices.grouped) {
        log::info!("écrans de l'appareil : {} écran(s)", d.grouped.len());
        log_devices(&d);
        *devices = d;
        changed = true;
    }
    let current = devices.current(wins.list[0].window);
    wins.list[0].screen = current;
    if let Some(s) = devices.grouped.get(current) {
        let size = (s.pixel_w, s.pixel_h);
        if app.machine.display != size || app.machine.refresh != s.refresh || app.machine.display_name != s.name {
            app.machine.display = size;
            app.machine.refresh = s.refresh;
            app.machine.display_name = s.name.clone();
            changed = true;
        }
    }
    changed
}

/// Ce que la session prendra pour l'ordinateur choisi, avec les écrans de l'appareil d'à présent.
fn selected_plan(app: &App, devices: &screens::Detected, window: *mut sdl::SDL_Window) -> crate::app::ScreensPlan {
    let Some(h) = app.selected() else { return crate::app::ScreensPlan::default() };
    let current = devices.current(window);
    // Les mêmes réglages du flux que la session enverra (`Action::Connect`), le même calcul du cœur.
    let light = h.settings.battery_light && app.machine.on_battery();
    let settings = stream_settings(app, h, h.settings.profile, light);
    let cs = screens::core_screens(devices, current, h, station_screens(app, h), session::capabilities(h, app.codec_for(h), &app.machine), app.machine.on_battery());
    screens::summary(&cs, &settings, h.settings.virtual_display)
}

/// Ce que montre une fenêtre (voir `ui::View`) : celle d'une session à plusieurs écrans a son rôle ;
/// une seule fenêtre montre tout.
fn view_of(app: &App, live: Option<&Live>, win: &Win, wi: usize) -> ui::View {
    let multi = app.session.as_ref().is_some_and(SessionUi::multi);
    let Some(l) = live.filter(|_| multi) else { return ui::View::SINGLE };
    ui::View { main: wi == 0, bar: wi == l.bar_win, missing: l.not_created.contains(&win.screen), has_frame: win.has_frame() }
}

/// La fenêtre dont les éléments (boutons, menus) se choisissent : celle de l'app, ou, avec
/// plusieurs écrans, celle où la barre est ouverte.
fn hits_window(app: &App, live: Option<&Live>) -> usize {
    match (live, app.session.as_ref()) {
        (Some(l), Some(s)) if s.multi() && s.bar && matches!(s.phase, Phase::Streaming | Phase::Reconnecting { .. }) => l.bar_win,
        _ => 0,
    }
}

/// Un événement de SDL dit que les écrans de l'appareil ont peut-être changé.
static RESCAN: AtomicBool = AtomicBool::new(false);

/// La session à plusieurs écrans est finie et son message lu : la fenêtre de l'app redevient ce
/// qu'elle était (fenêtre, position, taille, plein écran). Tant qu'un échec s'affiche, elle reste
/// en plein écran sur l'écran principal.
fn restore_main(app: &mut App, wins: &mut Wins) {
    let Some(p) = wins.prev.take() else { return };
    let window = wins.list[0].window;
    unsafe {
        sdl::SDL_SetWindowFullscreen(window, if p.fullscreen { sdl::SDL_WINDOW_FULLSCREEN_DESKTOP } else { 0 });
        if !p.fullscreen {
            sdl::SDL_SetWindowSize(window, p.size.0, p.size.1);
            sdl::SDL_SetWindowPosition(window, p.position.0, p.position.1);
            if p.maximized {
                sdl::SDL_MaximizeWindow(window);
            }
        }
    }
    app.fullscreen = p.fullscreen;
}

/// Position de la souris (fenêtre `win`) sur l'écran de l'ordinateur de son écran, en pixels de cet écran.
fn host_point(win: &Win, l: &Live, x: i32, y: i32) -> Option<(i16, i16)> {
    let dst = win.video_rect?;
    let d = win.density();
    let (px, py) = (x as f32 * d, y as f32 * d);
    let (hw, hh) = l.handle.shared.host_size_of(win.screen);
    let hx = ((px - dst.x as f32) / dst.w.max(1) as f32 * hw as f32).clamp(0.0, hw as f32 - 1.0);
    let hy = ((py - dst.y as f32) / dst.h.max(1) as f32 * hh as f32).clamp(0.0, hh as f32 - 1.0);
    Some((hx as i16, hy as i16))
}

#[allow(clippy::too_many_arguments)]
fn handle_sdl(ev: &sdl::SDL_Event, app: &mut App, pads: &mut pads::Pads, mut live: Option<&mut Live>, wins: &mut Wins, actions: &mut Vec<Action>, dirty: &mut bool, quit: &mut bool, start_down: &mut Option<Instant>) {
    let kind = ev.kind();
    if kind == video::FRAME_EVENT.load(Ordering::Relaxed) {
        return;
    }
    // La fenêtre que vise l'événement (fenêtre, clavier, texte, souris, molette) : une
    // fenêtre fermée entre-temps n'en reçoit plus.
    let on_window = matches!(kind, sdl::SDL_WINDOWEVENT | sdl::SDL_KEYDOWN | sdl::SDL_KEYUP | sdl::SDL_TEXTINPUT | sdl::SDL_MOUSEMOTION | sdl::SDL_MOUSEBUTTONDOWN | sdl::SDL_MOUSEBUTTONUP | sdl::SDL_MOUSEWHEEL);
    let wi = match (on_window, wins.list.len()) {
        (false, _) | (_, 1) => 0,
        (true, _) => match wins.index_of(ev.window_id()) {
            Some(i) => i,
            None => return,
        },
    };
    let in_game = app.screen == Screen::Session && app.session.as_ref().is_some_and(|s| !s.menu && !s.keys && matches!(s.phase, Phase::Streaming | Phase::Reconnecting { .. }));
    let input = live.as_ref().and_then(|l| l.handle.shared.current_input());
    match kind {
        sdl::SDL_QUIT => {
            if live.is_some() {
                actions.push(Action::Disconnect);
            }
            *quit = true;
        }
        // Un écran branché, débranché ou déplacé : les écrans de l'appareil sont relus.
        sdl::SDL_DISPLAYEVENT => RESCAN.store(true, Ordering::Relaxed),
        sdl::SDL_WINDOWEVENT => {
            let (e, _, _) = ev.window();
            match e {
                sdl::SDL_WINDOWEVENT_FOCUS_LOST => {
                    // Le clavier passe à une autre fenêtre de l'app (plusieurs écrans) : les touches tenues restent tenues.
                    let focus = unsafe { sdl::SDL_GetKeyboardFocus() };
                    let elsewhere = !focus.is_null() && wins.index_of(unsafe { sdl::SDL_GetWindowID(focus) }).is_some_and(|i| i != wi);
                    if !elsewhere && let Some(p) = &input {
                        p.release_all();
                    }
                    if let Some(l) = live.as_deref_mut() {
                        l.bar_arm = None;
                    }
                }
                sdl::SDL_WINDOWEVENT_ENTER => {
                    if let Some(l) = live.as_deref_mut() {
                        l.hover = wi;
                    }
                }
                sdl::SDL_WINDOWEVENT_LEAVE => {
                    if app.layout == Layout::Desktop {
                        // Une autre fenêtre a déjà le pointeur : son mouvement a posé le sien.
                        if live.as_ref().is_none_or(|l| l.hover == wi) {
                            app.pointer = None;
                        }
                        // Le pointeur quitte la fenêtre : plus d'ouverture de la barre ; ouverte, elle se referme.
                        if let Some(l) = live.as_deref_mut() {
                            if l.hover == wi {
                                l.bar_arm = None;
                            }
                            if app.session.as_ref().is_some_and(|s| s.bar) && l.bar_win == wi {
                                l.bar_hide.get_or_insert(Instant::now() + Duration::from_millis(400));
                            }
                        }
                    }
                }
                // Fermer une fenêtre de la session (plusieurs écrans), c'est se déconnecter.
                sdl::SDL_WINDOWEVENT_CLOSE => {
                    if live.is_some() && wins.list.len() > 1 {
                        actions.push(Action::Disconnect);
                    }
                }
                sdl::SDL_WINDOWEVENT_DISPLAY_CHANGED => RESCAN.store(true, Ordering::Relaxed),
                sdl::SDL_WINDOWEVENT_MAXIMIZED if wi == 0 => app.maximized = true,
                sdl::SDL_WINDOWEVENT_RESTORED if wi == 0 => app.maximized = false,
                _ => {}
            }
            *dirty = true;
        }
        sdl::SDL_RENDER_TARGETS_RESET | sdl::SDL_RENDER_DEVICE_RESET => {
            wins.invalidate();
            *dirty = true;
        }
        sdl::SDL_KEYDOWN | sdl::SDL_KEYUP => {
            let down = kind == sdl::SDL_KEYDOWN;
            let (sc, repeat, mods) = ev.key();
            if app.screen == Screen::Standby {
                // Une touche réveille.
                if down && !repeat {
                    actions.extend(app.back());
                    *dirty = true;
                }
                return;
            }
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
            if app.screen == Screen::Add && matches!(app.add.step, AddStep::Address | AddStep::Mac) {
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
                // Barre du bureau. Fermée : elle s'ouvre quand le pointeur reste un quart de
                // seconde contre le bord haut, dans la zone de sa largeur (`session_tick`
                // mesure le délai : un pointeur immobile n'envoie plus d'événements) ;
                // le quitter annule. Ouverte : elle reste sur elle, ou pendant un menu.
                l.hover = wi;
                if app.layout == Layout::Desktop {
                    let (ux, uy) = to_ui(&wins.list[wi], app.layout, x, y);
                    let wpts = wins.list[wi].width_points(app.layout);
                    if s.bar {
                        // Plusieurs écrans : la barre n'est que dans la fenêtre où elle s'est ouverte.
                        if wi == l.bar_win && (s.bar_menu.is_some() || ui::over_bar(&mut wins.list[wi].cv, s, wpts, &app.hits, ux, uy)) {
                            l.bar_hide = None;
                            if app.pointer_moved(ux, uy) {
                                *dirty = true;
                            }
                            return;
                        }
                        app.pointer = Some((ux, uy));
                        if l.bar_hide.is_none() {
                            l.bar_hide = Some(Instant::now() + Duration::from_millis(400));
                        }
                    } else {
                        app.pointer = Some((ux, uy));
                        if ui::at_bar_edge(&mut wins.list[wi].cv, s, wpts, ux, uy) && wins.any_focused() && !sdl::mouse_held() {
                            l.bar_arm_win = wi;
                            l.bar_arm.get_or_insert(Instant::now() + Duration::from_millis(250));
                        } else {
                            l.bar_arm = None;
                        }
                    }
                }
                if let (Some(p), Some((hx, hy))) = (&input, host_point(&wins.list[wi], l, x, y)) {
                    // Multi-écrans : la position part avec l'écran de cette fenêtre.
                    p.mouse_on(wins.list[wi].screen, hx, hy);
                    l.pointers.entry(wins.list[wi].screen).or_insert((None, true)).0 = Some((hx as f64, hy as f64));
                    wins.list[wi].present = true;
                }
                return;
            }
            let (ux, uy) = to_ui(&wins.list[wi], app.layout, x, y);
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
                let (ux, uy) = to_ui(&wins.list[wi], app.layout, x, y);
                // Un toucher en haut de l'écran (Deck) ouvre le menu.
                if touch && down && app.layout.console() && uy < wins.list[wi].cv.h * 0.06 {
                    actions.extend(app.open_menu());
                    *dirty = true;
                    return;
                }
                // Les éléments de la barre sont ceux de sa fenêtre (plusieurs écrans : une seule barre).
                let bar_win = live.as_ref().map_or(0, |l| l.bar_win);
                if s.bar && wi == bar_win && app.hits.iter().any(|(_, r)| r.contains(ux, uy)) {
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
                        && let Some((hx, hy)) = host_point(&wins.list[wi], l, x, y)
                    {
                        p.mouse_on(wins.list[wi].screen, hx, hy);
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
                let (ux, uy) = to_ui(&wins.list[wi], app.layout, x, y);
                // Double clic sur la barre de titre : agrandir.
                if ev.u8(18) == 2 && ui::title_drag_area(app, wins.list[wi].cv.w).is_some_and(|r| r.contains(ux, uy)) {
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
            if app.is_os() && is_button {
                if app.screen == Screen::Standby {
                    if down {
                        actions.extend(app.back());
                        *dirty = true;
                    }
                    return;
                }
                // ☰ hors session : un appui court ouvre les réglages (sur l'accueil)
                // au relâchement ; tenu trois secondes, il bascule Salon ⇄ Bureau
                // sans les ouvrir. En session (menu et écrans ouverts depuis elle
                // compris), Start va au jeu et à la combinaison, jamais à la bascule.
                if which == 6 && app.session.is_none() && app.screen != Screen::Launch {
                    if down {
                        *start_down = Some(Instant::now());
                    } else if let Some(t) = start_down.take() {
                        if t.elapsed() >= Duration::from_secs(3) {
                            app.store.app.interface = if app.store.app.interface == "desktop" { "tv".into() } else { "desktop".into() };
                            actions.push(Action::Save);
                            actions.push(Action::Interface);
                        } else {
                            actions.extend(app.nav(Nav::Menu));
                        }
                        *dirty = true;
                    }
                    return;
                }
                // Guide / PS sur l'accueil : la veille.
                if which == 5 && down && app.screen == Screen::Home {
                    actions.extend(app.activate(Target::Setting(crate::app::Setting::OsStandby)));
                    *dirty = true;
                    return;
                }
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
fn handle_event(e: Event, app: &mut App, pads: &mut pads::Pads, live: &mut Option<Live>, wins: &mut Wins, actions: &mut Vec<Action>, pairing: &mut Vec<Arc<AtomicBool>>) {
    match e {
        Event::HostInfo { id, online, os, gpu, av1, virtual_display, max_screens, station, wake, own_device } => {
            let entry = app.live.entry(id.clone()).or_insert_with(HostLive::default);
            entry.online = if online { Online::Online } else { Online::Offline };
            if online {
                entry.virtual_display = virtual_display;
                entry.max_screens = max_screens;
                entry.station = station;
                entry.own_device = own_device;
                // Le réveil a marché : l'ordinateur répond.
                if app.waking.as_ref().is_some_and(|(w, _)| *w == id) {
                    app.waking = None;
                }
            }
            app.station_progress(&id);
            if online && let Some(h) = app.store.host_mut(&id) {
                // Les cartes du réveil : les nouvelles, sauf une liste vide (Station 0.4.9).
                let mut changed = h.os != os || h.gpu != gpu || h.av1 != av1;
                if !wake.is_empty() && h.wake != wake {
                    h.wake = wake;
                    changed = true;
                }
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
            let was_streaming = matches!(s.phase, Phase::Streaming | Phase::Reconnecting { .. });
            s.phase = match st {
                Status::Preparing => Phase::Preparing,
                // La Station redémarre (dernier recours) : « Connexion » en attendant de le montrer.
                Status::Connecting => Phase::Connecting,
                Status::StationRestarting => Phase::StationRestarting,
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
                    // La session est finie : plus qu'une fenêtre ; une erreur se lit sur l'écran principal.
                    wins.close_extras();
                    Phase::Ended { error }
                }
            };
            // Plusieurs écrans : la connexion reprend à sa première étape (un profil qui change leur
            // nombre, des écrans branchés ou débranchés) : les images s'effacent, le fond revient.
            // Sans nouvelle image, un échec est celui de la connexion.
            if was_streaming && s.phase == Phase::Preparing {
                s.opened_at = None;
                wins.forget_frames();
                // Les écrans attendus : ceux du plan que le cœur refait (profil ou écrans de l'appareil
                // changés) ; ses fenêtres suivent son annonce, une fois la session prête.
                if let Some(l) = live.as_ref()
                    && l.multi
                {
                    s.expected_screens = plan_of(l).map_or(1, |p| p.len());
                }
            }
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
                l.hz = m.refresh;
            }
        }
        Event::StationReply { host, kind, result } => app.station_reply(&host, kind, result),
        Event::Woken { id, result } => {
            if let Err(e) = result {
                if app.waking.as_ref().is_some_and(|(w, _)| *w == id) {
                    app.waking = None;
                }
                app.toast(e);
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
            // Rien par-dessus l'image à son arrivée : une capsule seulement après les 3 premières secondes.
            if let Some(s) = app.session.as_mut()
                && s.opened_at.is_some_and(|t| t.elapsed() > Duration::from_secs(3))
            {
                s.notice = Some((n, Instant::now()));
            } else {
                log::info!("session : {n}");
            }
        }
        Event::Pointer { screen, position, hidden, shape } => {
            if let Some(l) = live.as_mut() {
                l.pointers.insert(screen, (position, hidden));
            }
            // Position et visibilité sont celles de l'écran ; la forme, la dernière annoncée, vaut pour toutes les fenêtres.
            if let Some((w, h, hx, hy, px)) = shape
                && w > 0
                && h > 0
            {
                for win in wins.list.iter_mut() {
                    set_cursor_shape(win, live.as_ref(), w, h, hx, hy, &px);
                }
            }
        }
        // Multi-écrans : les écrans que l'ordinateur a pris, et ceux qu'il n'a pas pu créer.
        Event::Screens => {
            let Some(l) = live.as_mut() else { return };
            follow_screens(app, l, wins);
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
        Event::Bt(list) => {
            app.setup.bt = list;
        }
        Event::Wifi(r) => {
            app.setup.wifi_scanning = false;
            match r {
                Ok(l) => app.setup.wifi = l,
                Err(e) => app.setup.net_message = Some(e),
            }
            if app.screen == Screen::Setup && app.setup.wifi_pick.is_none() {
                app.go(Screen::Setup);
            }
        }
        Event::WifiDone(r) => {
            app.setup.net_busy = false;
            match r {
                Ok(ssid) => {
                    app.setup.net_message = Some(format!("{} {ssid}", tr("Connecté à", "Connected to")));
                    app.net = system::net();
                    app.setup.wifi_pick = None;
                    if app.setup.active {
                        actions.extend(app.activate(Target::SetupContinue));
                    } else {
                        app.go(Screen::Setup);
                    }
                }
                Err(e) => app.setup.net_message = Some(e),
            }
        }
        Event::OsStatus(info) => app.os = Some(info),
        Event::OsApplied(r) => {
            app.os_update.applying = false;
            if let Err(e) = r {
                app.os_update.error = Some(e);
            }
        }
        Event::OsDownload(done, total) => app.os_update.downloading = Some((done, total)),
        Event::OsDownloaded(r) => {
            app.os_update.downloading = None;
            match r {
                Ok(p) => {
                    app.os_update.ready = Some(p);
                    app.toast(tr("Nouvelle version prête : Réglages de l'appareil › Mise à jour", "New version ready: Device settings › Update"));
                }
                Err(e) => app.os_update.error = Some(e),
            }
        }
        Event::OsLoading(p, net, bt) => {
            if let Some((label, progress)) = app.os_loading.as_mut() {
                *progress = p;
                let mut parts = Vec::new();
                if !net {
                    parts.push(tr("réseau", "network"));
                }
                if !bt {
                    parts.push("bluetooth");
                }
                *label = if parts.is_empty() { tr("prêt", "ready").into() } else { parts.join(" · ") };
            }
        }
        Event::OsReady => {
            if app.os_loading.take().is_some() {
                let hs = if app.reduce_motion { 0.45 } else { 1.8 };
                if app.launched.elapsed().as_secs_f32() > hs {
                    app.launched = Instant::now() - Duration::from_secs_f32(hs);
                }
            }
        }
        Event::Update(r) => {
            app.update = match r {
                Ok(Some(a)) => {
                    log::info!("mise à jour : version {} ({}) disponible, {}", a.version, a.channel.as_str(), a.page);
                    jobs::remember(a.clone());
                    if app.is_os() && app.store.app.os_auto_update && app.os_update.ready.is_none() && app.os_update.downloading.is_none() {
                        app.os_update.downloading = Some((0, a.size));
                        actions.push(Action::Os(OsAction::UpdateDownload));
                    }
                    crate::app::UpdateState::Available { version: a.version.clone(), channel: a.channel.as_str().into(), page: a.page.clone(), file: a.file.clone(), url: a.url.clone(), sha256: a.sha256.clone(), size: a.size }
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
fn set_cursor_shape(win: &mut Win, live: Option<&Live>, w: u32, h: u32, hx: u32, hy: u32, px: &[u32]) {
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
            (Some(l), Some(dst)) => (dst.w as f32 / l.handle.shared.host_size_of(win.screen).0.max(1) as f32 / win.density()).clamp(0.5, 3.0),
            _ => 1.0,
        };
        let (sw, sh) = (((w as f32 * scale).round() as u32).max(1), ((h as f32 * scale).round() as u32).max(1));
        win.cursor_pixels = scale_argb(px, w, h, sw, sh);
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
fn perform(a: Action, app: &mut App, pads: &mut pads::Pads, live: &mut Option<Live>, wins: &mut Wins, tx: &Sender<Event>, pairing: &mut Vec<Arc<AtomicBool>>, quit: &mut bool) {
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
            let layout = if app.is_os() { if app.store.app.interface == "desktop" { Layout::Desktop } else { Layout::Tv } } else { system::layout(&app.store.app.interface, None, display) };
            if layout != app.layout {
                app.layout = layout;
                app.keyboard_focus = layout.console();
                let full = layout.console();
                unsafe { sdl::SDL_SetWindowFullscreen(wins.main().window, if full { sdl::SDL_WINDOW_FULLSCREEN_DESKTOP } else { 0 }) };
                app.fullscreen = full && !app.is_os();
                if app.is_os() {
                    app.go(Screen::Home);
                } else {
                    app.open_settings(true, Screen::Home);
                }
            }
        }
        Action::Os(o) => match o {
            OsAction::BtListen(on) => {
                app.setup.bt_listening = os::listen(on, tx.clone());
            }
            OsAction::WifiScan => os::wifi_scan(tx.clone()),
            OsAction::WifiConnect { ssid, password } => os::wifi_connect(ssid, password, tx.clone()),
            OsAction::SetupDone => {
                os::listen(false, tx.clone());
            }
            OsAction::Status => {
                os::status(tx.clone());
                os::paired(tx.clone());
            }
            OsAction::UpdateDownload => jobs::download_os(tx.clone()),
            OsAction::UpdateApply => {
                if let Some(p) = app.os_update.ready.clone() {
                    os::apply(p, tx.clone());
                }
            }
            OsAction::Rollback => os::rollback(tx.clone()),
            OsAction::Ssh(on) => os::ctl(&["ssh", if on { "on" } else { "off" }]),
            OsAction::Reboot => os::ctl(&["reboot"]),
            OsAction::Poweroff => os::ctl(&["poweroff"]),
            // HDMI-CEC coupé : la veille de l'appareil reste, le téléviseur n'en sait rien.
            OsAction::Standby(on) => {
                if app.store.app.os_cec {
                    os::ctl(&["standby", if on { "on" } else { "off" }]);
                }
            }
            OsAction::Cec(on) => os::ctl(&[if on { "cec-name" } else { "cec-off" }]),
            OsAction::Hdmi(m) => os::ctl(&["hdmi", &m]),
        },
        Action::CheckUpdate => {
            app.update = crate::app::UpdateState::Checking;
            jobs::check_update(app.store.app.beta, tx.clone());
        }
        Action::OpenUrl(url) => open_url(&url),
        Action::Minimize => unsafe { sdl::SDL_MinimizeWindow(wins.main().window) },
        Action::Maximize => unsafe {
            if sdl::SDL_GetWindowFlags(wins.main().window) & sdl::SDL_WINDOW_MAXIMIZED != 0 {
                sdl::SDL_RestoreWindow(wins.main().window);
                app.maximized = false;
            } else {
                sdl::SDL_MaximizeWindow(wins.main().window);
                app.maximized = true;
            }
        },
        Action::ToggleFullscreen => {
            if app.layout.console() {
                return;
            }
            // Plusieurs écrans : « Plein écran » n'est pas proposé (barre, menu, raccourci) ; les fenêtres restent en plein écran.
            if app.session.as_ref().is_some_and(SessionUi::multi) {
                return;
            }
            app.fullscreen = !app.fullscreen;
            let window = wins.main().window;
            unsafe {
                sdl::SDL_SetWindowFullscreen(window, if app.fullscreen { sdl::SDL_WINDOW_FULLSCREEN_DESKTOP } else { 0 });
                // En plein écran pendant la session, les raccourcis du système vont à l'ordinateur.
                sdl::SDL_SetWindowKeyboardGrab(window, (app.fullscreen && live.is_some()) as c_int);
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
            // Les écrans de l'appareil à cet instant : « Cet écran » est celui de la fenêtre, dont la
            // définition et la fréquence donnent le mode d'une session à un écran.
            let devices = screens::detect();
            let window_screen = devices.current(wins.main().window);
            set_machine_screen(app, &devices, window_screen);
            let codec = app.codec_for(&h);
            let device = device_info(app);
            let light = h.settings.battery_light && app.machine.on_battery();
            let relative = h.settings.mouse == "game";
            let virtual_display = h.settings.virtual_display;
            // Les réglages du flux : ceux que reçoivent la Station et le calcul du nombre d'écrans.
            let settings = stream_settings(app, &h, profile, light);
            // Multi-écrans (interface de bureau) : le cœur reçoit les écrans de l'appareil et décide du
            // nombre d'écrans (son calcul, sur ces réglages) ; sans (Steam Deck, salon, KyberNow Link
            // OS), une session à un écran.
            let mut base = app.multi_screens().then(|| {
                let capabilities = session::capabilities(&h, codec, &app.machine);
                screens::core_screens(&devices, window_screen, &h, station_screens(app, &h), capabilities, app.machine.on_battery())
            });
            let plan = base.as_ref().and_then(|b| screens::session_plan(b, &settings, virtual_display));
            // « Cet écran » est celui où se trouve la fenêtre pendant la session : avec plusieurs écrans,
            // elle passe en plein écran sur l'écran principal, et c'est là que le cœur la met (l'écran de
            // la session à un écran où le profil changé la ramènerait, et son mode), comme sur Mac, qui
            // relit l'écran de la fenêtre après l'avoir posée. Une session à un écran : celui de la fenêtre.
            let current = plan.as_ref().map_or(window_screen, |p| p[0].device);
            if current != window_screen {
                set_machine_screen(app, &devices, current);
                if let Some(b) = base.as_mut() {
                    b.current = current;
                }
            }
            wins.main().screen = current;
            let mode = app.session_mode(&h);
            let primary_mode = plan.as_deref().and_then(screens::primary_mode).map_or(mode, |m| (m.width, m.height, m.refresh));
            let handle = session::start(&h, settings.clone(), mode, &app.machine, relative, device, base.clone(), current, tx.clone());
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
                screens: 1,
                expected_screens: plan.as_ref().map_or(1, Vec::len),
            });
            app.screen = Screen::Session;
            app.focus = Target::CancelConnect;
            let vsync = wants_vsync(app, &h);
            wins.main().set_vsync(vsync);
            let mut l = Live {
                handle,
                host: h.id.clone(),
                pointers: HashMap::new(),
                started: Instant::now(),
                latencies: Vec::new(),
                mbps: Vec::new(),
                mode: format!("{} × {}", primary_mode.0, primary_mode.1),
                hz: primary_mode.2,
                codec: codec_label,
                game_fullscreen: false,
                mouse: match h.settings.mouse.as_str() {
                    "desktop" => MouseMode::Desktop,
                    "game" => MouseMode::Game,
                    _ => MouseMode::Auto,
                },
                typing: false,
                escape_down: None,
                bar_hide: None,
                bar_arm: None,
                bar_arm_win: 0,
                bar_win: 0,
                hover: 0,
                decoded: None,
                last_frame: None,
                shown: None,
                snapshot: None,
                snapshot_at: None,
                multi: base.is_some(),
                devices,
                screens_base: base,
                settings,
                virtual_display,
                screens: 1,
                not_created: Vec::new(),
                placed: None,
                rescan_at: Instant::now(),
                changed: None,
            };
            // Plusieurs écrans : la fenêtre de l'app en plein écran sur l'écran principal, une fenêtre
            // plein écran de plus par autre écran (le fond de la connexion, dès le départ comme sur Mac) ;
            // ensuite le cœur annonce ce qu'il a pris et les fenêtres le suivent (`follow_screens`).
            wins.close_extras();
            if let Some(plan) = plan.as_deref() {
                take_over_main_window(app, &mut l, wins, plan[0].device);
                sync_windows(wins, &plan_screens(plan), plan[0].device, &l.devices, true);
            }
            unsafe {
                sdl::SDL_DisableScreenSaver();
                if app.fullscreen || app.layout.console() {
                    sdl::SDL_SetWindowKeyboardGrab(wins.main().window, 1);
                }
            }
            log::info!(
                "session : {} ({}, {} × {} à {} Hz, {codec}, {}, {} écran{})",
                h.name,
                profile.id(),
                primary_mode.0,
                primary_mode.1,
                primary_mode.2,
                if vsync { "synchronisé" } else { "immédiat" },
                plan.as_ref().map_or(1, Vec::len),
                if plan.as_ref().is_some_and(|p| p.len() > 1) { "s" } else { "" }
            );
            *live = Some(l);
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
            // La fin de session ferme toutes les fenêtres (celle de l'app reste).
            wins.close_extras();
            unsafe {
                sdl::SDL_SetRelativeMouseMode(0);
                sdl::SDL_ShowCursor(1);
                sdl::SDL_SetCursor(sdl::SDL_GetDefaultCursor());
            }
        }
        Action::SetProfile(p) => {
            // Le profil choisi dans la barre s'applique tout de suite : le cœur en tire le nombre
            // d'écrans (son calcul sur ces réglages) et refait la session s'il change ; ses fenêtres
            // suivent son annonce (`follow_screens`), sans que l'app ouvre ni ferme rien d'elle-même.
            if let (Some(l), Some(s)) = (live.as_mut(), app.session.as_ref())
                && let Some(h) = app.store.host(&s.host)
            {
                let settings = stream_settings(app, h, p, s.light);
                push_settings(l, settings, session::capabilities(h, app.codec_for(h), &app.machine));
            }
        }
        Action::Light(on) => {
            if let (Some(l), Some(s)) = (live.as_mut(), app.session.as_ref())
                && let Some(h) = app.store.host(&s.host)
            {
                let (w, hh, fps) = app.session_mode(h);
                let fps = if on { fps.min(60) } else { fps };
                let codec = app.codec_for(h);
                push_settings(l, session::settings(h, s.profile, codec, (w, hh, fps), &app.machine, on), session::capabilities(h, codec, &app.machine));
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
        Action::Poll => {
            for h in &app.store.hosts {
                jobs::poll(h.clone(), false, tx.clone());
            }
        }
        Action::Station { host, kind, force } => {
            if let Some(h) = app.store.host(&host).cloned() {
                log::info!("station : {} demandé{}", if kind == crate::app::StationKind::Update { "mise à jour" } else { "redémarrage" }, if force { " (session coupée)" } else { "" });
                jobs::station(h, kind, force, app.store.device_id.clone(), tx.clone());
            }
        }
        Action::Wake(id) => {
            if let Some(h) = app.store.host(&id).cloned() {
                jobs::wake(h, tx.clone());
            }
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

/// Fin de session : la dernière session (latence médiane et 95e centile, durée,
/// définition, fréquence, format, débit) pour Réglages › Statistiques.
fn finish(app: &mut App, wins: &mut Wins, l: Live) {
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
        sdl::SDL_SetWindowKeyboardGrab(wins.main().window, 0);
        sdl::SDL_EnableScreenSaver();
        sdl::SDL_StopTextInput();
    }
    // Les fenêtres de plus se ferment (plusieurs écrans) ; celle de l'app n'a plus d'image.
    wins.close_extras();
    wins.main().set_vsync(true);
    wins.forget_frames();
    let mut lat = l.latencies.clone();
    lat.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let seconds = l.started.elapsed().as_secs();
    log::info!("session : {} s, {} secondes mesurées", seconds, lat.len());
    let Some(id) = host else { return };
    // La dernière session : latence si l'ordinateur l'a mesurée, sinon durée et mode seuls.
    let mbps = if l.mbps.is_empty() { 0.0 } else { l.mbps.iter().sum::<f64>() / l.mbps.len() as f64 };
    let (median, p95) = if lat.is_empty() { (0.0, 0.0) } else { (lat[lat.len() / 2], lat[((lat.len() as f64 * 0.95) as usize).min(lat.len() - 1)]) };
    if seconds >= 3
        && let Some(h) = app.store.host_mut(&id)
    {
        h.last = Some(LastSession { median, p95, seconds, mode: l.mode.clone(), hz: l.hz, codec: l.codec.into(), mbps, at: now_secs(), screens: l.screens as u32 });
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

#[cfg(test)]
mod tests {
    use super::*;

    fn fake_display(name: &str, x: i32) -> Display {
        Display { sdl: 0, name: name.into(), x, y: 0, w: 640, h: 400, pixel_w: 640, pixel_h: 400, refresh: 60 }
    }

    /// Plusieurs fenêtres plein écran (multi-écrans) sur le seul écran de la machine d'essai :
    /// ouverture et fermeture selon les écrans voulus, une image et un dessin par fenêtre, les
    /// événements retrouvent leur fenêtre. Il faut un affichage :
    /// `DISPLAY=:0 cargo test -p kybernow-linux -- --ignored multi_window`.
    #[test]
    #[ignore]
    fn multi_window_lifecycle() {
        unsafe {
            assert_eq!(sdl::SDL_Init(sdl::SDL_INIT_VIDEO | sdl::SDL_INIT_EVENTS), 0, "{}", sdl::error());
        }
        let real = screens::detect();
        assert!(!real.grouped.is_empty(), "SDL ne voit aucun écran");
        // Trois « écrans » qui sont en fait le même, pour ouvrir trois fenêtres.
        let devices = screens::Detected { all: Vec::new(), grouped: vec![fake_display("principal", 0), fake_display("droite", 640), fake_display("gauche", -640)] };
        let title = CString::new("KyberNow Link").unwrap();
        let window = unsafe { sdl::SDL_CreateWindow(title.as_ptr(), sdl::SDL_WINDOWPOS_CENTERED, sdl::SDL_WINDOWPOS_CENTERED, 640, 400, sdl::SDL_WINDOW_SHOWN | sdl::SDL_WINDOW_ALLOW_HIGHDPI) };
        assert!(!window.is_null(), "{}", sdl::error());
        let renderer = unsafe { sdl::SDL_CreateRenderer(window, -1, sdl::SDL_RENDERER_ACCELERATED) };
        assert!(!renderer.is_null(), "{}", sdl::error());
        let premul = unsafe { sdl::SDL_ComposeCustomBlendMode(2, 6, 1, 2, 6, 1) };
        let mut wins = Wins { prev: None, list: vec![Win::new(window, renderer, premul, 0, (640, 400))] };

        // Les trois écrans du plan : la fenêtre de l'app et deux fenêtres de plus.
        sync_windows(&mut wins, &[0, 1, 2], 0, &devices, false);
        assert_eq!(wins.list.len(), 3);
        assert_eq!(wins.list.iter().map(|w| w.screen).collect::<Vec<_>>(), vec![0, 1, 2]);
        let ids: Vec<u32> = wins.list.iter().map(|w| w.id).collect();
        assert!(ids.iter().all(|i| *i != 0) && ids[0] != ids[1] && ids[1] != ids[2], "chaque fenêtre a son identifiant");
        assert_eq!((wins.index_of(ids[2]), wins.index_of(0xFFFF_FF00)), (Some(2), None));
        assert_eq!(wins.index_of_screen(1), Some(1));

        // Une image par fenêtre, posée dans le cadre de sa fenêtre.
        let mut app = App::new(Store::default(), Layout::Desktop);
        app.screen = Screen::Session;
        app.session = Some(SessionUi {
            host: "a".into(),
            name: "A".into(),
            profile: crate::model::Profile::Bureau,
            phase: Phase::Streaming,
            menu: false,
            keys: false,
            stats: None,
            show_stats: false,
            pad_mouse: false,
            muted: false,
            light: false,
            relative: false,
            bar: false,
            bar_menu: None,
            notice: None,
            started: Instant::now(),
            ending_since: None,
            opened_at: Some(Instant::now()),
            screens: 3,
            expected_screens: 3,
        });
        for (i, w) in wins.list.iter_mut().enumerate() {
            w.sizes();
            assert!(!w.has_frame());
            let (fw, fh) = (320u32, 200u32);
            let frame = video::Frame {
                width: fw,
                height: fh,
                format: video::Format::Nv12,
                planes: [vec![(60 + 50 * i) as u8; (fw * fh) as usize], vec![128; (fw * fh / 2) as usize], Vec::new()],
                pitch: [fw as usize, fw as usize, 0],
                pts: 1,
                received: 0,
                decoded: 0,
                bt709: true,
                full_range: false,
            };
            assert!(w.video.upload(w.renderer, &frame), "l'image de la fenêtre {i} ne part pas à sa texture");
            assert!(w.has_frame());
            draw_frame(w, false, None, None, &app);
            let r = w.video_rect.expect("l'image est posée dans la fenêtre");
            assert!(r.w > 0 && r.h > 0 && r.w <= w.out_size.0 && r.h <= w.out_size.1);
        }

        // Le fond de la connexion s'envoie à la texture d'interface de chaque fenêtre.
        for w in wins.list.iter_mut() {
            let (pw, ph) = (w.out_size.0 as u32, w.out_size.1 as u32);
            w.cv.resize(pw, ph, 1.0);
            w.cv.clear(ui::theme::INK);
            upload_ui(w);
            assert!(!w.ui.is_null());
            draw_frame(w, true, None, None, &app);
        }

        // Des écrans de l'appareil dont la place a changé : leur fenêtre se ferme, les autres restent.
        wins.close_extras_where(|w| w.screen == 2);
        assert_eq!(wins.list.iter().map(|w| w.screen).collect::<Vec<_>>(), vec![0, 1]);
        let kept = wins.list[1].id;
        sync_windows(&mut wins, &[0, 1, 2], 0, &devices, false);
        assert_eq!((wins.list.len(), wins.list[1].id), (3, kept), "la fenêtre qui reste n'est pas refaite");
        // Moins d'écrans (Station qui n'en crée plus que deux, profil Jeu) : les fenêtres de plus se ferment.
        sync_windows(&mut wins, &[0, 2], 0, &devices, false);
        assert_eq!(wins.list.iter().map(|w| w.screen).collect::<Vec<_>>(), vec![0, 2]);
        sync_windows(&mut wins, &[0], 0, &devices, false);
        assert_eq!(wins.list.len(), 1);
        // L'écran principal devient un autre écran de l'appareil : la fenêtre de l'app le montre.
        sync_windows(&mut wins, &[1, 0], 1, &devices, false);
        assert_eq!(wins.list.iter().map(|w| w.screen).collect::<Vec<_>>(), vec![1, 0]);
        wins.forget_frames();
        assert!(wins.list.iter().all(|w| !w.has_frame()));
        wins.close_extras();
        assert_eq!(wins.list.len(), 1);
        let main = wins.list.pop().unwrap();
        main.destroy();
        unsafe { sdl::SDL_Quit() };
    }
}
