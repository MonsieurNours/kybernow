//! Analyse du Mac pendant la bienvenue : puce, version de macOS, réseau,
//! ports, écrans, sortie son et installation en place. Rien n'est modifié.

use std::path::Path;

use super::commands::{CheckStatus, Emit, Event, Existing};
use crate::i18n::t;

/// Ce que l'analyse a trouvé, repris par l'installation.
#[derive(Debug, Clone, Default)]
pub struct MacInfo {
    pub stream_port: u16,
    pub console_port: u16,
    pub lan_ip: Option<String>,
    pub audio_driver: bool,
    /// Pare-feu de macOS allumé.
    pub firewall_on: bool,
}

fn check(emit: &Emit, id: &str, status: CheckStatus, detail: String) {
    emit(Event::Check { id: id.into(), status, title: t(&format!("mac.checks.{id}"), &[]), detail });
}

fn running(emit: &Emit, id: &str) {
    check(emit, id, CheckStatus::Running, String::new());
}

pub fn run(emit: &Emit) -> MacInfo {
    let mut info = MacInfo::default();

    running(emit, "chip");
    let arm = super::read("/usr/sbin/sysctl", &["-n", "hw.optional.arm64"]).as_deref() == Some("1");
    let chip = super::read("/usr/sbin/sysctl", &["-n", "machdep.cpu.brand_string"]).unwrap_or_default();
    if arm {
        check(emit, "chip", CheckStatus::Ok, t("mac.checks.chipOk", &[("chip", &chip)]));
    } else {
        check(emit, "chip", CheckStatus::Error, t("mac.checks.chipIntel", &[("chip", &chip)]));
    }

    running(emit, "macos");
    let version = super::read("/usr/bin/sw_vers", &["-productVersion"]).unwrap_or_default();
    let major: u32 = version.split('.').next().and_then(|m| m.parse().ok()).unwrap_or(0);
    if major >= 15 {
        check(emit, "macos", CheckStatus::Ok, t("mac.checks.macosOk", &[("version", &version)]));
    } else {
        check(emit, "macos", CheckStatus::Error, t("mac.checks.macosOld", &[("version", &version)]));
    }

    running(emit, "network");
    let (status, detail, ip) = network();
    info.lan_ip = ip;
    check(emit, "network", status, detail);

    running(emit, "ports");
    let stream = crate::ports::pick_random(&[], &[]);
    let console = crate::ports::pick_random(&[], &[stream]);
    info.stream_port = stream;
    info.console_port = console;
    check(emit, "ports", CheckStatus::Ok, t("mac.checks.portsDetail", &[("stream", &stream), ("console", &console)]));

    running(emit, "displays");
    let count = displays();
    let (status, key) = match count {
        0 => (CheckStatus::Info, "mac.checks.displaysNone"),
        1 => (CheckStatus::Warn, "mac.checks.displaysOne"),
        _ => (CheckStatus::Warn, "mac.checks.displaysMany"),
    };
    check(emit, "displays", status, t(key, &[("count", &count)]));

    running(emit, "audio");
    info.audio_driver = Path::new(super::HAL_DIR).join(super::DRIVER_NAME).exists();
    check(emit, "audio", CheckStatus::Info, t(if info.audio_driver { "mac.checks.audioThere" } else { "mac.checks.audioNew" }, &[]));

    info.firewall_on = firewall_on();

    let existing = existing();
    if let Some(e) = &existing {
        check(emit, "existing", CheckStatus::Info, t("mac.checks.existingDetail", &[("version", &e.version)]));
    }

    emit(Event::ChecksDone {
        stream_port: info.stream_port,
        console_port: info.console_port,
        lan_ip: info.lan_ip.clone(),
        existing,
        audio_driver: info.audio_driver,
    });
    info
}

/// La Station déjà installée (`kybernow.json`), avec ses ports en service.
pub fn existing() -> Option<Existing> {
    let record = super::read_record()?;
    let settings = super::kyber_dir().join("kyber_config.toml").is_file() && super::admin_dir().join("kyber-admin.toml").is_file();
    Some(Existing { version: record.version, stream_port: record.stream_port, console_port: record.console_port, settings })
}

/// Pare-feu de l'application de macOS (`socketfilterfw`, lisible sans droits).
pub fn firewall_on() -> bool {
    super::read("/usr/libexec/ApplicationFirewall/socketfilterfw", &["--getglobalstate"])
        .is_some_and(|s| s.contains("enabled") || s.contains("State = 1") || s.contains("State = 2"))
}

/// Cartes réseau avec une adresse IPv4 : nom (« Ethernet », « Wi-Fi »…)
/// d'après `networksetup`, adresse d'après `ipconfig`, celle de la route
/// principale en premier.
fn network() -> (CheckStatus, String, Option<String>) {
    let ports = super::read("/usr/sbin/networksetup", &["-listallhardwareports"]).unwrap_or_default();
    let mut cards: Vec<(String, String, String)> = Vec::new(); // (nom, appareil, adresse)
    let mut name = String::new();
    for line in ports.lines() {
        if let Some(n) = line.strip_prefix("Hardware Port: ") {
            name = n.trim().to_string();
        } else if let Some(dev) = line.strip_prefix("Device: ") {
            let dev = dev.trim();
            if let Some(ip) = super::read("/usr/sbin/ipconfig", &["getifaddr", dev]).filter(|ip| !ip.is_empty()) {
                cards.push((name.clone(), dev.to_string(), ip));
            }
        }
    }
    let primary = super::read("/sbin/route", &["-n", "get", "default"])
        .and_then(|r| r.lines().find_map(|l| l.trim().strip_prefix("interface: ").map(|s| s.trim().to_string())));
    if let Some(p) = &primary {
        cards.sort_by_key(|(_, dev, _)| dev != p);
    }
    let wifi = |n: &str| n.contains("Wi-Fi") || n.contains("AirPort");
    match cards.as_slice() {
        [] => (CheckStatus::Warn, t("mac.checks.networkNone", &[]), None),
        [(n, _, ip), rest @ ..] => {
            let lan = Some(ip.clone());
            // « USB 10/100/1G/2.5G LAN », « Thunderbolt Ethernet Slot 0 »… : de l'Ethernet.
            let label = if wifi(n) { "Wi-Fi".to_string() } else { "Ethernet".to_string() };
            if wifi(n) {
                (CheckStatus::Warn, t("mac.checks.networkWifi", &[("ip", ip)]), lan)
            } else if rest.iter().any(|(m, _, _)| wifi(m)) {
                (CheckStatus::Ok, t("mac.checks.networkPreferred", &[("name", &label), ("ip", ip)]), lan)
            } else {
                (CheckStatus::Ok, t("mac.checks.networkOne", &[("name", &label), ("ip", ip)]), lan)
            }
        }
    }
}

#[link(name = "CoreGraphics", kind = "framework")]
unsafe extern "C" {
    fn CGGetOnlineDisplayList(max: u32, displays: *mut u32, count: *mut u32) -> i32;
}

/// Écrans branchés au Mac (allumés ou en veille).
fn displays() -> u32 {
    let mut ids = [0u32; 16];
    let mut count = 0u32;
    let err = unsafe { CGGetOnlineDisplayList(ids.len() as u32, ids.as_mut_ptr(), &mut count) };
    if err == 0 { count } else { 0 }
}
