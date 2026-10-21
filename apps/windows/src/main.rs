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
mod plan;
#[cfg(windows)]
mod render;
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
    system::init_logging();
    system::dpi_aware();
    if std::env::args().len() > 1 {
        // Lancée depuis un terminal : on écrit dans le sien.
        system::attach_console();
        std::process::exit(cli::main());
    }
    ui::run();
}
