//! Ce que l'app sait du téléviseur : modèle, version de webOS (fichiers de
//! `/var/run/nyx`, lisibles depuis l'app quand webOS le permet), dalle (SDL),
//! réseau (Ethernet ou Wi-Fi, vitesse du lien).

use crate::app::{NetInfo, NetKind, TvInfo};

fn json_field(text: &str, key: &str) -> Option<String> {
    let v: serde_json::Value = serde_json::from_str(text).ok()?;
    v.get(key).and_then(|x| x.as_str()).map(str::to_string).filter(|s| !s.is_empty())
}

/// webOS 22 est le webOS 7 du système, 23 le 8, et ainsi de suite.
fn marketing(release: &str) -> String {
    let major: u32 = release.split('.').next().and_then(|m| m.parse().ok()).unwrap_or(0);
    match major {
        7..=20 => format!("{}", major + 15),
        6 => "6.0".into(),
        5 => "5".into(),
        4 => "4.5".into(),
        0 => String::new(),
        m => m.to_string(),
    }
}

pub fn tv(panel: (u32, u32), refresh: u32, decoder: bool) -> TvInfo {
    let os = std::fs::read_to_string("/var/run/nyx/os_info.json").unwrap_or_default();
    let device = std::fs::read_to_string("/var/run/nyx/device_info.json").unwrap_or_default();
    let release = json_field(&os, "webos_release").unwrap_or_default();
    let model = ["model_name", "device_name", "product_id", "hardware_id"]
        .iter()
        .find_map(|k| json_field(&device, k))
        .unwrap_or_else(|| "LG".into());
    let webos = marketing(&release);
    let major: u32 = release.split('.').next().and_then(|m| m.parse().ok()).unwrap_or(7);
    log::info!("téléviseur : {model}, webOS {webos} ({release}), dalle {} × {} à {refresh} Hz", panel.0, panel.1);
    TvInfo {
        model,
        webos,
        panel,
        refresh,
        decoder,
        // L'AV1 matériel arrive avec les téléviseurs de 2020 et plus (webOS 5) ;
        // NDL le prend en API 2.
        av1: decoder && major >= 5,
    }
}

/// Le lien réseau en service : Ethernet (et sa vitesse) ou Wi-Fi.
pub fn net() -> NetInfo {
    let mut info = NetInfo::default();
    let Ok(dir) = std::fs::read_dir("/sys/class/net") else { return info };
    for e in dir.flatten() {
        let name = e.file_name().to_string_lossy().into_owned();
        if name == "lo" {
            continue;
        }
        let path = e.path();
        let up = std::fs::read_to_string(path.join("operstate")).map(|s| s.trim() == "up").unwrap_or(false);
        if !up {
            continue;
        }
        if path.join("wireless").exists() || name.starts_with("wlan") {
            if info.kind == NetKind::Unknown {
                info.kind = NetKind::Wifi;
            }
        } else if name.starts_with("eth") || name.starts_with("en") || name.starts_with("usb") {
            info.kind = NetKind::Ethernet;
            info.link_mbps = std::fs::read_to_string(path.join("speed")).ok().and_then(|s| s.trim().parse::<i64>().ok()).filter(|v| *v > 0).map(|v| v as u32);
        }
    }
    info
}
