//! Icône de KyberNow Station dans la zone de notification de Windows, et près
//! de l'horloge sous Linux (`linux`, AppIndicator) ; sur une Station Mac,
//! seulement l'agent de console d'un prêt (`kybernow-console --console`,
//! l'icône du Mac étant `server/macos/menubar`).
//!
//! Tourne dans la session de l'utilisateur, sans droits administrateur : accès à
//! KyberNow Admin, état de la Station et de la session, démarrage et arrêt des services.
//! Avec `--console`, le mode console d'un prêt de l'ordinateur (`lend::agent`).

#![cfg_attr(windows, windows_subsystem = "windows")]

#[cfg(windows)]
mod app;
// Sur Mac, l'agent de console ne prend que ce qui sert au prêt.
#[cfg(any(windows, target_os = "macos", target_os = "linux"))]
#[cfg_attr(any(target_os = "macos", target_os = "linux"), allow(dead_code))]
mod console;
mod fonts;
#[cfg(any(windows, target_os = "macos", target_os = "linux"))]
#[cfg_attr(target_os = "macos", allow(dead_code))]
mod http;
#[cfg_attr(not(any(windows, target_os = "linux")), allow(dead_code))]
mod i18n;
#[cfg(any(windows, target_os = "linux"))]
mod icons;
#[cfg_attr(not(windows), allow(dead_code))]
mod lend;
#[cfg(target_os = "linux")]
mod linux;
#[cfg(windows)]
mod panel;
#[cfg(windows)]
mod password;
#[cfg_attr(not(windows), allow(dead_code))]
mod qr;
#[cfg(windows)]
mod services;
#[cfg_attr(not(windows), allow(dead_code))]
mod share;
#[cfg(windows)]
mod startup;
#[cfg_attr(not(any(windows, target_os = "linux")), allow(dead_code))]
mod status;

#[cfg(windows)]
fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().map(String::as_str) == Some(startup::ELEVATED_ARG) {
        std::process::exit(startup::elevated_main(args.get(1).map(String::as_str)));
    }
    if args.first().map(String::as_str) == Some(password::ELEVATED_ARG) {
        std::process::exit(password::elevated_main(args.get(1).map(String::as_str)));
    }
    // Mode console d'un prêt (docs/pret.md) : lancé par Admin dans la session
    // de l'utilisateur ; ni icône, ni verrou de l'icône.
    if args.first().map(String::as_str) == Some(lend::agent::ARG) {
        std::process::exit(lend::agent::run());
    }
    if let Err(e) = app::run() {
        app::alert(&i18n::t("tray.startFailed", &[("error", &format!("{e:#}"))]));
        std::process::exit(1);
    }
}

/// Station Mac : l'agent de console d'un prêt, lancé par Admin (docs/pret.md).
#[cfg(target_os = "macos")]
fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().map(String::as_str) == Some(lend::agent::ARG) {
        std::process::exit(lend::agent::run());
    }
    eprintln!("kybernow-console : seulement --console, lancé par KyberNow Admin pendant un prêt.");
    std::process::exit(2);
}

/// Station Linux : l'icône près de l'horloge (service `kybernow-tray.service`).
#[cfg(target_os = "linux")]
fn main() {
    // L'agent de console d'un prêt, lancé par Admin (docs/pret.md, « Station Linux »).
    if std::env::args().nth(1).as_deref() == Some(lend::agent::ARG) {
        std::process::exit(lend::agent::run());
    }
    if let Err(e) = linux::run_app() {
        linux::alert(&i18n::t("tray.startFailed", &[("error", &format!("{e:#}"))]));
        std::process::exit(1);
    }
}

#[cfg(not(any(windows, target_os = "macos", target_os = "linux")))]
fn main() {
    eprintln!("kybernow-tray ne fonctionne que sous Windows et Linux.");
    std::process::exit(1);
}
