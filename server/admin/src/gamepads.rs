//! Manettes de la Station : `gamepads` de `GET /api/stream/info`
//! (docs/station-macos.md, « Manettes »).
//!
//! Sous Windows, les manettes passent par ViGEmBus : toujours annoncées. Sur
//! Mac, kynputserver les crée par l'aide « KyberNow Manettes » (CoreHID), que
//! macOS n'autorise qu'avec le droit d'Apple `com.apple.developer.hid.virtual.device` :
//! `kybernow-gamepads --check` le dit, une fois par lancement d'Admin (le droit
//! ne change qu'avec une mise à jour de l'app).
//!
//! `reason` quand elles manquent : `helper` (aide absente : Mac de
//! développement, app incomplète), `entitlement` (macOS refuse : Apple n'a
//! pas encore accordé le droit).

use serde_json::{json, Value};

pub fn info() -> Value {
    match unavailable() {
        None => json!({ "available": true, "reason": null }),
        Some(reason) => json!({ "available": false, "reason": reason }),
    }
}

#[cfg(not(target_os = "macos"))]
fn unavailable() -> Option<&'static str> {
    None
}

#[cfg(target_os = "macos")]
fn unavailable() -> Option<&'static str> {
    use std::path::PathBuf;
    use std::process::{Command, Stdio};
    use std::sync::OnceLock;
    use std::time::{Duration, Instant};

    /// Comme kynputserver (correctif Kyber Mac 0011) : `KYBERNOW_GAMEPADS_HELPER`,
    /// sinon l'aide de l'app, à côté de kyber-admin.
    fn helper() -> Option<PathBuf> {
        std::env::var_os("KYBERNOW_GAMEPADS_HELPER")
            .filter(|p| !p.is_empty())
            .map(PathBuf::from)
            .or_else(|| {
                let exe = std::env::current_exe().ok()?;
                Some(exe.parent()?.join("../Helpers/KyberNow Manettes.app/Contents/MacOS/kybernow-gamepads"))
            })
            .filter(|p| p.is_file())
    }

    static CHECK: OnceLock<Option<&'static str>> = OnceLock::new();
    *CHECK.get_or_init(|| {
        let Some(path) = helper() else {
            tracing::info!("manettes indisponibles : aide KyberNow Manettes absente");
            return Some("helper");
        };
        let mut child = match Command::new(&path).arg("--check").stdout(Stdio::null()).stderr(Stdio::null()).spawn() {
            Ok(c) => c,
            Err(e) => {
                tracing::warn!("manettes : aide {} non lancée : {e}", path.display());
                return Some("helper");
            }
        };
        let deadline = Instant::now() + Duration::from_secs(5);
        let status = loop {
            match child.try_wait() {
                Ok(Some(s)) => break Some(s),
                Ok(None) if Instant::now() < deadline => std::thread::sleep(Duration::from_millis(20)),
                _ => {
                    let _ = child.kill();
                    let _ = child.wait();
                    break None;
                }
            }
        };
        match status.and_then(|s| s.code()) {
            Some(0) => None,
            Some(2) => {
                tracing::info!(
                    "manettes indisponibles : macOS refuse les manettes virtuelles (droit d'Apple \
                     com.apple.developer.hid.virtual.device pas encore accordé)"
                );
                Some("entitlement")
            }
            other => {
                tracing::warn!("manettes : vérification de l'aide sans réponse claire ({other:?})");
                Some("helper")
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shape() {
        let v = info();
        let available = v["available"].as_bool().unwrap();
        assert_eq!(available, v["reason"].is_null());
        if !available {
            assert!(["helper", "entitlement"].contains(&v["reason"].as_str().unwrap()));
        }
    }
}
