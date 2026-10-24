// Ce que ce Mac annonce à l'ordinateur : taille et fréquence de son écran,
// nombre de canaux de sa sortie audio.

import AppKit
import CoreAudio
import KyberKit

enum DisplayInfo {
    /// Mode de l'écran en pixels réels : 5K ou 6K au pixel près si l'écran
    /// l'est. Garde-fous : l'encodeur H.264 de NVIDIA s'arrête à 4096 de
    /// large, l'écran virtuel et les autres formats à 7680 × 4320 ; « au plus
    /// 4K » allège l'encodage, le décodage et le débit.
    static func mode(for screen: NSScreen, codec: VideoCodec = .hevc, limit4K: Bool = false) -> (width: Int, height: Int, refresh: Int) {
        let native = nativePixels(screen)
        var w = CGFloat(native.width)
        var h = CGFloat(native.height)
        let (maxW, maxH): (CGFloat, CGFloat) = limit4K ? (3840, 2160) : codec == .h264 ? (4096, 4096) : (7680, 4320)
        let fit = min(1, maxW / w, maxH / h)
        w = (w * fit / 2).rounded() * 2
        h = (h * fit / 2).rounded() * 2
        let hz = max(30, screen.maximumFramesPerSecond)
        return (Int(w), Int(h), hz)
    }

    /// Pixels réels de la dalle. En mode « plus d'espace », macOS dessine plus
    /// grand puis réduit (un 4K réglé en 3008 × 1692 points dessine
    /// 6016 × 3384) : demander ces pixels-là à l'ordinateur ne servirait qu'à
    /// les jeter. Repli : points × échelle.
    static func nativePixels(_ screen: NSScreen) -> (width: Int, height: Int) {
        let drawn = (Int(screen.frame.width * screen.backingScaleFactor), Int(screen.frame.height * screen.backingScaleFactor))
        guard let id = screen.deviceDescription[NSDeviceDescriptionKey("NSScreenNumber")] as? CGDirectDisplayID,
              let modes = CGDisplayCopyAllDisplayModes(id, [kCGDisplayShowDuplicateLowResolutionModes: true] as CFDictionary) as? [CGDisplayMode],
              let native = modes.first(where: { $0.ioFlags & UInt32(kDisplayModeNativeFlag) != 0 })
        else { return drawn }
        return (native.pixelWidth, native.pixelHeight)
    }

    /// « 3008 × 1692 » si macOS agrandit l'affichage (mode « plus d'espace »), nil sinon.
    static func scaledLooksLike(_ screen: NSScreen) -> String? {
        let native = nativePixels(screen)
        let drawn = Int(screen.frame.width * screen.backingScaleFactor)
        guard drawn != native.width else { return nil }
        return "\(Int(screen.frame.width)) × \(Int(screen.frame.height))"
    }

    /// Fréquence variable (ProMotion, Adaptive-Sync) : l'écran attend l'image
    /// au lieu de l'afficher au top suivant.
    static func variableRefresh(_ screen: NSScreen) -> Bool {
        screen.maximumRefreshInterval > screen.minimumRefreshInterval + 0.0005
    }

    /// « 120 Hz variable (60–120) », « 60 Hz ».
    static func refreshLabel(_ screen: NSScreen) -> String {
        let max = screen.maximumFramesPerSecond
        guard variableRefresh(screen) else { return "\(max) Hz" }
        let min = Int((1 / screen.maximumRefreshInterval).rounded())
        return String(localized: "\(max) Hz variable (\(min)–\(max))")
    }
}

enum AudioOutput {
    /// Canaux à demander à l'ordinateur. Stéréo par défaut : le nombre de
    /// sorties d'une carte son ne dit rien des enceintes branchées (une
    /// Focusrite à 8 sorties pilote souvent une simple paire). Le 5.1 et le
    /// 7.1 se choisissent dans les réglages de l'ordinateur (`AudioChoice`).
    static func channels() -> Int { 2 }
}
