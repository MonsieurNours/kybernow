//! Clavier et souris traduits en manette (docs/playstation.md : la lecture
//! à distance ne transmet qu'une manette).
//!
//! Les touches sont désignées par leur usage USB HID (page 7), c'est-à-dire
//! leur place sur le clavier : la disposition par défaut met le stick
//! gauche sous W A S D en QWERTY, donc sous Z Q S D en AZERTY ; l'app montre
//! la lettre du clavier du Mac. Chaque console a sa disposition ([`Layout`],
//! JSON stable gardé par l'app).
//!
//! La souris tient le stick droit : sa vitesse (comptes par seconde, sans
//! l'accélération du système) devient une inclinaison, multipliée par la
//! sensibilité, passée par une courbe, et décalée d'une zone morte de départ
//! pour franchir celle des jeux. Une souris immobile depuis 25 ms ramène le
//! stick au centre.

use std::collections::{HashSet, VecDeque};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use super::controller::{ControllerState, button};

/// Ce qui déclenche : une touche (usage HID) ou un bouton de la souris
/// (0 gauche, 1 droit, 2 milieu, 3 et 4 les boutons latéraux).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Input {
    Key(u16),
    Mouse(u8),
}

/// Ce que ça fait sur la manette.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Action {
    Cross,
    Circle,
    Square,
    Triangle,
    L1,
    R1,
    L2,
    R2,
    L3,
    R3,
    Options,
    Create,
    Ps,
    Touchpad,
    DpadUp,
    DpadDown,
    DpadLeft,
    DpadRight,
    LeftStickUp,
    LeftStickDown,
    LeftStickLeft,
    LeftStickRight,
    RightStickUp,
    RightStickDown,
    RightStickLeft,
    RightStickRight,
}

impl Action {
    /// Toutes, dans l'ordre des réglages.
    pub const ALL: [Action; 26] = [
        Action::LeftStickUp,
        Action::LeftStickDown,
        Action::LeftStickLeft,
        Action::LeftStickRight,
        Action::Cross,
        Action::Circle,
        Action::Square,
        Action::Triangle,
        Action::L1,
        Action::R1,
        Action::L2,
        Action::R2,
        Action::L3,
        Action::R3,
        Action::DpadUp,
        Action::DpadDown,
        Action::DpadLeft,
        Action::DpadRight,
        Action::Options,
        Action::Create,
        Action::Ps,
        Action::Touchpad,
        Action::RightStickUp,
        Action::RightStickDown,
        Action::RightStickLeft,
        Action::RightStickRight,
    ];

    fn button(self) -> Option<u32> {
        Some(match self {
            Action::Cross => button::CROSS,
            Action::Circle => button::MOON,
            Action::Square => button::BOX,
            Action::Triangle => button::PYRAMID,
            Action::L1 => button::L1,
            Action::R1 => button::R1,
            Action::L3 => button::L3,
            Action::R3 => button::R3,
            Action::Options => button::OPTIONS,
            Action::Create => button::SHARE,
            Action::Ps => button::PS,
            Action::Touchpad => button::TOUCHPAD,
            Action::DpadUp => button::DPAD_UP,
            Action::DpadDown => button::DPAD_DOWN,
            Action::DpadLeft => button::DPAD_LEFT,
            Action::DpadRight => button::DPAD_RIGHT,
            _ => return None,
        })
    }
}

/// Une disposition : liens (plusieurs touches peuvent mener à la même
/// action) et réglages de la souris.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Layout {
    pub bindings: Vec<(Input, Action)>,
    /// La souris tient le stick droit.
    #[serde(default = "yes")]
    pub mouse_stick: bool,
    /// Multiplie la vitesse de la souris (1 : 800 comptes/s pour un stick
    /// au bout).
    #[serde(default = "one")]
    pub sensitivity: f32,
    /// Exposant de la courbe (1 : linéaire ; moins de 1 : plus vif au départ).
    #[serde(default = "curve")]
    pub curve: f32,
    /// Inclinaison minimale dès que la souris bouge (0 à 0,5), pour franchir
    /// la zone morte des jeux.
    #[serde(default = "deadzone")]
    pub deadzone: f32,
    /// Inverser l'axe vertical de la souris.
    #[serde(default)]
    pub invert_y: bool,
}

fn yes() -> bool {
    true
}
fn one() -> f32 {
    1.0
}
fn deadzone() -> f32 {
    0.2
}
fn curve() -> f32 {
    0.8
}

/// Usages HID des touches de la disposition par défaut.
pub mod key {
    pub const A: u16 = 0x04;
    pub const C: u16 = 0x06;
    pub const D: u16 = 0x07;
    pub const E: u16 = 0x08;
    pub const F: u16 = 0x09;
    pub const Q: u16 = 0x14;
    pub const R: u16 = 0x15;
    pub const S: u16 = 0x16;
    pub const V: u16 = 0x19;
    pub const W: u16 = 0x1a;
    pub const RETURN: u16 = 0x28;
    pub const ESCAPE: u16 = 0x29;
    pub const BACKSPACE: u16 = 0x2a;
    pub const TAB: u16 = 0x2b;
    pub const SPACE: u16 = 0x2c;
    pub const BACKSLASH: u16 = 0x31;
    pub const GRAVE: u16 = 0x35;
    pub const RIGHT: u16 = 0x4f;
    pub const LEFT: u16 = 0x50;
    pub const DOWN: u16 = 0x51;
    pub const UP: u16 = 0x52;
    pub const LEFT_SHIFT: u16 = 0xe1;
}

impl Default for Layout {
    /// La disposition par défaut : celle d'un jeu d'action sur PC, et de quoi
    /// naviguer dans les menus.
    fn default() -> Self {
        use Action::*;
        use Input::{Key, Mouse};
        Layout {
            bindings: vec![
                (Key(key::W), LeftStickUp),
                (Key(key::S), LeftStickDown),
                (Key(key::A), LeftStickLeft),
                (Key(key::D), LeftStickRight),
                (Key(key::SPACE), Cross),
                (Key(key::RETURN), Cross),
                (Key(key::C), Circle),
                (Key(key::BACKSPACE), Circle),
                (Key(key::R), Square),
                (Key(key::F), Triangle),
                (Key(key::Q), L1),
                (Key(key::E), R1),
                (Mouse(1), L2),
                (Mouse(0), R2),
                (Key(key::LEFT_SHIFT), L3),
                (Key(key::V), R3),
                (Mouse(2), R3),
                (Key(key::UP), DpadUp),
                (Key(key::DOWN), DpadDown),
                (Key(key::LEFT), DpadLeft),
                (Key(key::RIGHT), DpadRight),
                (Key(key::ESCAPE), Options),
                (Key(key::BACKSLASH), Create),
                (Key(key::GRAVE), Ps),
                (Key(key::TAB), Touchpad),
            ],
            mouse_stick: true,
            sensitivity: 1.0,
            curve: curve(),
            deadzone: deadzone(),
            invert_y: false,
        }
    }
}

/// Vitesse de la souris (comptes/s) qui met le stick au bout, sensibilité 1
/// (essais du 03/11/2026 : 4000 bien trop lent, 1200 encore un peu lent).
const FULL_SPEED: f32 = 800.0;
/// La vitesse est mesurée sur ce temps : une souris envoie ses mouvements
/// toutes les 1 à 8 ms, les lectures se font toutes les 4 ms. Chaque
/// mouvement est réparti sur le temps écoulé depuis le précédent, et seule
/// sa part dans la fenêtre compte : la vitesse ne saute pas quand un
/// mouvement entre ou sort de la fenêtre (essai du 03/11/2026 : un compte
/// entier par mouvement faisait osciller le stick).
const MOUSE_WINDOW: Duration = Duration::from_millis(15);
/// Temps couvert au plus par un mouvement (le premier après un arrêt).
const MOVE_SPAN_MAX: Duration = Duration::from_millis(8);
/// Plus de mouvement depuis ce temps : la souris est arrêtée, stick au
/// centre. La fenêtre se termine au dernier mouvement reçu (pas à
/// l'instant de lecture : le suivant n'est simplement pas encore arrivé).
const MOUSE_IDLE: Duration = Duration::from_millis(20);

/// L'état du clavier et de la souris, et la manette qui en sort.
pub struct KeyMouse {
    layout: Layout,
    held: HashSet<Input>,
    /// Mouvements récents : (début, fin, dx, dy), le début étant la fin du
    /// précédent.
    moves: VecDeque<(Instant, Instant, f32, f32)>,
}

impl Default for KeyMouse {
    fn default() -> Self {
        Self::new(Layout::default())
    }
}

impl KeyMouse {
    pub fn new(layout: Layout) -> Self {
        KeyMouse { layout, held: HashSet::new(), moves: VecDeque::new() }
    }

    pub fn set_layout(&mut self, layout: Layout) {
        self.layout = layout;
    }

    pub fn key(&mut self, usage: u16, pressed: bool) {
        self.input(Input::Key(usage), pressed);
    }

    pub fn mouse_button(&mut self, b: u8, pressed: bool) {
        self.input(Input::Mouse(b), pressed);
    }

    fn input(&mut self, i: Input, pressed: bool) {
        if pressed {
            self.held.insert(i);
        } else {
            self.held.remove(&i);
        }
    }

    /// Déplacement brut de la souris (comptes, y vers le bas).
    pub fn mouse_move(&mut self, dx: f32, dy: f32, now: Instant) {
        let floor = now.checked_sub(MOVE_SPAN_MAX).unwrap_or(now);
        let start = self.moves.back().map(|m| m.1).unwrap_or(floor).max(floor).min(now);
        self.moves.push_back((start, now, dx, dy));
        while self.moves.len() > 256 {
            self.moves.pop_front();
        }
    }

    /// Tout relâché (fenêtre qui perd le clavier, souris rendue).
    pub fn release_all(&mut self) {
        self.held.clear();
        self.moves.clear();
    }

    /// Quelque chose est tenu ou la souris bouge.
    pub fn active(&self) -> bool {
        !self.held.is_empty() || !self.moves.is_empty()
    }

    /// La manette correspondante à l'instant `now`.
    pub fn read(&mut self, now: Instant) -> ControllerState {
        let mut s = ControllerState::default();
        let (mut lx, mut ly, mut rx, mut ry) = (0i32, 0i32, 0i32, 0i32);
        for (input, action) in &self.layout.bindings {
            if !self.held.contains(input) {
                continue;
            }
            if let Some(b) = action.button() {
                s.buttons |= b;
                continue;
            }
            match action {
                Action::L2 => s.l2 = 255,
                Action::R2 => s.r2 = 255,
                Action::LeftStickUp => ly -= 1,
                Action::LeftStickDown => ly += 1,
                Action::LeftStickLeft => lx -= 1,
                Action::LeftStickRight => lx += 1,
                Action::RightStickUp => ry -= 1,
                Action::RightStickDown => ry += 1,
                Action::RightStickLeft => rx -= 1,
                Action::RightStickRight => rx += 1,
                _ => {}
            }
        }
        let full = |v: i32| (v.clamp(-1, 1) * i16::MAX as i32) as i16;
        s.left_x = full(lx);
        s.left_y = full(ly);
        s.right_x = full(rx);
        s.right_y = full(ry);
        if self.layout.mouse_stick {
            let (mx, my) = self.mouse_stick(now);
            if mx != 0 || my != 0 {
                s.right_x = mx;
                s.right_y = my;
            }
        }
        s
    }

    fn mouse_stick(&mut self, now: Instant) -> (i16, i16) {
        let Some(&(_, last, _, _)) = self.moves.back() else { return (0, 0) };
        if now.saturating_duration_since(last) >= MOUSE_IDLE {
            self.moves.clear();
            return (0, 0);
        }
        let from = last.checked_sub(MOUSE_WINDOW).unwrap_or(last);
        while self.moves.front().is_some_and(|m| m.1 <= from) {
            self.moves.pop_front();
        }
        let (sx, sy) = self.moves.iter().fold((0.0f32, 0.0f32), |(x, y), &(start, end, dx, dy)| {
            let span = end.saturating_duration_since(start).as_secs_f32();
            let part = if span <= 0.0 { 1.0 } else { (end.saturating_duration_since(start.max(from)).as_secs_f32() / span).clamp(0.0, 1.0) };
            (x + dx * part, y + dy * part)
        });
        let w = MOUSE_WINDOW.as_secs_f32();
        let l = &self.layout;
        let ux = sx / w * l.sensitivity / FULL_SPEED;
        let uy = sy / w * l.sensitivity / FULL_SPEED * if l.invert_y { -1.0 } else { 1.0 };
        let m = (ux * ux + uy * uy).sqrt();
        if m < 1e-4 {
            return (0, 0);
        }
        let dz = l.deadzone.clamp(0.0, 0.5);
        let out = dz + (1.0 - dz) * m.min(1.0).powf(l.curve.clamp(0.2, 3.0));
        let k = out / m;
        let to = |u: f32| ((u * k).clamp(-1.0, 1.0) * i16::MAX as f32) as i16;
        (to(ux), to(uy))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wasd_and_buttons() {
        let mut km = KeyMouse::default();
        let t = Instant::now();
        km.key(key::W, true);
        km.key(key::D, true);
        km.key(key::SPACE, true);
        km.mouse_button(0, true);
        let s = km.read(t);
        assert_eq!((s.left_x, s.left_y), (i16::MAX, -i16::MAX));
        assert_eq!(s.buttons & button::CROSS, button::CROSS);
        assert_eq!(s.r2, 255);
        km.key(key::W, false);
        km.key(key::S, true);
        km.key(key::W, true);
        // Haut et bas ensemble : le stick reste au centre.
        assert_eq!(km.read(t).left_y, 0);
        km.release_all();
        assert!(!km.active());
        assert_eq!(km.read(t), ControllerState::default());
    }

    #[test]
    fn mouse_moves_right_stick_then_recenters() {
        let mut km = KeyMouse::default();
        let t0 = Instant::now();
        // Souris à 125 Hz : 4 comptes toutes les 8 ms (500 comptes/s), lue
        // toutes les 4 ms : l'inclinaison ne retombe pas entre deux mouvements.
        let mut values = Vec::new();
        for i in 0..10u64 {
            if i % 2 == 0 {
                km.mouse_move(4.0, 0.0, t0 + Duration::from_millis(i * 4));
            }
            values.push(km.read(t0 + Duration::from_millis(i * 4 + 1)).right_x);
        }
        let steady = &values[6..];
        assert!(steady.iter().all(|&v| v > i16::MAX / 2), "{values:?}");
        let spread = steady.iter().max().unwrap() - steady.iter().min().unwrap();
        assert!(spread < i16::MAX / 20, "{values:?}");
        // Rapide : au bout.
        km.mouse_move(100.0, 0.0, t0 + Duration::from_millis(41));
        assert_eq!(km.read(t0 + Duration::from_millis(42)).right_x, i16::MAX);
        // Petit mouvement isolé : au moins la zone morte de départ.
        let t1 = t0 + Duration::from_secs(1);
        km.mouse_move(1.0, 0.0, t1);
        assert!(km.read(t1 + Duration::from_millis(1)).right_x as f32 >= 0.2 * i16::MAX as f32);
        // Souris immobile : retour au centre.
        let s = km.read(t1 + Duration::from_millis(60));
        assert_eq!((s.right_x, s.right_y), (0, 0));
    }

    #[test]
    fn layout_json_round_trip() {
        let l = Layout::default();
        let json = serde_json::to_string(&l).unwrap();
        assert!(json.contains("\"key\":26") && json.contains("\"left_stick_up\""));
        assert_eq!(serde_json::from_str::<Layout>(&json).unwrap(), l);
        let minimal: Layout = serde_json::from_str(r#"{"bindings":[[{"mouse":0},"r2"]]}"#).unwrap();
        assert_eq!((minimal.sensitivity, minimal.deadzone, minimal.curve, minimal.mouse_stick), (1.0, 0.2, 0.8, true));
    }

    #[test]
    fn every_action_is_reachable_by_default_or_mouse() {
        let l = Layout::default();
        for a in Action::ALL {
            let bound = l.bindings.iter().any(|(_, b)| *b == a);
            let mouse_stick = matches!(a, Action::RightStickUp | Action::RightStickDown | Action::RightStickLeft | Action::RightStickRight);
            assert!(bound || mouse_stick, "{a:?}");
        }
    }
}
