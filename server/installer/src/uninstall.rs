//! Désinstallation complète de KyberNow Station.
//!
//! ViGEmBus n'est pas retiré : d'autres logiciels (Steam, Sunshine…) peuvent s'en servir.
//! Le dossier d'installation est effacé par une commande différée, lancée par
//! [`schedule_removal`] une fois la boîte de fin fermée : le désinstalleur
//! tourne depuis ce dossier.

use anyhow::Result;

use crate::install::{
    DISPLAY_CLASS_GUID, HIGH_PERFORMANCE_SCHEME, InstallRecord, KYBER_CONFIG, RUN_KEY, Restore, SUDOVDA_HWID, UNINSTALL_KEY,
    read_record, remove_firewall_rules, wait_stopped,
};
use crate::sys::{self, run};

/// Mode « Utilisation normale » de Windows, repli si l'ancien mode a disparu.
const BALANCED_SCHEME: &str = "381b4222-f694-41f0-9685-ff5bb260df2e";

/// Retire KyberNow Station. Avec `keep_data`, `C:\ProgramData\KyberNow` reste (réglages
/// d'Admin, certificat, appareils, journaux, sauvegardes, plus une copie
/// de `kyber_config.toml`), réservé aux administrateurs.
pub fn run_uninstall(keep_data: bool) -> Result<()> {
    let dir = sys::install_dir();
    let record: Option<InstallRecord> = read_record(&dir);

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
        remove_driver_package("sudovda.inf");
        if let Some(thumb) = record.as_ref().and_then(|r| r.sudovda_thumbprint.clone()) {
            let _ = run("certutil", &["-delstore", "Root", &thumb]);
            let _ = run("certutil", &["-delstore", "TrustedPublisher", &thumb]);
        }
    }
    if let Some(record) = &record {
        restore_windows(&record.restore);
    }

    let _ = run("reg", &["delete", UNINSTALL_KEY, "/f"]);
    // Tout le dossier du menu Démarrer : raccourcis actuels (« KyberNow Station »,
    // « KyberNow Admin ») et ceux des versions précédentes (« KyberNow », « Console KyberNow »).
    let _ = std::fs::remove_dir_all(crate::install::start_menu_dir());
    if keep_data {
        // Les comptes des appareils sont dans kyber_config.toml, sous Program Files.
        let backups = sys::data_dir().join("backups");
        let _ = std::fs::create_dir_all(&backups);
        let _ = std::fs::copy(dir.join(KYBER_CONFIG), backups.join("kyber_config.toml.desinstallation"));
    } else {
        let _ = std::fs::remove_dir_all(sys::data_dir());
    }

    let current = std::env::current_exe().ok();
    for entry in std::fs::read_dir(&dir).into_iter().flatten().flatten() {
        let path = entry.path();
        if Some(&path) == current.as_ref() {
            continue;
        }
        let _ = if path.is_dir() { std::fs::remove_dir_all(&path) } else { std::fs::remove_file(&path) };
    }
    Ok(())
}

/// Efface le dossier d'installation quelques secondes après la sortie du
/// désinstalleur. À appeler juste avant de quitter, après la boîte de fin.
pub fn schedule_removal() {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // Ligne passée telle quelle : les arguments de Rust échapperaient les
        // guillemets en \", que cmd ne comprend pas (le dossier restait).
        let script = format!("/C ping 127.0.0.1 -n 4 > nul & rmdir /s /q \"{}\"", sys::install_dir().display());
        let _ = std::process::Command::new("cmd")
            .raw_arg(script)
            // Pas depuis le dossier à effacer.
            .current_dir(std::env::temp_dir())
            .creation_flags(0x0800_0000 | 0x0000_0008) // sans fenêtre, détaché
            .spawn();
    }
}

/// Retire du magasin de pilotes (DriverStore) les paquets dont le nom
/// d'origine est `original` : `pnputil /delete-driver oemNN.inf /uninstall`.
fn remove_driver_package(original: &str) {
    let Ok(out) = run("pnputil", &["/enum-drivers"]) else { return };
    for published in published_infs(&sys::text(&out), original) {
        let _ = run("pnputil", &["/delete-driver", &published, "/uninstall", "/force"]);
    }
}

/// Noms publiés (`oemNN.inf`) des paquets dont le nom d'origine est
/// `original`, dans la sortie de `pnputil /enum-drivers`. Les intitulés
/// changent avec la langue de Windows : seules les valeurs comptent, bloc par bloc.
pub fn published_infs(text: &str, original: &str) -> Vec<String> {
    let value = |line: &str| line.split_once(':').map(|(_, v)| v.trim().to_string()).unwrap_or_default();
    text.replace("\r\n", "\n")
        .split("\n\n")
        .filter(|block| block.lines().any(|l| value(l).eq_ignore_ascii_case(original)))
        .filter_map(|block| {
            block.lines().map(value).find(|v| {
                let v = v.to_ascii_lowercase();
                v.starts_with("oem") && v.ends_with(".inf") && v[3..v.len() - 4].bytes().all(|b| b.is_ascii_digit())
            })
        })
        .collect()
}

/// Remet le mode d'alimentation et la catégorie du réseau d'avant KyberNow.
fn restore_windows(restore: &Restore) {
    // Seulement si l'ordinateur est encore dans le mode de KyberNow : un autre
    // choix fait depuis par l'utilisateur est gardé.
    let active = run("powercfg", &["/getactivescheme"]).ok().and_then(|o| crate::install::find_guid(&sys::text(&o), ""));
    let ours = |g: &str| g == HIGH_PERFORMANCE_SCHEME || restore.created_power_scheme.as_deref() == Some(g);
    if active.as_deref().is_some_and(|g| !ours(g)) {
        if let Some(created) = &restore.created_power_scheme {
            let _ = run("powercfg", &["/delete", created]);
        }
    } else if let Some(created) = &restore.created_power_scheme {
        let back = restore.power_scheme.as_deref().unwrap_or(BALANCED_SCHEME);
        if run("powercfg", &["/setactive", back]).is_ok_and(|o| !o.status.success()) {
            let _ = run("powercfg", &["/setactive", BALANCED_SCHEME]);
        }
        let _ = run("powercfg", &["/delete", created]);
    } else if let Some(previous) = restore.power_scheme.as_deref().filter(|g| *g != HIGH_PERFORMANCE_SCHEME) {
        let _ = run("powercfg", &["/setactive", previous]);
    }
    if let Some(network) = &restore.public_network {
        // Seulement si c'est toujours le même réseau sur la même carte.
        let name = network.name.replace('\'', "''");
        let _ = sys::powershell(&format!(
            "$p = Get-NetConnectionProfile -InterfaceIndex {} -ErrorAction SilentlyContinue; \
             if ($p -and $p.Name -eq '{name}' -and $p.NetworkCategory -eq 'Private') {{ \
             Set-NetConnectionProfile -InterfaceIndex {} -NetworkCategory Public }}",
            network.interface_index, network.interface_index
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sudovda_package_is_found_whatever_the_language() {
        let fr = "Pilotes tiers Microsoft PnP\r\n\r\nNom publié :              oem12.inf\r\nNom d'origine :           nvhda.inf\r\nNom du fournisseur :      NVIDIA\r\n\r\nNom publié :              oem57.inf\r\nNom d'origine :           sudovda.inf\r\nNom du fournisseur :      SudoMaker\r\nNom de la classe :        Cartes graphiques\r\n\r\nNom publié : oem60.inf\r\nNom d'origine : SudoVDA.inf\r\n";
        assert_eq!(published_infs(fr, "sudovda.inf"), ["oem57.inf", "oem60.inf"]);
        let en = "Published Name:     oem3.inf\nOriginal Name:      steamstreamingspeakers.inf\n\nPublished Name:     oem9.inf\nOriginal Name:      sudovda.inf\n";
        assert_eq!(published_infs(en, "sudovda.inf"), ["oem9.inf"]);
        assert!(published_infs(en, "absent.inf").is_empty());
    }
}
