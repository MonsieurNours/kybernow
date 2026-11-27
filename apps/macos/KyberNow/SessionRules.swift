// Règles sans état de la fenêtre de session et du mode immersif, à part pour
// être essayées seules (tests/rules/run.sh).

import Foundation

enum SessionRules {
    /// Un état du cœur arrivé une fois la fin lancée (`.ending`, `.ended`) :
    /// tout est ignoré sauf la fin elle-même (MAC-07, une session finie ne
    /// revit pas).
    static func ignoresAfterEnd(ending: Bool, incomingIsEnded: Bool) -> Bool {
        ending && !incomingIsEnded
    }

    /// Bits propres à chaque modificateur, gauche et droite (IOLLEvent.h,
    /// `NX_DEVICE…KEYMASK`) : posé à l'appui, retiré au relâchement.
    private static let deviceBits: [UInt16: UInt64] = [
        0x37: 0x0000_0008, 0x36: 0x0000_0010, // ⌘ gauche, droite
        0x38: 0x0000_0002, 0x3C: 0x0000_0004, // ⇧
        0x3A: 0x0000_0020, 0x3D: 0x0000_0040, // ⌥
        0x3B: 0x0000_0001, 0x3E: 0x0000_2000, // ⌃
    ]

    /// Pour un `flagsChanged` : vrai à l'appui, faux au relâchement ; nil
    /// pour une touche sans bit propre (Verr. Maj, fn), qui bascule.
    static func modifierPressed(code: UInt16, flags: UInt64) -> Bool? {
        guard let bit = deviceBits[code] else { return nil }
        return flags & bit != 0
    }
}
