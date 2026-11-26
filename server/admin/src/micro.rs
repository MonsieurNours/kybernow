//! Le micro virtuel de la Station (docs/micro.md) : la carte « Micro
//! virtuel » d'Admin › Image et son.
//!
//! L'aide `kybernow-micro` (server/micro), que kycontroller lance pour chaque
//! session qui envoie son micro, dit si le micro virtuel est là
//! (`--check` : 0 prêt, 3 absent, autre : impossible). Sur Windows, il manque
//! tant que VB-CABLE n'est pas installé : `POST /api/micro/install` lance le
//! programme d'installation de VB-Audio livré avec la Station
//! (`drivers\vbcable`), sans fenêtre.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use serde_json::{Value, json};

use crate::AppState;
use crate::api::{ApiError, ApiResult, blocking};

const CHECK_TIMEOUT: Duration = Duration::from_secs(5);
const INSTALL_TIMEOUT: Duration = Duration::from_secs(120);

/// L'aide, à côté de kycontroller.
fn helper(kyber_dir: &Path) -> Option<PathBuf> {
    if let Some(p) = std::env::var_os("KYBERNOW_MICRO").filter(|p| !p.is_empty()) {
        return Some(PathBuf::from(p)).filter(|p| p.is_file());
    }
    let p = if cfg!(windows) {
        kyber_dir.join("kybernow-micro.exe")
    } else if cfg!(target_os = "linux") {
        // /usr/lib/kybernow-station/bin/kyber-admin → ../runtime/bin
        std::env::current_exe().ok()?.parent()?.parent()?.join("runtime/bin/kybernow-micro")
    } else {
        std::env::current_exe().ok()?.parent()?.join("kybernow-micro")
    };
    p.is_file().then_some(p)
}

/// Le programme d'installation de VB-CABLE livré avec la Station (Windows).
fn vbcable_setup(kyber_dir: &Path) -> Option<PathBuf> {
    let p = kyber_dir.parent()?.join("drivers").join("vbcable").join("VBCABLE_Setup_x64.exe");
    p.is_file().then_some(p)
}

fn no_window(cmd: &mut Command) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }
    #[cfg(not(windows))]
    let _ = cmd;
}

/// Attend la fin de `cmd` (`timeout` au plus) ; `None` s'il ne se lance pas ou dépasse.
fn run(mut cmd: Command, timeout: Duration) -> Option<(Option<i32>, String)> {
    cmd.stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::piped());
    no_window(&mut cmd);
    let mut child = cmd.spawn().ok()?;
    let deadline = Instant::now() + timeout;
    let status = loop {
        match child.try_wait() {
            Ok(Some(s)) => break s,
            Ok(None) if Instant::now() < deadline => std::thread::sleep(Duration::from_millis(50)),
            _ => {
                let _ = child.kill();
                return None;
            }
        }
    };
    let mut err = String::new();
    if let Some(mut e) = child.stderr.take() {
        use std::io::Read;
        let _ = e.read_to_string(&mut err);
    }
    Some((status.code(), err.trim().to_owned()))
}

/// L'état de la carte : `ready` (avec `device`, le nom que prennent les apps
/// de l'ordinateur), `missing` (avec `installable` sur Windows quand VB-CABLE
/// est livré), `unsupported`.
pub fn status(kyber_dir: &Path) -> Value {
    let Some(helper) = helper(kyber_dir) else {
        return json!({ "state": "unsupported", "reason": "kybernow-micro introuvable" });
    };
    let mut cmd = Command::new(&helper);
    cmd.arg("--check");
    match run(cmd, CHECK_TIMEOUT) {
        Some((Some(0), err)) => {
            let device = err.rsplit(" : ").next().unwrap_or_default().to_owned();
            json!({ "state": "ready", "device": device })
        }
        Some((Some(3), err)) => json!({
            "state": "missing",
            "reason": err,
            "installable": cfg!(windows) && vbcable_setup(kyber_dir).is_some(),
        }),
        Some((_, err)) => json!({ "state": "unsupported", "reason": err }),
        None => json!({ "state": "unsupported", "reason": "kybernow-micro ne répond pas" }),
    }
}

/// `GET /api/micro`
pub async fn get(State(s): State<AppState>) -> ApiResult<Json<Value>> {
    let dir = s.kyber_dir.clone();
    Ok(Json(blocking(move || Ok(status(&dir))).await?))
}

/// `POST /api/micro/install` (Windows) : installe VB-CABLE, puis rend l'état.
pub async fn install(State(s): State<AppState>) -> ApiResult<Json<Value>> {
    let dir = s.kyber_dir.clone();
    let Some(setup) = vbcable_setup(&dir).filter(|_| cfg!(windows)) else {
        return Err(ApiError(StatusCode::NOT_FOUND, "VB-CABLE n'est pas livré avec cette Station".into()));
    };
    let result = blocking(move || {
        let mut cmd = Command::new(&setup);
        cmd.args(["-i", "-h"]);
        let outcome = run(cmd, INSTALL_TIMEOUT);
        tracing::info!("micro : VB-CABLE installé depuis Admin ({outcome:?})");
        Ok(status(&dir))
    })
    .await?;
    if result["state"] == "ready" {
        s.activity.event("info", crate::i18n::Msg::new("events.microInstalled"));
    }
    Ok(Json(result))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sans_aide_la_carte_le_dit() {
        let dir = std::env::temp_dir().join("kybernow-micro-absent");
        // SAFETY: test seul à lire cette variable.
        unsafe { std::env::set_var("KYBERNOW_MICRO", dir.join("rien")) };
        assert_eq!(status(&dir)["state"], "unsupported");
        unsafe { std::env::remove_var("KYBERNOW_MICRO") };
    }
}
