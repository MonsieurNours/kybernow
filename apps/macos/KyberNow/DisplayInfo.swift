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
        let scale = screen.backingScaleFactor
        var w = screen.frame.width * scale
        var h = screen.frame.height * scale
        let (maxW, maxH): (CGFloat, CGFloat) = limit4K ? (3840, 2160) : codec == .h264 ? (4096, 4096) : (7680, 4320)
        let fit = min(1, maxW / w, maxH / h)
        w = (w * fit / 2).rounded() * 2
        h = (h * fit / 2).rounded() * 2
        let hz = max(30, screen.maximumFramesPerSecond)
        return (Int(w), Int(h), hz)
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
        return "\(max) Hz variable (\(min)–\(max))"
    }
}

enum AudioOutput {
    /// Canaux à demander à l'ordinateur. Stéréo par défaut : le nombre de
    /// sorties d'une carte son ne dit rien des enceintes branchées (une
    /// Focusrite à 8 sorties pilote souvent une simple paire). Le 5.1 et le
    /// 7.1 viendront d'un choix explicite dans les réglages de l'app.
    static func channels() -> Int { 2 }
}
