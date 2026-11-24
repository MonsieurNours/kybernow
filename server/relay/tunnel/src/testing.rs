//! Relais simulé, pour les essais de l'agent de la Station et d'Admin
//! (fonction `test-relay`) : un point QUIC sur la boucle locale, certificat
//! signé par lui-même (l'adresse rendue porte son empreinte, comme un relais
//! installé chez soi), qui accueille la Station (`hello` → `welcome`), garde
//! ses messages et répond à chacun ce que dit l'essai.
//!
//! Il ne fait que la connexion de contrôle : ni flux, ni paquets. C'est assez
//! pour le rattachement et l'enrôlement (docs/comptes.md).

use std::net::{Ipv4Addr, SocketAddr};
use std::sync::{Arc, Mutex};

use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer};

use crate::addr::RelayAddr;
use crate::frame::{read_msg, write_msg};
use crate::msg::{Msg, Welcome};
use crate::quic::{self, ALPN_CTL, ALPN_MEDIA};
use crate::tls::{self, AnyClientCert};
use crate::transport::{AbortOnDrop, RelayStream};

/// Ce que le relais simulé répond à un message de la Station (`None` : rien).
pub type Answer = dyn Fn(&Msg) -> Option<Msg> + Send + Sync;

pub struct FakeRelay {
    /// `127.0.0.1:port#sha256:…` : l'adresse à régler dans Admin.
    pub addr: RelayAddr,
    received: Arc<Mutex<Vec<Msg>>>,
    _task: AbortOnDrop,
}

impl FakeRelay {
    /// Démarre le relais (dans un runtime tokio).
    pub async fn start(answer: impl Fn(&Msg) -> Option<Msg> + Send + Sync + 'static) -> FakeRelay {
        let key = rcgen::KeyPair::generate().expect("clé du relais simulé");
        let cert = rcgen::CertificateParams::new(vec!["relay.test".into()]).expect("certificat").self_signed(&key).expect("certificat");
        let der: CertificateDer<'static> = cert.der().clone();
        let mut sc = rustls::ServerConfig::builder_with_provider(tls::provider())
            .with_protocol_versions(&[&rustls::version::TLS13])
            .expect("TLS 1.3")
            .with_client_cert_verifier(AnyClientCert::new())
            .with_single_cert(vec![der.clone()], PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(key.serialize_der())))
            .expect("certificat du relais simulé");
        sc.alpn_protocols = vec![ALPN_CTL.to_vec(), ALPN_MEDIA.to_vec()];
        let qsc = quinn::crypto::rustls::QuicServerConfig::try_from(sc).expect("QUIC");
        let base = quinn::ServerConfig::with_crypto(Arc::new(qsc));
        let endpoint = quinn::Endpoint::server(base.clone(), SocketAddr::from((Ipv4Addr::LOCALHOST, 0))).expect("point QUIC");
        let port = endpoint.local_addr().expect("port").port();
        let addr = RelayAddr { host: "127.0.0.1".into(), port, pin: Some(crate::id::fingerprint(der.as_ref())) };
        let received: Arc<Mutex<Vec<Msg>>> = Arc::default();
        let answer: Arc<Answer> = Arc::new(answer);
        let seen = received.clone();
        let task = tokio::spawn(async move {
            while let Some(incoming) = endpoint.accept().await {
                let (cfg, _) = quic::incoming_config(&base);
                let Ok(connecting) = incoming.accept_with(Arc::new(cfg)) else { continue };
                let (seen, answer) = (seen.clone(), answer.clone());
                tokio::spawn(async move {
                    let Ok(conn) = connecting.await else { return };
                    // La connexion des paquets (`kn-media/1`) est gardée ouverte, sans plus.
                    if quic::alpn(&conn).as_deref() != Some(ALPN_CTL) {
                        let _ = conn.closed().await;
                        return;
                    }
                    let id = quic::peer_station_id(&conn).unwrap_or_default();
                    let Ok((send, recv)) = conn.accept_bi().await else { return };
                    let mut ctl = RelayStream::quic(send, recv);
                    let Ok(Some(Msg::Hello(hello))) = read_msg(&mut ctl).await else { return };
                    let welcome = Welcome { v: crate::PROTOCOL_VERSION, id, relay: "simulé".into(), online: None, codes: hello.codes };
                    if write_msg(&mut ctl, &Msg::Welcome(welcome)).await.is_err() {
                        return;
                    }
                    while let Ok(Some(msg)) = read_msg(&mut ctl).await {
                        let reply = answer(&msg);
                        seen.lock().unwrap().push(msg);
                        if let Some(reply) = reply
                            && write_msg(&mut ctl, &reply).await.is_err()
                        {
                            break;
                        }
                    }
                });
            }
        });
        FakeRelay { addr, received, _task: AbortOnDrop(task) }
    }

    /// Les messages reçus de la Station, dans l'ordre (hors `hello`).
    pub fn received(&self) -> Vec<Msg> {
        self.received.lock().unwrap().clone()
    }
}
