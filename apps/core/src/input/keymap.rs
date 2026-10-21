//! Clavier : usages USB HID (page 7) → scancodes Windows via kynput, et les
//! règles communes aux appareils Apple : « ⌘ envoyée comme Ctrl », échange
//! des touches des claviers Apple ISO, touches de verrouillage.
//!
//! Chaque plateforme fournit l'usage HID de la touche et dit si le clavier
//! est ISO :
//! - Mac : table kVK → HID et `KBGetLayoutType` (restent dans l'app, voir
//!   `KeymapMac.swift`) ;
//! - Apple TV et iOS : GameController donne directement l'usage HID
//!   (`GCKeyCode`), tvOS ne dit pas la disposition : réglage « Clavier Apple
//!   ISO », appliqué aux claviers Apple ou de fabricant inconnu
//!   ([`apple_keyboard`]) ;
//! - Windows : les scancodes de Windows partent tels quels (`InputPipeline::key`),
//!   sans passer par ici.
//!
//! Clavier Apple ISO (européen, une touche de plus près de ⇧ gauche) : la
//! touche sous Échap (`@#` sur un AZERTY, `§±` ailleurs) arrive comme usage
//! 0x64 et celle près de ⇧ (`<>`) comme 0x35, à l'inverse de ce qu'attend
//! l'ordinateur (quirk hid-apple ; SDL fait le même échange). Confirmé le
//! 2026-10-20 sur un MacBook AZERTY : `@` donnait `<`.
//!
//! Verr. maj. et Verr. num. sont des bascules, pas des touches tenues : chaque
//! plateforme les envoie en appui + relâchement (une frappe), l'ordinateur
//! bascule son verrouillage à l'appui, et elles ne sont jamais comptées parmi
//! les modificateurs tenus ni resynchronisées : les deux côtés restent
//! d'accord tant que chaque bascule est signalée une fois.

use std::ops::RangeInclusive;

use kynput_sys as sys;

// Usages HID des touches de modification (0xE0 à 0xE7).
pub const LEFT_CONTROL: u16 = 0xE0;
pub const LEFT_SHIFT: u16 = 0xE1;
pub const LEFT_ALT: u16 = 0xE2;
pub const LEFT_GUI: u16 = 0xE3;
pub const RIGHT_CONTROL: u16 = 0xE4;
pub const RIGHT_SHIFT: u16 = 0xE5;
pub const RIGHT_ALT: u16 = 0xE6;
pub const RIGHT_GUI: u16 = 0xE7;
pub const MODIFIER_USAGES: RangeInclusive<u16> = 0xE0..=0xE7;

/// Touches de verrouillage (bascules) : Verr. maj. et Verr. num.
pub const CAPS_LOCK: u16 = 0x39;
pub const NUM_LOCK: u16 = 0x53;

/// Les deux touches échangées par les claviers Apple ISO : `<>` près de ⇧
/// (0x35, kVK_ANSI_Grave) et la touche sous Échap (0x64, kVK_ISO_Section).
pub const GRAVE: u16 = 0x35;
pub const ISO_SECTION: u16 = 0x64;

pub fn is_modifier(usage: u16) -> bool {
    MODIFIER_USAGES.contains(&usage)
}

pub fn is_lock(usage: u16) -> bool {
    usage == CAPS_LOCK || usage == NUM_LOCK
}

/// Usage HID à envoyer, après « ⌘ envoyée comme Ctrl » et l'échange des
/// claviers Apple ISO. `iso` n'est appelé que pour les deux touches
/// concernées (sur Mac, la première évaluation interroge le système).
pub fn usage(hid: u16, command_as_control: bool, iso: impl FnOnce() -> bool) -> u16 {
    let mut usage = hid;
    if command_as_control {
        if usage == LEFT_GUI {
            usage = LEFT_CONTROL;
        } else if usage == RIGHT_GUI {
            usage = RIGHT_CONTROL;
        }
    }
    if (usage == GRAVE || usage == ISO_SECTION) && iso() {
        usage = if usage == GRAVE { ISO_SECTION } else { GRAVE };
    }
    usage
}

/// Scancode Windows (kynput) d'un usage HID ; `None` : touche inconnue de kynput.
pub fn scancode_of_usage(usage: u16) -> Option<u16> {
    let mut scancode: u16 = 0;
    unsafe { sys::kynput_keycode_usb_to_kyber(usage, &mut scancode) }.then_some(scancode)
}

/// Les deux en un : scancode à envoyer pour une touche donnée en usage HID.
pub fn scancode(hid: u16, command_as_control: bool, iso: impl FnOnce() -> bool) -> Option<u16> {
    scancode_of_usage(usage(hid, command_as_control, iso))
}

/// Apple TV et iOS : le réglage « Clavier Apple ISO » s'applique aux
/// claviers dont le fabricant est Apple, ou inconnu (le clavier fusionné de
/// GameController ne dit pas toujours le sien).
pub fn apple_keyboard(vendor_name: Option<&str>) -> bool {
    vendor_name.is_none_or(|name| name.to_lowercase().contains("apple"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn table_du_clavier() {
        assert_eq!(scancode(0x04, false, || false), Some(0x1E), "A (usage 0x04) → scancode 0x1E");
        assert_eq!(scancode(0x28, false, || false), Some(0x1C), "Entrée (0x28) → 0x1C");
        assert_eq!(scancode(0x29, false, || false), Some(0x01), "Échap (0x29) → 0x01");
        assert_eq!(scancode(0x50, false, || false), scancode_of_usage(0x50), "flèche gauche : même scancode que kynput");
        assert!(scancode_of_usage(0x50).is_some());
        assert_eq!(scancode(0x1234, false, || false), None, "usage inconnu : None");
    }

    #[test]
    fn clavier_iso() {
        assert!(usage(0x35, false, || false) == 0x35 && usage(0x64, false, || false) == 0x64, "clavier ANSI : `<>` et la touche sous Échap inchangées");
        assert!(usage(0x35, false, || true) == 0x64 && usage(0x64, false, || true) == 0x35, "clavier Apple ISO : 0x35 ↔ 0x64 échangés");
        assert_eq!(usage(0x04, false, || true), 0x04, "clavier Apple ISO : les autres touches inchangées");
        assert_eq!(scancode(0x64, false, || true), scancode_of_usage(0x35), "la touche sous Échap part avec le scancode de `<>`");
        let mut evaluated = false;
        let _ = usage(0x04, false, || {
            evaluated = true;
            true
        });
        assert!(!evaluated, "la disposition n'est interrogée que pour les deux touches échangées");
    }

    #[test]
    fn commande_comme_controle() {
        assert!(usage(0xE3, true, || false) == 0xE0 && usage(0xE7, true, || false) == 0xE4, "⌘ envoyée comme Ctrl, des deux côtés");
        assert_eq!(usage(0xE3, false, || false), 0xE3, "⌘ gardée sinon");
    }

    #[test]
    fn verrouillages_et_modificateurs() {
        assert!(is_lock(CAPS_LOCK) && is_lock(NUM_LOCK) && !is_lock(0x04) && !is_lock(0xE1), "Verr. maj. et Verr. num. sont des bascules, pas A ni ⇧");
        assert_eq!(scancode(CAPS_LOCK, false, || true), Some(0x3A), "Verr. maj. → scancode 0x3A, même sur un clavier ISO");
        assert!(is_modifier(0xE0) && is_modifier(0xE7) && !is_modifier(0xDF) && !is_modifier(CAPS_LOCK), "modificateurs : usages 0xE0 à 0xE7 seulement");
        assert!(apple_keyboard(None) && apple_keyboard(Some("Apple Inc.")) && !apple_keyboard(Some("Logitech")), "réglage ISO : claviers Apple ou inconnus");
    }
}
