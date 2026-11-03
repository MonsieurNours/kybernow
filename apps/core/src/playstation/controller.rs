//! La manette, d'après `controller.c`, `feedback.c` et `feedbacksender.c`
//! de chiaki-ng.
//!
//! La console reçoit deux sortes de paquets (Takion, type 6 et type 1) :
//!
//! - **l'état** (sticks, capteurs de mouvement) à chaque changement (8 ms au
//!   moins entre deux) et au moins toutes les 200 ms ;
//! - **l'historique** des boutons, gâchettes et doigts sur le pavé tactile :
//!   chaque événement prend un numéro, un paquet porte les 8 derniers (le
//!   plus récent d'abord) et le numéro du plus récent ; la console applique
//!   ceux qu'elle n'a pas encore vus. Le dernier paquet est renvoyé 20, 50,
//!   100 et 200 ms après (un relâchement perdu laisserait la touche tenue).

use std::collections::{HashSet, VecDeque};

use crate::input::{PadAxis, PadButton};

/// Boutons (`ChiakiControllerButton`).
pub mod button {
    pub const CROSS: u32 = 1 << 0;
    pub const MOON: u32 = 1 << 1;
    pub const BOX: u32 = 1 << 2;
    pub const PYRAMID: u32 = 1 << 3;
    pub const DPAD_LEFT: u32 = 1 << 4;
    pub const DPAD_RIGHT: u32 = 1 << 5;
    pub const DPAD_UP: u32 = 1 << 6;
    pub const DPAD_DOWN: u32 = 1 << 7;
    pub const L1: u32 = 1 << 8;
    pub const R1: u32 = 1 << 9;
    pub const L3: u32 = 1 << 10;
    pub const R3: u32 = 1 << 11;
    pub const OPTIONS: u32 = 1 << 12;
    pub const SHARE: u32 = 1 << 13;
    pub const TOUCHPAD: u32 = 1 << 14;
    pub const PS: u32 = 1 << 15;
    /// Gâchettes analogiques, pour l'historique seulement.
    pub const L2: u32 = 1 << 16;
    pub const R2: u32 = 1 << 17;
}

/// Un doigt sur le pavé tactile (`id` -1 : levé).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Touch {
    pub id: i8,
    pub x: u16,
    pub y: u16,
}

/// État de la manette (`ChiakiControllerState`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ControllerState {
    pub buttons: u32,
    pub l2: u8,
    pub r2: u8,
    pub left_x: i16,
    pub left_y: i16,
    pub right_x: i16,
    pub right_y: i16,
    pub touches: [Touch; 2],
    pub gyro: [f32; 3],
    pub accel: [f32; 3],
    pub orient: [f32; 4],
}

impl Default for ControllerState {
    /// Au repos (`chiaki_controller_state_set_idle`) : manette à plat.
    fn default() -> Self {
        ControllerState {
            buttons: 0,
            l2: 0,
            r2: 0,
            left_x: 0,
            left_y: 0,
            right_x: 0,
            right_y: 0,
            touches: [Touch { id: -1, x: 0, y: 0 }; 2],
            gyro: [0.0; 3],
            accel: [0.0, 1.0, 0.0],
            orient: [0.0, 0.0, 0.0, 1.0],
        }
    }
}

impl ControllerState {
    /// D'après les touches tenues et les axes d'une manette du lecteur du
    /// cœur (noms SDL, à la Xbox : A est Croix, B Rond, X Carré, Y
    /// Triangle ; Back est Create, Start Options, Guide le bouton PS).
    pub fn from_pad(pressed: &HashSet<PadButton>, axes: &[i16; 6]) -> Self {
        let mut s = ControllerState::default();
        for b in pressed {
            s.buttons |= match b {
                PadButton::A => button::CROSS,
                PadButton::B => button::MOON,
                PadButton::X => button::BOX,
                PadButton::Y => button::PYRAMID,
                PadButton::Back => button::SHARE,
                PadButton::Guide => button::PS,
                PadButton::Start => button::OPTIONS,
                PadButton::LeftStick => button::L3,
                PadButton::RightStick => button::R3,
                PadButton::LeftShoulder => button::L1,
                PadButton::RightShoulder => button::R1,
                PadButton::DpadUp => button::DPAD_UP,
                PadButton::DpadDown => button::DPAD_DOWN,
                PadButton::DpadLeft => button::DPAD_LEFT,
                PadButton::DpadRight => button::DPAD_RIGHT,
            };
        }
        let axis = |a: PadAxis| axes[PadAxis::ALL.iter().position(|x| *x == a).unwrap_or(0)];
        s.left_x = axis(PadAxis::LeftX);
        s.left_y = axis(PadAxis::LeftY);
        s.right_x = axis(PadAxis::RightX);
        s.right_y = axis(PadAxis::RightY);
        // Gâchettes SDL : 0 à 32767.
        s.l2 = (axis(PadAxis::LeftTrigger).max(0) >> 7) as u8;
        s.r2 = (axis(PadAxis::RightTrigger).max(0) >> 7) as u8;
        s
    }

    /// Union de deux états (plusieurs manettes, `chiaki_controller_state_or`) :
    /// boutons réunis, la plus grande valeur de chaque axe ; les capteurs de
    /// la première qui en a.
    pub fn or(&self, other: &Self) -> Self {
        let pick = |a: i16, b: i16| if a.unsigned_abs() >= b.unsigned_abs() { a } else { b };
        let moving = |s: &Self| s.gyro != [0.0; 3] || s.accel != [0.0, 1.0, 0.0];
        let motion = if moving(self) { self } else { other };
        ControllerState {
            buttons: self.buttons | other.buttons,
            l2: self.l2.max(other.l2),
            r2: self.r2.max(other.r2),
            left_x: pick(self.left_x, other.left_x),
            left_y: pick(self.left_y, other.left_y),
            right_x: pick(self.right_x, other.right_x),
            right_y: pick(self.right_y, other.right_y),
            touches: if self.touches[0].id >= 0 || self.touches[1].id >= 0 { self.touches } else { other.touches },
            gyro: motion.gyro,
            accel: motion.accel,
            orient: motion.orient,
        }
    }
}

const GYRO: (f32, f32) = (-30.0, 30.0);
const ACCEL: (f32, f32) = (-5.0, 5.0);

fn scale(v: f32, (min, max): (f32, f32)) -> u16 {
    (65535.0 * ((v - min) / (max - min))).clamp(0.0, 65535.0) as u16
}

/// Quaternion compressé sur 32 bits (`compress_quat`).
fn compress_quat(q: [f32; 4]) -> u32 {
    let mut largest = 0;
    for i in 1..4 {
        if q[i].abs() > q[largest].abs() {
            largest = i;
        }
    }
    let mut r = u32::from(q[largest] < 0.0) | ((largest as u32) << 1);
    let s = std::f32::consts::FRAC_1_SQRT_2;
    for i in 0..3 {
        let qi = if i < largest { i } else { i + 1 };
        let v = (q[qi].clamp(-s, s) + s) * (0x1ff as f32 / (2.0 * s));
        r |= (v as u32) << (3 + i * 9);
    }
    r
}

/// L'état au format de la console : 0x19 octets (v9, PS4) ou 0x1c (v12,
/// PS5) (`chiaki_feedback_state_format_v9`/`_v12`).
pub fn format_state(s: &ControllerState, v12: bool) -> Vec<u8> {
    let mut b = vec![0xa0u8];
    for (v, range) in [(s.gyro[0], GYRO), (s.gyro[1], GYRO), (s.gyro[2], GYRO), (s.accel[0], ACCEL), (s.accel[1], ACCEL), (s.accel[2], ACCEL)] {
        b.extend_from_slice(&scale(v, range).to_le_bytes());
    }
    b.extend_from_slice(&compress_quat(s.orient).to_le_bytes());
    for v in [s.left_x, s.left_y, s.right_x, s.right_y] {
        b.extend_from_slice(&(v as u16).to_be_bytes());
    }
    if v12 {
        b.extend_from_slice(&[0, 0, 1]);
    }
    b
}

/// Un événement d'historique (`chiaki_feedback_history_event_set_button`).
pub(crate) fn button_event(button: u32, state: u8) -> Option<Vec<u8>> {
    let two = |pressed: u8, released: u8| Some(vec![0x80, if state != 0 { pressed } else { released }]);
    let code = match button {
        button::CROSS => 0x88,
        button::MOON => 0x89,
        button::BOX => 0x8a,
        button::PYRAMID => 0x8b,
        button::DPAD_LEFT => 0x82,
        button::DPAD_RIGHT => 0x83,
        button::DPAD_UP => 0x80,
        button::DPAD_DOWN => 0x81,
        button::L1 => 0x84,
        button::R1 => 0x85,
        button::L2 => 0x86,
        button::R2 => 0x87,
        button::L3 => return two(0xaf, 0x8f),
        button::R3 => return two(0xb0, 0x90),
        button::OPTIONS => return two(0xac, 0x8c),
        button::SHARE => return two(0xad, 0x8d),
        button::TOUCHPAD => return two(0xb1, 0x91),
        button::PS => return two(0xae, 0x8e),
        _ => return None,
    };
    Some(vec![0x80, code, state])
}

fn touch_event(down: bool, id: u8, x: u16, y: u16) -> Vec<u8> {
    vec![if down { 0xd0 } else { 0xc0 }, id & 0x7f, (x >> 4) as u8, (((x & 0xf) << 4) as u8) | (y >> 8) as u8, y as u8]
}

/// Les événements entre deux états (`chiaki_feedback_history_buffer_record_state`) :
/// relâchements, doigts, appuis, gâchettes.
pub(crate) fn history_events(prev: &ControllerState, now: &ControllerState) -> Vec<Vec<u8>> {
    let mut out = Vec::new();
    let buttons = |out: &mut Vec<Vec<u8>>, pressed: bool| {
        for i in 0..16 {
            let b = 1u32 << i;
            let (p, n) = (prev.buttons & b != 0, now.buttons & b != 0);
            if p != n && n == pressed {
                out.extend(button_event(b, if n { 0xff } else { 0 }));
            }
        }
    };
    buttons(&mut out, false);
    for i in 0..2 {
        let (p, n) = (prev.touches[i], now.touches[i]);
        if p.id != n.id && p.id >= 0 {
            out.push(touch_event(false, p.id as u8, p.x, p.y));
        } else if n.id >= 0 && (p.id != n.id || p.x != n.x || p.y != n.y) {
            out.push(touch_event(true, n.id as u8, n.x, n.y));
        }
    }
    buttons(&mut out, true);
    if prev.l2 != now.l2 {
        out.extend(button_event(button::L2, now.l2));
    }
    if prev.r2 != now.r2 {
        out.extend(button_event(button::R2, now.r2));
    }
    out
}

/// Historique des 16 derniers événements, le plus récent d'abord.
pub(crate) struct History {
    events: VecDeque<Vec<u8>>,
    /// Numéro du prochain événement.
    seq: u16,
}

/// Événements repris dans chaque paquet d'historique.
const HISTORY_RESEND: usize = 8;

impl History {
    pub(crate) fn new() -> Self {
        History { events: VecDeque::new(), seq: 0 }
    }

    /// Ajoute des événements ; rend le paquet à envoyer (numéro du plus
    /// récent, nombre d'événements, contenu), ou rien s'il n'y en a pas.
    pub(crate) fn push(&mut self, events: Vec<Vec<u8>>) -> Option<(u16, u8, Vec<u8>)> {
        if events.is_empty() {
            return None;
        }
        for e in events {
            self.events.push_front(e);
            self.events.truncate(16);
            self.seq = self.seq.wrapping_add(1);
        }
        let count = self.events.len();
        let body: Vec<u8> = self.events.iter().flatten().copied().collect();
        self.events.truncate(HISTORY_RESEND);
        Some((self.seq.wrapping_sub(1), count as u8, body))
    }
}

/// Paquet d'état (type 6) avant chiffrement : numéro, état.
pub(crate) fn state_packet(seq: u16, state: &ControllerState, v12: bool) -> Vec<u8> {
    let mut b = vec![super::takion::packet::FEEDBACK_STATE];
    b.extend_from_slice(&seq.to_be_bytes());
    b.push(0);
    b.extend_from_slice(&[0; 8]);
    b.extend_from_slice(&format_state(state, v12));
    b
}

/// Paquet d'historique (type 1) avant chiffrement : numéro du plus récent,
/// nombre d'événements moins un (la console en rajoute un).
pub(crate) fn history_packet(seq: u16, count: u8, body: &[u8]) -> Vec<u8> {
    let mut b = vec![super::takion::packet::FEEDBACK_HISTORY];
    b.extend_from_slice(&seq.to_be_bytes());
    b.push(count.saturating_sub(1));
    b.extend_from_slice(&[0; 8]);
    b.extend_from_slice(body);
    b
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn idle_state_v12() {
        let b = format_state(&ControllerState::default(), true);
        assert_eq!(b.len(), 0x1c);
        assert_eq!(b[0], 0xa0);
        // Gyroscope à 0 : milieu de l'échelle (0x7fff), petit-boutiste.
        assert_eq!(&b[1..3], &[0xff, 0x7f]);
        // Sticks au centre, fin v12.
        assert_eq!(&b[0x11..0x19], &[0; 8]);
        assert_eq!(&b[0x19..], &[0, 0, 1]);
        assert_eq!(format_state(&ControllerState::default(), false).len(), 0x19);
    }

    #[test]
    fn sticks_are_big_endian() {
        let s = ControllerState { left_x: -2, right_y: 0x1234, ..Default::default() };
        let b = format_state(&s, false);
        assert_eq!(&b[0x11..0x13], &[0xff, 0xfe]);
        assert_eq!(&b[0x17..0x19], &[0x12, 0x34]);
    }

    #[test]
    fn button_events_match_chiaki() {
        assert_eq!(button_event(button::CROSS, 0xff), Some(vec![0x80, 0x88, 0xff]));
        assert_eq!(button_event(button::OPTIONS, 0xff), Some(vec![0x80, 0xac]));
        assert_eq!(button_event(button::OPTIONS, 0), Some(vec![0x80, 0x8c]));
        assert_eq!(button_event(button::R2, 0x40), Some(vec![0x80, 0x87, 0x40]));
    }

    #[test]
    fn history_orders_releases_first() {
        let prev = ControllerState { buttons: button::CROSS, ..Default::default() };
        let now = ControllerState { buttons: button::MOON, l2: 10, ..Default::default() };
        let ev = history_events(&prev, &now);
        assert_eq!(ev, vec![vec![0x80, 0x88, 0], vec![0x80, 0x89, 0xff], vec![0x80, 0x86, 10]]);
    }

    #[test]
    fn history_packets_number_events() {
        let mut h = History::new();
        let (seq, n, body) = h.push(vec![vec![1], vec![2]]).unwrap();
        assert_eq!((seq, n, body), (1, 2, vec![2, 1]));
        let (seq, n, body) = h.push(vec![vec![3]]).unwrap();
        assert_eq!((seq, n, body), (2, 3, vec![3, 2, 1]));
        for i in 0..10 {
            h.push(vec![vec![10 + i]]);
        }
        let (_, n, _) = h.push(vec![vec![99]]).unwrap();
        assert_eq!(n as usize, HISTORY_RESEND + 1);
        assert!(h.push(Vec::new()).is_none());
    }

    #[test]
    fn from_pad_maps_xbox_names() {
        let pressed: HashSet<PadButton> = [PadButton::A, PadButton::Guide, PadButton::Back].into_iter().collect();
        let s = ControllerState::from_pad(&pressed, &[100, -200, 0, 0, 32767, 0]);
        assert_eq!(s.buttons, button::CROSS | button::PS | button::SHARE);
        assert_eq!((s.left_x, s.left_y, s.l2, s.r2), (100, -200, 255, 0));
    }

    #[test]
    fn touch_event_layout() {
        assert_eq!(touch_event(true, 1, 0x123, 0x456), vec![0xd0, 1, 0x12, 0x34, 0x56]);
    }
}
