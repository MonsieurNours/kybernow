// Une session avec un ordinateur : préparation par la console (écran
// virtuel à la taille de ce Mac, son), connexion à Kyber, image en plein
// écran, barre au survol, signal de vie, puis fin propre et vérifiée.

import AppKit
import KyberKit
import SwiftUI

@MainActor
final class SessionController: NSObject, ObservableObject, NSWindowDelegate {
    enum Phase: Equatable {
        case preparing, connecting, starting, streaming
        case reconnecting(Int)
        case ending
        case ended
        case failed(String)
    }

    /// Vérifications affichées en fin de session.
    struct Closing: Equatable {
        var displayRestored: Bool?
        var audioRestored: Bool?
        var error: String?
        var done = false
    }

    @Published private(set) var phase: Phase = .preparing
    /// Instant où l'image est apparue (gerbe de particules).
    @Published private(set) var revealedAt: Date?
    @Published private(set) var barVisible = false
    /// La manette pilote la barre (L1 + R1 + START + SELECT).
    @Published private(set) var padMode = false
    /// Élément de la barre qui a le focus en mode manette.
    @Published private(set) var padFocus = 0
    static let padItems = 4 // profil, souris, plein écran, déconnexion
    @Published private(set) var closing = Closing()
    @Published private(set) var host: Host
    @Published private(set) var modeLabel = ""
    /// Mode de l'écran de la session (celui obtenu de l'ordinateur), pour le débit.
    private var streamMode = (width: 0, height: 0, refresh: 0)

    private var streamOptions: StreamOptions {
        host.settings.options(width: streamMode.width, height: streamMode.height, refresh: streamMode.refresh)
    }
    /// Mesures de l'image (décodage par l'app), une fois par seconde.
    @Published private(set) var stats: VideoStats?
    /// Souris en mode jeu en ce moment.
    @Published private(set) var mouseCaptured = false
    /// Jeu en plein écran sur l'ordinateur (nom de l'exécutable), d'après la console.
    @Published private(set) var pcGame: String?
    private var foregroundPoll: Task<Void, Never>?
    private var lastForeground: String?
    private var lastStallRestart = Date.distantPast
    @Published private(set) var audioStats: AudioStats?
    /// Rappel affiché quelques secondes après l'apparition.
    enum Hint { case commands, mouse }
    /// Rappel affiché quelques secondes en haut de l'écran.
    @Published private(set) var hint: Hint?
    private var mouseHintShown = false

    private let store: HostStore
    private let onFinish: (SessionController) -> Void

    private let streamView = StreamView(frame: .zero)
    private let scene = ConnectScene(frame: .zero)
    /// Délai entre le démarrage du flux et l'apparition : le décodeur et la
    /// synchronisation du son se calent, les premières images irrégulières
    /// restent sous l'écran d'attente.
    private static let revealDelay: TimeInterval = 0.9
    private var window: NSWindow?
    private var console: ConsoleClient?
    private var kyber: KyberClient?
    private var heartbeat: Task<Void, Never>?
    private var hideBar: Task<Void, Never>?
    private var prepared: PrepareResult?
    private var display: KyberDisplay?
    /// Écrans annoncés par Kyber (la liste se met à jour quelques secondes
    /// après la création de l'écran virtuel : les écrans éteints en sortent).
    private var displays: [KyberDisplay] = []
    private var streaming: UInt32?
    private var settling: Task<Void, Never>?

    /// Hauteur de la bande où vit la barre au survol.
    static let barBand: CGFloat = 96
    /// Bande occupée par la barre (plus haute avec le bandeau manette).
    var barHeight: CGFloat { padMode ? 150 : Self.barBand }

    init(host: Host, store: HostStore, onFinish: @escaping (SessionController) -> Void) {
        self.host = host
        self.store = store
        self.onFinish = onFinish
    }

    // MARK: - Fenêtre

    private func openWindow() {
        let screen = NSScreen.main ?? NSScreen.screens[0]
        let window = NSWindow(contentRect: screen.frame, styleMask: [.titled, .closable, .miniaturizable, .resizable, .fullSizeContentView], backing: .buffered, defer: false)
        window.title = host.name
        window.titlebarAppearsTransparent = true
        window.titleVisibility = .hidden
        window.backgroundColor = .black
        window.collectionBehavior = [.fullScreenPrimary]
        window.delegate = self
        window.isReleasedWhenClosed = false
        window.acceptsMouseMovedEvents = true

        let container = NSView(frame: window.contentRect(forFrameRect: window.frame))
        container.autoresizingMask = [.width, .height]
        streamView.frame = container.bounds
        streamView.autoresizingMask = [.width, .height]
        streamView.onPointerMoved = { [weak self] distance in self?.pointerMoved(distanceFromTop: distance) }
        container.addSubview(streamView)
        scene.frame = container.bounds
        scene.autoresizingMask = [.width, .height]
        container.addSubview(scene)
        scene.hide(streamView)
        let chrome = ChromeHostingView(rootView: SessionChrome(session: self))
        chrome.session = self
        chrome.frame = container.bounds
        chrome.autoresizingMask = [.width, .height]
        container.addSubview(chrome)
        window.contentView = container
        window.makeKeyAndOrderFront(nil)
        window.makeFirstResponder(streamView)
        if host.settings.display == .window {
            // Session fenêtrée : l'écran virtuel suivra la taille de la fenêtre.
            let visible = screen.visibleFrame
            window.setFrame(visible.insetBy(dx: visible.width * 0.1, dy: visible.height * 0.1), display: true)
        } else {
            window.toggleFullScreen(nil)
        }
        self.window = window
    }

    func toggleFullScreen() {
        window?.toggleFullScreen(nil)
    }

    // Plein écran sans barre des menus ni Dock : le haut de l'écran est à la barre KyberNow.
    func window(_ window: NSWindow, willUseFullScreenPresentationOptions proposedOptions: NSApplication.PresentationOptions = []) -> NSApplication.PresentationOptions {
        [.fullScreen, .hideDock, .hideMenuBar]
    }

    func windowWillClose(_ notification: Notification) {
        if phase != .ended { end() }
    }

    /// Les couches SwiftUI captent-elles tout (attente, fin) ?
    var chromeCapturesAll: Bool {
        phase != .streaming
    }

    // MARK: - Barre au survol

    private func pointerMoved(distanceFromTop distance: CGFloat) {
        guard phase == .streaming else { return }
        if distance < 3 {
            hideBar?.cancel()
            setBar(true)
        } else if barVisible, !padMode, distance > Self.barBand + 30 {
            hideBar?.cancel()
            hideBar = Task { [weak self] in
                try? await Task.sleep(for: .milliseconds(350))
                guard !Task.isCancelled else { return }
                self?.setBar(false)
            }
        }
    }

    func setBar(_ visible: Bool) {
        if !visible, padMode {
            padMode = false
            kyber?.setGamepadCaptured(false)
        }
        guard visible != barVisible else { return }
        withAnimation(.spring(response: 0.32, dampingFraction: 0.86)) { barVisible = visible }
        streamView.reservedTopBand = visible ? 150 : 0
        streamView.relativeSuspended = visible
        window?.invalidateCursorRects(for: streamView)
    }

    // MARK: - Manette

    private func gamepadCombo() {
        guard phase == .streaming else { return }
        if padMode {
            setBar(false)
        } else {
            hideBar?.cancel()
            padFocus = 0
            withAnimation(.easeOut(duration: 0.2)) { padMode = true }
            setBar(true)
        }
    }

    private func gamepadNavigate(_ key: PadKey) {
        guard padMode else { return }
        switch key {
        case .left: padFocus = (padFocus + Self.padItems - 1) % Self.padItems
        case .right: padFocus = (padFocus + 1) % Self.padItems
        case .back: setBar(false)
        case .accept: activatePadFocus()
        case .up, .down: break
        }
    }

    private func activatePadFocus() {
        switch padFocus {
        case 0:
            // Profil suivant (Jeu → Bureau → Max).
            let all = Profile.allCases
            let next = host.settings.profile.flatMap { all.firstIndex(of: $0) }.map { all[($0 + 1) % all.count] } ?? .jeu
            select(next)
        case 1:
            // Souris suivante (Auto → Bureau → Jeu).
            let all = MouseChoice.allCases
            let i = all.firstIndex(of: host.settings.mouse) ?? 0
            setMouse(all[(i + 1) % all.count])
        case 2:
            toggleFullScreen()
        default:
            end()
        }
    }

    /// Change de profil en cours de session : le débit change en direct,
    /// le 4:4:4 ou le format relancent l'image (une seconde environ).
    func select(_ profile: Profile) {
        guard profile != host.settings.profile else { return }
        let before = streamOptions
        host.settings.apply(profile)
        store.update(host)
        let after = streamOptions
        if before.yuv444 != after.yuv444 || before.codec != after.codec {
            restartStreaming()
        } else {
            kyber?.setBitrate(after.bitrate)
            kyber?.setVSync(after.vsync)
        }
    }

    // MARK: - Déroulé

    func start() {
        openWindow()
        guard let password = store.password(for: host) else {
            fail("mot de passe introuvable dans le Trousseau")
            return
        }
        let screen = window?.screen ?? NSScreen.main ?? NSScreen.screens[0]
        let settings = host.settings
        let mode: (width: Int, height: Int, refresh: Int)
        switch settings.display {
        case .custom:
            mode = (settings.customWidth, settings.customHeight, settings.customRefresh)
        case .window:
            // Session fenêtrée : l'écran virtuel prend la taille de la fenêtre, en pixels.
            // Pixels de la dalle par point (moins que l'échelle de macOS en mode « plus d'espace »).
            let content = window?.contentLayoutRect.size ?? screen.frame.size
            let scale = CGFloat(DisplayInfo.nativePixels(screen).width) / screen.frame.width
            mode = (Int(content.width * scale) / 2 * 2, Int(content.height * scale) / 2 * 2, max(30, screen.maximumFramesPerSecond))
        case .match, .physical:
            mode = DisplayInfo.mode(for: screen, codec: settings.codec, limit4K: settings.limit4K)
        }
        modeLabel = "\(mode.width) × \(mode.height) · \(mode.refresh) Hz"
        streamMode = mode
        let console = ConsoleClient(host: host.address, port: host.consolePort, username: host.username, password: password, fingerprint: host.consoleFingerprint)
        self.console = console

        Task {
            do {
                phase = .preparing
                let channels = settings.audio.channels
                KyberLog.app.notice("préparation : \(mode.width)×\(mode.height) à \(mode.refresh) Hz, son \(channels) canaux")
                let result = try await console.prepare(width: mode.width, height: mode.height, refresh: mode.refresh, audioChannels: channels, virtualDisplay: settings.display != .physical)
                prepared = result
                if let w = result.width, let h = result.height, let hz = result.refresh {
                    modeLabel = "\(w) × \(h) · \(hz) Hz"
                    streamMode = (w, h, hz)
                }
                if host.consoleFingerprint == nil || host.streamPort != result.kyberPort {
                    host.consoleFingerprint = console.fingerprint
                    host.streamPort = result.kyberPort ?? host.streamPort
                    store.update(host)
                }
                startHeartbeat(every: result.heartbeatSeconds)
                startForegroundPoll()
                try connect(password: password)
            } catch {
                fail(error.localizedDescription)
            }
        }
    }

    private func connect(password: String) throws {
        guard let port = prepared?.kyberPort ?? host.streamPort else {
            throw ConsoleError.http(0, "port du flux inconnu")
        }
        phase = .connecting
        let kyber = try KyberClient(host: host.address, port: port, username: host.username, password: password, autoReconnect: host.settings.autoReconnect)
        kyber.onEvent = { [weak self] in self?.handle($0) }
        kyber.onGamepadCombo = { [weak self] in self?.gamepadCombo() }
        kyber.onGamepadNavigate = { [weak self] in self?.gamepadNavigate($0) }
        kyber.onVideoStats = { [weak self] in self?.stats = $0 }
        kyber.onVideoStalled = { [weak self] in
            // Ordinateur sans le correctif d'image clé : un flux neuf repart sur une image clé.
            guard let self, Date().timeIntervalSince(self.lastStallRestart) > 5 else { return }
            self.lastStallRestart = Date()
            self.restartStreaming()
        }
        kyber.onAudioStats = { [weak self] in self?.audioStats = $0 }
        self.kyber = kyber
        try kyber.connect()
    }

    private func handle(_ event: KyberEvent) {
        KyberLog.app.notice("événement : \(String(describing: event), privacy: .public)")
        switch event {
        case let .connected(list):
            displays = list
            chooseDisplay()
        case let .displaysChanged(list):
            displays = list
            if let streaming, !list.contains(where: { $0.id == streaming }) {
                // L'écran suivi a disparu (écran physique éteint) : on passe au bon.
                restartStreaming()
            } else if settling != nil, list.count == 1 {
                settling?.cancel()
                settling = nil
                chooseDisplay()
            }
        case .streamingStarted:
            guard revealedAt == nil else {
                phase = .streaming
                return
            }
            DispatchQueue.main.asyncAfter(deadline: .now() + Self.revealDelay) { [weak self] in
                guard let self, self.revealedAt == nil, self.kyber != nil else { return }
                self.revealedAt = Date()
                self.phase = .streaming
                self.scene.reveal(self.streamView)
                self.showHint()
                self.host.lastSession = Date()
                self.store.update(self.host)
                self.window?.makeFirstResponder(self.streamView)
            }
        case let .streamingFailed(reason), let .pipelineError(reason):
            fail(reason)
        case let .connectFailed(reason):
            fail(reason)
        case let .reconnecting(attempt, _):
            phase = .reconnecting(attempt)
        case let .reconnected(restart):
            phase = .streaming
            if restart {
                streaming = nil
                startStreaming()
            }
        case .reconnectionFailed:
            fail("connexion perdue")
        case let .stopped(reason):
            if phase != .ending && phase != .ended { fail(reason) }
        case .streamingStopped:
            break
        }
    }

    /// Avec l'écran virtuel, Kyber doit finir par n'annoncer qu'un écran :
    /// on attend que sa liste se stabilise (6 s au plus) avant de démarrer.
    private func chooseDisplay() {
        if prepared?.virtualDisplay == true, displays.count > 1, settling == nil, streaming == nil {
            phase = .starting
            settling = Task { [weak self] in
                try? await Task.sleep(for: .seconds(6))
                guard !Task.isCancelled, let self else { return }
                self.settling = nil
                self.chooseDisplay()
            }
            return
        }
        guard let best = bestDisplay() else {
            fail("l'ordinateur n'a annoncé aucun écran")
            return
        }
        if streaming == best.id { return }
        display = best
        startStreaming()
    }

    private func bestDisplay() -> KyberDisplay? {
        if displays.count == 1 { return displays[0] }
        let sameMode = displays.filter { $0.width == prepared?.width && $0.height == prepared?.height }
        // L'écran virtuel est placé en haut à gauche du bureau.
        return sameMode.first { $0.x == 0 && $0.y == 0 } ?? sameMode.first ?? displays.first
    }

    private func startStreaming() {
        guard let kyber, let display else { return }
        if revealedAt == nil { phase = .starting }
        do {
            KyberLog.app.notice("image : écran \(display.id) \(display.width)×\(display.height) en (\(display.x), \(display.y))")
            streamView.commandAsControl = host.settings.commandAsControl
            streamView.mouseMode = host.settings.mouse.streamMode
            streamView.onRelativeChanged = { [weak self] on in
                guard let self else { return }
                self.mouseCaptured = on
                // Premier passage en mode jeu : on dit comment reprendre la souris.
                guard on, !self.mouseHintShown else { return }
                self.mouseHintShown = true
                self.showHint(.mouse, after: 0)
            }
            try kyber.startStreaming(display: display, view: streamView, options: streamOptions)
            streaming = display.id
        } catch {
            fail(error.localizedDescription)
        }
    }

    private func restartStreaming() {
        guard let kyber, let best = bestDisplay() else { return }
        kyber.stopStreaming()
        streaming = nil
        display = best
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.3) { [weak self] in self?.startStreaming() }
    }

    private func startHeartbeat(every seconds: Int) {
        heartbeat?.cancel()
        heartbeat = Task { [weak self] in
            while !Task.isCancelled {
                try? await Task.sleep(for: .seconds(max(1, seconds)))
                await self?.console?.heartbeat()
            }
        }
    }

    /// Fin de session : l'image se referme, la console rallume les écrans du
    /// PC et rétablit son son, l'app affiche ce qui a été vérifié.
    func end() {
        guard phase != .ending && phase != .ended else { return }
        setBar(false)
        if revealedAt != nil { scene.close(streamView) }
        withAnimation(.easeInOut(duration: 0.7)) { phase = .ending }
        heartbeat?.cancel()
        foregroundPoll?.cancel()
        streamView.detach()
        // Latence médiane de la session, montrée sur l'accueil.
        if let latency = kyber?.sessionLatencyMs {
            host.lastLatencyMs = latency
            store.update(host)
        }
        kyber?.stop()
        kyber = nil
        stats = nil
        audioStats = nil
        let console = self.console
        Task {
            var report: EndReport?
            var problem: String?
            do {
                report = try await console?.end()
            } catch {
                problem = error.localizedDescription
            }
            withAnimation(.spring(response: 0.4, dampingFraction: 0.8)) {
                closing.displayRestored = report?.displayRestored ?? false
            }
            try? await Task.sleep(for: .milliseconds(450))
            withAnimation(.spring(response: 0.4, dampingFraction: 0.8)) {
                closing.audioRestored = report?.audioRestored ?? false
                closing.error = problem ?? report?.error
            }
            let clean = report?.displayRestored == true && report?.audioRestored == true && problem == nil
            withAnimation { closing.done = true }
            if clean {
                try? await Task.sleep(for: .milliseconds(1100))
                finish()
            }
        }
    }

    private func showHint(_ which: Hint = .commands, after delay: TimeInterval = 1.6) {
        DispatchQueue.main.asyncAfter(deadline: .now() + delay) { [weak self] in
            withAnimation(.easeOut(duration: 0.35)) { self?.hint = which }
        }
        DispatchQueue.main.asyncAfter(deadline: .now() + delay + 3.4) { [weak self] in
            guard self?.hint == which else { return }
            withAnimation(.easeIn(duration: 0.35)) { self?.hint = nil }
        }
    }

    /// ⌃⌥⌘M : mode jeu si la souris est en mode bureau, et inversement.
    func toggleMouseCapture() {
        setMouse(streamView.isRelative ? .desktop : .game)
    }

    /// Choix de la barre ou de ⌃⌥⌘M, gardé pour cet ordinateur.
    func setMouse(_ choice: MouseChoice) {
        host.settings.mouse = choice
        store.update(host)
        streamView.mouseMode = choice.streamMode
    }

    /// Mode Auto : la console dit chaque seconde si un jeu est en plein écran.
    private func startForegroundPoll() {
        foregroundPoll?.cancel()
        foregroundPoll = Task { [weak self] in
            while !Task.isCancelled {
                guard let self, let console = self.console else { return }
                let state = await console.foreground()
                let game = state?.game == true ? (state?.exe ?? "jeu") : nil
                // Journal : veille muette, ou plein écran qui n'est pas un jeu.
                let seen = state == nil ? "veille du plein écran sans réponse"
                    : state!.fullscreen ? "plein écran : \(state!.exe ?? "?")\(state!.game ? "" : " (pas un jeu)")"
                    : "pas de plein écran (premier plan : \(state!.exe ?? "?"))"
                if seen != self.lastForeground {
                    self.lastForeground = seen
                    KyberLog.app.notice("ordinateur : \(seen, privacy: .public)")
                }
                if game != self.pcGame {
                    KyberLog.app.notice("ordinateur : \(game.map { "jeu en plein écran (\($0))" } ?? "pas de jeu en plein écran", privacy: .public)")
                    self.pcGame = game
                    self.streamView.pcGameFullscreen = game != nil
                }
                try? await Task.sleep(for: .seconds(1))
            }
        }
    }

    func finish() {
        phase = .ended
        window?.delegate = nil
        if window?.styleMask.contains(.fullScreen) == true { window?.toggleFullScreen(nil) }
        window?.close()
        onFinish(self)
    }

    private func fail(_ message: String) {
        KyberLog.app.error("session : \(message, privacy: .public)")
        heartbeat?.cancel()
        foregroundPoll?.cancel()
        kyber?.stop()
        kyber = nil
        stats = nil
        audioStats = nil
        setBar(false)
        phase = .failed(message)
        Task { try? await console?.end() }
    }

    func dismissFailure() {
        finish()
    }
}
