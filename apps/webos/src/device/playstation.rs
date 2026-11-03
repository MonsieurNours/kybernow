//! Consoles PlayStation sur le téléviseur (docs/playstation.md) : ce que le cœur fait (`kybernow_core::playstation`)
//! et ce que le téléviseur y met, comme le pont Apple (`apps/apple/src/playstation.rs`) pour les apps Apple.
//!
//! - **Recherche** : un fil qui cherche les consoles toutes les 3 s (diffusion, et en direct aux adresses retenues),
//!   sauf pendant une partie (une recherche faisait perdre des images sur le Mac).
//! - **Association** par le code de la console ; **compte saisi à la main** (numéro ou base64) ; **envoi entre
//!   appareils** (`handoff` : recevoir avec le code en grand, ou envoyer à un autre appareil).
//! - **Stations** : `GET /api/consoles` une fois par minute au plus par ordinateur, pour savoir quelles Stations
//!   voient chaque console ; hors de chez soi, la partie passe par l'une d'elles et le relais (`Remote`).
//! - **Partie** : `Play` du cœur, l'image sur le décodeur de jeu de NDL (`media::VideoSink`, le même fil que pour un
//!   ordinateur : HEVC, HDR10), le son Opus stéréo par SDL (`media::AudioSink`). Les manettes de SDL vont au lecteur
//!   du cœur (`gamepads::plug`, `button`, `axis`) pendant la partie ; vibrations et gâchettes adaptatives reviennent
//!   par `gamepads::set_app_feedback`. Sans manette, la Magic Remote et un clavier passent par la disposition par
//!   défaut du cœur (flèches : croix, OK : Croix, Effacement : Rond).

use std::net::{Ipv4Addr, SocketAddr};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use kybernow_core::console::{Console, ConsoleError};
use kybernow_core::input::{GamepadRumble, PadId};
use kybernow_core::playstation::discovery::{self, Model, State};
use kybernow_core::playstation::endpoint::Endpoint;
use kybernow_core::playstation::handoff;
use kybernow_core::playstation::play::{KeyboardEvent, PadEvent, Play, PlayEnd, PlayOptions, PlayState};
use kybernow_core::playstation::regist::{self, PsnId, RegistError, Registered, TargetName};
use kybernow_core::playstation::remote::{Remote, RemoteError};
use kybernow_core::playstation::settings::PlaySettings;
use kybernow_core::{gamepads, media::PacketHandler};

use super::media::{AudioSink, VideoSink};
use super::session::{ConsoleHooks, Shared};
use super::{pads, sdl, Event};
use crate::app::{Action, App, Phase, Screen};
use crate::model::{Host, PsConsole};
use crate::ps::{PairError, PsAction, PsEnd, PsFound, PsPairing, PsState, PsStats, PsWaiting};

/// Ce que les fils des consoles rapportent à la boucle principale.
pub enum PsEvent {
    Found(Vec<PsFound>),
    /// « Par adresse » : la console trouvée à cette adresse, ou rien (puis la Station si un port était donné).
    AtAddress { address: String, found: Option<PsFound> },
    Paired { pairing: PsPairing, result: Result<serde_json::Value, PairError> },
    Manual { result: Result<String, ()>, name: String, then_pair: bool },
    ReceiveCode(String),
    Received(String),
    ReceiveExpired,
    Waiting(Vec<PsWaiting>),
    Sent(Result<(), bool>),
    /// Les consoles que voit la Station de cet ordinateur (`host_id`).
    StationConsoles { host: String, consoles: Vec<String> },
    // La partie.
    Phase(Phase),
    Streaming { width: u32, height: u32 },
    CantDisplay(bool),
    Ended { end: PsEnd, detail: String },
    Keyboard(KeyboardEvent),
    Stats(PsStats),
    Route(crate::app::RouteUi),
    Effect { slot: usize, data: Vec<u8> },
}

fn send(tx: &Sender<Event>, e: PsEvent) {
    let _ = tx.send(Event::Ps(e));
}

fn model_name(m: Model) -> &'static str {
    m.name()
}

fn found_of(c: &discovery::Console) -> PsFound {
    let target = serde_json::to_value(TargetName::from_target(c.target())).ok().and_then(|v| v.as_str().map(str::to_string)).unwrap_or_default();
    PsFound {
        address: c.addr.to_string(),
        host_id: c.host_id.to_ascii_uppercase(),
        name: c.name.clone(),
        model: model_name(c.model()).to_string(),
        state: match c.state {
            State::Ready => PsState::On,
            State::Standby => PsState::Standby,
            State::Unknown => PsState::Unknown,
        },
        target,
        running_app: c.running_app.clone().filter(|a| !a.is_empty()),
    }
}

// MARK: - Recherche

/// Le fil de recherche : en pause pendant une partie ; `direct` : les adresses retenues des consoles.
pub struct Search {
    paused: AtomicBool,
    direct: Mutex<Vec<Ipv4Addr>>,
    now: AtomicBool,
}

impl Search {
    pub fn start(tx: Sender<Event>) -> Arc<Search> {
        let s = Arc::new(Search { paused: AtomicBool::new(false), direct: Mutex::new(Vec::new()), now: AtomicBool::new(true) });
        let me = s.clone();
        let _ = std::thread::Builder::new().name("ps-recherche".into()).spawn(move || {
            let mut last = Instant::now() - Duration::from_secs(60);
            loop {
                std::thread::sleep(Duration::from_millis(200));
                if me.paused.load(Ordering::Relaxed) {
                    continue;
                }
                if !me.now.swap(false, Ordering::Relaxed) && last.elapsed() < Duration::from_secs(3) {
                    continue;
                }
                let direct = me.direct.lock().unwrap().clone();
                let found = discovery::search(Duration::from_millis(1500), &direct).unwrap_or_default();
                last = Instant::now();
                if tx.send(Event::Ps(PsEvent::Found(found.iter().map(found_of).collect()))).is_err() {
                    return;
                }
            }
        });
        s
    }

    pub fn set_paused(&self, on: bool) {
        self.paused.store(on, Ordering::Relaxed);
        if !on {
            self.now.store(true, Ordering::Relaxed);
        }
    }

    pub fn now(&self) {
        self.now.store(true, Ordering::Relaxed);
    }

    pub fn set_direct(&self, store: &crate::model::PsStore) {
        let list: Vec<Ipv4Addr> = store.consoles.iter().flat_map(|c| c.addresses.iter()).filter_map(|a| a.parse().ok()).collect();
        *self.direct.lock().unwrap() = list;
    }
}

/// Un client d'Admin pour cet ordinateur : sur le réseau, ou par le relais (`relay` : l'ordinateur n'est joignable
/// que par lui).
fn admin(host: &Host, relay: bool) -> Option<Console> {
    let c = Console::new(&host.address, host.port, &host.user, &host.password, host.fingerprint.clone());
    if !relay {
        return Some(c);
    }
    c.with_relay(super::session::relay_route(host)).relay_twin()
}

/// Les consoles que voit la Station de cet ordinateur (`GET /api/consoles`).
pub fn station_consoles(host: Host, relay: bool, tx: Sender<Event>) {
    std::thread::spawn(move || {
        let Some(c) = admin(&host, relay) else { return };
        match c.consoles() {
            Ok(list) => send(&tx, PsEvent::StationConsoles { host: host.id.clone(), consoles: list.into_iter().map(|c| c.host_id.to_ascii_uppercase()).collect() }),
            Err(e) => log::debug!("PlayStation : consoles de {} illisibles ({e})", host.name),
        }
    });
}

// MARK: - Ce que l'interface demande (hors partie)

pub fn perform(a: PsAction, app: &mut App, tx: &Sender<Event>, search: &Search, playing: &mut Option<Playing>) {
    match a {
        PsAction::Play { host_id } => {
            if playing.is_some() {
                return;
            }
            let Some(c) = app.store.playstation.console(&host_id).cloned() else { return };
            *playing = Playing::start(app, &c, tx.clone());
            if playing.is_some() {
                search.set_paused(true);
            } else {
                app.ps_ended(PsEnd::NotPaired);
            }
        }
        PsAction::ProbeAddress { address, then_station } => {
            let tx = tx.clone();
            std::thread::spawn(move || {
                let found = address.parse::<Ipv4Addr>().ok().and_then(|ip| discovery::search_at(Duration::from_millis(1500), false, &[Endpoint::direct(ip)]).ok()).and_then(|l| l.first().map(found_of));
                match (found, then_station) {
                    (Some(f), _) => send(&tx, PsEvent::AtAddress { address, found: Some(f) }),
                    // Pas de console : la Station, comme avant.
                    (None, Some(port)) => super::jobs::probe(address, port, tx),
                    (None, None) => send(&tx, PsEvent::AtAddress { address, found: None }),
                }
            });
        }
        PsAction::Pair { pairing, psn_id, pin } => {
            let tx = tx.clone();
            std::thread::spawn(move || {
                let result = pair(&pairing, &psn_id, pin);
                send(&tx, PsEvent::Paired { pairing, result });
            });
        }
        PsAction::AddManual { id, name, then_pair } => {
            // Le numéro du compte, ou l'identifiant en base64 (celui des outils de chiaki-ng).
            let t = id.trim();
            let parsed = if !t.is_empty() && t.chars().all(|c| c.is_ascii_digit()) { t.parse::<u64>().ok().map(PsnId::from_user_id) } else { PsnId::from_base64(t) };
            let result = parsed.and_then(|p| p.to_base64()).ok_or(());
            send(tx, PsEvent::Manual { result, name, then_pair });
        }
        PsAction::ReceiveStart => receive_start(app.device_name(), tx.clone()),
        PsAction::ReceiveStop => receive_stop(),
        PsAction::Discover => {
            let tx = tx.clone();
            std::thread::spawn(move || {
                let list = handoff::discover(Duration::from_millis(1500), &[]).unwrap_or_default();
                send(&tx, PsEvent::Waiting(list.into_iter().map(|w| PsWaiting { name: w.name, platform: w.platform, address: w.addr.to_string() }).collect()));
            });
            search.now();
        }
        PsAction::Send { to, code } => {
            let tx = tx.clone();
            let bundle = app.ps_bundle();
            std::thread::spawn(move || {
                let r = match to.address.parse::<SocketAddr>() {
                    Ok(addr) => match handoff::send(addr, &code, &bundle) {
                        Ok(()) => Ok(()),
                        Err(handoff::SendError::Refused) => Err(true),
                        Err(handoff::SendError::Unreachable(e)) => {
                            log::info!("PlayStation : envoi impossible ({e})");
                            Err(false)
                        }
                    },
                    Err(_) => Err(false),
                };
                send(&tx, PsEvent::Sent(r));
            });
        }
        PsAction::Mute(on) => super::media::set_muted(on),
        PsAction::Sleep => {
            if let Some(p) = playing.as_ref() {
                p.with(|p| p.goto_bed());
            }
        }
        PsAction::Intensities(r, t) => {
            if let Some(p) = playing.as_ref() {
                p.with(|p| p.set_intensities(r, t));
            }
        }
        PsAction::KeyboardText(text) => {
            if let Some(p) = playing.as_ref() {
                p.with(|p| {
                    p.keyboard_text(&text);
                });
            }
        }
        PsAction::KeyboardAccept => {
            if let Some(p) = playing.as_ref() {
                p.with(|p| {
                    p.keyboard_accept();
                });
            }
        }
        PsAction::KeyboardReject => {
            if let Some(p) = playing.as_ref() {
                p.with(|p| {
                    p.keyboard_reject();
                });
            }
        }
    }
}

/// L'association : la cible de la console (lue à la découverte, ou cherchée), puis le code.
fn pair(p: &PsPairing, psn_id: &str, pin: u32) -> Result<serde_json::Value, PairError> {
    let ip: Ipv4Addr = p.address.parse().map_err(|_| PairError::Unreachable)?;
    let at = Endpoint::direct(ip);
    let target = match serde_json::from_value::<TargetName>(serde_json::Value::String(p.target.clone())) {
        Ok(t) => t,
        Err(_) => {
            let found = discovery::search_at(Duration::from_millis(1500), false, &[at.clone()]).map_err(|_| PairError::Unreachable)?;
            TargetName::from_target(found.first().ok_or(PairError::Unreachable)?.target())
        }
    };
    let id = PsnId::from_base64(psn_id).ok_or(PairError::Account)?;
    match regist::regist_at(&at, target.into(), &id, pin) {
        Ok(r) => serde_json::to_value(r).map_err(|_| PairError::Unreachable),
        Err(e) => {
            log::warn!("PlayStation : association refusée : {e}");
            Err(match &e {
                RegistError::NoAnswer | RegistError::Refused => PairError::NoAnswer,
                RegistError::Rejected { reason: Some(regist::REASON_REGIST_FAILED), .. } => PairError::WrongCode,
                RegistError::Rejected { reason: Some(regist::REASON_INVALID_PSN_ID), .. } => PairError::Account,
                _ => PairError::Unreachable,
            })
        }
    }
}

// MARK: - Recevoir d'un autre appareil

static RECEIVING: Mutex<Option<Arc<AtomicBool>>> = Mutex::new(None);

fn receive_start(name: String, tx: Sender<Event>) {
    receive_stop();
    let stop = Arc::new(AtomicBool::new(false));
    *RECEIVING.lock().unwrap() = Some(stop.clone());
    std::thread::spawn(move || {
        let r = match handoff::Receiver::start(&name, "webos") {
            Ok(r) => r,
            Err(e) => {
                log::warn!("PlayStation : attente d'un envoi impossible ({e})");
                send(&tx, PsEvent::ReceiveExpired);
                return;
            }
        };
        send(&tx, PsEvent::ReceiveCode(r.code()));
        while !stop.load(Ordering::Relaxed) {
            match r.next(Duration::from_millis(300)) {
                Some(handoff::Event::Received(bundle)) => {
                    log::info!("PlayStation : comptes et consoles reçus d'un autre appareil");
                    send(&tx, PsEvent::Received(bundle));
                    break;
                }
                Some(handoff::Event::NewCode(code)) => send(&tx, PsEvent::ReceiveCode(code)),
                Some(handoff::Event::Expired) => {
                    send(&tx, PsEvent::ReceiveExpired);
                    break;
                }
                None => {}
            }
        }
        r.stop();
    });
}

fn receive_stop() {
    if let Some(s) = RECEIVING.lock().unwrap().take() {
        s.store(true, Ordering::Relaxed);
    }
}

// MARK: - Ce que les fils rapportent

pub fn handle(e: PsEvent, app: &mut App, actions: &mut Vec<Action>, search: &Search, playing: &mut Option<Playing>, pads: &mut pads::Pads) {
    match e {
        PsEvent::Found(list) => {
            if playing.is_none() {
                app.ps_found(list);
                search.set_direct(&app.store.playstation);
            }
        }
        PsEvent::AtAddress { address, found } => {
            app.add.busy = false;
            match found {
                Some(f) => {
                    app.add.message = None;
                    // L'adresse est retenue : la console y est cherchée en direct ensuite.
                    app.store.playstation.put_console(&f.host_id, &f.name, &f.model, &f.target, Some(&address));
                    app.store.save();
                    search.set_direct(&app.store.playstation);
                    if !app.ps.found.iter().any(|x| x.host_id == f.host_id) {
                        app.ps.found.push(f.clone());
                    }
                    let paired = app.store.playstation.console(&f.host_id).is_some_and(|c| !c.pairings.is_empty());
                    if paired && app.ps_candidates(&f.host_id).is_empty() {
                        app.store.selected = Some(format!("ps:{}", f.host_id));
                        app.go(Screen::Home);
                    } else {
                        let p = PsPairing { host_id: f.host_id, name: f.name, model: f.model, address, target: f.target, account: None };
                        actions.extend(app.ps_start_pairing(p));
                    }
                }
                None => app.add.message = Some(PairError::Unreachable.text().into()),
            }
        }
        PsEvent::Paired { pairing, result } => match result {
            Ok(r) => {
                app.ps_paired(&pairing, r);
                search.set_direct(&app.store.playstation);
            }
            Err(e) => app.ps_pair_failed(e),
        },
        PsEvent::Manual { result, name, then_pair } => actions.extend(app.ps_manual_done(result, name, then_pair)),
        PsEvent::ReceiveCode(code) => {
            app.ps.add.receive_code = Some(code);
            app.ps.add.receive_expired = false;
        }
        PsEvent::Received(bundle) => {
            actions.extend(app.ps_received(&bundle));
            search.set_direct(&app.store.playstation);
        }
        PsEvent::ReceiveExpired => {
            app.ps.add.receive_expired = true;
            if app.screen == crate::app::Screen::PlayStation && app.ps.add.step == crate::ps::PsStep::Receive {
                app.focus = crate::app::Target::Ps(crate::ps::PsTarget::NewCode);
            }
        }
        PsEvent::Waiting(list) => app.ps.add.waiting = list,
        PsEvent::Sent(r) => app.ps_sent(r),
        PsEvent::StationConsoles { host, consoles } => {
            let mut changed = false;
            for c in app.store.playstation.consoles.iter_mut() {
                let sees = consoles.iter().any(|h| h.eq_ignore_ascii_case(&c.host_id));
                let has = c.stations.contains(&host);
                if sees && !has {
                    c.stations.push(host.clone());
                    changed = true;
                }
            }
            if changed {
                app.store.save();
            }
        }
        PsEvent::Phase(p) => app.ps_phase(p),
        PsEvent::Streaming { width, height } => {
            if let Some(s) = app.session.as_mut().and_then(|s| s.ps.as_mut()) {
                let st = s.stats.get_or_insert_with(PsStats::default);
                st.width = width;
                st.height = height;
            }
            app.ps_phase(Phase::FirstImages);
        }
        PsEvent::CantDisplay(on) => {
            if let Some(s) = app.session.as_mut().and_then(|s| s.ps.as_mut()) {
                s.cant_display = on;
            }
        }
        PsEvent::Ended { end, detail } => {
            if !detail.is_empty() {
                log::info!("PlayStation : partie finie ({end:?}, {detail})");
            }
            app.ps_ended(end);
        }
        PsEvent::Keyboard(k) => {
            let Some(s) = app.session.as_mut().and_then(|s| s.ps.as_mut()) else { return };
            match k {
                KeyboardEvent::Open(t) => {
                    s.keyboard = Some(t);
                    app.focus = crate::app::Target::Key('q');
                }
                KeyboardEvent::Text(t) => {
                    if s.keyboard.is_some() {
                        s.keyboard = Some(t);
                    }
                }
                KeyboardEvent::Closed => s.keyboard = None,
            }
        }
        PsEvent::Stats(st) => {
            if let Some(s) = app.session.as_mut().and_then(|s| s.ps.as_mut()) {
                let (lost, recovered) = s.stats.as_ref().map(|o| (o.lost, o.recovered)).unwrap_or((0, 0));
                s.stats = Some(PsStats { lost: lost + st.lost, recovered: recovered + st.recovered, ..st });
            }
        }
        PsEvent::Route(r) => {
            if let Some(s) = app.session.as_mut() {
                s.route = Some(r);
            }
        }
        PsEvent::Effect { slot, data } => pads.effect(slot, &data),
    }
}

// MARK: - La partie

/// Une partie en cours : `Play` du cœur (créé quand la console est jointe et réveillée), la Station et le relais hors
/// de chez soi.
pub struct Playing {
    play: Arc<Mutex<Option<Play>>>,
    stop: Arc<AtomicBool>,
    remote: Arc<Mutex<Option<Remote>>>,
    captured: bool,
    /// Les manettes branchées au lecteur du cœur pour la partie (joueurs de `pads::Pads`).
    plugged: Vec<usize>,
}

/// Les crochets de l'image (`Shared::console`) : images clés demandées à la console, première image montrée.
struct Hooks {
    play: Arc<Mutex<Option<Play>>>,
    shown: AtomicBool,
    tx: Mutex<Sender<Event>>,
}

impl ConsoleHooks for Hooks {
    fn keyframe(&self, lost: bool) {
        if let Some(p) = self.play.lock().unwrap().as_ref() {
            if lost { p.key_frame_lost() } else { p.key_frame_recovered() }
        }
    }

    fn frame_shown(&self) {
        if !self.shown.swap(true, Ordering::Relaxed) {
            let _ = self.tx.lock().unwrap().send(Event::Ps(PsEvent::Phase(Phase::Streaming)));
        }
    }
}

/// Vibrations et gâchettes adaptatives de la partie, jouées par les manettes de SDL (boucle principale).
struct Feedback(Mutex<Sender<Event>>);

impl gamepads::AppFeedback for Feedback {
    fn rumble(&self, r: GamepadRumble) {
        let _ = self.0.lock().unwrap().send(Event::Rumble(r));
    }

    fn effect(&self, id: PadId, data: &[u8]) {
        let _ = self.0.lock().unwrap().send(Event::Ps(PsEvent::Effect { slot: id as usize, data: data.to_vec() }));
    }
}

impl Playing {
    /// Lance la partie : un fil joint la console (en direct, ou par une Station et le relais), la réveille si elle
    /// est en veille, puis ouvre la partie. `None` : pas d'association lisible.
    fn start(app: &App, c: &PsConsole, tx: Sender<Event>) -> Option<Playing> {
        let account = c.play_account(&app.store.playstation.accounts)?;
        let registered: Registered = serde_json::from_value(c.pairings.get(&account.id)?.clone()).ok()?;
        let mut settings = serde_json::from_value::<PlaySettings>(serde_json::to_value(&c.settings).ok()?).unwrap_or_default();
        settings.hdr = c.hdr && app.ps_hdr_available(c);
        let state = app.ps_state(&c.host_id);
        let address = app.ps.found.iter().find(|f| f.host_id == c.host_id).map(|f| f.address.clone()).or_else(|| c.addresses.first().cloned());
        // Hors de chez soi : les Stations qui voient la console et que le relais joint.
        let stations: Vec<Host> = if state == PsState::Away {
            c.stations.iter().filter_map(|id| app.store.host(id)).filter(|h| app.live.get(&h.id).is_some_and(|l| l.online == crate::app::Online::Away)).cloned().collect()
        } else {
            Vec::new()
        };
        let model = if c.ps5() { Model::Ps5 } else { Model::Ps4 };
        let refresh = app.tv.refresh;
        let hdr = settings.hdr;
        let intensities = settings.intensities();
        let fps_target = if settings.fps == 30 { 30 } else { 60 };
        let name = c.name.clone();
        let host_id = c.host_id.clone();
        let play = Arc::new(Mutex::new(None));
        let stop = Arc::new(AtomicBool::new(false));
        let remote: Arc<Mutex<Option<Remote>>> = Arc::new(Mutex::new(None));
        log::info!("PlayStation : partie sur {name} ({}, {}, HDR {hdr})", if stations.is_empty() { "sur le réseau" } else { "par une Station et le relais" }, if state == PsState::Standby { "en veille" } else { "allumée" });
        gamepads::set_app_feedback(Some(Arc::new(Feedback(Mutex::new(tx.clone())))));
        {
            let (play, stop, remote) = (play.clone(), stop.clone(), remote.clone());
            std::thread::Builder::new()
                .name("ps-depart".into())
                .spawn(move || {
                    let ended = |end: PsEnd, detail: String| send(&tx, PsEvent::Ended { end, detail });
                    // Où joindre la console.
                    let (at, mut standby) = if stations.is_empty() {
                        let Some(ip) = address.as_deref().and_then(|a| a.parse::<Ipv4Addr>().ok()) else {
                            return ended(PsEnd::Unreachable, "pas d'adresse".into());
                        };
                        (Endpoint::direct(ip), state == PsState::Standby)
                    } else {
                        match reach(&stations, &host_id, &remote, &tx) {
                            Ok(x) => x,
                            Err(text) => {
                                // Les textes d'un ordinateur par le relais, avec le nom de la Station.
                                return send(&tx, PsEvent::Ended { end: PsEnd::Unreachable, detail: text });
                            }
                        }
                    };
                    // Réveil, puis la console est cherchée jusqu'à ce qu'elle dise être allumée (une minute au plus).
                    if standby {
                        send(&tx, PsEvent::Phase(Phase::Preparing));
                        let Some(cred) = registered.wake_credential() else { return ended(PsEnd::NotPaired, String::new()) };
                        let _ = discovery::wake_at(&at, model, cred);
                        let t0 = Instant::now();
                        while standby {
                            if stop.load(Ordering::Relaxed) {
                                return ended(PsEnd::Stopped, String::new());
                            }
                            if t0.elapsed() > Duration::from_secs(60) {
                                return ended(PsEnd::Unreachable, "pas réveillée en 60 s".into());
                            }
                            std::thread::sleep(Duration::from_millis(800));
                            standby = !discovery::search_at(Duration::from_millis(700), false, std::slice::from_ref(&at)).unwrap_or_default().iter().any(|c| c.state == State::Ready);
                            if t0.elapsed() > Duration::from_secs(10) && standby {
                                // Un paquet de réveil perdu : renvoyé.
                                let _ = discovery::wake_at(&at, model, cred);
                            }
                        }
                    }
                    if stop.load(Ordering::Relaxed) {
                        return ended(PsEnd::Stopped, String::new());
                    }
                    send(&tx, PsEvent::Phase(Phase::Connecting));
                    let hooks = Arc::new(Hooks { play: play.clone(), shown: AtomicBool::new(false), tx: Mutex::new(tx.clone()) });
                    let shared = Shared::for_console(tx.clone(), refresh, hdr, hooks.clone());
                    let video: Arc<dyn PacketHandler> = VideoSink::new(shared.clone());
                    let audio: Arc<dyn PacketHandler> = AudioSink::new(false, false);
                    let tx_state = Mutex::new(tx.clone());
                    let sh = shared.clone();
                    let on_state = Box::new(move |s: PlayState| {
                        let e = match s {
                            PlayState::Connecting => PsEvent::Phase(Phase::Connecting),
                            PlayState::Streaming { width, height, hdr } => {
                                sh.set_video_size(width, height);
                                sh.set_hdr_expected(hdr);
                                // Une partie reprise par le cœur (réseau) : la première image montrée finit de nouveau
                                // la connexion.
                                hooks.shown.store(false, Ordering::Relaxed);
                                PsEvent::Streaming { width, height }
                            }
                            PlayState::CantDisplay(on) => PsEvent::CantDisplay(on),
                            PlayState::Ended(end) => {
                                let (end, detail) = match end {
                                    PlayEnd::Stopped => (PsEnd::Stopped, String::new()),
                                    PlayEnd::InUse => (PsEnd::InUse, String::new()),
                                    PlayEnd::NotPaired => (PsEnd::NotPaired, String::new()),
                                    PlayEnd::Unreachable(d) => (PsEnd::Unreachable, d),
                                    PlayEnd::Remote(d) => (PsEnd::Remote, d),
                                };
                                PsEvent::Ended { end, detail }
                            }
                        };
                        let _ = tx_state.lock().unwrap().send(Event::Ps(e));
                    });
                    let tx_pad = Mutex::new(tx.clone());
                    let on_pad = Box::new(move |e: PadEvent| {
                        // Options + Create + L1 + R1 tenus 250 ms : la barre de la partie. La navigation dans la barre
                        // vient des manettes de SDL elles-mêmes (comme pour un ordinateur).
                        if e == PadEvent::Menu {
                            let _ = tx_pad.lock().unwrap().send(Event::Combo);
                        }
                    });
                    let p = Play::start(at, &registered, PlayOptions { settings, hevc_decoder: true }, video, Some(audio), on_state, on_pad);
                    let tx_kb = Mutex::new(tx.clone());
                    p.set_on_keyboard(Some(Box::new(move |k: KeyboardEvent| {
                        let _ = tx_kb.lock().unwrap().send(Event::Ps(PsEvent::Keyboard(k)));
                    })));
                    p.set_intensities(intensities.0, intensities.1);
                    *play.lock().unwrap() = Some(p);
                    // Arrêtée pendant l'ouverture : la partie est arrêtée tout de suite (sa fin arrive comme d'habitude).
                    if stop.load(Ordering::Relaxed) {
                        let p = play.lock().unwrap().take();
                        if let Some(mut p) = p {
                            p.stop();
                        }
                        return;
                    }
                    // Les mesures, une fois par seconde, et le chemin par le relais.
                    while !stop.load(Ordering::Relaxed) {
                        std::thread::sleep(Duration::from_secs(1));
                        let stats = play.lock().unwrap().as_ref().map(|p| p.stats());
                        let Some(st) = stats else { break };
                        send(&tx, PsEvent::Stats(PsStats { width: st.width, height: st.height, fps_target, hevc: st.hevc, hdr: st.hdr, target_kbps: st.target_kbps, fps: st.fps, mbps: st.mbps, lost: st.lost, recovered: st.recovered }));
                        if let Some(r) = remote.lock().unwrap().as_ref() {
                            send(&tx, PsEvent::Route(super::session::route_of(&r.route_info())));
                        }
                    }
                })
                .ok()?;
        }
        Some(Playing { play, stop, remote, captured: false, plugged: Vec::new() })
    }

    /// Appelle la partie, si elle est ouverte.
    pub fn with(&self, f: impl FnOnce(&Play)) {
        if let Some(p) = self.play.lock().unwrap().as_ref() {
            f(p);
        }
    }

    /// La barre, le clavier de la console ou un autre écran sont ouverts : la console reçoit une manette au repos.
    pub fn set_captured(&mut self, on: bool) {
        if self.captured != on {
            self.captured = on;
            self.with(|p| p.set_captured(on));
        }
    }

    /// Arrête la partie (sans attendre : la fin arrive par `PsEvent::Ended`).
    pub fn stop(&self) {
        self.stop.store(true, Ordering::Relaxed);
        let p = self.play.lock().unwrap().take();
        let remote = self.remote.clone();
        std::thread::spawn(move || {
            if let Some(mut p) = p {
                p.stop();
            }
            remote.lock().unwrap().take();
        });
    }

    /// La partie est finie : manettes rendues, retour des manettes à l'app, recherche reprise.
    pub fn finish(mut self, pads: &mut pads::Pads, search: &Search) {
        self.stop();
        for slot in self.plugged.drain(..) {
            gamepads::unplug(slot as PadId);
        }
        gamepads::set_app_feedback(None);
        for slot in pads.slots() {
            pads.rumble(slot, 0, 0, 1);
        }
        super::media::set_muted(false);
        search.set_paused(false);
    }

    /// Les manettes branchées vont au lecteur du cœur (une manette branchée exclut la Magic Remote et le clavier).
    pub fn plug_all(&mut self, pads: &pads::Pads) {
        for slot in pads.slots() {
            if !self.plugged.contains(&slot) {
                let (v, p) = pads.ids(slot);
                gamepads::plug(slot as PadId, Some(v), Some(p));
                self.plugged.push(slot);
            }
        }
    }
}

/// Joint la console par une des Stations qui la voient, et le relais : l'adresse de la console par le tunnel, et si
/// elle est en veille. L'erreur : le texte à montrer.
fn reach(stations: &[Host], host_id: &str, remote: &Arc<Mutex<Option<Remote>>>, tx: &Sender<Event>) -> Result<(Endpoint, bool), String> {
    let mut last = crate::ps::end_text(PsEnd::Unreachable).unwrap_or_default();
    for h in stations {
        let Some(c) = admin(h, true) else { continue };
        match Remote::open(&c, host_id) {
            Ok(r) => {
                let at = r.endpoint();
                let standby = r.seen().is_some_and(|s| s.state == "standby");
                send(tx, PsEvent::Route(super::session::route_of(&r.route_info())));
                *remote.lock().unwrap() = Some(r);
                return Ok((at, standby));
            }
            Err(RemoteError::Console(ConsoleError::Relay(e))) => {
                log::info!("PlayStation : relais par {} : {}", h.name, e.kind());
                last = super::session::relay_failure_text(Some(&e), None, &h.name).unwrap_or(last);
            }
            Err(e) => log::info!("PlayStation : {} ne joint pas la console ({e})", h.name),
        }
    }
    Err(last)
}

/// Les événements de SDL pendant une partie sur une console. Rend vrai s'il est traité ici (sinon, le chemin commun :
/// la barre ou le clavier de la console ouverts se pilotent comme les menus de l'app).
pub fn handle_sdl(ev: &sdl::SDL_Event, app: &mut App, pads: &mut pads::Pads, playing: &mut Playing, actions: &mut Vec<Action>, dirty: &mut bool) -> bool {
    let Some(s) = app.session.as_ref().filter(|s| s.ps.is_some()) else { return false };
    let in_game = app.screen == Screen::Session && !s.menu && s.ps.as_ref().is_some_and(|p| p.keyboard.is_none()) && matches!(s.phase, Phase::Streaming | Phase::Reconnecting { .. });
    match ev.kind() {
        sdl::SDL_KEYDOWN | sdl::SDL_KEYUP if in_game => {
            let down = ev.kind() == sdl::SDL_KEYDOWN;
            let (sc, repeat) = ev.key();
            // Retour, Quitter et le bouton rouge : la barre de la partie.
            if sc == sdl::SC_WEBOS_BACK || sc == sdl::SC_AC_BACK || sc == sdl::SC_WEBOS_EXIT || sc == sdl::SC_WEBOS_RED {
                if down && !repeat {
                    actions.extend(app.open_menu());
                    *dirty = true;
                }
                return true;
            }
            // Le scancode de SDL est l'usage USB HID : la disposition du cœur le traduit en manette.
            if !repeat && (4..=231).contains(&sc) {
                playing.with(|p| p.key(sc as u16, down));
            }
            true
        }
        sdl::SDL_TEXTINPUT | sdl::SDL_MOUSEMOTION | sdl::SDL_MOUSEBUTTONDOWN | sdl::SDL_MOUSEBUTTONUP | sdl::SDL_MOUSEWHEEL if in_game => true,
        sdl::SDL_CONTROLLERDEVICEADDED => {
            let (index, _, _) = ev.pad();
            if let Some(slot) = pads.add(index) {
                playing.plug_all(pads);
                if let Some(s) = app.session.as_mut() {
                    s.notice = Some((format!("{} {}{} : {}", crate::i18n::tr("Manette", "Controller"), crate::i18n::tr("J", "P"), slot + 1, pads.name(slot)), Instant::now()));
                }
                *dirty = true;
            }
            true
        }
        sdl::SDL_CONTROLLERDEVICEREMOVED => {
            let (instance, _, _) = ev.pad();
            if let Some(slot) = pads.remove(instance) {
                gamepads::unplug(slot as PadId);
                playing.plugged.retain(|s| *s != slot);
                *dirty = true;
            }
            true
        }
        sdl::SDL_CONTROLLERBUTTONDOWN | sdl::SDL_CONTROLLERBUTTONUP | sdl::SDL_CONTROLLERAXISMOTION => {
            let (instance, which, value) = ev.pad();
            let Some(slot) = pads.slot_of(instance) else { return true };
            let is_button = ev.kind() != sdl::SDL_CONTROLLERAXISMOTION;
            let down = ev.kind() == sdl::SDL_CONTROLLERBUTTONDOWN;
            pads.update(slot, is_button, which, down, value);
            let id = slot as PadId;
            if in_game {
                if is_button && which == 20 {
                    // Le clic du pavé tactile (DualSense, DualShock 4).
                    gamepads::global().touch_click(id, down);
                } else if is_button {
                    if let Some(b) = pads::core_button(which) {
                        gamepads::button(id, b, down);
                    }
                } else if let Some(a) = pads::core_axis(which) {
                    gamepads::axis(id, a, value);
                }
                return true;
            }
            // Barre ou clavier ouverts, connexion : les relâchements vont quand même au lecteur (rien ne reste tenu),
            // la manette pilote l'app.
            if is_button && !down {
                if let Some(b) = pads::core_button(which) {
                    gamepads::button(id, b, false);
                }
            }
            if let Some(n) = pads.nav(is_button, which, down, value) {
                actions.extend(app.nav(n));
                *dirty = true;
            }
            true
        }
        _ => false,
    }
}

/// La console doit-elle recevoir une manette au repos (barre, clavier de la console, autre écran) ?
pub fn want_captured(app: &App) -> bool {
    let Some(s) = app.session.as_ref() else { return true };
    !(app.screen == Screen::Session && !s.menu && s.ps.as_ref().is_some_and(|p| p.keyboard.is_none()) && matches!(s.phase, Phase::Streaming | Phase::Reconnecting { .. }))
}
