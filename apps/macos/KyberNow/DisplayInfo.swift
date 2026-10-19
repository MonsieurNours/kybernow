// Ce que ce Mac annonce à l'ordinateur : taille et fréquence de son écran,
// nombre de canaux de sa sortie audio.

import AppKit
import CoreAudio

enum DisplayInfo {
    /// Mode de l'écran en pixels réels, ramené à la 4K au plus (au-delà,
    /// l'encodeur n'apporte rien de visible et coûte en latence).
    static func mode(for screen: NSScreen) -> (width: Int, height: Int, refresh: Int) {
        let scale = screen.backingScaleFactor
        var w = screen.frame.width * scale
        var h = screen.frame.height * scale
        let fit = min(1, 3840 / w, 2160 / h)
        w = (w * fit / 2).rounded() * 2
        h = (h * fit / 2).rounded() * 2
        let hz = max(30, screen.maximumFramesPerSecond)
        return (Int(w), Int(h), hz)
    }
}

enum AudioOutput {
    /// Canaux à demander à l'ordinateur. Stéréo par défaut : le nombre de
    /// sorties d'une carte son ne dit rien des enceintes branchées (une
    /// Focusrite à 8 sorties pilote souvent une simple paire). Le 5.1 et le
    /// 7.1 viendront d'un choix explicite dans les réglages de l'app.
    static func channels() -> Int { 2 }
}
