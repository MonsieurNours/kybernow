//! Une session : préparation par la console (écran virtuel, son), connexion à
//! Kyber, choix de l'écran, image et son vers l'app, signal de vie, reprise
//! après une perte, fin propre. Même déroulé que `SessionController.swift`
//! de l'app Mac, dont le cœur reprend aussi la continuité de l'image : image
//! clé redemandée puis flux relancé, repli quand aucune image ne vient,
//! taille de l'écran de l'ordinateur suivie, politique de mode (« Remettre »),
//! adaptation à un nouveau mode de l'écran de l'appareil.
//!
//! Tout se décide sur un fil dédié ; les événements de Kyber, le signal de vie
//! et les demandes de l'app lui arrivent par un canal.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use kyclient::{
    AudioProtocol, AudioSessionConfig, AuthCredentials, Client, ConnectConfig, Display, Event,
    EventSink, InputSessionConfig, KyProtoConfig, KymuxConfig, StreamerPipelineEventType,
    StreamingProtocol, StreamingSessionConfig, VideoPlayerConfig, VideoProtocol,
    VideoSessionConfig,
};
use kynput_rs::{ControlEvent, InputTarget, Pipeline};
use kyvlcplayer::WindowHandle;
use log::{info, warn};

use crate::console::{Console, DeviceInfo, Heartbeat, HeartbeatReply, Mode, PrepareRequest, Prepared};
use crate::i18n::{self, Lang};
use crate::latency::{LatencyTracker, Second};
use crate::media::{PacketHandler, packet_sink};
use crate::profile::StreamSettings;

/// Reprise de l'image, comme l'app Mac. Par défaut (Windows), la plateforme
/// tient elle-même ses veilles et appelle [`Session::request_keyframe`] et
/// [`Session::no_image`].
#[derive(Debug, Clone, Default)]
pub struct Recovery {
    /// Le cœur veille lui-même à la première image (4 s après le démarrage du
    /// flux, d'après [`Session::frame_displayed`]) et aux images clés
    /// ([`Session::keyframe_lost`] : redemandée toutes les 300 ms, flux relancé
    /// à 1,5 s).
    pub watch_image: bool,
    /// Sans image : le flux est d'abord allégé, pour cette session seulement
    /// (débit ramené à ce plafond, puis 4:2:0), avant d'être relancé.
    pub fallback_bitrate: Option<u32>,
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
}

enum Msg {
    Event(Event),
    Stop,
    Fail(String),
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
    /// « Remettre » : le mode du départ redemandé à la console.
    RestoreMode,
    /// Réponses de la console, depuis un fil d'appel.
    Adapted(Mode, Result<Prepared, String>),
    ModeRestored,
}

struct Events(Mutex<Sender<Msg>>);

impl EventSink for Events {
    fn on_event(&self, event: &Event) {
        // Appelé sous le verrou de Kyber : on ne fait que transmettre.
        let _ = self.0.lock().unwrap().send(Msg::Event(event.clone()));
    }
}

pub struct Session {
    tx: Mutex<Sender<Msg>>,
    latency: Arc<LatencyTracker>,
    has_image: Arc<AtomicBool>,
    thread: Mutex<Option<JoinHandle<()>>>,
}

impl Session {
    pub fn start(params: SessionParams, platform: Arc<dyn Platform>) -> Arc<Session> {
        let (tx, rx) = mpsc::channel();
        let latency = Arc::new(LatencyTracker::new());
        let has_image = Arc::new(AtomicBool::new(false));
        let profile_label = Arc::new(Mutex::new((params.settings.label().to_string(), params.settings.mode_policy().to_string())));
        let worker = Worker {
            requested: params.mode,
            stream_mode: params.mode,
            params,
            platform,
            tx: tx.clone(),
            rx,
            latency: latency.clone(),
            has_image: has_image.clone(),
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
            device_id: String::new(),
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
        };
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
    /// au plus, puis la session s'arrête avec la raison. Comme l'app Mac.
    pub fn no_image(&self) {
        self.send(Msg::NoImage);
    }

    /// Change de profil pendant la session (barre) : nouveau débit sur-le-champ,
    /// ou flux relancé si le format, le 4:4:4 ou le transport changent. Comme sur Mac.
    pub fn set_settings(&self, settings: StreamSettings) {
        self.send(Msg::Settings(settings));
    }

    /// Plafond de débit le temps que la fenêtre est occultée ; `None` le retire.
    pub fn set_bitrate_cap(&self, cap: Option<u32>) {
        self.send(Msg::BitrateCap(cap));
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

    /// Image `pts` reçue, décodée puis montrée (µs, horloge
    /// [`crate::latency::now_micros`]) : latence, et première image du flux.
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
}

/// Raison d'une fin de session, dans la langue de l'app (montrée à l'utilisateur).
fn t(key: &str, params: &[(&str, &str)]) -> String {
    i18n::tr(i18n::current(), key, params)
}

fn usable(d: &Display) -> bool {
    d.width > 0 && d.height > 0
}

impl Worker {
    fn run(mut self) {
        self.device_id = self.params.device.id.clone();
        let error = match self.open() {
            Ok(()) => self.event_loop(),
            Err(e) => Some(e),
        };
        self.finish(error);
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
        let p = &self.params;
        PrepareRequest {
            width: mode.width,
            height: mode.height,
            refresh: mode.refresh,
            audio_channels: p.audio_channels,
            virtual_display: p.virtual_display,
            device: p.device.clone(),
            profile: Some(self.profile_label.lock().unwrap().0.clone()),
            mode_policy: Some(mode_policy.unwrap_or(&self.profile_label.lock().unwrap().1).to_string()),
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
        self.prepared = Some(prepared);
    }

    /// Mode obtenu de la console au début de la session (ou après une adaptation).
    fn prepared_size(&self) -> Option<(u32, u32)> {
        self.prepared.as_ref().and_then(|p| p.applied).map(|m| (m.width, m.height))
    }

    fn virtual_display(&self) -> bool {
        self.prepared.as_ref().is_some_and(|p| p.virtual_display)
    }

    fn open(&mut self) -> Result<(), String> {
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
        client
            .get_event_manager()
            .add_event_sink(Box::new(Events(Mutex::new(self.tx.clone()))));
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
                    match console.heartbeat(&hb) {
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

    fn effective_bitrate(&self) -> u32 {
        let m = self.stream_mode;
        let mut b = self.params.settings.bitrate(m.width, m.height, m.refresh);
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

    fn event_loop(&mut self) -> Option<String> {
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
                Ok(Msg::Adapted(mode, result)) => self.adapted(mode, result),
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
        if let Some(c) = &self.client {
            self.latency.keyframe_requested();
            let _ = c.request_keyframe();
        }
    }

    /// Veilles du cœur (`watch_image`) : première image en 4 s, image clé en
    /// 1,5 s (redemandée toutes les 300 ms).
    fn watch_image(&mut self, now: Instant) -> Option<String> {
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
        let m = self.stream_mode;
        let mut next = self.fallback + 1;
        // Cran sans effet (débit déjà bas) : on passe au suivant.
        if next == 1 && self.params.settings.bitrate(m.width, m.height, m.refresh) <= cap {
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

    fn no_image_restart(&mut self) -> Option<String> {
        if self.streaming.is_some() && self.start_at.is_none() {
            self.no_image += 1;
            if self.no_image > 3 {
                return Some(t("session.no_image", &[]));
            }
            warn!("aucune image : flux relancé ({}/3)", self.no_image);
            self.restart_streaming(Duration::from_millis(500));
        }
        None
    }

    fn on_beat(&mut self, r: HeartbeatReply) -> Option<String> {
        if r.end {
            info!("la console met fin à la session");
            return None.or_else(|| {
                let _ = self.tx.send(Msg::Stop);
                None
            });
        }
        if r.active == Some(false) {
            // Comme sur Mac : redemandé, au plus une fois par 30 s.
            let recent = self.last_restore.is_some_and(|t| t.elapsed() < Duration::from_secs(30));
            if self.virtual_display() && self.streaming.is_some() && !self.adapting && !recent {
                info!("la console n'a plus d'écran virtuel : redemandé");
                self.last_restore = Some(Instant::now());
                self.stop_streaming();
                if let Err(e) = self.restore_screen() {
                    return Some(e);
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

    fn on_event(&mut self, event: Event) -> Option<String> {
        match event {
            Event::ControlPlaneConnected { displays } | Event::DisplayListUpdated { displays } => {
                self.displays_changed(displays);
            }
            Event::ControlPlaneConnectFailed { reason } => {
                return Some(match reason {
                    // Compte d'appareil tout juste créé par la console : Kyber
                    // ne le lit qu'à son démarrage, que la console reporte
                    // pendant une session.
                    kyclient::ConnectFailedReason::Unauthorized => t("session.unauthorized", &[]),
                    kyclient::ConnectFailedReason::TlsVerificationFailed => t("session.tls", &[]),
                    other => t("session.connect_failed", &[("reason", &format!("{other:?}"))]),
                });
            }
            Event::DataPlaneConnectFailed => return Some(t("session.stream_failed", &[])),
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
            Event::StreamingStartFailed => return Some(t("session.image_refused", &[])),
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
                        return Some(e);
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
            Event::ReconnectionFailed => return Some(t("session.not_responding", &[])),
            Event::Stopped { reason } => {
                if !self.ending {
                    return Some(t("session.interrupted", &[("reason", &format!("{reason:?}"))]));
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
    fn pipeline_failed(&mut self, kind: &str) -> Option<String> {
        warn!("{}", i18n::tr(Lang::Fr, "session.pipeline", &[("kind", kind)]));
        if self.adapting {
            info!("image refusée pendant que l'écran virtuel revient : on attend");
            return None;
        }
        if let Some(at) = self.reconnected_at {
            if at.elapsed() < Duration::from_secs(15) && !self.restored_after_reconnect {
                self.restored_after_reconnect = true;
                self.stop_streaming();
                return self.restore_screen().err();
            }
        }
        if self.last_retry.is_some_and(|t| t.elapsed() > Duration::from_secs(30)) {
            self.retries = 0;
        }
        if self.retries >= 3 {
            return Some(t("session.pipeline", &[("kind", kind)]));
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
                let _ = tx.send(Msg::Adapted(mode, r));
            })
            .expect("fil de préparation");
    }

    fn adapted(&mut self, mode: Mode, result: Result<Prepared, String>) {
        self.adapting = false;
        match result {
            Ok(p) => {
                self.requested = mode;
                self.apply_prepared(p);
                let m = self.stream_mode;
                self.pending_notice = Some(Notice::Adapted { width: m.width, height: m.height });
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

    /// Avec l'écran virtuel, attend (6 s au plus) que Kyber n'annonce plus que
    /// lui, puis démarre l'image sur le meilleur écran.
    fn choose_display(&mut self, deadline_passed: bool) {
        if self.adapting {
            return;
        }
        let usable_count = self.displays.iter().filter(|d| usable(d)).count();
        if usable_count == 0 {
            if deadline_passed && self.wait_until.is_some() {
                self.wait_until = None;
                let _ = self.tx.send(Msg::Fail(t("session.no_display", &[])));
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

    fn start_streaming(&mut self, display: Display) {
        let Some(client) = self.client.clone() else { return };
        let s = self.effective();
        self.host_size = (display.width as u32, display.height as u32);
        self.list_size = self.host_size;
        self.frame_size = None;
        self.stream_mode.width = self.host_size.0;
        self.stream_mode.height = self.host_size.1;
        let bitrate = self.effective_bitrate();
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
        let pipeline = self.platform.input_pipeline();
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
            video: vec![VideoSessionConfig {
                display_id: display.id,
                codec: s.codec.kyber(),
                yuv444: s.yuv444,
                intra_refresh: None,
                player: VideoPlayerConfig {
                    window_handle: self.platform.window_handle(),
                    software_decode: false,
                    video_buffer: 0,
                    packet_sink: Some(packet_sink(self.platform.video_handler())),
                },
            }],
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
                let _ = self.tx.send(Msg::Fail(t("session.start_image", &[("error", &e.to_string())])));
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
        let mut report = None;
        if let Some(console) = &self.console {
            let latency = if error.is_none() { self.latency.session().0 } else { None };
            report = Some(match console.end(&self.device_id, latency) {
                Ok(e) => {
                    if !(e.display_restored && e.audio_restored) {
                        warn!("fin : écrans {} son {} ({:?})", e.display_restored, e.audio_restored, e.error);
                    }
                    EndReport {
                        display_restored: e.display_restored,
                        audio_restored: e.audio_restored,
                        error: e.error,
                    }
                }
                Err(e) => {
                    warn!("fin de session : {}", e.text(Lang::Fr));
                    EndReport {
                        display_restored: false,
                        audio_restored: false,
                        error: Some(e.to_string()),
                    }
                }
            });
        }
        self.status(Status::Ended { error, report });
    }
}
