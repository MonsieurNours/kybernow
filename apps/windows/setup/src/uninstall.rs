//! Désinstallation : `uninstall.exe --uninstall` (« Applications installées »).

use std::time::Duration;

use crate::{install, sys};

/// Retire l'app. `erase_data` : efface aussi les ordinateurs, réglages et
/// mots de passe. Le dossier de l'app est effacé à la fermeture du désinstalleur.
pub fn run_uninstall(erase_data: bool) {
    let dir = sys::install_dir();
    sys::log(&format!("désinstallation (données {})", if erase_data { "effacées" } else { "gardées" }));
    if let Err(e) = sys::close_app(Duration::from_secs(5)) {
        sys::log(&format!("fermeture : {e:#}"));
    }
    // Liste de la barre des tâches : elle pointerait vers un programme effacé.
    if let Err(e) = install::run_app(&dir, &["--clear-shell"], Duration::from_secs(15)) {
        sys::log(&format!("liste de la barre des tâches : {e:#}"));
    }
    if erase_data {
        // L'app efface ses mots de passe du gestionnaire d'identifiants.
        if let Err(e) = install::run_app(&dir, &["--forget-all"], Duration::from_secs(15)) {
            sys::log(&format!("mots de passe : {e:#}"));
        }
    }
    for link in sys::shortcuts() {
        let _ = std::fs::remove_file(link);
    }
    // Restes d'une mise à jour interrompue, à côté du dossier de l'app.
    if let Some(parent) = dir.parent() {
        for leftover in ["KyberNow.new", "KyberNow.old"] {
            let _ = std::fs::remove_dir_all(parent.join(leftover));
        }
    }
    sys::reg_delete(sys::UNINSTALL_KEY);
    sys::reg_delete(r"Software\Classes\kybernow");
    sys::reg_delete(r"Software\Classes\AppUserModelId\LesLignesClaires.KyberNow");
    if erase_data {
        // Le journal de la désinstallation est dans l'un de ces dossiers.
        sys::close_log();
        for d in sys::data_dirs() {
            let _ = std::fs::remove_dir_all(d);
        }
    }
}
