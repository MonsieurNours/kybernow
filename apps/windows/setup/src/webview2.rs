//! Moteur WebView2 d'Edge : il affiche l'interface de l'app (et de cet
//! installateur). Présent sur Windows 11 et la plupart des Windows 10 ; sinon
//! installé pour cet utilisateur par l'installateur officiel de Microsoft.

use anyhow::Result;
use windows_sys::Win32::System::Registry::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE};

use crate::sys;

const CLIENT: &str = "{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}";
const BOOTSTRAPPER: &str = "https://go.microsoft.com/fwlink/p/?LinkId=2124703";

/// Version installée, ou `None`.
pub fn version() -> Option<String> {
    [
        (HKEY_LOCAL_MACHINE, format!(r"SOFTWARE\WOW6432Node\Microsoft\EdgeUpdate\Clients\{CLIENT}")),
        (HKEY_LOCAL_MACHINE, format!(r"SOFTWARE\Microsoft\EdgeUpdate\Clients\{CLIENT}")),
        (HKEY_CURRENT_USER, format!(r"Software\Microsoft\EdgeUpdate\Clients\{CLIENT}")),
    ]
    .into_iter()
    .filter_map(|(root, path)| sys::reg_get(root, &path, "pv"))
    .find(|v| !v.is_empty() && v != "0.0.0.0")
}

/// Télécharge et lance l'installateur de Microsoft (pour cet utilisateur).
pub fn install() -> Result<String> {
    let dir = std::env::temp_dir().join(format!("KyberNow-WebView2-{}", std::process::id()));
    std::fs::create_dir_all(&dir)?;
    let exe = dir.join("MicrosoftEdgeWebview2Setup.exe");
    sys::log("WebView2 absent : téléchargement de l'installateur de Microsoft");
    // Le chemin passe par une variable d'environnement : une apostrophe dans le
    // nom du dossier temporaire ne casse pas la commande.
    let out = {
        use std::os::windows::process::CommandExt;
        std::process::Command::new(sys::system32(r"WindowsPowerShell\v1.0\powershell.exe"))
            .args([
                "-NoProfile",
                "-NonInteractive",
                "-ExecutionPolicy",
                "Bypass",
                "-Command",
                &format!("$ProgressPreference='SilentlyContinue'; Invoke-WebRequest -UseBasicParsing -Uri '{BOOTSTRAPPER}' -OutFile $env:KYBERNOW_WV2"),
            ])
            .env("KYBERNOW_WV2", &exe)
            .creation_flags(sys::CREATE_NO_WINDOW)
            .output()
    };
    if !exe.exists() {
        let why = out.map(|o| String::from_utf8_lossy(&o.stderr).trim().to_string()).unwrap_or_else(|e| e.to_string());
        let _ = std::fs::remove_dir_all(&dir);
        return Err(crate::i18n::err("errors.webview2Download", &[("error", &why)]));
    }
    sys::log("installation de WebView2");
    let status = std::process::Command::new(&exe).args(["/silent", "/install"]).status();
    let _ = std::fs::remove_dir_all(&dir);
    match (status, version()) {
        (_, Some(v)) => Ok(v),
        (Ok(s), None) => Err(crate::i18n::err("errors.webview2Code", &[("code", &s.code().unwrap_or(-1))])),
        (Err(e), None) => Err(crate::i18n::err("errors.webview2Start", &[("error", &e)])),
    }
}
