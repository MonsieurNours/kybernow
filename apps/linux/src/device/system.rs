//! Ce que l'app sait de cette machine : système (celui de l'hôte, même dans
//! le bac à sable Flatpak : `/run/host/os-release`), bureau et compositeur,
//! modèle (Steam Deck LCD ou OLED, d'après le DMI), carte graphique et
//! décodeurs (VA-API), Raspberry Pi (arbre des périphériques, décodeurs V4L2,
//! température), écran et fréquence variable (DRM), son, batterie, réseau. Et
//! l'interface à prendre (bureau, portable, salon).

use std::ffi::{c_char, c_int};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use super::sdl;
use crate::app::{Layout, Machine, NetInfo, NetKind, Pi};

unsafe extern "C" {
    fn kn_vaapi_probe(vendor: *mut c_char, len: c_int) -> c_int;
    fn kn_vrr_probe(name: *mut c_char, len: c_int, connected: *mut c_int) -> c_int;
    fn kn_modes_probe(hz_1080p: *mut c_int, w: *mut c_int, h: *mut c_int, hz: *mut c_int) -> c_int;
}

fn os_release() -> String {
    for p in ["/run/host/os-release", "/run/host/usr/lib/os-release", "/etc/os-release"] {
        if let Ok(t) = std::fs::read_to_string(p) {
            let get = |k: &str| t.lines().find_map(|l| l.strip_prefix(&format!("{k}="))).map(|v| v.trim_matches('"').to_string());
            if let Some(n) = get("PRETTY_NAME").or_else(|| get("NAME")) {
                // « SteamOS » seul dans PRETTY_NAME : la version vient de VERSION_ID.
                if n == "SteamOS"
                    && let Some(v) = get("VERSION_ID")
                {
                    return format!("SteamOS {v}");
                }
                // Raspberry Pi OS se présente comme Debian : son nom vient de /etc/rpi-issue.
                if n.starts_with("Debian") && (std::path::Path::new("/etc/rpi-issue").exists() || std::path::Path::new("/run/host/etc/rpi-issue").exists()) {
                    let codename = get("VERSION_CODENAME").unwrap_or_default();
                    let mut c = codename.chars();
                    let cap: String = c.next().map(|f| f.to_uppercase().collect::<String>() + c.as_str()).unwrap_or_default();
                    return if cap.is_empty() { "Raspberry Pi OS".into() } else { format!("Raspberry Pi OS ({cap})") };
                }
                return n.replace(" (Holo)", "");
            }
        }
    }
    "Linux".into()
}

// MARK: - Raspberry Pi

fn read_trim(p: &str) -> String {
    std::fs::read_to_string(p).map(|s| s.trim_matches(['\0', '\n', ' ']).to_string()).unwrap_or_default()
}

/// Le modèle court d'après l'arbre des périphériques : « Raspberry Pi 5 Model B
/// Rev 1.0 » → « Raspberry Pi 5 », « Raspberry Pi Compute Module 5 Rev 1.0 » →
/// « Compute Module 5 », « Raspberry Pi 3 Model B Plus Rev 1.3 » → « Raspberry Pi 3 B+ ».
pub fn pi_short_model(full: &str) -> String {
    let s = full.trim();
    if !s.starts_with("Raspberry Pi") {
        return s.to_string();
    }
    let s = s.split(" Rev").next().unwrap_or(s).trim();
    if let Some(rest) = s.strip_prefix("Raspberry Pi Compute Module") {
        return format!("Compute Module{}", rest.split(" Model").next().unwrap_or(""));
    }
    let s = s.replace(" Model B Plus", " B+").replace(" Model A Plus", " A+");
    let s = s.replace(" Model B", "").replace(" Model A", "");
    s.trim().to_string()
}

/// La génération d'après la puce (`compatible` de l'arbre des périphériques).
fn pi_generation(compatible: &str) -> u8 {
    if compatible.contains("bcm2712") {
        5
    } else if compatible.contains("bcm2711") {
        4
    } else if compatible.contains("bcm2837") {
        3
    } else {
        0
    }
}

/// La mémoire vive, en Go arrondis (« MemTotal: 8046412 kB » → 8).
fn ram_gb() -> u32 {
    let text = std::fs::read_to_string("/proc/meminfo").unwrap_or_default();
    let kb: u64 = text.lines().find_map(|l| l.strip_prefix("MemTotal:")).and_then(|v| v.trim().trim_end_matches("kB").trim().parse().ok()).unwrap_or(0);
    // Le noyau et la carte graphique en retiennent un peu : on arrondit au Go.
    ((kb as f64 / (1024.0 * 1024.0)) + 0.5).floor().max(0.0) as u32
}

/// Les noms des périphériques V4L2 (`/sys/class/video4linux/video*/name`).
fn v4l2_names() -> Vec<String> {
    let mut v = Vec::new();
    for root in ["/sys/class/video4linux", "/run/host/sys/class/video4linux"] {
        if let Ok(dir) = std::fs::read_dir(root) {
            for e in dir.flatten() {
                let n = read_trim(&e.path().join("name").to_string_lossy());
                if !n.is_empty() {
                    v.push(n);
                }
            }
            if !v.is_empty() {
                break;
            }
        }
    }
    v
}

/// Un Raspberry Pi, d'après l'arbre des périphériques ; `None` ailleurs.
pub fn pi() -> Option<Pi> {
    let model = ["/proc/device-tree/model", "/sys/firmware/devicetree/base/model"].iter().map(|p| read_trim(p)).find(|s| !s.is_empty())?;
    if !model.starts_with("Raspberry Pi") {
        return None;
    }
    let compatible = ["/proc/device-tree/compatible", "/sys/firmware/devicetree/base/compatible"].iter().map(|p| read_trim(p)).find(|s| !s.is_empty()).unwrap_or_default();
    let names = v4l2_names();
    // bcm2835-codec expose plusieurs nœuds (decode, encode, isp…) ; rpivid un seul.
    let v4l2_h264 = names.iter().any(|n| n.contains("bcm2835-codec-decode") || n == "bcm2835-codec");
    let v4l2_hevc = names.iter().any(|n| n.contains("rpivid"));
    Some(Pi { model: pi_short_model(&model), generation: pi_generation(&compatible), ram_gb: ram_gb(), v4l2_h264, v4l2_hevc })
}

/// La température du processeur (°C), zone thermique 0 (Pi : la puce).
pub fn temperature() -> Option<f32> {
    for p in ["/sys/class/thermal/thermal_zone0/temp", "/run/host/sys/class/thermal/thermal_zone0/temp"] {
        if let Ok(v) = read_trim(p).parse::<f32>() {
            return Some(v / 1000.0);
        }
    }
    None
}

/// Sous gamescope (mode Jeu de SteamOS, Bazzite, ChimeraOS).
pub fn gamescope() -> bool {
    std::env::var_os("GAMESCOPE_WAYLAND_DISPLAY").is_some()
        || std::env::var("XDG_CURRENT_DESKTOP").is_ok_and(|d| d.eq_ignore_ascii_case("gamescope"))
        || std::env::var("SteamGamepadUI").is_ok_and(|v| v == "1")
}

fn desktop() -> String {
    if gamescope() {
        return "gamescope".into();
    }
    let de = std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_default();
    let de = de.split(':').next_back().unwrap_or("").to_string();
    let session = if std::env::var_os("WAYLAND_DISPLAY").is_some() { "Wayland" } else { "X11" };
    if de.is_empty() { session.into() } else { format!("{de}, {session}") }
}

fn dmi(name: &str) -> String {
    std::fs::read_to_string(format!("/sys/class/dmi/id/{name}")).map(|s| s.trim().to_string()).unwrap_or_default()
}

/// Le modèle : Steam Deck (Jupiter : LCD, Galileo : OLED), sinon le produit.
fn model() -> String {
    let product = dmi("product_name");
    let vendor = dmi("sys_vendor");
    match (vendor.as_str(), product.as_str()) {
        ("Valve", "Jupiter") => "Steam Deck LCD".into(),
        ("Valve", "Galileo") => "Steam Deck OLED".into(),
        ("Valve", p) => format!("Valve {p}"),
        (_, p) if p.is_empty() || p.contains("O.E.M") || p.contains("System Product") || p.contains("Default string") => String::new(),
        (v, p) if !v.is_empty() && !p.contains(v) => format!("{v} {p}"),
        (_, p) => p.to_string(),
    }
}

pub fn is_steam_deck() -> bool {
    dmi("sys_vendor") == "Valve"
}

/// La machine, d'après la fenêtre (son écran) et les sondes.
pub fn machine(window: *mut sdl::SDL_Window) -> Machine {
    let mut m = Machine { os: os_release(), desktop: desktop(), model: model(), ..Machine::default() };
    let mut vendor = [0 as c_char; 256];
    let mask = unsafe { kn_vaapi_probe(vendor.as_mut_ptr(), vendor.len() as c_int) };
    if mask >= 0 {
        m.vaapi = true;
        m.hw_h264 = mask & 1 != 0;
        m.hw_hevc = mask & 2 != 0;
        m.hw_av1 = mask & 4 != 0;
        m.hw_hevc444 = mask & 8 != 0;
        m.gpu = unsafe { std::ffi::CStr::from_ptr(vendor.as_ptr()) }.to_string_lossy().into_owned();
    } else {
        m.vaapi = false;
        m.hw_h264 = false;
        m.hw_hevc = false;
        m.hw_av1 = false;
        m.hw_hevc444 = false;
    }
    // Un Raspberry Pi : pas de VA-API, mais les décodeurs V4L2 de la puce.
    if let Some(p) = pi() {
        if m.model.is_empty() {
            m.model = p.model.clone();
        }
        if !m.vaapi {
            m.hw_h264 = p.v4l2_h264;
            m.hw_hevc = p.v4l2_hevc;
            m.hw_av1 = false;
            m.hw_hevc444 = false;
            m.gpu = p.gpu().into();
        }
        m.temp_c = temperature();
        m.pi = Some(p);
    }
    let (size, refresh, name) = sdl::display_mode(window);
    m.display = size;
    m.refresh = refresh;
    m.display_name = name;
    let mut conn = [0 as c_char; 64];
    let mut connected: c_int = 0;
    let vrr = unsafe { kn_vrr_probe(conn.as_mut_ptr(), conn.len() as c_int, &mut connected) };
    m.vrr = match vrr {
        1 => Some(true),
        0 => Some(false),
        _ => None,
    };
    let (mut hz1080, mut mw, mut mh, mut mhz): (c_int, c_int, c_int, c_int) = (0, 0, 0, 0);
    if unsafe { kn_modes_probe(&mut hz1080, &mut mw, &mut mh, &mut mhz) } > 0 {
        m.max_hz_1080p = hz1080.max(0) as u32;
        m.max_mode = (mw.max(0) as u32, mh.max(0) as u32, mhz.max(0) as u32);
    }
    if let Some((name, ch, rate)) = sdl::default_audio() {
        m.audio_channels = ch.max(2);
        m.audio_rate = rate;
        // Sur le Pi (ALSA), la carte s'appelle vc4hdmi0 : « HDMI » suffit.
        m.audio_name = if name.is_empty() { sdl::audio_driver() } else if name.starts_with("vc4hdmi") || name.contains("vc4-hdmi") { "HDMI".into() } else { name };
    } else {
        m.audio_name = sdl::audio_driver();
    }
    m.battery = sdl::power();
    log::info!(
        "machine : {} ({}), {}, {} × {} à {} Hz{}, modes : 1080p à {} Hz au plus, {} × {} à {} Hz, {} {} ({}), son {} voies ({})",
        m.os,
        m.desktop,
        if m.model.is_empty() { "modèle inconnu" } else { &m.model },
        m.display.0,
        m.display.1,
        m.refresh,
        match m.vrr {
            Some(true) => format!(", VRR ({})", unsafe { std::ffi::CStr::from_ptr(conn.as_ptr()) }.to_string_lossy()),
            Some(false) => ", sans VRR".into(),
            None => ", VRR inconnu".into(),
        },
        m.max_hz_1080p,
        m.max_mode.0,
        m.max_mode.1,
        m.max_mode.2,
        if m.pi.is_some() { "V4L2" } else { "VA-API" },
        if m.vaapi || m.pi.is_some() { format!("H.264 {} HEVC {} AV1 {} HEVC 4:4:4 {}", m.hw_h264, m.hw_hevc, m.hw_av1, m.hw_hevc444) } else { "absent".into() },
        m.gpu,
        m.audio_channels,
        m.audio_name
    );
    if let Some(p) = &m.pi {
        log::info!("Raspberry Pi : {} (génération {}), {} Go, V4L2 H.264 {} HEVC {}, {:?} °C", p.model, p.generation, p.ram_gb, p.v4l2_h264, p.v4l2_hevc, m.temp_c);
    }
    m
}

/// L'interface : `--ui`, sinon `KYBERNOW_UI`, sinon le réglage de l'app (hors
/// Auto), sinon console sous gamescope (portable si l'écran ne dépasse pas
/// 1280 × 800, ou 800 de haut sur un Steam Deck ; salon au-delà), bureau ailleurs.
pub fn layout(setting: &str, arg: Option<&str>, display: (u32, u32)) -> Layout {
    let pick = |s: &str| match s {
        "desktop" | "bureau" => Some(Layout::Desktop),
        "handheld" | "deck" | "portable" => Some(Layout::Handheld),
        "tv" | "salon" => Some(Layout::Tv),
        _ => None,
    };
    if let Some(l) = arg.and_then(pick).or_else(|| std::env::var("KYBERNOW_UI").ok().as_deref().and_then(pick)).or_else(|| pick(setting)) {
        return l;
    }
    if gamescope() {
        if display.0 <= 1280 && display.1 <= 800 || is_steam_deck() && display.1 <= 800 { Layout::Handheld } else { Layout::Tv }
    } else {
        Layout::Desktop
    }
}

/// Le nom de cet appareil pour Admin.
pub fn device_name(m: &Machine) -> String {
    let host = std::fs::read_to_string("/etc/hostname").or_else(|_| std::fs::read_to_string("/run/host/etc/hostname")).map(|s| s.trim().to_string()).unwrap_or_default();
    let host = if host.is_empty() { libc_hostname() } else { host };
    match (m.model.as_str(), host.as_str()) {
        (model, h) if model.starts_with("Steam Deck") && (h.is_empty() || h == "steamdeck") => "Steam Deck".into(),
        (model, h) if model.starts_with("Steam Deck") => format!("Steam Deck ({h})"),
        // Le nom par défaut de Raspberry Pi OS ne dit rien : le modèle le remplace.
        (_, h) if m.pi.is_some() && (h.is_empty() || h == "raspberrypi") => m.pi.as_ref().map(|p| p.model.clone()).unwrap_or_else(|| "Raspberry Pi".into()),
        (_, h) if !h.is_empty() => h.to_string(),
        _ => "Linux".into(),
    }
}

fn libc_hostname() -> String {
    let mut buf = [0u8; 256];
    let r = unsafe { libc::gethostname(buf.as_mut_ptr() as *mut c_char, buf.len()) };
    if r != 0 {
        return String::new();
    }
    let end = buf.iter().position(|b| *b == 0).unwrap_or(0);
    String::from_utf8_lossy(&buf[..end]).into_owned()
}

/// Le lien réseau en service : Ethernet (et sa vitesse) ou Wi-Fi.
pub fn net() -> NetInfo {
    let mut info = NetInfo::default();
    let Ok(dir) = std::fs::read_dir("/sys/class/net") else { return info };
    for e in dir.flatten() {
        let name = e.file_name().to_string_lossy().into_owned();
        if name == "lo" || name.starts_with("docker") || name.starts_with("veth") || name.starts_with("virbr") || name.starts_with("tailscale") || name.starts_with("wg") {
            continue;
        }
        let path = e.path();
        let up = std::fs::read_to_string(path.join("operstate")).map(|s| s.trim() == "up").unwrap_or(false);
        if !up {
            continue;
        }
        if path.join("wireless").exists() || path.join("phy80211").exists() || name.starts_with("wl") {
            if info.kind == NetKind::Unknown {
                info.kind = NetKind::Wifi;
                info.band = wifi_band();
            }
        } else if name.starts_with("eth") || name.starts_with("en") || name.starts_with("usb") {
            info.kind = NetKind::Ethernet;
            info.link_mbps = std::fs::read_to_string(path.join("speed")).ok().and_then(|s| s.trim().parse::<i64>().ok()).filter(|v| *v > 0).map(|v| v as u32);
        }
    }
    info
}

/// La bande du Wi-Fi en service (« 2,4 GHz », « 5 GHz », « 6 GHz »), lue par
/// NetworkManager (`nmcli`, la fréquence du réseau actif). `net()` tourne sur
/// la boucle principale : nmcli part sur un fil, et la bande lue sert au
/// relevé suivant (5 s plus tard ; une lecture toutes les 4 s au plus, le
/// lancement de l'OS appelant `net()` plus souvent). Sans nmcli (Flatpak, autre gestionnaire
/// de réseau) : pas de bande, « Wi-Fi » seul.
fn wifi_band() -> Option<String> {
    static BAND: Mutex<Option<String>> = Mutex::new(None);
    static LAST: Mutex<Option<Instant>> = Mutex::new(None);
    static READING: AtomicBool = AtomicBool::new(false);
    let due = LAST.lock().unwrap().is_none_or(|t| t.elapsed() > Duration::from_secs(4));
    if due && !READING.swap(true, Ordering::AcqRel) {
        *LAST.lock().unwrap() = Some(Instant::now());
        std::thread::spawn(|| {
            let out = std::process::Command::new("nmcli").args(["-t", "-f", "ACTIVE,FREQ", "dev", "wifi", "list", "--rescan", "no"]).output();
            let mhz = out.ok().filter(|o| o.status.success()).and_then(|o| {
                String::from_utf8_lossy(&o.stdout).lines().find_map(|l| l.strip_prefix("yes:").and_then(|f| f.split_whitespace().next()?.parse::<u32>().ok()))
            });
            *BAND.lock().unwrap() = mhz.map(|f| {
                match f {
                    0..=3000 => crate::i18n::tr("2,4 GHz", "2.4 GHz"),
                    3001..=5924 => "5 GHz",
                    _ => "6 GHz",
                }
                .to_string()
            });
            READING.store(false, Ordering::Release);
        });
    }
    BAND.lock().unwrap().clone()
}

/// Moins d'animations, demandé par `KYBERNOW_REDUCE_MOTION=1` (le réglage
/// d'accessibilité du bureau n'est pas lu).
pub fn reduce_motion() -> bool {
    std::env::var("KYBERNOW_REDUCE_MOTION").is_ok_and(|v| v == "1")
}
