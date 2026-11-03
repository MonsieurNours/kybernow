//! Consoles PlayStation, côté appareil (docs/playstation.md) : le cœur (`kybernow_core::playstation`) les cherche,
//! les associe, les réveille et y joue ; ici, ce que l'app Linux y branche.
//!
//! - **Recherche** : toutes les 3 s hors partie (accueil, ajout, réglages), en diffusion et en direct vers les
//!   adresses retenues ; une fois par minute, les consoles que voit la Station de chaque ordinateur
//!   (`GET /api/consoles`, sur le réseau ou par le relais) : hors de chez soi, une console qui ne répond pas et
//!   qu'une Station joignable par le relais voit est « Hors de chez vous ».
//! - **Partie** : une session de l'app (`SessionUi::console`) dans la fenêtre de l'app : la même connexion, la même
//!   barre. Un fil prépare le départ (réveil, Station et relais), puis `Play::start` envoie l'image au même décodeur
//!   que les sessions d'ordinateur (`VideoSink` : VA-API, V4L2 sur le Pi, sinon le processeur) et le son au même
//!   moteur (`AudioSink`). Le clavier et la souris tiennent lieu de manette (`Play::key`, `mouse_*`), les manettes
//!   passent par le lecteur du cœur (que la boucle SDL nourrit, avec pavé tactile et capteurs), vibrations et
//!   gâchettes reviennent par `gamepads::set_app_feedback`, posé le temps de la partie.
//! - **Vibrations HD** : une DualSense branchée en USB est aussi une carte son à quatre voies ; ses voies 3 et 4
//!   tiennent les moteurs. SDL l'ouvre par son nom (PipeWire, PulseAudio ou ALSA) avec quatre voies ; sinon le cœur
//!   traduit en vibrations simples.
//! - **Envoi entre appareils** (`playstation::handoff`) : « Recevoir d'un autre appareil » partout, « Envoyer à un
//!   appareil » au bureau ; le paquet au format de l'app Mac.

use std::collections::HashMap;
use std::ffi::{c_int, c_void, CStr};
use std::net::Ipv4Addr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex, Weak};
use std::time::{Duration, Instant};

use kybernow_core::console::Console as Admin;
use kybernow_core::gamepads::{self, AppFeedback};
use kybernow_core::input::{GamepadRumble, PadId};
use kybernow_core::media::PacketHandler;
use kybernow_core::playstation::discovery::{self, Console, Model, State};
use kybernow_core::playstation::dualsense;
use kybernow_core::playstation::endpoint::Endpoint;
use kybernow_core::playstation::handoff;
use kybernow_core::playstation::keymouse::Layout as KmLayout;
use kybernow_core::playstation::play::{KeyboardEvent, PadEvent, Play, PlayEnd, PlayOptions, PlayState};
use kybernow_core::playstation::psn;
use kybernow_core::playstation::regist::{self, PsnId, RegistError, Registered, TargetName};
use kybernow_core::playstation::remote::{Remote, RemoteError};
use kybernow_core::playstation::settings::PlaySettings;

use super::media::AudioSink;
use super::session::{relay_route, Shared};
use super::video::VideoSink;
use super::{pads, sdl, Event, Wins};
use crate::app::{Action, App, Layout, Online, Phase, Screen, SessionId};
use crate::i18n::tr;
use crate::model::{PsConsole, PsPairing};
use crate::ps::{PsAction, PsEnd, PsFound, PsInput, PsState, PsStep, PsWaiting};

/// Ce qui revient des fils des consoles à la boucle principale.
pub enum PsEvent {
    Found(Vec<PsFound>),
    /// Les consoles que voit la Station de cet ordinateur (`host_id`, en majuscules).
    Stations { host: String, consoles: Vec<String> },
    Paired { console: PsConsole, account: String, result: Result<serde_json::Value, String> },
    Account { result: Result<(String, Option<String>), String>, console: Option<String> },
    /// « Par adresse » : la console qui a répondu, ou rien (l'ajout d'un ordinateur continue).
    Probed { address: String, ip: String, port: u16, found: Option<PsFound> },
    ReceiveCode(String),
    Received(String),
    ReceiveExpired,
    Devices(Vec<PsWaiting>),
    Sent { name: String, result: Result<(), bool> },
    /// La partie : le départ est prêt (où joindre la console ; par une Station : le signe de vie), a échoué, la
    /// console se réveille ; puis ce que dit la partie.
    Ready { id: SessionId, endpoint: Endpoint, remote: Option<Box<Remote>> },
    StartFailed { id: SessionId, text: String },
    Waking { id: SessionId },
    State { id: SessionId, state: PlayState },
    Pad { id: SessionId, event: PadEvent },
    Keyboard { id: SessionId, event: KeyboardEvent },
    Feedback(Feedback),
}

/// Ce que la partie demande aux manettes (`gamepads::AppFeedback`), joué par la boucle SDL.
pub enum Feedback {
    Rumble(GamepadRumble),
    /// Rapport d'effets de la DualSense (47 octets : gâchettes adaptatives).
    Effect { id: PadId, data: Vec<u8> },
}

/// Le retour des manettes d'une partie : posé le temps de la partie seulement (`gamepads::set_app_feedback`) ;
/// pendant une session d'ordinateur, les vibrations passent par `InputPlatform::rumble`.
struct FeedbackSink(Mutex<Sender<Event>>);

impl AppFeedback for FeedbackSink {
    fn rumble(&self, r: GamepadRumble) {
        send(&self.0.lock().unwrap(), PsEvent::Feedback(Feedback::Rumble(r)));
    }

    fn effect(&self, id: PadId, data: &[u8]) {
        send(&self.0.lock().unwrap(), PsEvent::Feedback(Feedback::Effect { id, data: data.to_vec() }));
    }
}

/// Parties commencées : le retour des manettes d'une partie finie n'est retiré que si aucune autre n'a commencé.
static GAMES: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// La touche V (code USB).
const SC_V: i32 = 25;

fn send(tx: &Sender<Event>, e: PsEvent) {
    let _ = tx.send(Event::Ps(e));
    super::video::wake_main();
}

fn state_of(s: State) -> PsState {
    match s {
        State::Ready => PsState::Ready,
        State::Standby => PsState::Standby,
        State::Unknown => PsState::Unknown,
    }
}

fn found_of(c: &Console) -> PsFound {
    PsFound {
        host_id: c.host_id.to_ascii_uppercase(),
        name: c.name.clone(),
        model: c.model().name().to_string(),
        state: state_of(c.state),
        address: c.addr.to_string(),
        target: serde_json::to_value(TargetName::from_target(c.target())).ok().and_then(|v| v.as_str().map(str::to_string)).unwrap_or_default(),
        running_app: c.running_app.clone(),
    }
}

/// Le nom de la plateforme pour les recherches de l'envoi (`handoff`) : `steamdeck`, `raspberrypi`, `linux`.
fn platform(app: &App) -> &'static str {
    if app.machine.pi.is_some() {
        "raspberrypi"
    } else if app.layout == Layout::Handheld {
        "steamdeck"
    } else {
        "linux"
    }
}

/// Une partie en cours.
struct Game {
    id: SessionId,
    host_id: String,
    account: String,
    /// Le départ (réveil, Station) : interrompu par « Annuler ».
    cancel: Arc<AtomicBool>,
    shared: Option<Arc<Shared>>,
    play: Option<Arc<Play>>,
    /// Par une Station : le signe de vie tient tant que la partie dure.
    remote: Option<Box<Remote>>,
    haptics: Option<Haptics>,
    last_stats: Instant,
    bar_arm: Option<Instant>,
    bar_hide: Option<Instant>,
    /// La barre ou le menu ouverts : la console reçoit une manette au repos.
    captured: bool,
}

/// Ce que l'app tient des consoles, à côté des sessions d'ordinateur.
pub struct Runtime {
    tx: Sender<Event>,
    game: Option<Game>,
    searching: Arc<AtomicBool>,
    last_search: Instant,
    last_stations: Instant,
    /// Les consoles que voit la Station de chaque ordinateur (dernière réponse).
    seen_by: HashMap<String, Vec<String>>,
    receiving: Option<Arc<AtomicBool>>,
    discovering: Arc<AtomicBool>,
    last_discover: Instant,
    /// Le plein écran de la fenêtre de l'app avant la partie : rendu quand la partie est fermée (bureau).
    restore_fullscreen: Option<bool>,
}

impl Runtime {
    pub fn new(tx: Sender<Event>) -> Self {
        Self {
            tx,
            game: None,
            searching: Arc::new(AtomicBool::new(false)),
            last_search: Instant::now() - Duration::from_secs(60),
            last_stations: Instant::now() - Duration::from_secs(50),
            seen_by: HashMap::new(),
            receiving: None,
            discovering: Arc::new(AtomicBool::new(false)),
            last_discover: Instant::now() - Duration::from_secs(60),
            restore_fullscreen: None,
        }
    }

    /// La session de l'app est une partie sur une console.
    pub fn owns(&self, id: SessionId) -> bool {
        self.game.as_ref().is_some_and(|g| g.id == id)
    }

    // MARK: - Chaque tour de la boucle

    /// Recherches, statistiques, images, barre du bureau. Rend vrai quand l'interface doit se redessiner.
    pub fn tick(&mut self, app: &mut App, wins: &mut Wins, has_sessions: bool) -> bool {
        let mut dirty = app.ps_tick();
        let idle = !has_sessions && self.game.is_none() && matches!(app.screen, Screen::Home | Screen::Add | Screen::Settings);
        if idle && self.last_search.elapsed() > Duration::from_secs(3) && !self.searching.swap(true, Ordering::SeqCst) {
            self.last_search = Instant::now();
            let mut direct: Vec<Ipv4Addr> = app.store.ps_consoles.iter().flat_map(|c| c.addresses.iter()).chain(app.ps.probed.iter()).filter_map(|a| a.parse().ok()).collect();
            direct.sort();
            direct.dedup();
            let (tx, flag) = (self.tx.clone(), self.searching.clone());
            std::thread::Builder::new()
                .name("ps-recherche".into())
                .spawn(move || {
                    let found = discovery::search(Duration::from_millis(1500), &direct).unwrap_or_default();
                    flag.store(false, Ordering::SeqCst);
                    let mut list: Vec<PsFound> = found.iter().map(found_of).collect();
                    list.dedup_by(|a, b| a.host_id == b.host_id);
                    send(&tx, PsEvent::Found(list));
                })
                .ok();
        }
        // Les consoles que voient les Stations : une fois par minute au plus, s'il y a des consoles.
        if idle && !app.store.ps_consoles.is_empty() && self.last_stations.elapsed() > Duration::from_secs(60) {
            self.last_stations = Instant::now();
            for h in &app.store.hosts {
                let online = app.live.get(&h.id).map(|l| l.online).unwrap_or_default();
                if !matches!(online, Online::Online | Online::Away) {
                    continue;
                }
                let mut admin = Admin::new(&h.address, h.port, &h.user, &h.password, h.fingerprint.clone());
                if online == Online::Away {
                    admin = admin.with_relay(relay_route(h));
                }
                let (tx, host) = (self.tx.clone(), h.id.clone());
                std::thread::spawn(move || {
                    if let Ok(list) = admin.consoles() {
                        send(&tx, PsEvent::Stations { host, consoles: list.into_iter().map(|c| c.host_id.to_ascii_uppercase()).collect() });
                    }
                });
            }
        }
        // « Envoyer à un appareil » : les appareils en attente, cherchés toutes les 3 s pendant que l'onglet est ouvert.
        let tab = app.screen == Screen::Add && matches!(&app.add.step, crate::app::AddStep::Ps(PsStep::Tab)) && app.layout == Layout::Desktop && !app.store.ps_accounts.is_empty();
        if tab && self.last_discover.elapsed() > Duration::from_secs(3) && !self.discovering.swap(true, Ordering::SeqCst) {
            self.last_discover = Instant::now();
            let (tx, flag) = (self.tx.clone(), self.discovering.clone());
            std::thread::spawn(move || {
                let list = handoff::discover(Duration::from_millis(1200), &[]).unwrap_or_default();
                flag.store(false, Ordering::SeqCst);
                send(&tx, PsEvent::Devices(list.into_iter().map(|w| PsWaiting { name: w.name, platform: w.platform, addr: w.addr.to_string() }).collect()));
            });
        }
        if self.game.is_some() {
            dirty |= self.game_tick(app, wins);
        }
        // La partie est fermée : la fenêtre de l'app redevient ce qu'elle était.
        if self.game.is_none()
            && app.sessions.is_empty()
            && let Some(full) = self.restore_fullscreen.take()
            && full != app.fullscreen
        {
            app.fullscreen = full;
            unsafe { sdl::SDL_SetWindowFullscreen(wins.app().window, if full { sdl::SDL_WINDOW_FULLSCREEN_DESKTOP } else { 0 }) };
            dirty = true;
        }
        dirty
    }

    fn game_tick(&mut self, app: &mut App, wins: &mut Wins) -> bool {
        let Some(g) = self.game.as_mut() else { return false };
        let mut dirty = false;
        let id = g.id;
        // Statistiques, une fois par seconde (perdues et réparées : depuis le début).
        if let Some(play) = &g.play
            && g.last_stats.elapsed() >= Duration::from_secs(1)
        {
            g.last_stats = Instant::now();
            let st = play.stats();
            if let Some(ps) = app.session_by_mut(id).and_then(|s| s.console.as_mut()) {
                ps.width = st.width;
                ps.height = st.height;
                ps.hevc = st.hevc;
                ps.target_kbps = st.target_kbps;
                ps.fps = st.fps;
                ps.mbps = st.mbps;
                ps.lost += st.lost;
                ps.recovered += st.recovered;
                dirty |= app.session_by(id).is_some_and(|s| s.show_stats);
            }
        }
        // L'image : la plus récente part à la texture de la fenêtre de l'app ; la première ouvre la partie.
        if let Some(shared) = &g.shared {
            let latest = shared.latest_for(0);
            if let Some(f) = latest.take() {
                let w = wins.app();
                if w.session == Some(id) && w.video.upload(w.renderer, &f) {
                    w.present = true;
                    if let Some(s) = app.session_by_mut(id)
                        && s.phase != Phase::Streaming
                        && !matches!(s.phase, Phase::Ending | Phase::Ended { .. })
                    {
                        s.phase = Phase::Streaming;
                        s.opened_at.get_or_insert_with(Instant::now);
                        app.focus = crate::app::Target::Menu(crate::app::MenuItem::Resume);
                        dirty = true;
                    }
                }
                latest.give_back(f);
            }
        }
        // Une manette branchée exclut clavier et souris.
        // La barre du bureau : un quart de seconde contre le bord haut, seulement quand la souris n'est pas
        // capturée ; elle se referme 400 ms après que le pointeur l'a quittée.
        let Some(s) = app.session_by(id) else { return dirty };
        let streaming = matches!(s.phase, Phase::Streaming | Phase::Reconnecting { .. });
        if app.layout == Layout::Desktop && streaming && !s.relative && app.active == Some(id) {
            let w = wins.app();
            let wpts = w.width_points(app.layout);
            let at_edge = app.pointer.is_some_and(|(ux, uy)| crate::ui::at_bar_edge(&mut w.cv, app, s, wpts, ux, uy));
            if !s.bar {
                match (at_edge, g.bar_arm) {
                    (true, None) => g.bar_arm = Some(Instant::now() + Duration::from_millis(250)),
                    (true, Some(t)) if Instant::now() >= t => {
                        g.bar_arm = None;
                        g.bar_hide = None;
                        if let Some(s) = app.session_by_mut(id) {
                            s.bar = true;
                        }
                        dirty = true;
                    }
                    (false, _) => g.bar_arm = None,
                    _ => {}
                }
            } else {
                let over = app.pointer.is_some_and(|(ux, uy)| crate::ui::over_bar(&mut w.cv, app, s, wpts, &w.hits, ux, uy));
                if over {
                    g.bar_hide = None;
                } else {
                    let t = *g.bar_hide.get_or_insert(Instant::now() + Duration::from_millis(400));
                    if Instant::now() >= t {
                        g.bar_hide = None;
                        if let Some(s) = app.session_by_mut(id) {
                            s.bar = false;
                        }
                        dirty = true;
                    }
                }
            }
        }
        // La barre, le menu, les réglages ouverts depuis la partie : la console reçoit une manette au repos.
        let captured = app.session_by(id).is_some_and(|s| s.bar || s.menu) || app.screen != Screen::Session;
        let g = self.game.as_mut().expect("partie");
        if captured != g.captured {
            g.captured = captured;
            if let Some(p) = &g.play {
                p.set_captured(captured);
            }
        }
        dirty
    }

    // MARK: - Actions

    pub fn perform(&mut self, a: PsAction, app: &mut App, wins: &mut Wins, pads: &pads::Pads) {
        let _: Vec<Action> = match a {
            PsAction::Play { host_id } => self.play(app, wins, &host_id),
            PsAction::Pair { console, address, account, pin } => {
                let Some(psn_id) = app.store.ps_account(&account).map(|a| a.psn_id.clone()) else { return };
                let tx = self.tx.clone();
                std::thread::spawn(move || {
                    let fail = |code: &str| Err(code.to_string());
                    let result = match (Endpoint::parse(&address), PsnId::from_base64(&psn_id), console.target.as_deref().and_then(|t| serde_json::from_value::<TargetName>(serde_json::Value::String(t.into())).ok())) {
                        (None, ..) => fail("other"),
                        (_, None, _) => fail("account"),
                        (Some(at), Some(id), target) => {
                            // Sans cible connue (console reçue d'un autre appareil, par adresse) : celle de la recherche.
                            let target = target.map(Into::into).or_else(|| discovery::search_at(Duration::from_millis(1500), false, &[at]).ok().and_then(|l| l.first().map(|c| c.target())));
                            match target {
                                None => fail("no_answer"),
                                Some(t) => match regist::regist_at(&at, t, &id, pin) {
                                    Ok(r) => Ok(serde_json::to_value(&r).unwrap_or_default()),
                                    Err(e) => {
                                        log::warn!("PlayStation : association refusée : {e}");
                                        fail(match &e {
                                            RegistError::NoAnswer | RegistError::Refused => "no_answer",
                                            RegistError::Rejected { reason: Some(regist::REASON_REGIST_FAILED), .. } => "wrong_code",
                                            RegistError::Rejected { reason: Some(regist::REASON_INVALID_PSN_ID), .. } => "account",
                                            _ => "other",
                                        })
                                    }
                                },
                            }
                        }
                    };
                    send(&tx, PsEvent::Paired { console, account, result });
                });
                Vec::new()
            }
            PsAction::Search => {
                self.last_search = Instant::now() - Duration::from_secs(60);
                Vec::new()
            }
            PsAction::SignIn => {
                super::open_url(psn::LOGIN_URL);
                Vec::new()
            }
            PsAction::ReadAccount { redirect, console } => {
                let tx = self.tx.clone();
                std::thread::spawn(move || {
                    let result = match psn::code_from_redirect(&redirect) {
                        None => Err(psn::PsnError::NoCode.to_string()),
                        Some(code) => match psn::account(&code) {
                            Ok(a) => a.id.to_base64().map(|id| (id, a.online_id)).ok_or_else(|| "identifiant illisible".to_string()),
                            Err(e) => Err(e.to_string()),
                        },
                    };
                    if let Err(e) = &result {
                        log::warn!("PlayStation : identifiant du compte non lu : {e}");
                    }
                    send(&tx, PsEvent::Account { result, console });
                });
                Vec::new()
            }
            PsAction::Receive(on) => {
                if let Some(stop) = self.receiving.take() {
                    stop.store(true, Ordering::SeqCst);
                }
                if on {
                    let stop = Arc::new(AtomicBool::new(false));
                    self.receiving = Some(stop.clone());
                    let (tx, name, platform) = (self.tx.clone(), app.device_name.clone(), platform(app));
                    std::thread::Builder::new()
                        .name("ps-recevoir".into())
                        .spawn(move || match handoff::Receiver::start(&name, platform) {
                            Err(e) => log::warn!("PlayStation : attente d'un envoi impossible ({e})"),
                            Ok(r) => {
                                send(&tx, PsEvent::ReceiveCode(r.code()));
                                while !stop.load(Ordering::SeqCst) {
                                    match r.next(Duration::from_millis(300)) {
                                        Some(handoff::Event::Received(b)) => {
                                            send(&tx, PsEvent::Received(b));
                                            break;
                                        }
                                        Some(handoff::Event::NewCode(c)) => send(&tx, PsEvent::ReceiveCode(c)),
                                        Some(handoff::Event::Expired) => {
                                            send(&tx, PsEvent::ReceiveExpired);
                                            break;
                                        }
                                        None => {}
                                    }
                                }
                                r.stop();
                            }
                        })
                        .ok();
                }
                Vec::new()
            }
            PsAction::Send { addr, name, code } => {
                let bundle = bundle(app);
                let tx = self.tx.clone();
                std::thread::spawn(move || {
                    let result = match addr.parse() {
                        Err(_) => Err(false),
                        Ok(to) => handoff::send(to, &code, &bundle).map_err(|e| {
                            log::warn!("PlayStation : envoi à {name} : {e}");
                            matches!(e, handoff::SendError::Refused)
                        }),
                    };
                    send(&tx, PsEvent::Sent { name, result });
                });
                Vec::new()
            }
            PsAction::Rest(id) => {
                if let Some(p) = self.game.as_ref().filter(|g| g.id == id).and_then(|g| g.play.clone()) {
                    p.goto_bed();
                }
                Vec::new()
            }
            PsAction::Changed { host_id } => {
                let _ = pads;
                if let Some(g) = self.game.as_ref().filter(|g| g.host_id.eq_ignore_ascii_case(&host_id))
                    && let Some(p) = &g.play
                    && let Some(c) = app.store.console(&host_id)
                {
                    p.set_layout(layout_of(c));
                    p.set_intensities(c.settings.rumble.min(100), c.settings.triggers.min(100));
                }
                Vec::new()
            }
        };
    }

    /// « Par adresse » : une console à cette adresse d'abord (1,5 s), puis la Station comme avant.
    pub fn probe(&self, address: String, port: u16) {
        let tx = self.tx.clone();
        std::thread::spawn(move || {
            let ip = address.rsplit_once(':').map(|(h, _)| h).unwrap_or(&address).trim_matches(['[', ']']).to_string();
            let found = ip.parse::<Ipv4Addr>().ok().and_then(|ip| discovery::search_at(Duration::from_millis(1500), false, &[Endpoint::direct(ip)]).ok()).and_then(|l| l.first().map(found_of));
            send(&tx, PsEvent::Probed { address, ip, port, found });
        });
    }

    // MARK: - Partie

    fn play(&mut self, app: &mut App, wins: &mut Wins, host_id: &str) -> Vec<Action> {
        if self.game.is_some() || !app.sessions.is_empty() {
            return Vec::new();
        }
        let Some(c) = app.store.console(host_id).cloned() else { return Vec::new() };
        let Some(pairing) = c.pairing().cloned() else { return Vec::new() };
        let Ok(registered) = serde_json::from_value::<Registered>(pairing.registered.clone()) else {
            log::warn!("PlayStation : association illisible pour {}", c.name);
            return Vec::new();
        };
        let live = app.ps_live(&c.host_id);
        let away = live.state == PsState::Away;
        let Some(id) = app.ps_open_session(&c.host_id, away) else { return Vec::new() };
        // La fenêtre de l'app porte la partie, à son nom ; plein écran retenu par console (bureau).
        let w = wins.app();
        w.session = Some(id);
        w.extra = false;
        sdl::set_title(w.window, &super::session_title(&c.name));
        self.restore_fullscreen = (app.layout == Layout::Desktop).then_some(app.fullscreen);
        if app.layout == Layout::Desktop && c.settings.fullscreen != app.fullscreen {
            app.fullscreen = c.settings.fullscreen;
            unsafe { sdl::SDL_SetWindowFullscreen(w.window, if app.fullscreen { sdl::SDL_WINDOW_FULLSCREEN_DESKTOP } else { 0 }) };
        }
        let full = app.layout.console() || app.fullscreen;
        if let Some(s) = app.session_by_mut(id) {
            s.fullscreen = full;
            // En veille (ou hors de chez soi, l'état n'est lu qu'en joignant la Station) : la connexion commence
            // par le réveil ou la Station ; allumée, directement par la connexion à la console.
            s.phase = if live.state == PsState::Ready { Phase::Connecting } else { Phase::Preparing };
            if let Some(ps) = s.console.as_mut() {
                ps.waking = live.state == PsState::Standby;
                ps.max_fps = c.settings.fps;
            }
        }
        if app.fullscreen || app.layout.console() {
            unsafe { sdl::SDL_SetWindowKeyboardGrab(wins.app().window, 1) };
        }
        let cancel = Arc::new(AtomicBool::new(false));
        self.game = Some(Game { id, host_id: c.host_id.clone(), account: pairing.account.clone(), cancel: cancel.clone(), shared: None, play: None, remote: None, haptics: None, last_stats: Instant::now(), bar_arm: None, bar_hide: None, captured: false });
        // Les Stations qui voient la console, joignables par le relais (hors de chez soi).
        let stations: Vec<(String, Admin)> = app
            .store
            .hosts
            .iter()
            .filter(|h| c.stations.iter().any(|s| s == &h.id) || self.seen_by.get(&h.id).is_some_and(|l| l.iter().any(|x| x.eq_ignore_ascii_case(&c.host_id))))
            .filter_map(|h| relay_route(h).map(|r| (h.name.clone(), Admin::new(&h.address, h.port, &h.user, &h.password, h.fingerprint.clone()).with_relay(Some(r)))))
            .collect();
        let addresses: Vec<Ipv4Addr> = live.address.iter().chain(c.addresses.iter()).filter_map(|a| a.parse().ok()).collect();
        let (tx, host_id) = (self.tx.clone(), c.host_id.clone());
        log::info!("PlayStation : partie sur {} ({}, {} Station(s) par le relais)", c.name, if away { "hors de chez soi" } else { "réseau" }, stations.len());
        std::thread::Builder::new()
            .name("ps-depart".into())
            .spawn(move || prepare(id, &host_id, &registered, addresses, stations, cancel, tx))
            .ok();
        Vec::new()
    }

    /// Le départ est prêt : la partie s'ouvre.
    fn start(&mut self, app: &mut App, pads: &pads::Pads, id: SessionId, endpoint: Endpoint, remote: Option<Box<Remote>>) {
        let Some(g) = self.game.as_mut().filter(|g| g.id == id) else { return };
        let Some(c) = app.store.console(&g.host_id).cloned() else { return };
        let Some(registered) = c.pairings.iter().find(|p| p.account == g.account).and_then(|p| serde_json::from_value::<Registered>(p.registered.clone()).ok()) else { return };
        let shared = Shared::for_console(id, self.tx.clone(), &app.machine);
        let video: Arc<dyn PacketHandler> = VideoSink::new(shared.clone(), 0);
        let audio = AudioSink::new(2, shared.silenced_flag());
        *shared.audio.lock().unwrap() = Some(Arc::downgrade(&audio));
        let settings = serde_json::to_value(&c.settings).ok().and_then(|v| serde_json::from_value::<PlaySettings>(v).ok()).unwrap_or_default();
        let options = PlayOptions { settings, hevc_decoder: app.machine.decodes("hevc") };
        let tx = Mutex::new(self.tx.clone());
        let on_state = Box::new(move |state: PlayState| send(&tx.lock().unwrap(), PsEvent::State { id, state }));
        let tx = Mutex::new(self.tx.clone());
        let on_pad = Box::new(move |event: PadEvent| send(&tx.lock().unwrap(), PsEvent::Pad { id, event }));
        // Vibrations et gâchettes de la partie : le fil de la partie les envoie ici, la boucle SDL les joue.
        GAMES.fetch_add(1, Ordering::SeqCst);
        gamepads::set_app_feedback(Some(Arc::new(FeedbackSink(Mutex::new(self.tx.clone())))));
        let play = Arc::new(Play::start(endpoint, &registered, options, video, Some(audio as Arc<dyn PacketHandler>), on_state, on_pad));
        let tx = Mutex::new(self.tx.clone());
        play.set_on_keyboard(Some(Box::new(move |event| send(&tx.lock().unwrap(), PsEvent::Keyboard { id, event }))));
        play.set_layout(layout_of(&c));
        // Le décodeur a perdu sa référence : la partie redemande une image clé à la console.
        let weak: Weak<Play> = Arc::downgrade(&play);
        *shared.key_hook.lock().unwrap() = Some(Arc::new(move |lost| {
            if let Some(p) = weak.upgrade() {
                if lost { p.key_frame_lost() } else { p.key_frame_recovered() }
            }
        }));
        // Vibrations HD : la carte son d'une DualSense branchée en USB.
        if pads.dualsense_usb() {
            g.haptics = Haptics::open();
        }
        g.shared = Some(shared);
        g.play = Some(play);
        g.remote = remote;
        if let Some(ps) = app.session_by_mut(id).and_then(|s| s.console.as_mut()) {
            ps.waking = false;
            ps.pads = pads.count() > 0;
        }
    }

    /// La partie est finie (ou le départ a échoué) : tout est rendu ; `Play` s'arrête sur un fil (il attend sa fin).
    fn finish(&mut self, app: &mut App) {
        let Some(g) = self.game.take() else { return };
        g.cancel.store(true, Ordering::SeqCst);
        unsafe {
            sdl::SDL_SetRelativeMouseMode(0);
            sdl::SDL_StopTextInput();
        }
        if let Some(s) = app.session_by_mut(g.id) {
            s.relative = false;
        }
        drop(g.haptics);
        let (play, shared, remote) = (g.play, g.shared, g.remote);
        let generation = GAMES.load(Ordering::SeqCst);
        std::thread::spawn(move || {
            // La partie s'arrête : ses derniers effets (moteurs arrêtés, gâchettes libres) passent encore par le
            // retour des manettes, retiré ensuite (sauf si une autre partie a commencé entre-temps).
            drop(play);
            if GAMES.load(Ordering::SeqCst) == generation {
                gamepads::set_app_feedback(None);
            }
            drop(shared);
            drop(remote);
        });
    }

    /// « Se déconnecter », « Annuler », la fenêtre fermée : la partie s'arrête ; la fin vient de la partie.
    pub fn stop(&mut self, app: &mut App, id: SessionId) {
        let Some(g) = self.game.as_ref().filter(|g| g.id == id) else { return };
        if let Some(s) = app.session_by_mut(id) {
            if matches!(s.phase, Phase::Ended { .. }) {
                app.close_session(id);
                if app.sessions.is_empty() {
                    app.go(Screen::Home);
                }
                self.finish(app);
                return;
            }
            s.phase = Phase::Ending;
            s.ending_since.get_or_insert_with(Instant::now);
            s.bar = false;
            s.menu = false;
        }
        match g.play.clone() {
            // La fin arrive par `PlayState::Ended(Stopped)`, sur le fil de l'arrêt.
            Some(_) => self.finish(app),
            None => {
                self.finish(app);
                app.ps_ended(id, PsEnd::Stopped);
            }
        }
        // La fin voulue : la session de l'app se ferme quand l'animation de la déconnexion est passée.
        let tx = self.tx.clone();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(900));
            send(&tx, PsEvent::State { id, state: PlayState::Ended(PlayEnd::Stopped) });
        });
    }

    /// Quitter l'app : la partie s'arrête (la console est prévenue).
    pub fn stop_all(&mut self) {
        if let Some(g) = self.game.take() {
            g.cancel.store(true, Ordering::SeqCst);
            drop(g.play);
        }
        if let Some(s) = self.receiving.take() {
            s.store(true, Ordering::SeqCst);
        }
        gamepads::set_app_feedback(None);
    }

    // MARK: - Événements

    pub fn event(&mut self, e: PsEvent, app: &mut App, pads: &mut pads::Pads, actions: &mut Vec<Action>) {
        match e {
            PsEvent::Found(list) => {
                if list.iter().map(|f| (&f.host_id, f.state)).ne(app.ps.found.iter().map(|f| (&f.host_id, f.state))) {
                    log::info!("PlayStation : {} console(s) : {}", list.len(), list.iter().map(|f| format!("{} {} {} ({:?})", f.model, f.host_id, f.address, f.state)).collect::<Vec<_>>().join(", "));
                }
                let away: Vec<String> = app
                    .store
                    .ps_consoles
                    .iter()
                    .filter(|c| {
                        app.store.hosts.iter().any(|h| {
                            app.live.get(&h.id).is_some_and(|l| l.online == Online::Away) && h.relay.is_some() && (c.stations.contains(&h.id) || self.seen_by.get(&h.id).is_some_and(|l| l.iter().any(|x| x.eq_ignore_ascii_case(&c.host_id))))
                        })
                    })
                    .map(|c| c.host_id.to_ascii_uppercase())
                    .collect();
                app.ps_found(list, &away);
            }
            PsEvent::Stations { host, consoles } => {
                // Retenues avec la console : hors de chez soi, l'app sait par où la joindre.
                let mut changed = false;
                for c in &mut app.store.ps_consoles {
                    if consoles.iter().any(|x| x.eq_ignore_ascii_case(&c.host_id)) && !c.stations.contains(&host) {
                        c.stations.push(host.clone());
                        changed = true;
                    }
                }
                self.seen_by.insert(host, consoles);
                if changed {
                    actions.push(Action::Save);
                }
            }
            PsEvent::Paired { console, account, result } => actions.extend(app.ps_paired(console, &account, result)),
            PsEvent::Account { result, console } => actions.extend(app.ps_account_read(result, console)),
            PsEvent::Probed { address, ip, port, found } => match found.inspect(|f| log::info!("PlayStation : « Par adresse » : {} {} à {ip} ({:?})", f.model, f.name, f.state)) {
                Some(f) => actions.extend(app.ps_probed(&ip, Some(f))),
                // Pas de console : la Station, comme avant ; une adresse seule, sans port d'Admin : « Console injoignable. ».
                None if port == 0 => {
                    app.add.busy = false;
                    app.add.message = Some(tr("Console injoignable.", "Console unreachable.").into());
                }
                None => super::jobs::probe(address, port, self.tx.clone()),
            },
            PsEvent::ReceiveCode(code) => {
                app.ps.add.receive_code = Some(code);
                app.ps.add.receive_expired = false;
            }
            PsEvent::ReceiveExpired => app.ps.add.receive_expired = true,
            PsEvent::Received(bundle) => {
                let (accounts, consoles) = merge(app, &bundle);
                log::info!("PlayStation : reçu {accounts} compte(s) et {consoles} console(s)");
                self.receiving = None;
                actions.extend(app.ps_received(accounts, consoles));
            }
            PsEvent::Devices(list) => {
                app.ps.add.devices = list;
                app.ps.add.devices_searched = true;
            }
            PsEvent::Sent { name, result } => app.ps_sent(&name, result),
            PsEvent::Waking { id } => {
                if let Some(ps) = app.session_by_mut(id).and_then(|s| s.console.as_mut()) {
                    ps.waking = true;
                }
            }
            PsEvent::Ready { id, endpoint, remote } => {
                if let Some(s) = app.session_by_mut(id)
                    && matches!(s.phase, Phase::Preparing | Phase::Connecting)
                {
                    s.phase = Phase::Connecting;
                    if let Some(ps) = s.console.as_mut() {
                        ps.relayed = endpoint.relayed;
                    }
                    self.start(app, pads, id, endpoint, remote);
                }
            }
            PsEvent::StartFailed { id, text } => {
                if self.owns(id) {
                    self.finish(app);
                    if let Some(s) = app.session_by_mut(id)
                        && !matches!(s.phase, Phase::Ending)
                    {
                        s.phase = Phase::Ended { error: Some(text) };
                    }
                }
            }
            PsEvent::State { id, state } => self.state(app, id, state, actions),
            PsEvent::Pad { id, event } => {
                // La combinaison Options + Create + L1 + R1 : la barre (bureau) ou le menu, ouverts ou refermés.
                let desktop = app.layout == Layout::Desktop;
                if event == PadEvent::Menu
                    && let Some(s) = app.session_by_mut(id)
                    && matches!(s.phase, Phase::Streaming | Phase::Reconnecting { .. })
                {
                    if desktop {
                        s.bar = !s.bar;
                    } else {
                        s.menu = !s.menu;
                        if s.menu {
                            app.focus = crate::app::Target::Menu(crate::app::MenuItem::Resume);
                        }
                    }
                }
            }
            PsEvent::Keyboard { id, event } => {
                let Some(ps) = app.session_by_mut(id).and_then(|s| s.console.as_mut()) else { return };
                match event {
                    KeyboardEvent::Open(t) | KeyboardEvent::Text(t) => {
                        ps.keyboard = Some(t);
                        unsafe { sdl::SDL_StartTextInput() };
                    }
                    KeyboardEvent::Closed => {
                        ps.keyboard = None;
                        unsafe { sdl::SDL_StopTextInput() };
                    }
                }
            }
            PsEvent::Feedback(f) => match f {
                Feedback::Rumble(r) => pads.rumble_exact(r.id as usize, r.large_motor, r.small_motor, r.duration_ms),
                Feedback::Effect { id, data } => pads.effect(id as usize, &data),
            },
        }
    }

    fn state(&mut self, app: &mut App, id: SessionId, state: PlayState, actions: &mut Vec<Action>) {
        match state {
            PlayState::Connecting => {
                if let Some(s) = app.session_by_mut(id) {
                    // Une partie qui reprend toute seule après une coupure.
                    if matches!(s.phase, Phase::Streaming | Phase::Reconnecting { .. }) {
                        let attempt = if let Phase::Reconnecting { attempt, .. } = s.phase { attempt + 1 } else { 1 };
                        s.phase = Phase::Reconnecting { attempt: attempt.min(3), max: 3 };
                    }
                }
            }
            PlayState::Streaming { width, height, .. } => {
                if let Some(s) = app.session_by_mut(id) {
                    if matches!(s.phase, Phase::Preparing | Phase::Connecting) {
                        s.phase = Phase::FirstImages;
                    } else if matches!(s.phase, Phase::Reconnecting { .. }) {
                        s.phase = Phase::Streaming;
                    }
                    if let Some(ps) = s.console.as_mut() {
                        ps.width = width;
                        ps.height = height;
                    }
                }
            }
            PlayState::CantDisplay(on) => {
                if let Some(ps) = app.session_by_mut(id).and_then(|s| s.console.as_mut()) {
                    ps.protected = on;
                }
            }
            PlayState::Ended(end) => {
                log::info!("PlayStation : partie finie ({end:?})");
                let end = match end {
                    PlayEnd::Stopped => PsEnd::Stopped,
                    PlayEnd::InUse => PsEnd::InUse,
                    PlayEnd::NotPaired => PsEnd::NotPaired,
                    PlayEnd::Unreachable(_) => PsEnd::Unreachable,
                    PlayEnd::Remote(_) => PsEnd::Remote,
                };
                // La console ne reconnaît plus ce compte : cette association est effacée.
                if end == PsEnd::NotPaired
                    && let Some(g) = self.game.as_ref().filter(|g| g.id == id)
                {
                    let (h, a) = (g.host_id.clone(), g.account.clone());
                    app.store.forget_pairing(&h, &a);
                    actions.push(Action::Save);
                }
                if self.owns(id) {
                    self.finish(app);
                }
                app.ps_ended(id, end);
            }
        }
    }

    // MARK: - Entrées

    /// Ce que la boucle SDL donne à la partie ; rend vrai quand l'événement est pris (sinon le chemin habituel).
    #[allow(clippy::too_many_arguments)]
    pub fn handle_sdl(&mut self, ev: &sdl::SDL_Event, app: &mut App, pads: &mut pads::Pads, wins: &mut Wins, actions: &mut Vec<Action>, dirty: &mut bool) -> bool {
        let kind = ev.kind();
        // Réglages › Commandes : la touche ou le clic attendu.
        if app.screen == Screen::Settings && app.ps.capture.is_some() {
            match kind {
                sdl::SDL_KEYDOWN => {
                    let (sc, repeat, _) = ev.key();
                    if repeat || matches!(sc, sdl::SC_ESCAPE | sdl::SC_BACKSPACE) {
                        return false;
                    }
                    actions.extend(app.ps_capture_input(PsInput::Key(sc as u16)));
                    *dirty = true;
                    return true;
                }
                sdl::SDL_MOUSEBUTTONDOWN => {
                    let (b, ..) = ev.button();
                    actions.extend(app.ps_capture_input(PsInput::Mouse(mouse_button(b))));
                    *dirty = true;
                    return true;
                }
                _ => {}
            }
        }
        // L'onglet PlayStation : le texte tapé dans le champ de l'étape, et Ctrl+V (l'adresse de la page de Sony).
        if app.typing_ps() {
            match kind {
                sdl::SDL_TEXTINPUT => {
                    for c in ev.text().chars() {
                        actions.extend(app.nav(crate::app::Nav::Char(c)));
                    }
                    *dirty = true;
                    return true;
                }
                sdl::SDL_KEYDOWN => {
                    let (sc, _, mods) = ev.key();
                    if sc == SC_V && mods & sdl::KMOD_CTRL != 0 {
                        if let Some(t) = sdl::clipboard() {
                            app.ps_paste(&t);
                        }
                        *dirty = true;
                        return true;
                    }
                }
                _ => {}
            }
        }
        // Pavé tactile et capteurs : au lecteur du cœur, toujours (seule la partie s'en sert).
        match kind {
            sdl::SDL_CONTROLLERTOUCHPADDOWN | sdl::SDL_CONTROLLERTOUCHPADMOTION | sdl::SDL_CONTROLLERTOUCHPADUP => {
                let (instance, finger, x, y) = ev.touchpad();
                if let Some(slot) = pads.slot_of(instance)
                    && (0..2).contains(&finger)
                {
                    let at = (kind != sdl::SDL_CONTROLLERTOUCHPADUP).then_some((x.clamp(0.0, 1.0), y.clamp(0.0, 1.0)));
                    gamepads::global().touch_finger(slot as u32, finger as usize, at);
                }
                return true;
            }
            sdl::SDL_CONTROLLERSENSORUPDATE => {
                let (instance, sensor, data, us) = ev.sensor();
                if let Some(slot) = pads.slot_of(instance) {
                    let us = if us == 0 { (unsafe { sdl::SDL_GetTicks() } as u64) * 1000 } else { us };
                    gamepads::global().sensor(slot as u32, sensor == sdl::SDL_SENSOR_GYRO, data, us);
                }
                return true;
            }
            _ => {}
        }
        let Some(g) = self.game.as_ref() else { return false };
        let id = g.id;
        let play = g.play.clone();
        let Some(s) = app.session_by(id) else { return false };
        if app.active != Some(id) {
            return false;
        }
        let streaming = matches!(s.phase, Phase::Streaming | Phase::Reconnecting { .. });
        let in_game = app.screen == Screen::Session && streaming && !s.menu && !s.bar;
        let relative = s.relative;
        let bar_open = s.bar;
        let keyboard = s.console.as_ref().and_then(|c| c.keyboard.clone());
        let has_pads = pads.count() > 0;
        match kind {
            sdl::SDL_CONTROLLERBUTTONDOWN | sdl::SDL_CONTROLLERBUTTONUP | sdl::SDL_CONTROLLERAXISMOTION => {
                // Les manettes vont à la console (état tenu par le lecteur du cœur, même barre ouverte : la partie
                // envoie alors une manette au repos) ; barre ou menu ouverts, elles pilotent aussi l'app.
                let (instance, which, value) = ev.pad();
                let Some(slot) = pads.slot_of(instance) else { return false };
                if kind == sdl::SDL_CONTROLLERAXISMOTION {
                    if let Some(a) = pads::core_axis(which) {
                        gamepads::axis(slot as u32, a, value);
                    }
                } else {
                    let down = kind == sdl::SDL_CONTROLLERBUTTONDOWN;
                    // Le clic du pavé tactile (SDL : bouton 20).
                    if which == 20 {
                        gamepads::global().touch_click(slot as u32, down);
                    } else if let Some(b) = pads::core_button(which) {
                        gamepads::button(slot as u32, b, down);
                    }
                }
                if in_game {
                    pads.update(slot, kind != sdl::SDL_CONTROLLERAXISMOTION, which, kind == sdl::SDL_CONTROLLERBUTTONDOWN, value);
                    return true;
                }
                false
            }
            sdl::SDL_CONTROLLERDEVICEADDED | sdl::SDL_CONTROLLERDEVICEREMOVED => {
                // Une manette branchée exclut clavier et souris : la souris est rendue.
                let after = if kind == sdl::SDL_CONTROLLERDEVICEADDED { true } else { pads.count() > 1 };
                if let Some(ps) = app.session_by_mut(id).and_then(|s| s.console.as_mut()) {
                    ps.pads = after;
                }
                if after && relative {
                    self.capture(app, false);
                }
                if let Some(p) = &play {
                    p.release_all();
                }
                false
            }
            sdl::SDL_WINDOWEVENT => {
                let (e, _, _) = ev.window();
                if e == sdl::SDL_WINDOWEVENT_FOCUS_LOST
                    && let Some(p) = &play
                {
                    p.release_all();
                }
                false
            }
            sdl::SDL_KEYDOWN | sdl::SDL_KEYUP => {
                let down = kind == sdl::SDL_KEYDOWN;
                let (sc, repeat, mods) = ev.key();
                let app_combo = mods & sdl::KMOD_CTRL != 0 && mods & sdl::KMOD_ALT != 0 && mods & sdl::KMOD_SHIFT != 0;
                if app_combo || !in_game {
                    return false;
                }
                let Some(p) = play else { return true };
                // Le clavier de la console est ouvert : le clavier écrit dans son champ.
                if let Some(text) = keyboard {
                    if down {
                        let ps = app.session_by_mut(id).and_then(|s| s.console.as_mut());
                        match sc {
                            sdl::SC_RETURN | sdl::SC_KP_ENTER => {
                                p.keyboard_accept();
                                if let Some(ps) = ps {
                                    ps.keyboard = None;
                                }
                                unsafe { sdl::SDL_StopTextInput() };
                            }
                            sdl::SC_ESCAPE => {
                                p.keyboard_reject();
                                if let Some(ps) = ps {
                                    ps.keyboard = None;
                                }
                                unsafe { sdl::SDL_StopTextInput() };
                            }
                            sdl::SC_BACKSPACE => {
                                let mut t = text;
                                t.pop();
                                p.keyboard_text(&t);
                                if let Some(ps) = ps {
                                    ps.keyboard = Some(t);
                                }
                            }
                            _ => {}
                        }
                        *dirty = true;
                    }
                    return true;
                }
                // Clavier et souris en manette : seulement la souris prise et sans manette.
                if relative && !has_pads && !repeat {
                    p.key(sc as u16, down);
                }
                true
            }
            sdl::SDL_TEXTINPUT => {
                let (Some(text), Some(p)) = (keyboard, play) else { return in_game };
                if !in_game {
                    return false;
                }
                let t = format!("{text}{}", ev.text());
                p.keyboard_text(&t);
                if let Some(ps) = app.session_by_mut(id).and_then(|s| s.console.as_mut()) {
                    ps.keyboard = Some(t);
                }
                *dirty = true;
                true
            }
            sdl::SDL_MOUSEMOTION => {
                let (x, y, xr, yr, _) = ev.motion();
                if in_game && relative {
                    if let Some(p) = &play {
                        p.mouse_move(xr as f32, yr as f32);
                    }
                    return true;
                }
                // Le pointeur sur l'image : la barre s'ouvre contre le bord haut (`tick`).
                if app.layout == Layout::Desktop && streaming && app.screen == Screen::Session {
                    let (ux, uy) = super::to_ui(&wins.list[0], app.layout, x, y);
                    wins.hover = Some(wins.list[0].id);
                    if bar_open {
                        if super::in_window(app, &mut wins.list[0], |app| app.pointer_moved(ux, uy)) {
                            *dirty = true;
                        }
                    } else {
                        app.pointer = Some((ux, uy));
                    }
                    return true;
                }
                false
            }
            sdl::SDL_MOUSEBUTTONDOWN | sdl::SDL_MOUSEBUTTONUP => {
                let (b, x, y, _) = ev.button();
                if in_game && relative {
                    if let Some(p) = &play {
                        p.mouse_button(mouse_button(b), kind == sdl::SDL_MOUSEBUTTONDOWN);
                    }
                    return true;
                }
                // Un clic dans l'image (pas sur la barre) prend la souris : bureau, sans manette.
                let (ux, uy) = super::to_ui(&wins.list[0], app.layout, x, y);
                let on_ui = wins.list[0].hits.iter().any(|(_, r)| r.contains(ux, uy));
                // La barre ouverte : ses boutons.
                if bar_open && streaming && app.screen == Screen::Session {
                    if on_ui && b == 1 && kind == sdl::SDL_MOUSEBUTTONDOWN {
                        actions.extend(super::in_window(app, &mut wins.list[0], |app| app.pointer_click(ux, uy)));
                        *dirty = true;
                    }
                    return true;
                }
                if in_game && !on_ui && kind == sdl::SDL_MOUSEBUTTONDOWN && b == 1 && app.layout == Layout::Desktop && !has_pads && play.is_some() && keyboard.is_none() {
                    self.capture(app, true);
                    *dirty = true;
                    return true;
                }
                false
            }
            sdl::SDL_MOUSEWHEEL => in_game,
            _ => false,
        }
    }

    /// Prendre la souris (clavier et souris en manette) ou la rendre.
    pub fn capture(&mut self, app: &mut App, on: bool) {
        let Some(g) = self.game.as_ref() else { return };
        if let Some(p) = &g.play {
            p.release_all();
        }
        unsafe { sdl::SDL_SetRelativeMouseMode(on as c_int) };
        if let Some(s) = app.session_by_mut(g.id) {
            s.relative = on;
            s.bar = false;
            if let Some(ps) = s.console.as_mut() {
                ps.captured_at = on.then(Instant::now);
            }
        }
        log::info!("PlayStation : souris {}", if on { "prise (clavier et souris en manette)" } else { "rendue" });
    }

    /// Ctrl+Alt+Maj+M, « Prendre la souris » / « Rendre la souris ».
    pub fn toggle_mouse(&mut self, app: &mut App, pads: &pads::Pads) {
        let on = !app.session().is_some_and(|s| s.relative);
        if on && pads.count() > 0 {
            return;
        }
        self.capture(app, on);
    }

    /// « Couper le son » de la partie.
    pub fn mute(&self, on: bool) {
        if let Some(s) = self.game.as_ref().and_then(|g| g.shared.as_ref()) {
            s.set_silenced(on);
        }
    }

    /// Plein écran changé pendant la partie : retenu pour la console.
    pub fn remember_fullscreen(&self, app: &mut App) {
        let Some(g) = self.game.as_ref() else { return };
        if app.layout != Layout::Desktop {
            return;
        }
        let on = app.fullscreen;
        if let Some(c) = app.store.console_mut(&g.host_id) {
            c.settings.fullscreen = on;
        }
        app.store.save();
    }
}

/// Le départ d'une partie, sur son fil : la console sur le réseau (réveillée si elle est en veille), sinon par
/// une Station et le relais.
fn prepare(id: SessionId, host_id: &str, registered: &Registered, addresses: Vec<Ipv4Addr>, stations: Vec<(String, Admin)>, cancel: Arc<AtomicBool>, tx: Sender<Event>) {
    let model = if matches!(registered.target, TargetName::Ps5V1) { Model::Ps5 } else { Model::Ps4 };
    let find = |targets: &[Endpoint], broadcast: bool| -> Option<Console> {
        discovery::search_at(Duration::from_millis(1200), broadcast, targets).ok().and_then(|l| l.into_iter().find(|c| c.host_id.eq_ignore_ascii_case(host_id)))
    };
    // Réveille la console à `at` si elle est en veille, puis attend qu'elle soit allumée (une minute au plus).
    let wake_and_wait = |at: Endpoint, state: State| -> bool {
        if state == State::Ready {
            return true;
        }
        send(&tx, PsEvent::Waking { id });
        let Some(cred) = registered.wake_credential() else { return false };
        let started = Instant::now();
        let mut last_wake: Option<Instant> = None;
        while started.elapsed() < Duration::from_secs(60) && !cancel.load(Ordering::SeqCst) {
            if last_wake.is_none_or(|t| t.elapsed() > Duration::from_secs(5)) {
                let _ = discovery::wake_at(&at, model, cred);
                last_wake = Some(Instant::now());
            }
            if find(&[at], false).is_some_and(|c| c.state == State::Ready) {
                return true;
            }
        }
        false
    };
    // Sur le réseau (ou par un VPN) : l'adresse retenue, sinon une recherche en diffusion.
    let targets: Vec<Endpoint> = addresses.iter().map(|a| Endpoint::direct(*a)).collect();
    let local = if targets.is_empty() { None } else { find(&targets, false) }.or_else(|| find(&[], true));
    if let Some(c) = local {
        let at = Endpoint::direct(c.addr);
        if cancel.load(Ordering::SeqCst) {
            return;
        }
        if wake_and_wait(at, c.state) {
            send(&tx, PsEvent::Ready { id, endpoint: at, remote: None });
        } else if !cancel.load(Ordering::SeqCst) {
            send(&tx, PsEvent::StartFailed { id, text: tr("La console ne répond pas.", "The console isn't answering.").into() });
        }
        return;
    }
    // Hors de chez soi : la première Station qui la voit, par le relais.
    let mut refusal: Option<String> = None;
    for (name, admin) in stations {
        if cancel.load(Ordering::SeqCst) {
            return;
        }
        match Remote::open(&admin, host_id) {
            Ok(remote) => {
                let at = remote.endpoint();
                let state = match remote.seen().map(|s| s.state) {
                    Some(s) if s == "standby" => State::Standby,
                    _ => State::Ready,
                };
                if wake_and_wait(at, state) {
                    send(&tx, PsEvent::Ready { id, endpoint: at, remote: Some(Box::new(remote)) });
                    return;
                }
            }
            Err(RemoteError::NotSeen) => {}
            Err(RemoteError::NotRelayed) => {}
            Err(RemoteError::Console(e)) => {
                log::warn!("PlayStation : la Station {name} refuse ({e})");
                refusal = Some(match &e {
                    kybernow_core::console::ConsoleError::Relay(r) => super::session::relay_failure_text(Some(r), None, &name).unwrap_or_else(|| e.to_string()),
                    _ => e.to_string(),
                });
            }
        }
    }
    if !cancel.load(Ordering::SeqCst) {
        send(&tx, PsEvent::StartFailed { id, text: refusal.unwrap_or_else(|| tr("La console ne répond pas.", "The console isn't answering.").into()) });
    }
}

/// Le bouton de la souris de SDL (1 gauche, 2 milieu, 3 droit, 4 et 5) dans la numérotation du cœur (0 gauche,
/// 1 droit, 2 milieu, 3 et 4).
fn mouse_button(b: u8) -> u8 {
    match b {
        1 => 0,
        3 => 1,
        2 => 2,
        n => n.saturating_sub(1),
    }
}

fn layout_of(c: &PsConsole) -> KmLayout {
    c.layout.as_ref().and_then(|v| serde_json::from_value(v.clone()).ok()).unwrap_or_default()
}

/// Le nom de chaque touche du clavier de cet appareil, dans sa disposition (Z en AZERTY là où le QWERTY a W).
pub fn key_names() -> HashMap<u16, String> {
    let mut m = HashMap::new();
    for u in (0x04u16..=0x65).chain(0xe0..=0xe7) {
        let name = unsafe {
            let k = sdl::SDL_GetKeyFromScancode(u as c_int);
            let p = sdl::SDL_GetKeyName(k);
            if p.is_null() { String::new() } else { CStr::from_ptr(p).to_string_lossy().into_owned() }
        };
        if !name.is_empty() {
            m.insert(u, name);
        }
    }
    m
}

// MARK: - Vibrations HD

/// La carte son d'une DualSense branchée en USB, ouverte avec quatre voies : les vibrations HD y vont (voies 3 et 4).
struct Haptics {
    device: u32,
}

impl Haptics {
    fn open() -> Option<Haptics> {
        let n = unsafe { sdl::SDL_GetNumAudioDevices(0) };
        let name = (0..n.max(0)).filter_map(|i| unsafe {
            let p = sdl::SDL_GetAudioDeviceName(i, 0);
            (!p.is_null()).then(|| CStr::from_ptr(p).to_string_lossy().into_owned())
        }).find(|n| n.contains("DualSense"));
        let Some(name) = name else {
            log::info!("PlayStation : pas de carte son de DualSense : vibrations simples");
            return None;
        };
        let want = sdl::SDL_AudioSpec { freq: 48_000, format: sdl::AUDIO_F32SYS, channels: 4, silence: 0, samples: 256, padding: 0, size: 0, callback: Some(fill_haptics), userdata: std::ptr::null_mut() };
        let mut have = sdl::SDL_AudioSpec::empty();
        let cname = std::ffi::CString::new(name.clone()).ok()?;
        // SDL_AUDIO_ALLOW_CHANNELS_CHANGE (4) : une sortie qui n'a que deux voies (profil stéréo de PipeWire) se voit.
        let dev = unsafe { sdl::SDL_OpenAudioDevice(cname.as_ptr(), 0, &want, &mut have, 4) };
        if dev == 0 {
            log::warn!("PlayStation : carte son de la DualSense « {name} » impossible ({})", sdl::error());
            return None;
        }
        if have.channels < 4 {
            unsafe { sdl::SDL_CloseAudioDevice(dev) };
            log::warn!("PlayStation : la carte son de la DualSense n'a que {} voies (profil « Surround 4.0 » à choisir) : vibrations simples", have.channels);
            return None;
        }
        CHANNELS.store(have.channels as usize, Ordering::Relaxed);
        dualsense::hd_haptics().set_output(true);
        unsafe { sdl::SDL_PauseAudioDevice(dev, 0) };
        log::info!("PlayStation : vibrations HD par « {name} » ({} voies, {} Hz)", have.channels, have.freq);
        Some(Haptics { device: dev })
    }
}

impl Drop for Haptics {
    fn drop(&mut self) {
        dualsense::hd_haptics().set_output(false);
        unsafe { sdl::SDL_CloseAudioDevice(self.device) };
    }
}

static CHANNELS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(4);

extern "C" fn fill_haptics(_user: *mut c_void, stream: *mut u8, len: c_int) {
    let n = len.max(0) as usize / 4;
    let out = unsafe { std::slice::from_raw_parts_mut(stream as *mut f32, n) };
    let channels = CHANNELS.load(Ordering::Relaxed).max(1);
    dualsense::hd_haptics().fill(out, n / channels, channels, 2);
}

// MARK: - Envoi entre appareils (format de l'app Mac)

/// Le paquet de « Envoyer à un appareil » : comptes (avec leur identifiant), consoles (associations, adresses,
/// Stations qui les voient).
pub fn bundle(app: &App) -> String {
    let accounts: Vec<_> = app.store.ps_accounts.iter().map(|a| serde_json::json!({ "id": a.id, "name": a.name, "psn_id": a.psn_id })).collect();
    let consoles: Vec<_> = app
        .store
        .ps_consoles
        .iter()
        .map(|c| {
            let pairings: serde_json::Map<String, serde_json::Value> = c.pairings.iter().map(|p| (p.account.clone(), serde_json::Value::String(p.registered.to_string()))).collect();
            let stations: Vec<_> = c
                .stations
                .iter()
                .filter_map(|id| app.store.host(id))
                .map(|h| serde_json::json!({ "id": h.id, "name": h.name, "address": h.address, "fingerprint": h.fingerprint }))
                .collect();
            serde_json::json!({
                "host_id": c.host_id, "name": c.name, "model": c.model, "addresses": c.addresses,
                "accounts": c.pairings.iter().map(|p| p.account.clone()).collect::<Vec<_>>(),
                "target": c.target, "pairings": pairings, "stations": stations,
            })
        })
        .collect();
    serde_json::json!({ "version": 1, "from": app.device_name, "accounts": accounts, "consoles": consoles }).to_string()
}

/// Le paquet reçu, fusionné : un compte au même identifiant PSN est le même (son nom pris s'il n'en avait pas),
/// une console au même `host_id` aussi ; les associations s'ajoutent. Rend les nombres du paquet.
pub fn merge(app: &mut App, text: &str) -> (usize, usize) {
    let Ok(v) = serde_json::from_str::<serde_json::Value>(text) else { return (0, 0) };
    let empty = Vec::new();
    let accounts = v["accounts"].as_array().unwrap_or(&empty);
    let consoles = v["consoles"].as_array().unwrap_or(&empty);
    // L'identifiant du compte chez l'envoyeur → ici.
    let mut ids: HashMap<String, String> = HashMap::new();
    for a in accounts {
        let psn = a["psn_id"].as_str().or_else(|| a["account"].as_str()).or_else(|| a["account_id"].as_str()).unwrap_or("").trim().to_string();
        let from = a["id"].as_str().unwrap_or("").to_string();
        if psn.is_empty() {
            continue;
        }
        let name = a["name"].as_str().map(str::to_string).filter(|n| !n.is_empty());
        let had_name = app.store.ps_accounts.iter().find(|x| x.psn_id == psn).is_some_and(|x| x.name.is_some());
        let (id, _) = app.store.put_ps_account(&psn, if had_name { None } else { name }, Some(from.clone()));
        ids.insert(from, id);
    }
    for c in consoles {
        let host_id = c["host_id"].as_str().unwrap_or("").to_ascii_uppercase();
        if host_id.is_empty() {
            continue;
        }
        let model = c["model"].as_str().unwrap_or("PS5").to_ascii_uppercase();
        let mut addresses: Vec<String> = c["addresses"].as_array().map(|l| l.iter().filter_map(|a| a.as_str().map(str::to_string)).collect()).unwrap_or_default();
        if let Some(a) = c["address"].as_str() {
            addresses.push(a.to_string());
        }
        // Les associations : un objet {compte: Registered (texte ou objet)}, ou une liste [{account, registered}].
        let mut pairings: Vec<(String, serde_json::Value)> = Vec::new();
        let reg = |r: &serde_json::Value| match r {
            serde_json::Value::String(s) => serde_json::from_str(s).ok(),
            serde_json::Value::Object(_) => Some(r.clone()),
            _ => None,
        };
        match &c["pairings"] {
            serde_json::Value::Object(m) => pairings.extend(m.iter().filter_map(|(k, r)| reg(r).map(|r| (k.clone(), r)))),
            serde_json::Value::Array(l) => pairings.extend(l.iter().filter_map(|p| Some((p["account"].as_str()?.to_string(), reg(&p["registered"])?)))),
            _ => {}
        }
        // Les Stations : reliées aux ordinateurs d'ici par l'identifiant, sinon l'empreinte, sinon l'adresse.
        let stations: Vec<String> = c["stations"]
            .as_array()
            .map(|l| {
                l.iter()
                    .filter_map(|s| {
                        let (sid, fp, addr) = (s["id"].as_str().or(s.as_str()), s["fingerprint"].as_str(), s["address"].as_str());
                        app.store.hosts.iter().find(|h| Some(h.id.as_str()) == sid || (fp.is_some() && h.fingerprint.as_deref() == fp) || Some(h.address.as_str()) == addr).map(|h| h.id.clone())
                    })
                    .collect()
            })
            .unwrap_or_default();
        let base = PsConsole { host_id: host_id.clone(), name: c["name"].as_str().unwrap_or("").to_string(), model, target: c["target"].as_str().map(str::to_string), addresses, ..Default::default() };
        for (from, registered) in pairings {
            let Some(account) = ids.get(&from).cloned() else { continue };
            // Une association déjà là pour ce compte et cette console reste.
            if app.store.console(&host_id).is_some_and(|x| x.pairings.iter().any(|p| p.account == account)) {
                continue;
            }
            app.store.put_pairing(base.clone(), &account, registered);
        }
        if let Some(cm) = app.store.console_mut(&host_id) {
            for s in stations {
                if !cm.stations.contains(&s) {
                    cm.stations.push(s);
                }
            }
        }
    }
    (accounts.len(), consoles.len())
}

/// Une association telle que le cœur la lit.
#[allow(dead_code)]
fn registered_of(p: &PsPairing) -> Option<Registered> {
    serde_json::from_value(p.registered.clone()).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::Layout;
    use crate::model::Store;

    #[test]
    fn default_layout_is_the_cores() {
        let ours = serde_json::to_value(crate::ps::PsLayout::default()).unwrap();
        let core = serde_json::to_value(KmLayout::default()).unwrap();
        assert_eq!(ours, core, "la disposition par défaut de l'interface est celle du cœur");
        let s = serde_json::to_value(crate::model::PsSettings::default()).unwrap();
        let c = serde_json::to_value(PlaySettings::default()).unwrap();
        assert_eq!(s, c, "les réglages par défaut sont ceux du cœur");
    }

    #[test]
    fn bundle_round_trip() {
        let registered = serde_json::json!({ "target": "ps5_v1", "nickname": "PS5", "mac": "A1B2C3D4E5F6", "regist_key": "00", "rp_key": "00", "rp_key_type": 0 });
        let mut a = App::new(Store::default(), Layout::Desktop);
        let (acc, _) = a.store.put_ps_account("AAAAAAAAAAA=", Some("Léa".into()), Some("ACC-1".into()));
        a.store.put_pairing(PsConsole { host_id: "A1B2C3D4E5F6".into(), name: "PS5 du salon".into(), model: "PS5".into(), addresses: vec!["10.0.0.5".into()], ..Default::default() }, &acc, registered.clone());
        let text = bundle(&a);
        let mut b = App::new(Store::default(), Layout::Tv);
        // Le receveur a déjà ce compte, sans nom : il prend celui du paquet.
        b.store.put_ps_account("AAAAAAAAAAA=", None, Some("OTHER".into()));
        assert_eq!(merge(&mut b, &text), (1, 1));
        assert_eq!(b.store.ps_accounts.len(), 1);
        assert_eq!(b.store.ps_accounts[0].id, "OTHER");
        assert_eq!(b.store.ps_accounts[0].name.as_deref(), Some("Léa"));
        let c = b.store.console("a1b2c3d4e5f6").expect("console");
        assert_eq!(c.pairings[0].account, "OTHER");
        assert_eq!(c.pairings[0].registered, registered);
        assert_eq!(c.addresses, vec!["10.0.0.5".to_string()]);
        // Reçu deux fois : rien en double.
        merge(&mut b, &text);
        assert_eq!(b.store.ps_consoles.len(), 1);
        assert_eq!(b.store.ps_consoles[0].pairings.len(), 1);
        // Le format toléré (webOS) : associations en liste, objet, `account`.
        let alt = serde_json::json!({ "accounts": [{ "id": "x", "name": "Tom", "account": "BBBBBBBBBBB=" }], "consoles": [{ "host_id": "ff00ff00ff00", "model": "ps4", "address": "10.0.0.6", "pairings": [{ "account": "x", "registered": registered }] }] });
        assert_eq!(merge(&mut b, &alt.to_string()), (1, 1));
        let c = b.store.console("FF00FF00FF00").expect("console");
        assert_eq!(c.model, "PS4");
        assert_eq!(c.addresses, vec!["10.0.0.6".to_string()]);
    }

    /// Essai à la main : « Recevoir d'un autre appareil » ouvert dans une app Link du réseau, ce test lui envoie un
    /// compte et une console d'essai (association factice) avec le code affiché :
    /// `KN_HANDOFF_CODE=482913 cargo test -p kybernow-linux -- --ignored handoff_send`.
    #[test]
    #[ignore]
    fn handoff_send() {
        let code = std::env::var("KN_HANDOFF_CODE").expect("KN_HANDOFF_CODE");
        let list = handoff::discover(Duration::from_millis(1500), &[]).unwrap();
        let w = list.first().expect("un appareil en attente");
        eprintln!("envoi à {} ({}, {})", w.name, w.platform, w.addr);
        let registered = serde_json::json!({ "target": "ps5_v1", "nickname": "PS5 d'essai", "mac": "0A0B0C0D0E0F", "regist_key": "00", "rp_key": "00", "rp_key_type": 0 }).to_string();
        let bundle = serde_json::json!({
            "from": "essai", "accounts": [{ "id": "ESSAI-1", "name": "Essai", "psn_id": "AQIDBAUGBwg=" }],
            "consoles": [{ "host_id": "0A0B0C0D0E0F", "name": "PS5 d'essai", "model": "PS5", "addresses": ["192.0.2.10"], "accounts": ["ESSAI-1"], "pairings": { "ESSAI-1": registered } }],
        });
        handoff::send(w.addr, &code, &bundle.to_string()).unwrap();
    }

    #[test]
    fn mouse_buttons() {
        assert_eq!([1, 2, 3, 4, 5].map(mouse_button), [0, 2, 1, 3, 4]);
    }
}
