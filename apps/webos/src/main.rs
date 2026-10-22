//! KyberNow Link pour les téléviseurs LG (webOS), voir apps/webos/README.md.

// Sans le téléviseur (captures), une partie de l'état ne sert pas.
#![cfg_attr(not(feature = "device"), allow(dead_code))]

mod app;
#[cfg(feature = "device")]
mod device;
mod i18n;
mod model;
mod preview;
mod ui;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if let Some(i) = args.iter().position(|a| a == "--preview") {
        let dir = args.get(i + 1).cloned().unwrap_or_else(|| "preview".into());
        let lang = args.get(i + 2).cloned().unwrap_or_else(|| "fr".into());
        preview::run(std::path::Path::new(&dir), &lang);
        return;
    }
    #[cfg(feature = "device")]
    device::run();
}

/// Identifiant de l'app sur webOS (appinfo.json).
pub const APP_ID: &str = "fr.leslignesclaires.kybernow";
