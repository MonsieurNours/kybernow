// Clavier : usages USB HID (page 7) → scancodes Windows via kynput, et les
// règles communes aux appareils Apple : « ⌘ envoyée comme Ctrl », échange des
// touches des claviers Apple ISO, touches de verrouillage.
//
// Chaque plateforme fournit l'usage HID de la touche et dit si le clavier
// est ISO :
// - Mac : table kVK → HID et `KBGetLayoutType` (KeymapMac.swift) ;
// - Apple TV et iOS : GameController donne directement l'usage HID
//   (`GCKeyCode`), tvOS ne dit pas la disposition : réglage « Clavier Apple
//   ISO », appliqué aux claviers Apple ou de fabricant inconnu (`appleKeyboard`).
//
// Clavier Apple ISO (européen, une touche de plus près de ⇧ gauche) : la
// touche sous Échap (`@#` sur un AZERTY, `§±` ailleurs) arrive comme usage
// 0x64 et celle près de ⇧ (`<>`) comme 0x35, à l'inverse de ce qu'attend
// l'ordinateur (quirk hid-apple ; SDL fait le même échange). Confirmé le
// 2026-10-20 sur un MacBook AZERTY : `@` donnait `<`.
//
// Verr. maj. et Verr. num. sont des bascules, pas des touches tenues : chaque
// plateforme les envoie en appui + relâchement (une frappe), l'ordinateur
// bascule son verrouillage à l'appui, et elles ne sont jamais comptées parmi
// les modificateurs tenus ni resynchronisées : les deux côtés restent
// d'accord tant que chaque bascule est signalée une fois.

@_implementationOnly import CKyber

public enum Keymap {
    // Usages HID des touches de modification (0xE0 à 0xE7).
    static let leftControl: UInt16 = 0xE0
    static let leftShift: UInt16 = 0xE1
    static let leftAlt: UInt16 = 0xE2
    static let leftGUI: UInt16 = 0xE3
    static let rightControl: UInt16 = 0xE4
    static let rightShift: UInt16 = 0xE5
    static let rightAlt: UInt16 = 0xE6
    static let rightGUI: UInt16 = 0xE7
    static let modifierUsages: ClosedRange<UInt16> = 0xE0...0xE7

    /// Touches de verrouillage (bascules) : Verr. maj. et Verr. num.
    static let capsLock: UInt16 = 0x39
    static let numLock: UInt16 = 0x53

    /// Les deux touches échangées par les claviers Apple ISO : `<>` près de ⇧
    /// (0x35, kVK_ANSI_Grave) et la touche sous Échap (0x64, kVK_ISO_Section).
    static let grave: UInt16 = 0x35
    static let isoSection: UInt16 = 0x64

    static func isModifier(_ usage: UInt16) -> Bool { modifierUsages.contains(usage) }
    static func isLock(_ usage: UInt16) -> Bool { usage == capsLock || usage == numLock }

    /// Usage HID à envoyer, après « ⌘ envoyée comme Ctrl » et l'échange des
    /// claviers Apple ISO. `iso` n'est évalué que pour les deux touches
    /// concernées (sur Mac, la première évaluation interroge le système).
    static func usage(hid: UInt16, commandAsControl: Bool = false, iso: @autoclosure () -> Bool) -> UInt16 {
        var usage = hid
        if commandAsControl {
            if usage == leftGUI { usage = leftControl } else if usage == rightGUI { usage = rightControl }
        }
        if usage == grave || usage == isoSection, iso() {
            usage = usage == grave ? isoSection : grave
        }
        return usage
    }

    /// Scancode Windows (kynput) d'un usage HID ; nil : touche inconnue de kynput.
    static func scancode(usage: UInt16) -> UInt16? {
        var scancode: UInt16 = 0
        return kynput_keycode_usb_to_kyber(usage, &scancode) ? scancode : nil
    }

    /// Les deux en un : scancode à envoyer pour une touche donnée en usage HID.
    static func scancode(hid: UInt16, commandAsControl: Bool = false, iso: @autoclosure () -> Bool) -> UInt16? {
        scancode(usage: usage(hid: hid, commandAsControl: commandAsControl, iso: iso()))
    }

    /// Apple TV et iOS : le réglage « Clavier Apple ISO » s'applique aux
    /// claviers dont le fabricant est Apple, ou inconnu (le clavier fusionné de
    /// GameController ne dit pas toujours le sien).
    static func appleKeyboard(vendorName: String?) -> Bool {
        vendorName.map { $0.localizedCaseInsensitiveContains("Apple") } ?? true
    }
}
