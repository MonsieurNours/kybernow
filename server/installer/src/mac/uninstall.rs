//! Désinstallation de KyberNow Station sur un Mac : services de la session,
//! écrans et son rendus, app, sortie son, pare-feu, autorisations, et les
//! réglages si on ne les garde pas. Le mot de passe du Mac est demandé une
//! fois, pour la sortie son et l'app.

use std::path::Path;

use anyhow::Result;

use super::root::{self, quote, quote_path};
use super::{app_exe, app_path};
use crate::i18n::t;

pub fn run(keep_settings: bool) -> Result<()> {
    let app = app_path();
    let log = |line: &str| {
        let _ = std::fs::create_dir_all(super::logs_dir());
        if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(super::logs_dir().join("install.log")) {
            use std::io::Write;
            let _ = writeln!(f, "désinstallation : {line}");
        }
    };
    log(if keep_settings { "réglages gardés" } else { "tout retirer" });

    // 1. Les services : l'icône, Admin (son agent rend écrans et son), Kyber.
    for label in super::LABELS.iter().rev() {
        super::bootout(label);
    }
    // Filets : écrans du Mac rallumés et sortie son d'origine, si une session
    // a été coupée net.
    let agent = app_exe(&app, "kybernow-display");
    if agent.is_file() {
        let _ = std::process::Command::new(&agent).arg("recover").output();
        let _ = std::process::Command::new(&agent).args(["audio", "restore"]).output();
    }
    for label in super::LABELS {
        let _ = std::fs::remove_file(super::plist_path(label));
    }
    log("services retirés");

    // 2. En administrateur : sortie son, pare-feu, l'app.
    let mut script = String::new();
    let driver = quote(format!("{}/{}", super::HAL_DIR, super::DRIVER_NAME));
    script.push_str(&format!("if [ -e {driver} ]; then rm -rf {driver}; /usr/bin/killall coreaudiod 2>/dev/null || true; echo 'sortie son retirée'; fi\n"));
    let fw = "/usr/libexec/ApplicationFirewall/socketfilterfw";
    for exe in ["kycontroller", "kyber-admin"] {
        script.push_str(&format!("{fw} --remove {} >/dev/null 2>&1 || true\n", quote_path(&app_exe(&app, exe))));
    }
    if app.exists() {
        script.push_str(&format!("rm -rf {}\necho 'app retirée'\n", quote_path(&app)));
    }
    let out = root::run(&script, &t("mac.uninstall.passwordPrompt", &[]))?;
    log(&out.replace(['\n', '\r'], ", "));

    // 3. Autorisations données à l'app (sans effet si déjà oubliées).
    for service in ["ScreenCapture", "Accessibility", "Microphone"] {
        let _ = std::process::Command::new("/usr/bin/tccutil").args(["reset", service, super::BUNDLE_ID]).output();
    }

    // 4. Réglages, comptes, appareils, certificat : seulement si on ne les garde pas.
    if !keep_settings {
        let _ = std::fs::remove_dir_all(super::station_dir());
        for file in ["hdr-sdr-white.json"] {
            let _ = std::fs::remove_file(super::support_dir().join(file));
        }
        remove_if_empty(&super::support_dir());
        log("réglages effacés");
    }
    log("terminée");
    Ok(())
}

fn remove_if_empty(dir: &Path) {
    if std::fs::read_dir(dir).is_ok_and(|mut d| d.next().is_none()) {
        let _ = std::fs::remove_dir(dir);
    }
}
