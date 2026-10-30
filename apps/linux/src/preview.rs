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

use crate::app::{AddStep, App, BarMenu, BorrowSessionUi, BorrowSource, BorrowUi, Confirm, Found, FriendForm, HostLive, InviteUi, JoinField, JoinState, Layout, LendAvail, LendCodeUi, LendHostState, LendHostView, LendLive, LendNoticeKind, LendRequestUi, LendSlotUi, LendState, LendSteam, LinkUi, MenuItem, NetKind, Online, PadInfo, PathKind, Phase, RouteUi, Screen, ScreensPlan, Section, SessionUi, Setting, ShareAvail, ShareGuest, ShareQuality, ShareRelay, ShareRole, ShareView, StationKind, StationLive, StationOp, StationStep, Stats, Target};
use crate::model::{Borrowed, Host, HostSettings, LastSession, Profile, Store, WakeCard};
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
        relay: None,
        settings: HostSettings::default(),
        last: None,
    }
}

fn demo(english: bool, layout: Layout) -> App {
    crate::i18n::set_english(english);
    let mut store = Store::default();
    let mut a = host("a", "Tour du salon", "192.168.1.42");
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    a.last = Some(LastSession { median: 6.8, p95: 9.4, seconds: 4320, mode: "1920 × 1200".into(), hz: 120, codec: "HEVC".into(), mbps: 38.0, at: now - 86400, screens: 1, route: String::new() });
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
    app.live.insert("a".into(), HostLive { online: Online::Online, virtual_display: Some(true), station: Some(station("0.5.0", None)), own_device: Some(true), max_screens: Some(4), share: Some(crate::app::ShareAvail::Available), lend: Some(crate::app::LendAvail::Available), lend_view: None });
    app.live.insert("b".into(), HostLive { online: Online::Offline, ..Default::default() });
    app.live.insert("c".into(), HostLive { online: Online::Online, virtual_display: Some(true), station: Some(station("0.5.0", None)), own_device: Some(true), max_screens: Some(4), share: Some(crate::app::ShareAvail::Available), lend: Some(crate::app::LendAvail::Available), lend_view: None });
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
        h.last = Some(LastSession { median: 8.9, p95: 14.2, seconds: 2520, mode: "3840 × 2160".into(), hz: 160, codec: "HEVC".into(), mbps: 120.0, at: 0, screens: 3, route: String::new() });
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

/// Une session d'hôte avec un partage ouvert : le code, le lien, quatre invités, un par combinaison de droits
/// (spectateur, manettes, clavier et souris, les deux).
fn shared_view() -> ShareView {
    let now = crate::model::now_secs() as i64;
    ShareView {
        code: "482917".into(),
        link: "kybernow://join?host=192.168.1.42&port=24811&fp=sha256:7c:3a:91:e0:4b:2d:a8:1f&code=482917".into(),
        web: "https://192.168.1.42:24811/link/?join=482917".into(),
        relay: None,
        relay_since: None,
        guests: vec![
            ShareGuest { id: "g1".into(), name: "Léa".into(), role: ShareRole::Spectator, app: "ios".into(), since: Some(now - 4 * 60), quality: None, ..Default::default() },
            ShareGuest { id: "g2".into(), name: "Tom".into(), role: ShareRole::Player, app: "webos".into(), since: Some(now - 35 * 60), quality: Some("1080p · 60 Hz".into()), ..Default::default() },
            ShareGuest { id: "g3".into(), name: "Hugo".into(), role: ShareRole::Kbm, app: "mac".into(), since: Some(now - 12 * 60), quality: None, ..Default::default() },
            ShareGuest { id: "g4".into(), name: "Camille".into(), role: ShareRole::PlayerKbm, app: "windows".into(), since: Some(now - 3700), quality: Some("720p · 30 Hz".into()), ..Default::default() },
        ],
    }
}

/// Une session d'invité à l'image, sur « Atelier ».
fn guest_session(role: ShareRole) -> SessionUi {
    let mut s = SessionUi::guest(1, "share:g1", "Atelier", role);
    s.phase = Phase::Streaming;
    s.stats = Some(Stats { latency_ms: Some(8.1), fps: 60.0, mbps: 24.0, lost: 0, mode: "1920 × 1080".into(), decoder: "VA-API HEVC".into(), audio_ms: Some(14.0), history: vec![8.0, 8.4, 7.9, 9.1, 8.2, 8.0, 8.8, 8.1] });
    s.started = Instant::now() - Duration::from_millis(2100);
    s.opened_at = Some(Instant::now() - Duration::from_secs(30));
    // « Auto » demandé, 1080p à 60 Hz reçu : la puce de la barre et la tuile du menu montrent la qualité reçue.
    if let Some(g) = s.guest.as_mut() {
        g.image = Some("1080p · 60 Hz".into());
    }
    s
}

/// KyberNow Share (docs/kybernow-share.md) : « Rejoindre un partage » (code, prénom, recherche, refus), le panneau
/// de l'hôte (ouverture, partage ouvert, « Partager » inactif), et la session d'invité (connexion, barre réduite,
/// rôles, fin).
fn share_shots(app: &mut App, cv: &mut Canvas, video: &Pixmap, dir: &Path, prefix: &str) {
    let name = |n: &str| format!("{prefix}-{n}");
    let console = app.layout.console();
    app.set_session(None);
    app.pointer = None;
    app.store.selected = Some("a".into());

    // Rejoindre : le code à taper, le prénom proposé ; le code en cours ; la recherche ; ce que le cœur a répondu ; le prénom.
    app.join = JoinState { name: "Atelier de Léa".into(), ..JoinState::new() };
    app.go(Screen::Join);
    shot(app, cv, None, dir, &name("20-rejoindre"));
    // « Qualité de l'image » : un autre choix, le focus dessus.
    app.join.quality = ShareQuality::P720_30;
    app.focus = Target::JoinQuality(ShareQuality::P720_30);
    shot(app, cv, None, dir, &name("20f-rejoindre-qualite"));
    app.join.quality = ShareQuality::Auto;
    app.focus = if console { Target::Key('1') } else { Target::JoinCode };
    app.join.code = "4829".into();
    shot(app, cv, None, dir, &name("20b-rejoindre-code"));
    app.join.code = "482917".into();
    app.join.busy = true;
    shot(app, cv, None, dir, &name("20c-rejoindre-recherche"));
    app.join.busy = false;
    app.join.message = Some(crate::i18n::tr("Aucun partage avec ce code sur le réseau.", "No share with this code on your network.").into());
    shot(app, cv, None, dir, &name("20d-rejoindre-refus"));
    app.join.message = None;
    app.join.field = JoinField::Name;
    app.go(Screen::Join);
    shot(app, cv, None, dir, &name("20e-rejoindre-prenom"));
    app.join = JoinState::new();

    // L'hôte : la barre avec « Partager » et la pastille, le panneau, l'attente, « Partager » inactif.
    let mut s = session(app);
    s.menu = false;
    s.relative = false;
    s.share = Some(shared_view());
    s.opened_at = Some(Instant::now() - Duration::from_secs(30));
    if !console {
        s.bar = true;
        app.set_session(Some(s));
        app.screen = Screen::Session;
        app.pointer = Some((700.0, 20.0));
        shot(app, cv, Some(video), dir, &name("21-partage-barre"));
        if let Some(s) = app.session_mut() {
            s.bar_menu = Some(BarMenu::Share);
            s.share_copied = Some((Instant::now(), true));
        }
        shot(app, cv, Some(video), dir, &name("21b-partage-panneau"));
        if let Some(s) = app.session_mut() {
            s.share_copied = Some((Instant::now(), false));
            s.share = Some(ShareView { guests: Vec::new(), ..shared_view() });
        }
        shot(app, cv, Some(video), dir, &name("21c-partage-panneau-vide"));
        if let Some(s) = app.session_mut() {
            s.share = None;
            s.share_busy = true;
        }
        shot(app, cv, Some(video), dir, &name("21d-partage-ouverture"));
        // Refusé par Admin.
        if let Some(s) = app.session_mut() {
            s.share_busy = false;
            s.share_error = Some(crate::i18n::tr("Une connexion est en cours sur cet ordinateur : partagez un écran une fois qu'elle est finie.", "A connection is in progress on this computer: share a screen once it has ended.").into());
        }
        shot(app, cv, Some(video), dir, &name("21e-partage-refuse"));
        // Station trop ancienne : le bouton est éteint, son nom dit pourquoi.
        if let Some(s) = app.session_mut() {
            s.bar_menu = None;
            s.share_error = None;
            s.share = None;
        }
        app.live.insert("a".into(), HostLive { share: Some(ShareAvail::Unsupported), ..app.live.get("a").cloned().unwrap_or_default() });
        let hits = ui::draw(app, cv);
        if let Some((_, r)) = hits.iter().find(|(t, _)| *t == Target::BarShare) {
            app.pointer = Some((r.x + r.w / 2.0, r.y + r.h / 2.0));
        }
        shot(app, cv, Some(video), dir, &name("21f-partage-inactif"));
        app.live.insert("a".into(), HostLive { share: Some(ShareAvail::Available), ..app.live.get("a").cloned().unwrap_or_default() });
    } else {
        s.share_panel = true;
        app.set_session(Some(s));
        app.screen = Screen::Session;
        app.focus = Target::ShareNewCode;
        shot(app, cv, Some(video), dir, &name("21-partage-panneau"));
        if let Some(s) = app.session_mut() {
            s.share = Some(ShareView { guests: Vec::new(), ..shared_view() });
        }
        shot(app, cv, Some(video), dir, &name("21c-partage-panneau-vide"));
        // Le menu, avec la tuile « Partager » et le partage ouvert.
        let mut s = session(app);
        s.share = Some(shared_view());
        app.set_session(Some(s));
        app.screen = Screen::Session;
        app.focus = Target::Menu(MenuItem::Share);
        shot(app, cv, Some(video), dir, &name("21g-partage-menu"));
        // Partage ouvert, personne encore : la tuile dit le code.
        let mut s = session(app);
        s.share = Some(ShareView { guests: Vec::new(), ..shared_view() });
        app.set_session(Some(s));
        shot(app, cv, Some(video), dir, &name("21h-partage-menu-code"));
        // Station trop ancienne : la tuile est estompée mais se choisit ; le panneau qu'elle ouvre dit pourquoi.
        app.live.insert("a".into(), HostLive { share: Some(ShareAvail::Unsupported), ..app.live.get("a").cloned().unwrap_or_default() });
        app.set_session(Some(session(app)));
        shot(app, cv, Some(video), dir, &name("21i-partage-inactif"));
        let mut s = session(app);
        s.menu = false;
        s.share_panel = true;
        app.set_session(Some(s));
        shot(app, cv, Some(video), dir, &name("21j-partage-raison"));
        app.live.insert("a".into(), HostLive { share: Some(ShareAvail::Available), ..app.live.get("a").cloned().unwrap_or_default() });
    }

    // L'invité : la connexion, la barre ou le menu d'un spectateur, d'un joueur, le rôle changé, la fin.
    app.pointer = None;
    let mut g = guest_session(ShareRole::Spectator);
    g.phase = Phase::Connecting;
    g.opened_at = None;
    app.set_session(Some(g));
    app.screen = Screen::Session;
    app.focus = Target::CancelConnect;
    shot(app, cv, None, dir, &name("22-invite-connexion"));
    let mut g = guest_session(ShareRole::Spectator);
    if console {
        g.menu = true;
    } else {
        g.bar = true;
        app.pointer = Some((700.0, 20.0));
    }
    app.set_session(Some(g));
    app.screen = Screen::Session;
    app.focus = Target::Menu(MenuItem::Resume);
    shot(app, cv, Some(video), dir, &name("22b-invite-spectateur"));
    // « Qualité de l'image » : au bureau le menu de la puce de la barre, à la manette la tuile du menu puis ses quatre
    // choix ; puis l'avis de la Station (image allégée).
    let mut g = guest_session(ShareRole::Spectator);
    if console {
        g.menu = false;
        g.share_panel = true;
        app.focus = Target::QualityPick(ShareQuality::Auto);
    } else {
        g.bar = true;
        g.bar_menu = Some(BarMenu::Quality);
        app.pointer = None;
    }
    app.set_session(Some(g));
    shot(app, cv, Some(video), dir, &name("22f-invite-qualite"));
    let mut g = guest_session(ShareRole::Spectator);
    if let Some(u) = g.guest.as_mut() {
        u.quality = ShareQuality::P720_30;
        u.image = Some("720p · 30 Hz".into());
    }
    g.notice = Some((crate::i18n::tr("Image allégée pour suivre.", "Picture lightened to keep up.").to_string(), Instant::now()));
    if console {
        g.menu = true;
    } else {
        g.bar = true;
    }
    app.set_session(Some(g));
    app.focus = Target::Menu(MenuItem::Quality);
    shot(app, cv, Some(video), dir, &name("22g-invite-image-allegee"));
    // Chaque droit donné par l'hôte, avec son avis : les manettes, le clavier et la souris, puis les deux.
    for (role, from, n) in [
        (ShareRole::Player, ShareRole::Spectator, "22c-invite-joueur"),
        (ShareRole::Kbm, ShareRole::Spectator, "22d-invite-clavier-souris"),
        (ShareRole::PlayerKbm, ShareRole::Player, "22d-invite-joueur-clavier"),
    ] {
        let mut g = guest_session(role);
        if console {
            g.menu = true;
        } else {
            g.bar = true;
        }
        g.notice = role.notice(Some(from)).map(|t| (t.to_string(), Instant::now()));
        app.set_session(Some(g));
        shot(app, cv, Some(video), dir, &name(n));
    }
    app.pointer = None;
    let mut g = guest_session(ShareRole::Player);
    g.phase = Phase::Ended { error: Some(crate::i18n::tr("Léa a arrêté le partage.", "Léa stopped sharing.").into()) };
    app.set_session(Some(g));
    app.focus = Target::Back;
    shot(app, cv, None, dir, &name("22e-invite-fin"));
    app.set_session(None);
    app.screen = Screen::Home;
}

/// Un partage ouvert par une Station dont le relais est joint : un invité sur le réseau, un autre venu par le relais, et
/// deux demandes qui attendent l'accord de l'hôte.
fn relay_view(relay: Option<ShareRelay>) -> ShareView {
    let now = crate::model::now_secs() as i64;
    let mut v = shared_view();
    v.guests.truncate(2);
    v.guests[1] = ShareGuest { id: "g2".into(), name: "Tom".into(), role: ShareRole::Spectator, app: "android".into(), since: Some(now - 3 * 60), quality: None, pending: false, via_relay: true };
    v.guests.push(ShareGuest { id: "g5".into(), name: "Eva".into(), role: ShareRole::Spectator, app: "ios".into(), since: Some(now - 20), quality: None, pending: true, via_relay: true });
    v.guests.push(ShareGuest { id: "g6".into(), name: "Hugo".into(), role: ShareRole::Spectator, app: "web".into(), since: Some(now - 5), quality: None, pending: true, via_relay: true });
    v.link = "kybernow://join?host=192.168.1.42&port=24811&fp=sha256:7c:3a:91:e0:4b:2d:a8:1f&code=482917&relay=share.kybernow.app&sid=ocean7xk3qdhvm2wpa5fzt4ylb".into();
    v.web = "https://share.kybernow.app/j/482917".into();
    v.relay = relay;
    v
}

/// Une session d'hôte par le relais (chemin, aller-retour mesuré).
fn relayed(kind: PathKind) -> RouteUi {
    RouteUi { kind, rtt_client: Some(18.0), rtt_station: Some(2.5) }
}

/// Le relais (docs/relais.md, textes de docs/link-ui.md, « Hors de chez vous » et « Partage ») : l'ordinateur joignable
/// seulement par le relais sur l'accueil, « Rejoindre » avec l'attente de l'accord de l'hôte et ce qu'il répond, le
/// panneau de l'hôte avec ses demandes, la session d'invité et d'hôte par le relais.
fn relay_shots(app: &mut App, cv: &mut Canvas, video: &Pixmap, dir: &Path, prefix: &str) {
    let name = |n: &str| format!("{prefix}-{n}");
    let console = app.layout.console();
    app.set_session(None);
    app.pointer = None;
    app.store.selected = Some("a".into());

    // L'accueil : « Hors de chez vous » à la place de « En ligne », le bouton reste « Se connecter ».
    app.live.insert("a".into(), HostLive { online: Online::Away, ..app.live.get("a").cloned().unwrap_or_default() });
    app.go(Screen::Home);
    shot(app, cv, None, dir, &name("01f-accueil-hors-de-chez-vous"));
    app.live.insert("a".into(), HostLive { online: Online::Online, ..app.live.get("a").cloned().unwrap_or_default() });

    // Rejoindre par le relais : l'attente de l'accord de l'hôte (« Annuler »), puis ce que l'hôte répond.
    app.join = JoinState { name: "Atelier de Léa".into(), code: "482917".into(), busy: true, waiting: true, ..JoinState::new() };
    app.go(Screen::Join);
    app.focus = Target::JoinCancel;
    shot(app, cv, None, dir, &name("20g-rejoindre-attente"));
    app.join.busy = false;
    app.join.waiting = false;
    app.focus = if console { Target::Key('1') } else { Target::JoinCode };
    for (n, fr, en) in [("20h-rejoindre-refuse", "L'hôte a refusé.", "The host declined."), ("20i-rejoindre-sans-reponse", "L'hôte n'a pas répondu.", "The host did not answer."), ("20j-rejoindre-relais-muet", "Le relais ne répond pas.", "The relay is not answering.")] {
        app.join.message = Some(crate::i18n::tr(fr, en).into());
        shot(app, cv, None, dir, &name(n));
    }
    app.join = JoinState::new();

    // Le panneau de l'hôte : le relais joint (lien et QR code du relais, aide, « hors de chez vous », demandes), puis injoignable.
    let mut s = session(app);
    s.menu = false;
    s.relative = false;
    s.share = Some(relay_view(Some(ShareRelay::Joined)));
    if console {
        s.share_panel = true;
    } else {
        s.bar = true;
        s.bar_menu = Some(BarMenu::Share);
        app.pointer = Some((700.0, 20.0));
    }
    app.set_session(Some(s));
    app.screen = Screen::Session;
    app.focus = Target::ShareAccept(2);
    shot(app, cv, Some(video), dir, &name("21k-partage-relais"));
    if let Some(s) = app.session_mut() {
        s.share = Some(ShareView { guests: relay_view(None).guests.into_iter().take(2).collect(), ..relay_view(Some(ShareRelay::Unreachable)) });
    }
    app.focus = Target::ShareNewCode;
    shot(app, cv, Some(video), dir, &name("21l-partage-relais-injoignable"));
    // Le lien du relais en préparation : le lien du réseau encore, « Copier le lien » inactif, le QR code attend.
    if let Some(s) = app.session_mut() {
        let lan = shared_view();
        s.share = Some(ShareView { link: lan.link, web: lan.web, relay_since: Some(Instant::now()), ..relay_view(Some(ShareRelay::Connecting)) });
        if let Some(v) = s.share.as_mut() {
            v.guests.truncate(1);
        }
    }
    shot(app, cv, Some(video), dir, &name("21o-partage-relais-preparation"));
    // Les quatre places prises : « Accepter » est inactif (« Quatre invités au plus. »), « Refuser » reste, le focus y commence.
    let mut complet = relay_view(Some(ShareRelay::Joined));
    complet.guests.truncate(1);
    complet.guests.extend((0..3).map(|i| ShareGuest { id: format!("f{i}"), name: ["Zoé", "Luc", "Nina"][i].into(), app: "mac".into(), ..Default::default() }));
    complet.guests.push(ShareGuest { id: "g5".into(), name: "Eva".into(), app: "ios".into(), pending: true, via_relay: true, ..Default::default() });
    app.focus = Target::ShareRefuse(complet.guests.len() - 1);
    if let Some(s) = app.session_mut() {
        s.share = Some(complet);
    }
    shot(app, cv, Some(video), dir, &name("21n-partage-relais-complet"));
    // La barre fermée sur le panneau : les deux pastilles, invités et demandes.
    if !console {
        if let Some(s) = app.session_mut() {
            s.bar_menu = None;
            s.share = Some(relay_view(Some(ShareRelay::Joined)));
        }
        shot(app, cv, Some(video), dir, &name("21m-partage-relais-barre"));
    }

    // Une session par le relais : la puce (bureau) ou la tuile (menu), les statistiques avec leur ligne « Chemin », le repli TCP.
    app.pointer = None;
    for (kind, n) in [(PathKind::Relay, "23-session-relais"), (PathKind::RelayTcp, "23b-session-relais-tcp")] {
        let mut s = session(app);
        s.route = Some(relayed(kind));
        s.show_stats = true;
        if console {
            s.menu = true;
        } else {
            s.menu = false;
            s.bar = true;
            app.pointer = Some((700.0, 20.0));
        }
        app.set_session(Some(s));
        app.screen = Screen::Session;
        app.focus = Target::Menu(MenuItem::Resume);
        shot(app, cv, Some(video), dir, &name(n));
    }
    // Un invité venu par le relais.
    let mut g = guest_session(ShareRole::Spectator);
    g.route = Some(relayed(PathKind::Relay));
    if console {
        g.menu = true;
    } else {
        g.bar = true;
    }
    app.set_session(Some(g));
    shot(app, cv, Some(video), dir, &name("23c-invite-relais"));
    app.pointer = None;
    app.set_session(None);
    app.screen = Screen::Home;
}

/// L'ordinateur de Matthieu, emprunté avec un compte d'ami (prêt de l'ordinateur).
fn borrowed() -> Borrowed {
    Borrowed { id: "lend:m".into(), host: "Matthieu".into(), computer: "PC-SALON".into(), address: "192.168.1.42".into(), port: 24811, fingerprint: Some("sha256:3e:91:0a:7c:55:d2:18:b4".into()), relay: None, sid: None, login: "lea".into(), password: "secret".into(), remember: true }
}

/// Un emprunt à l'image : l'ordinateur de Matthieu, jusqu'à dans 1 h 20.
fn borrow_session(source: BorrowSource) -> SessionUi {
    let until = crate::model::now_secs() + 80 * 60 + 30;
    let mut s = SessionUi::borrower(1, "lend:m", BorrowSessionUi::new("Matthieu", "PC-SALON", Some(until), source));
    s.phase = Phase::Streaming;
    s.stats = Some(Stats { latency_ms: Some(9.2), fps: 60.0, mbps: 28.0, lost: 0, mode: "1920 × 1080".into(), decoder: "VA-API HEVC".into(), audio_ms: Some(13.0), history: vec![9.0, 9.4, 8.9, 10.1, 9.2, 9.0, 9.8, 9.1] });
    s.started = Instant::now() - Duration::from_millis(2100);
    s.opened_at = Some(Instant::now() - Duration::from_secs(30));
    s
}

/// La vue du prêt chez l'hôte : un code de séance (2 heures, son compte Steam), et une demande s'il en faut une.
fn lend_view(code: bool, request: bool) -> LendHostView {
    LendHostView {
        enabled: true,
        code: code.then(|| LendCodeUi { code: "731402".into(), link: "kybernow://join?host=192.168.1.42&port=24811&fp=sha256:7c:3a:91:e0:4b:2d:a8:1f&code=731402".into(), web: "https://192.168.1.42:24811/link/?join=731402".into(), duration: Some(7200), steam: LendSteam::Friend, relay: None, relay_since: None }),
        requests: if request { vec![LendRequestUi { id: "r1".into(), name: "Léa".into(), app: "ios".into(), via_relay: true, ..Default::default() }] } else { Vec::new() },
        ..Default::default()
    }
}

/// Le prêt de l'ordinateur (docs/pret.md ; textes : docs/link-ui.md, « Prêt de l'ordinateur ») : chez l'ami, la carte
/// « Ordinateur de {hôte} » (libre, occupé avec un créneau), l'attente de l'accord, un refus, « Avec un compte d'ami »,
/// un code de prêt dans « Rejoindre », la session d'emprunt (connexion, barre ou menu, préavis, « Rendre
/// l'ordinateur », fins) ; chez l'hôte, le panneau « Prêter l'ordinateur… » (sans code, avec un code, une demande,
/// Station trop ancienne), et l'accueil de l'ordinateur prêté.
fn lend_shots(app: &mut App, cv: &mut Canvas, video: &Pixmap, dir: &Path, prefix: &str) {
    let name = |n: &str| format!("{prefix}-{n}");
    let console = app.layout.console();
    app.set_session(None);
    app.pointer = None;
    let now = crate::model::now_secs();
    let hour = now - now % 3600;

    // L'ami : la carte de l'ordinateur emprunté, libre pendant son créneau ; occupé, le prochain créneau ; l'attente ; un refus.
    app.store.borrowed = vec![borrowed()];
    app.store.selected = Some("lend:m".into());
    app.lend_live.insert("lend:m".into(), LendLive { state: Some(LendState::Free), slot: Some(LendSlotUi { now: true, start: hour - 3600, end: hour + 3 * 3600 }) });
    app.go(Screen::Home);
    shot(app, cv, None, dir, &name("24-pret-accueil"));
    app.lend_live.insert("lend:m".into(), LendLive { state: Some(LendState::Busy), slot: Some(LendSlotUi { now: false, start: hour + 2 * 86400, end: hour + 2 * 86400 + 4 * 3600 }) });
    shot(app, cv, None, dir, &name("24b-pret-accueil-occupe"));
    app.borrow = BorrowUi { id: "lend:m".into(), busy: true, waiting: true, ..BorrowUi::default() };
    app.focus = Target::BorrowCancel;
    shot(app, cv, None, dir, &name("24c-pret-attente"));
    app.borrow = BorrowUi { id: "lend:m".into(), message: Some(crate::i18n::tr("Matthieu a refusé.", "Matthieu declined.").into()), ..BorrowUi::default() };
    app.focus = Target::Borrow;
    shot(app, cv, None, dir, &name("24d-pret-refus"));
    app.borrow = BorrowUi::default();

    // « Ajouter un ordinateur » › « Avec un compte d'ami ».
    app.friend = FriendForm { login: "lea".into(), password: "secret".into(), ..FriendForm::default() };
    app.add.step = AddStep::Friend;
    app.go(Screen::Add);
    shot(app, cv, None, dir, &name("24e-pret-compte-ami"));
    // Un lien d'invitation avec le prénom de l'hôte : sa Station, l'identifiant rempli.
    app.add.step = AddStep::Code;
    app.go(Screen::Home);
    app.open_link(LinkUi::Invite(InviteUi { address: "192.168.1.42".into(), port: 24811, fingerprint: Some("sha256:3e:91:0a:7c".into()), login: "lea".into(), name: Some("Matthieu".into()), ..InviteUi::default() }));
    shot(app, cv, None, dir, &name("24h-pret-invitation"));
    app.add.step = AddStep::Code;
    app.friend = FriendForm::default();
    // Un lien d'invitation illisible : l'avis sur l'accueil.
    app.go(Screen::Home);
    app.open_link(LinkUi::BadInvite);
    shot(app, cv, None, dir, &name("24i-pret-lien-illisible"));
    app.toast = None;
    // Le mot de passe n'est pas retenu : « Emprunter » le redemande.
    app.store.borrowed[0].remember = false;
    app.store.borrowed[0].password.clear();
    app.lend_live.insert("lend:m".into(), LendLive { state: Some(LendState::Free), slot: None });
    app.go(Screen::Home);
    app.activate(Target::Borrow);
    shot(app, cv, None, dir, &name("24j-pret-mot-de-passe"));
    app.store.borrowed = vec![borrowed()];
    app.add.step = AddStep::Code;
    app.friend = FriendForm::default();
    // « Retirer cet ordinateur » : la confirmation.
    app.go(Screen::Home);
    app.confirm = Some(Confirm::Remove { host: "lend:m".into(), name: borrowed().title() });
    app.focus = Target::ConfirmNo;
    shot(app, cv, None, dir, &name("24k-pret-retirer"));
    app.confirm = None;

    // « Rejoindre avec un code » tombe sur un code de prêt : la préparation de l'ordinateur, puis l'attente de l'hôte.
    app.join = JoinState { name: "Léa".into(), code: "731402".into(), busy: true, lending: true, ..JoinState::new() };
    app.go(Screen::Join);
    shot(app, cv, None, dir, &name("24f-pret-code-preparation"));
    app.join.waiting = true;
    app.join.wait_host = Some("Matthieu".into());
    app.focus = Target::JoinCancel;
    shot(app, cv, None, dir, &name("24g-pret-code-attente"));
    app.join = JoinState::new();

    // La session d'emprunt : la connexion (« Préparation de l'ordinateur »), la barre ou le menu, le préavis, « Rendre
    // l'ordinateur », la fin dite par le prêt, la fin sans le prêt (« Revenir »).
    let source = BorrowSource::Account { id: "lend:m".into(), attempt: 1 };
    let mut s = borrow_session(source.clone());
    s.phase = Phase::Connecting;
    s.opened_at = None;
    app.set_session(Some(s));
    app.screen = Screen::Session;
    app.focus = Target::CancelConnect;
    shot(app, cv, None, dir, &name("25-emprunt-connexion"));
    let mut s = borrow_session(source.clone());
    if console {
        s.menu = true;
    } else {
        s.bar = true;
        app.pointer = Some((700.0, 20.0));
    }
    app.set_session(Some(s));
    app.focus = Target::Menu(MenuItem::Resume);
    shot(app, cv, Some(video), dir, &name("25b-emprunt-barre"));
    let mut s = borrow_session(source.clone());
    if let Some(b) = s.borrow.as_mut() {
        b.notice = Some(now + 110);
    }
    s.notice = Some((crate::i18n::tr("Matthieu reprend l'ordinateur dans 2 min : sauvegardez votre partie.", "Matthieu is taking back the computer in 2 min: save your game.").into(), Instant::now()));
    if !console {
        s.bar = true;
    }
    app.set_session(Some(s));
    shot(app, cv, Some(video), dir, &name("25c-emprunt-preavis"));
    app.pointer = None;
    app.set_session(Some(borrow_session(source.clone())));
    app.confirm = Some(Confirm::GiveBack { session: 1, host: "Matthieu".into() });
    app.focus = Target::ConfirmNo;
    shot(app, cv, Some(video), dir, &name("25d-emprunt-rendre"));
    app.confirm = None;
    let mut s = borrow_session(source.clone());
    s.phase = Phase::Ended { error: Some(crate::i18n::tr("Matthieu a repris l'ordinateur.", "Matthieu took back the computer.").into()) };
    if let Some(b) = s.borrow.as_mut() {
        b.ended_by_lend = true;
    }
    app.set_session(Some(s));
    app.focus = Target::Back;
    shot(app, cv, None, dir, &name("25e-emprunt-fin"));
    let mut s = borrow_session(source);
    s.phase = Phase::Ended { error: Some(crate::i18n::tr("L'ordinateur prêté ne répond plus.", "The lent computer is not responding.").into()) };
    app.set_session(Some(s));
    app.focus = Target::ComeBack;
    shot(app, cv, None, dir, &name("25f-emprunt-revenir"));
    app.set_session(None);
    app.store.borrowed.clear();
    app.store.selected = Some("a".into());

    // L'hôte : le panneau « Prêter l'ordinateur… » sans code (avertissement, durée, compte Steam), avec un code, avec une
    // demande ; Station trop ancienne.
    let mut s = session(app);
    s.menu = false;
    s.relative = false;
    let open = |s: &mut SessionUi| {
        if console {
            s.lend_panel = true;
        } else {
            s.bar = true;
            s.bar_menu = Some(BarMenu::Lend);
        }
    };
    open(&mut s);
    app.set_session(Some(s));
    app.screen = Screen::Session;
    app.pointer = if console { None } else { Some((700.0, 20.0)) };
    app.focus = Target::LendCreate;
    shot(app, cv, Some(video), dir, &name("26-pret-panneau"));
    app.store.app.lend_warned = true;
    if let Some(s) = app.session_mut() {
        s.lend = Some(lend_view(true, false));
    }
    app.focus = Target::LendCancelCode;
    shot(app, cv, Some(video), dir, &name("26b-pret-code"));
    // Le relais du code : le lien du relais en préparation (le cercle d'attente à la place du QR code), puis le
    // relais pas joignable (le lien du réseau, « Pas encore joignable hors de chez vous. »).
    let relay_code = |relay: ShareRelay, since: Option<Instant>| {
        let mut v = lend_view(true, false);
        if let Some(c) = v.code.as_mut() {
            c.relay = Some(relay);
            c.relay_since = since;
        }
        v
    };
    if let Some(s) = app.session_mut() {
        s.lend = Some(relay_code(ShareRelay::Connecting, Some(Instant::now())));
    }
    shot(app, cv, Some(video), dir, &name("26b2-pret-code-relais-preparation"));
    if let Some(s) = app.session_mut() {
        s.lend = Some(relay_code(ShareRelay::Connecting, Instant::now().checked_sub(crate::app::RELAY_LINK_WAIT + Duration::from_secs(1))));
    }
    shot(app, cv, Some(video), dir, &name("26b3-pret-code-relais-muet"));
    if let Some(s) = app.session_mut() {
        s.lend = Some(lend_view(false, true));
    }
    app.focus = Target::LendAccept(0);
    shot(app, cv, Some(video), dir, &name("26c-pret-demande"));
    // La demande d'un créneau : son texte, « Accepter » seul.
    if let Some(s) = app.session_mut() {
        let mut v = lend_view(false, true);
        v.requests[0].slot = true;
        v.requests[0].via_relay = false;
        s.lend = Some(v);
    }
    shot(app, cv, Some(video), dir, &name("26c2-pret-demande-creneau"));
    app.live.insert("a".into(), HostLive { lend: Some(LendAvail::Unsupported), ..app.live.get("a").cloned().unwrap_or_default() });
    if let Some(s) = app.session_mut() {
        s.lend = None;
    }
    app.focus = Target::LendClose;
    shot(app, cv, Some(video), dir, &name("26d-pret-inactif"));
    app.live.insert("a".into(), HostLive { lend: Some(LendAvail::Impossible), ..app.live.get("a").cloned().unwrap_or_default() });
    shot(app, cv, Some(video), dir, &name("26d2-pret-impossible"));
    app.live.insert("a".into(), HostLive { lend: Some(LendAvail::Available), ..app.live.get("a").cloned().unwrap_or_default() });
    // Le menu : la tuile « Prêter l'ordinateur… » (une demande attend).
    if console {
        let mut s = session(app);
        s.lend = Some(lend_view(true, true));
        app.set_session(Some(s));
        app.focus = Target::Menu(MenuItem::Lend);
        shot(app, cv, Some(video), dir, &name("26e-pret-menu"));
    }
    app.store.app.lend_warned = false;
    app.pointer = None;
    app.set_session(None);

    // L'accueil de l'ordinateur prêté : « Prêté à Léa jusqu'à 18 h », « Reprendre l'ordinateur » ; pendant la reprise.
    let lent = |notice: Option<(LendNoticeKind, u64)>| LendHostView { enabled: true, state: LendHostState::Lent, borrower: Some(("Léa".into(), Some(now + 2 * 3600))), notice, ..Default::default() };
    app.live.insert("a".into(), HostLive { lend_view: Some(lent(None)), ..app.live.get("a").cloned().unwrap_or_default() });
    app.go(Screen::Home);
    app.focus = Target::TakeBack;
    shot(app, cv, None, dir, &name("26f-pret-accueil-prete"));
    app.live.insert("a".into(), HostLive { lend_view: Some(lent(Some((LendNoticeKind::TakenBack, now + 102)))), ..app.live.get("a").cloned().unwrap_or_default() });
    app.focus = Target::TakeBackNow;
    shot(app, cv, None, dir, &name("26g-pret-accueil-reprise"));
    app.live.insert("a".into(), HostLive { lend_view: None, ..app.live.get("a").cloned().unwrap_or_default() });
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
        // KyberNow Share : rejoindre, le panneau de l'hôte, la session d'invité.
        share_shots(&mut app, &mut cv, &video, dir, prefix);
        // Hors de chez soi (docs/relais.md) : l'accueil, l'attente de l'hôte, ses demandes, une session par le relais.
        relay_shots(&mut app, &mut cv, &video, dir, prefix);
        // Le prêt de l'ordinateur (docs/pret.md) : l'ami (carte, attente, compte d'ami, session d'emprunt), l'hôte.
        lend_shots(&mut app, &mut cv, &video, dir, prefix);
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
        h.last = Some(LastSession { median: 9.8, p95: 14.0, seconds: 6000, mode: "3840 × 2160".into(), hz: 60, codec: "HEVC".into(), mbps: 60.0, at: 0, screens: 1, route: String::new() });
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
        h.last = Some(LastSession { median: 11.2, p95: 15.0, seconds: 6000, mode: "1920 × 1080".into(), hz: 60, codec: "HEVC".into(), mbps: 28.0, at: 0, screens: 1, route: String::new() });
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
        // Les captures en anglais des autres tests changent la langue de tous les fils : celui de cet essai reste en
        // français (les largeurs de la barre en dépendent).
        crate::i18n::force_english(false);
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

    // MARK: - Le relais

    /// Les captures du relais se produisent au bureau et au salon, dans les deux langues.
    #[test]
    fn relay_shots_are_produced() {
        let dir = std::env::temp_dir().join(format!("kn-linux-relais-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        for (layout, prefix, size) in [(Layout::Desktop, "bureau", (1280, 800)), (Layout::Tv, "salon", (1280, 720))] {
            for en in [false, true] {
                let mut app = demo(en, layout);
                let mut cv = Canvas::new();
                cv.resize(size.0, size.1, 1.0);
                relay_shots(&mut app, &mut cv, &demo_image(3), &dir, prefix);
            }
        }
        for n in ["01f-accueil-hors-de-chez-vous", "20g-rejoindre-attente", "21k-partage-relais", "21n-partage-relais-complet", "23-session-relais", "23c-invite-relais"] {
            assert!(dir.join(format!("bureau-{n}.png")).exists() && dir.join(format!("salon-{n}.png")).exists(), "{n}");
        }
        crate::i18n::force_english(false);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Le panneau de l'hôte avec le relais joint, quatre « places » (deux invités et deux demandes) ou quatre invités et
    /// trois demandes, tient dans la fenêtre la plus basse (960 × 600) : chaque bouton qu'il inscrit y est entier, les
    /// demandes ont « Accepter » et « Refuser », rien ne se chevauche.
    #[test]
    fn share_panel_with_requests_fits_the_smallest_window() {
        for (guests, requests) in [(2usize, 2usize), (4, 3)] {
            let (mut app, mut cv) = hosting(0, (960, 600));
            let mut view = relay_view(Some(ShareRelay::Joined));
            view.guests.truncate(2);
            while view.guests.iter().filter(|g| !g.pending).count() < guests {
                view.guests.insert(0, ShareGuest { id: format!("x{}", view.guests.len()), name: "Zoé".into(), app: "mac".into(), ..Default::default() });
            }
            while view.guests.iter().filter(|g| g.pending).count() < requests {
                view.guests.push(ShareGuest { id: format!("y{}", view.guests.len()), name: "Luc".into(), app: "web".into(), pending: true, via_relay: true, ..Default::default() });
            }
            app.session_mut().unwrap().share = Some(view.clone());
            let hits = ui::draw(&app, &mut cv);
            let rect = |t: Target| hits.iter().find(|(h, _)| *h == t).unwrap_or_else(|| panic!("{t:?} absent du panneau ({guests} invités, {requests} demandes)")).1;
            let mut need = vec![Target::ShareClose, Target::ShareCopy, Target::ShareNewCode, Target::ShareStop];
            for (i, g) in view.guests.iter().enumerate() {
                if g.pending {
                    need.push(Target::ShareRefuse(i));
                    // Les quatre places prises : « Accepter » est dessiné inactif, il n'est pas inscrit (ni focus ni clic).
                    if view.full() {
                        assert!(!hits.iter().any(|(h, _)| *h == Target::ShareAccept(i)), "« Accepter » doit être inactif ({guests} invités, {requests} demandes)");
                    } else {
                        need.push(Target::ShareAccept(i));
                    }
                } else {
                    need.extend([Target::SharePads(i), Target::ShareKbm(i), Target::ShareKick(i)]);
                }
            }
            let rects: Vec<crate::app::Rect> = need.iter().map(|t| rect(*t)).collect();
            for (t, r) in need.iter().zip(&rects) {
                assert!(r.x >= 0.0 && r.y >= 0.0 && r.x + r.w <= 960.0 && r.y + r.h <= 600.0 - 8.0, "{t:?} dépasse la fenêtre ({guests} invités, {requests} demandes) : {r:?}");
            }
            for (i, a) in rects.iter().enumerate() {
                for (j, b) in rects.iter().enumerate().skip(i + 1) {
                    let overlap = a.x < b.x + b.w - 0.5 && b.x < a.x + a.w - 0.5 && a.y < b.y + b.h - 0.5 && b.y < a.y + a.h - 0.5;
                    assert!(!overlap, "{:?} et {:?} se chevauchent ({guests} invités, {requests} demandes)", need[i], need[j]);
                }
            }
        }
    }

    /// À la manette, le panneau de l'hôte (salon, Deck) avec ses demandes reste dans l'écran : ses boutons y sont entiers.
    #[test]
    fn share_panel_with_requests_fits_a_pad_screen() {
        // Trois invités (« Accepter » actif) ou quatre (les quatre places sont prises : « Accepter » est dessiné inactif).
        for (layout, w, h, extra) in [(Layout::Handheld, 1280u32, 800u32, 1usize), (Layout::Handheld, 1280, 800, 2), (Layout::Tv, 1920, 1080, 1), (Layout::Tv, 1920, 1080, 2)] {
            let mut app = demo(false, layout);
            let mut cv = Canvas::new();
            cv.resize(w, h, 1.0);
            let mut s = session(&app);
            s.menu = false;
            s.share_panel = true;
            let mut view = relay_view(Some(ShareRelay::Joined));
            for (id, name) in [("x", "Zoé"), ("w", "Nina")].into_iter().take(extra) {
                view.guests.insert(0, ShareGuest { id: id.into(), name: name.into(), app: "mac".into(), ..Default::default() });
            }
            assert_eq!(view.full(), extra == 2);
            s.share = Some(view.clone());
            app.set_session(Some(s));
            app.screen = Screen::Session;
            let hits = ui::draw(&app, &mut cv);
            for (i, g) in view.guests.iter().enumerate().filter(|(_, g)| g.pending) {
                // Les quatre places prises : « Accepter » n'est pas inscrit (ni focus ni clic), « Refuser » l'est.
                assert_eq!(hits.iter().any(|(h, _)| *h == Target::ShareAccept(i)), !view.full(), "« Accepter » inactif quand les quatre places sont prises");
                for t in [Target::ShareAccept(i), Target::ShareRefuse(i)].into_iter().filter(|t| !(view.full() && matches!(t, Target::ShareAccept(_)))) {
                    let r = hits.iter().find(|(h, _)| *h == t).unwrap_or_else(|| panic!("{t:?} absent")).1;
                    assert!(r.x >= 0.0 && r.y >= 0.0 && r.x + r.w <= w as f32 && r.y + r.h <= h as f32 - 56.0, "{t:?} hors de l'écran de {layout:?} : {r:?} ({})", g.name);
                }
            }
        }
    }

    /// Une session par le relais montre sa puce « Par le relais » dans la barre, ses statistiques leur ligne « Chemin » ;
    /// directe, rien de plus.
    #[test]
    fn a_session_through_the_relay_says_so() {
        crate::i18n::force_english(false);
        let mut app = demo(false, Layout::Desktop);
        let mut cv = Canvas::new();
        cv.resize(1280, 800, 1.0);
        let lit = |app: &App, cv: &mut Canvas| {
            cv.clear(crate::ui::theme::INK_BG);
            ui::draw(app, cv);
            cv.pix.data().chunks(4).filter(|p| p[3] > 0).count()
        };
        let mut s = session(&app);
        s.menu = false;
        s.bar = true;
        s.show_stats = true;
        app.set_session(Some(s));
        app.screen = Screen::Session;
        app.pointer = Some((700.0, 20.0));
        let direct = lit(&app, &mut cv);
        app.session_mut().unwrap().route = Some(relayed(PathKind::Relay));
        let relay = lit(&app, &mut cv);
        assert_ne!(direct, relay, "la puce et le chemin se dessinent");
    }

    // MARK: - KyberNow Share

    /// Un hôte au bureau, partage ouvert, panneau déployé, `n` invités, dans une fenêtre de `(w, h)` points.
    fn hosting(n: usize, size: (u32, u32)) -> (App, Canvas) {
        crate::i18n::force_english(false);
        let mut app = demo(false, Layout::Desktop);
        let mut cv = Canvas::new();
        cv.resize(size.0, size.1, 1.0);
        let mut view = shared_view();
        view.guests.truncate(n);
        while view.guests.len() < n {
            view.guests.push(ShareGuest { id: format!("x{}", view.guests.len()), name: "Zoé".into(), role: ShareRole::Spectator, app: "android".into(), since: None, quality: None, ..Default::default() });
        }
        let mut s = session(&app);
        s.menu = false;
        s.relative = false;
        s.bar = true;
        s.bar_menu = Some(BarMenu::Share);
        s.share = Some(view);
        s.opened_at = Some(Instant::now() - Duration::from_secs(30));
        app.set_session(Some(s));
        app.screen = Screen::Session;
        app.pointer = Some((size.0 as f32 / 2.0, 20.0));
        (app, cv)
    }

    /// Le panneau de l'hôte au bureau tient dans la fenêtre la plus basse (960 × 600) avec quatre invités, chaque
    /// bouton qu'il inscrit y est entier ; il a tout ce que dit link-ui : code, QR code, aide, invités, actions, boutons du bas.
    /// Chaque invité a ses deux boutons de droits et « Retirer », sans chevauchement, dans la largeur du panneau.
    #[test]
    fn share_panel_fits_the_smallest_window() {
        let (app, mut cv) = hosting(4, (960, 600));
        let hits = ui::draw(&app, &mut cv);
        let rect = |t: Target| hits.iter().find(|(h, _)| *h == t).unwrap_or_else(|| panic!("{t:?} absent du panneau")).1;
        let mut need = vec![Target::ShareClose, Target::ShareCopy, Target::ShareNewCode, Target::ShareStop];
        for i in 0..4 {
            need.extend([Target::SharePads(i), Target::ShareKbm(i), Target::ShareKick(i)]);
        }
        for t in need {
            let r = rect(t);
            assert!(r.x >= 0.0 && r.y >= 0.0 && r.x + r.w <= 960.0 && r.y + r.h <= 600.0, "{t:?} sort de la fenêtre : {r:?}");
        }
        // Sur une ligne : manettes, clavier et souris, « Retirer », côte à côte et dans la largeur du panneau.
        let right = rect(Target::ShareStop);
        for i in 0..4 {
            let (p, k, x) = (rect(Target::SharePads(i)), rect(Target::ShareKbm(i)), rect(Target::ShareKick(i)));
            assert!(p.x + p.w <= k.x && k.x + k.w <= x.x, "ligne {i} : boutons qui se chevauchent : {p:?} {k:?} {x:?}");
            assert!(x.x + x.w <= right.x + right.w, "ligne {i} : « Retirer » sort du panneau : {x:?} contre {right:?}");
        }
        // Le panneau ne chevauche pas la barre, et le bouton du bas est sous le dernier invité.
        let bottom = rect(Target::ShareKick(3));
        let foot = rect(Target::ShareNewCode);
        assert!(foot.y >= bottom.y + bottom.h);
        // Sans invité : « Personne pour l'instant. » et rien à faire sur une ligne.
        let (app, mut cv) = hosting(0, (1280, 800));
        let hits = ui::draw(&app, &mut cv);
        assert!(hits.iter().all(|(t, _)| !matches!(t, Target::SharePads(_) | Target::ShareKbm(_) | Target::ShareKick(_))));
        assert!(hits.iter().any(|(t, _)| *t == Target::ShareStop));
    }

    /// À la manette, le panneau prend l'image : ses boutons sont entiers à l'écran, sans « Copier le lien » ni croix
    /// (pas de presse-papiers utile, B referme) ; au salon comme au Deck.
    #[test]
    fn share_panel_on_a_pad_has_no_copy() {
        for (layout, size) in [(Layout::Handheld, (1280, 800)), (Layout::Tv, (1920, 1080))] {
            crate::i18n::force_english(false);
            let mut app = demo(false, layout);
            let mut cv = Canvas::new();
            cv.resize(size.0, size.1, 1.0);
            let mut s = session(&app);
            s.menu = false;
            s.share = Some(shared_view());
            s.share_panel = true;
            app.set_session(Some(s));
            app.screen = Screen::Session;
            assert!(ui::session_overlay_visible(&app), "le panneau est une surimpression");
            let hits = ui::draw(&app, &mut cv);
            assert!(hits.iter().all(|(t, _)| *t != Target::ShareCopy && *t != Target::ShareClose));
            for (t, r) in &hits {
                assert!(r.x >= 0.0 && r.y >= 0.0 && r.x + r.w <= size.0 as f32 && r.y + r.h <= (size.1 - 40) as f32, "{layout:?} : {t:?} sort de l'écran : {r:?}");
            }
            assert!(hits.iter().any(|(t, _)| *t == Target::ShareStop) && hits.iter().any(|(t, _)| *t == Target::ShareKick(3)));
        }
    }

    /// La barre d'un hôte propose « Partager » (éteint sans Station qui partage), celle d'un invité n'a ni profil ni
    /// « Partager » ; le clavier et la souris n'y sont qu'avec ce rôle ; « Quitter le partage » remplace « Se déconnecter ».
    #[test]
    fn bar_of_a_host_and_of_a_guest() {
        crate::i18n::force_english(false);
        let mut app = demo(false, Layout::Desktop);
        let mut cv = Canvas::new();
        cv.resize(1280, 800, 1.0);
        let mut s = session(&app);
        s.menu = false;
        s.relative = false;
        s.bar = true;
        s.opened_at = Some(Instant::now() - Duration::from_secs(30));
        app.set_session(Some(s));
        app.screen = Screen::Session;
        let hits = ui::draw(&app, &mut cv);
        for t in [Target::BarProfile, Target::BarKeys, Target::BarMouse, Target::BarShare, Target::BarQuit] {
            assert!(hits.iter().any(|(h, _)| *h == t), "{t:?} manque à la barre de l'hôte");
        }
        // Le partage ouvert : la pastille « 4 invités » rouvre le panneau (deux endroits pour la même cible).
        app.session_mut().unwrap().share = Some(shared_view());
        let hits = ui::draw(&app, &mut cv);
        assert_eq!(hits.iter().filter(|(h, _)| *h == Target::BarShare).count(), 2);
        // Un invité, avec chacun des quatre rôles : touches et souris seulement avec le clavier et la souris.
        for role in [ShareRole::Spectator, ShareRole::Player, ShareRole::Kbm, ShareRole::PlayerKbm] {
            let keys = role.kbm();
            let mut g = guest_session(role);
            g.bar = true;
            app.set_session(Some(g));
            let hits = ui::draw(&app, &mut cv);
            for t in [Target::BarProfile, Target::BarShare, Target::BarComputers, Target::BarPick(Profile::Jeu)] {
                assert!(hits.iter().all(|(h, _)| *h != t), "{role:?} : {t:?} n'a pas sa place à la barre d'un invité");
            }
            assert_eq!(hits.iter().any(|(h, _)| *h == Target::BarKeys), keys, "{role:?} : touches");
            assert_eq!(hits.iter().any(|(h, _)| *h == Target::BarMouse), keys, "{role:?} : souris");
            assert!(hits.iter().any(|(h, _)| *h == Target::BarQuit) && hits.iter().any(|(h, _)| *h == Target::BarFullscreen));
        }
        // La zone du geste suit la largeur de la barre réduite.
        let g = guest_session(ShareRole::Spectator);
        app.set_session(Some(g));
        let s = app.session().unwrap().clone();
        assert!(ui::at_bar_edge(&mut cv, &app, &s, 1280.0, 640.0, 0.0));
    }

    /// « Qualité de l'image » d'un invité, au bureau : une puce dans la barre (la qualité reçue), les cinq choix dans son
    /// menu ; l'hôte n'en a pas. Même dans la fenêtre la plus étroite (960 points), avec tous les droits et la qualité reçue
    /// la plus longue, la barre reste dans la fenêtre.
    #[test]
    fn picture_quality_on_the_bar_of_a_guest() {
        crate::i18n::force_english(false);
        for size in [(960u32, 600u32), (1280, 800)] {
            let mut app = demo(false, Layout::Desktop);
            let mut cv = Canvas::new();
            cv.resize(size.0, size.1, 1.0);
            let mut host = session(&app);
            host.menu = false;
            host.bar = true;
            app.set_session(Some(host));
            app.screen = Screen::Session;
            let hits = ui::draw(&app, &mut cv);
            assert!(hits.iter().all(|(t, _)| !matches!(t, Target::BarQuality | Target::QualityPick(_))), "{size:?} : l'hôte n'a pas de qualité d'image");
            for role in [ShareRole::Spectator, ShareRole::PlayerKbm] {
                let mut g = guest_session(role);
                g.bar = true;
                if let Some(u) = g.guest.as_mut() {
                    u.quality = ShareQuality::Host;
                    u.image = Some("2160p · 120 Hz".into());
                }
                app.set_session(Some(g));
                let hits = ui::draw(&app, &mut cv);
                assert_eq!(hits.iter().filter(|(t, _)| *t == Target::BarQuality).count(), 1, "{size:?} {role:?} : la puce");
                assert!(hits.iter().all(|(t, _)| !matches!(t, Target::QualityPick(_))), "menu fermé");
                for (t, r) in &hits {
                    assert!(r.x >= 0.0 && r.x + r.w <= size.0 as f32, "{size:?} {role:?} : {t:?} sort de la fenêtre : {r:?}");
                }
                // Le menu : les cinq choix, dans la fenêtre, chacun choisissable.
                app.session_mut().unwrap().bar_menu = Some(BarMenu::Quality);
                let hits = ui::draw(&app, &mut cv);
                let picks: Vec<_> = hits.iter().filter(|(t, _)| matches!(t, Target::QualityPick(_))).collect();
                assert_eq!(picks.len(), ShareQuality::ALL.len(), "{size:?} {role:?}");
                for (t, r) in &picks {
                    assert!(r.x >= 0.0 && r.y >= 0.0 && r.x + r.w <= size.0 as f32 && r.y + r.h <= size.1 as f32, "{size:?} : {t:?} sort : {r:?}");
                }
            }
        }
    }

    /// « Qualité de l'image » d'un invité à la manette : la tuile du menu (sa qualité reçue en sous-titre), puis le panneau des
    /// cinq choix ; rien de cela pour l'hôte.
    #[test]
    fn picture_quality_in_the_menu_of_a_guest() {
        crate::i18n::force_english(false);
        for (layout, size) in [(Layout::Handheld, (1280u32, 800u32)), (Layout::Tv, (1920, 1080))] {
            let mut app = demo(false, layout);
            let mut cv = Canvas::new();
            cv.resize(size.0, size.1, 1.0);
            let mut host = session(&app);
            host.menu = true;
            app.set_session(Some(host));
            app.screen = Screen::Session;
            assert!(ui::draw(&app, &mut cv).iter().all(|(t, _)| *t != Target::Menu(MenuItem::Quality)), "{layout:?} : pas pour l'hôte");
            let mut g = guest_session(ShareRole::PlayerKbm);
            g.menu = true;
            app.set_session(Some(g));
            let hits = ui::draw(&app, &mut cv);
            let tile = hits.iter().find(|(t, _)| *t == Target::Menu(MenuItem::Quality)).unwrap_or_else(|| panic!("{layout:?} : la tuile")).1;
            assert!(tile.x + tile.w <= size.0 as f32 && tile.y + tile.h <= size.1 as f32, "{layout:?} : {tile:?}");
            // Le panneau.
            app.activate(Target::Menu(MenuItem::Quality));
            let hits = ui::draw(&app, &mut cv);
            let picks: Vec<_> = hits.iter().filter(|(t, _)| matches!(t, Target::QualityPick(_))).collect();
            assert_eq!(picks.len(), ShareQuality::ALL.len(), "{layout:?}");
            for (t, r) in &picks {
                assert!(r.x >= 0.0 && r.y >= 0.0 && r.x + r.w <= size.0 as f32 && r.y + r.h <= size.1 as f32, "{layout:?} : {t:?} sort : {r:?}");
            }
            assert_eq!(app.focus, Target::QualityPick(ShareQuality::Auto));
            assert!(hits.iter().all(|(t, _)| !matches!(t, Target::Menu(_) | Target::ShareNewCode)), "{layout:?} : rien d'autre que les choix");
        }
    }

    /// Le menu à la manette d'un invité : « Quitter le partage » en dernier, rien d'un hôte (profils, « Alléger », « Partager »),
    /// le clavier, la souris et la manette en souris seulement avec ce rôle ; celui d'un hôte a la tuile « Partager ».
    #[test]
    fn menu_of_a_guest_and_of_a_host() {
        crate::i18n::force_english(false);
        let mut app = demo(false, Layout::Handheld);
        let mut cv = Canvas::new();
        cv.resize(1280, 800, 1.0);
        let items = |app: &App, cv: &mut Canvas| -> Vec<Target> { ui::draw(app, cv).into_iter().map(|(t, _)| t).filter(|t| matches!(t, Target::Menu(_))).collect() };
        let mut host = session(&app);
        host.menu = true;
        app.set_session(Some(host));
        app.screen = Screen::Session;
        let t = items(&app, &mut cv);
        assert!(t.contains(&Target::Menu(MenuItem::Share)) && t.contains(&Target::Menu(MenuItem::Profile(Profile::Bureau))));
        assert_eq!(t.last(), Some(&Target::Menu(MenuItem::Disconnect)));
        for role in [ShareRole::Spectator, ShareRole::Player, ShareRole::Kbm, ShareRole::PlayerKbm] {
            let mut g = guest_session(role);
            g.menu = true;
            app.set_session(Some(g));
            let t = items(&app, &mut cv);
            assert!(t.iter().all(|i| !matches!(i, Target::Menu(MenuItem::Share | MenuItem::Light | MenuItem::Profile(_)))), "{role:?} : {t:?}");
            assert_eq!(t.contains(&Target::Menu(MenuItem::Keys)), role.kbm(), "{role:?}");
            assert_eq!(t.contains(&Target::Menu(MenuItem::Keyboard)), role.kbm(), "{role:?}");
            assert_eq!(t.contains(&Target::Menu(MenuItem::PadMouse)), role.kbm(), "{role:?}");
            assert_eq!(t.contains(&Target::Menu(MenuItem::Gamepads)), role.pads(), "{role:?} : les manettes seulement avec les manettes");
            assert_eq!(t.last(), Some(&Target::Menu(MenuItem::Disconnect)));
            assert!(t.contains(&Target::Menu(MenuItem::Resume)) && t.contains(&Target::Menu(MenuItem::Sound)) && t.contains(&Target::Menu(MenuItem::Stats)));
        }
    }

    /// « Rejoindre un partage » se dessine pour chaque interface : les champs, « Rejoindre », et à la manette le pavé qui suit le champ
    /// (chiffres pour le code, lettres pour le prénom) ; rien ne sort de l'écran.
    #[test]
    fn join_screen_draws_for_every_interface() {
        for (layout, size) in [(Layout::Desktop, (960, 600)), (Layout::Desktop, (1280, 800)), (Layout::Handheld, (1280, 800)), (Layout::Tv, (1920, 1080))] {
            crate::i18n::force_english(false);
            let mut app = demo(false, layout);
            let mut cv = Canvas::new();
            cv.resize(size.0, size.1, 1.0);
            app.join = JoinState { name: "Atelier de Léa".into(), message: Some("Aucun partage avec ce code sur le réseau.".into()), ..JoinState::new() };
            app.go(Screen::Join);
            let hits = ui::draw(&app, &mut cv);
            for t in [Target::JoinCode, Target::JoinName, Target::JoinGo] {
                assert!(hits.iter().any(|(h, _)| *h == t), "{layout:?} : {t:?}");
            }
            // « Qualité de l'image » : les cinq choix, sous le prénom ; à la manette, ils ne passent pas sous le pavé.
            let quality: Vec<crate::app::Rect> = ShareQuality::ALL.iter().map(|q| hits.iter().find(|(h, _)| *h == Target::JoinQuality(*q)).unwrap_or_else(|| panic!("{layout:?} : {q:?}")).1).collect();
            if layout.console() {
                let pad_x = hits.iter().filter(|(t, _)| matches!(t, Target::Key(_))).map(|(_, r)| r.x).fold(f32::MAX, f32::min);
                let last = quality.last().unwrap();
                assert!(last.x + last.w < pad_x, "{layout:?} : les choix passent sous le pavé ({} > {pad_x})", last.x + last.w);
            }
            for (t, r) in &hits {
                assert!(r.x >= 0.0 && r.y >= 0.0 && r.x + r.w <= size.0 as f32 && r.y + r.h <= size.1 as f32, "{layout:?} {size:?} : {t:?} sort de l'écran : {r:?}");
            }
            let digits = hits.iter().filter(|(t, _)| matches!(t, Target::Key(c) if c.is_ascii_digit())).count();
            if layout.console() {
                assert_eq!(digits, 10, "le pavé des chiffres");
                assert!(hits.iter().all(|(t, _)| *t != Target::Key('q')));
                app.join.field = JoinField::Name;
                let hits = ui::draw(&app, &mut cv);
                assert!(hits.iter().any(|(t, _)| *t == Target::Key('q')) && hits.iter().any(|(t, _)| *t == Target::KeyShift) && hits.iter().any(|(t, _)| *t == Target::Key(' ')));
            } else {
                assert_eq!(digits, 0, "au bureau, le clavier");
            }
        }
    }

    /// Sur l'accueil, « Rejoindre avec un code » est à côté de « Ajouter un ordinateur » (deux cartes dans la place d'une),
    /// et le Select (⧉) le porte à la manette ; sans ordinateur, un bouton à côté de l'ajout.
    #[test]
    fn home_offers_to_join() {
        for layout in [Layout::Desktop, Layout::Handheld, Layout::Tv] {
            crate::i18n::force_english(false);
            let mut app = demo(false, layout);
            let mut cv = Canvas::new();
            let size = if layout == Layout::Tv { (1920, 1080) } else { (1280, 800) };
            cv.resize(size.0, size.1, 1.0);
            app.go(Screen::Home);
            let hits = ui::draw(&app, &mut cv);
            let add = hits.iter().find(|(t, _)| *t == Target::AddCard).expect("ajouter").1;
            let join = hits.iter().find(|(t, _)| *t == Target::JoinCard).expect("rejoindre").1;
            assert!((add.x - join.x).abs() < 1.0 && join.y >= add.y + add.h, "{layout:?} : une carte sous l'autre");
            // Les ordinateurs gardent leur place : trois cartes d'ordinateur et la paire.
            assert_eq!(hits.iter().filter(|(t, _)| matches!(t, Target::Host(_))).count(), 3);
            let mut empty = App::new(Store::default(), layout);
            empty.machine = app.machine.clone();
            empty.go(Screen::Home);
            let hits = ui::draw(&empty, &mut cv);
            assert!(hits.iter().any(|(t, _)| *t == Target::AddCard) && hits.iter().any(|(t, _)| *t == Target::JoinCard), "{layout:?} : accueil vide");
        }
    }

    /// La barre d'un hôte qui partage, avec « Ordinateurs » et « Garder le son » (plusieurs sessions) et les quatre
    /// invités, tient dans la fenêtre la plus étroite (960 points) : « Partager » et sa pastille « 4 invités »
    /// restent entiers (sinon la pastille, qui rouvre le panneau, sortirait de la fenêtre).
    #[test]
    fn bar_with_a_share_fits_the_narrowest_window() {
        let (mut app, mut cv) = hosting(4, (960, 600));
        let mut second = SessionUi::new(2, "c", "Mini du bureau", Profile::Bureau, false, false, 1);
        second.phase = Phase::Streaming;
        app.open_session(second);
        app.activate_session(1);
        app.session_mut().unwrap().bar = true;
        app.session_mut().unwrap().bar_menu = None;
        let hits = ui::draw(&app, &mut cv);
        let shares: Vec<_> = hits.iter().filter(|(t, _)| *t == Target::BarShare).map(|(_, r)| *r).collect();
        assert_eq!(shares.len(), 2, "le bouton et la pastille");
        for (t, r) in &hits {
            assert!(r.x >= 0.0 && r.x + r.w <= 960.0, "{t:?} sort de la fenêtre étroite : {r:?}");
        }
        // Partage ouvert sans invité : le bouton seul, pas de pastille « 0 invité ».
        let (mut app, mut cv) = hosting(0, (960, 600));
        app.session_mut().unwrap().bar_menu = None;
        let hits = ui::draw(&app, &mut cv);
        assert_eq!(hits.iter().filter(|(t, _)| *t == Target::BarShare).count(), 1);
    }

    /// Le panneau d'un hôte dont le partage est ouvert propose à chaque invité les deux droits, quel que soit son rôle
    /// (spectateur, manettes, clavier et souris, les deux) : « Donner » ou « Reprendre les manettes », « Donner » ou
    /// « Reprendre le clavier et la souris », puis « Retirer » ; jamais « Faire jouer » ni « Faire regarder ».
    #[test]
    fn share_panel_offers_both_rights_to_every_guest() {
        let (app, mut cv) = hosting(4, (1280, 800));
        let hits = ui::draw(&app, &mut cv);
        let has = |t: Target| hits.iter().any(|(h, _)| *h == t);
        for i in 0..4 {
            assert!(has(Target::SharePads(i)) && has(Target::ShareKbm(i)) && has(Target::ShareKick(i)), "invité {i}");
        }
        // Un seul bouton par droit et par invité : trois cibles par ligne, pas une de plus.
        let per_guest = hits.iter().filter(|(t, _)| matches!(t, Target::SharePads(_) | Target::ShareKbm(_) | Target::ShareKick(_))).count();
        assert_eq!(per_guest, 12);
    }

    /// Partage impossible : à la souris le bouton de la barre est estompé et son infobulle dit pourquoi ; sur l'interface
    /// de console la tuile « Partager » est estompée, se choisit, et le panneau qu'elle ouvre est réduit à la raison
    /// (ni code, ni invités) ; avant la réponse d'Admin, ni l'un ni l'autre n'est estompé.
    #[test]
    fn share_unavailable_is_dimmed_and_says_why() {
        crate::i18n::force_english(false);
        let reason = |a: ShareAvail| a.text().unwrap();
        // Le bureau.
        let (mut app, mut cv) = streaming(Layout::Desktop);
        app.session_mut().unwrap().bar = true;
        let share_rect = |app: &App, cv: &mut Canvas| ui::draw(app, cv).iter().find(|(t, _)| *t == Target::BarShare).map(|(_, r)| *r).unwrap();
        // Le pixel du bouton : la couleur de l'icône (estompée, ou non).
        let icon_ink = |app: &App, cv: &mut Canvas, r: crate::app::Rect| {
            ui::draw(app, cv);
            let mut best = 0u32;
            for y in (r.y as u32 + 8)..(r.y as u32 + 26) {
                for x in (r.x as u32 + 8)..(r.x as u32 + 26) {
                    let p = cv.pix.pixel(x, y).unwrap();
                    best = best.max(p.red() as u32 + p.green() as u32 + p.blue() as u32);
                }
            }
            best
        };
        app.live.get_mut("a").unwrap().share = None;
        let r = share_rect(&app, &mut cv);
        let unknown = icon_ink(&app, &mut cv, r);
        app.live.get_mut("a").unwrap().share = Some(ShareAvail::Available);
        let available = icon_ink(&app, &mut cv, r);
        assert_eq!(unknown, available, "avant la réponse d'Admin, le bouton n'est pas estompé");
        app.live.get_mut("a").unwrap().share = Some(ShareAvail::Unsupported);
        assert!(icon_ink(&app, &mut cv, r) < available, "estompé");
        // L'infobulle : la raison, sous la barre, au survol.
        app.pointer = Some((r.x + r.w / 2.0, r.y + r.h / 2.0));
        ui::draw(&app, &mut cv);
        assert!(cv.pix.pixel((r.x + r.w / 2.0) as u32, 46 + 4 + 13).unwrap().alpha() > 0, "{}", reason(ShareAvail::Unsupported));
        // La console : la tuile est choisissable, le panneau est réduit à la raison.
        for layout in [Layout::Handheld, Layout::Tv] {
            let (mut app, mut cv) = streaming(layout);
            app.live.get_mut("a").unwrap().share = Some(ShareAvail::Disabled);
            app.session_mut().unwrap().menu = true;
            let hits = ui::draw(&app, &mut cv);
            assert!(hits.iter().any(|(t, _)| *t == Target::Menu(MenuItem::Share)), "{layout:?} : la tuile se choisit");
            app.activate(Target::Menu(MenuItem::Share));
            assert!(app.session().unwrap().share_panel);
            let hits = ui::draw(&app, &mut cv);
            assert!(hits.iter().all(|(t, _)| !matches!(t, Target::ShareNewCode | Target::ShareStop | Target::ShareKick(_))), "{layout:?} : rien d'autre que la raison");
        }
    }

    /// Le refus du cœur sous « Rejoindre » (jusqu'à deux lignes : un refus long d'Admin) reste dans la fenêtre
    /// la plus basse (960 × 600), sous le bouton.
    #[test]
    fn join_refusal_fits_the_smallest_window() {
        crate::i18n::force_english(false);
        let mut app = demo(false, Layout::Desktop);
        let mut cv = Canvas::new();
        cv.resize(960, 600, 1.0);
        let long = "Admin : le partage demandé n'existe plus sur cet ordinateur ; vérifiez le code affiché chez la personne qui partage, puis réessayez dans un instant (412).";
        app.join = JoinState { name: "Atelier de Léa".into(), code: "482917".into(), message: Some(long.into()), ..JoinState::new() };
        app.go(Screen::Join);
        let hits = ui::draw(&app, &mut cv);
        let go = hits.iter().find(|(t, _)| *t == Target::JoinGo).expect("rejoindre").1;
        // Le texte est à droite du bouton, deux lignes de 22 points au plus : il ne sort ni du bas ni du côté de la fenêtre.
        assert!(go.y + go.h <= 600.0 && go.x + go.w + 24.0 < 960.0 / 2.0, "le bouton est dans la fenêtre : {go:?}");
        let ink_below = (go.y + go.h + 4.0) as u32..600;
        let lit = |cv: &Canvas, rows: std::ops::Range<u32>, from: u32| rows.into_iter().any(|y| (from..960u32).any(|x| cv.pix.pixel(x, y).is_some_and(|p| p.alpha() > 0 && p.red() > 150 && p.green() > 120)));
        assert!(!lit(&cv, ink_below, 0), "rien sous le bouton : le refus est à sa droite");
        assert!(lit(&cv, go.y as u32..(go.y + go.h) as u32, (go.x + go.w + 24.0) as u32), "le refus se lit à droite du bouton");
    }
}
