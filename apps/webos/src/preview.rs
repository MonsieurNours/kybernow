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
    AddStep, App, Found, HostLive, JoinField, JoinState, MenuItem, NetKind, Online, PadInfo, Phase, Polled, Problem, Screen, Section, SessionUi, Setting, ShareAvail, ShareGuest, ShareQuality, ShareRole, ShareView, StationKnown, StationLive, StationOp,
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

/// Un partage ouvert : le code, le lien, quatre invités, un par combinaison de droits (spectateur, manettes, clavier
/// et souris, les deux).
fn shared_view() -> ShareView {
    let now = crate::model::now_secs() as i64;
    ShareView {
        code: "482917".into(),
        link: "kybernow://join?host=192.168.1.20&port=24811&fp=sha256:9f:3a:c2:71:0b:44:de:aa&code=482917".into(),
        web: "https://192.168.1.20:24811/link/?join=482917".into(),
        guests: vec![
            ShareGuest { id: "g1".into(), name: "Léa".into(), role: ShareRole::Spectator, app: "ios".into(), since: Some(now - 4 * 60), quality: None },
            ShareGuest { id: "g2".into(), name: "Tom".into(), role: ShareRole::Player, app: "android".into(), since: Some(now - 35 * 60), quality: Some("1080p · 60 Hz".into()) },
            ShareGuest { id: "g3".into(), name: "Hugo".into(), role: ShareRole::Kbm, app: "mac".into(), since: Some(now - 12 * 60), quality: None },
            ShareGuest { id: "g4".into(), name: "Camille".into(), role: ShareRole::PlayerKbm, app: "windows".into(), since: Some(now - 3700), quality: Some("720p · 30 Hz".into()) },
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
    // « Auto » demandé, 1080p à 60 Hz reçu : la tuile du menu montre la qualité reçue.
    if let Some(g) = s.guest.as_mut() {
        g.image = Some("1080p · 60 Hz".into());
    }
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
    // « Qualité de l'image » : un autre choix, le focus dessus.
    app.join.quality = ShareQuality::P720_30;
    app.focus = Target::JoinQuality(ShareQuality::P720_30);
    shot(app, cv, assets, None, dir, "20f-rejoindre-qualite");
    app.join.quality = ShareQuality::Auto;
    app.focus = Target::Key('1');
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
    // Station trop ancienne : la tuile est estompée mais se choisit ; le panneau qu'elle ouvre dit pourquoi.
    app.live.get_mut("a").unwrap().share = Some(ShareAvail::Unsupported);
    app.session = Some(session(app));
    app.focus = Target::Menu(MenuItem::Share);
    shot(app, cv, assets, Some(video), dir, "21f-partage-inactif");
    let mut s = session(app);
    s.menu = false;
    s.share_panel = true;
    app.session = Some(s);
    shot(app, cv, assets, Some(video), dir, "21g-partage-raison");
    // Partage ouvert, personne encore : la tuile du menu dit le code ; avec un invité, le nombre d'invités.
    app.live.get_mut("a").unwrap().share = Some(ShareAvail::Available);
    let mut s = session(app);
    s.share = Some(ShareView { guests: Vec::new(), ..shared_view() });
    app.session = Some(s);
    app.focus = Target::Menu(MenuItem::Share);
    shot(app, cv, assets, Some(video), dir, "21h-partage-menu-code");

    // L'invité : la connexion, le menu pour chaque combinaison de droits (aucun, manettes, clavier et souris, les deux), l'avis, la fin.
    let mut g = guest_session(ShareRole::Spectator);
    g.phase = Phase::Connecting;
    g.streamed = false;
    app.session = Some(g);
    app.focus = Target::CancelConnect;
    shot(app, cv, assets, None, dir, "22-invite-connexion");
    for (role, name) in [
        (ShareRole::Spectator, "22b-invite-spectateur"),
        (ShareRole::Player, "22c-invite-joueur"),
        (ShareRole::Kbm, "22d-invite-clavier-souris"),
        (ShareRole::PlayerKbm, "22d-invite-joueur-clavier"),
    ] {
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
    // « Qualité de l'image » : la tuile du menu (la qualité reçue), puis ses cinq choix, puis l'avis de la Station.
    let mut g = guest_session(ShareRole::Spectator);
    g.menu = false;
    g.share_panel = true;
    app.session = Some(g);
    app.focus = Target::QualityPick(ShareQuality::Auto);
    shot(app, cv, assets, Some(video), dir, "22g-invite-qualite");
    let mut g = guest_session(ShareRole::Spectator);
    if let Some(u) = g.guest.as_mut() {
        u.quality = ShareQuality::P720_30;
        u.image = Some("720p · 30 Hz".into());
    }
    g.menu = true;
    g.notice = Some((crate::i18n::tr("Image allégée pour suivre.", "Picture lightened to keep up.").to_string(), Instant::now()));
    app.session = Some(g);
    app.focus = Target::Menu(MenuItem::Quality);
    shot(app, cv, assets, Some(video), dir, "22h-invite-image-allegee");
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
        app_label, duration_label, format_code, guests_label, share_tile, Action, Nav, ShareMove, ShareOp, NAME_MAX,
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
        assert_eq!(acts, vec![Action::Save, Action::JoinShare { code: "482917".into(), name: "Ém".into(), quality: ShareQuality::Auto }]);
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

    /// Une Station qui ne partage pas : la tuile « Partager » se choisit et ouvre le panneau réduit à la raison ; avant la
    /// première réponse d'Admin, le panneau attend cette réponse (jamais une tuile muette) ; avec un partage possible, il
    /// ouvre le panneau et demande l'ouverture à Admin, une fois ; ses invités : les manettes et le clavier et la souris, donnés
    /// ou repris chacun sans toucher à l'autre, « Retirer » ; B referme le panneau, le signe de vie le met à jour.
    #[test]
    fn sharing_from_the_menu() {
        let mut app = tv();
        let mut s = SessionUi::new("a", "Atelier", Profile::Jeu, false);
        s.phase = Phase::Streaming;
        s.menu = true;
        app.session = Some(s);
        app.screen = Screen::Session;
        // Sans réponse d'Admin : le panneau s'ouvre et attend, Admin est sondé de nouveau.
        app.live.get_mut("a").unwrap().share = None;
        assert_eq!(app.activate(Target::Menu(MenuItem::Share)), vec![Action::ReleaseAll, Action::Poll("a".into())]);
        let s = app.session.as_ref().unwrap();
        assert!(s.share_wait && s.share_busy && s.share_panel && !s.menu);
        // Ordinateur injoignable (rien de neuf) : la demande part quand même, son refus dira pourquoi.
        assert_eq!(app.share_known("a"), vec![Action::Share(ShareOp::Open)]);
        assert!(!app.session.as_ref().unwrap().share_wait && app.session.as_ref().unwrap().share_busy);
        app.share_reply(Err("Admin injoignable".into()));
        assert_eq!(app.session.as_ref().unwrap().share_error.as_deref(), Some("Admin injoignable"));
        assert!(app.share_known("a").is_empty(), "plus rien n'attend");
        // Le panneau refermé pendant l'attente n'ouvre rien.
        assert!(app.session_back().is_empty());
        app.session.as_mut().unwrap().menu = true;
        assert_eq!(app.activate(Target::Menu(MenuItem::Share)), vec![Action::ReleaseAll, Action::Poll("a".into())]);
        assert!(app.session_back().is_empty());
        assert!(app.share_known("a").is_empty() && !app.session.as_ref().unwrap().share_busy);
        // La réponse dit que le partage est impossible : le panneau se réduit à la raison, rien ne part.
        app.session.as_mut().unwrap().menu = true;
        assert_eq!(app.activate(Target::Menu(MenuItem::Share)), vec![Action::ReleaseAll, Action::Poll("a".into())]);
        app.live.get_mut("a").unwrap().share = Some(ShareAvail::Unsupported);
        assert!(app.share_known("a").is_empty());
        let s = app.session.as_ref().unwrap();
        assert!(s.share_panel && !s.share_busy && s.share.is_none());
        assert!(app.session_back().is_empty() && !app.session.as_ref().unwrap().share_panel);
        // Déjà su impossible : la tuile se choisit, le panneau s'ouvre réduit à la raison, sans rien demander.
        for a in [ShareAvail::Unsupported, ShareAvail::Disabled] {
            app.live.get_mut("a").unwrap().share = Some(a);
            app.session.as_mut().unwrap().menu = true;
            assert_eq!(app.activate(Target::Menu(MenuItem::Share)), vec![Action::ReleaseAll]);
            let s = app.session.as_ref().unwrap();
            assert!(s.share_panel && !s.menu && !s.share_busy && s.share.is_none() && s.notice.is_none());
            assert!(app.session_back().is_empty() && !app.session.as_ref().unwrap().share_panel);
        }
        app.live.get_mut("a").unwrap().share = Some(ShareAvail::Available);
        app.session.as_mut().unwrap().menu = true;
        assert_eq!(app.activate(Target::Menu(MenuItem::Share)), vec![Action::ReleaseAll, Action::Share(ShareOp::Open)]);
        let s = app.session.as_ref().unwrap();
        assert!(s.share_panel && !s.menu && s.share_busy);
        assert!(app.activate(Target::ShareNewCode).is_empty(), "une demande à la fois");
        // Un signe de vie parti avant l'ouverture ne la défait pas.
        app.share_guests(None);
        assert!(app.session.as_ref().unwrap().share_panel);
        let guest = |role| ShareGuest { id: "g1".into(), name: "Léa".into(), role, app: "ios".into(), since: Some(1), quality: None };
        let view = |role| ShareView { code: "482917".into(), link: "kybernow://join?x".into(), web: "https://h:1/link/?join=482917".into(), guests: vec![guest(role)] };
        app.share_reply(Ok(Some(view(ShareRole::Spectator))));
        assert_eq!(app.focus, Target::ShareNewCode);
        let role = |r| vec![Action::Share(ShareOp::Role("g1".into(), r))];
        // Deux droits indépendants, chacun donné ou repris sans toucher à l'autre. Une seule demande à la fois,
        // jusqu'à la réponse d'Admin : le rôle voulu se calcule d'après celui qui est montré, deux clics rapides
        // sur les deux droits s'annuleraient sinon (Spectator → Kbm écraserait Player).
        assert_eq!(app.activate(Target::SharePads(0)), role(ShareRole::Player));
        assert!(app.activate(Target::ShareKbm(0)).is_empty() && app.activate(Target::SharePads(0)).is_empty() && app.activate(Target::ShareKick(0)).is_empty());
        app.share_reply(Ok(Some(view(ShareRole::Player)))); // les manettes données
        let ask = |app: &mut App, t: Target, to: ShareRole| {
            assert_eq!(app.activate(t), role(to), "{t:?}");
            app.share_reply(Ok(Some(view(to))));
        };
        ask(&mut app, Target::ShareKbm(0), ShareRole::PlayerKbm); // puis le clavier et la souris, les manettes gardées
        ask(&mut app, Target::SharePads(0), ShareRole::Kbm); // les manettes reprises, le clavier et la souris gardés
        ask(&mut app, Target::SharePads(0), ShareRole::PlayerKbm); // rendues
        ask(&mut app, Target::ShareKbm(0), ShareRole::Player); // le clavier et la souris repris, les manettes gardées
        ask(&mut app, Target::SharePads(0), ShareRole::Spectator); // plus aucun droit
        ask(&mut app, Target::ShareKbm(0), ShareRole::Kbm); // le clavier et la souris seuls, sans passer par les manettes
        ask(&mut app, Target::ShareKbm(0), ShareRole::Spectator);
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
        // Un signe de vie parti avant l'arrêt ne rouvre rien (3 s) ; un plus tardif, si.
        app.share_guests(Some(view(ShareRole::Spectator)));
        assert!(app.session.as_ref().unwrap().share.is_none());
        app.session.as_mut().unwrap().share_closed = Some(Instant::now() - Duration::from_secs(4));
        app.share_guests(Some(view(ShareRole::Spectator)));
        assert!(app.session.as_ref().unwrap().share.is_some());
        // Le partage fermé par ailleurs (Admin) longtemps après son ouverture : il se ferme ; juste après, non.
        app.share_guests(None);
        assert!(app.session.as_ref().unwrap().share.is_some(), "ouvert il y a moins de 8 s");
        app.session.as_mut().unwrap().share_opened = Some(Instant::now() - Duration::from_secs(9));
        app.share_guests(None);
        assert!(app.session.as_ref().unwrap().share.is_none());
        app.share_reply(Ok(None));
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
        let notice = |app: &App| app.session.as_ref().unwrap().notice.as_ref().map(|n| n.0.clone());
        app.share_role_changed(ShareRole::Player, Some(ShareRole::Spectator));
        assert_eq!(notice(&app).as_deref(), Some("Vous pouvez jouer à la manette."));
        assert!(!app.session.as_ref().unwrap().keyboard_mouse());
        // Le clavier et la souris donnés par-dessus les manettes : un avis pour le droit donné seulement.
        app.share_role_changed(ShareRole::PlayerKbm, Some(ShareRole::Player));
        assert_eq!(notice(&app).as_deref(), Some("Vous avez le clavier et la souris."));
        assert!(app.session.as_ref().unwrap().keyboard_mouse());
        {
            let s = app.session.as_mut().unwrap();
            s.keys = true;
            s.remote_mouse = true;
            s.pad_mouse = true;
        }
        // Les manettes reprises : le clavier et la souris restent ouverts (touches, Magic Remote, manette en souris).
        app.share_role_changed(ShareRole::Kbm, Some(ShareRole::PlayerKbm));
        let s = app.session.as_ref().unwrap();
        assert_eq!(notice(&app).as_deref(), Some("Les manettes sont reprises."));
        assert!(s.keys && s.remote_mouse && s.pad_mouse && s.keyboard_mouse());
        // Les manettes rendues, puis le clavier et la souris repris : ce qu'ils ouvraient se ferme.
        app.share_role_changed(ShareRole::PlayerKbm, Some(ShareRole::Kbm));
        assert_eq!(notice(&app).as_deref(), Some("Vous pouvez jouer à la manette."));
        app.share_role_changed(ShareRole::Player, Some(ShareRole::PlayerKbm));
        let s = app.session.as_ref().unwrap();
        assert_eq!(notice(&app).as_deref(), Some("Le clavier et la souris sont repris."));
        assert!(!s.keys && !s.remote_mouse && !s.pad_mouse && !s.keyboard_mouse());
        // Plus aucun droit : « Vous regardez de nouveau. » à la place de l'avis de reprise.
        app.share_role_changed(ShareRole::Spectator, Some(ShareRole::Player));
        assert_eq!(notice(&app).as_deref(), Some("Vous regardez de nouveau."));
        // Les quatre rôles, de l'un à l'autre : un seul avis ; les deux droits changés d'un coup, le droit donné
        // avant le droit repris, les manettes avant le clavier et la souris ; plus aucun droit : « Vous regardez
        // de nouveau. » à la place de l'avis de reprise.
        use ShareRole::{Kbm, Player, PlayerKbm, Spectator};
        let said = |to: ShareRole, from: ShareRole| to.notice(Some(from));
        assert_eq!(said(Player, Spectator), Some("Vous pouvez jouer à la manette."));
        assert_eq!(said(Kbm, Spectator), Some("Vous avez le clavier et la souris."));
        assert_eq!(said(PlayerKbm, Spectator), Some("Vous pouvez jouer à la manette."), "deux droits donnés : les manettes d'abord");
        assert_eq!(said(PlayerKbm, Kbm), Some("Vous pouvez jouer à la manette."));
        assert_eq!(said(PlayerKbm, Player), Some("Vous avez le clavier et la souris."));
        assert_eq!(said(Kbm, Player), Some("Vous avez le clavier et la souris."), "un droit donné, un repris : le donné d'abord");
        assert_eq!(said(Player, Kbm), Some("Vous pouvez jouer à la manette."), "un droit donné, un repris : le donné d'abord");
        assert_eq!(said(Spectator, Player), Some("Vous regardez de nouveau."));
        assert_eq!(said(Spectator, Kbm), Some("Vous regardez de nouveau."));
        assert_eq!(said(Spectator, PlayerKbm), Some("Vous regardez de nouveau."), "deux droits repris : plus aucun, un seul avis");
        assert_eq!(said(Kbm, PlayerKbm), Some("Les manettes sont reprises."));
        assert_eq!(said(Player, PlayerKbm), Some("Le clavier et la souris sont repris."));
        // Le rôle de départ (avant la connexion) et un rôle inchangé n'ont pas d'avis.
        assert_eq!(Spectator.notice(None), None);
        for r in [Spectator, Player, Kbm, PlayerKbm] {
            assert_eq!(r.notice(Some(r)), None);
        }
        crate::i18n::force_english(true);
        assert_eq!(said(Player, Spectator), Some("You can play with a controller."));
        assert_eq!(said(Kbm, Spectator), Some("You have the keyboard and mouse."));
        assert_eq!(said(Spectator, Player), Some("You are watching again."));
        assert_eq!(said(Kbm, PlayerKbm), Some("The controllers were taken back."));
        assert_eq!(said(Player, PlayerKbm), Some("The keyboard and mouse were taken back."));
        crate::i18n::force_english(false);
        // Les deux droits : « Joueur » dès qu'il y en a un ; chacun donné ou repris sans toucher à l'autre.
        assert_eq!([Spectator, Player, Kbm, PlayerKbm].map(|r| (r.label(), r.pads(), r.kbm(), r.plays())), [("Spectateur", false, false, false), ("Joueur", true, false, true), ("Joueur", false, true, true), ("Joueur", true, true, true)]);
        for (pads, kbm) in [(false, false), (true, false), (false, true), (true, true)] {
            let r = ShareRole::of(pads, kbm);
            assert_eq!((r.pads(), r.kbm()), (pads, kbm));
            for on in [false, true] {
                assert_eq!((r.with_pads(on).pads(), r.with_pads(on).kbm()), (on, kbm), "{r:?} : les manettes, le clavier et la souris tels quels");
                assert_eq!((r.with_kbm(on).pads(), r.with_kbm(on).kbm()), (pads, on), "{r:?} : le clavier et la souris, les manettes telles quelles");
            }
        }
        // Quitter le partage : « Se déconnecter ».
        assert_eq!(app.activate(Target::Menu(MenuItem::Disconnect)), vec![Action::Disconnect]);
    }

    /// « Qualité de l'image » à « Rejoindre » : « Auto » est proposé, les cinq choix se prennent (croix, OK, Magic Remote), le
    /// choix est retenu avec le prénom (`kybernow.json`) et revient la fois suivante ; il part avec « Rejoindre » ; pendant la
    /// recherche, il ne change plus ; un nom inconnu du fichier se lit « Auto ». La croix y mène du prénom à « Rejoindre ».
    #[test]
    fn joining_a_share_with_a_picture_quality() {
        let mut app = tv();
        app.go(Screen::Home);
        app.activate(Target::JoinCard);
        assert_eq!(app.join.quality, ShareQuality::Auto, "« Auto » proposé");
        assert_eq!(ShareQuality::ALL.map(ShareQuality::label), ["Auto", "Comme l'hôte", "4K · 60 Hz", "1080p · 60 Hz", "720p · 30 Hz"]);
        crate::i18n::force_english(true);
        assert_eq!(ShareQuality::ALL.map(ShareQuality::label), ["Auto", "Same as host", "4K · 60 Hz", "1080p · 60 Hz", "720p · 30 Hz"]);
        crate::i18n::force_english(false);
        assert_eq!(ShareQuality::ALL.map(ShareQuality::as_str), ["auto", "host", "2160p60", "1080p60", "720p30"], "les noms de l'échange avec Admin, dans l'ordre des choix");
        for q in ShareQuality::ALL {
            assert!(app.activate(Target::JoinQuality(q)).is_empty());
            assert_eq!(app.join.quality, q);
            assert_eq!(ShareQuality::parse(q.as_str()), q, "le nom retenu se relit");
        }
        assert_eq!((ShareQuality::parse(""), ShareQuality::parse("4k240")), (ShareQuality::Auto, ShareQuality::Auto));
        // La croix : du prénom aux cinq choix, de l'un à l'autre, puis « Rejoindre ».
        let mut cv = Canvas::new();
        let assets = Assets::load();
        app.hits = ui::draw(&app, &mut cv, &assets);
        app.focus = Target::JoinName;
        app.nav(Nav::Down);
        assert!(matches!(app.focus, Target::JoinQuality(_)), "{:?}", app.focus);
        app.focus = Target::JoinQuality(ShareQuality::Auto);
        app.nav(Nav::Right);
        assert_eq!(app.focus, Target::JoinQuality(ShareQuality::Host));
        app.nav(Nav::Ok);
        assert_eq!(app.join.quality, ShareQuality::Host, "OK choisit");
        app.nav(Nav::Right);
        assert_eq!(app.focus, Target::JoinQuality(ShareQuality::P2160_60), "« 4K · 60 Hz » après « Comme l'hôte »");
        app.nav(Nav::Right);
        assert_eq!(app.focus, Target::JoinQuality(ShareQuality::P1080_60));
        app.nav(Nav::Left);
        app.nav(Nav::Ok);
        assert_eq!(app.join.quality, ShareQuality::P2160_60);
        app.nav(Nav::Down);
        assert_eq!(app.focus, Target::JoinGo);
        // « 4K · 60 Hz » part avec le code et le prénom, et il est retenu (`2160p60`) pour la fois suivante.
        for c in "482917".chars() {
            app.nav(Nav::Char(c));
        }
        let acts = app.activate(Target::JoinGo);
        assert_eq!(acts, vec![Action::Save, Action::JoinShare { code: "482917".into(), name: "LG OLED83G2".into(), quality: ShareQuality::P2160_60 }]);
        assert_eq!(app.store.app.share_quality, "2160p60");
        app.nav(Nav::Back);
        app.activate(Target::JoinCard);
        assert_eq!(app.join.quality, ShareQuality::P2160_60, "4K retenu pour la fois suivante");
        app.nav(Nav::Back);
        app.activate(Target::JoinCard);
        // Le choix part avec le code et le prénom, et il est retenu.
        app.activate(Target::JoinQuality(ShareQuality::P720_30));
        for c in "482917".chars() {
            app.nav(Nav::Char(c));
        }
        let acts = app.activate(Target::JoinGo);
        assert_eq!(acts, vec![Action::Save, Action::JoinShare { code: "482917".into(), name: "LG OLED83G2".into(), quality: ShareQuality::P720_30 }]);
        assert_eq!(app.store.app.share_quality, "720p30");
        app.activate(Target::JoinQuality(ShareQuality::Host));
        assert_eq!(app.join.quality, ShareQuality::P720_30, "pendant la recherche, le choix ne bouge plus");
        // La fois suivante : le choix retenu ; un fichier à la main avec un nom inconnu : « Auto ».
        app.nav(Nav::Back);
        app.activate(Target::JoinCard);
        assert_eq!(app.join.quality, ShareQuality::P720_30);
        app.nav(Nav::Back);
        app.store.app.share_quality = "4k240".into();
        app.activate(Target::JoinCard);
        assert_eq!(app.join.quality, ShareQuality::Auto);
    }

    /// La qualité de l'image en session : la tuile montre la qualité reçue (le choix tant que Kyber ne la donne pas) ; elle
    /// ouvre les cinq choix sur l'image, Retour revient au menu ; un choix part à la Station, est retenu, revient à
    /// l'image ; le même choix ne demande rien ; l'avis de la Station s'affiche en capsule ; l'hôte n'a pas ce choix.
    #[test]
    fn picture_quality_in_a_guest_session() {
        let mut app = tv();
        app.session = Some(SessionUi::guest("share:g1", "Atelier", ShareRole::Spectator));
        app.session.as_mut().unwrap().phase = Phase::Streaming;
        app.screen = Screen::Session;
        let shown = |app: &App| app.session.as_ref().unwrap().guest.as_ref().unwrap().shown().to_string();
        assert_eq!(shown(&app), "Auto", "la qualité reçue n'est pas connue : le choix");
        app.share_image(Some("1080p · 60 Hz".into()), None);
        assert_eq!(shown(&app), "1080p · 60 Hz");
        assert!(app.session.as_ref().unwrap().notice.is_none());
        // La tuile ouvre le panneau, le focus sur le choix en place ; Retour revient au menu.
        app.session.as_mut().unwrap().menu = true;
        assert_eq!(app.activate(Target::Menu(MenuItem::Quality)), vec![Action::ReleaseAll]);
        let s = app.session.as_ref().unwrap();
        assert!(s.share_panel && !s.menu);
        assert_eq!(app.focus, Target::QualityPick(ShareQuality::Auto));
        assert!(app.session_back().is_empty());
        let s = app.session.as_ref().unwrap();
        assert!(!s.share_panel && s.menu, "Retour revient au menu");
        assert_eq!(app.focus, Target::Menu(MenuItem::Quality));
        // Un choix : il part à la Station, il est retenu, l'image reprend.
        app.activate(Target::Menu(MenuItem::Quality));
        let acts = app.activate(Target::QualityPick(ShareQuality::P720_30));
        assert_eq!(acts, vec![Action::ShareQuality(ShareQuality::P720_30), Action::Save]);
        let s = app.session.as_ref().unwrap();
        assert!(!s.share_panel && !s.menu, "le choix revient à l'image");
        assert_eq!((s.guest.as_ref().unwrap().quality, app.store.app.share_quality.as_str()), (ShareQuality::P720_30, "720p30"));
        // La Station répond : la qualité reçue et l'avis bref ; puis Kyber ne sait plus la taille : le choix se montre.
        app.share_image(Some("720p · 30 Hz".into()), Some("Picture lightened to keep up.".into()));
        assert_eq!(shown(&app), "720p · 30 Hz");
        assert_eq!(app.session.as_ref().unwrap().notice.as_ref().map(|n| n.0.as_str()), Some("Picture lightened to keep up."));
        // « 4K · 60 Hz » en cours de session : il part à la Station, est retenu, et la tuile dit la qualité reçue.
        app.session.as_mut().unwrap().menu = true;
        app.activate(Target::Menu(MenuItem::Quality));
        let acts = app.activate(Target::QualityPick(ShareQuality::P2160_60));
        assert_eq!(acts, vec![Action::ShareQuality(ShareQuality::P2160_60), Action::Save]);
        assert_eq!(app.store.app.share_quality, "2160p60");
        app.share_image(Some("4K · 60 Hz".into()), None);
        assert_eq!(shown(&app), "4K · 60 Hz");
        app.session.as_mut().unwrap().menu = true;
        app.activate(Target::Menu(MenuItem::Quality));
        app.activate(Target::QualityPick(ShareQuality::P720_30));
        app.share_image(Some("720p · 30 Hz".into()), None);
        app.share_image(None, None);
        assert!(app.session.as_ref().unwrap().guest.as_ref().unwrap().image.is_none());
        assert_eq!(shown(&app), "720p · 30 Hz");
        app.session.as_mut().unwrap().menu = true;
        app.activate(Target::Menu(MenuItem::Quality));
        assert!(app.activate(Target::QualityPick(ShareQuality::P720_30)).is_empty(), "le même choix : rien à demander");
        // Une session d'hôte n'a pas ce choix.
        let mut host = session(&app);
        host.menu = true;
        app.session = Some(host);
        assert!(app.activate(Target::Menu(MenuItem::Quality)).is_empty() && !app.session.as_ref().unwrap().share_panel);
        assert!(app.activate(Target::QualityPick(ShareQuality::Host)).is_empty());
        app.share_image(Some("1080p · 60 Hz".into()), Some("x".into()));
        assert!(app.session.as_ref().unwrap().notice.is_none(), "rien pour un hôte");
    }

    /// Le menu d'un invité a la tuile « Qualité de l'image » (sa qualité reçue en sous-titre), pour chaque rôle, dans l'écran ;
    /// son panneau montre les cinq choix, rien d'autre ; l'hôte n'a ni l'une ni l'autre.
    #[test]
    fn picture_quality_draws() {
        let mut app = tv();
        let mut cv = Canvas::new();
        let assets = Assets::load();
        let mut host = session(&app);
        host.menu = true;
        app.session = Some(host);
        app.screen = Screen::Session;
        assert!(ui::draw(&app, &mut cv, &assets).iter().all(|(t, _)| *t != Target::Menu(MenuItem::Quality)), "pas pour l'hôte");
        for role in [ShareRole::Spectator, ShareRole::Player, ShareRole::Kbm, ShareRole::PlayerKbm] {
            let mut g = guest_session(role);
            g.menu = true;
            app.session = Some(g);
            let hits = ui::draw(&app, &mut cv, &assets);
            let tile = hits.iter().find(|(t, _)| *t == Target::Menu(MenuItem::Quality)).unwrap_or_else(|| panic!("{role:?} : la tuile")).1;
            assert!(tile.x >= 0.0 && tile.x + tile.w <= 1920.0 && tile.y + tile.h <= 1080.0, "{role:?} : {tile:?}");
            app.activate(Target::Menu(MenuItem::Quality));
            let hits = ui::draw(&app, &mut cv, &assets);
            let picks: Vec<_> = hits.iter().filter(|(t, _)| matches!(t, Target::QualityPick(_))).collect();
            assert_eq!(picks.len(), ShareQuality::ALL.len(), "{role:?}");
            for (t, r) in &picks {
                assert!(r.x >= 0.0 && r.y >= 0.0 && r.x + r.w <= 1920.0 && r.y + r.h <= 1032.0, "{role:?} : {t:?} sort : {r:?}");
            }
            assert!(hits.iter().all(|(t, _)| !matches!(t, Target::Menu(_) | Target::ShareNewCode)), "{role:?} : rien d'autre que les choix");
        }
    }

    /// Les mots du panneau : durée, app, pastille, code.
    #[test]
    fn share_words() {
        crate::i18n::force_english(false);
        assert_eq!(duration_label(0), "moins d'une minute");
        assert_eq!(duration_label(59), "moins d'une minute");
        assert_eq!(duration_label(60), "1 min");
        assert_eq!(duration_label(3 * 60 + 40), "3 min");
        assert_eq!(duration_label(59 * 60 + 59), "59 min");
        assert_eq!(duration_label(3600), "1 h 00");
        assert_eq!(duration_label(3600 + 12 * 60), "1 h 12");
        assert_eq!(duration_label(2 * 3600 + 5 * 60), "2 h 05");
        assert_eq!((app_label("ios"), app_label("webos"), app_label("web"), app_label("tvos")), ("iPhone ou iPad".to_string(), "Téléviseur LG".to_string(), "Navigateur".to_string(), "Apple TV".to_string()));
        assert_eq!((guests_label(0), guests_label(1), guests_label(2)), ("0 invité".to_string(), "1 invité".to_string(), "2 invités".to_string()));
        assert_eq!(format_code("482917"), "482 917");
        // Le sous-titre de la tuile « Partager » : les invités, sinon le code, sinon « Code et QR code ».
        let view = |n: usize| ShareView { code: "482917".into(), guests: (0..n).map(|i| ShareGuest { id: format!("g{i}"), name: "Léa".into(), ..Default::default() }).collect(), ..Default::default() };
        assert_eq!(share_tile(None), ("Code et QR code".to_string(), false));
        assert_eq!(share_tile(Some(&view(0))), ("482 917".to_string(), true));
        assert_eq!(share_tile(Some(&view(1))), ("1 invité".to_string(), true));
        assert_eq!(share_tile(Some(&view(3))), ("3 invités".to_string(), true));
        let g = ShareGuest { id: "g".into(), name: "Léa".into(), role: ShareRole::PlayerKbm, app: "ios".into(), since: Some(1_000), quality: None };
        assert_eq!(g.line(), "Léa · Joueur, clavier et souris");
        assert_eq!(g.meta(1_000 + 4 * 60), "iPhone ou iPad · depuis 4 min");
        assert_eq!(g.meta(1_000 + 20), "iPhone ou iPad · depuis moins d'une minute");
        // Une horloge du téléviseur en retard sur celle de la Station : jamais de durée négative.
        assert_eq!(g.meta(1_000 - 90), "iPhone ou iPad · depuis moins d'une minute");
        assert_eq!(ShareGuest { since: None, ..g.clone() }.meta(5), "iPhone ou iPad");
        assert_eq!(ShareGuest { since: Some(0), ..g }.meta(5), "iPhone ou iPad");
        crate::i18n::force_english(true);
        assert_eq!(
            (duration_label(20), duration_label(3 * 60), duration_label(3600 + 12 * 60), guests_label(1), guests_label(3), app_label("webos")),
            ("under a minute".to_string(), "3 min".to_string(), "1 h 12".to_string(), "1 guest".to_string(), "3 guests".to_string(), "LG TV".to_string())
        );
        assert_eq!(
            [ShareRole::Spectator, ShareRole::Player, ShareRole::Kbm, ShareRole::PlayerKbm].map(|r| r.long_label()),
            ["Spectator", "Player", "Keyboard and mouse", "Player, keyboard and mouse"]
        );
        // Actions sur un invité : deux droits indépendants, chacun donné ou repris, jamais « Faire jouer » ni « Faire regarder ».
        let labels = |r: ShareRole| ShareMove::ALL.map(|m| m.label(r));
        assert_eq!(labels(ShareRole::Spectator), ["Give controllers", "Give keyboard and mouse"]);
        assert_eq!(labels(ShareRole::Player), ["Take back controllers", "Give keyboard and mouse"]);
        assert_eq!(labels(ShareRole::Kbm), ["Give controllers", "Take back keyboard and mouse"]);
        assert_eq!(labels(ShareRole::PlayerKbm), ["Take back controllers", "Take back keyboard and mouse"]);
        crate::i18n::force_english(false);
        assert_eq!(labels(ShareRole::Spectator), ["Donner les manettes", "Donner le clavier et la souris"]);
        assert_eq!(labels(ShareRole::Player), ["Reprendre les manettes", "Donner le clavier et la souris"]);
        assert_eq!(labels(ShareRole::Kbm), ["Donner les manettes", "Reprendre le clavier et la souris"]);
        assert_eq!(labels(ShareRole::PlayerKbm), ["Reprendre les manettes", "Reprendre le clavier et la souris"]);
        // Les libellés d'un invité : « Léa · Spectateur », « · Joueur », « · Clavier et souris », « · Joueur, clavier et souris ».
        let lines = [ShareRole::Spectator, ShareRole::Player, ShareRole::Kbm, ShareRole::PlayerKbm].map(|role| ShareGuest { name: "Léa".into(), role, ..Default::default() }.line());
        assert_eq!(lines, ["Léa · Spectateur", "Léa · Joueur", "Léa · Clavier et souris", "Léa · Joueur, clavier et souris"]);
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
        // « Qualité de l'image » : les cinq choix en rangée sous le prénom, entre lui et « Rejoindre », sans passer sous le pavé.
        let quality: Vec<crate::app::Rect> = ShareQuality::ALL.iter().map(|q| hits.iter().find(|(h, _)| *h == Target::JoinQuality(*q)).unwrap_or_else(|| panic!("{q:?}")).1).collect();
        assert!(quality.windows(2).all(|w| w[0].x + w[0].w <= w[1].x && (w[0].y - w[1].y).abs() < 0.5), "une ligne, dans l'ordre");
        let (name, go) = (hits.iter().find(|(t, _)| *t == Target::JoinName).unwrap().1, hits.iter().find(|(t, _)| *t == Target::JoinGo).unwrap().1);
        assert!(name.y + name.h <= quality[0].y && quality[0].y + quality[0].h <= go.y, "sous le prénom, au-dessus de « Rejoindre »");
        let pad_x = hits.iter().filter(|(t, _)| matches!(t, Target::Key(_))).map(|(_, r)| r.x).fold(f32::MAX, f32::min);
        assert!(quality.last().unwrap().x + quality.last().unwrap().w < pad_x, "les choix ne passent pas sous le pavé");
        app.join.field = JoinField::Name;
        let hits = ui::draw(&app, &mut cv, &assets);
        for t in [Target::Key('q'), Target::Key('é'), Target::Key(' '), Target::KeyShift, Target::KeyErase] {
            assert!(hits.iter().any(|(h, _)| *h == t), "{t:?}");
        }
        for (t, r) in &hits {
            assert!(r.x >= 0.0 && r.y >= 0.0 && r.x + r.w <= 1920.0 && r.y + r.h <= 1080.0, "{t:?} sort de l'écran : {r:?}");
        }
        // Le panneau de l'hôte, avec quatre invités (un par combinaison de droits), tient à l'écran, boutons du bas
        // compris ; chaque invité a ses deux boutons de droits et « Retirer », côte à côte, dans la largeur du panneau.
        let view = shared_view();
        assert_eq!(view.guests.len(), 4);
        let mut s = session(&app);
        s.menu = false;
        s.share = Some(view);
        s.share_panel = true;
        app.session = Some(s);
        app.screen = Screen::Session;
        assert!(ui::session_overlay_visible(&app));
        let hits = ui::draw(&app, &mut cv, &assets);
        let rect = |t: Target| hits.iter().find(|(h, _)| *h == t).unwrap_or_else(|| panic!("{t:?} absent du panneau")).1;
        let mut need = vec![Target::ShareNewCode, Target::ShareStop];
        for i in 0..4 {
            need.extend([Target::SharePads(i), Target::ShareKbm(i), Target::ShareKick(i)]);
        }
        for t in need {
            let r = rect(t);
            assert!(r.x >= 0.0 && r.y >= 0.0 && r.x + r.w <= 1920.0 && r.y + r.h <= 1032.0, "{t:?} sort de l'écran : {r:?}");
        }
        let right = rect(Target::ShareStop);
        for i in 0..4 {
            let (p, k, x) = (rect(Target::SharePads(i)), rect(Target::ShareKbm(i)), rect(Target::ShareKick(i)));
            assert!(p.x + p.w <= k.x && k.x + k.w <= x.x, "ligne {i} : boutons qui se chevauchent : {p:?} {k:?} {x:?}");
            assert!(x.x + x.w <= right.x + right.w, "ligne {i} : « Retirer » sort du panneau : {x:?} contre {right:?}");
        }
        let per_guest = hits.iter().filter(|(t, _)| matches!(t, Target::SharePads(_) | Target::ShareKbm(_) | Target::ShareKick(_))).count();
        assert_eq!(per_guest, 12, "trois boutons par invité, jamais « Faire jouer » ni « Faire regarder »");
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
        for role in [ShareRole::Spectator, ShareRole::Player, ShareRole::Kbm, ShareRole::PlayerKbm] {
            let mut g = guest_session(role);
            g.menu = true;
            app.session = Some(g);
            let hits = ui::draw(&app, &mut cv, &assets);
            let has = |t: Target| hits.iter().any(|(h, _)| *h == t);
            assert!(!has(Target::Menu(MenuItem::Share)) && !has(Target::Menu(MenuItem::Settings)) && !has(Target::Menu(MenuItem::Profile(Profile::Bureau))), "{role:?}");
            assert_eq!(has(Target::Menu(MenuItem::Keys)), role.kbm(), "{role:?} : touches");
            assert_eq!(has(Target::Menu(MenuItem::RemoteMouse)), role.kbm(), "{role:?} : Magic Remote en souris");
            assert_eq!(has(Target::Menu(MenuItem::PadMouse)), role.kbm(), "{role:?} : manette en souris");
            assert_eq!(has(Target::Menu(MenuItem::Gamepads)), role.pads(), "{role:?} : les manettes seulement avec les manettes");
            assert!(has(Target::Menu(MenuItem::Resume)) && has(Target::Menu(MenuItem::Stats)) && has(Target::Menu(MenuItem::Disconnect)));
        }
    }

    /// Une rangée de l'accueil avec `n` ordinateurs (les deux de la démonstration et `n - 2` autres), le focus sur le premier.
    fn home_with(n: usize) -> App {
        let mut app = tv();
        for i in 0..n.saturating_sub(2) {
            let id = format!("x{i}");
            app.store.hosts.push(host(&id, &format!("Ordinateur {i}"), "192.168.1.90"));
            app.live.insert(id, HostLive { online: Online::Online, ..Default::default() });
        }
        app.go(Screen::Home);
        app.focus = Target::Host(0);
        app
    }

    fn rect_of(app: &App, t: Target) -> crate::app::Rect {
        app.hits.iter().find(|(h, _)| *h == t).map(|(_, r)| *r).unwrap_or_else(|| panic!("{t:?} absent"))
    }

    /// Avec beaucoup d'ordinateurs, la rangée de l'accueil glisse : tous restent atteignables à la croix (puis la carte du
    /// réseau, « Ajouter » et « Rejoindre »), la carte qui a le focus est toujours entière à l'écran, et un retour vers la
    /// gauche ramène la rangée. Avec peu d'ordinateurs elle ne bouge pas.
    #[test]
    fn home_row_scrolls_to_reach_every_computer() {
        let mut cv = Canvas::new();
        let assets = Assets::load();
        // Peu d'ordinateurs : la rangée tient, « Rejoindre » est entier à l'écran sans que rien ne glisse.
        let mut app = home_with(2);
        app.hits = ui::draw(&app, &mut cv, &assets);
        assert_eq!(rect_of(&app, Target::Host(0)).x, 96.0);
        let join = rect_of(&app, Target::JoinCard);
        assert!(join.x + join.w <= 1920.0 - 96.0);
        // Six ordinateurs : la croix passe par chaque carte, dans l'ordre, et chacune est entière à l'écran.
        let n = 6;
        let mut app = home_with(n);
        app.hits = ui::draw(&app, &mut cv, &assets);
        assert_eq!(app.hits.iter().filter(|(t, _)| matches!(t, Target::Host(_))).count(), n, "tous les ordinateurs sont inscrits");
        assert_eq!(rect_of(&app, Target::Host(0)).x, 96.0, "la rangée commence à sa place");
        let cut = rect_of(&app, Target::Host(5));
        assert!(cut.x + cut.w > 1920.0 - 96.0 && cut.x < 1920.0, "le sixième est coupé au bord : {cut:?}");
        let mut order = vec![Target::Host(0)];
        while app.focus != Target::JoinCard && order.len() < n + 4 {
            app.nav(Nav::Right);
            app.hits = ui::draw(&app, &mut cv, &assets);
            order.push(app.focus);
            let r = rect_of(&app, app.focus);
            assert!(r.x >= 0.0 && r.x + r.w <= 1920.0, "{:?} sort de l'écran : {r:?}", app.focus);
        }
        let mut expected: Vec<Target> = (0..n).map(Target::Host).collect();
        expected.extend([Target::Found(0), Target::AddCard, Target::JoinCard]);
        assert_eq!(order, expected, "la croix passe par chaque carte, dans l'ordre");
        // Le retour, un pas à gauche : la carte d'avant, entière.
        app.nav(Nav::Left);
        app.hits = ui::draw(&app, &mut cv, &assets);
        assert_eq!(app.focus, Target::AddCard);
        let r = rect_of(&app, Target::AddCard);
        assert!(r.x >= 0.0 && r.x + r.w <= 1920.0);
        // La Magic Remote : la carte coupée au bord se choisit en la survolant, et vient en entier.
        app.focus = Target::Host(0);
        app.hits = ui::draw(&app, &mut cv, &assets);
        let cut = rect_of(&app, Target::Host(5));
        app.pointer_moved(cut.x + 20.0, cut.y + 20.0);
        assert_eq!(app.focus, Target::Host(5));
        app.hits = ui::draw(&app, &mut cv, &assets);
        let r = rect_of(&app, Target::Host(5));
        assert!(r.x + r.w <= 1920.0 - 96.0 + 0.5, "{r:?}");
    }

    /// Hors de la rangée (« Se connecter » a le focus), l'ordinateur choisi reste visible, même au bout de la rangée.
    #[test]
    fn home_row_keeps_the_selected_computer_visible() {
        let mut cv = Canvas::new();
        let assets = Assets::load();
        let mut app = tv();
        for i in 0..6 {
            app.store.hosts.push(host(&format!("x{i}"), &format!("Ordinateur {i}"), "192.168.1.90"));
        }
        app.store.selected = Some("x5".into());
        app.go(Screen::Home);
        let hits = ui::draw(&app, &mut cv, &assets);
        let r = hits.iter().find(|(t, _)| *t == Target::Host(7)).map(|(_, r)| *r).unwrap();
        assert!(r.x >= 0.0 && r.x + r.w <= 1920.0, "{r:?}");
    }

    /// « Partager » impossible : le panneau que la tuile ouvre est réduit à la raison (ni code, ni invités, ni bouton),
    /// et tient à l'écran.
    #[test]
    fn share_panel_reduced_to_the_reason() {
        let mut cv = Canvas::new();
        let assets = Assets::load();
        let mut app = tv();
        app.live.get_mut("a").unwrap().share = Some(ShareAvail::Unsupported);
        let mut s = session(&app);
        s.menu = true;
        app.session = Some(s);
        app.screen = Screen::Session;
        let hits = ui::draw(&app, &mut cv, &assets);
        assert!(hits.iter().any(|(t, _)| *t == Target::Menu(MenuItem::Share)), "la tuile se choisit");
        app.activate(Target::Menu(MenuItem::Share));
        assert!(app.session.as_ref().unwrap().share_panel);
        let hits = ui::draw(&app, &mut cv, &assets);
        assert!(hits.iter().all(|(t, _)| !matches!(t, Target::ShareNewCode | Target::ShareStop | Target::ShareKick(_) | Target::SharePads(_) | Target::ShareKbm(_))));
    }
}
