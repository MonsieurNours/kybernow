//! Ce que l'appareil tient enfoncé sur l'ordinateur, et la politique « tout
//! relâcher » commune au Mac, à l'Apple TV et à Windows.
//!
//! Le protocole kynput est en deltas : un relâchement qui ne part pas laisse
//! la touche collée sur l'ordinateur jusqu'à la fin de la session. Trois
//! moments où les relâchements ne nous parviendraient plus :
//! - la fenêtre perd le clavier (⌘Tab, ⌘Espace, Spotlight, autre app ;
//!   Alt+Tab sur Windows) : [`ReleaseAll::release`] relâche touches,
//!   modificateurs et boutons de souris tenus. Pas de FlushKeys : il
//!   relâcherait aussi les manettes, qui, lues par SDL, continuent de parler
//!   à l'ordinateur sans la fenêtre et ne renverraient pas leur état ;
//! - l'app Apple TV passe en arrière-plan (bouton TV, autre app, veille) : la
//!   manette en souris est relâchée (`release`), les manettes remises à zéro,
//!   puis FlushKeys pour le clavier Bluetooth et tout le reste, l'ordinateur
//!   relâchant de son côté ;
//! - la fin de session : FlushKeys, et tout est oublié (`forget`).
//!
//! Au retour (clavier retrouvé ; entrées reliées de nouveau après une
//! relance de l'image : le service d'entrées de Kyber est neuf et ne tient
//! plus rien), les modificateurs réellement tenus sont resynchronisés
//! ([`ReleaseAll::resync_modifiers`]) : ceux que nous ne comptons pas encore
//! repartent, du côté réellement tenu. Les autres touches et les boutons ne
//! sont pas renvoyés : un jeu y verrait un nouvel appui.
//!
//! Aucun verrou ici : chaque plateforme l'appelle là où elle tient ses
//! entrées, ou passe par [`super::pipeline::InputPipeline`], qui en tient un
//! sous verrou pour les touches et clics qu'il envoie.

use std::collections::HashSet;

use kynput_rs::MouseButtonType;

/// Ce qu'un relâchement général envoie.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Release {
    MouseButton(MouseButtonType),
    Key { scancode: u16 },
}

#[derive(Default)]
pub struct ReleaseAll {
    /// Scancodes partis en appui, pas encore en relâchement.
    scancodes: HashSet<u16>,
    mouse_buttons: HashSet<MouseButtonType>,
    /// Touches de modification tenues, dans l'identifiant de la plateforme
    /// (code kVK sur Mac, usage HID avec GameController, touche virtuelle sur
    /// Windows) : ce qu'il faut pour les resynchroniser.
    modifiers: HashSet<u16>,
}

impl ReleaseAll {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn scancodes(&self) -> &HashSet<u16> {
        &self.scancodes
    }

    pub fn mouse_buttons(&self) -> &HashSet<MouseButtonType> {
        &self.mouse_buttons
    }

    pub fn modifiers(&self) -> &HashSet<u16> {
        &self.modifiers
    }

    pub fn is_held(&self, scancode: u16) -> bool {
        self.scancodes.contains(&scancode)
    }

    pub fn key(&mut self, scancode: u16, pressed: bool) {
        if pressed {
            self.scancodes.insert(scancode);
        } else {
            self.scancodes.remove(&scancode);
        }
    }

    pub fn mouse_button(&mut self, button: MouseButtonType, pressed: bool) {
        if pressed {
            self.mouse_buttons.insert(button);
        } else {
            self.mouse_buttons.remove(&button);
        }
    }

    /// `true` : l'état a changé (touche nouvellement tenue, ou relâchée alors
    /// qu'elle l'était) ; `false` : rien à envoyer.
    pub fn modifier(&mut self, id: u16, pressed: bool) -> bool {
        if pressed { self.modifiers.insert(id) } else { self.modifiers.remove(&id) }
    }

    /// Relâche sur l'ordinateur tout ce qui est tenu : boutons de souris, puis
    /// touches (modificateurs compris, par leur scancode), et oublie tout.
    pub fn release(&mut self, mut send: impl FnMut(Release)) {
        for button in self.mouse_buttons.drain() {
            send(Release::MouseButton(button));
        }
        for scancode in self.scancodes.drain() {
            send(Release::Key { scancode });
        }
        self.modifiers.clear();
    }

    /// Plus rien n'est tenu (fin de session, après FlushKeys).
    pub fn forget(&mut self) {
        self.scancodes.clear();
        self.mouse_buttons.clear();
        self.modifiers.clear();
    }

    /// Les modificateurs ne sont plus tenus sur l'ordinateur (service
    /// d'entrées relancé) : à resynchroniser.
    pub fn forget_modifiers(&mut self) {
        self.modifiers.clear();
    }

    /// Modificateurs réellement tenus à cet instant : ceux que nous ne
    /// comptons pas encore repartent en appui (`send`, qui doit aussi noter le
    /// scancode par [`Self::key`]).
    pub fn resync_modifiers(&mut self, held: &[u16], mut send: impl FnMut(u16)) {
        for &id in held {
            if self.modifiers.insert(id) {
                send(id);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tout_relacher() {
        let mut held = ReleaseAll::new();
        held.key(0x1E, true);
        held.key(0x1D, true);
        held.key(0x1E, false);
        held.mouse_button(MouseButtonType::Left, true);
        assert!(held.modifier(0x37, true), "modificateur nouvellement tenu");
        assert!(!held.modifier(0x37, true), "déjà tenu : rien à envoyer");
        let mut sent = Vec::new();
        held.release(|r| sent.push(r));
        assert_eq!(sent, [Release::MouseButton(MouseButtonType::Left), Release::Key { scancode: 0x1D }], "boutons puis touches tenues");
        assert!(held.scancodes().is_empty() && held.mouse_buttons().is_empty() && held.modifiers().is_empty());
    }

    #[test]
    fn modificateurs_renvoyes_au_retour() {
        let mut held = ReleaseAll::new();
        held.modifier(0x38, true);
        held.forget_modifiers();
        let mut sent = Vec::new();
        held.resync_modifiers(&[0x38, 0x3B], |id| sent.push(id));
        assert_eq!(sent, [0x38, 0x3B], "ceux que nous ne comptons plus repartent");
        sent.clear();
        held.resync_modifiers(&[0x38], |id| sent.push(id));
        assert!(sent.is_empty(), "déjà comptés : rien");
        assert!(held.modifier(0x3B, false), "relâché alors qu'il était tenu");
        assert!(!held.modifier(0x3B, false), "déjà relâché : rien");
    }
}
