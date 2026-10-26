//! Captures des écrans, avec des données de démonstration, sans SDL ni cœur
//! (sur Mac aussi) :
//!
//!     cargo run -p kybernow-linux --no-default-features -- --preview <dossier> [fr|en]
//!
//! Une image PNG par écran et par interface : bureau (1280 × 800, en pixels
//! doubles : écran HiDPI ; et la fenêtre la plus étroite, 960 × 600), Steam
//! Deck (1280 × 800) et salon (1920 × 1080). Les écrans de session sont posés
//! sur une fausse image d'ordinateur.

use std::path::Path;
use std::time::{Duration, Instant};

use tiny_skia::Pixmap;

use crate::app::{AddStep, App, BarMenu, Confirm, Found, HostLive, Layout, MenuItem, NetKind, Online, PadInfo, Phase, Screen, ScreensPlan, Section, SessionUi, Setting, StationKind, StationLive, StationOp, StationStep, Stats, Target};
use crate::model::{Host, HostSettings, LastSession, Profile, Store, WakeCard};
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

/// Une Station 0.5.0 ; `update` : une version publiée, installable depuis un appareil.
fn station(version: &str, update: Option<&str>) -> StationLive {
    StationLive {
        version: version.into(),
        supported: true,
        available: update.is_some(),
        update_version: update.map(str::to_string),
        installable: update.is_some(),
        ..Default::default()
    }
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
        mac: None,
        wake: vec![WakeCard { mac: "3C:7C:3F:12:AB:CD".into(), ipv4: Some(addr.into()), kind: "ethernet".into(), armed: Some(true), ..Default::default() }],
        settings: HostSettings::default(),
        last: None,
    }
}

fn demo(english: bool, layout: Layout) -> App {
    crate::i18n::set_english(english);
    let mut store = Store::default();
    let mut a = host("a", "Tour du salon", "192.168.1.42");
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    a.last = Some(LastSession { median: 6.8, p95: 9.4, seconds: 4320, mode: "1920 × 1200".into(), hz: 120, codec: "HEVC".into(), mbps: 38.0, at: now - 86400, screens: 1 });
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
    app.live.insert("a".into(), HostLive { online: Online::Online, virtual_display: Some(true), station: Some(station("0.5.0", None)), own_device: Some(true), max_screens: Some(4) });
    app.live.insert("b".into(), HostLive { online: Online::Offline, ..Default::default() });
    app.live.insert("c".into(), HostLive { online: Online::Online, virtual_display: Some(true), station: Some(station("0.5.0", None)), own_device: Some(true), max_screens: Some(4) });
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
    let mut s = SessionUi::new(1, "a", "Tour du salon", Profile::Jeu, false, true, 1);
    s.phase = Phase::Streaming;
    s.menu = true;
    s.stats = Some(Stats {
        latency_ms: Some(7.4),
        fps: fps as f64,
        mbps: 31.0,
        lost: 0,
        mode: format!("{w} × {h}"),
        decoder: "VA-API HEVC".into(),
        audio_ms: Some(12.0),
        history: vec![7.0, 7.4, 6.9, 8.1, 7.2, 7.6, 6.8, 9.0, 7.1, 7.3, 7.0, 7.5],
    });
    s.started = Instant::now() - Duration::from_millis(2100);
    s.opened_at = Some(Instant::now() - Duration::from_secs(30));
    s
}

/// Une capture : l'écran dessiné deux fois (la seconde avec le focus posé
/// sur un élément inscrit au premier dessin).
fn shot(app: &mut App, cv: &mut Canvas, video: Option<&Pixmap>, dir: &Path, name: &str) {
    shot_view(app, cv, video, dir, name, ui::View::SINGLE);
}

/// Une capture d'une fenêtre d'une session à plusieurs écrans (voir `ui::View`).
fn shot_view(app: &mut App, cv: &mut Canvas, video: Option<&Pixmap>, dir: &Path, name: &str, view: ui::View) {
    let hits = ui::draw_view(app, cv, view);
    app.hits = hits;
    let hits = ui::draw_view(app, cv, view);
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

/// Plusieurs écrans (docs/multi-ecrans.md) : la ligne de l'accueil, la ligne « Écrans » des
/// réglages (Auto, Tous les écrans, inactive), le nombre d'écrans des statistiques, et chaque
/// fenêtre de la session : la connexion sur l'écran principal, le fond des autres, un écran
/// que l'ordinateur n'a pas pu créer, le trait du haut, la barre sans « Plein écran ».
fn multi_shots(app: &mut App, cv: &mut Canvas, video: &Pixmap, dir: &Path, prefix: &str) {
    let name = |n: &str| format!("{prefix}-{n}");
    app.set_session(None);
    app.store.selected = Some("a".into());
    // L'accueil : trois écrans, à la taille de ceux de l'appareil.
    app.screens_plan = ScreensPlan { count: 3, enabled: true, shown: 0 };
    app.go(Screen::Home);
    shot(app, cv, None, dir, &name("15-accueil-ecrans"));
    // Les réglages : Auto, puis Tous les écrans, puis inactive (définition sur mesure).
    app.open_host_section(Section::Picture, Screen::Home);
    shot(app, cv, None, dir, &name("15b-reglages-ecrans-auto"));
    if let Some(h) = app.store.host_mut("a") {
        h.settings.displays = "all".into();
    }
    app.screens_plan = ScreensPlan { count: 3, enabled: true, shown: 2 };
    app.open_host_section(Section::Picture, Screen::Home);
    shot(app, cv, None, dir, &name("15c-reglages-ecrans-tous"));
    if let Some(h) = app.store.host_mut("a") {
        h.settings.resolution = 1080;
    }
    app.screens_plan = ScreensPlan { count: 1, enabled: false, shown: 1 };
    app.open_host_section(Section::Picture, Screen::Home);
    shot(app, cv, None, dir, &name("15d-reglages-ecrans-inactif"));
    // Inactive faute d'écran virtuel (réglage coupé, Station sans pilote) : la ligne grisée sur « Cet écran », sans aide.
    if let Some(h) = app.store.host_mut("a") {
        h.settings.resolution = 0;
    }
    app.open_host_section(Section::Picture, Screen::Home);
    shot(app, cv, None, dir, &name("15f-reglages-ecrans-sans-ecran-virtuel"));
    if let Some(h) = app.store.host_mut("a") {
        h.settings.displays = "auto".into();
    }
    // Les statistiques : leur nombre d'abord, puis l'écran principal et le débit total.
    let last = app.store.host_mut("a").and_then(|h| h.last.take());
    if let Some(h) = app.store.host_mut("a") {
        h.last = Some(LastSession { median: 8.9, p95: 14.2, seconds: 2520, mode: "3840 × 2160".into(), hz: 160, codec: "HEVC".into(), mbps: 120.0, at: 0, screens: 3 });
    }
    app.open_host_section(Section::Statistics, Screen::Home);
    shot(app, cv, None, dir, &name("15e-reglages-statistiques-ecrans"));
    if let Some(h) = app.store.host_mut("a") {
        h.last = last;
    }
    app.screens_plan = ScreensPlan::default();

    // La session à trois écrans : la connexion sur l'écran principal, le fond sur les autres.
    let mut s = session(app);
    s.menu = false;
    s.relative = false;
    s.screens = 3;
    s.expected_screens = 3;
    s.phase = Phase::Connecting;
    s.opened_at = None;
    app.set_session(Some(s));
    app.screen = Screen::Session;
    app.focus = Target::CancelConnect;
    let main = ui::View { session: None, main: true, bar: false, missing: false, has_frame: false };
    let other = ui::View { session: None, main: false, bar: false, missing: false, has_frame: false };
    shot_view(app, cv, None, dir, &name("16-connexion-ecrans-principal"), main);
    shot_view(app, cv, None, dir, &name("16b-connexion-ecrans-autre"), other);
    shot_view(app, cv, None, dir, &name("16c-ecran-non-cree"), ui::View { missing: true, ..other });
    // Écran suivant : l'image, le trait du haut seul ; la barre dans la fenêtre du geste, sans « Plein écran ».
    let mut s = session(app);
    s.menu = false;
    s.relative = false;
    s.screens = 3;
    s.expected_screens = 3;
    s.opened_at = Some(Instant::now() - Duration::from_secs(30));
    app.set_session(Some(s));
    let shown = ui::View { session: None, main: false, bar: false, missing: false, has_frame: true };
    shot_view(app, cv, Some(video), dir, &name("16d-ecrans-image-trait"), shown);
    if let Some(s) = app.session_mut() {
        s.bar = true;
    }
    app.pointer = Some((700.0, 20.0));
    shot_view(app, cv, Some(video), dir, &name("16e-ecrans-barre"), ui::View { bar: true, ..shown });
    app.pointer = None;
    app.set_session(None);
    app.screen = Screen::Home;
}

/// Plusieurs ordinateurs (docs/plusieurs-ordinateurs.md) : le menu « Ordinateurs » de la barre (autres
/// ordinateurs, point et état, « Se connecter » ou « Réveiller », puis « Afficher » et le raccourci avec deux
/// sessions, la limite de quatre), le haut-parleur « Garder le son » allumé et son nom au survol, la
/// fenêtre d'une session de plus avec sa connexion et son « Plein écran ».
fn computers_shots(app: &mut App, cv: &mut Canvas, video: &Pixmap, dir: &Path, prefix: &str) {
    let name = |n: &str| format!("{prefix}-{n}");
    app.store.selected = Some("a".into());
    // Une session sur « Tour du salon » : le menu montre les deux autres, l'un hors ligne (réveil possible), l'autre en ligne.
    let mut s = session(app);
    s.menu = false;
    s.relative = false;
    s.bar = true;
    s.bar_menu = Some(BarMenu::Computers);
    app.set_session(Some(s));
    app.screen = Screen::Session;
    app.pointer = Some((700.0, 20.0));
    shot(app, cv, Some(video), dir, &name("17-barre-ordinateurs"));
    // Deux sessions : « Mini du bureau » est ouverte, devant ; le menu la coche avec « Afficher », et dit le raccourci.
    let mut second = SessionUi::new(2, "c", "Mini du bureau", Profile::Bureau, false, false, 1);
    second.phase = Phase::Streaming;
    second.opened_at = Some(Instant::now() - Duration::from_secs(30));
    second.bar = true;
    second.bar_menu = Some(BarMenu::Computers);
    app.open_session(second);
    shot(app, cv, Some(video), dir, &name("17b-ordinateurs-deux-sessions"));
    // « Garder le son » allumé sur cette session, et son nom au survol du haut-parleur.
    if let Some(s) = app.session_mut() {
        s.bar_menu = None;
        s.keep_sound = true;
    }
    let hits = ui::draw(app, cv);
    if let Some((_, r)) = hits.iter().find(|(t, _)| *t == Target::BarSound) {
        app.pointer = Some((r.x + r.w / 2.0, r.y + r.h / 2.0));
    }
    shot(app, cv, Some(video), dir, &name("17c-barre-garder-le-son"));
    // Quatre ordinateurs : « Se connecter » est inactif, avec la raison.
    for (id, host, label) in [(3, "d", "Atelier"), (4, "e", "Salon du haut")] {
        if app.store.host(host).is_none() {
            app.store.hosts.push(crate::model::Host { id: host.into(), name: label.into(), ..app.store.hosts[2].clone() });
            app.live.insert(host.into(), HostLive { online: Online::Online, ..Default::default() });
        }
        let mut s = SessionUi::new(id, host, label, Profile::Jeu, false, false, 1);
        s.phase = Phase::Streaming;
        s.opened_at = Some(Instant::now() - Duration::from_secs(30));
        app.open_session(s);
    }
    app.store.hosts.push(crate::model::Host { id: "f".into(), name: "Bureau de Léa".into(), ..app.store.hosts[2].clone() });
    app.live.insert("f".into(), HostLive { online: Online::Online, ..Default::default() });
    if let Some(s) = app.session_mut() {
        s.bar = true;
        s.bar_menu = Some(BarMenu::Computers);
    }
    app.pointer = Some((700.0, 20.0));
    shot(app, cv, Some(video), dir, &name("17e-ordinateurs-quatre-au-plus"));
    // La fenêtre d'une session de plus : sa connexion s'affiche dans sa fenêtre, pendant que l'autre garde son image.
    app.pointer = None;
    app.set_session(None);
    let mut first = session(app);
    first.menu = false;
    first.relative = false;
    app.open_session(first);
    let mut next = SessionUi::new(2, "c", "Mini du bureau", Profile::Bureau, false, false, 1);
    next.phase = Phase::Connecting;
    app.open_session(next);
    app.focus = Target::CancelConnect;
    shot_view(app, cv, None, dir, &name("17d-fenetre-suivante"), ui::View { session: Some(2), ..ui::View::SINGLE });
    // Une fois à l'image, sa barre propose « Plein écran » (elle n'a qu'un écran).
    if let Some(s) = app.session_mut() {
        s.phase = Phase::Streaming;
        s.opened_at = Some(Instant::now() - Duration::from_secs(30));
        s.bar = true;
    }
    app.pointer = Some((700.0, 20.0));
    shot_view(app, cv, Some(video), dir, &name("17f-fenetre-suivante-barre"), ui::View { session: Some(2), ..ui::View::SINGLE });
    app.pointer = None;
    app.set_session(None);
    app.screen = Screen::Home;
}

pub fn run(dir: &Path, lang: &str) {
    let _ = std::fs::create_dir_all(dir);
    let en = lang == "en";
    let video = demo_image(3);
    for (layout, prefix, (pw, ph, k)) in [(Layout::Desktop, "bureau", (2560u32, 1600u32, 2.0f32)), (Layout::Desktop, "bureau-etroit", (960, 600, 1.0)), (Layout::Handheld, "deck", (1280, 800, 1.0)), (Layout::Tv, "salon", (1920, 1080, 1.0))] {
        let mut app = demo(en, layout);
        let mut cv = Canvas::new();
        cv.resize(pw, ph, k);
        let p = |n: &str| format!("{prefix}-{n}");
        let narrow = prefix == "bureau-etroit";

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
        // Hors ligne : réveiller l'ordinateur (une carte apprise d'Admin), le réveil en cours,
        // puis sans carte ni MAC : Réessayer.
        app.store.selected = Some("b".into());
        app.go(Screen::Home);
        shot(&mut app, &mut cv, None, dir, &p("01b-accueil-hors-ligne"));
        app.waking = Some(("b".into(), Instant::now()));
        shot(&mut app, &mut cv, None, dir, &p("01d-accueil-reveil-en-cours"));
        app.waking = None;
        let wake_b = app.store.host_mut("b").map(|h| std::mem::take(&mut h.wake)).unwrap_or_default();
        app.go(Screen::Home);
        shot(&mut app, &mut cv, None, dir, &p("01e-accueil-reessayer"));
        if let Some(h) = app.store.host_mut("b") {
            h.wake = wake_b;
        }
        // Les réglages s'écartent du profil : Personnalisé.
        app.store.selected = Some("c".into());
        if let Some(h) = app.store.host_mut("c") {
            h.settings.mbps = 80;
        }
        app.go(Screen::Home);
        shot(&mut app, &mut cv, None, dir, &p("01c-accueil-personnalise"));
        app.store.selected = Some("a".into());
        app.go(Screen::Home);
        app.images.clear();
        app.update = crate::app::UpdateState::Available { version: "0.5.1".into(), channel: "stable".into(), page: String::new(), file: String::new(), url: String::new(), sha256: String::new(), size: 0 };
        shot(&mut app, &mut cv, None, dir, &p("02-accueil-signature"));
        app.update = crate::app::UpdateState::Unknown;
        app.images.insert("a".into(), demo_image(1));

        let mut empty = App::new(Store::default(), layout);
        empty.found = app.found.clone();
        empty.machine = app.machine.clone();
        empty.net = app.net.clone();
        empty.go(Screen::Home);
        shot(&mut empty, &mut cv, None, dir, &p("03-accueil-vide"));

        if !narrow {
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
            app.add.step = AddStep::Mac;
            app.add.mac = "3C:7C:3F:12:A".into();
            app.go(Screen::Add);
            app.focus = Target::Key('B');
            shot(&mut app, &mut cv, None, dir, &p("06b-reveil-mac"));
        }

        // Les réglages de l'ordinateur, rubrique par rubrique.
        let sections: &[(Section, &str)] = &[
            (Section::Profile, "07a-reglages-profil"),
            (Section::Picture, "07b-reglages-image"),
            (Section::Sound, "07c-reglages-son"),
            (Section::Controls, "07d-reglages-commandes"),
            (Section::Network, "07e-reglages-reseau"),
            (Section::Statistics, "07f-reglages-statistiques"),
            (Section::Station, "07h-reglages-station"),
        ];
        for (section, name) in sections {
            // Le profil est une rangée de boutons de la colonne du bureau.
            if *section == Section::Profile && layout == Layout::Desktop {
                continue;
            }
            app.open_host_section(*section, Screen::Home);
            if layout.console() && *section == Section::Picture {
                app.focus = Target::Setting(Setting::Display);
            }
            shot(&mut app, &mut cv, None, dir, &p(name));
        }
        // Réseau : pas encore de carte apprise.
        let wake_a = app.store.host_mut("a").map(|h| std::mem::take(&mut h.wake)).unwrap_or_default();
        app.open_host_section(Section::Network, Screen::Home);
        shot(&mut app, &mut cv, None, dir, &p("07e2-reglages-reseau-sans-adresse"));
        if let Some(h) = app.store.host_mut("a") {
            h.wake = wake_a;
        }
        // Statistiques sans session.
        let last = app.store.host_mut("a").and_then(|h| h.last.take());
        app.open_host_section(Section::Statistics, Screen::Home);
        shot(&mut app, &mut cv, None, dir, &p("07g-reglages-statistiques-vide"));
        if let Some(h) = app.store.host_mut("a") {
            h.last = last;
        }
        // La Station : une version disponible ; la mise à jour en cours ; faite ;
        // une version qui demande l'installateur ; une Station ancienne.
        let set_station = |app: &mut App, s: StationLive| {
            if let Some(l) = app.live.get_mut("a") {
                l.station = Some(s);
            }
        };
        set_station(&mut app, station("0.5.0", Some("0.5.1")));
        app.open_host_section(Section::Station, Screen::Home);
        app.focus = Target::Setting(Setting::StationUpdate);
        shot(&mut app, &mut cv, None, dir, &p("07i-reglages-station-version"));
        app.station_op = Some(StationOp { host: "a".into(), kind: StationKind::Update, step: StationStep::Waiting { since: Instant::now(), target: Some("0.5.1".into()), saw_down: true } });
        app.focus = Target::Section(6);
        shot(&mut app, &mut cv, None, dir, &p("07j-reglages-station-en-cours"));
        app.station_op = Some(StationOp { host: "a".into(), kind: StationKind::Update, step: StationStep::Done(if en { "KyberNow Station 0.5.1 is installed.".into() } else { "KyberNow Station 0.5.1 est en place.".into() }) });
        set_station(&mut app, station("0.5.1", None));
        shot(&mut app, &mut cv, None, dir, &p("07k-reglages-station-faite"));
        app.station_op = None;
        let mut setup = station("0.5.0", Some("0.6.0"));
        setup.installable = false;
        setup.requires_setup = true;
        set_station(&mut app, setup);
        app.open_host_section(Section::Station, Screen::Home);
        shot(&mut app, &mut cv, None, dir, &p("07l-reglages-station-installateur"));
        set_station(&mut app, StationLive { version: "0.4.9".into(), ..Default::default() });
        app.open_host_section(Section::Station, Screen::Home);
        shot(&mut app, &mut cv, None, dir, &p("07m-reglages-station-ancienne"));
        set_station(&mut app, station("0.5.0", Some("0.5.1")));
        app.open_host_section(Section::Station, Screen::Home);
        app.confirm = Some(Confirm::Force { host: "a".into(), kind: StationKind::Restart, device: "iPad de Léa".into() });
        app.focus = Target::ConfirmNo;
        shot(&mut app, &mut cv, None, dir, &p("07n-reglages-station-confirmation"));
        app.confirm = Some(Confirm::Remove { host: "a".into(), name: "Tour du salon".into() });
        shot(&mut app, &mut cv, None, dir, &p("07o-reglages-retirer"));
        app.confirm = None;
        set_station(&mut app, station("0.5.0", None));

        app.open_settings(true, Screen::Home);
        shot(&mut app, &mut cv, None, dir, &p("08-reglages-app"));
        if narrow {
            continue;
        }

        app.pad_selected = 0;
        app.go(Screen::Gamepads);
        shot(&mut app, &mut cv, None, dir, &p("09-manettes"));

        // La connexion : ses étapes, puis l'image seule.
        let mut s = session(&app);
        s.phase = Phase::Connecting;
        s.opened_at = None;
        s.menu = false;
        app.set_session(Some(s));
        app.screen = Screen::Session;
        app.focus = Target::CancelConnect;
        shot(&mut app, &mut cv, None, dir, &p("10-connexion"));
        // Le dernier recours : la Station redémarre, puis les étapes reprennent.
        if let Some(s) = app.session_mut() {
            s.phase = Phase::StationRestarting;
        }
        shot(&mut app, &mut cv, None, dir, &p("10a-connexion-station-redemarre"));
        let mut s = session(&app);
        s.phase = Phase::Ended { error: Some(if en { "Admin did not answer: is the computer still on the network?" } else { "Admin ne répond plus : l'ordinateur est-il toujours sur le réseau ?" }.into()) };
        s.opened_at = None;
        app.set_session(Some(s));
        app.focus = Target::Back;
        shot(&mut app, &mut cv, None, dir, &p("10b-connexion-echec"));
        let mut s = session(&app);
        s.menu = false;
        s.opened_at = Some(Instant::now() - Duration::from_millis(100));
        app.set_session(Some(s));
        let overlay = ui::session_overlay_visible(&app);
        assert!(!overlay, "l'image seule dès la première image");
        shot(&mut app, &mut cv, Some(&video), dir, &p("10c-image-seule"));
        // Au bureau, souris du bureau : le trait qui signale la barre, seul par-dessus l'image.
        if layout == Layout::Desktop {
            let mut s = session(&app);
            s.menu = false;
            s.relative = false;
            app.set_session(Some(s));
            assert!(ui::session_hint_only(&app));
            shot(&mut app, &mut cv, Some(&video), dir, &p("10d-image-trait-barre"));
        }

        let mut s = session(&app);
        if layout == Layout::Desktop {
            s.menu = false;
            s.bar = true;
            s.bar_menu = Some(BarMenu::Profile);
            app.pointer = Some((700.0, 20.0));
        }
        app.set_session(Some(s));
        app.focus = Target::Menu(MenuItem::Resume);
        shot(&mut app, &mut cv, Some(&video), dir, &p("11-session"));
        app.pointer = None;

        let mut s = session(&app);
        s.keys = true;
        app.set_session(Some(s));
        app.focus = Target::Shortcut(0);
        shot(&mut app, &mut cv, Some(&video), dir, &p("12-session-touches"));

        let mut s = session(&app);
        s.menu = false;
        s.show_stats = true;
        app.set_session(Some(s));
        shot(&mut app, &mut cv, Some(&video), dir, &p("13-session-statistiques"));

        let mut s = session(&app);
        s.phase = Phase::Ended { error: Some(if en { "Admin did not answer: is the computer still on the network?" } else { "Admin ne répond plus : l'ordinateur est-il toujours sur le réseau ?" }.into()) };
        app.set_session(Some(s));
        app.focus = Target::Back;
        shot(&mut app, &mut cv, None, dir, &p("14-session-fin"));
        app.set_session(None);
        // Plusieurs écrans : seulement au bureau (le Deck, le salon et l'OS restent sur un écran).
        if layout == Layout::Desktop {
            multi_shots(&mut app, &mut cv, &video, dir, prefix);
            computers_shots(&mut app, &mut cv, &video, dir, prefix);
        }
    }

    // Raspberry Pi 5 sous Raspberry Pi OS (interface Bureau, écran 1080p) : les
    // écrans des maquettes « App sur Raspberry Pi OS », en pixels simples.
    let mut app = demo(en, Layout::Desktop);
    raspberry(&mut app);
    let mut cv = Canvas::new();
    cv.resize(1280, 800, 1.0);
    app.go(Screen::Home);
    shot(&mut app, &mut cv, None, dir, "rpi-01-accueil");
    app.open_host_section(Section::Picture, Screen::Home);
    shot(&mut app, &mut cv, None, dir, "rpi-07b-reglages-image");
    app.open_host_section(Section::Sound, Screen::Home);
    shot(&mut app, &mut cv, None, dir, "rpi-07c-reglages-son");
    app.open_host_section(Section::Statistics, Screen::Home);
    shot(&mut app, &mut cv, None, dir, "rpi-07f-reglages-statistiques");
    app.open_host_section(Section::Station, Screen::Home);
    shot(&mut app, &mut cv, None, dir, "rpi-07h-reglages-station");
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
        h.last = Some(LastSession { median: 9.8, p95: 14.0, seconds: 6000, mode: "3840 × 2160".into(), hz: 60, codec: "HEVC".into(), mbps: 60.0, at: 0, screens: 1 });
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
    // La même, à jour (le site la montre).
    let (update, ready) = (std::mem::replace(&mut app.update, UpdateState::UpToDate), app.os_update.ready.take());
    shot(&mut app, &mut cv, None, dir, "os-10b-accueil-a-jour");
    (app.update, app.os_update.ready) = (update, ready);
    // Les réglages de l'ordinateur : Admin par un QR code (pas de navigateur sur l'OS).
    app.open_host_section(Section::Station, Screen::Home);
    shot(&mut app, &mut cv, None, dir, "os-07h-reglages-station");
    app.open_host_section(Section::Statistics, Screen::Home);
    shot(&mut app, &mut cv, None, dir, "os-07f-reglages-statistiques");
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
    // KyberNow Link OS au bureau : Admin par le QR code aussi.
    app.layout = Layout::Desktop;
    app.open_host_section(Section::Station, Screen::Home);
    shot(&mut app, &mut cv, None, dir, "os-bureau-07h-reglages-station");
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
        h.last = Some(LastSession { median: 11.2, p95: 15.0, seconds: 6000, mode: "1920 × 1080".into(), hz: 60, codec: "HEVC".into(), mbps: 28.0, at: 0, screens: 1 });
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

    /// Au Deck et au salon, « Réglages » se voit sur l'accueil : le toucher et la souris l'ouvrent.
    #[test]
    fn console_settings_target() {
        for layout in [Layout::Handheld, Layout::Tv] {
            let mut app = demo(false, layout);
            let mut cv = Canvas::new();
            cv.resize(if layout == Layout::Tv { 1920 } else { 1280 }, if layout == Layout::Tv { 1080 } else { 800 }, 1.0);
            app.go(Screen::Home);
            app.hits = ui::draw(&app, &mut cv);
            let (_, r) = *app.hits.iter().find(|(t, _)| *t == Target::HostSettings).expect("Réglages sur l'accueil");
            app.pointer_click(r.x + 4.0, r.y + 4.0);
            assert_eq!((app.screen, app.settings_global), (Screen::Settings, false));
        }
    }

    /// Dès la première image, rien par-dessus : ni passage, ni capsule.
    #[test]
    fn picture_alone() {
        let mut app = demo(false, Layout::Handheld);
        let mut cv = Canvas::new();
        cv.resize(1280, 800, 1.0);
        let mut s = session(&app);
        s.menu = false;
        s.opened_at = Some(Instant::now() - Duration::from_millis(50));
        app.set_session(Some(s));
        app.screen = Screen::Session;
        assert!(!ui::session_overlay_visible(&app));
        ui::draw(&app, &mut cv);
        assert_eq!(cv.pix.pixel(200, 400).unwrap().alpha(), 0);
    }

    /// Une session à l'image : barre fermée, menus fermés, souris du bureau.
    fn streaming(layout: Layout) -> (App, Canvas) {
        let mut app = demo(false, layout);
        let mut cv = Canvas::new();
        cv.resize(1280, 800, 1.0);
        let mut s = session(&app);
        s.menu = false;
        s.relative = false;
        s.opened_at = Some(Instant::now() - Duration::from_secs(30));
        app.set_session(Some(s));
        app.screen = Screen::Session;
        (app, cv)
    }

    /// Au bureau, à la souris du bureau, barre fermée : un trait de 220 × 3 points,
    /// centré contre le bord haut, seul par-dessus l'image (un bandeau, rien à
    /// chaque seconde). Ni en souris des jeux, ni au Deck, ni au salon, ni barre ouverte.
    #[test]
    fn bar_hint_only_on_desktop_mouse() {
        let (mut app, mut cv) = streaming(Layout::Desktop);
        assert!(ui::session_overlay_visible(&app) && ui::session_hint_only(&app));
        app.hits = ui::draw(&app, &mut cv);
        assert!(app.hits.is_empty());
        let alpha = |cv: &Canvas, x: u32, y: u32| cv.pix.pixel(x, y).unwrap().alpha();
        assert!((80..=95).contains(&alpha(&cv, 640, 1)), "capsule blanche à 35 %");
        assert_eq!(alpha(&cv, 640 - 112, 1), 0);
        assert_eq!(alpha(&cv, 640 + 112, 1), 0);
        assert_eq!(alpha(&cv, 640, 4), 0);
        assert_eq!(alpha(&cv, 200, 400), 0);

        // Barre ouverte : la barre à la place du trait, l'interface entière.
        app.session_mut().unwrap().bar = true;
        assert!(ui::session_overlay_visible(&app) && !ui::session_hint_only(&app));
        app.hits = ui::draw(&app, &mut cv);
        assert!(!app.hits.is_empty());
        // Souris des jeux : le pointeur est capturé, ni trait ni barre.
        app.session_mut().unwrap().bar = false;
        app.session_mut().unwrap().relative = true;
        assert!(!ui::session_overlay_visible(&app) && !ui::session_hint_only(&app));
        // Un réglage ouvert depuis la session : l'écran n'est plus la session.
        app.session_mut().unwrap().relative = false;
        app.screen = Screen::Settings;
        assert!(!ui::session_hint_only(&app));
        for layout in [Layout::Handheld, Layout::Tv] {
            let (app, _) = streaming(layout);
            assert!(!ui::session_overlay_visible(&app), "salon et console : rien ne change");
        }
    }

    /// La zone d'ouverture est celle de la barre : sa largeur réelle, centrée, à 2
    /// points du bord haut ; la barre reste ouverte sur son rectangle (petite marge).
    #[test]
    fn bar_zone_is_bar_width() {
        let (mut app, mut cv) = streaming(Layout::Desktop);
        app.session_mut().unwrap().bar = true;
        app.hits = ui::draw(&app, &mut cv);
        // Où la barre est dessinée : première et dernière colonne peintes sur une ligne de la barre.
        let painted: Vec<u32> = (0..cv.pix.width()).filter(|x| cv.pix.pixel(*x, 30).unwrap().alpha() > 0).collect();
        let (left, right) = (*painted.first().unwrap() as f32, *painted.last().unwrap() as f32 + 1.0);
        assert!(left > 100.0 && right < 1180.0 && ((left + right) / 2.0 - 640.0).abs() < 1.0, "centrée, plus étroite que la fenêtre");
        let s = app.session().cloned().unwrap();
        let w = cv.w;
        for y in [0.0, 1.0, 2.0] {
            assert!(ui::at_bar_edge(&mut cv, &app, &s, w, left + 2.0, y) && ui::at_bar_edge(&mut cv, &app, &s, w, right - 2.0, y));
            assert!(!ui::at_bar_edge(&mut cv, &app, &s, w, left - 2.0, y) && !ui::at_bar_edge(&mut cv, &app, &s, w, right + 2.0, y));
        }
        assert!(!ui::at_bar_edge(&mut cv, &app, &s, w, 640.0, 2.5), "au-delà de 2 points : pas contre le bord");
        // Rester ouverte : sur la barre (même hors de ses boutons) et 8 points autour, pas plus loin.
        let none: Vec<(Target, crate::app::Rect)> = Vec::new();
        assert!(ui::over_bar(&mut cv, &app, &s, w, &none, 640.0, 30.0));
        assert!(ui::over_bar(&mut cv, &app, &s, w, &none, right + 7.0, 20.0));
        assert!(!ui::over_bar(&mut cv, &app, &s, w, &none, right + 9.0, 20.0));
        assert!(ui::over_bar(&mut cv, &app, &s, w, &none, left - 7.0, 20.0));
        assert!(ui::over_bar(&mut cv, &app, &s, w, &none, 640.0, 46.0 + 7.0));
        assert!(!ui::over_bar(&mut cv, &app, &s, w, &none, 640.0, 46.0 + 9.0), "plus de bande de 58 points");
        assert!(!ui::over_bar(&mut cv, &app, &s, w, &none, 100.0, 20.0), "pas sur toute la largeur");
    }

    /// Plusieurs écrans : ce que montre chaque fenêtre de la session. L'écran principal porte la
    /// connexion ; les autres le même fond sans texte (sa couleur, pas la photo de l'accueil), ou
    /// la ligne d'un écran que l'ordinateur n'a pas pu créer ; chaque écran passe à son image
    /// dès sa première image ; le trait du haut dans toutes, la barre dans une seule.
    #[test]
    fn windows_of_several_displays() {
        let (mut app, mut cv) = streaming(Layout::Desktop);
        app.session_mut().unwrap().screens = 3;
        app.session_mut().unwrap().expected_screens = 3;
        let main = ui::View { session: None, main: true, bar: false, missing: false, has_frame: false };
        let other = ui::View { session: None, main: false, bar: false, missing: false, has_frame: false };
        // La connexion : l'écran principal dessine les étapes, les autres un aplat sans texte.
        app.session_mut().unwrap().phase = Phase::Connecting;
        app.session_mut().unwrap().opened_at = None;
        assert!(ui::overlay_visible(&app, main) && ui::overlay_visible(&app, other));
        ui::draw_view(&app, &mut cv, other);
        let flat = (cv.pix.pixel(10, 10).unwrap(), cv.pix.pixel(640, 400).unwrap(), cv.pix.pixel(1270, 790).unwrap());
        assert!(flat.0 == flat.1 && flat.1 == flat.2, "un aplat : pas de photo, pas de tube, pas de texte");
        assert_eq!((flat.0.red(), flat.0.green(), flat.0.blue(), flat.0.alpha()), (5, 7, 10, 255));
        let hits = ui::draw_view(&app, &mut cv, main);
        assert!(hits.iter().any(|(t, _)| *t == Target::CancelConnect), "Annuler : sur l'écran principal");
        // Un écran que l'ordinateur n'a pas pu créer : le même fond, avec une seule ligne.
        ui::draw_view(&app, &mut cv, ui::View { missing: true, ..other });
        let lit = (0..cv.pix.width()).filter(|x| cv.pix.pixel(*x, 400).unwrap() != flat.0).count();
        assert!(lit > 100, "la ligne d'un écran non créé se lit au milieu");
        let hits = ui::draw_view(&app, &mut cv, ui::View { missing: true, ..other });
        assert!(hits.is_empty());

        // À l'image : un écran qui n'a pas encore la sienne garde le fond ; avec elle, le trait seul.
        app.session_mut().unwrap().phase = Phase::Streaming;
        app.session_mut().unwrap().opened_at = Some(Instant::now() - Duration::from_secs(30));
        assert!(ui::overlay_visible(&app, other) && !ui::hint_only(&app, other), "pas d'image : le fond de la connexion");
        let shown = ui::View { has_frame: true, ..other };
        assert!(ui::hint_only(&app, shown) && ui::hint_only(&app, ui::View { has_frame: true, ..main }), "le trait du haut dans chaque fenêtre");
        // Barre ouverte dans une fenêtre : elle y est seule ; les autres montrent l'image seule.
        app.session_mut().unwrap().bar = true;
        let bar = ui::View { bar: true, ..shown };
        assert!(!ui::hint_only(&app, bar) && ui::overlay_visible(&app, bar));
        let hits = ui::draw_view(&app, &mut cv, bar);
        assert!(!hits.is_empty() && hits.iter().any(|(t, _)| *t == Target::BarQuit));
        assert!(!hits.iter().any(|(t, _)| *t == Target::BarFullscreen), "« Plein écran » n'est pas proposé avec plusieurs écrans");
        assert!(!ui::overlay_visible(&app, shown), "ni trait ni barre ailleurs : l'image seule");
        let hits = ui::draw_view(&app, &mut cv, shown);
        assert!(hits.is_empty());
        // Une seule fenêtre : tout est comme avant, avec « Plein écran ».
        app.session_mut().unwrap().screens = 1;
        app.session_mut().unwrap().expected_screens = 1;
        let hits = ui::draw(&app, &mut cv);
        assert!(hits.iter().any(|(t, _)| *t == Target::BarFullscreen));
    }

    /// La barre de plusieurs écrans dit leur nombre et n'a pas de « Plein écran » : plus étroite ;
    /// chaque fenêtre a sa largeur (le geste d'ouverture est centré sur la sienne).
    #[test]
    fn bar_of_several_displays() {
        let width = |cv: &Canvas| {
            let painted: Vec<u32> = (0..cv.pix.width()).filter(|x| cv.pix.pixel(*x, 30).unwrap().alpha() > 0).collect();
            (*painted.first().unwrap(), *painted.last().unwrap() + 1)
        };
        let (mut app, mut cv) = streaming(Layout::Desktop);
        app.session_mut().unwrap().bar = true;
        ui::draw(&app, &mut cv);
        let one = width(&cv);
        app.session_mut().unwrap().screens = 3;
        ui::draw(&app, &mut cv);
        let three = width(&cv);
        assert!(three.1 - three.0 > 100 && one != three, "la barre de plusieurs écrans a sa propre largeur : son texte dit leur nombre, sans le bouton « Plein écran »");
        // La zone d'ouverture est celle de la barre, centrée sur la fenêtre dont on donne la largeur.
        let s = app.session().cloned().unwrap();
        let (left, right) = (three.0 as f32, three.1 as f32);
        assert!(ui::at_bar_edge(&mut cv, &app, &s, 1280.0, left + 2.0, 1.0));
        for w in [1920.0f32, 2560.0] {
            let shift = (w - 1280.0) / 2.0;
            assert!(ui::at_bar_edge(&mut cv, &app, &s, w, left + shift + 2.0, 1.0) && ui::at_bar_edge(&mut cv, &app, &s, w, right + shift - 2.0, 1.0));
            assert!(!ui::at_bar_edge(&mut cv, &app, &s, w, left + shift - 2.0, 1.0) && !ui::at_bar_edge(&mut cv, &app, &s, w, right + shift + 2.0, 1.0));
        }
    }

    /// Plusieurs ordinateurs (docs/link-ui.md) : la barre a le menu « Ordinateurs », présenté comme « Touches »
    /// (une icône, dans le même groupe) s'il y a un autre ordinateur, et le haut-parleur « Garder le son » à
    /// partir de deux sessions ouvertes ; au bureau seulement. Le menu liste les autres ordinateurs, jamais celui
    /// de la session ; chaque bouton se nomme au survol.
    #[test]
    fn bar_of_several_computers() {
        let (mut app, mut cv) = streaming(Layout::Desktop);
        app.session_mut().unwrap().bar = true;
        let hits = ui::draw(&app, &mut cv);
        let has = |hits: &[(Target, crate::app::Rect)], t: Target| hits.iter().any(|(h, _)| *h == t);
        // Une seule session : « Ordinateurs » à côté de « Touches », pas de haut-parleur (la barre reste comme avant).
        assert!(has(&hits, Target::BarComputers) && !has(&hits, Target::BarSound));
        let rect = |hits: &[(Target, crate::app::Rect)], t: Target| hits.iter().find(|(h, _)| *h == t).map(|(_, r)| *r).unwrap();
        let (computers, keys) = (rect(&hits, Target::BarComputers), rect(&hits, Target::BarKeys));
        assert!((computers.y, computers.w, computers.h) == (keys.y, keys.w, keys.h) && computers.x + computers.w == keys.x, "même bouton, juste avant « Touches »");
        // Le menu : les deux autres ordinateurs (b et c), pas « Tour du salon » (a), celui de la session.
        app.session_mut().unwrap().bar_menu = Some(BarMenu::Computers);
        let hits = ui::draw(&app, &mut cv);
        assert!(has(&hits, Target::BarComputer(1)) && has(&hits, Target::BarComputer(2)) && !has(&hits, Target::BarComputer(0)));
        // Une seconde session ouverte : le haut-parleur apparaît.
        app.session_mut().unwrap().bar_menu = None;
        let mut second = SessionUi::new(2, "c", "Mini du bureau", Profile::Bureau, false, false, 1);
        second.phase = Phase::Streaming;
        app.open_session(second);
        app.activate_session(1);
        app.session_mut().unwrap().bar = true;
        let hits = ui::draw(&app, &mut cv);
        assert!(has(&hits, Target::BarSound));
        // « Garder le son » allumé : le bouton est teinté, comme souris et plein écran allumés.
        let r = rect(&hits, Target::BarSound);
        let before = cv.pix.pixel((r.x + 2.0) as u32, (r.y + 2.0) as u32).unwrap();
        app.session_mut().unwrap().keep_sound = true;
        ui::draw(&app, &mut cv);
        let on = cv.pix.pixel((r.x + 2.0) as u32, (r.y + 2.0) as u32).unwrap();
        assert!(on != before && on.alpha() > 0, "le bouton allumé se voit");
        // Le survol nomme le bouton, sous la barre ; hors survol, rien sous la barre.
        let below = |cv: &Canvas| cv.pix.pixel((r.x + r.w / 2.0) as u32, 46 + 4 + 13).unwrap().alpha();
        app.pointer = None;
        ui::draw(&app, &mut cv);
        assert_eq!(below(&cv), 0);
        app.pointer = Some((r.x + r.w / 2.0, r.y + r.h / 2.0));
        ui::draw(&app, &mut cv);
        assert!(below(&cv) > 0, "« Garder le son » sous le haut-parleur");
        // Un seul ordinateur enregistré : pas de menu ; deux sessions : le haut-parleur reste.
        app.pointer = None;
        app.store.hosts.truncate(1);
        let hits = ui::draw(&app, &mut cv);
        assert!(!has(&hits, Target::BarComputers) && has(&hits, Target::BarSound));
        // Le Steam Deck, le salon et KyberNow Link OS gardent une session : ni l'un ni l'autre.
        for layout in [Layout::Handheld, Layout::Tv] {
            let (mut app, mut cv) = streaming(layout);
            app.session_mut().unwrap().bar = true;
            let hits = ui::draw(&app, &mut cv);
            assert!(!has(&hits, Target::BarComputers) && !has(&hits, Target::BarSound), "{layout:?}");
        }
        let (mut app, mut cv) = streaming(Layout::Desktop);
        app.os = Some(crate::app::OsInfo::default());
        app.session_mut().unwrap().bar = true;
        let hits = ui::draw(&app, &mut cv);
        assert!(!has(&hits, Target::BarComputers) && !has(&hits, Target::BarSound), "KyberNow Link OS");
    }

    /// Chaque fenêtre dessine la session qui est la sienne : celle qui n'est pas active garde sa
    /// connexion (avec son Annuler) ou son image, même quand la session active montre un autre écran.
    #[test]
    fn each_window_draws_its_own_session() {
        let (mut app, mut cv) = streaming(Layout::Desktop);
        let mut b = SessionUi::new(2, "c", "Mini du bureau", Profile::Bureau, false, false, 1);
        b.phase = Phase::Connecting;
        app.open_session(b);
        app.activate_session(1);
        let own = |id| ui::View { session: Some(id), ..ui::View::SINGLE };
        let hits = ui::draw_view(&app, &mut cv, own(2));
        assert!(hits.iter().any(|(t, _)| *t == Target::CancelConnect), "la connexion de la session de la fenêtre, active ou non");
        let hits = ui::draw_view(&app, &mut cv, own(1));
        assert!(!hits.iter().any(|(t, _)| *t == Target::CancelConnect), "la session à l'image n'a pas de connexion");
        // La session active montre les manettes : l'autre fenêtre garde sa session.
        app.screen = Screen::Gamepads;
        let hits = ui::draw_view(&app, &mut cv, own(2));
        assert!(hits.iter().any(|(t, _)| *t == Target::CancelConnect));
        assert!(ui::overlay_visible(&app, own(2)) && !ui::overlay_visible(&app, own(1)), "la fenêtre de la session active est l'écran des manettes, pas l'image seule");
        // Seule la session active a une barre (le trait du haut) : une session qui n'est pas active ne reçoit rien.
        app.screen = Screen::Session;
        app.session_by_mut(2).unwrap().phase = Phase::Streaming;
        assert!(ui::hint_only(&app, own(1)) && !ui::overlay_visible(&app, own(2)));
        app.activate_session(2);
        assert!(ui::hint_only(&app, own(2)) && !ui::overlay_visible(&app, own(1)));
        // Une session finie sur une erreur : « Fermer » avec d'autres sessions, « Retour à l'accueil » seule.
        app.session_by_mut(2).unwrap().phase = Phase::Ended { error: Some("coupée".into()) };
        app.session_by_mut(2).unwrap().opened_at = None;
        let lit = |cv: &Canvas| (0..cv.pix.width()).filter(|x| cv.pix.pixel(*x, 400).unwrap().alpha() > 0).count();
        ui::draw_view(&app, &mut cv, own(2));
        let with_others = lit(&cv);
        app.close_session(1);
        ui::draw_view(&app, &mut cv, own(2));
        assert!(with_others > 0 && lit(&cv) > 0);
    }

    /// Les captures de plusieurs ordinateurs se produisent, dans les deux langues.
    #[test]
    fn several_computers_shots() {
        let dir = std::env::temp_dir().join(format!("kn-linux-ordinateurs-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        for en in [false, true] {
            let mut app = demo(en, Layout::Desktop);
            let mut cv = Canvas::new();
            cv.resize(1280, 800, 1.0);
            computers_shots(&mut app, &mut cv, &demo_image(3), &dir, "bureau");
        }
        assert!(dir.join("bureau-17-barre-ordinateurs.png").exists() && dir.join("bureau-17d-fenetre-suivante.png").exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Les captures de plusieurs écrans se produisent, dans les deux langues.
    #[test]
    fn several_displays_shots() {
        let dir = std::env::temp_dir().join(format!("kn-linux-ecrans-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        for en in [false, true] {
            let mut app = demo(en, Layout::Desktop);
            let mut cv = Canvas::new();
            cv.resize(1280, 800, 1.0);
            multi_shots(&mut app, &mut cv, &demo_image(3), &dir, "bureau");
        }
        assert!(dir.join("bureau-16c-ecran-non-cree.png").exists() && dir.join("bureau-15e-reglages-statistiques-ecrans.png").exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Les sept rubriques des réglages de l'ordinateur se dessinent aux trois
    /// interfaces, avec et sans dernière session, Station ancienne comprise.
    #[test]
    fn host_sections_draw() {
        for (layout, w, h) in [(Layout::Desktop, 960, 600), (Layout::Handheld, 1280, 800), (Layout::Tv, 1920, 1080)] {
            let mut app = demo(true, layout);
            let mut cv = Canvas::new();
            cv.resize(w, h, 1.0);
            for s in crate::app::HOST_SECTIONS {
                app.open_host_section(s, Screen::Home);
                app.hits = ui::draw(&app, &mut cv);
            }
            app.live.get_mut("a").unwrap().station = Some(StationLive { version: "0.4.9".into(), ..Default::default() });
            app.store.host_mut("a").unwrap().last = None;
            app.open_host_section(Section::Station, Screen::Home);
            app.hits = ui::draw(&app, &mut cv);
            assert!(!app.hits.iter().any(|(t, _)| *t == Target::Setting(Setting::StationRestart)), "Station ancienne : pas de redémarrage");
            app.open_host_section(Section::Statistics, Screen::Home);
            ui::draw(&app, &mut cv);
        }
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
