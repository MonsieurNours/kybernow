// Une session avec un ordinateur, sur l'Apple TV, vue de l'interface : le
// passage et les étapes de la connexion, le menu (bouton Retour ou L1 + R1 +
// Start + Select) et ses mesures, les avis, la fréquence du téléviseur. Le déroulé lui-même
// (préparation par Admin : écran virtuel à la taille du téléviseur, 4K
// à 60 Hz au plus, son au format de la sortie ; connexion à Kyber, choix de
// l'écran, relances, reconnexions, repli quand l'image ne passe pas, signal
// de vie, fin vérifiée) est dans le cœur Rust, par `SessionEngine`, comme
// sur Mac (docs/coeur-apple.md).

import AVFoundation
import AVKit
import Combine
import CoreMedia
import KyberKit
import SwiftUI
import UIKit

@MainActor
final class SessionController: ObservableObject {
    enum Phase: Equatable {
        case preparing, connecting, starting, streaming
        case reconnecting(Int)
        case ending
        case ended
        case failed(String)
    }

    @Published private(set) var phase: Phase = .preparing
    @Published private(set) var host: Host
    /// Instant où l'image est apparue (le passage s'ouvre).
    @Published private(set) var revealedAt: Date?
    /// Le passage (les deux moitiés de l'accueil) est dans la hiérarchie :
    /// jusqu'à la fin de son ouverture, puis retiré pour laisser la couche
    /// Metal seule à l'écran (voir SessionView).
    @Published private(set) var passageShown = true
    /// Dernier recours de la connexion : le cœur redémarre la Station et
    /// attend son retour (« La Station redémarre… ») ; jusqu'à la phase suivante.
    @Published private(set) var stationRestarting = false
    @Published private(set) var menuOpen = false
    @Published private(set) var stats: VideoStats?
    @Published private(set) var audioStats: AudioStats?
    @Published private(set) var route = AudioRoute.current
    @Published private(set) var modeLabel = ""
    @Published private(set) var remoteMouse = false
    /// La manette sert de souris et de clavier (actif d'office en profil Bureau).
    @Published private(set) var padMouse = false
    /// Rappel du menu, en haut de l'écran, à la fermeture du menu.
    @Published private(set) var menuHint = false
    @Published private(set) var startedAt = Date()
    @Published private(set) var fallbackNote: String?
    /// Capsule d'information en haut de l'écran (mode de l'ordinateur changé…).
    @Published private(set) var notice: String?
    /// Profil Bureau, écran de l'ordinateur passé à un autre mode : « Remettre
    /// 3840 × 2160 » dans le menu (le cœur le propose et le retire).
    @Published private(set) var modeRestoreOffer: String?

    let streamView = StreamView(frame: CGRect(x: 0, y: 0, width: 1920, height: 1080))
    private let store: HostStore
    private let onFinish: (SessionController) -> Void
    private let prefs = Preferences.shared
    private var engine: SessionEngine?
    private var engineStreaming = false
    private var streamMode = (width: 0, height: 0, refresh: 0)
    private var routeObserver: NSObjectProtocol?
    private static let revealDelay: TimeInterval = 0.9
    /// Repli quand l'image ne passe pas : un débit plus bas que sur Mac.
    private static let fallbackBitrate: UInt32 = 150_000_000

    init(host: Host, store: HostStore, onFinish: @escaping (SessionController) -> Void) {
        self.host = host
        self.store = store
        self.onFinish = onFinish
        remoteMouse = host.settings.profile == .bureau || host.settings.mouse == .desktop
        padMouse = host.settings.profile == .bureau
    }

    /// Les réglages de l'image, pour le cœur. L'Apple TV ne lit pas l'AV1 en
    /// matériel : HEVC (ou H.264).
    private var engineSettings: SessionEngine.Settings {
        let s = host.settings
        var codec = s.codec
        if codec == .av1 { codec = DecoderCapabilities.current.hevc ? .hevc : .h264 }
        return SessionEngine.Settings(
            profile: s.profile?.rawValue,
            codec: codec,
            yuv444: s.yuv444 && ConnectionSettings.decodes444(codec),
            mbps: s.bitrateMbps,
            reliable: s.transport == .reliable
        )
    }

    // MARK: - Déroulé

    func start() {
        guard let password = store.password(for: host) else {
            fail(String(localized: "mot de passe introuvable dans le Trousseau"))
            return
        }
        let settings = host.settings
        let mode = TVDisplay.mode(for: settings)
        streamMode = mode
        modeLabel = String(localized: "\(mode.width) × \(mode.height) à \(mode.refresh) Hz")
        matchFrameRate(mode.refresh)
        let gamepads = GamepadBridge.shared
        gamepads.onMenuRequest = { [weak self] in self?.menuRequested() }
        gamepads.menuFromRemote = prefs.menuBack
        gamepads.menuFromCombo = prefs.menuCombo
        gamepads.rumbleEnabled = prefs.rumble
        gamepads.padMouseSpeed = prefs.padMouseSpeed.factor
        streamView.padMouse = padMouse
        streamView.remoteMouse = remoteMouse
        streamView.appleISOKeyboard = prefs.appleISOKeyboard
        routeObserver = NotificationCenter.default.addObserver(forName: AVAudioSession.routeChangeNotification, object: nil, queue: .main) { [weak self] _ in
            Task { @MainActor in self?.route = AudioRoute.current }
        }

        let engine = SessionEngine(view: streamView)
        engine.onPhase = { [weak self] in self?.phaseChanged($0) }
        engine.onNotice = { [weak self] in self?.notice($0) }
        engine.onMode = { [weak self] mode, _ in
            guard let self else { return }
            self.streamMode = (mode.width, mode.height, mode.refresh)
            self.modeLabel = String(localized: "\(mode.width) × \(mode.height) à \(mode.refresh) Hz")
        }
        engine.onVideoStats = { [weak self] in self?.stats = $0 }
        engine.onAudioStats = { [weak self] in self?.audioStats = $0 }
        engine.onGameFullscreen = { game in
            KyberLog.app.notice("ordinateur : \(game ? "jeu en plein écran" : "pas de jeu en plein écran", privacy: .public)")
        }
        self.engine = engine

        // Son : celui de la sortie (ampli 5.1 ou 7.1, sinon stéréo), ou resté sur l'ordinateur.
        let channels = settings.audio == .off ? 0 : settings.audio.channels
        var params = SessionEngine.Params(
            host: host.address, consolePort: host.consolePort, username: host.username, password: password,
            consoleFingerprint: host.consoleFingerprint,
            mode: SessionEngine.Mode(width: mode.width, height: mode.height, refresh: mode.refresh),
            audioChannels: channels, virtualDisplay: settings.display != .physical, settings: engineSettings
        )
        params.autoReconnect = settings.autoReconnect
        params.gamepadAsXbox360 = settings.gamepad == .xbox360
        params.clipboard = false
        params.vsync = true
        params.fallbackBitrate = Self.fallbackBitrate
        KyberLog.app.notice("session : \(mode.width)×\(mode.height) à \(mode.refresh) Hz, son \(channels) canaux, \(settings.consoleProfile, privacy: .public)")
        do {
            try engine.start(params)
        } catch {
            fail(error.localizedDescription)
        }
    }

    /// Le téléviseur passe à la fréquence de la session (réglage « Ajuster la
    /// fréquence » de tvOS), puis revient à la fin.
    private func matchFrameRate(_ hz: Int?) {
        guard prefs.matchFrameRate,
              let window = UIApplication.shared.connectedScenes.compactMap({ ($0 as? UIWindowScene)?.keyWindow }).first
        else { return }
        guard let hz else {
            window.avDisplayManager.preferredDisplayCriteria = nil
            return
        }
        // Image SDR à la définition de la session : le téléviseur garde sa
        // définition, seule la fréquence suit.
        var format: CMVideoFormatDescription?
        CMVideoFormatDescriptionCreate(allocator: nil, codecType: kCMVideoCodecType_HEVC, width: Int32(streamMode.width), height: Int32(streamMode.height), extensions: nil, formatDescriptionOut: &format)
        if let format {
            window.avDisplayManager.preferredDisplayCriteria = AVDisplayCriteria(refreshRate: Float(hz), formatDescription: format)
        }
    }

    /// Les états du cœur : l'image n'apparaît qu'après `revealDelay`, seule
    /// (aucune capsule à l'arrivée : les mesures sont dans le menu).
    private func phaseChanged(_ p: SessionEngine.Phase) {
        if case .failed = phase { return }
        engineStreaming = p == .streaming
        if stationRestarting, p != .connecting { stationRestarting = false }
        switch p {
        case .preparing:
            phase = .preparing
        case .connecting:
            phase = .connecting
        case .starting:
            if revealedAt == nil { phase = .starting }
        case .streaming:
            guard revealedAt == nil else {
                phase = .streaming
                return
            }
            phase = .starting
            DispatchQueue.main.asyncAfter(deadline: .now() + Self.revealDelay) { [weak self] in
                guard let self, self.revealedAt == nil, self.engineStreaming, self.engine != nil else { return }
                withAnimation(.timingCurve(0.2, 0.9, 0.1, 1, duration: 1.5)) {
                    self.revealedAt = Date()
                    self.phase = .streaming
                }
                self.startedAt = Date()
                self.dismissPassage()
            }
        case let .reconnecting(attempt, _):
            phase = .reconnecting(attempt)
        case .ending:
            // Fin demandée par Admin : comme une fin voulue.
            if phase != .ending, phase != .ended { end() }
        case let .ended(error, _):
            if let error, phase != .ending, phase != .ended {
                fail(error)
                return
            }
            if phase != .ending, phase != .ended { end() }
        }
    }

    /// Le passage a fini de s'ouvrir (1,5 s) : il quitte la hiérarchie.
    private func dismissPassage() {
        DispatchQueue.main.asyncAfter(deadline: .now() + 1.7) { [weak self] in
            guard let self, self.revealedAt != nil else { return }
            self.passageShown = false
        }
    }

    /// Les avis du cœur, en capsules.
    private func notice(_ n: SessionEngine.Notice) {
        switch n {
        case .relaunched:
            show(notice: String(localized: "L'image a été relancée"), for: 4)
        case let .modeChanged(width, height, back):
            streamMode = (width, height, streamMode.refresh)
            modeLabel = String(localized: "\(width) × \(height) à \(streamMode.refresh) Hz")
            var text = back ? String(localized: "L'ordinateur est revenu en \(width) × \(height)") : String(localized: "L'ordinateur est passé en \(width) × \(height)")
            if let offer = modeRestoreOffer { text += String(localized: " · « \(offer) » dans le menu") }
            show(notice: text)
        case let .fallback(level, mbps):
            fallbackNote = level == 1 ? String(localized: "L'image ne passait pas : débit ramené à \(mbps) Mbit/s pour cette session.") : String(localized: "L'image ne passait pas : 4:2:0 pour cette session.")
            DispatchQueue.main.asyncAfter(deadline: .now() + 6) { [weak self] in self?.fallbackNote = nil }
        case .stationRestarting:
            stationRestarting = true
        case .adapting, .adapted, .adaptFailed:
            // Pas d'adaptation à l'écran sur l'Apple TV : le téléviseur ne change pas en session.
            break
        case let .restoreOffer(mode):
            modeRestoreOffer = mode.map { String(localized: "Remettre \($0.width) × \($0.height)") }
        }
    }

    /// Capsule en haut de l'écran pendant quelques secondes.
    private func show(notice text: String, for seconds: TimeInterval = 5) {
        withAnimation(.easeOut(duration: 0.35)) { notice = text }
        DispatchQueue.main.asyncAfter(deadline: .now() + seconds) { [weak self] in
            guard self?.notice == text else { return }
            withAnimation(.easeIn(duration: 0.35)) { self?.notice = nil }
        }
    }

    /// « Remettre » : Admin remet le mode demandé au début de la session.
    func restoreMode() {
        modeRestoreOffer = nil
        engine?.restoreMode()
    }

    // MARK: - Menu

    /// Bouton Retour de la télécommande, ou combinaison à la manette.
    private func menuRequested() {
        guard phase == .streaming else { return }
        if menuOpen {
            closeMenu()
        } else {
            openMenu()
        }
    }

    func openMenu() {
        guard !menuOpen else { return }
        GamepadBridge.shared.setCaptured(true)
        withAnimation(.timingCurve(0.2, 0.9, 0.1, 1, duration: 0.5)) { menuOpen = true }
        route = AudioRoute.current
    }

    func closeMenu() {
        guard menuOpen else { return }
        withAnimation(.timingCurve(0.2, 0.9, 0.1, 1, duration: 0.5)) { menuOpen = false }
        GamepadBridge.shared.setCaptured(false)
        withAnimation(.easeOut(duration: 0.35)) { menuHint = true }
        DispatchQueue.main.asyncAfter(deadline: .now() + 4) { [weak self] in
            withAnimation(.easeIn(duration: 0.35)) { self?.menuHint = false }
        }
    }

    func togglePadMouse() {
        padMouse.toggle()
        streamView.padMouse = padMouse
    }

    func toggleRemoteMouse() {
        remoteMouse.toggle()
        streamView.remoteMouse = remoteMouse
    }

    func type(_ text: String) {
        streamView.type(text)
    }

    /// Profil changé dans le menu : le cœur change le débit en direct, ou
    /// relance l'image pour le 4:4:4, le format ou le transport.
    func select(_ profile: Profile) {
        guard profile != host.settings.profile else { return }
        host.settings.apply(profile)
        store.update(host)
        engine?.setSettings(engineSettings)
    }

    var sessionMinutes: Int { Int(Date().timeIntervalSince(startedAt) / 60) }

    #if DEBUG
    enum Demo { case connecting, restarting, passage, menu }

    /// Démonstration (simulateur), sans ordinateur : les étapes de la
    /// connexion (arrêtées sur « Connexion »), l'image seule, ou le menu.
    func demo(_ kind: Demo) {
        phase = .preparing
        streamView.startTestPattern()
        if kind == .connecting || kind == .restarting {
            DispatchQueue.main.asyncAfter(deadline: .now() + 1) { [weak self] in
                self?.phase = .connecting
                if kind == .restarting { self?.stationRestarting = true }
            }
            return
        }
        DispatchQueue.main.asyncAfter(deadline: .now() + 1.6) { [weak self] in
            guard let self else { return }
            withAnimation(.timingCurve(0.2, 0.9, 0.1, 1, duration: 1.5)) {
                self.revealedAt = Date()
                self.phase = .streaming
            }
            self.dismissPassage()
            self.startedAt = Date().addingTimeInterval(-42 * 60)
            self.modeLabel = String(localized: "\(3840) × \(2160) à \(60) Hz")
            var stats = VideoStats()
            stats.latencyMs = 9.1
            stats.fps = 60
            stats.mbps = 78
            stats.format = "4:2:0 8 bits"
            self.stats = stats
            if kind == .menu {
                DispatchQueue.main.asyncAfter(deadline: .now() + 0.5) { self.openMenu() }
            }
        }
    }
    #endif

    // MARK: - Fin

    /// Fin de session : le cœur rend la main à Admin (écrans et son de
    /// l'ordinateur rétablis, latence de la session donnée à ses appareils).
    /// Une session d'au moins 3 s devient la « Dernière session » des Statistiques.
    func end() {
        guard phase != .ending && phase != .ended else { return }
        GamepadBridge.shared.onMenuRequest = nil
        streamView.padMouse = false
        if let last = engine?.snapshot(maxWidth: 1920) { store.imagery.saveLast(last, for: host) }
        if let revealedAt {
            let m = streamMode
            host.recordSession(start: revealedAt, latency: engine?.sessionLatency, width: m.width, height: m.height, refresh: m.refresh,
                               codec: engineSettings.codec, mbps: host.settings.bitrate(width: m.width, height: m.height, refresh: m.refresh))
        }
        store.update(host)
        withAnimation(.easeInOut(duration: 0.4)) {
            phase = .ending
            menuOpen = false
        }
        streamView.detach()
        engine?.stop()
        matchFrameRate(nil)
        if let routeObserver { NotificationCenter.default.removeObserver(routeObserver) }
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.6) { [weak self] in self?.finish() }
    }

    func finish() {
        guard phase != .ended else { return }
        phase = .ended
        onFinish(self)
    }

    private func fail(_ message: String) {
        KyberLog.app.error("session : \(message, privacy: .public)")
        GamepadBridge.shared.onMenuRequest = nil
        streamView.detach()
        engine?.stop()
        matchFrameRate(nil)
        withAnimation(Theme.easeOut) {
            menuOpen = false
            phase = .failed(message)
        }
    }
}
