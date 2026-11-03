//! Icône de KyberNow Station dans la zone de notification de Windows ; sur une
//! Station Mac, seulement l'agent de console d'un prêt (`kybernow-console
//! --console`, l'icône du Mac étant `server/macos/menubar`).
//!
//! Tourne dans la session de l'utilisateur, sans droits administrateur : accès à
//! KyberNow Admin, état de la Station et de la session, démarrage et arrêt des services.
//! Avec `--console`, le mode console d'un prêt de l'ordinateur (`lend::agent`).

#![cfg_attr(windows, windows_subsystem = "windows")]

#[cfg(windows)]
mod app;
// Sur Mac, l'agent de console ne prend que ce qui sert au prêt.
#[cfg(any(windows, target_os = "macos"))]
#[cfg_attr(target_os = "macos", allow(dead_code))]
mod console;
mod fonts;
#[cfg(any(windows, target_os = "macos"))]
#[cfg_attr(target_os = "macos", allow(dead_code))]
mod http;
#[cfg_attr(not(windows), allow(dead_code))]
mod i18n;
#[cfg(windows)]
mod icons;
#[cfg_attr(not(windows), allow(dead_code))]
mod lend;
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

#[cfg(windows)]
fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().map(String::as_str) == Some(startup::ELEVATED_ARG) {
        std::process::exit(startup::elevated_main(args.get(1).map(String::as_str)));
    }
    if args.first().map(String::as_str) == Some(password::ELEVATED_ARG) {
        std::process::exit(password::elevated_main());
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

#[cfg(not(any(windows, target_os = "macos")))]
fn main() {
    eprintln!("kybernow-tray ne fonctionne que sous Windows.");
    std::process::exit(1);
}
