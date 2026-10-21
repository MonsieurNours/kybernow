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

enum AudioOutput {
    /// Voies de la sortie actuelle (ampli HDMI 5.1 ou 7.1, sinon stéréo).
    static func channels() -> Int {
        // Avec l'audio spatial, des AirPods acceptent le 5.1 et le 7.1.
        try? AVAudioSession.sharedInstance().setSupportsMultichannelContent(Preferences.shared.spatialAudio)
        let n = AVAudioSession.sharedInstance().maximumOutputNumberOfChannels
        return n >= 8 ? 8 : n >= 6 ? 6 : 2
    }
}

/// Sortie du son vue par l'Apple TV, et ce qu'elle ajoute de retard.
struct AudioRoute: Equatable {
    enum Kind { case hdmi, airpods, bluetooth, airplay, builtIn }
    var kind: Kind
    var name: String
    var channels: Int
    var latencyMs: Int

    static var current: AudioRoute {
        let session = AVAudioSession.sharedInstance()
        let port = session.currentRoute.outputs.first
        let latency = Int(((session.outputLatency + session.ioBufferDuration) * 1000).rounded())
        let channels = AudioOutput.channels()
        switch port?.portType {
        case .HDMI?: return AudioRoute(kind: .hdmi, name: channels > 2 ? String(localized: "ampli HDMI") : String(localized: "téléviseur"), channels: channels, latencyMs: latency)
        case .bluetoothA2DP?, .bluetoothLE?:
            let name = port?.portName ?? "Bluetooth"
            return AudioRoute(kind: name.localizedCaseInsensitiveContains("AirPods") ? .airpods : .bluetooth, name: name, channels: 2, latencyMs: latency)
        case .airPlay?: return AudioRoute(kind: .airplay, name: port?.portName ?? "AirPlay", channels: 2, latencyMs: latency)
        default: return AudioRoute(kind: .builtIn, name: port?.portName ?? String(localized: "téléviseur"), channels: channels, latencyMs: latency)
        }
    }

    /// AirPods, HomePod, AirPlay : le son arrive nettement après l'image.
    var isLate: Bool { kind != .hdmi && kind != .builtIn && latencyMs > 40 }

    var channelsLabel: String { channels >= 8 ? "7.1" : channels >= 6 ? "5.1" : String(localized: "Stéréo") }
}

// MARK: - Préférences de l'app (sur cette Apple TV, pour ce profil)

final class Preferences: ObservableObject {
    static let shared = Preferences()
    private let d = UserDefaults.standard

    @Published var menuBack: Bool { didSet { d.set(menuBack, forKey: "menuBack") } }
    @Published var menuCombo: Bool { didSet { d.set(menuCombo, forKey: "menuCombo") } }
    @Published var rumble: Bool { didSet { d.set(rumble, forKey: "rumble") } }
    @Published var padOrderFixed: Bool {
        didSet {
            d.set(padOrderFixed, forKey: "padOrderFixed")
            GamepadBridge.shared.fixedOrder = padOrderFixed
        }
    }
    @Published var spatialAudio: Bool { didSet { d.set(spatialAudio, forKey: "spatialAudio") } }
    @Published var latencyAlert: Bool { didSet { d.set(latencyAlert, forKey: "latencyAlert") } }
    @Published var matchFrameRate: Bool { didSet { d.set(matchFrameRate, forKey: "matchFrameRate") } }
    @Published var icloudSync: Bool { didSet { d.set(icloudSync, forKey: "icloudSync") } }
    /// Clavier Apple ISO (européen) branché à l'Apple TV : touches `@#` et `<>` échangées.
    @Published var appleISOKeyboard: Bool { didSet { d.set(appleISOKeyboard, forKey: "appleISOKeyboard") } }

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

    private static func flag(_ key: String) -> Bool { UserDefaults.standard.object(forKey: key) as? Bool ?? true }

    private init() {
        let flag = Self.flag
        menuBack = flag("menuBack")
        menuCombo = flag("menuCombo")
        rumble = flag("rumble")
        padOrderFixed = UserDefaults.standard.bool(forKey: "padOrderFixed")
        spatialAudio = flag("spatialAudio")
        latencyAlert = flag("latencyAlert")
        matchFrameRate = flag("matchFrameRate")
        icloudSync = flag("icloudSync")
        appleISOKeyboard = UserDefaults.standard.bool(forKey: "appleISOKeyboard")
        padMouseSpeed = PadMouseSpeed(rawValue: UserDefaults.standard.string(forKey: "padMouseSpeed") ?? "") ?? .normal
        guideButton = GuideChoice(rawValue: UserDefaults.standard.string(forKey: "guideButton") ?? "") ?? .game
        GamepadBridge.shared.fixedOrder = padOrderFixed
        GamepadBridge.shared.guideMode = guideButton.mode
    }
}

// MARK: - Images des ordinateurs

/// L'image d'un ordinateur sur l'accueil : la dernière image de sa dernière
/// session, un aperçu demandé à sa console, sinon sa signature. Les images
/// vivent dans le conteneur partagé, pour le Top Shelf.
@MainActor
final class HostImagery: ObservableObject {
    enum Source: Equatable { case lastSession, preview }

    struct Picture {
        let image: UIImage
        let date: Date
        let source: Source
    }

    @Published private(set) var pictures: [UUID: Picture] = [:]
    private var lastFetch: [UUID: Date] = [:]
    private let folder: URL

    init() {
        let base = TopShelfStore.folder
            ?? FileManager.default.urls(for: .cachesDirectory, in: .userDomainMask)[0].appendingPathComponent("TopShelf")
        folder = base
        try? FileManager.default.createDirectory(at: folder, withIntermediateDirectories: true)
        let files = (try? FileManager.default.contentsOfDirectory(at: folder, includingPropertiesForKeys: [.contentModificationDateKey])) ?? []
        for url in files.sorted(by: { $0.lastPathComponent.hasSuffix("-last.jpg") && !$1.lastPathComponent.hasSuffix("-last.jpg") }) {
            guard url.pathExtension == "jpg" else { continue }
            let name = url.deletingPathExtension().lastPathComponent
            guard let dash = name.lastIndex(of: "-"), let id = UUID(uuidString: String(name[..<dash])),
                  pictures[id] == nil, let image = UIImage(contentsOfFile: url.path) else { continue }
            let date = (try? url.resourceValues(forKeys: [.contentModificationDateKey]))?.contentModificationDate ?? Date()
            pictures[id] = Picture(image: image, date: date, source: name.hasSuffix("-last") ? .lastSession : .preview)
        }
    }

    func file(_ id: UUID) -> String? {
        guard let p = pictures[id] else { return nil }
        return "\(id.uuidString)-\(p.source == .lastSession ? "last" : "preview").jpg"
    }

    private func url(_ id: UUID, _ source: Source) -> URL {
        folder.appendingPathComponent("\(id.uuidString)-\(source == .lastSession ? "last" : "preview").jpg")
    }

    func wantsPreview(_ host: Host) -> Bool {
        if pictures[host.id]?.source == .lastSession { return false }
        return Date().timeIntervalSince(lastFetch[host.id] ?? .distantPast) > 60
    }

    func fetchPreview(for host: Host, console: ConsoleClient) async {
        lastFetch[host.id] = Date()
        guard let shot = await console.preview(width: 1920), let image = UIImage(data: shot.jpeg),
              pictures[host.id]?.source != .lastSession
        else { return }
        let file = url(host.id, .preview)
        try? shot.jpeg.write(to: file, options: .atomic)
        try? FileManager.default.setAttributes([.modificationDate: shot.capturedAt], ofItemAtPath: file.path)
        pictures[host.id] = Picture(image: image, date: shot.capturedAt, source: .preview)
    }

    func saveLast(_ cg: CGImage, for host: Host) {
        let image = UIImage(cgImage: cg)
        guard host.settings.keepLastImage, let data = image.jpegData(compressionQuality: 0.82) else { return }
        try? data.write(to: url(host.id, .lastSession), options: .atomic)
        try? FileManager.default.removeItem(at: url(host.id, .preview))
        pictures[host.id] = Picture(image: image, date: Date(), source: .lastSession)
    }

    func forgetLast(_ host: Host) {
        try? FileManager.default.removeItem(at: url(host.id, .lastSession))
        if pictures[host.id]?.source == .lastSession { pictures[host.id] = nil }
        lastFetch[host.id] = nil
    }

    func forget(_ host: Host) {
        try? FileManager.default.removeItem(at: url(host.id, .lastSession))
        try? FileManager.default.removeItem(at: url(host.id, .preview))
        pictures[host.id] = nil
    }
}
