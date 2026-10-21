//! Diagnostic de l'ordinateur avant installation. Rien n'est modifié ici.

use std::time::Duration;

use serde::Serialize;

use crate::events::{BusyPort, CheckStatus, Emit, Event, Existing};
use crate::i18n::t;
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
    /// KyberNow déjà installé.
    pub existing: Option<Existing>,
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

    running(emit, "gpu", &t("checks.gpu.title", &[]));
    let gpu = best_gpu(gpus());
    info.encoder = gpu.as_ref().map_or("x264", |g| g.vendor.encoder()).into();
    match &gpu {
        Some(g) => emit_item(emit, Item { id: "gpu", status: CheckStatus::Ok, title: t("checks.gpu.title", &[]), detail: t("checks.gpu.detail", &[("name", &g.name), ("encoder", &g.vendor.encoder_name())]) }),
        None => emit_item(emit, Item { id: "gpu", status: CheckStatus::Warn, title: t("checks.gpu.noneTitle", &[]), detail: t("checks.gpu.noneDetail", &[]) }),
    }

    running(emit, "driver", &t("checks.driver.title", &[]));
    match &gpu {
        Some(Gpu { driver: Some(v), .. }) => emit_item(emit, Item { id: "driver", status: CheckStatus::Ok, title: t("checks.driver.title", &[]), detail: t("checks.driver.version", &[("version", v)]) }),
        Some(_) => emit_item(emit, Item { id: "driver", status: CheckStatus::Warn, title: t("checks.driver.title", &[]), detail: t("checks.driver.unknown", &[]) }),
        None => emit_item(emit, Item { id: "driver", status: CheckStatus::Warn, title: t("checks.driver.title", &[]), detail: t("checks.driver.none", &[]) }),
    }

    running(emit, "network", &t("checks.network.title", &[]));
    info.lan_ip = lan_ip();
    let net = network(info.lan_ip.as_deref());
    let ip = info.lan_ip.clone().unwrap_or_else(|| t("checks.network.unknownAddress", &[]));
    let (status, mut detail) = match net.as_ref().map(|n| n.media.as_str()) {
        Some("ethernet") => (CheckStatus::Ok, t("checks.network.ethernet", &[("ip", &ip)])),
        Some("wifi") => (CheckStatus::Warn, t("checks.network.wifi", &[("ip", &ip)])),
        _ => (CheckStatus::Ok, ip.clone()),
    };
    let mut status = status;
    if let Some(n) = net.as_ref().filter(|n| n.public) {
        info.public_interface = Some(n.index);
        status = CheckStatus::Warn;
        detail.push_str(&t("checks.network.public", &[]));
    }
    emit_item(emit, Item { id: "network", status, title: t("checks.network.title", &[]), detail });

    running(emit, "ports", &t("checks.ports.title", &[]));
    let excluded = ports::excluded_ranges();
    let busy = (!ports::is_free(ports::KYBER_DEFAULT_PORT, &excluded))
        .then(|| BusyPort { port: ports::KYBER_DEFAULT_PORT, process: ports::owner(ports::KYBER_DEFAULT_PORT) });
    info.stream_port = ports::pick_random(&excluded, &[]);
    info.console_port = ports::pick_random(&excluded, &[info.stream_port]);
    let reason = match &busy {
        Some(b) => {
            let by = b.process.as_ref().map(|p| t("ports.by", &[("process", p)])).unwrap_or_default();
            t("checks.ports.defaultBusy", &[("by", &by)])
        }
        None => String::new(),
    };
    emit_item(emit, Item {
        id: "ports",
        status: CheckStatus::Ok,
        title: t("checks.ports.title", &[]),
        detail: t("checks.ports.picked", &[("reason", &reason), ("stream", &info.stream_port), ("console", &info.console_port)]),
    });

    running(emit, "display", &t("checks.display.title", &[]));
    match display_mode() {
        Some((w, h, hz)) => emit_item(emit, Item { id: "display", status: CheckStatus::Ok, title: t("checks.display.title", &[]), detail: t("checks.display.mode", &[("width", &w), ("height", &h), ("hz", &hz)]) }),
        None => emit_item(emit, Item { id: "display", status: CheckStatus::Warn, title: t("checks.display.title", &[]), detail: t("checks.display.none", &[]) }),
    }

    running(emit, "power", &t("checks.power.title", &[]));
    let (plan, tuned) = power_plan();
    info.power_needs_tuning = !tuned;
    if tuned {
        emit_item(emit, Item { id: "power", status: CheckStatus::Ok, title: t("checks.power.titleWithPlan", &[("plan", &plan)]), detail: t("checks.power.tuned", &[]) });
    } else {
        emit_item(emit, Item { id: "power", status: CheckStatus::Warn, title: t("checks.power.titleWithPlan", &[("plan", &plan)]), detail: t("checks.power.notTuned", &[]) });
    }

    running(emit, "audio", &t("checks.audio.title", &[]));
    info.steam_speakers_inf = steam_speakers_inf();
    if info.steam_speakers_inf.is_some() {
        emit_item(emit, Item { id: "audio", status: CheckStatus::Ok, title: t("checks.audio.title", &[]), detail: t("checks.audio.ok", &[]) });
    } else {
        emit_item(emit, Item { id: "audio", status: CheckStatus::Warn, title: t("checks.audio.title", &[]), detail: t("checks.audio.missing", &[]) });
    }

    let (uninstaller, service) = existing_kyber();
    info.kyber_uninstaller = uninstaller.clone();
    info.kyber_service_exists = service;
    info.existing = existing_kybernow();
    if let Some(existing) = &info.existing {
        running(emit, "existing", &t("checks.existing.title", &[]));
        let detail = if existing.settings {
            t("checks.existing.keepChoice", &[])
        } else {
            t("checks.existing.noSettings", &[])
        };
        emit_item(emit, Item { id: "existing", status: CheckStatus::Ok, title: t("checks.existing.installed", &[("version", &existing.version)]), detail });
    } else if uninstaller.is_some() || service {
        running(emit, "existing", &t("checks.existing.title", &[]));
        emit_item(emit, Item { id: "existing", status: CheckStatus::Warn, title: t("checks.existing.kyberTitle", &[]), detail: t("checks.existing.kyberDetail", &[]) });
    }

    emit(Event::ChecksDone {
        stream_port: info.stream_port,
        console_port: info.console_port,
        busy,
        lan_ip: info.lan_ip.clone(),
        steam_speakers: info.steam_speakers_inf.is_some(),
        power_needs_tuning: info.power_needs_tuning,
        existing: info.existing.clone(),
    });
    info
}

/// KyberNow déjà installé : `kybernow.json`, et les ports réellement en
/// service (la console peut avoir changé celui de Kyber).
pub fn existing_kybernow() -> Option<Existing> {
    let dir = crate::sys::install_dir();
    let record: crate::install::InstallRecord = serde_json::from_slice(&std::fs::read(dir.join("kybernow.json")).ok()?).ok()?;
    let kyber_config = std::fs::read_to_string(dir.join(r"kyber\kyber_config.toml")).ok();
    let admin = std::fs::read_to_string(crate::sys::data_dir().join("kyber-admin.toml")).ok();
    Some(Existing {
        version: record.version,
        stream_port: kyber_config.as_deref().and_then(kyber_port).unwrap_or(record.stream_port),
        console_port: admin.as_deref().and_then(console_port).unwrap_or(record.console_port),
        settings: kyber_config.is_some() && admin.is_some(),
    })
}

/// `port` de la table `[kycontroller]` de `kyber_config.toml`.
pub fn kyber_port(toml: &str) -> Option<u16> {
    let mut section = "";
    for line in toml.lines().map(str::trim) {
        if let Some(name) = line.strip_prefix('[').and_then(|l| l.split(']').next()) {
            section = name.trim();
        } else if section == "kycontroller"
            && let Some((key, value)) = line.split_once('=')
            && key.trim() == "port"
        {
            return value.split('#').next()?.trim().parse().ok();
        }
    }
    None
}

/// Port de `listen = "0.0.0.0:9443"` dans `kyber-admin.toml`.
pub fn console_port(toml: &str) -> Option<u16> {
    toml.lines().find_map(|line| {
        let (key, value) = line.split_once('=')?;
        (key.trim() == "listen").then_some(())?;
        value.trim().trim_matches('"').rsplit(':').next()?.parse().ok()
    })
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Vendor {
    // Du plus performant au moins performant : l'ordre décide de l'encodeur.
    Nvidia,
    Amd,
    Intel,
}

impl Vendor {
    fn from_pci(id: u16) -> Option<Self> {
        match id {
            0x10DE => Some(Self::Nvidia),
            0x1002 | 0x1022 => Some(Self::Amd),
            0x8086 => Some(Self::Intel),
            _ => None,
        }
    }

    /// Valeur de `[kyavserver] encoder` dans kyber_config.toml.
    fn encoder(self) -> &'static str {
        match self {
            Self::Nvidia => "nvenc",
            Self::Amd => "amf",
            Self::Intel => "qsv",
        }
    }

    fn encoder_name(self) -> &'static str {
        match self {
            Self::Nvidia => "NVENC",
            Self::Amd => "AMF",
            Self::Intel => "Quick Sync",
        }
    }
}

#[derive(Debug, Clone)]
struct Gpu {
    vendor: Vendor,
    name: String,
    driver: Option<String>,
}

/// La carte qui encodera : NVIDIA, sinon AMD, sinon Intel. Sur un portable
/// hybride (Intel intégré + NVIDIA), c'est donc la NVIDIA.
fn best_gpu(gpus: Vec<Gpu>) -> Option<Gpu> {
    gpus.into_iter().min_by_key(|g| g.vendor)
}

/// Cartes graphiques en service, cartes externes (eGPU) comprises.
fn gpus() -> Vec<Gpu> {
    let mut found = if cfg!(windows) {
        crate::sys::powershell(
            "ConvertTo-Json -Compress -InputObject @(Get-CimInstance Win32_VideoController | \
             Select-Object Name,PNPDeviceID,DriverVersion,ConfigManagerErrorCode,InfFilename)",
        )
        .map(|json| parse_video_controllers(&json))
        .unwrap_or_default()
    } else {
        vec![Gpu { vendor: Vendor::Nvidia, name: "NVIDIA GeForce RTX (simulation)".into(), driver: Some("000.00".into()) }]
    };
    // Repli si Windows n'a pas répondu : NVML voit au moins les cartes NVIDIA.
    if found.is_empty()
        && let Some(nvml) = nvml_wrapper::Nvml::init().ok()
        && let Some(name) = nvml.device_by_index(0).ok().and_then(|d| d.name().ok())
    {
        found.push(Gpu { vendor: Vendor::Nvidia, name, driver: nvml.sys_driver_version().ok() });
    }
    found
}

/// Lit la liste de `Win32_VideoController`. Ne garde que les cartes PCI
/// des fabricants connus, en état de marche, avec le pilote du fabricant :
/// les écrans virtuels, le bureau à distance et l'adaptateur de base de
/// Microsoft (`display.inf`, aucun encodeur matériel) sont écartés.
fn parse_video_controllers(json: &str) -> Vec<Gpu> {
    let Ok(items) = serde_json::from_str::<Vec<serde_json::Value>>(json) else {
        return Vec::new();
    };
    items
        .iter()
        .filter_map(|item| {
            let field = |key: &str| item.get(key).and_then(|v| v.as_str()).map(str::trim).filter(|s| !s.is_empty());
            if item.get("ConfigManagerErrorCode").and_then(|v| v.as_u64()).is_some_and(|code| code != 0) {
                return None;
            }
            if field("InfFilename").is_some_and(|inf| inf.eq_ignore_ascii_case("display.inf")) {
                return None;
            }
            let pnp = field("PNPDeviceID")?.to_ascii_uppercase();
            let ven = pnp.strip_prefix(r"PCI\VEN_")?.get(..4)?;
            let vendor = Vendor::from_pci(u16::from_str_radix(ven, 16).ok()?)?;
            let driver = field("DriverVersion").map(|v| driver_version(vendor, v));
            Some(Gpu { vendor, name: field("Name").unwrap_or(vendor.encoder_name()).to_string(), driver })
        })
        .collect()
}

/// NVIDIA publie ses pilotes sous un autre numéro que celui de Windows :
/// 32.0.15.6094 est le pilote 560.94 (les cinq derniers chiffres).
fn driver_version(vendor: Vendor, windows: &str) -> String {
    if vendor == Vendor::Nvidia {
        let digits: String = windows.split('.').skip(2).collect();
        if digits.len() >= 5 && digits.bytes().all(|b| b.is_ascii_digit()) {
            let last = &digits[digits.len() - 5..];
            return format!("{}.{}", last[..3].trim_start_matches('0'), &last[3..]);
        }
    }
    windows.to_string()
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
        return (t("checks.power.balanced", &[]), false);
    }
    let Ok(out) = crate::sys::run("powercfg", &["/getactivescheme"]) else {
        return (t("checks.power.unknown", &[]), true);
    };
    let text = String::from_utf8_lossy(&out.stdout).to_lowercase();
    if text.contains(HIGH) {
        (t("checks.power.high", &[]), true)
    } else if text.contains(ULTIMATE) {
        (t("checks.power.ultimate", &[]), true)
    } else if text.contains(SAVER) {
        (t("checks.power.saver", &[]), false)
    } else {
        (t("checks.power.balanced", &[]), false)
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
            .and_then(uninstaller_path)
    });
    let service = crate::sys::run("sc", &["query", "kyber"]).is_ok_and(|o| o.status.success());
    (uninstaller, service)
}

/// Chemin du programme dans une ligne de commande de désinstallation :
/// `"C:\…\uninstall.exe" /arg` ou `C:\…\uninstall.exe /arg` (les arguments
/// sont ajoutés par `remove_official_kyber`).
pub fn uninstaller_path(command: &str) -> Option<String> {
    let command = command.trim();
    let path = if let Some(rest) = command.strip_prefix('"') {
        rest.split('"').next()?
    } else {
        let end = command.to_ascii_lowercase().find(".exe").map(|i| i + 4).unwrap_or(command.len());
        &command[..end]
    };
    let path = path.trim();
    (!path.is_empty()).then(|| path.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uninstall_string_with_arguments() {
        assert_eq!(uninstaller_path(r#""C:\Program Files\Kyber\uninstall.exe" /currentuser"#).as_deref(), Some(r"C:\Program Files\Kyber\uninstall.exe"));
        assert_eq!(uninstaller_path(r"C:\Program Files\Kyber\Uninstall.EXE /S").as_deref(), Some(r"C:\Program Files\Kyber\Uninstall.EXE"));
        assert_eq!(uninstaller_path(r#"  "C:\Kyber\uninstall.exe"  "#).as_deref(), Some(r"C:\Kyber\uninstall.exe"));
        assert_eq!(uninstaller_path(r"C:\Kyber\uninstall.exe").as_deref(), Some(r"C:\Kyber\uninstall.exe"));
        assert_eq!(uninstaller_path("  "), None);
    }

    const HYBRID: &str = r#"[
        {"Name":"Intel(R) UHD Graphics","PNPDeviceID":"PCI\\VEN_8086&DEV_9A60&SUBSYS_0A6D1028&REV_01\\3&11583659&0&10","DriverVersion":"31.0.101.5186","ConfigManagerErrorCode":0,"InfFilename":"oem42.inf"},
        {"Name":"NVIDIA GeForce RTX 4070 Laptop GPU","PNPDeviceID":"PCI\\VEN_10DE&DEV_2860&SUBSYS_0A6D1028&REV_A1\\4&2B1D6F0E&0&0008","DriverVersion":"32.0.15.6094","ConfigManagerErrorCode":0,"InfFilename":"oem55.inf"},
        {"Name":"SudoMaker Virtual Display Adapter","PNPDeviceID":"ROOT\\DISPLAY\\0000","DriverVersion":"1.0.0.0","ConfigManagerErrorCode":0,"InfFilename":"oem60.inf"}
    ]"#;

    #[test]
    fn ports_of_an_existing_installation() {
        let kyber = "[log]\nport = 1\n[kycontroller]\n#port = 8080\nlisten_mode = \"all\"\nport = 23456 # flux\n[kycontroller.security]\nport = 2\n";
        assert_eq!(kyber_port(kyber), Some(23456));
        assert_eq!(kyber_port("[kycontroller]\n#port = 8080\n"), None);
        assert_eq!(console_port("kyber_dir = 'x'\nlisten = \"0.0.0.0:31337\"\n"), Some(31337));
        assert_eq!(console_port("admin_password_hash = \"a:b\"\n"), None);
    }

    #[test]
    fn hybrid_laptop_picks_nvidia() {
        let gpu = best_gpu(parse_video_controllers(HYBRID)).unwrap();
        assert_eq!(gpu.vendor.encoder(), "nvenc");
        assert_eq!(gpu.name, "NVIDIA GeForce RTX 4070 Laptop GPU");
        assert_eq!(gpu.driver.as_deref(), Some("560.94"));
    }

    #[test]
    fn amd_before_intel() {
        let json = r#"[
            {"Name":"Intel(R) Arc(TM) A770","PNPDeviceID":"PCI\\VEN_8086&DEV_56A0","DriverVersion":"32.0.101.6078","ConfigManagerErrorCode":0,"InfFilename":"oem3.inf"},
            {"Name":"AMD Radeon RX 7900 XTX","PNPDeviceID":"PCI\\VEN_1002&DEV_744C","DriverVersion":"32.0.12011.1036","ConfigManagerErrorCode":0,"InfFilename":"oem7.inf"}
        ]"#;
        let gpu = best_gpu(parse_video_controllers(json)).unwrap();
        assert_eq!(gpu.vendor.encoder(), "amf");
        assert_eq!(gpu.driver.as_deref(), Some("32.0.12011.1036"));
    }

    #[test]
    fn intel_alone_uses_quick_sync() {
        let json = r#"[{"Name":"Intel(R) Iris(R) Xe Graphics","PNPDeviceID":"PCI\\VEN_8086&DEV_9A49","DriverVersion":"31.0.101.5186","ConfigManagerErrorCode":0,"InfFilename":"oem12.inf"}]"#;
        assert_eq!(best_gpu(parse_video_controllers(json)).unwrap().vendor.encoder(), "qsv");
    }

    #[test]
    fn unusable_adapters_fall_back_to_processor() {
        // Carte sans pilote (adaptateur de base), carte désactivée, bureau à distance.
        let json = r#"[
            {"Name":"Microsoft Basic Display Adapter","PNPDeviceID":"PCI\\VEN_1002&DEV_73BF","DriverVersion":"10.0.22621.1","ConfigManagerErrorCode":0,"InfFilename":"display.inf"},
            {"Name":"NVIDIA GeForce RTX 3080","PNPDeviceID":"PCI\\VEN_10DE&DEV_2206","DriverVersion":"32.0.15.6094","ConfigManagerErrorCode":22,"InfFilename":"oem5.inf"},
            {"Name":"Microsoft Remote Display Adapter","PNPDeviceID":"SWD\\REMOTEDISPLAYENUM\\RDPIDD_INDIRECTDISPLAY","DriverVersion":"10.0.22621.1","ConfigManagerErrorCode":0,"InfFilename":"rdpidd.inf"}
        ]"#;
        assert!(best_gpu(parse_video_controllers(json)).is_none());
        assert!(parse_video_controllers("").is_empty());
    }
}
