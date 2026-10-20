//! `KyberNow-Setup.exe` : lanceur de l'installation.
//!
//! Il porte le contenu à installer (accolé à la fin du fichier), dépose
//! l'interface et `WebView2Loader.dll` dans un dossier temporaire, puis lance
//! l'interface. Avec `--uninstall`, il désinstalle KyberNow.

#![cfg_attr(windows, windows_subsystem = "windows")]

use std::path::PathBuf;

const UI_EXE: &[u8] = include_bytes!(env!("KYBERNOW_UI_EXE"));
const WEBVIEW2_LOADER: &[u8] = include_bytes!(env!("KYBERNOW_WEBVIEW2_LOADER"));

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|a| a == "--uninstall") {
        uninstall();
        return;
    }
    if let Err(e) = launch() {
        message("KyberNow", &format!("L'installation n'a pas pu démarrer :\n\n{e:#}"), false);
    }
}

fn launch() -> anyhow::Result<()> {
    let exe = std::env::current_exe()?;
    if kybernow::payload::read_footer(&exe).is_err() {
        anyhow::bail!("ce fichier ne contient pas KyberNow. Téléchargez à nouveau l'installateur complet.");
    }
    if !webview2_installed() {
        message(
            "KyberNow",
            "L'installateur a besoin de Microsoft Edge WebView2, présent sur Windows 11 et sur la plupart des Windows 10.\n\nLa page de téléchargement de Microsoft va s'ouvrir : installez-le, puis relancez KyberNow-Setup.",
            false,
        );
        let _ = std::process::Command::new("cmd")
            .args(["/C", "start", "", "https://go.microsoft.com/fwlink/p/?LinkId=2124703"])
            .spawn();
        return Ok(());
    }

    let tmp: PathBuf = std::env::temp_dir().join(format!("KyberNow-Setup-{}", std::process::id()));
    std::fs::create_dir_all(&tmp)?;
    let ui = tmp.join("kybernow-ui.exe");
    std::fs::write(&ui, UI_EXE)?;
    std::fs::write(tmp.join("WebView2Loader.dll"), WEBVIEW2_LOADER)?;

    let status = std::process::Command::new(&ui)
        .arg("--payload")
        .arg(&exe)
        .arg("--webview-data")
        .arg(tmp.join("webview"))
        .status();

    for _ in 0..10 {
        if std::fs::remove_dir_all(&tmp).is_ok() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(300));
    }
    status?;
    Ok(())
}

fn uninstall() {
    if !message(
        "Désinstaller KyberNow",
        "KyberNow va être retiré de cet ordinateur : serveur de streaming, console d'administration, icône, écran virtuel. \
         Le mode d'alimentation et la catégorie du réseau d'avant KyberNow sont remis.\n\n\
         Le pilote de manettes ViGEmBus reste installé, car d'autres logiciels peuvent l'utiliser.\n\nContinuer ?",
        true,
    ) {
        return;
    }
    let keep_data = message(
        "Désinstaller KyberNow",
        "Garder les réglages de KyberNow sur cet ordinateur ?\n\n\
         Oui : C:\\ProgramData\\KyberNow reste (réglages et certificat de la console, appareils connus, journaux, \
         copie des réglages de Kyber), lisible par les administrateurs seulement.\n\
         Non : tout est effacé.",
        true,
    );
    let done = match kybernow::uninstall::run_uninstall(keep_data) {
        Ok(()) if keep_data => "KyberNow a été désinstallé. Ses réglages restent dans C:\\ProgramData\\KyberNow.".to_string(),
        Ok(()) => "KyberNow a été désinstallé.".to_string(),
        Err(e) => format!("La désinstallation a rencontré un problème :\n\n{e:#}"),
    };
    message("KyberNow", &done, false);
    // Après la boîte : le dossier est effacé une fois ce programme fermé.
    kybernow::uninstall::schedule_removal();
}

fn webview2_installed() -> bool {
    if !cfg!(windows) {
        return true;
    }
    const KEY: &str = "{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}";
    [
        format!(r"HKLM\SOFTWARE\WOW6432Node\Microsoft\EdgeUpdate\Clients\{KEY}"),
        format!(r"HKLM\SOFTWARE\Microsoft\EdgeUpdate\Clients\{KEY}"),
        format!(r"HKCU\SOFTWARE\Microsoft\EdgeUpdate\Clients\{KEY}"),
    ]
    .iter()
    .any(|k| {
        kybernow::sys::run("reg", &["query", k, "/v", "pv"]).is_ok_and(|o| {
            o.status.success() && !String::from_utf8_lossy(&o.stdout).contains("0.0.0.0")
        })
    })
}

/// Boîte de dialogue Windows. Avec `ask`, renvoie `true` si l'utilisateur accepte.
#[cfg(windows)]
fn message(title: &str, text: &str, ask: bool) -> bool {
    use windows_sys::Win32::UI::WindowsAndMessaging::{IDYES, MB_ICONINFORMATION, MB_ICONQUESTION, MB_OK, MB_YESNO, MessageBoxW};
    let wide = |s: &str| s.encode_utf16().chain(std::iter::once(0)).collect::<Vec<u16>>();
    let (t, m) = (wide(title), wide(text));
    let flags = if ask { MB_YESNO | MB_ICONQUESTION } else { MB_OK | MB_ICONINFORMATION };
    unsafe { MessageBoxW(std::ptr::null_mut(), m.as_ptr(), t.as_ptr(), flags) == IDYES }
}

#[cfg(not(windows))]
fn message(title: &str, text: &str, ask: bool) -> bool {
    println!("[{title}] {text}");
    ask
}
