//! Exécution de commandes Windows et emplacements système.

use std::path::PathBuf;
use std::process::{Command, Output};

use anyhow::{Context, Result, bail};

use crate::i18n::t;

/// Outils de Windows lancés par leur nom : toujours par leur chemin complet
/// sous `%SystemRoot%\System32`. Sous Windows, un nom nu est d'abord cherché
/// dans le dossier de l'exécutable courant : un `reg.exe` déposé à côté de
/// `Setup.exe` dans Téléchargements s'exécuterait en administrateur.
pub fn resolve(program: &str) -> PathBuf {
    if !cfg!(windows) || program.contains(['\\', '/']) {
        return PathBuf::from(program);
    }
    let lower = program.to_ascii_lowercase();
    let name = if lower.ends_with(".exe") { lower } else { format!("{lower}.exe") };
    let system32 = system_root().join("System32");
    match name.as_str() {
        "powershell.exe" => system32.join(r"WindowsPowerShell\v1.0\powershell.exe"),
        _ => system32.join(name),
    }
}

/// Lance une commande sans fenêtre de console et renvoie sa sortie.
pub fn run(program: &str, args: &[&str]) -> Result<Output> {
    let mut cmd = Command::new(resolve(program));
    cmd.args(args);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    cmd.output().with_context(|| t("install.launching", &[("program", &program)]))
}

/// Comme [`run`], mais échoue si le code de sortie n'est pas dans `ok_codes`.
pub fn run_ok(program: &str, args: &[&str], ok_codes: &[i32]) -> Result<Output> {
    let out = run(program, args)?;
    let code = out.status.code().unwrap_or(-1);
    if !ok_codes.contains(&code) {
        let command = format!("{program} {}", args.join(" "));
        bail!(t("install.commandFailed", &[("command", &command), ("code", &code), ("output", &text(&out).trim())]));
    }
    Ok(out)
}

/// Sortie standard et d'erreur, décodées au mieux (console Windows en OEM).
pub fn text(out: &Output) -> String {
    let mut s = String::from_utf8_lossy(&out.stdout).into_owned();
    s.push_str(&String::from_utf8_lossy(&out.stderr));
    s
}

/// Lance PowerShell sans profil et renvoie la sortie standard, en UTF-8
/// (sinon PowerShell 5 écrit dans la page de code OEM : « Réseau » arriverait abîmé).
pub fn powershell(script: &str) -> Result<String> {
    let script = format!("[Console]::OutputEncoding = [Text.Encoding]::UTF8; {script}");
    let out = run(
        "powershell.exe",
        &["-NoProfile", "-NonInteractive", "-ExecutionPolicy", "Bypass", "-Command", &script],
    )?;
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

fn env_dir(var: &str, fallback: &str) -> PathBuf {
    std::env::var_os(var).map(PathBuf::from).unwrap_or_else(|| PathBuf::from(fallback))
}

/// `C:\Program Files\KyberNow`
pub fn install_dir() -> PathBuf {
    if cfg!(windows) {
        env_dir("ProgramFiles", r"C:\Program Files").join(crate::PRODUCT)
    } else {
        std::env::temp_dir().join("KyberNow-simulation")
    }
}

/// `C:\ProgramData\KyberNow` (réglages de la console, certificat, sauvegardes)
pub fn data_dir() -> PathBuf {
    if cfg!(windows) {
        env_dir("ProgramData", r"C:\ProgramData").join(crate::PRODUCT)
    } else {
        std::env::temp_dir().join("KyberNow-simulation-data")
    }
}

pub fn system_root() -> PathBuf {
    env_dir("SystemRoot", r"C:\Windows")
}

/// Adresse web qu'on peut confier au navigateur : `https://` (ou `http://`),
/// sans espace, guillemet ni caractère de contrôle.
pub fn safe_web_url(url: &str) -> bool {
    (url.starts_with("https://") || url.starts_with("http://"))
        && url.len() <= 4096
        && !url.chars().any(|c| c.is_control() || c.is_whitespace() || c == '"')
}

/// Ouvre une adresse web dans le navigateur par défaut. Sous Windows par
/// `ShellExecuteW` : jamais par `cmd /C start`, qui exécuterait ce qui suit un
/// `&` de l'adresse.
pub fn open_url(url: &str) {
    if !safe_web_url(url) {
        return;
    }
    #[cfg(windows)]
    {
        use windows_sys::Win32::UI::Shell::ShellExecuteW;
        use windows_sys::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
        let wide = |s: &str| s.encode_utf16().chain(std::iter::once(0)).collect::<Vec<u16>>();
        let (verb, target) = (wide("open"), wide(url));
        unsafe {
            ShellExecuteW(std::ptr::null_mut(), verb.as_ptr(), target.as_ptr(), std::ptr::null(), std::ptr::null(), SW_SHOWNORMAL);
        }
    }
    #[cfg(target_os = "macos")]
    let _ = Command::new("/usr/bin/open").arg(url).spawn();
    #[cfg(not(any(windows, target_os = "macos")))]
    let _ = Command::new("xdg-open").arg(url).spawn();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_plain_web_addresses_reach_the_browser() {
        assert!(safe_web_url("https://share.kybernow.app/compte?a=1&b=2"));
        assert!(!safe_web_url("https://x/?a&calc.exe\" & calc"));
        assert!(!safe_web_url("https://x/ a"));
        assert!(!safe_web_url("file:///C:/Windows/System32/calc.exe"));
        assert!(!safe_web_url("javascript:alert(1)"));
    }

    #[test]
    fn bare_tool_names_resolve_outside_the_current_folder() {
        // Hors Windows, rien ne change (simulation).
        assert_eq!(resolve("reg"), PathBuf::from("reg"));
        assert_eq!(resolve("C:/x/nefconc.exe"), PathBuf::from("C:/x/nefconc.exe"));
    }
}
