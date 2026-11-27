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
use std::process::Command;
use std::time::Duration;

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

/// Le programme d'installation de VB-CABLE livré avec la Station (Windows) :
/// celui de l'installateur (`drivers\vbcable`), sinon celui que la mise à
/// jour apporte avec Admin (`admin\vbcable`).
fn vbcable_setup(kyber_dir: &Path) -> Option<PathBuf> {
    let install = kyber_dir.parent()?;
    [install.join("drivers").join("vbcable"), install.join("admin").join("vbcable")]
        .into_iter()
        .map(|d| d.join("VBCABLE_Setup_x64.exe"))
        .find(|p| p.is_file())
}

/// Marque posée quand VB-CABLE est là (déjà présent, ou installé avec
/// succès) : il n'est plus jamais réinstallé ensuite, même retiré exprès.
const VBCABLE_MARK: &str = "micro-vbcable";

/// Le choix « Micro de vos appareils » de l'installateur (`microphone` de
/// `kybernow.json`) : `None` pour une installation d'avant la 0.11.8.
fn microphone_chosen(kyber_dir: &Path) -> Option<bool> {
    let record: Value = serde_json::from_slice(&std::fs::read(kyber_dir.parent()?.join("kybernow.json")).ok()?).ok()?;
    record["microphone"].as_bool()
}

/// Au démarrage d'Admin (Windows) : installe VB-CABLE s'il manque, qu'il est
/// livré et que le micro a été choisi à l'installation. C'est ainsi que la
/// mise à jour depuis Admin l'apporte : le pack voyage dans le dossier
/// `admin` du paquet. Une seule marque (`micro-vbcable`), posée seulement
/// quand VB-CABLE est là : un échec est réessayé au démarrage suivant, un
/// retrait volontaire est respecté. Jamais bloquant.
pub fn ensure_on_new_version(kyber_dir: &Path, data_dir: &Path) {
    if !cfg!(windows) {
        return;
    }
    // Une aide tuée (fin de session brutale) a laissé CABLE en micro par
    // défaut : `--check` rend celui d'avant (registre `micro-defaut.txt`).
    if data_dir.join("micro-defaut.txt").exists()
        && let Some(h) = helper(kyber_dir)
    {
        let mut cmd = Command::new(h);
        cmd.arg("--check");
        let _ = crate::proc::code_and_stderr(cmd, CHECK_TIMEOUT);
    }
    let mark = data_dir.join(VBCABLE_MARK);
    // Marques d'avant la 0.11.8, une par version : VB-CABLE a déjà été proposé.
    let mut old_marks = false;
    for entry in std::fs::read_dir(data_dir).into_iter().flatten().flatten() {
        if entry.file_name().to_string_lossy().starts_with("micro-vbcable-") {
            old_marks = true;
            let _ = std::fs::remove_file(entry.path());
        }
    }
    if old_marks {
        let _ = std::fs::write(&mark, b"");
    }
    if mark.exists() {
        return;
    }
    if microphone_chosen(kyber_dir) == Some(false) {
        tracing::info!("micro : non choisi à l'installation, VB-CABLE pas installé");
        return;
    }
    let Some(setup) = vbcable_setup(kyber_dir) else { return };
    let present = || {
        matches!(helper(kyber_dir).map(|h| {
            let mut cmd = Command::new(h);
            cmd.arg("--check");
            crate::proc::code_and_stderr(cmd, CHECK_TIMEOUT)
        }), Some(Some((Some(0), _))))
    };
    if present() {
        tracing::info!("micro : VB-CABLE déjà présent");
    } else {
        let mut cmd = Command::new(&setup);
        cmd.args(["-i", "-h"]);
        let outcome = crate::proc::code_and_stderr(cmd, INSTALL_TIMEOUT);
        if !present() {
            tracing::warn!("micro : VB-CABLE pas installé ({outcome:?}) : nouvel essai au prochain démarrage");
            return;
        }
        tracing::info!("micro : VB-CABLE installé après la mise à jour (VB-Audio Software, donationware : vb-cable.com)");
    }
    let _ = std::fs::write(&mark, b"");
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
    match crate::proc::code_and_stderr(cmd, CHECK_TIMEOUT) {
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
        return Err(ApiError::msg(StatusCode::NOT_FOUND, crate::i18n::Msg::new("api.vbcableMissing")));
    };
    let result = blocking(move || {
        let mut cmd = Command::new(&setup);
        cmd.args(["-i", "-h"]);
        let outcome = crate::proc::code_and_stderr(cmd, INSTALL_TIMEOUT);
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
    fn le_pack_de_vbcable_se_trouve_aussi_dans_admin() {
        let root = std::env::temp_dir().join(format!("kybernow-vbcable-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let kyber = root.join("kyber");
        std::fs::create_dir_all(&kyber).unwrap();
        assert!(vbcable_setup(&kyber).is_none());
        // Apporté par la mise à jour, dans le dossier d'Admin.
        std::fs::create_dir_all(root.join("admin/vbcable")).unwrap();
        std::fs::write(root.join("admin/vbcable/VBCABLE_Setup_x64.exe"), b"").unwrap();
        assert_eq!(vbcable_setup(&kyber).unwrap(), root.join("admin/vbcable/VBCABLE_Setup_x64.exe"));
        // Celui de l'installateur passe d'abord.
        std::fs::create_dir_all(root.join("drivers/vbcable")).unwrap();
        std::fs::write(root.join("drivers/vbcable/VBCABLE_Setup_x64.exe"), b"").unwrap();
        assert_eq!(vbcable_setup(&kyber).unwrap(), root.join("drivers/vbcable/VBCABLE_Setup_x64.exe"));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn le_choix_du_micro_se_lit_dans_kybernow_json() {
        let root = std::env::temp_dir().join(format!("kybernow-micro-choix-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("kyber")).unwrap();
        assert_eq!(microphone_chosen(&root.join("kyber")), None);
        std::fs::write(root.join("kybernow.json"), r#"{"version":"0.11.8","microphone":false}"#).unwrap();
        assert_eq!(microphone_chosen(&root.join("kyber")), Some(false));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn sans_aide_la_carte_le_dit() {
        let dir = std::env::temp_dir().join("kybernow-micro-absent");
        // SAFETY: test seul à lire cette variable.
        unsafe { std::env::set_var("KYBERNOW_MICRO", dir.join("rien")) };
        assert_eq!(status(&dir)["state"], "unsupported");
        unsafe { std::env::remove_var("KYBERNOW_MICRO") };
    }
}
