//! Manettes lues par kynput lui-même via SDL, sur Mac : la combinaison
//! L1 + R1 + Start + Select, et la manette prise par l'app quand sa barre
//! s'ouvre. (Windows lit aussi ses manettes par SDL dans kynput, sans ce
//! hook : pas de combinaison.)
//!
//! Branché comme « hook » du routeur kynput : chaque paquet de manette y
//! passe avant de partir vers l'ordinateur. Le routeur appelle le hook sous
//! son verrou : on n'y envoie jamais de paquet (les relâchements de la prise
//! partent de [`super::InputPipeline::set_gamepad_captured`], hors du hook).
//!
//! Le hook ne décide pas : il signale la combinaison (`PadCombo`, partagé),
//! et c'est le contrôleur de session qui prend la manette quand la barre
//! s'ouvre vraiment. Hors session (connexion, reconnexion), rien n'est pris.
//!
//! Manette prise : appuis et axes en mouvement restent à l'app (touches de
//! navigation) ; relâchements et axes au repos passent toujours, quelle que
//! soit leur origine. Un relâchement ou un repos de plus ne change rien pour
//! l'ordinateur, et une règle aussi simple ne peut pas avaler les réessais de
//! la file de secours (une autorisation à usage unique, consommée par le
//! hook avant que la file d'envoi se prononce, les perdrait).
//!
//! Hors prise : un axe qui n'a pas bougé (au 1/512 de la course) ne repart
//! pas. Une manette Bluetooth bavarde remplirait sinon la file d'envoi de
//! kynput (64 paquets, jetés sans retour), et avec elle les relâchements ;
//! les paquets SDL ne passent pas par la file de l'app, seul le hook peut en
//! réduire le débit.

use std::collections::HashMap;
use std::sync::{Mutex, Weak};

use kynput_rs::{Packet, RouterHook, RouterHookPacketResult};
use kynput_sys as sys;

use super::pad_combo::{PadButton, PadCombo, PadId, PadKey};
use super::pipeline::InputPipeline;

#[derive(Default)]
struct GateState {
    captured: bool,
    /// Dernière valeur partie de chaque axe, par manette.
    last_axis: HashMap<PadId, HashMap<u32, i16>>,
}

#[derive(Default)]
pub(crate) struct GamepadGate {
    state: Mutex<GateState>,
}

impl GamepadGate {
    /// L'app prend la manette (barre ouverte) ou la rend (barre fermée).
    /// `true` : elle vient d'être prise (relâchements à envoyer).
    pub(crate) fn set_captured(&self, captured: bool) -> bool {
        let mut state = self.state.lock().unwrap();
        let was = state.captured;
        state.captured = captured;
        captured && !was
    }

    /// Un axe envoyé par l'app hors du hook (remise au repos à la prise).
    pub(crate) fn note_axis(&self, id: PadId, axis: u32, value: i16) {
        self.state.lock().unwrap().last_axis.entry(id).or_default().insert(axis, value);
    }

    /// `true` : le paquet part vers l'ordinateur.
    fn filter(&self, packet: *const sys::kynput_Packet, combo: &PadCombo) -> Filtered {
        unsafe {
            let mut control: sys::kynput_GamepadControlId = 0;
            if sys::kynput_gamepad_control_id_from_packet(packet, &mut control) {
                // Branchements : toujours transmis, et notés pour la remise à zéro.
                if control == sys::kynput_GamepadControlId_KYNPUT_GAMEPAD_CONTROL_ID_PLUG {
                    let mut plug = std::mem::zeroed::<sys::kynput_GamepadControlIdPlug>();
                    if sys::kynput_gamepad_plug_from_packet(packet, &mut plug) {
                        combo.note(plug.id);
                    }
                } else if control == sys::kynput_GamepadControlId_KYNPUT_GAMEPAD_CONTROL_ID_UNPLUG {
                    let mut unplug = std::mem::zeroed::<sys::kynput_GamepadControlIdUnplug>();
                    if sys::kynput_gamepad_unplug_from_packet(packet, &mut unplug) {
                        combo.forget(unplug.id);
                        self.state.lock().unwrap().last_axis.remove(&unplug.id);
                    }
                }
                return Filtered::pass();
            }
            let mut event: sys::kynput_GamepadEventId = 0;
            if !sys::kynput_gamepad_event_id_from_packet(packet, &mut event) {
                return Filtered::pass();
            }
            let mut state = self.state.lock().unwrap();
            if event == sys::kynput_GamepadEventId_KYNPUT_GAMEPAD_EVENT_ID_AXIS {
                let mut a = std::mem::zeroed::<sys::kynput_GamepadEventAxis>();
                if !sys::kynput_gamepad_axis_from_packet(packet, &mut a) {
                    return Filtered::verdict(!state.captured);
                }
                combo.note(a.id);
                let axis = a.type_ as u32;
                if state.captured {
                    // Manette prise : seuls les retours au repos passent.
                    if a.value != 0 {
                        return Filtered::verdict(false);
                    }
                    state.last_axis.entry(a.id).or_default().insert(axis, 0);
                    return Filtered::pass();
                }
                let last = state.last_axis.get(&a.id).and_then(|m| m.get(&axis)).copied();
                if !InputPipeline::axis_moved(last, a.value) {
                    return Filtered::verdict(false);
                }
                state.last_axis.entry(a.id).or_default().insert(axis, a.value);
                return Filtered::pass();
            }
            if event != sys::kynput_GamepadEventId_KYNPUT_GAMEPAD_EVENT_ID_BUTTON {
                return Filtered::verdict(!state.captured);
            }
            let mut b = std::mem::zeroed::<sys::kynput_GamepadEventButton>();
            if !sys::kynput_gamepad_button_from_packet(packet, &mut b) {
                return Filtered::verdict(!state.captured);
            }
            let Some(button) = PadButton::from_u32(b.type_ as u32) else {
                return Filtered::verdict(!state.captured);
            };
            let captured = state.captured;
            drop(state);
            // Les touches de la combinaison partent au jeu comme les autres ;
            // elle se forme après 250 ms tenues (PadCombo).
            combo.button(b.id, button, b.pressed);
            if !captured || !b.pressed {
                return Filtered {
                    pass: true,
                    navigate: None,
                    armed: combo.next_deadline().is_some(),
                };
            }
            Filtered {
                pass: false,
                navigate: PadCombo::navigation_key(button),
                armed: combo.next_deadline().is_some(),
            }
        }
    }
}

struct Filtered {
    pass: bool,
    navigate: Option<PadKey>,
    /// La combinaison attend ses 250 ms : réveiller le fil des entrées.
    armed: bool,
}

impl Filtered {
    fn pass() -> Self {
        Self::verdict(true)
    }

    fn verdict(pass: bool) -> Self {
        Self { pass, navigate: None, armed: false }
    }
}

/// Le hook branché sur le routeur.
pub(crate) struct GateHook {
    pub(crate) pipeline: Weak<InputPipeline>,
}

impl RouterHook for GateHook {
    fn on_packet(&self, pkt: &Packet) -> RouterHookPacketResult {
        let Some(p) = self.pipeline.upgrade() else {
            return RouterHookPacketResult { process_event: true };
        };
        let Some(gate) = p.gamepad_gate() else {
            return RouterHookPacketResult { process_event: true };
        };
        let f = gate.filter(pkt.to_ptr(), p.combo());
        if f.armed {
            p.wake();
        }
        if let Some(key) = f.navigate {
            p.navigate(key);
        }
        RouterHookPacketResult { process_event: f.pass }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::clock::ManualClock;
    use crate::input::packet::HOST;
    use crate::input::pad_combo::{COMBO, PadAxis};
    use std::sync::Arc;

    /// Un paquet kynput, libéré à la fin du test.
    struct Owned(*mut sys::kynput_Packet);

    impl Drop for Owned {
        fn drop(&mut self) {
            unsafe { sys::kynput_packet_unref(self.0) };
        }
    }

    fn button(id: PadId, button: PadButton, pressed: bool) -> Owned {
        let b = sys::kynput_GamepadEventButton { id, type_: button.to_u32(), pressed };
        Owned(unsafe { sys::kynput_gamepad_button_to_packet(HOST, &b) })
    }

    fn axis(id: PadId, axis: PadAxis, value: i16) -> Owned {
        let a = sys::kynput_GamepadEventAxis { id, type_: axis.to_u32(), value };
        Owned(unsafe { sys::kynput_gamepad_axis_to_packet(HOST, &a) })
    }

    fn setup() -> (GamepadGate, PadCombo) {
        (GamepadGate::default(), PadCombo::new(Arc::new(ManualClock::new())))
    }

    #[test]
    fn libre_tout_passe_et_axes_immobiles_ecartes() {
        let (gate, combo) = setup();
        assert!(gate.filter(button(0, PadButton::A, true).0, &combo).pass);
        assert!(gate.filter(button(0, PadButton::A, false).0, &combo).pass);
        assert!(gate.filter(axis(0, PadAxis::LeftX, 10_000).0, &combo).pass);
        // Même valeur au 1/512 près : écartée.
        assert!(!gate.filter(axis(0, PadAxis::LeftX, 10_010).0, &combo).pass);
        assert!(gate.filter(axis(0, PadAxis::LeftX, 0).0, &combo).pass);
    }

    #[test]
    fn prise_appuis_retenus_relachements_et_repos_passent() {
        let (gate, combo) = setup();
        assert!(gate.set_captured(true));
        assert!(!gate.set_captured(true), "déjà prise : rien à relâcher");
        let f = gate.filter(button(0, PadButton::DpadLeft, true).0, &combo);
        assert!(!f.pass);
        assert_eq!(f.navigate, Some(PadKey::Left));
        let f = gate.filter(button(0, PadButton::A, true).0, &combo);
        assert!(!f.pass);
        assert_eq!(f.navigate, Some(PadKey::Accept));
        assert!(gate.filter(button(0, PadButton::A, false).0, &combo).pass);
        assert!(!gate.filter(axis(0, PadAxis::RightY, -20_000).0, &combo).pass);
        assert!(gate.filter(axis(0, PadAxis::RightY, 0).0, &combo).pass);
        gate.set_captured(false);
        assert!(gate.filter(button(0, PadButton::A, true).0, &combo).pass);
    }

    #[test]
    fn combinaison_vue_par_le_portier() {
        let (gate, combo) = setup();
        for b in COMBO {
            assert!(gate.filter(button(3, b, true).0, &combo).pass, "les touches de la combinaison partent au jeu");
        }
        assert!(combo.next_deadline().is_some(), "la combinaison attend ses 250 ms");
        assert_eq!(combo.pads(), vec![3]);
    }
}
