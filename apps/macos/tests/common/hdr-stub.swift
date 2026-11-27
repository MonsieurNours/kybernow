// Bouchon de `HDRDisplay` (KyberKit/HDR.swift) pour les essais d'affichage :
// HDR.swift tire le cœur et la session entière. Écran SDR, pas d'observation.
import AppKit

enum HDRDisplay {
    static var forced: Bool? { nil }
    static func isHDR(_ screen: NSScreen?) -> Bool { false }
    static func headroom(_ screen: NSScreen?) -> Double { 1 }
    static func observe(_ handler: @escaping () -> Void) -> [NSObjectProtocol] { [] }
}

// Catalogue de traduction de KyberKit : celui du binaire d'essai.
extension Bundle { static let kyberKit = Bundle.main }
