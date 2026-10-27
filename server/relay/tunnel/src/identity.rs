//! L'identité de la Station chez le relais (docs/relais.md, « Qui est qui ») :
//! une clé Ed25519 créée au premier usage (`<données>/relay/station.key`, en
//! PKCS#8 PEM), un certificat signé par elle-même, présenté comme certificat
//! client, et l'identifiant qui en découle.

use std::fs;
use std::io;
use std::path::Path;

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use ring::rand::SystemRandom;
use ring::signature::Ed25519KeyPair;
use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer};

use crate::id::station_id;
use crate::lookup::sig_message;
use crate::tls::ClientAuth;

/// L'identité d'une Station.
pub struct StationIdentity {
    pkcs8: Vec<u8>,
    pair: Ed25519KeyPair,
    cert: CertificateDer<'static>,
    spki: Vec<u8>,
    id: String,
}

impl std::fmt::Debug for StationIdentity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StationIdentity").field("id", &self.id).finish_non_exhaustive()
    }
}

fn invalid(e: impl std::fmt::Display) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, e.to_string())
}

impl StationIdentity {
    /// Une nouvelle clé.
    pub fn generate() -> io::Result<Self> {
        let doc = Ed25519KeyPair::generate_pkcs8(&SystemRandom::new()).map_err(invalid)?;
        Self::from_pkcs8(doc.as_ref())
    }

    /// D'après une clé Ed25519 en PKCS#8 (DER).
    pub fn from_pkcs8(der: &[u8]) -> io::Result<Self> {
        let pair = Ed25519KeyPair::from_pkcs8_maybe_unchecked(der).map_err(invalid)?;
        let key = rcgen::KeyPair::try_from(der).map_err(invalid)?;
        let mut params = rcgen::CertificateParams::new(vec!["kybernow-station".to_string()]).map_err(invalid)?;
        params.distinguished_name = rcgen::DistinguishedName::new();
        params.distinguished_name.push(rcgen::DnType::CommonName, "KyberNow Station");
        // Le relais ne regarde pas les dates : un certificat très long évite
        // qu'une horloge fausse le fasse refuser par rustls.
        params.not_before = rcgen::date_time_ymd(2024, 1, 1);
        params.not_after = rcgen::date_time_ymd(4000, 1, 1);
        let cert = params.self_signed(&key).map_err(invalid)?;
        let spki = key.public_key_der();
        Ok(StationIdentity { pkcs8: der.to_vec(), pair, cert: cert.der().clone(), id: station_id(&spki), spki })
    }

    /// Lit la clé, ou la crée (droits réservés au propriétaire sur Unix ; sur
    /// Windows, Admin protège déjà son dossier de données).
    pub fn load_or_create(path: &Path) -> io::Result<Self> {
        match fs::read(path) {
            Ok(bytes) => Self::from_pkcs8(&pem_or_der(&bytes)?),
            Err(e) if e.kind() == io::ErrorKind::NotFound => {
                let id = Self::generate()?;
                if let Some(dir) = path.parent() {
                    fs::create_dir_all(dir)?;
                }
                let tmp = path.with_extension("key.tmp");
                write_private(&tmp, id.pem().as_bytes())?;
                fs::rename(&tmp, path)?;
                Ok(id)
            }
            Err(e) => Err(e),
        }
    }

    /// L'identifiant de la Station (26 caractères).
    pub fn id(&self) -> &str {
        &self.id
    }

    /// La clé publique, SPKI en DER.
    pub fn spki_der(&self) -> &[u8] {
        &self.spki
    }

    /// La clé publique, SPKI en DER, en base64 (`station_key`).
    pub fn station_key(&self) -> String {
        STANDARD.encode(&self.spki)
    }

    /// La clé privée en PEM (PKCS#8).
    pub fn pem(&self) -> String {
        let b64 = STANDARD.encode(&self.pkcs8);
        let mut out = String::from("-----BEGIN PRIVATE KEY-----\n");
        for line in b64.as_bytes().chunks(64) {
            out.push_str(std::str::from_utf8(line).expect("base64 en ASCII"));
            out.push('\n');
        }
        out.push_str("-----END PRIVATE KEY-----\n");
        out
    }

    /// La signature de l'empreinte d'Admin (`sig` de `hello`), en base64.
    pub fn sign_admin_fp(&self, admin_fp: &str) -> String {
        STANDARD.encode(self.pair.sign(&sig_message(admin_fp)).as_ref())
    }

    /// Le certificat client à présenter au relais.
    pub fn client_auth(&self) -> ClientAuth {
        ClientAuth {
            certs: vec![self.cert.clone()],
            key: PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(self.pkcs8.clone())),
        }
    }
}

fn pem_or_der(bytes: &[u8]) -> io::Result<Vec<u8>> {
    let Ok(text) = std::str::from_utf8(bytes) else { return Ok(bytes.to_vec()) };
    let Some(start) = text.find("-----BEGIN PRIVATE KEY-----") else { return Ok(bytes.to_vec()) };
    let body = &text[start + "-----BEGIN PRIVATE KEY-----".len()..];
    let end = body.find("-----END PRIVATE KEY-----").ok_or_else(|| invalid("clé PEM incomplète"))?;
    let b64: String = body[..end].chars().filter(|c| !c.is_whitespace()).collect();
    STANDARD.decode(b64).map_err(invalid)
}

#[cfg(unix)]
fn write_private(path: &Path, bytes: &[u8]) -> io::Result<()> {
    use std::io::Write;
    use std::os::unix::fs::OpenOptionsExt;
    let mut f = fs::OpenOptions::new().write(true).create(true).truncate(true).mode(0o600).open(path)?;
    f.write_all(bytes)?;
    f.sync_all()
}

#[cfg(not(unix))]
fn write_private(path: &Path, bytes: &[u8]) -> io::Result<()> {
    fs::write(path, bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lookup::verify_admin_fp;

    #[test]
    fn cle_creee_puis_relue() {
        let dir = std::env::temp_dir().join(format!("kn-relay-id-{}", crate::id::random_id()));
        let path = dir.join("relay").join("station.key");
        let a = StationIdentity::load_or_create(&path).unwrap();
        let b = StationIdentity::load_or_create(&path).unwrap();
        assert_eq!(a.id(), b.id(), "même clé, même identifiant");
        assert!(crate::id::is_station_id(a.id()));
        assert_eq!(crate::id::station_id_of_cert(&a.client_auth().certs[0]).as_deref(), Some(a.id()));
        let sig = a.sign_admin_fp("sha256:aa");
        assert!(verify_admin_fp(a.id(), &a.station_key(), "sha256:aa", &sig));
        let _ = fs::remove_dir_all(dir);
    }
}
