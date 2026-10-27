//! TLS du tunnel : les apps et la Station vérifient le relais par les
//! autorités du web, ou par l'empreinte épinglée dans son adresse ; la
//! Station présente son certificat client (signé par elle-même), que le
//! relais accepte sans autorité, après la preuve de possession de la clé.

use std::sync::Arc;

use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::crypto::CryptoProvider;
use rustls::pki_types::{CertificateDer, PrivateKeyDer, ServerName, UnixTime};
use rustls::{DigitallySignedStruct, RootCertStore, SignatureScheme};

use crate::addr::RelayAddr;
use crate::id::fingerprint;

/// Le fournisseur de cryptographie du tunnel : ring, partout.
pub fn provider() -> Arc<CryptoProvider> {
    Arc::new(rustls::crypto::ring::default_provider())
}

/// Les autorités du web (`webpki-roots`).
pub fn web_roots() -> Arc<RootCertStore> {
    let mut roots = RootCertStore::empty();
    roots.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
    Arc::new(roots)
}

/// Le certificat client de la Station.
pub struct ClientAuth {
    pub certs: Vec<CertificateDer<'static>>,
    pub key: PrivateKeyDer<'static>,
}

impl Clone for ClientAuth {
    fn clone(&self) -> Self {
        ClientAuth { certs: self.certs.clone(), key: self.key.clone_key() }
    }
}

impl std::fmt::Debug for ClientAuth {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ClientAuth").field("certs", &self.certs.len()).finish_non_exhaustive()
    }
}

/// Configuration TLS 1.3 vers un relais, pour un ALPN. `roots` remplace les
/// autorités du web (essais, relais d'entreprise) ; une empreinte dans
/// l'adresse du relais passe avant tout.
pub fn client_config(
    relay: &RelayAddr,
    roots: Option<Arc<RootCertStore>>,
    alpn: &[u8],
    auth: Option<&ClientAuth>,
) -> Result<rustls::ClientConfig, rustls::Error> {
    let provider = provider();
    let builder = rustls::ClientConfig::builder_with_provider(provider.clone())
        .with_protocol_versions(&[&rustls::version::TLS13])?;
    let builder = match &relay.pin {
        Some(pin) => builder
            .dangerous()
            .with_custom_certificate_verifier(Arc::new(PinnedVerifier { pin: pin.clone(), provider })),
        None => builder.with_root_certificates(roots.unwrap_or_else(web_roots)),
    };
    let mut cfg = match auth {
        Some(a) => builder.with_client_auth_cert(a.certs.clone(), a.key.clone_key())?,
        None => builder.with_no_client_auth(),
    };
    cfg.alpn_protocols = vec![alpn.to_vec()];
    Ok(cfg)
}

/// Le relais vérifié par l'empreinte de son certificat (le nom est ignoré).
#[derive(Debug)]
struct PinnedVerifier {
    pin: String,
    provider: Arc<CryptoProvider>,
}

impl ServerCertVerifier for PinnedVerifier {
    fn verify_server_cert(
        &self,
        end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp: &[u8],
        _now: UnixTime,
    ) -> Result<ServerCertVerified, rustls::Error> {
        if fingerprint(end_entity) == self.pin {
            Ok(ServerCertVerified::assertion())
        } else {
            Err(rustls::Error::InvalidCertificate(rustls::CertificateError::ApplicationVerificationFailure))
        }
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls12_signature(message, cert, dss, &self.provider.signature_verification_algorithms)
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls13_signature(message, cert, dss, &self.provider.signature_verification_algorithms)
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.provider.signature_verification_algorithms.supported_schemes()
    }
}

/// Le nom présenté au relais (SNI), ou son adresse IP.
pub fn server_name(relay: &RelayAddr) -> Result<ServerName<'static>, rustls::Error> {
    ServerName::try_from(relay.host.clone()).map_err(|_| rustls::Error::General(format!("nom de relais invalide : {}", relay.host)))
}

#[cfg(feature = "server")]
pub use server::AnyClientCert;

#[cfg(feature = "server")]
mod server {
    use super::*;
    use rustls::DistinguishedName;
    use rustls::server::danger::{ClientCertVerified, ClientCertVerifier};

    /// Côté relais : un certificat client facultatif, accepté sans autorité
    /// (la Station le signe elle-même). La négociation TLS prouve que le
    /// client tient la clé ; son identifiant vient de la clé publique.
    #[derive(Debug)]
    pub struct AnyClientCert {
        provider: Arc<CryptoProvider>,
    }

    impl AnyClientCert {
        pub fn new() -> Arc<Self> {
            Arc::new(AnyClientCert { provider: provider() })
        }
    }

    impl ClientCertVerifier for AnyClientCert {
        fn offer_client_auth(&self) -> bool {
            true
        }

        fn client_auth_mandatory(&self) -> bool {
            false
        }

        fn root_hint_subjects(&self) -> &[DistinguishedName] {
            &[]
        }

        fn verify_client_cert(
            &self,
            end_entity: &CertificateDer<'_>,
            _intermediates: &[CertificateDer<'_>],
            _now: UnixTime,
        ) -> Result<ClientCertVerified, rustls::Error> {
            // Un certificat lisible, dont on tire une clé publique.
            match crate::id::spki_of_cert(end_entity) {
                Some(_) => Ok(ClientCertVerified::assertion()),
                None => Err(rustls::Error::InvalidCertificate(rustls::CertificateError::BadEncoding)),
            }
        }

        fn verify_tls12_signature(
            &self,
            message: &[u8],
            cert: &CertificateDer<'_>,
            dss: &DigitallySignedStruct,
        ) -> Result<HandshakeSignatureValid, rustls::Error> {
            rustls::crypto::verify_tls12_signature(message, cert, dss, &self.provider.signature_verification_algorithms)
        }

        fn verify_tls13_signature(
            &self,
            message: &[u8],
            cert: &CertificateDer<'_>,
            dss: &DigitallySignedStruct,
        ) -> Result<HandshakeSignatureValid, rustls::Error> {
            rustls::crypto::verify_tls13_signature(message, cert, dss, &self.provider.signature_verification_algorithms)
        }

        fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
            self.provider.signature_verification_algorithms.supported_schemes()
        }
    }
}
