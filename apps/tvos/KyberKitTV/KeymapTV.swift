// Le nom d'une touche d'après son usage HID sur l'Apple TV : la disposition
// du clavier n'y est pas lue, et la disposition du clavier et de la souris
// d'une console n'y est pas montrée (la partie se joue à la manette). Le
// pendant de `Keymap.character(hid:)` du Mac (KeymapMac.swift) et de l'iPad
// (apps/ios/KyberKitIOS/KeymapIOS.swift), pour le texte partagé `PSText.input`.

extension Keymap {
    /// Toujours nil : `PSText` dit alors « Touche {n} ».
    public static func character(hid usage: UInt16) -> String? { nil }
}
