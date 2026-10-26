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

    /// Identifiant Core Graphics d'un écran.
    static func displayID(_ screen: NSScreen) -> CGDirectDisplayID? {
        screen.deviceDescription[NSDeviceDescriptionKey("NSScreenNumber")] as? CGDirectDisplayID
    }

    /// Écrans que la Station sait créer (`virtual_display.max_screens`, 1
    /// avant le multi-écrans). `KN_MAX_SCREENS=4` le force (essai des
    /// fenêtres et du repli sur un écran avec une Station qui n'en crée qu'un).
    static func maxScreens(_ info: HostInfo?) -> Int {
        if let forced = ProcessInfo.processInfo.environment["KN_MAX_SCREENS"].flatMap(Int.init), forced > 0 { return forced }
        return info?.maxScreens ?? 1
    }

    /// Multi-écrans (docs/multi-ecrans.md) : les écrans de ce Mac, des écrans
    /// en recopie comptant pour un, dans l'ordre de `NSScreen.screens` (le
    /// premier, celui de la barre des menus, est l'écran principal), et leur
    /// description pour le cœur : cadre en points converti du repère de Cocoa
    /// (origine en bas à gauche de l'écran principal, y vers le haut) vers
    /// celui du contrat (y vers le bas), pixels natifs, fréquence ; « Cet
    /// écran » (`current`), réglages de l'ordinateur et écrans que la Station
    /// sait créer.
    static func deviceScreens(current: NSScreen?, settings: ConnectionSettings, info: HostInfo?) -> (screens: [NSScreen], request: SessionEngine.DeviceScreens) {
        let all = NSScreen.screens.filter { screen in
            guard let id = displayID(screen) else { return true }
            return CGDisplayMirrorsDisplay(id) == kCGNullDirectDisplay
        }
        let top = all.first?.frame.maxY ?? 0
        let list = all.enumerated().map { i, screen in
            let native = nativePixels(screen)
            return SessionEngine.DeviceScreens.Screen(
                x: screen.frame.minX, y: top - screen.frame.maxY, width: screen.frame.width, height: screen.frame.height,
                pixelWidth: native.width, pixelHeight: native.height, refresh: max(30, screen.maximumFramesPerSecond), primary: i == 0)
        }
        let currentID = current.flatMap(displayID)
        let index = all.firstIndex { displayID($0) == currentID } ?? 0
        let request = SessionEngine.DeviceScreens(
            list: list, current: index, limit4K: settings.limit4K,
            customResolution: settings.display == .custom || settings.display == .window,
            setting: settings.screens.rawValue, maxScreens: maxScreens(info),
            // L'écran virtuel utilisable sur la Station (inconnu tant qu'elle n'a pas répondu : oui).
            virtualDisplayAvailable: info?.virtualDisplay ?? true)
        return (all, request)
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
