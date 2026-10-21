//! Ce que l'installateur regarde avant d'installer (rien n'est modifié) :
//! Windows, processeur, carte graphique, écran, réseau, WebView2, et une
//! éventuelle version déjà installée.

use windows_sys::Win32::System::Registry::HKEY_LOCAL_MACHINE;

use crate::events::{CheckStatus, Emit, Event};
use crate::payload::Arch;
use crate::{sys, webview2};

/// Windows 10 22H2 : la plus ancienne version prise en charge.
const MIN_BUILD: u32 = 19045;
const DISPLAY_CLASS: &str = r"SYSTEM\CurrentControlSet\Control\Class\{4d36e968-e325-11ce-bfc1-08002be10318}";

pub fn run(emit: &Emit) {
    let check = |id: &str, title: &str, f: &dyn Fn() -> (CheckStatus, String, String)| {
        emit(Event::Check { id: id.into(), status: CheckStatus::Running, title: title.into(), detail: String::new() });
        let (status, title, detail) = f();
        sys::log(&format!("analyse {id} : {title} · {detail}"));
        emit(Event::Check { id: id.into(), status, title, detail });
    };

    let arch = sys::native_arch();
    check("windows", "Windows", &windows);
    check("arch", "Processeur", &|| processor(arch));
    check("gpu", "Carte graphique", &gpu);
    check("display", "Écran", &display);
    check("network", "Réseau local", &network);
    check("webview", "Moteur d'affichage", &|| match webview2::version() {
        Some(v) => (CheckStatus::Ok, "Moteur d'affichage WebView2".into(), format!("Version {v}, de Microsoft : il dessine l'interface de KyberNow.")),
        None => (CheckStatus::Warn, "Moteur d'affichage WebView2".into(), "Absent : il sera téléchargé et installé (environ 2 Mo).".into()),
    });

    let existing = sys::installed_version();
    if let Some(v) = &existing {
        check("existing", "Version installée", &|| {
            (CheckStatus::Info, format!("KyberNow {v} est installée"), "Elle sera mise à jour. Vos ordinateurs, leurs mots de passe et vos réglages restent.".into())
        });
    }
    let running = sys::app_running();
    if running {
        check("running", "KyberNow est ouverte", &|| {
            (CheckStatus::Warn, "KyberNow est ouverte".into(), "Elle sera fermée au moment d'installer : une session en cours s'arrêtera.".into())
        });
    }
    let desktop = sys::desktop_shortcut().is_some_and(|p| p.exists());
    emit(Event::ChecksDone { arch, existing, running, desktop, dir: sys::install_dir().display().to_string() });
}

fn windows() -> (CheckStatus, String, String) {
    let key = r"SOFTWARE\Microsoft\Windows NT\CurrentVersion";
    let build: u32 = sys::reg_get(HKEY_LOCAL_MACHINE, key, "CurrentBuild").and_then(|b| b.parse().ok()).unwrap_or(0);
    let release = sys::reg_get(HKEY_LOCAL_MACHINE, key, "DisplayVersion").unwrap_or_default();
    // ProductName dit « Windows 10 » même sur Windows 11 : le numéro fait foi.
    let name = if build >= 22000 { "Windows 11" } else { "Windows 10" };
    let title = if release.is_empty() { name.to_string() } else { format!("{name} · {release}") };
    if build != 0 && build < MIN_BUILD {
        (CheckStatus::Warn, title, format!("Build {build} : KyberNow est prévue pour Windows 10 22H2 (19045) ou plus récent. Faites les mises à jour de Windows."))
    } else {
        (CheckStatus::Ok, title, format!("Build {build}"))
    }
}

fn processor(arch: Arch) -> (CheckStatus, String, String) {
    match arch {
        Arch::Arm64 => (
            CheckStatus::Ok,
            "Processeur ARM64".into(),
            "La version ARM64 de KyberNow s'installe : elle tourne sans émulation, avec le décodage vidéo de la puce.".into(),
        ),
        Arch::X64 => (CheckStatus::Ok, "Processeur x64".into(), "La version x64 de KyberNow s'installe.".into()),
    }
}

/// Cartes graphiques connues de Windows (sans pilote ou de base : décodage par le processeur).
fn gpu() -> (CheckStatus, String, String) {
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
            let more = if real.len() > 1 { format!(" (et {} autre{})", real.len() - 1, if real.len() > 2 { "s" } else { "" }) } else { String::new() };
            (CheckStatus::Ok, "Carte graphique".into(), format!("{first}{more} : elle décode l'image, pour moins de latence et moins de batterie."))
        }
        None => (
            CheckStatus::Warn,
            "Carte graphique".into(),
            "Pilote de base de Windows : l'image sera décodée par le processeur. Installez le pilote de la carte graphique pour de meilleures performances.".into(),
        ),
    }
}

fn display() -> (CheckStatus, String, String) {
    use windows_sys::Win32::Graphics::Gdi::{DEVMODEW, ENUM_CURRENT_SETTINGS, EnumDisplaySettingsW};
    let mut mode: DEVMODEW = unsafe { std::mem::zeroed() };
    mode.dmSize = std::mem::size_of::<DEVMODEW>() as u16;
    if unsafe { EnumDisplaySettingsW(std::ptr::null(), ENUM_CURRENT_SETTINGS, &mut mode) } == 0 {
        return (CheckStatus::Ok, "Écran".into(), "Taille inconnue : KyberNow s'adaptera à la fenêtre.".into());
    }
    (
        CheckStatus::Ok,
        "Écran".into(),
        format!(
            "{} × {} · {} Hz : votre ordinateur peut prendre exactement cette taille pendant la session.",
            mode.dmPelsWidth, mode.dmPelsHeight, mode.dmDisplayFrequency
        ),
    )
}

fn network() -> (CheckStatus, String, String) {
    let ip = std::net::UdpSocket::bind("0.0.0.0:0").ok().and_then(|s| {
        s.connect("192.0.2.1:9").ok()?;
        Some(s.local_addr().ok()?.ip().to_string())
    });
    match ip {
        Some(ip) => (CheckStatus::Ok, "Réseau local".into(), format!("{ip} : un ordinateur sur ce réseau apparaît tout seul dans KyberNow.")),
        None => (CheckStatus::Warn, "Réseau local".into(), "Pas de réseau pour l'instant : branchez-vous avant de vous connecter.".into()),
    }
}
