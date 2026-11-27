// Le nom d'une touche d'après son usage HID, sur l'iPad (réglages du clavier
// et de la souris d'une console : « Z », « Q »…), selon la « Disposition du
// clavier » des Réglages de l'app (docs/link-ui.md, « Réglages de l'app » ›
// Clavier) : Auto suit le clavier physique branché quand iPadOS le dit (la
// langue de saisie du premier répondeur, qui suit le clavier branché), sinon
// la langue du premier clavier ; AZERTY pour le français de France et de
// Belgique, QWERTZ pour l'allemand et le français de Suisse, QWERTY sinon (le
// français du Canada compris). Le pendant de `Keymap.character(hid:)` du Mac
// (KeymapMac.swift), où le système donne la disposition.

import GameController
import UIKit

/// La disposition du clavier branché (Réglages de l'app › Clavier).
public enum KeyboardLayout: String, CaseIterable, Sendable {
    case auto, azerty, qwerty, qwertz

    /// La disposition d'une langue de clavier (« fr-FR », « de-CH »…).
    public static func of(language: String) -> KeyboardLayout {
        let l = language.replacingOccurrences(of: "_", with: "-")
        if l.hasPrefix("fr-FR") || l.hasPrefix("fr-BE") || l == "fr" { return .azerty }
        if l.hasPrefix("de") || l.hasPrefix("fr-CH") { return .qwertz }
        return .qwerty
    }
}

extension Keymap {
    /// Ce qu'affiche la touche de cet usage HID, en majuscules ; nil pour une
    /// touche sans caractère. Fil principal.
    public static func character(hid usage: UInt16) -> String? {
        let layout = Self.resolvedLayout
        let azerty = layout == .azerty
        if layout == .qwertz, let c = qwertz(usage) { return c }
        switch usage {
        case 0x04...0x1D:
            var letter = UInt16(0x41) + usage - 0x04
            if azerty {
                // A ↔ Q, Z ↔ W ; la touche M est à la place du point-virgule.
                switch usage {
                case 0x04: letter = 0x51
                case 0x14: letter = 0x41
                case 0x1A: letter = 0x5A
                case 0x1D: letter = 0x57
                case 0x10: return ","
                default: break
                }
            }
            return String(UnicodeScalar(UInt8(letter)))
        case 0x1E...0x26:
            return azerty ? ["&", "É", "\"", "'", "(", "§", "È", "!", "Ç"][Int(usage - 0x1E)] : String(usage - 0x1E + 1)
        case 0x27: return azerty ? "À" : "0"
        case 0x2D: return azerty ? ")" : "-"
        case 0x2E: return azerty ? "-" : "="
        case 0x2F: return azerty ? "^" : "["
        case 0x30: return azerty ? "$" : "]"
        case 0x31: return "\\"
        case 0x33: return azerty ? "M" : ";"
        case 0x34: return azerty ? "Ù" : "'"
        case 0x35: return azerty ? "@" : "`"
        case 0x36: return azerty ? ";" : ","
        case 0x37: return azerty ? ":" : "."
        case 0x38: return azerty ? "=" : "/"
        case 0x64: return "<"
        case 0x54: return "/"
        case 0x55: return "*"
        case 0x56: return "-"
        case 0x57: return "+"
        case 0x59...0x61: return String(usage - 0x59 + 1)
        case 0x62: return "0"
        case 0x63: return "."
        case 0x67: return "="
        default: return nil
        }
    }

    /// La disposition choisie dans les Réglages de l'app (Auto par défaut).
    nonisolated(unsafe) public static var layout: KeyboardLayout = .auto

    /// La disposition en vigueur : le choix, ou pour Auto celle du clavier
    /// branché, sinon celle du premier clavier de l'appareil.
    public static var resolvedLayout: KeyboardLayout {
        guard layout == .auto else { return layout }
        // La langue de saisie du premier répondeur se lit sur le fil principal (UIKit).
        let typed = GCKeyboard.coalesced != nil && Thread.isMainThread
            ? MainActor.assumeIsolated { FirstResponder.current?.textInputMode?.primaryLanguage }
            : nil
        let language = typed
            ?? UITextInputMode.activeInputModes.first?.primaryLanguage
            ?? Locale.preferredLanguages.first ?? ""
        return KeyboardLayout.of(language: language)
    }

    /// Les touches que le QWERTZ allemand nomme autrement que le QWERTY.
    private static func qwertz(_ usage: UInt16) -> String? {
        switch usage {
        case 0x1C: return "Z"
        case 0x1D: return "Y"
        case 0x2D: return "ß"
        case 0x2E: return "´"
        case 0x2F: return "Ü"
        case 0x30: return "+"
        case 0x31: return "#"
        case 0x33: return "Ö"
        case 0x34: return "Ä"
        case 0x35: return "^"
        case 0x36: return ","
        case 0x37: return "."
        case 0x38: return "-"
        default: return nil
        }
    }
}

/// Le premier répondeur de l'app (sa langue de saisie suit le clavier branché).
@MainActor private enum FirstResponder {
    private static weak var found: UIResponder?

    static var current: UIResponder? {
        found = nil
        UIApplication.shared.sendAction(#selector(UIResponder.kn_reportFirstResponder), to: nil, from: nil, for: nil)
        return found
    }

    static func report(_ r: UIResponder) { found = r }
}

extension UIResponder {
    @objc fileprivate func kn_reportFirstResponder() {
        MainActor.assumeIsolated { FirstResponder.report(self) }
    }
}
