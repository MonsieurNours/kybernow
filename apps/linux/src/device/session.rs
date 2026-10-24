//! Une session vue de Linux : ce que l'app fournit au cœur
//! (`session::Platform`) et les rappels vers la boucle principale, comme sur
//! webOS (apps/webos/src/device/session.rs) et Android.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex, OnceLock, Weak};

use kybernow_core::console::{DeviceInfo, Mode};
use kybernow_core::input::{Clock, GamepadRumble, InputPipeline, InputPlatform, MonotonicClock, Options, PadId, PadKey, Pasteboard, PointerLayer};
use kybernow_core::kynput_rs::Pipeline;
use kybernow_core::latency::Second;
use kybernow_core::media::PacketHandler;
use kybernow_core::profile::{Capabilities, Codec, Profile as CoreProfile, StreamSettings};
use kybernow_core::session::{Notice, Platform, Recovery, Session, SessionParams, Status};
use kybernow_core::{Display, WindowHandle};

use super::media::AudioSink;
use super::video::{Latest, VideoSink};
use super::Event;
use crate::app::Machine;
use crate::model::{Host, Profile};

/// Le presse-papiers du système, vu depuis le fil des entrées : SDL ne se
/// touche que depuis le fil principal, qui recopie ici le texte du système
/// et applique ce que l'ordinateur envoie.
#[derive(Default)]
pub struct Clipboard {
    text: Mutex<Option<String>>,
    count: AtomicU64,
    pub outgoing: Mutex<Option<String>>,
}

impl Clipboard {
    /// Le texte du système (fil principal).
    pub fn system_text(&self, t: Option<String>) {
        let mut cur = self.text.lock().unwrap();
        if *cur != t {
            *cur = t;
            self.count.fetch_add(1, Ordering::Relaxed);
        }
    }
}

impl Pasteboard for Clipboard {
    fn change_count(&self) -> u64 {
        self.count.load(Ordering::Relaxed)
    }
    fn text(&self) -> Option<String> {
        self.text.lock().unwrap().clone()
    }
    fn set_text(&self, text: &str) {
        *self.text.lock().unwrap() = Some(text.to_string());
        self.count.fetch_add(1, Ordering::Relaxed);
        *self.outgoing.lock().unwrap() = Some(text.to_string());
    }
}

/// Ce que partagent la session, ses flux et la boucle principale.
pub struct Shared {
    tx: Mutex<Sender<Event>>,
    session: OnceLock<Weak<Session>>,
    pub pointer: Arc<PointerLayer>,
    pub clock: Arc<MonotonicClock>,
    pub input: Mutex<Option<Arc<InputPipeline>>>,
    display: Mutex<(u32, u32, u32)>,
    mode: Mutex<Mode>,
    audio_channels: u8,
    pub combo_enabled: AtomicBool,
    pub relative: AtomicBool,
    codec: Mutex<String>,
    /// L'image la plus récente, pour le fil principal.
    pub latest: Latest,
    /// Décodeur matériel par format : H.264, HEVC, AV1 (VA-API, ou V4L2 sur le Pi).
    hw: [bool; 3],
    /// Lequel : 1 VA-API, 2 V4L2 (Raspberry Pi).
    pub hw_backend: i32,
    pub clipboard: Option<Arc<Clipboard>>,
    pub audio: Mutex<Option<Weak<AudioSink>>>,
}

impl Shared {
    pub fn session(&self) -> Option<Arc<Session>> {
        self.session.get().and_then(Weak::upgrade)
    }

    fn send(&self, e: Event) {
        let _ = self.tx.lock().unwrap().send(e);
    }

    pub fn host_size(&self) -> (u32, u32) {
        let d = self.display.lock().unwrap();
        if d.1 > 0 && d.2 > 0 {
            (d.1, d.2)
        } else {
            let m = self.mode.lock().unwrap();
            (m.width.max(1), m.height.max(1))
        }
    }

    pub fn current_input(&self) -> Option<Arc<InputPipeline>> {
        self.input.lock().unwrap().clone()
    }

    pub fn set_codec(&self, name: &str) {
        *self.codec.lock().unwrap() = name.to_string();
    }

    pub fn codec(&self) -> String {
        self.codec.lock().unwrap().clone()
    }

    pub fn decoder_failed(&self, e: &str) {
        self.send(Event::DecoderFailed(e.to_string()));
    }

    /// Un décodeur matériel pour ce format (0 H.264, 1 HEVC, 2 AV1).
    pub fn hw_for(&self, codec: i32) -> bool {
        self.hw.get(codec as usize).copied().unwrap_or(false)
    }

    /// Latence du son jusqu'à la sortie, si le moteur la connaît.
    pub fn audio_ms(&self) -> Option<f64> {
        let a = self.audio.lock().unwrap().as_ref().and_then(Weak::upgrade)?;
        let s = a.engine().take_stats()?;
        Some(s.buffer_ms + s.output_ms)
    }
}

struct LinuxPlatform(Arc<Shared>);

impl Platform for LinuxPlatform {
    fn window_handle(&self) -> WindowHandle {
        // L'image et le son vont à l'app : Kyber ne crée pas de lecteur VLC.
        WindowHandle::Xlib(0)
    }

    fn video_handler(&self) -> Arc<dyn PacketHandler> {
        VideoSink::new(self.0.clone())
    }

    fn audio_handler(&self) -> Option<Arc<dyn PacketHandler>> {
        if self.0.audio_channels == 0 {
            return None;
        }
        let sink = AudioSink::new(self.0.audio_channels);
        *self.0.audio.lock().unwrap() = Some(Arc::downgrade(&sink));
        Some(sink as Arc<dyn PacketHandler>)
    }

    fn input_pipeline(&self) -> Pipeline {
        let (id, w, h) = *self.0.display.lock().unwrap();
        let callbacks: Arc<dyn InputPlatform> = Arc::new(InputCallbacks(self.0.clone()));
        let clock: Arc<dyn Clock> = self.0.clock.clone();
        let clipboard = self.0.clipboard.clone().map(|c| c as Arc<dyn Pasteboard>);
        let pipeline = InputPipeline::new(Options { display_id: id, host_size: (w, h), clipboard, gamepads_from_app: true }, self.0.pointer.clone(), callbacks, clock);
        pipeline.combo().set_enabled(self.0.combo_enabled.load(Ordering::Relaxed));
        pipeline.set_relative(self.0.relative.load(Ordering::Relaxed));
        if let Err(e) = pipeline.spawn_driver() {
            log::error!("entrées : fil des minuteries impossible ({e})");
        }
        let kynput = pipeline.kynput();
        if let Some(old) = self.0.input.lock().unwrap().replace(pipeline) {
            old.stop();
            old.wake();
        }
        kynput
    }

    fn on_status(&self, status: &Status) {
        if matches!(status, Status::Ended { .. }) {
            self.0.pointer.reset();
        }
        self.0.send(Event::Status(status.clone()));
    }

    fn on_display(&self, d: &Display) {
        *self.0.display.lock().unwrap() = (d.id, d.width.max(0) as u32, d.height.max(0) as u32);
        self.0.pointer.set_host_size((d.width as f64, d.height as f64));
    }

    fn on_mode(&self, mode: Mode, _virtual_display: bool) {
        *self.0.mode.lock().unwrap() = mode;
        self.0.send(Event::Mode(mode));
    }

    fn on_stream_stopped(&self) {
        if let Some(p) = self.0.input.lock().unwrap().take() {
            p.stop();
            p.wake();
        }
    }

    fn on_input_connected(&self) {
        if let Some(p) = self.0.current_input() {
            p.network_connected();
            if self.0.clipboard.is_some() {
                p.start_clipboard();
            }
        }
        self.0.pointer.resend_held();
    }

    fn on_second(&self, s: &Second) {
        self.0.send(Event::Second {
            latency_ms: s.latency_ms,
            fps: s.displayed as f64,
            mbps: s.bytes as f64 * 8.0 / 1e6,
            lost: s.lost_frames + s.holes,
            codec: self.0.codec(),
            audio_ms: self.0.audio_ms(),
        });
    }

    fn on_game_fullscreen(&self, game: bool) {
        self.0.send(Event::GameFullscreen(game));
    }

    fn on_notice(&self, notice: &Notice) {
        use crate::i18n::tr;
        let msg = match notice {
            Notice::Relaunched => tr("Image relancée", "Picture restarted").to_string(),
            Notice::ModeChanged { width, height, .. } => format!("{} {width} × {height}", tr("L'ordinateur est passé en", "The computer switched to")),
            Notice::Fallback { bitrate_mbps, .. } => format!("{} {bitrate_mbps} Mbit/s", tr("Image allégée :", "Lighter picture:")),
            Notice::Adapting { width, height } => format!("{} {width} × {height}…", tr("Écran virtuel adapté :", "Adapting virtual display:")),
            Notice::Adapted { width, height } => format!("{} {width} × {height}", tr("Écran virtuel :", "Virtual display:")),
            Notice::AdaptFailed { error } => error.clone(),
            Notice::RestoreOffer { .. } => return,
        };
        self.0.send(Event::Notice(msg));
    }
}

struct InputCallbacks(Arc<Shared>);

impl InputPlatform for InputCallbacks {
    fn rumble(&self, r: GamepadRumble) {
        self.0.send(Event::Rumble(r));
    }

    fn combo_formed(&self, _pad: PadId) {
        self.0.send(Event::Combo);
    }

    /// Les entrées sont reliées à l'ordinateur : les manettes y sont annoncées,
    /// une fois par liaison.
    fn connected(&self) {
        self.0.send(Event::InputReady);
    }

    fn navigate(&self, _key: PadKey) {}
}

pub struct Handle {
    pub shared: Arc<Shared>,
    pub session: Arc<Session>,
}

pub fn core_profile(p: Profile) -> CoreProfile {
    CoreProfile::parse(p.id()).unwrap_or(CoreProfile::Jeu)
}

/// Les réglages du flux : le profil ramené à ce que décode cette machine, le
/// format choisi, le 4:4:4 s'il est demandé (HEVC seulement), le débit choisi
/// ou allégé.
pub fn settings(host: &Host, profile: Profile, codec: &str, mode: (u32, u32, u32), m: &Machine, light: bool) -> StreamSettings {
    let yuv444 = host.settings.yuv444 && codec == "hevc";
    let caps = Capabilities { hevc: m.decodes("hevc"), hevc444: yuv444, av1: codec == "av1" };
    let mut s = StreamSettings::for_profile(core_profile(profile), caps);
    s.codec = match codec {
        "h264" => Codec::H264,
        "av1" => Codec::Av1,
        _ => Codec::Hevc,
    };
    s.yuv444 = yuv444;
    if host.settings.reliable {
        s.reliable = true;
    }
    let (w, h, fps) = mode;
    let ratio = (w as f64 * h as f64 * fps.max(1) as f64) / (3840.0 * 2160.0 * 60.0);
    let scale = ratio.powf(0.75).clamp(0.4, 3.0);
    // Débit choisi : la base du profil ramenée pour l'atteindre à ce mode.
    if host.settings.mbps > 0 {
        s.mbps = ((host.settings.mbps as f64 / scale).floor() as u32).max(5);
    }
    // Alléger (batterie) : 20 Mbit/s au plus.
    if light {
        let actual = s.bitrate(w, h, fps) / 1_000_000;
        if actual > 20 {
            s.mbps = ((20.0 / scale).floor() as u32).max(5);
        }
    }
    s
}

/// Voies de son demandées à l'ordinateur.
pub fn audio_channels(host: &Host, m: &Machine) -> u16 {
    match host.settings.audio.as_str() {
        "computer" => 0,
        "stereo" => 2,
        "5.1" => 6,
        "7.1" => 8,
        _ => match m.audio_channels {
            8.. => 8,
            6..=7 => 6,
            _ => 2,
        },
    }
}

/// Démarre une session.
#[allow(clippy::too_many_arguments)]
pub fn start(host: &Host, profile: Profile, codec: &str, mode: (u32, u32, u32), m: &Machine, light: bool, relative: bool, device: DeviceInfo, tx: Sender<Event>) -> Handle {
    let (w, h, fps) = mode;
    let clock = Arc::new(MonotonicClock::new());
    let pointer = Arc::new(PointerLayer::new(clock.clone(), true));
    let audio_channels = audio_channels(host, m);
    let shared = Arc::new(Shared {
        tx: Mutex::new(tx),
        session: OnceLock::new(),
        pointer: pointer.clone(),
        clock,
        input: Mutex::new(None),
        display: Mutex::new((0, w, h)),
        mode: Mutex::new(Mode { width: w, height: h, refresh: fps }),
        audio_channels: audio_channels as u8,
        combo_enabled: AtomicBool::new(host.settings.combo),
        relative: AtomicBool::new(relative),
        codec: Mutex::new(String::new()),
        latest: Latest::default(),
        hw: [m.hw_h264 && (m.vaapi || m.pi.is_some()), m.hw_hevc && (m.vaapi || m.pi.is_some()), m.hw_av1 && m.vaapi],
        hw_backend: if m.pi.is_some() && !m.vaapi { 2 } else { 1 },
        clipboard: host.settings.clipboard.then(|| Arc::new(Clipboard::default())),
        audio: Mutex::new(None),
    });
    // Le pointeur de l'ordinateur : position et forme, pour la fenêtre.
    let weak = Arc::downgrade(&shared);
    pointer.set_on_changed(move |state, change| {
        let Some(sh) = weak.upgrade() else { return };
        let shape = if change.shape { state.shape.as_ref().map(|s| (s.width, s.height, s.hot_x, s.hot_y, s.premultiplied())) } else { None };
        sh.send(Event::Pointer { position: state.position(), hidden: state.remote_hidden, shape });
    });
    let params = SessionParams {
        console_host: host.address.clone(),
        console_port: host.port,
        user: host.user.clone(),
        password: host.password.clone(),
        console_fingerprint: host.fingerprint.clone(),
        device,
        mode: Mode { width: w, height: h, refresh: fps },
        audio_channels,
        virtual_display: host.settings.virtual_display,
        settings: settings(host, profile, codec, mode, m, light),
        auto_reconnect: host.settings.reconnect,
        gamepad_xbox360: host.settings.xbox360,
        clipboard: host.settings.clipboard,
        recovery: Recovery { watch_image: true, fallback_bitrate: Some(20_000_000) },
    };
    let session = Session::start(params, Arc::new(LinuxPlatform(shared.clone())));
    let _ = shared.session.set(Arc::downgrade(&session));
    Handle { shared, session }
}
