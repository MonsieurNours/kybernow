//! Client HTTPS minimal.
//!
//! - **Vers Admin**, qui n'accepte que son certificat : le même que celui de
//!   l'icône Windows (`server/tray/src/http.rs`), pour la fenêtre de mise à
//!   jour de la Station Mac et l'étape « Compte KyberNow » des deux
//!   installateurs. L'empreinte attendue est `console_fingerprint` de
//!   `kybernow.json` (écrite par Admin à chaque démarrage).
//! - **Vers un relais** (« Autre relais… », vérifié en direct) : autorités du
//!   web (`webpki-roots`), ou l'empreinte d'un relais signé par lui-même.
//!
//! Une requête HTTP/1.1 par connexion (`Connection: close`) : la réponse va
//! jusqu'à la fermeture (un corps découpé, `chunked`, est recollé).

use std::io::{Read, Write};
use std::net::{IpAddr, Ipv4Addr, SocketAddr, TcpStream, ToSocketAddrs};
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result};
use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::crypto::{CryptoProvider, verify_tls12_signature, verify_tls13_signature};
use rustls::pki_types::{CertificateDer, ServerName, UnixTime};
use rustls::{DigitallySignedStruct, SignatureScheme};
use sha2::{Digest, Sha256};

use crate::i18n::lang;

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

    /// Un relais du web (autorités de `webpki-roots`), ou signé par lui-même
    /// quand `pin` (« sha256:aa:bb:… ») est donné.
    pub fn relay(pin: Option<&str>) -> Result<Self> {
        if let Some(pin) = pin {
            return Self::pinned(pin);
        }
        let provider = Arc::new(rustls::crypto::ring::default_provider());
        let mut roots = rustls::RootCertStore::empty();
        roots.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
        let config = rustls::ClientConfig::builder_with_provider(provider)
            .with_safe_default_protocol_versions()
            .context("versions TLS")?
            .with_root_certificates(roots)
            .with_no_client_auth();
        Ok(Self { config: Arc::new(config) })
    }

    /// `GET https://{host}:{port}{path}` (relais) ; le nom de l'hôte sert à la
    /// vérification du certificat, sauf pour un certificat épinglé.
    pub fn get(&self, host: &str, port: u16, path: &str, timeout: Duration) -> Result<Reply> {
        let addrs: Vec<SocketAddr> = (host, port).to_socket_addrs().with_context(|| format!("{host} introuvable"))?.collect();
        let mut last = None;
        for addr in addrs {
            match TcpStream::connect_timeout(&addr, timeout) {
                Ok(tcp) => {
                    let name = match host.parse::<IpAddr>() {
                        Ok(ip) => ServerName::IpAddress(ip.into()),
                        Err(_) => ServerName::try_from(host.to_string()).context("nom d'hôte invalide")?,
                    };
                    let authority = if host.contains(':') { format!("[{host}]:{port}") } else { format!("{host}:{port}") };
                    return self.exchange(tcp, name, &authority, "GET", path, &[], None, timeout);
                }
                Err(e) => last = Some(e),
            }
        }
        Err(last.map(anyhow::Error::from).unwrap_or_else(|| anyhow::anyhow!("aucune adresse"))).with_context(|| format!("{host}:{port} injoignable"))
    }

    /// Une requête sans corps vers `https://127.0.0.1:{port}{path}`, avec la
    /// langue de l'icône (`Accept-Language`) : la console y répond dans cette langue.
    pub fn call(&self, method: &str, port: u16, path: &str, headers: &[(&str, &str)], timeout: Duration) -> Result<Reply> {
        self.send(method, port, path, headers, None, timeout)
    }

    /// [`Client::call`] avec un corps JSON (`body`), pour les actions du partage.
    pub fn send(&self, method: &str, port: u16, path: &str, headers: &[(&str, &str)], body: Option<&[u8]>, timeout: Duration) -> Result<Reply> {
        let addr = SocketAddr::from((Ipv4Addr::LOCALHOST, port));
        let tcp = TcpStream::connect_timeout(&addr, timeout).context("KyberNow Admin ne répond pas")?;
        let name = ServerName::IpAddress(Ipv4Addr::LOCALHOST.into());
        self.exchange(tcp, name, &format!("127.0.0.1:{port}"), method, path, headers, body, timeout)
    }

    #[allow(clippy::too_many_arguments)]
    fn exchange(
        &self,
        tcp: TcpStream,
        name: ServerName<'static>,
        authority: &str,
        method: &str,
        path: &str,
        headers: &[(&str, &str)],
        body: Option<&[u8]>,
        timeout: Duration,
    ) -> Result<Reply> {
        tcp.set_read_timeout(Some(timeout))?;
        tcp.set_write_timeout(Some(timeout))?;
        tcp.set_nodelay(true)?;
        let conn = rustls::ClientConnection::new(self.config.clone(), name).context("connexion TLS")?;
        let mut tls = rustls::StreamOwned::new(conn, tcp);

        let length = body.map_or(0, <[u8]>::len);
        let mut request = format!("{method} {path} HTTP/1.1\r\nHost: {authority}\r\nConnection: close\r\nContent-Length: {length}\r\nAccept-Language: {}\r\n", lang().code());
        if body.is_some() {
            request.push_str("Content-Type: application/json\r\n");
        }
        for (name, value) in headers {
            request.push_str(&format!("{name}: {value}\r\n"));
        }
        request.push_str("\r\n");
        let mut bytes = request.into_bytes();
        bytes.extend_from_slice(body.unwrap_or_default());
        tls.write_all(&bytes).context("envoi de la requête")?;

        let mut raw = Vec::new();
        match tls.read_to_end(&mut raw) {
            Ok(_) => {}
            // Fin de connexion sans close_notify : fréquent, le corps est entier avec Connection: close.
            Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => {}
            Err(e) if !raw.is_empty() && e.kind() == std::io::ErrorKind::ConnectionReset => {}
            Err(e) => return Err(e).context("lecture de la réponse"),
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
    let chunked = head.lines().any(|l| {
        let l = l.to_ascii_lowercase();
        l.starts_with("transfer-encoding:") && l.contains("chunked")
    });
    let body = &raw[end + 4..];
    Ok(Reply { status, body: if chunked { dechunk(body) } else { body.to_vec() } })
}

/// Recolle un corps découpé (`Transfer-Encoding: chunked`).
fn dechunk(mut rest: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    while let Some(line_end) = rest.windows(2).position(|w| w == b"\r\n") {
        let size_text = std::str::from_utf8(&rest[..line_end]).unwrap_or("0");
        let size = usize::from_str_radix(size_text.split(';').next().unwrap_or("0").trim(), 16).unwrap_or(0);
        rest = &rest[line_end + 2..];
        if size == 0 || rest.len() < size {
            out.extend_from_slice(&rest[..size.min(rest.len())]);
            break;
        }
        out.extend_from_slice(&rest[..size]);
        rest = rest.get(size + 2..).unwrap_or_default();
    }
    out
}

/// « sha256:aa:bb:… », « aa:bb:… » ou 64 caractères hexadécimaux.
pub fn parse_fingerprint(text: &str) -> Option<[u8; 32]> {
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
            Err(rustls::Error::General("certificat inattendu : ce n'est pas KyberNow Admin".into()))
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
        let r = parse_reply(b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n4\r\n{\"a\"\r\n3\r\n:1}\r\n0\r\n\r\n").unwrap();
        assert_eq!(r.body, b"{\"a\":1}");
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
