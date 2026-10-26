//! Fenêtre de session Win32 : plein écran sans bordure, clavier et souris
//! vers l'ordinateur, pointeur de l'ordinateur, raccourcis de l'app
//! (Ctrl+Alt+Maj + lettre, comme ⌃⌥⌘ sur Mac).
//!
//! Les entrées partent par le module d'entrées du cœur
//! (`kybernow_core::input::InputPipeline`, une chaîne par démarrage de
//! l'image, voir `host.rs`) ; le pointeur de l'ordinateur et le mode de la
//! souris viennent de sa `PointerLayer` et de `MouseModeRules`. La fenêtre
//! ne fait que capter les événements de Windows et dessiner le pointeur.
//!
//! | Ctrl+Alt+Maj+D | se déconnecter |
//! | Ctrl+Alt+Maj+F | plein écran ou fenêtre |
//! | Ctrl+Alt+Maj+M | souris des jeux (relative) ou du bureau |
//!
//! La fenêtre vit sur le fil de sa session (voir `host.rs`) : l'accueil de
//! l'app garde le fil principal. Elle reste cachée jusqu'à la première image.

use std::sync::atomic::{AtomicBool, AtomicIsize, AtomicU32, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::HiDpi::GetDpiForWindow;
use windows::Win32::UI::Input::KeyboardAndMouse::*;
use windows::Win32::UI::Input::*;
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::{PCWSTR, w};

use kybernow_core::i18n::Lang;
use kybernow_core::input::{Clock, InputPipeline, MouseMode, MouseModeRules, PointerLayer, Shape};
use kybernow_core::kynput_rs::MouseButtonType;
use kybernow_core::session::{Session, Status};

use crate::i18n::t;
use crate::render::video_rect;
use crate::video::Shared;

pub const WM_APP_STATUS: u32 = WM_APP + 1;
pub const WM_APP_CURSOR: u32 = WM_APP + 2;
pub const WM_APP_MOUSE: u32 = WM_APP + 3;
/// Première image présentée : la fenêtre se montre.
pub const WM_APP_FIRST_IMAGE: u32 = WM_APP + 4;
/// Ordre de l'app (barre de session) : `wParam` = `Command`.
pub const WM_APP_COMMAND: u32 = WM_APP + 5;

/// Minuterie du recontrôle du mode de la souris (pointeur remontré en jeu).
const TIMER_MOUSE: usize = 1;
/// Minuterie de la barre de session : le pointeur reste contre le bord haut.
const TIMER_EDGE: usize = 2;
/// Le pointeur reste ce temps contre le bord haut avant que la barre s'ouvre
/// (un passage rapide ne l'ouvre pas).
const EDGE_DWELL_MS: u32 = 250;
/// Boutons de la souris enfoncés dans le `wParam` d'un `WM_MOUSEMOVE`
/// (MK_LBUTTON, MK_RBUTTON, MK_MBUTTON, MK_XBUTTON1, MK_XBUTTON2).
const MK_BUTTONS: usize = 0x01 | 0x02 | 0x10 | 0x20 | 0x40;
/// « Contre le bord haut » : 2 points au plus, mis à l'échelle de l'écran.
const EDGE_SLACK_POINTS: f32 = 2.0;
/// Largeur de la barre de session en points, tant que son interface ne l'a
/// pas mesurée (sa largeur naturelle, à quelques points près).
const BAR_WIDTH_GUESS: u32 = 860;
/// Largeur de la barre de session en points, telle que son interface la
/// mesure (elle change avec le texte des mesures et la langue).
static BAR_WIDTH: AtomicU32 = AtomicU32::new(BAR_WIDTH_GUESS);

/// La barre de session fait cette largeur (en points) : la zone du bord haut
/// où le pointeur l'ouvre est centrée de cette largeur.
pub fn set_bar_width(points: f64) {
    if points.is_finite() && points >= 1.0 {
        BAR_WIDTH.store(points.round() as u32, Ordering::Relaxed);
    }
}

/// La barre de session vient de se montrer ou de se cacher : le trait du bord
/// haut s'efface pendant qu'elle est là, et l'ouverture au pointeur attend.
pub fn set_bar_visible(visible: bool) {
    if let Some(a) = app() {
        a.bar_visible.store(visible, Ordering::Relaxed);
        a.refresh_hint();
    }
}

/// Où se place la barre de session : le haut de la zone cliente de la session,
/// en pixels de l'écran (son milieu, son bord haut) et l'échelle de son
/// écran. Rien si la fenêtre n'existe pas ou est réduite.
pub fn bar_anchor(hwnd: isize) -> Option<(i32, i32, f32)> {
    let hwnd = HWND(hwnd as *mut _);
    unsafe {
        if hwnd.0.is_null() || IsIconic(hwnd).as_bool() {
            return None;
        }
        let mut client = RECT::default();
        GetClientRect(hwnd, &mut client).ok()?;
        let mut tl = POINT { x: client.left, y: client.top };
        let mut br = POINT { x: client.right, y: client.bottom };
        let _ = ClientToScreen(hwnd, &mut tl);
        let _ = ClientToScreen(hwnd, &mut br);
        Some(((tl.x + br.x) / 2, tl.y, dpi_scale(hwnd)))
    }
}

/// Échelle de l'écran de la fenêtre (1,5 : 150 %) ; 1 sans fenêtre.
fn dpi_scale(hwnd: HWND) -> f32 {
    let dpi = unsafe { GetDpiForWindow(hwnd) };
    if dpi == 0 { 1.0 } else { dpi as f32 / 96.0 }
}

/// Auto : un jeu en plein écran qui cache son pointeur (comme sur Mac).
const MOUSE_RULES: MouseModeRules = MouseModeRules { needs_hidden_pointer: true };

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(usize)]
pub enum Command {
    Disconnect = 1,
    Fullscreen = 2,
    MouseToggle = 3,
    Focus = 4,
    // Mode de la souris choisi dans le menu de l'icône.
    MouseAuto = 5,
    MouseDesktop = 6,
    MouseGame = 7,
    // Menu Touches de la barre : combinaisons que ce PC garderait pour lui.
    CtrlAltDel = 10,
    WinKey = 11,
    AltTab = 12,
    PrintScreen = 13,
}

/// Codes de touche (jeu 1, `0xE000` : touche étendue).
mod sc {
    pub const CTRL: u16 = 0x1D;
    pub const ALT: u16 = 0x38;
    pub const TAB: u16 = 0x0F;
    pub const DEL: u16 = 0xE053;
    pub const LWIN: u16 = 0xE05B;
    pub const PRINT: u16 = 0xE037;
}

/// Envoie une combinaison : touches enfoncées dans l'ordre, relâchées à l'envers.
fn send_combo(app: &App, keys: &[u16]) {
    for &k in keys {
        send_key(app, k, true);
    }
    for &k in keys.iter().rev() {
        send_key(app, k, false);
    }
}

/// Touche vers l'ordinateur. Le relâchement d'une touche dont l'appui n'est
/// pas parti (raccourci de l'app, touche tenue avant la session ou avant une
/// relance de l'image) ne part pas.
fn send_key(app: &App, scancode: u16, pressed: bool) {
    let Some(input) = app.input() else { return };
    if !pressed && !input.held().lock().unwrap().is_held(scancode) {
        return;
    }
    input.key(scancode, pressed);
}

/// La touche est-elle tenue sur l'ordinateur ?
fn key_held(app: &App, scancode: u16) -> bool {
    app.input().is_some_and(|i| i.held().lock().unwrap().is_held(scancode))
}

/// Touches dont l'enfoncement a été pris par le crochet (leur relâchement suit).
static GRABBED: Mutex<Vec<u32>> = Mutex::new(Vec::new());

/// Crochet clavier de bas niveau : en plein écran et au premier plan, les
/// raccourcis que Windows garderait pour ce PC partent à l'ordinateur (touche
/// ⊞ et ses combinaisons, Alt+Tab, Alt+Échap, Ctrl+Échap, touche Menu).
/// Ctrl+Alt+Suppr reste à Windows : menu Touches de la barre.
pub fn install_keyboard_hook() -> Option<HHOOK> {
    unsafe {
        let module = GetModuleHandleW(None).ok()?;
        SetWindowsHookExW(WH_KEYBOARD_LL, Some(ll_keyboard), Some(module.into()), 0).ok()
    }
}

pub fn remove_keyboard_hook(hook: Option<HHOOK>) {
    if let Some(h) = hook {
        unsafe {
            let _ = UnhookWindowsHookEx(h);
        }
    }
    GRABBED.lock().unwrap().clear();
}

unsafe extern "system" fn ll_keyboard(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    unsafe {
        if code == HC_ACTION as i32 {
            let k = &*(lparam.0 as *const KBDLLHOOKSTRUCT);
            // Touches simulées (par une autre app, ou par Windows) : pas les nôtres.
            if (k.flags.0 & LLKHF_INJECTED.0) == 0 && grab_key(k, wparam.0 as u32) {
                return LRESULT(1);
            }
        }
        CallNextHookEx(None, code, wparam, lparam)
    }
}

/// Vrai : la touche part à l'ordinateur et Windows ne la voit pas.
fn grab_key(k: &KBDLLHOOKSTRUCT, msg: u32) -> bool {
    let down = msg == WM_KEYDOWN || msg == WM_SYSKEYDOWN;
    let Some(app) = app() else { return false };
    let sc = (k.scanCode & 0xFF) as u16;
    let code = if (k.flags.0 & LLKHF_EXTENDED.0) != 0 { 0xE000 | sc } else { sc };
    if !down {
        let mut grabbed = GRABBED.lock().unwrap();
        if let Some(i) = grabbed.iter().position(|&v| v == k.vkCode) {
            grabbed.remove(i);
            send_key(&app, code, false);
            return true;
        }
        return false;
    }
    // Répétition d'une touche déjà prise : prise aussi, même si la session a
    // perdu le premier plan entre-temps (sinon Windows verrait l'enfoncement
    // sans le relâchement, et la touche resterait tenue sur ce PC).
    if GRABBED.lock().unwrap().contains(&k.vkCode) {
        return true;
    }
    // Pas de verrou ici : `set_fullscreen` tient le sien pendant SetWindowPos,
    // et Windows peut appeler ce crochet sur ce même fil pendant ce temps.
    if !app.fullscreen_now.load(Ordering::Relaxed) || unsafe { GetForegroundWindow() } != app.hwnd() {
        return false;
    }
    let alt = (k.flags.0 & LLKHF_ALTDOWN.0) != 0;
    let ctrl = unsafe { GetAsyncKeyState(VK_CONTROL.0 as i32) } < 0;
    let vk = VIRTUAL_KEY(k.vkCode as u16);
    let grab = match vk {
        VK_LWIN | VK_RWIN | VK_APPS => true,
        VK_TAB => alt,
        VK_ESCAPE => alt || ctrl,
        _ => false,
    };
    if !grab {
        return false;
    }
    let mut grabbed = GRABBED.lock().unwrap();
    if !grabbed.contains(&k.vkCode) {
        grabbed.push(k.vkCode);
    }
    // Les répétitions de Windows ne partent pas (l'ordinateur répète lui-même).
    if !key_held(&app, code) {
        send_key(&app, code, true);
    }
    true
}

/// Format demandé après un changement de profil (décodeur de secours).
pub fn set_session_codec(fourcc: u32) {
    if let Some(a) = app() {
        a.shared.codec.store(fourcc, Ordering::Relaxed);
    }
}

/// La session est en plein écran (menu de l'icône : « Plein écran » coché).
pub fn fullscreen_now() -> bool {
    app().is_some_and(|a| a.fullscreen_now.load(Ordering::Relaxed))
}

/// Session en cours, pour l'accueil (changement de profil).
pub fn current_session() -> Option<Arc<Session>> {
    app().and_then(|a| a.session.get().cloned())
}

/// Pointeur de Windows fait de la forme annoncée par l'ordinateur.
#[derive(Default)]
struct CursorState {
    current: isize,
    /// Pixels de la forme en place et l'échelle à laquelle il a été construit :
    /// les annonces de position (jusqu'à 120 par seconde) ne le refont pas.
    built: Option<(Arc<[u32]>, f32)>,
}

pub struct App {
    pub hwnd: AtomicIsize,
    pub host_name: String,
    /// Chaîne d'entrées de l'image en cours (cœur) ; aucune entre deux images.
    input: Mutex<Option<Arc<InputPipeline>>>,
    /// Pointeur de l'ordinateur (cœur) : survit aux relances de l'image.
    pub pointer: Arc<PointerLayer>,
    pub clock: Arc<dyn Clock>,
    /// Souris relative (jeux), appliquée à chaque nouvelle chaîne d'entrées.
    relative: AtomicBool,
    pub shared: Arc<Shared>,
    pub session: OnceLock<Arc<Session>>,
    pub host_size: Mutex<(u32, u32)>,
    /// Écran de l'ordinateur affiché (`Display::id`).
    pub display_id: AtomicU32,
    pub status: Mutex<Option<Status>>,
    pub latency: Mutex<Option<f64>>,
    pub fps: Mutex<u64>,
    /// Un jeu en plein écran sur l'ordinateur (console).
    pub game: AtomicBool,
    /// Auto, ou forcé par Ctrl+Alt+Maj+M et la barre (Jeu, Bureau).
    pub mouse_mode: Mutex<MouseMode>,
    cursor: Mutex<CursorState>,
    saved_placement: Mutex<Option<(WINDOWPLACEMENT, isize)>>,
    pub closing: AtomicBool,
    pub error: Mutex<Option<String>>,
    pub events: Arc<dyn crate::host::HostEvents>,
    /// Plein écran quand la fenêtre se montre.
    pub fullscreen: bool,
    /// Mesures dans le titre (ligne de commande) ; sinon le nom seul.
    title_stats: bool,
    /// Plein écran en ce moment (lu par le crochet clavier, sans verrou).
    fullscreen_now: AtomicBool,
    shown: AtomicBool,
    /// La fenêtre est la fenêtre active (premier plan).
    active: AtomicBool,
    /// La barre de session est affichée (fenêtre de l'interface).
    bar_visible: AtomicBool,
    /// Le pointeur est dans la zone du bord haut et la minuterie court.
    edge_armed: AtomicBool,
}

/// La session en cours (une à la fois).
static APP: Mutex<Option<Arc<App>>> = Mutex::new(None);

fn app() -> Option<Arc<App>> {
    APP.lock().unwrap().clone()
}

impl App {
    pub fn new(
        host_name: String,
        clock: Arc<dyn Clock>,
        shared: Arc<Shared>,
        events: Arc<dyn crate::host::HostEvents>,
        fullscreen: bool,
        title_stats: bool,
    ) -> Arc<Self> {
        let app = Arc::new(Self {
            hwnd: AtomicIsize::new(0),
            host_name,
            input: Mutex::new(None),
            // La souris de Windows est celle du système : pas de position tenue.
            pointer: Arc::new(PointerLayer::new(clock.clone(), false)),
            clock,
            relative: AtomicBool::new(false),
            shared,
            session: OnceLock::new(),
            host_size: Mutex::new((1920, 1080)),
            display_id: AtomicU32::new(0),
            status: Mutex::new(None),
            latency: Mutex::new(None),
            fps: Mutex::new(0),
            game: AtomicBool::new(false),
            mouse_mode: Mutex::new(MouseMode::Auto),
            cursor: Mutex::new(CursorState::default()),
            saved_placement: Mutex::new(None),
            closing: AtomicBool::new(false),
            error: Mutex::new(None),
            events,
            fullscreen,
            title_stats,
            fullscreen_now: AtomicBool::new(false),
            shown: AtomicBool::new(false),
            active: AtomicBool::new(false),
            bar_visible: AtomicBool::new(false),
            edge_armed: AtomicBool::new(false),
        });
        *APP.lock().unwrap() = Some(app.clone());
        app
    }

    /// Fin de la session : la fenêtre n'a plus d'état.
    pub fn clear() {
        *APP.lock().unwrap() = None;
    }

    pub fn shown(&self) -> bool {
        self.shown.load(Ordering::Relaxed)
    }

    /// La barre de session peut s'ouvrir au pointeur : une session montrée
    /// (la ligne de commande n'a pas de barre), la souris du bureau (en souris
    /// des jeux, le pointeur est capturé), la barre pas déjà là.
    fn edge_ready(&self) -> bool {
        !self.title_stats && self.shown() && !self.relative() && !self.bar_visible.load(Ordering::Relaxed)
    }

    /// Le trait du bord haut se montre quand le geste est possible et que la
    /// fenêtre est au premier plan ; le fil de l'image le trace à sa prochaine
    /// image (ou dans les 100 ms si l'écran est fixe).
    fn refresh_hint(&self) {
        let on = self.edge_ready() && self.active.load(Ordering::Relaxed);
        let scale = if on { (dpi_scale(self.hwnd()) * 100.0).round() as u32 } else { 0 };
        self.shared.edge_hint.store(scale, Ordering::Relaxed);
    }

    pub fn hwnd(&self) -> HWND {
        HWND(self.hwnd.load(Ordering::Relaxed) as *mut _)
    }

    pub fn post(&self, msg: u32) {
        unsafe {
            let _ = PostMessageW(Some(self.hwnd()), msg, WPARAM(0), LPARAM(0));
        }
    }

    /// Chaîne d'entrées de l'image en cours.
    pub fn input(&self) -> Option<Arc<InputPipeline>> {
        self.input.lock().unwrap().clone()
    }

    /// Nouvelle chaîne d'entrées (début ou relance de l'image), au mode de
    /// souris en cours ; rend la précédente, à arrêter.
    pub fn attach_input(&self, input: Arc<InputPipeline>) -> Option<Arc<InputPipeline>> {
        let mut current = self.input.lock().unwrap();
        input.set_relative(self.relative());
        current.replace(input)
    }

    /// Image arrêtée : plus de chaîne d'entrées ; rend celle qui était là, à arrêter.
    pub fn detach_input(&self) -> Option<Arc<InputPipeline>> {
        self.input.lock().unwrap().take()
    }

    /// Souris relative (jeux) en ce moment.
    pub fn relative(&self) -> bool {
        self.relative.load(Ordering::Relaxed)
    }

    fn set_relative(&self, on: bool) {
        let current = self.input.lock().unwrap();
        self.relative.store(on, Ordering::Relaxed);
        if let Some(input) = current.as_ref() {
            input.set_relative(on);
        }
        self.refresh_hint();
    }

    /// « {nom} · KyberNow Link » ; en ligne de commande, suivi de l'étape ou
    /// des mesures (latence, définition, décodeur, images par seconde).
    fn title(&self) -> String {
        let name = format!("{} · {}", self.host_name, crate::shell::DISPLAY_NAME);
        if !self.title_stats {
            return name;
        }
        let status = self.status.lock().unwrap().clone();
        let detail = match status {
            None | Some(Status::Preparing) => t("window.preparing", &[]),
            // La Station redémarre en dernier recours : l'étape reste « Connexion ».
            Some(Status::Connecting | Status::StationRestarting) => t("window.connecting", &[]),
            Some(Status::FirstImages) => t("window.first_images", &[]),
            Some(Status::Streaming) => {
                let lat = self
                    .latency
                    .lock()
                    .unwrap()
                    .map(|l| {
                        let v = format!("{:.1} ms", l);
                        // Virgule décimale en français.
                        if crate::i18n::lang() == Lang::Fr { v.replace('.', ",") } else { v }
                    })
                    .unwrap_or_else(|| "— ms".into());
                let dec = self.shared.decoder.lock().unwrap().clone();
                let (w, h) = *self.host_size.lock().unwrap();
                let fps = self.fps.lock().unwrap().to_string();
                t(
                    "window.streaming",
                    &[("latency", &lat), ("width", &w.to_string()), ("height", &h.to_string()), ("decoder", &dec), ("fps", &fps)],
                )
            }
            Some(Status::Reconnecting { attempt, max }) => {
                t("window.reconnecting", &[("attempt", &attempt.to_string()), ("max", &max.to_string())])
            }
            Some(Status::Ending) => t("window.ending", &[]),
            Some(Status::Ended { .. }) => t("window.ended", &[]),
        };
        format!("{name} · {detail}")
    }
}

fn to_wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

pub fn create(title: &str, size: (i32, i32)) -> windows::core::Result<HWND> {
    unsafe {
        let instance = GetModuleHandleW(None)?;
        let class = w!("KyberNowSession");
        let wc = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(wndproc),
            hInstance: instance.into(),
            hCursor: LoadCursorW(None, IDC_ARROW)?,
            hbrBackground: HBRUSH(GetStockObject(BLACK_BRUSH).0),
            lpszClassName: class,
            ..Default::default()
        };
        RegisterClassExW(&wc);
        let mut rect = RECT { left: 0, top: 0, right: size.0, bottom: size.1 };
        let _ = AdjustWindowRectEx(&mut rect, WS_OVERLAPPEDWINDOW, false, WINDOW_EX_STYLE(0));
        let title = to_wide(title);
        let hwnd = CreateWindowExW(
            WINDOW_EX_STYLE(0),
            class,
            PCWSTR(title.as_ptr()),
            WS_OVERLAPPEDWINDOW,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            rect.right - rect.left,
            rect.bottom - rect.top,
            None,
            None,
            Some(instance.into()),
            None,
        )?;
        // Déplacements bruts de la souris (mode jeu).
        let rid = RAWINPUTDEVICE {
            usUsagePage: 0x01,
            usUsage: 0x02,
            dwFlags: RAWINPUTDEVICE_FLAGS(0),
            hwndTarget: hwnd,
        };
        let _ = RegisterRawInputDevices(&[rid], std::mem::size_of::<RAWINPUTDEVICE>() as u32);
        Ok(hwnd)
    }
}

/// Traite les messages en attente sans bloquer (pendant une attente du fil
/// principal).
pub fn pump_messages() {
    unsafe {
        let mut msg = MSG::default();
        while PeekMessageW(&mut msg, None, 0, 0, PM_REMOVE).as_bool() {
            if msg.message == WM_QUIT {
                PostQuitMessage(msg.wParam.0 as i32);
                return;
            }
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
}

pub fn run_loop() {
    unsafe {
        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
}

pub fn update_client_size(hwnd: HWND) {
    let mut r = RECT::default();
    unsafe {
        let _ = GetClientRect(hwnd, &mut r);
    }
    if let Some(app) = app() {
        app.shared.set_client_size((r.right - r.left) as u32, (r.bottom - r.top) as u32);
        if app.relative() {
            clip_cursor(hwnd, true);
        }
        // Un changement d'écran change l'échelle du trait du bord haut.
        app.refresh_hint();
    }
}

/// Met la fenêtre au premier plan. Windows le refuse à un fil qui n'a pas
/// reçu la dernière entrée (lancement par la barre des tâches, une
/// notification, un lien, un outil à distance) : le fil se rattache alors un
/// instant à celui de la fenêtre de premier plan, la manière admise de lui
/// passer la main. Sinon, la fenêtre clignote dans la barre des tâches.
fn bring_to_front(hwnd: HWND) -> bool {
    use windows::Win32::System::Threading::{AttachThreadInput, GetCurrentThreadId};
    unsafe {
        if SetForegroundWindow(hwnd).as_bool() && GetForegroundWindow() == hwnd {
            return true;
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
        if GetForegroundWindow() == hwnd {
            return true;
        }
        let flash = FLASHWINFO {
            cbSize: std::mem::size_of::<FLASHWINFO>() as u32,
            hwnd,
            dwFlags: FLASHW_ALL | FLASHW_TIMERNOFG,
            uCount: 0,
            dwTimeout: 0,
        };
        let _ = FlashWindowEx(&flash);
        false
    }
}

fn set_fullscreen(hwnd: HWND, on: bool) {
    let Some(app) = app() else { return };
    let mut saved = app.saved_placement.lock().unwrap();
    unsafe {
        if on && saved.is_none() {
            let mut wp = WINDOWPLACEMENT {
                length: std::mem::size_of::<WINDOWPLACEMENT>() as u32,
                ..Default::default()
            };
            let _ = GetWindowPlacement(hwnd, &mut wp);
            let style = GetWindowLongPtrW(hwnd, GWL_STYLE);
            *saved = Some((wp, style));
            let monitor = MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST);
            let mut mi = MONITORINFO {
                cbSize: std::mem::size_of::<MONITORINFO>() as u32,
                ..Default::default()
            };
            let _ = GetMonitorInfoW(monitor, &mut mi);
            SetWindowLongPtrW(hwnd, GWL_STYLE, (WS_POPUP | WS_VISIBLE).0 as isize);
            let r = mi.rcMonitor;
            let _ = SetWindowPos(
                hwnd,
                Some(HWND_TOP),
                r.left,
                r.top,
                r.right - r.left,
                r.bottom - r.top,
                SWP_NOOWNERZORDER | SWP_FRAMECHANGED,
            );
        } else if !on {
            if let Some((wp, style)) = saved.take() {
                SetWindowLongPtrW(hwnd, GWL_STYLE, style);
                let _ = SetWindowPlacement(hwnd, &wp);
                let _ = SetWindowPos(
                    hwnd,
                    None,
                    0,
                    0,
                    0,
                    0,
                    SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_NOOWNERZORDER | SWP_FRAMECHANGED,
                );
            }
        }
    }
    app.fullscreen_now.store(saved.is_some(), Ordering::Relaxed);
}

fn clip_cursor(hwnd: HWND, on: bool) {
    unsafe {
        if on {
            let mut r = RECT::default();
            let _ = GetClientRect(hwnd, &mut r);
            let mut tl = POINT { x: r.left, y: r.top };
            let mut br = POINT { x: r.right, y: r.bottom };
            let _ = ClientToScreen(hwnd, &mut tl);
            let _ = ClientToScreen(hwnd, &mut br);
            let _ = ClipCursor(Some(&RECT { left: tl.x, top: tl.y, right: br.x, bottom: br.y }));
        } else {
            let _ = ClipCursor(None);
        }
    }
}

/// Souris relative quand on la force, ou (Auto) quand un jeu est en plein
/// écran sur l'ordinateur et y cache son pointeur (règles du cœur : un
/// pointeur remontré moins de 0,6 s ne fait pas quitter le mode jeu, revu
/// 0,7 s après).
fn apply_mouse_mode(hwnd: HWND) {
    let Some(app) = app() else { return };
    let mode = *app.mouse_mode.lock().unwrap();
    let pointer = app.pointer.state();
    let visible_for = pointer.visible_for(app.clock.now());
    let current = app.relative();
    let relative = MOUSE_RULES.wants_relative(mode, app.game.load(Ordering::Relaxed), pointer.remote_hidden, visible_for, current);
    if mode == MouseMode::Auto && current && relative && !pointer.remote_hidden {
        // Pointeur remontré depuis peu : on reste en jeu, et on revoit à 0,7 s.
        let left = MouseModeRules::RECHECK_DELAY.saturating_sub(visible_for).max(Duration::from_millis(10));
        unsafe {
            SetTimer(Some(hwnd), TIMER_MOUSE, left.as_millis() as u32, None);
        }
    }
    let focused = unsafe { GetForegroundWindow() } == hwnd;
    if relative == current {
        return;
    }
    log::info!("souris : {}", if relative { "jeu (relative)" } else { "bureau" });
    app.set_relative(relative);
    if relative {
        edge_cancel(hwnd, &app);
    }
    clip_cursor(hwnd, relative && focused);
    unsafe {
        if relative {
            SetCursor(None);
        } else {
            let cur = app.cursor.lock().unwrap().current;
            SetCursor(if cur != 0 { Some(HCURSOR(cur as *mut _)) } else { LoadCursorW(None, IDC_ARROW).ok() });
        }
    }
    // Pointeur visible sur l'ordinateur (menus d'un jeu) : montré tout de suite.
    if relative {
        draw_game_pointer(&app, hwnd);
    }
}

/// Échelle de l'image : pixels de ce PC par pixel de l'ordinateur. Le
/// pointeur la suit, pour garder sa taille par rapport à l'image (bornée).
fn image_scale(app: &App) -> f32 {
    let host = *app.host_size.lock().unwrap();
    let r = video_rect(app.shared.client_size(), host);
    let s = (r.right - r.left) as f32 / host.0.max(1) as f32;
    if s.is_finite() { s.clamp(0.5, 3.0) } else { 1.0 }
}

/// Image du pointeur agrandie ou réduite (au plus proche : il reste net).
fn scaled(px: &[u32], w: u32, h: u32, scale: f32) -> (Vec<u32>, u32, u32) {
    if (scale - 1.0).abs() < 0.05 {
        return (px.to_vec(), w, h);
    }
    let (nw, nh) = (((w as f32 * scale).round() as u32).max(1), ((h as f32 * scale).round() as u32).max(1));
    let mut out = Vec::with_capacity((nw * nh) as usize);
    for y in 0..nh {
        let sy = ((y as f32 + 0.5) / scale) as u32;
        for x in 0..nw {
            let sx = ((x as f32 + 0.5) / scale) as u32;
            out.push(px[(sy.min(h - 1) * w + sx.min(w - 1)) as usize]);
        }
    }
    (out, nw, nh)
}

/// Construit un pointeur Windows depuis l'image ARVB de l'ordinateur (alpha
/// non prémultiplié, comme dans les pointeurs de Windows), à l'échelle de
/// l'image. Les formes vides ou entièrement transparentes sont déjà écartées
/// par le cœur.
fn make_cursor(shape: &Shape, scale: f32) -> Option<HCURSOR> {
    let (width, height) = (shape.width, shape.height);
    if width == 0 || height == 0 || shape.argb.len() < (width * height) as usize {
        return None;
    }
    let (px, w, h) = scaled(&shape.argb[..(width * height) as usize], width, height, scale);
    let (w, h) = (w as i32, h as i32);
    let k = w as f32 / width as f32;
    let (xhot, yhot) = (((shape.hot_x as f32) * k) as u32, ((shape.hot_y as f32) * k) as u32);
    unsafe {
        let bmi = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: w,
                biHeight: -h,
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };
        let hdc = GetDC(None);
        let mut bits = std::ptr::null_mut();
        let color = CreateDIBSection(Some(hdc), &bmi, DIB_RGB_COLORS, &mut bits, None, 0).ok();
        ReleaseDC(None, hdc);
        let color = color?;
        std::ptr::copy_nonoverlapping(px.as_ptr(), bits as *mut u32, (w * h) as usize);
        let mask = CreateBitmap(w, h, 1, 1, None);
        let info = ICONINFO {
            fIcon: false.into(),
            xHotspot: xhot,
            yHotspot: yhot,
            hbmMask: mask,
            hbmColor: color,
        };
        let icon = CreateIconIndirect(&info).ok();
        let _ = DeleteObject(color.into());
        let _ = DeleteObject(mask.into());
        icon.map(|i| HCURSOR(i.0))
    }
}

/// Le pointeur de l'ordinateur a changé (`PointerLayer`, fil de kynput) :
/// nouvelle forme ou nouvelle échelle de l'image, mode de la souris.
fn update_cursor(hwnd: HWND) {
    let Some(app) = app() else { return };
    let pointer = app.pointer.state();
    if let Some(shape) = &pointer.shape {
        let scale = image_scale(&app);
        let mut st = app.cursor.lock().unwrap();
        // Le cœur ne livre une forme neuve que si elle a changé : les mêmes
        // pixels sont le même tableau.
        let same = st.built.as_ref().is_some_and(|(px, s)| Arc::ptr_eq(px, &shape.argb) && (scale - s).abs() <= 0.01);
        if !same && let Some(new) = make_cursor(shape, scale) {
            st.built = Some((shape.argb.clone(), scale));
            let old = std::mem::replace(&mut st.current, new.0 as isize);
            unsafe {
                let mut p = POINT::default();
                let _ = GetCursorPos(&mut p);
                if WindowFromPoint(p) == hwnd && (!app.relative() || !pointer.remote_hidden) {
                    SetCursor(Some(new));
                }
                if old != 0 {
                    let _ = DestroyCursor(HCURSOR(old as *mut _));
                }
            }
        }
    }
    apply_mouse_mode(hwnd);
    draw_game_pointer(&app, hwnd);
}

/// Mode jeu (souris relative) avec un pointeur visible sur l'ordinateur
/// (menus d'un jeu) : le pointeur de ce PC le montre là où l'ordinateur
/// l'annonce. Sinon, caché.
fn draw_game_pointer(app: &App, hwnd: HWND) {
    if !app.relative() || unsafe { GetForegroundWindow() } != hwnd {
        return;
    }
    let pointer = app.pointer.state();
    let st = app.cursor.lock().unwrap();
    unsafe {
        let shown = pointer.remote_position.filter(|_| !pointer.remote_hidden && st.current != 0);
        let Some((x, y)) = shown else {
            SetCursor(None);
            return;
        };
        let host = *app.host_size.lock().unwrap();
        let r = video_rect(app.shared.client_size(), host);
        let (vw, vh) = (f64::from(r.right - r.left), f64::from(r.bottom - r.top));
        let mut p = POINT {
            x: r.left + (x * vw / f64::from(host.0.max(1))) as i32,
            y: r.top + (y * vh / f64::from(host.1.max(1))) as i32,
        };
        let _ = ClientToScreen(hwnd, &mut p);
        let _ = SetCursorPos(p.x, p.y);
        SetCursor(Some(HCURSOR(st.current as *mut _)));
    }
}

/// Ctrl+Alt+Maj+M, ou la barre : souris des jeux ou du bureau, forcée.
fn toggle_mouse(hwnd: HWND) {
    if let Some(a) = app() {
        *a.mouse_mode.lock().unwrap() = if a.relative() { MouseMode::Desktop } else { MouseMode::Game };
        apply_mouse_mode(hwnd);
    }
}

/// Menu de l'icône : Auto, souris du bureau ou des jeux.
fn set_mouse(hwnd: HWND, mode: MouseMode) {
    if let Some(a) = app() {
        *a.mouse_mode.lock().unwrap() = mode;
        apply_mouse_mode(hwnd);
    }
}

fn shortcut_held() -> bool {
    unsafe {
        GetKeyState(VK_CONTROL.0 as i32) < 0 && GetKeyState(VK_MENU.0 as i32) < 0 && GetKeyState(VK_SHIFT.0 as i32) < 0
    }
}

fn request_close(hwnd: HWND) {
    let Some(app) = app() else {
        unsafe { let _ = DestroyWindow(hwnd); }
        return;
    };
    if app.closing.swap(true, Ordering::Relaxed) {
        return;
    }
    match app.session.get() {
        Some(s) => s.stop(),
        None => unsafe {
            let _ = DestroyWindow(hwnd);
        },
    }
}

/// Le point (pixels de la zone cliente) est contre le bord haut de la fenêtre,
/// à 2 points au plus, dans la zone centrée de la largeur de la barre de
/// session : l'endroit que signale le trait.
fn in_edge_zone(hwnd: HWND, app: &App, x: i32, y: i32) -> bool {
    let scale = dpi_scale(hwnd);
    let width = app.shared.client_size().0 as i32;
    let half = ((BAR_WIDTH.load(Ordering::Relaxed) as f32 * scale / 2.0).round() as i32).min(width / 2);
    (0..=(EDGE_SLACK_POINTS * scale).round() as i32).contains(&y) && (x - width / 2).abs() <= half
}

/// Le pointeur est, en ce moment, sur la fenêtre de la session et dans la
/// zone du bord haut.
fn cursor_in_edge_zone(hwnd: HWND, app: &App) -> bool {
    unsafe {
        let mut p = POINT::default();
        if GetCursorPos(&mut p).is_err() || WindowFromPoint(p) != hwnd {
            return false;
        }
        let _ = ScreenToClient(hwnd, &mut p);
        in_edge_zone(hwnd, app, p.x, p.y)
    }
}

/// Le pointeur ne reste plus contre le bord haut : plus d'attente.
fn edge_cancel(hwnd: HWND, app: &App) {
    if app.edge_armed.swap(false, Ordering::Relaxed) {
        unsafe {
            let _ = KillTimer(Some(hwnd), TIMER_EDGE);
        }
    }
}

/// Mouvement du pointeur sur la session : arrivé dans la zone du bord haut
/// (fenêtre au premier plan, souris du bureau, aucun bouton enfoncé : un
/// glisser vers le haut ne l'ouvre pas), il lance une attente de 0,25 s ;
/// sorti de la zone, il l'annule. Un pointeur immobile n'envoie plus rien :
/// c'est la minuterie qui décide (`edge_fired`).
fn edge_pointer(hwnd: HWND, wparam: WPARAM, lparam: LPARAM) {
    let Some(app) = app() else { return };
    let x = (lparam.0 & 0xFFFF) as i16 as i32;
    let y = ((lparam.0 >> 16) & 0xFFFF) as i16 as i32;
    let on = wparam.0 & MK_BUTTONS == 0
        && app.edge_ready()
        && in_edge_zone(hwnd, &app, x, y)
        && unsafe { GetForegroundWindow() } == hwnd;
    if !on {
        edge_cancel(hwnd, &app);
    } else if !app.edge_armed.swap(true, Ordering::Relaxed) {
        // Les petits mouvements dans la zone ne relancent pas l'attente.
        unsafe {
            SetTimer(Some(hwnd), TIMER_EDGE, EDGE_DWELL_MS, None);
        }
    }
}

/// Un bouton de la souris est tenu (une fenêtre de l'ordinateur glissée contre
/// le bord haut, par exemple).
fn mouse_button_held() -> bool {
    [VK_LBUTTON, VK_RBUTTON, VK_MBUTTON, VK_XBUTTON1, VK_XBUTTON2]
        .iter()
        .any(|k| unsafe { GetAsyncKeyState(k.0 as i32) } < 0)
}

/// 0,25 s contre le bord haut : la barre s'ouvre si tout tient encore (le
/// pointeur dans la zone et sur la session, premier plan, souris du bureau,
/// aucun bouton tenu).
fn edge_fired(hwnd: HWND) {
    let Some(app) = app() else { return };
    edge_cancel(hwnd, &app);
    if app.edge_ready()
        && unsafe { GetForegroundWindow() } == hwnd
        && !mouse_button_held()
        && cursor_in_edge_zone(hwnd, &app)
    {
        app.events.top_edge(hwnd.0 as isize);
    }
}

fn mouse_position(hwnd: HWND, lparam: LPARAM) {
    let Some(app) = app() else { return };
    if app.relative() {
        return;
    }
    let Some(input) = app.input() else { return };
    let x = (lparam.0 & 0xFFFF) as i16 as i32;
    let y = ((lparam.0 >> 16) & 0xFFFF) as i16 as i32;
    let host = *app.host_size.lock().unwrap();
    let r = video_rect(app.shared.client_size(), host);
    let (vw, vh) = ((r.right - r.left).max(1), (r.bottom - r.top).max(1));
    let hx = ((x - r.left) as i64 * host.0 as i64 / vw as i64).clamp(0, host.0 as i64 - 1);
    let hy = ((y - r.top) as i64 * host.1 as i64 / vh as i64).clamp(0, host.1 as i64 - 1);
    input.mouse(hx.min(i16::MAX as i64) as i16, hy.min(i16::MAX as i64) as i16);
    let _ = hwnd;
}

fn mouse_button(hwnd: HWND, button: MouseButtonType, pressed: bool) {
    let Some(app) = app() else { return };
    unsafe {
        if pressed {
            SetCapture(hwnd);
        } else {
            let _ = ReleaseCapture();
        }
    }
    let Some(input) = app.input() else { return };
    // Relâchement d'un bouton dont l'appui n'est pas parti : rien.
    if !pressed && !input.held().lock().unwrap().mouse_buttons().contains(&button) {
        return;
    }
    input.mouse_button(button, pressed);
}

fn raw_input(lparam: LPARAM) {
    let Some(app) = app() else { return };
    if !app.relative() {
        return;
    }
    let Some(input) = app.input() else { return };
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
            input.mouse_move(m.lLastX.clamp(-32768, 32767) as i16, m.lLastY.clamp(-32768, 32767) as i16);
        }
    }
}

unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    unsafe {
        match msg {
            WM_SIZE => {
                update_client_size(hwnd);
                LRESULT(0)
            }
            WM_ERASEBKGND => LRESULT(1),
            WM_KEYDOWN | WM_SYSKEYDOWN | WM_KEYUP | WM_SYSKEYUP => {
                let pressed = msg == WM_KEYDOWN || msg == WM_SYSKEYDOWN;
                let repeat = (lparam.0 >> 30) & 1 == 1;
                if pressed && shortcut_held() {
                    let handled = match wparam.0 as u8 {
                        b'D' => {
                            request_close(hwnd);
                            true
                        }
                        b'F' => {
                            if !repeat {
                                let full = app().is_some_and(|a| a.saved_placement.lock().unwrap().is_some());
                                set_fullscreen(hwnd, !full);
                            }
                            true
                        }
                        b'M' => {
                            if !repeat {
                                toggle_mouse(hwnd);
                            }
                            true
                        }
                        _ => false,
                    };
                    if handled {
                        return LRESULT(0);
                    }
                }
                if pressed && repeat {
                    // L'ordinateur répète lui-même les touches tenues.
                    return LRESULT(0);
                }
                if let Some(a) = app() {
                    let sc = ((lparam.0 >> 16) & 0xFF) as u16;
                    let extended = (lparam.0 >> 24) & 1 == 1;
                    let code = if extended { 0xE000 | sc } else { sc };
                    if sc != 0 {
                        send_key(&a, code, pressed);
                    }
                }
                // Pas de menu système sur Alt ni F10 : tout part à l'ordinateur.
                LRESULT(0)
            }
            WM_SYSCHAR | WM_CHAR => LRESULT(0),
            WM_MOUSEMOVE => {
                mouse_position(hwnd, lparam);
                edge_pointer(hwnd, wparam, lparam);
                LRESULT(0)
            }
            WM_ACTIVATE => {
                if let Some(a) = app() {
                    // Active, et pas réduite : le trait du bord haut se montre.
                    let active = wparam.0 & 0xFFFF != WA_INACTIVE as usize && (wparam.0 >> 16) & 0xFFFF == 0;
                    a.active.store(active, Ordering::Relaxed);
                    if !active {
                        edge_cancel(hwnd, &a);
                    }
                    a.refresh_hint();
                }
                DefWindowProcW(hwnd, msg, wparam, lparam)
            }
            WM_APP_FIRST_IMAGE => {
                if let Some(a) = app() {
                    if !a.shown.swap(true, Ordering::Relaxed) {
                        let _ = ShowWindow(hwnd, SW_SHOW);
                        if a.fullscreen {
                            set_fullscreen(hwnd, true);
                        }
                        update_client_size(hwnd);
                        let front = bring_to_front(hwnd);
                        if !front {
                            log::warn!("Windows refuse le premier plan à la session : elle attend dans la barre des tâches");
                        }
                        a.refresh_hint();
                        // L'accueil se cache dans tous les cas : laissé devant, il
                        // montrait le passage ouvert sur du noir.
                        a.events.visible(front);
                    }
                }
                LRESULT(0)
            }
            WM_APP_COMMAND => {
                match wparam.0 {
                    x if x == Command::Disconnect as usize => request_close(hwnd),
                    x if x == Command::Fullscreen as usize => {
                        let full = app().is_some_and(|a| a.saved_placement.lock().unwrap().is_some());
                        set_fullscreen(hwnd, !full);
                    }
                    x if x == Command::MouseToggle as usize => toggle_mouse(hwnd),
                    x if x == Command::MouseAuto as usize => set_mouse(hwnd, MouseMode::Auto),
                    x if x == Command::MouseDesktop as usize => set_mouse(hwnd, MouseMode::Desktop),
                    x if x == Command::MouseGame as usize => set_mouse(hwnd, MouseMode::Game),
                    x if x == Command::Focus as usize => {
                        let _ = SetForegroundWindow(hwnd);
                    }
                    x if x == Command::CtrlAltDel as usize => {
                        if let Some(a) = app() {
                            send_combo(&a, &[sc::CTRL, sc::ALT, sc::DEL]);
                        }
                    }
                    x if x == Command::WinKey as usize => {
                        if let Some(a) = app() {
                            send_combo(&a, &[sc::LWIN]);
                        }
                    }
                    x if x == Command::AltTab as usize => {
                        if let Some(a) = app() {
                            send_combo(&a, &[sc::ALT, sc::TAB]);
                        }
                    }
                    x if x == Command::PrintScreen as usize => {
                        if let Some(a) = app() {
                            send_combo(&a, &[sc::PRINT]);
                        }
                    }
                    _ => {}
                }
                LRESULT(0)
            }
            WM_LBUTTONDOWN | WM_LBUTTONUP => {
                mouse_button(hwnd, MouseButtonType::Left, msg == WM_LBUTTONDOWN);
                LRESULT(0)
            }
            WM_RBUTTONDOWN | WM_RBUTTONUP => {
                mouse_button(hwnd, MouseButtonType::Right, msg == WM_RBUTTONDOWN);
                LRESULT(0)
            }
            WM_MBUTTONDOWN | WM_MBUTTONUP => {
                mouse_button(hwnd, MouseButtonType::Middle, msg == WM_MBUTTONDOWN);
                LRESULT(0)
            }
            WM_XBUTTONDOWN | WM_XBUTTONUP => {
                let b = if (wparam.0 >> 16) & 0xFFFF == 1 { MouseButtonType::Side } else { MouseButtonType::Extra };
                mouse_button(hwnd, b, msg == WM_XBUTTONDOWN);
                LRESULT(1)
            }
            WM_MOUSEWHEEL | WM_MOUSEHWHEEL => {
                if let Some(input) = app().and_then(|a| a.input()) {
                    let delta = ((wparam.0 >> 16) & 0xFFFF) as i16 as f32 / 120.0;
                    if msg == WM_MOUSEWHEEL {
                        input.wheel(0.0, -delta);
                    } else {
                        // Comme le client de Kyber : les deux axes inversés.
                        input.wheel(-delta, 0.0);
                    }
                }
                LRESULT(0)
            }
            WM_INPUT => {
                raw_input(lparam);
                DefWindowProcW(hwnd, msg, wparam, lparam)
            }
            WM_SETCURSOR => {
                if (lparam.0 & 0xFFFF) as u32 == HTCLIENT {
                    if let Some(a) = app() {
                        if a.relative() && a.pointer.state().remote_hidden {
                            SetCursor(None);
                        } else {
                            let cur = a.cursor.lock().unwrap().current;
                            SetCursor(if cur != 0 { Some(HCURSOR(cur as *mut _)) } else { LoadCursorW(None, IDC_ARROW).ok() });
                        }
                        return LRESULT(1);
                    }
                }
                DefWindowProcW(hwnd, msg, wparam, lparam)
            }
            WM_KILLFOCUS => {
                // Rien ne reste enfoncé sur l'ordinateur quand on change d'app.
                if let Some(input) = app().and_then(|a| a.input()) {
                    input.release_all();
                }
                clip_cursor(hwnd, false);
                LRESULT(0)
            }
            WM_SETFOCUS => {
                if app().is_some_and(|a| a.relative()) {
                    clip_cursor(hwnd, true);
                }
                LRESULT(0)
            }
            WM_APP_CURSOR => {
                update_cursor(hwnd);
                LRESULT(0)
            }
            WM_APP_MOUSE => {
                apply_mouse_mode(hwnd);
                LRESULT(0)
            }
            WM_TIMER if wparam.0 == TIMER_EDGE => {
                edge_fired(hwnd);
                LRESULT(0)
            }
            WM_TIMER if wparam.0 == TIMER_MOUSE => {
                let _ = KillTimer(Some(hwnd), TIMER_MOUSE);
                apply_mouse_mode(hwnd);
                LRESULT(0)
            }
            WM_APP_STATUS => {
                if let Some(a) = app() {
                    let title = to_wide(&a.title());
                    let _ = SetWindowTextW(hwnd, PCWSTR(title.as_ptr()));
                    if matches!(*a.status.lock().unwrap(), Some(Status::Ended { .. })) {
                        let _ = DestroyWindow(hwnd);
                    }
                }
                LRESULT(0)
            }
            WM_CLOSE => {
                request_close(hwnd);
                LRESULT(0)
            }
            WM_DESTROY => {
                clip_cursor(hwnd, false);
                PostQuitMessage(0);
                LRESULT(0)
            }
            _ => DefWindowProcW(hwnd, msg, wparam, lparam),
        }
    }
}
