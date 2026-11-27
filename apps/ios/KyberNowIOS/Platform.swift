// Ce que l'iPhone ou l'iPad sait de lui-même : son écran (définition réelle
// et fréquence, ProMotion), la sortie du son (haut-parleur, AirPods, HDMI…),
// les préférences de l'app, et les images des ordinateurs sur l'accueil.

import AVFoundation
import KyberKit
import SwiftUI
import UIKit

// MARK: - Écran

enum DeviceDisplay {
    /// iPad (orientation libre en session, statistiques en panneau) ou iPhone.
    static var isPad: Bool { UIDevice.current.userInterfaceIdiom == .pad }

    /// Pixels réels de l'écran, en paysage, et sa fréquence la plus haute.
    static func native(_ screen: UIScreen = .main) -> (width: Int, height: Int, refresh: Int) {
        let px = screen.nativeBounds.size
        let w = Int(max(px.width, px.height)), h = Int(min(px.width, px.height))
        let hz = max(30, screen.maximumFramesPerSecond)
        guard w > 0, h > 0 else { return (1920, 1080, 60) }
        return (w / 2 * 2, h / 2 * 2, hz)
    }

    /// L'écran de l'invité pour « Auto » (KyberNow Share) : celui de la fenêtre
    /// de l'app, ses pixels réels et sa fréquence.
    static func shareScreen() -> ShareScreen {
        let screen = UIApplication.shared.connectedScenes
            .compactMap { $0 as? UIWindowScene }
            .first { $0.session.role == .windowApplication }?.screen ?? UIScreen.main
        let n = native(screen)
        return ShareScreen(pixelWidth: n.width, pixelHeight: n.height, refresh: n.refresh)
    }

    /// Mode de la session selon les réglages de l'ordinateur : « comme cet
    /// écran » (au plus 4K si demandé), ou une définition choisie.
    static func mode(for settings: ConnectionSettings, screen: UIScreen = .main) -> (width: Int, height: Int, refresh: Int) {
        let n = native(screen)
        if settings.display == .custom {
            return (settings.customWidth / 2 * 2, settings.customHeight / 2 * 2, min(n.refresh, settings.customRefresh))
        }
        if settings.limit4K, n.width > 3840 {
            let h = n.height * 3840 / n.width
            return (3840, h / 2 * 2, n.refresh)
        }
        return n
    }

    static func label(_ w: Int, _ h: Int) -> String {
        switch (w, h) {
        case (3840, 2160): return "4K"
        case (2560, 1440): return "1440p"
        case (1920, 1080): return "1080p"
        default: return "\(w) × \(h)"
        }
    }
}

// MARK: - Son

/// Les noms de la sortie du son sur l'iPhone et l'iPad (`AudioRoute`, AudioRoute.swift).
extension AudioRoute {
    static func hdmiName(_ port: AVAudioSessionPortDescription?, channels: Int) -> String { port?.portName ?? "HDMI" }
    static func builtInName(_ port: AVAudioSessionPortDescription?) -> String { String(localized: "haut-parleur") }
    static func builtInChannels(_ channels: Int) -> Int { 2 }
}

// MARK: - Préférences de l'app (sur cet appareil)

/// Celles de l'iPhone et de l'iPad, en plus des communes (`AppPreferences`).
final class Preferences: AppPreferences {
    static let shared = Preferences()

    /// « Disposition du clavier » (Réglages de l'app › Clavier) : nomme les
    /// touches d'une partie sur console (`Keymap.character(hid:)`).
    @Published var keyboardLayout: KeyboardLayout {
        didSet {
            d.set(keyboardLayout.rawValue, forKey: "keyboardLayout")
            Keymap.layout = keyboardLayout
        }
    }
    /// Pavé tactile virtuel (le doigt déplace le pointeur) ou toucher direct (le doigt est le pointeur).
    @Published var directTouch: Bool { didSet { d.set(directTouch, forKey: "directTouch") } }
    /// « Sessions en données mobiles » (Réglages de l'app) : coupé, l'accueil
    /// demande avant d'ouvrir une session en données mobiles (`AppModel.connect`).
    @Published var allowCellular: Bool { didSet { d.set(allowCellular, forKey: "allowCellular") } }
    /// Vitesse du pointeur au pavé tactile et à la manette.
    @Published var pointerSpeed: Double { didSet { d.set(pointerSpeed, forKey: "pointerSpeed") } }

    private override init() {
        let layout = UserDefaults.standard.string(forKey: "keyboardLayout").flatMap(KeyboardLayout.init(rawValue:)) ?? .auto
        keyboardLayout = layout
        directTouch = Self.flag("directTouch", false)
        allowCellular = Self.flag("allowCellular", false)
        pointerSpeed = UserDefaults.standard.object(forKey: "pointerSpeed") as? Double ?? 1.4
        super.init()
        Keymap.layout = layout
    }
}

// MARK: - Images des ordinateurs

extension HostImagery {
    static var folder: URL {
        FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask)[0].appendingPathComponent("Images")
    }

    /// La largeur de l'aperçu demandé à Admin.
    static let previewWidth = 1600
}
