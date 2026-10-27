//! KyberNow Link pour Windows. Sans argument : l'app (accueil, ajout, réglages,
//! session). Avec une adresse : une session directe en ligne de commande
//! (`kybernow --help`). Voir apps/windows/README.md.

#![cfg_attr(windows, windows_subsystem = "windows")]

#[cfg(not(windows))]
fn main() {
    eprintln!("KyberNow Link pour Windows ne se compile que pour Windows (apps/windows/scripts/build.sh).");
}

#[cfg(windows)]
mod audio;
#[cfg(windows)]
mod cli;
#[cfg(windows)]
mod decoder;
// Les écrans de ce PC : la relève est Windows, le reste (conversion pour le
// cœur, point sous un écran) se teste sur le Mac.
#[cfg_attr(not(windows), allow(dead_code))]
mod displays;
#[cfg(windows)]
mod discovery;
// Plusieurs ordinateurs à la fois : la session active et « Garder le son »,
// le point d'application unique (se teste sur le Mac).
#[cfg_attr(not(windows), allow(dead_code))]
mod focus;
// Règles et dessins sans rien de Windows (menu de l'icône, suivis, dessin de
// l'icône, textes) : compilés partout pour se tester sur le Mac.
#[cfg_attr(not(windows), allow(dead_code))]
mod follow;
#[cfg_attr(not(windows), allow(dead_code))]
mod glyph;
#[cfg(windows)]
mod fonds;
#[cfg(windows)]
mod gpu;
#[cfg(windows)]
mod host;
#[cfg(windows)]
mod i18n;
#[cfg(windows)]
mod instance;
#[cfg_attr(not(windows), allow(dead_code))]
mod link;
#[cfg(windows)]
mod login;
#[cfg_attr(not(windows), allow(dead_code))]
mod menu;
#[cfg_attr(not(windows), allow(dead_code))]
mod notice;
#[cfg(windows)]
mod pasteboard;
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
// KyberNow Share : Stations à essayer, prénom, panneau de l'hôte, QR code, avis
// de rôle (se teste sur le Mac).
#[cfg_attr(not(windows), allow(dead_code))]
mod sharing;
#[cfg_attr(not(windows), allow(dead_code))]
mod text;
#[cfg(windows)]
mod tray;
#[cfg(windows)]
mod ui;
#[cfg(windows)]
mod video;
#[cfg(windows)]
mod window;

#[cfg(windows)]
fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    // Langue de Windows, ou celle choisie dans les réglages de l'app.
    i18n::apply(&ui::load_prefs().language);
    // L'app : sans argument, ou ouverte par un lien, la barre des tâches, une
    // notification ou l'installateur. Le reste : la ligne de commande.
    // `--background` : ouverture de session ; `--quit` : l'installateur la ferme.
    let app = args
        .first()
        .is_none_or(|a| a.starts_with("kybernow://") || ["--connect", "--add", "--updated", "--background", "--quit"].contains(&a.as_str()));
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
    // L'installateur ferme l'app (`--quit`) : transmis à l'app ouverte, sans
    // attendre la fin de l'installation (elle attendrait celle-ci).
    if args.first().is_some_and(|a| a == "--quit") {
        if !instance::claim() {
            let _ = instance::forward(&args);
        }
        return;
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
