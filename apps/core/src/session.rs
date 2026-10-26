//! Une session : préparation par la console (écran virtuel, son), connexion à
//! Kyber, choix de l'écran, image et son vers l'app, signal de vie, reprise
//! après une perte, fin propre ; et la continuité de l'image : image clé
//! redemandée puis flux relancé, repli quand aucune image ne vient, taille
//! de l'écran de l'ordinateur suivie, politique de mode (« Remettre »),
//! adaptation à un nouveau mode de l'écran de l'appareil ; en dernier
//! recours, la Station redémarrée pendant la connexion (voir
//! [`Recovery::restart_station`]). Multi-écrans
//! ([`Session::start_with_screens`], docs/multi-ecrans.md) : un écran
//! virtuel par écran de l'appareil, une entrée vidéo par écran.
//!
//! Tout se décide sur un fil dédié ; les événements de Kyber, le signal de vie
//! et les demandes de l'app lui arrivent par un canal.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
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
use log::{info, warn};

use crate::console::{
    Console, ConsoleError, DeviceInfo, Heartbeat, HeartbeatReply, Mode, PrepareRequest, Prepared,
    ScreenMode, StationError,
};
use crate::i18n::{self, Lang};
use crate::latency::{LatencyTracker, Second};
use crate::media::{Packet, PacketHandler, packet_sink};
use crate::profile::StreamSettings;
use crate::screens::{self, PlannedScreen, Screens};

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

/// Écrans en place d'après la réponse de la Station : pour chaque écran du
/// plan créé (même ordre que la demande), l'écran de l'appareil et ce qui est
/// appliqué. Vide sans multi-écrans, avec une Station qui ne gère qu'un écran
/// (pas de `screens` dans la réponse), ou si l'écran principal manque.
fn targets(plan: &[PlannedScreen], prepared: &Prepared) -> Vec<(usize, ScreenMode)> {
    if plan.len() < 2 {
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

/// Écrans de l'appareil du plan que l'ordinateur n'a pas pu créer : sans
/// flux alors que la Station gère le multi-écrans (`targets` non vide). Vide
/// avec une Station à un écran : la session y est sur l'écran principal seul.
fn not_created(plan: &[PlannedScreen], targets: &[(usize, ScreenMode)], streams: &[StreamScreen]) -> Vec<usize> {
    if targets.is_empty() {
        return Vec::new();
    }
    plan.iter().map(|p| p.device).filter(|d| !streams.iter().any(|s| s.screen == *d)).collect()
}

pub struct Session {
    tx: Mutex<Sender<Msg>>,
    latency: Arc<LatencyTracker>,
    has_image: Arc<AtomicBool>,
    hidden: Arc<AtomicBool>,
    thread: Mutex<Option<JoinHandle<()>>>,
}

impl Session {
    /// Une session à un écran (celui de `params.mode`).
    pub fn start(params: SessionParams, platform: Arc<dyn Platform>) -> Arc<Session> {
        Self::launch(params, None, platform)
    }

    /// Une session qui prend les écrans de l'appareil (docs/multi-ecrans.md) :
    /// un écran virtuel par écran (quatre au plus, disposés comme sur
    /// l'appareil, voir [`crate::screens::plan`]), une entrée vidéo par écran.
    /// `params.mode` décrit l'écran principal, comme pour une session à un
    /// écran (il est remplacé par celui du plan). La plateforme reçoit les
    /// écrans pris par [`Platform::on_screens`] et les paquets de chacun par
    /// [`Platform::screen_video_handler`] ; avec une Station qui ne gère qu'un
    /// écran, la session se fait sur l'écran principal, de la même façon.
    pub fn start_with_screens(params: SessionParams, screens: Screens, platform: Arc<dyn Platform>) -> Arc<Session> {
        Self::launch(params, Some(screens), platform)
    }

    fn launch(params: SessionParams, screens: Option<Screens>, platform: Arc<dyn Platform>) -> Arc<Session> {
        let (tx, rx) = mpsc::channel();
        let latency = Arc::new(LatencyTracker::new());
        let has_image = Arc::new(AtomicBool::new(false));
        let hidden = Arc::new(AtomicBool::new(false));
        let worker = Worker::new(params, screens, platform, tx.clone(), rx, latency.clone(), has_image.clone(), hidden.clone());
        let guard_platform = worker.platform.clone();
        let thread = std::thread::Builder::new()
            .name("kybernow-session".into())
            .spawn(move || {
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
        })
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
    ) -> Self {
        let profile_label = Arc::new(Mutex::new((params.settings.label().to_string(), params.settings.mode_policy().to_string())));
        let plan = screens.as_ref().map(|s| screens::plan(s, params.settings.codec)).unwrap_or_default();
        // Le mode de l'écran principal, d'après le plan.
        let mode = plan.first().map_or(params.mode, |p| p.mode);
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
        }
    }

    fn run(mut self) {
        let failure = match self.attempt() {
            Some(f) if self.restart_wanted(&f) => self.last_resort(f),
            other => other,
        };
        self.finish(failure.map(|f| f.text));
    }

    /// Une tentative de connexion : préparation, Kyber, puis la session
    /// jusqu'à sa fin ; `None` : fin voulue (app, Admin).
    fn attempt(&mut self) -> Option<Failure> {
        match self.open() {
            Ok(()) => self.event_loop(),
            Err(e) => Some(e),
        }
    }

    fn restart_wanted(&self, f: &Failure) -> bool {
        restart_wanted(
            f.cause,
            Attempt {
                enabled: self.params.recovery.restart_station,
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

    /// `plan` : les écrans virtuels demandés (le champ `screens` n'est envoyé
    /// qu'à partir de deux ; `mode` est alors celui du principal).
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
            screens: (plan.len() > 1).then(|| plan.iter().map(PlannedScreen::request).collect()),
        }
    }

    fn prepare(&mut self) -> Result<(), String> {
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
        self.targets = targets(&self.plan, &prepared);
        if self.plan.len() > 1 {
            match &prepared.screens {
                None => info!("la Station ne gère qu'un écran : session sur l'écran principal"),
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
        let not_created = not_created(&self.plan, &self.targets, &self.streams);
        if !not_created.is_empty() {
            info!("écrans que l'ordinateur n'a pas pu créer : {not_created:?}");
        }
        self.platform.on_screens(&self.streams, &not_created);
    }

    /// L'écran principal de l'appareil (index dans [`Screens::list`]).
    fn primary_screen(&self) -> usize {
        self.plan.first().map_or(0, |p| p.device)
    }

    /// Mode obtenu de la console au début de la session (ou après une adaptation).
    fn prepared_size(&self) -> Option<(u32, u32)> {
        self.prepared.as_ref().and_then(|p| p.applied).map(|m| (m.width, m.height))
    }

    fn virtual_display(&self) -> bool {
        self.prepared.as_ref().is_some_and(|p| p.virtual_display)
    }

    fn open(&mut self) -> Result<(), Failure> {
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

    fn spawn_heartbeat(&self, seconds: u64) {
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
        std::thread::Builder::new()
            .name("kybernow-heartbeat".into())
            .spawn(move || {
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
        let console = self.console.clone().expect("console");
        let platform = self.platform.clone();
        let alive = self.alive.clone();
        std::thread::Builder::new()
            .name("kybernow-foreground".into())
            .spawn(move || {
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
                Ok(Msg::Settings(s)) => {
                    let before = self.effective();
                    self.params.settings = s.clone();
                    // Profil choisi à la main : le repli de la session repart de zéro.
                    self.fallback = 0;
                    *self.profile_label.lock().unwrap() = (s.label().to_string(), s.mode_policy().to_string());
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
    /// 1,5 s (redemandée toutes les 300 ms).
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
            if asked.elapsed() > Duration::from_millis(1500) {
                self.keyframe = None;
                warn!("pas d'image clé en 1,5 s : relance du flux");
                // Jamais d'image depuis le démarrage : le flux est trop lourd, on l'allège.
                if !self.has_image.load(Ordering::Relaxed) && self.degrade() {
                    return None;
                }
                // Ordinateur sans le correctif d'image clé : un flux neuf repart sur une image clé.
                if self.last_stall_restart.is_none_or(|t| t.elapsed() > Duration::from_secs(5)) {
                    self.last_stall_restart = Some(now);
                    self.restart_streaming(Duration::from_millis(300));
                }
            } else if last.elapsed() >= Duration::from_millis(300) {
                self.keyframe = Some((asked, now));
                self.request_keyframe();
            }
        }
        None
    }

    /// Allège le flux d'un cran (débit, puis 4:2:0) et le relance. Faux s'il
    /// n'y a plus rien à alléger.
    fn degrade(&mut self) -> bool {
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
            Event::ControlPlaneConnected { displays } | Event::DisplayListUpdated { displays } => {
                self.displays_changed(displays);
            }
            Event::ControlPlaneConnectFailed { reason } => {
                let text = match reason {
                    // Compte d'appareil tout juste créé par la console : Kyber
                    // ne le lit qu'à son démarrage, que la console reporte
                    // pendant une session.
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
            Event::StreamingStartFailed => return Some(Failure::no_image(t("session.image_refused", &[]))),
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
        let Some(console) = self.console.clone() else { return };
        if self.multi() {
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
        let plan = screens::plan(&screens, self.params.settings.codec);
        if plan.is_empty() || plan == self.plan {
            self.screens = Some(screens);
            return;
        }
        let mode = plan[0].mode;
        let Some(console) = self.console.clone() else { return };
        if !self.virtual_display() {
            // Sans écran virtuel, seul l'écran principal compte : l'image
            // repart vers le nouvel écran principal de l'appareil.
            info!("écrans de l'appareil changés (sans écran virtuel) : image relancée");
            let moved = plan[0].device != self.primary_screen();
            self.screens = Some(screens);
            self.plan = plan;
            if moved && self.streaming.is_some() {
                self.restart_streaming(Duration::from_millis(300));
            }
            return;
        }
        info!("écrans de l'appareil changés : {} écran(s), la session se refait", plan.len());
        // La connexion reprend à sa première étape sur l'écran principal
        // (docs/link-ui.md), sans capsule d'adaptation.
        self.status(Status::Preparing);
        self.stop_streaming();
        self.start_at = None;
        self.adapting = true;
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
                self.apply_prepared(p);
                if screens_changed {
                    self.status(Status::Connecting);
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
    }

    /// « Remettre » : la console remet le mode demandé au début de la session.
    fn start_restore_mode(&mut self) {
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
        std::thread::Builder::new()
            .name("kybernow-prepare".into())
            .spawn(move || {
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
            self.platform.audio_handler().map(|h| AudioSessionConfig {
                audio_buffer: None,
                packet_sink: Some(packet_sink(h)),
            })
        } else {
            None
        };
        self.platform.on_display(&display);
        let multi = self.multi();
        if multi {
            self.announce_screens();
        }
        let pipeline = self.platform.input_pipeline();
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
            input: Some(InputSessionConfig {
                pipeline: pipeline.clone(),
                gamepad_force_xbox360: Some(self.params.gamepad_xbox360),
                clipboard: self.params.clipboard,
            }),
        };
        self.pipeline = Some(pipeline);
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
        self.keyframe = None;
        self.image_deadline = None;
    }

    /// Arrêt puis démarrage sur le meilleur écran de la liste à jour.
    fn restart_streaming(&mut self, after: Duration) {
        self.stop_streaming();
        self.start_at = Some(Instant::now() + after);
    }

    fn finish(&mut self, error: Option<String>) {
        self.ending = true;
        self.alive.store(false, Ordering::Relaxed);
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
        let console = self.console.as_ref()?;
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
        // Un seul écran demandé : pas de multi-écrans.
        assert!(targets(&plan[..1], &prepared(Some(vec![Some(plan[0].request())]))).is_empty());
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
        assert_eq!(not_created(&plan, &t, &shown), [1]);
        assert!(not_created(&plan, &t, &match_screens(&t, &kyber)).is_empty());
        // Station à un écran : rien de « non créé », la session est sur l'écran principal seul.
        assert!(not_created(&plan, &[], &shown[..1]).is_empty());
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

    #[test]
    fn image_montree_avant_une_remise_a_zero() {
        let l = LatencyTracker::new();
        assert!(!l.any_displayed());
        l.displayed(1, crate::latency::now_micros());
        l.reset();
        assert!(l.any_displayed(), "la session a eu ses premières images");
    }
}
