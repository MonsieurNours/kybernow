//! Consoles PlayStation (PS5 et PS4) : le protocole de lecture à distance
//! de Sony, écrit en Rust d'après chiaki-ng (AGPL-3.0, comme KyberNow).
//! Contrat : `docs/playstation.md`.
//!
//! Écrit pour l'instant : [`discovery`] (trouver les consoles du réseau et
//! les réveiller), [`psn`] (identifiant du compte, lu une fois),
//! [`regist`] (association par le code), [`rpcrypt`] (son chiffrement) et
//! [`session`] (demande de session et canal de contrôle), [`takion`] (le
//! transport), [`gkcrypt`] (son chiffrement), [`stream`] (l'ouverture du
//! flux), [`frame`] et [`fec`] (les images), [`audio`] (le son) et [`play`]
//! (une partie entière, vers les gestionnaires de paquets de l'app),
//! [`endpoint`] (où joindre la console : son adresse et ses ports) et
//! [`remote`] (hors de chez soi, par une Station de son réseau et le relais).

pub mod audio;
pub mod controller;
pub mod discovery;
pub mod endpoint;
pub mod fec;
pub mod frame;
pub mod gkcrypt;
mod keys;
pub mod keymouse;
pub mod play;
pub mod proto;
pub mod psn;
pub mod refs;
pub mod regist;
pub mod remote;
pub mod rpcrypt;
pub mod session;
pub mod settings;
pub mod stream;
pub mod takion;
#[cfg(test)]
mod test_vectors;
