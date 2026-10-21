//! Fenêtre de session Win32 : plein écran sans bordure, clavier et souris
//! vers l'ordinateur, pointeur de l'ordinateur, raccourcis de l'app
//! (Ctrl+Alt+Maj + lettre, comme ⌃⌥⌘ sur Mac).
//!
//! | Ctrl+Alt+Maj+D | se déconnecter |
//! | Ctrl+Alt+Maj+F | plein écran ou fenêtre |
//! | Ctrl+Alt+Maj+M | souris des jeux (relative) ou du bureau |

use std::sync::atomic::{AtomicBool, AtomicIsize, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

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

#[derive(Default)]
struct CursorState {
    pending: Option<Cursor>,
    current: isize,
    host_visible: bool,
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
}

static APP: OnceLock<Arc<App>> = OnceLock::new();

fn app() -> Option<&'static Arc<App>> {
    APP.get()
}

impl App {
    pub fn new(host_name: String, input: Arc<Input>, shared: Arc<Shared>) -> Arc<Self> {
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
        });
        let _ = APP.set(app.clone());
        app
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

pub fn show(hwnd: HWND, fullscreen: bool) {
    unsafe {
        let _ = ShowWindow(hwnd, SW_SHOW);
        let _ = UpdateWindow(hwnd);
    }
    if fullscreen {
        set_fullscreen(hwnd, true);
    }
    update_client_size(hwnd);
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

fn update_client_size(hwnd: HWND) {
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
    }
}

/// Construit un pointeur Windows depuis l'image ARVB de l'ordinateur.
fn make_cursor(c: &Cursor) -> Option<HCURSOR> {
    let CursorData::Raw(px) = &c.data else { return None };
    let (w, h) = (c.width as i32, c.height as i32);
    if w <= 0 || h <= 0 || px.len() < (w * h) as usize {
        return None;
    }
    // Forme entièrement transparente (lecteurs vidéo) : on garde la précédente.
    if px.iter().all(|p| p >> 24 == 0) {
        return None;
    }
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
            xHotspot: c.xhot,
            yHotspot: c.yhot,
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
    if let Some(new) = make_cursor(&c) {
        let old = std::mem::replace(&mut st.current, new.0 as isize);
        unsafe {
            if !app.input.relative() {
                let mut p = POINT::default();
                let _ = GetCursorPos(&mut p);
                if WindowFromPoint(p) == hwnd {
                    SetCursor(Some(new));
                }
            }
            if old != 0 {
                let _ = DestroyCursor(HCURSOR(old as *mut _));
            }
        }
    }
    drop(st);
    apply_mouse_mode(hwnd);
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
                        if a.input.relative() {
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
