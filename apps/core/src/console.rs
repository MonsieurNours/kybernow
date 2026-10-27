//! Client de KyberNow Admin (kyber-admin) : les routes `/api/stream/*` d'une
//! session et de l'accueil (info, aperçu), de la rubrique « Station » des
//! réglages (redémarrer, mettre à jour la Station, ouvrir Admin),
//! `/api/pair/*` (appairage par code) et `/api/share/*` (KyberNow Share,
//! [`crate::share`]). Les apps Apple y passent par `ConsoleClient`
//! (ConsoleClientCore.swift).
//!
//! Compte : Basic, le même que pour Kyber ; aucun pour un invité de KyberNow
//! Share ([`Console::guest`], son jeton dans `x-kybernow-guest`). Certificat :
//! auto-signé, épinglé par son empreinte (`sha256:aa:bb:…`, SHA-256 du
//! certificat DER).

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
use crate::share::{JoinRequest, ShareBeat, ShareError, ShareInfo, ShareJoined, ShareOpened, ShareRole, ShareState};
use crate::wake::WakeTarget;

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
    /// Le compte qui appelle (Station 0.5.0 et suivantes).
    #[serde(default)]
    pub account: AccountInfo,
    /// La Station (0.5.0 et suivantes) ; `None` avec une Station plus
    /// ancienne : les apps n'y proposent ni redémarrage ni mise à jour.
    #[serde(default)]
    pub station: Option<StationInfo>,
    /// Les cartes réseau de l'ordinateur pour le réveiller par le réseau
    /// (Station 0.5.0 et suivantes ; vide avec une plus ancienne), Ethernet
    /// d'abord. Les apps les gardent pour chaque ordinateur : voir [`crate::wake`].
    #[serde(default, deserialize_with = "crate::wake::lenient")]
    pub wake: Vec<WakeTarget>,
    /// KyberNow Share (Station 0.8.0 et suivantes) ; `None` avec une Station
    /// antérieure : voir [`crate::share::ShareAvailability`].
    #[serde(default)]
    pub share: Option<ShareInfo>,
}

/// Le compte avec lequel l'app appelle Admin.
#[derive(Debug, Clone, Deserialize, Serialize, Default, PartialEq, Eq)]
pub struct AccountInfo {
    /// Compte créé pour cet appareil par l'appairage par code : les apps
    /// l'affichent comme « Accès propre à cet appareil ».
    #[serde(default)]
    pub own_device: bool,
}

/// `https://adresse:port` d'Admin, une adresse IPv6 entre crochets.
fn base_url(host: &str, port: u16) -> String {
    if host.contains(':') && !host.starts_with('[') {
        format!("https://[{host}]:{port}")
    } else {
        format!("https://{host}:{port}")
    }
}

/// Adresse d'Admin à ouvrir dans le navigateur ou à mettre dans un QR code
/// (`https://adresse:port/`), sans créer de [`Console`] : pour les écrans qui
/// la montrent hors de toute connexion.
pub fn admin_url(host: &str, port: u16) -> String {
    format!("{}/", base_url(host, port))
}

/// La Station, pour la rubrique « Station » des réglages d'un ordinateur.
#[derive(Debug, Clone, Deserialize, Serialize, Default, PartialEq, Eq)]
pub struct StationInfo {
    /// Version de KyberNow Station (celle d'Admin).
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub update: StationUpdate,
    /// Un redémarrage est en cours : Admin va s'arrêter puis revenir.
    #[serde(default)]
    pub restarting: bool,
}

/// Mise à jour de la Station, telle qu'Admin la connaît (sans appel réseau).
#[derive(Debug, Clone, Deserialize, Serialize, Default, PartialEq, Eq)]
pub struct StationUpdate {
    /// Une version plus récente, signée par KyberNow, est publiée.
    #[serde(default)]
    pub available: bool,
    #[serde(default)]
    pub version: Option<String>,
    /// Elle s'installe depuis un appareil (`Console::station_update`).
    #[serde(default)]
    pub installable: bool,
    /// Elle s'installe avec l'installateur, sur l'ordinateur.
    #[serde(default)]
    pub requires_setup: bool,
    /// Une installation est en cours.
    #[serde(default)]
    pub installing: bool,
}

/// Refus d'un redémarrage ou d'une mise à jour de la Station.
#[derive(Debug)]
pub enum StationError {
    /// Un appareil est en session (son nom, si Admin le donne) : refaire
    /// l'appel avec `force` coupe sa session.
    InSession { device: Option<String>, message: String },
    /// Trop tôt (429) : un redémarrage par minute au plus, ou trop d'essais
    /// ratés depuis cette adresse ; `seconds` : l'attente.
    TooSoon { seconds: Option<u64>, message: String },
    /// Rien à installer depuis un appareil : Station à jour, ou version refusée.
    NothingToInstall { message: String },
    /// La version publiée s'installe avec l'installateur, sur l'ordinateur.
    RequiresSetup { message: String },
    /// Une installation ou un redémarrage est déjà en cours.
    Busy { message: String },
    /// Station 0.4.9 ou plus ancienne : elle ne connaît pas l'appel.
    Unsupported,
    /// Le reste : Admin injoignable, certificat changé, compte refusé, échec.
    Other(ConsoleError),
}

impl StationError {
    /// Le message dans une langue : celui d'Admin (écrit dans la langue de
    /// l'app, `Accept-Language`), ou celui du cœur.
    pub fn text(&self, lang: Lang) -> String {
        match self {
            Self::InSession { message, .. }
            | Self::TooSoon { message, .. }
            | Self::NothingToInstall { message }
            | Self::RequiresSetup { message }
            | Self::Busy { message } => message.clone(),
            Self::Unsupported => tr(lang, "station.unsupported", &[]),
            Self::Other(e) => e.text(lang),
        }
    }

    /// Nom stable du refus, pour les ponts des apps : `in_session`,
    /// `too_soon`, `nothing_to_install`, `requires_setup`, `busy`,
    /// `unsupported`, `other`.
    pub fn kind(&self) -> &'static str {
        match self {
            Self::InSession { .. } => "in_session",
            Self::TooSoon { .. } => "too_soon",
            Self::NothingToInstall { .. } => "nothing_to_install",
            Self::RequiresSetup { .. } => "requires_setup",
            Self::Busy { .. } => "busy",
            Self::Unsupported => "unsupported",
            Self::Other(_) => "other",
        }
    }

    /// Le refus en JSON pour les ponts : `kind`, `message` (langue de
    /// l'app), `device` (appareil en session) et `seconds` (attente), nuls
    /// s'ils ne s'appliquent pas.
    pub fn to_json(&self) -> serde_json::Value {
        let (device, seconds) = match self {
            Self::InSession { device, .. } => (device.clone(), None),
            Self::TooSoon { seconds, .. } => (None, *seconds),
            _ => (None, None),
        };
        serde_json::json!({ "kind": self.kind(), "message": self.to_string(), "device": device, "seconds": seconds })
    }
}

impl std::fmt::Display for StationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.text(i18n::current()))
    }
}

impl std::error::Error for StationError {}

/// Refus d'Admin sur une route de la Station, d'après le code HTTP et le corps
/// (`error`, `code`, `device`, `retry_after`). `None` : erreur ordinaire.
fn station_refusal(status: u16, body: &serde_json::Value, retry_after: Option<u64>) -> Option<StationError> {
    let message = body
        .get("error")
        .and_then(|e| e.as_str())
        .filter(|m| !m.is_empty())
        .map(str::to_owned)
        .unwrap_or_else(|| tr(i18n::current(), "console.http_status", &[("status", &status.to_string())]));
    let code = body.get("code").and_then(|c| c.as_str()).unwrap_or_default();
    Some(match (status, code) {
        // Une Station qui ne connaît pas la route.
        (404 | 405, _) => StationError::Unsupported,
        (429, _) => StationError::TooSoon { seconds: body.get("retry_after").and_then(|s| s.as_u64()).or(retry_after), message },
        (409, "session") => StationError::InSession { device: body.get("device").and_then(|d| d.as_str()).map(str::to_owned), message },
        (409, "up_to_date" | "not_installable") => StationError::NothingToInstall { message },
        (409, "requires_setup") => StationError::RequiresSetup { message },
        (409, "installing" | "restarting") => StationError::Busy { message },
        _ => return None,
    })
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct VirtualDisplayInfo {
    #[serde(default)]
    pub available: bool,
    /// Écrans que la Station sait créer en une session (multi-écrans) ;
    /// absent chez une Station antérieure : 1 ([`Self::max_screens`]).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_screens: Option<usize>,
}

impl VirtualDisplayInfo {
    /// Pour [`crate::screens::Screens::max_screens`].
    pub fn max_screens(&self) -> usize {
        self.max_screens.unwrap_or(1).max(1)
    }
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
    /// Multi-écrans (docs/multi-ecrans.md) : un écran virtuel par entrée, de
    /// 1 à 4, exactement un `primary` en (0, 0) ; `width`, `height` et
    /// `refresh` décrivent alors l'écran principal (une Station qui ignore ce
    /// champ fait une session à un écran). Absent : une session à un écran.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub screens: Option<Vec<ScreenMode>>,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
pub struct Mode {
    pub width: u32,
    pub height: u32,
    pub refresh: u32,
}

/// Un écran virtuel d'une session à plusieurs écrans : son mode et sa
/// position en pixels sur le bureau de Windows (demandés, ou appliqués par
/// la Station).
#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
pub struct ScreenMode {
    pub width: u32,
    pub height: u32,
    pub refresh: u32,
    pub x: i32,
    pub y: i32,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub primary: bool,
}

impl ScreenMode {
    pub fn mode(&self) -> Mode {
        Mode { width: self.width, height: self.height, refresh: self.refresh }
    }
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
    /// Multi-écrans : ce qui est en place, dans l'ordre de la demande
    /// (`None` : écran que la Station n'a pas pu créer). Absent : Station qui
    /// ne gère qu'un écran (ou demande sans `screens`) : la session repart
    /// sur un écran.
    pub screens: Option<Vec<Option<ScreenMode>>>,
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
    #[serde(default)]
    screens: Option<Vec<Option<ScreenMode>>>,
}

#[derive(Deserialize)]
struct AudioWire {
    #[serde(default)]
    routed: bool,
    channels: Option<u16>,
}

impl PreparedWire {
    fn prepared(self) -> Prepared {
        let usable = |m: &ScreenMode| m.width > 0 && m.height > 0;
        Prepared {
            virtual_display: self.virtual_display,
            applied: self.applied.filter(|m| m.width > 0 && m.height > 0),
            audio_channels: self.audio.filter(|a| a.routed).and_then(|a| a.channels),
            kyber_port: self.kyber_port,
            heartbeat_seconds: self.heartbeat_seconds.unwrap_or(5).max(1),
            kyber_fingerprint: self.kyber_fingerprint.filter(|s| !s.is_empty()),
            screens: self.screens.map(|list| list.into_iter().map(|m| m.filter(usable)).collect()),
        }
    }
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
    /// Le partage ouvert par cet hôte (KyberNow Share) ; absent sans partage.
    #[serde(default, deserialize_with = "crate::share::lenient_state")]
    pub share: Option<ShareState>,
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
    /// `None` : invité de KyberNow Share, sans compte.
    authorization: Option<String>,
    agent: ureq::Agent,
    seen: Arc<Mutex<Option<String>>>,
}

impl Console {
    /// `pinned` : empreinte attendue ; sans elle, le premier certificat vu est
    /// accepté (lisible ensuite par `fingerprint_seen`).
    pub fn new(host: &str, port: u16, user: &str, password: &str, pinned: Option<String>) -> Self {
        let token = base64::engine::general_purpose::STANDARD.encode(format!("{user}:{password}"));
        Self::build(host, port, Some(format!("Basic {token}")), pinned)
    }

    /// Sans compte, pour un invité de KyberNow Share (`share_join`,
    /// `share_beat`, `share_leave`) ; `pinned` comme pour [`Self::new`].
    pub fn guest(host: &str, port: u16, pinned: Option<String>) -> Self {
        Self::build(host, port, None, pinned)
    }

    fn build(host: &str, port: u16, authorization: Option<String>, pinned: Option<String>) -> Self {
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
        Self {
            base: base_url(host, port),
            host: host.to_string(),
            authorization,
            agent,
            seen,
        }
    }

    /// Adresse de l'ordinateur, sans crochets : Kyber écoute à la même.
    pub fn host(&self) -> &str {
        &self.host
    }

    /// Adresse d'Admin à ouvrir dans le navigateur (`https://adresse:port/`,
    /// une adresse IPv6 entre crochets). Son certificat est auto-signé : le
    /// navigateur demande de l'accepter la première fois.
    pub fn admin_url(&self) -> String {
        format!("{}/", self.base)
    }

    pub fn fingerprint_seen(&self) -> Option<String> {
        self.seen.lock().unwrap().clone()
    }

    fn request(&self, method: &str, path: &str, timeout: Duration) -> ureq::Request {
        let r = self
            .agent
            .request(method, &format!("{}/{path}", self.base))
            .timeout(timeout)
            .set("x-kyber-admin", "1")
            // La console répond dans la langue de l'app (messages d'erreur, refus).
            .set("Accept-Language", i18n::current().code());
        match &self.authorization {
            Some(a) => r.set("Authorization", a),
            None => r,
        }
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

    /// Une réponse sans contenu utile (corps vide, `{}`, `{"ok":true}`).
    fn read_unit(&self, r: std::result::Result<ureq::Response, ureq::Error>) -> Result<()> {
        match r {
            Ok(_) => Ok(()),
            Err(e) => self.read::<serde_json::Value>(Err(e)).map(|_| ()),
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
        Ok(w.prepared())
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

    /// Redémarre la Station entière (le serveur Kyber et Admin, environ 30
    /// secondes ; Admin ne répond plus pendant ce temps). `device_id` : cet
    /// appareil, nommé dans le fil d'Admin ; `force` : même si un appareil
    /// est en session (elle est coupée). Tout appareil appairé peut le faire.
    pub fn station_restart(&self, device_id: Option<&str>, force: bool) -> std::result::Result<(), StationError> {
        self.station_call("api/stream/station/restart", device_id, force, Duration::from_secs(15)).map(|_| ())
    }

    /// Lance la mise à jour de la Station vers la version annoncée par
    /// `Info::station` (comme « Installer maintenant » dans Admin) ; rend
    /// cette version. Admin télécharge et vérifie le paquet avant de répondre
    /// (quelques minutes au plus sur une connexion lente), puis s'arrête et
    /// revient dans la nouvelle version, en une minute environ.
    pub fn station_update(&self, device_id: Option<&str>, force: bool) -> std::result::Result<String, StationError> {
        let v = self.station_call("api/stream/station/update", device_id, force, Duration::from_secs(300))?;
        Ok(v.get("version").and_then(|v| v.as_str()).unwrap_or_default().to_string())
    }

    fn station_call(
        &self,
        path: &str,
        device_id: Option<&str>,
        force: bool,
        timeout: Duration,
    ) -> std::result::Result<serde_json::Value, StationError> {
        let mut body = serde_json::json!({ "force": force });
        if let Some(id) = device_id.filter(|id| !id.is_empty()) {
            body["device_id"] = serde_json::json!(id);
        }
        match self.request("POST", path, timeout).send_json(body) {
            // Une Station 0.4.9 répond la page d'Admin (200, HTML) à une route
            // qu'elle ne connaît pas.
            Ok(resp) => match resp.into_json::<serde_json::Value>() {
                Ok(v) if v.get("ok").and_then(|o| o.as_bool()) == Some(true) => Ok(v),
                _ => Err(StationError::Unsupported),
            },
            Err(ureq::Error::Status(status, resp)) => {
                let retry_after = resp.header("Retry-After").and_then(|h| h.trim().parse().ok());
                let body: serde_json::Value = resp.into_json().unwrap_or(serde_json::Value::Null);
                Err(station_refusal(status, &body, retry_after).unwrap_or_else(|| {
                    let message = body
                        .get("error")
                        .and_then(|e| e.as_str())
                        .map(str::to_owned)
                        .unwrap_or_else(|| tr(i18n::current(), "console.http_status", &[("status", &status.to_string())]));
                    StationError::Other(ConsoleError::Http { status, message })
                }))
            }
            Err(e) => Err(StationError::Other(
                self.read::<serde_json::Value>(Err(e)).err().unwrap_or_else(|| ConsoleError::Invalid(String::new())),
            )),
        }
    }

    // KyberNow Share, côté hôte (docs/kybernow-share.md, « Échange avec
    // Admin ») : le compte de l'appareil en session. Bloquants.

    /// Ouvre le partage de l'écran principal de la session : code, lien, lien
    /// du client web. Admin refuse (409) sans session de cet appareil, ou si
    /// le partage est coupé sur l'ordinateur ; une Station antérieure répond 404.
    pub fn share_open(&self) -> Result<ShareOpened> {
        let r = self.request("POST", "api/share/open", Duration::from_secs(10)).send_json(serde_json::json!({}));
        self.read(r)
    }

    /// Le partage en cours (`open` faux sans partage), avec ses invités.
    pub fn share_state(&self) -> Result<ShareState> {
        let r = self.request("GET", "api/share", Duration::from_secs(6)).call();
        self.read(r)
    }

    /// « Nouveau code » : les invités déjà là restent.
    pub fn share_code(&self) -> Result<ShareOpened> {
        let r = self.request("POST", "api/share/code", Duration::from_secs(6)).send_json(serde_json::json!({}));
        self.read(r)
    }

    /// « Faire jouer », « Donner le clavier et la souris », « Reprendre le
    /// clavier et la souris » : le rôle d'un invité.
    pub fn share_guest(&self, guest: &str, role: ShareRole) -> Result<()> {
        let path = format!("api/share/guest/{}", path_segment(guest));
        let r = self.request("POST", &path, Duration::from_secs(6)).send_json(serde_json::json!({ "role": role.as_str() }));
        self.read_unit(r)
    }

    /// « Retirer » un invité du partage.
    pub fn share_kick(&self, guest: &str) -> Result<()> {
        let path = format!("api/share/guest/{}", path_segment(guest));
        let r = self.request("DELETE", &path, Duration::from_secs(6)).call();
        self.read_unit(r)
    }

    /// « Arrêter le partage » : tout le monde sort.
    pub fn share_close(&self) -> Result<()> {
        let r = self.request("DELETE", "api/share", Duration::from_secs(6)).call();
        self.read_unit(r)
    }

    // KyberNow Share, côté invité ([`Self::guest`], sans compte).

    /// `POST /api/share/join` : le code, le prénom, l'appareil. Voir
    /// [`crate::share::join`] pour l'envoyer à toutes les Stations du réseau.
    pub fn share_join(&self, req: &JoinRequest) -> std::result::Result<ShareJoined, ShareError> {
        match self.request("POST", "api/share/join", Duration::from_secs(6)).send_json(req) {
            // Une Station antérieure peut répondre la page d'Admin (200, HTML) :
            // ce n'est pas elle qui partage.
            Ok(resp) => resp.into_json::<ShareJoined>().map_err(|_| ShareError::NoShare).and_then(|j| {
                if j.guest.is_empty() || j.secret.is_empty() { Err(ShareError::NoShare) } else { Ok(j) }
            }),
            Err(ureq::Error::Status(status, resp)) => {
                let retry_after = resp.header("Retry-After").and_then(|h| h.trim().parse().ok());
                let body: serde_json::Value = resp.into_json().unwrap_or(serde_json::Value::Null);
                let code = body.get("error").and_then(|e| e.as_str()).unwrap_or_default();
                Err(match (status, code) {
                    (404 | 405, _) => ShareError::NoShare,
                    (409, "full") => ShareError::Full,
                    (429, _) => ShareError::TooManyTries { seconds: body.get("retry_after").and_then(|s| s.as_u64()).or(retry_after) },
                    _ => {
                        let message = if code.is_empty() { tr(i18n::current(), "console.http_status", &[("status", &status.to_string())]) } else { code.to_string() };
                        ShareError::Other(ConsoleError::Http { status, message })
                    }
                })
            }
            Err(e) => Err(ShareError::Other(self.read::<serde_json::Value>(Err(e)).err().unwrap_or_else(|| ConsoleError::Invalid(String::new())))),
        }
    }

    /// Signe de vie de l'invité (toutes les `beat` secondes) : rôle, jeton
    /// de Kyber renouvelé, fin du partage.
    pub fn share_beat(&self, guest: &str, secret: &str) -> Result<ShareBeat> {
        let r = self
            .request("POST", "api/share/beat", Duration::from_secs(10))
            .set("x-kybernow-guest", secret)
            .send_json(serde_json::json!({ "guest": guest }));
        self.read(r)
    }

    /// L'invité quitte le partage.
    pub fn share_leave(&self, guest: &str, secret: &str) -> Result<()> {
        let r = self
            .request("POST", "api/share/leave", Duration::from_secs(6))
            .set("x-kybernow-guest", secret)
            .send_json(serde_json::json!({ "guest": guest }));
        self.read_unit(r)
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

/// Un identifiant d'invité dans un chemin : seulement ce qui ne demande pas
/// d'encodage (les identifiants de la Station sont alphanumériques).
fn path_segment(id: &str) -> String {
    id.chars().filter(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.')).collect()
}

#[cfg(test)]
mod tests {
    #[test]
    fn adresse_d_admin_sans_console() {
        assert_eq!(super::admin_url("192.168.1.20", 24811), "https://192.168.1.20:24811/");
        assert_eq!(super::admin_url("fe80::1", 9443), "https://[fe80::1]:9443/");
        assert_eq!(super::admin_url("[fe80::1]", 9443), "https://[fe80::1]:9443/");
    }

    #[test]
    fn compte_propre_a_l_appareil() {
        let info: super::Info = serde_json::from_str(r#"{"account":{"own_device":true}}"#).unwrap();
        assert!(info.account.own_device);
        let old: super::Info = serde_json::from_str("{}").unwrap();
        assert!(!old.account.own_device);
    }

    use super::*;

    fn console(host: &str) -> Console {
        Console::new(host, 9443, "mac", "secret", None)
    }

    fn device() -> DeviceInfo {
        DeviceInfo { id: "mac-1".into(), name: "Mac".into(), kind: "mac".into(), os: "macOS".into(), app: "KyberNow Link".into() }
    }

    fn request(screens: Option<Vec<ScreenMode>>) -> PrepareRequest {
        PrepareRequest {
            width: 3840,
            height: 2160,
            refresh: 160,
            audio_channels: 2,
            virtual_display: true,
            device: device(),
            profile: Some("Bureau".into()),
            mode_policy: None,
            screens,
        }
    }

    #[test]
    fn ecrans_que_la_station_sait_creer() {
        let info: Info = serde_json::from_str(r#"{"virtual_display": {"available": true, "max_screens": 4}}"#).unwrap();
        assert_eq!(info.virtual_display.max_screens(), 4);
        let old: Info = serde_json::from_str(r#"{"virtual_display": {"available": true}}"#).unwrap();
        assert_eq!(old.virtual_display.max_screens(), 1, "Station antérieure : un écran");
        assert!(serde_json::to_value(&old).unwrap()["virtual_display"].get("max_screens").is_none());
    }

    #[test]
    fn demande_a_un_ecran_inchangee() {
        let v = serde_json::to_value(request(None)).unwrap();
        assert!(v.get("screens").is_none(), "pas de champ screens pour un écran");
        assert_eq!(v["width"], 3840);
    }

    #[test]
    fn demande_multi_ecrans() {
        let screens = vec![
            ScreenMode { width: 3840, height: 2160, refresh: 160, x: 0, y: 0, primary: true },
            ScreenMode { width: 5120, height: 2880, refresh: 60, x: 3840, y: 0, primary: false },
            ScreenMode { width: 3456, height: 2234, refresh: 60, x: -3456, y: 400, primary: false },
        ];
        let v = serde_json::to_value(request(Some(screens))).unwrap();
        assert_eq!(
            v["screens"],
            serde_json::json!([
                { "width": 3840, "height": 2160, "refresh": 160, "x": 0, "y": 0, "primary": true },
                { "width": 5120, "height": 2880, "refresh": 60, "x": 3840, "y": 0 },
                { "width": 3456, "height": 2234, "refresh": 60, "x": -3456, "y": 400 }
            ])
        );
        assert_eq!((v["width"].clone(), v["refresh"].clone()), (serde_json::json!(3840), serde_json::json!(160)));
    }

    #[test]
    fn reponse_multi_ecrans() {
        let body = r#"{"virtual_display": true, "applied": {"width": 3840, "height": 2160, "refresh": 160},
            "kyber_port": 8080, "screens": [
              { "width": 3840, "height": 2160, "refresh": 160, "x": 0, "y": 0, "primary": true },
              { "width": 5120, "height": 2880, "refresh": 60, "x": 3840, "y": 0 },
              null ]}"#;
        let p = serde_json::from_str::<PreparedWire>(body).unwrap().prepared();
        let screens = p.screens.expect("écrans");
        assert_eq!(screens.len(), 3, "dans l'ordre de la demande");
        assert!(screens[0].unwrap().primary);
        assert_eq!(screens[1].unwrap().mode(), Mode { width: 5120, height: 2880, refresh: 60 });
        assert_eq!((screens[1].unwrap().x, screens[1].unwrap().y), (3840, 0));
        assert!(screens[2].is_none(), "écran non créé");
        // Station qui ne gère qu'un écran : pas de champ, session à un écran.
        let old = serde_json::from_str::<PreparedWire>(r#"{"virtual_display": true, "applied": {"width": 3840, "height": 2160, "refresh": 160}}"#)
            .unwrap()
            .prepared();
        assert!(old.screens.is_none());
        assert_eq!(old.heartbeat_seconds, 5);
    }

    #[test]
    fn adresse_d_admin() {
        assert_eq!(console("192.168.1.20").admin_url(), "https://192.168.1.20:9443/");
        assert_eq!(console("fe80::1").admin_url(), "https://[fe80::1]:9443/");
        assert_eq!(console("[fe80::1]").admin_url(), "https://[fe80::1]:9443/");
        assert_eq!(console("bureau.local").admin_url(), "https://bureau.local:9443/");
    }

    #[test]
    fn station_absente_avant_0_5_0() {
        let old: Info = serde_json::from_str(r#"{"hostname":"Bureau","os":"Windows 11","admin_version":"0.4.9"}"#).unwrap();
        assert_eq!(old.station, None);
        let v = serde_json::to_value(&old).unwrap();
        assert!(v["station"].is_null());
        // Ni cartes pour le réveil : une liste vide.
        assert!(old.wake.is_empty());
        assert_eq!(v["wake"], serde_json::json!([]));
    }

    #[test]
    fn cartes_du_reveil_dans_info() {
        let info: Info = serde_json::from_str(
            r#"{"hostname":"Bureau","wake":[
            {"mac":"3C:7C:3F:12:AB:CD","ipv4":"192.168.1.20","broadcast":"192.168.1.255","kind":"ethernet","armed":true,"name":"Intel(R) Ethernet Connection I219-V"},
            {"mac":"A4:B1:C1:00:11:22","ipv4":null,"broadcast":null,"kind":"wifi","armed":null}]}"#,
        )
        .unwrap();
        assert_eq!(info.wake.len(), 2);
        assert_eq!(info.wake[0].broadcast.as_deref(), Some("192.168.1.255"));
        assert_eq!((info.wake[1].kind.as_str(), info.wake[1].armed), ("wifi", None));
        // `wake` illisible : ignoré, le reste de la réponse est lu (l'ordinateur ne passe pas pour hors ligne).
        for odd in [r#""wake":null"#, r#""wake":"?""#, r#""wake":[{"ipv4":"192.168.1.20"},{"mac":"3C:7C:3F:12:AB:CD"}]"#] {
            let info: Info = serde_json::from_str(&format!(r#"{{"hostname":"Bureau",{odd}}}"#)).unwrap();
            assert_eq!(info.hostname, "Bureau");
            assert!(info.wake.len() <= 1, "{odd}");
        }
    }

    #[test]
    fn station_lue_dans_info() {
        let info: Info = serde_json::from_str(
            r#"{"hostname":"Bureau","admin_version":"0.5.0","station":{"version":"0.5.0","restarting":false,
            "update":{"available":true,"version":"0.5.1","installable":true,"requires_setup":false,"installing":false}}}"#,
        )
        .unwrap();
        let station = info.station.unwrap();
        assert_eq!(station.version, "0.5.0");
        assert!(station.update.available && station.update.installable && !station.update.requires_setup);
        assert_eq!(station.update.version.as_deref(), Some("0.5.1"));
        // Champs absents : valeurs par défaut.
        let partial: Info = serde_json::from_str(r#"{"station":{"version":"0.5.0"}}"#).unwrap();
        assert_eq!(partial.station.unwrap().update, StationUpdate::default());
    }

    #[test]
    fn refus_de_la_station() {
        let body = serde_json::json!({ "error": "Un appareil est connecté (Mac)", "code": "session", "device": "Mac" });
        let refusal = station_refusal(409, &body, None).unwrap();
        assert_eq!(
            refusal.to_json(),
            serde_json::json!({ "kind": "in_session", "message": "Un appareil est connecté (Mac)", "device": "Mac", "seconds": null })
        );
        match refusal {
            StationError::InSession { device, message } => {
                assert_eq!(device.as_deref(), Some("Mac"));
                assert_eq!(message, "Un appareil est connecté (Mac)");
            }
            other => panic!("{other:?}"),
        }
        let soon = serde_json::json!({ "error": "réessayez dans 42 s", "code": "too_soon", "retry_after": 42 });
        assert!(matches!(station_refusal(429, &soon, None), Some(StationError::TooSoon { seconds: Some(42), .. })));
        // Le délai de l'en-tête, si le corps n'en a pas.
        assert!(matches!(station_refusal(429, &serde_json::Value::Null, Some(7)), Some(StationError::TooSoon { seconds: Some(7), .. })));
        for code in ["up_to_date", "not_installable"] {
            let body = serde_json::json!({ "error": "à jour", "code": code });
            assert!(matches!(station_refusal(409, &body, None), Some(StationError::NothingToInstall { .. })), "{code}");
        }
        let setup = serde_json::json!({ "error": "installateur", "code": "requires_setup" });
        assert!(matches!(station_refusal(409, &setup, None), Some(StationError::RequiresSetup { .. })));
        for code in ["installing", "restarting"] {
            let body = serde_json::json!({ "error": "déjà", "code": code });
            assert!(matches!(station_refusal(409, &body, None), Some(StationError::Busy { .. })), "{code}");
        }
        assert!(matches!(station_refusal(404, &serde_json::Value::Null, None), Some(StationError::Unsupported)));
        // Le reste reste une erreur ordinaire (compte refusé, échec).
        assert!(station_refusal(401, &serde_json::json!({ "error": "identifiants", "code": "unauthorized" }), None).is_none());
        assert!(station_refusal(500, &serde_json::json!({ "error": "échec", "code": "failed" }), None).is_none());
        assert!(station_refusal(409, &serde_json::json!({ "error": "autre" }), None).is_none());
    }

    #[test]
    fn station_trop_ancienne() {
        assert_eq!(
            StationError::Unsupported.text(Lang::Fr),
            "cette version de KyberNow Station ne le permet pas : mettez-la à jour depuis Admin"
        );
        assert_eq!(StationError::Unsupported.text(Lang::En), "this version of KyberNow Station can’t do this: update it from Admin");
    }
}
