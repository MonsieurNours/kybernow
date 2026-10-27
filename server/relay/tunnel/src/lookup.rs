//! Un code de partage chez le relais (`GET /v1/codes/{code}`) et la
//! signature de la Station sur l'empreinte d'Admin (docs/relais.md).

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use ring::signature::{ED25519, UnparsedPublicKey};
use serde::{Deserialize, Serialize};

use crate::id::station_id;

/// Début de la clé publique Ed25519 en SPKI (DER) : 12 octets, puis les 32 de la clé.
const ED25519_SPKI_PREFIX: [u8; 12] = [0x30, 0x2a, 0x30, 0x05, 0x06, 0x03, 0x2b, 0x65, 0x70, 0x03, 0x21, 0x00];

/// La réponse de `GET /v1/codes/{code}`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CodeLookup {
    /// L'identifiant de la Station qui partage.
    pub station: String,
    /// Sa clé publique, SPKI en DER, en base64.
    pub station_key: String,
    /// L'empreinte d'Admin, attestée par le relais (code tapé) et signée par la Station.
    pub admin_fp: String,
    /// La signature de la Station sur `admin_fp`, en base64.
    pub sig: String,
    /// La Station est-elle connectée au relais en ce moment.
    pub online: bool,
}

impl CodeLookup {
    /// La clé correspond à l'identifiant et la signature tient.
    pub fn verify(&self) -> bool {
        verify_admin_fp(&self.station, &self.station_key, &self.admin_fp, &self.sig)
    }
}

/// Le chemin de la recherche d'un code.
pub fn lookup_path(code: &str) -> String {
    format!("/v1/codes/{code}")
}

/// Un code de partage : six chiffres.
pub fn is_code(s: &str) -> bool {
    s.len() == 6 && s.bytes().all(|b| b.is_ascii_digit())
}

/// Ce que la Station signe : `kybernow-relay/1 admin_fp ` suivi de l'empreinte.
pub fn sig_message(admin_fp: &str) -> Vec<u8> {
    let mut m = b"kybernow-relay/1 admin_fp ".to_vec();
    m.extend_from_slice(admin_fp.as_bytes());
    m
}

/// La clé Ed25519 brute (32 octets) d'une SPKI.
pub fn ed25519_key(spki_der: &[u8]) -> Option<&[u8]> {
    (spki_der.len() == 44 && spki_der[..12] == ED25519_SPKI_PREFIX).then(|| &spki_der[12..])
}

/// Vérifie qu'une signature de `admin_fp` vient bien de la clé de cette SPKI.
pub fn verify_sig(spki_der: &[u8], admin_fp: &str, sig: &[u8]) -> bool {
    let Some(raw) = ed25519_key(spki_der) else { return false };
    UnparsedPublicKey::new(&ED25519, raw).verify(&sig_message(admin_fp), sig).is_ok()
}

/// Vérifie la clé (elle donne bien l'identifiant `station`) et la signature
/// de `admin_fp`, telles que le relais les rend.
pub fn verify_admin_fp(station: &str, station_key_b64: &str, admin_fp: &str, sig_b64: &str) -> bool {
    let (Ok(spki), Ok(sig)) = (STANDARD.decode(station_key_b64), STANDARD.decode(sig_b64)) else { return false };
    station_id(&spki) == station && verify_sig(&spki, admin_fp, &sig)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ring::rand::SystemRandom;
    use ring::signature::{Ed25519KeyPair, KeyPair};

    #[test]
    fn signature_de_l_empreinte() {
        let pkcs8 = Ed25519KeyPair::generate_pkcs8(&SystemRandom::new()).unwrap();
        let pair = Ed25519KeyPair::from_pkcs8(pkcs8.as_ref()).unwrap();
        let mut spki = ED25519_SPKI_PREFIX.to_vec();
        spki.extend_from_slice(pair.public_key().as_ref());
        let fp = "sha256:aa:bb";
        let sig = pair.sign(&sig_message(fp));
        let lookup = CodeLookup {
            station: station_id(&spki),
            station_key: STANDARD.encode(&spki),
            admin_fp: fp.into(),
            sig: STANDARD.encode(sig.as_ref()),
            online: true,
        };
        assert!(lookup.verify());
        assert!(!CodeLookup { admin_fp: "sha256:cc".into(), ..lookup.clone() }.verify());
        assert!(!CodeLookup { station: station_id(b"autre"), ..lookup.clone() }.verify());
        assert!(is_code("012345"));
        assert!(!is_code("12345a"));
        assert_eq!(lookup_path("123456"), "/v1/codes/123456");
    }
}
