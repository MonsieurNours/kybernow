//! Exécution de commandes Windows et emplacements système.

use std::path::PathBuf;
use std::process::{Command, Output};

use anyhow::{Context, Result, bail};

/// Lance une commande sans fenêtre de console et renvoie sa sortie.
pub fn run(program: &str, args: &[&str]) -> Result<Output> {
    let mut cmd = Command::new(program);
    cmd.args(args);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    cmd.output().with_context(|| format!("lancement de {program}"))
}

/// Comme [`run`], mais échoue si le code de sortie n'est pas dans `ok_codes`.
pub fn run_ok(program: &str, args: &[&str], ok_codes: &[i32]) -> Result<Output> {
    let out = run(program, args)?;
    let code = out.status.code().unwrap_or(-1);
    if !ok_codes.contains(&code) {
        bail!(
            "{program} {} a échoué (code {code}) : {}",
            args.join(" "),
            text(&out).trim()
        );
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
