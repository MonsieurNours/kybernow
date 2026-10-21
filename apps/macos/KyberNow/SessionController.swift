// Une session avec un ordinateur : préparation par la console (écran
// virtuel à la taille de ce Mac, son), connexion à Kyber, image en plein
// écran, barre au survol, signal de vie, puis fin propre et vérifiée.
//
// La session se joue dans la fenêtre de l'accueil : le passage (Passage.swift)
// ouvre l'accueil en deux sur l'image, et l'y referme à la fin.

import AppKit
import Combine
import KyberKit
import SwiftUI

@MainActor
final class SessionController: NSObject, ObservableObject {
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
        var o = host.settings.options(width: streamMode.width, height: streamMode.height, refresh: streamMode.refresh)
        // Repli de cette session seulement (les réglages ne changent pas).
        if fallback >= 1 { o.bitrate = min(o.bitrate, Self.fallbackBitrate) }
        if fallback >= 2 { o.yuv444 = false }
        return o
    }

    /// L'image ne vient pas : 1 = débit ramené à `fallbackBitrate`, 2 = 4:2:0 en plus.
    private var fallback = 0
    private static let fallbackBitrate: UInt32 = 250_000_000
    /// Une image a été montrée depuis le dernier démarrage du flux.
    private var hasImage = false
    private var imageWatch: Task<Void, Never>?
    /// Cran de repli du flux en cours (Kyber relance l'ancien après une reconnexion).
    private var streamingFallback = 0
    /// Mesures de l'image (décodage par l'app), une fois par seconde.
    @Published private(set) var stats: VideoStats?
    /// Latences des dernières secondes (courbe de la barre).
    @Published private(set) var latencyHistory: [Double] = []
    /// Mesures des 30 dernières secondes (panneau des mesures).
    @Published private(set) var statsHistory: [VideoStats] = []
    /// Pertes depuis le début de la session.
    @Published private(set) var sessionLosses = (frames: 0, packets: 0)
    /// Panneau des mesures détaillées, en bas à droite (⌃⌥⌘S).
    @Published private(set) var statsPanel = false
    /// Bouton de fermeture du panneau, dans la couche de la session : seul
    /// endroit du panneau qui prend les clics.
    var statsCloseRect: CGRect = .zero
    /// Quelques secondes après l'apparition : « Vous entrez dans… ».
    @Published private(set) var entering = false
    /// Souris en mode jeu en ce moment.
    @Published private(set) var mouseCaptured = false
    /// Jeu en plein écran sur l'ordinateur (nom de l'exécutable), d'après la console.
    @Published private(set) var pcGame: String?
    private var foregroundPoll: Task<Void, Never>?
    private var lastForeground: String?
    private var lastStallRestart = Date.distantPast
    /// Dernière reconnexion de Kyber, et nouvel essai déjà fait après elle.
    private var lastReconnect: Date?
    private var retriedAfterReconnect = false
    /// Relances de l'image après une erreur de l'ordinateur (trois au plus,
    /// le compte repart après 30 s sans relance), la relance prévue, et une
    /// relance faite dont la capsule reste à montrer.
    private var pipelineRetries = 0
    private var lastPipelineRetry = Date.distantPast
    private var pipelineRetry: Task<Void, Never>?
    private var pipelineRelaunched = false
    /// Écran et son demandés à la console (redemandés si l'écran virtuel disparaît).
    private var requested: (width: Int, height: Int, refresh: Int, channels: Int)?
    /// Écran virtuel redemandé à la console, et dernière demande.
    private var restoring: Task<Void, Never>?
    private var lastRestore = Date.distantPast
    @Published private(set) var audioStats: AudioStats?
    /// Rappel affiché quelques secondes après l'apparition.
    enum Hint: Equatable { case commands, mouse, fallback(String), notice(String) }
    /// Profil Bureau, écran de l'ordinateur passé à un autre mode : « Remettre
    /// 3840 × 2160 » dans la barre.
    @Published private(set) var modeRestoreOffer: String?
    /// Dernière demande de remise du mode faite d'après le signal de vie, et
    /// la demande en cours.
    private var lastModeRestore = Date.distantPast
    private var modeRestore: Task<Void, Never>?
    /// Rappel affiché quelques secondes en haut de l'écran.
    @Published private(set) var hint: Hint?
    private var mouseHintShown = false

    /// Mode d'un écran : définition réelle et fréquence.
    struct ScreenMode: Equatable {
        let width: Int
        let height: Int
        let refresh: Int
    }

    /// « Comme cet écran », écran de ce Mac changé en session (écran externe
    /// branché ou débranché, définition changée) : mode proposé pour l'écran
    /// de l'ordinateur, jamais appliqué sans l'accord de l'utilisateur
    /// (capsule, puis puce dans la barre).
    @Published private(set) var screenOffer: ScreenMode?
    /// Capsule de la proposition, en haut de l'image (quelques secondes).
    @Published private(set) var screenOfferShown = false
    /// Capsule de la proposition, dans l'espace « chrome » : seul endroit
    /// cliquable hors de la barre.
    var screenOfferRect: CGRect = .zero
    private var screenObservers: [NSObjectProtocol] = []
    private var screenCheck: Task<Void, Never>?
    private var screenOfferHide: Task<Void, Never>?
    /// Plein écran recalé sur l'écran restant, en cours.
    private var refitting = false
    /// Capsule à montrer au prochain démarrage de l'image.
    private var pendingNotice: String?

    private let store: HostStore
    private let onFinish: (SessionController) -> Void

    private let streamView = StreamView(frame: .zero)
    private let scene = PassageScene(frame: .zero)
    /// L'accueil, retiré de la fenêtre le temps de la session, et sa photographie.
    private var homeView: NSView?
    private var homeImage: CGImage?
    private let windowDelegate = SessionWindowDelegate()
    private var enteredFullScreen = false
    /// Barre, rappels, panneaux (SwiftUI), par-dessus l'image. Retirée de la
    /// composition quand elle n'a rien à montrer : l'image peut aller
    /// directement à l'écran (présentation directe en plein écran).
    private var chrome: ChromeHostingView?
    private var compositing: AnyCancellable?
    /// Le passage (accueil ouvert en deux) sort de la composition une fois ouvert.
    private var sceneHide: Task<Void, Never>?
    /// Fenêtre occultée (autre espace, réduite, recouverte) : rendu arrêté, débit baissé.
    private var occlusionObserver: NSObjectProtocol?
    private var occluded = false
    private static let occludedBitrate: UInt32 = 20_000_000
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
    private var displays: [KyberDisplay] = [] { didSet { screens = displays } }
    /// Écrans de l'ordinateur, pour le choix dans la barre (sans écran virtuel).
    @Published private(set) var screens: [KyberDisplay] = []
    /// Écran affiché en ce moment.
    @Published private(set) var shownDisplay: UInt32?
    /// Écran choisi dans la barre, tant qu'il existe.
    private var pickedDisplay: UInt32?
    private var streaming: UInt32?
    private var settling: Task<Void, Never>?
    /// Fin de l'attente d'une liste d'écrans stable (une seule attente par démarrage).
    private var settleUntil: Date?

    /// Hauteur de la bande où vit la barre au survol.
    static let barBand: CGFloat = 96
    /// Bande occupée par la barre (plus haute avec le bandeau manette).
    var barHeight: CGFloat { padMode ? 150 : Self.barBand }

    init(host: Host, store: HostStore, window: NSWindow?, onFinish: @escaping (SessionController) -> Void) {
        self.host = host
        self.store = store
        self.window = window
        self.onFinish = onFinish
    }

    // MARK: - Fenêtre

    private func openWindow() {
        guard let window, let home = window.contentView else { return }
        homeView = home
        homeImage = Self.photograph(home)
        let container = NSView(frame: home.frame)
        container.autoresizingMask = [.width, .height]
        streamView.frame = container.bounds
        streamView.autoresizingMask = [.width, .height]
        streamView.onPointerMoved = { [weak self] distance in self?.pointerMoved(distanceFromTop: distance) }
        container.addSubview(streamView)
        scene.frame = container.bounds
        scene.autoresizingMask = [.width, .height]
        container.addSubview(scene)
        scene.prepare(home: homeImage)
        let chrome = ChromeHostingView(rootView: SessionChrome(session: self))
        chrome.session = self
        chrome.frame = container.bounds
        chrome.autoresizingMask = [.width, .height]
        container.addSubview(chrome)
        self.chrome = chrome
        window.contentView = container
        // Après chaque changement d'état : ce qui n'a rien à montrer sort de la composition.
        compositing = objectWillChange.receive(on: RunLoop.main).sink { [weak self] _ in self?.updateCompositing() }
        occlusionObserver = NotificationCenter.default.addObserver(forName: NSWindow.didChangeOcclusionStateNotification, object: window, queue: .main) { [weak self] _ in
            MainActor.assumeIsolated { self?.occlusionChanged() }
        }
        // Écran branché, débranché, définition changée ; fenêtre passée sur un autre écran.
        screenObservers = [
            NotificationCenter.default.addObserver(forName: NSApplication.didChangeScreenParametersNotification, object: nil, queue: .main) { [weak self] _ in
                MainActor.assumeIsolated { self?.screensChanged() }
            },
            NotificationCenter.default.addObserver(forName: NSWindow.didChangeScreenNotification, object: window, queue: .main) { [weak self] _ in
                MainActor.assumeIsolated { self?.screensChanged() }
            },
        ]
        windowDelegate.original = window.delegate
        windowDelegate.onClose = { [weak self] in
            guard let self, self.phase != .ended else { return }
            self.end()
        }
        window.delegate = windowDelegate
        window.acceptsMouseMovedEvents = true
        window.makeFirstResponder(streamView)
        scene.ignite()
        // Session fenêtrée : l'écran virtuel suivra la taille de la fenêtre.
        if host.settings.display != .window, !window.styleMask.contains(.fullScreen) {
            enteredFullScreen = true
            window.toggleFullScreen(nil)
        }
    }

    /// Pendant le flux, seule la couche Metal doit rester dans la composition
    /// de la fenêtre pour que l'image aille directement à l'écran : la couche
    /// SwiftUI est cachée tant qu'elle n'a rien à montrer (barre, manette,
    /// rappel, panneau des mesures, entrée, fin), et le passage une fois
    /// ouvert (son animation dure 1,1 s). Hors flux, tout revient.
    private func updateCompositing() {
        let streaming = phase == .streaming
        let needed = !streaming || barVisible || padMode || hint != nil || statsPanel || entering || screenOfferShown
        if chrome?.isHidden == needed { chrome?.isHidden = !needed }
        if streaming {
            guard sceneHide == nil, !scene.isHidden else { return }
            sceneHide = Task { [weak self] in
                try? await Task.sleep(for: .milliseconds(1400))
                guard let self, !Task.isCancelled, self.phase == .streaming else { return }
                self.scene.isHidden = true
            }
        } else {
            sceneHide?.cancel()
            sceneHide = nil
            if scene.isHidden { scene.isHidden = false }
        }
    }

    /// Fenêtre plus visible (autre espace, réduite dans le Dock, recouverte) :
    /// plus de rendu (`nextDrawable` attendait jusqu'à 1 s) et débit ramené à
    /// 20 Mb/s ; au retour, rendu et débit du profil reviennent.
    private func occlusionChanged() {
        guard let window else { return }
        let hidden = !window.occlusionState.contains(.visible)
        guard hidden != occluded else { return }
        occluded = hidden
        kyber?.setOccluded(hidden)
        guard phase == .streaming || hidden == false else { return }
        kyber?.setBitrate(hidden ? min(streamOptions.bitrate, Self.occludedBitrate) : streamOptions.bitrate)
        KyberLog.app.notice("fenêtre \(hidden ? "occultée : rendu arrêté, débit réduit" : "visible : rendu et débit repris", privacy: .public)")
    }

    /// Photographie de l'accueil, pour les deux moitiés du passage.
    private static func photograph(_ view: NSView) -> CGImage? {
        guard let rep = view.bitmapImageRepForCachingDisplay(in: view.bounds) else { return nil }
        view.cacheDisplay(in: view.bounds, to: rep)
        return rep.cgImage
    }

    func toggleFullScreen() {
        window?.toggleFullScreen(nil)
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

    /// L1 + R1 + START + SELECT : la barre s'ouvre et la manette la pilote,
    /// ou se ferme et la manette revient à l'ordinateur. Hors session
    /// (connexion, reconnexion), rien : la manette reste à l'ordinateur.
    private func gamepadCombo() {
        guard phase == .streaming else { return }
        if padMode {
            setBar(false)
        } else {
            hideBar?.cancel()
            padFocus = 0
            withAnimation(.easeOut(duration: 0.2)) { padMode = true }
            kyber?.setGamepadCaptured(true)
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
        refreshModeOffer()
        // Profil choisi à la main : le repli de la session repart de zéro.
        fallback = 0
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
                requested = (mode.width, mode.height, mode.refresh, channels)
                KyberLog.app.notice("préparation : \(mode.width)×\(mode.height) à \(mode.refresh) Hz, son \(channels) canaux")
                let result = try await console.prepare(width: mode.width, height: mode.height, refresh: mode.refresh, audioChannels: channels, virtualDisplay: settings.display != .physical, profile: settings.profileLabel, modePolicy: settings.modePolicy)
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
        kyber.onVideoStats = { [weak self] stats in
            guard let self else { return }
            self.stats = stats
            if stats.fps > 0, !self.hasImage {
                self.hasImage = true
                self.imageWatch?.cancel()
            }
            if let ms = stats.latencyMs {
                self.latencyHistory.append(ms)
                if self.latencyHistory.count > 40 { self.latencyHistory.removeFirst() }
            }
            self.statsHistory.append(stats)
            if self.statsHistory.count > 30 { self.statsHistory.removeFirst() }
            self.sessionLosses.frames += stats.lostFrames
            self.sessionLosses.packets += stats.lostPackets
        }
        kyber.onVideoStalled = { [weak self] in
            guard let self else { return }
            // Jamais d'image depuis le démarrage : le flux est trop lourd, on l'allège.
            if !self.hasImage, self.degrade() { return }
            // Ordinateur sans le correctif d'image clé : un flux neuf repart sur une image clé.
            guard Date().timeIntervalSince(self.lastStallRestart) > 5 else { return }
            self.lastStallRestart = Date()
            self.restartStreaming()
        }
        kyber.onAudioStats = { [weak self] in self?.audioStats = $0 }
        kyber.onHostSizeChanged = { [weak self] width, height in self?.hostSizeChanged(width: width, height: height) }
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
            // Même écran, autre taille (un jeu a changé le mode) : l'image
            // continue (txproto recrée l'encodeur), les entrées se recalent.
            if let shown = display, shown.id == streaming,
               let now = list.first(where: { $0.id == shown.id }), now.width > 0, now.height > 0 {
                if now != shown { display = now }
                // KyberClient compare à la dernière taille de la liste, pas à
                // celle des images : la liste corrige une image mise à l'échelle.
                kyber?.displayResized(width: now.width, height: now.height)
            }
            if case .reconnecting = phase {
                // Kyber se reconnecte et relancera l'image lui-même (reconnected) :
                // la relancer ici en ferait démarrer deux.
                if let streaming, !list.contains(where: { $0.id == streaming }) { display = bestDisplay() }
            } else if let streaming, !list.contains(where: { $0.id == streaming }) {
                // L'écran suivi a disparu (écran physique éteint) : on passe au bon.
                restartStreaming()
            } else if settling != nil, usableDisplays.count == 1 {
                settling?.cancel()
                settling = nil
                chooseDisplay()
            }
        case .streamingStarted:
            if let notice = pendingNotice {
                pendingNotice = nil
                pipelineRelaunched = false
                showHint(.notice(notice), after: 0, for: 4)
            } else if pipelineRelaunched {
                pipelineRelaunched = false
                showHint(.notice("L'image a été relancée."), after: 0, for: 4)
            }
            guard revealedAt == nil else {
                phase = .streaming
                return
            }
            DispatchQueue.main.asyncAfter(deadline: .now() + Self.revealDelay) { [weak self] in
                guard let self, self.revealedAt == nil, self.kyber != nil else { return }
                self.revealedAt = Date()
                self.phase = .streaming
                self.scene.open()
                withAnimation(Theme.easeOut) { self.entering = true }
                DispatchQueue.main.asyncAfter(deadline: .now() + 4.2) { [weak self] in
                    withAnimation(.easeIn(duration: 0.35)) { self?.entering = false }
                }
                self.showHint(after: 5)
                self.host.lastSession = Date()
                self.store.update(self.host)
                self.window?.makeFirstResponder(self.streamView)
            }
        case let .streamingFailed(reason), let .pipelineError(reason):
            // Juste après une reconnexion, l'écran virtuel a pu disparaître
            // (Kyber redémarré) : on le redemande, puis on retente une fois.
            if restoring != nil {
                KyberLog.app.notice("image refusée pendant que l'écran virtuel revient : on attend")
            } else if let reconnect = lastReconnect, Date().timeIntervalSince(reconnect) < 15, !retriedAfterReconnect {
                retriedAfterReconnect = true
                KyberLog.app.notice("image refusée après la reconnexion : nouvel essai sur l'écran à jour")
                restoreScreen()
            } else if !retryPipeline(reason) {
                fail(reason)
            }
        case let .connectFailed(reason):
            fail(reason)
        case let .reconnecting(attempt, _):
            // Une relance prévue tomberait sur la reconnexion : c'est
            // « reconnected » qui relancera l'image.
            pipelineRetry?.cancel()
            pipelineRetry = nil
            phase = .reconnecting(attempt)
        case let .reconnected(restart):
            pipelineRetry?.cancel()
            pipelineRetry = nil
            phase = .streaming
            lastReconnect = Date()
            retriedAfterReconnect = false
            if restart {
                // Kyber a redémarré sur l'ordinateur : l'écran virtuel a pu
                // partir avec lui. On le redemande à la console avant l'image.
                restoreScreen()
            } else if streaming == nil {
                // Image arrêtée par l'app juste avant la coupure (relance après
                // une image clé perdue) : Kyber ne la relance pas, l'app si.
                KyberLog.app.notice("reconnecté sans image : relance")
                chooseDisplay()
            } else if streamingFallback != fallback {
                // Repli décidé pendant la coupure : Kyber a relancé l'ancien flux.
                restartStreaming()
            }
        case .reconnectionFailed:
            fail("connexion perdue")
        case let .stopped(reason):
            if phase != .ending && phase != .ended { fail(reason) }
        case .streamingStopped:
            break
        }
    }

    /// Erreur de la chaîne d'image sur l'ordinateur, ou image refusée : on
    /// relance l'image (arrêt puis démarrage sur le meilleur écran de la
    /// liste à jour) une seconde après, trois fois au plus, avant d'abandonner.
    /// Faux s'il n'y a plus d'essai.
    private func retryPipeline(_ reason: String) -> Bool {
        if Date().timeIntervalSince(lastPipelineRetry) > 30 { pipelineRetries = 0 }
        guard kyber != nil else { return false }
        if pipelineRetry != nil { return true }
        guard pipelineRetries < 3 else { return false }
        pipelineRetries += 1
        lastPipelineRetry = Date()
        KyberLog.app.notice("\(reason, privacy: .public) : relance de l'image (\(self.pipelineRetries)/3)")
        pipelineRetry = Task { [weak self] in
            try? await Task.sleep(for: .seconds(1))
            guard let self, !Task.isCancelled else { return }
            self.pipelineRetry = nil
            // L'écran virtuel revient (restoreScreen) : c'est lui qui relancera l'image.
            guard self.kyber != nil, self.restoring == nil, self.phase != .ending, self.phase != .ended else { return }
            if case .failed = self.phase { return }
            // Pendant une reconnexion, c'est « reconnected » qui relancera l'image.
            if case .reconnecting = self.phase { return }
            guard self.bestDisplay() != nil else {
                self.fail(reason)
                return
            }
            self.pipelineRelaunched = true
            self.restartStreaming()
        }
        return true
    }

    /// Redemande l'écran virtuel à la console (sans effet s'il est toujours là),
    /// attend que Kyber l'annonce, puis relance l'image dessus. Sans écran
    /// virtuel, relance seulement l'image sur la liste d'écrans à jour.
    private func restoreScreen() {
        guard restoring == nil else { return }
        streaming = nil
        lastRestore = Date()
        restoring = Task { [weak self] in
            guard let self else { return }
            defer { self.restoring = nil }
            if self.prepared?.virtualDisplay == true, let console = self.console, let r = self.requested {
                KyberLog.app.notice("écran virtuel redemandé à la console")
                do {
                    _ = try await console.prepare(width: r.width, height: r.height, refresh: r.refresh, audioChannels: r.channels, virtualDisplay: true, profile: self.host.settings.profileLabel, modePolicy: self.host.settings.modePolicy)
                } catch {
                    KyberLog.app.error("écran virtuel pas rétabli : \(error.localizedDescription, privacy: .public)")
                }
                // Kyber annonce l'écran quelques instants après la console.
                for _ in 0..<30 where !self.virtualDisplayAnnounced {
                    try? await Task.sleep(for: .milliseconds(100))
                }
            } else {
                try? await Task.sleep(for: .milliseconds(600))
            }
            guard !Task.isCancelled, self.kyber != nil, self.phase != .ending, self.phase != .ended else { return }
            self.restoring = nil
            self.chooseDisplay()
        }
    }

    /// Kyber n'annonce plus que l'écran virtuel (les écrans physiques éteints),
    /// au mode obtenu de la console.
    private var virtualDisplayAnnounced: Bool {
        let size = preparedSize ?? (streamMode.width, streamMode.height)
        return displays.count == 1 && displays[0].width == size.width && displays[0].height == size.height
    }

    /// Mode obtenu de la console au début de la session.
    private var preparedSize: (width: Int, height: Int)? {
        guard let w = prepared?.width, let h = prepared?.height else { return nil }
        return (w, h)
    }

    // MARK: - Écran de ce Mac

    /// Les écrans changent par rafales (branchement, réveil, plein écran qui
    /// se recale) : on attend qu'ils se posent.
    private func screensChanged() {
        screenCheck?.cancel()
        screenCheck = Task { [weak self] in
            try? await Task.sleep(for: .milliseconds(800))
            guard let self, !Task.isCancelled else { return }
            self.screenCheck = nil
            self.refitWindowIfNeeded()
            self.checkScreenMode()
        }
    }

    /// Écran de la fenêtre débranché : macOS la passe sur un écran restant,
    /// mais une fenêtre en plein écran peut y garder la taille et la place de
    /// l'ancien. On sort du plein écran, on la pose sur l'écran principal et
    /// on y revient en plein écran. Fenêtre restée hors de tout écran : recentrée.
    private func refitWindowIfNeeded() {
        // Avant l'image, la fenêtre peut être encore en train de passer en plein écran.
        guard let window, !refitting, revealedAt != nil, phase != .ending, phase != .ended else { return }
        let screens = NSScreen.screens
        guard let main = NSScreen.main ?? screens.first else { return }
        if window.styleMask.contains(.fullScreen) {
            if let screen = window.screen, abs(window.frame.width - screen.frame.width) < 2,
               screen.frame.insetBy(dx: -1, dy: -1).contains(window.frame) { return }
            refitting = true
            KyberLog.app.notice("plein écran sur un écran débranché : retour en plein écran sur l'écran restant")
            windowDelegate.onExitFullScreen = { [weak self] in
                guard let self else { return }
                self.windowDelegate.onExitFullScreen = nil
                let area = (NSScreen.main ?? main).visibleFrame
                self.window?.setFrame(area.insetBy(dx: area.width * 0.1, dy: area.height * 0.1), display: true)
                DispatchQueue.main.async { [weak self] in
                    guard let self else { return }
                    self.refitting = false
                    guard self.phase != .ending, self.phase != .ended else { return }
                    if self.window?.styleMask.contains(.fullScreen) == false { self.window?.toggleFullScreen(nil) }
                }
            }
            window.toggleFullScreen(nil)
            // Sortie du plein écran refusée (transition en cours) : on n'attend pas sans fin.
            DispatchQueue.main.asyncAfter(deadline: .now() + 4) { [weak self] in
                guard let self, self.refitting else { return }
                self.refitting = false
                self.windowDelegate.onExitFullScreen = nil
            }
        } else if !screens.contains(where: { $0.frame.intersects(window.frame) }) {
            var frame = window.frame
            frame.size.width = min(frame.width, main.visibleFrame.width)
            frame.size.height = min(frame.height, main.visibleFrame.height)
            frame.origin = CGPoint(x: main.visibleFrame.midX - frame.width / 2, y: main.visibleFrame.midY - frame.height / 2)
            window.setFrame(frame, display: true)
        }
    }

    /// « Comme cet écran » : si le mode de l'écran qui montre la session
    /// (définition réelle, fréquence) n'est plus celui demandé à la console,
    /// on propose d'adapter l'image ; revenu au mode demandé, la proposition part.
    private func checkScreenMode() {
        guard phase == .streaming, restoring == nil else { return }
        guard host.settings.display == .match, prepared?.virtualDisplay == true, let r = requested,
              let screen = window?.screen ?? NSScreen.main else {
            clearScreenOffer()
            return
        }
        let m = DisplayInfo.mode(for: screen, codec: host.settings.codec, limit4K: host.settings.limit4K)
        let mode = ScreenMode(width: m.width, height: m.height, refresh: m.refresh)
        guard mode != ScreenMode(width: r.width, height: r.height, refresh: r.refresh) else {
            clearScreenOffer()
            return
        }
        guard mode != screenOffer else { return }
        KyberLog.app.notice("écran de ce Mac : \(mode.width)×\(mode.height) à \(mode.refresh) Hz, l'ordinateur est en \(r.width)×\(r.height) à \(r.refresh) Hz : adaptation proposée")
        screenOffer = mode
        screenOfferHide?.cancel()
        withAnimation(Theme.easeOut) { screenOfferShown = true }
        // La capsule s'efface ; la puce reste dans la barre.
        screenOfferHide = Task { [weak self] in
            try? await Task.sleep(for: .seconds(15))
            guard !Task.isCancelled else { return }
            self?.dismissScreenOffer()
        }
    }

    private func clearScreenOffer() {
        screenOfferHide?.cancel()
        screenOffer = nil
        if screenOfferShown { withAnimation(.easeIn(duration: 0.25)) { screenOfferShown = false } }
        screenOfferRect = .zero
    }

    /// « Plus tard » : la capsule part, la puce reste dans la barre.
    func dismissScreenOffer() {
        screenOfferHide?.cancel()
        withAnimation(.easeIn(duration: 0.25)) { screenOfferShown = false }
        screenOfferRect = .zero
    }

    /// « Adapter » : la console refait l'écran virtuel au mode de cet écran
    /// (`prepare`), puis l'image repart dessus, comme après une reconnexion
    /// (attente que Kyber l'annonce, choix de l'écran, démarrage du flux).
    func adaptToScreen() {
        guard let offer = screenOffer, let console, let previous = requested, restoring == nil, phase == .streaming else { return }
        clearScreenOffer()
        requested = (offer.width, offer.height, offer.refresh, previous.channels)
        KyberLog.app.notice("adaptation à l'écran de ce Mac : écran virtuel redemandé en \(offer.width)×\(offer.height) à \(offer.refresh) Hz")
        showHint(.notice("Adaptation de l'image à \(offer.width) × \(offer.height)…"), after: 0, for: 3)
        kyber?.stopStreaming()
        streaming = nil
        lastRestore = Date()
        restoring = Task { [weak self] in
            guard let self else { return }
            defer { self.restoring = nil }
            do {
                let result = try await console.prepare(width: offer.width, height: offer.height, refresh: offer.refresh, audioChannels: previous.channels, virtualDisplay: true, profile: self.host.settings.profileLabel, modePolicy: self.host.settings.modePolicy)
                self.prepared = result
                let mode = (result.width ?? offer.width, result.height ?? offer.height, result.refresh ?? offer.refresh)
                self.streamMode = mode
                self.modeLabel = "\(mode.0) × \(mode.1) · \(mode.2) Hz"
                self.pendingNotice = "Image adaptée à \(mode.0) × \(mode.1)."
            } catch {
                // L'ancien mode reste celui à redemander (reconnexion, écran virtuel perdu).
                self.requested = previous
                KyberLog.app.error("adaptation refusée : \(error.localizedDescription, privacy: .public)")
                self.showHint(.fallback("L'écran de l'ordinateur n'a pas pu être adapté : \(error.localizedDescription)"), after: 0, for: 6)
            }
            // Kyber annonce l'écran recréé quelques instants après la console.
            for _ in 0..<60 where !self.virtualDisplayAnnounced {
                try? await Task.sleep(for: .milliseconds(100))
            }
            guard !Task.isCancelled, self.kyber != nil, self.phase != .ending, self.phase != .ended else { return }
            self.restoring = nil
            self.settleUntil = nil
            self.refreshModeOffer()
            self.chooseDisplay()
            // L'écran a pu changer encore pendant l'adaptation.
            self.checkScreenMode()
        }
    }

    // MARK: - Mode de l'écran de l'ordinateur

    /// L'écran affiché a changé de taille (repère des entrées déjà recalé par
    /// KyberClient) : la barre et le débit suivent, la capsule le dit.
    private func hostSizeChanged(width: Int, height: Int) {
        streamMode = (width, height, streamMode.refresh)
        modeLabel = "\(width) × \(height) · \(streamMode.refresh) Hz"
        if let shown = display, shown.width != width || shown.height != height {
            display = shown.resized(width: width, height: height)
        }
        refreshModeOffer()
        let back = preparedSize.map { $0 == (width, height) } ?? false
        var text = back ? "L'ordinateur est revenu en \(width) × \(height)." : "L'ordinateur est passé en \(width) × \(height)."
        if let offer = modeRestoreOffer { text += " « \(offer) » dans la barre." }
        showHint(.notice(text), after: 0, for: 5)
    }

    /// Profil Bureau avec l'écran virtuel : remettre la taille de ce Mac.
    private func refreshModeOffer() {
        guard host.settings.profile == .bureau, prepared?.virtualDisplay == true, let size = preparedSize,
              size != (streamMode.width, streamMode.height) else {
            modeRestoreOffer = nil
            return
        }
        modeRestoreOffer = "Remettre \(size.width) × \(size.height)"
    }

    /// Politique « remettre » (profil Bureau, ou réglage de la console) : la
    /// console remet elle-même le mode. Si le signal de vie dit encore un
    /// autre mode (console d'avant cette politique, remise refusée), l'app le
    /// redemande, au plus une fois toutes les 30 s. En « suivre », rien.
    private func checkHostMode(_ beat: ConsoleClient.Beat) {
        guard (beat.modePolicy ?? host.settings.modePolicy) == "restore", prepared?.virtualDisplay == true,
              phase == .streaming, restoring == nil, beat.displayActive == true,
              let mode = beat.mode, let size = preparedSize, (mode.width, mode.height) != size,
              Date().timeIntervalSince(lastModeRestore) > 30 else { return }
        lastModeRestore = Date()
        KyberLog.app.notice("signal de vie : écran de l'ordinateur en \(mode.width)×\(mode.height), \(size.width)×\(size.height) redemandé")
        restoreMode()
    }

    /// « Remettre » : la console remet le mode demandé au début de la session.
    func restoreMode() {
        guard let console, let r = requested, modeRestore == nil else { return }
        modeRestoreOffer = nil
        KyberLog.app.notice("mode de l'ordinateur : retour à \(r.width)×\(r.height) demandé")
        modeRestore = Task { [weak self] in
            defer { self?.modeRestore = nil }
            // Session finie entre-temps : un prepare recréerait l'écran virtuel.
            guard let self, !Task.isCancelled, self.phase == .streaming else { return }
            do {
                _ = try await console.prepare(width: r.width, height: r.height, refresh: r.refresh, audioChannels: r.channels, virtualDisplay: true, profile: self.host.settings.profileLabel, modePolicy: "restore")
                // La console peut répondre sans avoir rien remis (mode refusé,
                // jeu qui insiste) : si l'image n'est pas revenue, le bouton revient.
                try? await Task.sleep(for: .seconds(6))
            } catch {
                KyberLog.app.error("mode de l'ordinateur pas remis : \(error.localizedDescription, privacy: .public)")
            }
            guard !Task.isCancelled else { return }
            self.refreshModeOffer()
        }
    }

    /// Avec l'écran virtuel, Kyber doit finir par n'annoncer qu'un écran :
    /// on attend que sa liste se stabilise (6 s au plus) avant de démarrer.
    private func chooseDisplay() {
        // Une seule attente de 6 s : si la liste ne se réduit pas à un écran
        // d'ici là, on prend le meilleur (avant, l'attente se relançait sans fin).
        if prepared?.virtualDisplay == true, usableDisplays.count != 1, settling == nil, streaming == nil {
            let until = settleUntil ?? Date().addingTimeInterval(6)
            settleUntil = until
            if until > Date() {
                phase = .starting
                settling = Task { [weak self] in
                    try? await Task.sleep(for: .seconds(max(0.1, until.timeIntervalSinceNow)))
                    guard !Task.isCancelled, let self else { return }
                    self.settling = nil
                    self.chooseDisplay()
                }
                return
            }
        }
        settleUntil = nil
        guard let best = bestDisplay() else {
            // Windows ne présente que des écrans de 0 × 0 quand aucun n'est
            // allumé sur le bureau (écrans physiques pas revenus, écran virtuel absent).
            KyberLog.app.error("aucun écran utilisable : \(self.displays.map { "\($0.id) \($0.width)×\($0.height)" }.joined(separator: ", "), privacy: .public)")
            fail(displays.isEmpty ? "l'ordinateur n'a annoncé aucun écran" : "aucun écran de l'ordinateur n'est allumé : rallumez ses écrans ou redémarrez-le")
            return
        }
        if streaming == best.id { return }
        display = best
        startStreaming()
    }

    /// Écrans annoncés avec une taille (Windows liste aussi des sorties éteintes, en 0 × 0).
    private var usableDisplays: [KyberDisplay] { displays.filter { $0.width > 0 && $0.height > 0 } }

    private func bestDisplay() -> KyberDisplay? {
        let displays = usableDisplays
        if let picked = pickedDisplay, let d = displays.first(where: { $0.id == picked }) { return d }
        if displays.count == 1 { return displays[0] }
        // L'écran affiché revenu sous un autre numéro (câble rebranché, écran
        // recréé) : même place et même taille. Kyber connaît le nom de l'écran
        // mais kyclient ne le transmet pas (kyclient_Display : numéro, taille,
        // position), d'où la géométrie.
        if let last = display, let same = displays.first(where: { $0.x == last.x && $0.y == last.y && $0.width == last.width && $0.height == last.height }) {
            return same
        }
        let sameMode = displays.filter { $0.width == prepared?.width && $0.height == prepared?.height }
        // L'écran virtuel est placé en haut à gauche du bureau.
        return sameMode.first { $0.x == 0 && $0.y == 0 } ?? sameMode.first ?? displays.first { $0.x == 0 && $0.y == 0 } ?? displays.first
    }

    private func startStreaming() {
        guard let kyber, var display else { return }
        // Pendant une reconnexion, c'est « reconnected » qui relance l'image.
        if case .reconnecting = phase { return }
        // Jamais un numéro absent de la dernière liste (refusé, 403) : la
        // liste a pu changer depuis le choix (relance différée de 0,3 s).
        if !displays.isEmpty, !usableDisplays.contains(where: { $0.id == display.id }) {
            guard let best = bestDisplay() else {
                fail("aucun écran de l'ordinateur n'est allumé : rallumez ses écrans ou redémarrez-le")
                return
            }
            KyberLog.app.notice("écran \(display.id) absent de la liste : écran \(best.id) à la place")
            display = best
            self.display = best
        }
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
            let options = streamOptions
            try kyber.startStreaming(display: display, view: streamView, options: options)
            // Chaîne d'entrées neuve : si la barre a la manette, elle la garde.
            kyber.setGamepadCaptured(padMode)
            streaming = display.id
            shownDisplay = display.id
            streamingFallback = fallback
            KyberLog.app.notice("flux : \(options.yuv444 ? "4:4:4" : "4:2:0", privacy: .public), \(options.bitrate / 1_000_000) Mb/s\(self.fallback > 0 ? " (repli \(self.fallback))" : "", privacy: .public)")
            watchForImage(native: options.nativeVideo)
        } catch {
            fail(error.localizedDescription)
        }
    }

    /// Pas d'image 4 s après le démarrage du flux : on l'allège.
    private func watchForImage(native: Bool) {
        hasImage = false
        imageWatch?.cancel()
        // Avec VLC, l'app ne voit pas les images passer.
        guard native else { return }
        imageWatch = Task { [weak self] in
            try? await Task.sleep(for: .seconds(4))
            guard !Task.isCancelled, let self, !self.hasImage, self.phase == .streaming || self.phase == .starting else { return }
            KyberLog.app.error("aucune image en 4 s")
            self.degrade()
        }
    }

    /// Allège le flux d'un cran (débit, puis 4:2:0) et le relance. Faux s'il
    /// n'y a plus rien à alléger.
    @discardableResult
    private func degrade() -> Bool {
        let current = streamOptions
        var next = fallback + 1
        // Cran sans effet (débit déjà bas) : on passe au suivant.
        if next == 1, current.bitrate <= Self.fallbackBitrate { next = 2 }
        if next == 2, !current.yuv444 { next = 3 }
        guard next <= 2 else { return false }
        fallback = next
        // Le délai repart du prochain démarrage du flux.
        imageWatch?.cancel()
        KyberLog.app.error("pas d'image : repli \(next, privacy: .public) (\(next == 1 ? "débit \(Self.fallbackBitrate / 1_000_000) Mb/s" : "4:2:0", privacy: .public))")
        showHint(.fallback(next == 1 ? "L'image ne passait pas : débit ramené à \(Self.fallbackBitrate / 1_000_000) Mb/s pour cette session." : "L'image ne passait pas : 4:2:0 pour cette session."), after: 0, for: 6)
        lastStallRestart = Date()
        restartStreaming()
        return true
    }

    /// Sans écran virtuel : afficher un autre écran de l'ordinateur.
    var canPickScreen: Bool { prepared?.virtualDisplay != true && screens.count > 1 }

    func showScreen(_ id: UInt32) {
        guard id != streaming else { return }
        pickedDisplay = id
        restartStreaming()
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
                guard let self else { return }
                let latency = self.stats?.latencyMs
                let profile = self.host.settings.profileLabel
                guard let beat = await self.console?.heartbeat(latencyMs: latency, profile: profile, modePolicy: self.host.settings.modePolicy) else { continue }
                // La console a demandé la fin de la session.
                if beat.end {
                    KyberLog.app.notice("fin de session demandée par la console")
                    self.end()
                    return
                }
                // L'écran virtuel a disparu (console redémarrée, par exemple) : on le redemande.
                if beat.displayActive == false, self.prepared?.virtualDisplay == true, self.phase == .streaming,
                   self.restoring == nil, Date().timeIntervalSince(self.lastRestore) > 30 {
                    KyberLog.app.notice("la console n'a plus d'écran virtuel")
                    self.restoreScreen()
                }
                self.checkHostMode(beat)
            }
        }
    }

    /// Fin de session : l'image se referme, la console rallume les écrans du
    /// PC et rétablit son son, l'app affiche ce qui a été vérifié.
    func end() {
        guard phase != .ending && phase != .ended else { return }
        setBar(false)
        // Latence médiane de la session, donnée à la console (ses appareils).
        let sessionLatency = kyber?.sessionLatencyMs
        // La dernière image : gardée pour l'accueil, et figée sous le passage qui se referme.
        let last = kyber?.snapshot(maxWidth: 1600)
        if let last { store.imagery.saveLast(last, for: host) }
        if let revealedAt { host.lastDuration = Date().timeIntervalSince(revealedAt) }
        // Les moitiés se referment, puis l'accueil revient tout de suite ; la
        // console vérifie la remise en état en arrière-plan (onReport).
        scene.close(over: revealedAt != nil ? last : nil, home: homeImage) { [weak self] in self?.finish() }
        withAnimation(.easeInOut(duration: 0.3)) { phase = .ending }
        heartbeat?.cancel()
        foregroundPoll?.cancel()
        pipelineRetry?.cancel()
        modeRestore?.cancel()
        clearScreenOffer()
        streamView.detach()
        // Latence médiane de la session, montrée sur l'accueil.
        if let latency = kyber?.sessionLatencyMs {
            host.lastLatencyMs = latency
            host.lastLatencyP95Ms = kyber?.sessionLatencyP95Ms
        }
        store.update(host)
        kyber?.stop()
        kyber = nil
        stats = nil
        audioStats = nil
        statsPanel = false
        statsCloseRect = .zero
        imageWatch?.cancel()
        let console = self.console
        Task {
            var report: EndReport?
            var problem: String?
            do {
                report = try await console?.end(latencyMs: sessionLatency)
            } catch {
                problem = error.localizedDescription
            }
            closing.displayRestored = report?.displayRestored ?? false
            closing.audioRestored = report?.audioRestored ?? false
            closing.error = problem ?? report?.error
            closing.done = true
            onReport?(host, closing)
        }
    }

    /// Résultat de la remise en état par la console, après le retour à l'accueil.
    var onReport: ((Host, Closing) -> Void)?

    private func showHint(_ which: Hint = .commands, after delay: TimeInterval = 1.6, for duration: TimeInterval = 3.4) {
        DispatchQueue.main.asyncAfter(deadline: .now() + delay) { [weak self] in
            withAnimation(.easeOut(duration: 0.35)) { self?.hint = which }
        }
        DispatchQueue.main.asyncAfter(deadline: .now() + delay + duration) { [weak self] in
            guard self?.hint == which else { return }
            withAnimation(.easeIn(duration: 0.35)) { self?.hint = nil }
        }
    }

    /// ⌃⌥⌘M : mode jeu si la souris est en mode bureau, et inversement.
    func toggleStatsPanel() {
        withAnimation(Theme.easeOut) { statsPanel.toggle() }
        if !statsPanel { statsCloseRect = .zero }
    }

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

    /// Retour à l'accueil : l'accueil reprend la fenêtre tout de suite (il est
    /// utilisable pendant que la fenêtre quitte le plein écran).
    func finish() {
        guard phase != .ended else { return }
        phase = .ended
        let wasFullScreen = enteredFullScreen && window?.styleMask.contains(.fullScreen) == true
        restoreHome()
        if wasFullScreen { window?.toggleFullScreen(nil) }
    }

    private func restoreHome() {
        windowDelegate.onExitFullScreen = nil
        windowDelegate.onClose = nil
        compositing = nil
        sceneHide?.cancel()
        chrome = nil
        if let occlusionObserver { NotificationCenter.default.removeObserver(occlusionObserver) }
        occlusionObserver = nil
        screenObservers.forEach { NotificationCenter.default.removeObserver($0) }
        screenObservers = []
        screenCheck?.cancel()
        screenOfferHide?.cancel()
        if let window {
            window.delegate = windowDelegate.original
            if let homeView { window.contentView = homeView }
            window.makeFirstResponder(nil)
        }
        homeView = nil
        onFinish(self)
    }

    private func fail(_ message: String) {
        KyberLog.app.error("session : \(message, privacy: .public)")
        heartbeat?.cancel()
        pipelineRetry?.cancel()
        modeRestore?.cancel()
        foregroundPoll?.cancel()
        clearScreenOffer()
        kyber?.stop()
        kyber = nil
        stats = nil
        audioStats = nil
        statsPanel = false
        statsCloseRect = .zero
        imageWatch?.cancel()
        setBar(false)
        scene.dim()
        phase = .failed(message)
        Task { try? await console?.end() }
    }

    func dismissFailure() {
        finish()
    }
}
