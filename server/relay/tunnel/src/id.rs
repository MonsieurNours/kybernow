//! Qui est qui (docs/relais.md, « Qui est qui ») : identifiant de la Station
//! d'après sa clé publique, identifiant d'un client d'après son jeton,
//! empreintes de certificat.

use ring::digest::{SHA256, digest};
use ring::rand::{SecureRandom, SystemRandom};

/// Longueur d'un identifiant de Station (16 octets en base32).
pub const STATION_ID_LEN: usize = 26;
/// Longueur d'un identifiant de client (10 octets en base32).
pub const CLIENT_ID_LEN: usize = 16;

const ALPHABET: &[u8; 32] = b"abcdefghijklmnopqrstuvwxyz234567";

/// Base32 minuscule sans remplissage (alphabet de la RFC 4648).
pub fn base32_lower(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len().div_ceil(5) * 8);
    let (mut acc, mut bits) = (0u32, 0u32);
    for &b in bytes {
        acc = (acc << 8) | u32::from(b);
        bits += 8;
        while bits >= 5 {
            bits -= 5;
            out.push(ALPHABET[((acc >> bits) & 31) as usize] as char);
        }
    }
    if bits > 0 {
        out.push(ALPHABET[((acc << (5 - bits)) & 31) as usize] as char);
    }
    out
}

/// Hexadécimal minuscule.
pub fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Identifiant de la Station : les 16 premiers octets du SHA-256 de sa clé
/// publique (SPKI en DER), en base32 minuscule : 26 caractères, une étiquette DNS.
pub fn station_id(spki_der: &[u8]) -> String {
    base32_lower(&digest(&SHA256, spki_der).as_ref()[..16])
}

/// Identifiant d'un client : les 10 premiers octets du SHA-256 de
/// `kn-client/1` suivi du jeton, en base32 minuscule (16 caractères).
pub fn client_id(token: &str) -> String {
    let mut input = b"kn-client/1".to_vec();
    input.extend_from_slice(token.as_bytes());
    base32_lower(&digest(&SHA256, &input).as_ref()[..10])
}

/// Un jeton de tunnel : 32 octets aléatoires en hexadécimal.
pub fn new_token() -> String {
    hex(&random_bytes::<32>())
}

/// Un identifiant tiré au hasard (connexion d'un navigateur) : 16 caractères.
pub fn random_id() -> String {
    base32_lower(&random_bytes::<10>())
}

pub(crate) fn random_bytes<const N: usize>() -> [u8; N] {
    let mut buf = [0u8; N];
    SystemRandom::new().fill(&mut buf).expect("aléa du système");
    buf
}

/// Empreinte d'un certificat, même forme que celle d'Admin : `sha256:aa:bb:…`.
pub fn fingerprint(cert_der: &[u8]) -> String {
    let d = digest(&SHA256, cert_der);
    let hex: Vec<String> = d.as_ref().iter().map(|b| format!("{b:02x}")).collect();
    format!("sha256:{}", hex.join(":"))
}

/// Une empreinte écrite à la main (majuscules, sans deux-points) remise en forme.
pub fn normalize_fingerprint(fp: &str) -> Option<String> {
    let rest = fp.trim().strip_prefix("sha256:").or_else(|| fp.trim().strip_prefix("SHA256:"))?;
    let digits: String = rest.chars().filter(|c| *c != ':').collect::<String>().to_ascii_lowercase();
    if digits.len() != 64 || !digits.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    let pairs: Vec<&str> = (0..32).map(|i| &digits[i * 2..i * 2 + 2]).collect();
    Some(format!("sha256:{}", pairs.join(":")))
}

/// Un identifiant de Station bien formé.
pub fn is_station_id(s: &str) -> bool {
    s.len() == STATION_ID_LEN && s.bytes().all(|b| ALPHABET.contains(&b))
}

/// Un identifiant de client bien formé.
pub fn is_client_id(s: &str) -> bool {
    s.len() == CLIENT_ID_LEN && s.bytes().all(|b| ALPHABET.contains(&b))
}

/// Un jeton de tunnel bien formé (16 à 128 caractères hexadécimaux).
pub fn is_token(s: &str) -> bool {
    (16..=128).contains(&s.len()) && s.bytes().all(|b| b.is_ascii_hexdigit())
}

/// La clé publique (SPKI en DER) d'un certificat.
pub fn spki_of_cert(cert_der: &[u8]) -> Option<Vec<u8>> {
    let der = rustls::pki_types::CertificateDer::from(cert_der);
    let cert = webpki::EndEntityCert::try_from(&der).ok()?;
    Some(cert.subject_public_key_info().as_ref().to_vec())
}

/// L'identifiant de Station d'un certificat client.
pub fn station_id_of_cert(cert_der: &[u8]) -> Option<String> {
    spki_of_cert(cert_der).map(|spki| station_id(&spki))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base32_de_la_rfc() {
        // RFC 4648, § 10, en minuscules et sans remplissage.
        for (input, out) in [
            ("", ""),
            ("f", "my"),
            ("fo", "mzxq"),
            ("foo", "mzxw6"),
            ("foob", "mzxw6yq"),
            ("fooba", "mzxw6ytb"),
            ("foobar", "mzxw6ytboi"),
        ] {
            assert_eq!(base32_lower(input.as_bytes()), out);
        }
    }

    #[test]
    fn longueurs_des_identifiants() {
        let s = station_id(b"cle");
        assert_eq!(s.len(), STATION_ID_LEN);
        assert!(is_station_id(&s));
        let token = new_token();
        assert!(is_token(&token));
        let c = client_id(&token);
        assert_eq!(c.len(), CLIENT_ID_LEN);
        assert!(is_client_id(&c));
        assert_eq!(c, client_id(&token), "même jeton, même identifiant");
        assert_ne!(c, client_id(&new_token()));
        assert!(is_client_id(&random_id()));
        assert!(!is_station_id("ABCDEFGHIJKLMNOPQRSTUVWXYZ"));
    }

    #[test]
    fn empreintes() {
        let fp = fingerprint(b"certificat");
        assert!(fp.starts_with("sha256:"));
        assert_eq!(fp.len(), 7 + 32 * 3 - 1);
        assert_eq!(normalize_fingerprint(&fp.to_uppercase().replace("SHA256:", "sha256:")), Some(fp.clone()));
        assert_eq!(normalize_fingerprint(&fp.replace(':', "").replacen("sha256", "sha256:", 1)), Some(fp));
        assert_eq!(normalize_fingerprint("sha256:12"), None);
    }

    #[test]
    fn identifiant_d_un_certificat() {
        let key = rcgen::KeyPair::generate_for(&rcgen::PKCS_ED25519).unwrap();
        let cert = rcgen::CertificateParams::new(vec!["kybernow-station".into()]).unwrap().self_signed(&key).unwrap();
        let id = station_id_of_cert(cert.der()).unwrap();
        assert_eq!(id, station_id(&key.public_key_der()));
    }
}
