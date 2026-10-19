//! Diagnostic de l'ordinateur avant installation. Rien n'est modifié ici.

use std::time::Duration;

use serde::Serialize;

use crate::events::{BusyPort, CheckStatus, Emit, Event};
use crate::ports;

#[derive(Debug, Clone, Default, Serialize)]
pub struct SystemInfo {
    pub encoder: String,
    pub lan_ip: Option<String>,
    /// Index de l'interface réseau classée « Public », à passer en « Privé ».
    pub public_interface: Option<u32>,
    /// Installation Kyber officielle à retirer (commande de désinstallation).
    pub kyber_uninstaller: Option<String>,
    pub kyber_service_exists: bool,
    pub power_needs_tuning: bool,
    /// Pilote des haut-parleurs virtuels de Steam (Steam Streaming Speakers).
    pub steam_speakers_inf: Option<std::path::PathBuf>,
    pub stream_port: u16,
    pub console_port: u16,
}

struct Item {
    id: &'static str,
    status: CheckStatus,
    title: String,
    detail: String,
}

fn emit_item(emit: &Emit, item: Item) {
    emit(Event::Check { id: item.id.into(), status: item.status, title: item.title, detail: item.detail });
}

fn running(emit: &Emit, id: &str, title: &str) {
    emit(Event::Check { id: id.into(), status: CheckStatus::Running, title: title.into(), detail: String::new() });
    // Laisse le temps à l'animation de montrer chaque contrôle.
    std::thread::sleep(Duration::from_millis(450));
}

pub fn run(emit: &Emit) -> SystemInfo {
    let mut info = SystemInfo::default();

    running(emit, "gpu", "Carte graphique NVIDIA");
    let gpu = gpu();
    match &gpu {
        Some((name, _)) => {
            info.encoder = "nvenc".into();
            emit_item(emit, Item { id: "gpu", status: CheckStatus::Ok, title: "Carte graphique NVIDIA".into(), detail: format!("{name} · encodeur matériel NVENC disponible") });
        }
        None => {
            info.encoder = "x264".into();
            emit_item(emit, Item { id: "gpu", status: CheckStatus::Warn, title: "Aucune carte NVIDIA détectée".into(), detail: "L'encodage se fera par le processeur (x264) : plus lent et plus gourmand.".into() });
        }
    }

    running(emit, "driver", "Pilote graphique");
    match gpu.and_then(|(_, v)| v) {
        Some(v) => emit_item(emit, Item { id: "driver", status: CheckStatus::Ok, title: "Pilote graphique".into(), detail: format!("Version {v}") }),
        None => emit_item(emit, Item { id: "driver", status: CheckStatus::Warn, title: "Pilote graphique".into(), detail: "Version inconnue : pensez à mettre à jour le pilote NVIDIA.".into() }),
    }

    running(emit, "network", "Réseau local");
    info.lan_ip = lan_ip();
    let net = network(info.lan_ip.as_deref());
    let ip = info.lan_ip.clone().unwrap_or_else(|| "adresse inconnue".into());
    let (status, mut detail) = match net.as_ref().map(|n| n.media.as_str()) {
        Some("ethernet") => (CheckStatus::Ok, format!("Ethernet · {ip}")),
        Some("wifi") => (CheckStatus::Warn, format!("Wi-Fi · {ip} · un câble Ethernet évite les micro-coupures")),
        _ => (CheckStatus::Ok, ip.clone()),
    };
    let mut status = status;
    if let Some(n) = net.as_ref().filter(|n| n.public) {
        info.public_interface = Some(n.index);
        status = CheckStatus::Warn;
        detail.push_str(" · réseau classé « Public » : nous le passerons en « Privé » pour que vos appareils puissent se connecter");
    }
    emit_item(emit, Item { id: "network", status, title: "Réseau local".into(), detail });

    running(emit, "ports", "Ports réseau");
    let excluded = ports::excluded_ranges();
    let busy = (!ports::is_free(ports::KYBER_DEFAULT_PORT, &excluded))
        .then(|| BusyPort { port: ports::KYBER_DEFAULT_PORT, process: ports::owner(ports::KYBER_DEFAULT_PORT) });
    info.stream_port = ports::pick_random(&excluded, &[]);
    info.console_port = ports::pick_random(&excluded, &[info.stream_port]);
    let reason = match &busy {
        Some(b) => format!(
            "Le port habituel 8080 est déjà utilisé{}. ",
            b.process.as_ref().map(|p| format!(" par {p}")).unwrap_or_default()
        ),
        None => String::new(),
    };
    emit_item(emit, Item {
        id: "ports",
        status: CheckStatus::Ok,
        title: "Ports réseau".into(),
        detail: format!("{reason}Tirés au hasard et vérifiés libres : stream {} · console {}", info.stream_port, info.console_port),
    });

    running(emit, "display", "Écran principal");
    match display_mode() {
        Some((w, h, hz)) => emit_item(emit, Item { id: "display", status: CheckStatus::Ok, title: "Écran principal".into(), detail: format!("{w} × {h} · {hz} Hz") }),
        None => emit_item(emit, Item { id: "display", status: CheckStatus::Warn, title: "Écran principal".into(), detail: "Aucun écran détecté : branchez un écran ou activez l'écran virtuel.".into() }),
    }

    running(emit, "power", "Mode d'alimentation");
    let (plan, tuned) = power_plan();
    info.power_needs_tuning = !tuned;
    if tuned {
        emit_item(emit, Item { id: "power", status: CheckStatus::Ok, title: format!("Mode d'alimentation : {plan}"), detail: "Le GPU garde toute sa puissance pendant le stream.".into() });
    } else {
        emit_item(emit, Item { id: "power", status: CheckStatus::Warn, title: format!("Mode d'alimentation : {plan}"), detail: "Le GPU ralentit pour économiser l'énergie, ce qui peut tripler le temps d'encodage. Nous proposerons de le régler à l'étape Options.".into() });
    }

    running(emit, "audio", "Son à distance");
    info.steam_speakers_inf = steam_speakers_inf();
    if info.steam_speakers_inf.is_some() {
        emit_item(emit, Item { id: "audio", status: CheckStatus::Ok, title: "Son à distance".into(), detail: "Haut-parleurs virtuels de Steam disponibles : pendant le stream, l'ordinateur se tait et le son part en stéréo, 5.1 ou 7.1.".into() });
    } else {
        emit_item(emit, Item { id: "audio", status: CheckStatus::Warn, title: "Son à distance".into(), detail: "Steam n'est pas installé : le son sortira aussi des haut-parleurs de l'ordinateur, au format de Windows. Installez Steam puis relancez l'installation pour en profiter.".into() });
    }

    let (uninstaller, service) = existing_kyber();
    info.kyber_uninstaller = uninstaller.clone();
    info.kyber_service_exists = service;
    if uninstaller.is_some() || service {
        running(emit, "existing", "Installation existante");
        emit_item(emit, Item { id: "existing", status: CheckStatus::Warn, title: "Une installation de Kyber existe".into(), detail: "Elle sera retirée puis remplacée par celle de KyberNow (même moteur, réglé pour vous).".into() });
    }

    emit(Event::ChecksDone {
        stream_port: info.stream_port,
        console_port: info.console_port,
        busy,
        lan_ip: info.lan_ip.clone(),
        steam_speakers: info.steam_speakers_inf.is_some(),
    });
    info
}

/// Pilote des haut-parleurs virtuels livré avec Steam (même emplacement que
/// celui qu'utilise Sunshine), signé par Microsoft.
pub fn steam_speakers_inf() -> Option<std::path::PathBuf> {
    let common = std::env::var_os("CommonProgramFiles(x86)").or_else(|| std::env::var_os("CommonProgramFiles"))?;
    let base = std::path::PathBuf::from(common).join("Steam").join("drivers");
    ["Windows10", "Windows"]
        .iter()
        .map(|dir| base.join(dir).join("x64").join("SteamStreamingSpeakers.inf"))
        .find(|p| p.is_file())
}

/// (nom du GPU, version du pilote)
fn gpu() -> Option<(String, Option<String>)> {
    let nvml = nvml_wrapper::Nvml::init().ok()?;
    let device = nvml.device_by_index(0).ok()?;
    Some((device.name().ok()?, nvml.sys_driver_version().ok()))
}

pub fn lan_ip() -> Option<String> {
    let socket = std::net::UdpSocket::bind("0.0.0.0:0").ok()?;
    socket.connect("192.0.2.1:9").ok()?;
    Some(socket.local_addr().ok()?.ip().to_string())
}

struct Network {
    media: String,
    public: bool,
    index: u32,
}

fn network(ip: Option<&str>) -> Option<Network> {
    if !cfg!(windows) {
        return Some(Network { media: "ethernet".into(), public: false, index: 0 });
    }
    let ip = ip?;
    let script = format!(
        "$a = Get-NetIPAddress -AddressFamily IPv4 -IPAddress '{ip}' -ErrorAction SilentlyContinue | Select-Object -First 1; \
         if ($a) {{ $n = Get-NetAdapter -InterfaceIndex $a.InterfaceIndex; \
         $p = Get-NetConnectionProfile -InterfaceIndex $a.InterfaceIndex -ErrorAction SilentlyContinue; \
         \"$($n.PhysicalMediaType)|$($p.NetworkCategory)|$($a.InterfaceIndex)\" }}"
    );
    let out = crate::sys::powershell(&script).ok()?;
    let mut parts = out.split('|');
    let media = parts.next()?.to_lowercase();
    let category = parts.next().unwrap_or_default();
    let index = parts.next()?.trim().parse().ok()?;
    Some(Network {
        media: if media.contains("802.11") { "wifi".into() } else if media.contains("802.3") { "ethernet".into() } else { media },
        public: category.eq_ignore_ascii_case("Public"),
        index,
    })
}

#[cfg(windows)]
fn display_mode() -> Option<(u32, u32, u32)> {
    use windows_sys::Win32::Graphics::Gdi::{DEVMODEW, ENUM_CURRENT_SETTINGS, EnumDisplaySettingsW};
    unsafe {
        let mut mode: DEVMODEW = std::mem::zeroed();
        mode.dmSize = std::mem::size_of::<DEVMODEW>() as u16;
        if EnumDisplaySettingsW(std::ptr::null(), ENUM_CURRENT_SETTINGS, &mut mode) == 0 {
            return None;
        }
        Some((mode.dmPelsWidth, mode.dmPelsHeight, mode.dmDisplayFrequency))
    }
}

#[cfg(not(windows))]
fn display_mode() -> Option<(u32, u32, u32)> {
    Some((3840, 2160, 160))
}

/// (nom du mode, déjà réglé pour la performance ?)
fn power_plan() -> (String, bool) {
    const HIGH: &str = "8c5e7fda-e8bf-4a96-9a85-a6e23a8c635c";
    const ULTIMATE: &str = "e9a42b02-d5df-448d-aa00-03f14749eb61";
    const SAVER: &str = "a1841308-3541-4fab-bc81-f71556f20b4a";
    if !cfg!(windows) {
        return ("Équilibré".into(), false);
    }
    let Ok(out) = crate::sys::run("powercfg", &["/getactivescheme"]) else {
        return ("inconnu".into(), true);
    };
    let text = String::from_utf8_lossy(&out.stdout).to_lowercase();
    if text.contains(HIGH) {
        ("Performances élevées".into(), true)
    } else if text.contains(ULTIMATE) {
        ("Performances optimales".into(), true)
    } else if text.contains(SAVER) {
        ("Économie d'énergie".into(), false)
    } else {
        ("Équilibré".into(), false)
    }
}

/// (commande de désinstallation de Kyber officiel, service `kyber` présent ?)
pub fn existing_kyber() -> (Option<String>, bool) {
    if !cfg!(windows) {
        return (None, false);
    }
    let uninstaller = crate::sys::run(
        "reg",
        &["query", r"HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\Kyber", "/v", "UninstallString"],
    )
    .ok()
    .filter(|o| o.status.success())
    .and_then(|o| {
        let text = String::from_utf8_lossy(&o.stdout).into_owned();
        text.lines()
            .find(|l| l.contains("UninstallString"))
            .and_then(|l| l.split("REG_SZ").nth(1))
            .map(|s| s.trim().trim_matches('"').to_string())
    });
    let service = crate::sys::run("sc", &["query", "kyber"]).is_ok_and(|o| o.status.success());
    (uninstaller, service)
}
