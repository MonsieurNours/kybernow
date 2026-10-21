//! KyberNow pour Windows. Sans argument : l'app (accueil, ajout, réglages,
//! session). Avec une adresse : une session directe en ligne de commande
//! (`kybernow --help`). Voir apps/windows/README.md.

#![cfg_attr(windows, windows_subsystem = "windows")]

#[cfg(not(windows))]
fn main() {
    eprintln!("KyberNow pour Windows ne se compile que pour Windows (apps/windows/scripts/build.sh).");
}

#[cfg(windows)]
mod audio;
#[cfg(windows)]
mod cli;
#[cfg(windows)]
mod decoder;
#[cfg(windows)]
mod discovery;
#[cfg(windows)]
mod host;
#[cfg(windows)]
mod input;
#[cfg(windows)]
mod instance;
#[cfg(windows)]
mod pc;
#[cfg(windows)]
mod plan;
#[cfg(windows)]
mod render;
#[cfg(windows)]
mod shell;
#[cfg(windows)]
mod store;
#[cfg(windows)]
mod system;
#[cfg(windows)]
mod ui;
#[cfg(windows)]
mod video;
#[cfg(windows)]
mod window;

#[cfg(windows)]
fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    // L'app : sans argument, ou ouverte par un lien, la barre des tâches, une
    // notification ou l'installateur. Le reste : la ligne de commande.
    let app = args.first().is_none_or(|a| a.starts_with("kybernow://") || ["--connect", "--add", "--updated"].contains(&a.as_str()));
    if !app {
        let probe = args.iter().any(|a| a == "--probe-json");
        system::init_logging(if probe { "kybernow-probe" } else { "kybernow" });
        system::dpi_aware();
        // Lancée depuis un terminal : on écrit dans le sien.
        system::attach_console();
        std::process::exit(cli::main());
    }
    // Déjà ouverte : elle reçoit la demande, celle-ci s'arrête (sans toucher
    // à son journal), même si la transmission échoue (jamais deux apps).
    if !instance::claim() {
        let _ = instance::forward(&args);
        return;
    }
    instance::listen();
    system::init_logging("kybernow");
    system::dpi_aware();
    ui::run(args);
}
