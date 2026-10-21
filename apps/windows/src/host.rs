//! Une session sur son propre fil : fenêtre Win32 (cachée jusqu'à la première
//! image), fil de l'image, son, entrées et session du cœur. L'app (Tauri) ou
//! la ligne de commande la lancent et en reçoivent les nouvelles par
//! `HostEvents`.

use std::sync::atomic::{AtomicIsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde::Serialize;
use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::{DestroyWindow, PostMessageW};

use kybernow_core::kynput_rs::Pipeline;
use kybernow_core::media::PacketHandler;
use kybernow_core::session::{Platform, Session, SessionParams, Status};
use kybernow_core::{Display, WindowHandle};

use crate::audio::{self, AudioHandler};
use crate::input::Input;
use crate::video::{self, Shared, VideoHandler, VideoOptions};
use crate::window::{self, App, Command, WM_APP_COMMAND, WM_APP_MOUSE, WM_APP_STATUS};

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

struct WinPlatform {
    app: Arc<App>,
    hwnd: isize,
    video: Arc<VideoHandler>,
    audio: Option<Arc<AudioHandler>>,
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
        self.audio.clone().map(|a| a as Arc<dyn PacketHandler>)
    }
    fn input_pipeline(&self) -> Pipeline {
        self.app.input.pipeline()
    }
    fn on_status(&self, status: &Status) {
        if let Status::Ended { error: Some(e) } = status {
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
        *self.app.host_size.lock().unwrap() = (display.width as u32, display.height as u32);
        self.app.input.display_id.store(display.id, Ordering::Relaxed);
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
        self.events.stats(&Stats {
            latency_ms: ms,
            fps,
            width,
            height,
            decoder: self.app.shared.decoder.lock().unwrap().clone(),
            relative: self.app.input.relative(),
            rtt_ms,
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
    let cell: Arc<Mutex<Option<Arc<App>>>> = Arc::new(Mutex::new(None));
    let c = cell.clone();
    let input = Input::new(Box::new(move |cursor| {
        if let Some(app) = c.lock().unwrap().as_ref() {
            app.cursor_from_host(cursor);
        }
    }));
    let app = App::new(host_name.clone(), input, shared.clone(), events.clone(), opts.fullscreen);
    *app.mouse_choice.lock().unwrap() = opts.mouse;
    *cell.lock().unwrap() = Some(app.clone());

    let hwnd = match window::create(&format!("KyberNow — {host_name}"), opts.window_size) {
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
    let audio = (params.audio_channels > 0).then(audio::spawn);

    let platform = Arc::new(WinPlatform {
        app: app.clone(),
        hwnd: hwnd.0 as isize,
        video: video_handler.clone(),
        audio: audio.clone(),
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
    // Le fil de l'image finit (il garde la dernière image) avant le résumé.
    video_handler.stop_and_wait(Duration::from_secs(2));
    if let Some(a) = &audio {
        a.stop();
    }
    if app.input.refused() > 0 {
        log::info!("entrées : {} paquets refusés par kynput (réessayés)", app.input.refused());
    }
    let (median_ms, p95_ms) = session.latency().session();
    if let Some(m) = median_ms {
        log::info!("latence de la session : médiane {m:.1} ms, 95e centile {:.1} ms", p95_ms.unwrap_or(0.0));
    }
    let error = app.error.lock().unwrap().clone();
    *platform.session.lock().unwrap() = None;
    *cell.lock().unwrap() = None;
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
