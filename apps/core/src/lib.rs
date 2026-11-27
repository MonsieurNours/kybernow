//! Cœur des apps KyberNow Link écrit en Rust : dialogue avec KyberNow Admin,
//! session Kyber, profils et débit, mesure de la latence, module d'entrées,
//! moteur de son, mises à jour, réveil par le réseau, plusieurs ordinateurs
//! à la fois ([`focus`], [`gamepads`]). Rien ici ne dépend d'un système : la
//! fenêtre, le décodage, le rendu, la sortie du son et la capture des
//! entrées sont fournis par l'app à travers [`session::Platform`] et les
//! traits des modules [`input`] et [`audio`] ; seul le lecteur de manettes
//! peut ouvrir SDL lui-même (feature `sdl`, Mac et Windows).

pub mod account;
pub mod adaptive;
pub mod autotest;
pub mod audio;
pub mod camera;
pub mod console;
pub mod discovery;
pub mod ffi;
pub mod focus;
pub mod frames;
pub mod gamepads;
pub mod i18n;
pub mod input;
pub mod invite;
pub mod journal;
pub mod latency;
pub mod lend;
pub mod media;
pub mod micro;
pub mod plan;
pub mod playstation;
pub mod profile;
pub mod pyrowave;
pub mod recording;
pub mod relay;
pub mod screens;
pub mod share;
pub mod station_offer;
pub mod session;
pub mod update;
mod url;
pub mod wake;
pub mod welcome;

/// Règles pures communes aux apps (adresses, empreintes, codes, relais,
/// bornes et valeurs partagées, courbes HDR) : la caisse `kybernow-rules`,
/// reprise telle quelle pour les ponts.
pub use kybernow_rules as rules;
/// Le découpage d'un flux H.264 ou HEVC en Annex B (`rules::nal`, WEBOS-27) :
/// unités NAL, leur type, octets d'émulation.
pub use kybernow_rules::nal;

pub use kyclient::Display;
pub use kynput_rs;
pub use kyvlcplayer::WindowHandle;
