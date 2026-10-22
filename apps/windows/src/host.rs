//! Une session sur son propre fil : fenêtre Win32 (cachée jusqu'à la première
//! image), fil de l'image, son, entrées et session du cœur. L'app (Tauri) ou
//! la ligne de commande la lancent et en reçoivent les nouvelles par
//! `HostEvents`.
//!
//! Entrées et son viennent du cœur : une `InputPipeline` par démarrage de
//! l'image (pour l'écran affiché), arrêtée avec elle ; un `AudioEngine` pour
//! la session, joué par WASAPI (`audio.rs`).

use std::sync::atomic::{AtomicIsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde::Serialize;
use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::{DestroyWindow, PostMessageW};

use kybernow_core::audio::AudioStats;
use kybernow_core::input::{Clock, InputPipeline, InputPlatform, MonotonicClock, MouseMode, Options};
use kybernow_core::kynput_rs::Pipeline;
use kybernow_core::media::PacketHandler;
use kybernow_core::session::{Platform, Session, SessionParams, Status};
use kybernow_core::{Display, WindowHandle};

use crate::audio::{self, AudioOutput};
use crate::video::{self, Shared, VideoHandler, VideoOptions};
use crate::window::{self, App, Command, WM_APP_COMMAND, WM_APP_CURSOR, WM_APP_MOUSE, WM_APP_STATUS};

/// Mesures de la seconde écoulée (barre de session).
#[derive(Debug, Clone, Serialize, Default)]
pub struct Stats {
    pub latency_ms: Option<f64>,
    pub fps: u64,
    pub width: u32,
    pub height: u32,
    pub decoder: String,
    pub relative: bool,
    pub rtt_ms: Option<f64>,
    /// Son : tampon avant la carte son plus latence de la sortie (ms), et
    /// trous comblés dans la seconde ; `None` sans son ou sans mesure.
    pub audio_ms: Option<f64>,
    pub audio_gaps: Option<usize>,
}

/// Bilan d'une session terminée.
#[derive(Debug, Clone, Serialize, Default)]
pub struct Summary {
    pub error: Option<String>,
    pub median_ms: Option<f64>,
    pub p95_ms: Option<f64>,
    pub seconds: u64,
    /// Dernière image de la session (JPEG), pour l'accueil.
    #[serde(skip)]
    pub last_image: Option<Vec<u8>>,
}

pub trait HostEvents: Send + Sync {
    fn status(&self, _status: &Status) {}
    fn stats(&self, _stats: &Stats) {}
    /// La fenêtre de session vient de se montrer (première image) ;
    /// `foreground` : elle a obtenu le premier plan.
    fn visible(&self, _foreground: bool) {}
    /// Pointeur contre le haut de l'écran de la session.
    fn top_edge(&self, _hwnd: isize) {}
    /// Tout est arrêté, fenêtre fermée, console prévenue.
    fn ended(&self, _summary: &Summary) {}
}

pub struct HostOptions {
    pub vsync: bool,
    pub hardware: bool,
    pub fullscreen: bool,
    /// Souris forcée (`Some(true)` : jeux) ; `None` : automatique.
    pub mouse: Option<bool>,
    pub window_size: (i32, i32),
}

pub struct SessionHost {
    hwnd: Arc<AtomicIsize>,
}

impl SessionHost {
    pub fn start(host_name: String, params: SessionParams, opts: HostOptions, events: Arc<dyn HostEvents>) -> Self {
        let hwnd = Arc::new(AtomicIsize::new(0));
        let h = hwnd.clone();
        std::thread::Builder::new()
            .name("kybernow-window".into())
            .spawn(move || run(host_name, params, opts, events, h))
            .expect("fil de la fenêtre de session");
        Self { hwnd }
    }

    pub fn hwnd(&self) -> isize {
        self.hwnd.load(Ordering::Relaxed)
    }

    pub fn command(&self, c: Command) {
        let h = self.hwnd();
        if h != 0 {
            unsafe {
                let _ = PostMessageW(Some(HWND(h as *mut _)), WM_APP_COMMAND, WPARAM(c as usize), LPARAM(0));
            }
        }
    }
}

/// Rappels du module d'entrées : rien à faire sous Windows (manettes lues et
/// vibrations jouées par kynput, pas de combinaison de manette).
struct WinInputs;

impl InputPlatform for WinInputs {}

/// Une chaîne d'entrées remplacée ou finie : plus rien n'en part (FlushKeys,
/// pertes au journal) et kynput arrête ses gestionnaires (manettes SDL).
fn retire(input: Option<Arc<InputPipeline>>) {
    if let Some(old) = input {
        old.stop();
        old.shutdown();
    }
}

struct WinPlatform {
    app: Arc<App>,
    hwnd: isize,
    video: Arc<VideoHandler>,
    audio: Option<Arc<AudioOutput>>,
    /// Dernières mesures du son (le moteur en fait une par seconde, pas
    /// forcément calée sur celles de la latence).
    audio_stats: Mutex<Option<AudioStats>>,
    last_frames: Mutex<u64>,
    events: Arc<dyn HostEvents>,
    session: Mutex<Option<Arc<Session>>>,
}

impl Platform for WinPlatform {
    fn window_handle(&self) -> WindowHandle {
        WindowHandle::Win32(self.hwnd as u64)
    }
    fn video_handler(&self) -> Arc<dyn PacketHandler> {
        self.video.clone()
    }
    fn audio_handler(&self) -> Option<Arc<dyn PacketHandler>> {
        self.audio.as_ref().map(|a| a.engine().clone() as Arc<dyn PacketHandler>)
    }
    /// Chaîne d'entrées neuve pour l'écran affiché (donné juste avant par
    /// `on_display`) ; celle d'avant s'arrête (deux chaînes se disputeraient
    /// les manettes SDL).
    fn input_pipeline(&self) -> Pipeline {
        let input = InputPipeline::new(
            Options {
                display_id: self.app.display_id.load(Ordering::Relaxed),
                host_size: *self.app.host_size.lock().unwrap(),
                // Presse-papiers : celui de kynput sous Windows (voir README).
                clipboard: None,
                gamepads_from_app: false,
            },
            self.app.pointer.clone(),
            Arc::new(WinInputs),
            self.app.clock.clone(),
        );
        if let Err(e) = input.spawn_driver() {
            log::error!("entrées : fil des réessais refusé ({e})");
        }
        let kynput = input.kynput();
        retire(self.app.attach_input(input));
        kynput
    }
    fn on_stream_started(&self) {
        if let Some(input) = self.app.input() {
            input.start_clipboard();
        }
    }
    fn on_stream_stopped(&self) {
        retire(self.app.detach_input());
    }
    fn on_input_connected(&self) {
        if let Some(input) = self.app.input() {
            input.network_connected();
        }
    }
    fn on_status(&self, status: &Status) {
        if let Status::Ended { error: Some(e), .. } = status {
            *self.app.error.lock().unwrap() = Some(e.clone());
        }
        // Surveillance de la première image, à chaque (re)démarrage du flux.
        match status {
            Status::Streaming => self.app.shared.arm_watch(),
            Status::Reconnecting { .. } | Status::Ending | Status::Ended { .. } => self.app.shared.disarm_watch(),
            _ => {}
        }
        *self.app.status.lock().unwrap() = Some(status.clone());
        self.app.post(WM_APP_STATUS);
        self.events.status(status);
    }
    fn on_display(&self, display: &Display) {
        let size = (display.width as u32, display.height as u32);
        *self.app.host_size.lock().unwrap() = size;
        self.app.display_id.store(display.id, Ordering::Relaxed);
        self.app.pointer.set_host_size((f64::from(size.0), f64::from(size.1)));
    }
    fn on_latency(&self, ms: Option<f64>) {
        let frames = self.app.shared.frames.load(Ordering::Relaxed);
        let fps = {
            let mut last = self.last_frames.lock().unwrap();
            let fps = frames - *last;
            *last = frames;
            fps
        };
        *self.app.fps.lock().unwrap() = fps;
        *self.app.latency.lock().unwrap() = ms;
        self.app.post(WM_APP_STATUS);
        let (width, height) = *self.app.host_size.lock().unwrap();
        let rtt_ms = self.session.lock().unwrap().as_ref().and_then(|s| s.latency().rtt_ms());
        let audio = {
            let mut last = self.audio_stats.lock().unwrap();
            if let Some(s) = self.audio.as_ref().and_then(|a| a.engine().take_stats()) {
                *last = Some(s);
            }
            *last
        };
        self.events.stats(&Stats {
            latency_ms: ms,
            fps,
            width,
            height,
            decoder: self.app.shared.decoder.lock().unwrap().clone(),
            relative: self.app.relative(),
            rtt_ms,
            audio_ms: audio.map(|a| a.buffer_ms + a.output_ms),
            audio_gaps: audio.map(|a| a.gaps),
        });
    }
    fn on_game_fullscreen(&self, game: bool) {
        self.app.game.store(game, Ordering::Relaxed);
        self.app.post(WM_APP_MOUSE);
    }
}

fn run(host_name: String, params: SessionParams, opts: HostOptions, events: Arc<dyn HostEvents>, hwnd_out: Arc<AtomicIsize>) {
    let started = Instant::now();
    let fail = |events: &Arc<dyn HostEvents>, e: String| {
        log::error!("{e}");
        events.ended(&Summary { error: Some(e), ..Default::default() });
    };

    let shared = Arc::new(Shared::default());
    let clock: Arc<dyn Clock> = Arc::new(MonotonicClock::new());
    let app = App::new(host_name.clone(), clock.clone(), shared.clone(), events.clone(), opts.fullscreen);
    *app.mouse_mode.lock().unwrap() = match opts.mouse {
        None => MouseMode::Auto,
        Some(true) => MouseMode::Game,
        Some(false) => MouseMode::Desktop,
    };
    // Pointeur de l'ordinateur changé (fil de kynput) : la fenêtre redessine.
    let weak = Arc::downgrade(&app);
    app.pointer.set_on_changed(move |_, _| {
        if let Some(app) = weak.upgrade() {
            app.post(WM_APP_CURSOR);
        }
    });

    let hwnd = match window::create(&format!("{} — {host_name}", crate::shell::DISPLAY_NAME), opts.window_size) {
        Ok(h) => h,
        Err(e) => {
            App::clear();
            return fail(&events, crate::i18n::t("errors.session_window", &[("error", &e.to_string())]));
        }
    };
    app.hwnd.store(hwnd.0 as isize, Ordering::Relaxed);
    hwnd_out.store(hwnd.0 as isize, Ordering::Relaxed);
    window::update_client_size(hwnd);

    let video_handler = match video::spawn(
        hwnd.0 as isize,
        VideoOptions { hardware: opts.hardware, vsync: opts.vsync, codec: params.settings.codec.fourcc() },
        shared.clone(),
    ) {
        Ok(v) => v,
        Err(e) => {
            unsafe {
                let _ = DestroyWindow(hwnd);
            }
            App::clear();
            return fail(&events, e);
        }
    };
    let audio = (params.audio_channels > 0).then(|| Arc::new(audio::spawn(clock.clone())));

    let platform = Arc::new(WinPlatform {
        app: app.clone(),
        hwnd: hwnd.0 as isize,
        video: video_handler.clone(),
        audio: audio.clone(),
        audio_stats: Mutex::new(None),
        last_frames: Mutex::new(0),
        events: events.clone(),
        session: Mutex::new(None),
    });
    let session = Session::start(params, platform.clone());
    *platform.session.lock().unwrap() = Some(session.clone());
    let _ = shared.session.set(Arc::downgrade(&session));
    let _ = shared.latency.set(session.latency());
    let _ = app.session.set(session.clone());

    // Raccourcis de Windows (⊞, Alt+Tab…) vers l'ordinateur en plein écran.
    let hook = window::install_keyboard_hook();
    if hook.is_none() {
        log::warn!("raccourcis de Windows : crochet clavier refusé, ils restent à ce PC");
    }
    window::run_loop();
    window::remove_keyboard_hook(hook);

    session.stop();
    session.wait();
    // Normalement déjà fait à l'arrêt de l'image.
    retire(app.detach_input());
    // Le fil de l'image finit (il garde la dernière image) avant le résumé.
    video_handler.stop_and_wait(Duration::from_secs(2));
    if let Some(a) = &audio {
        a.stop();
    }
    let (median_ms, p95_ms) = session.latency().session();
    if let Some(m) = median_ms {
        log::info!("latence de la session : médiane {m:.1} ms, 95e centile {:.1} ms", p95_ms.unwrap_or(0.0));
    }
    let error = app.error.lock().unwrap().clone();
    *platform.session.lock().unwrap() = None;
    App::clear();
    hwnd_out.store(0, Ordering::Relaxed);
    events.ended(&Summary {
        error,
        median_ms,
        p95_ms,
        seconds: started.elapsed().as_secs(),
        last_image: shared.last_image.lock().unwrap().take(),
    });
}
