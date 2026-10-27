//! Une session vue d'Android : ce que la plateforme fournit au cœur
//! (`session::Platform`) et les rappels vers Kotlin (`SessionListener`).
//!
//! Tout le chemin chaud reste en Rust : paquets d'image vers MediaCodec,
//! paquets de son vers AAudio, chaîne d'entrées kynput. Kotlin ne reçoit
//! que les états, les mesures de chaque seconde, le pointeur à dessiner,
//! les vibrations et la combinaison de la manette.

use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex, OnceLock, Weak};

use jni::objects::{JObject, JValue};
use ndk::native_window::NativeWindow;
use serde::Deserialize;

use kybernow_core::console::Mode;
use kybernow_core::input::{Clock, GamepadRumble, InputPipeline, InputPlatform, MonotonicClock, Options, PadId, PadKey, Pasteboard, PointerLayer};
use kybernow_core::kynput_rs::Pipeline;
use kybernow_core::latency::Second;
use kybernow_core::media::PacketHandler;
use kybernow_core::profile::StreamSettings;
use kybernow_core::session::{Notice, Platform, Session, SessionParams, Status};
use kybernow_core::share::{ShareEnd, ShareImage, ShareRole, ShareState};
use kybernow_core::{Display, WindowHandle};

use crate::audio::AudioSink;
use crate::jvm::{jstring_or_null, Listener};
use crate::video::{Msg, VideoSink};

/// Ce que partagent la session, ses flux et les appels de Kotlin.
pub struct Shared {
    pub listener: Listener,
    session: OnceLock<Weak<Session>>,
    pub window: Mutex<Option<NativeWindow>>,
    pub video_tx: Mutex<Option<Sender<Msg>>>,
    pub pointer: Arc<PointerLayer>,
    clock: Arc<MonotonicClock>,
    pub input: Mutex<Option<Arc<InputPipeline>>>,
    /// Écran de l'ordinateur affiché : numéro, largeur, hauteur.
    display: Mutex<(u32, u32, u32)>,
    /// Mode de la session (taille du décodeur au départ).
    mode: Mutex<Mode>,
    /// Fréquence de l'écran de l'appareil (instant d'affichage estimé).
    refresh: AtomicU32,
    has_audio: bool,
    clipboard: bool,
    pub combo_enabled: AtomicBool,
}

impl Shared {
    pub fn session(&self) -> Option<Arc<Session>> {
        self.session.get().and_then(Weak::upgrade)
    }

    pub fn video_size(&self) -> (u32, u32) {
        let m = self.mode.lock().unwrap();
        if m.width > 0 && m.height > 0 { (m.width, m.height) } else { (1920, 1080) }
    }

    pub fn frame_period_micros(&self) -> i64 {
        1_000_000 / self.refresh.load(Ordering::Relaxed).max(24) as i64
    }

    pub fn set_window(&self, window: Option<NativeWindow>) {
        *self.window.lock().unwrap() = window;
        if let Some(tx) = self.video_tx.lock().unwrap().as_ref() {
            let _ = tx.send(Msg::Surface);
        }
    }

    pub fn set_device_refresh(&self, hz: u32) {
        self.refresh.store(hz, Ordering::Relaxed);
    }

    pub fn current_input(&self) -> Option<Arc<InputPipeline>> {
        self.input.lock().unwrap().clone()
    }
}

struct AndroidPlatform(Arc<Shared>);

/// Avis `onNotice` : la Station redémarre en dernier recours pendant la
/// connexion (`SessionListener.NOTICE_STATION_RESTARTING` côté Kotlin) ;
/// toujours juste après l'état 1 (connexion), que montre une app qui ne le
/// connaît pas. Pas de nouvel état : Kotlin prend un état inconnu pour la fin.
const NOTICE_STATION_RESTARTING: i32 = 7;

impl Platform for AndroidPlatform {
    /// KyberNow Share, hôte : `SessionListener.onGuests(json)` (nul sans partage).
    fn on_guests(&self, share: Option<&ShareState>) {
        let json = share.map(|s| serde_json::to_string(s).unwrap_or_default());
        self.0.listener.with(|env, obj| {
            let s = jstring_or_null(env, json.as_deref())?;
            env.call_method(obj, "onGuests", "(Ljava/lang/String;)V", &[JValue::Object(&s)]).map(|_| ())
        });
    }

    /// KyberNow Share, invité : `SessionListener.onShareRole(role, previous)`
    /// (0 spectateur, 1 manettes, 2 manettes avec clavier et souris, 3 clavier
    /// et souris seuls ; −1 au départ).
    fn on_share_role(&self, role: ShareRole, previous: Option<ShareRole>) {
        self.0.listener.void("onShareRole", "(II)V", &[JValue::Int(role.code()), JValue::Int(previous.map_or(-1, ShareRole::code))]);
    }

    /// KyberNow Share, invité : `SessionListener.onShareEnd(end, text)` (0
    /// arrêté, 1 retiré, 2 format illisible), juste avant la fin.
    /// KyberNow Share, invité : `SessionListener.onShareImage(image, previous,
    /// notice)` (JSON ; `previous` nul au départ ; `notice` : l'avis bref, nul
    /// s'il n'y a rien à dire).
    fn on_share_image(&self, image: &ShareImage, previous: Option<&ShareImage>, notice: Option<&str>) {
        let image = serde_json::to_string(image).unwrap_or_default();
        let previous = previous.map(|p| serde_json::to_string(p).unwrap_or_default());
        self.0.listener.with(|env, obj| {
            let i = env.new_string(&image)?;
            let p = jstring_or_null(env, previous.as_deref())?;
            let n = jstring_or_null(env, notice)?;
            env.call_method(
                obj,
                "onShareImage",
                "(Ljava/lang/String;Ljava/lang/String;Ljava/lang/String;)V",
                &[JValue::Object(&i), JValue::Object(&p), JValue::Object(&n)],
            )
            .map(|_| ())
        });
    }

    fn on_share_end(&self, end: &ShareEnd, text: &str) {
        self.0.listener.with(|env, obj| {
            let s = env.new_string(text)?;
            env.call_method(obj, "onShareEnd", "(ILjava/lang/String;)V", &[JValue::Int(end.code()), JValue::Object(&s)]).map(|_| ())
        });
    }

    fn window_handle(&self) -> WindowHandle {
        // L'image et le son vont à l'app : Kyber ne crée pas de lecteur VLC.
        WindowHandle::Android(0)
    }

    fn video_handler(&self) -> Arc<dyn PacketHandler> {
        VideoSink::new(self.0.clone())
    }

    fn audio_handler(&self) -> Option<Arc<dyn PacketHandler>> {
        self.0.has_audio.then(|| AudioSink::new(self.0.clone()) as Arc<dyn PacketHandler>)
    }

    fn input_pipeline(&self) -> Pipeline {
        let (id, w, h) = *self.0.display.lock().unwrap();
        let callbacks: Arc<dyn InputPlatform> = Arc::new(InputCallbacks(self.0.clone()));
        let clipboard = self.0.clipboard.then(|| Arc::new(Board(self.0.clone())) as Arc<dyn Pasteboard>);
        let clock: Arc<dyn Clock> = self.0.clock.clone();
        let pipeline = InputPipeline::new(
            Options { display_id: id, host_size: (w, h), clipboard, gamepads_from_app: true },
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
        kynput
    }

    fn on_status(&self, status: &Status) {
        let (kind, attempt, max) = match status {
            Status::Preparing => (0, 0, 0),
            Status::Connecting => (1, 0, 0),
            Status::FirstImages => (2, 0, 0),
            Status::Streaming => (3, 0, 0),
            Status::Reconnecting { attempt, max } => (4, *attempt, *max),
            // « Connexion » pour l'app, puis l'avis NOTICE_STATION_RESTARTING (ci-dessous).
            Status::StationRestarting => (1, 0, 0),
            Status::Ending => (5, 0, 0),
            Status::Ended { .. } => (6, 0, 0),
        };
        let (error, report) = match status {
            Status::Ended { error, report } => (error.clone(), report.clone()),
            _ => (None, None),
        };
        if kind == 6 {
            self.0.pointer.reset();
        }
        self.0.listener.with(|env, obj| {
            let e = jstring_or_null(env, error.as_deref())?;
            let re = jstring_or_null(env, report.as_ref().and_then(|r| r.error.as_deref()))?;
            env.call_method(
                obj,
                "onStatus",
                "(IIILjava/lang/String;ZZZLjava/lang/String;)V",
                &[
                    JValue::Int(kind),
                    JValue::Int(attempt as i32),
                    JValue::Int(max as i32),
                    JValue::Object(&e),
                    JValue::Bool(report.is_some() as u8),
                    JValue::Bool(report.as_ref().is_some_and(|r| r.display_restored) as u8),
                    JValue::Bool(report.as_ref().is_some_and(|r| r.audio_restored) as u8),
                    JValue::Object(&re),
                ],
            )
            .map(|_| ())
        });
        if matches!(status, Status::StationRestarting) {
            let null = JObject::null();
            self.0.listener.void(
                "onNotice",
                "(IIIZIIZLjava/lang/String;)V",
                &[
                    JValue::Int(NOTICE_STATION_RESTARTING),
                    JValue::Int(0),
                    JValue::Int(0),
                    JValue::Bool(0),
                    JValue::Int(0),
                    JValue::Int(0),
                    JValue::Bool(0),
                    JValue::Object(&null),
                ],
            );
        }
    }

    fn on_display(&self, d: &Display) {
        *self.0.display.lock().unwrap() = (d.id, d.width.max(0) as u32, d.height.max(0) as u32);
        self.0.pointer.set_host_size((d.width as f64, d.height as f64));
        self.0
            .listener
            .void("onDisplay", "(III)V", &[JValue::Int(d.id as i32), JValue::Int(d.width), JValue::Int(d.height)]);
    }

    fn on_displays(&self, displays: &[Display]) {
        let json = serde_json::to_string(
            &displays
                .iter()
                .map(|d| serde_json::json!({ "id": d.id, "width": d.width, "height": d.height, "x": d.x, "y": d.y }))
                .collect::<Vec<_>>(),
        )
        .unwrap_or_default();
        self.0.listener.with(|env, obj| {
            let s = env.new_string(&json)?;
            env.call_method(obj, "onDisplays", "(Ljava/lang/String;)V", &[JValue::Object(&s)]).map(|_| ())
        });
    }

    fn on_mode(&self, mode: Mode, virtual_display: bool) {
        *self.0.mode.lock().unwrap() = mode;
        self.0.listener.void(
            "onMode",
            "(IIIZ)V",
            &[
                JValue::Int(mode.width as i32),
                JValue::Int(mode.height as i32),
                JValue::Int(mode.refresh as i32),
                JValue::Bool(virtual_display as u8),
            ],
        );
    }

    fn on_stream_started(&self) {
        if let Some(p) = self.0.current_input() {
            p.start_clipboard();
        }
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
        let chain = s.chain.map(|c| c.all()).unwrap_or([0.0; 6]);
        let values = [
            s.latency_ms.map_or(-1.0, |v| v),
            s.rtt_ms.map_or(-1.0, |v| v),
            s.displayed as f64,
            s.measured as f64,
            s.rejected as f64,
            s.lost_frames as f64,
            s.holes as f64,
            s.keyframe_requests as f64,
            s.lost_packets as f64,
            s.bytes as f64,
            if s.chain.is_some() { 1.0 } else { 0.0 },
            chain[0],
            chain[1],
            chain[2],
            chain[3],
            chain[4],
            chain[5],
        ];
        self.0.listener.with(|env, obj| {
            let a = env.new_double_array(values.len() as i32)?;
            env.set_double_array_region(&a, 0, &values)?;
            env.call_method(obj, "onSecond", "([D)V", &[JValue::Object(&JObject::from(a))]).map(|_| ())
        });
    }

    fn on_game_fullscreen(&self, game: bool) {
        self.0.listener.void("onGameFullscreen", "(Z)V", &[JValue::Bool(game as u8)]);
    }

    fn on_notice(&self, notice: &Notice) {
        let (kind, w, h, back, level, mbps, has_mode, error): (i32, u32, u32, bool, u8, u32, bool, Option<String>) = match notice {
            Notice::Relaunched => (0, 0, 0, false, 0, 0, false, None),
            Notice::ModeChanged { width, height, back } => (1, *width, *height, *back, 0, 0, false, None),
            Notice::Fallback { level, bitrate_mbps } => (2, 0, 0, false, *level, *bitrate_mbps, false, None),
            Notice::Adapting { width, height } => (3, *width, *height, false, 0, 0, false, None),
            Notice::Adapted { width, height } => (4, *width, *height, false, 0, 0, false, None),
            Notice::AdaptFailed { error } => (5, 0, 0, false, 0, 0, false, Some(error.clone())),
            Notice::RestoreOffer { mode } => {
                let (w, h) = mode.map(|m| (m.width, m.height)).unwrap_or((0, 0));
                (6, w, h, false, 0, 0, mode.is_some(), None)
            }
        };
        self.0.listener.with(|env, obj| {
            let e = jstring_or_null(env, error.as_deref())?;
            env.call_method(
                obj,
                "onNotice",
                "(IIIZIIZLjava/lang/String;)V",
                &[
                    JValue::Int(kind),
                    JValue::Int(w as i32),
                    JValue::Int(h as i32),
                    JValue::Bool(back as u8),
                    JValue::Int(level as i32),
                    JValue::Int(mbps as i32),
                    JValue::Bool(has_mode as u8),
                    JValue::Object(&e),
                ],
            )
            .map(|_| ())
        });
    }
}

// MARK: - Entrées

struct InputCallbacks(Arc<Shared>);

impl InputPlatform for InputCallbacks {
    fn rumble(&self, r: GamepadRumble) {
        self.0.listener.void(
            "onRumble",
            "(IIII)V",
            &[JValue::Int(r.id as i32), JValue::Int(r.large_motor as i32), JValue::Int(r.small_motor as i32), JValue::Int(r.duration_ms)],
        );
    }

    fn combo_formed(&self, pad: PadId) {
        self.0.listener.void("onCombo", "(I)V", &[JValue::Int(pad as i32)]);
    }

    fn connected(&self) {
        self.0.listener.void("onInputConnected", "()V", &[]);
    }

    fn navigate(&self, _key: PadKey) {}
}

/// Presse-papiers d'Android : lire le texte affiche un avis (Android 12),
/// donc il n'est lu que quand l'ordinateur le demande ; les changements sont
/// annoncés sans lecture.
struct Board(Arc<Shared>);

impl Pasteboard for Board {
    fn change_count(&self) -> u64 {
        self.0.listener.with(|env, obj| env.call_method(obj, "clipChangeCount", "()J", &[])?.j()).unwrap_or(0) as u64
    }

    fn text(&self) -> Option<String> {
        self.0
            .listener
            .with(|env, obj| {
                let v = env.call_method(obj, "clipText", "()Ljava/lang/String;", &[])?.l()?;
                if v.is_null() {
                    return Ok(None);
                }
                let s: String = env.get_string(&v.into())?.into();
                Ok(Some(s))
            })
            .flatten()
    }

    fn set_text(&self, text: &str) {
        self.0.listener.with(|env, obj| {
            let s = env.new_string(text)?;
            env.call_method(obj, "clipSetText", "(Ljava/lang/String;)V", &[JValue::Object(&s)]).map(|_| ())
        });
    }

    fn has_text(&self) -> bool {
        self.0.listener.with(|env, obj| env.call_method(obj, "clipHasText", "()Z", &[])?.z()).unwrap_or(false)
    }

    fn reading_asks_user(&self) -> bool {
        true
    }
}

// MARK: - Démarrage

pub fn parse_settings(json: &str) -> Result<StreamSettings, String> {
    StreamSettings::from_json(json)
}

/// Ce que le JSON des paramètres ajoute pour Android.
#[derive(Deserialize)]
struct AndroidJson {
    /// La combinaison L1 + R1 + Start + Select ouvre le menu.
    #[serde(default = "yes")]
    combo: bool,
}

fn yes() -> bool {
    true
}

pub struct Handle {
    pub shared: Arc<Shared>,
    pub session: Arc<Session>,
}

/// Démarre une session ; même JSON que `kn_session_start` des apps Apple
/// (`SessionParams::from_json` du cœur, `guest` compris pour une session
/// d'invité de KyberNow Share), plus `combo` (la combinaison ouvre le menu).
pub fn start(params_json: &str, listener: Listener, window: Option<NativeWindow>, refresh: u32) -> Result<Handle, String> {
    let (params, _screens) = SessionParams::from_json(params_json)?;
    let android: AndroidJson = serde_json::from_str(params_json).map_err(|e| e.to_string())?;
    let mode = params.mode;
    let clock = Arc::new(MonotonicClock::new());
    let pointer = Arc::new(PointerLayer::new(clock.clone(), true));
    let shared = Arc::new(Shared {
        listener,
        session: OnceLock::new(),
        window: Mutex::new(window),
        video_tx: Mutex::new(None),
        pointer: pointer.clone(),
        clock: clock.clone(),
        input: Mutex::new(None),
        display: Mutex::new((0, mode.width, mode.height)),
        mode: Mutex::new(mode),
        refresh: AtomicU32::new(refresh),
        has_audio: params.audio_channels > 0,
        // Jamais le presse-papiers pour un invité.
        clipboard: params.clipboard && !params.is_guest(),
        combo_enabled: AtomicBool::new(android.combo),
    });
    // Le pointeur à dessiner : position, visibilité, forme.
    let weak = Arc::downgrade(&shared);
    pointer.set_on_changed(move |state, change| {
        let Some(sh) = weak.upgrade() else { return };
        let (has, x, y) = match state.position() {
            Some((x, y)) => (true, x, y),
            None => (false, 0.0, 0.0),
        };
        let shape = if change.shape { state.shape.as_ref() } else { None };
        let pixels = shape.map(|s| s.premultiplied());
        sh.listener.with(|env, obj| {
            let arr: JObject = match &pixels {
                Some(p) => {
                    let a = env.new_int_array(p.len() as i32)?;
                    let ints: Vec<i32> = p.iter().map(|v| *v as i32).collect();
                    env.set_int_array_region(&a, 0, &ints)?;
                    a.into()
                }
                None => JObject::null(),
            };
            env.call_method(
                obj,
                "onPointer",
                "(ZDDZIIII[I)V",
                &[
                    JValue::Bool(has as u8),
                    JValue::Double(x),
                    JValue::Double(y),
                    JValue::Bool(state.remote_hidden as u8),
                    JValue::Int(shape.map_or(0, |s| s.width as i32)),
                    JValue::Int(shape.map_or(0, |s| s.height as i32)),
                    JValue::Int(shape.map_or(0, |s| s.hot_x as i32)),
                    JValue::Int(shape.map_or(0, |s| s.hot_y as i32)),
                    JValue::Object(&arr),
                ],
            )
            .map(|_| ())
        });
    });
    let session = Session::start(params, Arc::new(AndroidPlatform(shared.clone())));
    let _ = shared.session.set(Arc::downgrade(&session));
    Ok(Handle { shared, session })
}
