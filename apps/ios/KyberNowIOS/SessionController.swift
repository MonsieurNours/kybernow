// Une session avec un ordinateur, sur l'iPhone ou l'iPad, vue de
// l'interface : le passage, la barre (languette ou glissement depuis le bord
// haut sur iPhone, survol du bord haut au trackpad sur iPad), le clavier et
// sa rangée de touches, le menu Touches, les mesures, les capsules. Le
// déroulé lui-même (préparation par la console, connexion à Kyber, choix de
// l'écran, relances, reconnexions, repli, signal de vie, fin vérifiée) est
// dans le cœur Rust, par `SessionEngine`, comme sur Mac et Apple TV
// (docs/coeur-apple.md, jalon 5).
//
// Écran externe : l'image y passe, à sa définition, et l'appareil devient le
// pupitre (pavé tactile sur toute la surface, barre permanente, mesures,
// Touches et clavier sans jamais couvrir l'image) ; débranché, l'image
// revient sur l'appareil sans couper la session. Stage Manager : la session
// vit dans sa fenêtre ; perdre le premier plan relâche tout sur
// l'ordinateur et ramène le débit, comme l'arrière-plan.

import AVFoundation
import Combine
import GameController
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
    /// Capsule « Vous entrez dans… », quelques secondes après l'apparition.
    @Published private(set) var entering = false
    /// Barre de session ouverte.
    @Published private(set) var barOpen = false
    /// La manette pilote la barre (L1 + R1 + Start + Select).
    @Published private(set) var padMode = false
    @Published private(set) var stats: VideoStats?
    @Published private(set) var audioStats: AudioStats?
    /// Latences des dernières secondes (courbe de la barre).
    @Published private(set) var latencyHistory: [Double] = []
    /// Mesures des 30 dernières secondes (feuille des mesures).
    @Published private(set) var statsHistory: [VideoStats] = []
    /// Pertes depuis le début de la session.
    @Published private(set) var sessionLosses = (frames: 0, packets: 0)
    @Published private(set) var route = AudioRoute.current
    @Published private(set) var modeLabel = ""
    @Published private(set) var startedAt = Date()
    /// Capsule d'information en haut de l'image (mode changé, image relancée…).
    @Published private(set) var notice: String?
    @Published private(set) var fallbackNote: String?
    /// Profil Bureau, écran de l'ordinateur passé à un autre mode : « Remettre 2622 × 1206 ».
    @Published private(set) var modeRestoreOffer: String?
    /// Du démarrage du flux à la première image affichée, en secondes.
    @Published private(set) var firstImageDelay: Double?
    /// Clavier de l'écran ouvert.
    @Published private(set) var keyboardShown = false
    /// Ctrl, Alt, ⊞ collants allumés (rangée de touches).
    @Published private(set) var sticky: Set<UInt16> = []
    /// Feuilles : menu Touches, mesures détaillées.
    @Published var keysMenu = false
    @Published var statsSheet = false
    /// Écrans de l'ordinateur (choix dans la barre, sans écran virtuel).
    @Published private(set) var screens: [KyberDisplay] = []
    @Published private(set) var shownDisplay: UInt32?
    @Published private(set) var virtualDisplay = true
    /// Ce que dit la capsule de l'entrée (écran, son, manette).
    @Published private(set) var gamepadName: String?
    /// L'image est sur l'écran externe : l'appareil est le pupitre.
    @Published private(set) var external = false

    let streamView = StreamView(frame: CGRect(x: 0, y: 0, width: 874, height: 402))
    private let store: HostStore
    private let onFinish: (SessionController) -> Void
    private let prefs = Preferences.shared
    private var engine: SessionEngine?
    private var engineStreaming = false
    private var streamStartedAt: Date?
    private var observers: [NSObjectProtocol] = []
    private var externalWatch: AnyCancellable?
    private static let revealDelay: TimeInterval = 0.9
    /// Arrière-plan : débit plafonné, image figée (la session continue).
    private static let backgroundBitrate: UInt32 = 8_000_000

    init(host: Host, store: HostStore, onFinish: @escaping (SessionController) -> Void) {
        self.host = host
        self.store = store
        self.onFinish = onFinish
    }

    /// Les réglages de l'image, pour le cœur. AV1 seulement si l'appareil le décode.
    private var engineSettings: SessionEngine.Settings {
        let s = host.settings
        var codec = s.codec
        if codec == .av1, !DecoderCapabilities.current.av1 { codec = DecoderCapabilities.current.hevc ? .hevc : .h264 }
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
        // « Comme cet écran » : l'écran externe s'il est branché, sinon celui de l'appareil.
        let mode = ExternalDisplay.shared.mode(for: settings) ?? DeviceDisplay.mode(for: settings, screen: deviceScreen)
        modeLabel = "\(mode.width) × \(mode.height) · \(mode.refresh) Hz"
        configureView()
        observeApp()
        if ExternalDisplay.shared.scene != nil { useExternal(true, adapt: false) }
        externalWatch = ExternalDisplay.shared.$scene.dropFirst().sink { [weak self] scene in
            self?.useExternal(scene != nil, adapt: true)
        }

        let engine = SessionEngine(view: streamView)
        engine.onPhase = { [weak self] in self?.phaseChanged($0) }
        engine.onNotice = { [weak self] in self?.notice($0) }
        engine.onDisplays = { [weak self] in self?.screens = $0 }
        engine.onShownDisplay = { [weak self] in self?.shownDisplay = $0.id }
        engine.onMode = { [weak self] mode, virtualDisplay in
            guard let self else { return }
            self.modeLabel = "\(mode.width) × \(mode.height) · \(mode.refresh) Hz"
            self.virtualDisplay = virtualDisplay
        }
        engine.onVideoStats = { [weak self] in self?.received($0) }
        engine.onAudioStats = { [weak self] in self?.audioStats = $0 }
        engine.onGamepadCombo = { [weak self] in self?.gamepadCombo() }
        engine.onGameFullscreen = { game in
            KyberLog.app.notice("ordinateur : \(game ? "jeu en plein écran" : "pas de jeu en plein écran", privacy: .public)")
        }
        self.engine = engine

        // Son : celui de la sortie (AirPods en audio spatial, ampli HDMI), ou resté sur l'ordinateur.
        let channels = settings.audio == .off ? 0 : (settings.audio == .auto ? AudioOutput.channels() : settings.audio.channels)
        var params = SessionEngine.Params(
            host: host.address, consolePort: host.consolePort, username: host.username, password: password,
            consoleFingerprint: host.consoleFingerprint,
            mode: SessionEngine.Mode(width: mode.width, height: mode.height, refresh: mode.refresh),
            audioChannels: channels, virtualDisplay: settings.display != .physical, settings: engineSettings
        )
        params.autoReconnect = settings.autoReconnect
        params.gamepadAsXbox360 = settings.gamepad == .xbox360
        params.clipboard = settings.clipboard
        params.vsync = settings.vsync
        KyberLog.app.notice("session : \(mode.width)×\(mode.height) à \(mode.refresh) Hz, son \(channels) canaux, \(settings.consoleProfile, privacy: .public)")
        do {
            try engine.start(params)
        } catch {
            fail(error.localizedDescription)
        }
    }

    /// L'écran de l'appareil (celui de la fenêtre de l'app).
    private var deviceScreen: UIScreen {
        UIApplication.shared.connectedScenes
            .compactMap { $0 as? UIWindowScene }
            .first { $0.session.role == .windowApplication }?.screen ?? UIScreen.main
    }

    private var deviceName: String {
        DeviceDisplay.isPad ? String(localized: "l'iPad") : String(localized: "l'iPhone")
    }

    /// L'image passe sur l'écran externe, ou revient sur l'appareil. En cours
    /// de session (`adapt`), la console refait l'écran virtuel à la taille du
    /// nouvel écran, puis l'image repart dessus (avis « adaptation »).
    private func useExternal(_ on: Bool, adapt: Bool) {
        guard on != external, phase != .ending, phase != .ended else { return }
        let display = ExternalDisplay.shared
        external = on
        streamView.showImage(in: on ? display.imageHost : nil)
        display.showingImage(on)
        if on {
            setBar(true)
            if DeviceDisplay.isPad { statsSheet = true }
        } else {
            statsSheet = false
        }
        guard adapt else { return }
        show(notice: on ? String(localized: "L'image passe sur l'écran externe") : String(localized: "L'image est revenue sur \(deviceName)"))
        let mode = on ? display.mode(for: host.settings) : DeviceDisplay.mode(for: host.settings, screen: deviceScreen)
        if let mode, virtualDisplay, host.settings.display != .custom {
            engine?.setMode(SessionEngine.Mode(width: mode.width, height: mode.height, refresh: mode.refresh))
        }
    }

    private func configureView() {
        streamView.directTouch = prefs.directTouch
        streamView.pointerSpeed = prefs.pointerSpeed
        streamView.appleISOKeyboard = prefs.appleISOKeyboard
        streamView.mouseMode = host.settings.mouse.streamMode
        streamView.onTap = { [weak self] in self?.setBar(false) }
        streamView.onTopEdgeSwipe = { [weak self] in self?.setBar(true) }
        streamView.onStickyChanged = { [weak self] in self?.sticky = $0 }
        let gamepads = GamepadBridge.shared
        gamepads.onMenuRequest = { [weak self] in self?.gamepadCombo() }
        gamepads.menuFromCombo = prefs.menuCombo
        gamepads.rumbleEnabled = prefs.rumble
    }

    /// La fenêtre de la session perd le premier plan (autre app, verrouillage,
    /// autre fenêtre dans Stage Manager) : rien ne reste enfoncé sur
    /// l'ordinateur, le débit est plafonné, l'image se fige ; au retour, tout
    /// revient. Suivi par scène : avec Stage Manager, l'app peut rester
    /// visible sans avoir le clavier ni le toucher.
    private func observeApp() {
        let center = NotificationCenter.default
        observers = [
            center.addObserver(forName: UIScene.willDeactivateNotification, object: nil, queue: .main) { [weak self] note in
                MainActor.assumeIsolated {
                    guard let self, self.isSessionScene(note.object), self.phase == .streaming else { return }
                    KyberLog.app.notice("fenêtre de la session inactive : image figée")
                    self.streamView.releaseAll()
                    self.engine?.setBitrateCap(Self.backgroundBitrate)
                    // Sur l'écran externe, l'image reste vivante : on la regarde encore.
                    if !self.external { self.engine?.setOccluded(true) }
                    self.show(notice: String(localized: "KyberNow Link continue en arrière-plan"), for: 4)
                }
            },
            center.addObserver(forName: UIScene.didActivateNotification, object: nil, queue: .main) { [weak self] note in
                MainActor.assumeIsolated {
                    guard let self, self.isSessionScene(note.object) else { return }
                    KyberLog.app.notice("fenêtre de la session active : image reprise")
                    self.engine?.setOccluded(false)
                    self.engine?.setBitrateCap(nil)
                }
            },
            center.addObserver(forName: AVAudioSession.routeChangeNotification, object: nil, queue: .main) { [weak self] _ in
                MainActor.assumeIsolated { self?.route = AudioRoute.current }
            },
            center.addObserver(forName: .GCControllerDidConnect, object: nil, queue: .main) { [weak self] note in
                MainActor.assumeIsolated {
                    guard let self, let c = note.object as? GCController, c.extendedGamepad != nil else { return }
                    self.show(notice: String(localized: "\(c.vendorName ?? String(localized: "Manette")) branchée"), for: 3)
                }
            },
        ]
    }

    /// La scène de la fenêtre de la session (pas celle de l'écran externe).
    private func isSessionScene(_ object: Any?) -> Bool {
        guard let scene = object as? UIWindowScene, scene.session.role == .windowApplication else { return false }
        return streamView.window?.windowScene.map { $0 === scene } ?? true
    }

    private func received(_ stats: VideoStats) {
        self.stats = stats
        if stats.fps > 0, firstImageDelay == nil, let t = streamStartedAt {
            firstImageDelay = max(0.1, Date().timeIntervalSince(t) - 0.5)
        }
        if let ms = stats.latencyMs {
            latencyHistory.append(ms)
            if latencyHistory.count > 40 { latencyHistory.removeFirst() }
        }
        statsHistory.append(stats)
        if statsHistory.count > 30 { statsHistory.removeFirst() }
        sessionLosses.frames += stats.lostFrames
        sessionLosses.packets += stats.lostPackets
    }

    /// Les états du cœur : l'image n'apparaît qu'après `revealDelay`.
    private func phaseChanged(_ p: SessionEngine.Phase) {
        if case .failed = phase { return }
        engineStreaming = p == .streaming
        switch p {
        case .preparing:
            phase = .preparing
        case .connecting:
            phase = .connecting
        case .starting:
            if streamStartedAt == nil { streamStartedAt = Date() }
            if revealedAt == nil { phase = .starting }
        case .streaming:
            if streamStartedAt == nil { streamStartedAt = Date() }
            guard revealedAt == nil else {
                phase = .streaming
                return
            }
            phase = .starting
            DispatchQueue.main.asyncAfter(deadline: .now() + Self.revealDelay) { [weak self] in
                guard let self, self.revealedAt == nil, self.engineStreaming, self.engine != nil else { return }
                self.reveal()
            }
        case let .reconnecting(attempt, _):
            phase = .reconnecting(attempt)
        case .ending:
            if phase != .ending, phase != .ended { end() }
        case let .ended(error, _):
            if let error, phase != .ending, phase != .ended {
                fail(error)
                return
            }
            if phase != .ending, phase != .ended { end() }
        }
    }

    private func reveal() {
        withAnimation(.timingCurve(0.2, 0.9, 0.1, 1, duration: 1.2)) {
            revealedAt = Date()
            phase = .streaming
        }
        startedAt = Date()
        withAnimation(Theme.easeOut.delay(0.7)) { entering = true }
        DispatchQueue.main.asyncAfter(deadline: .now() + 4.2) { [weak self] in
            withAnimation(.easeIn(duration: 0.35)) { self?.entering = false }
        }
        host.lastSession = Date()
        store.update(host)
    }

    /// Les avis du cœur, en capsules.
    private func notice(_ n: SessionEngine.Notice) {
        switch n {
        case .relaunched:
            show(notice: String(localized: "L'image a été relancée"), for: 4)
        case let .modeChanged(width, height, back):
            var text = back ? String(localized: "L'ordinateur est revenu en \(width) × \(height)") : String(localized: "L'ordinateur est passé en \(width) × \(height)")
            if let offer = modeRestoreOffer { text += String(localized: " · « \(offer) » dans la barre") }
            show(notice: text)
        case let .fallback(level, mbps):
            fallbackNote = level == 1 ? String(localized: "L'image ne passait pas : débit ramené à \(mbps) Mbit/s pour cette session.") : String(localized: "L'image ne passait pas : 4:2:0 pour cette session.")
            DispatchQueue.main.asyncAfter(deadline: .now() + 6) { [weak self] in self?.fallbackNote = nil }
        case let .adapting(width, height):
            show(notice: String(localized: "Adaptation de l'image à \(width) × \(height)…"), for: 3)
        case let .adapted(width, height):
            show(notice: String(localized: "Image adaptée à \(width) × \(height)"), for: 4)
        case let .adaptFailed(message):
            show(notice: String(localized: "L'écran de l'ordinateur n'a pas pu être adapté : \(message)"), for: 6)
        case let .restoreOffer(mode):
            modeRestoreOffer = mode.map { String(localized: "Remettre \($0.width) × \($0.height)") }
        }
    }

    /// Capsule en haut de l'image pendant quelques secondes.
    func show(notice text: String, for seconds: TimeInterval = 5) {
        withAnimation(.easeOut(duration: 0.3)) { notice = text }
        DispatchQueue.main.asyncAfter(deadline: .now() + seconds) { [weak self] in
            guard self?.notice == text else { return }
            withAnimation(.easeIn(duration: 0.3)) { self?.notice = nil }
        }
    }

    // MARK: - Barre

    func setBar(_ open: Bool) {
        guard phase == .streaming || !open else { return }
        // Pupitre (image sur l'écran externe) : la barre reste.
        if !open, external, phase == .streaming { return }
        if !open, padMode {
            padMode = false
            GamepadBridge.shared.setCaptured(false)
        }
        guard open != barOpen else { return }
        withAnimation(.spring(response: 0.32, dampingFraction: 0.86)) { barOpen = open }
        // Pupitre : tout l'écran reste le pavé tactile, barre ouverte ou non.
        streamView.touchesSuspended = open && !external
    }

    func toggleBar() { setBar(!barOpen) }

    /// L1 + R1 + Start + Select : la barre s'ouvre et la manette la pilote, ou
    /// se ferme et la manette revient à l'ordinateur.
    private func gamepadCombo() {
        guard phase == .streaming else { return }
        if padMode {
            setBar(false)
        } else {
            withAnimation(.easeOut(duration: 0.2)) { padMode = true }
            GamepadBridge.shared.setCaptured(true)
            setBar(true)
        }
    }

    /// Profil changé dans la barre : le cœur change le débit en direct, ou
    /// relance l'image pour le 4:4:4, le format ou le transport.
    func select(_ profile: Profile) {
        guard profile != host.settings.profile else { return }
        host.settings.apply(profile)
        store.update(host)
        engine?.setSettings(engineSettings)
    }

    /// Souris : Auto (jeu en plein écran), Bureau, Jeu.
    func setMouse(_ choice: MouseChoice) {
        host.settings.mouse = choice
        store.update(host)
        streamView.mouseMode = choice.streamMode
    }

    /// Pavé tactile ou toucher direct.
    func setDirectTouch(_ on: Bool) {
        prefs.directTouch = on
        streamView.directTouch = on
    }

    var directTouch: Bool { prefs.directTouch }

    /// Sans écran virtuel : afficher un autre écran de l'ordinateur.
    var canPickScreen: Bool { !virtualDisplay && screens.count > 1 }

    func showScreen(_ id: UInt32) {
        guard id != shownDisplay else { return }
        engine?.pickDisplay(id)
    }

    /// « Remettre » : la console remet le mode demandé au début de la session.
    func restoreMode() {
        modeRestoreOffer = nil
        engine?.restoreMode()
    }

    // MARK: - Clavier et touches

    func toggleKeyboard() {
        keyboardShown.toggle()
        if keyboardShown {
            setBar(false)
            streamView.showKeyboard()
        } else {
            streamView.hideKeyboard()
        }
    }

    func keyboardHidden() { keyboardShown = false }

    func tap(_ usage: UInt16) { streamView.tap(usage: usage) }
    func toggleSticky(_ usage: UInt16) { streamView.toggleSticky(usage) }
    func chord(_ modifiers: [UInt16], _ usage: UInt16) { streamView.chord(modifiers, usage) }

    var sessionMinutes: Int { Int(Date().timeIntervalSince(startedAt) / 60) }

    #if DEBUG
    /// Démonstration (simulateur) : le passage, puis la barre, sans ordinateur.
    func demo(bar: Bool) {
        phase = .starting
        configureView()
        streamView.startTestPattern()
        externalWatch = ExternalDisplay.shared.$scene.sink { [weak self] scene in
            DispatchQueue.main.async { self?.useExternal(scene != nil, adapt: true) }
        }
        DispatchQueue.main.asyncAfter(deadline: .now() + 1.6) { [weak self] in
            guard let self else { return }
            withAnimation(.timingCurve(0.2, 0.9, 0.1, 1, duration: 1.2)) {
                self.revealedAt = Date()
                self.phase = .streaming
            }
            self.startedAt = Date().addingTimeInterval(-12 * 60)
            self.modeLabel = "2622 × 1206 · 120 Hz"
            for i in 0..<30 {
                var s = VideoStats()
                s.latencyMs = 8.4 + Double(i % 7) * 0.4
                s.fps = 120
                s.mbps = 46
                s.format = "4:2:0 8 bits"
                s.width = 2622
                s.height = 1206
                self.received(s)
            }
            if bar { DispatchQueue.main.asyncAfter(deadline: .now() + 0.4) { self.setBar(true) } }
        }
    }
    #endif

    // MARK: - Fin

    /// Fin de session : le cœur rend la main à la console (écrans et son de
    /// l'ordinateur rétablis, latence donnée à ses appareils).
    func end() {
        guard phase != .ending && phase != .ended else { return }
        GamepadBridge.shared.onMenuRequest = nil
        streamView.hideKeyboard()
        if let last = engine?.snapshot(maxWidth: 1600) { store.imagery.saveLast(last, for: host) }
        if let revealedAt { host.lastDuration = Date().timeIntervalSince(revealedAt) }
        if let latency = engine?.sessionLatency {
            host.lastLatencyMs = latency.median
            host.lastLatencyP95Ms = latency.p95
        }
        store.update(host)
        withAnimation(.easeInOut(duration: 0.4)) {
            phase = .ending
            barOpen = false
        }
        leaveExternal()
        streamView.detach()
        engine?.stop()
        observers.forEach(NotificationCenter.default.removeObserver)
        observers = []
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.6) { [weak self] in self?.finish() }
    }

    /// Fin de session : l'écran externe retrouve son image d'attente.
    private func leaveExternal() {
        externalWatch = nil
        guard external else { return }
        external = false
        streamView.showImage(in: nil)
        ExternalDisplay.shared.showingImage(false)
    }

    func finish() {
        guard phase != .ended else { return }
        phase = .ended
        onFinish(self)
    }

    private func fail(_ message: String) {
        KyberLog.app.error("session : \(message, privacy: .public)")
        GamepadBridge.shared.onMenuRequest = nil
        leaveExternal()
        streamView.detach()
        engine?.stop()
        observers.forEach(NotificationCenter.default.removeObserver)
        observers = []
        withAnimation(Theme.easeOut) {
            barOpen = false
            phase = .failed(message)
        }
    }

    func dismissFailure() { finish() }
}
