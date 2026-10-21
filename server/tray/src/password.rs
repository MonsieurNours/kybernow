//! « Réinitialiser le mot de passe de la console » : efface le mot de passe
//! administrateur de la console (`kyber-admin.exe reset-password`) et la
//! relance, qui écrit alors un code d'initialisation pour en choisir un autre.
//!
//! Le service et ses réglages demandent des droits administrateur : l'icône se
//! relance élevée (`--reset-console-password`), ce qui affiche une invite UAC.
//! Les appareils et leurs comptes ne changent pas.
//!
//! Le code est écrit par la console dans `C:\ProgramData\KyberNow`, réservé à
//! SYSTEM et aux administrateurs : la partie élevée l'y lit et en laisse une
//! copie dans le profil du compte (`%LOCALAPPDATA%\KyberNow\setup-code.txt`),
//! que l'icône ordinaire lit puis efface.

use std::os::windows::process::CommandExt;
use std::process::Command;
use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail};

use crate::console;
use crate::i18n::t;
use crate::services::{self, ADMIN, State};
use crate::startup;

/// Argument de la relance élevée.
pub const ELEVATED_ARG: &str = "--reset-console-password";

/// Efface le mot de passe et renvoie le nouveau code d'initialisation
/// (`Ok(None)` : invite UAC refusée ; code illisible : message explicite).
pub fn reset() -> Result<Option<String>> {
    match startup::elevate(ELEVATED_ARG)? {
        None => return Ok(None),
        Some(0) => {}
        Some(code) => bail!(t("tray.errors.resetFailed", &[("error", &std::io::Error::from_raw_os_error(code as i32))])),
    }
    // La partie élevée a attendu le code avant de rendre la main.
    if let Some(code) = console::take_setup_code() {
        return Ok(Some(code));
    }
    bail!(t("tray.errors.codeNotHandedOver", &[]))
}

/// Partie élevée : arrête la console, efface le mot de passe, la relance.
pub fn elevated_main() -> i32 {
    match run() {
        Ok(()) => 0,
        Err(e) => {
            crate::app::alert(&format!("{e:#}"));
            1
        }
    }
}

fn run() -> Result<()> {
    let exe = std::env::current_exe()?;
    let admin = exe
        .parent()
        .and_then(|tray| tray.parent())
        .map(|dir| dir.join(r"admin\kyber-admin.exe"))
        .context("dossier de KyberNow introuvable")?;
    if services::state(ADMIN) != State::Stopped {
        services::stop(ADMIN)?;
        services::wait(ADMIN, State::Stopped, Duration::from_secs(30))?;
    }
    // Sans fenêtre de console (CREATE_NO_WINDOW).
    let status = Command::new(&admin).arg("reset-password").creation_flags(0x0800_0000).status().with_context(|| format!("lancement de {}", admin.display()))?;
    // Un ancien code ne doit pas être pris pour celui de la console qui redémarre.
    let _ = std::fs::remove_file(console::data_setup_code_path());
    services::start(ADMIN)?;
    if !status.success() {
        bail!(t("tray.errors.resetCommandFailed", &[("status", &status)]));
    }
    services::wait(ADMIN, State::Running, Duration::from_secs(30))?;
    hand_over_code()
}

/// La console écrit le code à son démarrage, dans ProgramData : la partie
/// élevée en laisse une copie dans le profil du compte, pour l'icône ordinaire.
fn hand_over_code() -> Result<()> {
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        if let Some(code) = console::read_code(&console::data_setup_code_path()) {
            let path = console::handoff_path().context("dossier du profil introuvable (LOCALAPPDATA)")?;
            if let Some(dir) = path.parent() {
                std::fs::create_dir_all(dir)?;
            }
            std::fs::write(&path, code).with_context(|| format!("écriture de {}", path.display()))?;
            return Ok(());
        }
        if Instant::now() > deadline {
            bail!(t("tray.errors.noSetupCode", &[]));
        }
        std::thread::sleep(Duration::from_millis(500));
    }
}
