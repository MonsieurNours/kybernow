//! Une partie sur console PlayStation, dans sa fenêtre Win32 (textes :
//! docs/link-ui.md, « Consoles PlayStation »). L'image passe par le même fil,
//! le même décodeur (FFmpeg, D3D11VA) et le même rendu Direct3D 11 qu'une
//! session d'ordinateur (`video.rs`, `render.rs`) ; le son par le moteur du
//! cœur et WASAPI (`audio.rs`) ; le protocole est dans le cœur
//! (`kybernow_core::playstation::play`). La fenêtre reste cachée jusqu'à la
//! première image (le passage de l'accueil, ou la fenêtre de connexion,
//! montre les étapes jusque-là), puis s'ouvre en plein écran ou aux deux
//! tiers de l'écran en 16:9, selon les réglages de la console ; le plein
//! écran rejoint ou quitté pendant la partie y est retenu.
//!
//! Clavier et souris (comme l'app Mac, `PlayStationInput.swift`) : les
//! touches partent par leur place (usage HID, `ps_keys.rs`) dès que la
//! fenêtre a le clavier ; un clic dans l'image capture la souris (pointeur
//! caché et retenu, déplacements bruts) ; Ctrl+Alt+Maj+M la rend, comme une
//! fenêtre qui perd le clavier. Ctrl+Alt+Maj+D se déconnecte, Ctrl+Alt+Maj+F
//! passe en plein écran ou en sort. Une manette branchée l'emporte : le cœur
//! ignore alors clavier et souris. Barre ouverte : rien ne part à la console.
//!
//! Clavier de la console (`KEYBOARD_OPEN`) : les touches donnent du texte
//! (celui de la disposition du clavier, `WM_CHAR`) au lieu de tenir la
//! manette ; le texte entier part à chaque changement, Effacement retire le
//! dernier caractère, Entrée valide, Échap annule ; flèches et touches de
//! fonction ne partent pas. Un bandeau le dit en bas de l'image.
//!
//! Les aides (« Cliquez dans l'image pour jouer… », « Ctrl+Alt+Maj+M pour
//! rendre la souris », le bandeau du clavier, le contenu protégé) sont de
//! petites fenêtres transparentes posées sur celle de la partie.

use std::cell::RefCell;
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicIsize, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

use windows::Win32::Foundation::{COLORREF, HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::HiDpi::GetDpiForWindow;
use windows::Win32::UI::Input::KeyboardAndMouse::*;
use windows::Win32::UI::Input::*;
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::{PCWSTR, w};

use kybernow_core::input::{MonotonicClock, PadKey};
use kybernow_core::media::{FOURCC_H264, FOURCC_HEVC, PacketHandler};
use kybernow_core::playstation::endpoint::Endpoint;
use kybernow_core::playstation::keymouse::Layout;
use kybernow_core::playstation::play::{KeyboardEvent, PadEvent, Play, PlayEnd, PlayOptions, PlayState, PlayStats};
use kybernow_core::playstation::regist::Registered;
use kybernow_core::playstation::remote::Remote;
use kybernow_core::playstation::settings::PlaySettings;
use kybernow_core::relay::RouteInfo;

use crate::audio::{self, AudioOutput};
use crate::gpu::Gpu;
use crate::i18n::t;
use crate::render::Renderer;
use crate::video::{self, ConsoleLink, Link, Shared, VideoHandler, VideoOptions};

const WM_APP_PS_STATE: u32 = WM_APP + 40;
const WM_APP_PS_COMMAND: u32 = WM_APP + 41;
const WM_APP_PS_KEYBOARD: u32 = WM_APP + 42;
const WM_APP_PS_MENU: u32 = WM_APP + 43;

const TIMER_SECOND: usize = 1;
const TIMER_EDGE: usize = 2;
const TIMER_HINT: usize = 3;

/// Le pointeur reste ce temps contre le bord haut pour ouvrir la barre.
const EDGE_DWELL_MS: u32 = 250;
/// « Ctrl+Alt+Maj+M pour rendre la souris » : un instant après la capture.
const RELEASE_HINT_MS: u32 = 3000;

/// Ce que l'app demande à la partie (fil de l'interface → fil de la fenêtre).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(usize)]
pub enum Command {
    Disconnect = 1,
    Mute,
    Mouse,
    Fullscreen,
    Rest,
    /// La fenêtre passe devant et reprend le clavier.
    Focus,
    /// La barre s'ouvre (la console reçoit une manette au repos) ou se ferme.
    BarOpen,
    BarClosed,
}

impl Command {
    fn from(v: usize) -> Option<Command> {
        use Command::*;
        [Disconnect, Mute, Mouse, Fullscreen, Rest, Focus, BarOpen, BarClosed].into_iter().find(|c| *c as usize == v)
    }
}

/// Ce que la barre montre de la partie (`PsBar` de l'interface).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct BarState {
    /// Une manette est branchée : pas de « Prendre la souris ».
    pub pad: bool,
    pub captured: bool,
    pub muted: bool,
    pub fullscreen: bool,
}

/// Ce que la partie dit à l'app (sur ses fils : à relayer sans attendre).
pub trait Events: Send + Sync {
    /// `connecting`, puis `streaming` (flux ouvert, image pas encore montrée).
    fn phase(&self, _phase: &'static str) {}
    /// La fenêtre montre sa première image.
    fn shown(&self) {}
    /// Une fois par seconde pendant le flux.
    fn stats(&self, _stats: &PlayStats) {}
    fn bar(&self, _bar: BarState) {}
    /// Le pointeur est resté contre le bord haut : la barre s'ouvre (`hwnd` : la fenêtre).
    fn top_edge(&self, _hwnd: isize) {}
    /// Options + Create + L1 + R1 tenus 250 ms : la barre s'ouvre (ou se ferme) et la manette la pilote.
    fn menu(&self, _hwnd: isize) {}
    /// Un clic dans l'image (souris prise) pendant que la barre est ouverte : elle se ferme.
    fn close_bar(&self) {}
    /// Barre ouverte par la manette : une touche de navigation.
    fn navigate(&self, _key: PadKey) {}
    /// Plein écran rejoint ou quitté pendant la partie (retenu pour la console).
    fn fullscreen(&self, _on: bool) {}
    /// Hors de chez soi : le chemin, au départ puis quand il change.
    fn route(&self, _route: &RouteInfo) {}
    /// Tout est arrêté, la fenêtre fermée. `end` : la fin dite par le cœur
    /// (`None` si la partie n'a jamais commencé) ; `quiet` : fermée ici ou
    /// mise en veille, rien à dire.
    fn ended(&self, _end: Option<PlayEnd>, _quiet: bool, _seconds: u64) {}
}

/// De quoi lancer une partie.
pub struct Start {
    /// `ps-<HOSTID>` (journal, fenêtres).
    pub id: String,
    pub name: String,
    pub endpoint: Endpoint,
    pub registered: Registered,
    pub settings: PlaySettings,
    pub layout: Layout,
    /// L'appareil décode le H.265 (Auto en profite).
    pub hevc: bool,
    pub ps5: bool,
    /// Hors de chez soi : la console jointe par une Station, gardée le temps de la partie.
    pub remote: Option<Remote>,
    /// Un point de l'écran où ouvrir la fenêtre (le milieu de l'accueil).
    pub near: Option<(i32, i32)>,
}

/// Ce que partagent la fenêtre, la partie du cœur et l'app.
struct Inner {
    id: String,
    name: String,
    events: Arc<dyn Events>,
    play: Mutex<Option<Play>>,
    remote: Mutex<Option<Remote>>,
    hwnd: AtomicIsize,
    states: Mutex<VecDeque<PlayState>>,
    keyboard: Mutex<VecDeque<KeyboardEvent>>,
    bar: Mutex<BarState>,
    /// La barre est ouverte : rien ne part à la console.
    bar_open: AtomicBool,
    /// Fermée ici (« Se déconnecter », fenêtre fermée) ou mise en veille ici.
    quiet: AtomicBool,
    end: Mutex<Option<PlayEnd>>,
    started: Instant,
}

impl Inner {
    fn with_play(&self, f: impl FnOnce(&Play)) {
        if let Some(p) = self.play.lock().unwrap().as_ref() {
            f(p);
        }
    }

    fn post(&self, msg: u32, w: usize) {
        let h = self.hwnd.load(Ordering::Relaxed);
        if h != 0 {
            unsafe {
                let _ = PostMessageW(Some(HWND(h as *mut _)), msg, WPARAM(w), LPARAM(0));
            }
        }
    }

    fn set_bar(&self, f: impl FnOnce(&mut BarState)) {
        let changed = {
            let mut b = self.bar.lock().unwrap();
            let before = *b;
            f(&mut b);
            (before != *b).then_some(*b)
        };
        if let Some(b) = changed {
            self.events.bar(b);
        }
    }
}

/// Une partie en cours (une par console).
pub struct PsSession {
    inner: Arc<Inner>,
}

impl PsSession {
    /// La partie, pas encore lancée (`run`) : l'app l'inscrit d'abord.
    pub fn new(start: &Start, events: Arc<dyn Events>) -> PsSession {
        let inner = Arc::new(Inner {
            id: start.id.clone(),
            name: start.name.clone(),
            events,
            play: Mutex::new(None),
            remote: Mutex::new(None),
            hwnd: AtomicIsize::new(0),
            states: Mutex::new(VecDeque::new()),
            keyboard: Mutex::new(VecDeque::new()),
            bar: Mutex::new(BarState::default()),
            bar_open: AtomicBool::new(false),
            quiet: AtomicBool::new(false),
            end: Mutex::new(None),
            started: Instant::now(),
        });
        PsSession { inner }
    }

    /// Lance la partie sur le fil de sa fenêtre.
    pub fn run(&self, start: Start) {
        let i = self.inner.clone();
        let spawned = std::thread::Builder::new().name("ps-fenetre".into()).spawn(move || run(start, i));
        if let Err(e) = spawned {
            log::error!("PlayStation : fil de la fenêtre refusé ({e})");
            self.inner.events.ended(Some(PlayEnd::Unreachable(e.to_string())), false, 0);
        }
    }

    pub fn id(&self) -> &str {
        &self.inner.id
    }

    pub fn name(&self) -> &str {
        &self.inner.name
    }

    /// La fenêtre de la partie (0 avant sa création ou après sa fermeture).
    pub fn hwnd(&self) -> isize {
        self.inner.hwnd.load(Ordering::Relaxed)
    }

    pub fn command(&self, c: Command) {
        if c == Command::Disconnect && self.hwnd() == 0 {
            // Fenêtre pas encore là (connexion annulée tout de suite) : la partie s'arrêtera à sa création.
            self.inner.quiet.store(true, Ordering::Relaxed);
        }
        self.inner.post(WM_APP_PS_COMMAND, c as usize);
    }

    pub fn bar_state(&self) -> BarState {
        *self.inner.bar.lock().unwrap()
    }

    /// La disposition du clavier et de la souris, tout de suite.
    pub fn set_layout(&self, layout: Layout) {
        self.inner.with_play(|p| p.set_layout(layout));
    }

    /// Intensités de la DualSense, tout de suite.
    pub fn set_intensities(&self, rumble: u8, triggers: u8) {
        self.inner.with_play(|p| p.set_intensities(rumble, triggers));
    }

    /// Le chemin par le relais (puce « Par le relais »), s'il y passe.
    pub fn route(&self) -> Option<RouteInfo> {
        self.inner.remote.lock().unwrap().as_ref().map(|r| r.route_info())
    }
}

// MARK: - Fil de la fenêtre

/// Ce que tient le fil de la fenêtre (un fil par partie : `thread_local`).
struct Ctx {
    inner: Arc<Inner>,
    hwnd: HWND,
    shared: Arc<Shared>,
    video: Arc<VideoHandler>,
    audio: AudioOutput,
    fullscreen_first: bool,
    shown: bool,
    streaming: bool,
    cant_display: bool,
    captured: bool,
    release_hint: bool,
    console_keyboard: bool,
    console_text: String,
    /// Moitié haute d'une paire UTF-16 reçue par `WM_CHAR`.
    surrogate: Option<u16>,
    saved: Option<(WINDOWPLACEMENT, isize)>,
    edge_armed: bool,
    last_route: Option<RouteInfo>,
    hint: Hint,
    notice: Hint,
}

thread_local! {
    static CTX: RefCell<Option<Ctx>> = const { RefCell::new(None) };
}

fn with_ctx<T>(f: impl FnOnce(&mut Ctx) -> T) -> Option<T> {
    CTX.with(|c| c.try_borrow_mut().ok().and_then(|mut c| c.as_mut().map(f)))
}

fn to_wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(Some(0)).collect()
}

fn register_class() -> windows::core::Result<()> {
    static DONE: OnceLock<()> = OnceLock::new();
    if DONE.get().is_some() {
        return Ok(());
    }
    unsafe {
        let instance = GetModuleHandleW(None)?;
        let class = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(proc),
            hInstance: instance.into(),
            hCursor: LoadCursorW(None, IDC_ARROW)?,
            hbrBackground: HBRUSH(GetStockObject(BLACK_BRUSH).0),
            lpszClassName: w!("KyberNowConsole"),
            ..Default::default()
        };
        RegisterClassExW(&class);
        let hint = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            lpfnWndProc: Some(hint_proc),
            hInstance: instance.into(),
            hCursor: LoadCursorW(None, IDC_ARROW)?,
            lpszClassName: w!("KyberNowConsoleHint"),
            ..Default::default()
        };
        RegisterClassExW(&hint);
    }
    let _ = DONE.set(());
    Ok(())
}

/// La fenêtre aux deux tiers de l'écran (de `near`, sinon le principal), en 16:9.
fn frame_near(near: Option<(i32, i32)>) -> (i32, i32, i32, i32) {
    unsafe {
        let p = near.map(|(x, y)| POINT { x, y }).unwrap_or_default();
        let monitor = MonitorFromPoint(p, if near.is_some() { MONITOR_DEFAULTTONEAREST } else { MONITOR_DEFAULTTOPRIMARY });
        let mut mi = MONITORINFO { cbSize: std::mem::size_of::<MONITORINFO>() as u32, ..Default::default() };
        let _ = GetMonitorInfoW(monitor, &mut mi);
        let area = mi.rcWork;
        let (aw, ah) = ((area.right - area.left) as f32, (area.bottom - area.top) as f32);
        let (mut w, mut h) = (aw * 2.0 / 3.0, aw * 2.0 / 3.0 * 9.0 / 16.0);
        if h > ah * 0.85 {
            h = ah * 0.85;
            w = h * 16.0 / 9.0;
        }
        let mut rect = RECT { left: 0, top: 0, right: w as i32, bottom: h as i32 };
        let _ = AdjustWindowRectEx(&mut rect, WS_OVERLAPPEDWINDOW, false, WINDOW_EX_STYLE(0));
        let (fw, fh) = (rect.right - rect.left, rect.bottom - rect.top);
        (area.left + (aw as i32 - fw) / 2, area.top + (ah as i32 - fh) / 2, fw, fh)
    }
}

fn client_size(hwnd: HWND) -> (u32, u32) {
    let mut r = RECT::default();
    unsafe {
        let _ = GetClientRect(hwnd, &mut r);
    }
    ((r.right - r.left).max(0) as u32, (r.bottom - r.top).max(0) as u32)
}

fn scale_of(hwnd: HWND) -> f32 {
    let dpi = unsafe { GetDpiForWindow(hwnd) };
    if dpi == 0 { 1.0 } else { dpi as f32 / 96.0 }
}

fn run(start: Start, inner: Arc<Inner>) {
    let fail = |inner: &Arc<Inner>, why: String| {
        log::error!("PlayStation [{}] : {why}", inner.name);
        inner.events.ended(Some(PlayEnd::Unreachable(why)), inner.quiet.load(Ordering::Relaxed), 0);
    };
    if let Err(e) = register_class() {
        return fail(&inner, format!("classe de fenêtre : {e}"));
    }
    let gpu = match Gpu::new() {
        Ok(g) => g,
        Err(e) => return fail(&inner, format!("Direct3D 11 : {e}")),
    };
    let title = to_wide(&format!("{} · {}", start.name, crate::shell::DISPLAY_NAME));
    let (x, y, w, h) = frame_near(start.near);
    let hwnd = unsafe {
        let Ok(instance) = GetModuleHandleW(None) else { return fail(&inner, "module".into()) };
        match CreateWindowExW(
            WINDOW_EX_STYLE(0),
            w!("KyberNowConsole"),
            PCWSTR(title.as_ptr()),
            WS_OVERLAPPEDWINDOW,
            x,
            y,
            w,
            h,
            None,
            None,
            Some(instance.into()),
            None,
        ) {
            Ok(h) => h,
            Err(e) => return fail(&inner, format!("fenêtre : {e}")),
        }
    };
    inner.hwnd.store(hwnd.0 as isize, Ordering::Relaxed);
    let profile = start.settings.profile(start.hevc, start.ps5);
    let link = Arc::new(Link::default());
    {
        let (lost, recovered) = (inner.clone(), inner.clone());
        let _ = link.console.set(ConsoleLink {
            lost: Box::new(move || lost.with_play(|p| p.key_frame_lost())),
            recovered: Box::new(move || recovered.with_play(|p| p.key_frame_recovered())),
        });
    }
    let shared = Shared::new(link, true);
    let size = client_size(hwnd);
    shared.set_client_size(size.0, size.1);
    let renderer = match Renderer::new(&gpu, hwnd, size, false) {
        Ok(r) => r,
        Err(e) => {
            unsafe {
                let _ = DestroyWindow(hwnd);
            }
            inner.hwnd.store(0, Ordering::Relaxed);
            return fail(&inner, format!("rendu : {e}"));
        }
    };
    let codec = if profile.hevc || profile.hdr { FOURCC_HEVC } else { FOURCC_H264 };
    let video = match video::spawn(renderer, hwnd.0 as isize, gpu.clone(), VideoOptions { hardware: true, codec }, shared.clone()) {
        Ok(v) => v,
        Err(e) => {
            unsafe {
                let _ = DestroyWindow(hwnd);
            }
            inner.hwnd.store(0, Ordering::Relaxed);
            return fail(&inner, e);
        }
    };
    let audio = audio::spawn(Arc::new(MonotonicClock::new()));
    crate::ps_haptics::retain();
    *inner.remote.lock().unwrap() = start.remote;

    let (rumble, triggers) = start.settings.intensities();
    log::info!(
        "PlayStation [{}] : partie demandée en {}×{} à {} i/s, {}, vibrations {rumble} %, gâchettes {triggers} %",
        start.name,
        profile.width,
        profile.height,
        profile.max_fps,
        if profile.hevc { "H.265" } else { "H.264" }
    );
    let on_state = {
        let i = inner.clone();
        Box::new(move |s: PlayState| {
            i.states.lock().unwrap().push_back(s);
            i.post(WM_APP_PS_STATE, 0);
        })
    };
    let on_pad = {
        let i = inner.clone();
        Box::new(move |e: PadEvent| match e {
            PadEvent::Menu => i.post(WM_APP_PS_MENU, 0),
            PadEvent::Navigate(k) => i.events.navigate(k),
        })
    };
    let play = Play::start(
        start.endpoint,
        &start.registered,
        PlayOptions { settings: start.settings.clone(), hevc_decoder: start.hevc },
        video.clone() as Arc<dyn PacketHandler>,
        Some(audio.engine().clone() as Arc<dyn PacketHandler>),
        on_state,
        on_pad,
    );
    {
        let i = inner.clone();
        play.set_on_keyboard(Some(Box::new(move |e: KeyboardEvent| {
            i.keyboard.lock().unwrap().push_back(e);
            i.post(WM_APP_PS_KEYBOARD, 0);
        })));
    }
    play.set_layout(start.layout);
    *inner.play.lock().unwrap() = Some(play);

    let pad = !kybernow_core::gamepads::global().pads().is_empty();
    inner.set_bar(|b| b.pad = pad);
    let hint = Hint::new(hwnd, Place::Bottom);
    let notice = Hint::new(hwnd, Place::Middle);
    CTX.with(|c| {
        *c.borrow_mut() = Some(Ctx {
            inner: inner.clone(),
            hwnd,
            shared,
            video: video.clone(),
            audio,
            fullscreen_first: start.settings.fullscreen,
            shown: false,
            streaming: false,
            cant_display: false,
            captured: false,
            release_hint: false,
            console_keyboard: false,
            console_text: String::new(),
            surrogate: None,
            saved: None,
            edge_armed: false,
            last_route: None,
            hint,
            notice,
        })
    });
    unsafe {
        let _ = SetTimer(Some(hwnd), TIMER_SECOND, 1000, None);
    }
    // Annulée avant que la fenêtre soit là.
    if inner.quiet.load(Ordering::Relaxed) {
        user_close("annulée pendant la connexion");
    }
    crate::window::run_loop();

    // Fin : la partie (si elle tourne encore), l'image, le son, les vibrations HD.
    let ctx = CTX.with(|c| c.borrow_mut().take());
    // Le fil de l'image s'arrête d'abord (la partie ne reste pas bloquée sur sa file), puis
    // la partie (sa fin peut prendre quelques secondes pendant la connexion), verrou relâché.
    video.stop();
    let play = inner.play.lock().unwrap().take();
    if let Some(mut p) = play {
        p.stop();
    }
    video.stop_and_wait(Duration::from_secs(2));
    if let Some(ctx) = ctx {
        ctx.audio.stop();
        ctx.hint.destroy();
        ctx.notice.destroy();
    }
    crate::ps_haptics::release();
    // Par le relais : la Station reste jointe jusqu'à la fin de l'arrêt.
    inner.remote.lock().unwrap().take();
    inner.hwnd.store(0, Ordering::Relaxed);
    let end = inner.end.lock().unwrap().take();
    let quiet = inner.quiet.load(Ordering::Relaxed);
    log::info!("PlayStation [{}] : partie finie ({end:?}{})", inner.name, if quiet { ", voulue ici" } else { "" });
    inner.events.ended(end, quiet, inner.started.elapsed().as_secs());
}

/// Fin voulue ici : la fenêtre se cache, la partie s'arrête, la fenêtre part
/// (la boucle de messages s'arrête ensuite).
fn user_close(why: &str) {
    let Some((inner, hwnd)) = with_ctx(|c| (c.inner.clone(), c.hwnd)) else { return };
    log::info!("PlayStation [{}] : {why}", inner.name);
    inner.quiet.store(true, Ordering::Relaxed);
    release_mouse();
    unsafe {
        let _ = ShowWindow(hwnd, SW_HIDE);
    }
    // La partie s'arrête après la boucle de messages (`run`), pas dans la procédure de la fenêtre.
    destroy(hwnd);
}

fn destroy(hwnd: HWND) {
    unsafe {
        let _ = KillTimer(Some(hwnd), TIMER_SECOND);
        let _ = KillTimer(Some(hwnd), TIMER_EDGE);
        let _ = KillTimer(Some(hwnd), TIMER_HINT);
        let _ = DestroyWindow(hwnd);
    }
}

/// Les états de la partie arrivés depuis le fil du cœur.
fn states() {
    let Some(inner) = with_ctx(|c| c.inner.clone()) else { return };
    loop {
        let Some(s) = inner.states.lock().unwrap().pop_front() else { break };
        match s {
            PlayState::Connecting => {
                with_ctx(|c| c.streaming = false);
                inner.events.phase("connecting");
            }
            PlayState::Streaming { width, height, hdr } => {
                log::info!("PlayStation [{}] : image {width}×{height} {}", inner.name, if hdr { "HDR" } else { "SDR" });
                with_ctx(|c| c.streaming = true);
                inner.events.phase("streaming");
                refresh_hints();
            }
            PlayState::CantDisplay(on) => {
                with_ctx(|c| c.cant_display = on);
                refresh_hints();
            }
            PlayState::Ended(end) => {
                // Mise en veille demandée ici : sa fin ne se dit pas.
                *inner.end.lock().unwrap() = Some(end);
                let hwnd = with_ctx(|c| {
                    c.streaming = false;
                    c.hwnd
                });
                release_mouse();
                if let Some(h) = hwnd {
                    unsafe {
                        let _ = ShowWindow(h, SW_HIDE);
                    }
                    destroy(h);
                }
                return;
            }
        }
    }
}

/// Le clavier de la console : ouvert, texte renvoyé par la console, fermé.
fn keyboard_events() {
    let Some(inner) = with_ctx(|c| c.inner.clone()) else { return };
    loop {
        let Some(e) = inner.keyboard.lock().unwrap().pop_front() else { break };
        match e {
            KeyboardEvent::Open(text) => {
                log::info!("PlayStation [{}] : clavier de la console ouvert", inner.name);
                // Les touches tenues ne tiennent plus la manette.
                inner.with_play(|p| p.release_all());
                with_ctx(|c| {
                    c.console_keyboard = true;
                    c.console_text = text;
                    c.surrogate = None;
                });
            }
            KeyboardEvent::Text(text) => {
                with_ctx(|c| {
                    if c.console_keyboard {
                        c.console_text = text;
                    }
                });
            }
            KeyboardEvent::Closed => {
                log::info!("PlayStation [{}] : clavier de la console fermé", inner.name);
                with_ctx(|c| {
                    c.console_keyboard = false;
                    c.console_text.clear();
                });
            }
        }
    }
    refresh_hints();
}

/// Les aides du bas de l'image et l'avis du contenu protégé, selon l'état.
fn refresh_hints() {
    with_ctx(|c| {
        let bar = *c.inner.bar.lock().unwrap();
        let text = if !c.shown || !c.streaming {
            None
        } else if c.console_keyboard {
            Some(t("ps.console_keyboard", &[]))
        } else if c.release_hint {
            Some(t("ps.release_mouse", &[]))
        } else if c.captured || bar.pad {
            None
        } else {
            Some(t("ps.click_to_play", &[]))
        };
        c.hint.show(text.as_deref());
        let notice = (c.shown && c.cant_display).then(|| t("ps.cant_display", &[]));
        c.notice.show(notice.as_deref());
    });
}

fn first_image() {
    let Some((inner, hwnd, fullscreen)) = with_ctx(|c| {
        if c.shown {
            return None;
        }
        c.shown = true;
        Some((c.inner.clone(), c.hwnd, c.fullscreen_first))
    })
    .flatten() else {
        return;
    };
    unsafe {
        let _ = ShowWindow(hwnd, SW_SHOW);
    }
    if fullscreen {
        set_fullscreen(true, false);
    }
    bring_to_front(hwnd);
    log::info!("PlayStation [{}] : fenêtre montrée (première image)", inner.name);
    inner.events.shown();
    refresh_hints();
}

fn bring_to_front(hwnd: HWND) {
    use windows::Win32::System::Threading::{AttachThreadInput, GetCurrentThreadId};
    unsafe {
        if SetForegroundWindow(hwnd).as_bool() && GetForegroundWindow() == hwnd {
            return;
        }
        let fg = GetForegroundWindow();
        let fg_thread = if fg.0.is_null() { 0 } else { GetWindowThreadProcessId(fg, None) };
        let me = GetCurrentThreadId();
        let attached = fg_thread != 0 && fg_thread != me && AttachThreadInput(me, fg_thread, true).as_bool();
        let _ = BringWindowToTop(hwnd);
        let _ = SetForegroundWindow(hwnd);
        if attached {
            let _ = AttachThreadInput(me, fg_thread, false);
        }
    }
}

/// Plein écran sans bordure sur l'écran de la fenêtre, ou retour à la fenêtre.
/// `remember` : changé pendant la partie (retenu pour la console). Les appels
/// à Windows se font hors de `with_ctx` : ils renvoient `WM_SIZE` tout de suite.
fn set_fullscreen(on: bool, remember: bool) {
    enum Step {
        Enter,
        Leave(WINDOWPLACEMENT, isize),
        Nothing,
    }
    let Some((inner, hwnd, step)) = with_ctx(|c| {
        let step = if on && c.saved.is_none() {
            Step::Enter
        } else if !on {
            match c.saved.take() {
                Some((wp, style)) => Step::Leave(wp, style),
                None => Step::Nothing,
            }
        } else {
            Step::Nothing
        };
        (c.inner.clone(), c.hwnd, step)
    }) else {
        return;
    };
    unsafe {
        match step {
            Step::Nothing => return,
            Step::Enter => {
                let mut wp = WINDOWPLACEMENT { length: std::mem::size_of::<WINDOWPLACEMENT>() as u32, ..Default::default() };
                let _ = GetWindowPlacement(hwnd, &mut wp);
                let style = GetWindowLongPtrW(hwnd, GWL_STYLE);
                with_ctx(|c| c.saved = Some((wp, style)));
                let monitor = MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST);
                let mut mi = MONITORINFO { cbSize: std::mem::size_of::<MONITORINFO>() as u32, ..Default::default() };
                let _ = GetMonitorInfoW(monitor, &mut mi);
                SetWindowLongPtrW(hwnd, GWL_STYLE, (WS_POPUP | WS_VISIBLE).0 as isize);
                let r = mi.rcMonitor;
                let _ = SetWindowPos(hwnd, Some(HWND_TOP), r.left, r.top, r.right - r.left, r.bottom - r.top, SWP_NOOWNERZORDER | SWP_FRAMECHANGED | SWP_SHOWWINDOW);
            }
            Step::Leave(wp, style) => {
                SetWindowLongPtrW(hwnd, GWL_STYLE, style);
                let _ = SetWindowPlacement(hwnd, &wp);
                let _ = SetWindowPos(hwnd, None, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_NOOWNERZORDER | SWP_FRAMECHANGED);
            }
        }
    }
    inner.set_bar(|b| b.fullscreen = on);
    if remember {
        inner.events.fullscreen(on);
    }
    refresh_hints();
}

fn is_fullscreen() -> bool {
    with_ctx(|c| c.saved.is_some()).unwrap_or(false)
}

/// Prend la souris (clic dans l'image, « Prendre la souris ») : pointeur
/// caché et retenu dans l'image, déplacements bruts vers la console.
fn capture_mouse() {
    let Some((inner, hwnd)) = with_ctx(|c| (c.inner.clone(), c.hwnd)) else { return };
    let already = with_ctx(|c| c.captured).unwrap_or(true);
    // Une manette branchée : le cœur ignore clavier et souris, rien à prendre.
    if already || inner.bar.lock().unwrap().pad || unsafe { GetForegroundWindow() } != hwnd {
        return;
    }
    // Barre ouverte : un clic dans l'image la ferme (l'app la cache).
    if inner.bar_open.load(Ordering::Relaxed) {
        inner.events.close_bar();
    }
    crate::window::raw_input_to(hwnd.0 as isize);
    unsafe {
        let mut r = RECT::default();
        let _ = GetClientRect(hwnd, &mut r);
        let mut tl = POINT { x: r.left, y: r.top };
        let mut br = POINT { x: r.right, y: r.bottom };
        let _ = ClientToScreen(hwnd, &mut tl);
        let _ = ClientToScreen(hwnd, &mut br);
        let clip = RECT { left: tl.x, top: tl.y, right: br.x, bottom: br.y };
        let _ = ClipCursor(Some(&clip));
        SetCursor(None);
        let _ = SetTimer(Some(hwnd), TIMER_HINT, RELEASE_HINT_MS, None);
    }
    with_ctx(|c| {
        c.captured = true;
        c.release_hint = true;
    });
    inner.set_bar(|b| b.captured = true);
    refresh_hints();
}

/// Rend la souris et relâche tout (Ctrl+Alt+Maj+M, fenêtre qui perd le clavier, fin).
fn release_mouse() {
    let Some((inner, was)) = with_ctx(|c| {
        let was = c.captured;
        c.captured = false;
        c.release_hint = false;
        (c.inner.clone(), was)
    }) else {
        return;
    };
    inner.with_play(|p| p.release_all());
    if was {
        unsafe {
            let _ = ClipCursor(None);
        }
        crate::window::raw_input_back();
        inner.set_bar(|b| b.captured = false);
    }
    refresh_hints();
}

fn toggle_mouse() {
    if with_ctx(|c| c.captured).unwrap_or(false) {
        release_mouse();
    } else {
        capture_mouse();
    }
}

fn shortcut_held() -> bool {
    unsafe { GetKeyState(VK_CONTROL.0 as i32) < 0 && GetKeyState(VK_MENU.0 as i32) < 0 && GetKeyState(VK_SHIFT.0 as i32) < 0 }
}

/// Ctrl+Alt+Maj+M, D, F : ceux d'une session d'ordinateur, pour cette partie.
fn shortcut(vk: u16) -> bool {
    match vk {
        0x4D => toggle_mouse(),
        0x44 => user_close("Ctrl+Alt+Maj+D"),
        0x46 => set_fullscreen(!is_fullscreen(), true),
        _ => return false,
    }
    true
}

/// Une touche du clavier de la console : Entrée valide, Échap annule,
/// Effacement retire le dernier caractère ; le texte vient de `WM_CHAR`.
fn console_key(vk: u16, repeat: bool) {
    let Some(inner) = with_ctx(|c| c.inner.clone()) else { return };
    match VIRTUAL_KEY(vk) {
        VK_RETURN if !repeat => {
            inner.with_play(|p| {
                p.keyboard_accept();
            });
            with_ctx(|c| {
                c.console_keyboard = false;
                c.console_text.clear();
            });
            refresh_hints();
        }
        VK_ESCAPE if !repeat => {
            inner.with_play(|p| {
                p.keyboard_reject();
            });
            with_ctx(|c| {
                c.console_keyboard = false;
                c.console_text.clear();
            });
            refresh_hints();
        }
        VK_BACK => {
            let text = with_ctx(|c| c.console_text.pop().map(|_| c.console_text.clone())).flatten();
            if let Some(text) = text {
                inner.with_play(|p| {
                    p.keyboard_text(&text);
                });
            }
        }
        _ => {}
    }
}

/// Un caractère pour le clavier de la console (`WM_CHAR`, UTF-16).
fn console_char(unit: u16) {
    let Some(inner) = with_ctx(|c| c.inner.clone()) else { return };
    let text = with_ctx(|c| {
        if !c.console_keyboard {
            return None;
        }
        let ch = match unit {
            0xD800..=0xDBFF => {
                c.surrogate = Some(unit);
                return None;
            }
            0xDC00..=0xDFFF => {
                let high = c.surrogate.take()?;
                char::decode_utf16([high, unit]).next()?.ok()?
            }
            u => {
                c.surrogate = None;
                char::from_u32(u as u32)?
            }
        };
        // Pas de caractères de commande (Entrée, Échap, Effacement : `console_key`).
        if ch.is_control() {
            return None;
        }
        // Ctrl tenu sans Alt (AltGr = Ctrl+Alt) : un raccourci, pas du texte.
        unsafe {
            if GetKeyState(VK_CONTROL.0 as i32) < 0 && GetKeyState(VK_MENU.0 as i32) >= 0 {
                return None;
            }
        }
        c.console_text.push(ch);
        Some(c.console_text.clone())
    })
    .flatten();
    if let Some(text) = text {
        inner.with_play(|p| {
            p.keyboard_text(&text);
        });
    }
}

fn key(lparam: LPARAM, pressed: bool) {
    let Some(inner) = with_ctx(|c| c.inner.clone()) else { return };
    if inner.bar_open.load(Ordering::Relaxed) {
        return;
    }
    let repeat = (lparam.0 >> 30) & 1 == 1;
    if pressed && repeat {
        return;
    }
    if let Some(usage) = crate::ps_keys::usage_of_lparam(lparam.0) {
        inner.with_play(|p| p.key(usage, pressed));
    }
}

fn mouse_button(button: u8, pressed: bool) {
    let Some((inner, captured)) = with_ctx(|c| (c.inner.clone(), c.captured)) else { return };
    if !captured {
        if pressed && button == 0 {
            capture_mouse();
        }
        return;
    }
    if inner.bar_open.load(Ordering::Relaxed) && pressed {
        return;
    }
    inner.with_play(|p| p.mouse_button(button, pressed));
}

fn raw_input(lparam: LPARAM) {
    let Some((inner, captured)) = with_ctx(|c| (c.inner.clone(), c.captured)) else { return };
    if !captured || inner.bar_open.load(Ordering::Relaxed) {
        return;
    }
    unsafe {
        let mut raw = RAWINPUT::default();
        let mut size = std::mem::size_of::<RAWINPUT>() as u32;
        let n = GetRawInputData(
            HRAWINPUT(lparam.0 as *mut _),
            RID_INPUT,
            Some(&mut raw as *mut _ as *mut _),
            &mut size,
            std::mem::size_of::<RAWINPUTHEADER>() as u32,
        );
        if n == u32::MAX || raw.header.dwType != RIM_TYPEMOUSE.0 {
            return;
        }
        let m = raw.data.mouse;
        if (m.usFlags.0 & MOUSE_MOVE_ABSOLUTE.0) == 0 && (m.lLastX != 0 || m.lLastY != 0) {
            let (dx, dy) = (m.lLastX as f32, m.lLastY as f32);
            inner.with_play(|p| p.mouse_move(dx, dy));
        }
    }
}

/// Contre le bord haut de l'image, dans la largeur de la barre, centrée.
fn in_edge_zone(hwnd: HWND, x: i32, y: i32) -> bool {
    let scale = scale_of(hwnd);
    let (w, _) = client_size(hwnd);
    let half = crate::window::bar_width_points() as f32 * scale / 2.0;
    y >= 0 && (y as f32) <= 2.0 * scale && ((x as f32) - w as f32 / 2.0).abs() <= half
}

fn cursor_in_edge_zone(hwnd: HWND) -> bool {
    unsafe {
        let mut p = POINT::default();
        if GetCursorPos(&mut p).is_err() {
            return false;
        }
        let _ = ScreenToClient(hwnd, &mut p);
        in_edge_zone(hwnd, p.x, p.y)
    }
}

fn mouse_buttons_held() -> bool {
    unsafe { [VK_LBUTTON, VK_RBUTTON, VK_MBUTTON].iter().any(|k| GetAsyncKeyState(k.0 as i32) < 0) }
}

/// Le pointeur libre a bougé : resté un quart de seconde contre le bord haut,
/// il ouvre la barre (un passage rapide ne l'ouvre pas).
fn edge_pointer(lparam: LPARAM) {
    let Some((inner, hwnd, armed, can)) = with_ctx(|c| (c.inner.clone(), c.hwnd, c.edge_armed, c.shown && c.streaming && !c.captured)) else {
        return;
    };
    let x = (lparam.0 & 0xffff) as i16 as i32;
    let y = ((lparam.0 >> 16) & 0xffff) as i16 as i32;
    let ok = can && !inner.bar_open.load(Ordering::Relaxed) && unsafe { GetForegroundWindow() } == hwnd && in_edge_zone(hwnd, x, y) && !mouse_buttons_held();
    if !ok {
        if armed {
            unsafe {
                let _ = KillTimer(Some(hwnd), TIMER_EDGE);
            }
            with_ctx(|c| c.edge_armed = false);
        }
        return;
    }
    if !armed {
        unsafe {
            let _ = SetTimer(Some(hwnd), TIMER_EDGE, EDGE_DWELL_MS, None);
        }
        with_ctx(|c| c.edge_armed = true);
    }
}

fn edge_fired() {
    let Some((inner, hwnd)) = with_ctx(|c| {
        c.edge_armed = false;
        (c.inner.clone(), c.hwnd)
    }) else {
        return;
    };
    unsafe {
        let _ = KillTimer(Some(hwnd), TIMER_EDGE);
    }
    let captured = with_ctx(|c| c.captured).unwrap_or(true);
    if captured || inner.bar_open.load(Ordering::Relaxed) || unsafe { GetForegroundWindow() } != hwnd || !cursor_in_edge_zone(hwnd) || mouse_buttons_held() {
        return;
    }
    inner.events.top_edge(hwnd.0 as isize);
}

/// Une fois par seconde : les mesures, une manette branchée ou débranchée, le chemin.
fn second() {
    let Some((inner, streaming)) = with_ctx(|c| (c.inner.clone(), c.streaming)) else { return };
    if streaming {
        let stats = inner.play.lock().unwrap().as_ref().map(|p| p.stats());
        if let Some(s) = stats {
            inner.events.stats(&s);
        }
    }
    let pad = !kybernow_core::gamepads::global().pads().is_empty();
    if inner.bar.lock().unwrap().pad != pad {
        inner.set_bar(|b| b.pad = pad);
        refresh_hints();
    }
    let route = inner.remote.lock().unwrap().as_ref().map(|r| r.route_info());
    if let Some(r) = route {
        let changed = with_ctx(|c| {
            let changed = c.last_route.as_ref() != Some(&r);
            c.last_route = Some(r.clone());
            changed
        })
        .unwrap_or(false);
        if changed {
            inner.events.route(&r);
        }
    }
}

fn command(c: usize) {
    let Some(c) = Command::from(c) else { return };
    let Some((inner, hwnd)) = with_ctx(|x| (x.inner.clone(), x.hwnd)) else { return };
    match c {
        Command::Disconnect => user_close("« Se déconnecter »"),
        Command::Mute => {
            let muted = with_ctx(|x| {
                let m = !x.audio.engine().is_muted();
                x.audio.engine().set_muted(m);
                m
            })
            .unwrap_or(false);
            inner.set_bar(|b| b.muted = muted);
        }
        Command::Mouse => {
            // Depuis la barre : la fenêtre reprend le clavier d'abord.
            bring_to_front(hwnd);
            toggle_mouse();
        }
        Command::Fullscreen => set_fullscreen(!is_fullscreen(), true),
        Command::Rest => {
            log::info!("PlayStation [{}] : mise en veille demandée", inner.name);
            inner.with_play(|p| {
                if p.goto_bed() {
                    inner.quiet.store(true, Ordering::Relaxed);
                }
            });
        }
        Command::Focus => {
            unsafe {
                if IsIconic(hwnd).as_bool() {
                    let _ = ShowWindow(hwnd, SW_RESTORE);
                }
            }
            bring_to_front(hwnd);
        }
        Command::BarOpen => {
            release_mouse();
            inner.bar_open.store(true, Ordering::Relaxed);
            inner.with_play(|p| p.set_captured(true));
        }
        Command::BarClosed => {
            inner.bar_open.store(false, Ordering::Relaxed);
            inner.with_play(|p| p.set_captured(false));
        }
    }
}

unsafe extern "system" fn proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    unsafe {
        match msg {
            WM_APP_PS_STATE => {
                states();
                LRESULT(0)
            }
            WM_APP_PS_KEYBOARD => {
                keyboard_events();
                LRESULT(0)
            }
            WM_APP_PS_COMMAND => {
                command(wparam.0);
                LRESULT(0)
            }
            WM_APP_PS_MENU => {
                if let Some(inner) = with_ctx(|c| c.inner.clone()) {
                    inner.events.menu(hwnd.0 as isize);
                }
                LRESULT(0)
            }
            m if m == crate::window::WM_APP_FIRST_IMAGE => {
                first_image();
                LRESULT(0)
            }
            WM_TIMER => {
                match wparam.0 {
                    TIMER_SECOND => second(),
                    TIMER_EDGE => edge_fired(),
                    TIMER_HINT => {
                        let _ = KillTimer(Some(hwnd), TIMER_HINT);
                        with_ctx(|c| c.release_hint = false);
                        refresh_hints();
                    }
                    _ => {}
                }
                LRESULT(0)
            }
            WM_SIZE | WM_MOVE => {
                with_ctx(|c| {
                    let (w, h) = client_size(hwnd);
                    c.shared.set_client_size(w, h);
                    c.video.resized();
                });
                if with_ctx(|c| c.captured).unwrap_or(false) {
                    // Le pointeur reste retenu dans la nouvelle image.
                    release_mouse();
                }
                refresh_hints();
                DefWindowProcW(hwnd, msg, wparam, lparam)
            }
            WM_KEYDOWN | WM_SYSKEYDOWN => {
                let vk = wparam.0 as u16;
                if shortcut_held() && shortcut(vk) {
                    return LRESULT(0);
                }
                // Alt+F4 ferme la fenêtre, comme ailleurs dans Windows.
                if msg == WM_SYSKEYDOWN && VIRTUAL_KEY(vk) == VK_F4 {
                    return DefWindowProcW(hwnd, msg, wparam, lparam);
                }
                let repeat = (lparam.0 >> 30) & 1 == 1;
                if with_ctx(|c| c.console_keyboard).unwrap_or(false) {
                    if !with_ctx(|c| c.inner.bar_open.load(Ordering::Relaxed)).unwrap_or(false) {
                        console_key(vk, repeat);
                    }
                    return LRESULT(0);
                }
                key(lparam, true);
                LRESULT(0)
            }
            WM_KEYUP | WM_SYSKEYUP => {
                key(lparam, false);
                LRESULT(0)
            }
            WM_CHAR | WM_SYSCHAR => {
                if with_ctx(|c| c.console_keyboard && !c.inner.bar_open.load(Ordering::Relaxed)).unwrap_or(false) {
                    console_char(wparam.0 as u16);
                }
                LRESULT(0)
            }
            WM_LBUTTONDOWN => {
                mouse_button(0, true);
                LRESULT(0)
            }
            WM_LBUTTONUP => {
                mouse_button(0, false);
                LRESULT(0)
            }
            WM_RBUTTONDOWN => {
                mouse_button(1, true);
                LRESULT(0)
            }
            WM_RBUTTONUP => {
                mouse_button(1, false);
                LRESULT(0)
            }
            WM_MBUTTONDOWN => {
                mouse_button(2, true);
                LRESULT(0)
            }
            WM_MBUTTONUP => {
                mouse_button(2, false);
                LRESULT(0)
            }
            WM_XBUTTONDOWN | WM_XBUTTONUP => {
                let b = if (wparam.0 >> 16) & 0xffff == 1 { 3 } else { 4 };
                mouse_button(b, msg == WM_XBUTTONDOWN);
                LRESULT(1)
            }
            WM_MOUSEMOVE => {
                edge_pointer(lparam);
                LRESULT(0)
            }
            WM_INPUT => {
                raw_input(lparam);
                DefWindowProcW(hwnd, msg, wparam, lparam)
            }
            WM_SETCURSOR => {
                if (lparam.0 & 0xffff) as u32 == HTCLIENT && with_ctx(|c| c.captured).unwrap_or(false) {
                    SetCursor(None);
                    return LRESULT(1);
                }
                DefWindowProcW(hwnd, msg, wparam, lparam)
            }
            WM_ACTIVATE => {
                if (wparam.0 & 0xffff) as u32 == WA_INACTIVE {
                    release_mouse();
                }
                DefWindowProcW(hwnd, msg, wparam, lparam)
            }
            WM_KILLFOCUS => {
                release_mouse();
                DefWindowProcW(hwnd, msg, wparam, lparam)
            }
            WM_ERASEBKGND => LRESULT(1),
            WM_CLOSE => {
                user_close("fenêtre de la partie fermée");
                LRESULT(0)
            }
            WM_DESTROY => {
                PostQuitMessage(0);
                LRESULT(0)
            }
            _ => DefWindowProcW(hwnd, msg, wparam, lparam),
        }
    }
}

// MARK: - Aides posées sur l'image

#[derive(Clone, Copy, PartialEq, Eq)]
enum Place {
    /// Une capsule en bas de l'image (aides du clavier et de la souris).
    Bottom,
    /// Au milieu (contenu protégé).
    Middle,
}

/// Une petite fenêtre transparente au-dessus de celle de la partie : un texte
/// clair sur une capsule sombre, qui ne prend ni clic ni clavier.
struct Hint {
    owner: HWND,
    hwnd: HWND,
    place: Place,
    text: Option<String>,
}

thread_local! {
    /// Le texte de chaque aide, lu par son `WM_PAINT`.
    static HINT_TEXT: RefCell<Vec<(isize, Vec<u16>)>> = const { RefCell::new(Vec::new()) };
}

impl Hint {
    fn new(owner: HWND, place: Place) -> Hint {
        let hwnd = unsafe {
            let instance = GetModuleHandleW(None).ok();
            CreateWindowExW(
                WS_EX_LAYERED | WS_EX_TRANSPARENT | WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW,
                w!("KyberNowConsoleHint"),
                w!(""),
                WS_POPUP,
                0,
                0,
                10,
                10,
                Some(owner),
                None,
                instance.map(Into::into),
                None,
            )
            .unwrap_or_default()
        };
        if !hwnd.0.is_null() {
            unsafe {
                let _ = SetLayeredWindowAttributes(hwnd, COLORREF(0), 225, LWA_ALPHA);
            }
        }
        Hint { owner, hwnd, place, text: None }
    }

    fn font(scale: f32) -> HFONT {
        unsafe {
            CreateFontW(
                -((14.0 * scale) as i32),
                0,
                0,
                0,
                FW_SEMIBOLD.0 as i32,
                0,
                0,
                0,
                DEFAULT_CHARSET,
                OUT_DEFAULT_PRECIS,
                CLIP_DEFAULT_PRECIS,
                CLEARTYPE_QUALITY,
                0,
                w!("Segoe UI"),
            )
        }
    }

    /// Montre ce texte (ou se cache), à sa place sous la fenêtre de la partie.
    fn show(&mut self, text: Option<&str>) {
        if self.hwnd.0.is_null() {
            return;
        }
        let visible = unsafe { IsWindowVisible(self.owner).as_bool() && !IsIconic(self.owner).as_bool() };
        let Some(text) = text.filter(|_| visible) else {
            self.text = None;
            unsafe {
                let _ = ShowWindow(self.hwnd, SW_HIDE);
            }
            return;
        };
        self.text = Some(text.to_string());
        let wide: Vec<u16> = text.encode_utf16().collect();
        HINT_TEXT.with(|h| {
            let mut h = h.borrow_mut();
            h.retain(|(k, _)| *k != self.hwnd.0 as isize);
            h.push((self.hwnd.0 as isize, wide.clone()));
        });
        unsafe {
            let scale = scale_of(self.owner);
            let dc = GetDC(Some(self.hwnd));
            let font = Self::font(scale);
            let old = SelectObject(dc, font.into());
            let mut size = windows::Win32::Foundation::SIZE::default();
            let _ = GetTextExtentPoint32W(dc, &wide, &mut size);
            SelectObject(dc, old);
            let _ = DeleteObject(font.into());
            ReleaseDC(Some(self.hwnd), dc);
            let (pad_x, pad_y) = ((16.0 * scale) as i32, (9.0 * scale) as i32);
            let (w, h) = (size.cx + 2 * pad_x, size.cy + 2 * pad_y);
            let mut client = RECT::default();
            let _ = GetClientRect(self.owner, &mut client);
            let mut tl = POINT { x: client.left, y: client.top };
            let mut br = POINT { x: client.right, y: client.bottom };
            let _ = ClientToScreen(self.owner, &mut tl);
            let _ = ClientToScreen(self.owner, &mut br);
            let x = (tl.x + br.x) / 2 - w / 2;
            let y = match self.place {
                Place::Bottom => br.y - h - (24.0 * scale) as i32,
                Place::Middle => (tl.y + br.y) / 2 - h / 2,
            };
            let rgn = CreateRoundRectRgn(0, 0, w + 1, h + 1, h, h);
            let _ = SetWindowRgn(self.hwnd, Some(rgn), true);
            let _ = SetWindowPos(self.hwnd, None, x, y, w, h, SWP_NOACTIVATE | SWP_NOZORDER | SWP_SHOWWINDOW);
            let _ = InvalidateRect(Some(self.hwnd), None, true);
        }
    }

    fn destroy(&self) {
        if !self.hwnd.0.is_null() {
            unsafe {
                let _ = DestroyWindow(self.hwnd);
            }
        }
    }
}

unsafe extern "system" fn hint_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    unsafe {
        match msg {
            WM_PAINT => {
                let mut ps = PAINTSTRUCT::default();
                let dc = BeginPaint(hwnd, &mut ps);
                let mut r = RECT::default();
                let _ = GetClientRect(hwnd, &mut r);
                let brush = CreateSolidBrush(COLORREF(0x00120e0c));
                FillRect(dc, &r, brush);
                let _ = DeleteObject(brush.into());
                let owner = GetWindow(hwnd, GW_OWNER).unwrap_or(hwnd);
                let font = Hint::font(scale_of(owner));
                let old = SelectObject(dc, font.into());
                SetBkMode(dc, TRANSPARENT);
                SetTextColor(dc, COLORREF(0x00ffffff));
                let mut text = HINT_TEXT.with(|h| h.borrow().iter().find(|(k, _)| *k == hwnd.0 as isize).map(|(_, t)| t.clone())).unwrap_or_default();
                DrawTextW(dc, &mut text, &mut r, DT_CENTER | DT_VCENTER | DT_SINGLELINE | DT_NOPREFIX);
                SelectObject(dc, old);
                let _ = DeleteObject(font.into());
                let _ = EndPaint(hwnd, &ps);
                LRESULT(0)
            }
            WM_NCHITTEST => LRESULT(HTTRANSPARENT as isize),
            WM_MOUSEACTIVATE => LRESULT(MA_NOACTIVATE as isize),
            _ => DefWindowProcW(hwnd, msg, wparam, lparam),
        }
    }
}
