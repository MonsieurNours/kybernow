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

use crate::app::{
    AddStep, App, Found, HostLive, MenuItem, NetKind, Online, PadInfo, Phase, Polled, Problem, Screen, Section, SessionUi, Setting, StationKnown, StationLive, StationOp, StationPhase, StationRefusal, StationUi, Stats, Target,
    UpdateState, HOST_SECTIONS,
};
use crate::model::{Host, HostSettings, LastSession, Profile, Store, WakeTarget};
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
        gpu: Some("NVIDIA GeForce RTX 3080 Ti".into()),
        av1: true,
        mac: None,
        wake: Vec::new(),
        settings: HostSettings::default(),
        last: None,
    }
}

/// Une carte Ethernet de démonstration, comme Admin la donne.
fn wake_target(mac: &str, ipv4: &str) -> WakeTarget {
    WakeTarget { mac: mac.into(), ipv4: Some(ipv4.into()), broadcast: Some("192.168.1.255".into()), kind: "ethernet".into(), armed: Some(true), name: "Intel(R) Ethernet Connection I219-V".into() }
}

/// La Station de démonstration : 0.5.0, à jour.
fn station(version: &str) -> StationKnown {
    StationKnown::Info(StationLive { version: version.into(), ..Default::default() })
}

fn demo(english: bool) -> App {
    crate::i18n::set_english(english);
    let mut store = Store::default();
    let mut a = host("a", "Atelier", "192.168.1.20");
    let now = crate::model::now_secs();
    a.last = Some(LastSession {
        median: 8.9,
        p95: 14.2,
        seconds: 42 * 60 + 10,
        mode: "2560 × 1440 · HEVC".into(),
        mbps: 79.6,
        at: now - 86_400,
        width: 2560,
        height: 1440,
        refresh: 120,
        codec: "HEVC".into(),
    });
    a.wake = vec![wake_target("3c:7c:3f:12:ab:cd", "192.168.1.20")];
    store.hosts.push(a);
    store.hosts.push(host("b", "Portable", "192.168.1.31"));
    store.selected = Some("a".into());
    let mut app = App::new(store);
    app.tv.model = "OLED83G2".into();
    app.tv.webos = "25".into();
    app.net.kind = NetKind::Wifi;
    app.live.insert("a".into(), HostLive { online: Online::Online, virtual_display: Some(true), station: station("0.5.0"), problem: None, own_device: Some(true) });
    app.live.insert("b".into(), HostLive { online: Online::Offline, ..Default::default() });
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
        streamed: true,
        menu: true,
        keys: false,
        stats: Some(Stats { latency_ms: Some(8.9), fps: 120.0, mbps: 80.0, lost: 0, mode: "2560 × 1440".into(), codec: "HEVC".into() }),
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
            // Le plan vidéo dessous, l'interface par-dessus (rien en jeu).
            let mut base = Pixmap::new(1920, 1080).unwrap();
            base.draw_pixmap(0, 0, v.as_ref(), &tiny_skia::PixmapPaint { quality: tiny_skia::FilterQuality::Bilinear, ..Default::default() }, tiny_skia::Transform::from_scale(4.0, 4.0), None);
            if app.screen != Screen::Session || ui::session_overlay_visible(app) {
                base.draw_pixmap(0, 0, cv.pix.as_ref(), &tiny_skia::PixmapPaint::default(), tiny_skia::Transform::identity(), None);
            }
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

/// Ouvre une rubrique des réglages de l'ordinateur choisi.
fn open(app: &mut App, section: Section, focus: Target) {
    app.open_settings(false, Screen::Home);
    app.section = HOST_SECTIONS.iter().position(|s| *s == section).unwrap_or(0);
    app.focus = focus;
}

pub fn run(dir: &Path, lang: &str) {
    let _ = std::fs::create_dir_all(dir);
    let en = lang == "en";
    let mut app = demo(en);
    let mut cv = Canvas::new();
    let assets = Assets::load();
    let video = demo_image(3);

    // Accueil.
    app.go(Screen::Home);
    shot(&mut app, &mut cv, &assets, None, dir, "1-accueil");
    app.update = UpdateState::Available { version: "0.5.1".into(), channel: "stable".into(), page: String::new() };
    shot(&mut app, &mut cv, &assets, None, dir, "1b-accueil-mise-a-jour");
    app.update = UpdateState::Unknown;

    app.store.selected = Some("b".into());
    app.go(Screen::Home);
    shot(&mut app, &mut cv, &assets, None, dir, "1c-accueil-hors-ligne");
    app.store.host_mut("b").unwrap().wake = vec![wake_target("a4:bb:6d:01:02:03", "192.168.1.31")];
    app.live.get_mut("b").unwrap().problem = Some(Problem::Reinstalled);
    shot(&mut app, &mut cv, &assets, None, dir, "1d-accueil-reveil-erreur");
    app.live.get_mut("b").unwrap().problem = None;
    let _ = app.activate(Target::Connect);
    shot(&mut app, &mut cv, &assets, None, dir, "1g-accueil-reveil-en-cours");
    app.waking = None;
    app.store.host_mut("b").unwrap().wake.clear();
    app.store.selected = Some("a".into());

    // Profil personnalisé, définition sur mesure, Station sans écran virtuel.
    let saved = app.store.host("a").unwrap().settings.clone();
    {
        let st = &mut app.store.host_mut("a").unwrap().settings;
        st.mbps = 60;
        st.resolution = 1080;
        st.fps = 60;
    }
    app.pads.truncate(1);
    app.go(Screen::Home);
    shot(&mut app, &mut cv, &assets, None, dir, "1e-accueil-personnalise");
    app.live.get_mut("a").unwrap().virtual_display = Some(false);
    app.live.get_mut("a").unwrap().online = Online::Unknown;
    let pads = std::mem::take(&mut app.pads);
    shot(&mut app, &mut cv, &assets, None, dir, "1f-accueil-recherche-tels-quels");
    app.pads = pads;
    app.live.get_mut("a").unwrap().virtual_display = Some(true);
    app.live.get_mut("a").unwrap().online = Online::Online;
    app.store.host_mut("a").unwrap().settings = saved;
    app.pads = demo(en).pads;

    let mut empty = App::new(Store::default());
    empty.found = app.found.clone();
    empty.tv = app.tv.clone();
    empty.net = app.net.clone();
    empty.go(Screen::Home);
    shot(&mut empty, &mut cv, &assets, None, dir, "2-accueil-vide");
    crate::i18n::set_english(en);

    // Ajout.
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

    // Réglages de l'ordinateur.
    open(&mut app, Section::Picture, Target::Setting(Setting::Fps));
    shot(&mut app, &mut cv, &assets, None, dir, "6-reglages-image");
    open(&mut app, Section::Sound, Target::Section(1));
    shot(&mut app, &mut cv, &assets, None, dir, "7-reglages-son");
    open(&mut app, Section::Controls, Target::Section(2));
    shot(&mut app, &mut cv, &assets, None, dir, "8-reglages-commandes");
    open(&mut app, Section::Network, Target::Setting(Setting::Transport));
    shot(&mut app, &mut cv, &assets, None, dir, "9-reglages-reseau");
    let wake = std::mem::take(&mut app.store.host_mut("a").unwrap().wake);
    open(&mut app, Section::Network, Target::Section(3));
    shot(&mut app, &mut cv, &assets, None, dir, "9b-reglages-reseau-sans-adresse");
    app.store.host_mut("a").unwrap().wake = wake;
    open(&mut app, Section::Statistics, Target::Section(4));
    shot(&mut app, &mut cv, &assets, None, dir, "10-reglages-statistiques");
    let last = app.store.host_mut("a").unwrap().last.take();
    open(&mut app, Section::Statistics, Target::Chip(Profile::Jeu));
    shot(&mut app, &mut cv, &assets, None, dir, "10b-reglages-statistiques-sans-session");
    app.store.host_mut("a").unwrap().last = last;

    // Station.
    let set_station = |app: &mut App, st: StationKnown| app.live.get_mut("a").unwrap().station = st;
    open(&mut app, Section::Station, Target::Setting(Setting::Restart));
    shot(&mut app, &mut cv, &assets, None, dir, "11-reglages-station-a-jour");
    let available = StationKnown::Info(StationLive {
        version: "0.5.0".into(),
        update_available: true,
        update_version: Some("0.5.1".into()),
        installable: true,
        ..Default::default()
    });
    set_station(&mut app, available.clone());
    open(&mut app, Section::Station, Target::Setting(Setting::Update));
    shot(&mut app, &mut cv, &assets, None, dir, "11b-reglages-station-mise-a-jour-disponible");
    set_station(&mut app, StationKnown::Info(StationLive {
        version: "0.5.0".into(),
        update_available: true,
        update_version: Some("0.6.0".into()),
        requires_setup: true,
        ..Default::default()
    }));
    open(&mut app, Section::Station, Target::Setting(Setting::Restart));
    shot(&mut app, &mut cv, &assets, None, dir, "11c-reglages-station-installateur");
    set_station(&mut app, available);
    open(&mut app, Section::Station, Target::Section(5));
    app.station = Some(StationUi { host: "a".into(), phase: StationPhase::Asking(StationOp::Update), message: None });
    shot(&mut app, &mut cv, &assets, None, dir, "11d-reglages-station-mise-a-jour-en-cours");
    app.station = Some(StationUi { host: "a".into(), phase: StationPhase::Waiting { op: StationOp::Restart, since: Instant::now(), down: true, target: None }, message: None });
    shot(&mut app, &mut cv, &assets, None, dir, "11e-reglages-station-redemarre");
    // Retour de la Station après la mise à jour : le message de fin.
    app.station = Some(StationUi { host: "a".into(), phase: StationPhase::Waiting { op: StationOp::Update, since: Instant::now(), down: true, target: Some("0.5.1".into()) }, message: None });
    app.polled("a", Polled { online: true, station: station("0.5.1"), virtual_display: Some(true), ..Default::default() });
    open(&mut app, Section::Station, Target::Setting(Setting::Restart));
    shot(&mut app, &mut cv, &assets, None, dir, "11f-reglages-station-mise-a-jour-faite");
    app.station = Some(StationUi { host: "a".into(), phase: StationPhase::Asking(StationOp::Restart), message: None });
    app.station_answer("a", StationOp::Restart, Err(StationRefusal::TooSoon));
    open(&mut app, Section::Station, Target::Setting(Setting::Restart));
    shot(&mut app, &mut cv, &assets, None, dir, "11g-reglages-station-trop-tot");
    app.station = Some(StationUi { host: "a".into(), phase: StationPhase::Asking(StationOp::Restart), message: None });
    app.station_answer("a", StationOp::Restart, Err(StationRefusal::InSession(Some("Pixel 9".into()))));
    shot(&mut app, &mut cv, &assets, None, dir, "11h-reglages-station-appareil-en-session");
    app.dialog = None;
    app.station = None;
    set_station(&mut app, StationKnown::Old(Some("0.4.9".into())));
    open(&mut app, Section::Station, Target::Setting(Setting::Remove));
    shot(&mut app, &mut cv, &assets, None, dir, "11i-reglages-station-ancienne");
    set_station(&mut app, station("0.5.0"));
    let _ = app.nav(crate::app::Nav::Ok);
    shot(&mut app, &mut cv, &assets, None, dir, "11j-reglages-station-retirer");
    app.dialog = None;

    // Réglages de l'app.
    app.update = UpdateState::Available { version: "0.5.1".into(), channel: "stable".into(), page: String::new() };
    for (i, name) in ["12-reglages-app-televiseur", "12b-reglages-app-manettes"].iter().enumerate() {
        app.open_settings(true, Screen::Home);
        app.section = i;
        app.focus = Target::Section(i);
        shot(&mut app, &mut cv, &assets, None, dir, name);
    }
    app.update = UpdateState::Unknown;

    app.pad_selected = 0;
    app.go(Screen::Gamepads);
    shot(&mut app, &mut cv, &assets, None, dir, "13-manettes");

    // Session.
    let mut s = session(&app);
    s.phase = Phase::Connecting;
    s.streamed = false;
    s.menu = false;
    app.session = Some(s);
    app.screen = Screen::Session;
    app.focus = Target::CancelConnect;
    shot(&mut app, &mut cv, &assets, None, dir, "14-session-connexion");

    let mut s = session(&app);
    s.phase = Phase::FirstImages;
    s.streamed = false;
    s.menu = false;
    app.session = Some(s);
    shot(&mut app, &mut cv, &assets, None, dir, "14b-session-premieres-images");

    let mut s = session(&app);
    s.phase = Phase::StationRestarting;
    s.streamed = false;
    s.menu = false;
    app.session = Some(s);
    shot(&mut app, &mut cv, &assets, None, dir, "14c-session-station-redemarre");

    let mut s = session(&app);
    s.menu = false;
    app.session = Some(s);
    shot(&mut app, &mut cv, &assets, Some(&video), dir, "15-session-image");

    app.session = Some(session(&app));
    app.focus = Target::Menu(MenuItem::Resume);
    shot(&mut app, &mut cv, &assets, Some(&video), dir, "16-session-menu");

    let mut s = session(&app);
    s.keys = true;
    app.session = Some(s);
    app.focus = Target::Shortcut(0);
    shot(&mut app, &mut cv, &assets, Some(&video), dir, "17-session-touches");

    let mut s = session(&app);
    s.menu = false;
    s.show_stats = true;
    app.session = Some(s);
    shot(&mut app, &mut cv, &assets, Some(&video), dir, "18-session-statistiques");

    let mut s = session(&app);
    s.streamed = false;
    s.phase = Phase::Ended { error: Some(if en { "Admin did not answer: is the computer still on the network?" } else { "Admin ne répond plus : l'ordinateur est-il toujours sur le réseau ?" }.into()) };
    app.session = Some(s);
    app.focus = Target::Back;
    shot(&mut app, &mut cv, &assets, None, dir, "19-session-echec");

    let mut s = session(&app);
    s.phase = Phase::Ended { error: Some(if en { "the computer is no longer responding" } else { "l'ordinateur ne répond plus" }.into()) };
    app.session = Some(s);
    shot(&mut app, &mut cv, &assets, None, dir, "19b-session-fin");
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::Nav;

    /// Les réglages s'atteignent à la croix depuis « Se connecter », sans
    /// touche particulière de la télécommande (télécommande tierce, manette).
    #[test]
    fn settings_reachable_with_arrows() {
        let mut app = demo(false);
        let mut cv = Canvas::new();
        let assets = Assets::load();
        let mut redraw = |app: &mut App| app.hits = ui::draw(app, &mut cv, &assets);
        app.go(Screen::Home);
        redraw(&mut app);
        for _ in 0..4 {
            app.nav(Nav::Right);
            redraw(&mut app);
        }
        assert_eq!(app.focus, Target::HostSettings);
        app.nav(Nav::Ok);
        assert_eq!((app.screen, app.settings_global), (Screen::Settings, false));
        app.nav(Nav::Back);
        assert_eq!(app.screen, Screen::Home);

        app.go(Screen::Home);
        redraw(&mut app);
        app.nav(Nav::Up);
        assert_eq!(app.focus, Target::AppSettings);
        app.nav(Nav::Ok);
        assert_eq!((app.screen, app.settings_global), (Screen::Settings, true));

        // Sans ordinateur ajouté aussi.
        let mut empty = App::new(Store::default());
        empty.go(Screen::Home);
        redraw(&mut empty);
        empty.nav(Nav::Up);
        assert_eq!(empty.focus, Target::AppSettings);
    }
}
