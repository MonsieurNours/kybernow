//! Installateur de l'app KyberNow pour Windows : par utilisateur, sans droits
//! d'administrateur, une seule fois pour x64 et ARM64.

pub mod events;
pub mod payload;

#[cfg(windows)]
pub mod checks;
#[cfg(windows)]
pub mod install;
#[cfg(windows)]
pub mod sys;
#[cfg(windows)]
pub mod uninstall;
#[cfg(windows)]
pub mod webview2;
