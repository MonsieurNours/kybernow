// Le nom d'une touche d'après son usage HID, sur l'iPad (réglages du clavier
// et de la souris d'une console : « Z », « Q »…). iOS ne donne pas la
// disposition du clavier branché : celle du clavier actif à l'écran en tient
// lieu (AZERTY quand il est en français de France ou de Belgique, sinon
// QWERTY). Le pendant de `Keymap.character(hid:)` du Mac (KeymapMac.swift).

import UIKit

extension Keymap {
    /// Ce qu'affiche la touche de cet usage HID, en majuscules ; nil pour une
    /// touche sans caractère. Fil principal.
    public static func character(hid usage: UInt16) -> String? {
        let azerty = Self.azerty
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

    /// Le clavier actif est français (France, Belgique) : AZERTY.
    private static var azerty: Bool {
        let language = UITextInputMode.activeInputModes.first?.primaryLanguage
            ?? Locale.preferredLanguages.first ?? ""
        return ["fr-FR", "fr-BE", "fr"].contains(language) || language.hasPrefix("fr-FR") || language.hasPrefix("fr-BE")
    }
}
