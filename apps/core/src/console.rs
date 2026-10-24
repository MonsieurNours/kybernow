//! Client de KyberNow Admin (kyber-admin) : les routes `/api/stream/*` d'une
//! session et de l'accueil (info, aperçu), et `/api/pair/*` (appairage par
//! code). Les apps Apple y passent par `ConsoleClient` (ConsoleClientCore.swift).
//!
//! Compte : Basic, le même que pour Kyber. Certificat : auto-signé, épinglé
//! par son empreinte (`sha256:aa:bb:…`, SHA-256 du certificat DER).

use std::sync::{Arc, Mutex};
use std::time::Duration;

use base64::Engine;
use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::crypto::CryptoProvider;
use rustls::pki_types::{CertificateDer, ServerName, UnixTime};
use rustls::{DigitallySignedStruct, SignatureScheme};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::i18n::{self, Lang, tr};

#[derive(Debug)]
pub enum ConsoleError {
    /// Le certificat de la console n'est pas celui qu'on connaît.
    CertificateChanged { seen: String },
    /// Réponse HTTP d'erreur, avec le message de la console.
    Http { status: u16, message: String },
    Network(String),
    Invalid(String),
}

impl ConsoleError {
    /// Le message dans une langue (`Display` : celle de l'app ; les journaux
    /// prennent le français).
    pub fn text(&self, lang: Lang) -> String {
        match self {
            Self::CertificateChanged { seen } => tr(lang, "console.certificate_changed", &[("seen", seen)]),
            Self::Http { status, message } => tr(lang, "console.http", &[("message", message), ("status", &status.to_string())]),
            Self::Network(e) => tr(lang, "console.network", &[("error", e)]),
            Self::Invalid(e) => tr(lang, "console.invalid", &[("error", e)]),
        }
    }
}

impl std::fmt::Display for ConsoleError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.text(i18n::current()))
    }
}

impl std::error::Error for ConsoleError {}

pub type Result<T> = std::result::Result<T, ConsoleError>;

/// Empreinte d'un certificat au format des apps : `sha256:aa:bb:…`.
pub fn fingerprint(der: &[u8]) -> String {
    let digest = Sha256::digest(der);
    let hex: Vec<String> = digest.iter().map(|b| format!("{b:02x}")).collect();
    format!("sha256:{}", hex.join(":"))
}

#[derive(Debug)]
struct PinVerifier {
    pinned: Option<String>,
    seen: Arc<Mutex<Option<String>>>,
    provider: Arc<CryptoProvider>,
}

impl ServerCertVerifier for PinVerifier {
    fn verify_server_cert(
        &self,
        end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp_response: &[u8],
        _now: UnixTime,
    ) -> std::result::Result<ServerCertVerified, rustls::Error> {
        let fp = fingerprint(end_entity.as_ref());
        *self.seen.lock().unwrap() = Some(fp.clone());
        match &self.pinned {
            Some(pinned) if !pinned.eq_ignore_ascii_case(&fp) => Err(rustls::Error::General(
                "certificat de la console changé".into(),
            )),
            _ => Ok(ServerCertVerified::assertion()),
        }
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> std::result::Result<HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls12_signature(
            message,
            cert,
            dss,
            &self.provider.signature_verification_algorithms,
        )
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> std::result::Result<HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls13_signature(
            message,
            cert,
            dss,
            &self.provider.signature_verification_algorithms,
        )
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.provider
            .signature_verification_algorithms
            .supported_schemes()
    }
}

/// Réponse de `GET /api/stream/info`.
#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct Info {
    #[serde(default)]
    pub hostname: String,
    #[serde(default)]
    pub os: String,
    #[serde(default)]
    pub gpu: Option<String>,
    #[serde(default)]
    pub av1: bool,
    #[serde(default)]
    pub admin_version: Option<String>,
    #[serde(default)]
    pub virtual_display: VirtualDisplayInfo,
    #[serde(default)]
    pub audio: AudioInfo,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct VirtualDisplayInfo {
    #[serde(default)]
    pub available: bool,
}

/// Son de l'ordinateur, vu par la console.
#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct AudioInfo {
    /// Haut-parleurs virtuels (ceux de Steam) en place.
    #[serde(default)]
    pub speakers: bool,
}

/// L'appareil qui se présente à Admin (`app` : « KyberNow Link 0.3.1 »).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceInfo {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub os: String,
    pub app: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct PrepareRequest {
    pub width: u32,
    pub height: u32,
    pub refresh: u32,
    /// 0 : le son reste sur l'ordinateur ; 2, 6 ou 8 voies sinon.
    pub audio_channels: u16,
    pub virtual_display: bool,
    pub device: DeviceInfo,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub profile: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mode_policy: Option<String>,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
pub struct Mode {
    pub width: u32,
    pub height: u32,
    pub refresh: u32,
}

/// Réponse de `POST /api/stream/prepare`.
#[derive(Debug, Clone)]
pub struct Prepared {
    pub virtual_display: bool,
    /// Mode réellement appliqué à l'écran virtuel.
    pub applied: Option<Mode>,
    /// Voies du son si la sortie virtuelle est en place.
    pub audio_channels: Option<u16>,
    pub kyber_port: Option<u16>,
    pub heartbeat_seconds: u64,
    /// Empreinte du certificat de Kyber (console 0.2.8 et suivantes).
    pub kyber_fingerprint: Option<String>,
}

#[derive(Deserialize)]
struct PreparedWire {
    #[serde(default)]
    virtual_display: bool,
    applied: Option<Mode>,
    audio: Option<AudioWire>,
    kyber_port: Option<u16>,
    heartbeat_seconds: Option<u64>,
    kyber_fingerprint: Option<String>,
}

#[derive(Deserialize)]
struct AudioWire {
    #[serde(default)]
    routed: bool,
    channels: Option<u16>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Heartbeat {
    pub device_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub latency_ms: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub profile: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mode_policy: Option<String>,
}

/// Réponse de `POST /api/stream/heartbeat`.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct HeartbeatReply {
    /// L'écran virtuel de la session existe encore (`None` : console qui ne le dit pas).
    pub active: Option<bool>,
    /// La console demande la fin de la session.
    #[serde(default)]
    pub end: bool,
    pub mode: Option<Mode>,
    pub displays: Option<u32>,
    pub mode_policy: Option<String>,
}

/// Réponse de `GET /api/stream/foreground`.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct Foreground {
    #[serde(default)]
    pub known: bool,
    #[serde(default)]
    pub fullscreen: bool,
    #[serde(default)]
    pub game: bool,
}

/// Réponse de `POST /api/stream/end`.
#[derive(Debug, Clone, Deserialize)]
pub struct Ended {
    #[serde(default = "yes")]
    pub display_restored: bool,
    #[serde(default = "yes")]
    pub audio_restored: bool,
    pub error: Option<String>,
}

fn yes() -> bool {
    true
}

/// État d'une demande d'appairage par code.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PairStatus {
    Pending,
    Refused,
    Expired,
    /// Compte Kyber propre à cet appareil, créé par la console.
    Accepted { username: String, password: String },
}

pub struct Console {
    base: String,
    host: String,
    authorization: String,
    agent: ureq::Agent,
    seen: Arc<Mutex<Option<String>>>,
}

impl Console {
    /// `pinned` : empreinte attendue ; sans elle, le premier certificat vu est
    /// accepté (lisible ensuite par `fingerprint_seen`).
    pub fn new(host: &str, port: u16, user: &str, password: &str, pinned: Option<String>) -> Self {
        let provider = Arc::new(rustls::crypto::ring::default_provider());
        let seen = Arc::new(Mutex::new(None));
        let verifier = PinVerifier {
            pinned,
            seen: seen.clone(),
            provider: provider.clone(),
        };
        let tls = rustls::ClientConfig::builder_with_provider(provider)
            .with_safe_default_protocol_versions()
            .expect("versions TLS")
            .dangerous()
            .with_custom_certificate_verifier(Arc::new(verifier))
            .with_no_client_auth();
        let agent = ureq::AgentBuilder::new()
            .tls_config(Arc::new(tls))
            .timeout_connect(Duration::from_secs(5))
            .build();
        let bracketed = if host.contains(':') && !host.starts_with('[') {
            format!("[{host}]")
        } else {
            host.to_string()
        };
        let token = base64::engine::general_purpose::STANDARD.encode(format!("{user}:{password}"));
        Self {
            base: format!("https://{bracketed}:{port}"),
            host: host.to_string(),
            authorization: format!("Basic {token}"),
            agent,
            seen,
        }
    }

    /// Adresse de l'ordinateur, sans crochets : Kyber écoute à la même.
    pub fn host(&self) -> &str {
        &self.host
    }

    pub fn fingerprint_seen(&self) -> Option<String> {
        self.seen.lock().unwrap().clone()
    }

    fn request(&self, method: &str, path: &str, timeout: Duration) -> ureq::Request {
        self.agent
            .request(method, &format!("{}/{path}", self.base))
            .timeout(timeout)
            .set("Authorization", &self.authorization)
            .set("x-kyber-admin", "1")
            // La console répond dans la langue de l'app (messages d'erreur, refus).
            .set("Accept-Language", i18n::current().code())
    }

    fn read<T: for<'de> Deserialize<'de>>(&self, r: std::result::Result<ureq::Response, ureq::Error>) -> Result<T> {
        match r {
            Ok(resp) => resp.into_json().map_err(|e| ConsoleError::Invalid(e.to_string())),
            Err(ureq::Error::Status(status, resp)) => {
                let message = resp
                    .into_json::<serde_json::Value>()
                    .ok()
                    .and_then(|v| v.get("error").and_then(|e| e.as_str()).map(str::to_owned))
                    .unwrap_or_else(|| tr(i18n::current(), "console.http_status", &[("status", &status.to_string())]));
                Err(ConsoleError::Http { status, message })
            }
            Err(ureq::Error::Transport(t)) => {
                let text = t.to_string();
                if text.contains("certificat de la console") {
                    let seen = self.fingerprint_seen().unwrap_or_default();
                    Err(ConsoleError::CertificateChanged { seen })
                } else {
                    Err(ConsoleError::Network(text))
                }
            }
        }
    }

    pub fn info(&self) -> Result<Info> {
        let r = self.request("GET", "api/stream/info", Duration::from_secs(4)).call();
        self.read(r)
    }

    pub fn prepare(&self, req: &PrepareRequest) -> Result<Prepared> {
        let r = self
            .request("POST", "api/stream/prepare", Duration::from_secs(40))
            .send_json(req);
        let w: PreparedWire = self.read(r)?;
        Ok(Prepared {
            virtual_display: w.virtual_display,
            applied: w.applied.filter(|m| m.width > 0 && m.height > 0),
            audio_channels: w.audio.filter(|a| a.routed).and_then(|a| a.channels),
            kyber_port: w.kyber_port,
            heartbeat_seconds: w.heartbeat_seconds.unwrap_or(5).max(1),
            kyber_fingerprint: w.kyber_fingerprint.filter(|s| !s.is_empty()),
        })
    }

    pub fn heartbeat(&self, hb: &Heartbeat) -> Result<HeartbeatReply> {
        let r = self
            .request("POST", "api/stream/heartbeat", Duration::from_secs(20))
            .send_json(hb);
        self.read(r)
    }

    pub fn foreground(&self) -> Result<Foreground> {
        let r = self
            .request("GET", "api/stream/foreground", Duration::from_secs(2))
            .call();
        self.read(r)
    }

    /// Premier contact, sans compte : le certificat est vu (et retenu par
    /// `fingerprint_seen`) même si la console répond 401.
    pub fn probe(&self) -> Result<Option<Info>> {
        match self.info() {
            Ok(i) => Ok(Some(i)),
            Err(ConsoleError::Http { status: 401 | 403, .. }) => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// Aperçu JPEG de l'écran principal (hors session ; 503 pendant une
    /// session ou sans capture possible).
    pub fn preview(&self, width: u32) -> Result<Vec<u8>> {
        let r = self
            .request("GET", &format!("api/stream/preview?width={width}"), Duration::from_secs(8))
            .call();
        match r {
            Ok(resp) => {
                let mut buf = Vec::new();
                std::io::Read::read_to_end(&mut resp.into_reader(), &mut buf)
                    .map_err(|e| ConsoleError::Network(e.to_string()))?;
                Ok(buf)
            }
            Err(e) => self.read::<serde_json::Value>(Err(e)).map(|_| Vec::new()),
        }
    }

    /// Demande d'accès sans compte avec un code à six chiffres, à accepter
    /// dans la console ; rend l'identifiant de la demande.
    pub fn pair_request(&self, code: &str, device: &DeviceInfo) -> Result<String> {
        let r = self
            .request("POST", "api/pair/request", Duration::from_secs(6))
            .send_json(serde_json::json!({ "code": code, "device": device }));
        let v: serde_json::Value = self.read(r)?;
        v.get("id")
            .and_then(|i| i.as_str())
            .map(str::to_owned)
            .ok_or_else(|| ConsoleError::Invalid(tr(i18n::current(), "console.no_request_id", &[])))
    }

    pub fn pair_status(&self, id: &str) -> Result<PairStatus> {
        let r = self
            .request("GET", &format!("api/pair/request/{id}"), Duration::from_secs(6))
            .call();
        let v: serde_json::Value = self.read(r)?;
        let field = |k: &str| v.get(k).and_then(|x| x.as_str()).unwrap_or_default().to_string();
        Ok(match field("status").as_str() {
            "pending" => PairStatus::Pending,
            "refused" => PairStatus::Refused,
            "accepted" => PairStatus::Accepted {
                username: field("username"),
                password: field("password"),
            },
            _ => PairStatus::Expired,
        })
    }

    pub fn end(&self, device_id: &str, latency_ms: Option<f64>) -> Result<Ended> {
        let mut body = serde_json::json!({ "device_id": device_id });
        if let Some(l) = latency_ms {
            body["latency_ms"] = serde_json::json!((l * 10.0).round() / 10.0);
        }
        let r = self
            .request("POST", "api/stream/end", Duration::from_secs(20))
            .send_json(body);
        self.read(r)
    }
}
