//! Écrans d'une Station Linux (docs/station-linux.md) : bureaux GNOME et KDE Plasma.
//!
//! Admin lance l'agent `kybernow-display` (`server/linux/display-agent`,
//! Rust) dès son démarrage et lui parle par lignes JSON, avec le même
//! protocole que l'agent du Mac (`display/mac.rs`). L'agent passe par le
//! compositeur du bureau (Mutter sous GNOME, KWin sous KDE) : écrans
//! virtuels, capture des écrans de l'ordinateur sans le dialogue du partage
//! d'écran, et disposition temporaire du bureau. Il tient aussi le fichier
//! des écrans que Kyber lit (correctif Kyber Linux 0001).
//!
//! Différence avec le Mac : l'agent reste en marche tant qu'Admin tourne,
//! même sans session, puisque c'est lui qui tient la capture des écrans de
//! l'ordinateur pour Kyber. Filet de sécurité : les écrans virtuels vivent
//! avec sa connexion au bus de la session ; s'il s'arrête, GNOME les retire
//! et revient à sa disposition enregistrée.
//!
//! Le protocole, les écrans et la veille sont communs avec la Station Mac
//! (`agent_backend.rs`). Nom d'écran : `linux:<identifiant Kyber>` (101 pour
//! le premier écran virtuel, 1, 2… pour ceux de l'ordinateur). Pas encore de
//! HDR (docs/station-linux.md).

use std::path::PathBuf;

use serde_json::json;

use super::agent_backend::{AgentBackend, System};

pub use super::agent_backend::agent_path;

/// Les écrans de la Station Linux. Kyber a besoin de ses écrans dès son
/// démarrage : l'agent est lancé tout de suite, et dit quel bureau il a trouvé.
pub fn backend(path: PathBuf) -> AgentBackend {
    let backend = AgentBackend::new(System::Linux, path, "Linux", None);
    match backend.call(json!({ "cmd": "ping" }), true) {
        Ok(Some(v)) => {
            let desktop = v["desktop"].as_str().unwrap_or_default().to_string();
            tracing::info!("agent des écrans prêt, bureau : {desktop}");
            *backend.description.lock().unwrap() = description(&desktop).to_string();
            *backend.desktop.lock().unwrap() = desktop;
        }
        Ok(None) => {}
        Err(e) => tracing::warn!("agent des écrans (kybernow-display) : {e:#}"),
    }
    backend
}

/// Ce que `driver` dit du bureau.
fn description(desktop: &str) -> &'static str {
    match desktop {
        "GNOME" => "Mutter ScreenCast (GNOME)",
        "KDE" => "KWin zkde_screencast (KDE Plasma)",
        _ => "Linux",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bureau_dans_driver() {
        assert_eq!(description("KDE"), "KWin zkde_screencast (KDE Plasma)");
        assert_eq!(description("Sway"), "Linux");
    }
}
