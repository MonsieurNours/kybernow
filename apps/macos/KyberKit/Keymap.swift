// Touches du Mac (codes virtuels kVK, indépendants de la disposition du
// clavier) → usages USB HID (page 7) → scancodes Windows via kynput.

import AppKit
import Carbon.HIToolbox
@_implementationOnly import CKyber

/// Disposition physique (ANSI, JIS, ISO) d'un type de clavier. HIToolbox
/// exporte toujours la fonction, mais ses en-têtes ne la déclarent plus ;
/// SDL s'en sert de la même façon.
@_silgen_name("KBGetLayoutType") private func KBGetLayoutType(_ keyboardType: Int16) -> UInt8
private let kKeyboardISO: UInt8 = 2

enum Keymap {
    /// Usage HID des touches de modification, pour le réglage « ⌘ envoyée comme Ctrl ».
    static let leftControl: UInt16 = 0xE0
    static let leftGUI: UInt16 = 0xE3
    static let rightControl: UInt16 = 0xE4
    static let rightGUI: UInt16 = 0xE7

    private static let hid: [Int: UInt16] = [
        kVK_ANSI_A: 0x04, kVK_ANSI_B: 0x05, kVK_ANSI_C: 0x06, kVK_ANSI_D: 0x07, kVK_ANSI_E: 0x08,
        kVK_ANSI_F: 0x09, kVK_ANSI_G: 0x0A, kVK_ANSI_H: 0x0B, kVK_ANSI_I: 0x0C, kVK_ANSI_J: 0x0D,
        kVK_ANSI_K: 0x0E, kVK_ANSI_L: 0x0F, kVK_ANSI_M: 0x10, kVK_ANSI_N: 0x11, kVK_ANSI_O: 0x12,
        kVK_ANSI_P: 0x13, kVK_ANSI_Q: 0x14, kVK_ANSI_R: 0x15, kVK_ANSI_S: 0x16, kVK_ANSI_T: 0x17,
        kVK_ANSI_U: 0x18, kVK_ANSI_V: 0x19, kVK_ANSI_W: 0x1A, kVK_ANSI_X: 0x1B, kVK_ANSI_Y: 0x1C,
        kVK_ANSI_Z: 0x1D,
        kVK_ANSI_1: 0x1E, kVK_ANSI_2: 0x1F, kVK_ANSI_3: 0x20, kVK_ANSI_4: 0x21, kVK_ANSI_5: 0x22,
        kVK_ANSI_6: 0x23, kVK_ANSI_7: 0x24, kVK_ANSI_8: 0x25, kVK_ANSI_9: 0x26, kVK_ANSI_0: 0x27,
        kVK_Return: 0x28, kVK_Escape: 0x29, kVK_Delete: 0x2A, kVK_Tab: 0x2B, kVK_Space: 0x2C,
        kVK_ANSI_Minus: 0x2D, kVK_ANSI_Equal: 0x2E, kVK_ANSI_LeftBracket: 0x2F, kVK_ANSI_RightBracket: 0x30,
        kVK_ANSI_Backslash: 0x31, kVK_ANSI_Semicolon: 0x33, kVK_ANSI_Quote: 0x34, kVK_ANSI_Grave: 0x35,
        kVK_ANSI_Comma: 0x36, kVK_ANSI_Period: 0x37, kVK_ANSI_Slash: 0x38, kVK_CapsLock: 0x39,
        kVK_F1: 0x3A, kVK_F2: 0x3B, kVK_F3: 0x3C, kVK_F4: 0x3D, kVK_F5: 0x3E, kVK_F6: 0x3F,
        kVK_F7: 0x40, kVK_F8: 0x41, kVK_F9: 0x42, kVK_F10: 0x43, kVK_F11: 0x44, kVK_F12: 0x45,
        kVK_F13: 0x68, kVK_F14: 0x69, kVK_F15: 0x6A, kVK_F16: 0x6B, kVK_F17: 0x6C, kVK_F18: 0x6D,
        kVK_F19: 0x6E, kVK_F20: 0x6F,
        kVK_Help: 0x49, kVK_Home: 0x4A, kVK_PageUp: 0x4B, kVK_ForwardDelete: 0x4C, kVK_End: 0x4D,
        kVK_PageDown: 0x4E, kVK_RightArrow: 0x4F, kVK_LeftArrow: 0x50, kVK_DownArrow: 0x51, kVK_UpArrow: 0x52,
        kVK_ANSI_KeypadClear: 0x53, kVK_ANSI_KeypadDivide: 0x54, kVK_ANSI_KeypadMultiply: 0x55,
        kVK_ANSI_KeypadMinus: 0x56, kVK_ANSI_KeypadPlus: 0x57, kVK_ANSI_KeypadEnter: 0x58,
        kVK_ANSI_Keypad1: 0x59, kVK_ANSI_Keypad2: 0x5A, kVK_ANSI_Keypad3: 0x5B, kVK_ANSI_Keypad4: 0x5C,
        kVK_ANSI_Keypad5: 0x5D, kVK_ANSI_Keypad6: 0x5E, kVK_ANSI_Keypad7: 0x5F, kVK_ANSI_Keypad8: 0x60,
        kVK_ANSI_Keypad9: 0x61, kVK_ANSI_Keypad0: 0x62, kVK_ANSI_KeypadDecimal: 0x63,
        kVK_ISO_Section: 0x64, kVK_ANSI_KeypadEquals: 0x67,
        kVK_JIS_Yen: 0x89, kVK_JIS_Underscore: 0x87, kVK_JIS_KeypadComma: 0x85,
        kVK_JIS_Eisu: 0x91, kVK_JIS_Kana: 0x90,
        kVK_Control: 0xE0, kVK_Shift: 0xE1, kVK_Option: 0xE2, kVK_Command: 0xE3,
        kVK_RightControl: 0xE4, kVK_RightShift: 0xE5, kVK_RightOption: 0xE6, kVK_RightCommand: 0xE7,
        kVK_VolumeUp: 0x80, kVK_VolumeDown: 0x81, kVK_Mute: 0x7F,
    ]

    /// Usage HID d'une touche du Mac, en appliquant « ⌘ envoyée comme Ctrl »
    /// et l'échange des claviers Apple ISO. `keyboardType` : celui de
    /// l'événement (`kCGKeyboardEventKeyboardType`) ; nil : le dernier clavier
    /// utilisé.
    static func usage(keyCode: UInt16, commandAsControl: Bool, keyboardType: Int64? = nil) -> UInt16? {
        guard var usage = hid[Int(keyCode)] else { return nil }
        if commandAsControl {
            if usage == leftGUI { usage = leftControl } else if usage == rightGUI { usage = rightControl }
        }
        if usage == 0x35 || usage == 0x64, isISO(keyboardType: keyboardType) {
            usage = usage == 0x35 ? 0x64 : 0x35
        }
        return usage
    }

    /// Clavier Apple ISO (européen, une touche de plus près de ⇧ gauche) : le
    /// Mac rapporte la touche sous Échap (`@#` sur un AZERTY, `§±` ailleurs)
    /// comme kVK_ISO_Section, usage HID 0x64, et celle près de ⇧ (`<>`) comme
    /// kVK_ANSI_Grave, usage 0x35, à l'inverse de ce qu'attend l'ordinateur
    /// (quirk hid-apple ; SDL fait le même échange). Confirmé le 2026-10-20 sur
    /// un MacBook AZERTY : `@` donnait `<`. Fil principal seulement.
    private static var layouts: [Int16: Bool] = [:]

    static func isISO(keyboardType: Int64?) -> Bool {
        let type = keyboardType.map { Int16(clamping: $0) } ?? Int16(LMGetKbdType())
        if let known = layouts[type] { return known }
        let iso = KBGetLayoutType(type) == kKeyboardISO
        layouts[type] = iso
        KyberLog.app.notice("clavier de type \(type) : \(iso ? "ISO (échange @# / <>)" : "ANSI ou JIS", privacy: .public)")
        return iso
    }

    static func scancode(usage: UInt16) -> UInt16? {
        var scancode: UInt16 = 0
        return kynput_keycode_usb_to_kyber(usage, &scancode) ? scancode : nil
    }

    /// Touches de modification : (code kVK, drapeau qui les signale).
    static let modifiers: [UInt16: NSEvent.ModifierFlags] = [
        UInt16(kVK_Shift): .shift, UInt16(kVK_RightShift): .shift,
        UInt16(kVK_Control): .control, UInt16(kVK_RightControl): .control,
        UInt16(kVK_Option): .option, UInt16(kVK_RightOption): .option,
        UInt16(kVK_Command): .command, UInt16(kVK_RightCommand): .command,
        UInt16(kVK_CapsLock): .capsLock,
    ]

    /// Modificateurs tenus : drapeau, touche de chaque côté, et le bit propre
    /// à chaque côté dans `modifierFlags.rawValue` (NX_DEVICE…KEYMASK d'IOKit),
    /// seul moyen de savoir lequel des deux est enfoncé.
    struct HeldModifier {
        let flag: NSEvent.ModifierFlags
        let left: UInt16, right: UInt16
        let leftBit: UInt, rightBit: UInt
    }

    static let heldModifiers: [HeldModifier] = [
        .init(flag: .shift, left: UInt16(kVK_Shift), right: UInt16(kVK_RightShift), leftBit: 0x02, rightBit: 0x04),
        .init(flag: .control, left: UInt16(kVK_Control), right: UInt16(kVK_RightControl), leftBit: 0x01, rightBit: 0x2000),
        .init(flag: .option, left: UInt16(kVK_Option), right: UInt16(kVK_RightOption), leftBit: 0x20, rightBit: 0x40),
        .init(flag: .command, left: UInt16(kVK_Command), right: UInt16(kVK_RightCommand), leftBit: 0x08, rightBit: 0x10),
    ]

    /// Bit du côté que porte cette touche de modification (nil : Verr. maj.).
    static func sideBit(_ code: UInt16) -> UInt? {
        for m in heldModifiers {
            if m.left == code { return m.leftBit }
            if m.right == code { return m.rightBit }
        }
        return nil
    }
}
