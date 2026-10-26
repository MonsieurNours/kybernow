//! KyberNow Link pour Linux, le Steam Deck, SteamOS et le Raspberry Pi, voir apps/linux/README.md.

// Sans l'app (captures), une partie de l'état ne sert pas.
#![cfg_attr(not(feature = "device"), allow(dead_code))]

mod app;
mod app_time;
#[cfg(feature = "device")]
mod device;
mod displays;
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
    if let Some(i) = args.iter().position(|a| a == "--splash") {
        // L'image fixe du démarrage de KyberNow Link OS (Plymouth) : la première image de l'allumage.
        let out = args.get(i + 1).cloned().unwrap_or_else(|| "splash.png".into());
        let w: u32 = args.get(i + 2).and_then(|v| v.parse().ok()).unwrap_or(1920);
        let h: u32 = args.get(i + 3).and_then(|v| v.parse().ok()).unwrap_or(1080);
        preview::splash(std::path::Path::new(&out), w, h);
        return;
    }
    if args.iter().any(|a| a == "--version" || a == "-V") {
        println!("KyberNow Link {}", env!("CARGO_PKG_VERSION"));
        return;
    }
    #[cfg(feature = "device")]
    device::run(&args);
}

/// Identifiant de l'app (Flatpak, fichier .desktop, AppStream).
pub const APP_ID: &str = "fr.leslignesclaires.kybernow";
