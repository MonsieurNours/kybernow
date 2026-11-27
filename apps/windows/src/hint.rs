//! Petites fenêtres transparentes posées sur l'image d'une session ou d'une
//! partie : une capsule sombre au texte clair qui ne prend ni clic ni
//! clavier (`WS_EX_TRANSPARENT`, `HTTRANSPARENT`), à sa place au-dessus de la
//! fenêtre qui la porte. Les aides d'une partie PlayStation (en bas, au
//! milieu), et, barre fermée, les signes d'une session (docs/link-ui.md,
//! « Caméra de l'appareil » et « Micro ») : la capsule « ● Caméra » en haut à
//! gauche de l'image, l'icône du micro près du trait du bord haut.
//!
//! Créées, montrées et détruites sur le fil de la fenêtre qui les porte (elles
//! en reçoivent les messages) ; détruites avec elle par Windows.

use std::cell::RefCell;
use std::sync::OnceLock;

use windows::Win32::Foundation::{COLORREF, HWND, LPARAM, LRESULT, POINT, RECT, SIZE, WPARAM};
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::HiDpi::GetDpiForWindow;
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::w;

/// Où la capsule se pose sur la zone cliente de la fenêtre qui la porte.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Place {
    /// En bas, au milieu (aides du clavier et de la souris d'une partie).
    Bottom,
    /// Au milieu (contenu protégé).
    Middle,
    /// En haut à gauche (« ● Caméra »).
    TopLeft,
    /// En haut, juste à droite du trait du bord haut (220 points au milieu) :
    /// l'icône du micro près de l'indicateur de la session.
    BesideEdge,
}

/// Ce que la capsule montre.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Look {
    /// Du texte ; un « ● » en tête est dessiné en rouge (le voyant).
    Text,
    /// Une icône de la police des symboles de Windows (Segoe MDL2 Assets,
    /// sur Windows 10 et 11) : le texte est son caractère.
    Icon,
}

/// Le micro de la police des symboles de Windows.
pub const MIC_ICON: &str = "\u{E720}";
/// Le voyant rouge de la capsule de la caméra.
const DOT: char = '●';
/// Largeur du trait du bord haut, en points (`render.rs`, `draw_hint`).
const EDGE_WIDTH_POINTS: f32 = 220.0;

pub struct Hint {
    owner: isize,
    hwnd: isize,
    place: Place,
    look: Look,
    text: Option<String>,
}

thread_local! {
    /// Le texte et l'aspect de chaque capsule, lus par son `WM_PAINT`.
    static TEXTS: RefCell<Vec<(isize, Vec<u16>, Look)>> = const { RefCell::new(Vec::new()) };
}

fn register() -> bool {
    static DONE: OnceLock<bool> = OnceLock::new();
    *DONE.get_or_init(|| unsafe {
        let Ok(instance) = GetModuleHandleW(None) else { return false };
        let class = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            lpfnWndProc: Some(hint_proc),
            hInstance: instance.into(),
            hCursor: LoadCursorW(None, IDC_ARROW).unwrap_or_default(),
            lpszClassName: w!("KyberNowHint"),
            ..Default::default()
        };
        RegisterClassExW(&class) != 0
    })
}

pub fn scale_of(hwnd: HWND) -> f32 {
    let dpi = unsafe { GetDpiForWindow(hwnd) };
    if dpi == 0 { 1.0 } else { dpi as f32 / 96.0 }
}

impl Hint {
    /// Une capsule portée par `owner` (cachée tant que rien ne la montre).
    pub fn new(owner: HWND, place: Place, look: Look) -> Hint {
        let hwnd = if register() {
            unsafe {
                let instance = GetModuleHandleW(None).ok();
                CreateWindowExW(
                    WS_EX_LAYERED | WS_EX_TRANSPARENT | WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW,
                    w!("KyberNowHint"),
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
            }
        } else {
            HWND::default()
        };
        if !hwnd.0.is_null() {
            unsafe {
                let _ = SetLayeredWindowAttributes(hwnd, COLORREF(0), 225, LWA_ALPHA);
            }
        }
        Hint { owner: owner.0 as isize, hwnd: hwnd.0 as isize, place, look, text: None }
    }

    /// La fenêtre qui la porte.
    pub fn owner(&self) -> HWND {
        HWND(self.owner as *mut _)
    }

    fn hwnd(&self) -> HWND {
        HWND(self.hwnd as *mut _)
    }

    /// Encore là (Windows détruit la capsule avec la fenêtre qui la porte).
    pub fn alive(&self) -> bool {
        self.hwnd != 0 && unsafe { IsWindow(Some(self.hwnd())).as_bool() }
    }

    fn font(look: Look, scale: f32) -> HFONT {
        let (size, weight, face) = match look {
            Look::Text => (14.0, FW_SEMIBOLD.0 as i32, w!("Segoe UI")),
            Look::Icon => (14.0, FW_NORMAL.0 as i32, w!("Segoe MDL2 Assets")),
        };
        unsafe {
            CreateFontW(
                -((size * scale) as i32),
                0,
                0,
                0,
                weight,
                0,
                0,
                0,
                DEFAULT_CHARSET,
                OUT_DEFAULT_PRECIS,
                CLIP_DEFAULT_PRECIS,
                CLEARTYPE_QUALITY,
                0,
                face,
            )
        }
    }

    /// Montre ce texte (ou se cache), à sa place sur la fenêtre qui la porte ;
    /// cachée aussi quand celle-ci l'est ou est réduite. Rappelée quand la
    /// fenêtre bouge ou change de taille.
    pub fn show(&mut self, text: Option<&str>) {
        if !self.alive() {
            return;
        }
        let owner = self.owner();
        let visible = unsafe { IsWindowVisible(owner).as_bool() && !IsIconic(owner).as_bool() };
        let Some(text) = text.filter(|_| visible) else {
            self.text = None;
            unsafe {
                let _ = ShowWindow(self.hwnd(), SW_HIDE);
            }
            return;
        };
        self.text = Some(text.to_string());
        let wide: Vec<u16> = text.encode_utf16().collect();
        let key = self.hwnd;
        TEXTS.with(|h| {
            let mut h = h.borrow_mut();
            h.retain(|(k, _, _)| *k != key);
            h.push((key, wide.clone(), self.look));
        });
        unsafe {
            let scale = scale_of(owner);
            let dc = GetDC(Some(self.hwnd()));
            let font = Self::font(self.look, scale);
            let old = SelectObject(dc, font.into());
            let mut size = SIZE::default();
            let _ = GetTextExtentPoint32W(dc, &wide, &mut size);
            SelectObject(dc, old);
            let _ = DeleteObject(font.into());
            ReleaseDC(Some(self.hwnd()), dc);
            let (pad_x, pad_y) = match self.look {
                Look::Text => ((16.0 * scale) as i32, (9.0 * scale) as i32),
                Look::Icon => ((9.0 * scale) as i32, (7.0 * scale) as i32),
            };
            let (w, h) = (size.cx + 2 * pad_x, size.cy + 2 * pad_y);
            let mut client = RECT::default();
            let _ = GetClientRect(owner, &mut client);
            let mut tl = POINT { x: client.left, y: client.top };
            let mut br = POINT { x: client.right, y: client.bottom };
            let _ = ClientToScreen(owner, &mut tl);
            let _ = ClientToScreen(owner, &mut br);
            let margin = (12.0 * scale) as i32;
            let (x, y) = match self.place {
                Place::Bottom => ((tl.x + br.x) / 2 - w / 2, br.y - h - (24.0 * scale) as i32),
                Place::Middle => ((tl.x + br.x) / 2 - w / 2, (tl.y + br.y) / 2 - h / 2),
                Place::TopLeft => (tl.x + margin, tl.y + margin),
                Place::BesideEdge => ((tl.x + br.x) / 2 + ((EDGE_WIDTH_POINTS / 2.0 + 10.0) * scale) as i32, tl.y + (6.0 * scale) as i32),
            };
            let rgn = CreateRoundRectRgn(0, 0, w + 1, h + 1, h, h);
            let _ = SetWindowRgn(self.hwnd(), Some(rgn), true);
            let _ = SetWindowPos(self.hwnd(), None, x, y, w, h, SWP_NOACTIVATE | SWP_NOZORDER | SWP_SHOWWINDOW);
            let _ = InvalidateRect(Some(self.hwnd()), None, true);
        }
    }

    pub fn destroy(&self) {
        if self.alive() {
            unsafe {
                let _ = DestroyWindow(self.hwnd());
            }
        }
        let key = self.hwnd;
        TEXTS.with(|h| h.borrow_mut().retain(|(k, _, _)| *k != key));
    }
}

/// Rappel de Windows : une panique ne le traverse pas (rattrapée et notée au
/// journal, `ffi_guard!` du cœur), le corps est `hint_proc_body`.
unsafe extern "system" fn hint_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    kybernow_core::ffi_guard!({ unsafe { hint_proc_body(hwnd, msg, wparam, lparam) } })
}

unsafe fn hint_proc_body(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
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
                let (text, look) = TEXTS
                    .with(|h| h.borrow().iter().find(|(k, _, _)| *k == hwnd.0 as isize).map(|(_, t, l)| (t.clone(), *l)))
                    .unwrap_or((Vec::new(), Look::Text));
                let owner = GetWindow(hwnd, GW_OWNER).unwrap_or(hwnd);
                let font = Hint::font(look, scale_of(owner));
                let old = SelectObject(dc, font.into());
                SetBkMode(dc, TRANSPARENT);
                let dot = look == Look::Text && char::decode_utf16(text.iter().copied()).next().and_then(Result::ok) == Some(DOT);
                if dot {
                    // Le voyant rouge, puis le reste en clair, centrés ensemble.
                    let mut all = SIZE::default();
                    let _ = GetTextExtentPoint32W(dc, &text, &mut all);
                    let mut first = SIZE::default();
                    let _ = GetTextExtentPoint32W(dc, &text[..1], &mut first);
                    let x0 = r.left + (r.right - r.left - all.cx) / 2;
                    let mut rd = RECT { left: x0, right: x0 + first.cx, ..r };
                    let mut rest = RECT { left: x0 + first.cx, right: r.right, ..r };
                    SetTextColor(dc, COLORREF(0x003a3aff));
                    DrawTextW(dc, &mut text[..1].to_vec(), &mut rd, DT_LEFT | DT_VCENTER | DT_SINGLELINE | DT_NOPREFIX);
                    SetTextColor(dc, COLORREF(0x00ffffff));
                    DrawTextW(dc, &mut text[1..].to_vec(), &mut rest, DT_LEFT | DT_VCENTER | DT_SINGLELINE | DT_NOPREFIX);
                } else {
                    let mut text = text;
                    SetTextColor(dc, COLORREF(0x00ffffff));
                    DrawTextW(dc, &mut text, &mut r, DT_CENTER | DT_VCENTER | DT_SINGLELINE | DT_NOPREFIX);
                }
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
