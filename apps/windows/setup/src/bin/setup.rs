//! `KyberNow-Link-Setup.exe` : installateur de KyberNow Link, l'app Windows.
//!
//! Il porte les deux versions de l'app (x64 et ARM64) et pose celle du
//! processeur. Sans option : installe WebView2 s'il manque, dépose
//! l'interface dans `%TEMP%` (aucun droit d'administrateur : rien à
//! détourner) et la lance.
//!
//! - `/S` : silencieux ; `/S /UPDATE` : mise à jour lancée par l'app, qui
//!   attend sa fermeture puis la rouvre (`--updated`).
//! - `--uninstall [/S]` : désinstallation (copie sans contenu, `uninstall.exe`).

#![cfg_attr(windows, windows_subsystem = "windows")]

#[cfg(not(windows))]
fn main() {
    eprintln!("KyberNow-Link-Setup ne tourne que sous Windows.");
}

#[cfg(windows)]
fn main() {
    windows::main();
}

#[cfg(windows)]
mod windows {
    use std::path::{Path, PathBuf};
    use std::sync::Arc;
    use std::time::Duration;

    use knsetup::events::Emit;
    use knsetup::i18n::{describe, lang, tr};
    use knsetup::{install, payload, sys, uninstall, webview2};

    const UI_EXE: &[u8] = include_bytes!(env!("KYBERNOW_SETUP_UI_EXE"));
    const WEBVIEW2_LOADER: &[u8] = include_bytes!(env!("KYBERNOW_WEBVIEW2_LOADER"));
    const TITLE: &str = sys::DISPLAY_NAME;

    pub fn main() {
        let args: Vec<String> = std::env::args().skip(1).collect();
        let has = |flag: &str| args.iter().any(|a| a.eq_ignore_ascii_case(flag));
        let silent = has("/S");
        // Jamais le dossier de l'app comme dossier courant : Windows refuserait
        // de le renommer (l'app lance la mise à jour depuis ce dossier).
        let _ = std::env::set_current_dir(std::env::temp_dir());
        sys::init_log("setup");
        let exe = std::env::current_exe().unwrap_or_default();
        // uninstall.exe ouvert à la main (sans --uninstall) : même chose.
        let stub = exe.file_name().is_some_and(|n| n.eq_ignore_ascii_case(sys::UNINSTALLER));
        if has("--uninstall") || stub {
            run_uninstall(silent);
            return;
        }
        if let Err(e) = payload::read_footer(&exe) {
            sys::log(&format!("contenu illisible : {e:#}"));
            if !silent {
                sys::message(TITLE, &tr(lang(), "setup.notInstaller", &[("error", &describe(lang(), &e))]), false);
            }
            std::process::exit(2);
        }
        if silent {
            std::process::exit(silent_install(&exe, has("/UPDATE")));
        }
        if let Err(e) = launch_ui(&exe) {
            sys::log(&format!("interface : {e:#}"));
            sys::message(TITLE, &tr(lang(), "setup.cannotStart", &[("error", &describe(lang(), &e))]), false);
        }
    }

    /// Sans fenêtre : installation scriptée ou mise à jour depuis l'app.
    fn silent_install(exe: &Path, update: bool) -> i32 {
        let emit: Emit = Arc::new(|_| {});
        // L'app se ferme d'elle-même juste après avoir lancé la mise à jour.
        let wait = Duration::from_secs(if update { 20 } else { 5 });
        let desktop = sys::has_desktop_shortcut();
        match install::run_install(exe, desktop, wait, &emit) {
            Ok(()) => {
                if update {
                    let _ = sys::launch_app(&["--updated"]);
                }
                0
            }
            Err(e) => {
                if update {
                    sys::message(TITLE, &tr(lang(), "setup.updateFailed", &[("error", &describe(lang(), &e))]), false);
                    let _ = sys::launch_app(&[]);
                }
                1
            }
        }
    }

    fn launch_ui(exe: &Path) -> anyhow::Result<()> {
        if webview2::version().is_none() {
            if !sys::message(TITLE, &tr(lang(), "setup.webview2Ask", &[]), true) {
                return Ok(());
            }
            if let Err(e) = webview2::install() {
                sys::message(TITLE, &tr(lang(), "setup.webview2Failed", &[("error", &describe(lang(), &e))]), false);
                sys::open_url("https://go.microsoft.com/fwlink/p/?LinkId=2124703");
                return Ok(());
            }
        }

        let pid = std::process::id();
        let tmp: PathBuf = std::env::temp_dir().join(format!("KyberNow-Link-Setup-{pid}"));
        if std::fs::symlink_metadata(&tmp).is_ok() {
            std::fs::remove_dir_all(&tmp)?;
        }
        std::fs::create_dir_all(&tmp)?;
        let ui = tmp.join("kybernow-client-setup-ui.exe");
        std::fs::write(&ui, UI_EXE)?;
        std::fs::write(tmp.join("WebView2Loader.dll"), WEBVIEW2_LOADER)?;
        let webview_data = std::env::temp_dir().join(format!("KyberNow-Link-Setup-WebView-{pid}"));

        let status = std::process::Command::new(&ui).arg("--payload").arg(exe).arg("--webview-data").arg(&webview_data).status();

        // WebView2 relâche ses fichiers un peu après la fermeture de l'interface.
        for dir in [&tmp, &webview_data] {
            for _ in 0..10 {
                if std::fs::remove_dir_all(dir).is_ok() || !dir.exists() {
                    break;
                }
                std::thread::sleep(Duration::from_millis(300));
            }
        }
        status?;
        Ok(())
    }

    fn run_uninstall(silent: bool) {
        let title = tr(lang(), "uninstall.title", &[]);
        if !silent && !sys::message(&title, &tr(lang(), "uninstall.confirm", &[]), true) {
            return;
        }
        let erase = !silent && sys::message(&title, &tr(lang(), "uninstall.erase", &[]), true);
        uninstall::run_uninstall(erase);
        if !silent {
            sys::message(TITLE, &tr(lang(), "uninstall.done", &[]), false);
        }
        // Après la boîte : le dossier (où est ce programme) est effacé une fois fermé.
        sys::schedule_removal(&sys::install_dir());
    }
}
