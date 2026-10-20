//! Panneau de l'icône (clic gauche) : une petite fenêtre sans bordure avec une
//! vue web (WebView2) qui affiche `panel.html`, posée contre l'icône. Elle se
//! ferme dès qu'elle perd le focus, comme les panneaux de Windows.
//!
//! L'icône pousse son état au panneau (`window.__knState`) ; le panneau envoie
//! ses actions par `window.ipc.postMessage`.

use std::borrow::Cow;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use tao::dpi::{LogicalSize, PhysicalPosition};
use tao::event_loop::{EventLoopProxy, EventLoopWindowTarget};
use tao::platform::windows::{WindowBuilderExtWindows, WindowExtWindows};
use tao::window::{Window, WindowBuilder, WindowId};
use windows_sys::Win32::Foundation::{POINT, RECT};
use windows_sys::Win32::Graphics::Dwm::{DwmExtendFrameIntoClientArea, DwmSetWindowAttribute};
use windows_sys::Win32::UI::Controls::MARGINS;
use windows_sys::Win32::Graphics::Gdi::{GetMonitorInfoW, MONITOR_DEFAULTTONEAREST, MONITORINFO, MonitorFromPoint};
use wry::http::{Request, Response, header::CONTENT_TYPE};
use wry::{WebContext, WebView, WebViewBuilder};

use crate::app::UserEvent;

const HTML: &str = include_str!("panel.html");
const ARCHIVO: &[u8] = include_bytes!("../../../brand/v2/fonts/web/Archivo-wdth-wght.woff2");
const MARTIAN: &[u8] = include_bytes!("../../../brand/v2/fonts/web/MartianMono-wdth-wght.woff2");

/// Largeur du panneau, en points ; la hauteur suit son contenu.
const WIDTH: f64 = 340.0;
/// Écart avec la barre des tâches et les bords de l'écran, en points.
const MARGIN: f64 = 12.0;

pub struct Panel {
    window: Window,
    webview: WebView,
    height: f64,
    /// Fermé à l'instant par une perte de focus : le clic sur l'icône qui l'a
    /// provoquée ne doit pas le rouvrir aussitôt.
    hidden_at: Instant,
    /// Dernier emplacement de l'icône, pour replacer le panneau quand sa hauteur change.
    anchor: Option<(PhysicalPosition<f64>, (u32, u32))>,
    // La vue web garde son dossier de données tant qu'elle vit.
    _context: WebContext,
}

fn serve(request: Request<Vec<u8>>) -> Response<Cow<'static, [u8]>> {
    let (body, kind): (&'static [u8], &str) = match request.uri().path() {
        "/fonts/archivo.woff2" => (ARCHIVO, "font/woff2"),
        "/fonts/martian.woff2" => (MARTIAN, "font/woff2"),
        _ => (HTML.as_bytes(), "text/html; charset=utf-8"),
    };
    Response::builder().header(CONTENT_TYPE, kind).body(Cow::Borrowed(body)).unwrap()
}

impl Panel {
    pub fn new(target: &EventLoopWindowTarget<UserEvent>, proxy: EventLoopProxy<UserEvent>) -> Result<Self> {
        let window = WindowBuilder::new()
            .with_title("KyberNow")
            .with_inner_size(LogicalSize::new(WIDTH, 440.0))
            .with_resizable(false)
            .with_decorations(false)
            .with_always_on_top(true)
            .with_skip_taskbar(true)
            // L'ombre de tao réserve autour de la fenêtre une bande de cadre
            // que Windows peint en clair : on la fait nous-mêmes, sans bande.
            .with_undecorated_shadow(false)
            .with_visible(false)
            .build(target)
            .context("fenêtre du panneau")?;
        frame(window.hwnd() as _);

        let data = std::env::var_os("LOCALAPPDATA")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(std::env::temp_dir)
            .join(r"KyberNow\tray-webview");
        let mut context = WebContext::new(Some(data));
        let proxy = std::sync::Mutex::new(proxy);
        let webview = WebViewBuilder::new_with_web_context(&mut context)
            .with_custom_protocol("kn".into(), |_id, request| serve(request))
            .with_ipc_handler(move |req: Request<String>| {
                let _ = proxy.lock().unwrap().send_event(UserEvent::Panel(req.body().clone()));
            })
            .with_background_color((12, 16, 22, 255))
            .with_devtools(cfg!(debug_assertions))
            .with_url("http://kn.localhost/")
            .build(&window)
            .context("vue web du panneau (WebView2)")?;
        Ok(Panel { window, webview, height: 440.0, hidden_at: Instant::now() - Duration::from_secs(1), anchor: None, _context: context })
    }

    pub fn id(&self) -> WindowId {
        self.window.id()
    }

    pub fn is_visible(&self) -> bool {
        self.window.is_visible()
    }

    /// Clic sur l'icône : ouvre le panneau contre elle, ou le ferme.
    pub fn toggle(&mut self, icon: PhysicalPosition<f64>, icon_size: (u32, u32)) -> bool {
        if self.is_visible() {
            self.hide();
            return false;
        }
        if self.hidden_at.elapsed() < Duration::from_millis(300) {
            return false;
        }
        self.anchor = Some((icon, icon_size));
        self.place();
        self.window.set_visible(true);
        self.window.set_focus();
        let _ = self.webview.evaluate_script("window.__knShown && window.__knShown()");
        true
    }

    pub fn hide(&mut self) {
        if self.window.is_visible() {
            self.window.set_visible(false);
            self.hidden_at = Instant::now();
        }
    }

    /// Le contenu a changé de hauteur.
    pub fn resize(&mut self, height: f64) {
        let height = height.clamp(200.0, 760.0);
        if (height - self.height).abs() < 1.0 {
            return;
        }
        self.height = height;
        self.window.set_inner_size(LogicalSize::new(WIDTH, height));
        let _ = self.webview.set_bounds(wry::Rect {
            position: tao::dpi::LogicalPosition::new(0.0, 0.0).into(),
            size: LogicalSize::new(WIDTH, height).into(),
        });
        if self.window.is_visible() {
            self.place();
        }
    }

    pub fn push(&self, state_json: &str) {
        let _ = self.webview.evaluate_script(&format!("window.__knState && window.__knState({state_json})"));
    }

    /// Contre l'icône, du côté de la barre des tâches, dans la zone de travail de l'écran.
    fn place(&self) {
        let Some((icon, (iw, ih))) = self.anchor else { return };
        let scale = self.window.scale_factor();
        let (w, h, m) = (WIDTH * scale, self.height * scale, MARGIN * scale);
        let center = (icon.x + iw as f64 / 2.0, icon.y + ih as f64 / 2.0);
        let work = work_area(center.0 as i32, center.1 as i32);
        let (left, top, right, bottom) = (work.left as f64, work.top as f64, work.right as f64, work.bottom as f64);
        let clamp_x = |x: f64| x.clamp(left + m, (right - w - m).max(left + m));
        let clamp_y = |y: f64| y.clamp(top + m, (bottom - h - m).max(top + m));
        // L'icône est hors de la zone de travail : c'est là qu'est la barre des tâches.
        let (x, y) = if center.1 >= bottom {
            (clamp_x(center.0 - w / 2.0), bottom - h - m)
        } else if center.1 <= top {
            (clamp_x(center.0 - w / 2.0), top + m)
        } else if center.0 >= right {
            (right - w - m, clamp_y(center.1 - h / 2.0))
        } else if center.0 <= left {
            (left + m, clamp_y(center.1 - h / 2.0))
        } else {
            (clamp_x(center.0 - w / 2.0), clamp_y(center.1 - h - m))
        };
        self.window.set_outer_position(PhysicalPosition::new(x, y));
    }
}

/// Zone de travail (écran sans la barre des tâches) de l'écran qui contient ce point.
fn work_area(x: i32, y: i32) -> RECT {
    unsafe {
        let monitor = MonitorFromPoint(POINT { x, y }, MONITOR_DEFAULTTONEAREST);
        let mut info: MONITORINFO = std::mem::zeroed();
        info.cbSize = size_of::<MONITORINFO>() as u32;
        GetMonitorInfoW(monitor, &mut info);
        info.rcWork
    }
}

/// Habillage de Windows 11 sans cadre : coins arrondis, liseré sombre, et
/// l'ombre de la fenêtre (une marge d'un pixel étendue dans la zone cliente,
/// recouverte par la vue web). Sans effet visible sur Windows 10, hors l'ombre.
pub fn frame(hwnd: windows_sys::Win32::Foundation::HWND) {
    let corner: u32 = 2; // DWMWCP_ROUND
    let border: u32 = 0x0042_372B; // #2B3742 (COLORREF : 0x00BBGGRR)
    let dark: u32 = 1;
    let margins = MARGINS { cxLeftWidth: 0, cxRightWidth: 0, cyTopHeight: 0, cyBottomHeight: 1 };
    unsafe {
        DwmSetWindowAttribute(hwnd, 20 /* DWMWA_USE_IMMERSIVE_DARK_MODE */, (&dark as *const u32).cast(), 4);
        DwmSetWindowAttribute(hwnd, 33 /* DWMWA_WINDOW_CORNER_PREFERENCE */, (&corner as *const u32).cast(), 4);
        DwmSetWindowAttribute(hwnd, 34 /* DWMWA_BORDER_COLOR */, (&border as *const u32).cast(), 4);
        DwmExtendFrameIntoClientArea(hwnd, &margins);
    }
}
