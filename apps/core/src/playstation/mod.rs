//! Consoles PlayStation (PS5 et PS4) : le protocole de lecture à distance
//! de Sony, écrit en Rust d'après chiaki-ng (AGPL-3.0, comme KyberNow).
//! Contrat : `docs/playstation.md`.
//!
//! Écrit pour l'instant : [`discovery`] (trouver les consoles du réseau et
//! les réveiller). Viennent ensuite l'association par code, la session,
//! l'image et le son, puis la manette.

pub mod discovery;
