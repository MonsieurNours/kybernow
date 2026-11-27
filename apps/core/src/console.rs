//! Client de KyberNow Admin (kyber-admin) : les routes `/api/stream/*` d'une
//! session et de l'accueil (info, aperçu), de la rubrique « Station » des
//! réglages (redémarrer, mettre à jour la Station, ouvrir Admin),
//! `/api/pair/*` (appairage par code) et `/api/share/*` (KyberNow Share,
//! [`crate::share`]). Les apps Apple y passent par `ConsoleClient`
//! (ConsoleClientCore.swift).
//!
//! Compte : Basic, le même que pour Kyber ; aucun pour un invité de KyberNow
//! Share ([`Console::guest`], son jeton dans `x-kybernow-guest`) ; pour un
//! ordinateur d'un compte KyberNow ([`Console::account`], docs/comptes.md), le
//! jeton d'accès du relais (`Authorization: KyberNow <jeton>`), redemandé
//! avant son échéance (90 secondes) et après un refus `401 token_expired`. Certificat :
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

use crate::account::Reply;
use crate::i18n::{self, Lang, tr};
use crate::share::{JoinRequest, ShareAuto, ShareBeat, ShareError, ShareImage, ShareInfo, ShareJoined, ShareOpened, ShareQuality, ShareRole, ShareState, WatchRequest};
use crate::wake::WakeTarget;

#[derive(Debug)]
pub enum ConsoleError {
    /// Le certificat de la console n'est pas celui qu'on connaît.
    CertificateChanged { seen: String },
    /// Réponse HTTP d'erreur, avec le message de la console.
    Http { status: u16, message: String },
    /// Par le relais : le tunnel ne s'ouvre pas (relais muet, ordinateur
    /// absent du relais).
    Relay(crate::relay::RelayError),
    Network(String),
    Invalid(String),
    /// Un ordinateur d'un compte KyberNow : pas de jeton d'accès du relais
    /// (session de l'appareil fermée, ordinateur retiré du compte, relais muet).
    Account(crate::account::AccountError),
}

impl ConsoleError {
    /// Le message dans une langue (`Display` : celle de l'app ; les journaux
    /// prennent le français).
    pub fn text(&self, lang: Lang) -> String {
        match self {
            Self::CertificateChanged { seen } => tr(lang, "console.certificate_changed", &[("seen", seen)]),
            Self::Http { status, message } => tr(lang, "console.http", &[("message", message), ("status", &status.to_string())]),
            Self::Relay(e) => e.text(lang, None),
            Self::Network(e) => tr(lang, "console.network", &[("error", e)]),
            Self::Invalid(e) => tr(lang, "console.invalid", &[("error", e)]),
            Self::Account(e) => e.text(lang),
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

/// Une réponse d'erreur d'Admin : `{"error":"<texte>","code":"<code>"}`.
/// Par le relais, l'accès coupé dans Admin (403 `remote_access_off`) devient
/// [`crate::relay::RelayError::AccessOff`], pour que les apps ne le
/// reconnaissent pas à son texte.
fn http_error(status: u16, body: Option<serde_json::Value>) -> ConsoleError {
    let code = body.as_ref().and_then(|v| v.get("code")).and_then(|c| c.as_str()).unwrap_or_default();
    if status == 403 && code == "remote_access_off" {
        return ConsoleError::Relay(crate::relay::RelayError::AccessOff);
    }
    let message = body
        .as_ref()
        .and_then(|v| v.get("error").and_then(|e| e.as_str()).map(str::to_owned))
        .unwrap_or_else(|| tr(i18n::current(), "console.http_status", &[("status", &status.to_string())]));
    ConsoleError::Http { status, message }
}

/// Empreinte d'un certificat au format des apps : `sha256:aa:bb:…`.
pub fn fingerprint(der: &[u8]) -> String {
    let digest = Sha256::digest(der);
    let hex: Vec<String> = digest.iter().map(|b| format!("{b:02x}")).collect();
    format!("sha256:{}", hex.join(":"))
}

/// Réponse d'Admin dont le module lit lui-même les refus
/// ([`Console::answer`]).
pub(crate) enum Answer {
    /// Reçue (2xx), à lire.
    Ok(ureq::Response),
    /// Refusée : statut, corps, `Retry-After`.
    Refused(Reply),
}

#[derive(Debug)]
struct PinVerifier {
    pinned: Option<String>,
    /// Sans empreinte connue : la première vue, exigée ensuite de toutes
    /// les connexions de cette console (COEUR-19).
    first: Mutex<Option<String>>,
    /// La dernière vue (celle d'un refus, pour le dire).
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
        *self.seen.lock().unwrap_or_else(|e| e.into_inner()) = Some(fp.clone());
        let expected = match &self.pinned {
            Some(pinned) => Some(pinned.clone()),
            None => {
                let mut first = self.first.lock().unwrap_or_else(|e| e.into_inner());
                first.get_or_insert_with(|| fp.clone()).clone().into()
            }
        };
        match expected {
            Some(pinned) if !kybernow_rules::fingerprint::same(&pinned, &fp) => Err(rustls::Error::General(
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
    /// PyroWave de la Station (docs/pyrowave.md) ; `None` avec une Station
    /// d'avant PyroWave : ne jamais le demander (elle refuserait le format).
    /// Voir [`Info::pyrowave_offered`].
    #[serde(default)]
    pub pyrowave: Option<PyroWaveInfo>,
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
    /// Le relais de la Station (Station 0.8.0 et suivantes, relais réglé ;
    /// `None` sinon) : l'app le garde pour chaque ordinateur, pour passer
    /// par lui hors de chez soi ([`crate::relay::RelayInfo::route`]).
    #[serde(default)]
    pub relay: Option<crate::relay::RelayInfo>,
    /// Le prêt de l'ordinateur (Station 0.9.0 et suivantes ; `None` sinon) :
    /// `available` (la Station sait prêter), `enabled` (activé dans Admin).
    #[serde(default)]
    pub lend: Option<crate::lend::LendInfo>,
    /// Les cartes réseau de l'ordinateur pour le réveiller par le réseau
    /// (Station 0.5.0 et suivantes ; vide avec une plus ancienne), Ethernet
    /// d'abord. Les apps les gardent pour chaque ordinateur : voir [`crate::wake`].
    #[serde(default, deserialize_with = "crate::wake::lenient")]
    pub wake: Vec<WakeTarget>,
    /// KyberNow Share (Station 0.8.0 et suivantes) ; `None` avec une Station
    /// antérieure : voir [`crate::share::ShareAvailability`].
    #[serde(default)]
    pub share: Option<ShareInfo>,
    /// Le HDR de la Station (docs/hdr.md) ; `None` avec une Station d'avant
    /// le HDR : « L'ordinateur ne propose pas le HDR. ». Voir
    /// [`crate::profile::hdr_state`].
    #[serde(default)]
    pub hdr: Option<HdrInfo>,
    /// Les manettes de la Station (Station Mac : absentes tant qu'Apple n'a
    /// pas accordé son droit) ; `None` avec une Station d'avant : disponibles.
    /// Voir [`Info::gamepads_available`].
    #[serde(default)]
    pub gamepads: Option<GamepadsInfo>,
    /// La Station sait essayer son encodeur (`GET /api/stream/encoder`,
    /// docs/profil-auto.md, « L'encodeur de l'ordinateur ») ; absent : non.
    #[serde(default)]
    pub encoder_check: bool,
}

impl Info {
    /// La Station propose PyroWave : `pyrowave.available` (absent : non).
    pub fn pyrowave_offered(&self) -> bool {
        self.pyrowave.as_ref().is_some_and(|p| p.available)
    }

    /// L'ordinateur crée les manettes de l'appareil (docs/link-ui.md,
    /// « Manettes indisponibles ») : vrai sans le champ (Stations d'avant).
    pub fn gamepads_available(&self) -> bool {
        self.gamepads.as_ref().is_none_or(|g| g.available)
    }
}

/// Ce que [`Console::encoder_check`] demande à la Station : un écran, son
/// format et son débit (Mbit/s).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EncoderQuery {
    pub codec: crate::profile::Codec,
    pub width: u32,
    pub height: u32,
    pub fps: u32,
    pub mbps: u32,
    pub yuv444: bool,
    pub hdr: bool,
}

/// La réponse de la sonde de l'encodeur.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EncoderAnswer {
    Accepted,
    /// L'encodeur refuse (son message).
    Refused(String),
    /// La Station n'a pas pu essayer (outil absent, sonde occupée).
    Unknown(String),
}

#[derive(Deserialize)]
struct EncoderReply {
    #[serde(default)]
    ok: Option<bool>,
    #[serde(default)]
    error: Option<String>,
}

/// `gamepads` de `GET /api/stream/info`.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct GamepadsInfo {
    #[serde(default = "gamepads_yes")]
    pub available: bool,
    /// Pourquoi elles manquent (« entitlement » : droit d'Apple pas accordé).
    #[serde(default)]
    pub reason: Option<String>,
}

fn gamepads_yes() -> bool {
    true
}

/// `pyrowave` de `GET /api/stream/info` (docs/pyrowave.md), même forme que `hdr`.
#[derive(Debug, Clone, Deserialize, Serialize, Default, PartialEq, Eq)]
pub struct PyroWaveInfo {
    /// La Station propose PyroWave (carte d'Admin allumée, encodeur prêt).
    #[serde(default)]
    pub available: bool,
    /// Sinon pourquoi : `off` (réglage d'Admin), `windows`, `vulkan`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    /// « Débit maximal » d'Admin, en Mbit/s : réglé (300, 500, 800), ou en
    /// « Auto » d'après la prise réseau de la Station. Absent (Station plus
    /// ancienne, prise inconnue) : pas de plafond de l'ordinateur. Au-delà,
    /// les apps baissent la définition ([`crate::pyrowave`]).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_mbps: Option<u32>,
}

/// `hdr` de `GET /api/stream/info`.
#[derive(Debug, Clone, Deserialize, Serialize, Default, PartialEq, Eq)]
pub struct HdrInfo {
    /// La Station sait faire le HDR (Admin sur « Si l'appareil le demande »,
    /// Windows 11 24H2, encodeur 10 bits).
    #[serde(default)]
    pub available: bool,
    /// Sinon pourquoi : `off`, `windows`, `encoder` (et `display` : sans
    /// écran virtuel, l'écran du PC est en SDR).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

/// Le compte avec lequel l'app appelle Admin.
#[derive(Debug, Clone, Deserialize, Serialize, Default, PartialEq, Eq)]
pub struct AccountInfo {
    /// Compte créé pour cet appareil par l'appairage par code : les apps
    /// l'affichent comme « Accès propre à cet appareil ».
    #[serde(default)]
    pub own_device: bool,
}

/// `https://adresse:port` d'Admin, une adresse IPv6 entre crochets (une
/// seule fois, même donnée avec les siens).
fn base_url(host: &str, port: u16) -> String {
    format!("https://{}:{port}", host_for_url(host))
}

/// Une console PlayStation que voit une Station (`GET /api/consoles`) :
/// l'app retient, par `host_id`, les Stations qui la voient.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StationConsole {
    /// `host-id` de la console, en majuscules.
    pub host_id: String,
    #[serde(default)]
    pub name: String,
    /// `ps5` ou `ps4`.
    #[serde(default)]
    pub model: String,
    /// `ready`, `standby` (ou `unknown`).
    #[serde(default)]
    pub state: String,
    /// Dernière réponse à la Station, en secondes Unix.
    #[serde(default)]
    pub seen: u64,
}

#[derive(Deserialize)]
struct ConsoleList {
    #[serde(default)]
    consoles: Vec<StationConsole>,
}

/// Adresse d'Admin à ouvrir dans le navigateur ou à mettre dans un QR code
/// (`https://adresse:port/`, crochets IPv6 compris, jamais doublés), sans
/// créer de [`Console`] : pour les écrans qui la montrent hors de toute
/// connexion (TRANS-36). Lire une adresse tapée : [`parse_address`].
pub use kybernow_rules::address::admin_url;
pub use kybernow_rules::address::{ADDRESS_EXAMPLE, ADMIN_PORT, host_for_url, parse_address};
/// Empreinte du certificat d'Admin (TRANS-34, MAC-20) : l'affichage court
/// d'Admin et de toutes les apps (`7C41 E09B AABB CCDD`), la règle « même
/// empreinte » (préfixe, deux-points, espaces et casse ignorés), la forme
/// `sha256:aa:bb:…`.
pub use kybernow_rules::fingerprint::{normalize as normalize_fingerprint, same as same_fingerprint, same_opt as same_fingerprint_opt, short as short_fingerprint};

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
    /// HDR demandé pour l'écran d'une session à un écran (docs/hdr.md) ; à
    /// plusieurs écrans, celui de l'écran principal, et chaque écran a le
    /// sien (`screens[i].hdr`). Absent : SDR.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub hdr: bool,
    /// Les valeurs HDR de l'écran de l'appareil (docs/hdr.md, « Réglage du
    /// HDR »), avec `hdr` ; à plusieurs écrans, celles du principal, et
    /// chaque écran a les siennes (`screens[i].luminance`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub luminance: Option<crate::profile::HdrLuminance>,
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
#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq)]
pub struct ScreenMode {
    pub width: u32,
    pub height: u32,
    pub refresh: u32,
    pub x: i32,
    pub y: i32,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub primary: bool,
    /// HDR demandé pour cet écran (`prepare`) ; absent : SDR.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub hdr: bool,
    /// Les valeurs HDR de l'écran de l'appareil qui le reçoit, avec `hdr`
    /// (absentes : celles par défaut).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub luminance: Option<crate::profile::HdrLuminance>,
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
    /// Adresse de l'ordinateur à prendre pour l'image (Kyber) au lieu de celle
    /// d'Admin : la Station la donne quand l'appareil est sur le même réseau
    /// qu'une adresse qu'elle préfère (Station Mac branchée en Ethernet et en
    /// Wi-Fi : l'Ethernet). Absente : l'adresse d'Admin.
    pub kyber_host: Option<String>,
    pub heartbeat_seconds: u64,
    /// Empreinte du certificat de Kyber (console 0.2.8 et suivantes).
    pub kyber_fingerprint: Option<String>,
    /// Multi-écrans : ce qui est en place, dans l'ordre de la demande
    /// (`None` : écran que la Station n'a pas pu créer). Absent : Station qui
    /// ne gère qu'un écran (ou demande sans `screens`) : la session repart
    /// sur un écran.
    pub screens: Option<Vec<Option<ScreenMode>>>,
    /// Le HDR obtenu pour chaque écran, dans l'ordre de `screens` (un seul
    /// élément pour un écran). Absent (Station d'avant le HDR) : SDR partout.
    pub hdr: Vec<bool>,
    /// Un compte KyberNow (docs/comptes.md) : le jeton de Kyber (2 minutes, renouvelé par le signe de vie)
    /// sous le compte `kn-account-…`, à la place du compte Basic.
    pub kyber_token: Option<KyberToken>,
}

/// Un jeton de Kyber donné par Admin à un compte KyberNow (`prepare`, puis
/// chaque signe de vie).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct KyberToken {
    pub token: String,
    /// Le compte Kyber du jeton (`kn-account-…`).
    #[serde(default)]
    pub user: String,
    /// Durée de vie, en secondes.
    #[serde(default)]
    pub expires: u64,
}

#[derive(Deserialize)]
struct PreparedWire {
    #[serde(default)]
    virtual_display: bool,
    applied: Option<Mode>,
    audio: Option<AudioWire>,
    kyber_port: Option<u16>,
    #[serde(default)]
    kyber_host: Option<String>,
    heartbeat_seconds: Option<u64>,
    kyber_fingerprint: Option<String>,
    #[serde(default)]
    screens: Option<Vec<Option<ScreenMode>>>,
    #[serde(default)]
    hdr: Option<Vec<bool>>,
    #[serde(default)]
    kyber_token: Option<KyberToken>,
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
            kyber_host: self.kyber_host.map(|h| h.trim().to_string()).filter(|h| !h.is_empty()),
            heartbeat_seconds: self.heartbeat_seconds.unwrap_or(5).max(1),
            kyber_fingerprint: self.kyber_fingerprint.filter(|s| !s.is_empty()),
            screens: self.screens.map(|list| list.into_iter().map(|m| m.filter(usable)).collect()),
            hdr: self.hdr.unwrap_or_default(),
            kyber_token: self.kyber_token.filter(|t| !t.token.is_empty()),
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
    /// La vue du prêt de l'ordinateur (docs/pret.md) ; absente quand il n'y a
    /// rien à dire (ni code, ni demande, ni prêt).
    #[serde(default, deserialize_with = "crate::lend::lenient_view")]
    pub lend: Option<crate::lend::LendView>,
    /// L'enregistrement de la session (docs/enregistrement.md) ; absent :
    /// Station qui n'enregistre pas.
    #[serde(default, deserialize_with = "crate::recording::lenient")]
    pub recording: Option<crate::recording::RecordingState>,
    /// Un compte KyberNow : un jeton de Kyber neuf, pour ses reconnexions.
    #[serde(default)]
    pub kyber_token: Option<KyberToken>,
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

/// Le compte d'une console.
#[derive(Clone)]
enum Auth {
    /// Invité de KyberNow Share, sans compte.
    None,
    /// `Basic …` : le compte Kyber de l'appareil.
    Fixed(String),
    /// Un compte KyberNow : le jeton d'accès du relais pour cette Station.
    Account(Arc<crate::account::StationAccess>),
}

pub struct Console {
    base: String,
    host: String,
    authorization: Auth,
    agent: ureq::Agent,
    seen: Arc<Mutex<Option<String>>>,
    /// Par le relais ([`Self::with_relay`]) : Admin par le tunnel.
    relay: Option<RelayLink>,
}

/// La console par le relais : le tunnel, ouvert au premier appel, et la
/// raison d'un échec à l'ouvrir (rendue par l'appel qui échoue).
struct RelayLink {
    route: crate::relay::RelayRoute,
    tunnel: Mutex<Option<crate::relay::RelayTunnel>>,
}

/// Ce qu'un appel a appris avant de partir ou en échouant, rangé pour le
/// fil qui le fait et pour cette console seulement : le jeton d'accès qui
/// n'a pas pu être demandé, le tunnel qui ne s'ouvre pas, le jeton refusé
/// par Admin. Repris par la lecture de ce même appel (`read`,
/// `with_fresh_token`). Rangé dans la console, un autre fil le reprenait
/// (signe de vie, premier plan… partagent la console de la session) : une
/// raison fausse, et un réessai fait ou manqué à tort (COEUR-21).
#[derive(Default)]
struct CallNotes {
    account: Option<(usize, crate::account::AccountError)>,
    relay: Option<(usize, crate::relay::RelayError)>,
    token_refused: Option<usize>,
}

thread_local! {
    static CALL_NOTES: std::cell::RefCell<CallNotes> = std::cell::RefCell::new(CallNotes::default());
}

fn take_note<T>(console: usize, slot: impl FnOnce(&mut CallNotes) -> &mut Option<(usize, T)>) -> Option<T> {
    CALL_NOTES.with(|n| {
        let mut n = n.borrow_mut();
        let s = slot(&mut n);
        match s.take() {
            Some((c, v)) if c == console => Some(v),
            other => {
                *s = other;
                None
            }
        }
    })
}

impl RelayLink {
    fn tunnel(&self) -> std::result::Result<crate::relay::RelayTunnel, crate::relay::RelayError> {
        let mut slot = self.tunnel.lock().unwrap();
        if let Some(t) = slot.as_ref() {
            return Ok(t.clone());
        }
        let t = crate::relay::open(&self.route)?;
        *slot = Some(t.clone());
        Ok(t)
    }
}

impl Console {
    /// `pinned` : empreinte attendue ; sans elle, le premier certificat vu est
    /// accepté (lisible ensuite par `fingerprint_seen`), puis exigé de toutes
    /// les connexions suivantes de cette console.
    pub fn new(host: &str, port: u16, user: &str, password: &str, pinned: Option<String>) -> Self {
        let token = base64::engine::general_purpose::STANDARD.encode(format!("{user}:{password}"));
        Self::build(host, port, Auth::Fixed(format!("Basic {token}")), pinned)
    }

    /// Un ordinateur d'un compte KyberNow (docs/comptes.md) : chaque appel
    /// porte le jeton d'accès du relais (`Authorization: KyberNow <jeton>`),
    /// sur le réseau comme par le relais ([`Self::with_relay`]) ; `pinned`
    /// comme pour [`Self::new`] (l'empreinte que donne `/v1/computers`).
    pub fn account(host: &str, port: u16, access: Arc<crate::account::StationAccess>, pinned: Option<String>) -> Self {
        Self::build(host, port, Auth::Account(access), pinned)
    }

    /// Le compte KyberNow de cette console, s'il y en a un.
    #[cfg(test)]
    pub fn station_access(&self) -> Option<&Arc<crate::account::StationAccess>> {
        match &self.authorization {
            Auth::Account(a) => Some(a),
            _ => None,
        }
    }

    /// Sans compte, pour un invité de KyberNow Share (`share_join`,
    /// `share_beat`, `share_leave`) ; `pinned` comme pour [`Self::new`].
    pub fn guest(host: &str, port: u16, pinned: Option<String>) -> Self {
        Self::build(host, port, Auth::None, pinned)
    }

    fn build(host: &str, port: u16, authorization: Auth, pinned: Option<String>) -> Self {
        let provider = Arc::new(rustls::crypto::ring::default_provider());
        let seen = Arc::new(Mutex::new(None));
        let verifier = PinVerifier {
            pinned,
            first: Mutex::new(None),
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
            relay: None,
        }
    }

    /// Par le relais (`Some`) : tous les appels passent par le tunnel vers
    /// la Station de `route` (ouvert au premier appel, partagé, fermé 30 s
    /// après le dernier usage), Admin épinglé sur `route.admin_fp`, jamais de
    /// premier contact. `None` : la console telle quelle. `host` reste
    /// l'adresse de l'ordinateur sur son réseau (`admin_url`).
    pub fn with_relay(self, route: Option<crate::relay::RelayRoute>) -> Self {
        let Some(route) = route else { return self };
        let mut c = Self::build(&self.host, 0, self.authorization.clone(), Some(route.admin_fp.clone()));
        c.base = self.base;
        c.relay = Some(RelayLink { route, tunnel: Mutex::new(None) });
        c
    }

    /// La route par le relais de cette console, s'il y en a une.
    pub fn relay(&self) -> Option<&crate::relay::RelayRoute> {
        self.relay.as_ref().map(|l| &l.route)
    }

    /// Une autre console vers la même Station par le relais (même compte,
    /// même route, même tunnel) : pour un signe de vie tenu sur un autre fil
    /// (`playstation::remote`). `None` sans relais.
    pub fn relay_twin(&self) -> Option<Console> {
        let link = self.relay.as_ref()?;
        let mut c = Self::build(&self.host, 0, self.authorization.clone(), Some(link.route.admin_fp.clone()));
        c.base = self.base.clone();
        c.relay = Some(RelayLink { route: link.route.clone(), tunnel: Mutex::new(None) });
        Some(c)
    }

    /// Le tunnel de cette console (ouvert s'il ne l'est pas) : la session y
    /// prend aussi Kyber ([`crate::relay::RelayTunnel::kyber_addr`]).
    pub fn relay_tunnel(&self) -> Option<std::result::Result<crate::relay::RelayTunnel, crate::relay::RelayError>> {
        self.relay.as_ref().map(RelayLink::tunnel)
    }

    /// Où envoyer un appel : Admin sur son réseau, ou par le tunnel.
    fn url(&self, path: &str) -> String {
        let Some(link) = &self.relay else { return format!("{}/{path}", self.base) };
        match link.tunnel() {
            Ok(t) => {
                let _ = take_note(self.key(), |n| &mut n.relay);
                format!("https://127.0.0.1:{}/{path}", t.admin_addr().port())
            }
            Err(e) => {
                log::warn!("relais : {}", e.text(Lang::Fr, None));
                let key = self.key();
                CALL_NOTES.with(|n| n.borrow_mut().relay = Some((key, e)));
                // Un port fermé de la boucle locale : l'appel échoue tout de
                // suite, et rend la raison gardée (`read`).
                format!("https://127.0.0.1:1/{path}")
            }
        }
    }

    /// Adresse de l'ordinateur, sans crochets : Kyber écoute à la même.
    pub fn host(&self) -> &str {
        &self.host
    }

    /// Le port d'Admin sur le réseau de l'ordinateur (0 s'il est inconnu).
    pub fn port(&self) -> u16 {
        self.base.rsplit(':').next().and_then(|p| p.parse().ok()).unwrap_or(0)
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

    pub(crate) fn request(&self, method: &str, path: &str, timeout: Duration) -> ureq::Request {
        self.request_with(method, path, timeout, false)
    }

    /// `single_use` : pour un compte KyberNow, un jeton d'accès à part, que
    /// Admin consomme (redémarrer, mettre à jour la Station).
    fn request_with(&self, method: &str, path: &str, timeout: Duration, single_use: bool) -> ureq::Request {
        let authorization = match &self.authorization {
            Auth::None => None,
            Auth::Fixed(a) => Some(a.clone()),
            Auth::Account(access) => {
                let token = if single_use { access.single_use() } else { access.token() };
                match token {
                    Ok(t) => Some(format!("KyberNow {t}")),
                    Err(e) => {
                        let key = self.key();
                        CALL_NOTES.with(|n| n.borrow_mut().account = Some((key, e)));
                        // Pas de jeton : l'appel échoue tout de suite (port fermé de
                        // la boucle locale) et rend la raison gardée (`read`).
                        let r = self.agent.request(method, &format!("https://127.0.0.1:1/{path}")).timeout(timeout);
                        return r;
                    }
                }
            }
        };
        let r = self
            .agent
            .request(method, &self.url(path))
            .timeout(timeout)
            .set("x-kyber-admin", "1")
            // La console répond dans la langue de l'app (messages d'erreur, refus).
            .set("Accept-Language", i18n::current().code());
        match authorization {
            Some(a) => r.set("Authorization", &a),
            None => r,
        }
    }

    /// Un refus d'Admin : pour un compte KyberNow, un jeton refusé
    /// (`401 token_expired` ou `bad_token`) fait demander un jeton neuf.
    fn refused(&self, status: u16, body: Option<serde_json::Value>) -> ConsoleError {
        if status == 401
            && let Auth::Account(access) = &self.authorization
            && matches!(body.as_ref().and_then(|v| v.get("code")).and_then(|c| c.as_str()), Some("token_expired" | "bad_token"))
        {
            log::info!("comptes : jeton d'accès refusé par Admin, un neuf est demandé");
            access.invalidate();
            let key = self.key();
            CALL_NOTES.with(|n| n.borrow_mut().token_refused = Some(key));
        }
        http_error(status, body)
    }

    /// Fait l'appel ; refait une fois si Admin a refusé le jeton d'accès
    /// (échu : l'appareil a dormi, son horloge a sauté). Rien n'a été fait
    /// sur la Station : le jeton est vérifié avant tout.
    fn with_fresh_token<T>(&self, call: impl Fn() -> Result<T>) -> Result<T> {
        self.retry_on_refused_token(call)
    }

    /// Fait l'appel ; le refait une fois si Admin vient de refuser le jeton
    /// d'accès, pendant cet appel et sur ce fil.
    fn retry_on_refused_token<T, E>(&self, call: impl Fn() -> std::result::Result<T, E>) -> std::result::Result<T, E> {
        self.token_refused_take();
        let r = call();
        if r.is_err() && self.token_refused_take() {
            return call();
        }
        r
    }

    fn token_refused_take(&self) -> bool {
        let key = self.key();
        CALL_NOTES.with(|n| {
            let mut n = n.borrow_mut();
            if n.token_refused == Some(key) {
                n.token_refused = None;
                true
            } else {
                false
            }
        })
    }

    /// Identité de cette console pour les notes d'appel.
    fn key(&self) -> usize {
        self as *const Self as usize
    }

    pub(crate) fn read<T: for<'de> Deserialize<'de>>(&self, r: std::result::Result<ureq::Response, ureq::Error>) -> Result<T> {
        match r {
            Ok(resp) => resp.into_json().map_err(|e| ConsoleError::Invalid(e.to_string())),
            Err(e) => Err(self.failure(e)),
        }
    }

    /// Un appel qui n'a pas abouti, en [`ConsoleError`] : refus d'Admin
    /// (jeton d'accès refusé oublié), ou pas de réponse (compte, relais,
    /// certificat changé, réseau).
    pub(crate) fn failure(&self, e: ureq::Error) -> ConsoleError {
        match e {
            ureq::Error::Status(status, resp) => self.refused(status, resp.into_json::<serde_json::Value>().ok()),
            ureq::Error::Transport(t) => {
                let text = t.to_string();
                if let Some(e) = take_note(self.key(), |n| &mut n.account) {
                    return ConsoleError::Account(e);
                }
                if let Some(e) = take_note(self.key(), |n| &mut n.relay) {
                    return ConsoleError::Relay(e);
                }
                if text.contains("certificat de la console") {
                    let seen = self.fingerprint_seen().unwrap_or_default();
                    ConsoleError::CertificateChanged { seen }
                } else {
                    ConsoleError::Network(text)
                }
            }
        }
    }

    /// Une réponse d'Admin dont chaque module lit lui-même les refus
    /// (COEUR-38) : reçue (2xx), ou refusée, lue une fois ([`Reply`] :
    /// statut, corps JSON, `Retry-After`) ; sans réponse : [`Self::failure`].
    pub(crate) fn answer(&self, r: std::result::Result<ureq::Response, ureq::Error>) -> Result<Answer> {
        match r {
            Ok(resp) => Ok(Answer::Ok(resp)),
            Err(ureq::Error::Status(_, resp)) => Ok(Answer::Refused(Reply::read(resp))),
            Err(e) => Err(self.failure(e)),
        }
    }

    /// Une réponse sans contenu utile (corps vide, `{}`, `{"ok":true}`).
    pub(crate) fn read_unit(&self, r: std::result::Result<ureq::Response, ureq::Error>) -> Result<()> {
        r.map(|_| ()).map_err(|e| self.failure(e))
    }

    /// `GET /api/stream/ping` (docs/profil-auto.md) : un aller-retour vers Admin, en
    /// millisecondes. `Ok(None)` : Station d'avant (404).
    pub fn ping(&self) -> Result<Option<f64>> {
        let started = std::time::Instant::now();
        let r = self.request("GET", "api/stream/ping", Duration::from_secs(2)).call();
        match r {
            Ok(_) => Ok(Some(started.elapsed().as_secs_f64() * 1000.0)),
            Err(ureq::Error::Status(404, _)) => Ok(None),
            Err(e) => Err(self.failure(e)),
        }
    }

    /// `GET /api/stream/probe?ms=…` (docs/profil-auto.md) : la rafale de la
    /// Station, à lire aussi vite que possible. `Ok(None)` : Station d'avant
    /// (404).
    pub fn burst(&self, ms: u32) -> Result<Option<Box<dyn std::io::Read + Send + Sync>>> {
        let timeout = Duration::from_millis(u64::from(ms) + 5000);
        let r = self.with_fresh_token(|| match self.request("GET", &format!("api/stream/probe?ms={ms}"), timeout).call() {
            Ok(resp) => Ok(Some(resp)),
            Err(ureq::Error::Status(404, _)) => Ok(None),
            Err(e) => Err(self.failure(e)),
        })?;
        Ok(r.map(|resp| resp.into_reader()))
    }

    /// `GET /api/stream/encoder?…` (docs/profil-auto.md, « L'encodeur de
    /// l'ordinateur ») : l'encodeur de la Station accepte-t-il ce format à ce
    /// débit ? `Ok(None)` : Station d'avant (404).
    pub fn encoder_check(&self, q: &EncoderQuery) -> Result<Option<EncoderAnswer>> {
        let path = format!(
            "api/stream/encoder?codec={}&width={}&height={}&fps={}&mbps={}&yuv444={}&hdr={}",
            q.codec.key(),
            q.width,
            q.height,
            q.fps,
            q.mbps,
            u8::from(q.yuv444),
            u8::from(q.hdr)
        );
        self.with_fresh_token(|| match self.request("GET", &path, Duration::from_secs(15)).call() {
            Ok(resp) => {
                let body: EncoderReply = resp.into_json().map_err(|e| ConsoleError::Invalid(e.to_string()))?;
                Ok(Some(match body.ok {
                    Some(true) => EncoderAnswer::Accepted,
                    Some(false) => EncoderAnswer::Refused(body.error.unwrap_or_default()),
                    None => EncoderAnswer::Unknown(body.error.unwrap_or_default()),
                }))
            }
            Err(ureq::Error::Status(404, _)) => Ok(None),
            Err(ureq::Error::Status(429, _)) => Ok(Some(EncoderAnswer::Unknown("busy".into()))),
            Err(e) => Err(self.failure(e)),
        })
    }

    pub fn info(&self) -> Result<Info> {
        self.info_within(Duration::from_secs(4))
    }

    /// [`Self::info`] avec un autre délai (`Route::Auto` : 1,5 s en direct).
    pub fn info_within(&self, timeout: Duration) -> Result<Info> {
        self.with_fresh_token(|| {
            let r = self.request("GET", "api/stream/info", timeout).call();
            self.read(r)
        })
    }

    pub fn prepare(&self, req: &PrepareRequest) -> Result<Prepared> {
        let w: PreparedWire = self.with_fresh_token(|| {
            let r = self.request("POST", "api/stream/prepare", Duration::from_secs(40)).send_json(req);
            self.read(r)
        })?;
        Ok(w.prepared())
    }

    /// `POST /api/stream/hdr` (docs/hdr.md) : le HDR voulu pour chaque écran
    /// de la session de cet appareil, dans l'ordre de `screens` de `prepare`
    /// (un seul pour un écran), avec les valeurs HDR de chaque écran
    /// (`None` : `null`, écran en SDR) ; rend le HDR obtenu. `409` :
    /// l'appareil n'est pas en session ; `404` : Station d'avant le HDR.
    pub fn stream_hdr(&self, device_id: &str, hdr: &[bool], luminance: &[Option<crate::profile::HdrLuminance>]) -> Result<Vec<bool>> {
        #[derive(Serialize)]
        struct Req<'a> {
            device_id: &'a str,
            hdr: &'a [bool],
            luminance: &'a [Option<crate::profile::HdrLuminance>],
        }
        #[derive(Deserialize)]
        struct Resp {
            #[serde(default)]
            hdr: Vec<bool>,
        }
        let w: Resp = self.with_fresh_token(|| {
            let r = self.request("POST", "api/stream/hdr", Duration::from_secs(10)).send_json(Req { device_id, hdr, luminance });
            self.read(r)
        })?;
        Ok(w.hdr)
    }

    pub fn heartbeat(&self, hb: &Heartbeat) -> Result<HeartbeatReply> {
        self.with_fresh_token(|| {
            let r = self.request("POST", "api/stream/heartbeat", Duration::from_secs(20)).send_json(hb);
            self.read(r)
        })
    }

    /// `GET /api/consoles` : les consoles PlayStation que voit cette Station
    /// sur son réseau (docs/relais.md, « Consoles PlayStation »), jamais leur
    /// adresse. `404` : Station d'avant les consoles.
    pub fn consoles(&self) -> Result<Vec<StationConsole>> {
        self.with_fresh_token(|| {
            let r = self.request("GET", "api/consoles", Duration::from_secs(6)).call();
            self.read::<ConsoleList>(r).map(|l| l.consoles)
        })
    }

    /// `POST /api/consoles/beat` : par le relais, ce client joint les consoles
    /// de la Station 30 s de plus (état, réveil, partie) ; rend la liste de
    /// [`Self::consoles`].
    pub fn consoles_beat(&self) -> Result<Vec<StationConsole>> {
        let r = self.request("POST", "api/consoles/beat", Duration::from_secs(6)).send_json(serde_json::json!({}));
        self.read::<ConsoleList>(r).map(|l| l.consoles)
    }

    pub fn foreground(&self) -> Result<Foreground> {
        self.with_fresh_token(|| {
            let r = self.request("GET", "api/stream/foreground", Duration::from_secs(2)).call();
            self.read(r)
        })
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
            Err(e) => Err(self.failure(e)),
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

    /// Demande à la Station de vérifier ses mises à jour tout de suite
    /// (`POST /api/stream/station/check`, docs/link-ui.md « Mise à jour des
    /// Stations ») et rend ce qu'elle annonce ensuite. `Ok(None)` : une
    /// Station antérieure à la 0.11.3, qui ne connaît pas la route (404, ou la
    /// page d'Admin) ; on se fie alors à son annonce.
    pub fn station_check(&self) -> Result<Option<StationInfo>> {
        #[derive(Deserialize)]
        struct Checked {
            #[serde(default)]
            ok: bool,
            station: Option<StationInfo>,
        }
        let r = self.with_fresh_token(|| {
            match self.request("POST", "api/stream/station/check", Duration::from_secs(20)).send_json(serde_json::json!({})) {
                Ok(resp) => Ok(resp.into_json::<Checked>().ok()),
                Err(ureq::Error::Status(404 | 405, _)) => Ok(None),
                Err(e) => Err(self.failure(e)),
            }
        })?;
        Ok(r.filter(|c| c.ok).and_then(|c| c.station))
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
        // Un compte KyberNow : un jeton à part, qu'Admin consomme.
        let r = self.request_with("POST", path, timeout, true).send_json(body);
        match self.answer(r).map_err(StationError::Other)? {
            // Une Station 0.4.9 répond la page d'Admin (200, HTML) à une route
            // qu'elle ne connaît pas.
            Answer::Ok(resp) => match resp.into_json::<serde_json::Value>() {
                Ok(v) if v.get("ok").and_then(|o| o.as_bool()) == Some(true) => Ok(v),
                _ => Err(StationError::Unsupported),
            },
            Answer::Refused(r) => Err(station_refusal(r.status, &r.body, r.retry_after)
                .unwrap_or_else(|| StationError::Other(http_error(r.status, Some(r.body))))),
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

    /// Lance (`record`) ou arrête l'enregistrement de la session de cet
    /// appareil (docs/enregistrement.md) : rend l'état, ou le refus d'Admin
    /// avec son texte de link-ui. Une Station qui n'enregistre pas : 404,
    /// [`RecordingError::Failed`](crate::recording::RecordingError::Failed).
    pub fn recording(&self, device_id: &str, record: bool) -> std::result::Result<crate::recording::RecordingState, crate::recording::RecordingError> {
        use crate::recording::RecordingError;
        let r = self
            .request("POST", "api/stream/recording", Duration::from_secs(15))
            .send_json(serde_json::json!({ "record": record, "device_id": device_id }));
        match r {
            Ok(resp) => {
                let v: serde_json::Value =
                    resp.into_json().map_err(|e| RecordingError::Console(ConsoleError::Invalid(e.to_string()).text(i18n::current())))?;
                crate::recording::from_value(v).ok_or(RecordingError::Failed)
            }
            Err(ureq::Error::Status(409, resp)) => {
                let body = Reply::read(resp).body;
                Err(RecordingError::from_code(body["code"].as_str().unwrap_or_default(), body["error"].as_str().unwrap_or_default()))
            }
            Err(ureq::Error::Status(404, _)) => Err(RecordingError::Failed),
            Err(e) => Err(RecordingError::Console(self.failure(e).text(i18n::current()))),
        }
    }

    /// « Nouveau code » : les invités déjà là restent.
    pub fn share_code(&self) -> Result<ShareOpened> {
        let r = self.request("POST", "api/share/code", Duration::from_secs(6)).send_json(serde_json::json!({}));
        self.read(r)
    }

    /// Le rôle d'un invité : « Donner les manettes », « Reprendre les
    /// manettes », « Donner le clavier et la souris », « Reprendre le clavier
    /// et la souris », chacun sans toucher à l'autre droit
    /// ([`ShareRole::with_pads`], [`ShareRole::with_kbm`]).
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

    /// « Accepter » la demande d'un invité venu par le relais (docs/relais.md) :
    /// il devient spectateur, et son attente (`share_wait`) aboutit. Rend le
    /// partage à jour. Admin refuse (409 `full`) si les quatre places sont
    /// prises, et (404) si la demande n'existe plus.
    pub fn share_accept(&self, guest: &str) -> Result<ShareState> {
        let path = format!("api/share/guest/{}/accept", path_segment(guest));
        let r = self.request("POST", &path, Duration::from_secs(6)).send_json(serde_json::json!({}));
        self.read(r)
    }

    /// « Refuser » la demande d'un invité venu par le relais : son appareil ne
    /// redemande pas avant 10 minutes. Rend le partage à jour.
    pub fn share_refuse(&self, guest: &str) -> Result<ShareState> {
        let path = format!("api/share/guest/{}/refuse", path_segment(guest));
        let r = self.request("POST", &path, Duration::from_secs(6)).send_json(serde_json::json!({}));
        self.read(r)
    }

    /// « Laisser les invités parler » (docs/micro.md) : leur micro arrive sur
    /// l'ordinateur, tout de suite pour les invités déjà là. Rend le partage
    /// à jour (`guests_speak`).
    pub fn share_speak(&self, on: bool) -> Result<ShareState> {
        let r = self.request("POST", "api/share/speak", Duration::from_secs(6)).send_json(serde_json::json!({ "on": on }));
        self.read(r)
    }

    /// « Arrêter le partage » : tout le monde sort.
    pub fn share_close(&self) -> Result<()> {
        let r = self.request("DELETE", "api/share", Duration::from_secs(6)).call();
        self.read_unit(r)
    }

    // KyberNow Share, côté invité ([`Self::guest`], sans compte).

    /// `POST /api/share/join` : le code, le prénom, l'appareil. Voir
    /// [`crate::share::join`] pour l'envoyer à toutes les Stations du réseau.
    /// Une demande par le relais mise en attente (202) se lit comme une
    /// erreur ici : voir [`Self::share_join_answer`].
    pub fn share_join(&self, req: &JoinRequest) -> std::result::Result<ShareJoined, ShareError> {
        match self.share_join_answer(req)? {
            JoinAnswer::Joined(j) => Ok(j),
            JoinAnswer::Pending { .. } => Err(ShareError::Other(ConsoleError::Invalid("demande en attente de l'hôte".into()))),
        }
    }

    /// `POST /api/share/join`, avec la réponse d'une demande par le relais :
    /// l'invité entre, ou attend l'accord de l'hôte (`202 {"pending":true,
    /// "guest","secret","expires"}`).
    pub fn share_join_answer(&self, req: &JoinRequest) -> std::result::Result<JoinAnswer, ShareError> {
        let r = self.request("POST", "api/share/join", Duration::from_secs(6)).send_json(req);
        match self.answer(r).map_err(ShareError::Other)? {
            // Une Station antérieure peut répondre la page d'Admin (200, HTML) :
            // ce n'est pas elle qui partage.
            Answer::Ok(resp) => {
                let status = resp.status();
                let v: serde_json::Value = resp.into_json().map_err(|_| ShareError::NoShare)?;
                join_answer(status, v)
            }
            Answer::Refused(r) => Err(share_refusal(r.status, &r.body, r.retry_after)),
        }
    }

    /// `POST /api/share/wait` (par le relais) : réponse tenue 25 s au plus ;
    /// `Pending` : toujours en attente, rappeler ; `Joined` : l'hôte a
    /// accepté. Refus : [`ShareError::Refused`], [`ShareError::Expired`],
    /// [`ShareError::NoShare`] (partage arrêté).
    pub fn share_wait(&self, guest: &str, secret: &str) -> std::result::Result<JoinAnswer, ShareError> {
        let r = self
            .request("POST", "api/share/wait", Duration::from_secs(35))
            .set("x-kybernow-guest", secret)
            .send_json(serde_json::json!({ "guest": guest }));
        match self.answer(r).map_err(ShareError::Other)? {
            Answer::Ok(resp) => {
                let status = resp.status();
                let v: serde_json::Value = resp.into_json().map_err(|e| ShareError::Other(ConsoleError::Invalid(e.to_string())))?;
                match join_answer(status, v)? {
                    // `wait` ne redonne ni l'invité ni le secret : ceux de la demande.
                    JoinAnswer::Pending { .. } => Ok(JoinAnswer::Pending { guest: guest.into(), secret: secret.into(), expires: 0 }),
                    joined => Ok(joined),
                }
            }
            // `Retry-After` lu aussi ici (un 429 sur l'attente gardait le délai du corps seulement).
            Answer::Refused(r) => Err(share_refusal(r.status, &r.body, r.retry_after)),
        }
    }

    /// Signe de vie de l'invité (toutes les `beat` secondes) : rôle, jeton
    /// de Kyber renouvelé, fin du partage.
    pub fn share_beat(&self, guest: &str, secret: &str) -> Result<ShareBeat> {
        self.share_beat_within(guest, secret, Duration::from_secs(10))
    }

    /// [`Self::share_beat`] avec un autre délai (la session d'un invité qui
    /// finit demande vite si c'est le partage qui l'a finie).
    pub fn share_beat_within(&self, guest: &str, secret: &str, timeout: Duration) -> Result<ShareBeat> {
        let r = self
            .request("POST", "api/share/beat", timeout)
            .set("x-kybernow-guest", secret)
            .send_json(serde_json::json!({ "guest": guest }));
        self.read(r)
    }

    /// La « Qualité de l'image » de l'invité changée en cours de session :
    /// Kyber relance l'image de l'invité seulement (rien à relancer ici) ;
    /// rend l'image qu'il reçoit désormais. Avec « Auto », `auto` : ce que
    /// l'appareil sait afficher et décoder (celui de `join` ; sans lui, Admin
    /// lit « Auto » comme l'image de l'hôte).
    /// Admin refuse plus d'un changement par seconde : `429`, que
    /// [`crate::session::Session::set_share_quality`] réessaie une fois
    /// après le délai (voir [`Self::share_quality_or_wait`]).
    pub fn share_quality(&self, guest: &str, secret: &str, quality: ShareQuality, auto: Option<&ShareAuto>) -> Result<ShareImage> {
        self.share_quality_or_wait(guest, secret, quality, auto).map_err(|e| match e {
            QualityRefusal::TooSoon(wait) => ConsoleError::Http { status: 429, message: tr(i18n::current(), "console.http_status", &[("status", &format!("429, {} s", wait.as_secs()))]) },
            QualityRefusal::Failed(e) => e,
        })
    }

    /// [`Self::share_quality`], avec le délai d'un refus `429` (`retry_after`
    /// du corps, sinon `Retry-After`, 1 s sinon ; entre 1 et 5 s).
    pub fn share_quality_or_wait(&self, guest: &str, secret: &str, quality: ShareQuality, auto: Option<&ShareAuto>) -> std::result::Result<ShareImage, QualityRefusal> {
        let r = self
            .request("POST", "api/share/quality", Duration::from_secs(10))
            .set("x-kybernow-guest", secret)
            .send_json(crate::share::quality_body(guest, quality, auto));
        match r {
            Err(ureq::Error::Status(429, resp)) => {
                let r = Reply::read(resp);
                Err(QualityRefusal::TooSoon(quality_retry_delay(r.body.get("retry_after").and_then(|s| s.as_u64()), r.retry_after)))
            }
            r => self.read(r).map_err(QualityRefusal::Failed),
        }
    }

    // Regarder (droit « Regarder » d'un compte KyberNow, docs/comptes.md).

    /// `POST /api/share/watch` avec le jeton d'accès du compte
    /// ([`Self::account`]) : l'appareil entre en spectateur dans le partage
    /// de l'ordinateur (ouvert pour lui s'il n'y en a pas). Voir
    /// [`crate::share::watch`]. Un jeton échu est redemandé une fois.
    pub fn share_watch(&self, req: &WatchRequest) -> std::result::Result<ShareJoined, ShareError> {
        self.retry_on_refused_token(|| self.share_watch_once(req))
    }

    fn share_watch_once(&self, req: &WatchRequest) -> std::result::Result<ShareJoined, ShareError> {
        match self.request("POST", "api/share/watch", Duration::from_secs(10)).send_json(req) {
            // Une Station d'avant 0.4.9 répond la page d'Admin (200, HTML) à
            // une route qu'elle ne connaît pas.
            Ok(resp) => {
                let v: serde_json::Value = resp.into_json().map_err(|_| ShareError::WatchUnsupported)?;
                let j: ShareJoined = serde_json::from_value(v).map_err(|e| ShareError::Other(ConsoleError::Invalid(e.to_string())))?;
                if j.guest.is_empty() || j.secret.is_empty() { Err(ShareError::WatchUnsupported) } else { Ok(j) }
            }
            Err(ureq::Error::Status(status, resp)) => {
                let body = resp.into_json::<serde_json::Value>().ok();
                Err(watch_refusal(status, body.as_ref()).unwrap_or_else(|| ShareError::Other(self.refused(status, body))))
            }
            Err(e) => Err(ShareError::Other(self.failure(e))),
        }
    }

    /// « Écran » de qui regarde (`POST /api/share/watch/display`, secret de
    /// l'invité) : rend la réponse d'un signe de vie, avec le nouvel écran et
    /// un jeton de Kyber qui le porte. Refus : 409 (`fixed` : l'écran de ce
    /// partage ne se change pas), 404 (`unknown_display`, `unknown_guest`),
    /// 429 (un changement par seconde).
    pub fn share_watch_display(&self, guest: &str, secret: &str, display: u32) -> Result<ShareBeat> {
        let r = self
            .request("POST", "api/share/watch/display", Duration::from_secs(10))
            .set("x-kybernow-guest", secret)
            .send_json(serde_json::json!({ "guest": guest, "display": display }));
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

/// La réponse d'Admin à `join` (et à `wait`, par le relais).
#[derive(Debug, Clone)]
pub enum JoinAnswer {
    /// L'invité entre (la réponse de `join` sur le réseau).
    Joined(ShareJoined),
    /// Par le relais : en attente de l'accord de l'hôte (`expires` : 120 s).
    Pending { guest: String, secret: String, expires: u64 },
}

/// Une réponse 2xx de `join` ou `wait`.
fn join_answer(status: u16, v: serde_json::Value) -> std::result::Result<JoinAnswer, ShareError> {
    if status == 202 || v.get("pending").and_then(|p| p.as_bool()) == Some(true) {
        let field = |k: &str| v.get(k).and_then(|x| x.as_str()).unwrap_or_default().to_string();
        return Ok(JoinAnswer::Pending { guest: field("guest"), secret: field("secret"), expires: v.get("expires").and_then(|e| e.as_u64()).unwrap_or(120) });
    }
    let j: ShareJoined = serde_json::from_value(v).map_err(|_| ShareError::NoShare)?;
    if j.guest.is_empty() || j.secret.is_empty() { Err(ShareError::NoShare) } else { Ok(JoinAnswer::Joined(j)) }
}

/// Un refus de `join` ou `wait`.
fn share_refusal(status: u16, body: &serde_json::Value, retry_after: Option<u64>) -> ShareError {
    let code = body.get("error").and_then(|e| e.as_str()).unwrap_or_default();
    match (status, code) {
        // L'hôte arrête le partage pendant qu'un invité attend son accord.
        (_, "stopped") => ShareError::Stopped,
        // Un code de prêt (docs/pret.md) : la Station et le chemin viennent de `share::join`.
        (404, _) if body.get("lend").and_then(|l| l.as_bool()) == Some(true) => ShareError::Lend(Box::default()),
        // Un code d'invitation (docs/invitations.md) : l'app demande le mot de passe.
        (404, _) if body.get("invite").and_then(|l| l.as_bool()) == Some(true) => {
            let computer = body.get("computer").and_then(|c| c.as_str()).unwrap_or_default().to_string();
            ShareError::Invite(Box::new(crate::invite::InviteCode { computer, ..Default::default() }))
        }
        (404 | 405, _) => ShareError::NoShare,
        (409, "full") => ShareError::Full,
        (409, "pending_full") => ShareError::PendingFull,
        (403, "refused") => ShareError::Refused,
        (410, _) => ShareError::Expired,
        (429, _) => ShareError::TooManyTries { seconds: body.get("retry_after").and_then(|s| s.as_u64()).or(retry_after) },
        _ => {
            let message = if code.is_empty() { tr(i18n::current(), "console.http_status", &[("status", &status.to_string())]) } else { code.to_string() };
            ShareError::Other(ConsoleError::Http { status, message })
        }
    }
}

/// Un refus de `POST /api/share/watch` que le cœur dit avec ses mots
/// (docs/link-ui.md, « Regarder, fin et refus ») ; `None` : le texte d'Admin.
/// Une route inconnue (404 sans `code` : Station d'avant « Regarder ») :
/// [`ShareError::WatchUnsupported`].
fn watch_refusal(status: u16, body: Option<&serde_json::Value>) -> Option<ShareError> {
    let code = body.and_then(|v| v.get("code")).and_then(|c| c.as_str());
    match (status, code) {
        (404 | 405, None) => Some(ShareError::WatchUnsupported),
        (409, Some("lent")) => {
            let name = body.and_then(|v| v.get("computer")).and_then(|c| c.as_str()).unwrap_or_default().to_string();
            Some(ShareError::Lent { name })
        }
        (409, Some("disabled")) => Some(ShareError::SharingOff),
        (409, Some("full")) => Some(ShareError::Full),
        _ => None,
    }
}

#[cfg(test)]
pub(crate) fn share_refusal_for_tests(status: u16, body: &serde_json::Value) -> ShareError {
    share_refusal(status, body, None)
}

/// Refus d'un changement de qualité d'image d'un invité.
#[derive(Debug)]
pub enum QualityRefusal {
    /// Plus d'un changement par seconde (429) : réessayer après ce délai.
    TooSoon(Duration),
    Failed(ConsoleError),
}

/// Délai avant de réessayer un changement de qualité refusé : `retry_after`
/// du corps, sinon l'en-tête `Retry-After`, sinon 1 s ; entre 1 et 5 s.
pub(crate) fn quality_retry_delay(body: Option<u64>, header: Option<u64>) -> Duration {
    Duration::from_secs(body.or(header).unwrap_or(1).clamp(1, 5))
}

/// Un identifiant d'invité dans un chemin : seulement ce qui ne demande pas
/// d'encodage (les identifiants de la Station sont alphanumériques).
pub(crate) fn path_segment(id: &str) -> String {
    id.chars().filter(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.')).collect()
}

#[cfg(test)]
mod tests {
    /// COEUR-21 : la raison rangée par un fil (tunnel fermé) n'est reprise
    /// que par l'appel de ce fil, sur cette console.
    #[test]
    fn raison_d_un_appel_gardee_pour_son_fil() {
        use super::*;
        let closed = std::net::TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port();
        let c = Arc::new(Console::new("127.0.0.1", closed, "u", "p", None));
        let other = c.clone();
        std::thread::spawn(move || {
            let key = other.key();
            CALL_NOTES.with(|n| n.borrow_mut().relay = Some((key, crate::relay::RelayError::StationOffline)));
        })
        .join()
        .unwrap();
        let call = || c.read::<serde_json::Value>(c.request("GET", "api/stream/info", Duration::from_secs(2)).call());
        assert!(matches!(call(), Err(ConsoleError::Network(_))), "pas la raison d'un autre fil");
        let key = c.key();
        CALL_NOTES.with(|n| n.borrow_mut().relay = Some((key, crate::relay::RelayError::StationOffline)));
        let other_console = Console::new("127.0.0.1", closed, "u", "p", None);
        assert!(matches!(other_console.read::<serde_json::Value>(other_console.request("GET", "x", Duration::from_secs(2)).call()), Err(ConsoleError::Network(_))), "ni d'une autre console");
        assert!(matches!(call(), Err(ConsoleError::Relay(crate::relay::RelayError::StationOffline))));
        assert!(matches!(call(), Err(ConsoleError::Network(_))), "reprise une fois");
    }

    /// COEUR-19 : sans empreinte connue, le premier certificat vu est exigé
    /// des connexions suivantes ; avec une empreinte, elle seule passe.
    #[test]
    fn premier_certificat_exige_ensuite() {
        use super::*;
        let check = |v: &PinVerifier, der: &[u8]| {
            v.verify_server_cert(&CertificateDer::from(der.to_vec()), &[], &ServerName::try_from("station").unwrap(), &[], UnixTime::now()).is_ok()
        };
        let provider = Arc::new(rustls::crypto::ring::default_provider());
        let seen = Arc::new(Mutex::new(None));
        let v = PinVerifier { pinned: None, first: Mutex::new(None), seen: seen.clone(), provider: provider.clone() };
        assert!(check(&v, b"premier"));
        assert!(check(&v, b"premier"), "le même : accepté");
        assert!(!check(&v, b"autre"), "un autre ensuite : refusé");
        assert_eq!(seen.lock().unwrap().clone(), Some(fingerprint(b"autre")), "l'empreinte refusée est dite");
        let v = PinVerifier { pinned: Some(fingerprint(b"connu").to_uppercase()), first: Mutex::new(None), seen, provider };
        assert!(!check(&v, b"premier"));
        assert!(check(&v, b"connu"));
    }

    #[test]
    fn manettes_de_la_station() {
        let info = |json: &str| serde_json::from_str::<super::Info>(json).unwrap();
        assert!(info("{}").gamepads_available());
        assert!(info(r#"{"gamepads": null}"#).gamepads_available());
        assert!(info(r#"{"gamepads": {"available": true, "reason": null}}"#).gamepads_available());
        let mac = info(r#"{"os": "macOS 26.1", "gamepads": {"available": false, "reason": "entitlement"}}"#);
        assert!(!mac.gamepads_available());
        assert_eq!(mac.gamepads.unwrap().reason.as_deref(), Some("entitlement"));
    }

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
            hdr: false,
            luminance: None,
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
            ScreenMode { width: 3840, height: 2160, refresh: 160, x: 0, y: 0, primary: true, hdr: false, luminance: None },
            ScreenMode { width: 5120, height: 2880, refresh: 60, x: 3840, y: 0, primary: false, hdr: false, luminance: None },
            ScreenMode { width: 3456, height: 2234, refresh: 60, x: -3456, y: 400, primary: false, hdr: false, luminance: None },
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
    fn hdr_annonce_demande_et_obtenu() {
        // Annonce de la Station.
        let info: Info = serde_json::from_str(r#"{"hdr": {"available": false, "reason": "windows"}}"#).unwrap();
        assert_eq!(info.hdr, Some(HdrInfo { available: false, reason: Some("windows".into()) }));
        let old: Info = serde_json::from_str("{}").unwrap();
        assert_eq!(old.hdr, None, "Station d'avant le HDR");
        // Demande : `hdr` seulement s'il est demandé, et par écran.
        let mut req = request(Some(vec![
            ScreenMode { width: 3840, height: 2160, refresh: 120, x: 0, y: 0, primary: true, hdr: true, luminance: None },
            ScreenMode { width: 2560, height: 1440, refresh: 60, x: 3840, y: 0, primary: false, hdr: false, luminance: None },
        ]));
        req.hdr = true;
        let v = serde_json::to_value(&req).unwrap();
        assert_eq!(v["hdr"], true);
        assert_eq!(v["screens"][0]["hdr"], true);
        assert!(v["screens"][1].get("hdr").is_none());
        assert!(serde_json::to_value(request(None)).unwrap().get("hdr").is_none(), "SDR : rien de neuf");
        // Réponse : le HDR obtenu par écran ; absent : SDR partout.
        let p = serde_json::from_str::<PreparedWire>(r#"{"virtual_display": true, "hdr": [true, false]}"#).unwrap().prepared();
        assert_eq!(p.hdr, vec![true, false]);
        let p = serde_json::from_str::<PreparedWire>(r#"{"virtual_display": true}"#).unwrap().prepared();
        assert!(p.hdr.is_empty());
    }

    #[test]
    fn adresse_preferee_pour_l_image() {
        let p = serde_json::from_str::<PreparedWire>(r#"{"kyber_port": 8080, "kyber_host": "192.168.1.105"}"#).unwrap().prepared();
        assert_eq!(p.kyber_host.as_deref(), Some("192.168.1.105"));
        // Absente (Station d'avant) ou vide : l'adresse d'Admin.
        let p = serde_json::from_str::<PreparedWire>(r#"{"kyber_port": 8080}"#).unwrap().prepared();
        assert_eq!(p.kyber_host, None);
        let p = serde_json::from_str::<PreparedWire>(r#"{"kyber_host": " "}"#).unwrap().prepared();
        assert_eq!(p.kyber_host, None);
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
    fn signe_de_vie_avec_le_partage() {
        let r: HeartbeatReply = serde_json::from_str(
            r#"{"active":true,"end":false,"share":{"open":true,"code":"123456","guests":[{"id":"g1","name":"Léa","app":"ios","role":"player"}]}}"#,
        )
        .unwrap();
        let share = r.share.expect("partage");
        assert_eq!((share.code.as_str(), share.guests[0].role), ("123456", ShareRole::Player));
        // Sans partage, ou illisible : ignoré, la fin demandée est toujours lue.
        for odd in [r#"{"end":true}"#, r#"{"end":true,"share":null}"#, r#"{"end":true,"share":"?"}"#, r#"{"end":true,"share":{"guests":7,"open":"oui"}}"#] {
            let r: HeartbeatReply = serde_json::from_str(odd).unwrap();
            assert!(r.end && r.share.is_none(), "{odd}");
        }
    }

    #[test]
    fn reponses_de_join_et_wait() {
        let pending = join_answer(202, serde_json::json!({ "pending": true, "guest": "g1", "secret": "s", "expires": 120 })).unwrap();
        assert!(matches!(pending, JoinAnswer::Pending { ref guest, ref secret, expires: 120 } if guest == "g1" && secret == "s"));
        let joined = join_answer(200, serde_json::json!({ "guest": "g1", "secret": "s", "role": "spectator", "relay": true })).unwrap();
        assert!(matches!(joined, JoinAnswer::Joined(ref j) if j.guest == "g1"));
        assert!(matches!(join_answer(200, serde_json::json!({ "guest": "" })), Err(ShareError::NoShare)), "page d'une Station antérieure");
        assert!(matches!(share_refusal(403, &serde_json::json!({ "error": "refused" }), None), ShareError::Refused));
        assert!(matches!(share_refusal(410, &serde_json::json!({ "error": "expired" }), None), ShareError::Expired));
        assert!(matches!(share_refusal(409, &serde_json::json!({ "error": "pending_full" }), None), ShareError::PendingFull));
        assert!(matches!(share_refusal(409, &serde_json::json!({ "error": "full" }), None), ShareError::Full));
        assert!(matches!(share_refusal(404, &serde_json::json!({ "error": "no_share" }), None), ShareError::NoShare));
        assert!(matches!(share_refusal(429, &serde_json::Value::Null, Some(4)), ShareError::TooManyTries { seconds: Some(4) }));
        assert!(matches!(share_refusal(500, &serde_json::Value::Null, None), ShareError::Other(ConsoleError::Http { status: 500, .. })));
        // Partage arrêté pendant l'attente : « Ce partage est arrêté. », pas « Aucun partage avec ce code ».
        assert!(matches!(share_refusal(410, &serde_json::json!({ "error": "stopped" }), None), ShareError::Stopped));
        assert!(matches!(share_refusal(404, &serde_json::json!({ "error": "stopped" }), None), ShareError::Stopped));
        assert!(matches!(share_refusal(410, &serde_json::Value::Null, None), ShareError::Expired));
        assert_eq!(ShareError::Stopped.text(Lang::Fr), "Ce partage est arrêté.");
        assert_eq!(ShareError::Stopped.text(Lang::En), "This share has stopped.");
        assert_eq!(ShareError::Stopped.kind(), "stopped");
    }

    #[test]
    fn acces_coupe_par_le_relais() {
        use crate::relay::RelayError;
        let off = serde_json::json!({ "error": "L'accès hors de chez vous est coupé dans Admin.", "code": "remote_access_off" });
        assert!(matches!(http_error(403, Some(off)), ConsoleError::Relay(RelayError::AccessOff)));
        let other = serde_json::json!({ "error": "Pas par le relais.", "code": "relay_forbidden" });
        assert!(matches!(http_error(403, Some(other)), ConsoleError::Http { status: 403, ref message } if message == "Pas par le relais."));
        assert!(matches!(http_error(401, Some(serde_json::json!({ "error": "compte" }))), ConsoleError::Http { status: 401, ref message } if message == "compte"));
        assert!(matches!(http_error(502, None), ConsoleError::Http { status: 502, .. }));
        assert_eq!(ConsoleError::Relay(RelayError::AccessOff).text(Lang::Fr), "L'accès hors de chez vous est coupé dans Admin.");
    }

    #[test]
    fn delai_d_un_changement_de_qualite() {
        assert_eq!(quality_retry_delay(Some(1), Some(3)), Duration::from_secs(1), "le corps d'abord");
        assert_eq!(quality_retry_delay(None, Some(2)), Duration::from_secs(2), "puis l'en-tête");
        assert_eq!(quality_retry_delay(None, None), Duration::from_secs(1));
        assert_eq!(quality_retry_delay(Some(0), None), Duration::from_secs(1), "jamais tout de suite");
        assert_eq!(quality_retry_delay(Some(60), None), Duration::from_secs(5), "jamais plus de 5 s");
    }

    #[test]
    fn identifiant_d_invite_dans_un_chemin() {
        assert_eq!(path_segment("g1-a_b.c"), "g1-a_b.c");
        assert_eq!(path_segment("../../api/stream/end?x=1"), "....apistreamendx1");
    }

    #[test]
    fn station_trop_ancienne() {
        assert_eq!(
            StationError::Unsupported.text(Lang::Fr),
            "cette version de KyberNow Station ne le permet pas : mettez-la à jour depuis Admin"
        );
        assert_eq!(StationError::Unsupported.text(Lang::En), "this version of KyberNow Station can’t do this: update it from Admin");
    }

    // --- Comptes KyberNow : le jeton d'accès du relais (docs/comptes.md) ---

    use crate::account::{AccessParams, Account, AccountError, RelayAddr, RelayHttp, Reply, StationAccess, RENEW_AFTER};
    use std::sync::atomic::AtomicUsize;

    /// Le faux relais : un jeton neuf à chaque `POST /v1/access`, ou la
    /// session de l'appareil fermée.
    #[derive(Default)]
    struct Tokens {
        n: AtomicUsize,
        closed: bool,
    }

    impl RelayHttp for Tokens {
        fn call(&self, _: &RelayAddr, _: &str, path: &str, bearer: Option<&str>, _: Option<&serde_json::Value>) -> std::result::Result<Reply, String> {
            assert_eq!((path, bearer), ("/v1/access", Some("session")));
            if self.closed {
                return Ok(Reply { status: 401, body: serde_json::json!({ "error": "bad_session" }), retry_after: None });
            }
            let n = self.n.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            let body = serde_json::json!({ "token": format!("jeton-{n}"), "expires": crate::account::now_secs() + 300, "lifetime": 300 });
            Ok(Reply { status: 200, body, retry_after: None })
        }
    }

    fn access(tokens: Tokens) -> Arc<StationAccess> {
        let account = Account { relay: "share.kybernow.app".into(), token: "session".into(), ..Default::default() };
        StationAccess::with_http(AccessParams { account, station: "abcdefghijklmnopqrstuvwxyz".into() }, Arc::new(tokens), RENEW_AFTER)
    }

    /// Un faux Admin en HTTPS : chaque requête (une par connexion) est notée,
    /// ligne de requête et `Authorization`, et reçoit la réponse suivante.
    fn fake_admin(answers: Vec<(u16, &'static str)>) -> (u16, Arc<Mutex<Vec<(String, String)>>>) {
        fake_admin_with(answers.into_iter().map(|(status, body)| (status, "", body)).collect())
    }

    /// [`fake_admin`], avec des en-têtes en plus pour chaque réponse
    /// (`"Retry-After: 7\r\n"`).
    fn fake_admin_with(answers: Vec<(u16, &'static str, &'static str)>) -> (u16, Arc<Mutex<Vec<(String, String)>>>) {
        use std::io::{Read, Write};
        let key = rcgen::KeyPair::generate().unwrap();
        let cert = rcgen::CertificateParams::new(vec!["localhost".into()]).unwrap().self_signed(&key).unwrap();
        let config = Arc::new(
            rustls::ServerConfig::builder_with_provider(Arc::new(rustls::crypto::ring::default_provider()))
                .with_safe_default_protocol_versions()
                .unwrap()
                .with_no_client_auth()
                .with_single_cert(
                    vec![cert.der().clone()],
                    rustls::pki_types::PrivateKeyDer::Pkcs8(rustls::pki_types::PrivatePkcs8KeyDer::from(key.serialize_der())),
                )
                .unwrap(),
        );
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let seen = Arc::new(Mutex::new(Vec::new()));
        let log = seen.clone();
        std::thread::spawn(move || {
            for (stream, (status, extra, body)) in listener.incoming().zip(answers) {
                let Ok(stream) = stream else { return };
                let mut tls = rustls::StreamOwned::new(rustls::ServerConnection::new(config.clone()).unwrap(), stream);
                let (mut buf, mut chunk) = (Vec::new(), [0u8; 4096]);
                let head_end = loop {
                    let n = tls.read(&mut chunk).unwrap_or(0);
                    if n == 0 {
                        return;
                    }
                    buf.extend_from_slice(&chunk[..n]);
                    if let Some(i) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
                        break i + 4;
                    }
                };
                let head = String::from_utf8_lossy(&buf[..head_end]).to_string();
                let header = |name: &str| {
                    head.lines()
                        .find_map(|l| l.split_once(':').filter(|(k, _)| k.eq_ignore_ascii_case(name)).map(|(_, v)| v.trim().to_string()))
                        .unwrap_or_default()
                };
                let len: usize = header("content-length").parse().unwrap_or(0);
                while buf.len() < head_end + len {
                    let n = tls.read(&mut chunk).unwrap_or(0);
                    if n == 0 {
                        break;
                    }
                    buf.extend_from_slice(&chunk[..n]);
                }
                log.lock().unwrap().push((head.lines().next().unwrap_or_default().to_string(), header("authorization")));
                let reply = format!("HTTP/1.1 {status} R\r\nContent-Type: application/json\r\n{extra}Content-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len());
                let _ = tls.write_all(reply.as_bytes());
                tls.conn.send_close_notify();
                let _ = tls.flush();
            }
        });
        (port, seen)
    }

    /// TRANS-34, 35, 36 : l'empreinte courte, la règle « même empreinte »,
    /// l'adresse tapée et l'adresse d'Admin, données par le cœur.
    #[test]
    fn empreinte_et_adresse_communes() {
        let fp = fingerprint(b"certificat");
        let short = short_fingerprint(&fp);
        assert_eq!(short.len(), 19);
        assert_eq!(short, short.to_uppercase());
        assert!(same_fingerprint(&fp, &fp.to_uppercase().replace("SHA256:", "")));
        assert_eq!(normalize_fingerprint(&fp.to_uppercase()).as_deref(), Some(fp.as_str()));
        assert_eq!(parse_address("fe80::1", Some(ADMIN_PORT)), Some(("fe80::1".into(), 9443)));
        assert_eq!(parse_address("fe80::1", None), None);
        assert_eq!(admin_url("[fe80::1]", 9443), "https://[fe80::1]:9443/");
        let c = Console::guest("[fe80::1]", 9443, None);
        assert_eq!(c.admin_url(), "https://[fe80::1]:9443/", "crochets jamais doublés");
        // Le certificat épinglé dans une autre écriture reste le même.
        let provider = Arc::new(rustls::crypto::ring::default_provider());
        let seen = Arc::new(Mutex::new(None));
        let v = PinVerifier { pinned: Some(fp.trim_start_matches("sha256:").replace(':', "")), first: Mutex::new(None), seen, provider };
        let cert = CertificateDer::from(b"certificat".to_vec());
        assert!(v.verify_server_cert(&cert, &[], &ServerName::try_from("localhost").unwrap(), &[], UnixTime::now()).is_ok());
    }

    /// COEUR-38 : une réponse refusée est lue une fois (statut, corps,
    /// `Retry-After`) pour tous les modules ; l'attente d'un partage garde
    /// maintenant le délai de l'en-tête, le corps passe avant lui, et une
    /// Station injoignable reste une erreur de réseau.
    #[test]
    fn refus_lus_une_fois() {
        let (port, _) = fake_admin_with(vec![
            (429, "Retry-After: 7\r\n", r#"{"error":"too_many"}"#),
            (429, "Retry-After: 7\r\n", r#"{"error":"too_many","retry_after":3}"#),
            (429, "Retry-After: 9\r\n", r#"{"error":"too_many"}"#),
            (429, "Retry-After: 11\r\n", r#"{"error":"Trop tôt.","code":"too_soon"}"#),
            (503, "", "<html>"),
        ]);
        let c = Console::guest("127.0.0.1", port, None);
        assert!(matches!(c.share_wait("g", "s"), Err(ShareError::TooManyTries { seconds: Some(7) })));
        let req = JoinRequest::default();
        assert!(matches!(c.share_join_answer(&req), Err(ShareError::TooManyTries { seconds: Some(3) })));
        assert!(matches!(c.lend_status("ami", "mdp"), Err(crate::lend::LendError::TooMany { seconds: Some(9) })));
        assert!(matches!(c.station_call("api/stream/station/restart", None, false, Duration::from_secs(5)), Err(StationError::TooSoon { seconds: Some(11), .. })));
        assert!(matches!(c.station_call("api/stream/station/restart", None, false, Duration::from_secs(5)), Err(StationError::Other(ConsoleError::Http { status: 503, .. }))));
        let closed = std::net::TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port();
        let c = Console::guest("127.0.0.1", closed, None);
        assert!(matches!(c.share_wait("g", "s"), Err(ShareError::Other(ConsoleError::Network(_)))));
    }

    #[test]
    fn compte_kybernow_jeton_d_acces() {
        let (port, seen) = fake_admin(vec![
            // Jeton échu (l'appareil a dormi) : un neuf, et l'appel refait.
            (401, r#"{"error":"Jeton échu.","code":"token_expired"}"#),
            (200, "{}"),
            (200, r#"{"ok":true}"#),
            (200, "{}"),
        ]);
        let c = Console::account("127.0.0.1", port, access(Tokens::default()), None);
        assert!(c.station_access().is_some());
        assert!(c.info().is_ok(), "second essai avec un jeton neuf");
        // Redémarrer : un jeton à part, qu'Admin consomme ; le jeton en cours reste.
        assert!(c.station_restart(None, false).is_ok());
        assert!(c.info().is_ok());
        let seen = seen.lock().unwrap().clone();
        let auth: Vec<&str> = seen.iter().map(|(_, a)| a.as_str()).collect();
        assert_eq!(auth, ["KyberNow jeton-0", "KyberNow jeton-1", "KyberNow jeton-2", "KyberNow jeton-1"]);
        assert!(seen[0].0.starts_with("GET /api/stream/info") && seen[2].0.starts_with("POST /api/stream/station/restart"), "{seen:?}");
        // Session de l'appareil fermée : rien ne part vers Admin, la raison est rendue.
        let c = Console::account("127.0.0.1", port, access(Tokens { closed: true, ..Default::default() }), None);
        assert!(matches!(c.info(), Err(ConsoleError::Account(AccountError::SignedOut))));
        assert_eq!(ConsoleError::Account(AccountError::SignedOut).text(Lang::Fr), "Ce compte a été déconnecté de cet appareil : reconnectez-vous.");
    }

    /// Vérification des mises à jour de la Station (docs/link-ui.md, « Mise à
    /// jour des Stations ») : ce qu'elle annonce ensuite ; une Station qui ne
    /// connaît pas la route (404) rend `None` sans erreur.
    #[test]
    fn verification_des_mises_a_jour_de_la_station() {
        let (port, seen) = fake_admin(vec![
            (200, r#"{"ok":true,"station":{"version":"0.11.2","update":{"available":true,"version":"0.11.3","installable":true},"restarting":false}}"#),
            (404, r#"{"error":"Introuvable."}"#),
        ]);
        let c = Console::account("127.0.0.1", port, access(Tokens::default()), None);
        let station = c.station_check().unwrap().unwrap();
        assert_eq!(station.update.version.as_deref(), Some("0.11.3"));
        assert!(station.update.available && station.update.installable);
        assert!(c.station_check().unwrap().is_none());
        let seen = seen.lock().unwrap().clone();
        assert!(seen[0].0.starts_with("POST /api/stream/station/check"), "{seen:?}");
    }

    /// Regarder (docs/comptes.md) : `POST /api/share/watch` avec le jeton du
    /// compte ; la réponse est celle de `join`, avec les écrans ; un jeton
    /// échu est redemandé ; les refus que le cœur dit avec ses mots.
    #[test]
    fn regarder_avec_le_jeton_du_compte() {
        let joined = r#"{"guest":"g1","secret":"s1","role":"spectator","host":"PC-SALON","computer":"PC-SALON","kyber":{"port":12345,"fingerprint":"sha256:aa","token":"jwt","expires":120},"display":{"id":7,"width":3840,"height":2160,"refresh":60},"codec":"hevc","yuv444":false,"beat":5,"watch":{"screens":[{"id":7,"index":1,"label":"Écran 1","width":3840,"height":2160,"refresh":60,"primary":true},{"id":8,"index":2,"label":"Écran 2","width":1920,"height":1080,"refresh":60,"primary":false}],"switch":true,"display":7}}"#;
        let (port, seen) = fake_admin(vec![
            (401, r#"{"error":"Jeton échu.","code":"token_expired"}"#),
            (200, joined),
            (409, r#"{"error":"PC-SALON est prêté en ce moment.","code":"lent","computer":"PC-SALON"}"#),
            (409, r#"{"error":"Le partage est coupé…","code":"disabled"}"#),
            (403, r#"{"error":"Ce compte n'a pas ce droit sur cet ordinateur (view).","code":"no_right"}"#),
            (404, ""),
            (200, r#"{"role":"spectator","token":"jwt2","expires":120,"end":null,"display":{"id":8,"width":1920,"height":1080,"refresh":60}}"#),
        ]);
        let c = Console::account("127.0.0.1", port, access(Tokens::default()), None);
        assert_eq!(c.port(), port);
        let req = WatchRequest { app: "mac".into(), ..Default::default() };
        let j = c.share_watch(&req).unwrap();
        assert_eq!((j.guest.as_str(), j.display.id, j.kyber.token.as_str()), ("g1", 7, "jwt"));
        let w = j.watch.unwrap();
        assert!(w.switch && w.menu() && w.screens.len() == 2);
        assert_eq!(w.screens[1].label(Lang::Fr), "Écran 2 — 1920 × 1080");
        assert_eq!(w.screens[0].label(Lang::En), "Screen 1 — 3840 × 2160");
        match c.share_watch(&req) {
            Err(e @ ShareError::Lent { .. }) => {
                assert_eq!(e.text(Lang::Fr), "PC-SALON est prêté en ce moment.");
                assert_eq!(e.text(Lang::En), "PC-SALON is lent right now.");
                assert_eq!(e.kind(), "lent");
            }
            other => panic!("{other:?}"),
        }
        let off = c.share_watch(&req).unwrap_err();
        assert!(matches!(off, ShareError::SharingOff));
        assert_eq!(off.text(Lang::Fr), "Le partage est coupé sur cet ordinateur (Admin › Appareils Link).");
        assert_eq!(off.text(Lang::En), "Sharing is turned off on this computer (Admin › Link devices).");
        // Un autre refus : le texte d'Admin.
        match c.share_watch(&req) {
            Err(ShareError::Other(ConsoleError::Http { status: 403, message })) => assert_eq!(message, "Ce compte n'a pas ce droit sur cet ordinateur (view)."),
            other => panic!("{other:?}"),
        }
        // Une Station qui ne connaît pas la route.
        let old = c.share_watch(&req).unwrap_err();
        assert!(matches!(old, ShareError::WatchUnsupported));
        assert_eq!(old.text(Lang::Fr), "Cette Station ne permet pas encore de regarder : mettez-la à jour.");
        assert_eq!(old.text(Lang::En), "This Station can't be watched yet: update it.");
        // « Écran » : la réponse d'un signe de vie.
        let b = Console::guest("127.0.0.1", port, None).share_watch_display("g1", "s1", 8).unwrap();
        assert_eq!((b.display.unwrap().id, b.token.as_deref()), (8, Some("jwt2")));
        let seen = seen.lock().unwrap().clone();
        assert!(seen[0].0.starts_with("POST /api/share/watch ") && seen[6].0.starts_with("POST /api/share/watch/display"), "{seen:?}");
        assert_eq!((seen[0].1.as_str(), seen[1].1.as_str(), seen[6].1.as_str()), ("KyberNow jeton-0", "KyberNow jeton-1", ""));
        // Les mots de la barre et de la carte.
        assert_eq!(crate::share::watch_title(Lang::Fr, "PC-SALON"), "Regarder PC-SALON");
        assert_eq!(crate::share::watch_title(Lang::En, "PC-SALON"), "Watching PC-SALON");
        assert_eq!(crate::share::watch_word(Lang::Fr, "stop"), "Arrêter de regarder");
        assert_eq!(crate::share::watch_word(Lang::En, "screen"), "Screen");
        assert_eq!(crate::share::watch_word(Lang::Fr, "action"), "Regarder");
        assert_eq!(crate::share::ShareEnd::Removed.text_watching(Lang::Fr, "Matthieu"), "Matthieu vous a retiré.");
        assert_eq!(crate::share::ShareEnd::Removed.text_watching(Lang::En, "Matthieu"), "Matthieu removed you.");
        assert_eq!(crate::share::ShareEnd::Stopped.text_watching(Lang::Fr, "Matthieu"), "Matthieu a arrêté le partage.");
    }

    #[test]
    fn jeton_de_kyber_d_un_compte() {
        let w: PreparedWire = serde_json::from_str(r#"{"kyber_port":24812,"kyber_token":{"token":"jwt","user":"kn-account-abc","expires":300}}"#).unwrap();
        let p = w.prepared();
        assert_eq!(p.kyber_token, Some(KyberToken { token: "jwt".into(), user: "kn-account-abc".into(), expires: 300 }));
        let w: PreparedWire = serde_json::from_str(r#"{"kyber_token":{"token":""}}"#).unwrap();
        assert_eq!(w.prepared().kyber_token, None, "vide : pas de jeton");
        let beat: HeartbeatReply = serde_json::from_str(r#"{"active":true,"kyber_token":{"token":"jwt-2"}}"#).unwrap();
        assert_eq!(beat.kyber_token.map(|k| k.token).as_deref(), Some("jwt-2"));
    }
}
