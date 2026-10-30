//! Cœur des apps KyberNow Link écrit en Rust : dialogue avec KyberNow Admin,
//! session Kyber, profils et débit, mesure de la latence, module d'entrées,
//! moteur de son, mises à jour, réveil par le réseau, plusieurs ordinateurs
//! à la fois ([`focus`], [`gamepads`]). Rien ici ne dépend d'un système : la
//! fenêtre, le décodage, le rendu, la sortie du son et la capture des
//! entrées sont fournis par l'app à travers [`session::Platform`] et les
//! traits des modules [`input`] et [`audio`] ; seul le lecteur de manettes
//! peut ouvrir SDL lui-même (feature `sdl`, Mac et Windows).

pub mod audio;
pub mod console;
pub mod focus;
pub mod gamepads;
pub mod i18n;
pub mod input;
pub mod journal;
pub mod latency;
pub mod lend;
pub mod media;
pub mod profile;
pub mod relay;
pub mod screens;
pub mod share;
pub mod session;
pub mod update;
pub mod wake;

pub use kyclient::Display;
pub use kynput_rs;
pub use kyvlcplayer::WindowHandle;
