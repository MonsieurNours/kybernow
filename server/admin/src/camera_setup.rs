//! La caméra de l'appareil sur une Station Windows (docs/webcam.md) :
//! « KyberNow Camera » inscrite dans Windows au démarrage d'Admin si elle ne
//! l'est pas. Jusqu'à la 0.11.8, seul l'installateur lançait
//! `kybernow-camera --install` : une Station mise à jour depuis Admin n'avait
//! jamais de caméra, et l'app disait « n'a pas pu ouvrir sa caméra virtuelle ».
//! Admin tourne en SYSTEM : il peut l'inscrire lui-même. Une fois par version
//! (marque `camera-install-<version>`), jamais bloquant, tout est journalisé.

use std::path::Path;
use std::process::Command;
use std::time::Duration;

use crate::update::windows_common::exit_text;

const PROBE_TIMEOUT: Duration = Duration::from_secs(10);
const INSTALL_TIMEOUT: Duration = Duration::from_secs(60);

/// Ce qu'il faut faire, d'après `--probe` : `true` s'il faut inscrire.
pub fn needs_install(probe: Option<&(Option<i32>, String)>) -> bool {
    !matches!(probe, Some((Some(0), _)))
}

pub fn ensure_registered(kyber_dir: &Path, data_dir: &Path) {
    if !cfg!(windows) {
        return;
    }
    let tool = kyber_dir.join("kybernow-camera.exe");
    if !tool.is_file() {
        return;
    }
    let mark = data_dir.join(format!("camera-install-{}", env!("CARGO_PKG_VERSION")));
    if mark.exists() {
        return;
    }
    // Marques des versions précédentes : effacées.
    for entry in std::fs::read_dir(data_dir).into_iter().flatten().flatten() {
        if entry.file_name().to_string_lossy().starts_with("camera-install-") {
            let _ = std::fs::remove_file(entry.path());
        }
    }
    let mut probe = Command::new(&tool);
    probe.arg("--probe");
    let probed = crate::micro::run(probe, PROBE_TIMEOUT);
    if needs_install(probed.as_ref()) {
        let mut install = Command::new(&tool);
        install.arg("--install");
        match crate::micro::run(install, INSTALL_TIMEOUT) {
            Some((Some(0), _)) => tracing::info!("caméra « KyberNow Camera » inscrite dans Windows"),
            Some((code, err)) => tracing::warn!("caméra « KyberNow Camera » non inscrite ({}) : {err}", exit_text(code)),
            None => tracing::warn!("caméra « KyberNow Camera » non inscrite : kybernow-camera --install ne répond pas"),
        }
    }
    let _ = std::fs::write(&mark, b"");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn la_camera_s_inscrit_sauf_si_elle_est_prete() {
        assert!(!needs_install(Some(&(Some(0), String::new()))));
        assert!(needs_install(Some(&(Some(1), "virtual camera not installed".into()))));
        assert!(needs_install(None));
    }

    #[test]
    fn un_plantage_se_lit_dans_le_journal() {
        assert_eq!(exit_text(Some(-1073741819)), "plantage 0xC0000005 : accès mémoire interdit");
        assert_eq!(exit_text(Some(1)), "code 1");
        assert_eq!(exit_text(None), "arrêté sans code");
    }
}
