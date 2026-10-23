//! Une session vue du téléviseur : ce que webOS fournit au cœur
//! (`session::Platform`) et les rappels vers la boucle principale, comme le
//! pont JNI d'Android (apps/android/jni/src/session.rs).

use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex, OnceLock, Weak};

use kybernow_core::console::{DeviceInfo, Mode};
use kybernow_core::input::{Clock, GamepadRumble, InputPipeline, InputPlatform, MonotonicClock, Options, PadId, PadKey, PointerLayer};
use kybernow_core::kynput_rs::Pipeline;
use kybernow_core::latency::Second;
use kybernow_core::media::PacketHandler;
use kybernow_core::profile::{Capabilities, Codec, Profile as CoreProfile, StreamSettings};
use kybernow_core::session::{Notice, Platform, Recovery, Session, SessionParams, Status};
use kybernow_core::{Display, WindowHandle};

use super::ndl;
use super::media::{AudioSink, VideoSink};
use super::Event;
use crate::model::{Host, Profile};

/// Ce que partagent la session, ses flux et la boucle principale.
pub struct Shared {
    tx: Mutex<Sender<Event>>,
    session: OnceLock<Weak<Session>>,
    pub pointer: Arc<PointerLayer>,
    clock: Arc<MonotonicClock>,
    pub input: Mutex<Option<Arc<InputPipeline>>>,
    display: Mutex<(u32, u32, u32)>,
    mode: Mutex<Mode>,
    refresh: AtomicU32,
    has_audio: bool,
    /// Son 5.1 par NDL.
    surround: bool,
    pub combo_enabled: AtomicBool,
    codec: Mutex<String>,
}

impl Shared {
    pub fn session(&self) -> Option<Arc<Session>> {
        self.session.get().and_then(Weak::upgrade)
    }

    fn send(&self, e: Event) {
        let _ = self.tx.lock().unwrap().send(e);
    }

    pub fn video_size(&self) -> (u32, u32) {
        let m = self.mode.lock().unwrap();
        if m.width > 0 && m.height > 0 { (m.width, m.height) } else { (1920, 1080) }
    }

    pub fn host_size(&self) -> (u32, u32) {
        let d = self.display.lock().unwrap();
        if d.1 > 0 && d.2 > 0 { (d.1, d.2) } else { self.video_size() }
    }

    pub fn frame_period_micros(&self) -> i64 {
        1_000_000 / self.refresh.load(Ordering::Relaxed).max(24) as i64
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
}

struct WebOsPlatform(Arc<Shared>);

impl Platform for WebOsPlatform {
    fn window_handle(&self) -> WindowHandle {
        // L'image et le son vont à l'app : Kyber ne crée pas de lecteur VLC.
        WindowHandle::Xlib(0)
    }

    fn video_handler(&self) -> Arc<dyn PacketHandler> {
        VideoSink::new(self.0.clone())
    }

    fn audio_handler(&self) -> Option<Arc<dyn PacketHandler>> {
        self.0.has_audio.then(|| AudioSink::new(self.0.surround) as Arc<dyn PacketHandler>)
    }

    fn input_pipeline(&self) -> Pipeline {
        let (id, w, h) = *self.0.display.lock().unwrap();
        let callbacks: Arc<dyn InputPlatform> = Arc::new(InputCallbacks(self.0.clone()));
        let clock: Arc<dyn Clock> = self.0.clock.clone();
        let pipeline = InputPipeline::new(
            Options { display_id: id, host_size: (w, h), clipboard: None, gamepads_from_app: true },
            self.0.pointer.clone(),
            callbacks,
            clock,
        );
        pipeline.combo().set_enabled(self.0.combo_enabled.load(Ordering::Relaxed));
        if let Err(e) = pipeline.spawn_driver() {
            log::error!("entrées : fil des minuteries impossible ({e})");
        }
        let kynput = pipeline.kynput();
        if let Some(old) = self.0.input.lock().unwrap().replace(pipeline) {
            old.stop();
            old.wake();
        }
        self.0.send(Event::InputReady);
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

    fn connected(&self) {}

    fn navigate(&self, _key: PadKey) {}
}

pub struct Handle {
    pub shared: Arc<Shared>,
    pub session: Arc<Session>,
}

pub fn core_profile(p: Profile) -> CoreProfile {
    CoreProfile::parse(p.id()).unwrap_or(CoreProfile::Jeu)
}

/// Les réglages du flux : le profil ramené à ce que décode le téléviseur
/// (HEVC, pas de 4:4:4), le format choisi, le débit plafonné par le réseau.
pub fn settings(host: &Host, profile: Profile, codec: &str, mode: (u32, u32, u32), cap_mbps: u32) -> StreamSettings {
    let caps = Capabilities { hevc: true, hevc444: false, av1: codec == "av1" };
    let mut s = StreamSettings::for_profile(core_profile(profile), caps);
    s.codec = match codec {
        "h264" => Codec::H264,
        "av1" => Codec::Av1,
        _ => Codec::Hevc,
    };
    s.yuv444 = false;
    if host.settings.reliable {
        s.reliable = true;
    }
    // Le débit du profil suit les pixels par seconde ; on ramène sa base pour
    // ne pas dépasser le plafond (ou le débit choisi) au mode de la session.
    let (w, h, fps) = mode;
    let wanted = if host.settings.mbps > 0 { host.settings.mbps.min(cap_mbps) } else { cap_mbps };
    let actual = s.bitrate(w, h, fps) / 1_000_000;
    if host.settings.mbps > 0 || actual > wanted {
        let ratio = (w as f64 * h as f64 * fps.max(1) as f64) / (3840.0 * 2160.0 * 60.0);
        let scale = ratio.powf(0.75).clamp(0.4, 3.0);
        s.mbps = ((wanted as f64 / scale).floor() as u32).max(5);
    }
    s
}

/// Démarre une session.
#[allow(clippy::too_many_arguments)]
pub fn start(host: &Host, profile: Profile, codec: &str, mode: (u32, u32, u32), refresh: u32, cap_mbps: u32, device: DeviceInfo, tx: Sender<Event>) -> Handle {
    let (w, h, fps) = mode;
    let clock = Arc::new(MonotonicClock::new());
    let pointer = Arc::new(PointerLayer::new(clock.clone(), true));
    // 5.1 : demandé à l'ordinateur et confié au décodeur du téléviseur (NDL),
    // qui le sort vers l'ampli (eARC) ou le mixe pour ses haut-parleurs.
    let surround = host.settings.audio == "surround" && ndl::available();
    let audio_channels = match host.settings.audio.as_str() {
        "computer" => 0,
        _ if surround => 6,
        _ => 2,
    };
    ndl::set_audio_channels(if surround { 6 } else { 2 });
    let shared = Arc::new(Shared {
        tx: Mutex::new(tx),
        session: OnceLock::new(),
        pointer: pointer.clone(),
        clock,
        input: Mutex::new(None),
        display: Mutex::new((0, w, h)),
        mode: Mutex::new(Mode { width: w, height: h, refresh: fps }),
        refresh: AtomicU32::new(refresh),
        has_audio: audio_channels > 0,
        surround,
        combo_enabled: AtomicBool::new(host.settings.combo),
        codec: Mutex::new(String::new()),
    });
    // Le pointeur de l'ordinateur, dessiné par l'app : position et forme.
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
        virtual_display: true,
        settings: settings(host, profile, codec, mode, cap_mbps),
        auto_reconnect: host.settings.reconnect,
        gamepad_xbox360: host.settings.xbox360,
        clipboard: false,
        recovery: Recovery { watch_image: true, fallback_bitrate: Some((cap_mbps / 2).max(20)) },
    };
    let session = Session::start(params, Arc::new(WebOsPlatform(shared.clone())));
    let _ = shared.session.set(Arc::downgrade(&session));
    Handle { shared, session }
}
