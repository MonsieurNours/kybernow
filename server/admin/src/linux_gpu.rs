//! Station Linux : les cartes graphiques (`gpu.rs`), comme sous Windows.
//!
//! - Les cartes : `/sys/class/drm/card<N>/device` (fabricant et puce PCI,
//!   pilote du noyau : `nvidia`, `amdgpu`, `i915`, `xe`…). Seuls NVIDIA, AMD
//!   et Intel comptent (comme sous Windows), dans le même ordre : la première
//!   est celle qui encode (NVENC, sinon VA-API).
//! - Le nom : celui de la base PCI (`pci.ids`, paquet `hwdata` ou
//!   `pciutils`), le nom commercial entre crochets (« AD102 [GeForce RTX
//!   4090] » → « NVIDIA GeForce RTX 4090 »).
//! - Le pilote : sa version (`/sys/module/<pilote>/version`, celle du pilote
//!   NVIDIA), sinon le pilote du noyau et la version du noyau.
//! - La supervision : NVML pour NVIDIA (comme sous Windows, `gpu.rs`) ;
//!   `amdgpu` donne sa charge, sa mémoire, sa température et sa puissance
//!   dans `/sys` ; Intel ne donne rien sans droits particuliers.

#![cfg_attr(not(target_os = "linux"), allow(dead_code))]

use std::collections::HashMap;
use std::path::Path;
use std::sync::Mutex;

use crate::gpu::{GpuDevice, Vendor};

/// Où les distributions rangent la base PCI.
const PCI_IDS: [&str; 4] = ["/usr/share/hwdata/pci.ids", "/usr/share/misc/pci.ids", "/usr/share/pci.ids", "/usr/share/pciutils/pci.ids"];

/// Nom court d'un fabricant connu.
fn short_vendor(vendor: u16) -> Option<&'static str> {
    Some(match vendor {
        0x10DE => "NVIDIA",
        0x1002 | 0x1022 => "AMD",
        0x8086 => "Intel",
        0x10EC => "Realtek",
        0x14E4 => "Broadcom",
        0x168C | 0x1969 => "Qualcomm Atheros",
        0x17CB => "Qualcomm",
        0x14C3 => "MediaTek",
        0x1D6A => "Aquantia",
        0x1AF4 => "Virtio",
        0x1AB8 => "Parallels",
        0x15AD => "VMware",
        _ => return None,
    })
}

/// (nom du fabricant, nom de la puce) dans le texte de `pci.ids`.
pub fn lookup_pci_ids(text: &str, vendor: u16, device: u16) -> Option<(String, String)> {
    let (v, d) = (format!("{vendor:04x}"), format!("{device:04x}"));
    let mut vendor_name: Option<String> = None;
    for line in text.lines() {
        if line.starts_with('#') || line.is_empty() {
            continue;
        }
        if let Some(rest) = line.strip_prefix('\t') {
            if vendor_name.is_none() || rest.starts_with('\t') {
                continue;
            }
            if let Some(name) = rest.strip_prefix(d.as_str()).filter(|n| n.starts_with(' ')) {
                return Some((vendor_name?, name.trim().to_string()));
            }
        } else if vendor_name.is_some() {
            // Fabricant suivant : la puce n'est pas dans la base.
            return None;
        } else if let Some(name) = line.strip_prefix(v.as_str()).filter(|n| n.starts_with(' ')) {
            vendor_name = Some(name.trim().to_string());
        } else if line.starts_with('C') {
            // Classes de périphériques, après les fabricants.
            return None;
        }
    }
    None
}

/// « NVIDIA GeForce RTX 4090 » d'après l'entrée de la base.
pub fn display_name(vendor: u16, vendor_name: &str, device_name: &str) -> String {
    // Le nom commercial est entre crochets (le dernier : « Navi 31 [Radeon RX 7900 XT/7900 XTX] »).
    let model = match (device_name.rfind('['), device_name.rfind(']')) {
        (Some(a), Some(b)) if b > a + 1 => &device_name[a + 1..b],
        _ => device_name,
    };
    let brand = short_vendor(vendor).map(str::to_string).unwrap_or_else(|| {
        vendor_name.split([' ', ',']).next().unwrap_or(vendor_name).to_string()
    });
    if model.to_ascii_lowercase().starts_with(&brand.to_ascii_lowercase()) { model.to_string() } else { format!("{brand} {model}") }
}

/// Nom d'une puce PCI (`None` : pas de base PCI, ou puce inconnue).
pub fn pci_name(vendor: u16, device: u16) -> Option<String> {
    static CACHE: Mutex<Option<HashMap<(u16, u16), Option<String>>>> = Mutex::new(None);
    let mut cache = CACHE.lock().unwrap_or_else(|e| e.into_inner());
    let map = cache.get_or_insert_with(HashMap::new);
    if let Some(name) = map.get(&(vendor, device)) {
        return name.clone();
    }
    let name = PCI_IDS
        .iter()
        .find_map(|p| std::fs::read_to_string(p).ok())
        .and_then(|text| lookup_pci_ids(&text, vendor, device))
        .map(|(v, d)| display_name(vendor, &v, &d));
    map.insert((vendor, device), name.clone());
    name
}

/// Une carte de `/sys/class/drm`.
#[derive(Debug, Clone, PartialEq)]
pub struct DrmCard {
    pub vendor: Vendor,
    pub vendor_id: u16,
    pub device_id: u16,
    /// Pilote du noyau (`amdgpu`).
    pub driver: Option<String>,
    /// La carte qui affichait au démarrage (`boot_vga`).
    pub boot_vga: bool,
    pub busy_percent: Option<u32>,
    pub vram_used: Option<u64>,
    pub vram_total: Option<u64>,
    pub temperature_c: Option<u32>,
    pub power_w: Option<f32>,
}

fn read(dir: &Path, file: &str) -> Option<String> {
    std::fs::read_to_string(dir.join(file)).ok().map(|s| s.trim().to_string())
}

/// Lit `/sys/class/drm/card<N>` (`dir`). `None` : pas une carte (sortie
/// `card0-HDMI-A-1`), ou d'un fabricant que la Station ne connaît pas.
pub fn read_card(dir: &Path) -> Option<DrmCard> {
    let name = dir.file_name()?.to_string_lossy().into_owned();
    if !name.strip_prefix("card").is_some_and(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit())) {
        return None;
    }
    let device = dir.join("device");
    let vendor_id = crate::linux_net::hex_id(&read(&device, "vendor")?)?;
    let device_id = crate::linux_net::hex_id(&read(&device, "device")?)?;
    let vendor = match vendor_id {
        0x10DE => Vendor::Nvidia,
        0x1002 | 0x1022 => Vendor::Amd,
        0x8086 => Vendor::Intel,
        _ => return None,
    };
    let driver = std::fs::read_link(device.join("driver")).ok().and_then(|p| p.file_name().map(|f| f.to_string_lossy().into_owned()));
    let number = |file: &str| read(&device, file).and_then(|v| v.parse::<u64>().ok());
    // hwmon de la carte (amdgpu) : température en millièmes de degré, puissance en µW.
    let hwmon = std::fs::read_dir(device.join("hwmon")).ok().and_then(|mut d| d.next()).and_then(|e| e.ok()).map(|e| e.path());
    let hw = |file: &str| hwmon.as_deref().and_then(|h| read(h, file)).and_then(|v| v.parse::<u64>().ok());
    Some(DrmCard {
        vendor,
        vendor_id,
        device_id,
        driver,
        boot_vga: read(&device, "boot_vga").as_deref() == Some("1"),
        busy_percent: number("gpu_busy_percent").map(|p| p.min(100) as u32),
        vram_used: number("mem_info_vram_used"),
        vram_total: number("mem_info_vram_total"),
        temperature_c: hw("temp1_input").map(|t| (t / 1000) as u32),
        power_w: hw("power1_average").or_else(|| hw("power1_input")).map(|p| p as f32 / 1_000_000.0),
    })
}

/// Les cartes, dans l'ordre de l'installation (NVIDIA, AMD, Intel ; à
/// fabricant égal, celle de l'affichage au démarrage d'abord).
pub fn sort_cards(cards: &mut [DrmCard]) {
    cards.sort_by_key(|c| (c.vendor, !c.boot_vga));
}

/// La version du pilote affichée : celle du module (pilote NVIDIA), sinon
/// « amdgpu (Linux 6.8.0) ».
fn driver_label(driver: Option<&str>, module_version: Option<String>, kernel: Option<&str>) -> Option<String> {
    match (driver, module_version, kernel) {
        (_, Some(v), _) => Some(v),
        (Some(d), None, Some(k)) => Some(format!("{d} (Linux {k})")),
        (Some(d), None, None) => Some(d.to_string()),
        (None, _, _) => None,
    }
}

pub fn to_device(card: &DrmCard, name: Option<String>, driver: Option<String>) -> GpuDevice {
    let mut d = GpuDevice::new(card.vendor, name.unwrap_or_else(|| card.vendor.fallback_name().to_string()), driver);
    d.device_id = Some(card.device_id);
    d.gpu_utilization = card.busy_percent;
    d.memory_used_mb = card.vram_used.map(|b| b / 1_048_576);
    d.memory_total_mb = card.vram_total.map(|b| b / 1_048_576);
    d.temperature_c = card.temperature_c;
    d.power_w = card.power_w;
    d
}

/// Les cartes de cette Station (NVML les complète ensuite, `gpu.rs`).
#[cfg(target_os = "linux")]
pub fn devices() -> Vec<GpuDevice> {
    let mut cards: Vec<DrmCard> = std::fs::read_dir("/sys/class/drm")
        .map(|d| d.flatten().filter_map(|e| read_card(&e.path())).collect())
        .unwrap_or_default();
    sort_cards(&mut cards);
    let kernel = std::fs::read_to_string("/proc/sys/kernel/osrelease").ok().map(|k| k.trim().split('-').next().unwrap_or("").to_string());
    cards
        .iter()
        .map(|c| {
            let module = c.driver.as_deref().and_then(|d| read(&Path::new("/sys/module").join(d), "version"));
            to_device(c, pci_name(c.vendor_id, c.device_id), driver_label(c.driver.as_deref(), module, kernel.as_deref()))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const PCI: &str = "# pci.ids\n#\n10de  NVIDIA Corporation\n\t1b80  GP104 [GeForce GTX 1080]\n\t2684  AD102 [GeForce RTX 4090]\n\t\t1043 889d  ROG Strix\n1002  Advanced Micro Devices, Inc. [AMD/ATI]\n\t744c  Navi 31 [Radeon RX 7900 XT/7900 XTX/7900 GRE/7900M]\n8086  Intel Corporation\n\t15f3  Ethernet Controller I225-V\n\t46a6  Alder Lake-P GT2 [Iris Xe Graphics]\n\t46a7  Something\n10ec  Realtek Semiconductor Co., Ltd.\n\t8125  RTL8125 2.5GbE Controller\nC 00  Unclassified device\n\t00  Non-VGA unclassified device\n";

    #[test]
    fn reads_the_pci_database() {
        let name = |v, d| lookup_pci_ids(PCI, v, d).map(|(vn, dn)| display_name(v, &vn, &dn));
        assert_eq!(name(0x10DE, 0x2684).as_deref(), Some("NVIDIA GeForce RTX 4090"));
        assert_eq!(name(0x1002, 0x744C).as_deref(), Some("AMD Radeon RX 7900 XT/7900 XTX/7900 GRE/7900M"));
        assert_eq!(name(0x8086, 0x46A6).as_deref(), Some("Intel Iris Xe Graphics"));
        assert_eq!(name(0x8086, 0x15F3).as_deref(), Some("Intel Ethernet Controller I225-V"));
        assert_eq!(name(0x10EC, 0x8125).as_deref(), Some("Realtek RTL8125 2.5GbE Controller"));
        // Puce inconnue, fabricant inconnu, ligne de sous-système.
        assert_eq!(name(0x10DE, 0x9999), None);
        assert_eq!(name(0x1234, 0x0001), None);
        assert_eq!(name(0x10DE, 0x1043), None);
    }

    #[test]
    fn reads_drm_cards() {
        let root = std::env::temp_dir().join(format!("kn-drm-{}", rand::random::<u64>()));
        let card = |name: &str, files: &[(&str, &str)]| {
            let dir = root.join(name);
            for (f, v) in files {
                let p = dir.join("device").join(f);
                std::fs::create_dir_all(p.parent().unwrap()).unwrap();
                std::fs::write(p, format!("{v}\n")).unwrap();
            }
            std::fs::create_dir_all(&dir).unwrap();
            dir
        };
        let amd = card(
            "card1",
            &[
                ("vendor", "0x1002"),
                ("device", "0x744c"),
                ("boot_vga", "1"),
                ("gpu_busy_percent", "37"),
                ("mem_info_vram_used", "2147483648"),
                ("mem_info_vram_total", "25753026560"),
                ("hwmon/hwmon3/temp1_input", "52000"),
                ("hwmon/hwmon3/power1_average", "84000000"),
            ],
        );
        let intel = card("card0", &[("vendor", "0x8086"), ("device", "0x46a6"), ("boot_vga", "0")]);
        let virtio = card("card2", &[("vendor", "0x1af4"), ("device", "0x1050")]);
        let output = card("card1-DP-1", &[("vendor", "0x1002")]);
        let a = read_card(&amd).unwrap();
        assert_eq!((a.vendor, a.device_id, a.boot_vga, a.busy_percent, a.temperature_c), (Vendor::Amd, 0x744C, true, Some(37), Some(52)));
        assert_eq!(a.power_w, Some(84.0));
        let d = to_device(&a, Some("AMD Radeon RX 7900 XTX".into()), driver_label(Some("amdgpu"), None, Some("6.8.0")));
        assert_eq!((d.memory_used_mb, d.memory_total_mb, d.driver.as_deref()), (Some(2048), Some(24560), Some("amdgpu (Linux 6.8.0)")));
        assert!(read_card(&virtio).is_none() && read_card(&output).is_none());
        let mut cards = vec![read_card(&intel).unwrap(), a];
        sort_cards(&mut cards);
        assert_eq!(cards[0].vendor, Vendor::Amd);
        assert_eq!(driver_label(Some("nvidia"), Some("550.120".into()), Some("6.8.0")).as_deref(), Some("550.120"));
        let _ = std::fs::remove_dir_all(&root);
    }
}
