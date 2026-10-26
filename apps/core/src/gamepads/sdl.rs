//! Le fil SDL du lecteur de manettes (feature `sdl` : Mac et Windows), à la
//! place de celui de kynput (`kynput/src/sdl2/client_gamepad.rs`, retiré de
//! libkynput par les scripts de construction de ces apps) : mêmes réglages
//! (fil des manettes de SDL sous Windows), mêmes vibrations (-1 : 5 s), mais
//! un seul fil pour toutes les sessions de l'app.
//!
//! Comme celui de kynput, il ouvre SDL sur un fil à lui, pas sur le fil
//! principal : sous macOS, les manettes passent par IOKit, HIDAPI et
//! GameController, dont SDL reçoit les annonces sur le fil principal (la
//! boucle d'AppKit de l'app) ; sous Windows, SDL tient son propre fil de
//! messages (`SDL_JOYSTICK_THREAD`). Rien d'autre n'est ouvert que les
//! manettes : ni vidéo ni son, et SDL ne pose pas ses gestionnaires de
//! signaux (Ctrl+C reste à l'app).
//!
//! Les manettes sont numérotées dans l'ordre d'arrivée (0 à 3, joueurs 1 à
//! 4), comme sous Linux ; une cinquième est ignorée.

use std::collections::VecDeque;
use std::ffi::{CStr, CString, c_char, c_int};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock, mpsc};

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
    fn SDL_GetError() -> *const c_char;
    fn SDL_WaitEventTimeout(event: *mut SdlEvent, timeout: c_int) -> c_int;
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

static RUNNING: AtomicBool = AtomicBool::new(false);
static STARTED: OnceLock<bool> = OnceLock::new();
/// Vibrations à jouer, posées par le fil réseau de kynput.
static RUMBLES: Mutex<VecDeque<GamepadRumble>> = Mutex::new(VecDeque::new());

pub(super) fn running() -> bool {
    RUNNING.load(Ordering::Relaxed)
}

/// Lance le fil (une fois) et attend que SDL soit ouvert.
pub(super) fn start() -> bool {
    *STARTED.get_or_init(|| {
        let (tx, rx) = mpsc::channel();
        match std::thread::Builder::new().name("kybernow-manettes".into()).spawn(move || run(tx)) {
            Ok(_) => rx.recv().unwrap_or(false),
            Err(e) => {
                log::error!("manettes : fil SDL impossible ({e})");
                false
            }
        }
    })
}

pub(super) fn rumble(r: GamepadRumble) {
    if running() {
        RUMBLES.lock().unwrap().push_back(r);
    }
}

/// Une manette ouverte.
struct Open {
    ctrl: *mut GameController,
    instance: i32,
    slot: PadId,
}

fn run(ready: mpsc::Sender<bool>) {
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
    RUNNING.store(true, Ordering::Relaxed);
    let _ = ready.send(true);
    log::info!("manettes : lues par le cœur (SDL), pour toutes les sessions");
    let mut open: Vec<Open> = Vec::new();
    let mut event = SdlEvent { raw: [0; 56] };
    loop {
        // 10 ms avec des manettes (vibrations), 250 ms sinon.
        let timeout = if open.is_empty() { 250 } else { 10 };
        let got = unsafe { SDL_WaitEventTimeout(&mut event, timeout) };
        play_rumbles(&open);
        if got != 0 {
            handle(&event, &mut open);
        }
    }
}

fn play_rumbles(open: &[Open]) {
    let pending: Vec<GamepadRumble> = RUMBLES.lock().unwrap().drain(..).collect();
    for r in pending {
        let Some(pad) = open.iter().find(|p| p.slot == r.id) else { continue };
        let ms = if r.duration_ms < 0 { DEFAULT_RUMBLE_MS } else { r.duration_ms as u32 };
        unsafe { SDL_GameControllerRumble(pad.ctrl, r.large_motor, r.small_motor, ms) };
    }
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
