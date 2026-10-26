//! Les manettes (SDL GameController) : quatre joueurs au plus, dans l'ordre
//! d'arrivée ; vibrations des deux moteurs (et des gâchettes quand la
//! manette en a) ; la manette pilote l'app hors du jeu (croix et stick :
//! flèches, A : OK, B : retour, Y : réglages de l'ordinateur, X : ajouter,
//! L1 / R1 : profil ou rubrique, Start : réglages de l'app) ; manette en souris.
//! Et la conversion des touches du clavier (codes USB de SDL) en scancodes
//! de Windows, ceux qu'attend kynput.
//!
//! Plusieurs ordinateurs à la fois : le routage vers les sessions (branchements sur chacune,
//! boutons et axes à la seule session active, retour au repos, rebranchement quand une session
//! rejoint son ordinateur) est celui du lecteur unique du cœur (`kybernow_core::gamepads`), que la
//! boucle SDL nourrit (`plug`, `unplug`, `button`, `axis`). Ici reste ce que le lecteur n'a pas :
//! les manettes que SDL a ouvertes (vibrations, batterie), l'état montré par l'écran de test, la
//! navigation de l'interface et la manette en souris.

use std::sync::Arc;
use std::time::{Duration, Instant};

use keycode::{KeyMap, KeyMapping};
use kybernow_core::input::{InputPipeline, PadAxis, PadButton};
use kybernow_core::kynput_rs::MouseButtonType;

use super::sdl;
use super::session::Shared;
use crate::app::{Nav, PadInfo};

struct Pad {
    ctrl: *mut sdl::SDL_GameController,
    instance: i32,
    slot: usize,
    name: String,
    rumble: bool,
    triggers: bool,
    vendor: u16,
    product: u16,
    buttons: u32,
    axes: [i16; 6],
    motors: (f32, f32),
    motors_until: Instant,
}

#[derive(Default)]
pub struct Pads {
    pads: Vec<Pad>,
    /// Direction tenue au stick (menus) et prochaine répétition.
    held: Option<(Nav, Instant)>,
    mouse: bool,
    stick: (i16, i16),
    wheel: i16,
    boost: (i16, i16),
    last_step: Option<Instant>,
    wheel_acc: f32,
}

/// Boutons dans l'ordre de SDL, le même que `PadButton::ALL` du cœur.
pub fn core_button(which: u8) -> Option<PadButton> {
    PadButton::ALL.get(which as usize).copied()
}

pub fn button(which: u8) -> PadButton {
    core_button(which).unwrap_or(PadButton::ALL[0])
}

/// Axes dans l'ordre de SDL, le même que `PadAxis::ALL`.
pub fn core_axis(which: u8) -> Option<PadAxis> {
    PadAxis::ALL.get(which as usize).copied()
}

/// Scancode de Windows d'une touche de clavier (scancode SDL = code USB HID).
pub fn win_scancode(sc: i32) -> Option<u16> {
    if !(4..=231).contains(&sc) {
        return None;
    }
    KeyMap::from_key_mapping(KeyMapping::Usb(sc as u16)).ok().map(|k| k.win).filter(|w| *w != 0)
}

impl Pads {
    /// Une manette branchée (index SDL) ; rend son joueur.
    pub fn add(&mut self, index: i32) -> Option<usize> {
        unsafe {
            if sdl::SDL_IsGameController(index) == 0 {
                return None;
            }
            let ctrl = sdl::SDL_GameControllerOpen(index);
            if ctrl.is_null() {
                return None;
            }
            let joy = sdl::SDL_GameControllerGetJoystick(ctrl);
            let instance = sdl::SDL_JoystickInstanceID(joy);
            if self.pads.iter().any(|p| p.instance == instance) {
                return None;
            }
            let name = sdl::pad_name(ctrl);
            let slot = (0..4).find(|s| !self.pads.iter().any(|p| p.slot == *s))?;
            let pad = Pad {
                ctrl,
                instance,
                slot,
                name: name.clone(),
                rumble: sdl::SDL_GameControllerHasRumble(ctrl) != 0,
                triggers: sdl::SDL_GameControllerHasRumbleTriggers(ctrl) != 0,
                vendor: sdl::SDL_GameControllerGetVendor(ctrl),
                product: sdl::SDL_GameControllerGetProduct(ctrl),
                buttons: 0,
                axes: [0; 6],
                motors: (0.0, 0.0),
                motors_until: Instant::now(),
            };
            log::info!("manettes : J{} {name} ({:04x}:{:04x}), vibrations {}", slot + 1, pad.vendor, pad.product, if pad.rumble { "oui" } else { "non" });
            self.pads.push(pad);
            self.pads.sort_by_key(|p| p.slot);
            Some(slot)
        }
    }

    pub fn remove(&mut self, instance: i32) -> Option<usize> {
        let i = self.pads.iter().position(|p| p.instance == instance)?;
        let p = self.pads.remove(i);
        unsafe { sdl::SDL_GameControllerClose(p.ctrl) };
        log::info!("manettes : J{} débranchée", p.slot + 1);
        Some(p.slot)
    }

    pub fn slot_of(&self, instance: i32) -> Option<usize> {
        self.pads.iter().find(|p| p.instance == instance).map(|p| p.slot)
    }

    pub fn slots(&self) -> Vec<usize> {
        self.pads.iter().map(|p| p.slot).collect()
    }

    pub fn ids(&self, slot: usize) -> (u16, u16) {
        self.pads.iter().find(|p| p.slot == slot).map(|p| (p.vendor, p.product)).unwrap_or((0, 0))
    }

    pub fn name(&self, slot: usize) -> String {
        self.pads.iter().find(|p| p.slot == slot).map(|p| p.name.clone()).unwrap_or_default()
    }

    pub fn update(&mut self, slot: usize, is_button: bool, which: u8, down: bool, value: i16) {
        let Some(p) = self.pads.iter_mut().find(|p| p.slot == slot) else { return };
        if is_button {
            if which < 32 {
                if down {
                    p.buttons |= 1 << which;
                } else {
                    p.buttons &= !(1 << which);
                }
            }
        } else if (which as usize) < 6 {
            p.axes[which as usize] = value;
        }
    }

    /// Pour l'écran de test et l'accueil.
    pub fn infos(&self) -> Vec<PadInfo> {
        self.pads
            .iter()
            .map(|p| {
                let on = Instant::now() < p.motors_until;
                let battery = unsafe {
                    match sdl::SDL_JoystickCurrentPowerLevel(sdl::SDL_GameControllerGetJoystick(p.ctrl)) {
                        0 => Some(crate::i18n::tr("batterie vide", "battery empty").to_string()),
                        1 => Some(crate::i18n::tr("batterie faible", "battery low").to_string()),
                        2 => Some(crate::i18n::tr("batterie moyenne", "battery medium").to_string()),
                        3 | 5 => Some(crate::i18n::tr("batterie pleine", "battery full").to_string()),
                        4 => Some(crate::i18n::tr("filaire", "wired").to_string()),
                        _ => None,
                    }
                };
                PadInfo {
                    slot: p.slot,
                    name: p.name.clone(),
                    rumble: p.rumble,
                    trigger_rumble: p.triggers,
                    buttons: p.buttons,
                    axes: p.axes,
                    battery,
                    motors: if on { p.motors } else { (0.0, 0.0) },
                }
            })
            .collect()
    }

    /// La manette pilote l'app (menus, écrans) : une touche de navigation.
    pub fn nav(&mut self, is_button: bool, which: u8, down: bool, value: i16) -> Option<Nav> {
        if is_button {
            if !down {
                return None;
            }
            return match which {
                0 => Some(Nav::Ok),
                1 => Some(Nav::Back),
                2 => Some(Nav::Add),
                3 => Some(Nav::Options),
                6 => Some(Nav::Menu),
                9 => Some(Nav::PrevTab),
                10 => Some(Nav::NextTab),
                11 => Some(Nav::Up),
                12 => Some(Nav::Down),
                13 => Some(Nav::Left),
                14 => Some(Nav::Right),
                _ => None,
            };
        }
        // Stick gauche : une direction tenue, répétée.
        if which > 1 {
            return None;
        }
        let dir = match (which, value) {
            (0, v) if v > 20000 => Some(Nav::Right),
            (0, v) if v < -20000 => Some(Nav::Left),
            (1, v) if v > 20000 => Some(Nav::Down),
            (1, v) if v < -20000 => Some(Nav::Up),
            _ => None,
        };
        match dir {
            Some(d) if self.held.map(|(h, _)| h) != Some(d) => {
                self.held = Some((d, Instant::now() + Duration::from_millis(420)));
                Some(d)
            }
            Some(_) => None,
            None => {
                let axis_dir = |d: Nav| matches!((which, d), (0, Nav::Left | Nav::Right) | (1, Nav::Up | Nav::Down));
                if self.held.is_some_and(|(h, _)| axis_dir(h)) && value.unsigned_abs() < 12000 {
                    self.held = None;
                }
                None
            }
        }
    }

    pub fn repeat_nav(&mut self) -> Option<Nav> {
        let (d, at) = self.held?;
        if Instant::now() >= at {
            self.held = Some((d, Instant::now() + Duration::from_millis(130)));
            return Some(d);
        }
        None
    }

    /// Vibration demandée par le jeu (cœur) : moteurs gros et petit.
    pub fn rumble(&mut self, slot: usize, large: u16, small: u16, duration_ms: i32) {
        let Some(p) = self.pads.iter_mut().find(|p| p.slot == slot) else { return };
        let ms = if duration_ms <= 0 { 5000 } else { duration_ms.min(5000) as u32 };
        unsafe {
            sdl::SDL_GameControllerRumble(p.ctrl, large, small, ms);
        }
        p.motors = (large as f32 / 65535.0, small as f32 / 65535.0);
        p.motors_until = Instant::now() + Duration::from_millis(ms as u64);
    }

    /// Les moteurs s'arrêtent : la session qui faisait vibrer les manettes n'est plus l'active. Une
    /// vibration jouée par SDL dure jusqu'à son terme (5 s au plus quand l'ordinateur n'en donne pas) :
    /// le cœur n'envoie plus celles d'une session en arrière-plan, mais ne rattrape pas celle en cours.
    pub fn stop_rumble(&mut self) {
        for p in &mut self.pads {
            unsafe {
                sdl::SDL_GameControllerRumble(p.ctrl, 0, 0, 0);
                if p.triggers {
                    sdl::SDL_GameControllerRumbleTriggers(p.ctrl, 0, 0, 0);
                }
            }
            p.motors = (0.0, 0.0);
            p.motors_until = Instant::now();
        }
    }

    /// Vibration d'essai (écran des manettes).
    pub fn test_rumble(&mut self, slot: usize, low: u16, high: u16, triggers: bool) {
        let Some(p) = self.pads.iter_mut().find(|p| p.slot == slot) else { return };
        unsafe {
            if triggers {
                sdl::SDL_GameControllerRumbleTriggers(p.ctrl, 0xC000, 0xC000, 600);
            } else {
                sdl::SDL_GameControllerRumble(p.ctrl, low, high, 600);
            }
        }
        p.motors = (low as f32 / 65535.0, high as f32 / 65535.0);
        p.motors_until = Instant::now() + Duration::from_millis(600);
    }

    // MARK: - Manette en souris

    pub fn set_mouse(&mut self, on: bool) {
        self.mouse = on;
        self.stick = (0, 0);
        self.wheel = 0;
        self.last_step = None;
    }

    pub fn mouse_active(&self) -> bool {
        self.mouse && (self.stick.0.unsigned_abs() > 4000 || self.stick.1.unsigned_abs() > 4000 || self.wheel.unsigned_abs() > 6000)
    }

    /// Boutons et axes en souris : stick gauche pointeur (RT plus vite, LT
    /// plus précis), stick droit molette, A / B / X clics gauche, droit, milieu.
    pub fn mouse_input(&mut self, p: &Arc<InputPipeline>, is_button: bool, which: u8, down: bool, value: i16) {
        if is_button {
            let b = match which {
                0 => MouseButtonType::Left,
                1 => MouseButtonType::Right,
                2 => MouseButtonType::Middle,
                _ => return,
            };
            p.mouse_button(b, down);
            return;
        }
        match which {
            0 => self.stick.0 = value,
            1 => self.stick.1 = value,
            3 => self.wheel = value,
            4 => self.boost.0 = value,
            5 => self.boost.1 = value,
            _ => {}
        }
    }

    /// Un pas de déplacement du pointeur (à chaque tour de boucle).
    pub fn mouse_step(&mut self, shared: &Arc<Shared>) -> bool {
        let now = Instant::now();
        let dt = self.last_step.map(|t| (now - t).as_secs_f32()).unwrap_or(0.0).min(0.05);
        self.last_step = Some(now);
        if dt <= 0.0 {
            return false;
        }
        let dead = |v: i16| {
            let f = v as f32 / 32767.0;
            if f.abs() < 0.15 { 0.0 } else { f.signum() * ((f.abs() - 0.15) / 0.85).powi(2) }
        };
        let (vx, vy) = (dead(self.stick.0), dead(self.stick.1));
        let mut moved = false;
        if vx != 0.0 || vy != 0.0 {
            let (hw, _) = shared.host_size();
            let fast = 1.0 + 2.0 * (self.boost.1.max(0) as f32 / 32767.0);
            let slow = 1.0 - 0.6 * (self.boost.0.max(0) as f32 / 32767.0);
            let speed = 1500.0 * hw as f32 / 1920.0 * fast * slow;
            shared.pointer.move_by((vx * speed * dt) as f64, (vy * speed * dt) as f64);
            moved = true;
        }
        let w = dead(self.wheel);
        if w != 0.0 {
            self.wheel_acc += w * dt * 12.0;
            if self.wheel_acc.abs() >= 1.0 {
                if let Some(p) = shared.current_input() {
                    p.wheel(0.0, self.wheel_acc.trunc());
                }
                self.wheel_acc = self.wheel_acc.fract();
            }
        }
        moved
    }
}
