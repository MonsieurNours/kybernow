//! Le point d'application unique de « qui est actif » (docs/plusieurs-ordinateurs.md).
//!
//! `App::focus_states` dit, pour chaque session, si elle est active (la session de la fenêtre au
//! premier plan : elle seule reçoit le clavier, la souris, les manettes et le presse-papiers) et si
//! son son est coupé (ni active ni gardée par « Garder le son », ou coupé depuis son menu). La boucle
//! principale appelle `apply` à chaque tour : dès que cette liste change (activation, « Garder le
//! son », son coupé, session ouverte ou finie), quel que soit le chemin qui l'a changée, `apply`
//! donne à chaque session sa part par **une seule méthode** (`Live::apply_focus` : son,
//! presse-papiers, manettes) et règle l'état global de SDL d'après la session active (souris
//! relative, saisie de texte, économiseur d'écran, manette en souris).
//!
//! Au second temps (registre `focus` du cœur, `InputPipeline::set_active`,
//! `AudioEngine::set_muted`), seul le corps de `Live::apply_focus` change : il appellera
//! `focus.activate(id)` et `focus.set_keep_sound(id, …)` au lieu de faire ce travail ici.

use std::ffi::c_int;
use std::sync::Arc;

use kybernow_core::input::InputPipeline;

use super::pads::{self, Pads};
use super::{sdl, Live, Lives};
use crate::app::{App, FocusState, PadInfo};

/// Applique `App::focus_states` si elle a changé depuis `last`. Rend vrai si elle a été appliquée
/// (la session qui devient active relit alors tout de suite le presse-papiers de l'appareil).
pub fn apply(app: &mut App, lives: &mut Lives, pads: &mut Pads, last: &mut Vec<FocusState>) -> bool {
    let states = app.focus_states();
    if states == *last {
        return false;
    }
    let held = pads.infos();
    for st in &states {
        let pad_mouse = app.session_by(st.id).is_some_and(|s| s.pad_mouse);
        if let Some(l) = lives.get_mut(st.id) {
            l.apply_focus(st.active, st.muted, !pad_mouse, &held);
        }
        // Une session qui n'est pas active n'a pas la souris des jeux : le pointeur est à la session active.
        if !st.active
            && let Some(s) = app.session_by_mut(st.id)
        {
            s.relative = false;
        }
    }
    apply_sdl(app, lives, pads);
    *last = states;
    true
}

/// L'état global de SDL, d'après la session active : souris relative, saisie de texte, manette en
/// souris ; l'économiseur d'écran est coupé tant qu'une session au moins est en cours.
fn apply_sdl(app: &App, lives: &Lives, pads: &mut Pads) {
    let active = app.session();
    unsafe {
        sdl::SDL_SetRelativeMouseMode(active.is_some_and(|s| s.relative) as c_int);
        if app.sessions.is_empty() {
            sdl::SDL_ShowCursor(1);
            sdl::SDL_SetCursor(sdl::SDL_GetDefaultCursor());
            sdl::SDL_EnableScreenSaver();
            sdl::SDL_StopTextInput();
        } else {
            sdl::SDL_DisableScreenSaver();
            if active.is_some_and(|s| lives.get(s.id).is_some_and(|l| l.typing)) {
                sdl::SDL_StartTextInput();
            } else {
                sdl::SDL_StopTextInput();
            }
        }
    }
    pads.set_mouse(active.is_some_and(|s| s.pad_mouse));
}

impl Live {
    /// Ce que fait une session de « active » et « son coupé » : l'unique point où l'app les lui
    /// donne (voir le haut de ce fichier).
    ///
    /// - **Son** : coupé ou rendu, avec un fondu de 20 ms (`Shared::set_muted`) ; le son continue
    ///   d'être décodé, il revient sans décalage.
    /// - **Presse-papiers** : en pause tant que la session n'est pas active (rien n'est lu sur
    ///   l'appareil, ce que l'ordinateur y envoie est écarté).
    /// - **Clavier, souris, manettes** (quand l'état change) : inactive, touches et clics relâchés,
    ///   souris relative coupée, manettes au repos (boutons relâchés, axes au centre) ; active, les
    ///   boutons et axes tenus sont renvoyés (sauf en manette-souris). Boutons et axes ne vont
    ///   qu'à la session active (`handle_sdl`), les branchements vont à toutes.
    pub(super) fn apply_focus(&mut self, active: bool, muted: bool, replay_pads: bool, held: &[PadInfo]) {
        let sh = self.handle.shared.clone();
        sh.set_muted(muted);
        if let Some(c) = &sh.clipboard {
            c.set_active(active);
        }
        if active == self.active {
            return;
        }
        self.active = active;
        let Some(p) = sh.current_input() else { return };
        if active {
            if replay_pads {
                replay(&p, held);
            }
        } else {
            sh.relative.store(false, std::sync::atomic::Ordering::Relaxed);
            p.set_relative(false);
            p.release_all();
            rest(&p, held);
            self.escape_down = None;
            self.bar_arm = None;
        }
    }
}

/// Les manettes au repos sur cet ordinateur : boutons tenus relâchés, axes au centre.
fn rest(p: &Arc<InputPipeline>, held: &[PadInfo]) {
    for pad in held {
        for bit in 0..32u8 {
            if pad.buttons & (1 << bit) != 0
                && let Some(b) = pads::core_button(bit)
            {
                p.gamepad_button(pad.slot as u32, b, false);
            }
        }
        p.gamepad_axes_rest(pad.slot as u32);
    }
}

/// L'état tenu des manettes (boutons, axes) renvoyé à l'ordinateur qui devient actif.
fn replay(p: &Arc<InputPipeline>, held: &[PadInfo]) {
    for pad in held {
        for bit in 0..32u8 {
            if pad.buttons & (1 << bit) != 0
                && let Some(b) = pads::core_button(bit)
            {
                p.gamepad_button(pad.slot as u32, b, true);
            }
        }
        for (i, v) in pad.axes.iter().enumerate() {
            if *v != 0
                && let Some(a) = pads::core_axis(i as u8)
            {
                p.gamepad_axis(pad.slot as u32, a, *v);
            }
        }
    }
}
