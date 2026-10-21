//! Ce que l'app demande à Windows : journal, échelle d'affichage, écran,
//! identité de l'appareil, consoles connues, messages d'erreur.

use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::sync::Mutex;

use windows::Win32::Foundation::RECT;
use windows::Win32::Graphics::Gdi::{DEVMODEW, ENUM_CURRENT_SETTINGS, EnumDisplaySettingsW};
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

/// Journal dans la console et dans %LOCALAPPDATA%\KyberNow\kybernow.log.
pub fn init_logging() {
    let path = data_dir("LOCALAPPDATA").join("kybernow.log");
    let file = OpenOptions::new().create(true).write(true).truncate(true).open(&path).ok();
    let logger = Box::leak(Box::new(Logger { file: Mutex::new(file) }));
    let _ = log::set_logger(logger);
    log::set_max_level(log::LevelFilter::Info);
    log::info!("KyberNow {} ({})", env!("CARGO_PKG_VERSION"), std::env::consts::ARCH);
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
        MessageBoxW(None, PCWSTR(t.as_ptr()), w!("KyberNow"), MB_OK | MB_ICONERROR);
    }
}

/// Définition et fréquence de l'écran principal, en pixels réels.
pub fn screen_mode() -> Mode {
    let mut dm = DEVMODEW {
        dmSize: std::mem::size_of::<DEVMODEW>() as u16,
        ..Default::default()
    };
    let ok = unsafe { EnumDisplaySettingsW(PCWSTR::null(), ENUM_CURRENT_SETTINGS, &mut dm) }.as_bool();
    if !ok || dm.dmPelsWidth == 0 {
        return Mode { width: 1920, height: 1080, refresh: 60 };
    }
    Mode {
        width: dm.dmPelsWidth & !1,
        height: dm.dmPelsHeight & !1,
        refresh: dm.dmDisplayFrequency.max(30),
    }
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
        app: format!("KyberNow {}", env!("CARGO_PKG_VERSION")),
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
    println!("Son        : {} voies", crate::audio::default_output_channels());
    println!("Système    : {}", os_version());
    Ok(())
}
