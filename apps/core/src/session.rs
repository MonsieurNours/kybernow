//! Une session : préparation par la console (écran virtuel, son), connexion à
//! Kyber, choix de l'écran, image et son vers l'app, signal de vie, reprise
//! après une perte, fin propre ; et la continuité de l'image : image clé
//! redemandée puis flux relancé, repli quand aucune image ne vient, taille
//! de l'écran de l'ordinateur suivie, politique de mode (« Remettre »),
//! adaptation à un nouveau mode de l'écran de l'appareil ; en dernier
//! recours, la Station redémarrée pendant la connexion (voir
//! [`Recovery::restart_station`]). Multi-écrans
//! ([`Session::start_with_screens`], docs/multi-ecrans.md) : un écran
//! virtuel par écran de l'appareil, une entrée vidéo par écran. Plusieurs
//! ordinateurs à la fois ([`Session::set_active`], [`Session::set_muted`],
//! [`crate::focus`], docs/plusieurs-ordinateurs.md) : la session tient son
//! état « active » et « son coupé » d'un démarrage de l'image à l'autre et
//! l'applique à chaque chaîne d'entrées et moteur de son de la plateforme.
//! KyberNow Share ([`SessionRole`], docs/kybernow-share.md) : une session
//! d'invité rejoint par `share_join` au lieu de `prepare`, se connecte à
//! Kyber avec le jeton de la Station, reçoit l'écran qu'elle donne, ne
//! change jamais rien sur l'ordinateur ([`HostOnly`]) et part par
//! `share_leave` ; ses entrées suivent son rôle. Une session d'hôte reçoit
//! la liste de ses invités ([`Platform::on_guests`]).
//!
//! Tout se décide sur un fil dédié ; les événements de Kyber, le signal de vie
//! et les demandes de l'app lui arrivent par un canal.

use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex, Weak};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use kyclient::{
    AudioProtocol, AudioSessionConfig, AuthCredentials, Client, ConnectConfig, ConnectFailedReason,
    Display, Event, EventSink, InputSessionConfig, KyProtoConfig, KymuxConfig, StopReason,
    StreamerPipelineEventType, StreamingProtocol, StreamingSessionConfig, VideoPlayerConfig,
    VideoProtocol, VideoSessionConfig,
};
use kynput_rs::{ControlEvent, InputTarget, Pipeline};
use kyvlcplayer::WindowHandle;

use crate::audio::AudioEngine;
use crate::input::{InputPipeline, InputRights};
use crate::journal::{self, Label, jinfo as info, jwarn as warn};

use crate::console::{
    Console, ConsoleError, DeviceInfo, Heartbeat, HeartbeatReply, Mode, PrepareRequest, Prepared,
    ScreenMode, StationError,
};
use crate::i18n::{self, Lang};
use crate::latency::{LatencyTracker, Second};
use crate::media::{Packet, PacketHandler, packet_sink};
use crate::profile::{Capabilities, SettingsJson, StreamSettings};
use crate::screens::{self, PlannedScreen, Screens};
use crate::share::{self, Found, ShareBeat, ShareDisplay, ShareEnd, ShareError, ShareImage, ShareJoined, ShareQuality, ShareRole, ShareState};
#[cfg(test)]
use crate::share::ShareGuest;

/// Reprise de l'image par le cœur (apps Apple, Android, Linux, webOS). Par
/// défaut (Windows), la plateforme tient elle-même ses veilles et appelle
/// [`Session::request_keyframe`] et [`Session::no_image`] ; le dernier
/// recours ([`Self::restart_station`]) vaut pour toutes.
#[derive(Debug, Clone)]
pub struct Recovery {
    /// Le cœur veille lui-même à la première image (4 s après le démarrage du
    /// flux, d'après [`Session::frame_displayed`]) et aux images clés
    /// ([`Session::keyframe_lost`] : redemandée toutes les 300 ms, flux relancé
    /// à 1,5 s).
    pub watch_image: bool,
    /// Sans image : le flux est d'abord allégé, pour cette session seulement
    /// (débit ramené à ce plafond, en bit/s, puis 4:2:0), avant d'être relancé.
    pub fallback_bitrate: Option<u32>,
    /// Dernier recours de la connexion (vrai par défaut) : Admin a répondu,
    /// mais l'image n'arrive pas malgré les réparations ci-dessus, ou la
    /// connexion à Kyber échoue. La Station est alors redémarrée une fois,
    /// sans couper la session d'un autre appareil, attendue (90 s au plus),
    /// puis la session reprend du début. Jamais une fois la première image
    /// montrée. Faux : la session échoue tout de suite (ligne de commande,
    /// bancs d'essai).
    pub restart_station: bool,
}

impl Default for Recovery {
    fn default() -> Self {
        Self { watch_image: false, fallback_bitrate: None, restart_station: true }
    }
}

/// Le rôle de la session (KyberNow Share, docs/kybernow-share.md).
#[derive(Clone, Debug, Default)]
pub enum SessionRole {
    /// La session de l'appareil, comme toujours : `prepare`, écran virtuel,
    /// signe de vie, fin dite à Admin ; elle peut partager son écran.
    #[default]
    Host,
    /// Un invité d'un partage : spectateur ou joueur, selon la Station.
    Guest(Box<GuestParams>),
}

/// Ce qu'une session d'invité tient de « Rejoindre » ([`share::join`]).
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct GuestParams {
    /// La réponse de `share_join`, telle quelle.
    pub joined: ShareJoined,
    /// Ce que l'appareil décode : le format de l'image de l'hôte est vérifié
    /// avant l'image ([`share::guest_format`] : le sien, ou un format lisible).
    #[serde(default)]
    pub capabilities: Capabilities,
}

/// Ce qu'il faut pour ouvrir une session.
#[derive(Clone)]
pub struct SessionParams {
    pub console_host: String,
    pub console_port: u16,
    pub user: String,
    pub password: String,
    /// Empreinte attendue du certificat de la console.
    pub console_fingerprint: Option<String>,
    pub device: DeviceInfo,
    /// Mode demandé pour l'écran de l'ordinateur (celui de l'appareil).
    pub mode: Mode,
    /// 0 : son laissé sur l'ordinateur.
    pub audio_channels: u16,
    pub virtual_display: bool,
    pub settings: StreamSettings,
    pub auto_reconnect: bool,
    /// Manettes vues par l'ordinateur comme des Xbox 360 (sinon fidèles :
    /// DualShock 4 pour une manette Sony).
    pub gamepad_xbox360: bool,
    pub clipboard: bool,
    pub recovery: Recovery,
    /// Hôte (par défaut) ou invité d'un partage. Un invité ne se sert ni de
    /// `user` et `password` (jeton de la Station), ni de `mode` et
    /// `virtual_display` (l'écran du partage), ni du format de `settings`
    /// (celui de l'hôte), ni de `clipboard` (jamais pour un invité) ;
    /// `console_host`, `console_port` et `console_fingerprint` sont ceux de
    /// la Station qui partage ([`Found::target`]).
    pub role: SessionRole,
}

impl SessionParams {
    /// Les paramètres d'une session d'invité, d'après la Station trouvée par
    /// [`share::join`] : `settings` (le profil de l'app : transport ; format
    /// de repli tant que l'image de l'hôte n'a pas démarré), `capabilities`
    /// (format vérifié), `audio_channels` (0 : pas de son sur l'appareil).
    /// Les autres champs sont ceux d'une session ordinaire, à ajuster par
    /// l'app (`recovery`, `gamepad_xbox360`, `auto_reconnect`).
    pub fn guest(found: &Found, device: DeviceInfo, settings: StreamSettings, capabilities: Capabilities, audio_channels: u16) -> Self {
        let d = found.joined.display;
        SessionParams {
            console_host: found.target.host.clone(),
            console_port: found.target.port,
            user: String::new(),
            password: String::new(),
            console_fingerprint: found.target.fingerprint.clone(),
            device,
            mode: Mode { width: d.width, height: d.height, refresh: d.refresh },
            audio_channels,
            virtual_display: false,
            settings,
            auto_reconnect: true,
            gamepad_xbox360: true,
            clipboard: false,
            recovery: Recovery::default(),
            role: SessionRole::Guest(Box::new(GuestParams { joined: found.joined.clone(), capabilities })),
        }
    }

    pub fn is_guest(&self) -> bool {
        matches!(self.role, SessionRole::Guest(_))
    }

    /// Les paramètres en JSON, comme les donnent les ponts des apps
    /// (`kn_session_start`, JNI) : `console_host`, `console_port`, `user`,
    /// `password`, `console_fingerprint`, `device`, `mode`, `audio_channels`,
    /// `virtual_display`, `settings`, `auto_reconnect`, `gamepad_xbox360`,
    /// `clipboard`, `recovery`, et `screens` (multi-écrans, rendu à part pour
    /// [`Session::start_with_screens`]) ; pour un invité, `guest` :
    /// `{"joined": <réponse de share_join>, "capabilities": {hevc, hevc444,
    /// av1}}` (`user`, `password` et `mode` sont alors facultatifs).
    pub fn from_json(json: &str) -> Result<(SessionParams, Option<Screens>), String> {
        let wire: ParamsJson = serde_json::from_str(json).map_err(|e| e.to_string())?;
        let settings = wire.settings.settings()?;
        let role = match wire.guest {
            Some(g) => SessionRole::Guest(Box::new(g)),
            None => SessionRole::Host,
        };
        let mode = match (wire.mode, &role) {
            (Some(m), _) => m,
            (None, SessionRole::Guest(g)) => {
                let d = g.joined.display;
                Mode { width: d.width, height: d.height, refresh: d.refresh }
            }
            (None, SessionRole::Host) => return Err("mode absent".into()),
        };
        let params = SessionParams {
            console_host: wire.console_host,
            console_port: wire.console_port,
            user: wire.user,
            password: wire.password,
            console_fingerprint: wire.console_fingerprint.filter(|f| !f.is_empty()),
            device: wire.device,
            mode,
            audio_channels: wire.audio_channels,
            virtual_display: wire.virtual_display,
            settings,
            auto_reconnect: wire.auto_reconnect,
            gamepad_xbox360: wire.gamepad_xbox360,
            clipboard: wire.clipboard,
            recovery: Recovery {
                watch_image: wire.recovery.watch_image,
                fallback_bitrate: wire.recovery.fallback_bitrate,
                restart_station: wire.recovery.restart_station,
            },
            role,
        };
        Ok((params, wire.screens))
    }
}

#[derive(serde::Deserialize)]
struct RecoveryJson {
    #[serde(default)]
    watch_image: bool,
    /// Plafond du repli, en bit/s (comme `Recovery::fallback_bitrate`).
    fallback_bitrate: Option<u32>,
    /// Dernier recours (`Recovery::restart_station`) : vrai si absent.
    #[serde(default = "yes")]
    restart_station: bool,
}

impl Default for RecoveryJson {
    fn default() -> Self {
        Self { watch_image: false, fallback_bitrate: None, restart_station: true }
    }
}

#[derive(serde::Deserialize)]
struct ParamsJson {
    console_host: String,
    console_port: u16,
    #[serde(default)]
    user: String,
    #[serde(default)]
    password: String,
    console_fingerprint: Option<String>,
    device: DeviceInfo,
    #[serde(default)]
    mode: Option<Mode>,
    #[serde(default)]
    audio_channels: u16,
    #[serde(default = "yes")]
    virtual_display: bool,
    settings: SettingsJson,
    #[serde(default = "yes")]
    auto_reconnect: bool,
    #[serde(default = "yes")]
    gamepad_xbox360: bool,
    #[serde(default = "yes")]
    clipboard: bool,
    #[serde(default)]
    recovery: RecoveryJson,
    #[serde(default)]
    screens: Option<Screens>,
    #[serde(default)]
    guest: Option<GuestParams>,
}

fn yes() -> bool {
    true
}

/// Ce que la console confirme en fin de session.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EndReport {
    pub display_restored: bool,
    pub audio_restored: bool,
    /// Erreur de la console, ou console injoignable.
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Status {
    Preparing,
    Connecting,
    FirstImages,
    Streaming,
    Reconnecting { attempt: u32, max: u32 },
    /// Dernier recours de la connexion ([`Recovery::restart_station`]) : la
    /// Station redémarre, l'écran de connexion affiche « La Station
    /// redémarre… » jusqu'à l'état suivant (`Preparing` si elle revient,
    /// `Ending` puis `Ended` sinon). Une app qui ne le montre pas encore reste
    /// sur l'étape « Connexion ».
    StationRestarting,
    Ending,
    /// Fin de session ; `error` dit pourquoi si elle n'était pas voulue.
    Ended { error: Option<String>, report: Option<EndReport> },
}

/// Ce que la session montre dans une capsule ou dans la barre (sans texte :
/// la plateforme le traduit).
#[derive(Debug, Clone, PartialEq)]
pub enum Notice {
    /// « L'image a été relancée. »
    Relaunched,
    /// L'écran affiché a changé de taille sur l'ordinateur ; `back` : revenu
    /// au mode du départ.
    ModeChanged { width: u32, height: u32, back: bool },
    /// Flux allégé pour cette session : 1 = débit plafonné, 2 = 4:2:0.
    Fallback { level: u8, bitrate_mbps: u32 },
    /// Adaptation à un nouveau mode de l'écran de l'appareil : demandée, faite, refusée.
    Adapting { width: u32, height: u32 },
    Adapted { width: u32, height: u32 },
    AdaptFailed { error: String },
    /// « Remettre W × H » à proposer dans la barre (`None` : plus rien à proposer).
    RestoreOffer { mode: Option<Mode> },
}

/// Multi-écrans : un écran de l'ordinateur pris par la session, et l'écran
/// de l'appareil auquel il va.
#[derive(Debug, Clone, PartialEq)]
pub struct StreamScreen {
    /// Index de l'écran de l'appareil dans [`Screens::list`].
    pub screen: usize,
    /// L'écran de l'ordinateur tel que Kyber l'annonce : `id` (le
    /// `display_id` des positions de souris et du pointeur), taille en pixels
    /// (repère des entrées de cet écran), position sur le bureau de Windows.
    pub display: Display,
    /// Fréquence de l'écran virtuel (appliquée par la Station).
    pub refresh: u32,
    /// L'écran principal de Windows (le premier de la liste).
    pub primary: bool,
}

/// Ce que la plateforme fournit et reçoit (fenêtre, décodeurs, entrées).
pub trait Platform: Send + Sync {
    fn window_handle(&self) -> WindowHandle;
    fn video_handler(&self) -> Arc<dyn PacketHandler>;
    /// `None` : pas de son sur l'appareil.
    fn audio_handler(&self) -> Option<Arc<dyn PacketHandler>>;
    /// Appelée à chaque démarrage de l'image, après [`Self::on_display`].
    fn input_pipeline(&self) -> Pipeline;
    fn on_status(&self, status: &Status);
    /// L'écran de l'ordinateur affiché (repère des entrées) ; aussi quand sa
    /// taille change.
    fn on_display(&self, display: &Display);
    /// Les écrans annoncés par Kyber (choix dans la barre, sans écran virtuel).
    fn on_displays(&self, _displays: &[Display]) {}
    /// Mode de l'écran de la session (préparé, adapté, changé par l'ordinateur),
    /// et si c'est un écran virtuel (choix d'un écran dans la barre sinon).
    fn on_mode(&self, _mode: Mode, _virtual_display: bool) {}
    /// L'image vient d'être demandée à Kyber (entrées reliées à sa session).
    fn on_stream_started(&self) {}
    /// L'image vient d'être arrêtée (relance, fin).
    fn on_stream_stopped(&self) {}
    fn on_input_connected(&self) {}
    /// Latence médiane de la dernière seconde.
    fn on_latency(&self, _ms: Option<f64>) {}
    /// Mesures de la dernière seconde (barre, panneau des mesures).
    fn on_second(&self, _second: &Second) {}
    /// Un jeu est en plein écran sur l'ordinateur (souris Auto).
    fn on_game_fullscreen(&self, _game: bool) {}
    fn on_notice(&self, _notice: &Notice) {}
    /// Juste avant [`Status::Ending`] : la raison de la fin quand elle
    /// n'était pas voulue (la même que celle de [`Status::Ended`], qui suit
    /// après la fin dite à Admin), `None` sinon. Une app y voit tout de suite
    /// que c'est un échec (plusieurs ordinateurs : la fenêtre d'une session
    /// suivante montre « Fermer ») ; sans rien ici, rien ne change.
    /// `Status::Ending` reste sans champ, pour ne pas casser les apps qui le
    /// filtrent par son nom.
    fn on_ending(&self, _error: Option<&str>) {}

    // Multi-écrans ([`Session::start_with_screens`]) : rien à faire pour une
    // app qui n'ouvre que des sessions à un écran.

    /// Le gestionnaire des paquets d'image de l'écran `screen` de l'appareil
    /// (index dans [`Screens::list`]) : un par flux, comme `video_handler`.
    fn screen_video_handler(&self, _screen: usize) -> Arc<dyn PacketHandler> {
        self.video_handler()
    }
    /// Les écrans pris par la session, principal en premier, chacun avec son
    /// écran de l'appareil : à chaque démarrage de l'image (après
    /// [`Self::on_display`], qui donne toujours l'écran principal, et avant
    /// [`Self::input_pipeline`]), et quand la taille de l'un d'eux change.
    /// `not_created` : les écrans de l'appareil du plan que l'ordinateur n'a
    /// pas pu créer (refusés par la Station, ou jamais annoncés par Kyber) ;
    /// leur fenêtre garde le fond de la connexion avec « L'ordinateur n'a pas
    /// pu créer cet écran. » (docs/link-ui.md). Un écran du plan absent des
    /// deux listes : la Station ne gère qu'un écran (ou n'a pas créé l'écran
    /// principal) et la session se fait sur l'écran principal seul.
    fn on_screens(&self, _screens: &[StreamScreen], _not_created: &[usize]) {}

    // KyberNow Share (docs/kybernow-share.md) : rien à faire pour une app
    // qui ne partage pas encore.

    /// Session d'hôte : le partage ouvert et ses invités, tels que le dit le
    /// signe de vie (champ `share`) ; `None` : pas de partage (ou Station
    /// antérieure). Appelée quand cela change (le premier signe de vie qui
    /// en porte un, chaque arrivée, départ ou rôle changé, la fin).
    fn on_guests(&self, _share: Option<&ShareState>) {}
    /// Session d'invité : son rôle au départ (`previous` nul, avant la
    /// connexion à Kyber), puis à chaque changement par l'hôte (avis bref de
    /// link-ui : « Vous pouvez jouer à la manette. », « Les manettes sont
    /// reprises. », « Vous avez le clavier et la souris. », « Le clavier et
    /// la souris sont repris. », « Vous regardez de nouveau. »). Les entrées
    /// le suivent déjà ; un passage entre deux rôles qui ont chacun une
    /// chaîne d'entrées ne relance pas l'image.
    fn on_share_role(&self, _role: ShareRole, _previous: Option<ShareRole>) {}
    /// Session d'invité : c'est le partage qui la finit (arrêté, retiré,
    /// format illisible), juste avant [`Self::on_ending`] ; `text` : le texte
    /// de cette fin dans la langue de l'app (link-ui), qui est aussi la
    /// raison de la fin. Jamais « Reprendre la main ».
    fn on_share_end(&self, _end: &ShareEnd, _text: &str) {}
    /// Session d'invité : l'image qu'il reçoit (`join`, puis quand elle
    /// change : signe de vie, [`Session::set_share_quality`]), et celle d'avant
    /// (`None` au départ) ; `notice` : l'avis bref à montrer, dans la langue
    /// de l'app (« Image allégée pour suivre. », « L'ordinateur ne peut pas
    /// faire une image de plus : vous recevez {qualité}. », voir
    /// [`ShareImage::notice`]), `None` s'il n'y a rien à dire. La barre montre
    /// la qualité reçue ([`ShareImage::label`]).
    fn on_share_image(&self, _image: &ShareImage, _previous: Option<&ShareImage>, _notice: Option<&str>) {}
}

/// Ce qu'une session d'invité ne fait jamais : chaque appel à Admin ou à
/// Kyber qui change l'ordinateur passe par [`Worker::host_only`], qui le
/// ferme pour un invité (et le note, pour les tests).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum HostOnly {
    /// `POST /api/stream/prepare` (écran virtuel, son, mode).
    Prepare,
    /// `POST /api/stream/heartbeat`.
    Heartbeat,
    /// `GET /api/stream/foreground`.
    Foreground,
    /// Écran virtuel refait à un autre mode.
    Adapt,
    /// Session refaite (écrans de l'appareil, profil).
    Replan,
    /// « Remettre » le mode de l'ordinateur.
    RestoreMode,
    /// Débit de l'image de l'hôte (un invité règle celui de son image à
    /// part seulement : Kyber le borne).
    Bitrate,
    /// Image allégée (débit, 4:2:0).
    Fallback,
    /// Redémarrage de la Station en dernier recours.
    StationRestart,
    /// `POST /api/stream/end` (écrans et son rétablis).
    End,
    /// Un autre écran de l'ordinateur.
    PickDisplay,
    /// Un autre profil.
    Settings,
}

/// Paquets d'un écran d'une session à plusieurs écrans : note les premières
/// données du flux (une image clé ne se demande jamais avant, voir
/// `Worker::request_keyframe`).
struct ScreenSink {
    inner: Arc<dyn PacketHandler>,
    first_data: Arc<AtomicBool>,
}

impl ScreenSink {
    fn note(&self, packet: &Packet<'_>) {
        if matches!(packet, Packet::Media { .. }) && !self.first_data.load(Ordering::Relaxed) {
            self.first_data.store(true, Ordering::Relaxed);
        }
    }
}

impl PacketHandler for ScreenSink {
    fn packet(&self, packet: Packet<'_>) {
        self.note(&packet);
        self.inner.packet(packet);
    }

    fn packet_from(&self, source_id: u32, packet: Packet<'_>) {
        self.note(&packet);
        self.inner.packet_from(source_id, packet);
    }
}

enum Msg {
    Event(Event),
    Stop,
    Fail(Failure),
    KeyframeNeeded,
    KeyframeLost,
    KeyframeRecovered,
    NoImage,
    /// Réponse du signal de vie.
    Beat(HeartbeatReply),
    /// Réponse du signe de vie d'un invité.
    ShareBeat(ShareBeat),
    /// La Station ne connaît plus l'invité (partage fermé, invité retiré).
    ShareGone(ShareEnd),
    /// « Qualité de l'image » choisie par l'invité.
    ShareQuality(ShareQuality),
    /// L'image de l'invité, répondue par `POST /api/share/quality`.
    ShareImage(ShareImage),
    /// Autre profil pendant la session.
    Settings(StreamSettings),
    /// Plafond de débit (fenêtre occultée) ; `None` : plus de plafond.
    BitrateCap(Option<u32>),
    /// Taille des images décodées par l'app.
    DecodedSize(u32, u32),
    /// Écran choisi dans la barre.
    PickDisplay(u32),
    /// Nouveau mode pour l'écran de l'ordinateur (écran de l'appareil changé).
    SetMode(Mode),
    /// Multi-écrans : les écrans de l'appareil ont changé.
    SetScreens(Screens),
    /// « Remettre » : le mode du départ redemandé à la console.
    RestoreMode,
    /// Réponses de la console, depuis un fil d'appel (avec les nouveaux
    /// écrans de l'appareil et leur plan quand ce sont eux qui ont changé).
    Adapted(Mode, Option<(Screens, Vec<PlannedScreen>)>, Result<Prepared, String>),
    ModeRestored,
}

/// Les événements d'un client Kyber, tant que sa tentative de connexion vit
/// (ceux d'un client arrêté avant un redémarrage de la Station ne comptent plus).
struct Events {
    tx: Mutex<Sender<Msg>>,
    alive: Arc<AtomicBool>,
}

impl EventSink for Events {
    fn on_event(&self, event: &Event) {
        // Appelé sous le verrou de Kyber : on ne fait que transmettre.
        if self.alive.load(Ordering::Relaxed) {
            let _ = self.tx.lock().unwrap().send(Msg::Event(event.clone()));
        }
    }
}

/// Ce qu'un redémarrage de la Station peut réparer (voir [`restart_wanted`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Cause {
    /// L'image n'arrive pas malgré les réparations de la session (image clé,
    /// flux relancé, repli), ou Kyber la refuse.
    NoImage,
    /// La connexion à Kyber échoue, après ses propres essais.
    Kyber,
}

/// Échec d'une tentative de connexion : la raison montrée à l'utilisateur
/// (langue de l'app), et ce qu'un redémarrage de la Station pourrait réparer
/// (`None` : rien, la session échoue comme elle est).
#[derive(Debug, Clone, PartialEq)]
struct Failure {
    text: String,
    cause: Option<Cause>,
}

impl Failure {
    fn no_image(text: String) -> Self {
        Self { text, cause: Some(Cause::NoImage) }
    }

    fn kyber(text: String) -> Self {
        Self { text, cause: Some(Cause::Kyber) }
    }
}

/// Une erreur ordinaire (Admin, compte, certificat, erreur locale) : jamais
/// de redémarrage de la Station.
impl From<String> for Failure {
    fn from(text: String) -> Self {
        Self { text, cause: None }
    }
}

/// Kyber refuse la connexion : réseau, délai, erreur du serveur ou du
/// protocole se réparent en redémarrant la Station. Pas le compte (Admin
/// redémarre déjà Kyber pour les comptes en attente quand personne d'autre ne
/// s'en sert : le refaire couperait quelqu'un), ni le certificat, ni une
/// erreur de configuration de l'appareil.
fn connect_failed_cause(reason: ConnectFailedReason) -> Option<Cause> {
    match reason {
        ConnectFailedReason::NetworkError
        | ConnectFailedReason::TimedOut
        | ConnectFailedReason::ServerError
        | ConnectFailedReason::WebSocketFailed
        | ConnectFailedReason::ProtocolFailed => Some(Cause::Kyber),
        _ => None,
    }
}

/// Kyber s'arrête de lui-même : connexion fermée ou muette, réparable ; une
/// erreur de décodage de l'appareil, non.
fn stopped_cause(reason: StopReason) -> Option<Cause> {
    match reason {
        StopReason::ControlPlaneClosed | StopReason::ControlPlaneTimeout => Some(Cause::Kyber),
        _ => None,
    }
}

/// Où en est la tentative de connexion quand elle échoue.
#[derive(Debug, Clone, Copy)]
struct Attempt {
    /// [`Recovery::restart_station`].
    enabled: bool,
    /// Admin a répondu : `prepare` a réussi.
    prepared: bool,
    /// Une image a été montrée : la session était établie.
    shown: bool,
    /// Admin a demandé la fin (depuis Admin, ou un autre appareil prend la main).
    end_requested: bool,
    /// La Station a déjà été redémarrée pour cette connexion.
    restarted: bool,
}

/// Le dernier recours s'applique-t-il ? Seulement pour un échec qu'un
/// redémarrage répare, quand Admin a répondu, avant toute image, sans fin
/// demandée par Admin, et une seule fois par connexion. Une session établie
/// garde ses propres reprises (reconnexion, relances).
fn restart_wanted(cause: Option<Cause>, a: Attempt) -> bool {
    cause.is_some() && a.enabled && a.prepared && !a.shown && !a.end_requested && !a.restarted
}

/// Retour de la Station après un redémarrage de dernier recours : `info`
/// toutes les 2 s, 90 s au plus.
const STATION_POLL: Duration = Duration::from_secs(2);
const STATION_BACK_WITHIN: Duration = Duration::from_secs(90);

/// Ce que le dernier recours demande à Admin (une fausse console dans les tests).
trait StationAdmin {
    /// `POST /api/stream/station/restart` ; `device_id` nomme l'appareil dans le fil d'Admin.
    fn restart(&self, device_id: &str, force: bool) -> Result<(), StationError>;
    /// Admin répond, et la Station a fini de redémarrer.
    fn back(&self) -> bool;
}

impl StationAdmin for Console {
    fn restart(&self, device_id: &str, force: bool) -> Result<(), StationError> {
        self.station_restart(Some(device_id), force)
    }

    fn back(&self) -> bool {
        // `station` absent : rien ne dit qu'elle redémarre encore.
        self.info().is_ok_and(|i| !i.station.is_some_and(|s| s.restarting))
    }
}

/// Issue du redémarrage de dernier recours.
#[derive(Debug)]
enum Repair {
    /// Admin refuse : un autre appareil est en session, trop tôt, Station
    /// trop ancienne, ou erreur. La session échoue comme elle est.
    Refused(StationError),
    /// La Station est revenue : la connexion reprend du début.
    Back,
    /// Pas revenue dans le délai.
    Late,
    /// L'utilisateur a annulé pendant l'attente.
    Cancelled,
}

/// Redémarre la Station, jamais au détriment d'un autre appareil (`force`
/// faux : Admin refuse s'il en a un en session), puis attend son retour :
/// `back` toutes les `poll`, `within` au plus. `restarting` : le redémarrage
/// est accepté (la plateforme l'affiche). `stop(d)` attend au plus `d` et dit
/// si l'utilisateur annule.
fn repair(
    admin: &dyn StationAdmin,
    device_id: &str,
    poll: Duration,
    within: Duration,
    restarting: impl FnOnce(),
    mut stop: impl FnMut(Duration) -> bool,
) -> Repair {
    if let Err(e) = admin.restart(device_id, false) {
        return Repair::Refused(e);
    }
    restarting();
    let deadline = Instant::now() + within;
    loop {
        let left = deadline.saturating_duration_since(Instant::now());
        if left.is_zero() {
            return Repair::Late;
        }
        if stop(poll.min(left)) {
            return Repair::Cancelled;
        }
        if admin.back() {
            return Repair::Back;
        }
    }
}

/// Échec de la tentative qui suit le redémarrage : s'il est de ceux que le
/// redémarrage devait réparer, la raison dit qu'il n'a pas suffi.
fn after_restart(f: Failure) -> Failure {
    match f.cause {
        Some(_) => Failure { text: t("session.after_station_restart", &[("reason", &f.text)]), cause: f.cause },
        None => f,
    }
}

/// Le signe de vie d'un invité refusé : 404 (`unknown_guest`), la Station
/// ne le connaît plus (partage fermé, ou retrait passé depuis plus de 60 s :
/// avant, elle répond 200 avec la fin). Le reste (503 `unavailable` : Kyber
/// sans clé de jetons ; réseau) est passager : on réessaie.
fn share_gone(status: u16) -> Option<ShareEnd> {
    (status == 404).then_some(ShareEnd::Stopped)
}

/// Sans signe de vie réussi pendant ce délai, la Station a fait sortir
/// l'invité (docs/kybernow-share.md).
const GUEST_BEAT_LIMIT: Duration = Duration::from_secs(20);

/// Ce que fait le fil du signe de vie d'un invité d'une réponse.
#[derive(Debug, PartialEq)]
enum BeatOutcome {
    /// La réponse, au fil de session.
    Reply(ShareBeat),
    /// La Station ne connaît plus l'invité.
    Gone(ShareEnd),
    /// Passager (503, réseau) : nouvel essai au prochain signe de vie.
    Retry,
    /// Passager depuis [`GUEST_BEAT_LIMIT`] : la session finit (« le
    /// partage ne répond plus »).
    Lost,
}

/// `since_ok` : depuis le dernier signe de vie réussi (ou le départ).
fn beat_outcome(r: Result<ShareBeat, ConsoleError>, since_ok: Duration) -> BeatOutcome {
    match r {
        Ok(b) => BeatOutcome::Reply(b),
        Err(ConsoleError::Http { status, .. }) if share_gone(status).is_some() => BeatOutcome::Gone(share_gone(status).expect("fin")),
        Err(_) if since_ok >= GUEST_BEAT_LIMIT => BeatOutcome::Lost,
        Err(_) => BeatOutcome::Retry,
    }
}

/// Un changement de qualité d'image d'un invité : `call` le demande à Admin ;
/// refusé parce que trop rapproché (plus d'un par seconde), il est réessayé
/// une fois après le délai (`sleep`), si `still_wanted` (session en cours, et
/// rien de plus récent demandé). Rien n'est rendu à l'app en cas d'échec :
/// l'image reçue reste dite par le signe de vie.
fn ask_quality(
    mut call: impl FnMut() -> Result<ShareImage, crate::console::QualityRefusal>,
    mut sleep: impl FnMut(Duration),
    still_wanted: impl Fn() -> bool,
) -> Option<ShareImage> {
    use crate::console::QualityRefusal;
    for attempt in 0..2 {
        match call() {
            Ok(image) => return Some(image),
            Err(QualityRefusal::TooSoon(wait)) if attempt == 0 => {
                info!("qualité d'image : trop tôt, nouvel essai dans {} s", wait.as_secs());
                sleep(wait);
                if !still_wanted() {
                    info!("qualité d'image : une autre a été demandée entre-temps");
                    return None;
                }
            }
            Err(QualityRefusal::TooSoon(_)) => warn!("qualité d'image pas changée : toujours trop tôt"),
            Err(QualityRefusal::Failed(e)) => {
                warn!("qualité d'image pas changée : {}", e.text(Lang::Fr));
                return None;
            }
        }
    }
    None
}

/// L'écran d'un invité dans la liste de Kyber : celui que la Station donne
/// (`id`), ou le seul écran utilisable que Kyber annonce (il ne laisse à un
/// invité que l'écran du partage).
fn share_display(id: u32, displays: &[Display]) -> Option<Display> {
    let list: Vec<&Display> = displays.iter().filter(|d| usable(d)).collect();
    list.iter().find(|d| d.id == id).or(if list.len() == 1 { list.first() } else { None }).map(|d| (*d).clone())
}

/// Un écran secondaire sans données au bout de ce délai, alors que le
/// principal en reçoit, est retiré de la session.
const MUTED_AFTER: Duration = Duration::from_secs(4);

/// Les écrans de l'appareil des flux secondaires restés sans données, quand
/// le principal en reçoit (vide sinon : le principal a ses propres règles).
fn muted_streams(streams: &[StreamScreen], first_data: &[Arc<AtomicBool>]) -> Vec<usize> {
    let has = |k: usize| first_data.get(k).is_some_and(|f| f.load(Ordering::Relaxed));
    if streams.len() < 2 || !has(0) {
        return Vec::new();
    }
    (1..streams.len()).filter(|&k| !has(k)).map(|k| streams[k].screen).collect()
}

/// Écrans en place d'après la réponse de la Station : pour chaque écran du
/// plan créé (même ordre que la demande), l'écran de l'appareil et ce qui est
/// appliqué. Vide sans multi-écrans, avec une Station qui ne gère qu'un écran
/// (pas de `screens` dans la réponse), ou si l'écran principal manque.
fn targets(plan: &[PlannedScreen], prepared: &Prepared) -> Vec<(usize, ScreenMode)> {
    if plan.is_empty() {
        return Vec::new();
    }
    let Some(list) = &prepared.screens else { return Vec::new() };
    let out: Vec<(usize, ScreenMode)> =
        list.iter().zip(plan).filter_map(|(m, p)| m.map(|m| (p.device, ScreenMode { primary: p.primary, ..m }))).collect();
    if out.iter().any(|(_, m)| m.primary) { out } else { Vec::new() }
}

/// Multi-écrans : chaque écran en place retrouvé dans la liste de Kyber par
/// sa position et sa taille (à défaut, par sa taille seule si un seul écran
/// libre l'a), principal en premier ; ceux qui n'y sont pas encore manquent.
fn match_screens(targets: &[(usize, ScreenMode)], displays: &[Display]) -> Vec<StreamScreen> {
    let list: Vec<&Display> = displays.iter().filter(|d| usable(d)).collect();
    let mut used: Vec<u32> = Vec::new();
    let mut found: Vec<Option<StreamScreen>> = vec![None; targets.len()];
    let screen = |device: usize, m: &ScreenMode, d: &Display| StreamScreen { screen: device, display: d.clone(), refresh: m.refresh, primary: m.primary };
    for (k, (device, m)) in targets.iter().enumerate() {
        if let Some(d) =
            list.iter().find(|d| !used.contains(&d.id) && d.x == m.x && d.y == m.y && d.width as u32 == m.width && d.height as u32 == m.height)
        {
            used.push(d.id);
            found[k] = Some(screen(*device, m, d));
        }
    }
    for (k, (device, m)) in targets.iter().enumerate() {
        if found[k].is_some() {
            continue;
        }
        let same: Vec<&&Display> = list.iter().filter(|d| !used.contains(&d.id) && d.width as u32 == m.width && d.height as u32 == m.height).collect();
        if let [d] = same.as_slice() {
            used.push(d.id);
            found[k] = Some(screen(*device, m, d));
        }
    }
    let mut out: Vec<StreamScreen> = found.into_iter().flatten().collect();
    out.sort_by_key(|s| !s.primary);
    out
}

/// Écrans de l'appareil que l'ordinateur n'a pas pu créer : les écrans
/// retirés parce que muets, et ceux du plan restés sans flux quand la
/// Station en a créé plusieurs (refusés, ou jamais annoncés par Kyber).
/// Station à un écran, ou principal seul accepté : rien d'autre, la session
/// est sur l'écran principal seul (les fenêtres des autres ne s'ouvrent pas).
fn not_created(plan: &[PlannedScreen], targets: &[(usize, ScreenMode)], streams: &[StreamScreen], muted: &[usize]) -> Vec<usize> {
    let mut out: Vec<usize> = muted.to_vec();
    if targets.len() > 1 {
        out.extend(plan.iter().map(|p| p.device).filter(|d| !streams.iter().any(|s| s.screen == *d)));
    }
    out.sort_unstable();
    out.dedup();
    out
}

/// Le plan d'une session multi-écrans pour ce profil (`params`), sans les
/// écrans retirés : vide quand la session ne prend qu'un écran
/// ([`screens::session_plan`]).
fn wanted_plan(screens: &Screens, params: &SessionParams, excluded: &[usize]) -> Vec<PlannedScreen> {
    if screens::session_plan_for(screens, &params.settings, params.virtual_display).is_none() {
        return Vec::new();
    }
    screens::plan_excluding(screens, params.settings.codec, excluded)
}

/// Plusieurs ordinateurs à la fois : ce que la session tient d'un démarrage
/// de l'image à l'autre (active ou non, son coupé ou non, le nom de
/// l'ordinateur au journal) et ce à quoi elle l'applique : la chaîne
/// d'entrées de l'image en cours (créée par la plateforme dans
/// [`Platform::input_pipeline`], retrouvée par le cœur) et les moteurs de son
/// (celui que rend [`Platform::audio_handler`], ou donnés par
/// [`Session::attach_audio`]).
pub(crate) struct FocusState {
    active: AtomicBool,
    muted: AtomicBool,
    /// KyberNow Share : ce que les chaînes d'entrées laissent partir (tout
    /// pour un hôte).
    rights: Mutex<InputRights>,
    label: Arc<Label>,
    input: Mutex<Weak<InputPipeline>>,
    audio: Mutex<Vec<Weak<AudioEngine>>>,
}

impl FocusState {
    pub(crate) fn new() -> Arc<Self> {
        Arc::new(Self {
            active: AtomicBool::new(true),
            muted: AtomicBool::new(false),
            rights: Mutex::new(InputRights::ALL),
            label: Label::new(),
            input: Mutex::new(Weak::new()),
            audio: Mutex::new(Vec::new()),
        })
    }

    pub(crate) fn set_active(&self, active: bool) {
        // Sous le verrou de la chaîne : une chaîne qui arrive en même temps
        // reçoit la nouvelle valeur.
        let input = self.input.lock().unwrap();
        if self.active.swap(active, Ordering::SeqCst) != active {
            log::info!("{}session {}", self.label.prefix(), if active { "active" } else { "en arrière-plan" });
        }
        if let Some(i) = input.upgrade() {
            i.set_active(active);
        }
    }

    pub(crate) fn is_active(&self) -> bool {
        self.active.load(Ordering::SeqCst)
    }

    pub(crate) fn set_muted(&self, muted: bool) {
        let mut audio = self.audio.lock().unwrap();
        if self.muted.swap(muted, Ordering::SeqCst) != muted {
            log::info!("{}son {}", self.label.prefix(), if muted { "coupé" } else { "rendu" });
        }
        audio.retain(|w| w.strong_count() > 0);
        for engine in audio.iter().filter_map(Weak::upgrade) {
            engine.set_muted(muted);
        }
    }

    pub(crate) fn is_muted(&self) -> bool {
        self.muted.load(Ordering::SeqCst)
    }

    /// Les droits des chaînes d'entrées de la session (KyberNow Share),
    /// appliqués à la chaîne en cours et à chaque chaîne suivante.
    pub(crate) fn set_rights(&self, rights: InputRights) {
        let input = self.input.lock().unwrap();
        *self.rights.lock().unwrap() = rights;
        if let Some(i) = input.upgrade() {
            i.set_rights(rights);
        }
    }

    pub(crate) fn rights(&self) -> InputRights {
        *self.rights.lock().unwrap()
    }

    pub(crate) fn label(&self) -> &Arc<Label> {
        &self.label
    }

    /// La chaîne d'entrées d'un démarrage de l'image : elle prend l'état de
    /// la session, reçoit les manettes du lecteur du cœur, et ses lignes du
    /// journal portent le nom de l'ordinateur.
    pub(crate) fn attach_input(&self, input: &Arc<InputPipeline>) {
        if !Arc::ptr_eq(input.label(), &self.label) && input.label().name().is_empty() {
            input.label().set(&self.label.name());
        }
        {
            let mut slot = self.input.lock().unwrap();
            input.set_rights(self.rights());
            input.set_active(self.is_active());
            *slot = Arc::downgrade(input);
        }
        crate::gamepads::global().attach(input);
    }

    /// Un moteur de son de la session : il prend l'état « son coupé ».
    pub(crate) fn attach_audio(&self, engine: &Arc<AudioEngine>) {
        let mut audio = self.audio.lock().unwrap();
        engine.set_muted(self.is_muted());
        audio.retain(|w| w.strong_count() > 0);
        if !audio.iter().any(|w| w.as_ptr() == Arc::as_ptr(engine)) {
            audio.push(Arc::downgrade(engine));
        }
    }
}

pub struct Session {
    tx: Mutex<Sender<Msg>>,
    latency: Arc<LatencyTracker>,
    has_image: Arc<AtomicBool>,
    hidden: Arc<AtomicBool>,
    thread: Mutex<Option<JoinHandle<()>>>,
    focus: Arc<FocusState>,
}

impl Session {
    /// Une session à un écran (celui de `params.mode`).
    pub fn start(params: SessionParams, platform: Arc<dyn Platform>) -> Arc<Session> {
        Self::launch(params, None, platform)
    }

    /// Une session d'une app qui gère le multi-écrans (docs/multi-ecrans.md) :
    /// le cœur décide du nombre d'écrans d'après `screens` (réglage, écrans
    /// que la Station sait créer) et le profil de la session
    /// ([`crate::screens::session_plan`]), au départ et quand le profil change
    /// ([`Self::set_settings`]). Plusieurs écrans : un écran virtuel par écran
    /// (quatre au plus, disposés comme sur l'appareil), une entrée vidéo par
    /// écran. Un seul : la session à un écran sur « Cet écran »
    /// (`screens.current`, au mode `params.mode`). Dans les deux cas, la
    /// plateforme reçoit les écrans pris par [`Platform::on_screens`] et les
    /// paquets de chacun par [`Platform::screen_video_handler`].
    pub fn start_with_screens(params: SessionParams, screens: Screens, platform: Arc<dyn Platform>) -> Arc<Session> {
        Self::launch(params, Some(screens), platform)
    }

    fn launch(params: SessionParams, screens: Option<Screens>, platform: Arc<dyn Platform>) -> Arc<Session> {
        let (tx, rx) = mpsc::channel();
        let latency = Arc::new(LatencyTracker::new());
        let has_image = Arc::new(AtomicBool::new(false));
        let hidden = Arc::new(AtomicBool::new(false));
        let focus = FocusState::new();
        let worker = Worker::new(params, screens, platform, tx.clone(), rx, latency.clone(), has_image.clone(), hidden.clone(), focus.clone());
        let guard_platform = worker.platform.clone();
        let label = focus.label().clone();
        let thread = std::thread::Builder::new()
            .name("kybernow-session".into())
            .spawn(move || {
                // Les lignes du journal de la session portent le nom de son ordinateur.
                journal::enter(label);
                // Une panique ne doit pas laisser l'app attendre une fin qui
                // ne viendrait jamais.
                if std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| worker.run())).is_err() {
                    guard_platform.on_status(&Status::Ended {
                        error: Some(t("session.internal", &[])),
                        report: None,
                    });
                }
            })
            .expect("fil de session");
        Arc::new(Session {
            tx: Mutex::new(tx),
            latency,
            has_image,
            hidden,
            thread: Mutex::new(Some(thread)),
            focus,
        })
    }

    // Plusieurs ordinateurs à la fois (docs/plusieurs-ordinateurs.md). Le
    // registre `crate::focus` s'en sert ; une app à une session n'a rien à
    // appeler : elle est active, son son n'est pas coupé.

    /// La session devient active (`true`) ou passe derrière une autre. Pas
    /// active : rien ne part de l'appareil vers son ordinateur (clavier,
    /// souris, texte, touches et axes des manettes), ce qu'elle tenait est
    /// relâché, ses manettes sont mises au repos, la souris relative est
    /// coupée, le presse-papiers est en pause (rien lu sur l'appareil, rien
    /// écrit dessus) et les vibrations sont ignorées. De nouveau active : le
    /// contenu du presse-papiers de l'appareil est renvoyé à l'ordinateur.
    /// Valable pour toute la session : la chaîne d'entrées de chaque
    /// démarrage de l'image prend cet état. Active par défaut.
    pub fn set_active(&self, active: bool) {
        self.focus.set_active(active);
    }

    pub fn is_active(&self) -> bool {
        self.focus.is_active()
    }

    /// Son de la session coupé (`true`) ou rendu, avec un fondu de 20 ms ; il
    /// continue d'être décodé et revient sans décalage. Valable pour toute la
    /// session : chaque moteur de son (celui que rend
    /// [`Platform::audio_handler`] quand c'est un [`AudioEngine`], ou un
    /// gestionnaire qui le rend par `PacketHandler::audio_engine`, ou celui
    /// donné à [`Self::attach_audio`]) prend cet état. Pas coupé par défaut.
    pub fn set_muted(&self, muted: bool) {
        self.focus.set_muted(muted);
    }

    pub fn is_muted(&self) -> bool {
        self.focus.is_muted()
    }

    /// Un moteur de son de la session que le cœur ne voit pas passer (le pont
    /// Apple : le son va à Swift, qui le donne à son `KnAudio`) : il suivra
    /// [`Self::set_muted`]. Sans effet s'il est déjà connu.
    pub fn attach_audio(&self, engine: &Arc<AudioEngine>) {
        self.focus.attach_audio(engine);
    }

    /// Le nom de l'ordinateur, au début des lignes du journal de la session
    /// (« [PC-ATELIER] … ») ; vide : rien, comme avant. [`crate::focus`] le
    /// donne à l'inscription.
    pub fn set_name(&self, name: &str) {
        self.focus.label().set(name);
    }

    pub fn name(&self) -> String {
        self.focus.label().name()
    }

    pub(crate) fn focus_state(&self) -> Arc<FocusState> {
        self.focus.clone()
    }

    fn send(&self, msg: Msg) {
        let _ = self.tx.lock().unwrap().send(msg);
    }

    /// Demande la fin de la session (sans attendre).
    pub fn stop(&self) {
        self.send(Msg::Stop);
    }

    /// Une image clé est demandée une fois (plateforme qui tient sa propre veille).
    pub fn request_keyframe(&self) {
        self.send(Msg::KeyframeNeeded);
    }

    /// Le décodeur a perdu une image de référence : le cœur demande une image
    /// clé toutes les 300 ms jusqu'à [`Self::keyframe_recovered`], et relance
    /// le flux au bout de 1,5 s (ordinateur sans le correctif d'image clé).
    /// KyberNow Share, session d'invité : la « Qualité de l'image » changée
    /// (`POST /api/share/quality`, sur un fil d'appel ; avec « Auto », le
    /// `auto` mesuré à `join`, gardé dans `joined.auto`) ; Kyber relance
    /// l'image de l'invité seulement, la session n'a rien à relancer. L'image reçue
    /// arrive par [`Platform::on_share_image`]. Sans effet pour un hôte.
    pub fn set_share_quality(&self, quality: ShareQuality) {
        self.send(Msg::ShareQuality(quality));
    }

    pub fn keyframe_lost(&self) {
        self.send(Msg::KeyframeLost);
    }

    pub fn keyframe_recovered(&self) {
        self.send(Msg::KeyframeRecovered);
    }

    /// Aucune image depuis le démarrage du flux : il est relancé (ce qui
    /// relance aussi le service vidéo de Kyber sur l'ordinateur), trois fois
    /// au plus, puis la session s'arrête avec la raison (avant la première
    /// image : après le dernier recours, voir [`Recovery::restart_station`]).
    pub fn no_image(&self) {
        self.send(Msg::NoImage);
    }

    /// Change de profil pendant la session (barre) : nouveau débit sur-le-champ,
    /// ou flux relancé si le format, le 4:4:4 ou le transport changent.
    pub fn set_settings(&self, settings: StreamSettings) {
        self.send(Msg::Settings(settings));
    }

    /// Plafond de débit le temps que la fenêtre est occultée ; `None` le retire.
    pub fn set_bitrate_cap(&self, cap: Option<u32>) {
        self.send(Msg::BitrateCap(cap));
    }

    /// L'app a figé l'image (arrière-plan, fenêtre cachée) : aucune image
    /// n'est montrée, et ce n'est pas une panne. La veille « aucune image en
    /// 4 s » attend le retour au lieu de relancer le flux puis de couper.
    pub fn set_hidden(&self, hidden: bool) {
        self.hidden.store(hidden, Ordering::Relaxed);
    }

    /// Taille des images décodées par l'app : un changement en cours de flux
    /// dit que l'ordinateur a changé de mode (voir `decoded`).
    pub fn decoded_size(&self, width: u32, height: u32) {
        self.send(Msg::DecodedSize(width, height));
    }

    /// Sans écran virtuel : afficher un autre écran de l'ordinateur.
    pub fn pick_display(&self, id: u32) {
        self.send(Msg::PickDisplay(id));
    }

    /// L'écran de l'appareil a changé de mode : la console refait l'écran
    /// virtuel à ce mode, puis l'image repart dessus.
    pub fn set_mode(&self, mode: Mode) {
        self.send(Msg::SetMode(mode));
    }

    /// « Remettre » : la console remet le mode demandé au début de la session.
    pub fn restore_mode(&self) {
        self.send(Msg::RestoreMode);
    }

    /// Multi-écrans : les écrans de l'appareil ont changé (branché,
    /// débranché, disposition ou recopie changée) : la session se refait
    /// avec eux, sans revenir à l'accueil (sans effet si le plan est le
    /// même, ni sur une session ouverte par [`Self::start`]).
    pub fn set_screens(&self, screens: Screens) {
        self.send(Msg::SetScreens(screens));
    }

    /// Image `pts` reçue, décodée puis montrée (µs, horloge
    /// [`crate::latency::now_micros`]) : latence, et première image du flux.
    /// Multi-écrans : seulement pour l'écran principal.
    pub fn frame_displayed(&self, pts: u64, received: i64, decoded: i64, at: i64) {
        self.has_image.store(true, Ordering::Relaxed);
        self.latency.frame_displayed(pts, received, decoded, at);
    }

    pub fn latency(&self) -> Arc<LatencyTracker> {
        self.latency.clone()
    }

    /// Attend la fin du fil de session (console prévenue).
    pub fn wait(&self) {
        if let Some(t) = self.thread.lock().unwrap().take() {
            let _ = t.join();
        }
    }
}

struct Worker {
    params: SessionParams,
    platform: Arc<dyn Platform>,
    tx: Sender<Msg>,
    rx: Receiver<Msg>,
    latency: Arc<LatencyTracker>,
    has_image: Arc<AtomicBool>,
    /// Image figée par l'app ([`Session::set_hidden`]).
    hidden: Arc<AtomicBool>,
    console: Option<Arc<Console>>,
    client: Option<Arc<Client>>,
    prepared: Option<Prepared>,
    /// Mode demandé à la console (redemandé si l'écran virtuel disparaît ;
    /// change avec l'adaptation à l'écran de l'appareil).
    requested: Mode,
    /// Mode de l'écran de la session (obtenu de la console, puis suivi quand
    /// l'ordinateur en change) : pour le débit et l'étiquette de la barre.
    stream_mode: Mode,
    displays: Vec<Display>,
    streaming: Option<Display>,
    last_shown: Option<Display>,
    /// Chaîne d'entrées de l'image en cours (relâchement général à la fin).
    pipeline: Option<Pipeline>,
    /// Attente que Kyber n'annonce plus que l'écran virtuel.
    wait_until: Option<Instant>,
    /// Démarrage de l'image prévu (relance).
    start_at: Option<Instant>,
    retries: u32,
    /// Relances faute d'image (jamais remis à zéro dans la session).
    no_image: u32,
    /// Dernier écran redemandé parce que la console le disait parti.
    last_restore: Option<Instant>,
    /// Profil et règle de mode annoncés par le signal de vie (ils peuvent changer en session).
    profile_label: Arc<Mutex<(String, String)>>,
    last_retry: Option<Instant>,
    reconnected_at: Option<Instant>,
    restored_after_reconnect: bool,
    ending: bool,
    last_latency: Arc<Mutex<Option<f64>>>,
    device_id: String,
    alive: Arc<AtomicBool>,
    /// Repli de cette session : 1 = débit plafonné, 2 = 4:2:0 en plus ; et
    /// celui du flux en cours (Kyber relance l'ancien après une reconnexion).
    fallback: u8,
    streaming_fallback: u8,
    /// Première image attendue jusqu'à cet instant.
    image_deadline: Option<Instant>,
    last_stall_restart: Option<Instant>,
    /// Image clé demandée (première demande, dernière demande).
    keyframe: Option<(Instant, Instant)>,
    /// Taille de l'écran de l'ordinateur dans le repère des entrées, celle des
    /// dernières images décodées, et celle que la liste de Kyber donne à l'écran.
    host_size: (u32, u32),
    frame_size: Option<(u32, u32)>,
    list_size: (u32, u32),
    bitrate_cap: Option<u32>,
    /// Écran choisi dans la barre, tant qu'il existe.
    picked: Option<u32>,
    /// « Remettre W × H » proposé en ce moment.
    restore_offer: Option<Mode>,
    last_mode_restore: Option<Instant>,
    /// Après « Remettre » : la proposition revient si rien n'a changé.
    mode_restore_until: Option<Instant>,
    mode_restoring: bool,
    /// Écran virtuel en cours de recréation à un autre mode.
    adapting: bool,
    /// Capsule à montrer au prochain démarrage de l'image.
    pending_notice: Option<Notice>,
    relaunched: bool,
    /// Admin a répondu à `prepare` pendant cette tentative.
    prepared_ok: bool,
    /// Admin a demandé la fin de la session (signal de vie).
    end_requested: bool,
    /// La Station a été redémarrée pour cette connexion (une fois au plus).
    station_restarted: bool,
    /// Fin déjà dite à Admin avant le redémarrage de la Station, quand la
    /// session s'arrête là (refus, retard, annulation).
    end_report: Option<EndReport>,
    /// Multi-écrans : les écrans de l'appareil (`None` : session à un écran,
    /// API d'avant) et les écrans virtuels à demander (le principal en
    /// premier ; `screens` n'est envoyé qu'à partir de deux).
    screens: Option<Screens>,
    plan: Vec<PlannedScreen>,
    /// Écrans en place d'après la Station : écran de l'appareil, mode et
    /// position (vide : Station à un écran, ou pas de multi-écrans).
    targets: Vec<(usize, ScreenMode)>,
    /// Écrans du flux en cours (principal en premier) et leurs premières données.
    streams: Vec<StreamScreen>,
    first_data: Vec<Arc<AtomicBool>>,
    keyframe_held_logged: bool,
    /// Écrans de l'appareil arrivés pendant une adaptation.
    pending_screens: Option<Screens>,
    /// Mode d'une session à un écran (« Cet écran ») : `params.mode`, puis
    /// celui d'une adaptation ; repris quand le plan revient à un écran.
    single_mode: Mode,
    /// La Station a préparé la session sans écran virtuel, ou sans `screens`
    /// (Station à un écran) : un seul écran, « Cet écran », pour toute la
    /// session (le plan reste vide, même si le profil ou les écrans changent).
    station_single: bool,
    /// Une session refaite demandée pendant une préparation en cours
    /// (adaptation ou autre session refaite) : lancée à sa fin ; et si Kyber
    /// devra être reconnecté.
    pending_replan: Option<bool>,
    /// Écrans de l'appareil retirés en session : muets (ils restent « non
    /// créés » pour la plateforme), ou refusés par Kyber au démarrage.
    excluded: Vec<usize>,
    muted: Vec<usize>,
    /// Nouvel essai avec l'écran principal seul déjà fait (refus de Kyber).
    primary_only_tried: bool,
    /// Kyber à reconnecter une fois la Station préparée (après son refus).
    reconnect_after_prepare: bool,
    /// Quand regarder si un écran secondaire est resté muet.
    mute_check_at: Option<Instant>,
    /// Plusieurs ordinateurs : état « active » et « son coupé » de la session.
    focus: Arc<FocusState>,
    /// KyberNow Share : l'invité (`None` : session d'hôte), son rôle en
    /// cours, la fin que le partage a décidée, ce qui lui a été refusé.
    guest: Option<GuestParams>,
    share_role: ShareRole,
    share_end: Option<(ShareEnd, String)>,
    refused: Mutex<HashSet<HostOnly>>,
    /// L'image en cours a une chaîne d'entrées (un spectateur n'en a pas).
    stream_input: bool,
    /// Invité reconnecté à Kyber pour un autre écran du partage : l'arrêt
    /// demandé par le cœur lui-même n'est pas une coupure.
    share_reconnecting: bool,
    /// L'image que l'invité reçoit, d'après la Station.
    share_image: Option<ShareImage>,
    /// Changements de qualité demandés (le dernier gagne).
    quality_asked: Arc<std::sync::atomic::AtomicU64>,
    /// Session d'hôte : le partage du dernier signe de vie.
    last_share: Option<ShareState>,
}

/// Raison d'une fin de session, dans la langue de l'app (montrée à l'utilisateur).
fn t(key: &str, params: &[(&str, &str)]) -> String {
    i18n::tr(i18n::current(), key, params)
}

fn usable(d: &Display) -> bool {
    d.width > 0 && d.height > 0
}

impl Worker {
    #[allow(clippy::too_many_arguments)]
    fn new(
        params: SessionParams,
        screens: Option<Screens>,
        platform: Arc<dyn Platform>,
        tx: Sender<Msg>,
        rx: Receiver<Msg>,
        latency: Arc<LatencyTracker>,
        has_image: Arc<AtomicBool>,
        hidden: Arc<AtomicBool>,
        focus: Arc<FocusState>,
    ) -> Self {
        let profile_label = Arc::new(Mutex::new((params.settings.label().to_string(), params.settings.mode_policy().to_string())));
        let guest = match &params.role {
            SessionRole::Guest(g) => Some((**g).clone()),
            SessionRole::Host => None,
        };
        let share_role = guest.as_ref().map(|g| g.joined.role).unwrap_or_default();
        if guest.is_some() {
            // Un invité n'a d'entrées que selon son rôle, jamais le presse-papiers.
            focus.set_rights(share_role.rights());
        }
        // Un invité : l'écran du partage, jamais de plan.
        let plan = match (&screens, &guest) {
            (Some(s), None) => wanted_plan(s, &params, &[]),
            _ => Vec::new(),
        };
        // Le mode de l'écran principal, d'après le plan.
        let mode = plan.first().map_or(params.mode, |p| p.mode);
        let single_mode = params.mode;
        Worker {
            requested: mode,
            stream_mode: mode,
            device_id: params.device.id.clone(),
            params,
            platform,
            tx,
            rx,
            latency,
            has_image,
            hidden,
            console: None,
            client: None,
            prepared: None,
            displays: Vec::new(),
            streaming: None,
            last_shown: None,
            pipeline: None,
            wait_until: None,
            start_at: None,
            retries: 0,
            no_image: 0,
            last_restore: None,
            profile_label,
            last_retry: None,
            reconnected_at: None,
            restored_after_reconnect: false,
            ending: false,
            last_latency: Arc::new(Mutex::new(None)),
            alive: Arc::new(AtomicBool::new(true)),
            fallback: 0,
            streaming_fallback: 0,
            image_deadline: None,
            last_stall_restart: None,
            keyframe: None,
            host_size: (0, 0),
            frame_size: None,
            list_size: (0, 0),
            bitrate_cap: None,
            picked: None,
            restore_offer: None,
            last_mode_restore: None,
            mode_restore_until: None,
            mode_restoring: false,
            adapting: false,
            pending_notice: None,
            relaunched: false,
            prepared_ok: false,
            end_requested: false,
            station_restarted: false,
            end_report: None,
            screens,
            plan,
            targets: Vec::new(),
            streams: Vec::new(),
            first_data: Vec::new(),
            keyframe_held_logged: false,
            pending_screens: None,
            single_mode,
            station_single: guest.is_some(),
            pending_replan: None,
            excluded: Vec::new(),
            muted: Vec::new(),
            primary_only_tried: false,
            reconnect_after_prepare: false,
            mute_check_at: None,
            focus,
            share_role,
            share_end: None,
            refused: Mutex::new(HashSet::new()),
            stream_input: false,
            share_reconnecting: false,
            share_image: None,
            quality_asked: Arc::new(std::sync::atomic::AtomicU64::new(0)),
            last_share: None,
            guest,
        }
    }

    fn run(mut self) {
        let failure = match self.attempt() {
            Some(f) if self.restart_wanted(&f) => self.last_resort(f),
            other => other,
        };
        let failure = failure.map(|f| self.share_reason(f));
        self.finish(failure.map(|f| f.text));
    }

    /// Session d'invité qui échoue sans que le partage l'ait dit (Kyber coupé
    /// par Admin quand l'hôte retire l'invité ou arrête le partage, avant le
    /// signe de vie suivant) : la Station est interrogée une fois, vite ; si
    /// c'est le partage qui a fini, sa raison remplace celle de Kyber.
    fn share_reason(&mut self, f: Failure) -> Failure {
        if self.share_end.is_some() {
            return f;
        }
        let (Some(g), Some(console)) = (self.guest.clone(), self.console.clone()) else { return f };
        let end = match console.share_beat_within(&g.joined.guest, &g.joined.secret, Duration::from_secs(3)) {
            Ok(b) => b.end,
            Err(ConsoleError::Http { status, .. }) => share_gone(status),
            Err(_) => None,
        };
        match end {
            Some(end) => {
                info!("la session finit par le partage ({})", f.text);
                self.share_ended(end)
            }
            None => f,
        }
    }

    /// Une tentative de connexion : préparation, Kyber, puis la session
    /// jusqu'à sa fin ; `None` : fin voulue (app, Admin).
    fn attempt(&mut self) -> Option<Failure> {
        match self.open() {
            Ok(()) => self.event_loop(),
            Err(e) => Some(e),
        }
    }

    /// KyberNow Share : vrai pour une session d'hôte ; pour un invité,
    /// l'action est fermée (et notée) : il ne change jamais l'ordinateur.
    fn host_only(&self, action: HostOnly) -> bool {
        if self.guest.is_none() {
            return true;
        }
        if self.refused.lock().unwrap().insert(action) {
            info!("invité : {action:?} jamais demandé à l'ordinateur");
        }
        false
    }

    /// L'image demande une chaîne d'entrées : un hôte, ou un invité joueur
    /// (un spectateur n'en a aucune).
    fn wants_input(&self) -> bool {
        self.guest.is_none() || self.share_role.plays()
    }

    /// Le presse-papiers partagé, jamais pour un invité.
    fn input_clipboard(&self) -> bool {
        self.params.clipboard && self.guest.is_none()
    }

    /// {hôte} des textes du partage.
    fn share_host(&self) -> String {
        self.guest.as_ref().map(|g| g.joined.host_name().to_string()).unwrap_or_default()
    }

    /// Le partage finit la session d'un invité : la raison (texte de link-ui).
    fn share_ended(&mut self, end: ShareEnd) -> Failure {
        let text = end.text(i18n::current(), &self.share_host());
        info!("partage : {}", end.text(Lang::Fr, &self.share_host()));
        self.share_end = Some((end, text.clone()));
        text.into()
    }

    fn restart_wanted(&self, f: &Failure) -> bool {
        restart_wanted(
            f.cause,
            Attempt {
                enabled: self.params.recovery.restart_station && self.guest.is_none(),
                prepared: self.prepared_ok,
                shown: self.shown(),
                end_requested: self.end_requested,
                restarted: self.station_restarted,
            },
        )
    }

    /// Une image a été montrée pendant cette connexion : la session est établie.
    fn shown(&self) -> bool {
        self.latency.any_displayed() || self.has_image.load(Ordering::Relaxed)
    }

    /// Dernier recours de la connexion ([`Recovery::restart_station`]) : la
    /// tentative est close chez Admin (sinon il compterait cet appareil comme
    /// en session et refuserait), la Station redémarrée et attendue, puis la
    /// session refaite depuis le début (écran virtuel recréé), une fois.
    fn last_resort(&mut self, first: Failure) -> Option<Failure> {
        if !self.host_only(HostOnly::StationRestart) {
            return Some(first);
        }
        warn!("connexion sans issue ({}) : redémarrage de la Station en dernier recours", first.text);
        self.station_restarted = true;
        let Some((report, answered)) = self.release(true) else { return Some(first) };
        self.end_report = Some(report);
        if !answered {
            warn!("Admin ne répond plus : la Station n'est pas redémarrée");
            return Some(first);
        }
        // Annulé entre-temps : rien n'est redémarré.
        if self.stop_requested(Duration::ZERO) {
            return None;
        }
        let console = self.console.clone().expect("console");
        let device_id = self.device_id.clone();
        let platform = self.platform.clone();
        let restarting = || {
            info!("session : {:?}", Status::StationRestarting);
            platform.on_status(&Status::StationRestarting);
        };
        match repair(&*console, &device_id, STATION_POLL, STATION_BACK_WITHIN, restarting, |d| self.stop_requested(d)) {
            Repair::Refused(e) => {
                warn!("la Station n'est pas redémarrée : {}", e.text(Lang::Fr));
                Some(first)
            }
            Repair::Cancelled => None,
            Repair::Late => {
                warn!("la Station n'est pas revenue en {} s", STATION_BACK_WITHIN.as_secs());
                Some(t("session.station_not_back", &[]).into())
            }
            Repair::Back => {
                info!("la Station est revenue : la connexion reprend du début");
                self.renew();
                let failure = self.attempt()?;
                // Établie puis perdue plus tard : la raison telle quelle.
                Some(if self.shown() { failure } else { after_restart(failure) })
            }
        }
    }

    /// Attend au plus `wait` une demande de fin pendant le dernier recours
    /// (`Duration::ZERO` : seulement ce qui est déjà arrivé). Le reste de la
    /// tentative close est écarté ; les réglages changés entre-temps sont
    /// gardés pour la suivante.
    fn stop_requested(&mut self, wait: Duration) -> bool {
        let until = Instant::now() + wait;
        loop {
            match self.rx.recv_timeout(until.saturating_duration_since(Instant::now())) {
                Ok(Msg::Stop) | Err(RecvTimeoutError::Disconnected) => return true,
                Ok(Msg::Beat(r)) if r.end => {
                    self.end_requested = true;
                    return true;
                }
                Ok(Msg::Settings(s)) => self.params.settings = s,
                Ok(Msg::BitrateCap(cap)) => self.bitrate_cap = cap,
                Ok(_) => {}
                Err(RecvTimeoutError::Timeout) => return false,
            }
        }
    }

    /// Nouvelle tentative après le retour de la Station : tout ce qui tenait
    /// à la précédente repart de zéro (mêmes paramètres, plateforme, canal,
    /// mesures ; plafond de débit de la fenêtre gardé).
    fn renew(&mut self) {
        let rx = std::mem::replace(&mut self.rx, mpsc::channel().1);
        let mut next = Worker::new(
            self.params.clone(),
            self.screens.clone(),
            self.platform.clone(),
            self.tx.clone(),
            rx,
            self.latency.clone(),
            self.has_image.clone(),
            self.hidden.clone(),
            self.focus.clone(),
        );
        next.bitrate_cap = self.bitrate_cap;
        next.station_restarted = true;
        *self = next;
    }

    fn status(&self, s: Status) {
        info!("session : {s:?}");
        self.platform.on_status(&s);
    }

    fn notice(&self, n: Notice) {
        info!("avis : {n:?}");
        self.platform.on_notice(&n);
    }

    fn prepare_request(&self, mode: Mode, mode_policy: Option<&str>) -> PrepareRequest {
        self.prepare_request_with(mode, mode_policy, &self.plan)
    }

    /// `plan` : les écrans virtuels demandés (vide : session à un écran, sans
    /// `screens` ; `mode` est sinon celui du principal). Un plan d'un seul
    /// écran (les autres retirés en session) part avec `screens` à une entrée.
    fn prepare_request_with(&self, mode: Mode, mode_policy: Option<&str>, plan: &[PlannedScreen]) -> PrepareRequest {
        let p = &self.params;
        // Un seul verrou : le prendre deux fois dans la même expression bloquait le fil.
        let (label, policy) = self.profile_label.lock().unwrap().clone();
        PrepareRequest {
            width: mode.width,
            height: mode.height,
            refresh: mode.refresh,
            audio_channels: p.audio_channels,
            virtual_display: p.virtual_display,
            device: p.device.clone(),
            profile: Some(label),
            mode_policy: Some(mode_policy.map_or(policy, str::to_string)),
            screens: (!plan.is_empty()).then(|| plan.iter().map(PlannedScreen::request).collect()),
        }
    }

    fn prepare(&mut self) -> Result<(), String> {
        if !self.host_only(HostOnly::Prepare) {
            return Ok(());
        }
        let console = self.console.clone().expect("console");
        let req = self.prepare_request(self.requested, None);
        let prepared = console.prepare(&req).map_err(|e| e.to_string())?;
        info!(
            "console : écran {:?}, son {:?}, port {:?}",
            prepared.applied, prepared.audio_channels, prepared.kyber_port
        );
        self.prepared_ok = true;
        self.apply_prepared(prepared);
        Ok(())
    }

    fn apply_prepared(&mut self, prepared: Prepared) {
        if let Some(m) = prepared.applied {
            self.stream_mode = m;
        } else {
            self.stream_mode = self.requested;
        }
        self.platform.on_mode(self.stream_mode, prepared.virtual_display);
        // Préparée sans écran virtuel (pilote absent, déclenchement coupé) ou
        // sans `screens` (Station à un écran) : un seul écran, « Cet écran »,
        // comme une session à un écran, pour le reste de la session.
        if !self.plan.is_empty() && (!prepared.virtual_display || prepared.screens.is_none()) {
            info!(
                "la Station a préparé {} : session sur un écran",
                if prepared.virtual_display { "un seul écran" } else { "sans écran virtuel" }
            );
            self.station_single = true;
            self.plan.clear();
        }
        self.targets = targets(&self.plan, &prepared);
        if !self.plan.is_empty() {
            match &prepared.screens {
                None => {}
                Some(_) if self.targets.is_empty() => warn!("la Station n'a pas créé l'écran principal : session à un écran"),
                Some(list) => info!("écrans en place : {} sur {} ({:?})", self.targets.len(), list.len(), self.targets),
            }
        }
        self.prepared = Some(prepared);
    }

    /// Session ouverte par [`Session::start_with_screens`].
    fn multi(&self) -> bool {
        self.screens.is_some()
    }

    /// Les écrans du flux à la plateforme, avec ceux que l'ordinateur n'a
    /// pas pu créer (écrans du plan sans flux, quand la Station gère le
    /// multi-écrans).
    fn announce_screens(&self) {
        let not_created = not_created(&self.plan, &self.targets, &self.streams, &self.muted);
        if !not_created.is_empty() {
            info!("écrans que l'ordinateur n'a pas pu créer : {not_created:?}");
        }
        self.platform.on_screens(&self.streams, &not_created);
    }

    /// L'écran de l'appareil de l'écran principal de la session (index dans
    /// [`Screens::list`]) : celui du plan, ou « Cet écran » pour une session
    /// à un écran.
    fn primary_screen(&self) -> usize {
        self.plan.first().map(|p| p.device).or_else(|| self.screens.as_ref().map(Screens::current)).unwrap_or(0)
    }

    /// Le plan voulu pour ces écrans de l'appareil, avec le profil de la
    /// session et sans les écrans retirés.
    fn wanted_plan(&self, screens: &Screens) -> Vec<PlannedScreen> {
        if self.station_single {
            return Vec::new();
        }
        wanted_plan(screens, &self.params, &self.excluded)
    }

    /// Mode obtenu de la console au début de la session (ou après une adaptation).
    fn prepared_size(&self) -> Option<(u32, u32)> {
        self.prepared.as_ref().and_then(|p| p.applied).map(|m| (m.width, m.height))
    }

    fn virtual_display(&self) -> bool {
        self.prepared.as_ref().is_some_and(|p| p.virtual_display)
    }

    fn open(&mut self) -> Result<(), Failure> {
        if self.guest.is_some() {
            return self.open_guest();
        }
        self.status(Status::Preparing);
        let p = self.params.clone();
        let console = Arc::new(Console::new(
            &p.console_host,
            p.console_port,
            &p.user,
            &p.password,
            p.console_fingerprint.clone(),
        ));
        self.console = Some(console.clone());
        self.prepare()?;
        let prepared = self.prepared.clone().expect("prepare");

        self.status(Status::Connecting);
        // Fournisseur de chiffrement par défaut de rustls (kyclient::init_crypto
        // échoue s'il est déjà posé, à la deuxième session par exemple).
        let _ = rustls::crypto::ring::default_provider().install_default();
        let mut config = ConnectConfig {
            host: console.host().to_string(),
            port: prepared.kyber_port.unwrap_or(8080),
            tls_host: None,
            verify_mode: None,
            credentials: AuthCredentials::Basic {
                username: p.user.clone(),
                password: p.password.clone(),
            },
            auto_reconnection: p.auto_reconnect,
        };
        match &prepared.kyber_fingerprint {
            Some(fp) => config.set_tls_fingerprint(fp).map_err(|e| e.to_string())?,
            // Console d'avant 0.2.8 : certificat retenu au premier contact.
            None => config.enable_tofu_auto_accept().map_err(|e| e.to_string())?,
        }
        let client = Client::new(config, true).map_err(|e| format!("Kyber : {e}"))?;
        let tracker = self.latency.clone();
        client.set_metrics_callback(Some(Box::new(move |m| tracker.on_metric(&m))));
        client.get_event_manager().add_event_sink(Box::new(Events {
            tx: Mutex::new(self.tx.clone()),
            alive: self.alive.clone(),
        }));
        let client = Arc::new(client);
        self.client = Some(client.clone());
        client.connect().map_err(|e| format!("Kyber : {e}"))?;

        self.spawn_heartbeat(prepared.heartbeat_seconds);
        self.spawn_foreground();
        Ok(())
    }

    /// Session d'invité : rien n'est préparé sur l'ordinateur. Le format de
    /// l'image de l'hôte est vérifié, l'écran du partage pris pour mode, puis
    /// Kyber est joint avec le jeton de la Station et l'empreinte qu'elle
    /// donne (jamais de premier contact), et le signe de vie de l'invité démarre.
    fn open_guest(&mut self) -> Result<(), Failure> {
        let g = self.guest.clone().expect("invité");
        let j = &g.joined;
        info!(
            "invité sur {} (partage de {}), {:?}, écran {} ({}×{})",
            j.computer,
            j.host_name(),
            j.role,
            j.display.id,
            j.display.width,
            j.display.height
        );
        self.status(Status::Preparing);
        let p = self.params.clone();
        let console = Arc::new(Console::guest(&p.console_host, p.console_port, p.console_fingerprint.clone()));
        self.console = Some(console.clone());
        // Le format de l'hôte s'il est lisible ici (en 4:2:0 sans son 4:4:4),
        // sinon un format lisible : Kyber fait alors une image à part.
        match share::guest_format(j.codec.as_deref(), j.yuv444, g.capabilities) {
            Ok(Some((codec, yuv444))) => {
                if j.codec() != Some(codec) || yuv444 != j.yuv444 {
                    info!("format de l'hôte {:?} (4:4:4 {}) : {} {} demandé", j.codec, j.yuv444, codec.label(), if yuv444 { "4:4:4" } else { "4:2:0" });
                }
                self.params.settings.codec = codec;
                self.params.settings.yuv444 = yuv444;
            }
            // Image pas encore démarrée chez l'hôte : le format de l'app, en 4:2:0.
            Ok(None) => self.params.settings.yuv444 = false,
            Err(ShareError::Unreadable { format }) => return Err(self.share_ended(ShareEnd::Unreadable { format })),
            Err(e) => return Err(e.to_string().into()),
        }
        let Some(fingerprint) = j.kyber.fingerprint.clone().filter(|f| !f.is_empty()) else {
            return Err(t("share.no_kyber_fingerprint", &[]).into());
        };
        let d = j.display;
        let mode = Mode { width: d.width, height: d.height, refresh: d.refresh.max(1) };
        self.requested = mode;
        self.single_mode = mode;
        self.apply_prepared(Prepared {
            virtual_display: false,
            applied: (d.width > 0 && d.height > 0).then_some(mode),
            audio_channels: None,
            kyber_port: Some(j.kyber.port),
            heartbeat_seconds: j.beat.max(1),
            kyber_fingerprint: Some(fingerprint.clone()),
            screens: None,
        });
        self.platform.on_share_role(self.share_role, None);
        if let Some(image) = j.image.clone() {
            self.share_image_changed(image);
        }

        self.status(Status::Connecting);
        let _ = rustls::crypto::ring::default_provider().install_default();
        let mut config = ConnectConfig {
            host: console.host().to_string(),
            port: j.kyber.port,
            tls_host: None,
            verify_mode: None,
            credentials: AuthCredentials::Jwt { token: j.kyber.token.clone() },
            auto_reconnection: p.auto_reconnect,
        };
        config.set_tls_fingerprint(&fingerprint).map_err(|e| e.to_string())?;
        let client = Client::new(config, true).map_err(|e| format!("Kyber : {e}"))?;
        let tracker = self.latency.clone();
        client.set_metrics_callback(Some(Box::new(move |m| tracker.on_metric(&m))));
        client.get_event_manager().add_event_sink(Box::new(Events {
            tx: Mutex::new(self.tx.clone()),
            alive: self.alive.clone(),
        }));
        let client = Arc::new(client);
        self.client = Some(client.clone());
        client.connect().map_err(|e| format!("Kyber : {e}"))?;
        self.spawn_share_beat(j.beat.max(1));
        Ok(())
    }

    /// Signe de vie de l'invité, toutes les `seconds` : rôle, jeton de Kyber,
    /// écran, fin. Une Station qui ne connaît plus l'invité (404) finit la
    /// session ; une Station indisponible (503 : Kyber sans clé de jetons) ou
    /// injoignable est réessayée, 20 s au plus (voir [`beat_outcome`]).
    fn spawn_share_beat(&self, seconds: u64) {
        let Some(g) = self.guest.clone() else { return };
        let console = self.console.clone().expect("console");
        let tx = self.tx.clone();
        let alive = self.alive.clone();
        let label = journal::current();
        std::thread::Builder::new()
            .name("kybernow-partage".into())
            .spawn(move || {
                journal::resume(label);
                let mut last_ok = Instant::now();
                loop {
                    std::thread::sleep(Duration::from_secs(seconds));
                    if !alive.load(Ordering::Relaxed) {
                        return;
                    }
                    let reply = console.share_beat(&g.joined.guest, &g.joined.secret);
                    if !alive.load(Ordering::Relaxed) {
                        return;
                    }
                    let failed = match &reply {
                        Err(e) => Some(e.text(Lang::Fr)),
                        Ok(_) => None,
                    };
                    match beat_outcome(reply, last_ok.elapsed()) {
                        BeatOutcome::Reply(b) => {
                            last_ok = Instant::now();
                            let end = b.end.is_some();
                            let _ = tx.send(Msg::ShareBeat(b));
                            if end {
                                return;
                            }
                        }
                        BeatOutcome::Gone(end) => {
                            warn!("signe de vie de l'invité : la Station ne le connaît plus");
                            let _ = tx.send(Msg::ShareGone(end));
                            return;
                        }
                        BeatOutcome::Retry => warn!("signe de vie de l'invité : {} (nouvel essai)", failed.unwrap_or_default()),
                        BeatOutcome::Lost => {
                            warn!("signe de vie de l'invité : sans réponse depuis {} s", GUEST_BEAT_LIMIT.as_secs());
                            let _ = tx.send(Msg::Fail(t("share.lost", &[]).into()));
                            return;
                        }
                    }
                }
            })
            .expect("fil du signe de vie de l'invité");
    }

    /// Réponse du signe de vie de l'invité : la fin, le jeton de Kyber
    /// renouvelé (pour ses reconnexions), le rôle changé par l'hôte.
    fn on_share_beat(&mut self, b: ShareBeat) -> Option<Failure> {
        // Une fin : rien d'autre (le jeton est nul).
        if let Some(end) = b.end {
            return Some(self.share_ended(end));
        }
        if let Some(token) = b.token.filter(|t| !t.is_empty()) {
            if let Some(g) = self.guest.as_mut() {
                g.joined.kyber.token = token.clone();
            }
            if let Some(c) = &self.client
                && let Err(e) = c.update_credentials(AuthCredentials::Jwt { token })
            {
                warn!("jeton de Kyber pas renouvelé : {e}");
            }
        }
        let mut open_inputs = false;
        if b.role != self.share_role {
            let previous = std::mem::replace(&mut self.share_role, b.role);
            info!("partage : rôle {previous:?} → {:?}", b.role);
            self.focus.set_rights(b.role.rights());
            self.platform.on_share_role(b.role, Some(previous));
            // Un spectateur devient joueur : ses entrées s'ouvrent avec une
            // image neuve (Kyber les ouvre au démarrage de l'image).
            open_inputs = b.role.plays() && !self.stream_input && self.streaming.is_some();
        }
        if let Some(image) = b.image {
            self.share_image_changed(image);
        }
        let current = self.guest.as_ref().map(|g| g.joined.display.id);
        match b.display {
            // L'écran partagé a changé (refait par l'hôte) : Kyber ne change
            // l'écran d'une session qu'à une nouvelle connexion.
            Some(d) if Some(d.id) != current && d.id != 0 => self.switch_share_display(d),
            _ if open_inputs => {
                info!("partage : entrées ouvertes, image relancée");
                self.restart_streaming(Duration::from_millis(300));
            }
            _ => {}
        }
        None
    }

    /// L'écran du partage a changé : l'invité quitte sa session de Kyber et
    /// s'y reconnecte avec le jeton du dernier signe de vie (il porte le
    /// nouvel écran) ; l'image repart sur le nouvel écran (`on_screens`,
    /// premières images, image), sans fin de session.
    fn switch_share_display(&mut self, d: ShareDisplay) {
        let Some(g) = self.guest.as_mut() else { return };
        info!("partage : nouvel écran {} ({}×{}), reconnexion à Kyber", d.id, d.width, d.height);
        g.joined.display = d;
        let token = g.joined.kyber.token.clone();
        let mode = Mode { width: d.width, height: d.height, refresh: d.refresh.max(1) };
        self.requested = mode;
        self.single_mode = mode;
        self.stream_mode = mode;
        if let Some(p) = self.prepared.as_mut() {
            p.applied = (d.width > 0 && d.height > 0).then_some(mode);
        }
        self.platform.on_mode(mode, false);
        self.stop_streaming();
        self.start_at = None;
        self.wait_until = None;
        self.keyframe = None;
        // La liste revient avec la connexion.
        self.displays.clear();
        self.share_reconnecting = true;
        if let Some(c) = &self.client {
            let _ = c.disconnect();
            let _ = c.update_credentials(AuthCredentials::Jwt { token });
            if let Err(e) = c.connect() {
                let _ = self.tx.send(Msg::Fail(Failure::kyber(t("session.connect_failed", &[("reason", &e.to_string())]))));
            }
        }
    }

    /// L'image de l'invité a changé (ou arrive) : à la plateforme, avec l'avis.
    fn share_image_changed(&mut self, image: ShareImage) {
        if self.share_image.as_ref() == Some(&image) {
            return;
        }
        let previous = self.share_image.replace(image.clone());
        info!(
            "partage : image {} demandée, reçue {} ({}, {:?}{})",
            image.quality.as_str(),
            image.size().map_or_else(|| "pas encore sue".to_string(), |(w, h, f)| format!("{w}×{h} à {f} Hz")),
            match image.follows_host {
                Some(true) => "celle de l'hôte",
                Some(false) => "à part",
                None => "?",
            },
            image.reason,
            image.codec.as_ref().map(|c| format!(", {c}")).unwrap_or_default()
        );
        let notice = image.notice(previous.as_ref(), i18n::current());
        self.platform.on_share_image(&image, previous.as_ref(), notice.as_deref());
    }

    /// « Qualité de l'image » choisie par l'invité : dite à la Station sur un
    /// fil d'appel ; l'image reçue revient par le fil de session.
    fn ask_share_quality(&mut self, quality: ShareQuality) {
        let (Some(g), Some(console)) = (self.guest.clone(), self.console.clone()) else {
            info!("qualité d'image d'invité demandée hors d'une session d'invité : ignorée");
            return;
        };
        info!("partage : qualité {} demandée", quality.as_str());
        // Le dernier choix de l'invité gagne : une demande refusée (trop
        // tôt) n'est réessayée que si aucune autre n'est venue entre-temps.
        let ticket = self.quality_asked.fetch_add(1, Ordering::SeqCst) + 1;
        let latest = self.quality_asked.clone();
        let alive = self.alive.clone();
        let tx = self.tx.clone();
        let label = journal::current();
        std::thread::Builder::new()
            .name("kybernow-qualite".into())
            .spawn(move || {
                journal::resume(label);
                // Avec « Auto », ce que l'appareil a mesuré à `join`.
                let image = ask_quality(
                    || console.share_quality_or_wait(&g.joined.guest, &g.joined.secret, quality, g.joined.auto.as_ref()),
                    std::thread::sleep,
                    || alive.load(Ordering::Relaxed) && latest.load(Ordering::SeqCst) == ticket,
                );
                if let Some(image) = image {
                    let _ = tx.send(Msg::ShareImage(image));
                }
            })
            .expect("fil de la qualité");
    }

    /// L'invité reçoit une image à part (à lui ou à ceux de même qualité),
    /// et la Station le sait : il peut en régler le débit et y demander des
    /// images clés sans attendre. Tant que ce n'est pas su : l'image de l'hôte.
    fn own_image(&self) -> bool {
        self.guest.is_some() && self.share_image.as_ref().is_some_and(ShareImage::own)
    }

    /// Image clé : redemandée toutes les `.0`, flux relancé au bout de `.1`.
    /// Un invité sur l'image de l'hôte : toutes les 2 s (Kyber les espace et
    /// y voit un retard), relance à 5 s ; sur son image à part : comme un
    /// hôte, relance à 5 s (chaque relance le réabonne à l'image).
    fn keyframe_timing(&self) -> (Duration, Duration) {
        match (&self.guest, self.own_image()) {
            (None, _) => (Duration::from_millis(300), Duration::from_millis(1500)),
            (Some(_), true) => (Duration::from_millis(300), Duration::from_secs(5)),
            (Some(_), false) => (Duration::from_secs(2), Duration::from_secs(5)),
        }
    }

    fn spawn_heartbeat(&self, seconds: u64) {
        if !self.host_only(HostOnly::Heartbeat) {
            return;
        }
        let console = self.console.clone().expect("console");
        let tx = self.tx.clone();
        let hb = Heartbeat {
            device_id: self.device_id.clone(),
            latency_ms: None,
            profile: Some(self.params.settings.label().to_string()),
            mode_policy: Some(self.params.settings.mode_policy().to_string()),
        };
        let latency = self.last_latency.clone();
        let profile = self.profile_label.clone();
        let alive = self.alive.clone();
        let label = journal::current();
        std::thread::Builder::new()
            .name("kybernow-heartbeat".into())
            .spawn(move || {
                journal::resume(label);
                loop {
                    std::thread::sleep(Duration::from_secs(seconds));
                    if !alive.load(Ordering::Relaxed) {
                        return;
                    }
                    let mut hb = hb.clone();
                    hb.latency_ms = latency.lock().unwrap().map(|l| (l * 10.0).round() / 10.0);
                    let (label, policy) = profile.lock().unwrap().clone();
                    hb.profile = Some(label);
                    hb.mode_policy = Some(policy);
                    let reply = console.heartbeat(&hb);
                    // Tentative close pendant l'appel (redémarrage de la Station) : réponse périmée.
                    if !alive.load(Ordering::Relaxed) {
                        return;
                    }
                    match reply {
                        Ok(r) => {
                            let end = r.end;
                            let _ = tx.send(Msg::Beat(r));
                            if end {
                                return;
                            }
                        }
                        Err(e) => warn!("signal de vie : {}", e.text(Lang::Fr)),
                    }
                }
            })
            .expect("fil du signal de vie");
    }

    fn spawn_foreground(&self) {
        if !self.host_only(HostOnly::Foreground) {
            return;
        }
        let console = self.console.clone().expect("console");
        let platform = self.platform.clone();
        let alive = self.alive.clone();
        let label = journal::current();
        std::thread::Builder::new()
            .name("kybernow-foreground".into())
            .spawn(move || {
                journal::resume(label);
                let mut last = None;
                loop {
                    std::thread::sleep(Duration::from_secs(1));
                    if !alive.load(Ordering::Relaxed) {
                        return;
                    }
                    if let Ok(f) = console.foreground() {
                        let game = f.known && f.game;
                        if last != Some(game) {
                            info!(
                                "ordinateur : {}",
                                if game { "jeu en plein écran" } else { "pas de jeu en plein écran" }
                            );
                            platform.on_game_fullscreen(game);
                            last = Some(game);
                        }
                    }
                }
            })
            .expect("fil du premier plan");
    }

    /// Réglages du flux en cours : ceux de la session, moins le repli.
    fn effective(&self) -> StreamSettings {
        let mut s = self.params.settings.clone();
        if self.fallback >= 2 {
            s.yuv444 = false;
        }
        s
    }

    /// Débit du profil pour le flux en cours : sur la somme des pixels par
    /// seconde des écrans quand il y en a plusieurs.
    fn profile_bitrate(&self) -> u32 {
        if self.streams.len() > 1 {
            let rate: f64 = self
                .streams
                .iter()
                .map(|s| f64::from(s.display.width.max(0)) * f64::from(s.display.height.max(0)) * f64::from(s.refresh.max(1)))
                .sum();
            return self.params.settings.bitrate_for_pixel_rate(rate);
        }
        // Un invité sur son image à part : le débit de cette image.
        if self.own_image()
            && let Some((w, h, f)) = self.share_image.as_ref().and_then(ShareImage::size)
        {
            return self.params.settings.bitrate(w, h, f);
        }
        let m = self.stream_mode;
        self.params.settings.bitrate(m.width, m.height, m.refresh)
    }

    fn effective_bitrate(&self) -> u32 {
        let mut b = self.profile_bitrate();
        if self.fallback >= 1 {
            if let Some(cap) = self.params.recovery.fallback_bitrate {
                b = b.min(cap);
            }
        }
        if let Some(cap) = self.bitrate_cap {
            b = b.min(cap);
        }
        b
    }

    fn apply_bitrate(&self) {
        // Débit de l'image de l'hôte : réservé à l'hôte ; un invité règle
        // celui de son image à part (Kyber le borne).
        if !self.own_image() && !self.host_only(HostOnly::Bitrate) {
            return;
        }
        if self.streaming.is_some() {
            if let Some(c) = &self.client {
                let _ = c.set_bitrate(self.effective_bitrate());
            }
        }
    }

    fn event_loop(&mut self) -> Option<Failure> {
        let mut next_second = Instant::now() + Duration::from_secs(1);
        loop {
            match self.rx.recv_timeout(Duration::from_millis(100)) {
                Ok(Msg::Stop) => return None,
                Ok(Msg::Fail(e)) => return Some(e),
                Ok(Msg::KeyframeNeeded) => self.request_keyframe(),
                Ok(Msg::KeyframeLost) => {
                    if self.keyframe.is_none() && self.streaming.is_some() {
                        let now = Instant::now();
                        self.keyframe = Some((now, now));
                        self.request_keyframe();
                    }
                }
                Ok(Msg::KeyframeRecovered) => {
                    if let Some((asked, _)) = self.keyframe.take() {
                        info!("image clé reçue en {} ms", asked.elapsed().as_millis());
                    }
                }
                Ok(Msg::Settings(_)) if !self.host_only(HostOnly::Settings) => {}
                Ok(Msg::PickDisplay(_)) if !self.host_only(HostOnly::PickDisplay) => {}
                Ok(Msg::Settings(s)) => {
                    let before = self.effective();
                    self.params.settings = s.clone();
                    // Profil choisi à la main : le repli de la session repart de zéro.
                    self.fallback = 0;
                    *self.profile_label.lock().unwrap() = (s.label().to_string(), s.mode_policy().to_string());
                    // Multi-écrans : le nombre d'écrans suit le profil tout de suite.
                    if let Some(screens) = self.screens.clone()
                        && self.wanted_plan(&screens) != self.plan
                    {
                        info!("profil : {} (le nombre d'écrans change)", s.label());
                        self.refresh_restore_offer();
                        self.replan(false);
                        continue;
                    }
                    let after = self.effective();
                    let restart = before.codec != after.codec || before.yuv444 != after.yuv444 || before.reliable != after.reliable;
                    info!("profil : {} ({})", s.label(), if restart { "flux relancé" } else { "débit seul" });
                    self.refresh_restore_offer();
                    if self.streaming.is_some() {
                        if restart {
                            self.restart_streaming(Duration::from_millis(300));
                        } else {
                            self.apply_bitrate();
                        }
                    }
                }
                Ok(Msg::BitrateCap(cap)) => {
                    self.bitrate_cap = cap;
                    self.apply_bitrate();
                }
                Ok(Msg::DecodedSize(w, h)) => self.decoded(w, h),
                Ok(Msg::PickDisplay(id)) => {
                    if self.streaming.as_ref().map(|d| d.id) != Some(id) {
                        self.picked = Some(id);
                        self.restart_streaming(Duration::from_millis(300));
                    }
                }
                Ok(Msg::SetMode(mode)) => self.adapt(mode),
                Ok(Msg::SetScreens(screens)) => self.change_screens(screens),
                Ok(Msg::Adapted(mode, screens, result)) => self.adapted(mode, screens, result),
                Ok(Msg::RestoreMode) => self.start_restore_mode(),
                Ok(Msg::ModeRestored) => {
                    self.mode_restoring = false;
                    // La console peut répondre sans avoir rien remis (mode
                    // refusé, jeu qui insiste) : si l'image n'est pas revenue,
                    // le bouton revient.
                    self.mode_restore_until = Some(Instant::now() + Duration::from_secs(6));
                }
                Ok(Msg::Beat(reply)) => {
                    if let Some(e) = self.on_beat(reply) {
                        return Some(e);
                    }
                }
                Ok(Msg::ShareBeat(beat)) => {
                    if let Some(e) = self.on_share_beat(beat) {
                        return Some(e);
                    }
                }
                Ok(Msg::ShareGone(end)) => return Some(self.share_ended(end)),
                Ok(Msg::ShareQuality(quality)) => self.ask_share_quality(quality),
                Ok(Msg::ShareImage(image)) => self.share_image_changed(image),
                Ok(Msg::NoImage) => {
                    if let Some(e) = self.no_image_restart() {
                        return Some(e);
                    }
                }
                Ok(Msg::Event(e)) => {
                    if let Some(err) = self.on_event(e) {
                        return Some(err);
                    }
                }
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => return None,
            }
            let now = Instant::now();
            if self.streaming.is_none() {
                if let Some(at) = self.start_at {
                    if now >= at {
                        self.start_at = None;
                        self.choose_display(true);
                    }
                } else if let Some(until) = self.wait_until {
                    if now >= until {
                        self.choose_display(true);
                    }
                }
            } else if self.params.recovery.watch_image {
                if let Some(e) = self.watch_image(now) {
                    return Some(e);
                }
            }
            if self.mute_check_at.is_some_and(|at| now >= at) {
                self.mute_check_at = None;
                self.check_muted();
            }
            if let Some(until) = self.mode_restore_until {
                if now >= until {
                    self.mode_restore_until = None;
                    self.refresh_restore_offer();
                }
            }
            if now >= next_second {
                next_second += Duration::from_secs(1);
                let second = self.latency.drain();
                *self.last_latency.lock().unwrap() = second.latency_ms;
                self.platform.on_latency(second.latency_ms);
                self.platform.on_second(&second);
            }
        }
    }

    fn request_keyframe(&mut self) {
        // Un invité aussi : permise sur son image à part, espacée par Kyber
        // sur l'image de l'hôte (voir `keyframe_timing`).
        // Plusieurs flux : Kyber demande l'image clé à tous à la fois, et une
        // demande avant les premières données d'un flux laisse son encodeur
        // muet. On attend donc que chacun ait reçu quelque chose (il commence
        // par une image clé) ; la veille redemande toutes les 300 ms.
        if self.streams.len() > 1
            && let Some(i) = self.first_data.iter().position(|f| !f.load(Ordering::Relaxed))
        {
            if !self.keyframe_held_logged {
                self.keyframe_held_logged = true;
                info!("image clé pas encore demandée : l'écran {} n'a encore rien reçu", self.streams[i].screen);
            }
            return;
        }
        if let Some(c) = &self.client {
            self.latency.keyframe_requested();
            let _ = c.request_keyframe();
        }
    }

    /// Veilles du cœur (`watch_image`) : première image en 4 s, image clé en
    /// 1,5 s (redemandée toutes les 300 ms ; un invité : `keyframe_timing`).
    fn watch_image(&mut self, now: Instant) -> Option<Failure> {
        if self.hidden.load(Ordering::Relaxed) && self.image_deadline.is_some() {
            // Rien n'est montré tant que l'app a figé l'image : 4 s de nouveau au retour.
            self.image_deadline = Some(now + Duration::from_secs(4));
        }
        if self.has_image.load(Ordering::Relaxed) {
            self.image_deadline = None;
        } else if let Some(deadline) = self.image_deadline {
            if now >= deadline {
                self.image_deadline = None;
                warn!("aucune image en 4 s");
                if !self.degrade() {
                    return self.no_image_restart();
                }
            }
        }
        if let Some((asked, last)) = self.keyframe {
            let (repeat, relaunch) = self.keyframe_timing();
            if asked.elapsed() > relaunch {
                self.keyframe = None;
                warn!("pas d'image clé en {} ms : relance du flux", relaunch.as_millis());
                // Jamais d'image depuis le démarrage : le flux est trop lourd, on l'allège.
                if !self.has_image.load(Ordering::Relaxed) && self.degrade() {
                    return None;
                }
                // Ordinateur sans le correctif d'image clé : un flux neuf repart sur une image clé.
                if self.last_stall_restart.is_none_or(|t| t.elapsed() > Duration::from_secs(5)) {
                    self.last_stall_restart = Some(now);
                    self.restart_streaming(Duration::from_millis(300));
                }
            } else if last.elapsed() >= repeat {
                self.keyframe = Some((asked, now));
                self.request_keyframe();
            }
        }
        None
    }

    /// Allège le flux d'un cran (débit, puis 4:2:0) et le relance. Faux s'il
    /// n'y a plus rien à alléger.
    fn degrade(&mut self) -> bool {
        if !self.host_only(HostOnly::Fallback) {
            return false;
        }
        let Some(cap) = self.params.recovery.fallback_bitrate else { return false };
        let mut next = self.fallback + 1;
        // Cran sans effet (débit déjà bas) : on passe au suivant.
        if next == 1 && self.profile_bitrate() <= cap {
            next = 2;
        }
        if next == 2 && !self.params.settings.yuv444 {
            next = 3;
        }
        if next > 2 {
            return false;
        }
        self.fallback = next;
        warn!(
            "pas d'image : repli {next} ({})",
            if next == 1 { format!("débit {} Mb/s", cap / 1_000_000) } else { "4:2:0".to_string() }
        );
        self.notice(Notice::Fallback { level: next, bitrate_mbps: cap / 1_000_000 });
        self.last_stall_restart = Some(Instant::now());
        self.restart_streaming(Duration::from_millis(300));
        true
    }

    fn no_image_restart(&mut self) -> Option<Failure> {
        if self.streaming.is_some() && self.start_at.is_none() {
            self.no_image += 1;
            if self.no_image > 3 {
                return Some(Failure::no_image(t("session.no_image", &[])));
            }
            warn!("aucune image : flux relancé ({}/3)", self.no_image);
            self.restart_streaming(Duration::from_millis(500));
        }
        None
    }

    fn on_beat(&mut self, r: HeartbeatReply) -> Option<Failure> {
        // KyberNow Share : les invités de cet hôte, quand ils changent.
        if r.share != self.last_share {
            self.last_share = r.share.clone();
            if let Some(s) = &self.last_share {
                info!("partage : {} invité(s){}", s.guests.len(), if s.open { "" } else { ", fermé" });
            }
            self.platform.on_guests(self.last_share.as_ref());
        }
        if r.end {
            info!("la console met fin à la session");
            self.end_requested = true;
            return None.or_else(|| {
                let _ = self.tx.send(Msg::Stop);
                None
            });
        }
        if r.active == Some(false) {
            // Redemandé, au plus une fois toutes les 30 s.
            let recent = self.last_restore.is_some_and(|t| t.elapsed() < Duration::from_secs(30));
            if self.virtual_display() && self.streaming.is_some() && !self.adapting && !recent {
                info!("la console n'a plus d'écran virtuel : redemandé");
                self.last_restore = Some(Instant::now());
                self.stop_streaming();
                if let Err(e) = self.restore_screen() {
                    return Some(e.into());
                }
            }
            return None;
        }
        // Politique « remettre » (profil Bureau, ou réglage de la console) :
        // la console remet elle-même le mode. Si le signal de vie dit encore
        // un autre mode (console d'avant cette politique, remise refusée),
        // on le redemande, au plus une fois toutes les 30 s. En « suivre », rien.
        let policy = r.mode_policy.clone().unwrap_or_else(|| self.profile_label.lock().unwrap().1.clone());
        if policy == "restore" && self.virtual_display() && self.streaming.is_some() && !self.adapting && r.active == Some(true) {
            if let (Some(mode), Some(size)) = (r.mode, self.prepared_size()) {
                if (mode.width, mode.height) != size
                    && self.last_mode_restore.is_none_or(|t| t.elapsed() > Duration::from_secs(30))
                {
                    self.last_mode_restore = Some(Instant::now());
                    info!(
                        "signal de vie : écran de l'ordinateur en {}×{}, {}×{} redemandé",
                        mode.width, mode.height, size.0, size.1
                    );
                    self.start_restore_mode();
                }
            }
        }
        None
    }

    fn on_event(&mut self, event: Event) -> Option<Failure> {
        match event {
            Event::ControlPlaneConnected { displays } => {
                self.share_reconnecting = false;
                self.displays_changed(displays);
            }
            Event::DisplayListUpdated { displays } => self.displays_changed(displays),
            Event::ControlPlaneConnectFailed { reason } => {
                let text = match reason {
                    // Compte d'appareil tout juste créé par la console : Kyber
                    // ne le lit qu'à son démarrage, que la console reporte
                    // pendant une session.
                    // Invité : le jeton de la Station refusé (expiré, invité retiré).
                    ConnectFailedReason::Unauthorized if self.guest.is_some() => t("share.unauthorized", &[]),
                    ConnectFailedReason::Unauthorized => t("session.unauthorized", &[]),
                    ConnectFailedReason::TlsVerificationFailed => t("session.tls", &[]),
                    other => t("session.connect_failed", &[("reason", &format!("{other:?}"))]),
                };
                return Some(Failure { text, cause: connect_failed_cause(reason) });
            }
            Event::DataPlaneConnectFailed => return Some(Failure::kyber(t("session.stream_failed", &[]))),
            Event::StreamingStarted => {
                self.latency.reset();
                self.retries = 0;
                if let Some(n) = self.pending_notice.take() {
                    self.relaunched = false;
                    self.notice(n);
                } else if self.relaunched {
                    self.relaunched = false;
                    self.notice(Notice::Relaunched);
                }
                self.status(Status::Streaming);
            }
            // Kyber se déconnecte lui-même après cet échec : un nouvel essai
            // annoncerait « image démarrée » sans connexion (écran noir).
            Event::StreamingStartFailed => {
                // Plusieurs écrans refusés par Kyber (403, sans dire lequel) :
                // nouvel essai avec l'écran principal seul.
                if self.streams.len() > 1 && !self.primary_only_tried && self.screens.is_some() {
                    warn!("Kyber refuse l'image sur plusieurs écrans : nouvel essai avec l'écran principal seul");
                    self.primary_only_tried = true;
                    let others: Vec<usize> = self.plan.iter().skip(1).map(|p| p.device).collect();
                    self.excluded.extend(others);
                    self.replan(true);
                    return None;
                }
                return Some(Failure::no_image(t("session.image_refused", &[])));
            }
            Event::StreamerPipelineEvent { type_, event, .. } => {
                if event == StreamerPipelineEventType::Error {
                    return self.pipeline_failed(&format!("{type_:?}"));
                }
            }
            Event::Reconnecting { attempt, max_attempts } => {
                // Une relance prévue tomberait sur la reconnexion : c'est
                // « reconnected » qui relancera l'image.
                self.start_at = None;
                self.keyframe = None;
                self.status(Status::Reconnecting {
                    attempt,
                    max: max_attempts,
                });
            }
            Event::Reconnected {
                should_restart_streaming,
            } => {
                self.reconnected_at = Some(Instant::now());
                self.restored_after_reconnect = false;
                if should_restart_streaming {
                    // Kyber a pu redémarrer, l'écran virtuel parti avec lui.
                    self.streaming = None;
                    if let Err(e) = self.restore_screen() {
                        return Some(e.into());
                    }
                } else if self.streaming.is_none() {
                    // Image arrêtée par l'app juste avant la coupure (relance
                    // après une image clé perdue) : Kyber ne la relance pas, l'app si.
                    info!("reconnecté sans image : relance");
                    self.choose_display(false);
                } else if self.streaming_fallback != self.fallback {
                    // Repli décidé pendant la coupure : Kyber a relancé l'ancien flux.
                    self.restart_streaming(Duration::from_millis(300));
                } else {
                    self.status(Status::Streaming);
                }
            }
            Event::ReconnectionFailed => return Some(Failure::kyber(t("session.not_responding", &[]))),
            // L'invité quitte sa session de Kyber pour un autre écran du partage.
            Event::Stopped { reason: StopReason::Requested } if self.share_reconnecting => {
                info!("Kyber : ancienne session de l'invité close");
            }
            Event::Stopped { reason } => {
                if !self.ending {
                    let text = t("session.interrupted", &[("reason", &format!("{reason:?}"))]);
                    return Some(Failure { text, cause: stopped_cause(reason) });
                }
            }
            Event::InputServiceConnected => self.platform.on_input_connected(),
            _ => {}
        }
        None
    }

    /// Erreur de la chaîne d'image (`kind` : son étage) : écran redemandé
    /// juste après une reconnexion, sinon image relancée (3 fois au plus,
    /// le compte repart après 30 s sans relance).
    fn pipeline_failed(&mut self, kind: &str) -> Option<Failure> {
        warn!("{}", i18n::tr(Lang::Fr, "session.pipeline", &[("kind", kind)]));
        if self.adapting {
            info!("image refusée pendant que l'écran virtuel revient : on attend");
            return None;
        }
        if let Some(at) = self.reconnected_at {
            if at.elapsed() < Duration::from_secs(15) && !self.restored_after_reconnect {
                self.restored_after_reconnect = true;
                self.stop_streaming();
                return self.restore_screen().err().map(Failure::from);
            }
        }
        if self.last_retry.is_some_and(|t| t.elapsed() > Duration::from_secs(30)) {
            self.retries = 0;
        }
        if self.retries >= 3 {
            return Some(Failure::no_image(t("session.pipeline", &[("kind", kind)])));
        }
        self.retries += 1;
        self.last_retry = Some(Instant::now());
        self.relaunched = true;
        self.restart_streaming(Duration::from_secs(1));
        None
    }

    /// Redemande l'écran virtuel à la console (sans effet s'il est toujours
    /// là), attend que Kyber l'annonce, puis relance l'image dessus.
    fn restore_screen(&mut self) -> Result<(), String> {
        self.status(Status::Connecting);
        self.prepare()?;
        self.wait_until = Some(Instant::now() + Duration::from_secs(3));
        self.choose_display(false);
        Ok(())
    }

    /// Adaptation à un nouveau mode de l'écran de l'appareil : l'écran virtuel
    /// est refait à ce mode par la console (sur un fil d'appel), puis l'image
    /// repart dessus comme après une reconnexion.
    fn adapt(&mut self, mode: Mode) {
        if !self.host_only(HostOnly::Adapt) {
            return;
        }
        let Some(console) = self.console.clone() else { return };
        if !self.plan.is_empty() {
            info!("multi-écrans : un nouveau mode passe par la liste des écrans (set_screens)");
            return;
        }
        if self.adapting || self.streaming.is_none() || !self.virtual_display() {
            return;
        }
        info!(
            "adaptation à l'écran de l'appareil : écran virtuel redemandé en {}×{} à {} Hz",
            mode.width, mode.height, mode.refresh
        );
        self.notice(Notice::Adapting { width: mode.width, height: mode.height });
        self.stop_streaming();
        self.adapting = true;
        self.last_restore = Some(Instant::now());
        let req = self.prepare_request(mode, None);
        let tx = self.tx.clone();
        std::thread::Builder::new()
            .name("kybernow-prepare".into())
            .spawn(move || {
                let r = console.prepare(&req).map_err(|e| e.to_string());
                let _ = tx.send(Msg::Adapted(mode, None, r));
            })
            .expect("fil de préparation");
    }

    /// Multi-écrans : les écrans de l'appareil ont changé. La console refait
    /// les écrans virtuels d'après le nouveau plan (sur un fil d'appel), puis
    /// l'image repart dessus, comme pour une adaptation.
    fn change_screens(&mut self, screens: Screens) {
        if !self.multi() {
            info!("écrans de l'appareil donnés à une session à un écran : ignorés");
            return;
        }
        if self.adapting {
            self.pending_screens = Some(screens);
            return;
        }
        let old = self.screens.replace(screens.clone()).unwrap_or_default();
        if old.list != screens.list {
            // D'autres écrans : les index d'avant ne veulent plus rien dire.
            self.excluded.clear();
            self.muted.clear();
            self.primary_only_tried = false;
        }
        let plan = self.wanted_plan(&screens);
        if plan == self.plan {
            // Même plan. À un écran, la fenêtre a pu changer d'écran : l'image
            // continue dans sa fenêtre, et l'écran du flux suit « Cet écran »
            // (sans quoi une réannonce, une taille changée par exemple,
            // nommerait l'ancien écran et l'app y ouvrirait une fenêtre).
            if plan.is_empty() && self.streams.len() == 1 && self.streams[0].screen != screens.current() {
                info!("l'image suit la fenêtre sur l'écran {} de l'appareil", screens.current());
                self.streams[0].screen = screens.current();
                self.announce_screens();
            }
            return;
        }
        info!("écrans de l'appareil changés : la session se refait");
        self.replan(false);
    }

    /// Refait la session d'après le plan voulu (écrans de l'appareil ou
    /// profil changés, écran retiré) : la connexion reprend à sa première
    /// étape sur l'écran principal (docs/link-ui.md), sans capsule ; la
    /// console refait les écrans (sur un fil d'appel), puis l'image repart.
    /// `reconnect` : Kyber s'est déconnecté (refus au démarrage), il est
    /// reconnecté une fois la Station prête.
    fn replan(&mut self, reconnect: bool) {
        if !self.host_only(HostOnly::Replan) {
            return;
        }
        // Une préparation est en cours (adaptation, autre session refaite) :
        // celle-ci attend sa fin, jamais deux à la fois.
        if self.adapting {
            self.pending_replan = Some(self.pending_replan.unwrap_or(false) || reconnect);
            return;
        }
        let Some(screens) = self.screens.clone() else { return };
        let Some(console) = self.console.clone() else { return };
        let plan = self.wanted_plan(&screens);
        if !self.virtual_display() {
            // Sans écran virtuel, un seul écran : rien à refaire.
            self.plan = plan;
            return;
        }
        let mode = plan.first().map_or(self.single_mode, |p| p.mode);
        info!(
            "session refaite : {} (mode {}×{} à {} Hz)",
            if plan.is_empty() { "un écran".to_string() } else { format!("{} écran(s)", plan.len()) },
            mode.width,
            mode.height,
            mode.refresh
        );
        self.status(Status::Preparing);
        self.stop_streaming();
        self.start_at = None;
        self.adapting = true;
        self.reconnect_after_prepare = reconnect;
        self.last_restore = Some(Instant::now());
        let req = self.prepare_request_with(mode, None, &plan);
        let tx = self.tx.clone();
        std::thread::Builder::new()
            .name("kybernow-prepare".into())
            .spawn(move || {
                let r = console.prepare(&req).map_err(|e| e.to_string());
                let _ = tx.send(Msg::Adapted(mode, Some((screens, plan)), r));
            })
            .expect("fil de préparation");
    }

    fn adapted(&mut self, mode: Mode, screens: Option<(Screens, Vec<PlannedScreen>)>, result: Result<Prepared, String>) {
        self.adapting = false;
        match result {
            Ok(p) => {
                let screens_changed = screens.is_some();
                if let Some((s, plan)) = screens {
                    self.screens = Some(s);
                    self.plan = plan;
                }
                self.requested = mode;
                if !screens_changed {
                    self.single_mode = mode;
                }
                self.apply_prepared(p);
                if screens_changed {
                    self.status(Status::Connecting);
                    if std::mem::take(&mut self.reconnect_after_prepare) {
                        // Kyber s'est déconnecté après son refus : la liste de
                        // ses écrans revient avec la connexion.
                        self.displays.clear();
                        if let Some(c) = &self.client
                            && let Err(e) = c.connect()
                        {
                            let _ = self.tx.send(Msg::Fail(Failure::kyber(t("session.connect_failed", &[("reason", &e.to_string())]))));
                            return;
                        }
                    }
                } else {
                    let m = self.stream_mode;
                    self.pending_notice = Some(Notice::Adapted { width: m.width, height: m.height });
                }
            }
            Err(e) if screens.is_some() => {
                // La connexion avait repris à sa première étape : comme au
                // départ, un refus d'Admin met fin à la session avec sa raison.
                warn!("écrans de l'appareil refusés : {e}");
                let _ = self.tx.send(Msg::Fail(e.into()));
                return;
            }
            Err(e) => {
                // L'ancien mode reste celui à redemander (reconnexion, écran virtuel perdu).
                warn!("adaptation refusée : {e}");
                self.notice(Notice::AdaptFailed { error: e });
            }
        }
        self.refresh_restore_offer();
        // Kyber annonce l'écran recréé quelques instants après la console.
        self.wait_until = Some(Instant::now() + Duration::from_secs(6));
        self.choose_display(false);
        if let Some(next) = self.pending_screens.take() {
            self.change_screens(next);
        }
        if let Some(reconnect) = self.pending_replan.take()
            && (reconnect || self.wanted_plan(&self.screens.clone().unwrap_or_default()) != self.plan)
        {
            self.replan(reconnect);
        }
    }

    /// « Remettre » : la console remet le mode demandé au début de la session.
    fn start_restore_mode(&mut self) {
        if !self.host_only(HostOnly::RestoreMode) {
            return;
        }
        let Some(console) = self.console.clone() else { return };
        if self.mode_restoring || self.streaming.is_none() {
            return;
        }
        self.mode_restoring = true;
        if self.restore_offer.take().is_some() {
            self.notice(Notice::RestoreOffer { mode: None });
        }
        let r = self.requested;
        info!("mode de l'ordinateur : retour à {}×{} demandé", r.width, r.height);
        let req = self.prepare_request(r, Some("restore"));
        let tx = self.tx.clone();
        let label = journal::current();
        std::thread::Builder::new()
            .name("kybernow-prepare".into())
            .spawn(move || {
                journal::resume(label);
                if let Err(e) = console.prepare(&req) {
                    warn!("mode de l'ordinateur pas remis : {}", e.text(Lang::Fr));
                }
                let _ = tx.send(Msg::ModeRestored);
            })
            .expect("fil de préparation");
    }

    /// Profil Bureau avec l'écran virtuel : remettre la taille de l'appareil.
    fn refresh_restore_offer(&mut self) {
        let offer = match (self.params.settings.profile, self.prepared.as_ref().and_then(|p| p.applied)) {
            (Some(crate::profile::Profile::Bureau), Some(m))
                if self.virtual_display() && (m.width, m.height) != self.host_size && self.host_size != (0, 0) =>
            {
                Some(m)
            }
            _ => None,
        };
        if offer != self.restore_offer {
            self.restore_offer = offer;
            self.notice(Notice::RestoreOffer { mode: offer });
        }
    }

    // MARK: taille de l'écran de l'ordinateur

    /// Images d'une autre taille (txproto recrée l'encodeur quand le mode de
    /// l'écran change, le décodeur suit les nouveaux SPS/PPS) : les positions
    /// de souris se calent sur elles, avant même que Kyber annonce la liste.
    /// Seul un *changement* de taille compte : la première image peut être
    /// à l'échelle (encodeur limité, libavconv), la liste reste alors la
    /// référence.
    fn decoded(&mut self, width: u32, height: u32) {
        if width == 0 || height == 0 {
            return;
        }
        // Un invité reçoit une image à sa mesure (plus petite que l'écran de
        // l'ordinateur) : seule la liste de Kyber dit la taille de l'écran,
        // repère de ses positions de souris.
        if self.guest.is_some() {
            self.frame_size = Some((width, height));
            return;
        }
        let Some(previous) = self.frame_size else {
            self.frame_size = Some((width, height));
            return;
        };
        if previous == (width, height) {
            return;
        }
        self.frame_size = Some((width, height));
        // Quelques pixels d'écart : l'alignement de l'encodeur (2234 → 2240
        // lignes), pas un changement de mode.
        let (hw, hh) = self.host_size;
        if width.abs_diff(hw) <= 16 && height.abs_diff(hh) <= 16 {
            return;
        }
        self.set_host_size(width, height, "image décodée");
    }

    /// Taille que la liste de Kyber donne à l'écran affiché. Quand elle
    /// change, elle fait foi : avec libavconv, l'image garde sa taille de
    /// départ (mise à l'échelle) alors que les positions suivent l'écran.
    fn display_resized(&mut self, width: u32, height: u32) {
        if width == 0 || height == 0 || self.list_size == (width, height) {
            return;
        }
        self.list_size = (width, height);
        self.set_host_size(width, height, "liste des écrans");
    }

    fn set_host_size(&mut self, width: u32, height: u32, source: &str) {
        if self.streaming.is_none() || self.host_size == (width, height) {
            return;
        }
        info!(
            "écran de l'ordinateur : {}×{} → {}×{} ({source})",
            self.host_size.0, self.host_size.1, width, height
        );
        self.host_size = (width, height);
        self.stream_mode.width = width;
        self.stream_mode.height = height;
        if let Some(d) = self.streaming.as_mut() {
            d.width = width as i32;
            d.height = height as i32;
            let d = d.clone();
            self.platform.on_display(&d);
            if let Some(first) = self.streams.first_mut() {
                first.display = d;
                if self.multi() {
                    self.announce_screens();
                }
            }
        }
        self.platform.on_mode(self.stream_mode, self.virtual_display());
        self.apply_bitrate();
        self.refresh_restore_offer();
        let back = self.prepared_size() == Some((width, height));
        self.notice(Notice::ModeChanged { width, height, back });
    }

    fn displays_changed(&mut self, displays: Vec<Display>) {
        info!("écrans de l'ordinateur : {displays:?}");
        self.displays = displays;
        self.platform.on_displays(&self.displays);
        if let Some(cur) = self.streaming.clone() {
            match self.displays.iter().find(|d| d.id == cur.id) {
                Some(d) if usable(d) => {
                    let d = d.clone();
                    self.display_resized(d.width as u32, d.height as u32);
                    self.secondary_displays_changed();
                }
                _ => {
                    // Écran affiché disparu : l'image repart sur un autre.
                    self.restart_streaming(Duration::from_millis(300));
                }
            }
        } else if self.start_at.is_none() {
            self.choose_display(false);
        }
    }

    /// Multi-écrans : les autres écrans du flux dans la liste à jour de Kyber.
    /// Un écran disparu relance l'image (sur les écrans qui restent) ; un
    /// écran d'une autre taille est redonné à la plateforme (repère des entrées).
    fn secondary_displays_changed(&mut self) {
        // Un écran en place annoncé après le démarrage de l'image (Kyber en
        // retard sur le délai) : l'image repart sur tous.
        if !self.targets.is_empty() && self.streams.len() < self.targets.len() && self.matched_screens().len() == self.targets.len() {
            info!("tous les écrans de la session sont annoncés : image relancée sur chacun");
            self.restart_streaming(Duration::from_millis(300));
            return;
        }
        if self.streams.len() < 2 {
            return;
        }
        let mut changed = false;
        for k in 1..self.streams.len() {
            let id = self.streams[k].display.id;
            match self.displays.iter().find(|d| d.id == id) {
                Some(d) if usable(d) => {
                    let s = &mut self.streams[k];
                    if (s.display.width, s.display.height) != (d.width, d.height) {
                        info!("écran {} de l'ordinateur : {}×{} → {}×{}", s.screen, s.display.width, s.display.height, d.width, d.height);
                        s.display = d.clone();
                        changed = true;
                    }
                }
                _ => {
                    info!("un écran de la session a disparu de la liste de Kyber : image relancée");
                    self.restart_streaming(Duration::from_millis(300));
                    return;
                }
            }
        }
        if changed {
            self.announce_screens();
            self.apply_bitrate();
        }
    }

    fn matched_screens(&self) -> Vec<StreamScreen> {
        match_screens(&self.targets, &self.displays)
    }

    /// Avec l'écran virtuel, attend (6 s au plus) que Kyber n'annonce plus que
    /// lui, puis démarre l'image sur le meilleur écran. Multi-écrans : attend
    /// (même délai) que Kyber annonce tous les écrans en place, puis démarre
    /// l'image sur ceux qui sont là.
    fn choose_display(&mut self, deadline_passed: bool) {
        if self.adapting {
            return;
        }
        if self.guest.is_some() {
            return self.choose_share_display(deadline_passed);
        }
        let usable_count = self.displays.iter().filter(|d| usable(d)).count();
        if usable_count == 0 {
            if self.wait_until.is_none() {
                // Aucun écran utilisable dans la liste (Kyber n'annonce que des
                // écrans 0 × 0) : on attend 6 s qu'il en vienne un, puis la
                // session échoue au lieu de rester sur « Connexion ».
                self.wait_until = Some(Instant::now() + Duration::from_secs(6));
            } else if deadline_passed {
                self.wait_until = None;
                let _ = self.tx.send(Msg::Fail(Failure::no_image(t("session.no_display", &[]))));
            }
            return;
        }
        if !self.targets.is_empty() {
            let found = self.matched_screens();
            if found.len() < self.targets.len() && !deadline_passed {
                if self.wait_until.is_none() {
                    self.wait_until = Some(Instant::now() + Duration::from_secs(6));
                }
                return;
            }
            self.wait_until = None;
            if found.first().is_some_and(|s| s.primary) {
                if found.len() < self.targets.len() {
                    warn!("{} écran(s) sur {} annoncé(s) par Kyber : image sur ceux-là", found.len(), self.targets.len());
                }
                self.start_streams(found);
            } else if let Some(d) = self.best_display() {
                warn!("écran principal introuvable dans la liste de Kyber : image sur un écran");
                self.start_streaming(d);
            }
            return;
        }
        if self.virtual_display() && usable_count != 1 && !deadline_passed {
            if self.wait_until.is_none() {
                self.wait_until = Some(Instant::now() + Duration::from_secs(6));
            }
            return;
        }
        self.wait_until = None;
        if let Some(d) = self.best_display() {
            self.start_streaming(d);
        }
    }

    /// Session d'invité : l'écran que la Station donne (Kyber ne laisse que
    /// lui), attendu 6 s au plus ; à défaut le seul écran que Kyber annonce.
    fn choose_share_display(&mut self, deadline_passed: bool) {
        let Some(id) = self.guest.as_ref().map(|g| g.joined.display.id) else { return };
        match share_display(id, &self.displays) {
            Some(d) => {
                self.wait_until = None;
                if d.id != id {
                    info!("écran du partage annoncé sous le numéro {} (attendu : {id})", d.id);
                }
                self.start_streaming(d);
            }
            None if !deadline_passed => {
                if self.wait_until.is_none() {
                    self.wait_until = Some(Instant::now() + Duration::from_secs(6));
                }
            }
            None => {
                self.wait_until = None;
                let _ = self.tx.send(Msg::Fail(Failure::no_image(t("share.display_missing", &[]))));
            }
        }
    }

    fn best_display(&self) -> Option<Display> {
        let list: Vec<&Display> = self.displays.iter().filter(|d| usable(d)).collect();
        if let Some(id) = self.picked {
            if let Some(d) = list.iter().find(|d| d.id == id) {
                return Some((*d).clone());
            }
        }
        if list.len() == 1 {
            return Some(list[0].clone());
        }
        // L'écran affiché revenu sous un autre numéro (câble rebranché, écran
        // recréé) : même place et même taille. Kyber connaît le nom de l'écran
        // mais kyclient ne le transmet pas, d'où la géométrie.
        if let Some(last) = &self.last_shown {
            if let Some(d) = list.iter().find(|d| {
                d.x == last.x && d.y == last.y && d.width == last.width && d.height == last.height
            }) {
                return Some((*d).clone());
            }
        }
        if let Some((w, h)) = self.prepared_size() {
            let same: Vec<&&Display> = list
                .iter()
                .filter(|d| d.width as u32 == w && d.height as u32 == h)
                .collect();
            // L'écran virtuel est placé en haut à gauche du bureau.
            if let Some(d) = same.iter().find(|d| d.x == 0 && d.y == 0).or(same.first()) {
                return Some((**d).clone());
            }
        }
        list.iter()
            .find(|d| d.x == 0 && d.y == 0)
            .or(list.first())
            .map(|d| (*d).clone())
    }

    /// Image sur un écran de l'ordinateur (session à un écran, ou Station
    /// qui n'en gère qu'un : l'écran principal de l'appareil).
    fn start_streaming(&mut self, display: Display) {
        let screen = StreamScreen { screen: self.primary_screen(), display, refresh: self.stream_mode.refresh, primary: true };
        self.start_streams(vec![screen]);
    }

    /// Image sur ces écrans de l'ordinateur, le principal en premier : une
    /// entrée vidéo par écran dans la session de Kyber.
    fn start_streams(&mut self, screens: Vec<StreamScreen>) {
        let Some(client) = self.client.clone() else { return };
        let Some(display) = screens.first().map(|s| s.display.clone()) else { return };
        let s = self.effective();
        self.host_size = (display.width as u32, display.height as u32);
        self.list_size = self.host_size;
        self.frame_size = None;
        self.stream_mode.width = self.host_size.0;
        self.stream_mode.height = self.host_size.1;
        self.streams = screens;
        self.first_data = self.streams.iter().map(|_| Arc::new(AtomicBool::new(false))).collect();
        self.keyframe_held_logged = false;
        self.mute_check_at = (self.streams.len() > 1).then(|| Instant::now() + MUTED_AFTER);
        let bitrate = self.effective_bitrate();
        if self.streams.len() > 1 {
            let others: Vec<String> = self.streams[1..]
                .iter()
                .map(|s| format!("écran {} ({}×{} à {} Hz, en {},{})", s.display.id, s.display.width, s.display.height, s.refresh, s.display.x, s.display.y))
                .collect();
            info!("image sur {} écrans : principal {}, puis {}", self.streams.len(), display.id, others.join(", "));
        }
        info!(
            "image : écran {} ({}×{}), {} {}, {} Mbit/s, {}{}",
            display.id,
            display.width,
            display.height,
            s.codec.label(),
            if s.yuv444 { "4:4:4" } else { "4:2:0" },
            bitrate / 1_000_000,
            if s.reliable { "fiable" } else { "rapide" },
            if self.fallback > 0 { format!(" (repli {})", self.fallback) } else { String::new() }
        );
        let audio_wanted = self.prepared.as_ref().and_then(|p| p.audio_channels).unwrap_or(0) > 0
            || self.params.audio_channels > 0;
        let audio = if audio_wanted {
            self.platform.audio_handler().map(|h| {
                // Son coupé ou non (plusieurs ordinateurs) : le moteur derrière
                // le gestionnaire prend l'état de la session.
                if let Some(engine) = h.clone().audio_engine() {
                    self.focus.attach_audio(&engine);
                }
                AudioSessionConfig {
                    audio_buffer: None,
                    packet_sink: Some(packet_sink(h)),
                }
            })
        } else {
            None
        };
        self.platform.on_display(&display);
        let multi = self.multi();
        if multi {
            self.announce_screens();
        }
        // La plateforme crée sa chaîne d'entrées ici (sur ce fil) et n'en rend
        // que la chaîne kynput : le cœur la retrouve pour lui donner l'état de
        // la session (active ou non), ses droits (invité) et les manettes de
        // son lecteur. Un spectateur n'en a pas : aucune entrée demandée à
        // Kyber, rien de branché au lecteur de manettes.
        let wants_input = self.wants_input();
        let input_config = if wants_input {
            let (pipeline, input) = crate::input::pipeline::created_during(|| self.platform.input_pipeline());
            if let Some(input) = &input {
                self.focus.attach_input(input);
            }
            self.pipeline = Some(pipeline.clone());
            Some(InputSessionConfig {
                pipeline,
                gamepad_force_xbox360: Some(self.params.gamepad_xbox360),
                clipboard: self.input_clipboard(),
            })
        } else {
            self.pipeline = None;
            None
        };
        self.stream_input = wants_input;
        let window_handle = self.platform.window_handle();
        let video: Vec<VideoSessionConfig> = self
            .streams
            .iter()
            .zip(&self.first_data)
            .map(|(screen, first_data)| {
                let handler: Arc<dyn PacketHandler> = if multi {
                    Arc::new(ScreenSink { inner: self.platform.screen_video_handler(screen.screen), first_data: first_data.clone() })
                } else {
                    self.platform.video_handler()
                };
                VideoSessionConfig {
                    display_id: screen.display.id,
                    codec: s.codec.kyber(),
                    yuv444: s.yuv444,
                    intra_refresh: None,
                    player: VideoPlayerConfig {
                        window_handle,
                        software_decode: false,
                        video_buffer: 0,
                        packet_sink: Some(packet_sink(handler)),
                    },
                }
            })
            .collect();
        let config = StreamingSessionConfig {
            protocol: StreamingProtocol::Kymux(KymuxConfig::KyProto(KyProtoConfig {
                video_protocol: if s.reliable {
                    VideoProtocol::Reliable
                } else {
                    VideoProtocol::UnreliableFec
                },
                audio_protocol: AudioProtocol::UnreliableFec,
            })),
            bitrate,
            video,
            audio,
            input: input_config,
        };
        self.streaming = Some(display.clone());
        self.last_shown = Some(display);
        self.streaming_fallback = self.fallback;
        self.keyframe = None;
        self.has_image.store(false, Ordering::Relaxed);
        self.image_deadline = self.params.recovery.watch_image.then(|| Instant::now() + Duration::from_secs(4));
        self.status(Status::FirstImages);
        match client.start_streaming(config) {
            Ok(()) => self.platform.on_stream_started(),
            Err(e) => {
                let _ = self.tx.send(Msg::Fail(t("session.start_image", &[("error", &e.to_string())]).into()));
            }
        }
        self.refresh_restore_offer();
    }

    fn stop_streaming(&mut self) {
        if let Some(c) = &self.client {
            let _ = c.stop_streaming();
        }
        if self.streaming.take().is_some() || self.pipeline.is_some() {
            self.platform.on_stream_stopped();
        }
        self.streams.clear();
        self.first_data.clear();
        self.mute_check_at = None;
        self.keyframe = None;
        self.image_deadline = None;
    }

    /// Multi-écrans : un écran secondaire resté sans données alors que le
    /// principal en reçoit (son encodeur ne s'est pas ouvert, Kyber a quand
    /// même accepté la session) est retiré : la session se refait sans lui,
    /// et la plateforme le voit comme non créé. Le principal suit les règles
    /// d'une session à un écran.
    fn check_muted(&mut self) {
        let silent = muted_streams(&self.streams, &self.first_data);
        if silent.is_empty() {
            return;
        }
        warn!("écran(s) {silent:?} sans données en {} s : retiré(s), la session se refait", MUTED_AFTER.as_secs());
        self.excluded.extend(&silent);
        self.muted.extend(&silent);
        self.replan(false);
    }

    /// Arrêt puis démarrage sur le meilleur écran de la liste à jour.
    fn restart_streaming(&mut self, after: Duration) {
        self.stop_streaming();
        self.start_at = Some(Instant::now() + after);
    }

    fn finish(&mut self, error: Option<String>) {
        self.ending = true;
        self.alive.store(false, Ordering::Relaxed);
        // Session d'invité finie par le partage : dit avant la raison.
        if let Some((end, text)) = &self.share_end {
            self.platform.on_share_end(end, text);
        }
        // La raison d'un échec dès l'état « fin en cours » : Admin peut mettre
        // quelques secondes à répondre avant `Ended`.
        self.platform.on_ending(error.as_deref());
        self.status(Status::Ending);
        let report = match self.end_report.take() {
            // Déjà dite à Admin avant le redémarrage de la Station.
            Some(r) => Some(r),
            None => self.release(error.is_some()).map(|(r, _)| r),
        };
        self.status(Status::Ended { error, report });
    }

    /// Clôt la tentative en cours : touches relâchées, image arrêtée, Kyber
    /// déconnecté, fils du signal de vie arrêtés, fin dite à Admin (écrans
    /// et son rétablis). Rend ce qu'Admin confirme, et s'il a répondu ;
    /// `None` sans console.
    fn release(&mut self, failed: bool) -> Option<(EndReport, bool)> {
        self.ending = true;
        self.alive.store(false, Ordering::Relaxed);
        // Rien ne reste enfoncé sur l'ordinateur.
        if let Some(pipeline) = &self.pipeline {
            pipeline
                .get_input_sink()
                .consume(ControlEvent::FlushKeys.to_packet(InputTarget::Host));
        }
        self.platform.on_stream_stopped();
        if let Some(c) = self.client.take() {
            let _ = c.disconnect_and_stop(true);
        }
        self.pipeline = None;
        self.streaming = None;
        let console = self.console.clone()?;
        if !self.host_only(HostOnly::End) {
            // Invité : il quitte le partage ; rien à rétablir, pas de rapport.
            if let Some(g) = &self.guest {
                match console.share_leave(&g.joined.guest, &g.joined.secret) {
                    Ok(()) => info!("partage quitté"),
                    Err(e) => warn!("départ du partage : {}", e.text(Lang::Fr)),
                }
            }
            return None;
        }
        let latency = if failed { None } else { self.latency.session().0 };
        Some(match console.end(&self.device_id, latency) {
            Ok(e) => {
                if !(e.display_restored && e.audio_restored) {
                    warn!("fin : écrans {} son {} ({:?})", e.display_restored, e.audio_restored, e.error);
                }
                let report = EndReport {
                    display_restored: e.display_restored,
                    audio_restored: e.audio_restored,
                    error: e.error,
                };
                (report, true)
            }
            Err(e) => {
                warn!("fin de session : {}", e.text(Lang::Fr));
                // Une erreur d'Admin est une réponse ; pas un compte refusé, ni Admin injoignable.
                let answered = matches!(e, ConsoleError::Http { status, .. } if status != 401 && status != 403);
                let report = EndReport {
                    display_restored: false,
                    audio_restored: false,
                    error: Some(e.to_string()),
                };
                (report, answered)
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicUsize;

    /// Une tentative qui a tout pour le dernier recours.
    fn eligible() -> Attempt {
        Attempt { enabled: true, prepared: true, shown: false, end_requested: false, restarted: false }
    }

    #[test]
    fn dernier_recours_declenche() {
        // L'image n'arrive pas malgré les réparations, ou Kyber ne répond pas.
        assert!(restart_wanted(Some(Cause::NoImage), eligible()));
        assert!(restart_wanted(Some(Cause::Kyber), eligible()));
        assert!(Recovery::default().restart_station, "activé d'office");
        assert_eq!((STATION_POLL, STATION_BACK_WITHIN), (Duration::from_secs(2), Duration::from_secs(90)));
    }

    #[test]
    fn dernier_recours_exclu() {
        // Erreur ordinaire : Admin injoignable ou refusant le compte, certificat, erreur locale.
        assert!(!restart_wanted(None, eligible()));
        assert_eq!(Failure::from("Admin injoignable".to_string()).cause, None);
        let cases = [
            ("coupé (ligne de commande, bancs d'essai)", Attempt { enabled: false, ..eligible() }),
            ("Admin n'a pas répondu à prepare", Attempt { prepared: false, ..eligible() }),
            ("session établie : une image a été montrée", Attempt { shown: true, ..eligible() }),
            ("fin demandée par Admin ou par un autre appareil", Attempt { end_requested: true, ..eligible() }),
            ("une fois par connexion", Attempt { restarted: true, ..eligible() }),
        ];
        for (why, a) in cases {
            assert!(!restart_wanted(Some(Cause::NoImage), a), "{why}");
            assert!(!restart_wanted(Some(Cause::Kyber), a), "{why}");
        }
    }

    #[test]
    fn causes_de_kyber() {
        use ConnectFailedReason::*;
        for r in [NetworkError, TimedOut, ServerError, WebSocketFailed, ProtocolFailed] {
            assert_eq!(connect_failed_cause(r), Some(Cause::Kyber), "{r:?}");
        }
        // Compte inconnu de Kyber, certificat, configuration de l'appareil : pas de redémarrage.
        for r in [Unauthorized, TlsVerificationFailed, TlsConfigurationFailed] {
            assert_eq!(connect_failed_cause(r), None, "{r:?}");
        }
        assert_eq!(stopped_cause(StopReason::ControlPlaneClosed), Some(Cause::Kyber));
        assert_eq!(stopped_cause(StopReason::ControlPlaneTimeout), Some(Cause::Kyber));
        assert_eq!(stopped_cause(StopReason::DecodeError), None);
        assert_eq!(stopped_cause(StopReason::Requested), None);
    }

    /// Fausse console : refuse le redémarrage (une fois) ou l'accepte, puis
    /// revient au `back_after`-ième appel de `info` (jamais si `None`).
    struct FakeAdmin {
        refuse: Mutex<Option<StationError>>,
        back_after: Option<usize>,
        polls: AtomicUsize,
        restarts: Mutex<Vec<(String, bool)>>,
    }

    impl FakeAdmin {
        fn new(refuse: Option<StationError>, back_after: Option<usize>) -> Self {
            Self { refuse: Mutex::new(refuse), back_after, polls: AtomicUsize::new(0), restarts: Mutex::new(Vec::new()) }
        }

        fn polls(&self) -> usize {
            self.polls.load(Ordering::SeqCst)
        }
    }

    impl StationAdmin for FakeAdmin {
        fn restart(&self, device_id: &str, force: bool) -> Result<(), StationError> {
            self.restarts.lock().unwrap().push((device_id.to_string(), force));
            self.refuse.lock().unwrap().take().map_or(Ok(()), Err)
        }

        fn back(&self) -> bool {
            let n = self.polls.fetch_add(1, Ordering::SeqCst) + 1;
            self.back_after.is_some_and(|k| n >= k)
        }
    }

    const POLL: Duration = Duration::from_millis(2);

    fn sleep_only(d: Duration) -> bool {
        std::thread::sleep(d);
        false
    }

    #[test]
    fn redemarrage_refuse_la_session_echoue() {
        let refusals = [
            StationError::InSession { device: Some("Apple TV du salon".into()), message: "en session".into() },
            StationError::TooSoon { seconds: Some(42), message: "trop tôt".into() },
            StationError::Unsupported,
            StationError::Busy { message: "déjà en cours".into() },
            StationError::Other(ConsoleError::Network("injoignable".into())),
        ];
        for refusal in refusals {
            let kind = refusal.kind();
            let admin = FakeAdmin::new(Some(refusal), Some(1));
            let mut shown = 0;
            let r = repair(&admin, "mac-1", POLL, Duration::from_secs(1), || shown += 1, sleep_only);
            assert!(matches!(r, Repair::Refused(ref e) if e.kind() == kind), "{kind} : {r:?}");
            assert_eq!(shown, 0, "{kind} : rien à afficher");
            assert_eq!(admin.polls(), 0, "{kind} : pas d'attente");
            // Jamais au détriment d'un autre appareil : sans `force`, au nom de cet appareil.
            assert_eq!(*admin.restarts.lock().unwrap(), vec![("mac-1".to_string(), false)]);
        }
    }

    #[test]
    fn redemarrage_accepte_puis_retour() {
        // Admin injoignable deux fois (la Station redémarre), puis revenu.
        let admin = FakeAdmin::new(None, Some(3));
        let mut shown = 0;
        let r = repair(&admin, "mac-1", POLL, Duration::from_secs(5), || shown += 1, sleep_only);
        assert!(matches!(r, Repair::Back), "{r:?}");
        assert_eq!(shown, 1);
        assert_eq!(admin.polls(), 3);
        assert_eq!(*admin.restarts.lock().unwrap(), vec![("mac-1".to_string(), false)]);
    }

    #[test]
    fn station_pas_revenue_a_temps() {
        let admin = FakeAdmin::new(None, None);
        let start = Instant::now();
        let r = repair(&admin, "mac-1", Duration::from_millis(5), Duration::from_millis(40), || {}, sleep_only);
        assert!(matches!(r, Repair::Late), "{r:?}");
        assert!(start.elapsed() >= Duration::from_millis(40));
        // Un essai par intervalle, rien au-delà du délai.
        assert!((1..=8).contains(&admin.polls()), "{} essais", admin.polls());
    }

    #[test]
    fn annule_pendant_l_attente() {
        let admin = FakeAdmin::new(None, None);
        let mut waits = 0;
        let r = repair(&admin, "mac-1", POLL, Duration::from_secs(5), || {}, |d| {
            waits += 1;
            assert!(d <= POLL);
            waits == 2
        });
        assert!(matches!(r, Repair::Cancelled), "{r:?}");
        assert_eq!(admin.polls(), 1);
    }

    #[test]
    fn raison_apres_le_redemarrage() {
        let f = after_restart(Failure::no_image("l'ordinateur n'envoie pas d'image".into()));
        assert_eq!(f.cause, Some(Cause::NoImage));
        assert!(f.text.contains("l'ordinateur n'envoie pas d'image") && f.text.len() > 40, "{}", f.text);
        // Une autre erreur (Admin, compte) reste telle quelle.
        let other = Failure::from("Admin injoignable".to_string());
        assert_eq!(after_restart(other.clone()), other);
        assert_eq!(
            i18n::tr(Lang::Fr, "session.after_station_restart", &[("reason", "connexion au flux impossible")]),
            "connexion au flux impossible, même après un redémarrage de la Station"
        );
        assert_eq!(
            i18n::tr(Lang::En, "session.after_station_restart", &[("reason", "cannot connect to the stream")]),
            "cannot connect to the stream, even after restarting the Station"
        );
        assert!(i18n::tr(Lang::Fr, "session.station_not_back", &[]).starts_with("la Station tarde à revenir"));
        assert!(i18n::tr(Lang::En, "session.station_not_back", &[]).starts_with("the Station is taking a while"));
    }

    fn planned(device: usize, w: u32, h: u32, hz: u32, x: i32, y: i32, primary: bool) -> PlannedScreen {
        PlannedScreen { device, mode: Mode { width: w, height: h, refresh: hz }, x, y, primary }
    }

    fn prepared(screens: Option<Vec<Option<ScreenMode>>>) -> Prepared {
        Prepared {
            virtual_display: true,
            applied: Some(Mode { width: 3840, height: 2160, refresh: 160 }),
            audio_channels: None,
            kyber_port: None,
            heartbeat_seconds: 5,
            kyber_fingerprint: None,
            screens,
        }
    }

    fn display(id: u32, x: i32, y: i32, width: i32, height: i32) -> Display {
        Display { id, x, y, width, height }
    }

    /// Le plan du Mac de l'auteur : XG32UQ principal (écran 1 de l'appareil),
    /// MacBook Pro à gauche (0), Studio Display à droite (2).
    fn plan_auteur() -> Vec<PlannedScreen> {
        vec![
            planned(1, 3840, 2160, 160, 0, 0, true),
            planned(0, 3456, 2234, 60, -3456, 600, false),
            planned(2, 5120, 2880, 60, 3840, 0, false),
        ]
    }

    #[test]
    fn ecrans_en_place_d_apres_la_station() {
        let plan = plan_auteur();
        let applied: Vec<Option<ScreenMode>> = plan.iter().map(|p| Some(p.request())).collect();
        let t = targets(&plan, &prepared(Some(applied.clone())));
        assert_eq!(t.iter().map(|(d, _)| *d).collect::<Vec<_>>(), [1, 0, 2], "ordre de la demande, écran de l'appareil");
        assert!(t[0].1.primary);
        // Un écran que la Station n'a pas pu créer : les autres restent.
        let mut partial = applied.clone();
        partial[1] = None;
        assert_eq!(targets(&plan, &prepared(Some(partial))).iter().map(|(d, _)| *d).collect::<Vec<_>>(), [1, 2]);
        // Station qui ne gère qu'un écran (pas de `screens`) : session à un écran.
        assert!(targets(&plan, &prepared(None)).is_empty());
        // Sans écran principal : session à un écran.
        let mut no_primary = applied;
        no_primary[0] = None;
        assert!(targets(&plan, &prepared(Some(no_primary))).is_empty());
        // Les autres écrans retirés en session : le principal seul, avec `screens` à une entrée.
        assert_eq!(targets(&plan[..1], &prepared(Some(vec![Some(plan[0].request())]))).len(), 1);
        // Pas de plan : session à un écran.
        assert!(targets(&[], &prepared(Some(vec![Some(plan[0].request())]))).is_empty());
    }

    #[test]
    fn ecrans_retrouves_chez_kyber() {
        let plan = plan_auteur();
        let t = targets(&plan, &prepared(Some(plan.iter().map(|p| Some(p.request())).collect())));
        // Kyber annonce les écrans dans un autre ordre, avec un écran 0 × 0 en plus.
        let kyber = vec![display(7, 3840, 0, 5120, 2880), display(9, 0, 0, 0, 0), display(5, -3456, 600, 3456, 2234), display(3, 0, 0, 3840, 2160)];
        let m = match_screens(&t, &kyber);
        assert_eq!(m.iter().map(|s| (s.screen, s.display.id, s.refresh, s.primary)).collect::<Vec<_>>(), [(1, 3, 160, true), (0, 5, 60, false), (2, 7, 60, false)]);
        // Pas encore tous annoncés : ceux qui sont là.
        let m = match_screens(&t, &kyber[..2]);
        assert_eq!(m.iter().map(|s| s.display.id).collect::<Vec<_>>(), [7]);
        // Position différente mais taille unique : retrouvé par la taille.
        let moved = vec![display(3, 0, 0, 3840, 2160), display(5, -3456, 0, 3456, 2234), display(7, 3840, 0, 5120, 2880)];
        assert_eq!(match_screens(&t, &moved).len(), 3);
        // Ce que la plateforme apprend : l'écran non annoncé n'a pas pu être créé.
        let shown = match_screens(&t, &kyber[..3]);
        assert_eq!(not_created(&plan, &t, &shown, &[]), [1]);
        assert!(not_created(&plan, &t, &match_screens(&t, &kyber), &[]).is_empty());
        // Station à un écran, ou principal seul accepté : rien de « non créé »,
        // la session est sur l'écran principal seul.
        assert!(not_created(&plan, &[], &shown[..1], &[]).is_empty());
        assert!(not_created(&plan, &t[..1], &shown[..1], &[]).is_empty());
        // Un écran retiré parce que muet reste « non créé ».
        assert_eq!(not_created(&plan[..2], &t[..1], &shown[..1], &[2]), [2]);
        // Deux écrans libres de la même taille : pas de devinette.
        let twins = vec![
            planned(0, 1920, 1080, 60, 0, 0, true),
            planned(1, 1920, 1080, 60, 1920, 0, false),
            planned(2, 1920, 1080, 60, -1920, 0, false),
        ];
        let t = targets(&twins, &prepared(Some(twins.iter().map(|p| Some(p.request())).collect())));
        let kyber = vec![display(1, 0, 0, 1920, 1080), display(2, 1920, 50, 1920, 1080), display(3, -1920, 50, 1920, 1080)];
        assert_eq!(match_screens(&t, &kyber).iter().map(|s| s.display.id).collect::<Vec<_>>(), [1]);
    }

    #[test]
    fn ecran_muet_retire() {
        let streams: Vec<StreamScreen> = [(1, true), (0, false), (2, false)]
            .iter()
            .map(|&(screen, primary)| StreamScreen { screen, display: display(screen as u32, 0, 0, 100, 100), refresh: 60, primary })
            .collect();
        let flags = |v: [bool; 3]| v.iter().map(|&b| Arc::new(AtomicBool::new(b))).collect::<Vec<_>>();
        assert_eq!(muted_streams(&streams, &flags([true, true, false])), [2], "le Studio Display reste muet");
        assert_eq!(muted_streams(&streams, &flags([true, false, false])), [0, 2]);
        assert!(muted_streams(&streams, &flags([false, false, false])).is_empty(), "principal muet : ses propres règles");
        assert!(muted_streams(&streams, &flags([true, true, true])).is_empty());
        assert!(muted_streams(&streams[..1], &flags([true, false, false])).is_empty(), "un écran : rien");
        assert_eq!(MUTED_AFTER, Duration::from_secs(4));
    }

    #[test]
    fn premieres_donnees_notees_par_ecran() {
        struct Count(AtomicUsize);
        impl PacketHandler for Count {
            fn packet(&self, _: Packet<'_>) {
                self.0.fetch_add(1, Ordering::SeqCst);
            }
        }
        let inner = Arc::new(Count(AtomicUsize::new(0)));
        let flag = Arc::new(AtomicBool::new(false));
        let sink = ScreenSink { inner: inner.clone(), first_data: flag.clone() };
        sink.packet_from(4, Packet::Codec { codec: crate::media::FOURCC_HEVC });
        assert!(!flag.load(Ordering::SeqCst), "le format n'est pas une donnée");
        sink.packet_from(4, Packet::Media { is_config: true, is_key: false, pts: 0, data: &[0, 0, 1] });
        assert!(flag.load(Ordering::SeqCst));
        assert_eq!(inner.0.load(Ordering::SeqCst), 2, "tout passe à la plateforme");
    }

    // Une session sans réseau : un Worker avec une fausse plateforme, pour
    // les décisions du multi-écrans qui ne demandent ni Admin ni Kyber.

    #[derive(Default)]
    struct Recorder {
        screens: Mutex<Vec<(Vec<usize>, Vec<usize>)>>,
        /// États et raisons de fin reçus, dans l'ordre.
        events: Mutex<Vec<String>>,
        /// KyberNow Share : rôles de l'invité, fins par le partage, invités
        /// de l'hôte (leur nombre ; `None` : pas de partage), rapports de fin.
        roles: Mutex<Vec<(ShareRole, Option<ShareRole>)>>,
        guests: Mutex<Vec<Option<usize>>>,
        reports: Mutex<Vec<bool>>,
        modes: Mutex<Vec<(Mode, bool)>>,
        images: Mutex<Vec<(ShareImage, Option<ShareImage>, Option<String>)>>,
    }

    impl Platform for Recorder {
        fn window_handle(&self) -> WindowHandle {
            unreachable!("pas d'image dans ces tests")
        }
        fn video_handler(&self) -> Arc<dyn PacketHandler> {
            unreachable!("pas d'image dans ces tests")
        }
        fn audio_handler(&self) -> Option<Arc<dyn PacketHandler>> {
            None
        }
        fn input_pipeline(&self) -> Pipeline {
            unreachable!("pas d'image dans ces tests")
        }
        fn on_status(&self, s: &Status) {
            let line = match s {
                Status::Ending => "fin en cours".to_string(),
                Status::Ended { error, report } => {
                    self.reports.lock().unwrap().push(report.is_some());
                    format!("terminée {error:?}")
                }
                other => format!("{other:?}"),
            };
            self.events.lock().unwrap().push(line);
        }
        fn on_ending(&self, error: Option<&str>) {
            self.events.lock().unwrap().push(format!("raison {error:?}"));
        }
        fn on_share_role(&self, role: ShareRole, previous: Option<ShareRole>) {
            self.roles.lock().unwrap().push((role, previous));
        }
        fn on_mode(&self, mode: Mode, virtual_display: bool) {
            self.modes.lock().unwrap().push((mode, virtual_display));
        }
        fn on_share_image(&self, image: &ShareImage, previous: Option<&ShareImage>, notice: Option<&str>) {
            self.images.lock().unwrap().push((image.clone(), previous.cloned(), notice.map(str::to_string)));
        }
        fn on_share_end(&self, end: &ShareEnd, text: &str) {
            self.events.lock().unwrap().push(format!("fin du partage {end:?} : {text}"));
        }
        fn on_guests(&self, share: Option<&ShareState>) {
            self.guests.lock().unwrap().push(share.map(|s| s.guests.len()));
        }
        fn on_display(&self, _: &Display) {}
        fn on_screens(&self, screens: &[StreamScreen], not_created: &[usize]) {
            self.screens.lock().unwrap().push((screens.iter().map(|s| s.screen).collect(), not_created.to_vec()));
        }
    }

    fn device_screen(x: f64, primary: bool) -> crate::screens::DeviceScreen {
        crate::screens::DeviceScreen { x, y: 0.0, width: 1920.0, height: 1080.0, pixel_width: 1920, pixel_height: 1080, refresh: 60, primary }
    }

    /// Deux écrans côte à côte, « Cet écran » = `current`, Station à quatre écrans.
    fn two_screens(current: usize) -> Screens {
        Screens { list: vec![device_screen(0.0, true), device_screen(1920.0, false)], current, max_screens: 4, ..Screens::default() }
    }

    fn worker(profile: crate::profile::Profile, screens: Screens) -> (Worker, Arc<Recorder>) {
        let caps = crate::profile::Capabilities { hevc: true, hevc444: true, av1: false };
        let params = SessionParams {
            console_host: "192.0.2.1".into(),
            console_port: 9443,
            user: "u".into(),
            password: "p".into(),
            console_fingerprint: None,
            device: DeviceInfo { id: "mac".into(), name: "Mac".into(), kind: "mac".into(), os: "macOS".into(), app: "test".into() },
            mode: Mode { width: 1920, height: 1080, refresh: 60 },
            audio_channels: 0,
            virtual_display: true,
            settings: StreamSettings::for_profile(profile, caps),
            auto_reconnect: true,
            gamepad_xbox360: true,
            clipboard: false,
            recovery: Recovery::default(),
            role: SessionRole::Host,
        };
        let recorder = Arc::new(Recorder::default());
        let (tx, rx) = mpsc::channel();
        let w = Worker::new(params, Some(screens), recorder.clone(), tx, rx, Arc::new(LatencyTracker::new()), Arc::new(AtomicBool::new(false)), Arc::new(AtomicBool::new(false)), FocusState::new());
        (w, recorder)
    }

    #[test]
    fn raison_d_un_echec_des_la_fin_en_cours() {
        let (mut w, rec) = worker(crate::profile::Profile::Jeu, two_screens(0));
        w.finish(Some("Admin ne répond pas".into()));
        assert_eq!(
            *rec.events.lock().unwrap(),
            ["raison Some(\"Admin ne répond pas\")", "fin en cours", "terminée Some(\"Admin ne répond pas\")"],
            "la raison arrive juste avant « fin en cours », la même qu'à la fin"
        );
        let (mut w, rec) = worker(crate::profile::Profile::Jeu, two_screens(0));
        w.finish(None);
        assert_eq!(*rec.events.lock().unwrap(), ["raison None", "fin en cours", "terminée None"], "fin voulue : pas de raison");
    }

    // Plusieurs ordinateurs : l'état de la session suit les chaînes d'entrées
    // et les moteurs de son d'un démarrage de l'image à l'autre.

    struct Quiet;
    impl crate::input::InputPlatform for Quiet {}

    /// Une chaîne d'entrées comme la crée la plateforme dans `input_pipeline`.
    fn input_chain() -> (Arc<InputPipeline>, Arc<crate::input::packet_queue::tests::Host>) {
        use crate::input::{ManualClock, Options, PointerLayer};
        let clock = Arc::new(ManualClock::new());
        let host = Arc::new(crate::input::packet_queue::tests::Host::default());
        let input = InputPipeline::new_with_sink(
            Options { display_id: 1, host_size: (1920, 1080), clipboard: None, gamepads_from_app: true },
            Arc::new(PointerLayer::new(clock.clone(), true)),
            Arc::new(Quiet),
            clock,
            host.clone(),
        );
        (input, host)
    }

    #[test]
    fn active_et_son_coupe_d_une_image_a_l_autre() {
        let focus = FocusState::new();
        assert!(focus.is_active() && !focus.is_muted(), "seule, une session est active et s'entend");
        // La chaîne créée pendant `input_pipeline` est retrouvée par le cœur.
        let (kynput, created) = crate::input::pipeline::created_during(|| {
            let (input, host) = input_chain();
            (input.kynput(), input, host)
        });
        let (_kynput, first, host) = kynput;
        assert!(Arc::ptr_eq(created.as_ref().unwrap(), &first), "la chaîne de la plateforme");
        focus.attach_input(&first);
        assert!(first.is_active());
        focus.set_active(false);
        assert!(!first.is_active(), "appliqué à la chaîne en cours");
        first.key(0x1E, true);
        assert!(host.received().is_empty(), "rien ne part en arrière-plan");
        // Image relancée : la chaîne neuve prend l'état de la session.
        let (second, _) = input_chain();
        assert!(second.is_active(), "une chaîne neuve est active");
        focus.attach_input(&second);
        assert!(!second.is_active(), "et prend l'état de la session");
        focus.set_active(true);
        assert!(second.is_active() && !first.is_active(), "seule la chaîne en cours suit");
        // Le son : chaque moteur prend l'état, même arrivé après.
        let clock = Arc::new(crate::audio::ManualClock::new());
        let engine = Arc::new(AudioEngine::new(clock.clone()));
        focus.attach_audio(&engine);
        focus.set_muted(true);
        assert!(engine.is_muted());
        let later = Arc::new(AudioEngine::new(clock));
        focus.attach_audio(&later);
        assert!(later.is_muted(), "un moteur neuf prend l'état de la session");
        focus.set_muted(false);
        assert!(!engine.is_muted() && !later.is_muted());
        // Hors d'`input_pipeline`, rien n'est retrouvé.
        let (_, none) = crate::input::pipeline::created_during(|| ());
        assert!(none.is_none());
    }

    fn prepared_as(virtual_display: bool, screens: Option<Vec<Option<ScreenMode>>>) -> Prepared {
        Prepared { virtual_display, ..prepared(screens) }
    }

    #[test]
    fn station_sans_ecran_virtuel_un_seul_ecran() {
        // Bureau, deux écrans, accueil sur l'écran secondaire : le plan prend les deux.
        let (mut w, _) = worker(crate::profile::Profile::Bureau, two_screens(1));
        assert_eq!(w.plan.len(), 2);
        let applied: Vec<Option<ScreenMode>> = w.plan.iter().map(|p| Some(p.request())).collect();
        // La Station prépare sans écran virtuel (pilote absent) : un écran, « Cet écran ».
        w.apply_prepared(prepared_as(false, Some(applied.clone())));
        assert!(w.plan.is_empty() && w.targets.is_empty() && w.station_single);
        assert_eq!(w.primary_screen(), 1, "l'image va dans la fenêtre de l'accueil, sur son écran");
        // Le profil ou les écrans changent ensuite : toujours un seul écran.
        assert!(w.wanted_plan(&two_screens(1)).is_empty());
        // Même chose avec une Station qui répond sans `screens`.
        let (mut w, _) = worker(crate::profile::Profile::Bureau, two_screens(1));
        w.apply_prepared(prepared_as(true, None));
        assert!(w.plan.is_empty() && w.station_single && w.primary_screen() == 1);
        // Une Station multi-écrans qui les crée : le plan reste.
        let (mut w, _) = worker(crate::profile::Profile::Bureau, two_screens(1));
        w.apply_prepared(prepared_as(true, Some(applied)));
        assert!(!w.station_single && w.plan.len() == 2 && w.targets.len() == 2);
    }

    #[test]
    fn ecran_virtuel_indisponible_dans_le_plan() {
        // L'info de la Station dit l'écran virtuel inutilisable : le plan n'en prend qu'un dès le départ.
        let screens = Screens { virtual_display_available: false, ..two_screens(0) };
        let (w, _) = worker(crate::profile::Profile::Bureau, screens);
        assert!(w.plan.is_empty());
    }

    #[test]
    fn session_a_un_ecran_suit_la_fenetre() {
        // Profil Jeu : un seul écran, celui de la fenêtre (écran 0).
        let (mut w, recorder) = worker(crate::profile::Profile::Jeu, two_screens(0));
        assert!(w.plan.is_empty());
        w.streams = vec![StreamScreen { screen: 0, display: display(1, 0, 0, 1920, 1080), refresh: 60, primary: true }];
        // La fenêtre passe sur l'écran 1 : l'image continue, l'écran du flux suit.
        w.change_screens(two_screens(1));
        assert_eq!(w.streams[0].screen, 1);
        assert_eq!(*recorder.screens.lock().unwrap(), [(vec![1], vec![])], "réannoncé sur le nouvel écran, rien d'autre");
        // Rien ne change : rien n'est réannoncé.
        w.change_screens(two_screens(1));
        assert_eq!(recorder.screens.lock().unwrap().len(), 1);
    }

    #[test]
    fn session_refaite_attend_la_preparation_en_cours() {
        let (mut w, _) = worker(crate::profile::Profile::Bureau, two_screens(0));
        w.adapting = true;
        w.replan(false);
        assert_eq!(w.pending_replan, Some(false), "attend la fin de la préparation en cours");
        w.replan(true);
        assert_eq!(w.pending_replan, Some(true), "une reconnexion demandée n'est pas oubliée");
    }


    // KyberNow Share : une session d'invité, sans réseau (une Station sur un
    // port fermé de cette machine : tout appel échouerait tout de suite).

    /// Un port où rien n'écoute.
    fn closed_port() -> u16 {
        let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        l.local_addr().unwrap().port()
    }

    fn joined(role: ShareRole, codec: Option<&str>, fingerprint: Option<&str>) -> ShareJoined {
        serde_json::from_value(serde_json::json!({
            "guest": "g1", "secret": "s3cr3t", "role": role.as_str(), "host": "Léa", "computer": "PC-ATELIER",
            "kyber": { "port": 12345, "fingerprint": fingerprint, "token": "jwt", "expires": 120 },
            "display": { "id": 7, "width": 1920, "height": 1080, "refresh": 60 },
            "codec": codec, "yuv444": false, "beat": 5
        }))
        .unwrap()
    }

    fn guest_with(joined: ShareJoined, caps: crate::profile::Capabilities) -> (Worker, Arc<Recorder>) {
        let found = Found { target: share::ShareTarget { host: "127.0.0.1".into(), port: closed_port(), fingerprint: Some("sha256:ad".into()) }, joined };
        let device = DeviceInfo { id: "ipad".into(), name: "iPad".into(), kind: "ios".into(), os: "iPadOS".into(), app: "test".into() };
        let settings = StreamSettings::for_profile(crate::profile::Profile::Bureau, caps);
        let mut params = SessionParams::guest(&found, device, settings, caps, 2);
        params.recovery = Recovery { watch_image: true, fallback_bitrate: Some(20_000_000), restart_station: true };
        // Même demandé par l'app : jamais pour un invité.
        params.clipboard = true;
        let recorder = Arc::new(Recorder::default());
        let (tx, rx) = mpsc::channel();
        // Une app multi-écrans : un invité n'a pourtant qu'un écran.
        let w = Worker::new(params, Some(two_screens(0)), recorder.clone(), tx, rx, Arc::new(LatencyTracker::new()), Arc::new(AtomicBool::new(false)), Arc::new(AtomicBool::new(false)), FocusState::new());
        (w, recorder)
    }

    fn guest(role: ShareRole) -> (Worker, Arc<Recorder>) {
        guest_with(joined(role, Some("hevc"), Some("sha256:kk")), crate::profile::Capabilities { hevc: true, hevc444: true, av1: false })
    }

    fn beat(role: ShareRole) -> ShareBeat {
        ShareBeat { role, token: Some("jwt-2".into()), expires: Some(120), display: Some(ShareDisplay { id: 7, width: 1920, height: 1080, refresh: 60 }), image: None, end: None }
    }

    const ALL_HOST_ONLY: [HostOnly; 12] = [
        HostOnly::Prepare,
        HostOnly::Heartbeat,
        HostOnly::Foreground,
        HostOnly::Adapt,
        HostOnly::Replan,
        HostOnly::RestoreMode,
        HostOnly::Bitrate,
        HostOnly::Fallback,
        HostOnly::StationRestart,
        HostOnly::End,
        HostOnly::PickDisplay,
        HostOnly::Settings,
    ];

    #[test]
    fn un_invite_ne_change_jamais_l_ordinateur() {
        let (mut w, recorder) = guest(ShareRole::PlayerKbm);
        let port = w.params.console_port;
        w.console = Some(Arc::new(Console::guest("127.0.0.1", port, None)));
        assert!(w.plan.is_empty() && w.station_single, "jamais de plan multi-écrans");
        assert!(w.wanted_plan(&two_screens(1)).is_empty());
        let mode = Mode { width: 3840, height: 2160, refresh: 120 };
        // Chaque appel qui change l'ordinateur, appelé directement…
        assert!(w.prepare().is_ok());
        w.spawn_heartbeat(1);
        w.spawn_foreground();
        w.adapt(mode);
        w.replan(true);
        w.start_restore_mode();
        w.apply_bitrate();
        w.request_keyframe();
        assert!(!w.degrade(), "ni débit réduit ni 4:2:0");
        assert!(!w.restart_wanted(&Failure::no_image("pas d'image".into())), "jamais de dernier recours");
        assert!(w.last_resort(Failure::kyber("Kyber muet".into())).is_some());
        assert!(!w.adapting && w.pending_replan.is_none() && !w.mode_restoring);
        // … et demandé par l'app pendant la session.
        let before = w.params.settings.profile;
        for m in [
            Msg::Settings(StreamSettings::for_profile(crate::profile::Profile::Max, crate::profile::Capabilities::default())),
            Msg::PickDisplay(3),
            Msg::SetMode(mode),
            Msg::RestoreMode,
            Msg::BitrateCap(Some(5_000_000)),
            Msg::KeyframeNeeded,
            Msg::Stop,
        ] {
            w.tx.send(m).unwrap();
        }
        assert!(w.event_loop().is_none());
        assert_eq!((w.params.settings.profile, w.picked), (before, None), "profil et écran inchangés");
        // Le départ : `share_leave`, jamais `end`, et pas de rapport.
        assert!(w.release(false).is_none());
        let refused = w.refused.lock().unwrap().clone();
        for a in ALL_HOST_ONLY {
            assert!(refused.contains(&a), "{a:?} doit être fermé pour un invité");
        }
        assert!(recorder.events.lock().unwrap().iter().all(|e| !e.contains("Preparing")), "rien de préparé");
        // Une session d'hôte : tout est permis, rien n'est noté.
        let (host, _) = worker(crate::profile::Profile::Jeu, two_screens(0));
        for a in ALL_HOST_ONLY {
            assert!(host.host_only(a), "{a:?}");
        }
        assert!(host.refused.lock().unwrap().is_empty());
    }

    #[test]
    fn l_ecran_du_partage() {
        let list = [display(3, 0, 0, 3840, 2160), display(7, 3840, 0, 1920, 1080), display(9, 0, 0, 0, 0)];
        assert_eq!(share_display(7, &list).map(|d| d.id), Some(7), "celui que donne la Station");
        assert_eq!(share_display(5, &list), None, "deux écrans, aucun n'est le bon : pas de devinette");
        assert_eq!(share_display(5, &list[1..]).map(|d| d.id), Some(7), "le seul écran que Kyber laisse à l'invité");
        assert_eq!(share_display(9, &list[2..]), None, "écran 0 × 0");
        // Absent de la liste : attendu, puis la session échoue avec la raison.
        let (mut w, _) = guest(ShareRole::Spectator);
        w.displays = list[..1].to_vec();
        w.displays.push(display(4, 0, 2160, 1920, 1080));
        w.choose_display(false);
        assert!(w.wait_until.is_some(), "attendu 6 s");
        w.choose_display(true);
        match w.rx.try_recv() {
            Ok(Msg::Fail(f)) => assert_eq!(f.text, t("share.display_missing", &[])),
            _ => panic!("la session devait échouer"),
        }
    }

    #[test]
    fn droits_selon_le_role() {
        let (mut w, recorder) = guest(ShareRole::Spectator);
        assert_eq!(w.focus.rights(), InputRights::NONE);
        assert!(!w.wants_input(), "un spectateur n'a aucune chaîne d'entrées");
        assert!(!w.input_clipboard(), "jamais le presse-papiers");
        // Devenu joueur pendant l'image : droits, avis, image relancée avec les entrées.
        w.streaming = Some(display(7, 0, 0, 1920, 1080));
        w.stream_input = false;
        assert!(w.on_share_beat(beat(ShareRole::Player)).is_none());
        assert_eq!(w.focus.rights(), ShareRole::Player.rights());
        assert!(w.wants_input());
        assert!(w.streaming.is_none() && w.start_at.is_some(), "image relancée pour ouvrir les entrées");
        // La chaîne de l'image suivante prend les droits ; clavier et souris donnés sans relance.
        let (chain, _) = input_chain();
        w.focus.attach_input(&chain);
        assert_eq!(chain.rights(), ShareRole::Player.rights());
        w.streaming = Some(display(7, 0, 0, 1920, 1080));
        w.stream_input = true;
        w.start_at = None;
        assert!(w.on_share_beat(beat(ShareRole::PlayerKbm)).is_none());
        assert_eq!(chain.rights(), ShareRole::PlayerKbm.rights());
        assert!(w.streaming.is_some() && w.start_at.is_none(), "pas de relance");
        // Redevenu spectateur : la chaîne reste, sans droits.
        assert!(w.on_share_beat(beat(ShareRole::Spectator)).is_none());
        assert_eq!(chain.rights(), InputRights::NONE);
        assert!(w.streaming.is_some() && w.start_at.is_none());
        // Même rôle : rien de nouveau.
        assert!(w.on_share_beat(beat(ShareRole::Spectator)).is_none());
        assert_eq!(
            *recorder.roles.lock().unwrap(),
            [
                (ShareRole::Player, Some(ShareRole::Spectator)),
                (ShareRole::PlayerKbm, Some(ShareRole::Player)),
                (ShareRole::Spectator, Some(ShareRole::PlayerKbm))
            ]
        );
        // Un hôte : une chaîne, tous les droits, le presse-papiers s'il est réglé.
        let (mut host, _) = worker(crate::profile::Profile::Jeu, two_screens(0));
        host.params.clipboard = true;
        assert!(host.wants_input() && host.input_clipboard());
        assert_eq!(host.focus.rights(), InputRights::ALL);
    }

    #[test]
    fn deux_droits_et_passages_de_role() {
        // Spectateur → clavier et souris seuls : une chaîne d'entrées (sans
        // manettes), donc une image relancée pour l'ouvrir.
        let (mut w, recorder) = guest(ShareRole::Spectator);
        w.streaming = Some(display(7, 0, 0, 1920, 1080));
        w.stream_input = false;
        assert!(w.on_share_beat(beat(ShareRole::Kbm)).is_none());
        assert!(w.wants_input() && w.streaming.is_none() && w.start_at.is_some(), "image relancée");
        assert_eq!(w.focus.rights(), InputRights { gamepads: false, keyboard_mouse: true, clipboard: false });
        // Entre deux rôles qui ont chacun une chaîne : jamais de relance.
        let (chain, host) = input_chain();
        w.focus.attach_input(&chain);
        w.streaming = Some(display(7, 0, 0, 1920, 1080));
        w.stream_input = true;
        w.start_at = None;
        for r in [ShareRole::PlayerKbm, ShareRole::Player, ShareRole::Kbm, ShareRole::PlayerKbm, ShareRole::Kbm] {
            assert!(w.on_share_beat(beat(r)).is_none());
            assert_eq!(chain.rights(), r.rights(), "{r:?}");
            assert!(w.streaming.is_some() && w.start_at.is_none(), "pas de relance vers {r:?}");
        }
        // Le clavier et la souris partent, pas les manettes.
        host.clear();
        chain.key(0x1E, true);
        chain.gamepad_button(0, crate::input::PadButton::A, true);
        assert_eq!(host.received(), ["touche 0x1e appuyée"]);
        // Plus aucun droit : la chaîne reste, sans relance.
        assert!(w.on_share_beat(beat(ShareRole::Spectator)).is_none());
        assert!(w.streaming.is_some() && w.start_at.is_none() && chain.rights() == InputRights::NONE);
        assert_eq!(recorder.roles.lock().unwrap().len(), 7);
    }

    #[test]
    fn fins_par_le_partage() {
        // Arrêté par l'hôte (signe de vie) : la raison de link-ui, avant la fin.
        let (mut w, recorder) = guest(ShareRole::Player);
        let stop = ShareBeat { end: Some(ShareEnd::Stopped), ..beat(ShareRole::Player) };
        let f = w.on_share_beat(stop).expect("fin");
        let text = ShareEnd::Stopped.text(i18n::current(), "Léa");
        assert_eq!((f.text.as_str(), f.cause), (text.as_str(), None), "jamais de dernier recours");
        w.finish(Some(f.text));
        assert_eq!(
            *recorder.events.lock().unwrap(),
            [format!("fin du partage Stopped : {text}"), format!("raison Some({text:?})"), "fin en cours".into(), format!("terminée Some({text:?})")]
        );
        assert_eq!(*recorder.reports.lock().unwrap(), [false], "pas de rapport d'écrans rendus");
        // Retiré : 200 avec la fin, sans jeton (60 s durant) ; fin propre, rien de renouvelé.
        let (mut w, _) = guest(ShareRole::Spectator);
        w.tx.send(Msg::ShareBeat(ShareBeat { role: ShareRole::Spectator, token: None, expires: Some(0), display: None, image: None, end: Some(ShareEnd::Removed) })).unwrap();
        let f = w.event_loop().expect("fin");
        assert_eq!(f.text, ShareEnd::Removed.text(i18n::current(), "Léa"));
        assert_eq!(w.share_end.as_ref().map(|(e, _)| e.clone()), Some(ShareEnd::Removed));
        assert_eq!(w.guest.as_ref().unwrap().joined.kyber.token, "jwt", "jeton inchangé");
        // Ensuite, ou invité inconnu : 404, le partage est fermé.
        let (mut w, _) = guest(ShareRole::Spectator);
        w.tx.send(Msg::ShareGone(ShareEnd::Stopped)).unwrap();
        assert_eq!(w.event_loop().expect("fin").text, ShareEnd::Stopped.text(i18n::current(), "Léa"));
    }

    #[test]
    fn reponses_du_signe_de_vie() {
        let http = |status: u16| Err(ConsoleError::Http { status, message: "x".into() });
        let s = Duration::from_secs;
        assert_eq!(beat_outcome(Ok(beat(ShareRole::Player)), s(0)), BeatOutcome::Reply(beat(ShareRole::Player)));
        assert_eq!(beat_outcome(http(404), s(0)), BeatOutcome::Gone(ShareEnd::Stopped), "invité inconnu : tout de suite");
        // Kyber sans clé de jetons (503), réseau : réessayé, jamais une fin immédiate…
        assert_eq!(beat_outcome(http(503), s(0)), BeatOutcome::Retry);
        assert_eq!(beat_outcome(http(503), s(19)), BeatOutcome::Retry);
        assert_eq!(beat_outcome(Err(ConsoleError::Network("hors ligne".into())), s(10)), BeatOutcome::Retry);
        // … jusqu'aux 20 s après lesquelles la Station a fait sortir l'invité.
        assert_eq!(beat_outcome(http(503), s(20)), BeatOutcome::Lost);
        assert_eq!(beat_outcome(Err(ConsoleError::Network("hors ligne".into())), s(25)), BeatOutcome::Lost);
        assert_eq!(GUEST_BEAT_LIMIT, s(20));
        assert!(!t("share.lost", &[]).is_empty());
    }

    #[test]
    fn nouvel_ecran_du_partage() {
        let (mut w, recorder) = guest(ShareRole::Spectator);
        w.streaming = Some(display(7, 0, 0, 1920, 1080));
        w.displays = vec![display(7, 0, 0, 1920, 1080)];
        // Même écran : rien.
        assert!(w.on_share_beat(beat(ShareRole::Spectator)).is_none());
        assert!(w.streaming.is_some() && !w.share_reconnecting);
        // L'hôte a refait l'écran : reconnexion à Kyber avec le jeton du signe de vie.
        let moved = ShareBeat { display: Some(ShareDisplay { id: 9, width: 2560, height: 1440, refresh: 144 }), ..beat(ShareRole::Spectator) };
        assert!(w.on_share_beat(moved).is_none());
        let g = w.guest.as_ref().unwrap();
        assert_eq!((g.joined.display.id, g.joined.kyber.token.as_str()), (9, "jwt-2"));
        assert!(w.streaming.is_none() && w.displays.is_empty() && w.share_reconnecting, "image arrêtée, liste attendue de la nouvelle connexion");
        assert_eq!(w.stream_mode, Mode { width: 2560, height: 1440, refresh: 144 });
        assert_eq!(recorder.modes.lock().unwrap().last(), Some(&(Mode { width: 2560, height: 1440, refresh: 144 }, false)));
        // L'ancienne session de Kyber close par le cœur : pas une coupure.
        assert!(w.on_event(Event::Stopped { reason: StopReason::Requested }).is_none());
        assert!(w.on_event(Event::ControlPlaneConnected { displays: vec![display(9, 0, 0, 2560, 1440)] }).is_none());
        assert!(!w.share_reconnecting);
        assert_eq!(share_display(9, &w.displays).map(|d| d.id), Some(9), "l'image repart sur le nouvel écran");
        assert!(w.share_end.is_none() && recorder.events.lock().unwrap().iter().all(|e| !e.contains("fin du partage")), "pas une fin");
        // Ensuite, un arrêt de Kyber est de nouveau une coupure.
        assert!(w.on_event(Event::Stopped { reason: StopReason::Requested }).is_some());
        // Joueur et nouvel écran ensemble : la reconnexion ouvre aussi les entrées.
        let (mut w, _) = guest(ShareRole::Spectator);
        w.streaming = Some(display(7, 0, 0, 1920, 1080));
        let both = ShareBeat { display: Some(ShareDisplay { id: 4, width: 1920, height: 1080, refresh: 60 }), ..beat(ShareRole::Player) };
        assert!(w.on_share_beat(both).is_none());
        assert!(w.share_reconnecting && w.start_at.is_none() && w.wants_input());
    }

    #[test]
    fn changement_de_qualite_trop_rapproche() {
        use crate::console::QualityRefusal;
        use std::cell::RefCell;
        let image = || own(1280, 720, 30, share::ShareImageReason::Asked);
        let too_soon = || Err(QualityRefusal::TooSoon(Duration::from_secs(1)));
        // Refusé une fois : réessayé après le délai, l'image revient.
        let answers = RefCell::new(vec![Ok(image()), too_soon()]);
        let slept = RefCell::new(Vec::new());
        let got = ask_quality(|| answers.borrow_mut().pop().unwrap(), |d| slept.borrow_mut().push(d), || true);
        assert_eq!((got, slept.into_inner()), (Some(image()), vec![Duration::from_secs(1)]));
        // Une autre qualité demandée pendant l'attente : celle-ci n'est pas réessayée.
        let calls = RefCell::new(0);
        let got = ask_quality(
            || {
                *calls.borrow_mut() += 1;
                too_soon()
            },
            |_| {},
            || false,
        );
        assert_eq!((got, calls.into_inner()), (None, 1));
        // Encore trop tôt : une seule reprise, rien rendu à l'app.
        let calls = RefCell::new(0);
        let got = ask_quality(
            || {
                *calls.borrow_mut() += 1;
                too_soon()
            },
            |_| {},
            || true,
        );
        assert_eq!((got, calls.into_inner()), (None, 2));
        // Un autre échec : pas de reprise.
        let calls = RefCell::new(0);
        let got = ask_quality(
            || {
                *calls.borrow_mut() += 1;
                Err(QualityRefusal::Failed(ConsoleError::Network("hors ligne".into())))
            },
            |_| {},
            || true,
        );
        assert_eq!((got, calls.into_inner()), (None, 1));
        // Le dernier choix gagne : chaque demande prend son tour.
        let (mut w, _) = guest(ShareRole::Spectator);
        let port = w.params.console_port;
        w.console = Some(Arc::new(Console::guest("127.0.0.1", port, None)));
        w.ask_share_quality(ShareQuality::P2160_60);
        w.ask_share_quality(ShareQuality::P720_30);
        assert_eq!(w.quality_asked.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn kyber_coupe_avant_le_signe_de_vie() {
        assert_eq!(share_gone(404), Some(ShareEnd::Stopped), "unknown_guest : partage fermé, ou retrait ancien");
        for status in [401, 403, 429, 500, 503] {
            assert_eq!(share_gone(status), None, "{status}");
        }
        // Station injoignable : la raison de Kyber reste, sans attendre.
        let (mut w, _) = guest(ShareRole::Spectator);
        let port = w.params.console_port;
        w.console = Some(Arc::new(Console::guest("127.0.0.1", port, None)));
        let start = Instant::now();
        let f = w.share_reason(Failure::kyber("session interrompue".into()));
        assert_eq!(f.text, "session interrompue");
        assert!(start.elapsed() < Duration::from_secs(4));
        // Déjà finie par le partage : rien à demander.
        w.share_end = Some((ShareEnd::Removed, "retiré".into()));
        assert_eq!(w.share_reason(Failure::kyber("autre".into())).text, "autre");
    }

    #[test]
    fn format_lisible_au_lieu_de_refuser() {
        // Sans l'empreinte de Kyber, `open_guest` s'arrête juste après le
        // choix du format : on y lit ce qui serait demandé à Kyber.
        let pick = |codec: &str, yuv444: bool, caps: crate::profile::Capabilities| {
            let mut j = joined(ShareRole::Spectator, Some(codec), None);
            j.yuv444 = yuv444;
            let (mut w, recorder) = guest_with(j, caps);
            let f = w.open_guest().unwrap_err();
            assert_eq!(f.text, t("share.no_kyber_fingerprint", &[]), "jamais de premier contact");
            assert!(w.share_end.is_none() && w.client.is_none());
            assert!(recorder.events.lock().unwrap().iter().all(|e| !e.contains("fin du partage")), "on entre");
            (w.params.settings.codec, w.params.settings.yuv444)
        };
        use crate::profile::{Capabilities, Codec};
        let hevc = Capabilities { hevc: true, hevc444: false, av1: false };
        let hevc444 = Capabilities { hevc: true, hevc444: true, av1: false };
        let h264 = Capabilities::default();
        assert_eq!(pick("hevc", true, hevc444), (Codec::Hevc, true), "le format de l'hôte, tel quel");
        assert_eq!(pick("hevc", true, hevc), (Codec::Hevc, false), "sans son 4:4:4 : image à part en 4:2:0");
        assert_eq!(pick("av1", false, hevc), (Codec::Hevc, false), "AV1 illisible : un format lisible");
        assert_eq!(pick("hevc", false, h264), (Codec::H264, false), "H.264 sinon");
        assert_eq!(pick("h264", false, hevc444), (Codec::H264, false));
    }

    fn own(width: u32, height: u32, fps: u32, reason: share::ShareImageReason) -> ShareImage {
        ShareImage {
            quality: ShareQuality::P1080_60,
            width: Some(width),
            height: Some(height),
            fps: Some(fps),
            codec: Some("h264".into()),
            follows_host: Some(false),
            reason: Some(reason),
        }
    }

    #[test]
    fn image_de_l_invite() {
        use share::ShareImageReason as R;
        let (mut w, recorder) = guest(ShareRole::Spectator);
        // Sur l'image de l'hôte : images clés espacées, débit à l'hôte.
        assert_eq!(w.keyframe_timing(), (Duration::from_secs(2), Duration::from_secs(5)));
        w.apply_bitrate();
        assert!(w.refused.lock().unwrap().contains(&HostOnly::Bitrate));
        w.refused.lock().unwrap().clear();
        // Son image à part : annoncée sans avis, débit et images clés à lui.
        let mut b = beat(ShareRole::Spectator);
        b.image = Some(own(1920, 1080, 60, R::Asked));
        assert!(w.on_share_beat(b.clone()).is_none());
        assert!(w.own_image());
        assert_eq!(w.keyframe_timing(), (Duration::from_millis(300), Duration::from_secs(5)));
        w.apply_bitrate();
        assert!(w.refused.lock().unwrap().is_empty(), "son image à part : il en règle le débit");
        assert_eq!(w.profile_bitrate(), w.params.settings.bitrate(1920, 1080, 60), "le débit de son image");
        // La même image : rien de nouveau.
        assert!(w.on_share_beat(b).is_none());
        // En retard : un cran plus bas, avec l'avis ; plus d'image à part : l'image ouverte, avec l'autre avis.
        w.share_image_changed(own(1280, 720, 30, R::Lag));
        w.share_image_changed(own(1280, 720, 30, R::Limit));
        // Comme l'hôte : rien à dire.
        w.share_image_changed(ShareImage { follows_host: Some(true), ..own(3840, 2160, 120, R::Capped) });
        let images = recorder.images.lock().unwrap().clone();
        let lang = i18n::current();
        assert_eq!(
            images.iter().map(|(i, p, n)| (i.height, p.as_ref().and_then(|p| p.height), n.clone())).collect::<Vec<_>>(),
            [
                (Some(1080), None, None),
                (Some(720), Some(1080), Some(i18n::tr(lang, "share.image_lag", &[]))),
                (Some(720), Some(720), Some(i18n::tr(lang, "share.image_limit", &[("quality", "720p · 30 Hz")]))),
                (Some(2160), Some(720), None)
            ]
        );
        assert!(!w.own_image());
        // Une image à sa mesure ne change pas l'écran de l'ordinateur (repère de la souris).
        w.streaming = Some(display(7, 0, 0, 3840, 2160));
        w.host_size = (3840, 2160);
        w.decoded(3840, 2160);
        w.decoded(1280, 720);
        assert_eq!(w.host_size, (3840, 2160));
        // Qualité choisie : demandée à la Station, l'image revient par le fil de session.
        w.tx.send(Msg::ShareQuality(ShareQuality::P720_30)).unwrap();
        w.tx.send(Msg::ShareImage(own(1280, 720, 30, R::Asked))).unwrap();
        w.tx.send(Msg::Stop).unwrap();
        assert!(w.event_loop().is_none());
        assert_eq!(recorder.images.lock().unwrap().last().map(|(i, _, _)| (i.height, i.reason)), Some((Some(720), Some(R::Asked))));
        // Pas encore su (tout à `null`) : compté comme l'image de l'hôte, sans avis.
        w.share_image_changed(ShareImage { quality: ShareQuality::Auto, ..ShareImage::default() });
        assert!(!w.own_image());
        assert_eq!(w.keyframe_timing(), (Duration::from_secs(2), Duration::from_secs(5)));
        assert_eq!(recorder.images.lock().unwrap().last().map(|(_, _, n)| n.clone()), Some(None));
        // Un hôte : rien à demander, images clés et relance comme toujours.
        let (mut host, _) = worker(crate::profile::Profile::Jeu, two_screens(0));
        host.ask_share_quality(ShareQuality::Host);
        assert_eq!(host.keyframe_timing(), (Duration::from_millis(300), Duration::from_millis(1500)));
    }

    #[test]
    fn les_invites_de_l_hote() {
        let (mut w, recorder) = worker(crate::profile::Profile::Jeu, two_screens(0));
        let share: ShareState = serde_json::from_value(serde_json::json!({
            "open": true, "code": "123456", "guests": [{ "id": "g1", "name": "Léa", "app": "ios", "role": "spectator", "since": 1 }]
        }))
        .unwrap();
        let with = |s: Option<ShareState>| HeartbeatReply { share: s, ..HeartbeatReply::default() };
        assert!(w.on_beat(with(None)).is_none());
        assert!(recorder.guests.lock().unwrap().is_empty(), "pas de partage, rien de nouveau");
        w.on_beat(with(Some(share.clone())));
        w.on_beat(with(Some(share.clone())));
        let mut two = share.clone();
        two.guests.push(ShareGuest { id: "g2".into(), name: "Tom".into(), role: ShareRole::Player, ..ShareGuest::default() });
        w.on_beat(with(Some(two)));
        w.on_beat(with(None));
        assert_eq!(*recorder.guests.lock().unwrap(), [Some(1), Some(2), None], "à chaque changement seulement");
    }

    #[test]
    fn parametres_en_json() {
        let host = r#"{"console_host":"192.168.1.20","console_port":9443,"user":"u","password":"p","console_fingerprint":"",
            "device":{"id":"mac","name":"Mac","kind":"mac","os":"macOS","app":"test"},"mode":{"width":3840,"height":2160,"refresh":120},
            "settings":{"profile":"bureau","codec":"hevc","yuv444":true,"mbps":90,"reliable":true},
            "screens":{"list":[{"x":0,"y":0,"width":1920,"height":1080,"pixel_width":3840,"pixel_height":2160,"refresh":120,"primary":true}]}}"#;
        let (p, screens) = SessionParams::from_json(host).unwrap();
        assert!(!p.is_guest() && p.virtual_display && p.clipboard && p.recovery.restart_station);
        assert_eq!((p.mode.width, p.console_fingerprint.clone()), (3840, None), "empreinte vide : aucune");
        assert_eq!(p.settings.profile, Some(crate::profile::Profile::Bureau));
        assert_eq!(screens.map(|s| s.list.len()), Some(1));
        assert!(SessionParams::from_json(&host.replace(r#""mode":{"width":3840,"height":2160,"refresh":120},"#, "")).is_err(), "un hôte donne son mode");
        // Un invité : ni compte ni mode, l'écran du partage.
        let guest_json = serde_json::json!({
            "console_host": "192.168.1.20", "console_port": 9443, "console_fingerprint": "sha256:ad",
            "device": { "id": "tv", "name": "Salon", "kind": "webos", "os": "webOS", "app": "test" },
            "settings": { "codec": "hevc", "mbps": 60 },
            "guest": { "joined": serde_json::to_value(joined(ShareRole::Player, Some("hevc"), Some("sha256:kk"))).unwrap(), "capabilities": { "hevc": true } }
        });
        let (p, screens) = SessionParams::from_json(&guest_json.to_string()).unwrap();
        assert!(p.is_guest() && screens.is_none());
        assert_eq!(p.mode, Mode { width: 1920, height: 1080, refresh: 60 });
        assert!(p.user.is_empty() && p.password.is_empty());
        let SessionRole::Guest(g) = &p.role else { panic!() };
        assert_eq!((g.joined.role, g.joined.display.id, g.capabilities.hevc), (ShareRole::Player, 7, true));
        // Le `auto` mesuré à `join` arrive avec `joined` : la session le renverra avec « Auto ».
        let mut j = joined(ShareRole::Player, Some("hevc"), Some("sha256:kk"));
        j.auto = Some(share::ShareAuto { width: 2556, height: 1179, fps: 60, codecs: vec!["h264".into()], yuv444: false });
        let mut with_auto = guest_json.clone();
        with_auto["guest"]["joined"] = serde_json::to_value(&j).unwrap();
        let (p, _) = SessionParams::from_json(&with_auto.to_string()).unwrap();
        let SessionRole::Guest(g) = &p.role else { panic!() };
        assert_eq!(g.joined.auto.as_ref().map(|a| (a.width, a.fps)), Some((2556, 60)));
        // Les paramètres d'un invité d'après la Station trouvée.
        let (w, _) = guest(ShareRole::Spectator);
        assert_eq!((w.params.console_host.as_str(), w.params.console_fingerprint.as_deref()), ("127.0.0.1", Some("sha256:ad")));
        assert!(!w.params.virtual_display && w.params.user.is_empty());
        assert_eq!(w.params.mode, Mode { width: 1920, height: 1080, refresh: 60 });
    }

    #[test]
    fn image_montree_avant_une_remise_a_zero() {
        let l = LatencyTracker::new();
        assert!(!l.any_displayed());
        l.displayed(1, crate::latency::now_micros());
        l.reset();
        assert!(l.any_displayed(), "la session a eu ses premières images");
    }
}
