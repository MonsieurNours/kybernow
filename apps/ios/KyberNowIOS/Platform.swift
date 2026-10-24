// Ce que l'iPhone ou l'iPad sait de lui-même : son écran (définition réelle
// et fréquence, ProMotion), la sortie du son (haut-parleur, AirPods, HDMI…),
// les préférences de l'app, et les images des ordinateurs sur l'accueil.

import AVFoundation
import KyberKit
import SwiftUI
import UIKit

// MARK: - Écran

enum DeviceDisplay {
    /// iPad (orientation libre en session, colonne « Prêt à jouer », mesures
    /// en panneau) ou iPhone.
    static var isPad: Bool { UIDevice.current.userInterfaceIdiom == .pad }

    /// Pixels réels de l'écran, en paysage, et sa fréquence la plus haute.
    static func native(_ screen: UIScreen = .main) -> (width: Int, height: Int, refresh: Int) {
        let px = screen.nativeBounds.size
        let w = Int(max(px.width, px.height)), h = Int(min(px.width, px.height))
        let hz = max(30, screen.maximumFramesPerSecond)
        guard w > 0, h > 0 else { return (1920, 1080, 60) }
        return (w / 2 * 2, h / 2 * 2, hz)
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

enum AudioOutput {
    /// Voies de la sortie actuelle (ampli HDMI 5.1 ou 7.1, AirPods en audio spatial, sinon stéréo).
    static func channels() -> Int {
        try? AVAudioSession.sharedInstance().setSupportsMultichannelContent(Preferences.shared.spatialAudio)
        let n = AVAudioSession.sharedInstance().maximumOutputNumberOfChannels
        return n >= 8 ? 8 : n >= 6 ? 6 : 2
    }
}

/// Sortie du son vue par l'appareil, et ce qu'elle ajoute de retard.
struct AudioRoute: Equatable {
    enum Kind { case hdmi, airpods, bluetooth, airplay, builtIn, headphones }
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
        case .HDMI?: return AudioRoute(kind: .hdmi, name: port?.portName ?? "HDMI", channels: channels, latencyMs: latency)
        case .bluetoothA2DP?, .bluetoothLE?:
            let name = port?.portName ?? "Bluetooth"
            return AudioRoute(kind: name.localizedCaseInsensitiveContains("AirPods") ? .airpods : .bluetooth, name: name, channels: channels, latencyMs: latency)
        case .airPlay?: return AudioRoute(kind: .airplay, name: port?.portName ?? "AirPlay", channels: 2, latencyMs: latency)
        case .headphones?, .usbAudio?: return AudioRoute(kind: .headphones, name: port?.portName ?? String(localized: "casque"), channels: 2, latencyMs: latency)
        default: return AudioRoute(kind: .builtIn, name: String(localized: "haut-parleur"), channels: 2, latencyMs: latency)
        }
    }

    /// AirPods, AirPlay, Bluetooth : le son arrive nettement après l'image.
    var isLate: Bool { kind != .hdmi && kind != .builtIn && kind != .headphones && latencyMs > 40 }

    var channelsLabel: String { channels >= 8 ? "7.1" : channels >= 6 ? "5.1" : String(localized: "Stéréo") }
}

// MARK: - Préférences de l'app (sur cet appareil)

final class Preferences: ObservableObject {
    static let shared = Preferences()
    private let d = UserDefaults.standard

    /// L1 + R1 + Start + Select ouvre la barre.
    @Published var menuCombo: Bool { didSet { d.set(menuCombo, forKey: "menuCombo") } }
    @Published var rumble: Bool { didSet { d.set(rumble, forKey: "rumble") } }
    @Published var spatialAudio: Bool { didSet { d.set(spatialAudio, forKey: "spatialAudio") } }
    @Published var latencyAlert: Bool { didSet { d.set(latencyAlert, forKey: "latencyAlert") } }
    @Published var icloudSync: Bool { didSet { d.set(icloudSync, forKey: "icloudSync") } }
    /// Clavier Apple ISO (européen) branché : touches `@#` et `<>` échangées.
    @Published var appleISOKeyboard: Bool { didSet { d.set(appleISOKeyboard, forKey: "appleISOKeyboard") } }
    /// Pavé tactile virtuel (le doigt déplace le pointeur) ou toucher direct (le doigt est le pointeur).
    @Published var directTouch: Bool { didSet { d.set(directTouch, forKey: "directTouch") } }
    /// « Sessions en données mobiles » (aussi « Autoriser les données
    /// mobiles » dans les réglages d'un ordinateur) : coupé, l'accueil demande
    /// avant d'ouvrir une session en données mobiles (`AppModel.connect`).
    @Published var allowCellular: Bool { didSet { d.set(allowCellular, forKey: "allowCellular") } }
    /// Vitesse du pointeur au pavé tactile et à la manette.
    @Published var pointerSpeed: Double { didSet { d.set(pointerSpeed, forKey: "pointerSpeed") } }

    private static func flag(_ key: String, _ fallback: Bool = true) -> Bool {
        UserDefaults.standard.object(forKey: key) as? Bool ?? fallback
    }

    private init() {
        menuCombo = Self.flag("menuCombo")
        rumble = Self.flag("rumble")
        spatialAudio = Self.flag("spatialAudio")
        latencyAlert = Self.flag("latencyAlert")
        icloudSync = Self.flag("icloudSync")
        appleISOKeyboard = Self.flag("appleISOKeyboard", false)
        directTouch = Self.flag("directTouch", false)
        allowCellular = Self.flag("allowCellular", false)
        pointerSpeed = UserDefaults.standard.object(forKey: "pointerSpeed") as? Double ?? 1.4
    }
}

// MARK: - Images des ordinateurs

/// L'image d'un ordinateur sur l'accueil : la dernière image de sa dernière
/// session, un aperçu demandé à Admin, sinon sa signature (Backdrops).
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
        folder = FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask)[0].appendingPathComponent("Images")
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

    private func url(_ id: UUID, _ source: Source) -> URL {
        folder.appendingPathComponent("\(id.uuidString)-\(source == .lastSession ? "last" : "preview").jpg")
    }

    func wantsPreview(_ host: Host) -> Bool {
        if pictures[host.id]?.source == .lastSession { return false }
        return Date().timeIntervalSince(lastFetch[host.id] ?? .distantPast) > 60
    }

    func fetchPreview(for host: Host, console: ConsoleClient) async {
        lastFetch[host.id] = Date()
        guard let shot = await console.preview(width: 1600), let image = UIImage(data: shot.jpeg),
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
