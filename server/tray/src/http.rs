//! Client HTTPS minimal vers la console, qui n'accepte que son certificat.
//!
//! Tout compte interactif peut arrêter le service de la console (c'est
//! voulu : l'icône le fait) et servir autre chose sur son port. L'icône
//! vérifie donc le certificat : elle compare l'empreinte SHA-256 de
//! celui que le serveur présente à `console_fingerprint` de
//! `C:\Program Files\KyberNow\kybernow.json` (écrit par `kyber-admin init`
//! et par la console à chaque démarrage ; le dossier n'est modifiable que
//! par les administrateurs). Les appels de l'icône tiennent dans une requête
//! HTTP/1.1 par connexion (`Connection: close`, un petit corps JSON pour
//! celles du partage) : la réponse va jusqu'à la fermeture, sans découpage à décoder.

use std::io::{Read, Write};
use std::net::{Ipv4Addr, SocketAddr, TcpStream};
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result};
use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::crypto::{CryptoProvider, verify_tls12_signature, verify_tls13_signature};
use rustls::pki_types::{CertificateDer, ServerName, UnixTime};
use rustls::{DigitallySignedStruct, SignatureScheme};
use sha2::{Digest, Sha256};

use crate::i18n::{lang, t};

/// Réponse : code et corps.
pub struct Reply {
    pub status: u16,
    pub body: Vec<u8>,
}

pub struct Client {
    config: Arc<rustls::ClientConfig>,
}

impl Client {
    /// `fingerprint` : « sha256:aa:bb:… » (format de `kybernow.json` et de l'appairage).
    pub fn pinned(fingerprint: &str) -> Result<Self> {
        let fingerprint = parse_fingerprint(fingerprint).context("empreinte du certificat d'Admin illisible")?;
        let provider = Arc::new(rustls::crypto::ring::default_provider());
        let config = rustls::ClientConfig::builder_with_provider(provider.clone())
            .with_safe_default_protocol_versions()
            .context("versions TLS")?
            .dangerous()
            .with_custom_certificate_verifier(Arc::new(Pinned { fingerprint, provider }))
            .with_no_client_auth();
        Ok(Self { config: Arc::new(config) })
    }

    /// Une requête sans corps vers `https://127.0.0.1:{port}{path}`, avec la
    /// langue de l'icône (`Accept-Language`) : la console y répond dans cette langue.
    pub fn call(&self, method: &str, port: u16, path: &str, headers: &[(&str, &str)], timeout: Duration) -> Result<Reply> {
        self.send(method, port, path, headers, None, timeout)
    }

    /// [`Client::call`] avec un corps JSON (`body`), pour les actions du partage.
    pub fn send(&self, method: &str, port: u16, path: &str, headers: &[(&str, &str)], body: Option<&[u8]>, timeout: Duration) -> Result<Reply> {
        let addr = SocketAddr::from((Ipv4Addr::LOCALHOST, port));
        let tcp = TcpStream::connect_timeout(&addr, timeout).with_context(|| t("tray.errors.consoleSilent", &[]))?;
        tcp.set_read_timeout(Some(timeout))?;
        tcp.set_write_timeout(Some(timeout))?;
        tcp.set_nodelay(true)?;
        let name = ServerName::IpAddress(Ipv4Addr::LOCALHOST.into());
        let conn = rustls::ClientConnection::new(self.config.clone(), name).context("connexion TLS")?;
        let mut tls = rustls::StreamOwned::new(conn, tcp);

        let length = body.map_or(0, <[u8]>::len);
        let mut request = format!("{method} {path} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nConnection: close\r\nContent-Length: {length}\r\nAccept-Language: {}\r\n", lang().code());
        if body.is_some() {
            request.push_str("Content-Type: application/json\r\n");
        }
        for (name, value) in headers {
            request.push_str(&format!("{name}: {value}\r\n"));
        }
        request.push_str("\r\n");
        let mut bytes = request.into_bytes();
        bytes.extend_from_slice(body.unwrap_or_default());
        tls.write_all(&bytes).with_context(|| t("tray.errors.sendFailed", &[]))?;

        let mut raw = Vec::new();
        match tls.read_to_end(&mut raw) {
            Ok(_) => {}
            // Fin de connexion sans close_notify : fréquent, le corps est entier avec Connection: close.
            Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => {}
            Err(e) => return Err(e).context("lecture de la réponse d'Admin"),
        }
        parse_reply(&raw)
    }
}

/// Code de la ligne d'état et corps après la ligne vide.
pub fn parse_reply(raw: &[u8]) -> Result<Reply> {
    let end = raw.windows(4).position(|w| w == b"\r\n\r\n").context("réponse HTTP incomplète")?;
    let head = std::str::from_utf8(&raw[..end]).context("en-têtes illisibles")?;
    let status_line = head.lines().next().unwrap_or_default();
    let status: u16 = status_line.split_whitespace().nth(1).and_then(|s| s.parse().ok()).context("ligne d'état illisible")?;
    Ok(Reply { status, body: raw[end + 4..].to_vec() })
}

/// « sha256:aa:bb:… », « aa:bb:… » ou 64 caractères hexadécimaux.
fn parse_fingerprint(text: &str) -> Option<[u8; 32]> {
    let hex: String = text.trim().trim_start_matches("sha256:").chars().filter(|c| *c != ':').collect();
    if hex.len() != 64 || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    let mut out = [0u8; 32];
    for (i, byte) in out.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&hex[2 * i..2 * i + 2], 16).ok()?;
    }
    Some(out)
}

/// Seul le certificat dont l'empreinte est connue est accepté ; la signature
/// de la poignée de main est vérifiée normalement.
#[derive(Debug)]
struct Pinned {
    fingerprint: [u8; 32],
    provider: Arc<CryptoProvider>,
}

impl ServerCertVerifier for Pinned {
    fn verify_server_cert(
        &self,
        end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp_response: &[u8],
        _now: UnixTime,
    ) -> Result<ServerCertVerified, rustls::Error> {
        let seen: [u8; 32] = Sha256::digest(end_entity.as_ref()).into();
        if seen == self.fingerprint {
            Ok(ServerCertVerified::assertion())
        } else {
            Err(rustls::Error::General(t("tray.errors.wrongCertificate", &[])))
        }
    }

    fn verify_tls12_signature(&self, message: &[u8], cert: &CertificateDer<'_>, dss: &DigitallySignedStruct) -> Result<HandshakeSignatureValid, rustls::Error> {
        verify_tls12_signature(message, cert, dss, &self.provider.signature_verification_algorithms)
    }

    fn verify_tls13_signature(&self, message: &[u8], cert: &CertificateDer<'_>, dss: &DigitallySignedStruct) -> Result<HandshakeSignatureValid, rustls::Error> {
        verify_tls13_signature(message, cert, dss, &self.provider.signature_verification_algorithms)
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.provider.signature_verification_algorithms.supported_schemes()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reponse_decoupee() {
        let r = parse_reply(b"HTTP/1.1 403 Forbidden\r\nContent-Type: application/json\r\n\r\n{\"error\":\"x\"}").unwrap();
        assert_eq!(r.status, 403);
        assert_eq!(r.body, b"{\"error\":\"x\"}");
        assert!(parse_reply(b"HTTP/1.1 200 OK\r\n").is_err());
    }

    #[test]
    fn empreinte_lue() {
        let colons = (0..32).map(|_| "ab").collect::<Vec<_>>().join(":");
        assert_eq!(parse_fingerprint(&format!("sha256:{colons}")), Some([0xab; 32]));
        assert_eq!(parse_fingerprint(&"ab".repeat(32)), Some([0xab; 32]));
        assert_eq!(parse_fingerprint("sha256:ab"), None);
        assert_eq!(parse_fingerprint(&"zz".repeat(32)), None);
    }
}
