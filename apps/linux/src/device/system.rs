//! Ce que l'app sait de cette machine : système (celui de l'hôte, même dans
//! le bac à sable Flatpak : `/run/host/os-release`), bureau et compositeur,
//! modèle (Steam Deck LCD ou OLED, d'après le DMI), carte graphique et
//! décodeurs (VA-API), écran et fréquence variable (DRM), son, batterie,
//! réseau. Et l'interface à prendre (bureau, portable, salon).

use std::ffi::{c_char, c_int};

use super::sdl;
use crate::app::{Layout, Machine, NetInfo, NetKind};

unsafe extern "C" {
    fn kn_vaapi_probe(vendor: *mut c_char, len: c_int) -> c_int;
    fn kn_vrr_probe(name: *mut c_char, len: c_int, connected: *mut c_int) -> c_int;
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
                return n.replace(" (Holo)", "");
            }
        }
    }
    "Linux".into()
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
    if let Some((name, ch)) = sdl::default_audio() {
        m.audio_channels = ch.max(2);
        m.audio_name = if name.is_empty() { sdl::audio_driver() } else { name };
    } else {
        m.audio_name = sdl::audio_driver();
    }
    m.battery = sdl::power();
    log::info!(
        "machine : {} ({}), {}, {} × {} à {} Hz{}, VA-API {} ({}), son {} voies ({})",
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
        if m.vaapi { format!("H.264 {} HEVC {} AV1 {} HEVC 4:4:4 {}", m.hw_h264, m.hw_hevc, m.hw_av1, m.hw_hevc444) } else { "absent".into() },
        m.gpu,
        m.audio_channels,
        m.audio_name
    );
    m
}

/// L'interface : réglage de l'app, sinon `--ui`, sinon console sous gamescope
/// (portable si l'écran ne dépasse pas 1280 × 800, salon au-delà), bureau ailleurs.
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
            }
        } else if name.starts_with("eth") || name.starts_with("en") || name.starts_with("usb") {
            info.kind = NetKind::Ethernet;
            info.link_mbps = std::fs::read_to_string(path.join("speed")).ok().and_then(|s| s.trim().parse::<i64>().ok()).filter(|v| *v > 0).map(|v| v as u32);
        }
    }
    info
}

/// Le bureau demande-t-il moins d'animations (GNOME, réglage d'accessibilité).
pub fn reduce_motion() -> bool {
    std::env::var("KYBERNOW_REDUCE_MOTION").is_ok_and(|v| v == "1")
}
