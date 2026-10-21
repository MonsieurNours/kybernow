//! Fenêtre de session Win32 : plein écran sans bordure, clavier et souris
//! vers l'ordinateur, pointeur de l'ordinateur, raccourcis de l'app
//! (Ctrl+Alt+Maj + lettre, comme ⌃⌥⌘ sur Mac).
//!
//! | Ctrl+Alt+Maj+D | se déconnecter |
//! | Ctrl+Alt+Maj+F | plein écran ou fenêtre |
//! | Ctrl+Alt+Maj+M | souris des jeux (relative) ou du bureau |
//!
//! La fenêtre vit sur le fil de sa session (voir `host.rs`) : l'accueil de
//! l'app garde le fil principal. Elle reste cachée jusqu'à la première image.

use std::sync::atomic::{AtomicBool, AtomicIsize, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Instant;

use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Input::KeyboardAndMouse::*;
use windows::Win32::UI::Input::*;
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::{PCWSTR, w};

use kybernow_core::kynput_rs::{Cursor, CursorData, MouseButtonType};
use kybernow_core::session::{Session, Status};

use crate::input::Input;
use crate::render::video_rect;
use crate::video::Shared;

pub const WM_APP_STATUS: u32 = WM_APP + 1;
pub const WM_APP_CURSOR: u32 = WM_APP + 2;
pub const WM_APP_MOUSE: u32 = WM_APP + 3;
/// Première image présentée : la fenêtre se montre.
pub const WM_APP_FIRST_IMAGE: u32 = WM_APP + 4;
/// Ordre de l'app (barre de session) : `wParam` = `Command`.
pub const WM_APP_COMMAND: u32 = WM_APP + 5;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(usize)]
pub enum Command {
    Disconnect = 1,
    Fullscreen = 2,
    MouseToggle = 3,
    Focus = 4,
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
        app.input.key(k, true);
    }
    for &k in keys.iter().rev() {
        app.input.key(k, false);
    }
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
            app.input.key(code, false);
            return true;
        }
        return false;
    }
    let fullscreen = app.saved_placement.lock().unwrap().is_some();
    if !fullscreen || unsafe { GetForegroundWindow() } != app.hwnd() {
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
    if !app.input.is_held(code) {
        app.input.key(code, true);
    }
    true
}

/// Session en cours, pour l'accueil (changement de profil).
pub fn current_session() -> Option<Arc<Session>> {
    app().and_then(|a| a.session.get().cloned())
}

#[derive(Default)]
struct CursorState {
    pending: Option<Cursor>,
    current: isize,
    host_visible: bool,
    /// Forme du pointeur en place et l'échelle à laquelle il a été construit :
    /// les paquets de position (jusqu'à 120 par seconde) ne le refont pas.
    shape: Option<(u32, u32, u32, u32, Vec<u32>)>,
    scale: f32,
    /// Dernière position annoncée par l'ordinateur (pixels de son écran).
    pos: (i32, i32),
}

pub struct App {
    pub hwnd: AtomicIsize,
    pub host_name: String,
    pub input: Arc<Input>,
    pub shared: Arc<Shared>,
    pub session: OnceLock<Arc<Session>>,
    pub host_size: Mutex<(u32, u32)>,
    pub status: Mutex<Option<Status>>,
    pub latency: Mutex<Option<f64>>,
    pub fps: Mutex<u64>,
    /// Un jeu en plein écran sur l'ordinateur (console).
    pub game: AtomicBool,
    /// Choix forcé par Ctrl+Alt+Maj+M ; `None` : automatique.
    pub mouse_choice: Mutex<Option<bool>>,
    cursor: Mutex<CursorState>,
    saved_placement: Mutex<Option<(WINDOWPLACEMENT, isize)>>,
    pub closing: AtomicBool,
    pub error: Mutex<Option<String>>,
    pub events: Arc<dyn crate::host::HostEvents>,
    /// Plein écran quand la fenêtre se montre.
    pub fullscreen: bool,
    shown: AtomicBool,
    last_edge: Mutex<Option<Instant>>,
}

/// La session en cours (une à la fois).
static APP: Mutex<Option<Arc<App>>> = Mutex::new(None);

fn app() -> Option<Arc<App>> {
    APP.lock().unwrap().clone()
}

impl App {
    pub fn new(
        host_name: String,
        input: Arc<Input>,
        shared: Arc<Shared>,
        events: Arc<dyn crate::host::HostEvents>,
        fullscreen: bool,
    ) -> Arc<Self> {
        let app = Arc::new(Self {
            hwnd: AtomicIsize::new(0),
            host_name,
            input,
            shared,
            session: OnceLock::new(),
            host_size: Mutex::new((1920, 1080)),
            status: Mutex::new(None),
            latency: Mutex::new(None),
            fps: Mutex::new(0),
            game: AtomicBool::new(false),
            mouse_choice: Mutex::new(None),
            cursor: Mutex::new(CursorState { host_visible: true, ..Default::default() }),
            saved_placement: Mutex::new(None),
            closing: AtomicBool::new(false),
            error: Mutex::new(None),
            events,
            fullscreen,
            shown: AtomicBool::new(false),
            last_edge: Mutex::new(None),
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

    pub fn hwnd(&self) -> HWND {
        HWND(self.hwnd.load(Ordering::Relaxed) as *mut _)
    }

    pub fn post(&self, msg: u32) {
        unsafe {
            let _ = PostMessageW(Some(self.hwnd()), msg, WPARAM(0), LPARAM(0));
        }
    }

    /// Forme du pointeur annoncée par l'ordinateur (fil de kynput).
    pub fn cursor_from_host(&self, c: Cursor) {
        self.cursor.lock().unwrap().pending = Some(c);
        self.post(WM_APP_CURSOR);
    }

    fn title(&self) -> String {
        let status = self.status.lock().unwrap().clone();
        let detail = match status {
            None | Some(Status::Preparing) => "préparation de l'écran".to_string(),
            Some(Status::Connecting) => "connexion".to_string(),
            Some(Status::FirstImages) => "premières images".to_string(),
            Some(Status::Streaming) => {
                let lat = self
                    .latency
                    .lock()
                    .unwrap()
                    .map(|l| format!("{:.1} ms", l).replace('.', ","))
                    .unwrap_or_else(|| "— ms".into());
                let dec = self.shared.decoder.lock().unwrap().clone();
                let (w, h) = *self.host_size.lock().unwrap();
                format!("{lat} · {w} × {h} · {dec} · {} i/s", self.fps.lock().unwrap())
            }
            Some(Status::Reconnecting { attempt, max }) => format!("reconnexion ({attempt}/{max})"),
            Some(Status::Ending) => "fin de la session".to_string(),
            Some(Status::Ended { .. }) => "session terminée".to_string(),
        };
        format!("KyberNow — {} · {detail}", self.host_name)
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
        if app.input.relative() {
            clip_cursor(hwnd, true);
        }
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
/// écran sur l'ordinateur et y cache son pointeur.
fn apply_mouse_mode(hwnd: HWND) {
    let Some(app) = app() else { return };
    let auto = app.game.load(Ordering::Relaxed) && !app.cursor.lock().unwrap().host_visible;
    let relative = app.mouse_choice.lock().unwrap().unwrap_or(auto);
    let focused = unsafe { GetForegroundWindow() } == hwnd;
    if relative == app.input.relative() {
        return;
    }
    log::info!("souris : {}", if relative { "jeu (relative)" } else { "bureau" });
    app.input.set_relative(relative);
    clip_cursor(hwnd, relative && focused);
    unsafe {
        if relative {
            SetCursor(None);
        } else {
            let cur = app.cursor.lock().unwrap().current;
            SetCursor(if cur != 0 { Some(HCURSOR(cur as *mut _)) } else { LoadCursorW(None, IDC_ARROW).ok() });
        }
    }    // Pointeur visible sur l'ordinateur (menus d'un jeu) : montré tout de suite.
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

/// Construit un pointeur Windows depuis l'image ARVB de l'ordinateur, à l'échelle de l'image.
fn make_cursor(c: &Cursor, scale: f32) -> Option<HCURSOR> {
    let CursorData::Raw(px) = &c.data else { return None };
    if c.width == 0 || c.height == 0 || px.len() < (c.width * c.height) as usize {
        return None;
    }
    // Forme entièrement transparente (lecteurs vidéo) : on garde la précédente.
    if px.iter().all(|p| p >> 24 == 0) {
        return None;
    }
    let (px, w, h) = scaled(&px[..(c.width * c.height) as usize], c.width, c.height, scale);
    let (w, h) = (w as i32, h as i32);
    let k = w as f32 / c.width as f32;
    let (xhot, yhot) = (((c.xhot as f32) * k) as u32, ((c.yhot as f32) * k) as u32);
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

fn update_cursor(hwnd: HWND) {
    let Some(app) = app() else { return };
    let mut st = app.cursor.lock().unwrap();
    let Some(c) = st.pending.take() else { return };
    st.host_visible = c.visible;
    st.pos = (c.x, c.y);
    let scale = image_scale(&app);
    let shape = match &c.data {
        CursorData::Raw(px) => Some((c.width, c.height, c.xhot, c.yhot, px.clone())),
        _ => None,
    };
    let changed = shape.is_some() && (shape != st.shape || (scale - st.scale).abs() > 0.01);
    if changed {
        if let Some(new) = make_cursor(&c, scale) {
            st.shape = shape;
            st.scale = scale;
            let old = std::mem::replace(&mut st.current, new.0 as isize);
            unsafe {
                let mut p = POINT::default();
                let _ = GetCursorPos(&mut p);
                if WindowFromPoint(p) == hwnd && (!app.input.relative() || c.visible) {
                    SetCursor(Some(new));
                }
                if old != 0 {
                    let _ = DestroyCursor(HCURSOR(old as *mut _));
                }
            }
        }
    }
    drop(st);
    apply_mouse_mode(hwnd);
    draw_game_pointer(&app, hwnd);
}

/// Mode jeu (souris relative) avec un pointeur visible sur l'ordinateur
/// (menus d'un jeu) : le pointeur de ce PC le montre là où l'ordinateur
/// l'annonce. Sinon, caché.
fn draw_game_pointer(app: &App, hwnd: HWND) {
    if !app.input.relative() || unsafe { GetForegroundWindow() } != hwnd {
        return;
    }
    let st = app.cursor.lock().unwrap();
    unsafe {
        if !st.host_visible || st.current == 0 {
            SetCursor(None);
            return;
        }
        let host = *app.host_size.lock().unwrap();
        let r = video_rect(app.shared.client_size(), host);
        let (vw, vh) = ((r.right - r.left) as i64, (r.bottom - r.top) as i64);
        let mut p = POINT {
            x: r.left + (st.pos.0 as i64 * vw / host.0.max(1) as i64) as i32,
            y: r.top + (st.pos.1 as i64 * vh / host.1.max(1) as i64) as i32,
        };
        let _ = ClientToScreen(hwnd, &mut p);
        let _ = SetCursorPos(p.x, p.y);
        SetCursor(Some(HCURSOR(st.current as *mut _)));
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

fn mouse_position(hwnd: HWND, lparam: LPARAM) {
    let Some(app) = app() else { return };
    if app.input.relative() {
        return;
    }
    let x = (lparam.0 & 0xFFFF) as i16 as i32;
    let y = ((lparam.0 >> 16) & 0xFFFF) as i16 as i32;
    let host = *app.host_size.lock().unwrap();
    let r = video_rect(app.shared.client_size(), host);
    let (vw, vh) = ((r.right - r.left).max(1), (r.bottom - r.top).max(1));
    let hx = ((x - r.left) as i64 * host.0 as i64 / vw as i64).clamp(0, host.0 as i64 - 1);
    let hy = ((y - r.top) as i64 * host.1 as i64 / vh as i64).clamp(0, host.1 as i64 - 1);
    app.input.position(hx.min(i16::MAX as i64) as i16, hy.min(i16::MAX as i64) as i16);
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
    app.input.button(button, pressed);
}

fn raw_input(lparam: LPARAM) {
    let Some(app) = app() else { return };
    if !app.input.relative() {
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
        if (m.usFlags.0 & MOUSE_MOVE_ABSOLUTE.0) == 0 {
            app.input.motion(m.lLastX.clamp(-32768, 32767) as i16, m.lLastY.clamp(-32768, 32767) as i16);
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
                                if let Some(a) = app() {
                                    let now = a.input.relative();
                                    *a.mouse_choice.lock().unwrap() = Some(!now);
                                    apply_mouse_mode(hwnd);
                                }
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
                        a.input.key(code, pressed);
                    }
                }
                // Pas de menu système sur Alt ni F10 : tout part à l'ordinateur.
                LRESULT(0)
            }
            WM_SYSCHAR | WM_CHAR => LRESULT(0),
            WM_MOUSEMOVE => {
                mouse_position(hwnd, lparam);
                // Pointeur contre le haut de l'écran : la barre de session.
                let y = ((lparam.0 >> 16) & 0xFFFF) as i16;
                if y <= 1 {
                    // Pas en souris des jeux : regarder vers le haut y plaque le pointeur.
                    if let Some(a) = app().filter(|a| !a.input.relative()) {
                        let mut last = a.last_edge.lock().unwrap();
                        if a.shown() && last.is_none_or(|t| t.elapsed().as_millis() > 400) {
                            *last = Some(Instant::now());
                            drop(last);
                            a.events.top_edge(hwnd.0 as isize);
                        }
                    }
                }
                LRESULT(0)
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
                    x if x == Command::MouseToggle as usize => {
                        if let Some(a) = app() {
                            let now = a.input.relative();
                            *a.mouse_choice.lock().unwrap() = Some(!now);
                            apply_mouse_mode(hwnd);
                        }
                    }
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
                if let Some(a) = app() {
                    let delta = ((wparam.0 >> 16) & 0xFFFF) as i16 as f32 / 120.0;
                    if msg == WM_MOUSEWHEEL {
                        a.input.wheel(0.0, -delta);
                    } else {
                        // Comme le client de Kyber : les deux axes inversés.
                        a.input.wheel(-delta, 0.0);
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
                        let st = a.cursor.lock().unwrap();
                        if a.input.relative() && !st.host_visible {
                            SetCursor(None);
                        } else {
                            drop(st);
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
                if let Some(a) = app() {
                    a.input.release_all();
                }
                clip_cursor(hwnd, false);
                LRESULT(0)
            }
            WM_SETFOCUS => {
                if app().is_some_and(|a| a.input.relative()) {
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
