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
    // Par le client HTTPS de l'installateur (rustls, autorités du web), pas
    // par PowerShell (restreint sur certains PC d'entreprise) ; le fichier
    // n'est lancé qu'entier et signé par Microsoft.
    let fetched = crate::account::download(BOOTSTRAPPER, std::time::Duration::from_secs(60))
        .and_then(|body| std::fs::write(&exe, body).map_err(Into::into))
        .and_then(|()| sys::verify_signature(&exe, "Microsoft Corporation"));
    if let Err(e) = fetched {
        let _ = std::fs::remove_dir_all(&dir);
        return Err(crate::i18n::err("errors.webview2Download", &[("error", &e)]));
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
