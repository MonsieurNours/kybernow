//! L'app sur Linux : la fenêtre SDL (bureau : sans bordure, barre de titre
//! dessinée par l'app ; console : plein écran sous gamescope), la boucle des
//! événements (clavier, souris, pavés tactiles, manettes), l'interface, les
//! tâches de fond et les sessions. Pendant la session, l'image de l'ordinateur
//! est une texture YUV ; l'interface (barre, menu, capsules) est posée
//! par-dessus, avec le pointeur de l'ordinateur.
//!
//! Plusieurs ordinateurs à la fois (docs/plusieurs-ordinateurs.md, interface de bureau seulement) :
//! une liste de sessions (`App::sessions` pour l'interface, `Lives` pour ce que tient chaque
//! session), une fenêtre par ordinateur (`Win::session`), les événements étiquetés par leur
//! session (`Event::Session`). La session active est celle de la fenêtre au premier plan : elle
//! seule reçoit l'entrée. Qui est active, c'est le registre du cœur (`kybernow_core::focus`, voir
//! `focus.rs`) qui le dit et qui l'applique (touches relâchées, manettes au repos, presse-papiers en
//! pause, son coupé) ; l'app lui annonce ses fenêtres et recopie sa réponse.

mod chime;
mod focus;
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
use kybernow_core::focus::FocusId;
use kybernow_core::gamepads;
use kybernow_core::input::{GamepadRumble, MouseMode, MouseModeRules};
use kybernow_core::kynput_rs::MouseButtonType;
use kybernow_core::latency;
use kybernow_core::profile::{Capabilities, StreamSettings};
use kybernow_core::session::Status;
use kybernow_core::share::{self, JoinRequest, ShareTarget};

use crate::app::{Action, AddStep, App, Found, HostLive, Layout, Nav, Online, OsAction, Phase, Rect, Screen, SessionId, SessionUi, Stats, Target};
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
        /// Ce que « Partager » peut faire (KyberNow Share) ; `None` : pas de réponse d'Admin.
        share: Option<crate::app::ShareAvail>,
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
    /// Ce qu'une session envoie, étiqueté par sa session : plusieurs ordinateurs à la fois, un seul
    /// canal. Un événement d'une session finie est écarté.
    Session(SessionId, SessionEvent),
    Update(Result<Option<kybernow_core::update::Available>, String>),
    /// KyberNow Share, invité : la réponse de « Rejoindre » (numéro de la recherche), ou son refus.
    ShareJoined { attempt: u32, result: Result<share::Found, share::ShareError> },
    /// KyberNow Share, hôte : le partage de cette session après une demande à Admin (`None` : fermé), ou ce
    /// qu'Admin a refusé.
    ShareHost { session: SessionId, result: Result<Option<crate::app::ShareView>, String> },
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

/// Ce que le cœur dit d'une session (rappels de `session::Platform`).
pub enum SessionEvent {
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
    GameFullscreen(bool),
    DecoderFailed(String),
    /// KyberNow Share, hôte : le partage et ses invités (`None` : pas de partage).
    Guests(Option<crate::app::ShareView>),
    /// KyberNow Share, invité : son rôle (`previous` nul au départ), puis chaque changement de l'hôte.
    ShareRole { role: crate::app::ShareRole, previous: Option<crate::app::ShareRole> },
    /// KyberNow Share, invité : l'image qu'il reçoit (`label` : sa qualité, nulle tant que Kyber n'en sait pas la taille)
    /// et l'avis bref de la Station, s'il y en a un.
    ShareImage { label: Option<String>, notice: Option<String> },
    /// KyberNow Share, invité : le partage a fini la session (`kind` : stopped, removed, unreadable) avec ce texte.
    ShareEnd(String, String),
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
/// texture d'image et son dessin. La fenêtre de l'app est la première (l'accueil, puis la fenêtre
/// de la première session) ; une session à plusieurs écrans en ouvre une de plus par écran de
/// l'appareil (docs/multi-ecrans.md), chacune plein écran ; chaque session de plus a la sienne,
/// en fenêtre (docs/plusieurs-ordinateurs.md).
struct Win {
    window: *mut sdl::SDL_Window,
    renderer: *mut sdl::SDL_Renderer,
    /// Identifiant SDL de la fenêtre : celui des événements qui la visent.
    id: u32,
    /// L'écran de l'appareil dont cette fenêtre montre le flux (indice dans la
    /// liste regroupée de `screens::Detected`).
    screen: usize,
    /// La session que montre la fenêtre (`None` : l'app, l'accueil), et son rôle : la fenêtre
    /// principale de la session, ou une fenêtre de plus (un écran de plus, multi-écrans).
    session: Option<SessionId>,
    extra: bool,
    /// Plein écran (les fenêtres des sessions de plus ; la fenêtre de l'app suit `App::fullscreen`).
    fullscreen: bool,
    /// Cachée : la fenêtre de l'app quand les sessions qui restent ont chacune la leur.
    hidden: bool,
    /// Les éléments choisissables du dernier dessin de cette fenêtre : un clic dans une fenêtre
    /// s'adresse à ses éléments, pas à ceux de la fenêtre de la session active.
    hits: Vec<(Target, Rect)>,
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
            session: None,
            extra: false,
            fullscreen: false,
            hidden: false,
            hits: Vec::new(),
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

/// Les fenêtres de l'app : la sienne, puis celles des écrans de plus d'une session à plusieurs
/// écrans, puis une par session de plus.
struct Wins {
    /// La fenêtre de l'app avant qu'une session à plusieurs écrans la passe en plein écran ;
    /// remise comme elle était quand la session est finie et que son message est lu.
    prev: Option<PrevWindow>,
    list: Vec<Win>,
    /// La fenêtre que le pointeur survole (identifiant SDL).
    hover: Option<u32>,
}

impl Wins {
    /// La fenêtre de l'app : la première, jamais fermée.
    fn app(&mut self) -> &mut Win {
        &mut self.list[0]
    }

    fn index_of(&self, id: u32) -> Option<usize> {
        self.list.iter().position(|w| w.id == id)
    }

    /// La fenêtre principale d'une session (celle de son écran principal).
    fn main_of(&self, session: SessionId) -> Option<usize> {
        self.list.iter().position(|w| w.session == Some(session) && !w.extra)
    }

    /// La fenêtre de cette session qui montre le flux de cet écran de l'appareil.
    fn index_of_screen(&self, session: SessionId, screen: usize) -> Option<usize> {
        self.list.iter().position(|w| w.session == Some(session) && w.screen == screen)
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

    /// Ferme une fenêtre (jamais celle de l'app).
    fn destroy_at(&mut self, i: usize) {
        let w = self.list.remove(i);
        if self.hover == Some(w.id) {
            self.hover = None;
        }
        w.destroy();
    }

    /// La fin d'une session ferme ses fenêtres de plus (celles des autres écrans).
    fn close_extras(&mut self, session: SessionId) {
        self.close_extras_where(session, |_| true);
    }

    /// Ferme celles des fenêtres de plus de cette session que `stale` désigne (leur écran n'est plus au même endroit).
    fn close_extras_where(&mut self, session: SessionId, stale: impl Fn(&Win) -> bool) {
        let mut i = 0;
        while i < self.list.len() {
            if self.list[i].session == Some(session) && self.list[i].extra && stale(&self.list[i]) {
                self.destroy_at(i);
            } else {
                i += 1;
            }
        }
    }

    /// Les images reçues par une session sont oubliées (la connexion reprend à sa première étape).
    fn forget_frames(&mut self, session: SessionId) {
        for w in self.list.iter_mut().filter(|w| w.session == Some(session)) {
            w.video.reset();
            w.hint_key = None;
            w.present = true;
        }
    }
}

/// Le titre des fenêtres d'une session : le nom de son ordinateur (docs/link-ui.md).
fn session_title(name: &str) -> String {
    format!("{name} · KyberNow Link")
}

const APP_TITLE: &str = "KyberNow Link";

/// Une fenêtre plein écran de plus, sur l'écran de l'appareil `display` (multi-écrans).
/// Même titre que celle de la session. Elle s'affiche tout de suite (le fond de la connexion).
fn create_extra(display: &Display, screen: usize, premul: c_int, grab_keyboard: bool, title: &str) -> Option<Win> {
    let title = CString::new(title.replace('\0', "")).unwrap_or_default();
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

/// La fenêtre d'une session de plus (plusieurs ordinateurs) : une vraie fenêtre, décorée par le
/// gestionnaire de fenêtres (on la déplace, on la redimensionne, on la ferme), titrée du nom de son
/// ordinateur, posée à `frame` (centrée aux deux tiers de l'écran de la fenêtre active, en 16:9).
fn create_session_window(title: &str, frame: (i32, i32, i32, i32), screen: usize, premul: c_int) -> Option<Win> {
    let c_title = CString::new(title.replace('\0', "")).unwrap_or_default();
    let (x, y, w, h) = frame;
    let flags = sdl::SDL_WINDOW_SHOWN | sdl::SDL_WINDOW_RESIZABLE | sdl::SDL_WINDOW_ALLOW_HIGHDPI;
    let window = unsafe { sdl::SDL_CreateWindow(c_title.as_ptr(), x, y, w.max(1), h.max(1), flags) };
    if window.is_null() {
        log::warn!("session : fenêtre impossible ({})", sdl::error());
        return None;
    }
    // Comme les fenêtres de plus : sans attendre la synchro verticale (la fenêtre de l'app y est déjà).
    let renderer = unsafe { sdl::SDL_CreateRenderer(window, -1, sdl::SDL_RENDERER_ACCELERATED) };
    if renderer.is_null() {
        log::warn!("session : rendu impossible ({})", sdl::error());
        unsafe { sdl::SDL_DestroyWindow(window) };
        return None;
    }
    unsafe { sdl::SDL_SetWindowMinimumSize(window, 480, 270) };
    let mut win = Win::new(window, renderer, premul, screen, (w.max(1), h.max(1)));
    win.vsync = false;
    log::info!("session : fenêtre {} ouverte, {} × {} à ({}, {})", win.id, win.out_size.0, win.out_size.1, x, y);
    Some(win)
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

/// Ferme et ouvre les fenêtres de plus de cette session pour qu'il y en ait une par écran voulu
/// (`wanted`, l'écran principal compris), et que sa fenêtre principale montre l'écran
/// `primary`. Le reste ne bouge pas.
fn sync_windows(wins: &mut Wins, session: SessionId, title: &str, wanted: &[usize], primary: usize, devices: &screens::Detected, grab_keyboard: bool) {
    if let Some(m) = wins.main_of(session) {
        wins.list[m].screen = primary;
    }
    let mut i = 0;
    while i < wins.list.len() {
        let w = &wins.list[i];
        if w.session == Some(session) && w.extra && (w.screen == primary || !wanted.contains(&w.screen)) {
            wins.destroy_at(i);
        } else {
            i += 1;
        }
    }
    let premul = wins.list[0].premul;
    for &s in wanted {
        if s == primary || wins.index_of_screen(session, s).is_some() {
            continue;
        }
        if let Some(d) = devices.grouped.get(s)
            && let Some(mut w) = create_extra(d, s, premul, grab_keyboard, title)
        {
            w.session = Some(session);
            w.extra = true;
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
    /// La session de l'app (`App::sessions`) que cet état accompagne.
    id: SessionId,
    handle: session::Handle,
    /// Son inscription au registre des sessions du cœur (`focus`).
    focus: FocusId,
    /// L'ordinateur de la session (la dernière session y est enregistrée).
    host: String,
    /// Une session d'invité (KyberNow Share) : rien n'en est gardé (ni dernière session, ni image), l'invité n'est
    /// jamais dans la liste des ordinateurs.
    guest: bool,
    /// Une session de plus : la surface de sa fenêtre (pixels) et la fréquence de son écran, dont
    /// elle prend le mode (l'écran de l'ordinateur a la taille de la fenêtre) ; `None` : le mode de
    /// l'écran de l'appareil.
    surface: Option<((u32, u32), u32)>,
    /// Une session de plus : l'écran de l'appareil que le cœur connaît comme « Cet écran » ; il suit la
    /// fenêtre quand on la déplace sur un autre écran (`follow_window`).
    followed: usize,
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
    /// Multi-écrans : la fenêtre où ce geste a lieu et celle où la barre est ouverte
    /// (identifiants SDL de fenêtre : les indices changent quand une fenêtre se ferme).
    bar_arm_win: Option<u32>,
    bar_win: Option<u32>,
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
    /// l'identique, pour ce que la session attend avant son annonce (`SessionEvent::Screens`).
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

/// Ce que tient chaque session en cours (`App::sessions` en est l'interface), dans l'ordre d'ouverture.
#[derive(Default)]
struct Lives(Vec<Live>);

impl Lives {
    fn get(&self, id: SessionId) -> Option<&Live> {
        self.0.iter().find(|l| l.id == id)
    }

    fn get_mut(&mut self, id: SessionId) -> Option<&mut Live> {
        self.0.iter_mut().find(|l| l.id == id)
    }

    /// La session que le registre du cœur désigne par cette inscription.
    fn by_focus(&self, focus: FocusId) -> Option<&Live> {
        self.0.iter().find(|l| l.focus == focus)
    }

    fn iter(&self) -> impl Iterator<Item = &Live> {
        self.0.iter()
    }

    fn iter_mut(&mut self) -> impl Iterator<Item = &mut Live> {
        self.0.iter_mut()
    }

    fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    fn remove(&mut self, id: SessionId) -> Option<Live> {
        let i = self.0.iter().position(|l| l.id == id)?;
        Some(self.0.remove(i))
    }
}

impl Live {
    /// La fenêtre (indice) où la barre est ouverte ; à défaut, la principale de la session.
    fn bar_index(&self, wins: &Wins) -> usize {
        self.bar_win.and_then(|id| wins.index_of(id)).or_else(|| wins.main_of(self.id)).unwrap_or(0)
    }
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
    // Plusieurs fenêtres plein écran (multi-écrans, plusieurs ordinateurs) : le clavier passe de l'une à l'autre sans que SDL les réduise.
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
    let title = CString::new(APP_TITLE).unwrap();
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
    let mut wins = Wins { prev: None, list: vec![Win::new(window, renderer, premul, first_screen, (ww, wh))], hover: None };
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
    // Les sessions en cours (leur interface est dans `app.sessions`) et ce que `focus::sync` a déjà appliqué à SDL.
    let mut lives = Lives::default();
    let mut focus_applied = focus::Applied::default();
    // La session dont la fenêtre est déjà passée devant.
    let mut raised: Option<SessionId> = None;
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
        let streaming = !lives.is_empty() && app.sessions.iter().any(|s| matches!(s.phase, Phase::Streaming | Phase::Reconnecting { .. }));
        // Animations : lancement, passage, capsules, ajout, écran des manettes.
        let animating = app.screen == Screen::Launch
            || app.sessions.iter().any(|s| {
                matches!(s.phase, Phase::Preparing | Phase::Connecting | Phase::StationRestarting | Phase::FirstImages | Phase::Ending) || s.notice.as_ref().is_some_and(|(_, t)| t.elapsed().as_secs_f32() < 4.2)
            })
            || app.waking_host().is_some() && app.screen == Screen::Home
            || app.station_op.as_ref().is_some_and(|o| o.busy()) && app.screen == Screen::Settings
            || app.toast.as_ref().is_some_and(|(_, t)| t.elapsed().as_secs_f32() < 4.2)
            || app.screen == Screen::Gamepads
            || app.screen == Screen::Add
            || app.screen == Screen::Join
            || app.sessions.iter().any(|s| s.share_busy || s.share_copied.is_some_and(|(t, _)| t.elapsed().as_secs_f32() < 4.2))
            || pads.mouse_active();
        let timeout = if animating { 16 } else if streaming { 100 } else { 50 };
        let mut ev = sdl::SDL_Event::default();
        let mut actions: Vec<Action> = Vec::new();
        if unsafe { sdl::SDL_WaitEventTimeout(&mut ev, timeout) } != 0 {
            loop {
                handle_sdl(&ev, &mut app, &mut pads, &mut lives, &mut wins, &mut actions, &mut dirty, &mut quit, &mut start_down);
                if unsafe { sdl::SDL_PollEvent(&mut ev) } == 0 {
                    break;
                }
            }
        }
        // Manette en souris : déplacement continu (la session active).
        if let Some(s) = app.session()
            && s.pad_mouse
            && !s.menu
            && let Some(l) = lives.get(s.id)
            && pads.mouse_step(&l.handle.shared)
        {
            present = true;
        }
        if let Some(n) = pads.repeat_nav() {
            actions.extend(app.nav(n));
            dirty = true;
        }
        while let Ok(e) = rx.try_recv() {
            if let Event::Session(id, SessionEvent::Pointer { screen, .. }) = &e {
                // Le pointeur n'est dessiné que dans la fenêtre de son écran.
                match wins.index_of_screen(*id, *screen) {
                    Some(i) => wins.list[i].present = true,
                    None => present = true,
                }
            } else {
                dirty = true;
            }
            handle_event(e, &mut app, &mut pads, &mut lives, &mut wins, &mut actions, &mut pairing, &tx);
        }
        for a in actions.drain(..) {
            perform(a, &mut app, &mut pads, &mut lives, &mut wins, &tx, &mut pairing, &mut quit);
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
            perform(a, &mut app, &mut pads, &mut lives, &mut wins, &tx, &mut pairing, &mut quit);
            dirty = true;
        }

        // Fin des sessions : la dernière session de chaque ordinateur enregistrée, leurs fenêtres
        // fermées ; la dernière ramène l'accueil.
        let had_sessions = !lives.is_empty();
        if reap(&mut app, &mut lives, &mut wins) {
            dirty = true;
        }
        if had_sessions && lives.is_empty() {
            last_poll = Instant::now() - Duration::from_secs(60);
            last_scan = Instant::now() - Duration::from_secs(60);
        }
        // La session à plusieurs écrans est finie et son message lu : la fenêtre de l'app redevient ce qu'elle était.
        if lives.is_empty() && app.sessions.is_empty() && wins.prev.is_some() {
            restore_main(&mut app, &mut wins);
            dirty = true;
        }

        // Qui est actif : le registre du cœur le dit et l'applique ; l'interface le recopie, SDL suit.
        if focus::sync(&mut app, &lives, &mut pads, &mut focus_applied) {
            dirty = true;
        }
        raise_active(&app, &wins, &mut raised);

        // Les écrans de l'appareil ont peut-être changé (événement de SDL) : relus tout de suite.
        let rescan_now = RESCAN.swap(false, Ordering::Relaxed);
        for l in lives.iter_mut() {
            if rescan_now {
                l.rescan_at = Instant::now() - Duration::from_secs(10);
            }
            session_tick(&mut app, l, &mut wins, &mut dirty, &mut present);
        }
        // Presse-papiers : ce que l'ordinateur envoie (le pont du cœur n'envoie rien tant que la session
        // n'est pas active), et le texte du système, relu deux fois par seconde.
        for l in lives.iter() {
            if let Some(c) = &l.handle.shared.clipboard {
                c.write_outgoing();
            }
        }
        if last_clip.elapsed() > Duration::from_millis(500) {
            last_clip = Instant::now();
            focus::sync_clipboards(&lives);
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
        // Sondage d'Admin toutes les 8 s : hors session, tous les ordinateurs ; en session (plusieurs
        // ordinateurs), ceux qui ne sont pas en session, pour l'état du menu « Ordinateurs » de la barre.
        // Le réseau et la batterie toutes les 5 s.
        if last_poll.elapsed() > Duration::from_secs(8) {
            if lives.is_empty() {
                if matches!(app.screen, Screen::Home | Screen::Settings | Screen::Launch) {
                    last_poll = Instant::now();
                    for h in &app.store.hosts {
                        jobs::poll(h.clone(), !app.images.contains_key(&h.id), tx.clone());
                    }
                }
            } else if app.multi_sessions() {
                last_poll = Instant::now();
                for h in app.store.hosts.iter().filter(|h| app.session_of_host(&h.id).is_none()) {
                    jobs::poll(h.clone(), false, tx.clone());
                }
            }
        }
        if lives.is_empty() && last_update.elapsed() > Duration::from_secs(6 * 3600) {
            last_update = Instant::now();
            jobs::check_update(app.store.app.beta, tx.clone());
        }
        if last_net.elapsed() > Duration::from_secs(5) && lives.is_empty() {
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
        if lives.is_empty() && (rescan_now || last_scan.elapsed() > Duration::from_secs(2)) {
            last_scan = Instant::now();
            if refresh_devices(&mut app, &mut devices, &mut wins) {
                dirty = true;
            }
        }
        // Multi-écrans : ce que la session prendra pour l'ordinateur choisi (accueil, réglages).
        if lives.is_empty() && app.multi_screens() {
            let plan = selected_plan(&app, &devices, wins.list[0].window);
            if plan != app.screens_plan {
                app.screens_plan = plan;
                dirty = true;
            }
        }

        // Les images décodées qui attendent partent à la texture de leur fenêtre.
        let mut new_frames: Vec<(SessionId, (u64, i64, i64))> = Vec::new();
        for l in lives.iter_mut() {
            let primary = l.handle.shared.primary();
            for w in wins.list.iter_mut().filter(|w| w.session == Some(l.id) && !w.hidden) {
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
                        new_frames.push((l.id, (f.pts, f.received, f.decoded)));
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
        let active = app.active_id();
        let views: Vec<(ui::View, bool, bool)> = wins
            .list
            .iter()
            .map(|w| {
                let view = view_of(&app, &lives, w);
                // Une fenêtre dont la session n'est pas l'active garde sa session quel que soit l'écran de l'app.
                let background = view.session.is_some_and(|id| Some(id) != active);
                (view, (!background && app.screen != Screen::Session) || ui::overlay_visible(&app, view), ui::hint_only(&app, view))
            })
            .collect();
        let hits_win = hits_window(&app, &lives, &wins);
        let hover = wins.hover;
        for (wi, (view, show_ui, hint_only)) in views.iter().copied().enumerate() {
            if wins.list[wi].hidden {
                continue;
            }
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
                    if hover != Some(win.id) {
                        app.pointer = None;
                    }
                    let hits = ui::draw_view(&app, &mut win.cv, view);
                    app.pointer = pointer;
                    if wi == hits_win {
                        app.hits = hits.clone();
                    }
                    win.hits = hits;
                    upload_ui(win);
                    win.hint_key = hint_only.then_some(key);
                }
            } else {
                win.hits.clear();
                if wi == hits_win {
                    app.hits.clear();
                }
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
            if wins.list[wi].hidden || !(present || wins.list[wi].present) {
                continue;
            }
            let (_, show_ui, hint_only) = views[wi];
            let win = &mut wins.list[wi];
            let strip = hint_only.then(|| (ui::HINT_STRIP * win.cv.k).ceil() as c_int);
            let session = win.session;
            draw_frame(win, show_ui, strip, session.and_then(|id| lives.get(id)), session.and_then(|id| app.session_by(id)), &app);
            win.present = false;
            // La latence : l'image de l'écran principal de la session, une fois montrée dans sa fenêtre principale.
            if !win.extra
                && let Some(id) = session
                && let Some(pos) = new_frames.iter().position(|(i, _)| *i == id)
            {
                let (_, (pts, received, decoded)) = new_frames.remove(pos);
                if let Some(l) = lives.get_mut(id) {
                    let at = latency::now_micros();
                    l.handle.session.frame_displayed(pts, received, decoded, at);
                    l.last_frame = Some(Instant::now());
                    l.shown = Some((pts, received, decoded));
                }
            }
        }
        present = false;
    }

    // Quitter l'app termine toutes les sessions, et attend que chaque ordinateur soit prévenu.
    stop_all(&lives, &wins);
    for c in pairing.drain(..) {
        c.store(true, Ordering::Relaxed);
    }
    app.store.save();
    unsafe { sdl::SDL_Quit() };
    log::info!("KyberNow Link s'arrête");
}

/// Combien de temps l'app attend, au plus, que les ordinateurs soient prévenus quand elle se ferme.
const QUIT_WAIT: Duration = Duration::from_secs(8);

/// Quitter l'app : chaque session est arrêtée puis attendue (`Session::stop`, `Session::wait` : la Station est
/// prévenue, ses écrans, son son et ses manettes reviennent), les sessions en parallèle et au plus `QUIT_WAIT`
/// en tout, comme l'app Windows. Les fenêtres se cachent pendant l'attente : l'app a l'air fermée.
fn stop_all(lives: &Lives, wins: &Wins) {
    if lives.is_empty() {
        return;
    }
    for w in &wins.list {
        unsafe { sdl::SDL_HideWindow(w.window) };
    }
    let started = Instant::now();
    let (done_tx, done_rx) = channel::<()>();
    for l in lives.iter() {
        l.handle.session.stop();
        let (session, done) = (l.handle.session.clone(), done_tx.clone());
        std::thread::spawn(move || {
            session.wait();
            let _ = done.send(());
        });
    }
    for _ in 0..lives.0.len() {
        if done_rx.recv_timeout(QUIT_WAIT.saturating_sub(started.elapsed())).is_err() {
            log::warn!("fin de l'app : un ordinateur n'a pas répondu en {} s, l'app se ferme quand même", QUIT_WAIT.as_secs());
            return;
        }
    }
    log::info!("fin de l'app : {} session(s) terminée(s) en {} ms", lives.0.len(), started.elapsed().as_millis());
}

/// La fenêtre de la session active passe devant quand l'activation ne vient pas d'elle : « Afficher »,
/// « Ordinateur suivant », la session qu'on vient d'ouvrir, la session qui reprend la main quand l'active
/// se termine. Une fenêtre qui a déjà le clavier (le clic ou le focus l'a rendue active) ne bouge pas ;
/// et rien ne vole le focus quand l'app n'a plus le clavier (on travaille dans une autre app).
fn raise_active(app: &App, wins: &Wins, raised: &mut Option<SessionId>) {
    let active = app.active_id();
    if active == *raised {
        return;
    }
    *raised = active;
    let Some(i) = active.and_then(|id| wins.main_of(id)) else { return };
    let w = &wins.list[i];
    if !w.hidden && !w.focused() && wins.any_focused() {
        sdl::raise(w.window);
    }
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
/// en pixels), le pointeur de l'ordinateur. `live` et `s` : la session que
/// montre cette fenêtre (aucune : l'accueil).
fn draw_frame(win: &mut Win, show_ui: bool, strip: Option<c_int>, live: Option<&Live>, s: Option<&SessionUi>, app: &App) {
    unsafe {
        sdl::SDL_SetRenderDrawColor(win.renderer, 0, 0, 0, 255);
        sdl::SDL_RenderClear(win.renderer);
        // Une session qui n'est pas active garde son image quel que soit l'écran que l'app montre à l'active.
        let background = s.is_some_and(|s| Some(s.id) != app.active);
        let in_session = s.is_some_and(|s| (app.screen == Screen::Session || background) && matches!(s.phase, Phase::Streaming | Phase::Reconnecting { .. } | Phase::Ending));
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
        let draw_cursor = s.is_some_and(|s| !s.menu && !s.keys && !s.share_panel && s.phase == Phase::Streaming && (s.relative || s.pad_mouse || app.layout.console()));
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

/// Ce qui se règle à chaque tour pendant une session : pour la session active, souris des jeux ou du
/// bureau, barre du bureau, curseur du système ; pour toutes, les écrans de l'appareil.
fn session_tick(app: &mut App, l: &mut Live, wins: &mut Wins, dirty: &mut bool, present: &mut bool) {
    let layout = app.layout;
    let Some(si) = app.sessions.iter().position(|s| s.id == l.id) else { return };
    let is_active = app.active == Some(l.id);
    if is_active {
        let streaming = matches!(app.sessions[si].phase, Phase::Streaming);
        // Souris : Auto suit le jeu en plein écran et le pointeur caché (sur tous les écrans).
        let (remote_hidden, visible_for) = l.handle.shared.pointer_summary();
        let s = &mut app.sessions[si];
        let current = s.relative;
        let wanted = streaming && !s.menu && !s.keys && !s.share_panel && !s.bar_menu.is_some() && s.keyboard_mouse() && MOUSE_RULES.wants_relative(l.mouse, l.game_fullscreen, remote_hidden, visible_for, current);
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
        let show_sys = !streaming || s.menu || s.keys || s.share_panel || s.bar || (layout == Layout::Desktop && !s.relative);
        // Le pointeur du système prend la forme que l'ordinateur a annoncée, dans la fenêtre survolée si
        // elle est de cette session (celle d'une autre session ne le change pas).
        let hovered = wins.hover.and_then(|id| wins.index_of(id)).filter(|i| wins.list[*i].session == Some(l.id));
        unsafe {
            sdl::SDL_ShowCursor(show_sys as c_int);
            match hovered {
                Some(i) if !wins.list[i].sys_cursor.is_null() && streaming && !s.menu && !s.keys && !s.bar => sdl::SDL_SetCursor(wins.list[i].sys_cursor),
                _ => sdl::SDL_SetCursor(sdl::SDL_GetDefaultCursor()),
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
            if let Some(wi) = l.bar_arm_win.and_then(|id| wins.index_of(id)) {
                let wid = wins.list[wi].id;
                let wpts = wins.list[wi].width_points(layout);
                let s = &app.sessions[si];
                let here = wins.hover == Some(wid) && app.pointer.is_some_and(|(ux, uy)| ui::at_bar_edge(&mut wins.list[wi].cv, app, s, wpts, ux, uy));
                if layout == Layout::Desktop && matches!(s.phase, Phase::Streaming | Phase::Reconnecting { .. }) && !s.menu && !s.keys && !s.bar && !s.relative && here && wins.any_focused() && !sdl::mouse_held() {
                    app.sessions[si].bar = true;
                    l.bar_win = Some(wid);
                    l.bar_hide = None;
                    // La fenêtre où la barre s'ouvre devient la fenêtre active (le clavier y va).
                    if !wins.list[wi].focused() {
                        unsafe { sdl::SDL_RaiseWindow(wins.list[wi].window) };
                    }
                    *dirty = true;
                }
            }
        }
        // Fermeture : 400 ms après que le pointeur a quitté la barre, jamais pendant
        // un de ses menus. Un menu qui se ferme pointeur dehors, ou un pointeur sorti
        // de la fenêtre sans mouvement, arment le même délai.
        {
            let s = &app.sessions[si];
            if s.bar && s.bar_menu.is_none() && l.bar_hide.is_none() {
                let bw = l.bar_index(wins);
                let wpts = wins.list[bw].width_points(layout);
                let wid = wins.list[bw].id;
                let win = &mut wins.list[bw];
                let over = wins.hover == Some(wid) && app.pointer.is_some_and(|(ux, uy)| ui::over_bar(&mut win.cv, app, s, wpts, &win.hits, ux, uy));
                if !over {
                    l.bar_hide = Some(Instant::now() + Duration::from_millis(400));
                }
            }
        }
        if let Some(at) = l.bar_hide
            && Instant::now() >= at
        {
            l.bar_hide = None;
            let s = &mut app.sessions[si];
            if s.bar_menu.is_none() {
                s.bar = false;
                *dirty = true;
            }
        }
        // Échap tenu une seconde : la souris des jeux est rendue.
        let s = &mut app.sessions[si];
        if let Some(t) = l.escape_down
            && t.elapsed() > Duration::from_millis(1000)
            && s.relative
        {
            l.escape_down = None;
            l.mouse = MouseMode::Desktop;
            s.notice = Some((tr("Souris du bureau", "Desktop mouse").into(), Instant::now()));
            *dirty = true;
        }
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
    follow_window(app, l, wins);
}

/// Une session de plus a un seul écran, « Cet écran » : celui de sa fenêtre. Quand on la déplace sur un autre
/// écran de l'appareil, le cœur le sait (`Session::set_screens`) : le flux suit sa fenêtre sans relancer
/// l'image (`SessionEvent::Screens` ramène la fenêtre à son nouvel écran). La taille de l'écran de l'ordinateur,
/// elle, reste celle de la fenêtre à l'ouverture.
fn follow_window(app: &App, l: &mut Live, wins: &Wins) {
    if l.surface.is_none() || l.screens_base.is_none() {
        return;
    }
    let Some(wi) = wins.main_of(l.id) else { return };
    let current = l.devices.current(wins.list[wi].window);
    if current == l.followed {
        return;
    }
    l.followed = current;
    let (Some(h), Some(base)) = (app.store.host(&l.host), l.screens_base.clone()) else { return };
    log::info!("session {} : sa fenêtre est maintenant sur l'écran {current}", l.host);
    let screens = session_screens(app, h, &l.devices, current, &base);
    l.screens_base = Some(screens.clone());
    l.handle.session.set_screens(screens);
}

/// La fenêtre principale d'une session (la première, à défaut).
fn main_window_of(wins: &Wins, session: SessionId) -> *mut sdl::SDL_Window {
    wins.list[wins.main_of(session).unwrap_or(0)].window
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
    let current = l.devices.current(main_window_of(wins, l.id));
    // Ce qui décide du nombre d'écrans (le réglage « Écrans », une définition sur mesure, ce que la
    // Station crée) est celui du départ de la session : il ne change qu'à la session suivante.
    let screens = session_screens(app, &h, &l.devices, current, &base);
    l.screens_base = Some(screens.clone());
    l.followed = current;
    let after = plan_of(l);
    l.handle.session.set_screens(screens);
    if before == after {
        log::info!("écrans de l'appareil changés : le plan de la session ne change pas");
        return;
    }
    // Plan différent : le cœur refait la session (« Préparation » efface les images, puis ses écrans
    // arrivent). Les fenêtres de plus dont l'écran n'est plus au même endroit se ferment tout de suite
    // (leur indice a pu changer de sens) ; celles qui manquent s'ouvrent à l'annonce du cœur.
    wins.close_extras_where(l.id, |w| !displays::same_screen(&old.grouped, &l.devices.grouped, w.screen));
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
/// remet comme elle était. Seule la première session (celle de la fenêtre de l'app) prend plusieurs écrans.
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
    let mut title = String::new();
    if let Some(s) = app.session_by_mut(l.id) {
        s.screens = l.screens;
        s.expected_screens = wanted.len().max(1);
        title = session_title(&s.name);
    }
    if l.multi {
        // La fenêtre de l'app est en plein écran sur l'écran principal du flux tant qu'il y a plusieurs
        // écrans, et s'ils redeviennent un seul (profil changé) : elle montre alors l'écran du flux, celui
        // que le cœur a pris comme « Cet écran » (sa définition, son mode).
        if (wanted.len() >= 2 || wins.prev.is_some()) && wins.list[0].session == Some(l.id) {
            take_over_main_window(app, l, wins, primary);
        }
        sync_windows(wins, l.id, &title, &wanted, primary, &l.devices, true);
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

/// Ce que montre une fenêtre (voir `ui::View`) : la session qui est la sienne ; celle d'une session à
/// plusieurs écrans a son rôle ; une seule fenêtre montre tout.
fn view_of(app: &App, lives: &Lives, win: &Win) -> ui::View {
    let Some(id) = win.session else { return ui::View::SINGLE };
    let Some(s) = app.session_by(id) else { return ui::View::SINGLE };
    match lives.get(id).filter(|_| s.multi()) {
        None => ui::View { session: Some(id), ..ui::View::SINGLE },
        Some(l) => ui::View { session: Some(id), main: !win.extra, bar: l.bar_win == Some(win.id), missing: l.not_created.contains(&win.screen), has_frame: win.has_frame() },
    }
}

/// La fenêtre dont les éléments (boutons, menus) se choisissent au clavier et à la manette : celle de
/// l'app hors session ; en session, la fenêtre principale de la session active, ou, avec plusieurs
/// écrans, celle où sa barre est ouverte.
fn hits_window(app: &App, lives: &Lives, wins: &Wins) -> usize {
    let Some(s) = app.session() else { return 0 };
    match lives.get(s.id) {
        Some(l) if s.multi() && s.bar && matches!(s.phase, Phase::Streaming | Phase::Reconnecting { .. }) => l.bar_index(wins),
        _ => wins.main_of(s.id).unwrap_or(0),
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

/// Le bouton de la souris de SDL (1 gauche, 2 milieu, 3 droit, 4 et 5 latéraux) pour kynput.
fn mouse_button(b: u8) -> MouseButtonType {
    match b {
        3 => MouseButtonType::Right,
        2 => MouseButtonType::Middle,
        4 => MouseButtonType::Side,
        5 => MouseButtonType::Extra,
        _ => MouseButtonType::Left,
    }
}

/// Le clic qui vient de rendre la session `id` active (sa fenêtre n'avait pas le focus) part aussi à son
/// ordinateur, au pointeur : il suit le chemin d'un clic ordinaire (position, puis bouton), quand la session est à
/// l'image et que sa souris est celle du bureau ; en souris des jeux, seul le bouton part. Le relâchement, lui,
/// arrive à une session déjà active et suit le chemin ordinaire.
#[allow(clippy::too_many_arguments)]
fn forward_click(app: &App, lives: &Lives, wins: &Wins, wi: usize, id: SessionId, b: u8, x: i32, y: i32) {
    let Some(s) = app.session_by(id) else { return };
    let streaming = s.running() && app.screen == Screen::Session && !s.menu && !s.keys && matches!(s.phase, Phase::Streaming | Phase::Reconnecting { .. });
    let Some(l) = lives.get(id).filter(|_| streaming) else { return };
    let Some(p) = l.handle.shared.current_input() else { return };
    if !s.relative
        && let Some((hx, hy)) = host_point(&wins.list[wi], l, x, y)
    {
        p.mouse_on(wins.list[wi].screen, hx, hy);
    }
    p.mouse_button(mouse_button(b), true);
}

/// Un clic ou un survol dans une fenêtre s'adresse aux éléments de cette fenêtre (son dernier dessin),
/// pas à ceux de la fenêtre de la session active : `App` les lit dans `hits`.
fn in_window<R>(app: &mut App, win: &mut Win, f: impl FnOnce(&mut App) -> R) -> R {
    std::mem::swap(&mut app.hits, &mut win.hits);
    let r = f(app);
    std::mem::swap(&mut app.hits, &mut win.hits);
    r
}

#[allow(clippy::too_many_arguments)]
fn handle_sdl(ev: &sdl::SDL_Event, app: &mut App, pads: &mut pads::Pads, lives: &mut Lives, wins: &mut Wins, actions: &mut Vec<Action>, dirty: &mut bool, quit: &mut bool, start_down: &mut Option<Instant>) {
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
    // Plusieurs ordinateurs : la session de la fenêtre, et la session qui reçoit l'entrée. L'entrée d'une
    // fenêtre (clavier, souris, molette) ne va qu'à la session active, si c'est la sienne ; celle des
    // manettes, à la session active. Une fenêtre dont la session n'est pas active ne donne rien à son
    // ordinateur : un clic y rend sa session active, c'est tout.
    let ws: Option<SessionId> = if on_window { wins.list[wi].session } else { None };
    let active = app.active_id();
    let stale = ws.is_some() && ws != active;
    let routed: Option<SessionId> = if on_window { ws.filter(|s| Some(*s) == active) } else { active };
    let in_game = routed.and_then(|id| app.session_by(id)).is_some_and(|s| app.screen == Screen::Session && !s.menu && !s.keys && !s.share_panel && matches!(s.phase, Phase::Streaming | Phase::Reconnecting { .. }));
    let input = routed.and_then(|id| lives.get(id)).and_then(|l| l.handle.shared.current_input());
    if kind == sdl::SDL_MOUSEMOTION {
        wins.hover = Some(wins.list[wi].id);
    }
    if stale && matches!(kind, sdl::SDL_KEYDOWN | sdl::SDL_KEYUP | sdl::SDL_TEXTINPUT | sdl::SDL_MOUSEMOTION | sdl::SDL_MOUSEBUTTONDOWN | sdl::SDL_MOUSEBUTTONUP | sdl::SDL_MOUSEWHEEL) {
        match kind {
            // Le clavier est dans une fenêtre dont la session n'est pas (encore) active : seul « Ordinateur suivant » passe.
            sdl::SDL_KEYDOWN => {
                let (sc, repeat, mods) = ev.key();
                if !repeat && sc == sdl::SC_O && mods & sdl::KMOD_CTRL != 0 && mods & sdl::KMOD_ALT != 0 && mods & sdl::KMOD_SHIFT != 0 {
                    actions.push(Action::NextSession);
                }
            }
            // Rien pour l'ordinateur ; les éléments de la fenêtre (connexion, fin sur erreur, menu) se survolent.
            sdl::SDL_MOUSEMOTION if !wins.list[wi].hits.is_empty() => {
                let (x, y, _, _, _) = ev.motion();
                app.pointer = Some(to_ui(&wins.list[wi], app.layout, x, y));
                *dirty = true;
            }
            // Le clic qui fait passer la fenêtre devant rend sa session active, puis suit son chemin : sur un
            // élément de la fenêtre (Annuler, Fermer…) il agit sur lui, sinon il va aussi à son ordinateur, comme avec
            // une seule session, que le gestionnaire de fenêtres ait envoyé le focus avant le clic ou après (le
            // clic est gardé ici, la session est active, puis il part : une fois).
            sdl::SDL_MOUSEBUTTONDOWN => {
                let (b, x, y, _) = ev.button();
                if let Some(id) = ws {
                    let (ux, uy) = to_ui(&wins.list[wi], app.layout, x, y);
                    let on_ui = b == 1 && wins.list[wi].hits.iter().any(|(_, r)| r.contains(ux, uy));
                    focus::activate(app, lives, id);
                    if on_ui {
                        app.pointer = Some((ux, uy));
                        actions.extend(in_window(app, &mut wins.list[wi], |app| app.pointer_click(ux, uy)));
                    } else {
                        forward_click(app, lives, wins, wi, id, b, x, y);
                    }
                    *dirty = true;
                }
            }
            _ => {}
        }
        return;
    }
    match kind {
        sdl::SDL_QUIT => {
            // Quitter l'app termine toutes les sessions.
            for s in &app.sessions {
                actions.push(Action::Disconnect(s.id));
            }
            *quit = true;
        }
        // Un écran branché, débranché ou déplacé : les écrans de l'appareil sont relus.
        sdl::SDL_DISPLAYEVENT => RESCAN.store(true, Ordering::Relaxed),
        sdl::SDL_WINDOWEVENT => {
            let (e, _, _) = ev.window();
            let win_id = wins.list[wi].id;
            match e {
                // La fenêtre d'une session prend le clavier : sa session devient l'active.
                sdl::SDL_WINDOWEVENT_FOCUS_GAINED => {
                    if let Some(id) = ws {
                        focus::activate(app, lives, id);
                    }
                }
                sdl::SDL_WINDOWEVENT_FOCUS_LOST => {
                    // Le clavier passe à une autre fenêtre de la même session (plusieurs écrans) : les touches tenues
                    // restent tenues. Une autre session, ou une autre app : tout est relâché.
                    let focus = unsafe { sdl::SDL_GetKeyboardFocus() };
                    let elsewhere = ws.is_some() && !focus.is_null() && wins.index_of(unsafe { sdl::SDL_GetWindowID(focus) }).is_some_and(|i| i != wi && wins.list[i].session == ws);
                    if let Some(id) = ws
                        && let Some(l) = lives.get_mut(id)
                    {
                        if !elsewhere && let Some(p) = l.handle.shared.current_input() {
                            p.release_all();
                        }
                        l.bar_arm = None;
                    }
                }
                sdl::SDL_WINDOWEVENT_ENTER => wins.hover = Some(win_id),
                sdl::SDL_WINDOWEVENT_LEAVE => {
                    if app.layout == Layout::Desktop {
                        // Une autre fenêtre a déjà le pointeur : son mouvement a posé le sien.
                        if wins.hover == Some(win_id) {
                            app.pointer = None;
                            wins.hover = None;
                        }
                        // Le pointeur quitte la fenêtre : plus d'ouverture de la barre ; ouverte, elle se referme.
                        if let Some(id) = ws
                            && let Some(l) = lives.get_mut(id)
                        {
                            if l.bar_arm_win == Some(win_id) {
                                l.bar_arm = None;
                            }
                            if app.session_by(id).is_some_and(|s| s.bar) && l.bar_win == Some(win_id) {
                                l.bar_hide.get_or_insert(Instant::now() + Duration::from_millis(400));
                            }
                        }
                    }
                }
                // Fermer une fenêtre d'une session, c'est se déconnecter de cet ordinateur seulement.
                sdl::SDL_WINDOWEVENT_CLOSE => {
                    if let Some(id) = ws {
                        actions.push(Action::Disconnect(id));
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
            // Raccourcis de l'app : Ctrl+Alt+Maj + D (se déconnecter), F (plein écran), M (souris), O (ordinateur
            // suivant) ; ils visent la session de la fenêtre active.
            if app_combo && matches!(sc, sdl::SC_D | sdl::SC_F | sdl::SC_M | sdl::SC_O) {
                if down && !repeat {
                    match sc {
                        sdl::SC_D => actions.extend(routed.map(Action::Disconnect)),
                        sdl::SC_F => actions.push(Action::ToggleFullscreen(routed)),
                        sdl::SC_M => actions.extend(routed.map(Action::ToggleMouse)),
                        sdl::SC_O => actions.push(Action::NextSession),
                        _ => {}
                    }
                    if let Some(p) = &input {
                        p.release_all();
                    }
                }
                return;
            }
            if in_game {
                let Some(l) = routed.and_then(|id| lives.get_mut(id)) else { return };
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
                sdl::SC_TAB if app.screen == Screen::Join => Some(if mods & sdl::KMOD_SHIFT != 0 { Nav::Up } else { Nav::Down }),
                sdl::SC_TAB => Some(if mods & sdl::KMOD_SHIFT != 0 { Nav::Left } else { Nav::Right }),
                sdl::SC_F11 => {
                    actions.push(Action::ToggleFullscreen(routed));
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
                if let (Some(l), Some(p)) = (routed.and_then(|id| lives.get(id)), &input)
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
            // « Rejoindre un partage » : les chiffres du code, les lettres du prénom (accents compris).
            if app.screen == Screen::Join {
                for c in text.chars() {
                    actions.extend(app.nav(Nav::Char(c)));
                }
                *dirty = true;
            }
        }
        sdl::SDL_MOUSEMOTION => {
            let (x, y, xr, yr, _) = ev.motion();
            if in_game {
                let Some(id) = routed else { return };
                let Some(s) = app.session_by(id) else { return };
                let (bar, menu_open, relative) = (s.bar, s.bar_menu.is_some(), s.relative);
                if relative {
                    if let Some(p) = &input {
                        p.mouse_move(xr.clamp(-32768, 32767) as i16, yr.clamp(-32768, 32767) as i16);
                    }
                    return;
                }
                // Barre du bureau. Fermée : elle s'ouvre quand le pointeur reste un quart de
                // seconde contre le bord haut, dans la zone de sa largeur (`session_tick`
                // mesure le délai : un pointeur immobile n'envoie plus d'événements) ;
                // le quitter annule. Ouverte : elle reste sur elle, ou pendant un menu.
                let this = wins.list[wi].id;
                if app.layout == Layout::Desktop {
                    let (ux, uy) = to_ui(&wins.list[wi], app.layout, x, y);
                    let wpts = wins.list[wi].width_points(app.layout);
                    let bar_here = lives.get(id).is_some_and(|l| l.bar_win == Some(this));
                    if bar {
                        // Plusieurs écrans : la barre n'est que dans la fenêtre où elle s'est ouverte.
                        let over = {
                            let win = &mut wins.list[wi];
                            bar_here && (menu_open || ui::over_bar(&mut win.cv, app, app.session_by(id).unwrap(), wpts, &win.hits, ux, uy))
                        };
                        if over {
                            if let Some(l) = lives.get_mut(id) {
                                l.bar_hide = None;
                            }
                            if in_window(app, &mut wins.list[wi], |app| app.pointer_moved(ux, uy)) {
                                *dirty = true;
                            }
                            return;
                        }
                        app.pointer = Some((ux, uy));
                        if let Some(l) = lives.get_mut(id)
                            && l.bar_hide.is_none()
                        {
                            l.bar_hide = Some(Instant::now() + Duration::from_millis(400));
                        }
                    } else {
                        app.pointer = Some((ux, uy));
                        let at_edge = ui::at_bar_edge(&mut wins.list[wi].cv, app, app.session_by(id).unwrap(), wpts, ux, uy);
                        if let Some(l) = lives.get_mut(id) {
                            if at_edge && wins.any_focused() && !sdl::mouse_held() {
                                l.bar_arm_win = Some(this);
                                l.bar_arm.get_or_insert(Instant::now() + Duration::from_millis(250));
                            } else {
                                l.bar_arm = None;
                            }
                        }
                    }
                }
                if let (Some(p), Some(l)) = (&input, lives.get(id))
                    && let Some((hx, hy)) = host_point(&wins.list[wi], l, x, y)
                {
                    // Multi-écrans : la position part avec l'écran de cette fenêtre.
                    let screen = wins.list[wi].screen;
                    p.mouse_on(screen, hx, hy);
                    if let Some(l) = lives.get_mut(id) {
                        l.pointers.entry(screen).or_insert((None, true)).0 = Some((hx as f64, hy as f64));
                    }
                    wins.list[wi].present = true;
                }
                return;
            }
            let (ux, uy) = to_ui(&wins.list[wi], app.layout, x, y);
            if in_window(app, &mut wins.list[wi], |app| app.pointer_moved(ux, uy)) {
                *dirty = true;
            }
        }
        sdl::SDL_MOUSEBUTTONDOWN | sdl::SDL_MOUSEBUTTONUP => {
            let down = kind == sdl::SDL_MOUSEBUTTONDOWN;
            let (b, x, y, which) = ev.button();
            let touch = which == u32::MAX;
            if in_game {
                let Some(id) = routed else { return };
                let Some(s) = app.session_by(id) else { return };
                let (bar, menu_open, relative) = (s.bar, s.bar_menu.is_some(), s.relative);
                let (ux, uy) = to_ui(&wins.list[wi], app.layout, x, y);
                // Un toucher en haut de l'écran (Deck) ouvre le menu.
                if touch && down && app.layout.console() && uy < wins.list[wi].cv.h * 0.06 {
                    actions.extend(app.open_menu());
                    *dirty = true;
                    return;
                }
                // Les éléments de la barre sont ceux de sa fenêtre (plusieurs écrans : une seule barre).
                let bar_here = lives.get(id).is_some_and(|l| l.bar_win == Some(wins.list[wi].id));
                if bar && bar_here && wins.list[wi].hits.iter().any(|(_, r)| r.contains(ux, uy)) {
                    if down && b == 1 {
                        actions.extend(in_window(app, &mut wins.list[wi], |app| app.pointer_click(ux, uy)));
                        *dirty = true;
                    }
                    return;
                }
                if menu_open && down {
                    if let Some(s) = app.session_by_mut(id) {
                        s.bar_menu = None;
                    }
                    *dirty = true;
                    return;
                }
                if let Some(p) = input {
                    if !relative
                        && let Some(l) = lives.get(id)
                        && let Some((hx, hy)) = host_point(&wins.list[wi], l, x, y)
                    {
                        p.mouse_on(wins.list[wi].screen, hx, hy);
                    }
                    p.mouse_button(mouse_button(b), down);
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
                actions.extend(in_window(app, &mut wins.list[wi], |app| app.pointer_click(ux, uy)));
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
                // Le lecteur du cœur la branche sur chaque session (l'ordinateur ne perd pas la manette
                // quand on change de fenêtre) et la rebranche quand une session rejoint son ordinateur.
                let (v, pr) = pads.ids(slot);
                gamepads::plug(slot as u32, Some(v), Some(pr));
                if let Some(s) = app.session_mut() {
                    s.notice = Some((format!("{} {}{} : {}", tr("Manette", "Controller"), tr("J", "P"), slot + 1, pads.name(slot)), Instant::now()));
                }
                *dirty = true;
            }
        }
        sdl::SDL_CONTROLLERDEVICEREMOVED => {
            let (instance, _, _) = ev.pad();
            if let Some(slot) = pads.remove(instance) {
                gamepads::unplug(slot as u32);
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
                if which == 6 && app.sessions.is_empty() && app.screen != Screen::Launch {
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
            // Un invité sans les manettes (KyberNow Share : spectateur, ou clavier et souris seuls) n'envoie aucune
            // touche de manette, donc la combinaison du menu ne se forme pas : Start, Select ou B l'ouvrent. En
            // manette-souris (clavier et souris donnés), B est le clic droit : Start et Select seulement.
            if in_game
                && is_button
                && down
                && let Some((true, mouse)) = app.session().and_then(|s| s.role().map(|r| (!r.pads(), s.pad_mouse)))
                && (matches!(which, 4 | 6) || (which == 1 && !mouse))
            {
                actions.extend(app.open_menu());
                *dirty = true;
                return;
            }
            // Boutons et axes : au lecteur du cœur, qui ne les envoie qu'à la session active (en jeu, ni
            // menu ni touches ouverts) ; en manette-souris, les clics partent par la chaîne de la session active.
            if in_game && app.session().is_some_and(|s| s.pad_mouse) {
                if let Some(p) = &input {
                    pads.mouse_input(p, is_button, which, down, value);
                    if is_button {
                        gamepads::button(slot as u32, pads::button(which), down);
                    }
                }
                return;
            }
            if in_game {
                if is_button {
                    if let Some(b) = pads::core_button(which) {
                        gamepads::button(slot as u32, b, down);
                    }
                } else if let Some(a) = pads::core_axis(which) {
                    gamepads::axis(slot as u32, a, value);
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
fn handle_event(e: Event, app: &mut App, pads: &mut pads::Pads, lives: &mut Lives, wins: &mut Wins, actions: &mut Vec<Action>, pairing: &mut Vec<Arc<AtomicBool>>, tx: &Sender<Event>) {
    match e {
        Event::HostInfo { id, online, os, gpu, av1, virtual_display, max_screens, station, wake, own_device, share } => {
            let entry = app.live.entry(id.clone()).or_insert_with(HostLive::default);
            entry.online = if online { Online::Online } else { Online::Offline };
            if online {
                entry.virtual_display = virtual_display;
                entry.max_screens = max_screens;
                entry.station = station;
                entry.own_device = own_device;
                entry.share = share;
                // Le réveil a marché : l'ordinateur répond.
                if app.waking.as_ref().is_some_and(|(w, _)| *w == id) {
                    app.waking = None;
                }
            }
            app.station_progress(&id);
            // « Partager » attendait cette réponse : le partage s'ouvre, ou le panneau dit pourquoi il ne le peut pas.
            actions.extend(app.share_known(&id));
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
        Event::Session(id, e) => session_event(id, e, app, pads, lives, wins, actions),
        // KyberNow Share, invité : un ordinateur a pris le code. La session s'ouvre si la recherche est encore celle
        // de l'écran ; sinon (retour, autre recherche) l'invité repart de la Station tout de suite.
        Event::ShareJoined { attempt, result } => match result {
            Ok(found) if app.join_current(attempt) => {
                app.join.busy = false;
                connect_guest(app, lives, wins, tx, found);
            }
            Ok(found) => jobs::share_leave(found),
            Err(e) => {
                log::info!("partage : {e}");
                app.join_failed(attempt, e.to_string());
            }
        },
        Event::ShareHost { session, result } => app.share_reply(session, result),
        Event::StationReply { host, kind, result } => app.station_reply(&host, kind, result),
        Event::Woken { id, result } => {
            if let Err(e) = result {
                if app.waking.as_ref().is_some_and(|(w, _)| *w == id) {
                    app.waking = None;
                }
                app.toast(e);
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

/// Ce qu'une session dit à la boucle : chacune a son état (`App::sessions`) et son `Live`.
#[allow(clippy::too_many_arguments)]
fn session_event(id: SessionId, e: SessionEvent, app: &mut App, pads: &mut pads::Pads, lives: &mut Lives, wins: &mut Wins, actions: &mut Vec<Action>) {
    match e {
        SessionEvent::Status(st) => {
            let Some(si) = app.sessions.iter().position(|s| s.id == id) else { return };
            let is_active = app.active == Some(id);
            let s = &mut app.sessions[si];
            let was_streaming = matches!(s.phase, Phase::Streaming | Phase::Reconnecting { .. });
            s.phase = match st {
                Status::Preparing => Phase::Preparing,
                // La Station redémarre (dernier recours) : « Connexion » en attendant de le montrer.
                Status::Connecting => Phase::Connecting,
                Status::StationRestarting => Phase::StationRestarting,
                Status::FirstImages => Phase::FirstImages,
                Status::Streaming => {
                    if s.phase != Phase::Streaming && is_active {
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
                    if let Some(p) = lives.get(id).and_then(|l| l.handle.shared.current_input()) {
                        p.release_all();
                    }
                    // La session est finie : plus qu'une fenêtre ; une erreur se lit dans sa fenêtre principale.
                    wins.close_extras(id);
                    Phase::Ended { error }
                }
            };
            // Plusieurs écrans : la connexion reprend à sa première étape (un profil qui change leur
            // nombre, des écrans branchés ou débranchés) : les images s'effacent, le fond revient.
            // Sans nouvelle image, un échec est celui de la connexion.
            if was_streaming && s.phase == Phase::Preparing {
                s.opened_at = None;
                wins.forget_frames(id);
                // Les écrans attendus : ceux du plan que le cœur refait (profil ou écrans de l'appareil
                // changés) ; ses fenêtres suivent son annonce, une fois la session prête.
                if let Some(l) = lives.get(id)
                    && l.multi
                {
                    s.expected_screens = plan_of(l).map_or(1, |p| p.len());
                }
            }
            let finished = matches!(s.phase, Phase::Ended { error: None });
            let failed = matches!(s.phase, Phase::Ended { .. }) && !finished;
            // Une session finie n'a plus la souris des jeux (la carte d'erreur est une carte de l'interface) ; si
            // elle était l'active, le pointeur est rendu et visible tout de suite (l'état de SDL suit ensuite
            // d'après les sessions qui restent : `focus::sync`).
            if finished || failed {
                s.relative = false;
                s.bar = false;
                s.bar_menu = None;
                if is_active {
                    unsafe {
                        sdl::SDL_SetRelativeMouseMode(0);
                        sdl::SDL_ShowCursor(1);
                        sdl::SDL_SetCursor(sdl::SDL_GetDefaultCursor());
                    }
                }
            }
            if finished {
                // Fin sans erreur : la session sort de la liste, sa fenêtre se ferme ; la dernière ramène l'accueil.
                app.close_session(id);
                if app.sessions.is_empty() {
                    app.go(Screen::Home);
                }
            } else if failed && is_active {
                app.screen = Screen::Session;
                app.focus = Target::Back;
            }
        }
        SessionEvent::Mode(m) => {
            if let Some(l) = lives.get_mut(id) {
                l.mode = format!("{} × {}", m.width, m.height);
                l.hz = m.refresh;
            }
        }
        SessionEvent::Second { latency_ms, fps, mbps, lost, codec, audio_ms } => {
            let Some(l) = lives.get_mut(id) else { return };
            // Toutes les 10 s au journal : ce qu'on voit dans la barre.
            if l.started.elapsed().as_secs() % 10 == 0 {
                log::info!("session {} : latence {:?} ms, {fps:.0} i/s, {mbps:.1} Mbit/s, {lost} perdues, {codec}", l.host, latency_ms.map(|v| (v * 10.0).round() / 10.0));
            }
            if let Some(v) = latency_ms {
                l.latencies.push(v);
            }
            if fps > 0.0 {
                l.mbps.push(mbps);
            }
            if let Some(s) = app.session_by_mut(id) {
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
        SessionEvent::Notice(n) => {
            // Rien par-dessus l'image à son arrivée : une capsule seulement après les 3 premières secondes.
            if let Some(s) = app.session_by_mut(id)
                && s.opened_at.is_some_and(|t| t.elapsed() > Duration::from_secs(3))
            {
                s.notice = Some((n, Instant::now()));
            } else {
                log::info!("session : {n}");
            }
        }
        SessionEvent::Pointer { screen, position, hidden, shape } => {
            if let Some(l) = lives.get_mut(id) {
                l.pointers.insert(screen, (position, hidden));
            }
            // Position et visibilité sont celles de l'écran ; la forme, la dernière annoncée, vaut pour toutes les fenêtres de la session.
            if let Some((w, h, hx, hy, px)) = shape
                && w > 0
                && h > 0
            {
                let live = lives.get(id);
                for win in wins.list.iter_mut().filter(|win| win.session == Some(id)) {
                    set_cursor_shape(win, live, w, h, hx, hy, &px);
                }
            }
        }
        // Multi-écrans : les écrans que l'ordinateur a pris, et ceux qu'il n'a pas pu créer.
        SessionEvent::Screens => {
            let Some(l) = lives.get_mut(id) else { return };
            follow_screens(app, l, wins);
        }
        // Les vibrations : le cœur ignore celles d'une session qui n'est pas active.
        SessionEvent::Rumble(r) => pads.rumble(r.id as usize, r.large_motor, r.small_motor, r.duration_ms),
        // La combinaison de la manette n'arrive que de la session qui reçoit ses boutons : l'active.
        SessionEvent::Combo => {
            if app.active == Some(id) {
                actions.extend(app.open_menu());
            }
        }
        SessionEvent::GameFullscreen(g) => {
            if let Some(l) = lives.get_mut(id) {
                l.game_fullscreen = g;
            }
        }
        SessionEvent::DecoderFailed(e) => {
            if let Some(s) = app.session_by_mut(id) {
                s.notice = Some((format!("{} ({e})", tr("Décodeur indisponible", "Decoder unavailable")), Instant::now()));
            }
        }
        SessionEvent::Guests(view) => app.share_guests(id, view),
        SessionEvent::ShareRole { role, previous } => {
            app.share_role_changed(id, role, previous);
            // Le clavier saisi par une fenêtre en plein écran (les raccourcis du système vont alors à l'ordinateur)
            // ne l'est que pour un rôle qui l'envoie : Alt+Tab revient au système quand le clavier est repris.
            if let Some(wi) = wins.main_of(id)
                && (if wi == 0 { app.fullscreen } else { wins.list[wi].fullscreen } || app.layout.console())
            {
                unsafe { sdl::SDL_SetWindowKeyboardGrab(wins.list[wi].window, role.kbm() as c_int) };
            }
            // Un rôle sans clavier ni souris rend la souris des jeux (`session_tick` la coupe) et la manette en souris.
            if !role.kbm() {
                if let Some(l) = lives.get_mut(id) {
                    l.typing = false;
                }
                if app.active == Some(id) {
                    pads.set_mouse(false);
                }
            }
        }
        SessionEvent::ShareImage { label, notice } => app.share_image(id, label, notice),
        SessionEvent::ShareEnd(kind, text) => log::info!("partage : {kind} ({text})"),
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

/// Le mode de cette session : la surface de sa fenêtre pour une session de plus ; sinon celui de l'écran
/// de sa fenêtre (la machine garde l'écran de la dernière session ouverte).
fn session_mode_of(app: &mut App, wins: &Wins, l: &Live, h: &crate::model::Host) -> (u32, u32, u32) {
    if let Some((display, refresh)) = l.surface {
        return app.mode_for(h, display, refresh);
    }
    if let Some(wi) = wins.main_of(l.id) {
        set_machine_screen(app, &l.devices, wins.list[wi].screen);
    }
    app.session_mode(h)
}

/// La fenêtre de cette session passe devant et la session devient l'active (« Afficher », « Se connecter »
/// sur un ordinateur déjà en session, « Ordinateur suivant »).
fn show_session(app: &mut App, lives: &Lives, wins: &Wins, id: SessionId) {
    focus::activate(app, lives, id);
    if let Some(i) = wins.main_of(id) {
        sdl::raise(wins.list[i].window);
    }
}

/// Plein écran de la fenêtre d'une session (la fenêtre de l'app hors session). En plein écran pendant la
/// session, les raccourcis du système vont à l'ordinateur. Plusieurs écrans : pas de « Plein écran » (les
/// fenêtres restent en plein écran).
fn toggle_fullscreen(app: &mut App, lives: &Lives, wins: &mut Wins, id: Option<SessionId>) {
    if app.layout.console() {
        return;
    }
    let Some(id) = id else {
        app.fullscreen = !app.fullscreen;
        let window = wins.app().window;
        unsafe {
            sdl::SDL_SetWindowFullscreen(window, if app.fullscreen { sdl::SDL_WINDOW_FULLSCREEN_DESKTOP } else { 0 });
            sdl::SDL_SetWindowKeyboardGrab(window, (app.fullscreen && !lives.is_empty()) as c_int);
        }
        return;
    };
    if app.session_by(id).is_none_or(SessionUi::multi) {
        return;
    }
    let Some(wi) = wins.main_of(id) else { return };
    // La fenêtre de l'app suit `App::fullscreen` ; celles des sessions de plus, la leur.
    let on = !if wi == 0 { app.fullscreen } else { wins.list[wi].fullscreen };
    let window = wins.list[wi].window;
    unsafe {
        sdl::SDL_SetWindowFullscreen(window, if on { sdl::SDL_WINDOW_FULLSCREEN_DESKTOP } else { 0 });
        // Le clavier n'est saisi que pour une session qui l'envoie (pas un invité sans le clavier et la souris).
        sdl::SDL_SetWindowKeyboardGrab(window, (on && lives.get(id).is_some() && app.session_by(id).is_some_and(SessionUi::keyboard_mouse)) as c_int);
    }
    if wi == 0 {
        app.fullscreen = on;
    } else {
        wins.list[wi].fullscreen = on;
    }
    if let Some(s) = app.session_by_mut(id) {
        s.fullscreen = on;
    }
}

#[allow(clippy::too_many_arguments)]
fn perform(a: Action, app: &mut App, pads: &mut pads::Pads, lives: &mut Lives, wins: &mut Wins, tx: &Sender<Event>, pairing: &mut Vec<Arc<AtomicBool>>, quit: &mut bool) {
    match a {
        Action::Save => app.store.save(),
        Action::Quit => {
            // Quitter l'app termine toutes les sessions.
            for l in lives.iter() {
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
                unsafe { sdl::SDL_SetWindowFullscreen(wins.app().window, if full { sdl::SDL_WINDOW_FULLSCREEN_DESKTOP } else { 0 }) };
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
        Action::Minimize => unsafe { sdl::SDL_MinimizeWindow(wins.app().window) },
        Action::Maximize => unsafe {
            if sdl::SDL_GetWindowFlags(wins.app().window) & sdl::SDL_WINDOW_MAXIMIZED != 0 {
                sdl::SDL_RestoreWindow(wins.app().window);
                app.maximized = false;
            } else {
                sdl::SDL_MaximizeWindow(wins.app().window);
                app.maximized = true;
            }
        },
        Action::ToggleFullscreen(id) => toggle_fullscreen(app, lives, wins, id),
        Action::ToggleMouse(id) => {
            if let (Some(l), Some(s)) = (lives.get_mut(id), app.session_by(id)) {
                l.mouse = if s.relative { MouseMode::Desktop } else { MouseMode::Game };
            }
        }
        Action::OpenKeyboard(id) => {
            unsafe { sdl::SDL_StartTextInput() };
            if let Some(l) = id.and_then(|id| lives.get_mut(id)) {
                l.typing = app.layout.console();
            }
            if system::gamescope() {
                open_url("steam://open/keyboard");
            }
        }
        Action::Connect { host, profile } => connect(app, lives, wins, tx, &host, profile),
        Action::ShowSession(id) => show_session(app, lives, wins, id),
        Action::NextSession => {
            if let Some(id) = focus::next(lives) {
                show_session(app, lives, wins, id);
            }
        }
        Action::CancelConnect(id) | Action::Disconnect(id) => disconnect(app, lives, wins, id),
        Action::SetProfile(id, p) => {
            // Le profil choisi dans la barre s'applique tout de suite : le cœur en tire le nombre
            // d'écrans (son calcul sur ces réglages) et refait la session s'il change ; ses fenêtres
            // suivent son annonce (`follow_screens`), sans que l'app ouvre ni ferme rien d'elle-même.
            let Some((host, light)) = app.session_by(id).map(|s| (s.host.clone(), s.light)) else { return };
            let Some(h) = app.store.host(&host).cloned() else { return };
            if let Some(l) = lives.get_mut(id) {
                let mode = session_mode_of(app, wins, l, &h);
                let codec = app.codec_for(&h);
                push_settings(l, session::settings(&h, p, codec, mode, &app.machine, light), session::capabilities(&h, codec, &app.machine));
            }
        }
        Action::Light(id, on) => {
            let Some((host, profile)) = app.session_by(id).map(|s| (s.host.clone(), s.profile)) else { return };
            let Some(h) = app.store.host(&host).cloned() else { return };
            if let Some(l) = lives.get_mut(id) {
                let (w, hh, fps) = session_mode_of(app, wins, l, &h);
                let fps = if on { fps.min(60) } else { fps };
                let codec = app.codec_for(&h);
                push_settings(l, session::settings(&h, profile, codec, (w, hh, fps), &app.machine, on), session::capabilities(&h, codec, &app.machine));
                l.handle.session.set_mode(Mode { width: w, height: hh, refresh: fps });
            }
        }
        // « Couper le son » du menu : le choix de l'utilisateur, distinct du son que le cœur coupe aux
        // sessions en arrière-plan.
        Action::Mute(id, on) => {
            if let Some(l) = lives.get(id) {
                l.handle.shared.set_silenced(on);
            }
        }
        // « Garder le son » : le registre du cœur laisse la session audible sans être active.
        Action::KeepSound(id, on) => {
            if let Some(l) = lives.get(id) {
                focus::set_keep_sound(l, on);
            }
        }
        Action::PadMouse(id, on) => {
            if app.active == Some(id) {
                pads.set_mouse(on);
            }
        }
        Action::ReleaseAll(id) => {
            if let Some(l) = lives.get_mut(id) {
                if let Some(p) = l.handle.shared.current_input() {
                    p.release_all();
                    for slot in pads.slots() {
                        p.gamepad_axes_rest(slot as u32);
                    }
                }
                l.typing = false;
            }
        }
        Action::Shortcut(id, keys) => {
            if let Some(l) = lives.get(id)
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
        // KyberNow Share, invité : le code part aux Stations trouvées sur le réseau (celles de l'appairage), avec
        // l'empreinte qu'on leur connaît ; l'appareil se nomme comme pour Admin.
        Action::JoinShare { code, name, quality } => {
            let mut targets: Vec<ShareTarget> = app.found.iter().map(|f| ShareTarget { host: f.address.clone(), port: f.port, fingerprint: f.fingerprint.clone() }).collect();
            // Un ordinateur de l'appareil, en ligne, que le réseau n'annonce pas (Tailscale, WireGuard) : il peut partager aussi.
            for h in &app.store.hosts {
                if app.live.get(&h.id).is_some_and(|l| l.online == Online::Online) && !targets.iter().any(|t| t.host == h.address && t.port == h.port) {
                    targets.push(ShareTarget { host: h.address.clone(), port: h.port, fingerprint: h.fingerprint.clone() });
                }
            }
            // « Auto » : l'écran de la fenêtre et ce que le décodeur suit ; la Station en tire l'image de l'invité (jamais
            // au-dessus de celle de l'hôte). Les trois autres choix n'ont pas besoin de cette mesure.
            let auto = (quality == crate::app::ShareQuality::Auto).then(|| join_auto(app, wins));
            let request = JoinRequest { code, name, device: app.store.device_id.clone(), app: "linux".into(), quality: session::core_quality(quality), auto, ..JoinRequest::default() };
            jobs::share_join(targets, request, session::guest_capabilities(&app.machine), app.join.attempt, tx.clone());
        }
        // KyberNow Share, invité : la « Qualité de l'image » changée en session ; la Station relance l'image de l'invité
        // seulement, la session n'a rien à relancer (l'image reçue revient par `Platform::on_share_image`).
        Action::ShareQuality(id, q) => {
            if let Some(l) = lives.get(id) {
                l.handle.session.set_share_quality(session::core_quality(q));
            }
        }
        Action::Share(id, op) => {
            let host = lives.get(id).and_then(|l| app.store.host(&l.host)).cloned();
            match host {
                Some(h) => jobs::share_host(id, h, op, tx.clone()),
                None => app.share_reply(id, Err(tr("L'ordinateur n'est plus dans la liste de cet appareil.", "The computer is no longer in this device's list.").into())),
            }
        }
        Action::CopyText(text) => {
            if !sdl::set_clipboard(&text)
                && let Some(s) = app.session_mut()
            {
                s.share_copied = Some((Instant::now(), false));
            }
        }
        Action::TextInput => unsafe { sdl::SDL_StartTextInput() },
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

/// Ouvre une session sur cet ordinateur. La première prend la fenêtre de l'app (plein écran, ou tous les
/// écrans de l'appareil avec le multi-écrans) ; chaque session suivante (interface de bureau seulement,
/// quatre au plus, le registre du cœur refuse la cinquième) s'ouvre dans sa propre fenêtre, par-dessus,
/// centrée aux deux tiers de l'écran de la fenêtre active en 16:9, avec un seul écran (« Cet écran » :
/// deux sessions ne prennent jamais les mêmes écrans) à la taille de la fenêtre ; ces écrans et cette
/// fenêtre sont ceux des deux aides du cœur (`screens_for_extra_session`, `extra_session_window`). Elle est
/// inscrite au registre et devient la session active. Le même ordinateur deux fois : sa fenêtre passe devant.
fn connect(app: &mut App, lives: &mut Lives, wins: &mut Wins, tx: &Sender<Event>, host: &str, profile: crate::model::Profile) {
    open_session(app, lives, wins, tx, host, profile, None);
}

/// Ouvre la session d'invité d'un partage (KyberNow Share) : elle compte comme une session de plus (quatre au
/// plus, une fenêtre chacune, le registre du cœur). Sans place pour elle, l'invité repart de la Station.
fn connect_guest(app: &mut App, lives: &mut Lives, wins: &mut Wins, tx: &Sender<Event>, found: share::Found) {
    open_session(app, lives, wins, tx, "", crate::model::Profile::Jeu, Some(found));
}

/// « Auto » (KyberNow Share, invité) : ce que l'appareil sait afficher et décoder, que la Station reçoit avec le code
/// pour en tirer l'image de l'invité : l'écran de la fenêtre qui recevra la session (celle de l'app pour la
/// première, la fenêtre active pour les suivantes, comme `open_session`) et ce que le décodeur suit
/// (`session::decoder_caps`). L'image de l'hôte n'est pas connue d'ici : c'est la Station qui borne.
fn join_auto(app: &App, wins: &Wins) -> share::ShareAuto {
    let devices = screens::detect();
    let anchor = if app.sessions.is_empty() { 0 } else { app.active_id().and_then(|a| wins.main_of(a)).unwrap_or(0) };
    let (pixel, refresh) = devices.grouped.get(devices.current(wins.list[anchor].window)).map_or((app.machine.display, app.machine.refresh), |d| ((d.pixel_w, d.pixel_h), d.refresh));
    share::auto_quality(&session::auto_screen(pixel, refresh), &session::decoder_caps(&app.machine), None)
}

/// Le « ordinateur » d'une session d'invité : une enveloppe (nom, adresse de la Station qui partage, réglages par
/// défaut) que la session lit comme celle d'un ordinateur ; elle n'est jamais dans le magasin.
fn guest_host(found: &share::Found, title: &str) -> crate::model::Host {
    crate::model::Host {
        id: format!("share:{}", found.joined.guest),
        name: title.into(),
        address: found.target.host.clone(),
        port: found.target.port,
        fingerprint: found.target.fingerprint.clone(),
        user: String::new(),
        password: String::new(),
        os: None,
        gpu: None,
        av1: false,
        mac: None,
        wake: Vec::new(),
        settings: crate::model::HostSettings::default(),
        last: None,
    }
}

/// Une session de plus : celle d'un ordinateur de l'appareil (`found` nul), ou celle d'un invité.
#[allow(clippy::too_many_arguments)]
fn open_session(app: &mut App, lives: &mut Lives, wins: &mut Wins, tx: &Sender<Event>, host: &str, profile: crate::model::Profile, guest: Option<share::Found>) {
    if guest.is_none()
        && let Some(id) = app.session_of_host(host).map(|s| s.id)
    {
        show_session(app, lives, wins, id);
        return;
    }
    let first = app.sessions.is_empty();
    // Le Steam Deck, le salon et KyberNow Link OS gardent une session ; quatre ordinateurs au plus.
    if !first && (!app.multi_sessions() || app.sessions_full()) {
        if let Some(found) = guest {
            jobs::share_leave(found);
        }
        return;
    }
    let guest_title = guest.as_ref().map(|f| crate::app::guest_title(f.joined.computer.trim()));
    let h = match (&guest, &guest_title) {
        (Some(f), Some(title)) => guest_host(f, title),
        _ => {
            let Some(h) = app.store.host(host).cloned() else { return };
            h
        }
    };
    let id = app.new_session_id();
    // Les écrans de l'appareil à cet instant : « Cet écran » est celui de la fenêtre (la fenêtre de l'app
    // pour la première session, la fenêtre active pour les suivantes), dont la définition et la fréquence
    // donnent le mode d'une session à un écran.
    let devices = screens::detect();
    let anchor = if first { 0 } else { app.active_id().and_then(|a| wins.main_of(a)).unwrap_or(0) };
    let window_screen = devices.current(wins.list[anchor].window);
    if first {
        set_machine_screen(app, &devices, window_screen);
    }
    let codec = app.codec_for(&h);
    let device = device_info(app);
    // Un invité n'a ni batterie à ménager, ni souris des jeux d'office, ni écran virtuel : l'écran est celui de l'hôte.
    let light = guest.is_none() && h.settings.battery_light && app.machine.on_battery();
    let relative = guest.is_none() && h.settings.mouse == "game";
    let virtual_display = guest.is_none() && h.settings.virtual_display;
    // Multi-écrans (interface de bureau) : le cœur reçoit les écrans de l'appareil et décide du nombre
    // d'écrans (son calcul, sur les réglages du flux) ; sans (Steam Deck, salon, KyberNow Link OS), une
    // session à un écran. Une session de plus : les mêmes écrans réduits à « Cet écran » par le cœur, et sa
    // fenêtre.
    let all = (first && app.multi_screens() && guest.is_none() || !first).then(|| {
        let capabilities = session::capabilities(&h, codec, &app.machine);
        screens::core_screens(&devices, window_screen, &h, station_screens(app, &h), capabilities, app.machine.on_battery())
    });
    let mut base = if first { all } else { all.map(|a| kybernow_core::screens::screens_for_extra_session(&a)) };
    // Une session de plus a toujours sa propre fenêtre, jamais celle de l'app : sur l'écran de la fenêtre active, à
    // défaut sur le premier écran que le cœur connaît ; sans aucun écran, elle ne démarre pas.
    let extra_window = if first {
        None
    } else {
        let Some(w) = base.as_ref().and_then(|b| b.list.get(b.current).or_else(|| b.list.first())).map(kybernow_core::screens::extra_session_window) else {
            if let Some(found) = guest {
                jobs::share_leave(found);
            }
            return;
        };
        Some(w)
    };
    // Le mode : celui de l'écran de la fenêtre, ou, pour une session de plus, la surface de sa fenêtre.
    let surface = extra_window.map(|w| ((w.mode.width, w.mode.height), w.mode.refresh));
    let first_mode = match (&guest, surface) {
        // Un invité : l'écran du partage (celui de l'hôte), dans la fenêtre qu'il a.
        (Some(f), _) => (f.joined.display.width, f.joined.display.height, f.joined.display.refresh),
        (None, Some((display, refresh))) => app.mode_for(&h, display, refresh),
        (None, None) => app.session_mode(&h),
    };
    // Les réglages du flux : ceux que reçoivent la Station et le calcul du nombre d'écrans.
    let settings = session::settings(&h, profile, codec, first_mode, &app.machine, light);
    // Un invité n'a jamais plusieurs écrans : l'écran du partage, un seul.
    let plan = if guest.is_some() { None } else { base.as_ref().and_then(|b| screens::session_plan(b, &settings, virtual_display)) };
    // « Cet écran » est celui où se trouve la fenêtre pendant la session : avec plusieurs écrans,
    // elle passe en plein écran sur l'écran principal, et c'est là que le cœur la met (l'écran de
    // la session à un écran où le profil changé la ramènerait, et son mode), comme sur Mac, qui
    // relit l'écran de la fenêtre après l'avoir posée. Une session à un écran : celui de la fenêtre.
    let current = plan.as_ref().map_or(window_screen, |p| p[0].device);
    let mode = if current != window_screen {
        set_machine_screen(app, &devices, current);
        if let Some(b) = base.as_mut() {
            b.current = current;
        }
        app.session_mode(&h)
    } else {
        first_mode
    };
    let primary_mode = plan.as_deref().and_then(screens::primary_mode).map_or(mode, |m| (m.width, m.height, m.refresh));
    let title = session_title(&h.name);
    // La fenêtre d'une session de plus d'abord : sans elle, la session ne démarre pas.
    let own_window = match extra_window {
        None => None,
        Some(w) => {
            let frame = (w.x.round() as i32, w.y.round() as i32, w.width.round() as i32, w.height.round() as i32);
            let premul = wins.list[0].premul;
            match create_session_window(&title, frame, window_screen, premul) {
                Some(mut win) => {
                    win.session = Some(id);
                    Some(win)
                }
                None => {
                    if let Some(found) = guest {
                        jobs::share_leave(found);
                    }
                    return;
                }
            }
        }
    };
    let handle = session::start(id, &h, settings.clone(), mode, &app.machine, relative, device, if guest.is_some() { None } else { base.clone() }, current, guest.as_ref(), tx.clone());
    // Le registre du cœur : quatre au plus, et la session devient active. Refusée (il n'en reste pas de
    // place : l'interface l'aurait déjà grisé), elle s'arrête et sa fenêtre se ferme.
    let focus_id = match focus::register(&handle.session, &h.name) {
        Ok(f) => f,
        Err(_) => {
            handle.session.stop();
            if let Some(w) = own_window {
                w.destroy();
            }
            return;
        }
    };
    let codec_label = match codec {
        "h264" => "H.264",
        "av1" => "AV1",
        _ => "HEVC",
    };
    let mut ui = match &guest {
        // Arrivé en spectateur ; l'hôte donne la suite (`Platform::on_share_role`, avant la connexion).
        Some(f) => {
            let mut ui = SessionUi::guest(id, &h.id, f.joined.computer.trim(), session::role_of(f.joined.role));
            // La « Qualité de l'image » demandée à « Rejoindre », et l'image que la Station dit déjà (`on_share_image` la suit).
            if let Some(g) = ui.guest.as_mut() {
                g.quality = app.join.quality;
                g.image = f.joined.image.as_ref().and_then(|i| i.label());
            }
            ui
        }
        None => SessionUi::new(id, &h.id, &h.name, profile, light, relative, plan.as_ref().map_or(1, Vec::len)),
    };
    ui.fullscreen = first && app.fullscreen;
    let mut l = Live {
        id,
        handle,
        focus: focus_id,
        host: h.id.clone(),
        guest: guest.is_some(),
        surface,
        followed: current,
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
        bar_arm_win: None,
        bar_win: None,
        decoded: None,
        last_frame: None,
        shown: None,
        snapshot: None,
        snapshot_at: None,
        multi: guest.is_none() && base.is_some(),
        devices,
        screens_base: if guest.is_some() { None } else { base },
        settings,
        virtual_display,
        screens: 1,
        not_created: Vec::new(),
        placed: None,
        rescan_at: Instant::now(),
        changed: None,
    };
    let vsync = if first { wants_vsync(app, &h) } else { false };
    if first {
        // La première : la fenêtre de l'app, qui porte son titre.
        let w = wins.app();
        w.session = Some(id);
        w.extra = false;
        sdl::set_title(w.window, &title);
        w.screen = current;
        w.set_vsync(vsync);
    } else if let Some(w) = own_window {
        // Une session de plus : sa fenêtre, par-dessus. Jamais celle de la première : `own_window` existe
        // forcément ici (sans fenêtre, la session n'a pas démarré).
        wins.list.push(w);
    }
    app.open_session(ui);
    app.screen = Screen::Session;
    app.focus = Target::CancelConnect;
    // Plusieurs écrans : la fenêtre de l'app en plein écran sur l'écran principal, une fenêtre
    // plein écran de plus par autre écran (le fond de la connexion, dès le départ comme sur Mac) ;
    // ensuite le cœur annonce ce qu'il a pris et les fenêtres le suivent (`follow_screens`).
    if let Some(plan) = plan.as_deref() {
        take_over_main_window(app, &mut l, wins, plan[0].device);
        sync_windows(wins, id, &title, &plan_screens(plan), plan[0].device, &l.devices, true);
    }
    // Le clavier n'est saisi que pour une session qui l'envoie : un invité arrive en spectateur (rien à envoyer),
    // Alt+Tab et les raccourcis du système restent au système ; l'hôte lui donne le clavier plus tard (`ShareRole`).
    if first && guest.is_none() && (app.fullscreen || app.layout.console()) {
        unsafe { sdl::SDL_SetWindowKeyboardGrab(wins.app().window, 1) };
    }
    log::info!(
        "session : {} ({}, {} × {} à {} Hz, {codec}, {}, {} écran{}{})",
        h.name,
        profile.id(),
        primary_mode.0,
        primary_mode.1,
        primary_mode.2,
        if vsync { "synchronisé" } else { "immédiat" },
        plan.as_ref().map_or(1, Vec::len),
        if plan.as_ref().is_some_and(|p| p.len() > 1) { "s" } else { "" },
        if first { String::new() } else { format!(", dans sa fenêtre ({} sessions)", app.sessions.len()) }
    );
    lives.0.push(l);
    // Le presse-papiers de l'appareil est déjà dans la nouvelle session : elle l'annoncera à son ordinateur.
    focus::sync_clipboards(lives);
    // « Partager » dans la barre sait ce que l'ordinateur permet : Admin est relu une fois la session partie.
    if guest.is_none() {
        jobs::poll(h, false, tx.clone());
    }
}

/// « Se déconnecter » ou « Annuler » sur la session `id` : elle se termine, ses fenêtres de plus se
/// ferment tout de suite ; la sienne reste le temps de l'au revoir. Les autres sessions ne bougent pas.
fn disconnect(app: &mut App, lives: &mut Lives, wins: &mut Wins, id: SessionId) {
    // Une session déjà finie sur une erreur : fermer sa fenêtre la ferme (comme « Retour »).
    if app.session_by(id).is_some_and(|s| matches!(s.phase, Phase::Ended { .. })) {
        app.close_session(id);
        if app.sessions.is_empty() {
            app.go(Screen::Home);
        }
        return;
    }
    if let Some(l) = lives.get(id) {
        if let Some(p) = l.handle.shared.current_input() {
            p.release_all();
        }
        l.handle.session.stop();
        // Elle sort du registre : celle qui avait la main juste avant la reprend, son et entrées compris.
        focus::unregister(lives, l);
    }
    if let Some(s) = app.session_by_mut(id) {
        s.menu = false;
        s.keys = false;
        s.bar = false;
        s.bar_menu = None;
        s.relative = false;
        if !matches!(s.phase, Phase::Ended { .. }) {
            s.phase = Phase::Ending;
            s.ending_since.get_or_insert_with(Instant::now);
        }
    }
    // L'accueil qui revient montre déjà la dernière image de la session.
    if let (Some(l), Some(s)) = (lives.get(id), app.session_by(id))
        && !l.guest
        && app.store.app.keep_last_image
        && let Some(img) = l.snapshot.clone()
    {
        let host = s.host.clone();
        app.images.insert(host.clone(), img);
        app.image_is_last.insert(host, true);
    }
    // La fin de session ferme ses fenêtres de plus (la principale reste).
    wins.close_extras(id);
    if app.active == Some(id) {
        unsafe {
            sdl::SDL_SetRelativeMouseMode(0);
            sdl::SDL_ShowCursor(1);
            sdl::SDL_SetCursor(sdl::SDL_GetDefaultCursor());
        }
    }
}

/// Ouvre une adresse avec l'app du système (portail en Flatpak).
fn open_url(url: &str) {
    match std::process::Command::new("xdg-open").arg(url).spawn() {
        Ok(_) => log::info!("ouverture de {url}"),
        Err(e) => log::warn!("ouverture de {url} impossible ({e})"),
    }
}

/// La fin des sessions : celles dont l'interface a disparu (fin sans erreur, « Retour » sur une erreur) ou
/// s'est terminée (une erreur reste affichée) rendent leur dernière session ; les fenêtres des sessions
/// qui ne sont plus dans la liste se ferment (celle de l'app redevient l'accueil, ou se cache tant que les
/// sessions qui restent ont chacune la leur). Rend vrai si quelque chose a changé.
fn reap(app: &mut App, lives: &mut Lives, wins: &mut Wins) -> bool {
    let mut changed = false;
    let ended: Vec<SessionId> = lives.iter().map(|l| l.id).filter(|id| app.session_by(*id).is_none_or(|s| matches!(s.phase, Phase::Ended { .. }))).collect();
    for id in ended {
        if let Some(l) = lives.remove(id) {
            // Fin sur une erreur, ou décidée ailleurs (un autre appareil, Admin) : elle sort du registre si elle y est encore.
            focus::unregister(lives, &l);
            finish(app, l);
            end_windows(wins, id);
            changed = true;
        }
    }
    let mut i = 0;
    while i < wins.list.len() {
        match wins.list[i].session {
            Some(id) if app.session_by(id).is_none() => {
                changed = true;
                if i == 0 {
                    release_app_window(wins);
                    i += 1;
                } else {
                    wins.destroy_at(i);
                }
            }
            _ => i += 1,
        }
    }
    // La fenêtre de l'app : cachée tant que des sessions ont chacune la leur, rendue avec l'accueil.
    let w = &mut wins.list[0];
    if app.sessions.is_empty() && w.hidden {
        unsafe { sdl::SDL_ShowWindow(w.window) };
        w.hidden = false;
        w.present = true;
        changed = true;
    } else if !app.sessions.is_empty() && w.session.is_none() && !w.hidden {
        unsafe { sdl::SDL_HideWindow(w.window) };
        w.hidden = true;
        changed = true;
    }
    changed
}

/// La session est finie (son message d'erreur, s'il y en a un, reste affiché dans sa fenêtre) : ses fenêtres
/// ne saisissent plus le clavier (Alt+Tab revient au système) et n'ont plus d'image.
fn end_windows(wins: &mut Wins, session: SessionId) {
    for w in wins.list.iter_mut().filter(|w| w.session == Some(session)) {
        unsafe { sdl::SDL_SetWindowKeyboardGrab(w.window, 0) };
        w.video.reset();
        w.hint_key = None;
        w.present = true;
    }
    // La fenêtre de l'app reprend la synchro verticale du réglage.
    if wins.list[0].session == Some(session) {
        wins.list[0].set_vsync(true);
    }
}

/// La fenêtre de l'app n'est plus celle d'une session : son titre, plus de clavier saisi, la synchro
/// verticale du réglage, plus d'image.
fn release_app_window(wins: &mut Wins) {
    let w = &mut wins.list[0];
    w.session = None;
    w.extra = false;
    sdl::set_title(w.window, APP_TITLE);
    unsafe { sdl::SDL_SetWindowKeyboardGrab(w.window, 0) };
    w.set_vsync(true);
    w.video.reset();
    w.hint_key = None;
    w.hits.clear();
    w.present = true;
}

/// Fin de session : la dernière session (latence médiane et 95e centile, durée,
/// définition, fréquence, format, débit) pour Réglages › Statistiques. L'état global de SDL
/// (souris, curseur, économiseur d'écran, saisie de texte) et la session active suivante sont réglés
/// par `focus::sync` d'après le registre du cœur, les fenêtres par `reap`.
fn finish(app: &mut App, l: Live) {
    // Un invité ne laisse rien : ni dernière session, ni image, ni ordinateur ajouté.
    if l.guest {
        log::info!("session d'invité {} : {} s", l.host, l.started.elapsed().as_secs());
        return;
    }
    let id = l.host.clone();
    let mut lat = l.latencies.clone();
    lat.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let seconds = l.started.elapsed().as_secs();
    log::info!("session {} : {} s, {} secondes mesurées", l.host, seconds, lat.len());
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

    fn streaming(id: SessionId, host: &str, screens: usize) -> SessionUi {
        let mut s = SessionUi::new(id, host, &host.to_uppercase(), crate::model::Profile::Bureau, false, false, screens);
        s.phase = Phase::Streaming;
        s.screens = screens;
        s.opened_at = Some(Instant::now());
        s
    }

    /// Plusieurs fenêtres plein écran (multi-écrans) sur le seul écran de la machine d'essai :
    /// ouverture et fermeture selon les écrans voulus, une image et un dessin par fenêtre, les
    /// événements retrouvent leur fenêtre ; puis plusieurs ordinateurs : une fenêtre par session, la
    /// fin d'une session ne ferme que les siennes, les recherches se font par (session, écran). Il
    /// faut un affichage : `DISPLAY=:0 cargo test -p kybernow-linux -- --ignored multi_window`.
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
        let mut wins = Wins { prev: None, list: vec![Win::new(window, renderer, premul, 0, (640, 400))], hover: None };
        // La première session est celle de la fenêtre de l'app.
        wins.list[0].session = Some(1);

        // Les trois écrans du plan : la fenêtre de l'app et deux fenêtres de plus.
        sync_windows(&mut wins, 1, "A · KyberNow Link", &[0, 1, 2], 0, &devices, false);
        assert_eq!(wins.list.len(), 3);
        assert_eq!(wins.list.iter().map(|w| w.screen).collect::<Vec<_>>(), vec![0, 1, 2]);
        assert_eq!(wins.list.iter().map(|w| (w.session, w.extra)).collect::<Vec<_>>(), vec![(Some(1), false), (Some(1), true), (Some(1), true)]);
        let ids: Vec<u32> = wins.list.iter().map(|w| w.id).collect();
        assert!(ids.iter().all(|i| *i != 0) && ids[0] != ids[1] && ids[1] != ids[2], "chaque fenêtre a son identifiant");
        assert_eq!((wins.index_of(ids[2]), wins.index_of(0xFFFF_FF00)), (Some(2), None));
        assert_eq!((wins.index_of_screen(1, 1), wins.index_of_screen(2, 1), wins.main_of(1)), (Some(1), None, Some(0)));

        // Une image par fenêtre, posée dans le cadre de sa fenêtre.
        let mut app = App::new(Store::default(), Layout::Desktop);
        app.screen = Screen::Session;
        app.open_session(streaming(1, "a", 3));
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
            let s = app.session_by(1);
            draw_frame(w, false, None, None, s, &app);
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
            let s = app.session_by(1);
            draw_frame(w, true, None, None, s, &app);
        }

        // Plusieurs ordinateurs : la session suivante a sa fenêtre, décorée, à son nom, centrée aux deux tiers de l'écran.
        let extra = kybernow_core::screens::extra_session_window(&kybernow_core::screens::DeviceScreen { x: 0.0, y: 0.0, width: 1920.0, height: 1080.0, pixel_width: 1920, pixel_height: 1080, refresh: 60, primary: true });
        let frame = (extra.x.round() as i32, extra.y.round() as i32, extra.width.round() as i32, extra.height.round() as i32);
        assert_eq!(frame, (320, 180, 1280, 720));
        let mut b = create_session_window(&session_title("B"), frame, 0, premul).expect("la fenêtre d'une session de plus");
        b.session = Some(2);
        let b_id = b.id;
        wins.list.push(b);
        assert_eq!((wins.main_of(2), wins.index_of(b_id)), (Some(3), Some(3)));
        assert!(wins.list[3].window != window && wins.list[3].vsync == false);
        app.open_session(streaming(2, "b", 1));
        let s = app.session_by(2);
        let second = &mut wins.list[3];
        second.sizes();
        let (pw, ph) = (second.out_size.0 as u32, second.out_size.1 as u32);
        second.cv.resize(pw, ph, 1.0);
        draw_frame(second, false, None, None, s, &app);
        // Sa session est la sienne : la fin de la première ne ferme pas la fenêtre de la seconde.
        wins.close_extras(1);
        assert_eq!(wins.list.iter().map(|w| (w.session, w.extra)).collect::<Vec<_>>(), vec![(Some(1), false), (Some(2), false)]);
        // Des écrans de l'appareil dont la place a changé : leur fenêtre se ferme, les autres restent.
        sync_windows(&mut wins, 1, "A", &[0, 1, 2], 0, &devices, false);
        assert_eq!(wins.list.iter().map(|w| (w.session, w.screen)).collect::<Vec<_>>(), vec![(Some(1), 0), (Some(2), 0), (Some(1), 1), (Some(1), 2)]);
        wins.close_extras_where(1, |w| w.screen == 2);
        assert_eq!(wins.list.iter().map(|w| (w.session, w.screen)).collect::<Vec<_>>(), vec![(Some(1), 0), (Some(2), 0), (Some(1), 1)]);
        let kept = wins.list[2].id;
        sync_windows(&mut wins, 1, "A", &[0, 1, 2], 0, &devices, false);
        assert_eq!((wins.list.len(), wins.list[2].id), (4, kept), "la fenêtre qui reste n'est pas refaite");
        // Moins d'écrans (Station qui n'en crée plus que deux, profil Jeu) : les fenêtres de plus se ferment.
        sync_windows(&mut wins, 1, "A", &[0, 2], 0, &devices, false);
        assert_eq!(wins.list.iter().filter(|w| w.session == Some(1)).map(|w| w.screen).collect::<Vec<_>>(), vec![0, 2]);
        sync_windows(&mut wins, 1, "A", &[0], 0, &devices, false);
        assert_eq!(wins.list.iter().filter(|w| w.session == Some(1)).count(), 1);
        // L'écran principal devient un autre écran de l'appareil : la fenêtre principale le montre ; la seconde session ne bouge pas.
        sync_windows(&mut wins, 1, "A", &[1, 0], 1, &devices, false);
        assert_eq!(wins.list.iter().filter(|w| w.session == Some(1)).map(|w| w.screen).collect::<Vec<_>>(), vec![1, 0]);
        assert_eq!(wins.main_of(2).map(|i| wins.list[i].id), Some(b_id));
        wins.forget_frames(1);
        assert!(wins.list.iter().filter(|w| w.session == Some(1)).all(|w| !w.has_frame()));
        // Le pointeur : la fenêtre survolée disparaît avec elle.
        wins.hover = Some(b_id);
        let i = wins.index_of(b_id).unwrap();
        wins.destroy_at(i);
        assert!(wins.hover.is_none() && wins.index_of(b_id).is_none());
        wins.close_extras(1);
        assert_eq!(wins.list.len(), 1);
        let main = wins.list.pop().unwrap();
        main.destroy();
        unsafe { sdl::SDL_Quit() };
    }

    fn absent_station(id: &str, port: u16) -> crate::model::Host {
        crate::model::Host {
            id: id.into(),
            name: id.to_uppercase(),
            address: "127.0.0.1".into(),
            port,
            fingerprint: None,
            user: "u".into(),
            password: String::new(),
            os: None,
            gpu: None,
            av1: false,
            mac: None,
            wake: Vec::new(),
            settings: Default::default(),
            last: None,
        }
    }

    /// Plusieurs ordinateurs, de bout en bout côté appareil, sur des Stations qui n'existent pas (rien
    /// n'écoute sur 127.0.0.1) : la première session prend la fenêtre de l'app, la suivante la sienne,
    /// par-dessus, à la taille que donne le cœur ; toutes sont inscrites au registre du cœur, qui dit
    /// laquelle est active et laquelle garde son son ; le même ordinateur deux fois remet sa fenêtre
    /// devant ; la fin de la première cache la fenêtre de l'app, la fin de la dernière la rend avec
    /// l'accueil et vide le registre. Il faut un affichage ; SDL ne s'initialise qu'une fois à la fois par
    /// processus (et le registre du cœur est celui du processus), donc un test à la fois :
    /// `DISPLAY=:0 cargo test -p kybernow-linux -- --ignored --test-threads=1`.
    #[test]
    #[ignore]
    fn second_session_gets_its_own_window() {
        let dir = std::env::temp_dir().join(format!("kn-linux-sessions-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        unsafe {
            std::env::set_var("XDG_CONFIG_HOME", &dir);
            assert_eq!(sdl::SDL_Init(sdl::SDL_INIT_VIDEO | sdl::SDL_INIT_AUDIO | sdl::SDL_INIT_EVENTS), 0, "{}", sdl::error());
        }
        let title = CString::new(APP_TITLE).unwrap();
        let window = unsafe { sdl::SDL_CreateWindow(title.as_ptr(), sdl::SDL_WINDOWPOS_CENTERED, sdl::SDL_WINDOWPOS_CENTERED, 800, 500, sdl::SDL_WINDOW_SHOWN | sdl::SDL_WINDOW_RESIZABLE | sdl::SDL_WINDOW_ALLOW_HIGHDPI) };
        assert!(!window.is_null(), "{}", sdl::error());
        let renderer = unsafe { sdl::SDL_CreateRenderer(window, -1, sdl::SDL_RENDERER_ACCELERATED) };
        assert!(!renderer.is_null(), "{}", sdl::error());
        let premul = unsafe { sdl::SDL_ComposeCustomBlendMode(2, 6, 1, 2, 6, 1) };
        let mut wins = Wins { prev: None, list: vec![Win::new(window, renderer, premul, 0, (800, 500))], hover: None };
        let mut app = App::new(Store::default(), Layout::Desktop);
        for (id, port) in [("a", 1), ("b", 2), ("c", 3), ("d", 4), ("e", 5)] {
            app.store.hosts.push(absent_station(id, port));
        }
        app.go(Screen::Home);
        let (tx, _rx) = channel::<Event>();
        let (mut lives, mut pads, mut applied) = (Lives::default(), pads::Pads::default(), focus::Applied::default());
        let jeu = crate::model::Profile::Jeu;
        let core = kybernow_core::focus::global();
        let active_of = |lives: &Lives| core.active().and_then(|f| lives.by_focus(f)).map(|l| l.id);
        assert_eq!(core.count(), 0);

        // La première session : la fenêtre de l'app, à son nom, inscrite au registre.
        connect(&mut app, &mut lives, &mut wins, &tx, "a", jeu);
        assert_eq!((app.active_id(), app.sessions.len(), lives.0.len(), wins.list.len(), core.count()), (Some(1), 1, 1, 1, 1));
        assert_eq!((wins.list[0].session, wins.list[0].extra, wins.main_of(1)), (Some(1), false, Some(0)));
        assert_eq!(active_of(&lives), Some(1));
        assert!(focus::sync(&mut app, &lives, &mut pads, &mut applied));

        // La seconde : sa fenêtre, par-dessus, à la taille des deux tiers de l'écran ; elle est active pour
        // le cœur et pour l'interface. Son écran est « Cet écran » et son mode, la taille de sa fenêtre.
        connect(&mut app, &mut lives, &mut wins, &tx, "b", jeu);
        assert_eq!((app.active_id(), app.sessions.len(), wins.list.len(), core.count()), (Some(2), 2, 2, 2));
        assert_eq!((wins.list[1].session, wins.list[1].extra, wins.main_of(2), wins.list[1].vsync), (Some(2), false, Some(1), false));
        assert!(wins.list[1].window != window);
        assert_eq!(active_of(&lives), Some(2), "le registre du cœur dit laquelle est active");
        let second = lives.get(2).unwrap();
        assert!(second.multi && second.surface.is_some(), "écrans réduits à « Cet écran » par le cœur, mode de sa fenêtre");
        let (win_w, win_h) = (wins.list[1].out_size.0 as f64, wins.list[1].out_size.1 as f64);
        let (sw, sh) = second.surface.map(|((w, h), _)| (w as f64, h as f64)).unwrap();
        assert!((sw - win_w).abs() <= 2.0 && (sh - win_h).abs() <= 2.0, "la surface du mode est celle de la fenêtre : {sw}×{sh} contre {win_w}×{win_h}");
        assert!(focus::sync(&mut app, &lives, &mut pads, &mut applied));
        assert!(!focus::sync(&mut app, &lives, &mut pads, &mut applied), "rien n'a changé : rien à refaire");
        // « Garder le son » sur la première : le cœur la laisse audible ; l'interface le recopie.
        focus::set_keep_sound(lives.get(1).unwrap(), true);
        focus::sync(&mut app, &lives, &mut pads, &mut applied);
        assert!(core.keep_sound(lives.get(1).unwrap().focus) && app.session_by(1).unwrap().keep_sound && !app.session_by(2).unwrap().keep_sound);

        // Le même ordinateur deux fois : sa fenêtre passe devant et devient active, aucune session de plus.
        connect(&mut app, &mut lives, &mut wins, &tx, "a", jeu);
        assert_eq!((app.active_id(), app.sessions.len(), lives.0.len(), wins.list.len(), active_of(&lives)), (Some(1), 2, 2, 2, Some(1)));
        // « Ordinateur suivant » : celle qui s'est ouverte après l'active ; un ordinateur inconnu ne s'ouvre pas.
        assert_eq!(focus::next(&lives), Some(2));
        focus::activate(&mut app, &lives, 2);
        assert_eq!((app.active_id(), active_of(&lives), focus::next(&lives)), (Some(2), Some(2), Some(1)));
        connect(&mut app, &mut lives, &mut wins, &tx, "inconnu", jeu);
        assert_eq!(app.sessions.len(), 2);
        // Le clic qui fait passer une fenêtre devant rend sa session active (cœur et interface), que le focus
        // arrive avant ou après le clic ; il n'est pas perdu (la session des essais n'est pas à l'image : seule
        // l'activation se voit ici, `forward_click` n'envoie rien tant qu'il n'y a pas d'image ni de chaîne d'entrées).
        focus::activate(&mut app, &lives, 1);
        let second_window = wins.list[wins.main_of(2).unwrap()].id;
        let (mut actions, mut dirty, mut quit, mut start_down) = (Vec::new(), false, false, None);
        let mut ev = sdl::SDL_Event::default();
        ev.raw[0..4].copy_from_slice(&sdl::SDL_MOUSEBUTTONDOWN.to_ne_bytes());
        ev.raw[8..12].copy_from_slice(&second_window.to_ne_bytes());
        ev.raw[16] = 1;
        ev.raw[20..24].copy_from_slice(&10i32.to_ne_bytes());
        ev.raw[24..28].copy_from_slice(&10i32.to_ne_bytes());
        assert_eq!((app.active_id(), active_of(&lives)), (Some(1), Some(1)));
        handle_sdl(&ev, &mut app, &mut pads, &mut lives, &mut wins, &mut actions, &mut dirty, &mut quit, &mut start_down);
        assert_eq!((app.active_id(), active_of(&lives), dirty), (Some(2), Some(2), true), "le clic active la session de la fenêtre");
        // Quatre au plus : les deux suivantes s'ouvrent, la cinquième est refusée (interface et cœur).
        connect(&mut app, &mut lives, &mut wins, &tx, "c", jeu);
        connect(&mut app, &mut lives, &mut wins, &tx, "d", jeu);
        assert_eq!((app.sessions.len(), core.count(), wins.list.len()), (4, 4, 4));
        connect(&mut app, &mut lives, &mut wins, &tx, "e", jeu);
        assert_eq!((app.sessions.len(), core.count(), wins.list.len()), (4, 4, 4));
        // Une session s'arrête sans « Se déconnecter » (le cœur dit pourquoi), pendant qu'elle est active et en
        // souris des jeux : son message reste dans sa fenêtre, la souris est rendue, le cœur l'a désinscrite et elle
        // ne compte plus (quatre au plus, « Garder le son », « Afficher »).
        focus::activate(&mut app, &lives, 4);
        app.session_by_mut(4).unwrap().relative = true;
        session_event(4, SessionEvent::Status(Status::Ended { error: Some("Un autre appareil a pris la main".into()), report: None }), &mut app, &mut pads, &mut lives, &mut wins, &mut actions);
        {
            let s = app.session_by(4).unwrap();
            assert!(matches!(s.phase, Phase::Ended { error: Some(_) }) && !s.relative && !s.running(), "la souris des jeux est rendue");
        }
        assert!(reap(&mut app, &mut lives, &mut wins));
        focus::sync(&mut app, &lives, &mut pads, &mut applied);
        assert_eq!((app.sessions.len(), app.running_sessions(), core.count(), lives.0.len(), wins.list.len()), (4, 3, 3, 3, 4), "le message et sa fenêtre restent");
        assert_eq!(app.active_id(), Some(4), "la carte d'erreur reste la session active de l'interface tant que sa fenêtre a le clavier");
        assert!(!app.sessions_full() && app.session_of_host("d").is_none(), "une session finie ne compte plus : on peut s'y reconnecter");
        // Une session de plus s'ouvre à sa place (la finie attend « Fermer ») : quatre en cours, cinq fenêtres.
        connect(&mut app, &mut lives, &mut wins, &tx, "e", jeu);
        assert_eq!((app.sessions.len(), app.running_sessions(), core.count(), wins.list.len()), (5, 4, 4, 5));
        assert!(app.sessions_full());
        // « Fermer » : la fenêtre de la session finie se ferme, les autres continuent.
        app.activate_session(4);
        assert!(app.session_back().is_empty());
        assert!(reap(&mut app, &mut lives, &mut wins));
        focus::sync(&mut app, &lives, &mut pads, &mut applied);
        assert_eq!((app.sessions.len(), core.count(), wins.list.len()), (4, 4, 4));
        assert_eq!(app.active_id(), active_of(&lives), "la session que le cœur dit active est l'active de l'interface");
        for id in [5, 3] {
            app.close_session(id);
            reap(&mut app, &mut lives, &mut wins);
        }
        focus::sync(&mut app, &lives, &mut pads, &mut applied);
        assert_eq!((app.sessions.len(), core.count(), wins.list.len()), (2, 2, 2));

        // La session active se termine : celle qui l'était juste avant reprend la main, pour le cœur aussi.
        focus::activate(&mut app, &lives, 1);
        focus::activate(&mut app, &lives, 2);
        app.close_session(2);
        assert!(reap(&mut app, &mut lives, &mut wins));
        focus::sync(&mut app, &lives, &mut pads, &mut applied);
        assert_eq!((app.active_id(), active_of(&lives), core.count(), lives.0.len(), wins.list.len()), (Some(1), Some(1), 1, 1, 1));
        // La première se termine alors que la seconde (rouverte) tourne : la fenêtre de l'app se cache.
        connect(&mut app, &mut lives, &mut wins, &tx, "b", jeu);
        let reopened = app.active_id().unwrap();
        app.close_session(1);
        assert!(reap(&mut app, &mut lives, &mut wins));
        focus::sync(&mut app, &lives, &mut pads, &mut applied);
        assert_eq!((app.active_id(), active_of(&lives), core.count(), lives.0.len(), wins.list.len()), (Some(reopened), Some(reopened), 1, 1, 2));
        assert!(wins.list[0].hidden && wins.list[0].session.is_none() && wins.list[1].session == Some(reopened));
        // La dernière se termine : l'accueil revient, la fenêtre de l'app est rendue, le registre est vide.
        app.close_session(reopened);
        app.go(Screen::Home);
        assert!(reap(&mut app, &mut lives, &mut wins));
        assert_eq!((app.active_id(), lives.0.len(), wins.list.len(), core.count()), (None, 0, 1, 0));
        assert!(!wins.list[0].hidden && wins.list[0].session.is_none());
        assert!(focus::sync(&mut app, &lives, &mut pads, &mut applied), "plus de session : l'état global de SDL est rendu");
        // Quitter attend les sessions : chacune est arrêtée puis attendue, sans dépasser le délai.
        connect(&mut app, &mut lives, &mut wins, &tx, "a", jeu);
        connect(&mut app, &mut lives, &mut wins, &tx, "b", jeu);
        assert_eq!(lives.0.len(), 2);
        let started = Instant::now();
        stop_all(&lives, &wins);
        assert!(started.elapsed() < QUIT_WAIT, "les sessions ont fini avant le délai : {:?}", started.elapsed());
        let main = wins.list.pop().unwrap();
        main.destroy();
        unsafe { sdl::SDL_Quit() };
        let _ = std::fs::remove_dir_all(&dir);
    }
}
