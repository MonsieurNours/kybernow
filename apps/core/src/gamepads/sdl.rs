//! Le fil SDL du lecteur de manettes (feature `sdl` : Mac et Windows), à la
//! place de celui de kynput (`kynput/src/sdl2/client_gamepad.rs`, retiré de
//! libkynput par les scripts de construction de ces apps) : mêmes réglages
//! (fil des manettes de SDL sous Windows), même rythme (file vidée toutes les
//! 10 ms avec des manettes, toutes les 250 ms sans), mêmes vibrations (-1 :
//! 5 s), mais un seul fil pour toutes les sessions de l'app.
//!
//! Comme celui de kynput, il ouvre SDL sur un fil à lui, pas sur le fil
//! principal : sous macOS, les manettes passent par IOKit, HIDAPI et
//! GameController, dont SDL reçoit les annonces sur le fil principal (la
//! boucle d'AppKit de l'app) ; sous Windows, SDL tient son propre fil de
//! messages (`SDL_JOYSTICK_THREAD`). Rien d'autre n'est ouvert que les
//! manettes : ni vidéo ni son, et SDL ne pose pas ses gestionnaires de
//! signaux (Ctrl+C reste à l'app). Sans sous-système vidéo,
//! `SDL_WaitEventTimeout` tournerait 1 000 fois par seconde (`SDL_Delay(1)`) :
//! la file est vidée par `SDL_PollEvent`, puis le fil dort, réveillé plus tôt
//! par une vibration à jouer.
//!
//! Durée de vie : SDL garde ouvertes les manettes qu'il a ouvertes (diode de
//! joueur, mode HIDAPI des manettes Sony et Nintendo, manette réservée à
//! l'app pour certains pilotes). Comme celui de kynput, qui vivait avec sa
//! chaîne d'entrées, le fil ne tourne que tant qu'une chaîne d'entrées le
//! demande ([`acquire`], [`release`]) : 5 s après l'arrêt de la dernière (le
//! temps d'une image relancée), il ferme les manettes et SDL, puis repart à
//! la chaîne suivante. [`start`] avec `keep` le garde pour toute la vie de
//! l'app (`gamepads::start_sdl`).
//!
//! Les manettes sont numérotées dans l'ordre d'arrivée (0 à 3, joueurs 1 à
//! 4), comme sous Linux ; une cinquième est ignorée.

use std::ffi::{CStr, CString, c_char, c_int};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, mpsc};
use std::time::{Duration, Instant};

use super::{MAX_PADS, global};
use crate::input::pipeline::GamepadRumble;
use crate::input::{PadAxis, PadButton, PadId};

const SDL_INIT_GAMECONTROLLER: u32 = 0x2000;
const SDL_CONTROLLERAXISMOTION: u32 = 0x650;
const SDL_CONTROLLERBUTTONDOWN: u32 = 0x651;
const SDL_CONTROLLERBUTTONUP: u32 = 0x652;
const SDL_CONTROLLERDEVICEADDED: u32 = 0x653;
const SDL_CONTROLLERDEVICEREMOVED: u32 = 0x654;

/// Vibration sans durée (-1) : 5 s, comme kynput (le jeu l'arrête par une
/// vibration nulle).
const DEFAULT_RUMBLE_MS: u32 = 5000;
/// File vidée à ce rythme avec des manettes (celui de kynput) ; sans, plus lentement.
const PERIOD_PADS: Duration = Duration::from_millis(10);
const PERIOD_IDLE: Duration = Duration::from_millis(250);
/// Plus aucune chaîne d'entrées depuis ce temps : manettes et SDL fermés.
const IDLE_STOP: Duration = Duration::from_secs(5);

/// `SDL_Event` : 56 octets, champs lus par décalage (comme `apps/linux/src/device/sdl.rs`).
#[repr(C, align(8))]
struct SdlEvent {
    raw: [u8; 56],
}

impl SdlEvent {
    fn kind(&self) -> u32 {
        u32::from_ne_bytes(self.raw[0..4].try_into().unwrap())
    }
    /// Manette : identifiant (instance, ou index pour un ajout), bouton ou axe, valeur d'axe.
    fn pad(&self) -> (i32, u8, i16) {
        (i32::from_ne_bytes(self.raw[8..12].try_into().unwrap()), self.raw[12], i16::from_ne_bytes(self.raw[16..18].try_into().unwrap()))
    }
}

enum GameController {}
enum Joystick {}

unsafe extern "C" {
    fn SDL_SetHint(name: *const c_char, value: *const c_char) -> c_int;
    fn SDL_InitSubSystem(flags: u32) -> c_int;
    fn SDL_QuitSubSystem(flags: u32);
    fn SDL_GetError() -> *const c_char;
    fn SDL_PollEvent(event: *mut SdlEvent) -> c_int;
    fn SDL_IsGameController(index: c_int) -> c_int;
    fn SDL_GameControllerOpen(index: c_int) -> *mut GameController;
    fn SDL_GameControllerClose(c: *mut GameController);
    fn SDL_GameControllerName(c: *mut GameController) -> *const c_char;
    fn SDL_GameControllerGetJoystick(c: *mut GameController) -> *mut Joystick;
    fn SDL_JoystickInstanceID(j: *mut Joystick) -> i32;
    fn SDL_GameControllerGetVendor(c: *mut GameController) -> u16;
    fn SDL_GameControllerGetProduct(c: *mut GameController) -> u16;
    fn SDL_GameControllerRumble(c: *mut GameController, low: u16, high: u16, ms: u32) -> c_int;
}

fn hint(name: &str, value: &str) {
    let (n, v) = (CString::new(name).unwrap(), CString::new(value).unwrap());
    unsafe { SDL_SetHint(n.as_ptr(), v.as_ptr()) };
}

fn error() -> String {
    let p = unsafe { SDL_GetError() };
    if p.is_null() { String::new() } else { unsafe { CStr::from_ptr(p) }.to_string_lossy().into_owned() }
}

/// Qui demande le fil.
struct Life {
    /// Chaînes d'entrées vivantes qui lisent leurs manettes par ce fil.
    holders: usize,
    /// `gamepads::start_sdl` : pour toute la vie de l'app.
    keep: bool,
    /// Le fil tourne (SDL ouvert).
    alive: bool,
    /// Plus de chaîne depuis cet instant.
    idle_since: Option<Instant>,
}

static LIFE: Mutex<Life> = Mutex::new(Life { holders: 0, keep: false, alive: false, idle_since: None });
/// Un démarrage à la fois.
static START: Mutex<()> = Mutex::new(());
/// Ouverture et fermeture de SDL : jamais par deux fils à la fois (un fil qui
/// se ferme et le suivant qui démarre).
static SDL_LIFE: Mutex<()> = Mutex::new(());
static RUNNING: AtomicBool = AtomicBool::new(false);
/// Vibrations à jouer, posées par le fil réseau de kynput ; réveillent le fil.
static RUMBLES: Mutex<Option<mpsc::Sender<GamepadRumble>>> = Mutex::new(None);

pub(super) fn running() -> bool {
    RUNNING.load(Ordering::Relaxed)
}

/// Lance le fil s'il ne tourne pas et attend que SDL soit ouvert. `keep` : le
/// garder pour toute la vie de l'app.
pub(super) fn start(keep: bool) -> bool {
    let _one = START.lock().unwrap();
    {
        let mut life = LIFE.lock().unwrap();
        life.keep |= keep;
        if life.alive {
            return true;
        }
    }
    let (ready_tx, ready_rx) = mpsc::channel();
    let (rumble_tx, rumble_rx) = mpsc::channel();
    match std::thread::Builder::new().name("kybernow-manettes".into()).spawn(move || run(ready_tx, rumble_rx)) {
        Ok(_) => {
            let ok = ready_rx.recv().unwrap_or(false);
            if ok {
                *RUMBLES.lock().unwrap() = Some(rumble_tx);
            }
            ok
        }
        Err(e) => {
            log::error!("manettes : fil SDL impossible ({e})");
            false
        }
    }
}

/// Une chaîne d'entrées lit ses manettes par ce fil : il démarre s'il le faut.
pub(super) fn acquire() {
    {
        let mut life = LIFE.lock().unwrap();
        life.holders += 1;
        life.idle_since = None;
    }
    start(false);
}

/// Cette chaîne s'arrête : sans plus aucune, le fil se ferme après [`IDLE_STOP`].
pub(super) fn release() {
    let mut life = LIFE.lock().unwrap();
    life.holders = life.holders.saturating_sub(1);
    if life.holders == 0 {
        life.idle_since = Some(Instant::now());
    }
}

pub(super) fn rumble(r: GamepadRumble) {
    if let Some(tx) = RUMBLES.lock().unwrap().as_ref() {
        let _ = tx.send(r);
    }
}

/// Une manette ouverte.
struct Open {
    ctrl: *mut GameController,
    instance: i32,
    slot: PadId,
}

fn run(ready: mpsc::Sender<bool>, rumbles: mpsc::Receiver<GamepadRumble>) {
    {
        let _sdl = SDL_LIFE.lock().unwrap();
        // Pas de gestionnaires de signaux de SDL ; les manettes même quand une
        // fenêtre d'une autre app a le clavier (aucune fenêtre SDL ici).
        hint("SDL_NO_SIGNAL_HANDLERS", "1");
        hint("SDL_JOYSTICK_ALLOW_BACKGROUND_EVENTS", "1");
        // https://github.com/libsdl-org/SDL/issues/5380 (comme kynput).
        #[cfg(windows)]
        hint("SDL_JOYSTICK_THREAD", "1");
        if unsafe { SDL_InitSubSystem(SDL_INIT_GAMECONTROLLER) } != 0 {
            log::error!("manettes : SDL n'a pas pu s'ouvrir ({})", error());
            let _ = ready.send(false);
            return;
        }
        LIFE.lock().unwrap().alive = true;
        RUNNING.store(true, Ordering::Relaxed);
    }
    let _ = ready.send(true);
    log::info!("manettes : lues par le cœur (SDL), pour toutes les sessions");
    let mut open: Vec<Open> = Vec::new();
    let mut event = SdlEvent { raw: [0; 56] };
    loop {
        while unsafe { SDL_PollEvent(&mut event) } != 0 {
            handle(&event, &mut open);
        }
        if stop_now() {
            break;
        }
        // Dort jusqu'au tour suivant ; une vibration réveille tout de suite.
        let period = if open.is_empty() { PERIOD_IDLE } else { PERIOD_PADS };
        let deadline = Instant::now() + period;
        while let Ok(r) = rumbles.recv_timeout(deadline.saturating_duration_since(Instant::now())) {
            play_rumble(&open, r);
            while let Ok(r) = rumbles.try_recv() {
                play_rumble(&open, r);
            }
            if Instant::now() >= deadline {
                break;
            }
        }
    }
    // Plus aucune chaîne : manettes fermées (diodes, HIDAPI rendus), SDL fermé.
    let _sdl = SDL_LIFE.lock().unwrap();
    *RUMBLES.lock().unwrap() = None;
    for pad in open.drain(..) {
        unsafe { SDL_GameControllerClose(pad.ctrl) };
    }
    global().forget_all();
    unsafe { SDL_QuitSubSystem(SDL_INIT_GAMECONTROLLER) };
    RUNNING.store(false, Ordering::Relaxed);
    log::info!("manettes : plus de session, lecteur SDL fermé");
}

/// Plus de chaîne depuis assez longtemps (et pas gardé pour toute l'app) :
/// le fil s'arrête ; un démarrage qui arrive ensuite en lance un autre.
fn stop_now() -> bool {
    let mut life = LIFE.lock().unwrap();
    let idle = life.idle_since.is_some_and(|t| t.elapsed() >= IDLE_STOP);
    if life.keep || life.holders > 0 || !idle {
        return false;
    }
    life.alive = false;
    life.idle_since = None;
    true
}

fn play_rumble(open: &[Open], r: GamepadRumble) {
    let Some(pad) = open.iter().find(|p| p.slot == r.id) else { return };
    let ms = if r.duration_ms < 0 { DEFAULT_RUMBLE_MS } else { r.duration_ms as u32 };
    unsafe { SDL_GameControllerRumble(pad.ctrl, r.large_motor, r.small_motor, ms) };
}

fn handle(event: &SdlEvent, open: &mut Vec<Open>) {
    let kind = event.kind();
    let (which, index, value) = event.pad();
    match kind {
        SDL_CONTROLLERDEVICEADDED => add(which, open),
        SDL_CONTROLLERDEVICEREMOVED => {
            let Some(i) = open.iter().position(|p| p.instance == which) else { return };
            let pad = open.remove(i);
            unsafe { SDL_GameControllerClose(pad.ctrl) };
            log::info!("manettes : J{} débranchée", pad.slot + 1);
            global().unplug(pad.slot);
        }
        SDL_CONTROLLERBUTTONDOWN | SDL_CONTROLLERBUTTONUP => {
            let Some(pad) = open.iter().find(|p| p.instance == which) else { return };
            // Touches au-delà de la croix (palettes, pavé tactile…) : kynput les ignorait aussi.
            let Some(button) = PadButton::ALL.get(usize::from(index)).copied() else { return };
            global().button(pad.slot, button, kind == SDL_CONTROLLERBUTTONDOWN);
        }
        SDL_CONTROLLERAXISMOTION => {
            let Some(pad) = open.iter().find(|p| p.instance == which) else { return };
            let Some(axis) = PadAxis::ALL.get(usize::from(index)).copied() else { return };
            global().axis(pad.slot, axis, value);
        }
        _ => {}
    }
}

/// Une manette branchée (index SDL).
fn add(index: i32, open: &mut Vec<Open>) {
    unsafe {
        if SDL_IsGameController(index) == 0 {
            log::info!("manettes : appareil {index} ignoré (pas une manette)");
            return;
        }
        let ctrl = SDL_GameControllerOpen(index);
        if ctrl.is_null() {
            log::warn!("manettes : ouverture impossible ({})", error());
            return;
        }
        let instance = SDL_JoystickInstanceID(SDL_GameControllerGetJoystick(ctrl));
        if open.iter().any(|p| p.instance == instance) {
            // Déjà ouverte : SDL compte les ouvertures.
            SDL_GameControllerClose(ctrl);
            return;
        }
        let Some(slot) = (0..MAX_PADS as PadId).find(|s| !open.iter().any(|p| p.slot == *s)) else {
            log::warn!("manettes : {MAX_PADS} déjà branchées, la suivante est ignorée");
            SDL_GameControllerClose(ctrl);
            return;
        };
        let name = {
            let p = SDL_GameControllerName(ctrl);
            if p.is_null() { String::new() } else { CStr::from_ptr(p).to_string_lossy().into_owned() }
        };
        let id = |v: u16| (v != 0).then_some(v);
        let (vendor, product) = (id(SDL_GameControllerGetVendor(ctrl)), id(SDL_GameControllerGetProduct(ctrl)));
        log::info!("manettes : J{} {name} ({:04x}:{:04x})", slot + 1, vendor.unwrap_or(0), product.unwrap_or(0));
        open.push(Open { ctrl, instance, slot });
        global().plug_named(slot, vendor, product, name);
    }
}
