//! Une session sur son propre fil : fenêtres Win32 (une par écran, cachées
//! jusqu'à la première image de leur flux), fil de l'image de chaque écran,
//! son, entrées et session du cœur. L'app (Tauri) ou la ligne de commande la
//! lancent et en reçoivent les nouvelles par `HostEvents`.
//!
//! Entrées et son viennent du cœur : une `InputPipeline` par démarrage de
//! l'image (pour les écrans pris), arrêtée avec elle ; un `AudioEngine` pour
//! la session, joué par WASAPI (`audio.rs`). Un seul appareil Direct3D pour
//! tous les écrans (`gpu.rs`).

use std::sync::atomic::{AtomicIsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use serde::Serialize;
use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::PostMessageW;

use kybernow_core::audio::AudioStats;
use kybernow_core::input::{Clock, InputPipeline, InputPlatform, MonotonicClock, MouseMode, Options, PointerLayer};
use kybernow_core::kynput_rs::Pipeline;
use kybernow_core::media::PacketHandler;
use kybernow_core::screens::session_plan;
use kybernow_core::session::{Platform, Session, SessionParams, Status, StreamScreen};
use kybernow_core::{Display, WindowHandle};

use crate::audio::{self, AudioOutput};
use crate::displays::Layout;
use crate::gpu::Gpu;
use crate::video::Link;
use crate::window::{self, App, Command, VideoSettings, WM_APP_COMMAND, WM_APP_MOUSE, WM_APP_STATUS};

/// Mesures de la seconde écoulée (barre de session).
#[derive(Debug, Clone, Serialize, Default)]
pub struct Stats {
    pub latency_ms: Option<f64>,
    pub fps: u64,
    /// Définition de l'écran principal.
    pub width: u32,
    pub height: u32,
    pub decoder: String,
    pub relative: bool,
    pub rtt_ms: Option<f64>,
    /// Son : tampon avant la carte son plus latence de la sortie (ms), et
    /// trous comblés dans la seconde ; `None` sans son ou sans mesure.
    pub audio_ms: Option<f64>,
    pub audio_gaps: Option<usize>,
    /// Écrans de la session (la barre les dit avec plusieurs).
    pub screens: usize,
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

/// Ce que la session fait d'un écran de ce PC.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Role {
    /// Il a un flux ; `session_primary` : c'est l'écran principal de la
    /// session ; `width`, `height` et `refresh` : l'écran de l'ordinateur qui
    /// lui répond.
    Stream { session_primary: bool, width: u32, height: u32, refresh: u32 },
    /// L'ordinateur n'a pas pu créer son écran : sa fenêtre garde le fond de
    /// la connexion avec une ligne.
    NotCreated,
    /// Pas pris par la session.
    Unused,
}

/// Un écran de ce PC, tel que la session le prend.
#[derive(Debug, Clone)]
pub struct ScreenState {
    /// Nom du périphérique (`\\.\DISPLAY2`).
    pub key: String,
    /// Cadre sur le bureau de Windows, en pixels.
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub role: Role,
}

/// Les écrans de ce PC et ce qu'en fait la session, à chaque démarrage de
/// l'image.
#[derive(Debug, Clone)]
pub struct ScreenSet {
    pub screens: Vec<ScreenState>,
}

pub trait HostEvents: Send + Sync {
    fn status(&self, _status: &Status) {}
    fn stats(&self, _stats: &Stats) {}
    /// La fenêtre de session de l'écran principal vient de se montrer
    /// (première image) ; `foreground` : elle a obtenu le premier plan.
    fn visible(&self, _foreground: bool) {}
    /// Les écrans de la session : à chaque démarrage de l'image (fenêtres
    /// ouvertes, fermées, écrans que l'ordinateur n'a pas pu créer).
    fn screens(&self, _set: &ScreenSet) {}
    /// La fenêtre d'un écran vient de montrer l'image de son flux (la
    /// première fois, ou après une reprise de la connexion en session) :
    /// le fond de la connexion de cet écran s'efface.
    fn screen_visible(&self, _key: &str) {}
    /// Le pointeur est resté 0,25 s contre le bord haut d'une fenêtre de la
    /// session, dans la zone de la barre (app au premier plan, souris du
    /// bureau) : la barre de session s'ouvre sur cet écran.
    fn top_edge(&self, _hwnd: isize) {}
    /// Tout est arrêté, fenêtres fermées, console prévenue.
    fn ended(&self, _summary: &Summary) {}
}

pub struct HostOptions {
    pub vsync: bool,
    pub hardware: bool,
    pub fullscreen: bool,
    /// Souris forcée (`Some(true)` : jeux) ; `None` : automatique.
    pub mouse: Option<bool>,
    pub window_size: (i32, i32),
    /// Mesures dans le titre de la fenêtre (ligne de commande, qui n'a pas de
    /// barre de session) ; sinon « {nom} · KyberNow Link ».
    pub title_stats: bool,
    /// Les écrans de ce PC et ce qui décide de leur nombre : ce que le cœur
    /// reçoit (`Session::start_with_screens`), et de nouveau quand les écrans
    /// changent en session.
    pub layout: Layout,
}

pub struct SessionHost {
    control: Arc<AtomicIsize>,
}

impl SessionHost {
    pub fn start(host_name: String, params: SessionParams, opts: HostOptions, events: Arc<dyn HostEvents>) -> Self {
        let control = Arc::new(AtomicIsize::new(0));
        let c = control.clone();
        std::thread::Builder::new()
            .name("kybernow-window".into())
            .spawn(move || run(host_name, params, opts, events, c))
            .expect("fil de la fenêtre de session");
        Self { control }
    }

    fn post(&self, c: Command, arg: isize) {
        let h = self.control.load(Ordering::Relaxed);
        if h != 0 {
            unsafe {
                let _ = PostMessageW(Some(HWND(h as *mut _)), WM_APP_COMMAND, WPARAM(c as usize), LPARAM(arg));
            }
        }
    }

    pub fn command(&self, c: Command) {
        self.post(c, 0);
    }

    /// Le clavier va à cette fenêtre de session (celle où la barre s'ouvre).
    pub fn focus(&self, hwnd: isize) {
        self.post(Command::Focus, hwnd);
    }

    /// La fenêtre de session où la barre s'ouvre quand l'app la demande sans
    /// geste : celle où est le pointeur.
    pub fn bar_window(&self) -> isize {
        window::bar_window()
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
        WindowHandle::Win32(self.app.primary_hwnd())
    }
    fn video_handler(&self) -> Arc<dyn PacketHandler> {
        self.app.primary_video()
    }
    fn audio_handler(&self) -> Option<Arc<dyn PacketHandler>> {
        self.audio.as_ref().map(|a| a.engine().clone() as Arc<dyn PacketHandler>)
    }
    /// Un fil de l'image par écran : celui de la fenêtre de cet écran.
    fn screen_video_handler(&self, screen: usize) -> Arc<dyn PacketHandler> {
        self.app.video_for(screen)
    }
    /// Les écrans pris par la session : les fenêtres se mettent en place (le
    /// cœur attend), avant que `screen_video_handler` soit demandé.
    fn on_screens(&self, screens: &[StreamScreen], not_created: &[usize]) {
        self.app.screens_changed(screens, not_created);
    }
    /// Chaîne d'entrées neuve pour les écrans pris (donnés juste avant par
    /// `on_display` et `on_screens`) ; celle d'avant s'arrête (deux chaînes se
    /// disputeraient les manettes SDL).
    fn input_pipeline(&self) -> Pipeline {
        let (display_id, host_size) = self.app.primary_display();
        // Le pointeur de l'écran principal ; chaque autre écran a le sien
        // (le pointeur de l'ordinateur n'est dessiné que dans la fenêtre de son écran).
        let pointer = self.app.primary_pointer().unwrap_or_else(|| Arc::new(PointerLayer::new(self.app.clock.clone(), false)));
        let input = InputPipeline::new(
            Options {
                display_id,
                host_size,
                // Presse-papiers : celui de kynput sous Windows (voir README).
                clipboard: None,
                gamepads_from_app: false,
            },
            pointer,
            Arc::new(WinInputs),
            self.app.clock.clone(),
        );
        input.set_screens(self.app.targets());
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
            Status::Streaming => self.app.windows().iter().for_each(|w| w.shared.arm_watch()),
            Status::Reconnecting { .. } | Status::StationRestarting | Status::Ending | Status::Ended { .. } => {
                self.app.windows().iter().for_each(|w| w.shared.disarm_watch())
            }
            _ => {}
        }
        *self.app.status.lock().unwrap() = Some(status.clone());
        self.app.post(WM_APP_STATUS);
        self.events.status(status);
    }
    fn on_display(&self, display: &Display) {
        let size = (display.width as u32, display.height as u32);
        // L'écran principal pour les entrées ; les fenêtres reçoivent leurs
        // écrans par `on_screens`, juste après.
        self.app.set_primary_display(display.id, size);
    }
    fn on_latency(&self, ms: Option<f64>) {
        let primary = self.app.primary_window();
        let frames = primary.as_ref().map_or(0, |w| w.shared.frames.load(Ordering::Relaxed));
        let fps = {
            let mut last = self.last_frames.lock().unwrap();
            // L'écran principal peut changer (écran débranché) : le compte repart.
            let fps = frames.saturating_sub(*last);
            *last = frames;
            fps
        };
        *self.app.fps.lock().unwrap() = fps;
        *self.app.latency.lock().unwrap() = ms;
        self.app.post(WM_APP_STATUS);
        let (_, (width, height)) = self.app.primary_display();
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
            decoder: primary.map(|w| w.shared.decoder.lock().unwrap().clone()).unwrap_or_default(),
            relative: self.app.relative(),
            rtt_ms,
            audio_ms: audio.map(|a| a.buffer_ms + a.output_ms),
            audio_gaps: audio.map(|a| a.gaps),
            screens: window::stream_count(),
        });
    }
    fn on_game_fullscreen(&self, game: bool) {
        self.app.game.store(game, Ordering::Relaxed);
        self.app.post(WM_APP_MOUSE);
    }
}

fn run(host_name: String, params: SessionParams, opts: HostOptions, events: Arc<dyn HostEvents>, control_out: Arc<AtomicIsize>) {
    let started = Instant::now();
    let fail = |events: &Arc<dyn HostEvents>, e: String| {
        log::error!("{e}");
        events.ended(&Summary { error: Some(e), ..Default::default() });
    };

    let gpu = match Gpu::new() {
        Ok(g) => g,
        Err(e) => return fail(&events, format!("Direct3D 11 : {e}")),
    };
    let link = Arc::new(Link::default());
    let clock: Arc<dyn Clock> = Arc::new(MonotonicClock::new());
    let screens = opts.layout.screens();
    let layout = opts.layout.clone();
    let app = App::new(
        host_name.clone(),
        clock.clone(),
        link,
        events.clone(),
        gpu,
        layout.clone(),
        VideoSettings { hardware: opts.hardware, vsync: opts.vsync, codec: params.settings.codec.fourcc() },
        opts.fullscreen,
        opts.title_stats,
    );
    app.set_mouse_mode(match opts.mouse {
        None => MouseMode::Auto,
        Some(true) => MouseMode::Game,
        Some(false) => MouseMode::Desktop,
    });

    if let Err(e) = window::create_control(&app) {
        App::clear();
        return fail(&events, crate::i18n::t("errors.session_window", &[("error", &e.to_string())]));
    }
    control_out.store(app.control_hwnd(), Ordering::Relaxed);

    // La fenêtre de l'écran principal, avant le cœur (cachée jusqu'à sa
    // première image). Plusieurs écrans : posée sur le sien ; un seul : où
    // Windows la met, comme toujours.
    let multi = session_plan(&screens, params.settings.profile, params.virtual_display, params.settings.codec).is_some();
    let primary = layout.monitors.get(screens.current);
    let (key, at, size) = match primary {
        Some(m) if multi => (m.key.clone(), Some((m.x + 60, m.y + 60)), (((m.width as f32) * 0.8) as i32, ((m.height as f32) * 0.8) as i32)),
        Some(m) => (m.key.clone(), None, opts.window_size),
        None => (String::new(), None, opts.window_size),
    };
    if let Err(e) = window::create_screen(&app, &key, true, at, size) {
        App::clear();
        return fail(&events, crate::i18n::t("errors.session_window", &[("error", &e)]));
    }
    let audio = (params.audio_channels > 0).then(|| Arc::new(audio::spawn(clock.clone())));

    let platform = Arc::new(WinPlatform {
        app: app.clone(),
        audio: audio.clone(),
        audio_stats: Mutex::new(None),
        last_frames: Mutex::new(0),
        events: events.clone(),
        session: Mutex::new(None),
    });
    let session = Session::start_with_screens(params, screens, platform.clone());
    *platform.session.lock().unwrap() = Some(session.clone());
    let _ = app.link.session.set(Arc::downgrade(&session));
    let _ = app.link.latency.set(session.latency());
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
    // Les fils de l'image finissent (ils gardent la dernière image) avant le résumé.
    app.stop_video();
    if let Some(a) = &audio {
        a.stop();
    }
    let (median_ms, p95_ms) = session.latency().session();
    if let Some(m) = median_ms {
        log::info!("latence de la session : médiane {m:.1} ms, 95e centile {:.1} ms", p95_ms.unwrap_or(0.0));
    }
    let error = app.error.lock().unwrap().clone();
    *platform.session.lock().unwrap() = None;
    let last_image = app.take_last_image();
    App::clear();
    control_out.store(0, Ordering::Relaxed);
    events.ended(&Summary { error, median_ms, p95_ms, seconds: started.elapsed().as_secs(), last_image });
}
