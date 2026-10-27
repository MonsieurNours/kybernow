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
    AddStep, App, Found, HostLive, JoinField, JoinState, MenuItem, NetKind, Online, PadInfo, Phase, Polled, Problem, Screen, Section, SessionUi, Setting, ShareAvail, ShareGuest, ShareRole, ShareView, StationKnown, StationLive, StationOp,
    StationPhase, StationRefusal, StationUi, Stats, Target, UpdateState, HOST_SECTIONS,
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
    app.live.insert("a".into(), HostLive { online: Online::Online, virtual_display: Some(true), station: station("0.5.0"), problem: None, own_device: Some(true), share: Some(ShareAvail::Available) });
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
        phase: Phase::Streaming,
        streamed: true,
        menu: true,
        stats: Some(Stats { latency_ms: Some(8.9), fps: 120.0, mbps: 80.0, lost: 0, mode: "2560 × 1440".into(), codec: "HEVC".into() }),
        started: Instant::now() - Duration::from_millis(400),
        ..SessionUi::new("a", "Atelier", Profile::Jeu, app.selected().is_some_and(|h| h.settings.game_mode_hint))
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

/// Un partage ouvert : le code, le lien, trois invités (spectateur, joueur, joueur avec le clavier et la souris).
fn shared_view() -> ShareView {
    let now = crate::model::now_secs() as i64;
    ShareView {
        code: "482917".into(),
        link: "kybernow://join?host=192.168.1.20&port=24811&fp=sha256:9f:3a:c2:71:0b:44:de:aa&code=482917".into(),
        web: "https://192.168.1.20:24811/link/?join=482917".into(),
        guests: vec![
            ShareGuest { id: "g1".into(), name: "Léa".into(), role: ShareRole::Spectator, app: "ios".into(), since: Some(now - 4 * 60) },
            ShareGuest { id: "g2".into(), name: "Tom".into(), role: ShareRole::Player, app: "android".into(), since: Some(now - 35 * 60) },
            ShareGuest { id: "g3".into(), name: "Camille".into(), role: ShareRole::PlayerKbm, app: "windows".into(), since: Some(now - 3700) },
        ],
    }
}

/// Une session d'invité à l'image, sur « Atelier ».
fn guest_session(role: ShareRole) -> SessionUi {
    let mut s = SessionUi::guest("share:g1", "Atelier", role);
    s.phase = Phase::Streaming;
    s.streamed = true;
    s.stats = Some(Stats { latency_ms: Some(9.4), fps: 60.0, mbps: 38.0, lost: 0, mode: "1920 × 1080".into(), codec: "HEVC".into() });
    s.started = Instant::now() - Duration::from_millis(400);
    s
}

/// KyberNow Share (docs/kybernow-share.md) : « Rejoindre un partage » (code, prénom, recherche, refus), le panneau
/// de l'hôte (ouverture, partage ouvert, « Partager » inactif), et la session d'invité (connexion, menu, rôles, fin).
fn share_shots(app: &mut App, cv: &mut Canvas, assets: &Assets, video: &Pixmap, dir: &Path, en: bool) {
    app.session = None;
    app.pointer = None;
    app.store.selected = Some("a".into());

    // Rejoindre : le code à taper, le prénom proposé ; le code en cours ; la recherche ; ce que le cœur a répondu ; le prénom.
    app.join = JoinState { name: "LG OLED83G2".into(), ..JoinState::new() };
    app.go(Screen::Join);
    shot(app, cv, assets, None, dir, "20-rejoindre");
    app.join.code = "4829".into();
    shot(app, cv, assets, None, dir, "20b-rejoindre-code");
    app.join.code = "482917".into();
    app.join.busy = true;
    shot(app, cv, assets, None, dir, "20c-rejoindre-recherche");
    app.join.busy = false;
    app.join.message = Some(crate::i18n::tr("Aucun partage avec ce code sur le réseau.", "No share with this code on your network.").into());
    shot(app, cv, assets, None, dir, "20d-rejoindre-refus");
    app.join.message = None;
    app.join.field = JoinField::Name;
    app.go(Screen::Join);
    shot(app, cv, assets, None, dir, "20e-rejoindre-prenom");
    app.join = JoinState::new();

    // L'hôte : le menu avec « Partager », le panneau, l'attente, le refus d'Admin, « Partager » inactif.
    let mut s = session(app);
    s.share = Some(shared_view());
    app.session = Some(s);
    app.screen = Screen::Session;
    app.focus = Target::Menu(MenuItem::Share);
    shot(app, cv, assets, Some(video), dir, "21-partage-menu");
    if let Some(s) = app.session.as_mut() {
        s.menu = false;
        s.share_panel = true;
    }
    app.focus = Target::ShareNewCode;
    shot(app, cv, assets, Some(video), dir, "21b-partage-panneau");
    if let Some(s) = app.session.as_mut() {
        s.share = Some(ShareView { guests: Vec::new(), ..shared_view() });
    }
    shot(app, cv, assets, Some(video), dir, "21c-partage-panneau-vide");
    if let Some(s) = app.session.as_mut() {
        s.share = None;
        s.share_busy = true;
    }
    shot(app, cv, assets, Some(video), dir, "21d-partage-ouverture");
    if let Some(s) = app.session.as_mut() {
        s.share_busy = false;
        s.share_error = Some(crate::i18n::tr("Une connexion est en cours sur cet ordinateur : partagez un écran une fois qu'elle est finie.", "A connection is in progress on this computer: share a screen once it has ended.").into());
    }
    shot(app, cv, assets, Some(video), dir, "21e-partage-refuse");
    // Station trop ancienne : la tuile le dit, le choisir aussi.
    app.live.get_mut("a").unwrap().share = Some(ShareAvail::Unsupported);
    app.session = Some(session(app));
    app.focus = Target::Menu(MenuItem::Share);
    shot(app, cv, assets, Some(video), dir, "21f-partage-inactif");
    app.live.get_mut("a").unwrap().share = Some(ShareAvail::Available);

    // L'invité : la connexion, le menu d'un spectateur, d'un joueur, d'un joueur avec le clavier et la souris, l'avis, la fin.
    let mut g = guest_session(ShareRole::Spectator);
    g.phase = Phase::Connecting;
    g.streamed = false;
    app.session = Some(g);
    app.focus = Target::CancelConnect;
    shot(app, cv, assets, None, dir, "22-invite-connexion");
    for (role, name) in [(ShareRole::Spectator, "22b-invite-spectateur"), (ShareRole::Player, "22c-invite-joueur"), (ShareRole::PlayerKbm, "22d-invite-joueur-clavier")] {
        let mut g = guest_session(role);
        g.menu = true;
        app.session = Some(g);
        app.focus = Target::Menu(MenuItem::Resume);
        shot(app, cv, assets, Some(video), dir, name);
    }
    let mut g = guest_session(ShareRole::Player);
    g.notice = ShareRole::Player.notice(Some(ShareRole::Spectator)).map(|t| (t.to_string(), Instant::now()));
    app.session = Some(g);
    shot(app, cv, assets, Some(video), dir, "22e-invite-avis");
    let mut g = guest_session(ShareRole::Player);
    g.phase = Phase::Ended { error: Some(if en { "Léa stopped sharing." } else { "Léa a arrêté le partage." }.into()) };
    app.session = Some(g);
    app.focus = Target::Back;
    shot(app, cv, assets, None, dir, "22f-invite-fin");
    app.session = None;
    app.screen = Screen::Home;
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

    // KyberNow Share : rejoindre, le panneau de l'hôte, la session d'invité.
    share_shots(&mut app, &mut cv, &assets, &video, dir, en);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::{
        app_label, duration_label, format_code, guests_label, Action, Nav, ShareOp, NAME_MAX,
    };

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

    // MARK: - KyberNow Share

    /// Un téléviseur avec un ordinateur « a » en ligne, dont le partage est possible.
    fn tv() -> App {
        crate::i18n::force_english(false);
        let mut app = demo(false);
        app.tv.model = "OLED83G2".into();
        app.live.get_mut("a").unwrap().share = Some(ShareAvail::Available);
        app
    }

    /// « Rejoindre avec un code » : l'écran s'ouvre sur le prénom proposé (« LG … », puis celui de la fois d'avant),
    /// les chiffres se tapent (six au plus, rien d'autre ; le sixième porte le focus sur « Rejoindre »), « Rejoindre »
    /// ne part qu'avec six chiffres et un prénom et retient le prénom ; le refus du cœur s'affiche ; Retour abandonne.
    #[test]
    fn joining_a_share_with_the_remote() {
        let mut app = tv();
        app.go(Screen::Home);
        assert!(app.activate(Target::JoinCard).is_empty());
        assert_eq!((app.screen, app.join.name.as_str(), app.join.field, app.focus), (Screen::Join, "LG OLED83G2", JoinField::Code, Target::Key('1')));
        // Les touches chiffrées de la télécommande, ou le pavé ; une lettre n'entre pas dans le code.
        for c in "48a29 17x3".chars() {
            app.nav(Nav::Char(c));
        }
        assert_eq!((app.join.code.as_str(), app.focus), ("482917", Target::JoinGo));
        // Sans prénom : le focus va au prénom, rien ne part.
        app.join.name.clear();
        assert!(app.activate(Target::JoinGo).is_empty());
        assert_eq!((app.join.field, app.focus, app.join.busy), (JoinField::Name, Target::Key('q'), false));
        app.activate(Target::KeyShift);
        app.activate(Target::Key('é'));
        app.activate(Target::Key('m'));
        app.activate(Target::Key('i'));
        assert_eq!(app.join.name, "Émi");
        app.activate(Target::KeyErase);
        assert_eq!(app.join.name, "Ém");
        let acts = app.activate(Target::JoinGo);
        assert_eq!(acts, vec![Action::Save, Action::JoinShare { code: "482917".into(), name: "Ém".into() }]);
        assert!(app.join.busy && app.store.app.share_name == "Ém");
        assert!(app.activate(Target::JoinGo).is_empty(), "une recherche à la fois");
        let attempt = app.join.attempt;
        app.join_failed(attempt + 1, "autre".into());
        assert!(app.join.busy);
        app.join_failed(attempt, "Aucun partage avec ce code sur le réseau.".into());
        assert!(!app.join.busy && app.join.message.is_some());
        // Retour abandonne : l'accueil, et la recherche en vol n'ouvre plus rien.
        app.join.busy = true;
        assert!(app.join_current(attempt));
        app.nav(Nav::Back);
        assert!(app.screen == Screen::Home && !app.join_current(attempt));
        // La touche verte rouvre l'écran, avec le prénom retenu ; ailleurs elle ne fait rien.
        app.nav(Nav::Join);
        assert_eq!((app.screen, app.join.name.as_str()), (Screen::Join, "Ém"));
        assert!(app.nav(Nav::Join).is_empty());
        // Le prénom ne dépasse pas sa longueur.
        app.activate(Target::JoinName);
        for _ in 0..40 {
            app.activate(Target::Key('x'));
        }
        assert_eq!(app.join.name.chars().count(), NAME_MAX);
    }

    /// Une Station qui ne partage pas : « Partager » dit pourquoi, rien ne part ; avec un partage possible, il ouvre le
    /// panneau et demande l'ouverture à Admin, une fois ; ses invités : « Faire jouer », le clavier et la souris, « Faire
    /// regarder », « Retirer » ; B referme le panneau, le signe de vie le met à jour.
    #[test]
    fn sharing_from_the_menu() {
        let mut app = tv();
        let mut s = SessionUi::new("a", "Atelier", Profile::Jeu, false);
        s.phase = Phase::Streaming;
        s.menu = true;
        app.session = Some(s);
        app.screen = Screen::Session;
        app.live.get_mut("a").unwrap().share = Some(ShareAvail::Unsupported);
        assert!(app.activate(Target::Menu(MenuItem::Share)).is_empty());
        assert_eq!(app.session.as_ref().unwrap().notice.as_ref().map(|n| n.0.as_str()), Some("Le partage demande KyberNow Station 0.8.0 ou plus récente."));
        app.live.get_mut("a").unwrap().share = Some(ShareAvail::Disabled);
        assert!(app.activate(Target::Menu(MenuItem::Share)).is_empty());
        assert_eq!(app.session.as_ref().unwrap().notice.as_ref().map(|n| n.0.as_str()), Some("Le partage est coupé sur cet ordinateur (Admin › Appareils Link)."));
        app.live.get_mut("a").unwrap().share = Some(ShareAvail::Available);
        app.session.as_mut().unwrap().menu = true;
        assert_eq!(app.activate(Target::Menu(MenuItem::Share)), vec![Action::ReleaseAll, Action::Share(ShareOp::Open)]);
        let s = app.session.as_ref().unwrap();
        assert!(s.share_panel && !s.menu && s.share_busy);
        assert!(app.activate(Target::ShareNewCode).is_empty(), "une demande à la fois");
        // Un signe de vie parti avant l'ouverture ne la défait pas.
        app.share_guests(None);
        assert!(app.session.as_ref().unwrap().share_panel);
        let guest = |role| ShareGuest { id: "g1".into(), name: "Léa".into(), role, app: "ios".into(), since: Some(1) };
        let view = |role| ShareView { code: "482917".into(), link: "kybernow://join?x".into(), web: "https://h:1/link/?join=482917".into(), guests: vec![guest(role)] };
        app.share_reply(Ok(Some(view(ShareRole::Spectator))));
        assert_eq!(app.focus, Target::ShareNewCode);
        let role = |r| vec![Action::Share(ShareOp::Role("g1".into(), r))];
        assert_eq!(app.activate(Target::ShareRole(0)), role(ShareRole::Player));
        app.share_reply(Ok(Some(view(ShareRole::Player))));
        assert_eq!(app.activate(Target::ShareKbm(0)), role(ShareRole::PlayerKbm));
        app.share_reply(Ok(Some(view(ShareRole::PlayerKbm))));
        assert_eq!(app.activate(Target::ShareKbm(0)), role(ShareRole::Player));
        app.share_reply(Ok(Some(view(ShareRole::Player))));
        assert_eq!(app.activate(Target::ShareRole(0)), role(ShareRole::Spectator), "« Faire regarder »");
        app.share_reply(Ok(Some(view(ShareRole::Spectator))));
        assert_eq!(app.activate(Target::ShareKick(0)), vec![Action::Share(ShareOp::Kick("g1".into()))]);
        app.share_reply(Ok(Some(ShareView { guests: Vec::new(), ..view(ShareRole::Spectator) })));
        assert_eq!(app.activate(Target::ShareNewCode), vec![Action::Share(ShareOp::NewCode)]);
        app.share_reply(Ok(Some(view(ShareRole::Spectator))));
        // Le signe de vie met la liste à jour ; B referme le panneau, l'image reprend (pas le menu).
        app.share_guests(Some(ShareView { guests: vec![guest(ShareRole::Spectator), guest(ShareRole::Player)], ..view(ShareRole::Spectator) }));
        assert_eq!(app.session.as_ref().unwrap().share.as_ref().map(|v| v.guests.len()), Some(2));
        assert!(app.session_back().is_empty());
        let s = app.session.as_ref().unwrap();
        assert!(!s.share_panel && !s.menu);
        // Le menu rouvre le panneau sans redemander l'ouverture ; « Arrêter le partage » le ferme.
        assert!(app.open_menu().len() == 1);
        assert_eq!(app.activate(Target::Menu(MenuItem::Share)), vec![Action::ReleaseAll]);
        assert_eq!(app.activate(Target::ShareStop), vec![Action::Share(ShareOp::Close)]);
        app.share_reply(Ok(None));
        assert!(app.session.as_ref().unwrap().share.is_none() && !app.session.as_ref().unwrap().share_panel);
        // Un refus d'Admin reste dans le panneau.
        app.session.as_mut().unwrap().share_panel = true;
        app.share_reply(Err("Aucun partage en cours.".into()));
        assert_eq!(app.session.as_ref().unwrap().share_error.as_deref(), Some("Aucun partage en cours."));
    }

    /// Une session d'invité : titrée « Invité sur … », jamais dans la liste des ordinateurs ; le clavier et la souris
    /// seulement avec ce rôle ; l'avis du rôle changé (dont « Vous regardez de nouveau. ») ; ni « Partager » ni profil.
    #[test]
    fn a_guest_session() {
        let mut app = tv();
        app.session = Some(SessionUi::guest("share:g1", "Atelier", ShareRole::Spectator));
        app.screen = Screen::Session;
        let s = app.session.as_ref().unwrap();
        assert_eq!((s.name.as_str(), s.role(), s.keyboard_mouse()), ("Invité sur Atelier", Some(ShareRole::Spectator), false));
        assert!(app.store.host(&s.host).is_none());
        assert!(app.activate(Target::Menu(MenuItem::Share)).is_empty() && !app.session.as_ref().unwrap().share_panel);
        app.share_role_changed(ShareRole::Player, Some(ShareRole::Spectator));
        assert_eq!(app.session.as_ref().unwrap().notice.as_ref().map(|n| n.0.as_str()), Some("Vous pouvez jouer."));
        app.share_role_changed(ShareRole::PlayerKbm, Some(ShareRole::Player));
        assert_eq!(app.session.as_ref().unwrap().notice.as_ref().map(|n| n.0.as_str()), Some("Vous avez le clavier et la souris."));
        assert!(app.session.as_ref().unwrap().keyboard_mouse());
        {
            let s = app.session.as_mut().unwrap();
            s.keys = true;
            s.remote_mouse = true;
            s.pad_mouse = true;
        }
        app.share_role_changed(ShareRole::Player, Some(ShareRole::PlayerKbm));
        let s = app.session.as_ref().unwrap();
        assert_eq!(s.notice.as_ref().map(|n| n.0.as_str()), Some("Le clavier et la souris sont repris."));
        assert!(!s.keys && !s.remote_mouse && !s.pad_mouse);
        app.share_role_changed(ShareRole::Spectator, Some(ShareRole::Player));
        assert_eq!(app.session.as_ref().unwrap().notice.as_ref().map(|n| n.0.as_str()), Some("Vous regardez de nouveau."));
        // Le rôle de départ n'a pas d'avis.
        assert_eq!(ShareRole::Spectator.notice(None), None);
        // Quitter le partage : « Se déconnecter ».
        assert_eq!(app.activate(Target::Menu(MenuItem::Disconnect)), vec![Action::Disconnect]);
    }

    /// Les mots du panneau : durée, app, pastille, code.
    #[test]
    fn share_words() {
        crate::i18n::force_english(false);
        assert_eq!(duration_label(20), "moins d'une minute");
        assert_eq!(duration_label(60), "1 minute");
        assert_eq!(duration_label(42 * 60 + 5), "42 minutes");
        assert_eq!(duration_label(2 * 3600 + 5 * 60), "2 h 05");
        assert_eq!((app_label("ios"), app_label("webos"), app_label("web"), app_label("tvos")), ("iPhone ou iPad".to_string(), "Téléviseur LG".to_string(), "Navigateur".to_string(), "Apple TV".to_string()));
        assert_eq!((guests_label(0), guests_label(1), guests_label(2)), ("0 invité".to_string(), "1 invité".to_string(), "2 invités".to_string()));
        assert_eq!(format_code("482917"), "482 917");
        let g = ShareGuest { id: "g".into(), name: "Léa".into(), role: ShareRole::PlayerKbm, app: "ios".into(), since: Some(1_000) };
        assert_eq!(g.line(), "Léa · Joueur, clavier et souris");
        assert_eq!(g.meta(1_000 + 4 * 60), "iPhone ou iPad · depuis 4 minutes");
        assert_eq!(ShareGuest { since: None, ..g }.meta(5), "iPhone ou iPad");
        crate::i18n::force_english(true);
        assert_eq!((duration_label(20), guests_label(1), guests_label(3), app_label("webos")), ("less than a minute".to_string(), "1 guest".to_string(), "3 guests".to_string(), "LG TV".to_string()));
        crate::i18n::force_english(false);
    }

    /// L'accueil propose « Rejoindre avec un code » à côté de « Ajouter un ordinateur », sans perdre d'ordinateur ;
    /// « Partager » et l'invité se dessinent : le panneau tient à l'écran avec quatre invités, la barre d'un invité
    /// n'a ni profil, ni réglages, ni « Partager », et la manette et le clavier ne figurent qu'avec leur rôle.
    #[test]
    fn share_screens_draw() {
        let mut app = tv();
        let mut cv = Canvas::new();
        let assets = Assets::load();
        app.go(Screen::Home);
        let hits = ui::draw(&app, &mut cv, &assets);
        let add = hits.iter().find(|(t, _)| *t == Target::AddCard).expect("ajouter").1;
        let join = hits.iter().find(|(t, _)| *t == Target::JoinCard).expect("rejoindre").1;
        assert!(join.x >= add.x + add.w && (join.y - add.y).abs() < 1.0 && join.x + join.w <= 1920.0, "côte à côte, dans l'écran");
        assert_eq!(hits.iter().filter(|(t, _)| matches!(t, Target::Host(_))).count(), 2);
        // Rejoindre : les champs, le pavé de chaque champ, tout à l'écran.
        app.join = JoinState { name: "LG OLED83G2".into(), message: Some("Aucun partage avec ce code sur le réseau.".into()), ..JoinState::new() };
        app.go(Screen::Join);
        let hits = ui::draw(&app, &mut cv, &assets);
        for t in [Target::JoinCode, Target::JoinName, Target::JoinGo, Target::Key('0'), Target::KeyErase] {
            assert!(hits.iter().any(|(h, _)| *h == t), "{t:?}");
        }
        assert!(hits.iter().all(|(t, _)| *t != Target::Key('q')));
        app.join.field = JoinField::Name;
        let hits = ui::draw(&app, &mut cv, &assets);
        for t in [Target::Key('q'), Target::Key('é'), Target::Key(' '), Target::KeyShift, Target::KeyErase] {
            assert!(hits.iter().any(|(h, _)| *h == t), "{t:?}");
        }
        for (t, r) in &hits {
            assert!(r.x >= 0.0 && r.y >= 0.0 && r.x + r.w <= 1920.0 && r.y + r.h <= 1080.0, "{t:?} sort de l'écran : {r:?}");
        }
        // Le panneau de l'hôte, avec quatre invités, tient à l'écran, boutons du bas compris.
        let mut view = shared_view();
        view.guests.push(ShareGuest { id: "g4".into(), name: "Zoé".into(), role: ShareRole::Spectator, app: "web".into(), since: None });
        let mut s = session(&app);
        s.menu = false;
        s.share = Some(view);
        s.share_panel = true;
        app.session = Some(s);
        app.screen = Screen::Session;
        assert!(ui::session_overlay_visible(&app));
        let hits = ui::draw(&app, &mut cv, &assets);
        for t in [Target::ShareNewCode, Target::ShareStop, Target::ShareKick(0), Target::ShareKick(3), Target::ShareRole(0), Target::ShareKbm(1), Target::ShareRole(1), Target::ShareKbm(2), Target::ShareRole(3)] {
            let (_, r) = hits.iter().find(|(h, _)| *h == t).unwrap_or_else(|| panic!("{t:?} absent du panneau"));
            assert!(r.x >= 0.0 && r.y >= 0.0 && r.x + r.w <= 1920.0 && r.y + r.h <= 1032.0, "{t:?} sort de l'écran : {r:?}");
        }
        assert!(hits.iter().all(|(t, _)| *t != Target::ShareKbm(0) && *t != Target::ShareKbm(3)), "un spectateur n'a que « Faire jouer »");
        // Le menu d'un hôte (trois rangées : « Partager » y est, le rappel du mode jeu ne les recouvre pas), puis d'un invité.
        let mut s = session(&app);
        s.menu = true;
        s.game_hint = true;
        app.session = Some(s);
        let hits = ui::draw(&app, &mut cv, &assets);
        let share = hits.iter().find(|(t, _)| *t == Target::Menu(MenuItem::Share)).expect("la tuile Partager").1;
        let dismiss = hits.iter().find(|(t, _)| *t == Target::HintDismiss).expect("le rappel").1;
        let last = hits.iter().filter(|(t, _)| matches!(t, Target::Menu(_))).map(|(_, r)| r.y + r.h).fold(0.0, f32::max);
        assert!(share.w > 0.0 && dismiss.y >= last, "le rappel est sous les tuiles ({dismiss:?}, {last})");
        for (role, full) in [(ShareRole::Spectator, false), (ShareRole::Player, false), (ShareRole::PlayerKbm, true)] {
            let mut g = guest_session(role);
            g.menu = true;
            app.session = Some(g);
            let hits = ui::draw(&app, &mut cv, &assets);
            let has = |t: Target| hits.iter().any(|(h, _)| *h == t);
            assert!(!has(Target::Menu(MenuItem::Share)) && !has(Target::Menu(MenuItem::Settings)) && !has(Target::Menu(MenuItem::Profile(Profile::Bureau))), "{role:?}");
            assert_eq!(has(Target::Menu(MenuItem::Keys)), full, "{role:?} : touches");
            assert_eq!(has(Target::Menu(MenuItem::RemoteMouse)), full, "{role:?} : Magic Remote en souris");
            assert_eq!(has(Target::Menu(MenuItem::PadMouse)), full, "{role:?} : manette en souris");
            assert_eq!(has(Target::Menu(MenuItem::Gamepads)), role != ShareRole::Spectator, "{role:?} : la manette seulement en joueur");
            assert!(has(Target::Menu(MenuItem::Resume)) && has(Target::Menu(MenuItem::Stats)) && has(Target::Menu(MenuItem::Disconnect)));
        }
    }
}
