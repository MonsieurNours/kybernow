//! Consoles PlayStation (PS5 et PS4) : le protocole de lecture à distance
//! de Sony, écrit en Rust d'après chiaki-ng (AGPL-3.0, comme KyberNow).
//! Contrat : `docs/playstation.md`.
//!
//! Écrit pour l'instant : [`discovery`] (trouver les consoles du réseau et
//! les réveiller), [`psn`] (identifiant du compte, lu une fois),
//! [`regist`] (association par le code) et [`rpcrypt`] (son chiffrement).
//! Viennent ensuite la session, l'image et le son, puis la manette.

pub mod discovery;
mod keys;
pub mod psn;
pub mod regist;
pub mod rpcrypt;
#[cfg(test)]
mod test_vectors;
