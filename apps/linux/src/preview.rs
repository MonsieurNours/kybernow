//! Captures des écrans, avec des données de démonstration, sans SDL ni cœur
//! (sur Mac aussi) :
//!
//!     cargo run -p kybernow-linux --no-default-features -- --preview <dossier> [fr|en]
//!
//! Une image PNG par écran et par interface : bureau (1280 × 800), Steam
//! Deck (1280 × 800) et salon (1920 × 1080), en pixels doubles pour le bureau
//! (écran HiDPI). Les écrans de session sont posés sur une fausse image d'ordinateur.

use std::path::Path;
use std::time::{Duration, Instant};

use tiny_skia::Pixmap;

use crate::app::{AddStep, App, BarMenu, Found, HostLive, Layout, MenuItem, NetKind, Online, PadInfo, Phase, Screen, SessionUi, Setting, Stats, Target};
use crate::model::{Host, HostSettings, LastSession, Profile, Store};
use crate::ui::{self, canvas::Canvas};

fn demo_image(seed: u32) -> Pixmap {
    let (w, h) = (640u32, 400u32);
    let mut p = Pixmap::new(w, h).unwrap();
    let d = p.data_mut();
    for y in 0..h {
        for x in 0..w {
            let i = ((y * w + x) * 4) as usize;
            let t = y as f32 / h as f32;
            let sky = y < h * 6 / 10;
            let (r, g, b) = if sky { (40.0 + 30.0 * t, 58.0 + 40.0 * t, 85.0 + 60.0 * t) } else { (34.0 + (seed % 7) as f32, 48.0 + 20.0 * (1.0 - t), 42.0) };
            let sun = ((x as f32 - 440.0).powi(2) + (y as f32 - 120.0).powi(2)).sqrt() < 44.0;
            let (r, g, b) = if sun { (230.0, 200.0, 150.0) } else { (r, g, b) };
            d[i] = r as u8;
            d[i + 1] = g as u8;
            d[i + 2] = b as u8;
            d[i + 3] = 255;
        }
    }
    p
}

fn host(id: &str, name: &str, addr: &str) -> Host {
    Host {
        id: id.into(),
        name: name.into(),
        address: addr.into(),
        port: 24811,
        fingerprint: Some("sha256:7c:3a:91:e0:4b:2d:a8:1f".into()),
        user: "steamdeck-salon".into(),
        password: String::new(),
        os: Some("Windows 11 Pro 24H2".into()),
        gpu: Some("GeForce RTX 4070".into()),
        av1: true,
        mac: Some("00:11:22:33:44:55".into()),
        settings: HostSettings::default(),
        last: None,
    }
}

fn demo(english: bool, layout: Layout) -> App {
    crate::i18n::set_english(english);
    let mut store = Store::default();
    let mut a = host("a", "Tour du salon", "192.168.1.42");
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    a.last = Some(LastSession { median: 6.8, p95: 9.4, seconds: 4320, mode: "1920 × 1200 · HEVC".into(), mbps: 38.0, at: now - 86400 });
    store.hosts.push(a);
    let mut b = host("b", "Portable pro", "100.84.12.9");
    b.os = Some("Windows 11 Pro".into());
    b.gpu = None;
    store.hosts.push(b);
    let mut c = host("c", "Mini du bureau", "192.168.1.60");
    c.gpu = Some("Radeon 780M".into());
    store.hosts.push(c);
    store.selected = Some("a".into());
    let mut app = App::new(store, layout);
    app.device_name = if layout == Layout::Desktop { "fedora-bureau".into() } else { "Steam Deck".into() };
    let m = &mut app.machine;
    match layout {
        Layout::Desktop => {
            m.os = "Fedora Linux 42".into();
            m.desktop = "GNOME 48, Wayland".into();
            m.gpu = "Mesa Gallium driver 25.1 for AMD Radeon 780M (radeonsi, phoenix, ACO)".into();
            m.display = (2560, 1600);
            m.refresh = 165;
            m.vrr = Some(true);
            m.audio_name = "PipeWire".into();
        }
        Layout::Handheld => {
            m.os = "SteamOS 3.7".into();
            m.desktop = "gamescope".into();
            m.model = "Steam Deck OLED".into();
            m.gpu = "Mesa Gallium driver 25.0 for AMD Custom GPU 0932 (radeonsi, vangogh, ACO)".into();
            m.display = (1280, 800);
            m.refresh = 90;
            m.vrr = Some(false);
            m.battery = Some((68, false));
            m.audio_name = "PipeWire".into();
        }
        Layout::Tv => {
            m.os = "SteamOS 3.7".into();
            m.desktop = "gamescope".into();
            m.gpu = "Mesa Gallium driver 25.1 for AMD Radeon RX 7600 (radeonsi, navi33, ACO)".into();
            m.hw_av1 = true;
            m.display = (3840, 2160);
            m.refresh = 120;
            m.vrr = Some(true);
            m.display_name = "LG C4, HDMI 2.1".into();
            m.audio_channels = 6;
            m.audio_name = "HDMI · Denon AVR".into();
        }
    }
    app.net.kind = if layout == Layout::Tv { NetKind::Ethernet } else { NetKind::Wifi };
    app.net.link_mbps = Some(2500);
    app.net.band = Some("5 GHz".into());
    app.live.insert("a".into(), HostLive { online: Online::Online });
    app.live.insert("b".into(), HostLive { online: Online::Offline });
    app.live.insert("c".into(), HostLive { online: Online::Online });
    app.images.insert("a".into(), demo_image(1));
    app.image_is_last.insert("a".into(), true);
    app.found.push(Found { name: "MINI-PC".into(), address: "192.168.1.57".into(), port: 31207, fingerprint: Some("sha256:7c:3a:91:e0:4b:2d:a8:1f".into()) });
    app.pads = vec![
        PadInfo { slot: 0, name: "Xbox Wireless Controller".into(), rumble: true, trigger_rumble: true, buttons: 1, axes: [9000, -12000, 0, 0, 23000, 0], battery: Some("80 %".into()), motors: (0.6, 0.25) },
        PadInfo { slot: 1, name: "DualSense Wireless Controller".into(), rumble: true, trigger_rumble: false, buttons: 0, axes: [0; 6], battery: None, motors: (0.0, 0.0) },
    ];
    app
}

fn session(app: &App) -> SessionUi {
    let (w, h, fps) = app.session_mode(app.selected().unwrap());
    SessionUi {
        host: "a".into(),
        name: "Tour du salon".into(),
        profile: Profile::Jeu,
        phase: Phase::Streaming,
        menu: true,
        keys: false,
        stats: Some(Stats {
            latency_ms: Some(7.4),
            fps: fps as f64,
            mbps: 31.0,
            lost: 0,
            mode: format!("{w} × {h}"),
            decoder: "VA-API HEVC".into(),
            audio_ms: Some(12.0),
            history: vec![7.0, 7.4, 6.9, 8.1, 7.2, 7.6, 6.8, 9.0, 7.1, 7.3, 7.0, 7.5],
        }),
        show_stats: false,
        pad_mouse: false,
        muted: false,
        light: false,
        relative: true,
        bar: false,
        bar_menu: None,
        notice: None,
        started: Instant::now() - Duration::from_millis(2100),
        ending_since: None,
        opened_at: None,
        summary: format!("{} · {w} × {h} · {fps} Hz · HEVC · 30 Mbit/s", ui::profile_label(Profile::Jeu)),
    }
}

/// Une capture : l'écran dessiné deux fois (la seconde avec le focus posé
/// sur un élément inscrit au premier dessin).
fn shot(app: &mut App, cv: &mut Canvas, video: Option<&Pixmap>, dir: &Path, name: &str) {
    let hits = ui::draw(app, cv);
    app.hits = hits;
    let hits = ui::draw(app, cv);
    app.hits = hits;
    let out = match video {
        Some(v) => {
            let (pw, ph) = (cv.pix.width(), cv.pix.height());
            let mut base = Pixmap::new(pw, ph).unwrap();
            let s = (pw as f32 / v.width() as f32).max(ph as f32 / v.height() as f32);
            base.draw_pixmap(0, 0, v.as_ref(), &tiny_skia::PixmapPaint { quality: tiny_skia::FilterQuality::Bilinear, ..Default::default() }, tiny_skia::Transform::from_scale(s, s), None);
            base.draw_pixmap(0, 0, cv.pix.as_ref(), &tiny_skia::PixmapPaint::default(), tiny_skia::Transform::identity(), None);
            base
        }
        None => cv.pix.clone(),
    };
    let path = dir.join(format!("{name}.png"));
    if let Some(bytes) = ui::canvas::encode_png(&out) {
        let _ = std::fs::write(&path, bytes);
        println!("{}", path.display());
    }
}

pub fn run(dir: &Path, lang: &str) {
    let _ = std::fs::create_dir_all(dir);
    let en = lang == "en";
    let video = demo_image(3);
    for (layout, prefix, (pw, ph, k)) in [(Layout::Desktop, "bureau", (2560u32, 1600u32, 2.0f32)), (Layout::Handheld, "deck", (1280, 800, 1.0)), (Layout::Tv, "salon", (1920, 1080, 1.0))] {
        let mut app = demo(en, layout);
        let mut cv = Canvas::new();
        cv.resize(pw, ph, k);
        let p = |n: &str| format!("{prefix}-{n}");

        if layout.console() {
            app.launched = Instant::now() - Duration::from_millis(1000);
            app.screen = Screen::Launch;
            shot(&mut app, &mut cv, None, dir, &p("00-lancement"));
            app.launched = Instant::now() - Duration::from_millis(2100);
            shot(&mut app, &mut cv, None, dir, &p("00b-lancement-relais"));
        }
        app.go(Screen::Home);
        if layout == Layout::Desktop {
            app.pointer = Some((140.0, 330.0));
        }
        shot(&mut app, &mut cv, None, dir, &p("01-accueil"));
        app.pointer = None;
        app.images.clear();
        app.update = crate::app::UpdateState::Available { version: "0.4.7".into(), channel: "stable".into(), page: String::new(), file: String::new(), url: String::new(), sha256: String::new(), size: 0 };
        shot(&mut app, &mut cv, None, dir, &p("02-accueil-signature"));
        app.update = crate::app::UpdateState::Unknown;
        app.images.insert("a".into(), demo_image(1));

        let mut empty = App::new(Store::default(), layout);
        empty.found = app.found.clone();
        empty.machine = app.machine.clone();
        empty.net = app.net.clone();
        empty.go(Screen::Home);
        shot(&mut empty, &mut cv, None, dir, &p("03-accueil-vide"));

        app.add.code = "482917".into();
        app.add.step = AddStep::Code;
        app.add.asked = vec![("MINI-PC".into(), "192.168.1.57".into())];
        app.add.message = None;
        app.go(Screen::Add);
        shot(&mut app, &mut cv, None, dir, &p("04-ajout-code"));
        app.add.step = AddStep::Address;
        app.add.address = "192.168.1.20:248".into();
        app.go(Screen::Add);
        app.focus = Target::Key('1');
        shot(&mut app, &mut cv, None, dir, &p("05-ajout-adresse"));
        app.add.step = AddStep::Verify { address: "192.168.1.20".into(), port: 24811, fingerprint: "sha256:7c:3a:91:e0:4b:2d:a8:1f".into() };
        app.go(Screen::Add);
        shot(&mut app, &mut cv, None, dir, &p("06-ajout-empreinte"));

        app.open_settings(false, Screen::Home);
        if layout.console() {
            app.section = 1;
            app.focus = Target::Setting(Setting::Display);
        }
        shot(&mut app, &mut cv, None, dir, &p("07-reglages-ordinateur"));
        if layout.console() {
            app.section = 2;
            app.focus = Target::Section(2);
            shot(&mut app, &mut cv, None, dir, &p("07b-reglages-son"));
        }
        app.open_settings(true, Screen::Home);
        shot(&mut app, &mut cv, None, dir, &p("08-reglages-app"));

        app.pad_selected = 0;
        app.go(Screen::Gamepads);
        shot(&mut app, &mut cv, None, dir, &p("09-manettes"));

        let mut s = session(&app);
        s.phase = Phase::FirstImages;
        s.menu = false;
        app.session = Some(s);
        app.screen = Screen::Session;
        app.focus = Target::CancelConnect;
        shot(&mut app, &mut cv, None, dir, &p("10-passage"));
        let mut s = session(&app);
        s.menu = false;
        s.opened_at = Some(Instant::now() - Duration::from_millis(120));
        app.session = Some(s);
        shot(&mut app, &mut cv, Some(&video), dir, &p("10b-ouverture"));

        let mut s = session(&app);
        if layout == Layout::Desktop {
            s.menu = false;
            s.bar = true;
            s.bar_menu = Some(BarMenu::Profile);
            s.notice = Some(("Souris des jeux · Échap longtemps ou Ctrl+Alt+Maj+M pour rendre la souris".into(), Instant::now()));
            app.pointer = Some((700.0, 20.0));
        }
        app.session = Some(s);
        app.focus = Target::Menu(MenuItem::Resume);
        shot(&mut app, &mut cv, Some(&video), dir, &p("11-session"));
        app.pointer = None;

        let mut s = session(&app);
        s.keys = true;
        app.session = Some(s);
        app.focus = Target::Shortcut(0);
        shot(&mut app, &mut cv, Some(&video), dir, &p("12-session-touches"));

        let mut s = session(&app);
        s.menu = false;
        s.show_stats = true;
        app.session = Some(s);
        shot(&mut app, &mut cv, Some(&video), dir, &p("13-session-mesures"));

        let mut s = session(&app);
        s.phase = Phase::Ended { error: Some(if en { "Admin did not answer: is the computer still on the network?" } else { "Admin ne répond plus : l'ordinateur est-il toujours sur le réseau ?" }.into()) };
        app.session = Some(s);
        app.focus = Target::Back;
        shot(&mut app, &mut cv, None, dir, &p("14-session-fin"));
    }

    // Raspberry Pi 5 sous Raspberry Pi OS (interface Bureau, écran 1080p) : les
    // écrans des maquettes « App sur Raspberry Pi OS », en pixels simples.
    let mut app = demo(en, Layout::Desktop);
    raspberry(&mut app);
    let mut cv = Canvas::new();
    cv.resize(1280, 800, 1.0);
    app.go(Screen::Home);
    shot(&mut app, &mut cv, None, dir, "rpi-01-accueil");
    app.open_settings(false, Screen::Home);
    shot(&mut app, &mut cv, None, dir, "rpi-07-reglages-image");
    app.section = 2;
    shot(&mut app, &mut cv, None, dir, "rpi-07b-reglages-son");
    app.section = 3;
    shot(&mut app, &mut cv, None, dir, "rpi-07c-reglages-manettes");
    app.section = 6;
    shot(&mut app, &mut cv, None, dir, "rpi-07d-reglages-compte");
    app.open_settings(true, Screen::Home);
    shot(&mut app, &mut cv, None, dir, "rpi-08-reglages-app");

    os_shots(en, dir);
}

/// L'image fixe du démarrage de KyberNow Link OS (Plymouth) : la première
/// image de l'allumage (l'encre, le symbole éteint), aux pixels de l'app.
pub fn splash(out: &Path, w: u32, h: u32) {
    let mut app = App::new(Store::default(), Layout::Tv);
    app.screen = Screen::Launch;
    app.launched = Instant::now();
    let mut cv = Canvas::new();
    cv.resize(w.max(320), h.max(200), 1.0);
    ui::draw(&app, &mut cv);
    match ui::canvas::encode_png(&cv.pix) {
        Some(bytes) => {
            if let Err(e) = std::fs::write(out, bytes) {
                eprintln!("{} : {e}", out.display());
                std::process::exit(1);
            }
            println!("{}", out.display());
        }
        None => {
            eprintln!("PNG non produit");
            std::process::exit(1);
        }
    }
}

/// Les écrans de KyberNow Link OS (maquettes « KyberNow Link OS ») : le
/// lancement avec son chargement, le premier démarrage, l'accueil salon, les
/// réglages de l'appareil.
fn os_shots(en: bool, dir: &Path) {
    use crate::app::{BtDevice, OsInfo, SetupStep, UpdateState, WifiNet, OS_SECTIONS};
    let mut app = demo(en, Layout::Tv);
    raspberry(&mut app);
    app.os = Some(OsInfo { version: "0.6.0".into(), slot: "A".into(), other: "B".into(), other_version: String::new(), ab: true, pending: String::new(), tryboot: false, ssh: false, built: "2026-10-02".into() });
    app.machine.display = (3840, 2160);
    app.machine.refresh = 60;
    app.machine.max_hz_1080p = 120;
    app.machine.audio_channels = 6;
    app.machine.audio_name = "vc4hdmi0".into();
    if let Some(h) = app.store.host_mut("a") {
        h.settings.profile = Profile::Jeu;
        h.last = Some(LastSession { median: 9.8, p95: 14.0, seconds: 6000, mode: "4K HEVC 60 Hz".into(), mbps: 60.0, at: 0 });
    }
    let mut cv = Canvas::new();
    cv.resize(1920, 1080, 1.0);
    // Le lancement : la scène tient, le chargement sous le nom.
    app.screen = Screen::Launch;
    app.launched = Instant::now() - Duration::from_millis(1500);
    app.screen_since = Instant::now() - Duration::from_millis(2000);
    app.os_loading = Some((if en { "network · bluetooth" } else { "réseau · bluetooth" }.into(), 0.45));
    shot(&mut app, &mut cv, None, dir, "os-00-demarrage-chargement");
    app.os_loading = None;
    // Le premier démarrage.
    app.start_setup();
    app.setup.bt_listening = true;
    app.setup.bt = vec![BtDevice { mac: "AC:8E:BD:12:34:56".into(), name: "Xbox Wireless Controller".into(), paired: true, connected: true }];
    shot(&mut app, &mut cv, None, dir, "os-01-manette");
    app.setup.step = SetupStep::Language;
    app.go(Screen::Setup);
    shot(&mut app, &mut cv, None, dir, "os-02-langue");
    app.setup.step = SetupStep::Network;
    app.net.kind = NetKind::Unknown;
    app.setup.wifi = vec![
        WifiNet { ssid: "Maison".into(), signal: 92, secured: true, active: false },
        WifiNet { ssid: "Maison 5 GHz".into(), signal: 80, secured: true, active: false },
        WifiNet { ssid: "Freebox-7A2C".into(), signal: 54, secured: true, active: false },
        WifiNet { ssid: "Invités".into(), signal: 31, secured: false, active: false },
    ];
    app.go(Screen::Setup);
    shot(&mut app, &mut cv, None, dir, "os-03-reseau");
    app.setup.wifi_pick = Some(0);
    app.setup.password = "corrige".into();
    app.go(Screen::Setup);
    shot(&mut app, &mut cv, None, dir, "os-03b-reseau-mot-de-passe");
    app.setup.wifi_pick = None;
    app.net.kind = NetKind::Ethernet;
    app.net.link_mbps = Some(1000);
    app.setup.step = SetupStep::Usage;
    app.go(Screen::Setup);
    shot(&mut app, &mut cv, None, dir, "os-04-usage");
    // L'accueil salon, une version prête.
    app.setup.active = false;
    app.store.app.os_setup_done = true;
    app.update = UpdateState::Available { version: "0.6.1".into(), channel: "stable".into(), page: String::new(), file: "KyberNow-Link-OS.img.xz".into(), url: String::new(), sha256: String::new(), size: 412_000_000 };
    app.os_update.ready = Some(Path::new("/var/lib/kybernow/update/KyberNow-Link-OS.img.xz").to_path_buf());
    app.go(Screen::Home);
    shot(&mut app, &mut cv, None, dir, "os-10-accueil");
    // Les réglages de l'appareil : la mise à jour, puis l'écran.
    app.open_device_settings(Screen::Home);
    app.device_section = OS_SECTIONS.iter().position(|s| *s == crate::app::Section::OsUpdate).unwrap_or(5);
    app.focus = Target::DeviceSection(app.device_section);
    shot(&mut app, &mut cv, None, dir, "os-11-reglages-appareil");
    app.device_section = 3;
    app.focus = Target::DeviceSection(3);
    shot(&mut app, &mut cv, None, dir, "os-11b-reglages-ecran");
    app.device_section = 2;
    app.focus = Target::DeviceSection(2);
    shot(&mut app, &mut cv, None, dir, "os-11c-reglages-manettes");
}

/// La machine de démonstration devient un Raspberry Pi 5 (8 Go, Raspberry Pi
/// OS Trixie, HEVC par rpivid, téléviseur 1080p à 120 Hz au plus, Ethernet).
fn raspberry(app: &mut App) {
    app.device_name = "Raspberry Pi 5".into();
    let m = &mut app.machine;
    m.os = "Raspberry Pi OS (Trixie)".into();
    m.desktop = "labwc, Wayland".into();
    m.model = "Raspberry Pi 5".into();
    m.gpu = "VideoCore VII".into();
    m.vaapi = false;
    m.hw_h264 = false;
    m.hw_hevc = true;
    m.hw_av1 = false;
    m.hw_hevc444 = false;
    m.display = (1920, 1080);
    m.refresh = 60;
    m.vrr = Some(false);
    m.display_name = "LG C4".into();
    m.audio_name = "PipeWire".into();
    m.audio_rate = 48000;
    m.audio_channels = 2;
    m.battery = None;
    m.pi = Some(crate::app::Pi { model: "Raspberry Pi 5".into(), generation: 5, ram_gb: 8, v4l2_h264: false, v4l2_hevc: true });
    m.temp_c = Some(52.0);
    m.max_hz_1080p = 120;
    m.max_mode = (3840, 2160, 60);
    app.net.kind = NetKind::Ethernet;
    app.net.link_mbps = Some(1000);
    app.net.band = None;
    if let Some(h) = app.store.host_mut("a") {
        h.settings.profile = Profile::Bureau;
        h.last = Some(LastSession { median: 11.2, p95: 15.0, seconds: 6000, mode: "1080p HEVC".into(), mbps: 28.0, at: 0 });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::Nav;

    /// À la manette, les réglages s'atteignent par Y, et l'accueil par B.
    #[test]
    fn console_navigation() {
        let mut app = demo(false, Layout::Handheld);
        let mut cv = Canvas::new();
        cv.resize(1280, 800, 1.0);
        app.go(Screen::Home);
        app.hits = ui::draw(&app, &mut cv);
        assert_eq!(app.focus, Target::Connect);
        app.nav(Nav::Options);
        assert_eq!((app.screen, app.settings_global), (Screen::Settings, false));
        app.hits = ui::draw(&app, &mut cv);
        app.nav(Nav::Ok);
        app.hits = ui::draw(&app, &mut cv);
        assert!(matches!(app.focus, Target::Setting(_)));
        app.nav(Nav::Back);
        app.nav(Nav::Back);
        assert_eq!(app.screen, Screen::Home);
        // ‹ › sur « Se connecter » changent d'ordinateur.
        app.hits = ui::draw(&app, &mut cv);
        app.nav(Nav::Right);
        assert_eq!(app.selected().unwrap().id, "b");
    }

    /// Ouverture : les moitiés de l'accueil sont encore là juste après la première image.
    #[test]
    fn opening_draws_halves() {
        let mut app = demo(false, Layout::Handheld);
        let mut cv = Canvas::new();
        cv.resize(1280, 800, 1.0);
        let mut s = session(&app);
        s.menu = false;
        s.opened_at = Some(Instant::now() - Duration::from_millis(100));
        app.session = Some(s);
        app.screen = Screen::Session;
        ui::draw(&app, &mut cv);
        let a = cv.pix.pixel(200, 400).unwrap().alpha();
        assert!(a > 100, "alpha {a}");
    }

    /// Le bureau : survol puis clic sur un segment de profil.
    #[test]
    fn desktop_mouse() {
        let mut app = demo(false, Layout::Desktop);
        let mut cv = Canvas::new();
        cv.resize(1280, 800, 1.0);
        app.go(Screen::Home);
        app.hits = ui::draw(&app, &mut cv);
        let (_, r) = *app.hits.iter().find(|(t, _)| *t == Target::Chip(Profile::Max)).unwrap();
        let acts = app.pointer_click(r.x + 2.0, r.y + 2.0);
        assert_eq!(acts, vec![crate::app::Action::Save]);
        assert_eq!(app.selected().unwrap().settings.profile, Profile::Max);
    }
}
