//! Ce que le mode console fait à Windows (`docs/pret.md`, « Station ») :
//! crochets clavier et souris, fenêtres de l'utilisateur, barre des tâches,
//! curseur, fenêtres « Ouvrir » et « Enregistrer sous », fenêtres et
//! processus du prêt, premier plan, verrouillage, heure locale.

use std::collections::{HashMap, HashSet};
use std::ffi::c_void;
use std::io::Write;
use std::path::PathBuf;
use std::ptr::{null, null_mut};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{OnceLock, mpsc};
use std::thread::JoinHandle;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Result, bail};
use windows_sys::Win32::Foundation::{CloseHandle, FILETIME, HANDLE, HWND, LPARAM, LRESULT, POINT, RECT, SYSTEMTIME, WAIT_ABANDONED, WAIT_OBJECT_0, WPARAM};
use windows_sys::Win32::Graphics::Dwm::{DWMWA_CLOAKED, DwmGetWindowAttribute};
use windows_sys::Win32::System::Diagnostics::ToolHelp::{CreateToolhelp32Snapshot, PROCESSENTRY32W, Process32FirstW, Process32NextW, TH32CS_SNAPPROCESS};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::System::RemoteDesktop::ProcessIdToSessionId;
use windows_sys::Win32::System::Shutdown::LockWorkStation;
use windows_sys::Win32::System::SystemInformation::GetTickCount64;
use windows_sys::Win32::System::Threading::{
    AttachThreadInput, CreateMutexW, GetCurrentProcessId, GetCurrentThreadId, GetProcessTimes, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION,
    PROCESS_TERMINATE, ReleaseMutex, TerminateProcess, WaitForSingleObject,
};
use windows_sys::Win32::System::Time::{FileTimeToSystemTime, SystemTimeToTzSpecificLocalTime};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{GetAsyncKeyState, VK_CONTROL, VK_SHIFT};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    ASFW_ANY, AllowSetForegroundWindow, BringWindowToTop, CallNextHookEx, ClipCursor, DispatchMessageW, EnumChildWindows, EnumWindows, GW_OWNER,
    GWL_EXSTYLE, GetClassNameW, GetForegroundWindow, GetMessageW, GetWindow, GetWindowLongPtrW, GetWindowPlacement, GetWindowRect,
    GetWindowThreadProcessId, HC_ACTION, HHOOK, HWND_TOPMOST, IsHungAppWindow, IsIconic, IsWindow, IsWindowVisible, IsZoomed, KBDLLHOOKSTRUCT,
    LLKHF_ALTDOWN, LLKHF_INJECTED, LLMHF_INJECTED, MSG, MSLLHOOKSTRUCT, PM_NOREMOVE, PeekMessageW, PostMessageW, PostThreadMessageW, SW_HIDE,
    SW_RESTORE, SW_SHOWMINNOACTIVE, SW_SHOWNA, SW_SHOWNOACTIVATE, SW_SHOWNORMAL, SWP_ASYNCWINDOWPOS, SWP_NOACTIVATE, SWP_NOMOVE,
    SWP_NOSIZE, SWP_NOZORDER, SetCursorPos, SetForegroundWindow, SetWindowPlacement, SetWindowPos, SetWindowsHookExW, ShowWindowAsync,
    TranslateMessage, UnhookWindowsHookEx, WH_KEYBOARD_LL, WH_MOUSE_LL, WINDOWPLACEMENT, WM_CLOSE, WM_KEYDOWN, WM_QUIT, WM_SYSKEYDOWN,
    WS_EX_TOOLWINDOW,
};

use super::store::Window;
use super::{CoverKey, Display, Mods, cover_key_of, keep};

pub fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(Some(0)).collect()
}

/// `%LOCALAPPDATA%\KyberNow\lend` : l'état du prêt et le journal de l'agent.
pub fn dir() -> Option<PathBuf> {
    std::env::var_os("LOCALAPPDATA").map(|d| PathBuf::from(d).join(r"KyberNow\lend"))
}

/// Une ligne du journal de l'agent (`agent.log`, recommencé au-delà de 1 Mo) :
/// l'agent n'a ni console ni fenêtre à lui pour le dire.
pub fn log(line: &str) {
    let Some(dir) = dir() else { return };
    let _ = std::fs::create_dir_all(&dir);
    let path = dir.join("agent.log");
    if std::fs::metadata(&path).is_ok_and(|m| m.len() > 1 << 20) {
        let _ = std::fs::rename(&path, dir.join("agent.old.log"));
    }
    if let Ok(mut file) = std::fs::OpenOptions::new().create(true).append(true).open(path) {
        let _ = writeln!(file, "{} {line}", unix_now());
    }
}

pub fn unix_now() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs() as i64)
}

/// Démarrage de Windows, en secondes Unix (à une seconde près).
pub fn boot_time() -> i64 {
    unix_now() - (unsafe { GetTickCount64() } / 1000) as i64
}

/// Heure et minute locales d'un instant (secondes Unix).
pub fn local_clock(unix: i64) -> Option<(u32, u32)> {
    let ticks = (unix + 11_644_473_600).checked_mul(10_000_000)? as u64;
    let file = FILETIME { dwLowDateTime: ticks as u32, dwHighDateTime: (ticks >> 32) as u32 };
    unsafe {
        let (mut utc, mut local): (SYSTEMTIME, SYSTEMTIME) = (std::mem::zeroed(), std::mem::zeroed());
        if FileTimeToSystemTime(&file, &mut utc) == 0 || SystemTimeToTzSpecificLocalTime(null(), &utc, &mut local) == 0 {
            return None;
        }
        Some((local.wHour as u32, local.wMinute as u32))
    }
}

/// Un verrou nommé de la session (`Local\…`), attendu au plus `wait_ms` ;
/// `None` s'il est tenu ailleurs. Rendu par [`release`].
pub fn lock_named(name: &str, wait_ms: u32) -> Option<HANDLE> {
    let name = wide(name);
    let handle = unsafe { CreateMutexW(null(), 0, name.as_ptr()) };
    if handle.is_null() {
        return None;
    }
    match unsafe { WaitForSingleObject(handle, wait_ms) } {
        WAIT_OBJECT_0 | WAIT_ABANDONED => Some(handle),
        _ => {
            unsafe { CloseHandle(handle) };
            None
        }
    }
}

pub fn release(handle: HANDLE) {
    unsafe {
        ReleaseMutex(handle);
        CloseHandle(handle);
    }
}

pub fn lock_workstation() {
    unsafe { LockWorkStation() };
}

// --- Crochets clavier et souris ---------------------------------------------

/// Où vont Entrée et Échap du clavier du PC (le cache).
static KEY_SINK: OnceLock<Box<dyn Fn(CoverKey) + Send + Sync>> = OnceLock::new();
/// Entrée et Échap physiques enfoncées : une seule annonce par appui (pas la répétition).
static ENTER_DOWN: AtomicBool = AtomicBool::new(false);
static ESCAPE_DOWN: AtomicBool = AtomicBool::new(false);

fn pressed(vk: u16) -> bool {
    unsafe { GetAsyncKeyState(vk as i32) < 0 }
}

unsafe extern "system" fn keyboard(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code == HC_ACTION as i32 {
        let k = unsafe { &*(lparam as *const KBDLLHOOKSTRUCT) };
        let injected = k.flags & LLKHF_INJECTED != 0;
        if let Some(key) = cover_key_of(injected, k.vkCode) {
            let down = matches!(wparam as u32, WM_KEYDOWN | WM_SYSKEYDOWN);
            let flag = if key == CoverKey::Enter { &ENTER_DOWN } else { &ESCAPE_DOWN };
            if down && !flag.swap(true, Ordering::SeqCst) {
                if let Some(sink) = KEY_SINK.get() {
                    sink(key);
                }
            } else if !down {
                flag.store(false, Ordering::SeqCst);
            }
            return 1;
        }
        // Alt : l'état que Windows donne avec la touche ; Ctrl et Maj : l'état
        // connu du système (les touches physiques avalées n'y comptent pas).
        let mods = Mods { alt: k.flags & LLKHF_ALTDOWN != 0, ctrl: pressed(VK_CONTROL), shift: pressed(VK_SHIFT) };
        if !keep(injected, k.vkCode, mods) {
            return 1;
        }
    }
    unsafe { CallNextHookEx(null_mut(), code, wparam, lparam) }
}

unsafe extern "system" fn mouse(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code == HC_ACTION as i32 {
        let m = unsafe { &*(lparam as *const MSLLHOOKSTRUCT) };
        // La souris du PC ne fait rien ; celle de l'ami (injectée par Kyber) passe.
        if m.flags & LLMHF_INJECTED == 0 {
            return 1;
        }
    }
    unsafe { CallNextHookEx(null_mut(), code, wparam, lparam) }
}

/// Les crochets bas niveau, sur leur propre fil (avec sa boucle de messages :
/// Windows retire sans rien dire un crochet qui tarde à répondre, au-delà de
/// `LowLevelHooksTimeout`). Le fil passe en priorité « temps critique », et
/// les crochets sont reposés toutes les 3 s (les nouveaux d'abord, puis les
/// anciens retirés : jamais de trou).
pub struct Hooks {
    thread: u32,
    join: Option<JoinHandle<()>>,
}

impl Hooks {
    /// `sink` reçoit Entrée et Échap du clavier du PC ; le premier appel le fixe.
    pub fn start(sink: impl Fn(CoverKey) + Send + Sync + 'static) -> Result<Hooks> {
        let _ = KEY_SINK.set(Box::new(sink));
        let (tx, rx) = mpsc::channel();
        let join = std::thread::spawn(move || unsafe {
            use windows_sys::Win32::System::Threading::{GetCurrentThread, SetThreadPriority, THREAD_PRIORITY_TIME_CRITICAL};
            use windows_sys::Win32::UI::WindowsAndMessaging::{KillTimer, SetTimer, WM_TIMER};
            SetThreadPriority(GetCurrentThread(), THREAD_PRIORITY_TIME_CRITICAL);
            let mut msg: MSG = std::mem::zeroed();
            // File de messages créée avant d'annoncer le fil (PostThreadMessage).
            PeekMessageW(&mut msg, null_mut(), 0, 0, PM_NOREMOVE);
            let module = GetModuleHandleW(null());
            let set = || (SetWindowsHookExW(WH_KEYBOARD_LL, Some(keyboard), module, 0), SetWindowsHookExW(WH_MOUSE_LL, Some(mouse), module, 0));
            let unset = |(kb, ms): (HHOOK, HHOOK)| {
                if !kb.is_null() {
                    UnhookWindowsHookEx(kb);
                }
                if !ms.is_null() {
                    UnhookWindowsHookEx(ms);
                }
            };
            let mut hooks: (HHOOK, HHOOK) = set();
            let _ = tx.send((GetCurrentThreadId(), !hooks.0.is_null() && !hooks.1.is_null()));
            let timer = SetTimer(null_mut(), 0, 3000, None);
            while GetMessageW(&mut msg, null_mut(), 0, 0) > 0 {
                if msg.message == WM_TIMER && msg.hwnd.is_null() {
                    // Reposés : un crochet retiré par Windows revient ; les
                    // nouveaux d'abord, les anciens ensuite (pas de trou).
                    let fresh = set();
                    if !fresh.0.is_null() && !fresh.1.is_null() {
                        unset(std::mem::replace(&mut hooks, fresh));
                    } else {
                        unset(fresh);
                    }
                    continue;
                }
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
            KillTimer(null_mut(), timer);
            unset(hooks);
        });
        let (thread, ok) = rx.recv()?;
        let hooks = Hooks { thread, join: Some(join) };
        if !ok {
            drop(hooks);
            bail!("crochets clavier et souris refusés ({})", std::io::Error::last_os_error());
        }
        Ok(hooks)
    }
}

impl Drop for Hooks {
    fn drop(&mut self) {
        unsafe { PostThreadMessageW(self.thread, WM_QUIT, 0, 0) };
        if let Some(join) = self.join.take() {
            let _ = join.join();
        }
        ENTER_DOWN.store(false, Ordering::SeqCst);
        ESCAPE_DOWN.store(false, Ordering::SeqCst);
    }
}

// --- Fenêtres ---------------------------------------------------------------

unsafe extern "system" fn collect(hwnd: HWND, data: LPARAM) -> windows_sys::core::BOOL {
    unsafe { (*(data as *mut Vec<HWND>)).push(hwnd) };
    1
}

/// Fenêtres de premier niveau, de la plus haute à la plus basse.
fn top_windows() -> Vec<HWND> {
    let mut list: Vec<HWND> = Vec::new();
    unsafe { EnumWindows(Some(collect), &mut list as *mut _ as LPARAM) };
    list
}

pub fn class(hwnd: HWND) -> String {
    let mut buf = [0u16; 128];
    let n = unsafe { GetClassNameW(hwnd, buf.as_mut_ptr(), buf.len() as i32) };
    String::from_utf16_lossy(&buf[..n.max(0) as usize])
}

pub fn pid_of(hwnd: HWND) -> u32 {
    let mut pid = 0u32;
    unsafe { GetWindowThreadProcessId(hwnd, &mut pid) };
    pid
}

fn cloaked(hwnd: HWND) -> bool {
    let mut value = 0u32;
    let hr = unsafe { DwmGetWindowAttribute(hwnd, DWMWA_CLOAKED as u32, (&mut value as *mut u32).cast::<c_void>(), 4) };
    hr == 0 && value != 0
}

pub fn rect(hwnd: HWND) -> RECT {
    let mut r: RECT = unsafe { std::mem::zeroed() };
    unsafe { GetWindowRect(hwnd, &mut r) };
    r
}

pub fn me() -> u32 {
    unsafe { GetCurrentProcessId() }
}

/// Fenêtres du bureau et de la barre des tâches, jamais touchées.
const SHELL: &[&str] = &["Progman", "WorkerW", "Shell_TrayWnd", "Shell_SecondaryTrayWnd", "Windows.UI.Core.CoreWindow"];

/// Une fenêtre qu'on voit, à nous ou à l'utilisateur (pas le bureau, pas
/// une fenêtre outil, pas une fenêtre d'app UWP en sommeil).
fn shown(hwnd: HWND) -> bool {
    let visible = unsafe { IsWindowVisible(hwnd) } != 0;
    let tool = (unsafe { GetWindowLongPtrW(hwnd, GWL_EXSTYLE) } as u32) & WS_EX_TOOLWINDOW != 0;
    visible && !tool && !cloaked(hwnd) && !SHELL.contains(&class(hwnd).as_str())
}

/// Réduit les fenêtres de l'utilisateur (sans les activer) ; leur place,
/// pour les rendre à la fin.
pub fn minimize_all() -> Vec<Window> {
    let me = me();
    let mut saved = Vec::new();
    for hwnd in top_windows() {
        let pid = pid_of(hwnd);
        if pid == me || !shown(hwnd) || !unsafe { GetWindow(hwnd, GW_OWNER) }.is_null() {
            continue;
        }
        let mut wp: WINDOWPLACEMENT = unsafe { std::mem::zeroed() };
        wp.length = size_of::<WINDOWPLACEMENT>() as u32;
        if unsafe { GetWindowPlacement(hwnd, &mut wp) } == 0 {
            continue;
        }
        let r = wp.rcNormalPosition;
        let placement = [wp.flags as i32, wp.showCmd as i32, wp.ptMinPosition.x, wp.ptMinPosition.y, wp.ptMaxPosition.x, wp.ptMaxPosition.y, r.left, r.top, r.right, r.bottom];
        if unsafe { IsIconic(hwnd) } == 0 {
            unsafe { ShowWindowAsync(hwnd, SW_SHOWMINNOACTIVE) };
        }
        saved.push(Window { hwnd: hwnd as i64, pid, placement });
    }
    saved
}

/// La fenêtre notée est-elle toujours la même ?
fn alive(w: &Window) -> Option<HWND> {
    let hwnd = w.hwnd as HWND;
    (unsafe { IsWindow(hwnd) } != 0 && pid_of(hwnd) == w.pid).then_some(hwnd)
}

/// Agent relancé en plein prêt : les fenêtres notées que l'utilisateur
/// aurait rouvertes sont réduites à nouveau.
pub fn minimize_again(saved: &[Window]) {
    for hwnd in saved.iter().filter_map(alive) {
        if unsafe { IsIconic(hwnd) } == 0 {
            unsafe { ShowWindowAsync(hwnd, SW_SHOWMINNOACTIVE) };
        }
    }
}

/// Rend aux fenêtres leur place, de la plus basse à la plus haute (l'ordre
/// d'avant le prêt) ; une fenêtre qui ne répond plus est laissée.
pub fn restore(saved: &[Window]) {
    for w in saved.iter().rev() {
        let Some(hwnd) = alive(w) else { continue };
        if unsafe { IsHungAppWindow(hwnd) } != 0 {
            continue;
        }
        let p = w.placement;
        let show = match p[1] {
            s if s == SW_SHOWNORMAL => SW_SHOWNOACTIVATE,
            s => s,
        };
        let wp = WINDOWPLACEMENT {
            length: size_of::<WINDOWPLACEMENT>() as u32,
            flags: p[0] as u32,
            showCmd: show as u32,
            ptMinPosition: POINT { x: p[2], y: p[3] },
            ptMaxPosition: POINT { x: p[4], y: p[5] },
            rcNormalPosition: RECT { left: p[6], top: p[7], right: p[8], bottom: p[9] },
        };
        unsafe { SetWindowPlacement(hwnd, &wp) };
    }
}

/// Barres des tâches (principale et secondaires) cachées ou rendues. À
/// refaire régulièrement : l'Explorateur les remontre quand les écrans changent.
pub fn taskbars(show: bool) {
    for hwnd in top_windows() {
        let c = class(hwnd);
        if c != "Shell_TrayWnd" && c != "Shell_SecondaryTrayWnd" {
            continue;
        }
        let visible = unsafe { IsWindowVisible(hwnd) } != 0;
        if visible != show {
            unsafe { ShowWindowAsync(hwnd, if show { SW_SHOWNA } else { SW_HIDE }) };
        }
    }
}

/// Curseur gardé sur l'écran de l'ami (`None` : libéré).
pub fn clip(display: Option<&Display>) {
    match display {
        Some(d) => {
            let r = RECT { left: d.x, top: d.y, right: d.x + d.width as i32, bottom: d.y + d.height as i32 };
            unsafe { ClipCursor(&r) };
        }
        None => unsafe {
            ClipCursor(null());
        },
    }
}

/// Le curseur au milieu de l'écran de l'ami.
pub fn center_cursor(d: &Display) {
    unsafe { SetCursorPos(d.x + d.width as i32 / 2, d.y + d.height as i32 / 2) };
}

unsafe extern "system" fn find_shell_view(hwnd: HWND, data: LPARAM) -> windows_sys::core::BOOL {
    if class(hwnd) == "SHELLDLL_DefView" {
        unsafe { *(data as *mut bool) = true };
        return 0;
    }
    1
}

/// Fenêtres « Ouvrir » et « Enregistrer sous » ouvertes (boîtes `#32770` qui
/// montrent des dossiers), hors des nôtres.
pub fn file_dialogs() -> HashSet<isize> {
    let me = me();
    top_windows()
        .into_iter()
        .filter(|&hwnd| unsafe { IsWindowVisible(hwnd) } != 0 && class(hwnd) == "#32770" && pid_of(hwnd) != me)
        .filter(|&hwnd| {
            let mut found = false;
            unsafe { EnumChildWindows(hwnd, Some(find_shell_view), &mut found as *mut bool as LPARAM) };
            found
        })
        .map(|hwnd| hwnd as isize)
        .collect()
}

/// Ferme (Annuler) celles qui n'étaient pas là au début du prêt.
pub fn close_file_dialogs(known: &HashSet<isize>) {
    for hwnd in file_dialogs().difference(known) {
        unsafe { PostMessageW(*hwnd as HWND, WM_CLOSE, 0, 0) };
    }
}

// --- Processus et fenêtres du prêt -------------------------------------------

/// Les processus nés pendant le prêt (apps permises, jeux, Steam de l'ami) :
/// ce sont leurs fenêtres que l'agent garde sur l'écran de l'ami et ferme à la fin.
pub struct Born {
    /// Début du prêt, en unités de FILETIME (100 ns depuis 1601).
    since: u64,
    cache: HashMap<u32, bool>,
}

impl Born {
    pub fn new(started_unix: i64) -> Born {
        Born { since: ((started_unix + 11_644_473_600).max(0) as u64) * 10_000_000, cache: HashMap::new() }
    }

    pub fn is_new(&mut self, pid: u32) -> bool {
        let since = self.since;
        *self.cache.entry(pid).or_insert_with(|| unsafe {
            let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
            if process.is_null() {
                return false;
            }
            let (mut created, mut a, mut b, mut c): (FILETIME, FILETIME, FILETIME, FILETIME) = std::mem::zeroed();
            let ok = GetProcessTimes(process, &mut created, &mut a, &mut b, &mut c) != 0;
            CloseHandle(process);
            ok && ((created.dwHighDateTime as u64) << 32 | created.dwLowDateTime as u64) >= since
        })
    }
}

/// Fenêtres visibles des processus du prêt (hors les nôtres).
pub fn loan_windows(born: &mut Born) -> Vec<HWND> {
    let me = me();
    top_windows()
        .into_iter()
        .filter(|&hwnd| {
            let pid = pid_of(hwnd);
            pid != me && shown(hwnd) && born.is_new(pid)
        })
        .collect()
}

/// Une fenêtre du prêt ouverte sur un écran du PC (caché) revient sur celui
/// de l'ami ; agrandie, elle est d'abord rétablie (le relevé suivant la déplace).
pub fn keep_on(display: &Display, born: &mut Born) {
    for hwnd in loan_windows(born) {
        if unsafe { IsIconic(hwnd) } != 0 {
            continue;
        }
        let r = rect(hwnd);
        let (w, h) = (r.right - r.left, r.bottom - r.top);
        if w <= 0 || h <= 0 || display.contains(r.left + w / 2, r.top + h / 2) {
            continue;
        }
        if unsafe { IsZoomed(hwnd) } != 0 {
            unsafe { ShowWindowAsync(hwnd, SW_RESTORE) };
            continue;
        }
        let x = display.x + ((display.width as i32 - w) / 2).max(0);
        let y = display.y + ((display.height as i32 - h) / 2).max(0);
        unsafe { SetWindowPos(hwnd, null_mut(), x, y, 0, 0, SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE | SWP_ASYNCWINDOWPOS) };
    }
}

/// Demande aux fenêtres du prêt de se fermer (`WM_CLOSE`) ; combien il en reste.
pub fn close_loan_windows(born: &mut Born) -> usize {
    let windows = loan_windows(born);
    for &hwnd in &windows {
        unsafe { PostMessageW(hwnd, WM_CLOSE, 0, 0) };
    }
    windows.len()
}

/// Processus de ce nom (« steam.exe ») dans la session de l'agent.
pub fn processes(name: &str) -> Vec<u32> {
    let mut session = 0u32;
    unsafe { ProcessIdToSessionId(me(), &mut session) };
    let mut found = Vec::new();
    unsafe {
        let snap = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
        if snap.is_null() || snap as isize == -1 {
            return found;
        }
        let mut entry: PROCESSENTRY32W = std::mem::zeroed();
        entry.dwSize = size_of::<PROCESSENTRY32W>() as u32;
        let mut ok = Process32FirstW(snap, &mut entry) != 0;
        while ok {
            let len = entry.szExeFile.iter().position(|c| *c == 0).unwrap_or(entry.szExeFile.len());
            let exe = String::from_utf16_lossy(&entry.szExeFile[..len]);
            let mut sid = u32::MAX;
            if exe.eq_ignore_ascii_case(name) && ProcessIdToSessionId(entry.th32ProcessID, &mut sid) != 0 && sid == session {
                found.push(entry.th32ProcessID);
            }
            ok = Process32NextW(snap, &mut entry) != 0;
        }
        CloseHandle(snap);
    }
    found
}

pub fn kill(pid: u32) {
    unsafe {
        let process = OpenProcess(PROCESS_TERMINATE, 0, pid);
        if !process.is_null() {
            TerminateProcess(process, 1);
            CloseHandle(process);
        }
    }
}

// --- Premier plan -------------------------------------------------------------

pub fn foreground() -> HWND {
    unsafe { GetForegroundWindow() }
}

/// Les fenêtres que l'ami peut avoir devant lui : sur son écran, pas réduites,
/// pas à nous (le lanceur se compare à part), pas le bureau.
pub fn acceptable_foreground(hwnd: HWND, display: &Display) -> bool {
    if hwnd.is_null() || pid_of(hwnd) == me() || unsafe { IsIconic(hwnd) } != 0 || SHELL.contains(&class(hwnd).as_str()) {
        return false;
    }
    let r = rect(hwnd);
    display.contains(r.left + (r.right - r.left) / 2, r.top + (r.bottom - r.top) / 2)
}

/// Met une de nos fenêtres au premier plan, même quand une autre app l'a
/// (Windows ne le permet qu'au processus du premier plan : on s'attache
/// un instant à son fil d'entrée).
pub fn bring_to_front(hwnd: HWND) {
    unsafe {
        let fg = GetForegroundWindow();
        let theirs = if fg.is_null() { 0 } else { GetWindowThreadProcessId(fg, null_mut()) };
        let mine = GetCurrentThreadId();
        let attached = theirs != 0 && theirs != mine && AttachThreadInput(mine, theirs, 1) != 0;
        BringWindowToTop(hwnd);
        SetForegroundWindow(hwnd);
        if attached {
            AttachThreadInput(mine, theirs, 0);
        }
    }
}

/// L'app qu'Admin va ouvrir pourra passer au premier plan (nous l'avons).
pub fn allow_foreground() {
    unsafe { AllowSetForegroundWindow(ASFW_ANY) };
}

/// Le cache reste au-dessus de tout (une app « toujours visible » le passerait).
pub fn topmost(hwnd: HWND) {
    unsafe { SetWindowPos(hwnd, HWND_TOPMOST, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_ASYNCWINDOWPOS) };
}
