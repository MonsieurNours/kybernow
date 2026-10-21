// Presse-papiers du Mac pour le presse-papiers partagé (ClipboardBridge) :
// NSPasteboard, texte seulement. Sur iOS, l'équivalent branchera
// UIPasteboard ; tvOS n'a pas de presse-papiers (apps/tvos/KyberKitTV).

import AppKit

extension NSPasteboard: PlatformPasteboard {
    public var text: String? { string(forType: .string) }

    public func setText(_ text: String) {
        clearContents()
        setString(text, forType: .string)
    }
}

/// Presse-papiers de la plateforme.
enum Pasteboard {
    /// Le presse-papiers général ; nil sur une plateforme qui n'en a pas.
    static var general: PlatformPasteboard? { NSPasteboard.general }
}
