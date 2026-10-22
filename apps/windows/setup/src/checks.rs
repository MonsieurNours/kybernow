//! Ce que l'installateur regarde avant d'installer (rien n'est modifié) :
//! Windows, processeur, carte graphique, écran, réseau, WebView2, et une
//! éventuelle version déjà installée.

use windows_sys::Win32::System::Registry::HKEY_LOCAL_MACHINE;

use crate::events::{CheckStatus, Emit, Event};
use crate::i18n::{Lang, lang, tr};
use crate::payload::Arch;
use crate::{sys, webview2};

fn t(l: Lang, key: &str) -> String {
    tr(l, key, &[])
}

/// Windows 10 22H2 : la plus ancienne version prise en charge.
const MIN_BUILD: u32 = 19045;
const DISPLAY_CLASS: &str = r"SYSTEM\CurrentControlSet\Control\Class\{4d36e968-e325-11ce-bfc1-08002be10318}";

pub fn run(emit: &Emit) {
    let l = lang();
    // À l'écran dans la langue de Windows ; au journal, en français.
    let check = |id: &str, title: &str, f: &dyn Fn(Lang) -> (CheckStatus, String, String)| {
        emit(Event::Check { id: id.into(), status: CheckStatus::Running, title: title.into(), detail: String::new() });
        let (status, title, detail) = f(l);
        let (_, fr_title, fr_detail) = if l == Lang::Fr { (status, title.clone(), detail.clone()) } else { f(Lang::Fr) };
        sys::log(&format!("analyse {id} : {fr_title} · {fr_detail}"));
        emit(Event::Check { id: id.into(), status, title, detail });
    };

    let arch = sys::native_arch();
    check("windows", &t(l, "checks.windows.title"), &windows);
    check("arch", &t(l, "checks.arch.title"), &|l| processor(l, arch));
    check("gpu", &t(l, "checks.gpu.title"), &gpu);
    check("display", &t(l, "checks.display.title"), &display);
    check("network", &t(l, "checks.network.title"), &network);
    check("webview", &t(l, "checks.webview.title"), &|l| match webview2::version() {
        Some(v) => (CheckStatus::Ok, t(l, "checks.webview.fullTitle"), tr(l, "checks.webview.ok", &[("version", &v)])),
        None => (CheckStatus::Warn, t(l, "checks.webview.fullTitle"), t(l, "checks.webview.missing")),
    });

    let existing = sys::installed_version();
    if let Some(v) = &existing {
        check("existing", &t(l, "checks.existing.label"), &|l| {
            (CheckStatus::Info, tr(l, "checks.existing.title", &[("version", v)]), t(l, "checks.existing.detail"))
        });
    }
    let running = sys::app_running();
    if running {
        check("running", &t(l, "checks.running.title"), &|l| (CheckStatus::Warn, t(l, "checks.running.title"), t(l, "checks.running.detail")));
    }
    let desktop = sys::has_desktop_shortcut();
    emit(Event::ChecksDone { arch, existing, running, desktop, dir: sys::install_dir().display().to_string() });
}

fn windows(l: Lang) -> (CheckStatus, String, String) {
    let key = r"SOFTWARE\Microsoft\Windows NT\CurrentVersion";
    let build: u32 = sys::reg_get(HKEY_LOCAL_MACHINE, key, "CurrentBuild").and_then(|b| b.parse().ok()).unwrap_or(0);
    let release = sys::reg_get(HKEY_LOCAL_MACHINE, key, "DisplayVersion").unwrap_or_default();
    // ProductName dit « Windows 10 » même sur Windows 11 : le numéro fait foi.
    let name = if build >= 22000 { "Windows 11" } else { "Windows 10" };
    let title = if release.is_empty() { name.to_string() } else { format!("{name} · {release}") };
    if build != 0 && build < MIN_BUILD {
        (CheckStatus::Warn, title, tr(l, "checks.windows.old", &[("build", &build)]))
    } else {
        (CheckStatus::Ok, title, tr(l, "checks.windows.build", &[("build", &build)]))
    }
}

fn processor(l: Lang, arch: Arch) -> (CheckStatus, String, String) {
    match arch {
        Arch::Arm64 => (CheckStatus::Ok, t(l, "checks.arch.arm64Title"), t(l, "checks.arch.arm64")),
        Arch::X64 => (CheckStatus::Ok, t(l, "checks.arch.x64Title"), t(l, "checks.arch.x64")),
    }
}

/// Cartes graphiques connues de Windows (sans pilote ou de base : décodage par le processeur).
fn gpu(l: Lang) -> (CheckStatus, String, String) {
    let mut names: Vec<String> = sys::reg_subkeys(HKEY_LOCAL_MACHINE, DISPLAY_CLASS)
        .into_iter()
        .filter(|k| k.len() == 4 && k.chars().all(|c| c.is_ascii_digit()))
        .filter_map(|k| sys::reg_get(HKEY_LOCAL_MACHINE, &format!(r"{DISPLAY_CLASS}\{k}"), "DriverDesc"))
        .collect();
    names.dedup();
    let basic = |n: &String| n.contains("Microsoft Basic") || n.contains("Microsoft Remote Display");
    let real: Vec<&String> = names.iter().filter(|n| !basic(n)).collect();
    match real.first() {
        Some(first) => {
            let more = match real.len() {
                1 => String::new(),
                2 => t(l, "checks.gpu.more1"),
                n => tr(l, "checks.gpu.moreN", &[("count", &(n - 1))]),
            };
            (CheckStatus::Ok, t(l, "checks.gpu.title"), tr(l, "checks.gpu.ok", &[("name", first), ("more", &more)]))
        }
        None => (CheckStatus::Warn, t(l, "checks.gpu.title"), t(l, "checks.gpu.basic")),
    }
}

fn display(l: Lang) -> (CheckStatus, String, String) {
    use windows_sys::Win32::Graphics::Gdi::{DEVMODEW, ENUM_CURRENT_SETTINGS, EnumDisplaySettingsW};
    let mut mode: DEVMODEW = unsafe { std::mem::zeroed() };
    mode.dmSize = std::mem::size_of::<DEVMODEW>() as u16;
    if unsafe { EnumDisplaySettingsW(std::ptr::null(), ENUM_CURRENT_SETTINGS, &mut mode) } == 0 {
        return (CheckStatus::Ok, t(l, "checks.display.title"), t(l, "checks.display.unknown"));
    }
    let (width, height, hz) = (mode.dmPelsWidth, mode.dmPelsHeight, mode.dmDisplayFrequency);
    (CheckStatus::Ok, t(l, "checks.display.title"), tr(l, "checks.display.ok", &[("width", &width), ("height", &height), ("hz", &hz)]))
}

fn network(l: Lang) -> (CheckStatus, String, String) {
    let ip = std::net::UdpSocket::bind("0.0.0.0:0").ok().and_then(|s| {
        s.connect("192.0.2.1:9").ok()?;
        Some(s.local_addr().ok()?.ip().to_string())
    });
    match ip {
        Some(ip) => (CheckStatus::Ok, t(l, "checks.network.title"), tr(l, "checks.network.ok", &[("ip", &ip)])),
        None => (CheckStatus::Warn, t(l, "checks.network.title"), t(l, "checks.network.none")),
    }
}
