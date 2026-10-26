//! Ce que l'app demande à Windows : journal, échelle d'affichage, écran,
//! identité de l'appareil, consoles connues, messages d'erreur.

use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::sync::Mutex;

use windows::Win32::Foundation::RECT;
use windows::Win32::Foundation::HWND;
use windows::Win32::Graphics::Gdi::{
    DEVMODEW, ENUM_CURRENT_SETTINGS, EnumDisplaySettingsW, GetMonitorInfoW, MONITOR_DEFAULTTONEAREST, MONITORINFO, MONITORINFOEXW, MonitorFromWindow,
};
use windows::Win32::System::Registry::{HKEY_LOCAL_MACHINE, RRF_RT_REG_SZ, RegGetValueW};
use windows::Win32::UI::HiDpi::{DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, SetProcessDpiAwarenessContext};
use windows::Win32::UI::WindowsAndMessaging::{
    MB_ICONERROR, MB_OK, MessageBoxW, SPI_GETWORKAREA, SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS, SystemParametersInfoW,
};
use windows::core::{PCWSTR, w};

use kybernow_core::console::{DeviceInfo, Mode};

fn data_dir(var: &str) -> PathBuf {
    let base = std::env::var_os(var).map(PathBuf::from).unwrap_or_else(std::env::temp_dir);
    let dir = base.join("KyberNow");
    let _ = fs::create_dir_all(&dir);
    dir
}

struct Logger {
    file: Mutex<Option<File>>,
}

impl log::Log for Logger {
    fn enabled(&self, m: &log::Metadata) -> bool {
        let t = m.target();
        let ours = t.starts_with("kybernow") || t.starts_with("kyclient") || t.starts_with("kynput");
        m.level() <= if ours { log::Level::Info } else { log::Level::Warn }
    }

    fn log(&self, r: &log::Record) {
        if !self.enabled(r.metadata()) {
            return;
        }
        let secs = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs_f64())
            .unwrap_or(0.0);
        let line = format!("{secs:.3} {} {}: {}\n", r.level(), r.target(), r.args());
        eprint!("{line}");
        if let Some(f) = self.file.lock().unwrap().as_mut() {
            let _ = f.write_all(line.as_bytes());
        }
    }

    fn flush(&self) {}
}

/// Journal dans la console et dans %LOCALAPPDATA%\KyberNow\<nom>.log ; celui
/// du lancement précédent est gardé dans <nom>.prev.log.
pub fn init_logging(name: &str) {
    let dir = data_dir("LOCALAPPDATA");
    let path = dir.join(format!("{name}.log"));
    let _ = fs::rename(&path, dir.join(format!("{name}.prev.log")));
    let file = OpenOptions::new().create(true).write(true).truncate(true).open(&path).ok();
    let logger = Box::leak(Box::new(Logger { file: Mutex::new(file) }));
    let _ = log::set_logger(logger);
    log::set_max_level(log::LevelFilter::Info);
    log::info!(
        "KyberNow Link {} ({}, {}) : {:?}",
        env!("CARGO_PKG_VERSION"),
        std::env::consts::ARCH,
        os_version(),
        std::env::args().skip(1).collect::<Vec<_>>()
    );
    if emulated() {
        log::warn!("version x64 émulée sur un Windows ARM64 : installez la version ARM64 (KyberNow-Link-Setup.exe la choisit)");
    }
    install_crash_handlers();
}

/// L'adresse est-elle celle de ce PC ? (boucle locale, ou une adresse de ses
/// cartes réseau : Windows n'accepte d'y ouvrir un port que si elle est à lui.)
pub fn is_this_pc(host: &str) -> bool {
    use std::net::{ToSocketAddrs, UdpSocket};
    let host = host.trim_start_matches('[').trim_end_matches(']');
    let Ok(addrs) = (host, 0u16).to_socket_addrs() else { return false };
    addrs.into_iter().any(|a| a.ip().is_loopback() || UdpSocket::bind((a.ip(), 0)).is_ok())
}

/// Cette app x64 tourne-t-elle émulée sur un processeur ARM64 ?
pub fn emulated() -> bool {
    use windows::Win32::System::SystemInformation::{IMAGE_FILE_MACHINE, IMAGE_FILE_MACHINE_ARM64};
    use windows::Win32::System::Threading::{GetCurrentProcess, IsWow64Process2};
    if cfg!(target_arch = "aarch64") {
        return false;
    }
    let (mut process, mut native) = (IMAGE_FILE_MACHINE::default(), IMAGE_FILE_MACHINE::default());
    unsafe { IsWow64Process2(GetCurrentProcess(), &mut process, Some(&mut native)) }.is_ok() && native == IMAGE_FILE_MACHINE_ARM64
}

/// Module (exécutable ou DLL) qui contient une adresse, et l'écart depuis son début.
fn module_of(addr: usize) -> String {
    use windows::Win32::Foundation::HMODULE;
    use windows::Win32::System::LibraryLoader::{
        GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS, GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT, GetModuleFileNameW,
        GetModuleHandleExW,
    };
    let mut module = HMODULE::default();
    let flags = GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS | GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT;
    unsafe {
        if GetModuleHandleExW(flags, PCWSTR(addr as *const u16), &mut module).is_err() {
            return "?".into();
        }
        let mut buf = [0u16; 260];
        let n = GetModuleFileNameW(Some(module), &mut buf) as usize;
        let name = String::from_utf16_lossy(&buf[..n]);
        let short = name.rsplit('\\').next().unwrap_or(&name).to_string();
        format!("{short}+{:#x}", addr - module.0 as usize)
    }
}

unsafe extern "system" fn on_crash(info: *const windows::Win32::System::Diagnostics::Debug::EXCEPTION_POINTERS) -> i32 {
    unsafe {
        if let Some(rec) = info.as_ref().and_then(|i| i.ExceptionRecord.as_ref()) {
            let addr = rec.ExceptionAddress as usize;
            log::error!(
                "plantage : exception {:#010x} à {addr:#x} ({}), fil « {} »",
                rec.ExceptionCode.0 as u32,
                module_of(addr),
                std::thread::current().name().unwrap_or("?")
            );
        }
    }
    0 // EXCEPTION_CONTINUE_SEARCH : Windows termine comme d'habitude.
}

/// Paniques et plantages notés dans le journal avant la fin du programme.
fn install_crash_handlers() {
    std::panic::set_hook(Box::new(|info| {
        log::error!(
            "panique dans le fil « {} » : {info}\n{}",
            std::thread::current().name().unwrap_or("?"),
            std::backtrace::Backtrace::force_capture()
        );
    }));
    unsafe {
        windows::Win32::System::Diagnostics::Debug::SetUnhandledExceptionFilter(Some(on_crash));
    }
}

/// Lancée depuis un terminal (ligne de commande) : écrire dans le sien.
pub fn attach_console() {
    unsafe {
        let _ = windows::Win32::System::Console::AttachConsole(windows::Win32::System::Console::ATTACH_PARENT_PROCESS);
    }
}

pub fn dpi_aware() {
    unsafe {
        let _ = SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
    }
}

pub fn error_box(text: &str) {
    log::error!("{text}");
    let t: Vec<u16> = text.encode_utf16().chain(Some(0)).collect();
    unsafe {
        MessageBoxW(None, PCWSTR(t.as_ptr()), w!("KyberNow Link"), MB_OK | MB_ICONERROR);
    }
}

/// La fenêtre de l'accueil (HWND), posée au démarrage de l'interface : son
/// écran est « Cet écran » (docs/multi-ecrans.md), comme sur Mac et Linux.
static HOME_WINDOW: std::sync::atomic::AtomicIsize = std::sync::atomic::AtomicIsize::new(0);

pub fn set_home_window(hwnd: isize) {
    HOME_WINDOW.store(hwnd, std::sync::atomic::Ordering::Relaxed);
}

/// « Cet écran » : le nom de périphérique (`\\.\DISPLAY2`) de l'écran où se
/// trouve la fenêtre de l'accueil ; `None` sans elle (ligne de commande) :
/// l'écran principal.
pub fn home_monitor_key() -> Option<String> {
    let hwnd = HOME_WINDOW.load(std::sync::atomic::Ordering::Relaxed);
    if hwnd == 0 {
        return None;
    }
    unsafe {
        let monitor = MonitorFromWindow(HWND(hwnd as *mut _), MONITOR_DEFAULTTONEAREST);
        let mut info = MONITORINFOEXW::default();
        info.monitorInfo.cbSize = std::mem::size_of::<MONITORINFOEXW>() as u32;
        if !GetMonitorInfoW(monitor, &mut info as *mut MONITORINFOEXW as *mut MONITORINFO).as_bool() {
            return None;
        }
        let len = info.szDevice.iter().position(|&c| c == 0).unwrap_or(info.szDevice.len());
        Some(String::from_utf16_lossy(&info.szDevice[..len]))
    }
}

/// Définition et fréquence de « Cet écran » (l'écran de la fenêtre de
/// l'accueil, sinon l'écran principal), en pixels réels.
pub fn screen_mode() -> Mode {
    let mut dm = DEVMODEW {
        dmSize: std::mem::size_of::<DEVMODEW>() as u16,
        ..Default::default()
    };
    let device: Option<Vec<u16>> = home_monitor_key().map(|k| k.encode_utf16().chain(Some(0)).collect());
    let name = device.as_ref().map_or(PCWSTR::null(), |d| PCWSTR(d.as_ptr()));
    let ok = unsafe { EnumDisplaySettingsW(name, ENUM_CURRENT_SETTINGS, &mut dm) }.as_bool();
    if !ok || dm.dmPelsWidth == 0 {
        return Mode { width: 1920, height: 1080, refresh: 60 };
    }
    Mode {
        width: dm.dmPelsWidth & !1,
        height: dm.dmPelsHeight & !1,
        refresh: dm.dmDisplayFrequency.max(30),
    }
}

/// Où s'ouvre la fenêtre d'une session ouverte pendant qu'une autre tourne
/// (plusieurs ordinateurs, docs/plusieurs-ordinateurs.md).
#[derive(Debug, Clone)]
pub struct FollowWindow {
    /// L'écran (nom du périphérique) : « Cet écran » de cette session.
    pub monitor: String,
    /// Coin haut gauche du cadre de la fenêtre, en pixels du bureau.
    pub at: (i32, i32),
    /// Zone cliente : la taille de l'écran de l'ordinateur pour cette session.
    pub client: (i32, i32),
    /// Fréquence de l'écran.
    pub refresh: u32,
}

/// Une fenêtre qu'on voit (pas une fenêtre cachée ou minuscule, comme celle
/// de l'icône près de l'horloge, à qui Windows rend parfois le premier plan).
fn seen(hwnd: HWND) -> bool {
    use windows::Win32::UI::WindowsAndMessaging::{GetWindowRect, IsIconic, IsWindowVisible};
    let mut r = RECT::default();
    unsafe {
        !hwnd.0.is_null()
            && IsWindowVisible(hwnd).as_bool()
            && !IsIconic(hwnd).as_bool()
            && GetWindowRect(hwnd, &mut r).is_ok()
            && r.right - r.left > 100
            && r.bottom - r.top > 60
    }
}

/// Taille du cadre d'une fenêtre de session (`WS_OVERLAPPEDWINDOW`) autour
/// de cette zone cliente, à l'échelle de l'écran où elle se pose (`at` : un
/// point de cet écran).
pub fn session_frame(client: (i32, i32), at: (i32, i32)) -> (i32, i32) {
    use windows::Win32::Foundation::POINT;
    use windows::Win32::Graphics::Gdi::{MONITOR_DEFAULTTONEAREST, MonitorFromPoint};
    use windows::Win32::UI::HiDpi::{AdjustWindowRectExForDpi, GetDpiForMonitor, MDT_EFFECTIVE_DPI};
    use windows::Win32::UI::WindowsAndMessaging::{WINDOW_EX_STYLE, WS_OVERLAPPEDWINDOW};
    unsafe {
        let monitor = MonitorFromPoint(POINT { x: at.0, y: at.1 }, MONITOR_DEFAULTTONEAREST);
        let (mut dx, mut dy) = (96u32, 96u32);
        let _ = GetDpiForMonitor(monitor, MDT_EFFECTIVE_DPI, &mut dx, &mut dy);
        let mut r = RECT { left: 0, top: 0, right: client.0, bottom: client.1 };
        let _ = AdjustWindowRectExForDpi(&mut r, WS_OVERLAPPEDWINDOW, false, WINDOW_EX_STYLE(0), dx.max(48));
        (r.right - r.left, r.bottom - r.top)
    }
}

/// La fenêtre d'une session suivante : sur l'écran de la fenêtre active
/// (une fenêtre de l'app au premier plan : une session, l'accueil ; sinon
/// `fallback`, la dernière fenêtre de la session active ; sinon l'accueil),
/// centrée dans son espace de travail, aux deux tiers en 16:9.
pub fn follow_window(fallback: isize) -> FollowWindow {
    use windows::Win32::System::Threading::GetCurrentProcessId;
    use windows::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, GetWindowThreadProcessId};
    let fg = unsafe { GetForegroundWindow() };
    let mut pid = 0u32;
    unsafe {
        GetWindowThreadProcessId(fg, Some(&mut pid));
    }
    let ours = pid == unsafe { GetCurrentProcessId() } && seen(fg);
    let fallback = HWND(fallback as *mut _);
    let home = HWND(HOME_WINDOW.load(std::sync::atomic::Ordering::Relaxed) as *mut _);
    let anchor = if ours {
        fg
    } else if seen(fallback) {
        fallback
    } else {
        home
    };
    let mut info = MONITORINFOEXW::default();
    info.monitorInfo.cbSize = std::mem::size_of::<MONITORINFOEXW>() as u32;
    let ok = unsafe {
        let monitor = MonitorFromWindow(anchor, MONITOR_DEFAULTTONEAREST);
        GetMonitorInfoW(monitor, &mut info as *mut MONITORINFOEXW as *mut MONITORINFO).as_bool()
    };
    let (work, monitor) = if ok {
        let r = info.monitorInfo.rcWork;
        let len = info.szDevice.iter().position(|&c| c == 0).unwrap_or(info.szDevice.len());
        ((r.left, r.top, r.right - r.left, r.bottom - r.top), String::from_utf16_lossy(&info.szDevice[..len]))
    } else {
        ((0, 0, 1920, 1040), String::new())
    };
    let client = crate::displays::follow_size(work.2, work.3);
    let outer = session_frame(client, (work.0 + work.2 / 2, work.1 + work.3 / 2));
    let at = crate::displays::centered(work, outer);
    let mut dm = DEVMODEW { dmSize: std::mem::size_of::<DEVMODEW>() as u16, ..Default::default() };
    let refresh = if ok && unsafe { EnumDisplaySettingsW(PCWSTR(info.szDevice.as_ptr()), ENUM_CURRENT_SETTINGS, &mut dm) }.as_bool() && dm.dmDisplayFrequency > 1 {
        dm.dmDisplayFrequency.max(30)
    } else {
        60
    };
    FollowWindow { monitor, at, client, refresh }
}

/// Fenêtre aux proportions de l'image, à 80 % de l'espace de travail.
pub fn initial_window_size(mode: Mode) -> (i32, i32) {
    let mut r = RECT::default();
    unsafe {
        let _ = SystemParametersInfoW(SPI_GETWORKAREA, 0, Some(&mut r as *mut _ as *mut _), SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0));
    }
    let (aw, ah) = (((r.right - r.left) as f32 * 0.8).max(640.0), ((r.bottom - r.top) as f32 * 0.8).max(360.0));
    let scale = (aw / mode.width as f32).min(ah / mode.height as f32);
    ((mode.width as f32 * scale) as i32, (mode.height as f32 * scale) as i32)
}

fn registry_string(name: PCWSTR) -> Option<String> {
    let mut buf = [0u16; 128];
    let mut size = (buf.len() * 2) as u32;
    let r = unsafe {
        RegGetValueW(
            HKEY_LOCAL_MACHINE,
            w!("SOFTWARE\\Microsoft\\Windows NT\\CurrentVersion"),
            name,
            RRF_RT_REG_SZ,
            None,
            Some(buf.as_mut_ptr() as *mut _),
            Some(&mut size),
        )
    };
    if r.is_err() {
        return None;
    }
    let n = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    Some(String::from_utf16_lossy(&buf[..n]))
}

/// « Windows 11 24H2 » (le registre dit encore « Windows 10 » sur Windows 11).
pub fn os_version() -> String {
    let build: u32 = registry_string(w!("CurrentBuild")).and_then(|b| b.parse().ok()).unwrap_or(0);
    let name = if build >= 22000 { "Windows 11" } else { "Windows 10" };
    match registry_string(w!("DisplayVersion")) {
        Some(v) if !v.is_empty() => format!("{name} {v}"),
        _ => name.to_string(),
    }
}

/// Identifiant stable de cet appareil pour la console (tiré une fois).
fn device_id() -> String {
    let path = data_dir("APPDATA").join("device-id");
    if let Ok(id) = fs::read_to_string(&path) {
        let id = id.trim().to_string();
        if !id.is_empty() {
            return id;
        }
    }
    let mut b = [0u8; 16];
    let _ = getrandom::getrandom(&mut b);
    b[6] = (b[6] & 0x0f) | 0x40;
    b[8] = (b[8] & 0x3f) | 0x80;
    let h: String = b.iter().map(|x| format!("{x:02X}")).collect();
    let id = format!("{}-{}-{}-{}-{}", &h[0..8], &h[8..12], &h[12..16], &h[16..20], &h[20..32]);
    let _ = fs::write(&path, &id);
    id
}

pub fn device_info() -> DeviceInfo {
    DeviceInfo {
        id: device_id(),
        name: std::env::var("COMPUTERNAME").unwrap_or_else(|_| "PC".into()),
        kind: "windows".into(),
        os: os_version(),
        app: format!("KyberNow Link {}", env!("CARGO_PKG_VERSION")),
    }
}

fn consoles_file() -> PathBuf {
    data_dir("APPDATA").join("consoles.txt")
}

/// Empreinte retenue pour une console (`hôte:port empreinte` par ligne).
pub fn known_console(key: &str) -> Option<String> {
    let text = fs::read_to_string(consoles_file()).ok()?;
    text.lines().find_map(|l| {
        let (k, fp) = l.split_once(' ')?;
        (k == key).then(|| fp.trim().to_string())
    })
}

pub fn remember_console(key: &str, fp: &str) {
    let mut lines: Vec<String> = fs::read_to_string(consoles_file())
        .unwrap_or_default()
        .lines()
        .filter(|l| !l.starts_with(&format!("{key} ")))
        .map(str::to_owned)
        .collect();
    lines.push(format!("{key} {fp}"));
    let _ = fs::write(consoles_file(), lines.join("\n") + "\n");
}

/// `--probe` : ce que la carte graphique décode.
pub fn probe_only() -> Result<(), String> {
    let (device, _ctx) = crate::render::create_device().map_err(|e| format!("Direct3D 11 : {e}"))?;
    let p = crate::decoder::probe(&device);
    let yes = |b: bool| if b { "carte graphique" } else { "processeur" };
    println!("H.264      : {}", yes(p.h264));
    println!("HEVC       : {}", yes(p.caps.hevc));
    println!("HEVC 4:4:4 : {}", yes(p.caps.hevc444));
    println!("AV1        : {}", if p.caps.av1 { "carte graphique" } else { "non (pas de décodage AV1 par le processeur)" });
    let m = screen_mode();
    println!("Écran      : {} × {} à {} Hz", m.width, m.height, m.refresh);
    // Les écrans de ce PC, tels que le cœur les reçoit pour le multi-écrans.
    for (i, d) in crate::displays::enumerate().iter().enumerate() {
        println!(
            "Écran {}    : {} × {} à {} Hz en ({}, {}), échelle {} %{}",
            i + 1,
            d.width,
            d.height,
            d.refresh,
            d.x,
            d.y,
            d.dpi * 100 / 96,
            if d.primary { ", principal" } else { "" }
        );
    }
    println!("Son        : {} voies", crate::audio::default_output_channels());
    println!("Système    : {}", os_version());
    Ok(())
}
