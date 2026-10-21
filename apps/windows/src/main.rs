//! KyberNow pour Windows, jalon 1 : une session vers un ordinateur, lancée en
//! ligne de commande (l'accueil, l'ajout et les réglages viendront avec l'app
//! Tauri). Voir apps/windows/README.md.

#[cfg(not(windows))]
fn main() {
    eprintln!("KyberNow pour Windows ne se compile que pour Windows (apps/windows/scripts/build.sh).");
}

#[cfg(windows)]
mod audio;
#[cfg(windows)]
mod decoder;
#[cfg(windows)]
mod input;
#[cfg(windows)]
mod render;
#[cfg(windows)]
mod system;
#[cfg(windows)]
mod video;
#[cfg(windows)]
mod window;

#[cfg(windows)]
fn main() {
    std::process::exit(app::main());
}

#[cfg(windows)]
mod app {
    use std::sync::atomic::Ordering;
    use std::sync::{Arc, Mutex};

    use kybernow_core::console::{Console, Mode};
    use kybernow_core::kynput_rs::Pipeline;
    use kybernow_core::media::PacketHandler;
    use kybernow_core::profile::{Codec, Profile, StreamSettings};
    use kybernow_core::session::{Platform, Session, SessionParams, Status};
    use kybernow_core::{Display, WindowHandle};

    use crate::audio::{self, AudioHandler};
    use crate::input::Input;
    use crate::system;
    use crate::video::{self, Shared, VideoHandler, VideoOptions};
    use crate::window::{self, App, WM_APP_MOUSE, WM_APP_STATUS};

    const USAGE: &str = "\
KyberNow pour Windows (jalon 1 : session en ligne de commande)

Utilisation :
  kybernow <adresse>[:port] [options]

  <adresse>         adresse de l'ordinateur (sa console KyberNow)
  :port             port de la console (affiché dans la console, Accès)

Options :
  --user <compte>           compte KyberNow de l'ordinateur
  --password <mot de passe> (ou la variable KYBERNOW_PASSWORD, sinon demandé)
  --fingerprint <empreinte> empreinte de la console (sha256:…), sinon retenue
                            au premier contact
  --profile jeu|bureau|max  profil (Jeu par défaut)
  --codec h264|hevc|av1     format vidéo (selon la carte graphique par défaut)
  --444 / --420             couleurs complètes ou non
  --mbps <débit>            débit fixe en Mbit/s (sinon celui du profil)
  --size <L>x<H>@<Hz>       mode de l'écran de l'ordinateur (sinon cet écran)
  --window                  en fenêtre plutôt qu'en plein écran
  --no-vsync                présente chaque image sans attendre l'écran
  --no-hw                   décodage par le processeur
  --no-audio                son laissé sur l'ordinateur
  --physical                écrans de l'ordinateur tels quels, sans écran virtuel
  --probe                   affiche ce que la carte graphique décode, puis quitte

Raccourcis : Ctrl+Alt+Maj+D se déconnecter, +F plein écran, +M souris des jeux.
";

    struct Args {
        host: String,
        port: u16,
        user: Option<String>,
        password: Option<String>,
        fingerprint: Option<String>,
        profile: Profile,
        codec: Option<Codec>,
        yuv444: Option<bool>,
        mbps: Option<u32>,
        size: Option<Mode>,
        window: bool,
        vsync: bool,
        hardware: bool,
        audio: bool,
        physical: bool,
        probe: bool,
    }

    fn parse_args() -> Result<Args, String> {
        let mut a = Args {
            host: String::new(),
            port: 0,
            user: None,
            password: std::env::var("KYBERNOW_PASSWORD").ok(),
            fingerprint: None,
            profile: Profile::Jeu,
            codec: None,
            yuv444: None,
            mbps: None,
            size: None,
            window: false,
            vsync: true,
            hardware: true,
            audio: true,
            physical: false,
            probe: false,
        };
        let mut it = std::env::args().skip(1);
        let value = |name: &str, it: &mut dyn Iterator<Item = String>| {
            it.next().ok_or_else(|| format!("{name} : valeur manquante"))
        };
        while let Some(arg) = it.next() {
            match arg.as_str() {
                "-h" | "--help" => return Err(String::new()),
                "--user" => a.user = Some(value("--user", &mut it)?),
                "--password" => a.password = Some(value("--password", &mut it)?),
                "--fingerprint" => a.fingerprint = Some(value("--fingerprint", &mut it)?),
                "--profile" => {
                    let v = value("--profile", &mut it)?;
                    a.profile = Profile::parse(&v).ok_or(format!("profil inconnu : {v}"))?;
                }
                "--codec" => {
                    let v = value("--codec", &mut it)?;
                    a.codec = Some(Codec::parse(&v).ok_or(format!("format inconnu : {v}"))?);
                }
                "--444" => a.yuv444 = Some(true),
                "--420" => a.yuv444 = Some(false),
                "--mbps" => {
                    let v = value("--mbps", &mut it)?;
                    a.mbps = Some(v.parse().map_err(|_| format!("débit invalide : {v}"))?);
                }
                "--size" => {
                    let v = value("--size", &mut it)?;
                    a.size = Some(parse_mode(&v).ok_or(format!("mode invalide : {v} (exemple : 1920x1080@60)"))?);
                }
                "--window" => a.window = true,
                "--no-vsync" => a.vsync = false,
                "--no-hw" => a.hardware = false,
                "--no-audio" => a.audio = false,
                "--physical" => a.physical = true,
                "--probe" => a.probe = true,
                s if s.starts_with('-') => return Err(format!("option inconnue : {s}")),
                s => {
                    let (h, p) = split_host(s);
                    a.host = h;
                    a.port = p.unwrap_or(0);
                }
            }
        }
        if a.host.is_empty() && !a.probe {
            return Err(String::new());
        }
        Ok(a)
    }

    fn parse_mode(s: &str) -> Option<Mode> {
        let (size, hz) = s.split_once('@').unwrap_or((s, "60"));
        let (w, h) = size.split_once(['x', '×'])?;
        Some(Mode { width: w.trim().parse().ok()?, height: h.trim().parse().ok()?, refresh: hz.trim().parse().ok()? })
    }

    /// `hôte`, `hôte:port`, `[ipv6]:port`.
    fn split_host(s: &str) -> (String, Option<u16>) {
        if let Some(rest) = s.strip_prefix('[') {
            if let Some((h, p)) = rest.split_once(']') {
                return (h.to_string(), p.strip_prefix(':').and_then(|p| p.parse().ok()));
            }
        }
        match s.rsplit_once(':') {
            Some((h, p)) if !h.contains(':') => (h.to_string(), p.parse().ok()),
            _ => (s.to_string(), None),
        }
    }

    fn prompt(label: &str) -> String {
        use std::io::Write;
        eprint!("{label} : ");
        let _ = std::io::stderr().flush();
        let mut s = String::new();
        let _ = std::io::stdin().read_line(&mut s);
        s.trim().to_string()
    }

    struct WinPlatform {
        app: Arc<App>,
        hwnd: isize,
        video: Arc<VideoHandler>,
        audio: Option<Arc<AudioHandler>>,
        last_frames: Mutex<u64>,
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
            *self.app.status.lock().unwrap() = Some(status.clone());
            self.app.post(WM_APP_STATUS);
        }
        fn on_display(&self, display: &Display) {
            *self.app.host_size.lock().unwrap() = (display.width as u32, display.height as u32);
            self.app.input.display_id.store(display.id, Ordering::Relaxed);
        }
        fn on_latency(&self, ms: Option<f64>) {
            let frames = self.app.shared.frames.load(Ordering::Relaxed);
            let mut last = self.last_frames.lock().unwrap();
            *self.app.fps.lock().unwrap() = frames - *last;
            *last = frames;
            *self.app.latency.lock().unwrap() = ms;
            self.app.post(WM_APP_STATUS);
        }
        fn on_game_fullscreen(&self, game: bool) {
            self.app.game.store(game, Ordering::Relaxed);
            self.app.post(WM_APP_MOUSE);
        }
    }

    pub fn main() -> i32 {
        system::init_logging();
        system::dpi_aware();
        let args = match parse_args() {
            Ok(a) => a,
            Err(e) => {
                if !e.is_empty() {
                    eprintln!("{e}\n");
                }
                eprint!("{USAGE}");
                return 2;
            }
        };

        if args.probe {
            return match system::probe_only() {
                Ok(()) => 0,
                Err(e) => {
                    eprintln!("{e}");
                    1
                }
            };
        }

        let port = if args.port == 0 { 9443 } else { args.port };
        let user = args.user.clone().unwrap_or_else(|| prompt("Compte KyberNow de l'ordinateur"));
        let password = args.password.clone().unwrap_or_else(|| prompt("Mot de passe"));

        // Certificat de la console : celui qu'on a retenu, sinon le premier vu.
        let key = format!("{}:{port}", args.host);
        let known = args.fingerprint.clone().or_else(|| system::known_console(&key));
        let console = Console::new(&args.host, port, &user, &password, known.clone());
        let info = match console.info() {
            Ok(i) => i,
            Err(e) => {
                system::error_box(&format!("Impossible de joindre l'ordinateur : {e}"));
                return 1;
            }
        };
        if known.is_none() {
            if let Some(fp) = console.fingerprint_seen() {
                log::info!("empreinte de la console retenue : {fp}");
                eprintln!("Empreinte de la console : {fp}\n(comparez avec celle affichée dans la console, Accès)");
                system::remember_console(&key, &fp);
            }
        }
        let host_name = if info.hostname.is_empty() { args.host.clone() } else { info.hostname.clone() };
        log::info!("ordinateur : {host_name} ({}, {:?})", info.os, info.gpu);

        let mut mode = args.size.unwrap_or_else(system::screen_mode);
        let window_size = system::initial_window_size(mode);

        let shared = Arc::new(Shared::default());
        let app_cell: Arc<Mutex<Option<Arc<App>>>> = Arc::new(Mutex::new(None));
        let cell = app_cell.clone();
        let input = Input::new(Box::new(move |c| {
            if let Some(app) = cell.lock().unwrap().as_ref() {
                app.cursor_from_host(c);
            }
        }));
        let app = App::new(host_name.clone(), input, shared.clone());
        *app_cell.lock().unwrap() = Some(app.clone());

        let hwnd = match window::create(&format!("KyberNow — {host_name}"), window_size) {
            Ok(h) => h,
            Err(e) => {
                system::error_box(&format!("Fenêtre : {e}"));
                return 1;
            }
        };
        app.hwnd.store(hwnd.0 as isize, Ordering::Relaxed);
        window::show(hwnd, !args.window);

        let (video_handler, probe) = match video::spawn(
            hwnd.0 as isize,
            VideoOptions { hardware: args.hardware, vsync: args.vsync },
            shared.clone(),
        ) {
            Ok(v) => v,
            Err(e) => {
                system::error_box(&e);
                return 1;
            }
        };

        let mut settings = StreamSettings::for_profile(args.profile, probe.caps);
        if let Some(c) = args.codec {
            settings.codec = c;
            settings.profile = None;
        }
        if let Some(y) = args.yuv444 {
            settings.yuv444 = y && settings.codec == Codec::Hevc;
            settings.profile = None;
        }
        if let Some(m) = args.mbps {
            settings.mbps = m;
            settings.profile = None;
        }
        // Profil retouché : on garde le débit calé sur ce mode.
        if settings.profile.is_none() && args.mbps.is_none() {
            settings.mbps = settings.bitrate(mode.width, mode.height, mode.refresh) / 1_000_000;
        }
        if settings.codec == Codec::H264 && mode.width > 4096 {
            mode.height = mode.height * 4096 / mode.width / 2 * 2;
            mode.width = 4096;
        }
        log::info!(
            "réglages : {} · {} {} · {} Mbit/s · {}×{} à {} Hz",
            settings.label(),
            settings.codec.label(),
            if settings.yuv444 { "4:4:4" } else { "4:2:0" },
            settings.mbps,
            mode.width,
            mode.height,
            mode.refresh
        );

        let audio_channels = if args.audio { audio::default_output_channels() } else { 0 };
        let audio = (audio_channels > 0).then(audio::spawn);

        let platform = Arc::new(WinPlatform {
            app: app.clone(),
            hwnd: hwnd.0 as isize,
            video: video_handler.clone(),
            audio: audio.clone(),
            last_frames: Mutex::new(0),
        });
        let params = SessionParams {
            console_host: args.host.clone(),
            console_port: port,
            user,
            password,
            console_fingerprint: known.or_else(|| console.fingerprint_seen()),
            device: system::device_info(),
            mode,
            audio_channels,
            virtual_display: !args.physical,
            settings,
            auto_reconnect: true,
        };
        let session = Session::start(params, platform);
        let _ = shared.session.set(Arc::downgrade(&session));
        let _ = shared.latency.set(session.latency());
        let _ = app.session.set(session.clone());

        window::run_loop();

        session.stop();
        session.wait();
        video_handler.stop();
        if let Some(a) = &audio {
            a.stop();
        }
        let (median, p95) = session.latency().session();
        if let Some(m) = median {
            log::info!("latence de la session : médiane {m:.1} ms, 95e centile {:.1} ms", p95.unwrap_or(0.0));
        }
        let error = app.error.lock().unwrap().clone();
        match error {
            Some(e) => {
                system::error_box(&format!("La session s'est arrêtée : {e}"));
                1
            }
            None => 0,
        }
    }
}
