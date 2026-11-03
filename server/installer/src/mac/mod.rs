//! KyberNow Station pour Mac (docs/station-macos.md, jalon 6) : installation,
//! autorisations, désinstallation et mise à jour, derrière les fenêtres de
//! `ui/src/mac`.
//!
//! Tout tient dans une app, `/Applications/KyberNow Station.app` : l'icône de
//! la barre des menus (exécutable principal), Kyber (kycontroller, kyavserver,
//! kynputserver), Admin (`kyber-admin`), l'agent des écrans virtuels
//! (`kybernow-display`), cette interface (`kybernow-ui`) et, dans ses
//! ressources, la sortie son `KyberNow.driver` et le moteur du client web.
//! macOS rattache les autorisations (écran, accessibilité, micro) au paquet
//! qui contient l'exécutable : une seule ligne « KyberNow Station » à cocher
//! dans les réglages, pour Kyber comme pour Admin (essai du 03/11/2026).
//!
//! Tout tourne dans la session de l'utilisateur (LaunchAgents de
//! `~/Library/LaunchAgents`), comme la Station d'essai des jalons 1 à 7 :
//! l'écran virtuel, le son et les apps ne vivent que dans une session
//! ouverte. Seules trois choses demandent le mot de passe du Mac, en une
//! fois (`root`) : copier l'app dans Applications, poser la sortie son dans
//! `/Library/Audio/Plug-Ins/HAL`, et régler le pare-feu et le réveil par le
//! réseau.

pub mod checks;
pub mod commands;
pub mod install;
pub mod permissions;
pub mod root;
pub mod uninstall;
pub mod update;

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

/// Identifiant du paquet de l'app installée (autorisations, mises à jour).
pub const BUNDLE_ID: &str = "fr.leslignesclaires.kybernow.station";
pub const APP_NAME: &str = "KyberNow Station.app";
/// Exécutable principal de l'app : l'icône de la barre des menus.
pub const MENUBAR_EXE: &str = "KyberNow Station";
/// Services launchd de la session, dans l'ordre de démarrage.
pub const LABEL_KYBER: &str = "fr.leslignesclaires.kybernow.station.kyber";
pub const LABEL_ADMIN: &str = "fr.leslignesclaires.kybernow.station.admin";
pub const LABEL_MENUBAR: &str = "fr.leslignesclaires.kybernow.station.menubar";
pub const LABELS: [&str; 3] = [LABEL_KYBER, LABEL_ADMIN, LABEL_MENUBAR];
/// Sortie son KyberNow (pilote Core Audio, `server/macos/audio-driver`).
pub const DRIVER_NAME: &str = "KyberNow.driver";
pub const HAL_DIR: &str = "/Library/Audio/Plug-Ins/HAL";
/// Encodeur de Kyber sur Mac : la puce Apple, par VideoToolbox.
pub const ENCODER: &str = "videotoolbox";

pub fn home() -> PathBuf {
    std::env::var_os("HOME").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("/tmp"))
}

/// `/Applications/KyberNow Station.app` (`KYBERNOW_APP_DIR` pour les essais).
pub fn app_path() -> PathBuf {
    std::env::var_os("KYBERNOW_APP_DIR").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("/Applications")).join(APP_NAME)
}

/// Exécutables de l'app.
pub fn app_exe(app: &Path, name: &str) -> PathBuf {
    app.join("Contents/MacOS").join(name)
}

pub fn app_resource(app: &Path, name: &str) -> PathBuf {
    app.join("Contents/Resources").join(name)
}

/// `~/Library/Application Support/KyberNow` : dossier partagé avec l'agent des
/// écrans (`display-off.json`, `hdr-sdr-white.json`).
pub fn support_dir() -> PathBuf {
    home().join("Library/Application Support/KyberNow")
}

/// `~/Library/Application Support/KyberNow/Station` : `kybernow.json`,
/// `kyber/` (réglages et certificat de Kyber), `admin/` (données d'Admin).
pub fn station_dir() -> PathBuf {
    std::env::var_os("KYBERNOW_STATION_DIR").map(PathBuf::from).unwrap_or_else(|| support_dir().join("Station"))
}

pub fn kyber_dir() -> PathBuf {
    station_dir().join("kyber")
}

pub fn admin_dir() -> PathBuf {
    station_dir().join("admin")
}

/// `~/Library/Logs/KyberNow` : journaux d'Admin, de Kyber et de l'installation (Console.app les lit).
pub fn logs_dir() -> PathBuf {
    home().join("Library/Logs/KyberNow")
}

pub fn agents_dir() -> PathBuf {
    home().join("Library/LaunchAgents")
}

pub fn plist_path(label: &str) -> PathBuf {
    agents_dir().join(format!("{label}.plist"))
}

pub fn uid() -> u32 {
    unsafe { libc::getuid() }
}

pub fn user_name() -> String {
    std::env::var("USER").unwrap_or_else(|_| "root".into())
}

/// Domaine launchd de la session : `gui/<uid>`.
pub fn domain() -> String {
    format!("gui/{}", uid())
}

/// `kybernow.json` de la Station : ce qui est installé, relu par l'icône (port
/// et certificat d'Admin), la désinstallation et une réinstallation. Admin y
/// ajoute l'empreinte de son certificat (`console_fingerprint`).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Record {
    pub version: String,
    pub kyber_version: String,
    pub stream_port: u16,
    pub console_port: u16,
    #[serde(default)]
    pub virtual_display: bool,
    #[serde(default)]
    pub virtual_audio: bool,
    #[serde(default)]
    pub start_with_mac: bool,
    #[serde(default)]
    pub keep_reachable: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub console_fingerprint: Option<String>,
}

pub fn read_record() -> Option<Record> {
    serde_json::from_slice(&std::fs::read(station_dir().join("kybernow.json")).ok()?).ok()
}

pub fn write_record(record: &Record) -> Result<()> {
    let path = station_dir().join("kybernow.json");
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, serde_json::to_vec_pretty(record)?).with_context(|| tmp.display().to_string())?;
    std::fs::rename(&tmp, &path).with_context(|| path.display().to_string())?;
    Ok(())
}

/// `launchctl` avec ses arguments ; sortie et code rendus tels quels.
pub fn launchctl(args: &[&str]) -> Result<std::process::Output> {
    std::process::Command::new("/bin/launchctl").args(args).output().context("launchctl")
}

/// Le service est chargé dans la session.
pub fn loaded(label: &str) -> bool {
    launchctl(&["print", &format!("{}/{label}", domain())]).is_ok_and(|o| o.status.success())
}

/// Décharge un service de la session s'il l'est (arrêt du processus compris).
pub fn bootout(label: &str) {
    if loaded(label) {
        let _ = launchctl(&["bootout", &format!("{}/{label}", domain())]);
        // launchd rend la main avant la fin du processus : on attend qu'il ait disparu.
        for _ in 0..40 {
            if !loaded(label) {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(250));
        }
    }
}

/// Texte d'une sortie de commande (sortie standard puis d'erreur).
pub fn output_text(out: &std::process::Output) -> String {
    let mut s = String::from_utf8_lossy(&out.stdout).into_owned();
    s.push_str(&String::from_utf8_lossy(&out.stderr));
    s.trim().to_string()
}

/// Lance `program` et rend sa sortie standard, sans échouer.
pub fn read(program: &str, args: &[&str]) -> Option<String> {
    let out = std::process::Command::new(program).args(args).output().ok()?;
    out.status.success().then(|| String::from_utf8_lossy(&out.stdout).trim().to_string())
}
