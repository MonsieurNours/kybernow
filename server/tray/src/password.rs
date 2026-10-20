//! « Réinitialiser le mot de passe de la console » : efface le mot de passe
//! administrateur de la console (`kyber-admin.exe reset-password`) et la
//! relance, qui écrit alors un code d'initialisation pour en choisir un autre.
//!
//! Le service et ses réglages demandent des droits administrateur : l'icône se
//! relance élevée (`--reset-console-password`), ce qui affiche une invite UAC.
//! Les appareils et leurs comptes ne changent pas.

use std::os::windows::process::CommandExt;
use std::process::Command;
use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail};

use crate::console;
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
        Some(code) => bail!("Le mot de passe n'a pas pu être réinitialisé ({}).", std::io::Error::from_raw_os_error(code as i32)),
    }
    // La console écrit le code à son démarrage.
    let deadline = Instant::now() + Duration::from_secs(20);
    while Instant::now() < deadline {
        if let Some(code) = console::setup_code() {
            return Ok(Some(code));
        }
        std::thread::sleep(Duration::from_millis(500));
    }
    bail!("Mot de passe effacé, mais le code d'initialisation est illisible : il est dans C:\\ProgramData\\KyberNow\\setup-code.txt.")
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
    services::start(ADMIN)?;
    if !status.success() {
        bail!("kyber-admin reset-password a échoué ({status}).");
    }
    services::wait(ADMIN, State::Running, Duration::from_secs(30))
}
