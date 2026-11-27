//! Ce que l'app demande à Windows : journal, échelle d'affichage, écran,
//! identité de l'appareil, consoles connues, messages d'erreur.

use crate::sync::Locked;
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

/// La webcam choisie dans le menu « Caméra », gardée pour ce PC (docs/webcam.md).
pub fn camera_choice_path() -> PathBuf {
    data_dir("APPDATA").join("camera.txt")
}

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
        if let Some(f) = self.file.locked().as_mut() {
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
    if let Some(f) = &file {
        use std::os::windows::io::AsRawHandle;
        CRASH_LOG.store(f.as_raw_handle() as isize, std::sync::atomic::Ordering::Relaxed);
    }
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

/// La carte réseau que Windows prend pour joindre `host` (`GetBestInterfaceEx`),
/// `None` pour une adresse introuvable ou qu'on ne sait pas lire.
fn interface_to(host: &str) -> Option<windows::Win32::NetworkManagement::IpHelper::MIB_IF_ROW2> {
    use std::net::{IpAddr, ToSocketAddrs};
    use windows::Win32::NetworkManagement::IpHelper::{GetBestInterfaceEx, GetIfEntry2, MIB_IF_ROW2};
    use windows::Win32::Networking::WinSock::{AF_INET, AF_INET6, IN_ADDR, IN6_ADDR, SOCKADDR, SOCKADDR_IN, SOCKADDR_IN6};
    let host = host.trim_start_matches('[').trim_end_matches(']');
    let ip = (host, 0u16).to_socket_addrs().ok().and_then(|mut a| a.next()).map(|a| a.ip())?;
    let mut index = 0u32;
    let found = unsafe {
        match ip {
            IpAddr::V4(v4) => {
                let mut sa = SOCKADDR_IN { sin_family: AF_INET, ..Default::default() };
                sa.sin_addr = IN_ADDR { S_un: windows::Win32::Networking::WinSock::IN_ADDR_0 { S_addr: u32::from_ne_bytes(v4.octets()) } };
                GetBestInterfaceEx(&sa as *const SOCKADDR_IN as *const SOCKADDR, &mut index)
            }
            IpAddr::V6(v6) => {
                let mut sa = SOCKADDR_IN6 { sin6_family: AF_INET6, ..Default::default() };
                sa.sin6_addr = IN6_ADDR { u: windows::Win32::Networking::WinSock::IN6_ADDR_0 { Byte: v6.octets() } };
                GetBestInterfaceEx(&sa as *const SOCKADDR_IN6 as *const SOCKADDR, &mut index)
            }
        }
    };
    if found != 0 {
        return None;
    }
    let mut row = MIB_IF_ROW2 { InterfaceIndex: index, ..Default::default() };
    if unsafe { GetIfEntry2(&mut row) }.is_err() {
        return None;
    }
    Some(row)
}

/// La carte est filaire (Ethernet). Une carte Wi-Fi se dit IEEE 802.11 (71) ;
/// certaines cartes Wi-Fi ou mobiles se disent Ethernet : leur support
/// physique les trahit.
fn is_wired(row: &windows::Win32::NetworkManagement::IpHelper::MIB_IF_ROW2) -> bool {
    use windows::Win32::NetworkManagement::Ndis::{NdisPhysicalMedium802_3, NdisPhysicalMediumUnspecified};
    /// `IF_TYPE_ETHERNET_CSMACD` (ipifcons.h).
    const ETHERNET: u32 = 6;
    let medium = row.PhysicalMediumType;
    row.Type == ETHERNET && (medium == NdisPhysicalMedium802_3 || medium == NdisPhysicalMediumUnspecified)
}

/// PyroWave (docs/link-ui.md, « PyroWave ») : ce PC joint l'ordinateur
/// `host` par une carte réseau filaire (Ethernet), celle que Windows prend
/// pour cette adresse. Faux pour le Wi-Fi, le réseau mobile, une adresse
/// introuvable ou qu'on ne sait pas lire : « Auto » prend alors HEVC.
pub fn wired_to(host: &str) -> bool {
    let Some(row) = interface_to(host) else { return false };
    let wired = is_wired(&row);
    log::info!(
        "réseau vers {host} : carte {}, type {}, support {}, {}",
        row.InterfaceIndex,
        row.Type,
        row.PhysicalMediumType.0,
        if wired { "câble" } else { "sans fil" }
    );
    wired
}

/// Profil Auto (docs/profil-auto.md) : le nom du réseau par lequel ce PC
/// joint `host`, gardé avec le résultat du test et comparé au début de
/// chaque session : « Ethernet » par câble, le nom du Wi-Fi sinon (lu par
/// l'API WLAN ; Windows 11 le refuse quand la localisation est coupée pour
/// les apps de bureau). `None` : inconnu (une carte ni filaire ni Wi-Fi, un
/// nom refusé).
pub fn network_name(host: &str) -> Option<String> {
    let row = interface_to(host)?;
    if is_wired(&row) {
        return Some("Ethernet".into());
    }
    /// `IF_TYPE_IEEE80211` (ipifcons.h).
    const WIFI: u32 = 71;
    if row.Type != WIFI {
        return None;
    }
    let name = wifi_name(&row.InterfaceGuid);
    if name.is_none() {
        log::info!("profil Auto : nom du Wi-Fi illisible (localisation coupée ?)");
    }
    name
}

/// Le nom (SSID) du Wi-Fi auquel la carte `guid` est connectée.
fn wifi_name(guid: &windows::core::GUID) -> Option<String> {
    use windows::Win32::Foundation::HANDLE;
    use windows::Win32::NetworkManagement::WiFi::{
        WLAN_CONNECTION_ATTRIBUTES, WlanCloseHandle, WlanFreeMemory, WlanOpenHandle, WlanQueryInterface, wlan_intf_opcode_current_connection,
    };
    unsafe {
        let mut version = 0u32;
        let mut handle = HANDLE::default();
        if WlanOpenHandle(2, None, &mut version, &mut handle) != 0 {
            return None;
        }
        let mut size = 0u32;
        let mut data: *mut std::ffi::c_void = std::ptr::null_mut();
        let r = WlanQueryInterface(handle, guid, wlan_intf_opcode_current_connection, None, &mut size, &mut data, None);
        let name = if r == 0 && !data.is_null() && size as usize >= std::mem::size_of::<WLAN_CONNECTION_ATTRIBUTES>() {
            let attrs = &*(data as *const WLAN_CONNECTION_ATTRIBUTES);
            let ssid = &attrs.wlanAssociationAttributes.dot11Ssid;
            let len = (ssid.uSSIDLength as usize).min(ssid.ucSSID.len());
            let s = String::from_utf8_lossy(&ssid.ucSSID[..len]).trim().to_string();
            (!s.is_empty()).then_some(s)
        } else {
            None
        };
        if !data.is_null() {
            WlanFreeMemory(data);
        }
        WlanCloseHandle(handle, None);
        name
    }
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

/// Module (exécutable ou DLL) qui contient une adresse, et l'écart depuis son
/// début, écrits sans allocation.
fn module_of(addr: usize, out: &mut StackText) {
    use std::fmt::Write;
    use windows::Win32::Foundation::HMODULE;
    use windows::Win32::System::LibraryLoader::{
        GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS, GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT, GetModuleFileNameW,
        GetModuleHandleExW,
    };
    let mut module = HMODULE::default();
    let flags = GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS | GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT;
    unsafe {
        if GetModuleHandleExW(flags, PCWSTR(addr as *const u16), &mut module).is_err() {
            let _ = out.write_str("?");
            return;
        }
        let mut buf = [0u16; 260];
        let n = GetModuleFileNameW(Some(module), &mut buf) as usize;
        let start = buf[..n].iter().rposition(|&c| c == u16::from(b'\\')).map_or(0, |i| i + 1);
        for c in char::decode_utf16(buf[start..n].iter().copied()) {
            let _ = out.write_char(c.unwrap_or('?'));
        }
        let _ = write!(out, "+{:#x}", addr - module.0 as usize);
    }
}

/// Le fichier du journal, pour `on_crash` (gardé ouvert par le journal, qui
/// n'est jamais libéré).
static CRASH_LOG: std::sync::atomic::AtomicIsize = std::sync::atomic::AtomicIsize::new(0);

/// Texte dans un tampon fixe, sans allocation (au-delà, coupé).
struct StackText {
    buf: [u8; 512],
    len: usize,
}

impl std::fmt::Write for StackText {
    fn write_str(&mut self, s: &str) -> std::fmt::Result {
        let n = s.len().min(self.buf.len() - self.len);
        self.buf[self.len..self.len + n].copy_from_slice(&s.as_bytes()[..n]);
        self.len += n;
        Ok(())
    }
}

/// Plantage : une ligne écrite directement dans le fichier du journal, sans
/// verrou ni allocation (le plantage a pu arriver pendant une écriture du
/// journal, ou dans l'allocateur, sur ce même fil). Rien pour un débordement
/// de pile (plus de place pour rien faire).
unsafe extern "system" fn on_crash(info: *const windows::Win32::System::Diagnostics::Debug::EXCEPTION_POINTERS) -> i32 {
    use std::fmt::Write;
    const STACK_OVERFLOW: u32 = 0xC000_00FD;
    unsafe {
        let Some(rec) = info.as_ref().and_then(|i| i.ExceptionRecord.as_ref()) else { return 0 };
        let handle = CRASH_LOG.load(std::sync::atomic::Ordering::Relaxed);
        if rec.ExceptionCode.0 as u32 == STACK_OVERFLOW || handle == 0 {
            return 0;
        }
        let addr = rec.ExceptionAddress as usize;
        let mut line = StackText { buf: [0; 512], len: 0 };
        let _ = write!(line, "[ERROR] plantage : exception {:#010x} à {addr:#x} (", rec.ExceptionCode.0 as u32);
        module_of(addr, &mut line);
        let _ = writeln!(line, "), fil {}", windows::Win32::System::Threading::GetCurrentThreadId());
        let mut written = 0u32;
        let _ = windows::Win32::Storage::FileSystem::WriteFile(
            windows::Win32::Foundation::HANDLE(handle as *mut _),
            Some(&line.buf[..line.len]),
            Some(&mut written),
            None,
        );
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
    monitor_key_of(hwnd)
}

/// Le nom du périphérique (`\\.\DISPLAY2`) de l'écran où est la fenêtre `hwnd`.
pub fn monitor_key_of(hwnd: isize) -> Option<String> {
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

/// Le cadre d'une fenêtre de session (`WS_OVERLAPPEDWINDOW`) autour de cette
/// zone cliente, à l'échelle de l'écran où elle se pose (`at` : un point de
/// cet écran) : décalage du coin haut gauche du cadre par rapport à celui de
/// la zone cliente (négatif), puis taille du cadre.
pub fn session_frame(client: (i32, i32), at: (i32, i32)) -> (i32, i32, i32, i32) {
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
        (r.left, r.top, r.right - r.left, r.bottom - r.top)
    }
}

/// Le coin haut gauche du cadre d'une fenêtre de session dont la zone
/// cliente commence en `client_at` (pixels du bureau).
pub fn frame_origin(client_at: (i32, i32), client: (i32, i32)) -> (i32, i32) {
    let (left, top, _, _) = session_frame(client, client_at);
    (client_at.0 + left, client_at.1 + top)
}

/// L'écran (nom du périphérique) de la fenêtre active, où s'ouvre une session
/// de plus (plusieurs ordinateurs) : une fenêtre de l'app au premier plan
/// (une session, l'accueil, la barre) ; sinon `fallback`, la dernière fenêtre
/// de la session active ; sinon l'accueil.
pub fn active_monitor_key(fallback: isize) -> Option<String> {
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
    if anchor.0.is_null() {
        return None;
    }
    let mut info = MONITORINFOEXW::default();
    info.monitorInfo.cbSize = std::mem::size_of::<MONITORINFOEXW>() as u32;
    let ok = unsafe {
        let monitor = MonitorFromWindow(anchor, MONITOR_DEFAULTTONEAREST);
        GetMonitorInfoW(monitor, &mut info as *mut MONITORINFOEXW as *mut MONITORINFO).as_bool()
    };
    ok.then(|| {
        let len = info.szDevice.iter().position(|&c| c == 0).unwrap_or(info.szDevice.len());
        String::from_utf16_lossy(&info.szDevice[..len])
    })
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
    println!(
        "PyroWave   : {}",
        match crate::pyrowave::probe(&device) {
            Ok(()) => "carte graphique (Vulkan)".to_string(),
            Err(e) => format!("non ({e})"),
        }
    );
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
