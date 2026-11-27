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
pub mod bundle;
pub mod controller;
pub mod discovery;
pub mod dualsense;
pub mod endpoint;
pub mod fec;
pub mod frame;
pub mod gkcrypt;
pub mod handoff;
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

use crate::url::hex_pair;

/// Texte hexadécimal (espaces autour ignorés), lu octet par octet : un texte
/// non ASCII est refusé au lieu de faire paniquer une découpe de `&str` au
/// milieu d'un caractère (COEUR-07).
pub(crate) fn hex_decode(text: &str) -> Option<Vec<u8>> {
    let bytes = text.trim().as_bytes();
    if bytes.len() % 2 != 0 {
        return None;
    }
    bytes.chunks_exact(2).map(|p| hex_pair(p[0], p[1])).collect()
}

#[cfg(test)]
mod hex_tests {
    use super::*;

    #[test]
    fn hexadecimal_sur_des_octets() {
        assert_eq!(hex_decode(" 0aFf "), Some(vec![0x0a, 0xff]));
        assert_eq!(hex_decode("abc"), None);
        // Non ASCII : ni panique, ni valeur.
        assert_eq!(hex_decode("é0"), None);
        assert_eq!(hex_decode("0\u{fffd}0"), None);
        assert_eq!(hex_decode("ab\u{fffd}c"), None);
        // `from_str_radix` acceptait le signe.
        assert_eq!(hex_decode("+f"), None);
    }
}
