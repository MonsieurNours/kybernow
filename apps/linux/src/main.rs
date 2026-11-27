//! KyberNow Link pour Linux, le Steam Deck, SteamOS et le Raspberry Pi, voir apps/linux/README.md.

// Sans l'app (captures), une partie de l'état ne sert pas.
#![cfg_attr(not(feature = "device"), allow(dead_code))]

mod accounts;
mod app;
mod app_time;
mod autotest;
mod country;
#[cfg(feature = "device")]
mod device;
mod displays;
mod hdr;
mod i18n;
mod journal;
mod micro;
mod model;
// Les captures (`--preview`) et leurs essais : pas dans l'app livrée (LINAPP-39).
#[cfg(any(test, not(feature = "device")))]
// Avec l'app (`device`), seuls ses essais servent : les captures ne se lancent pas.
#[cfg_attr(all(test, feature = "device"), allow(dead_code))]
mod preview;
mod ps;
mod pyrowave;
mod splash;
mod station_offer;
mod synced;
mod ui;
mod video_rules;
mod welcome;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if let Some(i) = args.iter().position(|a| a == "--preview") {
        #[cfg(not(feature = "device"))]
        {
            let dir = args.get(i + 1).cloned().unwrap_or_else(|| "preview".into());
            let lang = args.get(i + 2).cloned().unwrap_or_else(|| "fr".into());
            preview::run(std::path::Path::new(&dir), &lang);
            return;
        }
        #[cfg(feature = "device")]
        {
            let _ = i;
            eprintln!("--preview : les captures se font avec l'interface seule (cargo run --no-default-features -- --preview)");
            std::process::exit(2);
        }
    }
    if let Some(i) = args.iter().position(|a| a == "--splash") {
        // L'image fixe du démarrage de KyberNow Link OS (Plymouth) : la première image de l'allumage.
        let out = args.get(i + 1).cloned().unwrap_or_else(|| "splash.png".into());
        let w: u32 = args.get(i + 2).and_then(|v| v.parse().ok()).unwrap_or(1920);
        let h: u32 = args.get(i + 3).and_then(|v| v.parse().ok()).unwrap_or(1080);
        splash::splash(std::path::Path::new(&out), w, h);
        return;
    }
    if args.iter().any(|a| a == "--version" || a == "-V") {
        println!("KyberNow Link {}", env!("CARGO_PKG_VERSION"));
        return;
    }
    #[cfg(feature = "device")]
    if args.iter().any(|a| a == "--pyrowave-probe") {
        // La sonde de PyroWave seule, dans ce processus (docs/pyrowave-client-pi.md).
        std::process::exit(device::pyrowave_probe_child());
    }
    #[cfg(feature = "device")]
    if args.iter().any(|a| a == "--hdr-test") {
        device::hdr_test();
        return;
    }
    #[cfg(feature = "device")]
    device::run(&args);
}

/// Identifiant de l'app (Flatpak, fichier .desktop, AppStream).
pub const APP_ID: &str = "fr.leslignesclaires.kybernow";
