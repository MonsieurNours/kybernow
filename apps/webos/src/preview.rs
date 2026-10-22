//! Captures des écrans sur l'ordinateur de développement, avec des données de
//! démonstration (comme les captures Roborazzi de l'app Android) :
//!
//!     cargo run -p kybernow-webos --no-default-features -- --preview <dossier> [fr|en]
//!
//! Une image PNG par écran, en 1920 × 1080. Les écrans de session sont posés
//! sur une fausse image d'ordinateur (le plan vidéo du téléviseur).

use std::path::Path;
use std::time::{Duration, Instant};

use tiny_skia::Pixmap;

use crate::app::{AddStep, App, Found, HostLive, MenuItem, NetKind, Online, PadInfo, Phase, Screen, SessionUi, Setting, Stats, Target};
use crate::model::{Host, HostSettings, LastSession, Profile, Store};
use crate::ui::{self, canvas::Canvas, Assets};

fn demo_image(seed: u32) -> Pixmap {
    let (w, h) = (480u32, 270u32);
    let mut p = Pixmap::new(w, h).unwrap();
    let d = p.data_mut();
    for y in 0..h {
        for x in 0..w {
            let i = ((y * w + x) * 4) as usize;
            let t = y as f32 / h as f32;
            let sky = y < h * 6 / 10;
            let (r, g, b) = if sky {
                (40.0 + 30.0 * t, 58.0 + 40.0 * t, 85.0 + 60.0 * t)
            } else {
                (34.0 + (seed % 7) as f32, 48.0 + 20.0 * (1.0 - t), 42.0)
            };
            let sun = ((x as f32 - 330.0).powi(2) + (y as f32 - 90.0).powi(2)).sqrt() < 34.0;
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
        fingerprint: Some("sha256:9f:3a:c2:71:0b:44:de:aa".into()),
        user: "link-g2-salon".into(),
        password: String::new(),
        os: Some("Windows 11 Pro 24H2".into()),
        gpu: Some("RTX 3080 Ti".into()),
        av1: true,
        mac: Some("00:11:22:33:44:55".into()),
        settings: HostSettings::default(),
        last: None,
    }
}

fn demo(english: bool) -> App {
    crate::i18n::set_english(english);
    let mut store = Store::default();
    let mut a = host("a", "Atelier", "192.168.1.20");
    a.last = Some(LastSession { median: 14.6, p95: 21.0, seconds: 3900, mode: "4K 120 Hz · HEVC".into(), mbps: 82.0, at: 0 });
    store.hosts.push(a);
    store.hosts.push(host("b", "Portable", "192.168.1.31"));
    store.selected = Some("a".into());
    let mut app = App::new(store);
    app.tv.model = "OLED83G2".into();
    app.tv.webos = "25".into();
    app.net.kind = NetKind::Wifi;
    app.live.insert("a".into(), HostLive { online: Online::Online, ready: None });
    app.live.insert("b".into(), HostLive { online: Online::Offline, ready: None });
    app.images.insert("a".into(), demo_image(1));
    app.found.push(Found { name: "Salon-PC".into(), address: "192.168.1.42".into(), port: 31250, fingerprint: None });
    app.pads = vec![
        PadInfo { slot: 0, name: "DualSense".into(), rumble: true, trigger_rumble: true, buttons: 1, axes: [9000, -12000, 0, 0, 23000, 0], battery: Some("80 %".into()), motors: (0.6, 0.25) },
        PadInfo { slot: 1, name: "Xbox Wireless Controller".into(), rumble: true, trigger_rumble: false, buttons: 0, axes: [0; 6], battery: None, motors: (0.0, 0.0) },
    ];
    app
}

fn session(app: &App) -> SessionUi {
    SessionUi {
        host: "a".into(),
        name: "Atelier".into(),
        profile: Profile::Jeu,
        phase: Phase::Streaming,
        menu: true,
        keys: false,
        stats: Some(Stats { latency_ms: Some(14.6), fps: 120.0, mbps: 82.0, lost: 0, mode: "3840 × 2160".into(), codec: "HEVC".into() }),
        show_stats: false,
        remote_mouse: false,
        pad_mouse: false,
        notice: None,
        game_hint: app.selected().is_some_and(|h| h.settings.game_mode_hint),
        started: Instant::now() - Duration::from_millis(400),
    }
}

/// Une capture : l'écran dessiné deux fois (la seconde avec le focus posé
/// sur un élément inscrit au premier dessin).
fn shot(app: &mut App, cv: &mut Canvas, assets: &Assets, video: Option<&Pixmap>, dir: &Path, name: &str) {
    let hits = ui::draw(app, cv, assets);
    app.hits = hits;
    let hits = ui::draw(app, cv, assets);
    app.hits = hits;
    let out = match video {
        Some(v) => {
            // Le plan vidéo dessous, l'interface par-dessus.
            let mut base = Pixmap::new(1920, 1080).unwrap();
            base.draw_pixmap(0, 0, v.as_ref(), &tiny_skia::PixmapPaint { quality: tiny_skia::FilterQuality::Bilinear, ..Default::default() }, tiny_skia::Transform::from_scale(4.0, 4.0), None);
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
    let mut app = demo(en);
    let mut cv = Canvas::new();
    let assets = Assets::load();
    let video = demo_image(3);

    app.go(Screen::Home);
    shot(&mut app, &mut cv, &assets, None, dir, "1-accueil");

    let mut empty = App::new(Store::default());
    empty.found = app.found.clone();
    empty.tv = app.tv.clone();
    empty.net = app.net.clone();
    empty.go(Screen::Home);
    shot(&mut empty, &mut cv, &assets, None, dir, "2-accueil-vide");

    app.add.code = "482913".into();
    app.add.step = AddStep::Code;
    app.add.asked = vec![("Salon-PC".into(), "192.168.1.42".into())];
    app.add.message = Some(if en { "Code shown to Salon-PC: accept it in Admin" } else { "Code présenté à Salon-PC : acceptez-le dans Admin" }.into());
    app.go(Screen::Add);
    shot(&mut app, &mut cv, &assets, None, dir, "3-ajout-code");

    app.add.step = AddStep::Address;
    app.add.address = "192.168.1.20:248".into();
    app.add.message = None;
    app.go(Screen::Add);
    app.focus = Target::Key('1');
    shot(&mut app, &mut cv, &assets, None, dir, "4-ajout-adresse");

    app.add.step = AddStep::Verify { address: "192.168.1.20".into(), port: 24811, fingerprint: "sha256:9f:3a:c2:71:0b:44:de:aa".into() };
    app.go(Screen::Add);
    shot(&mut app, &mut cv, &assets, None, dir, "5-ajout-empreinte");

    for (i, name) in ["6-reglages-image", "7-reglages-son", "8-reglages-manettes", "9-reglages-reseau", "10-reglages-compte", "11-reglages-televiseur"].iter().enumerate() {
        app.section = i;
        app.screen = Screen::Settings;
        app.focus = if i == 0 { Target::Setting(Setting::Fps) } else { Target::Section(i) };
        shot(&mut app, &mut cv, &assets, None, dir, name);
    }

    app.pad_selected = 0;
    app.go(Screen::Gamepads);
    shot(&mut app, &mut cv, &assets, None, dir, "12-manettes");

    let mut s = session(&app);
    s.phase = Phase::Connecting;
    s.menu = false;
    app.session = Some(s);
    app.screen = Screen::Session;
    app.focus = Target::CancelConnect;
    shot(&mut app, &mut cv, &assets, None, dir, "13-session-arrivee");

    app.session = Some(session(&app));
    app.focus = Target::Menu(MenuItem::Resume);
    shot(&mut app, &mut cv, &assets, Some(&video), dir, "14-session-menu");

    let mut s = session(&app);
    s.keys = true;
    app.session = Some(s);
    app.focus = Target::Shortcut(0);
    shot(&mut app, &mut cv, &assets, Some(&video), dir, "15-session-touches");

    let mut s = session(&app);
    s.menu = false;
    s.show_stats = true;
    app.session = Some(s);
    shot(&mut app, &mut cv, &assets, Some(&video), dir, "16-session-mesures");

    let mut s = session(&app);
    s.phase = Phase::Ended { error: Some(if en { "Admin did not answer: is the computer still on the network?" } else { "Admin ne répond plus : l'ordinateur est-il toujours sur le réseau ?" }.into()) };
    app.session = Some(s);
    app.focus = Target::Back;
    shot(&mut app, &mut cv, &assets, None, dir, "17-session-fin");
}
