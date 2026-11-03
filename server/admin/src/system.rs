//! Nom du système de l'ordinateur, tel qu'un utilisateur le reconnaît
//! (« Windows 11 Pro 24H2 », « macOS 26.1 », « Fedora Linux 41 »), affiché
//! par les apps sous le nom de l'ordinateur.

use std::sync::OnceLock;

/// Lu une fois : le système ne change qu'après un redémarrage.
pub fn os_name() -> Option<String> {
    static NAME: OnceLock<Option<String>> = OnceLock::new();
    NAME.get_or_init(read).clone()
}

/// Nom de l'ordinateur tel que l'utilisateur l'a choisi : celui de Windows,
/// ou sur un Mac celui des Réglages (« MacBook Pro de Matthieu »), pas son
/// nom réseau (« MacBook-Pro-de-Matthieu-6.local », que macOS renumérote).
pub fn computer_name() -> Option<String> {
    #[cfg(target_os = "macos")]
    {
        let named = std::process::Command::new("/usr/sbin/scutil")
            .args(["--get", "ComputerName"])
            .output()
            .ok()
            .filter(|o| o.status.success())
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
            .filter(|n| !n.is_empty());
        if named.is_some() {
            return named;
        }
    }
    hostname::get().ok().map(|h| h.to_string_lossy().into_owned())
}

/// Numéro de build de Windows (26100 : Windows 11 24H2) ; `None` ailleurs.
pub fn windows_build() -> Option<u32> {
    #[cfg(windows)]
    {
        static BUILD: OnceLock<Option<u32>> = OnceLock::new();
        *BUILD.get_or_init(|| reg_string("CurrentBuildNumber").and_then(|b| b.trim().parse().ok()))
    }
    #[cfg(not(windows))]
    None
}

/// Windows 11 24H2, le premier qui passe un écran en HDR par
/// `DISPLAYCONFIG_SET_HDR_STATE` (docs/hdr.md).
pub const HDR_BUILD: u32 = 26100;

#[cfg(windows)]
fn read() -> Option<String> {
    let product = reg_string("ProductName")?;
    let build: u32 = reg_string("CurrentBuildNumber").and_then(|b| b.trim().parse().ok()).unwrap_or(0);
    // ProductName dit encore « Windows 10 » sur Windows 11 : le numéro de build fait foi.
    let product = if build >= 22000 { product.replacen("Windows 10", "Windows 11", 1) } else { product };
    // DisplayVersion (« 24H2 ») depuis 20H2, ReleaseId (« 2004 ») avant.
    let version = reg_string("DisplayVersion").or_else(|| reg_string("ReleaseId"));
    Some(match version {
        Some(v) if !v.is_empty() => format!("{product} {v}"),
        _ => product,
    })
}

/// Valeur texte de `HKLM\SOFTWARE\Microsoft\Windows NT\CurrentVersion`.
#[cfg(windows)]
fn reg_string(name: &str) -> Option<String> {
    use windows_sys::Win32::System::Registry::{HKEY_LOCAL_MACHINE, RRF_RT_REG_SZ, RegGetValueW};
    let wide = |s: &str| s.encode_utf16().chain(std::iter::once(0)).collect::<Vec<u16>>();
    let key = wide(r"SOFTWARE\Microsoft\Windows NT\CurrentVersion");
    let value = wide(name);
    let mut buf = [0u16; 256];
    let mut size = (buf.len() * 2) as u32;
    let status = unsafe {
        RegGetValueW(
            HKEY_LOCAL_MACHINE,
            key.as_ptr(),
            value.as_ptr(),
            RRF_RT_REG_SZ,
            std::ptr::null_mut(),
            buf.as_mut_ptr().cast(),
            &mut size,
        )
    };
    if status != 0 {
        return None;
    }
    let len = (size as usize / 2).min(buf.len());
    let text = String::from_utf16_lossy(&buf[..len]);
    let text = text.trim_end_matches('\0').trim();
    (!text.is_empty()).then(|| text.to_string())
}

#[cfg(target_os = "macos")]
fn read() -> Option<String> {
    let sw_vers = |arg: &str| {
        std::process::Command::new("sw_vers")
            .arg(arg)
            .output()
            .ok()
            .filter(|o| o.status.success())
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
            .filter(|s| !s.is_empty())
    };
    let version = sw_vers("-productVersion")?;
    Some(format!("{} {version}", sw_vers("-productName").unwrap_or_else(|| "macOS".into())))
}

#[cfg(all(unix, not(target_os = "macos")))]
fn read() -> Option<String> {
    let text = std::fs::read_to_string("/etc/os-release")
        .or_else(|_| std::fs::read_to_string("/usr/lib/os-release"))
        .ok()?;
    let value = text.lines().find_map(|l| l.strip_prefix("PRETTY_NAME="))?;
    let value = value.trim().trim_matches(|c| c == '"' || c == '\'').trim();
    (!value.is_empty()).then(|| value.to_string())
}
