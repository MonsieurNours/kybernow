//! L'app sur le téléviseur : la fenêtre SDL (plan graphique), la boucle des
//! événements (Magic Remote, clavier, souris, manettes), l'interface, les
//! tâches de fond et la session.
//!
//! Pendant la session, l'image de l'ordinateur est sur le plan vidéo (NDL) ;
//! la fenêtre est effacée en transparent et ne montre que le menu, les
//! capsules et le pointeur de l'ordinateur.

mod jobs;
mod log_file;
mod eac3;
mod esplayer;
mod media;
mod starfish;
mod ndl;
mod pads;
pub mod sdl;
mod session;
mod system;

use std::ffi::CString;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Sender};
use std::sync::Arc;
use std::time::{Duration, Instant};

use kybernow_core::console::{DeviceInfo, Mode};
use kybernow_core::input::GamepadRumble;
use kybernow_core::kynput_rs::MouseButtonType;
use kybernow_core::session::Status;
use kybernow_core::share::{self, JoinRequest, ShareTarget};

use crate::app::{Action, AddStep, App, Found, Nav, Online, Phase, Polled, Screen, SessionUi, ShareRole, StationOp, StationRefusal, Stats};
use crate::i18n::{self, tr};
use crate::model::{now_secs, LastSession, Store};
use crate::ui::{self, canvas::Canvas, Assets};

/// Ce qui revient à la boucle principale (fils de fond, rappels du cœur).
pub enum Event {
    Polled { id: String, polled: Polled },
    /// Réponse d'Admin à un redémarrage ou une mise à jour de la Station
    /// (la version visée pour une mise à jour).
    Station { id: String, op: StationOp, result: Result<Option<String>, StationRefusal> },
    Preview { id: String, jpeg: Vec<u8> },
    Found(Vec<Found>),
    Probed { address: String, port: u16, fingerprint: Result<String, String> },
    PairAsked { name: String, address: String },
    PairAccepted(crate::model::Host),
    PairFailed(String),
    Status(Status),
    /// Mode de l'écran de la session (pour la dernière session).
    Mode(Mode),
    Second { latency_ms: Option<f64>, fps: f64, mbps: f64, lost: u32, codec: String },
    Notice(String),
    Pointer { position: Option<(f64, f64)>, hidden: bool, shape: Option<(u32, u32, u32, u32, Vec<u32>)> },
    Rumble(GamepadRumble),
    Combo,
    InputReady,
    GameFullscreen(bool),
    DecoderFailed(String),
    Update(Result<Option<kybernow_core::update::Available>, String>),
    /// KyberNow Share, invité : la réponse de « Rejoindre » (numéro de la recherche), ou son refus.
    ShareJoined { attempt: u32, result: Result<share::Found, share::ShareError> },
    /// KyberNow Share, invité : la demande est partie par le relais, l'hôte ne l'a pas encore acceptée
    /// (« En attente de l'accord de l'hôte… »).
    SharePending { attempt: u32 },
    /// KyberNow Share, hôte : le partage de la session après une demande à Admin (`None` : fermé), ou ce
    /// qu'Admin a refusé.
    ShareHost(Result<Option<crate::app::ShareView>, String>),
    /// KyberNow Share, hôte : le partage et ses invités, au signe de vie (`None` : pas de partage).
    Guests(Option<crate::app::ShareView>),
    /// KyberNow Share, invité : son rôle (`previous` nul au départ), puis chaque changement de l'hôte.
    ShareRole { role: ShareRole, previous: Option<ShareRole> },
    /// KyberNow Share, invité : l'image qu'il reçoit (`label` : sa qualité, nulle tant que Kyber n'en sait pas la taille)
    /// et l'avis bref de la Station, s'il y en a un.
    ShareImage { label: Option<String>, notice: Option<String> },
    /// KyberNow Share, invité : le partage a fini la session (`kind` : stopped, removed, unreadable) avec ce texte.
    ShareEnd(String, String),
    /// Par où passe la session : au départ, puis quand le chemin change (tuile « Par le relais », ligne « Chemin »).
    Route(crate::app::RouteUi),
}

struct Video {
    renderer: *mut sdl::SDL_Renderer,
    ui: *mut sdl::SDL_Texture,
    cursor: *mut sdl::SDL_Texture,
    cursor_size: (u32, u32, u32, u32),
}

/// État de la session en cours (le cœur, le pointeur de l'ordinateur).
struct Live {
    handle: session::Handle,
    /// Une session d'invité (KyberNow Share) : rien n'en est gardé (ni dernière session, ni aperçu), l'invité
    /// n'est jamais dans la liste des ordinateurs.
    guest: bool,
    pointer: Option<(f64, f64)>,
    pointer_hidden: bool,
    started: Instant,
    latencies: Vec<f64>,
    mbps: Vec<f64>,
    /// Mode de l'écran de la session : définition et fréquence.
    mode: (u32, u32, u32),
    /// Dernière position du pointeur de la Magic Remote (souris).
    remote: Option<(f32, f32)>,
    game_fullscreen: bool,
    /// Par où passe la session (`Platform::on_route`) : gardé pour la dernière session.
    path: crate::app::PathKind,
}

pub const UI_W: i32 = 1920;
pub const UI_H: i32 = 1080;

fn device_info(store: &Store, tv: &crate::app::TvInfo) -> DeviceInfo {
    DeviceInfo {
        id: store.device_id.clone(),
        name: format!("LG {}", tv.model),
        kind: "tv".into(),
        os: if tv.webos.is_empty() { "webOS".into() } else { format!("webOS {}", tv.webos) },
        app: format!("KyberNow Link {} (webOS)", env!("CARGO_PKG_VERSION")),
    }
}

pub fn run() {
    log_file::init();
    log::info!("KyberNow Link {} (webOS) démarre", env!("CARGO_PKG_VERSION"));

    // Réglages de SDL-webOS : la touche Retour et l'appui long (Quitter) vont
    // à l'app ; les manettes restent des manettes (pas des flèches).
    sdl::hint("SDL_WEBOS_ACCESS_POLICY_KEYS_BACK", "true");
    sdl::hint("SDL_WEBOS_ACCESS_POLICY_KEYS_EXIT", "true");
    sdl::hint("SDL_WEBOS_CLOUDGAME_ACTIVE", "true");
    sdl::hint("SDL_WEBOS_CURSOR_SLEEP_TIME", "5000");
    sdl::hint("SDL_RENDER_SCALE_QUALITY", "1");
    unsafe {
        if sdl::SDL_Init(sdl::SDL_INIT_VIDEO | sdl::SDL_INIT_AUDIO | sdl::SDL_INIT_GAMECONTROLLER | sdl::SDL_INIT_EVENTS) != 0 {
            log::error!("SDL : {}", sdl::error());
            return;
        }
    }
    let title = CString::new("KyberNow Link").unwrap();
    let window = unsafe {
        sdl::SDL_CreateWindow(title.as_ptr(), sdl::SDL_WINDOWPOS_UNDEFINED, sdl::SDL_WINDOWPOS_UNDEFINED, UI_W, UI_H, sdl::SDL_WINDOW_SHOWN | sdl::SDL_WINDOW_FULLSCREEN)
    };
    if window.is_null() {
        log::error!("SDL : fenêtre impossible ({})", sdl::error());
        return;
    }
    let renderer = unsafe { sdl::SDL_CreateRenderer(window, -1, sdl::SDL_RENDERER_ACCELERATED | sdl::SDL_RENDERER_PRESENTVSYNC) };
    if renderer.is_null() {
        log::error!("SDL : rendu impossible ({})", sdl::error());
        return;
    }
    unsafe {
        sdl::SDL_RenderSetLogicalSize(renderer, UI_W, UI_H);
        sdl::SDL_SetRenderDrawBlendMode(renderer, sdl::SDL_BLENDMODE_NONE);
    }
    let ui_tex = unsafe { sdl::SDL_CreateTexture(renderer, sdl::SDL_PIXELFORMAT_ABGR8888, sdl::SDL_TEXTUREACCESS_STREAMING, UI_W, UI_H) };
    unsafe { sdl::SDL_SetTextureBlendMode(ui_tex, sdl::SDL_BLENDMODE_NONE) };
    let mut video = Video { renderer, ui: ui_tex, cursor: std::ptr::null_mut(), cursor_size: (0, 0, 0, 0) };

    // L'app.
    let store = Store::load();
    i18n::set_english(i18n::resolve(&store.app.language, &sdl::locale()));
    kybernow_core::i18n::set_current(if i18n::english() { kybernow_core::i18n::Lang::En } else { kybernow_core::i18n::Lang::Fr });
    let mut app = App::new(store);
    let (panel, refresh) = sdl::panel();
    let decoder = ndl::available();
    app.tv = system::tv(panel, refresh, decoder);
    app.net = system::net();
    for h in app.store.hosts.clone() {
        if let Some(img) = std::fs::read(Store::image_path(&h.id)).ok().and_then(|b| ui::canvas::decode_jpeg(&b)) {
            app.images.insert(h.id.clone(), img);
        }
    }
    app.go(Screen::Home);

    let (tx, rx) = channel::<Event>();
    jobs::discover(tx.clone());
    let mut last_update = Instant::now();
    app.update = crate::app::UpdateState::Checking;
    jobs::check_update(app.store.app.beta, tx.clone());
    let mut pads = pads::Pads::default();
    let mut cv = Canvas::new();
    let assets = Assets::load();
    let mut live: Option<Live> = None;
    let mut pairing: Vec<Arc<AtomicBool>> = Vec::new();
    let mut last_poll = Instant::now() - Duration::from_secs(60);
    let mut last_fast_poll = Instant::now();
    let mut last_net = Instant::now();
    let mut last_multichannel = Instant::now() - Duration::from_secs(60);
    let mut dirty = true;
    let mut present = true;
    let mut ui_shown = true;
    let mut cursor_shown = true;
    let mut quit = false;

    while !quit {
        // Animations : l'arrivée en session, les capsules, les vibrations d'essai.
        let animating = app.session.as_ref().is_some_and(|s| matches!(s.phase, Phase::Preparing | Phase::Connecting | Phase::StationRestarting | Phase::FirstImages))
            || app.toast.as_ref().is_some_and(|(_, t)| t.elapsed().as_secs_f32() < 4.2)
            || app.session.as_ref().and_then(|s| s.notice.as_ref()).is_some_and(|(_, t)| t.elapsed().as_secs_f32() < 4.2)
            || (app.screen == Screen::Gamepads)
            || app.screen == Screen::Join
            || app.session.as_ref().is_some_and(|s| s.share_busy)
            || pads.mouse_active();
        let timeout = if animating { 16 } else { 50 };
        let mut ev = sdl::SDL_Event::default();
        let mut actions: Vec<Action> = Vec::new();
        if unsafe { sdl::SDL_WaitEventTimeout(&mut ev, timeout) } != 0 {
            loop {
                handle_sdl(&ev, &mut app, &mut pads, live.as_mut(), &mut actions, &mut dirty, &mut present, &mut quit);
                if unsafe { sdl::SDL_PollEvent(&mut ev) } == 0 {
                    break;
                }
            }
        }
        // Manette en souris : déplacement continu.
        if let Some(l) = live.as_ref()
            && app.session.as_ref().is_some_and(|s| s.pad_mouse && !s.menu)
            && pads.mouse_step(&l.handle.shared)
        {
            present = true;
        }
        // Flèches répétées au stick (menus).
        if let Some(n) = pads.repeat_nav() {
            actions.extend(app.nav(n));
            dirty = true;
        }

        while let Ok(e) = rx.try_recv() {
            // Le pointeur de l'ordinateur bouge : la fenêtre seule est recomposée.
            if matches!(e, Event::Pointer { .. }) {
                present = true;
            } else {
                dirty = true;
            }
            handle_event(e, &mut app, &mut pads, &mut live, &mut video, &mut actions, &mut pairing, &tx);
        }

        for a in actions {
            perform(a, &mut app, &mut pads, &mut live, &tx, &mut pairing, &mut quit);
            dirty = true;
        }

        // Fin de session : l'accueil, la dernière session enregistrée.
        if live.is_some() && matches!(app.session.as_ref().map(|s| &s.phase), Some(Phase::Ended { .. }) | None) {
            let l = live.take().unwrap();
            finish(&mut app, l);
            last_poll = Instant::now() - Duration::from_secs(60);
            dirty = true;
        }

        // Sondage d'Admin toutes les 8 s, hors session ; le réseau toutes les 5 s.
        if live.is_none() && last_poll.elapsed() > Duration::from_secs(8) && matches!(app.screen, Screen::Home | Screen::Settings) {
            last_poll = Instant::now();
            for h in &app.store.hosts {
                jobs::poll(h.clone(), !app.images.contains_key(&h.id), tx.clone());
            }
        }
        // La Station redémarre ou se met à jour, ou l'ordinateur se réveille :
        // sondé toutes les 2 s jusqu'à son retour.
        if live.is_none() && last_fast_poll.elapsed() > Duration::from_secs(2) {
            last_fast_poll = Instant::now();
            for id in app.fast_poll() {
                if let Some(h) = app.store.host(&id) {
                    jobs::poll(h.clone(), false, tx.clone());
                }
            }
        }
        // Sortie multicanal du téléviseur (réglages de son, ampli) : relue
        // toutes les 2 s pendant que la rubrique Son est ouverte (aussi
        // depuis le menu de session).
        if app.screen == Screen::Settings
            && !app.settings_global
            && app.current_section() == crate::app::Section::Sound
            && last_multichannel.elapsed() > Duration::from_secs(2)
        {
            last_multichannel = Instant::now();
            let m = match ndl::multichannel() {
                ndl::Multichannel::Unknown => 0,
                ndl::Multichannel::Unsupported => 1,
                ndl::Multichannel::NoDevice => 2,
                ndl::Multichannel::NotPassthrough => 3,
                ndl::Multichannel::Ready => 4,
            };
            if m != app.tv.multichannel {
                log::info!("son : sortie multicanal du téléviseur {:?}", ndl::multichannel());
                app.tv.multichannel = m;
                dirty = true;
            }
        }
        // Mises à jour : au lancement, puis toutes les 6 heures (hors session).
        if live.is_none() && last_update.elapsed() > Duration::from_secs(6 * 3600) {
            last_update = Instant::now();
            jobs::check_update(app.store.app.beta, tx.clone());
        }
        if last_net.elapsed() > Duration::from_secs(5) && live.is_none() {
            last_net = Instant::now();
            let n = system::net();
            if n.kind != app.net.kind || n.link_mbps != app.net.link_mbps {
                app.net = n;
                dirty = true;
            }
        }

        if animating {
            dirty = true;
        }
        let show_ui = app.screen != Screen::Session || ui::session_overlay_visible(&app);
        if dirty {
            if show_ui {
                app.pads = pads.infos();
                app.hits = ui::draw(&app, &mut cv, &assets);
                unsafe {
                    sdl::SDL_UpdateTexture(video.ui, std::ptr::null(), cv.bytes().as_ptr() as *const _, UI_W * 4);
                }
            } else {
                app.hits.clear();
            }
            dirty = false;
            present = true;
        }
        if show_ui != ui_shown {
            ui_shown = show_ui;
            present = true;
        }
        // Le pointeur de la Magic Remote : caché pendant le jeu (sauf en souris,
        // où l'app dessine celui de l'ordinateur), montré ailleurs.
        let in_game = app.screen == Screen::Session && app.session.as_ref().is_some_and(|s| !s.menu && !s.keys && matches!(s.phase, Phase::Streaming));
        if cursor_shown == in_game {
            cursor_shown = !in_game;
            sdl::system_cursor(cursor_shown);
        }
        if present {
            draw_frame(&video, show_ui, live.as_ref(), &app);
            present = false;
        }
    }

    if let Some(l) = live.take() {
        l.handle.session.stop();
    }
    for c in pairing.drain(..) {
        c.store(true, Ordering::Relaxed);
    }
    app.store.save();
    esplayer::quit();
    ndl::quit();
    unsafe { sdl::SDL_Quit() };
    log::info!("KyberNow Link s'arrête");
}

/// Compose la fenêtre : transparent (le plan vidéo dessous), l'interface si
/// elle est montrée, le pointeur de l'ordinateur pendant la session.
fn draw_frame(video: &Video, show_ui: bool, live: Option<&Live>, app: &App) {
    unsafe {
        sdl::SDL_SetRenderDrawColor(video.renderer, 0, 0, 0, 0);
        sdl::SDL_RenderClear(video.renderer);
        if show_ui {
            sdl::SDL_RenderCopy(video.renderer, video.ui, std::ptr::null(), std::ptr::null());
        }
        // Manette ou Magic Remote en souris : le pointeur est toujours dessiné.
        // Sans souris branchée, Windows cache le sien par intermittence
        // (annonces « caché » / « visible » plusieurs fois par seconde),
        // comme sur Android.
        let forced = app.session.as_ref().is_some_and(|s| s.remote_mouse || s.pad_mouse);
        if let Some(l) = live
            && !video.cursor.is_null()
            && (!l.pointer_hidden || forced)
            && app.screen == Screen::Session
            && app.session.as_ref().is_some_and(|s| !s.menu && matches!(s.phase, Phase::Streaming))
            && let Some((x, y)) = l.pointer
        {
            let (hw, hh) = l.handle.shared.host_size();
            let (cw, ch, hx, hy) = video.cursor_size;
            let sx = UI_W as f64 / hw.max(1) as f64;
            let sy = UI_H as f64 / hh.max(1) as f64;
            // Le pointeur à sa taille sur l'écran de l'ordinateur, ramené à 1080p.
            let dst = sdl::SDL_Rect {
                x: ((x - hx as f64) * sx) as i32,
                y: ((y - hy as f64) * sy) as i32,
                w: ((cw as f64 * sx).max(12.0)) as i32,
                h: ((ch as f64 * sy).max(12.0)) as i32,
            };
            sdl::SDL_RenderCopy(video.renderer, video.cursor, std::ptr::null(), &dst);
        }
        sdl::SDL_RenderPresent(video.renderer);
    }
}

#[allow(clippy::too_many_arguments)]
fn handle_sdl(ev: &sdl::SDL_Event, app: &mut App, pads: &mut pads::Pads, live: Option<&mut Live>, actions: &mut Vec<Action>, dirty: &mut bool, present: &mut bool, quit: &mut bool) {
    let in_game = app.screen == Screen::Session && app.session.as_ref().is_some_and(|s| !s.menu && !s.keys && !s.share_panel && matches!(s.phase, Phase::Streaming | Phase::Reconnecting { .. }));
    let input = live.as_ref().and_then(|l| l.handle.shared.current_input());
    match ev.kind() {
        sdl::SDL_QUIT => *quit = true,
        sdl::SDL_APP_WILLENTERBACKGROUND => {
            if let Some(p) = &input {
                p.release_all();
            }
        }
        sdl::SDL_KEYDOWN | sdl::SDL_KEYUP => {
            let down = ev.kind() == sdl::SDL_KEYDOWN;
            let (sc, repeat) = ev.key();
            // Touches propres à la télécommande.
            let remote_back = sc == sdl::SC_WEBOS_BACK || sc == sdl::SC_AC_BACK;
            if in_game {
                let s = app.session.as_ref().unwrap();
                // Un invité n'a que le menu à ouvrir : Retour l'ouvre toujours.
                let back_menu = s.guest.is_some() || app.selected().map(|h| h.settings.back_opens_menu).unwrap_or(true);
                if (remote_back && back_menu) || sc == sdl::SC_WEBOS_EXIT || sc == sdl::SC_WEBOS_RED {
                    if down && !repeat {
                        actions.extend(app.open_menu());
                    }
                    *dirty = true;
                    return;
                }
                let Some(p) = input else { return };
                if s.remote_mouse && (sc == sdl::SC_RETURN || sc == sdl::SC_KP_ENTER) {
                    p.mouse_button(MouseButtonType::Left, down);
                    return;
                }
                if s.remote_mouse && sc == sdl::SC_WEBOS_YELLOW {
                    p.mouse_button(MouseButtonType::Right, down);
                    return;
                }
                let sc = if remote_back { sdl::SC_ESCAPE } else { sc };
                if !repeat && let Some(win) = pads::win_scancode(sc) {
                    log::debug!("clavier : touche SDL {sc} (Windows {win:#x}) {} vers l'ordinateur", if down { "appuyée" } else { "relâchée" });
                    p.key(win, down);
                }
                return;
            }
            if !down {
                return;
            }
            let nav = match sc {
                sdl::SC_UP => Some(Nav::Up),
                sdl::SC_DOWN => Some(Nav::Down),
                sdl::SC_LEFT => Some(Nav::Left),
                sdl::SC_RIGHT => Some(Nav::Right),
                sdl::SC_RETURN | sdl::SC_KP_ENTER => Some(Nav::Ok),
                sdl::SC_ESCAPE | sdl::SC_WEBOS_BACK | sdl::SC_AC_BACK => Some(Nav::Back),
                sdl::SC_WEBOS_EXIT => Some(Nav::Back),
                sdl::SC_BACKSPACE => Some(Nav::Erase),
                sdl::SC_WEBOS_YELLOW | sdl::SC_WEBOS_BLUE => Some(Nav::Options),
                // La touche verte : « Rejoindre avec un code » (accueil).
                sdl::SC_WEBOS_GREEN => Some(Nav::Join),
                sdl::SC_WEBOS_CURSOR_HIDE => {
                    app.pointer = None;
                    *dirty = true;
                    None
                }
                _ => None,
            };
            if let Some(n) = nav {
                actions.extend(app.nav(n));
                *dirty = true;
            }
        }
        sdl::SDL_TEXTINPUT => {
            if in_game {
                return;
            }
            for c in ev.text().chars() {
                // « Rejoindre un partage » : les chiffres du code, les lettres du prénom (accents compris).
                if app.screen == Screen::Join || c.is_ascii_digit() || c == '.' || c == ':' || c.is_ascii_alphabetic() || c == '-' {
                    actions.extend(app.nav(Nav::Char(c)));
                }
            }
            *dirty = true;
        }
        sdl::SDL_MOUSEMOTION => {
            let (x, y, _, _) = ev.motion();
            if in_game {
                if let Some(l) = live
                    && app.session.as_ref().is_some_and(|s| s.remote_mouse)
                {
                    let (hw, hh) = l.handle.shared.host_size();
                    let tx = x as f64 / UI_W as f64 * hw as f64;
                    let ty = y as f64 / UI_H as f64 * hh as f64;
                    let ptr = &l.handle.shared.pointer;
                    let host = ptr.host_size();
                    let (cx, cy) = ptr.state().position().unwrap_or((host.0 / 2.0, host.1 / 2.0));
                    ptr.move_by(tx - cx, ty - cy);
                    l.remote = Some((x as f32, y as f32));
                    *present = true;
                }
                return;
            }
            if app.pointer_moved(x as f32, y as f32) {
                *dirty = true;
            }
        }
        sdl::SDL_MOUSEBUTTONDOWN | sdl::SDL_MOUSEBUTTONUP => {
            let down = ev.kind() == sdl::SDL_MOUSEBUTTONDOWN;
            let (b, x, y) = ev.button();
            if in_game {
                if app.session.as_ref().is_some_and(|s| s.remote_mouse)
                    && let Some(p) = input
                {
                    let button = match b {
                        3 => MouseButtonType::Right,
                        2 => MouseButtonType::Middle,
                        _ => MouseButtonType::Left,
                    };
                    p.mouse_button(button, down);
                }
                return;
            }
            if down && b == 1 {
                actions.extend(app.pointer_click(x as f32, y as f32));
                *dirty = true;
            }
        }
        sdl::SDL_MOUSEWHEEL => {
            let (wx, wy) = ev.wheel();
            if in_game {
                if app.session.as_ref().is_some_and(|s| s.remote_mouse)
                    && let Some(p) = input
                {
                    p.wheel(wx as f32, -wy as f32);
                }
                return;
            }
            if wy != 0 {
                actions.extend(app.nav(if wy > 0 { Nav::Up } else { Nav::Down }));
                *dirty = true;
            }
        }
        sdl::SDL_CONTROLLERDEVICEADDED => {
            let (index, _, _) = ev.pad();
            if let Some(slot) = pads.add(index) {
                if let Some(p) = &input {
                    let (v, pr) = pads.ids(slot);
                    p.gamepad_plug(slot as u32, Some(v), Some(pr));
                }
                if let Some(s) = app.session.as_mut() {
                    s.notice = Some((format!("{} {}{} : {}", tr("Manette", "Controller"), tr("J", "P"), slot + 1, pads.name(slot)), Instant::now()));
                }
                *dirty = true;
            }
        }
        sdl::SDL_CONTROLLERDEVICEREMOVED => {
            let (instance, _, _) = ev.pad();
            if let Some(slot) = pads.remove(instance) {
                if let Some(p) = &input {
                    p.gamepad_unplug(slot as u32);
                }
                *dirty = true;
            }
        }
        sdl::SDL_CONTROLLERBUTTONDOWN | sdl::SDL_CONTROLLERBUTTONUP | sdl::SDL_CONTROLLERAXISMOTION => {
            let (instance, which, value) = ev.pad();
            let Some(slot) = pads.slot_of(instance) else { return };
            let is_button = ev.kind() != sdl::SDL_CONTROLLERAXISMOTION;
            let down = ev.kind() == sdl::SDL_CONTROLLERBUTTONDOWN;
            pads.update(slot, is_button, which, down, value);
            if app.screen == Screen::Gamepads {
                *dirty = true;
            }
            let s = app.session.as_ref();
            // Un invité sans les manettes (KyberNow Share : spectateur, ou clavier et souris seuls) n'envoie aucune
            // touche de manette, donc la combinaison du menu ne se forme pas : Start, Select ou B l'ouvrent. En
            // manette-souris (clavier et souris donnés), B est le clic droit : Start et Select seulement.
            if in_game
                && is_button
                && down
                && let Some((true, mouse)) = s.and_then(|s| s.role().map(|r| (!r.pads(), s.pad_mouse)))
                && (matches!(which, 4 | 6) || (which == 1 && !mouse))
            {
                actions.extend(app.open_menu());
                *dirty = true;
                return;
            }
            if in_game && s.is_some_and(|s| s.pad_mouse) {
                if let Some(p) = &input {
                    pads.mouse_input(p, is_button, which, down, value);
                }
                // Le combo reste reconnu par le cœur : les boutons vont aussi au jeu.
                if is_button && let Some(p) = &input {
                    p.gamepad_button(slot as u32, pads::button(which), down);
                }
                return;
            }
            if in_game {
                if let Some(p) = &input {
                    if is_button {
                        if let Some(b) = pads::core_button(which) {
                            log::debug!("manettes : J{} {b:?} {} vers l'ordinateur", slot + 1, if down { "appuyé" } else { "relâché" });
                            p.gamepad_button(slot as u32, b, down);
                        }
                    } else if let Some(a) = pads::core_axis(which) {
                        p.gamepad_axis(slot as u32, a, value);
                    }
                }
                return;
            }
            // Hors jeu (menu ouvert, connexion) : les relâchements vont quand même
            // au suivi de la combinaison du cœur. Sans cela, les quatre touches
            // lâchées dans le menu qu'elles ont ouvert restaient « tenues » pour
            // lui, et un simple appui long sur L1 ou R1 en jeu le rouvrait.
            if is_button
                && !down
                && let Some(p) = &input
                && let Some(b) = pads::core_button(which)
            {
                p.combo().button(slot as u32, b, false);
            }
            // Hors jeu : la manette pilote l'app.
            if let Some(n) = pads.nav(is_button, which, down, value) {
                actions.extend(app.nav(n));
                *dirty = true;
            }
        }
        _ => {}
    }
}

#[allow(clippy::too_many_arguments)]
fn handle_event(e: Event, app: &mut App, pads: &mut pads::Pads, live: &mut Option<Live>, video: &mut Video, actions: &mut Vec<Action>, pairing: &mut Vec<Arc<AtomicBool>>, tx: &Sender<Event>) {
    match e {
        Event::Polled { id, polled } => {
            if app.polled(&id, polled) {
                app.store.save();
            }
            // « Partager » attendait cette réponse : le partage s'ouvre, ou le panneau dit pourquoi il ne le peut pas.
            actions.extend(app.share_known(&id));
        }
        Event::Station { id, op, result } => app.station_answer(&id, op, result),
        Event::Preview { id, jpeg } => {
            if let Some(img) = ui::canvas::decode_jpeg(&jpeg) {
                let _ = std::fs::write(Store::image_path(&id), &jpeg);
                app.images.insert(id, img);
            }
        }
        Event::Found(list) => {
            app.found = list;
            // Ajout par code en cours : le code va aussi aux Stations apparues depuis.
            if app.screen == Screen::Add
                && app.add.step == AddStep::Code
                && !app.add.code.is_empty()
                && app.new_found().iter().any(|f| !app.add.asked.iter().any(|(_, a)| *a == f.address))
            {
                actions.push(Action::StartPairing);
            }
        }
        Event::Probed { address, port, fingerprint } => {
            app.add.busy = false;
            match fingerprint {
                Ok(fp) => {
                    app.add.step = AddStep::Verify { address, port, fingerprint: fp };
                    app.add.message = None;
                    app.go(Screen::Add);
                }
                Err(e) => app.add.message = Some(e),
            }
        }
        Event::PairAsked { name, .. } => {
            app.add.message = Some(format!("{} {name} : {}", tr("Code présenté à", "Code shown to"), tr("acceptez-le dans Admin", "accept it in Admin")));
        }
        Event::PairAccepted(host) => {
            for c in pairing.drain(..) {
                c.store(true, Ordering::Relaxed);
            }
            let name = host.name.clone();
            let id = app.store.put(host);
            app.store.selected = Some(id);
            app.store.save();
            app.toast(format!("{name} {}", tr("est ajouté", "added")));
            app.go(Screen::Home);
        }
        Event::PairFailed(msg) => app.add.message = Some(msg),
        Event::Status(st) => {
            let Some(s) = app.session.as_mut() else { return };
            s.phase = match st {
                Status::Preparing => Phase::Preparing,
                // La Station redémarre (dernier recours) : « Connexion » en attendant de le montrer.
                Status::Connecting => Phase::Connecting,
                Status::StationRestarting => Phase::StationRestarting,
                Status::FirstImages => Phase::FirstImages,
                Status::Streaming => {
                    if s.phase != Phase::Streaming && app.screen == Screen::Session {
                        app.focus = crate::app::Target::Menu(crate::app::MenuItem::Resume);
                    }
                    s.streamed = true;
                    Phase::Streaming
                }
                Status::Reconnecting { attempt, max } => Phase::Reconnecting { attempt, max },
                Status::Ending => Phase::Ending,
                Status::Ended { error, .. } => {
                    if let Some(l) = live.as_ref()
                        && let Some(p) = l.handle.shared.current_input()
                    {
                        p.release_all();
                    }
                    // Par le relais, l'échec dit le nom de l'ordinateur (docs/link-ui.md, « Hors de chez vous »).
                    let may_relay = s.guest.is_none() && app.store.host(&s.host).is_some_and(|h| session::relay_route(h).is_some());
                    let error = match error {
                        Some(e) if may_relay => Some(session::relay_refusal(&e, &s.name).unwrap_or(e)),
                        other => other,
                    };
                    Phase::Ended { error }
                }
            };
            if let Phase::Ended { error: None } = s.phase {
                // Fin voulue : retour direct à l'accueil.
                app.session = None;
                app.go(Screen::Home);
            } else if let Phase::Ended { .. } = s.phase {
                app.screen = Screen::Session;
                app.focus = crate::app::Target::Back;
            }
        }
        Event::Mode(m) => {
            if let Some(l) = live.as_mut() {
                let refresh = if m.refresh > 0 { m.refresh } else { l.mode.2 };
                l.mode = (m.width, m.height, refresh);
            }
        }
        Event::Second { latency_ms, fps, mbps, lost, codec } => {
            if let Some(l) = live.as_mut() {
                if let Some(v) = latency_ms {
                    l.latencies.push(v);
                }
                if fps > 0.0 {
                    l.mbps.push(mbps);
                }
                if let Some(s) = app.session.as_mut() {
                    s.stats = Some(Stats { latency_ms, fps, mbps, lost, mode: format!("{} × {}", l.mode.0, l.mode.1), codec });
                }
            }
        }
        Event::Notice(n) => {
            if let Some(s) = app.session.as_mut() {
                s.notice = Some((n, Instant::now()));
            }
        }
        Event::Pointer { position, hidden, shape } => {
            if let Some(l) = live.as_mut() {
                l.pointer = position;
                l.pointer_hidden = hidden;
            }
            if let Some((w, h, hx, hy, px)) = shape
                && w > 0
                && h > 0
            {
                unsafe {
                    if !video.cursor.is_null() {
                        sdl::SDL_DestroyTexture(video.cursor);
                    }
                    video.cursor = sdl::SDL_CreateTexture(video.renderer, sdl::SDL_PIXELFORMAT_ARGB8888, sdl::SDL_TEXTUREACCESS_STREAMING, w as i32, h as i32);
                    sdl::SDL_SetTextureBlendMode(video.cursor, sdl::SDL_BLENDMODE_BLEND);
                    sdl::SDL_UpdateTexture(video.cursor, std::ptr::null(), px.as_ptr() as *const _, (w * 4) as i32);
                }
                video.cursor_size = (w, h, hx, hy);
            }
        }
        Event::Rumble(r) => pads.rumble(r.id as usize, r.large_motor, r.small_motor, r.duration_ms),
        Event::Combo => actions.extend(app.open_menu()),
        Event::InputReady => {
            if let Some(l) = live.as_ref()
                && let Some(p) = l.handle.shared.current_input()
            {
                if !pads.slots().is_empty() {
                    log::info!("manettes : {} annoncée(s) à l'ordinateur", pads.slots().len());
                }
                for slot in pads.slots() {
                    let (v, pr) = pads.ids(slot);
                    p.gamepad_plug(slot as u32, Some(v), Some(pr));
                }
            }
        }
        Event::GameFullscreen(g) => {
            if let Some(l) = live.as_mut() {
                l.game_fullscreen = g;
            }
        }
        Event::DecoderFailed(e) => {
            if let Some(s) = app.session.as_mut() {
                s.notice = Some((format!("{} ({e})", tr("Décodeur du téléviseur indisponible", "TV decoder unavailable")), Instant::now()));
            }
        }
        // KyberNow Share, invité : un ordinateur a pris le code. La session s'ouvre si la recherche est encore celle
        // de l'écran ; sinon (retour, autre recherche) l'invité repart de la Station tout de suite.
        Event::ShareJoined { attempt, result } => match result {
            Ok(found) if app.join_current(attempt) => {
                app.join.busy = false;
                app.join.waiting = false;
                connect_guest(app, live, tx, found);
            }
            Ok(found) => jobs::share_leave(found),
            Err(e) => {
                log::info!("partage : {e}");
                app.join_failed(attempt, e.to_string());
            }
        },
        // La demande attend l'hôte : « En attente de l'accord de l'hôte… ». Une recherche abandonnée entre-temps
        // retire sa demande aussitôt.
        Event::SharePending { attempt } => {
            if app.join_current(attempt) {
                app.join_pending(attempt);
            } else {
                jobs::share_cancel();
            }
        }
        Event::ShareHost(result) => app.share_reply(result),
        Event::Guests(view) => app.share_guests(view),
        Event::ShareRole { role, previous } => {
            app.share_role_changed(role, previous);
            // Un rôle sans clavier ni souris rend la manette en souris.
            if !role.kbm() {
                pads.set_mouse(false);
            }
        }
        Event::ShareImage { label, notice } => app.share_image(label, notice),
        Event::ShareEnd(kind, text) => log::info!("partage : {kind} ({text})"),
        // Le chemin de la session : la tuile et la ligne « Chemin » le lisent ; la dernière session le garde.
        Event::Route(route) => {
            if let Some(l) = live.as_mut() {
                l.path = route.kind;
            }
            if let Some(s) = app.session.as_mut() {
                s.route = Some(route);
            }
        }
        Event::Update(r) => {
            app.update = match r {
                Ok(Some(a)) => {
                    log::info!("mise à jour : version {} ({}) disponible, {}", a.version, a.channel.as_str(), a.page);
                    crate::app::UpdateState::Available { version: a.version, channel: a.channel.as_str().into(), page: a.page }
                }
                Ok(None) => {
                    log::info!("mise à jour : à jour ({})", app.version);
                    crate::app::UpdateState::UpToDate
                }
                Err(e) => {
                    log::warn!("mise à jour : {e}");
                    crate::app::UpdateState::Failed(e)
                }
            };
        }
    }
}

fn perform(a: Action, app: &mut App, pads: &mut pads::Pads, live: &mut Option<Live>, tx: &Sender<Event>, pairing: &mut Vec<Arc<AtomicBool>>, quit: &mut bool) {
    match a {
        Action::Save => app.store.save(),
        Action::Quit => *quit = true,
        Action::Language => {
            i18n::set_english(i18n::resolve(&app.store.app.language, &sdl::locale()));
            kybernow_core::i18n::set_current(if i18n::english() { kybernow_core::i18n::Lang::En } else { kybernow_core::i18n::Lang::Fr });
        }
        Action::CheckUpdate => {
            app.update = crate::app::UpdateState::Checking;
            jobs::check_update(app.store.app.beta, tx.clone());
        }
        Action::Connect { host, profile } => {
            if live.is_some() {
                return;
            }
            let Some(h) = app.store.host(&host).cloned() else { return };
            if !app.tv.decoder {
                app.toast(tr("Décodeur du téléviseur introuvable : webOS 5 (2020) au moins", "TV decoder not found: webOS 5 (2020) or later required"));
                return;
            }
            let mode = app.session_mode(&h);
            let codec = app.codec_for(&h);
            let device = device_info(&app.store, &app.tv);
            let handle = session::start(&h, profile, codec, mode, app.tv.refresh, app.net.cap_mbps(), device, None, tx.clone());
            app.session = Some(SessionUi::new(&h.id, &h.name, profile, h.settings.game_mode_hint));
            app.screen = Screen::Session;
            app.focus = crate::app::Target::CancelConnect;
            // « Partager » dans le menu sait ce que l'ordinateur permet : Admin est relu une fois la session partie.
            jobs::poll(h.clone(), false, tx.clone());
            *live = Some(Live {
                handle,
                guest: false,
                pointer: None,
                pointer_hidden: true,
                started: Instant::now(),
                latencies: Vec::new(),
                mbps: Vec::new(),
                mode,
                remote: None,
                game_fullscreen: false,
                path: crate::app::PathKind::Direct,
            });
            log::info!("session : {} ({}, {} × {} à {} Hz, {codec})", h.name, profile.id(), mode.0, mode.1, mode.2);
        }
        Action::CancelConnect | Action::Disconnect => {
            if let Some(l) = live.as_ref() {
                if let Some(p) = l.handle.shared.current_input() {
                    p.release_all();
                }
                l.handle.session.stop();
            }
            if let Some(s) = app.session.as_mut() {
                s.menu = false;
                s.phase = Phase::Ending;
            }
        }
        Action::SetProfile(p) => {
            if let (Some(l), Some(h)) = (live.as_ref(), app.session.as_ref().and_then(|s| app.store.host(&s.host))) {
                let mode = app.session_mode(h);
                let codec = app.codec_for(h);
                l.handle.session.set_settings(session::settings(h, p, codec, mode, app.net.cap_mbps()));
            }
        }
        Action::RemoteMouse(_) => {
            if let Some(l) = live.as_ref() {
                l.handle.shared.pointer.resend_held();
            }
        }
        Action::PadMouse(on) => pads.set_mouse(on),
        Action::ReleaseAll => {
            if let Some(l) = live.as_ref()
                && let Some(p) = l.handle.shared.current_input()
            {
                p.release_all();
                for slot in pads.slots() {
                    p.gamepad_axes_rest(slot as u32);
                }
            }
        }
        Action::Shortcut(keys) => {
            if let Some(l) = live.as_ref()
                && let Some(p) = l.handle.shared.current_input()
            {
                for k in &keys {
                    p.key(*k, true);
                }
                for k in keys.iter().rev() {
                    p.key(*k, false);
                }
            }
        }
        Action::StartPairing => {
            // Le code va aux Stations trouvées à qui il n'a pas encore été présenté.
            let targets: Vec<jobs::Target> = app
                .new_found()
                .iter()
                .filter(|f| !app.add.asked.iter().any(|(_, a)| *a == f.address))
                .map(|f| jobs::Target { name: f.name.clone(), address: f.address.clone(), port: f.port, fingerprint: f.fingerprint.clone() })
                .collect();
            if targets.is_empty() {
                if !app.add.asked.is_empty() {
                    return;
                }
                app.add.message = Some(tr("Aucune Station trouvée pour l'instant : le code sera présenté dès qu'elle apparaît, ou ajoutez-la par adresse.", "No Station found yet: the code is shown as soon as one appears, or add it by address.").into());
                return;
            }
            app.add.asked.extend(targets.iter().map(|t| (t.name.clone(), t.address.clone())));
            let cancel = Arc::new(AtomicBool::new(false));
            jobs::pair(targets, app.add.code.clone(), device_info(&app.store, &app.tv), cancel.clone(), tx.clone());
            pairing.push(cancel);
        }
        Action::PairAddress { address, port, fingerprint } => {
            let cancel = Arc::new(AtomicBool::new(false));
            app.add.asked.push((address.clone(), address.clone()));
            let t = jobs::Target { name: address.clone(), address, port, fingerprint: Some(fingerprint) };
            jobs::pair(vec![t], app.add.code.clone(), device_info(&app.store, &app.tv), cancel.clone(), tx.clone());
            pairing.push(cancel);
        }
        Action::StopPairing => {
            for c in pairing.drain(..) {
                c.store(true, Ordering::Relaxed);
            }
        }
        Action::Probe { address, port } => jobs::probe(address, port, tx.clone()),
        // KyberNow Share, invité : le code part aux Stations trouvées sur le réseau (celles de l'ajout), avec
        // l'empreinte qu'on leur connaît ; ce téléviseur se nomme comme pour Admin.
        Action::JoinCancel => jobs::share_cancel(),
        Action::JoinShare { code, name, quality } => {
            let mut targets: Vec<ShareTarget> = app.found.iter().map(|f| ShareTarget { host: f.address.clone(), port: f.port, fingerprint: f.fingerprint.clone() }).collect();
            // Un ordinateur du téléviseur, en ligne, que le réseau n'annonce pas : il peut partager aussi.
            for h in &app.store.hosts {
                if app.live.get(&h.id).is_some_and(|l| l.online == Online::Online) && !targets.iter().any(|t| t.host == h.address && t.port == h.port) {
                    targets.push(ShareTarget { host: h.address.clone(), port: h.port, fingerprint: h.fingerprint.clone() });
                }
            }
            // « Auto » : la sortie du téléviseur et ce que son décodeur suit ; la Station en tire l'image de l'invité (jamais
            // au-dessus de celle de l'hôte). Envoyé quel que soit le choix : le cœur le garde pour un passage à « Auto »
            // en cours de session (sans lui, « Auto » donnerait l'image de l'hôte).
            let auto = Some(session::auto_quality(&app.tv));
            let request = JoinRequest { code, name, device: app.store.device_id.clone(), app: "webos".into(), quality: session::core_quality(quality), auto, ..JoinRequest::default() };
            // Hors de chez soi : le relais public, et ceux des ordinateurs du téléviseur (sans doublon, `KYBERNOW_RELAY=off` : aucun).
            let relays = session::join_relays(app.store.hosts.iter().filter_map(|h| h.relay.as_ref().map(|r| r.server.as_str())));
            jobs::share_join(targets, relays, request, session::guest_capabilities(app.tv.av1), app.join.attempt, tx.clone());
        }
        // KyberNow Share, invité : la « Qualité de l'image » changée en session ; la Station relance l'image de l'invité
        // seulement, la session n'a rien à relancer (l'image reçue revient par `Platform::on_share_image`).
        Action::ShareQuality(q) => {
            if let Some(l) = live.as_ref() {
                l.handle.session.set_share_quality(session::core_quality(q));
            }
        }
        Action::Share(op) => {
            let host = live.as_ref().and_then(|_| app.session.as_ref()).and_then(|s| app.store.host(&s.host)).cloned();
            match host {
                Some(h) => jobs::share_host(h, op, tx.clone()),
                None => app.share_reply(Err(tr("L'ordinateur n'est plus dans la liste de ce téléviseur.", "The computer is no longer in this TV's list.").into())),
            }
        }
        Action::Poll(id) => {
            if let Some(h) = app.store.host(&id) {
                jobs::poll(h.clone(), !app.images.contains_key(&id), tx.clone());
            }
        }
        Action::Station { host, op, force } => {
            if let Some(h) = app.store.host(&host) {
                log::info!("station : {} demandé à {}{}", if op == StationOp::Restart { "redémarrage" } else { "mise à jour" }, h.name, if force { " (quand même)" } else { "" });
                jobs::station(h.clone(), op, force, app.store.device_id.clone(), tx.clone());
            }
        }
        // « Réveil de l'ordinateur… » sur l'accueil (`App::waking`) ; un envoi
        // impossible le dit, et le bouton revient.
        Action::Wake(id) => {
            let Some(h) = app.store.host(&id) else { return };
            if let Err(e) = jobs::wake(h) {
                app.waking = None;
                app.toast(e);
            }
        }
        Action::Rumble { pad, low, high, triggers } => pads.test_rumble(pad, low, high, triggers),
    }
}

/// Le « ordinateur » d'une session d'invité : une enveloppe (nom, adresse de la Station qui partage, réglages par
/// défaut) que la session lit comme celle d'un ordinateur ; elle n'est jamais dans le magasin.
fn guest_host(found: &share::Found, title: &str) -> crate::model::Host {
    crate::model::Host {
        id: format!("share:{}", found.joined.guest),
        name: title.into(),
        address: found.target.host.clone(),
        port: found.target.port,
        fingerprint: found.target.fingerprint.clone(),
        user: String::new(),
        password: String::new(),
        os: None,
        gpu: None,
        av1: false,
        mac: None,
        wake: Vec::new(),
        relay: None,
        settings: crate::model::HostSettings::default(),
        last: None,
    }
}

/// Ouvre la session d'invité d'un partage (KyberNow Share) : l'écran du partage, l'image et le son de l'hôte,
/// arrivée en spectateur (l'hôte donne la suite). Sans décodeur, ou une session déjà là, l'invité repart de la
/// Station.
fn connect_guest(app: &mut App, live: &mut Option<Live>, tx: &Sender<Event>, found: share::Found) {
    if live.is_some() {
        jobs::share_leave(found);
        return;
    }
    if !app.tv.decoder {
        app.toast(tr("Décodeur du téléviseur introuvable : webOS 5 (2020) au moins", "TV decoder not found: webOS 5 (2020) or later required"));
        jobs::share_leave(found);
        return;
    }
    let computer = found.joined.computer.trim().to_string();
    let title = crate::app::guest_title(&computer);
    let h = guest_host(&found, &title);
    let d = found.joined.display;
    let mode = (d.width, d.height, d.refresh);
    let codec = app.codec_for(&h);
    let device = device_info(&app.store, &app.tv);
    let caps = session::guest_capabilities(app.tv.av1);
    let handle = session::start(&h, crate::model::Profile::Jeu, codec, mode, app.tv.refresh, app.net.cap_mbps(), device, Some((&found, caps)), tx.clone());
    let mut ui = SessionUi::guest(&h.id, &computer, session::role_of(found.joined.role));
    // La « Qualité de l'image » demandée à « Rejoindre », et l'image que la Station dit déjà (`on_share_image` la suit).
    if let Some(g) = ui.guest.as_mut() {
        g.quality = app.join.quality;
        g.image = found.joined.image.as_ref().and_then(|i| i.label());
    }
    app.session = Some(ui);
    app.screen = Screen::Session;
    app.focus = crate::app::Target::CancelConnect;
    *live = Some(Live { handle, guest: true, pointer: None, pointer_hidden: true, started: Instant::now(), latencies: Vec::new(), mbps: Vec::new(), mode, remote: None, game_fullscreen: false, path: crate::app::PathKind::Direct });
    log::info!("session d'invité : {title} ({} × {} à {} Hz)", mode.0, mode.1, mode.2);
}

/// Fin de session : la dernière session (au moins 3 s), pour Statistiques.
fn finish(app: &mut App, l: Live) {
    // Un invité ne laisse rien : ni dernière session, ni aperçu, ni ordinateur ajouté.
    if l.guest {
        log::info!("session d'invité : {} s", l.started.elapsed().as_secs());
        if app.session.as_ref().is_some_and(|s| !matches!(s.phase, Phase::Ended { error: Some(_) })) {
            app.session = None;
            if app.screen == Screen::Session {
                app.go(Screen::Home);
            }
        }
        return;
    }
    let host = app.session.as_ref().map(|s| s.host.clone());
    if app.session.as_ref().is_some_and(|s| !matches!(s.phase, Phase::Ended { error: Some(_) })) {
        app.session = None;
        if app.screen == Screen::Session {
            app.go(Screen::Home);
        }
    }
    let mut lat = l.latencies.clone();
    lat.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let seconds = l.started.elapsed().as_secs();
    if let (Some(id), false, true) = (host, lat.is_empty(), seconds >= 3) {
        let median = lat[lat.len() / 2];
        let p95 = lat[((lat.len() as f64 * 0.95) as usize).min(lat.len() - 1)];
        let mbps = if l.mbps.is_empty() { 0.0 } else { l.mbps.iter().sum::<f64>() / l.mbps.len() as f64 };
        let codec = l.handle.shared.codec();
        let (width, height, refresh) = l.mode;
        if let Some(h) = app.store.host_mut(&id) {
            h.last = Some(LastSession {
                median,
                p95,
                seconds,
                // Lu par la 0.4.x.
                mode: format!("{width} × {height} · {codec}"),
                mbps,
                at: now_secs(),
                width,
                height,
                refresh,
                codec,
                route: l.path.as_str().into(),
            });
        }
        app.store.save();
        // L'aperçu sera repris d'Admin (l'image de la session est sur le plan vidéo).
        app.images.remove(&id);
    }
}
