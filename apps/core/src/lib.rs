//! Cœur des clients KyberNow écrits en Rust (Windows d'abord) : dialogue avec
//! la console, session Kyber, profils et débit, mesure de la latence. Rien
//! ici ne dépend d'un système : décodage, rendu, son et entrées sont fournis
//! par l'app à travers [`session::Platform`].

pub mod audio;
pub mod console;
pub mod i18n;
pub mod input;
pub mod latency;
pub mod media;
pub mod profile;
pub mod session;
pub mod update;

pub use kyclient::Display;
pub use kynput_rs;
pub use kyvlcplayer::WindowHandle;
