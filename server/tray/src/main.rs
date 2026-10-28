//! Icône de KyberNow Station dans la zone de notification de Windows.
//!
//! Tourne dans la session de l'utilisateur, sans droits administrateur : accès à
//! KyberNow Admin, état de la Station et de la session, démarrage et arrêt des services.

#![cfg_attr(windows, windows_subsystem = "windows")]

#[cfg(windows)]
mod app;
#[cfg(windows)]
mod console;
#[cfg(windows)]
mod http;
#[cfg_attr(not(windows), allow(dead_code))]
mod i18n;
#[cfg(windows)]
mod icons;
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
    if let Err(e) = app::run() {
        app::alert(&i18n::t("tray.startFailed", &[("error", &format!("{e:#}"))]));
        std::process::exit(1);
    }
}

#[cfg(not(windows))]
fn main() {
    eprintln!("kybernow-tray ne fonctionne que sous Windows.");
    std::process::exit(1);
}
