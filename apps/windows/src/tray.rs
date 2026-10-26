//! Icône de KyberNow Link dans la zone de notification (docs/link-ui.md,
//! « Icône près de l'horloge ») : une fenêtre cachée sur son propre fil (elle
//! ne dépend ni de l'accueil ni de la session), l'icône dessinée par
//! `glyph.rs` en blanc ou en noir selon le thème de la barre des tâches, et un
//! menu de Windows construit à chaque ouverture depuis `menu.rs`.
//!
//! Clic droit : le menu ; clic gauche : le même menu, après le délai du
//! double clic de Windows (un menu déjà ouvert mangerait le second clic) ;
//! double clic : la fenêtre. Menu ouvert, ses lignes suivent l'app chaque
//! seconde (mesures de la session, état des ordinateurs) et le sous-menu d'un
//! ordinateur est refait à son ouverture. Le menu est sombre ou clair comme la
//! barre des tâches ; le point de couleur de chaque ordinateur suit.
//!
//! Distincte de l'icône de KyberNow Station (`server/tray`) : sur un PC qui a
//! les deux, deux icônes.

use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicIsize, AtomicU32, Ordering};
use std::time::{Duration, Instant};

use tauri::AppHandle;
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::Graphics::Gdi::{
    BI_RGB, BITMAPINFO, BITMAPINFOHEADER, CreateDIBSection, DIB_RGB_COLORS, HBITMAP, RDW_ERASE, RDW_INVALIDATE, RDW_UPDATENOW,
    RedrawWindow,
};
use windows::Win32::System::LibraryLoader::{GetModuleHandleW, GetProcAddress, LoadLibraryW};
use windows::Win32::System::Registry::{HKEY_CURRENT_USER, RRF_RT_REG_DWORD, RegGetValueW};
use windows::Win32::System::Threading::{AttachThreadInput, GetCurrentThreadId};
use windows::Win32::UI::Controls::{
    TASKDIALOG_BUTTON, TASKDIALOGCONFIG, TDF_ALLOW_DIALOG_CANCELLATION, TaskDialogIndirect,
};
use windows::Win32::UI::HiDpi::{GetDpiForWindow, GetSystemMetricsForDpi};
use windows::Win32::UI::Input::KeyboardAndMouse::GetDoubleClickTime;
use windows::Win32::UI::Shell::{
    NIF_ICON, NIF_MESSAGE, NIF_SHOWTIP, NIF_TIP, NIM_ADD, NIM_DELETE, NIM_MODIFY, NIM_SETVERSION, NIN_SELECT, NINF_KEY,
    NOTIFYICON_VERSION_4, NOTIFYICONDATAW, Shell_NotifyIconW,
};
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::{BOOL, HSTRING, PCSTR, PCWSTR, PWSTR, w};

use crate::glyph::{self, Look};
use crate::i18n::t;
use crate::menu::{self, Cmd, Item, Mark};

/// Messages de l'icône (envoyés par Windows) et de l'app.
const WM_APP_TRAY: u32 = WM_APP + 1;
const WM_APP_REFRESH: u32 = WM_APP + 2;
/// Clic gauche en attente d'un éventuel double clic.
const TIMER_CLICK: usize = 1;
/// Menu ouvert : ses lignes suivent l'app.
const TIMER_LIVE: usize = 2;
/// L'icône n'a pas pu être posée (barre des tâches pas encore prête).
const TIMER_RETRY: usize = 3;
const ICON_ID: u32 = 1;

static HWND_: AtomicIsize = AtomicIsize::new(0);
static TASKBAR_CREATED: AtomicU32 = AtomicU32::new(0);
static APP: OnceLock<AppHandle> = OnceLock::new();

/// Un menu ouvert : la commande de chaque identifiant (`id - 1`), ses lignes
/// du haut telles qu'affichées, et le sous-menu de chaque ordinateur.
struct Open {
    cmds: Vec<Cmd>,
    top: Vec<Item>,
    root: HMENU,
    subs: Vec<(HMENU, String)>,
    look: Dots,
}

/// Les points de couleur d'un menu : sombre ou clair, taille en pixels.
#[derive(Clone, Copy)]
struct Dots {
    dark: bool,
    size: i32,
}

#[derive(Default)]
struct Tray {
    added: bool,
    icon: Option<HICON>,
    /// Ce qui est dessiné : état, barre claire, taille.
    drawn: Option<(Look, bool, i32)>,
    tip: String,
    /// Clic gauche en attente : où ouvrir le menu.
    click: Option<(i32, i32)>,
    last_double: Option<Instant>,
    open: Option<Open>,
}

thread_local! {
    static TRAY: RefCell<Tray> = RefCell::new(Tray::default());
    /// Points de couleur déjà faits (couleur, taille), gardés pour les menus suivants.
    static DOTS: RefCell<HashMap<([u8; 3], i32), HBITMAP>> = RefCell::new(HashMap::new());
}

fn hwnd() -> HWND {
    HWND(HWND_.load(Ordering::Relaxed) as *mut _)
}

fn tr(key: &str, params: &[(&str, &str)]) -> String {
    t(key, params)
}

/// Lance l'icône sur son fil.
pub fn start(app: AppHandle) {
    let _ = APP.set(app);
    let spawned = std::thread::Builder::new().name("kybernow-icone".into()).spawn(|| {
        if let Err(e) = run() {
            log::error!("icône près de l'horloge : {e}");
        }
    });
    if let Err(e) = spawned {
        log::error!("icône près de l'horloge : fil refusé ({e})");
    }
}

/// L'app a changé (session, ordinateurs, mesures) : icône, infobulle et menu
/// ouvert suivent.
pub fn refresh() {
    let h = hwnd();
    if !h.0.is_null() {
        unsafe {
            let _ = PostMessageW(Some(h), WM_APP_REFRESH, WPARAM(0), LPARAM(0));
        }
    }
}

/// Retire l'icône tout de suite (l'app se ferme) : sinon elle resterait
/// jusqu'au passage de la souris.
pub fn remove() {
    let h = hwnd();
    if !h.0.is_null() {
        let data = NOTIFYICONDATAW { cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32, hWnd: h, uID: ICON_ID, ..Default::default() };
        unsafe {
            let _ = Shell_NotifyIconW(NIM_DELETE, &data);
        }
    }
}

fn run() -> windows::core::Result<()> {
    unsafe {
        let instance = GetModuleHandleW(None)?;
        let class = w!("KyberNowLinkTray");
        let wc = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            lpfnWndProc: Some(wndproc),
            hInstance: instance.into(),
            lpszClassName: class,
            ..Default::default()
        };
        RegisterClassExW(&wc);
        TASKBAR_CREATED.store(RegisterWindowMessageW(w!("TaskbarCreated")), Ordering::Relaxed);
        // Une fenêtre de premier niveau cachée (pas une fenêtre de messages) :
        // elle reçoit les diffusions « TaskbarCreated » et WM_SETTINGCHANGE.
        let hwnd = CreateWindowExW(WS_EX_TOOLWINDOW, class, w!("KyberNow Link"), WS_POPUP, 0, 0, 0, 0, None, None, Some(instance.into()), None)?;
        HWND_.store(hwnd.0 as isize, Ordering::Relaxed);
        update(hwnd);
        log::info!("icône près de l'horloge : posée");
        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
    Ok(())
}

/// Barre des tâches claire (Windows 10 1903 et après) : icône noire.
fn light_taskbar() -> bool {
    let mut value = 0u32;
    let mut len = 4u32;
    let r = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            w!(r"Software\Microsoft\Windows\CurrentVersion\Themes\Personalize"),
            w!("SystemUsesLightTheme"),
            RRF_RT_REG_DWORD,
            None,
            Some((&raw mut value).cast()),
            Some(&raw mut len),
        )
    };
    r.is_ok() && value == 1
}

/// Taille d'une petite icône à l'échelle de l'écran (16 px à 100 %, 24 à 150 %…).
fn icon_size(hwnd: HWND) -> i32 {
    let dpi = unsafe { GetDpiForWindow(hwnd) }.max(96);
    unsafe { GetSystemMetricsForDpi(SM_CXSMICON, dpi) }.clamp(16, 64)
}

fn make_icon(size: i32, look: Look, light: bool) -> Option<HICON> {
    let color = if light { [0, 0, 0] } else { [255, 255, 255] };
    let mut px = glyph::rgba(size as u32, look, color);
    for p in px.chunks_mut(4) {
        p.swap(0, 2);
    }
    // Masque d'une couleur tout à zéro : l'alpha de l'image décide.
    let row = (size as usize).div_ceil(16) * 2;
    let mask = vec![0u8; row * size as usize];
    unsafe { CreateIcon(None, size, size, 1, 32, mask.as_ptr(), px.as_ptr()).ok() }
}

fn data(hwnd: HWND) -> NOTIFYICONDATAW {
    NOTIFYICONDATAW { cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32, hWnd: hwnd, uID: ICON_ID, ..Default::default() }
}

/// Icône et infobulle selon l'app ; posée si elle ne l'est pas encore.
fn update(hwnd: HWND) {
    let Some(app) = APP.get() else { return };
    let state = crate::ui::tray_state(app);
    let look = if state.session.is_some() { Look::Lit } else { Look::Idle };
    let tip = menu::tooltip(&state, &tr);
    let key = (look, light_taskbar(), icon_size(hwnd));
    TRAY.with_borrow_mut(|tray| {
        let mut changed = tray.tip != tip;
        if tray.drawn != Some(key) {
            if let Some(icon) = make_icon(key.2, key.0, key.1) {
                if let Some(old) = tray.icon.replace(icon) {
                    // Windows garde sa propre copie de l'icône posée.
                    let _ = unsafe { DestroyIcon(old) };
                }
                tray.drawn = Some(key);
                changed = true;
            }
        }
        tray.tip = tip;
        let Some(icon) = tray.icon else { return };
        if tray.added && !changed {
            return;
        }
        let mut d = data(hwnd);
        d.uFlags = NIF_MESSAGE | NIF_ICON | NIF_TIP | NIF_SHOWTIP;
        d.uCallbackMessage = WM_APP_TRAY;
        d.hIcon = icon;
        for (i, c) in tray.tip.encode_utf16().take(d.szTip.len() - 1).enumerate() {
            d.szTip[i] = c;
        }
        unsafe {
            if tray.added {
                let _ = Shell_NotifyIconW(NIM_MODIFY, &d);
            } else if Shell_NotifyIconW(NIM_ADD, &d).as_bool() {
                d.Anonymous.uVersion = NOTIFYICON_VERSION_4;
                let _ = Shell_NotifyIconW(NIM_SETVERSION, &d);
                tray.added = true;
                let _ = KillTimer(Some(hwnd), TIMER_RETRY);
            } else {
                // Barre des tâches pas encore là (ouverture de session) : on réessaie.
                SetTimer(Some(hwnd), TIMER_RETRY, 2000, None);
            }
        }
    });
    if TRAY.with_borrow(|t| t.open.is_some()) {
        live(&state);
    }
}

/// `&` introduit un raccourci dans un menu de Windows : doublé, il s'affiche.
fn menu_text(text: &str) -> Vec<u16> {
    text.replace('&', "&&").encode_utf16().chain(Some(0)).collect()
}

fn item_state(enabled: bool, mark: Mark, default: bool) -> MENU_ITEM_STATE {
    let mut s = MENU_ITEM_STATE(0);
    if !enabled {
        s |= MFS_DISABLED;
    }
    if mark != Mark::None {
        s |= MFS_CHECKED;
    }
    if default {
        s |= MFS_DEFAULT;
    }
    s
}

/// Le point de couleur d'un ordinateur : une image de 32 bits en alpha
/// prémultiplié, que les menus de Windows fondent sur leur fond.
fn dot_bitmap(dot: menu::Dot, look: Dots) -> Option<HBITMAP> {
    let rgb = dot.rgb(look.dark);
    let size = look.size;
    if let Some(b) = DOTS.with_borrow(|c| c.get(&(rgb, size)).copied()) {
        return Some(b);
    }
    let px = glyph::dot(size as u32, rgb);
    let info = BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: size,
            // Négatif : lignes de haut en bas.
            biHeight: -size,
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB.0,
            ..Default::default()
        },
        ..Default::default()
    };
    let mut bits: *mut core::ffi::c_void = std::ptr::null_mut();
    let bitmap = unsafe { CreateDIBSection(None, &info, DIB_RGB_COLORS, &mut bits, None, 0) }.ok()?;
    if bits.is_null() {
        return None;
    }
    let dst = unsafe { std::slice::from_raw_parts_mut(bits.cast::<u8>(), px.len()) };
    for (d, p) in dst.chunks_mut(4).zip(px.chunks(4)) {
        let a = u32::from(p[3]);
        let pre = |c: u8| ((u32::from(c) * a + 127) / 255) as u8;
        d.copy_from_slice(&[pre(p[2]), pre(p[1]), pre(p[0]), p[3]]);
    }
    DOTS.with_borrow_mut(|c| c.insert((rgb, size), bitmap));
    Some(bitmap)
}

/// Menus sombres quand la barre des tâches l'est : fonctions d'uxtheme sans
/// nom (Windows 10 1903 et après, celles qu'emploient les menus de Windows
/// eux-mêmes) : `SetPreferredAppMode` (n° 135), `FlushMenuThemes` (n° 136),
/// `AllowDarkModeForWindow` (n° 133). Absentes, les menus restent clairs.
struct Theme {
    set_mode: unsafe extern "system" fn(i32) -> i32,
    flush: unsafe extern "system" fn(),
    allow: Option<unsafe extern "system" fn(HWND, BOOL) -> BOOL>,
}

fn theme() -> Option<&'static Theme> {
    static THEME: OnceLock<Option<Theme>> = OnceLock::new();
    THEME
        .get_or_init(|| unsafe {
            let lib = LoadLibraryW(w!("uxtheme.dll")).ok()?;
            let by = |n: usize| GetProcAddress(lib, PCSTR(n as *const u8));
            Some(Theme {
                set_mode: std::mem::transmute::<unsafe extern "system" fn() -> isize, unsafe extern "system" fn(i32) -> i32>(by(135)?),
                flush: std::mem::transmute::<unsafe extern "system" fn() -> isize, unsafe extern "system" fn()>(by(136)?),
                allow: by(133).map(|f| std::mem::transmute::<unsafe extern "system" fn() -> isize, unsafe extern "system" fn(HWND, BOOL) -> BOOL>(f)),
            })
        })
        .as_ref()
}

/// Le menu prend le thème de la barre des tâches ; rend le mode d'avant (à
/// remettre après le menu) et s'il sera sombre.
fn menu_theme(hwnd: HWND) -> (Option<i32>, bool) {
    let dark = !light_taskbar();
    let Some(t) = theme() else { return (None, false) };
    unsafe {
        if let Some(allow) = t.allow {
            let _ = allow(hwnd, dark.into());
        }
        // 2 : toujours sombre ; 3 : toujours clair.
        let before = (t.set_mode)(if dark { 2 } else { 3 });
        (t.flush)();
        (Some(before), dark)
    }
}

fn restore_theme(before: Option<i32>) {
    if let (Some(t), Some(mode)) = (theme(), before) {
        unsafe {
            (t.set_mode)(mode);
            (t.flush)();
        }
    }
}

fn insert(menu: HMENU, text: &str, id: u32, sub: Option<HMENU>, state: MENU_ITEM_STATE, radio: bool, bitmap: Option<HBITMAP>) {
    let mut wide = menu_text(text);
    let mut mii = MENUITEMINFOW {
        cbSize: std::mem::size_of::<MENUITEMINFOW>() as u32,
        fMask: MIIM_STRING | MIIM_ID | MIIM_STATE | MIIM_FTYPE,
        fType: if radio { MFT_STRING | MFT_RADIOCHECK } else { MFT_STRING },
        fState: state,
        wID: id,
        dwTypeData: PWSTR(wide.as_mut_ptr()),
        cch: (wide.len() - 1) as u32,
        ..Default::default()
    };
    if let Some(sub) = sub {
        mii.fMask |= MIIM_SUBMENU;
        mii.hSubMenu = sub;
    }
    if let Some(b) = bitmap {
        mii.fMask |= MIIM_BITMAP;
        mii.hbmpItem = b;
    }
    unsafe {
        let at = GetMenuItemCount(Some(menu)).max(0) as u32;
        let _ = InsertMenuItemW(menu, at, true, &mii);
    }
}

/// Remplit `menu` ; chaque commande reçoit l'identifiant `cmds.len()` (à partir de 1).
fn fill(menu: HMENU, items: &[Item], cmds: &mut Vec<Cmd>, subs: &mut Vec<(HMENU, String)>, look: Dots) {
    for item in items {
        match item {
            Item::Separator => unsafe {
                let _ = AppendMenuW(menu, MF_SEPARATOR, 0, PCWSTR::null());
            },
            Item::Line { text, cmd, enabled, mark, default } => {
                let id = match cmd {
                    Some(c) => {
                        cmds.push(c.clone());
                        cmds.len() as u32
                    }
                    None => 0,
                };
                insert(menu, text, id, None, item_state(*enabled, *mark, *default), *mark == Mark::Radio, None);
            }
            Item::Sub { text, key, checked, dot, items } => {
                let Ok(sub) = (unsafe { CreatePopupMenu() }) else { continue };
                fill(sub, items, cmds, subs, look);
                if let Some(k) = key {
                    subs.push((sub, k.clone()));
                }
                let mark = if *checked { Mark::Check } else { Mark::None };
                let bitmap = dot.and_then(|d| dot_bitmap(d, look));
                insert(menu, text, 0, Some(sub), item_state(true, mark, false), false, bitmap);
            }
        }
    }
}

/// Mêmes lignes, dans le même ordre (seuls textes et états diffèrent).
fn same_shape(a: &[Item], b: &[Item]) -> bool {
    a.len() == b.len()
        && a.iter().zip(b).all(|pair| match pair {
            (Item::Line { cmd: x, .. }, Item::Line { cmd: y, .. }) => x == y,
            (Item::Sub { key: x, .. }, Item::Sub { key: y, .. }) => x == y,
            (Item::Separator, Item::Separator) => true,
            _ => false,
        })
}

/// Menu ouvert : ses lignes du haut prennent les textes et états du moment
/// (mesures de la session, état des ordinateurs), puis il se redessine.
fn live(state: &menu::State) {
    let fresh = menu::build(state, &tr);
    let changed = TRAY.with_borrow_mut(|tray| {
        let Some(open) = tray.open.as_mut() else { return false };
        if !same_shape(&open.top, &fresh) {
            return false;
        }
        let mut changed = false;
        for (i, (old, new)) in open.top.iter().zip(&fresh).enumerate() {
            if old == new {
                continue;
            }
            let (state, dot) = match new {
                Item::Line { enabled, mark, default, .. } => (item_state(*enabled, *mark, *default), None),
                Item::Sub { checked, dot, .. } => (item_state(true, if *checked { Mark::Check } else { Mark::None }, false), *dot),
                Item::Separator => continue,
            };
            let mut wide = menu_text(new.text());
            let mut mii = MENUITEMINFOW {
                cbSize: std::mem::size_of::<MENUITEMINFOW>() as u32,
                fMask: MIIM_STRING | MIIM_STATE,
                fState: state,
                dwTypeData: PWSTR(wide.as_mut_ptr()),
                cch: (wide.len() - 1) as u32,
                ..Default::default()
            };
            if let Some(b) = dot.and_then(|d| dot_bitmap(d, open.look)) {
                mii.fMask |= MIIM_BITMAP;
                mii.hbmpItem = b;
            }
            unsafe {
                let _ = SetMenuItemInfoW(open.root, i as u32, true, &mii);
            }
            changed = true;
        }
        open.top = fresh;
        changed
    });
    if changed {
        unsafe {
            let _ = EnumThreadWindows(GetCurrentThreadId(), Some(redraw_menu), LPARAM(0));
        }
    }
}

/// Redessine les menus ouverts par ce fil (classe `#32768`).
unsafe extern "system" fn redraw_menu(hwnd: HWND, _: LPARAM) -> BOOL {
    let mut class = [0u16; 16];
    let n = unsafe { GetClassNameW(hwnd, &mut class) }.max(0) as usize;
    if String::from_utf16_lossy(&class[..n]) == "#32768" {
        unsafe {
            let _ = RedrawWindow(Some(hwnd), None, None, RDW_INVALIDATE | RDW_ERASE | RDW_UPDATENOW);
        }
    }
    BOOL(1)
}

/// Le sous-menu d'un ordinateur va s'ouvrir : refait selon l'état du moment
/// (l'ordinateur a pu répondre depuis l'ouverture du menu).
fn refill(sub: HMENU) {
    let Some(app) = APP.get() else { return };
    let Some(key) = TRAY.with_borrow(|t| t.open.as_ref().and_then(|o| o.subs.iter().find(|(h, _)| *h == sub).map(|(_, k)| k.clone()))) else {
        return;
    };
    let state = crate::ui::tray_state(app);
    let Some(c) = state.computers.iter().find(|c| c.id == key) else { return };
    let items = menu::computer(&state, c, &tr);
    TRAY.with_borrow_mut(|tray| {
        let Some(open) = tray.open.as_mut() else { return };
        unsafe {
            while GetMenuItemCount(Some(sub)) > 0 {
                if DeleteMenu(sub, 0, MF_BYPOSITION).is_err() {
                    break;
                }
            }
        }
        let mut nested = Vec::new();
        let look = open.look;
        fill(sub, &items, &mut open.cmds, &mut nested, look);
    });
}

/// La fenêtre de l'icône au premier plan : sans cela, un menu ne se ferme
/// pas d'un clic ailleurs. Windows le permet juste après un clic sur l'icône ;
/// si ce n'est plus le cas (clic gauche, après le délai du double clic), en
/// se rattachant un instant au fil du premier plan, comme la session.
fn take_foreground(hwnd: HWND) {
    unsafe {
        if SetForegroundWindow(hwnd).as_bool() {
            return;
        }
        let front = GetWindowThreadProcessId(GetForegroundWindow(), None);
        let me = GetCurrentThreadId();
        if front != 0 && front != me && AttachThreadInput(me, front, true).as_bool() {
            let _ = SetForegroundWindow(hwnd);
            let _ = AttachThreadInput(me, front, false);
        }
    }
}

/// Ouvre le menu en `(x, y)` et fait ce qui y est choisi.
fn show_menu(hwnd: HWND, x: i32, y: i32) {
    let Some(app) = APP.get() else { return };
    if TRAY.with_borrow(|t| t.open.is_some()) {
        return;
    }
    // Les ordinateurs, tout de suite : le menu suit leurs réponses.
    crate::ui::poll_soon(app);
    let state = crate::ui::tray_state(app);
    let top = menu::build(&state, &tr);
    let Ok(root) = (unsafe { CreatePopupMenu() }) else { return };
    let (before, dark) = menu_theme(hwnd);
    let look = Dots { dark, size: icon_size(hwnd) };
    let mut cmds = Vec::new();
    let mut subs = Vec::new();
    fill(root, &top, &mut cmds, &mut subs, look);
    TRAY.with_borrow_mut(|t| t.open = Some(Open { cmds, top, root, subs, look }));
    let chosen = unsafe {
        take_foreground(hwnd);
        SetTimer(Some(hwnd), TIMER_LIVE, 1000, None);
        let align = if GetSystemMetrics(SM_MENUDROPALIGNMENT) != 0 { TPM_RIGHTALIGN } else { TPM_LEFTALIGN };
        // Au-dessus de la barre des tâches (zone de travail), comme les menus de Windows.
        let flags = TPM_RETURNCMD | TPM_RIGHTBUTTON | TPM_BOTTOMALIGN | TPM_WORKAREA | align;
        let r = TrackPopupMenuEx(root, flags.0, x, y, hwnd, None);
        let _ = KillTimer(Some(hwnd), TIMER_LIVE);
        // Conseillé par Windows après un menu de la zone de notification.
        let _ = PostMessageW(Some(hwnd), WM_NULL, WPARAM(0), LPARAM(0));
        r.0
    };
    let open = TRAY.with_borrow_mut(|t| t.open.take());
    restore_theme(before);
    unsafe {
        let _ = DestroyMenu(root);
    }
    let cmd = open.and_then(|o| usize::try_from(chosen).ok().filter(|&i| i > 0).and_then(|i| o.cmds.get(i - 1).cloned()));
    if let Some(cmd) = cmd {
        log::info!("icône près de l'horloge : {cmd:?}");
        crate::ui::tray_command(app, cmd);
    }
}

unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    unsafe {
        match msg {
            WM_APP_TRAY => {
                let event = (lparam.0 & 0xFFFF) as u32;
                // NOTIFYICON_VERSION_4 : le point d'ancrage du menu dans wParam.
                let x = (wparam.0 & 0xFFFF) as u16 as i16 as i32;
                let y = ((wparam.0 >> 16) & 0xFFFF) as u16 as i16 as i32;
                match event {
                    WM_LBUTTONDOWN | WM_RBUTTONDOWN => {
                        if let Some(app) = APP.get() {
                            crate::ui::poll_soon(app);
                        }
                    }
                    WM_CONTEXTMENU => show_menu(hwnd, x, y),
                    e if e == NIN_SELECT | NINF_KEY => show_menu(hwnd, x, y),
                    NIN_SELECT => {
                        let double = GetDoubleClickTime();
                        let after_double = TRAY.with_borrow(|t| t.last_double.is_some_and(|d| d.elapsed() < Duration::from_millis(u64::from(double))));
                        if !after_double {
                            // Le premier plan tout de suite, tant que Windows le permet.
                            let _ = SetForegroundWindow(hwnd);
                            TRAY.with_borrow_mut(|t| t.click = Some((x, y)));
                            SetTimer(Some(hwnd), TIMER_CLICK, double, None);
                        }
                    }
                    WM_LBUTTONDBLCLK => {
                        let _ = KillTimer(Some(hwnd), TIMER_CLICK);
                        TRAY.with_borrow_mut(|t| {
                            t.click = None;
                            t.last_double = Some(Instant::now());
                        });
                        if let Some(app) = APP.get() {
                            crate::ui::tray_command(app, Cmd::Open);
                        }
                    }
                    _ => {}
                }
                LRESULT(0)
            }
            WM_TIMER => {
                match wparam.0 {
                    TIMER_CLICK => {
                        let _ = KillTimer(Some(hwnd), TIMER_CLICK);
                        if let Some((x, y)) = TRAY.with_borrow_mut(|t| t.click.take()) {
                            show_menu(hwnd, x, y);
                        }
                    }
                    TIMER_LIVE | TIMER_RETRY => update(hwnd),
                    _ => {}
                }
                LRESULT(0)
            }
            WM_INITMENUPOPUP => {
                refill(HMENU(wparam.0 as *mut _));
                LRESULT(0)
            }
            WM_APP_REFRESH => {
                update(hwnd);
                LRESULT(0)
            }
            // Thème de Windows changé (barre claire ou sombre), échelle ou écran changés.
            WM_SETTINGCHANGE | WM_DPICHANGED | WM_DISPLAYCHANGE => {
                update(hwnd);
                DefWindowProcW(hwnd, msg, wparam, lparam)
            }
            m if m != 0 && m == TASKBAR_CREATED.load(Ordering::Relaxed) => {
                // L'Explorateur a redémarré : l'icône est à reposer.
                TRAY.with_borrow_mut(|t| {
                    t.added = false;
                    t.drawn = None;
                });
                update(hwnd);
                LRESULT(0)
            }
            _ => DefWindowProcW(hwnd, msg, wparam, lparam),
        }
    }
}

fn wide(s: &str) -> HSTRING {
    HSTRING::from(s)
}

/// Une question à l'utilisateur, au premier plan : `yes` ou « Annuler ».
/// Rend vrai pour `yes`.
pub fn confirm(title: &str, message: &str, yes: &str) -> bool {
    const YES: i32 = 100;
    const NO: i32 = 101;
    let (window, title, message) = (wide(crate::shell::DISPLAY_NAME), wide(title), wide(message));
    let (yes, no) = (wide(yes), wide(&t("station.cancel", &[])));
    let buttons = [
        TASKDIALOG_BUTTON { nButtonID: YES, pszButtonText: PCWSTR(yes.as_ptr()) },
        TASKDIALOG_BUTTON { nButtonID: NO, pszButtonText: PCWSTR(no.as_ptr()) },
    ];
    let mut config = TASKDIALOGCONFIG {
        cbSize: std::mem::size_of::<TASKDIALOGCONFIG>() as u32,
        dwFlags: TDF_ALLOW_DIALOG_CANCELLATION,
        pszWindowTitle: PCWSTR(window.as_ptr()),
        pszMainInstruction: PCWSTR(title.as_ptr()),
        pszContent: PCWSTR(message.as_ptr()),
        cButtons: buttons.len() as u32,
        pButtons: buttons.as_ptr(),
        nDefaultButton: NO,
        ..Default::default()
    };
    // L'icône de l'app (ressource 32512 posée par tauri-build), dans la boîte
    // et sa barre de titre.
    if let Ok(module) = unsafe { GetModuleHandleW(None) } {
        config.hInstance = module.into();
        config.Anonymous1.pszMainIcon = PCWSTR(32512 as *const u16);
    }
    let mut pressed = 0i32;
    match unsafe { TaskDialogIndirect(&config, Some(&raw mut pressed), None, None) } {
        Ok(()) => pressed == YES,
        Err(e) => {
            log::warn!("boîte de dialogue : {e}");
            let text = wide(&format!("{}\n\n{}", title, message));
            unsafe { MessageBoxW(None, &text, &window, MB_OKCANCEL | MB_ICONWARNING | MB_SETFOREGROUND) == IDOK }
        }
    }
}

/// Un message à l'utilisateur (refus, erreur), au premier plan.
pub fn alert(text: &str) {
    let (text, window) = (wide(text), wide(crate::shell::DISPLAY_NAME));
    unsafe {
        MessageBoxW(None, &text, &window, MB_OK | MB_ICONINFORMATION | MB_SETFOREGROUND);
    }
}
