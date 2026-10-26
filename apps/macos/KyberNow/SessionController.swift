// Une session avec un ordinateur, vue de l'interface : la fenêtre (plein
// écran, passage, composition), la barre au survol, la manette qui la
// pilote, les capsules, les mesures. Le déroulé lui-même (préparation par la
// console, connexion à Kyber, choix de l'écran, relances, reconnexions,
// signal de vie, fin vérifiée) est dans le cœur Rust, par `SessionEngine`
// (docs/coeur-apple.md) : le contrôleur lui donne les réglages et la vue,
// et traduit ses états, ses avis et ses mesures pour SwiftUI.
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
    /// Mode de l'écran de la session (celui obtenu de l'ordinateur).
    private var streamMode = (width: 0, height: 0, refresh: 0)
    /// Mode demandé à la console (celui de ce Mac) : « Comme cet écran » le
    /// compare à l'écran qui montre la session.
    private var requested: ScreenMode?
    /// La session a un écran virtuel (choix d'écran impossible dans la barre).
    private var virtualDisplay = true
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
    /// Souris en mode jeu en ce moment.
    @Published private(set) var mouseCaptured = false
    /// Jeu en plein écran sur l'ordinateur, d'après la veille de la console.
    @Published private(set) var pcGame = false
    @Published private(set) var audioStats: AudioStats?
    /// Dernier recours de la connexion : la Station redémarre (« La Station
    /// redémarre… » à la place des étapes, jusqu'à la phase suivante).
    @Published private(set) var stationRestarting = false
    /// Rappel affiché quelques secondes, sur un événement de la session (jamais à l'arrivée de l'image).
    enum Hint: Equatable { case mouse, fallback(String), notice(String) }
    /// Profil Bureau, écran de l'ordinateur passé à un autre mode : « Remettre
    /// 3840 × 2160 » dans la barre (le cœur le propose et le retire).
    @Published private(set) var modeRestoreOffer: String?
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
    /// Mode demandé avant une adaptation, à reprendre si elle est refusée.
    private var requestedBeforeAdapt: ScreenMode?

    /// La session s'est arrêtée sans « Se déconnecter » : Admin ou un autre
    /// appareil y a mis fin, ou une erreur (`endError`). Dit par une
    /// notification quand l'app n'est pas au premier plan.
    private(set) var endedByComputer = false
    private(set) var endError: String?
    /// Appelé une fois la session finie dans le cœur (`ended`) : Admin a dit la remise en état.
    var onEnded: ((SessionController) -> Void)?

    /// La session tient encore (ni finie, ni en échec).
    var isLive: Bool {
        switch phase {
        case .ended, .failed: return false
        default: return true
        }
    }

    private let store: HostStore
    private let onFinish: (SessionController) -> Void

    private let streamView = StreamView(frame: .zero)
    private let scene = PassageScene(frame: .zero)
    /// L'accueil, retiré de la fenêtre le temps de la session, et sa photographie.
    private var homeView: NSView?
    private var homeImage: CGImage?
    private let windowDelegate = SessionWindowDelegate()
    /// La session a mis la fenêtre en plein écran : `finish()` l'en fait sortir.
    private(set) var enteredFullScreen = false
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
    private var engine: SessionEngine?
    /// Le cœur dit l'image en cours (l'apparition attend `revealDelay`).
    private var engineStreaming = false
    private var hideBar: Task<Void, Never>?
    /// Écrans de l'ordinateur, pour le choix dans la barre (sans écran virtuel).
    @Published private(set) var screens: [KyberDisplay] = []
    /// Écran affiché en ce moment.
    @Published private(set) var shownDisplay: UInt32?

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
        // Démonstration : la fenêtre reste telle quelle, pour la capturer.
        if host.settings.display != .window, !window.styleMask.contains(.fullScreen), !AppModel.demo {
            enteredFullScreen = true
            window.toggleFullScreen(nil)
        }
    }

    /// Pendant le flux, seule la couche Metal doit rester dans la composition
    /// de la fenêtre pour que l'image aille directement à l'écran : la couche
    /// SwiftUI est cachée tant qu'elle n'a rien à montrer (barre, manette,
    /// rappel, panneau des mesures, fin), et le passage une fois
    /// ouvert (son animation dure 1,1 s). Hors flux, tout revient.
    private func updateCompositing() {
        let streaming = phase == .streaming
        let needed = !streaming || barVisible || padMode || hint != nil || statsPanel || screenOfferShown
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
    /// plus de rendu (`nextDrawable` y attendrait jusqu'à 1 s par image) et débit plafonné
    /// à 20 Mb/s ; au retour, rendu et débit du profil reviennent.
    private func occlusionChanged() {
        guard let window else { return }
        let hidden = !window.occlusionState.contains(.visible)
        guard hidden != occluded else { return }
        occluded = hidden
        engine?.setOccluded(hidden)
        engine?.setBitrateCap(hidden ? Self.occludedBitrate : nil)
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
            engine?.setGamepadCaptured(false)
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
            engine?.setGamepadCaptured(true)
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

    /// Change de profil en cours de session : le cœur change le débit en
    /// direct, ou relance l'image si le 4:4:4, le format ou le transport
    /// changent (une seconde environ) ; le repli de la session repart de zéro.
    func select(_ profile: Profile) {
        guard profile != host.settings.profile else { return }
        host.settings.apply(profile)
        store.update(host)
        engine?.setSettings(engineSettings)
        engine?.setVSync(host.settings.vsync)
    }

    /// Les réglages de l'image, pour le cœur (il calcule le débit du mode).
    private var engineSettings: SessionEngine.Settings {
        let s = host.settings
        return SessionEngine.Settings(
            profile: s.profile?.rawValue,
            codec: s.codec,
            yuv444: s.yuv444 && ConnectionSettings.decodes444(s.codec),
            mbps: s.bitrateMbps,
            reliable: s.transport == .reliable
        )
    }

    // MARK: - Déroulé

    func start() {
        openWindow()
        guard let password = store.password(for: host) else {
            fail(String(localized: "mot de passe introuvable dans le Trousseau"))
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
        requested = ScreenMode(width: mode.width, height: mode.height, refresh: mode.refresh)
        virtualDisplay = settings.display != .physical

        streamView.commandAsControl = settings.commandAsControl
        streamView.mouseMode = settings.mouse.streamMode
        streamView.onRelativeChanged = { [weak self] on in
            guard let self else { return }
            self.mouseCaptured = on
            // Premier passage en mode jeu : on dit comment reprendre la souris.
            guard on, !self.mouseHintShown else { return }
            self.mouseHintShown = true
            self.showHint(.mouse, after: 0)
        }

        let engine = SessionEngine(view: streamView)
        engine.onPhase = { [weak self] in self?.phaseChanged($0) }
        engine.onNotice = { [weak self] in self?.notice($0) }
        engine.onDisplays = { [weak self] in self?.screens = $0 }
        engine.onShownDisplay = { [weak self] in self?.shownDisplay = $0.id }
        engine.onMode = { [weak self] mode, virtualDisplay in
            guard let self else { return }
            self.streamMode = (mode.width, mode.height, mode.refresh)
            self.modeLabel = "\(mode.width) × \(mode.height) · \(mode.refresh) Hz"
            self.virtualDisplay = virtualDisplay
        }
        engine.onVideoStats = { [weak self] stats in
            guard let self else { return }
            self.stats = stats
            if let ms = stats.latencyMs {
                self.latencyHistory.append(ms)
                if self.latencyHistory.count > 40 { self.latencyHistory.removeFirst() }
            }
            self.statsHistory.append(stats)
            if self.statsHistory.count > 30 { self.statsHistory.removeFirst() }
            self.sessionLosses.frames += stats.lostFrames
            self.sessionLosses.packets += stats.lostPackets
        }
        engine.onAudioStats = { [weak self] in self?.audioStats = $0 }
        engine.onGamepadCombo = { [weak self] in self?.gamepadCombo() }
        engine.onGamepadNavigate = { [weak self] in self?.gamepadNavigate($0) }
        engine.onGameFullscreen = { [weak self] game in
            guard let self, game != self.pcGame else { return }
            KyberLog.app.notice("ordinateur : \(game ? "jeu en plein écran" : "pas de jeu en plein écran", privacy: .public)")
            self.pcGame = game
        }
        self.engine = engine

        var params = SessionEngine.Params(
            host: host.address, consolePort: host.consolePort, username: host.username, password: password,
            consoleFingerprint: host.consoleFingerprint,
            mode: SessionEngine.Mode(width: mode.width, height: mode.height, refresh: mode.refresh),
            audioChannels: settings.audio.channels, virtualDisplay: virtualDisplay, settings: engineSettings
        )
        params.autoReconnect = settings.autoReconnect
        params.gamepadAsXbox360 = settings.gamepad == .xbox360
        params.clipboard = settings.clipboard
        params.vsync = settings.vsync
        KyberLog.app.notice("session : \(mode.width)×\(mode.height) à \(mode.refresh) Hz, son \(settings.audio.channels) canaux, \(settings.consoleProfile, privacy: .public)")
        do {
            try engine.start(params)
        } catch {
            fail(error.localizedDescription)
        }
    }

    /// Les états du cœur : l'image n'apparaît qu'après `revealDelay`, les
    /// fins voulues et les fins en erreur se séparent ici.
    private func phaseChanged(_ p: SessionEngine.Phase) {
        if case .failed = phase { return }
        engineStreaming = p == .streaming
        if p != .connecting { stationRestarting = false }
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
                // Dès la première image, l'image seule : ni capsule, ni barre, ni chiffre.
                self.revealedAt = Date()
                self.phase = .streaming
                self.scene.open()
                self.window?.makeFirstResponder(self.streamView)
                // L'écran de ce Mac a pu changer pendant la connexion.
                self.checkScreenMode()
            }
        case let .reconnecting(attempt, _):
            phase = .reconnecting(attempt)
        case .ending:
            // Fin demandée par la console (ou connexion perdue sans reprise) :
            // le passage se referme comme pour une fin voulue.
            if phase != .ending, phase != .ended {
                endedByComputer = true
                end()
            }
        case let .ended(error, report):
            if phase != .ending, phase != .ended { endedByComputer = true }
            endError = error
            defer { onEnded?(self) }
            if let error, phase != .ending, phase != .ended {
                fail(error)
                return
            }
            if phase != .ending, phase != .ended { end() }
            closing.displayRestored = report?.displayRestored ?? false
            closing.audioRestored = report?.audioRestored ?? false
            closing.error = error ?? report?.error
            closing.done = true
            onReport?(host, closing)
        }
    }

    /// Les avis du cœur, en capsules.
    private func notice(_ n: SessionEngine.Notice) {
        switch n {
        case .relaunched:
            showHint(.notice(String(localized: "L'image a été relancée.")), after: 0, for: 4)
        case let .modeChanged(width, height, back):
            streamMode = (width, height, streamMode.refresh)
            modeLabel = "\(width) × \(height) · \(streamMode.refresh) Hz"
            var text = back ? String(localized: "L'ordinateur est revenu en \(width) × \(height).") : String(localized: "L'ordinateur est passé en \(width) × \(height).")
            if let offer = modeRestoreOffer { text += " " + String(localized: "« \(offer) » dans la barre.") }
            showHint(.notice(text), after: 0, for: 5)
        case let .fallback(level, mbps):
            showHint(.fallback(level == 1 ? String(localized: "L'image ne passait pas : débit ramené à \(mbps) Mbit/s pour cette session.") : String(localized: "L'image ne passait pas : 4:2:0 pour cette session.")), after: 0, for: 6)
        case let .adapting(width, height):
            showHint(.notice(String(localized: "Adaptation de l'image à \(width) × \(height)…")), after: 0, for: 3)
        case let .adapted(width, height):
            requestedBeforeAdapt = nil
            showHint(.notice(String(localized: "Image adaptée à \(width) × \(height).")), after: 0, for: 4)
            // L'écran a pu changer encore pendant l'adaptation.
            checkScreenMode()
        case let .adaptFailed(message):
            // L'ancien mode reste celui demandé (reconnexion, écran virtuel perdu).
            if let previous = requestedBeforeAdapt { requested = previous }
            requestedBeforeAdapt = nil
            showHint(.fallback(String(localized: "L'écran de l'ordinateur n'a pas pu être adapté : \(message)")), after: 0, for: 6)
        case let .restoreOffer(mode):
            modeRestoreOffer = mode.map { String(localized: "Remettre \($0.width) × \($0.height)") }
        case .stationRestarting:
            stationRestarting = true
        }
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
        guard phase == .streaming, requestedBeforeAdapt == nil else { return }
        guard host.settings.display == .match, virtualDisplay, let r = requested,
              let screen = window?.screen ?? NSScreen.main else {
            clearScreenOffer()
            return
        }
        let m = DisplayInfo.mode(for: screen, codec: host.settings.codec, limit4K: host.settings.limit4K)
        let mode = ScreenMode(width: m.width, height: m.height, refresh: m.refresh)
        guard mode != r else {
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

    /// « Adapter » : le cœur fait refaire l'écran virtuel au mode de cet écran
    /// par la console, puis relance l'image dessus (avis « adaptation »,
    /// « adaptée » ou « refusée »).
    func adaptToScreen() {
        guard let offer = screenOffer, let engine, phase == .streaming, requestedBeforeAdapt == nil else { return }
        clearScreenOffer()
        requestedBeforeAdapt = requested
        requested = offer
        KyberLog.app.notice("adaptation à l'écran de ce Mac : \(offer.width)×\(offer.height) à \(offer.refresh) Hz demandé")
        engine.setMode(SessionEngine.Mode(width: offer.width, height: offer.height, refresh: offer.refresh))
    }

    // MARK: - Mode et écrans de l'ordinateur

    /// « Remettre » : la console remet le mode demandé au début de la session.
    func restoreMode() {
        modeRestoreOffer = nil
        engine?.restoreMode()
    }

    /// Sans écran virtuel : afficher un autre écran de l'ordinateur.
    var canPickScreen: Bool { !virtualDisplay && screens.count > 1 }

    func showScreen(_ id: UInt32) {
        guard id != shownDisplay else { return }
        engine?.pickDisplay(id)
    }

    // MARK: - Fin

    /// Fin de session : l'image se referme, la console rallume les écrans de
    /// l'ordinateur et rétablit son son, l'app affiche ce qui a été vérifié.
    func end() {
        guard phase != .ending && phase != .ended else { return }
        setBar(false)
        // La dernière image : gardée pour l'accueil, et figée sous le passage qui se referme.
        let last = engine?.snapshot(maxWidth: 1600)
        if let last { store.imagery.saveLast(last, for: host) }
        recordSession()
        // Les moitiés se referment, puis l'accueil revient tout de suite ; la
        // console vérifie la remise en état en arrière-plan (onReport).
        scene.close(over: revealedAt != nil ? last : nil, home: homeImage) { [weak self] in self?.finish() }
        withAnimation(.easeInOut(duration: 0.3)) { phase = .ending }
        clearScreenOffer()
        streamView.detach()
        store.update(host)
        engine?.stop()
        stats = nil
        audioStats = nil
        statsPanel = false
        statsCloseRect = .zero
        #if DEBUG
        // Démonstration, sans cœur derrière : le bilan d'Admin un instant après.
        if engine == nil, AppModel.demo {
            DispatchQueue.main.asyncAfter(deadline: .now() + 1) { [weak self] in
                guard let self else { return }
                self.closing.done = true
                self.onEnded?(self)
            }
        }
        #endif
    }

    /// Dernière session (rubrique Statistiques) : médiane et 95e centile de
    /// la latence (le cœur les donne aussi à la console), durée, mode, format
    /// et débit demandés ; gardée si elle a duré au moins 3 s.
    private func recordSession() {
        guard let revealedAt else { return }
        let m = streamMode
        host.recordSession(start: revealedAt, latency: engine?.sessionLatency, width: m.width, height: m.height, refresh: m.refresh,
                           codec: host.settings.codec, mbps: host.settings.bitrate(width: m.width, height: m.height, refresh: m.refresh))
    }

    /// Résultat de la remise en état par la console, après le retour à l'accueil.
    var onReport: ((Host, Closing) -> Void)?

    private func showHint(_ which: Hint, after delay: TimeInterval = 1.6, for duration: TimeInterval = 3.4) {
        DispatchQueue.main.asyncAfter(deadline: .now() + delay) { [weak self] in
            withAnimation(.easeOut(duration: 0.35)) { self?.hint = which }
        }
        DispatchQueue.main.asyncAfter(deadline: .now() + delay + duration) { [weak self] in
            guard self?.hint == which else { return }
            withAnimation(.easeIn(duration: 0.35)) { self?.hint = nil }
        }
    }

    /// ⌃⌥⌘S : panneau des mesures détaillées.
    func toggleStatsPanel() {
        withAnimation(Theme.easeOut) { statsPanel.toggle() }
        if !statsPanel { statsCloseRect = .zero }
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

    /// Session finie en erreur : l'image s'assombrit, le message reste jusqu'à
    /// « Fermer » ; le cœur a prévenu la console.
    private func fail(_ message: String) {
        KyberLog.app.error("session : \(message, privacy: .public)")
        // Une session coupée après l'image compte aussi.
        if revealedAt != nil {
            recordSession()
            store.update(host)
        }
        clearScreenOffer()
        engine?.stop()
        stats = nil
        audioStats = nil
        statsPanel = false
        statsCloseRect = .zero
        setBar(false)
        streamView.detach()
        scene.dim()
        phase = .failed(message)
    }

    func dismissFailure() {
        finish()
    }

    #if DEBUG
    /// Démonstration (`-demo YES -screen connecting|failed|session`) : la
    /// fenêtre de session sans ordinateur derrière, à l'étape demandée.
    func demo(_ screen: String) {
        openWindow()
        modeLabel = "2560 × 1440 · 120 Hz"
        switch screen {
        case "failed":
            phase = .connecting
            DispatchQueue.main.asyncAfter(deadline: .now() + 1.5) { [weak self] in
                self?.scene.dim()
                self?.phase = .failed(String(localized: "la Station ne répond pas"))
            }
        case "restarting":
            phase = .connecting
            stationRestarting = true
        case "session", "bar", "stats":
            phase = .starting
            DispatchQueue.main.asyncAfter(deadline: .now() + 1.5) { [weak self] in
                guard let self else { return }
                self.revealedAt = Date()
                self.phase = .streaming
                self.scene.open()
                if screen == "bar" { self.setBar(true) }
                if screen == "stats" { self.toggleStatsPanel() }
                self.demoStats()
            }
        default:
            phase = .connecting
        }
    }

    /// Démonstration : des mesures qui bougent, une fois par seconde (barre,
    /// menu de la barre des menus ; mode commun, comme les mesures du cœur
    /// qui arrivent par la file principale, aussi menu ouvert).
    private func demoStats() {
        let timer = Timer(timeInterval: 1, repeats: true) { [weak self] timer in
            MainActor.assumeIsolated {
                guard let self, self.phase == .streaming else { timer.invalidate(); return }
                var s = VideoStats()
                s.latencyMs = 11.5 + Double.random(in: 0 ... 1.6)
                s.fps = 60 + Double.random(in: -0.4 ... 0.4)
                s.mbps = 80 + Double.random(in: -3 ... 3)
                s.width = 2560
                s.height = 1440
                s.format = "4:2:0 8 bits"
                self.stats = s
                self.latencyHistory.append(s.latencyMs ?? 0)
                if self.latencyHistory.count > 40 { self.latencyHistory.removeFirst() }
            }
        }
        RunLoop.main.add(timer, forMode: .common)
    }

    /// Démonstration : fin venue de l'ordinateur (Admin, un autre appareil),
    /// ou erreur (`error`), comme le cœur les dit.
    func demoEndedByComputer(error: String?) {
        guard isLive else { return }
        endedByComputer = true
        endError = error
        end()
    }
    #endif
}
