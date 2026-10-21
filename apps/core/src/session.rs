//! Une session : préparation par la console (écran virtuel, son), connexion à
//! Kyber, choix de l'écran, image et son vers l'app, signal de vie, reprise
//! après une perte, fin propre. Même déroulé que `SessionController.swift`.
//!
//! Tout se décide sur un fil dédié ; les événements de Kyber, le signal de vie
//! et les demandes de l'app lui arrivent par un canal.

use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::atomic::{AtomicBool, Ordering};
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

use crate::console::{Console, DeviceInfo, Heartbeat, Mode, PrepareRequest, Prepared};
use crate::latency::LatencyTracker;
use crate::media::{PacketHandler, packet_sink};
use crate::profile::StreamSettings;

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
    Ended { error: Option<String> },
}

/// Ce que la plateforme fournit et reçoit (fenêtre, décodeurs, entrées).
pub trait Platform: Send + Sync {
    fn window_handle(&self) -> WindowHandle;
    fn video_handler(&self) -> Arc<dyn PacketHandler>;
    /// `None` : pas de son sur l'appareil.
    fn audio_handler(&self) -> Option<Arc<dyn PacketHandler>>;
    fn input_pipeline(&self) -> Pipeline;
    fn on_status(&self, status: &Status);
    /// L'écran de l'ordinateur affiché (repère des entrées).
    fn on_display(&self, display: &Display);
    fn on_input_connected(&self) {}
    /// Latence médiane de la dernière seconde.
    fn on_latency(&self, _ms: Option<f64>) {}
    /// Un jeu est en plein écran sur l'ordinateur (souris Auto).
    fn on_game_fullscreen(&self, _game: bool) {}
}

enum Msg {
    Event(Event),
    Stop,
    Fail(String),
    KeyframeNeeded,
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
    thread: Mutex<Option<JoinHandle<()>>>,
}

impl Session {
    pub fn start(params: SessionParams, platform: Arc<dyn Platform>) -> Arc<Session> {
        let (tx, rx) = mpsc::channel();
        let latency = Arc::new(LatencyTracker::new());
        let worker = Worker {
            params,
            platform,
            tx: tx.clone(),
            rx,
            latency: latency.clone(),
            console: None,
            client: None,
            prepared: None,
            displays: Vec::new(),
            streaming: None,
            last_shown: None,
            wait_until: None,
            start_at: None,
            retries: 0,
            last_retry: None,
            reconnected_at: None,
            restored_after_reconnect: false,
            ending: false,
            last_latency: Arc::new(Mutex::new(None)),
            device_id: String::new(),
            alive: Arc::new(AtomicBool::new(true)),
        };
        let thread = std::thread::Builder::new()
            .name("kybernow-session".into())
            .spawn(move || worker.run())
            .expect("fil de session");
        Arc::new(Session {
            tx: Mutex::new(tx),
            latency,
            thread: Mutex::new(Some(thread)),
        })
    }

    /// Demande la fin de la session (sans attendre).
    pub fn stop(&self) {
        let _ = self.tx.lock().unwrap().send(Msg::Stop);
    }

    /// Le décodeur a perdu une image de référence.
    pub fn request_keyframe(&self) {
        let _ = self.tx.lock().unwrap().send(Msg::KeyframeNeeded);
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
    console: Option<Arc<Console>>,
    client: Option<Arc<Client>>,
    prepared: Option<Prepared>,
    displays: Vec<Display>,
    streaming: Option<Display>,
    last_shown: Option<Display>,
    /// Attente que Kyber n'annonce plus que l'écran virtuel.
    wait_until: Option<Instant>,
    /// Démarrage de l'image prévu (relance).
    start_at: Option<Instant>,
    retries: u32,
    last_retry: Option<Instant>,
    reconnected_at: Option<Instant>,
    restored_after_reconnect: bool,
    ending: bool,
    last_latency: Arc<Mutex<Option<f64>>>,
    device_id: String,
    alive: Arc<AtomicBool>,
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

    fn prepare(&mut self) -> Result<(), String> {
        let console = self.console.clone().expect("console");
        let p = &self.params;
        let req = PrepareRequest {
            width: p.mode.width,
            height: p.mode.height,
            refresh: p.mode.refresh,
            audio_channels: p.audio_channels,
            virtual_display: p.virtual_display,
            device: p.device.clone(),
            profile: Some(p.settings.label().to_string()),
            mode_policy: Some(p.settings.mode_policy().to_string()),
        };
        let prepared = console.prepare(&req).map_err(|e| e.to_string())?;
        info!(
            "console : écran {:?}, son {:?}, port {:?}",
            prepared.applied, prepared.audio_channels, prepared.kyber_port
        );
        self.prepared = Some(prepared);
        Ok(())
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
        kyclient::init_crypto();
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
        let alive = self.alive.clone();
        std::thread::Builder::new()
            .name("kybernow-heartbeat".into())
            .spawn(move || {
                loop {
                    std::thread::sleep(Duration::from_secs(seconds));
                    let mut hb = hb.clone();
                    hb.latency_ms = latency.lock().unwrap().map(|l| (l * 10.0).round() / 10.0);
                    match console.heartbeat(&hb) {
                        Ok(r) if r.end => {
                            info!("la console met fin à la session");
                            let _ = tx.send(Msg::Stop);
                            return;
                        }
                        Ok(_) => {}
                        Err(e) => warn!("signal de vie : {e}"),
                    }
                    if !alive.load(Ordering::Relaxed) {
                        return;
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
                            platform.on_game_fullscreen(game);
                            last = Some(game);
                        }
                    }
                }
            })
            .expect("fil du premier plan");
    }

    fn event_loop(&mut self) -> Option<String> {
        let mut next_second = Instant::now() + Duration::from_secs(1);
        loop {
            match self.rx.recv_timeout(Duration::from_millis(100)) {
                Ok(Msg::Stop) => return None,
                Ok(Msg::Fail(e)) => return Some(e),
                Ok(Msg::KeyframeNeeded) => {
                    if let Some(c) = &self.client {
                        let _ = c.request_keyframe();
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
            }
            if now >= next_second {
                next_second += Duration::from_secs(1);
                let l = self.latency.drain_second();
                *self.last_latency.lock().unwrap() = l;
                self.platform.on_latency(l);
            }
        }
    }

    fn on_event(&mut self, event: Event) -> Option<String> {
        match event {
            Event::ControlPlaneConnected { displays } | Event::DisplayListUpdated { displays } => {
                self.displays_changed(displays);
            }
            Event::ControlPlaneConnectFailed { reason } => {
                return Some(format!("connexion à Kyber impossible ({reason:?})"));
            }
            Event::DataPlaneConnectFailed => return Some("connexion au flux impossible".into()),
            Event::StreamingStarted => {
                self.latency.reset();
                self.retries = 0;
                self.status(Status::Streaming);
            }
            Event::StreamingStartFailed => return self.pipeline_failed("l'ordinateur refuse l'image"),
            Event::StreamerPipelineEvent { type_, event, .. } => {
                if event == StreamerPipelineEventType::Error {
                    return self.pipeline_failed(&format!("erreur de la chaîne d'image ({type_:?})"));
                }
            }
            Event::Reconnecting { attempt, max_attempts } => {
                self.start_at = None;
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
                    self.choose_display(false);
                } else {
                    self.status(Status::Streaming);
                }
            }
            Event::ReconnectionFailed => return Some("l'ordinateur ne répond plus".into()),
            Event::Stopped { reason } => {
                if !self.ending {
                    return Some(format!("session interrompue ({reason:?})"));
                }
            }
            Event::InputServiceConnected => self.platform.on_input_connected(),
            _ => {}
        }
        None
    }

    /// Erreur de la chaîne d'image : écran redemandé juste après une
    /// reconnexion, sinon image relancée (3 fois au plus).
    fn pipeline_failed(&mut self, what: &str) -> Option<String> {
        warn!("{what}");
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
            return Some(what.to_string());
        }
        self.retries += 1;
        self.last_retry = Some(Instant::now());
        self.stop_streaming();
        self.start_at = Some(Instant::now() + Duration::from_secs(1));
        None
    }

    fn restore_screen(&mut self) -> Result<(), String> {
        self.status(Status::Connecting);
        self.prepare()?;
        self.wait_until = Some(Instant::now() + Duration::from_secs(3));
        self.choose_display(false);
        Ok(())
    }

    fn displays_changed(&mut self, displays: Vec<Display>) {
        info!("écrans de l'ordinateur : {displays:?}");
        self.displays = displays;
        if let Some(cur) = self.streaming.clone() {
            match self.displays.iter().find(|d| d.id == cur.id) {
                Some(d) if usable(d) => {
                    if d.width != cur.width || d.height != cur.height {
                        self.streaming = Some(d.clone());
                        self.platform.on_display(d);
                    }
                }
                _ => {
                    // Écran affiché disparu : l'image repart sur un autre.
                    self.stop_streaming();
                    self.start_at = Some(Instant::now() + Duration::from_millis(300));
                }
            }
        } else if self.start_at.is_none() {
            self.choose_display(false);
        }
    }

    /// Avec l'écran virtuel, attend (6 s au plus) que Kyber n'annonce plus que
    /// lui, puis démarre l'image sur le meilleur écran.
    fn choose_display(&mut self, deadline_passed: bool) {
        let usable_count = self.displays.iter().filter(|d| usable(d)).count();
        if usable_count == 0 {
            if deadline_passed && self.wait_until.is_some() {
                self.wait_until = None;
                let _ = self.tx.send(Msg::Fail("aucun écran utilisable sur l'ordinateur".into()));
            }
            return;
        }
        let virtual_display = self.prepared.as_ref().is_some_and(|p| p.virtual_display);
        if virtual_display && usable_count != 1 && !deadline_passed {
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
        if list.len() == 1 {
            return Some(list[0].clone());
        }
        if let Some(last) = &self.last_shown {
            if let Some(d) = list.iter().find(|d| {
                d.x == last.x && d.y == last.y && d.width == last.width && d.height == last.height
            }) {
                return Some((*d).clone());
            }
        }
        let applied = self.prepared.as_ref().and_then(|p| p.applied);
        if let Some(m) = applied {
            let same: Vec<&&Display> = list
                .iter()
                .filter(|d| d.width as u32 == m.width && d.height as u32 == m.height)
                .collect();
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
        let s = &self.params.settings;
        let mode = self
            .prepared
            .as_ref()
            .and_then(|p| p.applied)
            .unwrap_or(self.params.mode);
        let bitrate = s.bitrate(mode.width, mode.height, mode.refresh);
        info!(
            "image : écran {} ({}×{}), {} {}, {} Mbit/s, {}",
            display.id,
            display.width,
            display.height,
            s.codec.label(),
            if s.yuv444 { "4:4:4" } else { "4:2:0" },
            bitrate / 1_000_000,
            if s.reliable { "fiable" } else { "rapide" }
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
                pipeline: self.platform.input_pipeline(),
                gamepad_force_xbox360: Some(true),
                clipboard: true,
            }),
        };
        self.platform.on_display(&display);
        self.streaming = Some(display.clone());
        self.last_shown = Some(display);
        self.status(Status::FirstImages);
        if let Err(e) = client.start_streaming(config) {
            let _ = self.tx.send(Msg::Fail(format!("démarrage de l'image : {e}")));
        }
    }

    fn stop_streaming(&mut self) {
        if let Some(c) = &self.client {
            let _ = c.stop_streaming();
        }
        self.streaming = None;
    }

    fn finish(&mut self, error: Option<String>) {
        self.ending = true;
        self.alive.store(false, Ordering::Relaxed);
        self.status(Status::Ending);
        // Rien ne reste enfoncé sur l'ordinateur.
        let pipeline = self.platform.input_pipeline();
        pipeline
            .get_input_sink()
            .consume(ControlEvent::FlushKeys.to_packet(InputTarget::Host));
        if let Some(c) = self.client.take() {
            let _ = c.disconnect_and_stop(true);
        }
        if let Some(console) = &self.console {
            let latency = if error.is_none() { self.latency.session().0 } else { None };
            match console.end(&self.device_id, latency) {
                Ok(e) if !(e.display_restored && e.audio_restored) => {
                    warn!("fin : écrans {} son {} ({:?})", e.display_restored, e.audio_restored, e.error)
                }
                Ok(_) => {}
                Err(e) => warn!("fin de session : {e}"),
            }
        }
        self.status(Status::Ended { error });
    }
}
