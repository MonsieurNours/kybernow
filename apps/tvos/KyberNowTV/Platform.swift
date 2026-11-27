// Ce que l'Apple TV sait d'elle-même : écran du téléviseur (au plus 4K à
// 60 Hz), sortie du son (ampli HDMI, AirPods, HomePod…), préférences de
// l'app, et images des ordinateurs sur l'accueil.

import AVFoundation
import KyberKit
import SwiftUI
import TVServices
import UIKit

// MARK: - Écran

enum TVDisplay {
    /// Le plus que l'Apple TV affiche : 4K à 60 Hz.
    static let maxWidth = 3840, maxHeight = 2160, maxRefresh = 60

    /// Définition et fréquence du téléviseur, ramenées à 4K60.
    static var native: (width: Int, height: Int, refresh: Int) {
        let screen = UIScreen.main
        let px = screen.nativeBounds.size
        let w = Int(max(px.width, px.height)), h = Int(min(px.width, px.height))
        let hz = min(maxRefresh, max(30, screen.maximumFramesPerSecond))
        guard w > 0, h > 0 else { return (1920, 1080, 60) }
        if w > maxWidth { return (maxWidth, maxHeight, hz) }
        return (w / 2 * 2, h / 2 * 2, hz)
    }

    /// Mode de la session selon les réglages de l'ordinateur.
    static func mode(for settings: ConnectionSettings) -> (width: Int, height: Int, refresh: Int) {
        let tv = native
        let hz = settings.display == .custom ? min(maxRefresh, settings.customRefresh) : tv.refresh
        guard settings.display == .custom else { return (tv.width, tv.height, hz) }
        let w = min(maxWidth, settings.customWidth), h = min(maxHeight, settings.customHeight)
        return (w / 2 * 2, h / 2 * 2, hz)
    }

    static func label(_ w: Int, _ h: Int) -> String {
        switch (w, h) {
        case (3840, 2160): return "4K"
        case (2560, 1440): return "1440p"
        case (1920, 1080): return "1080p"
        case (1280, 720): return "720p"
        default: return "\(w) × \(h)"
        }
    }
}

// MARK: - Son

/// Les noms de la sortie du son sur l'Apple TV (`AudioRoute`, AudioRoute.swift).
extension AudioRoute {
    static func hdmiName(_ port: AVAudioSessionPortDescription?, channels: Int) -> String {
        channels > 2 ? String(localized: "ampli HDMI") : String(localized: "téléviseur")
    }
    static func builtInName(_ port: AVAudioSessionPortDescription?) -> String { port?.portName ?? String(localized: "téléviseur") }
    static func builtInChannels(_ channels: Int) -> Int { channels }
}

// MARK: - Préférences de l'app (sur cette Apple TV, pour ce profil)

/// Celles de l'Apple TV, en plus des communes (`AppPreferences`).
final class Preferences: AppPreferences {
    static let shared = Preferences()

    @Published var menuBack: Bool { didSet { d.set(menuBack, forKey: "menuBack") } }
    @Published var padOrderFixed: Bool {
        didSet {
            d.set(padOrderFixed, forKey: "padOrderFixed")
            GamepadBridge.shared.fixedOrder = padOrderFixed
        }
    }
    @Published var matchFrameRate: Bool { didSet { d.set(matchFrameRate, forKey: "matchFrameRate") } }

    /// Vitesse du pointeur quand la manette sert de souris.
    enum PadMouseSpeed: String, CaseIterable {
        case slow = "Lente", normal = "Normale", fast = "Rapide"
        /// Libellé affiché (la valeur brute, en français, est celle enregistrée).
        var label: String {
            switch self {
            case .slow: return String(localized: "Lente")
            case .normal: return String(localized: "Normale")
            case .fast: return String(localized: "Rapide")
            }
        }
        var factor: Double { self == .slow ? 0.6 : self == .fast ? 1.6 : 1 }
    }
    @Published var padMouseSpeed: PadMouseSpeed {
        didSet {
            d.set(padMouseSpeed.rawValue, forKey: "padMouseSpeed")
            GamepadBridge.shared.padMouseSpeed = padMouseSpeed.factor
        }
    }

    /// Bouton Guide de la manette : tvOS, le jeu (si tvOS le laisse), ou
    /// appui long sur Menu ou Options.
    enum GuideChoice: String, CaseIterable {
        case system = "Système", game = "Jeu", longPress = "Appui long"
        /// Libellé affiché (la valeur brute, en français, est celle enregistrée).
        var label: String {
            switch self {
            case .system: return String(localized: "Système")
            case .game: return String(localized: "Jeu")
            case .longPress: return String(localized: "Appui long")
            }
        }
        var mode: GamepadBridge.GuideMode { self == .system ? .system : self == .longPress ? .longPress : .game }
    }
    @Published var guideButton: GuideChoice {
        didSet {
            d.set(guideButton.rawValue, forKey: "guideButton")
            GamepadBridge.shared.guideMode = guideButton.mode
        }
    }

    private override init() {
        let flag = { AppPreferences.flag($0) }
        menuBack = flag("menuBack")
        padOrderFixed = UserDefaults.standard.bool(forKey: "padOrderFixed")
        matchFrameRate = flag("matchFrameRate")
        padMouseSpeed = PadMouseSpeed(rawValue: UserDefaults.standard.string(forKey: "padMouseSpeed") ?? "") ?? .normal
        guideButton = GuideChoice(rawValue: UserDefaults.standard.string(forKey: "guideButton") ?? "") ?? .game
        super.init()
        GamepadBridge.shared.fixedOrder = padOrderFixed
        GamepadBridge.shared.guideMode = guideButton.mode
    }
}

// MARK: - Images des ordinateurs

extension HostImagery {
    /// Le conteneur partagé avec le Top Shelf.
    static var folder: URL {
        TopShelfStore.folder ?? FileManager.default.urls(for: .cachesDirectory, in: .userDomainMask)[0].appendingPathComponent("TopShelf")
    }

    /// La largeur de l'aperçu demandé à Admin : celle du téléviseur.
    static let previewWidth = 1920
}
