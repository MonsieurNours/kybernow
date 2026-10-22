//! L'app sur le téléviseur : la fenêtre SDL (plan graphique), la boucle des
//! événements (Magic Remote, clavier, souris, manettes), l'interface, les
//! tâches de fond et la session.
//!
//! Pendant la session, l'image de l'ordinateur est sur le plan vidéo (NDL) ;
//! la fenêtre est effacée en transparent et ne montre que le menu, les
//! capsules et le pointeur de l'ordinateur.

mod jobs;
mod log_file;
mod media;
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

use crate::app::{Action, AddStep, App, Found, HostLive, Nav, Online, Phase, Screen, SessionUi, Stats};
use crate::i18n::{self, tr};
use crate::model::{now_secs, LastSession, Store};
use crate::ui::{self, canvas::Canvas, Assets};

/// Ce qui revient à la boucle principale (fils de fond, rappels du cœur).
pub enum Event {
    HostInfo { id: String, online: bool, os: Option<String>, gpu: Option<String>, av1: bool },
    Preview { id: String, jpeg: Vec<u8> },
    Found(Vec<Found>),
    Probed { address: String, port: u16, fingerprint: Result<String, String> },
    PairAsked { name: String, address: String },
    PairAccepted(crate::model::Host),
    PairFailed(String),
    Status(Status),
    Mode(Mode),
    Second { latency_ms: Option<f64>, fps: f64, mbps: f64, lost: u32, codec: String },
    Notice(String),
    Pointer { position: Option<(f64, f64)>, hidden: bool, shape: Option<(u32, u32, u32, u32, Vec<u32>)> },
    Rumble(GamepadRumble),
    Combo,
    InputReady,
    GameFullscreen(bool),
    DecoderFailed(String),
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
    pointer: Option<(f64, f64)>,
    pointer_hidden: bool,
    started: Instant,
    latencies: Vec<f64>,
    mbps: Vec<f64>,
    mode: String,
    /// Dernière position du pointeur de la Magic Remote (souris).
    remote: Option<(f32, f32)>,
    game_fullscreen: bool,
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
    let mut pads = pads::Pads::default();
    let mut cv = Canvas::new();
    let assets = Assets::load();
    let mut live: Option<Live> = None;
    let mut pairing: Vec<Arc<AtomicBool>> = Vec::new();
    let mut last_poll = Instant::now() - Duration::from_secs(60);
    let mut last_net = Instant::now();
    let mut dirty = true;
    let mut present = true;
    let mut ui_shown = true;
    let mut cursor_shown = true;
    let mut quit = false;

    while !quit {
        // Animations : l'arrivée en session, les capsules, les vibrations d'essai.
        let animating = app.session.as_ref().is_some_and(|s| matches!(s.phase, Phase::Preparing | Phase::Connecting | Phase::FirstImages))
            || app.toast.as_ref().is_some_and(|(_, t)| t.elapsed().as_secs_f32() < 4.2)
            || app.session.as_ref().and_then(|s| s.notice.as_ref()).is_some_and(|(_, t)| t.elapsed().as_secs_f32() < 4.2)
            || (app.screen == Screen::Gamepads)
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
            handle_event(e, &mut app, &mut pads, &mut live, &mut video, &mut actions, &mut pairing);
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
        if let Some(l) = live
            && !video.cursor.is_null()
            && !l.pointer_hidden
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
    let in_game = app.screen == Screen::Session && app.session.as_ref().is_some_and(|s| !s.menu && !s.keys && matches!(s.phase, Phase::Streaming | Phase::Reconnecting { .. }));
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
                let back_menu = app.selected().map(|h| h.settings.back_opens_menu).unwrap_or(true);
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
                if c.is_ascii_digit() || c == '.' || c == ':' || c.is_ascii_alphabetic() || c == '-' {
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
                            p.gamepad_button(slot as u32, b, down);
                        }
                    } else if let Some(a) = pads::core_axis(which) {
                        p.gamepad_axis(slot as u32, a, value);
                    }
                }
                return;
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
fn handle_event(e: Event, app: &mut App, pads: &mut pads::Pads, live: &mut Option<Live>, video: &mut Video, actions: &mut Vec<Action>, pairing: &mut Vec<Arc<AtomicBool>>) {
    match e {
        Event::HostInfo { id, online, os, gpu, av1 } => {
            let entry = app.live.entry(id.clone()).or_insert_with(HostLive::default);
            entry.online = if online { Online::Online } else { Online::Offline };
            if online && let Some(h) = app.store.host_mut(&id) {
                let changed = h.os != os || h.gpu != gpu || h.av1 != av1;
                h.os = os.or(h.os.take());
                h.gpu = gpu.or(h.gpu.take());
                h.av1 = av1;
                if changed {
                    app.store.save();
                }
            }
        }
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
                Status::Connecting => Phase::Connecting,
                Status::FirstImages => Phase::FirstImages,
                Status::Streaming => {
                    if s.phase != Phase::Streaming && app.screen == Screen::Session {
                        app.focus = crate::app::Target::Menu(crate::app::MenuItem::Resume);
                    }
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
                l.mode = format!("{} × {}", m.width, m.height);
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
                    s.stats = Some(Stats { latency_ms, fps, mbps, lost, mode: l.mode.clone(), codec });
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
    }
}

fn perform(a: Action, app: &mut App, pads: &mut pads::Pads, live: &mut Option<Live>, tx: &Sender<Event>, pairing: &mut Vec<Arc<AtomicBool>>, quit: &mut bool) {
    match a {
        Action::Save => app.store.save(),
        Action::Quit => *quit = true,
        Action::Language => i18n::set_english(i18n::resolve(&app.store.app.language, &sdl::locale())),
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
            let handle = session::start(&h, profile, codec, mode, app.tv.refresh, app.net.cap_mbps(), device, tx.clone());
            app.session = Some(SessionUi {
                host: h.id.clone(),
                name: h.name.clone(),
                profile,
                phase: Phase::Preparing,
                menu: false,
                keys: false,
                stats: None,
                show_stats: false,
                remote_mouse: false,
                pad_mouse: false,
                notice: None,
                game_hint: h.settings.game_mode_hint,
                started: Instant::now(),
            });
            app.screen = Screen::Session;
            app.focus = crate::app::Target::CancelConnect;
            *live = Some(Live {
                handle,
                pointer: None,
                pointer_hidden: true,
                started: Instant::now(),
                latencies: Vec::new(),
                mbps: Vec::new(),
                mode: format!("{} × {}", mode.0, mode.1),
                remote: None,
                game_fullscreen: false,
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
        Action::Wake(id) => {
            let ok = app.store.host(&id).and_then(|h| h.mac.clone()).is_some_and(|m| jobs::wake(&m));
            app.toast(if ok { tr("Réveil envoyé : l'ordinateur démarre…", "Wake sent: the computer is starting…") } else { tr("Réveil impossible", "Could not wake") });
        }
        Action::Rumble { pad, low, high, triggers } => pads.test_rumble(pad, low, high, triggers),
    }
}

/// Fin de session : la dernière session (latence médiane, durée) pour l'accueil.
fn finish(app: &mut App, l: Live) {
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
    if let (Some(id), false) = (host, lat.is_empty()) {
        let median = lat[lat.len() / 2];
        let p95 = lat[((lat.len() as f64 * 0.95) as usize).min(lat.len() - 1)];
        let mbps = if l.mbps.is_empty() { 0.0 } else { l.mbps.iter().sum::<f64>() / l.mbps.len() as f64 };
        let codec = l.handle.shared.codec();
        if let Some(h) = app.store.host_mut(&id) {
            h.last = Some(LastSession { median, p95, seconds, mode: format!("{} · {codec}", l.mode), mbps, at: now_secs() });
        }
        app.store.save();
        // L'aperçu sera repris d'Admin (l'image de la session est sur le plan vidéo).
        app.images.remove(&id);
    }
}
