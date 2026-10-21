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
mod snapshot;
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
        // Sonde et options de l'installateur : leur propre journal, celui de
        // l'app (dernière session) reste intact.
        let has = |f: &str| args.iter().any(|a| a == f);
        let name = if has("--probe-json") {
            "kybernow-probe"
        } else if has("--setup-shell") || has("--clear-shell") || has("--forget-all") {
            "kybernow-setup"
        } else {
            "kybernow"
        };
        system::init_logging(name);
        system::dpi_aware();
        // Lancée depuis un terminal : on écrit dans le sien.
        system::attach_console();
        std::process::exit(cli::main());
    }
    // Déjà ouverte : elle reçoit la demande, celle-ci s'arrête (sans toucher
    // à son journal), même si la transmission échoue (jamais deux apps).
    // Mise à jour en cours (app rouverte pendant l'installation) : on attend
    // qu'elle finisse plutôt que de verrouiller les fichiers qu'elle remplace.
    instance::wait_for_setup();
    if !instance::claim() {
        let _ = instance::forward(&args);
        return;
    }
    instance::listen();
    system::init_logging("kybernow");
    system::dpi_aware();
    ui::run(args);
}
