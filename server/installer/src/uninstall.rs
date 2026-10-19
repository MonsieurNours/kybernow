//! Désinstallation complète de KyberNow.
//!
//! ViGEmBus n'est pas retiré : d'autres logiciels (Steam, Sunshine…) peuvent s'en servir.

use anyhow::Result;

use crate::install::{DISPLAY_CLASS_GUID, InstallRecord, RUN_KEY, SUDOVDA_HWID, UNINSTALL_KEY, remove_firewall_rules, wait_stopped};
use crate::sys::{self, run};

pub fn run_uninstall() -> Result<()> {
    let dir = sys::install_dir();
    let record: Option<InstallRecord> = std::fs::read(dir.join("kybernow.json"))
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok());

    for service in ["kyber-admin", "kyber"] {
        let _ = run("sc", &["stop", service]);
        wait_stopped(service);
        let _ = run("sc", &["delete", service]);
    }
    for exe in ["kyclient.exe", "kycontroller.exe", "kyavserver.exe", "kynputserver.exe", "kyservice.exe", "kyber-admin.exe", "kybernow-tray.exe"] {
        let _ = run("taskkill", &["/F", "/IM", exe]);
    }
    // Icône : lancement à l'ouverture de session (celle de l'utilisateur qui désinstalle).
    let _ = run("reg", &["delete", RUN_KEY, "/v", "KyberNow", "/f"]);
    remove_firewall_rules();

    if record.as_ref().is_none_or(|r| r.virtual_display) {
        let nefconc = dir.join(r"drivers\sudovda\nefconc.exe");
        if nefconc.exists() {
            let _ = run(&nefconc.display().to_string(), &["--remove-device-node", "--hardware-id", SUDOVDA_HWID, "--class-guid", DISPLAY_CLASS_GUID]);
        }
        if let Some(thumb) = record.as_ref().and_then(|r| r.sudovda_thumbprint.clone()) {
            let _ = run("certutil", &["-delstore", "Root", &thumb]);
            let _ = run("certutil", &["-delstore", "TrustedPublisher", &thumb]);
        }
    }

    let _ = run("reg", &["delete", UNINSTALL_KEY, "/f"]);
    let start_menu = std::path::PathBuf::from(std::env::var_os("ProgramData").unwrap_or_default())
        .join(r"Microsoft\Windows\Start Menu\Programs\KyberNow");
    let _ = std::fs::remove_dir_all(start_menu);
    let _ = std::fs::remove_dir_all(sys::data_dir());

    // Le désinstalleur tourne depuis le dossier à supprimer : on laisse une
    // commande différée finir le travail une fois qu'il s'est fermé.
    let current = std::env::current_exe().ok();
    for entry in std::fs::read_dir(&dir).into_iter().flatten().flatten() {
        let path = entry.path();
        if Some(&path) == current.as_ref() {
            continue;
        }
        let _ = if path.is_dir() { std::fs::remove_dir_all(&path) } else { std::fs::remove_file(&path) };
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        let script = format!("ping 127.0.0.1 -n 3 > nul & rmdir /s /q \"{}\"", dir.display());
        let _ = std::process::Command::new("cmd")
            .args(["/C", &script])
            .creation_flags(0x0800_0000 | 0x0000_0008) // sans fenêtre, détaché
            .spawn();
    }
    Ok(())
}
