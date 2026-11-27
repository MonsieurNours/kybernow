//! Écrans virtuels d'une Station Mac (docs/station-macos.md, jalon 3).
//!
//! Admin lance l'agent `kybernow-display` (`server/macos/display-agent`) et lui
//! parle par lignes JSON. L'agent tient les écrans `CGVirtualDisplay` tant
//! qu'il vit, fait de l'écran virtuel principal l'écran principal et éteint
//! les écrans du Mac un par un (comme sous Windows ; prêt : gardés allumés).
//! Filet de sécurité : les écrans virtuels partent avec l'agent ; les écrans
//! éteints sont notés sur disque et rallumés par l'agent suivant, par
//! `kybernow-display recover` au démarrage d'Admin, et par l'agent lui-même
//! si Admin s'arrête (fin de son entrée). C'est ce qui remplace, sur Mac, le
//! chien de garde de SudoVDA et `recover`.
//!
//! Le protocole, les écrans et la veille sont communs avec la Station Linux
//! (`agent_backend.rs`) ; ici, ce qui est propre au Mac : la reprise des
//! écrans au démarrage et le HDR. Nom d'écran : `mac:<CGDirectDisplayID>`.

use std::path::PathBuf;
use std::process::Command;

use anyhow::{Context, Result};
use serde_json::{Value, json};

use super::agent_backend::{AgentBackend, System};
use super::hdr;

pub use super::agent_backend::agent_path;

/// `~/Library/Application Support/KyberNow/hdr-sdr-white.json` : le blanc SDR
/// (nits) voulu par l'appareil, par écran (`CGDirectDisplayID`), lu par la
/// capture de Kyber au début de chaque image HDR (correctif Mac 0010).
fn write_sdr_whites(whites: &[(u32, f32)]) {
    let path = crate::system::home().join("Library/Application Support/KyberNow/hdr-sdr-white.json");
    let map: serde_json::Map<String, Value> = whites.iter().map(|(id, nits)| (id.to_string(), json!(nits))).collect();
    let _ = std::fs::create_dir_all(path.parent().unwrap());
    if let Err(e) = std::fs::write(&path, Value::Object(map).to_string()) {
        tracing::warn!("blanc SDR du HDR non écrit ({}) : {e}", path.display());
    }
}

/// Les écrans de la Station Mac. Écrans du Mac laissés éteints par un agent
/// arrêté en route (Admin ou l'agent tombé pendant une session) : rallumés au
/// démarrage (`kybernow-display recover`, fichier `display-off.json`).
pub fn backend(path: PathBuf) -> AgentBackend {
    let recover = path.clone();
    std::thread::spawn(move || match Command::new(&recover).arg("recover").output() {
        Ok(out) if out.status.success() => {}
        Ok(out) => tracing::warn!("reprise des écrans du Mac : {}", String::from_utf8_lossy(&out.stdout).trim()),
        Err(e) => tracing::warn!("reprise des écrans du Mac : {e}"),
    });
    AgentBackend::new(System::Mac, path, "CGVirtualDisplay (macOS)", Some(set_hdr))
}

/// Écran virtuel du Mac en HDR : même mode, fonction de transfert HDR
/// (`set_hdr` de l'agent), confirmé par macOS (2 s au plus).
fn set_hdr(backend: &AgentBackend, asks: &[hdr::HdrAsk]) -> Result<Vec<hdr::HdrSeen>> {
    // Blanc SDR de l'appareil pour chaque écran passé en HDR : la capture
    // de Kyber y remonte l'image (correctif Mac 0010 ; macOS met ce blanc
    // à 100 nits, Windows au blanc de l'appareil).
    let whites: Vec<(u32, f32)> = asks.iter().filter(|a| a.on).map(|a| (a.created.target, a.apply.as_ref().map_or(203.0, |c| c.sdr_white))).collect();
    if !whites.is_empty() {
        write_sdr_whites(&whites);
    }
    asks.iter()
        .map(|ask| {
            let started = std::time::Instant::now();
            let v = backend.call(json!({ "cmd": "set_hdr", "id": ask.created.target, "on": ask.on }), false)?.context("agent arrêté")?;
            let supported = v["supported"] == json!(true);
            Ok(hdr::HdrSeen {
                read: true,
                supported,
                before: v["before"] == json!(true),
                active: v["active"] == json!(true),
                gdi_name: Some(System::Mac.name(ask.created.target)),
                method: Some("transfer_function".into()),
                waited_ms: Some(started.elapsed().as_millis() as u64),
                error: (!supported).then(|| "écran virtuel inconnu de l'agent".to_string()),
                ..Default::default()
            })
        })
        .collect()
}
