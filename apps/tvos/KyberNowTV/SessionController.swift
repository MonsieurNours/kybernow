// Une session avec un ordinateur, sur l'Apple TV : préparation par la
// console (écran virtuel à la taille du téléviseur, 4K à 60 Hz au plus, son
// au format de la sortie), connexion à Kyber, le passage, le menu (bouton
// Retour ou L1 + R1 + Start + Select), signal de vie, puis fin propre.
//
// Même déroulé que sur Mac (apps/macos/KyberNow/SessionController.swift) :
// reconnexion, écran virtuel redemandé, repli quand l'image ne passe pas.

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
    /// Capsule « Vous entrez dans… », quelques secondes après l'apparition.
    @Published private(set) var entering = false
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
    /// 3840 × 2160 » dans le menu.
    @Published private(set) var modeRestoreOffer: String?
    /// Du démarrage du flux à la première image affichée, en secondes.
    @Published private(set) var firstImageDelay: Double?
    private var streamStartedAt: Date?

    let streamView = StreamView(frame: CGRect(x: 0, y: 0, width: 1920, height: 1080))
    private let store: HostStore
    private let onFinish: (SessionController) -> Void
    private let prefs = Preferences.shared
    private var console: ConsoleClient?
    private var kyber: KyberClient?
    private var heartbeat: Task<Void, Never>?
    /// La console dit chaque seconde si un jeu est en plein écran (souris Bluetooth en brut).
    private var foregroundPoll: Task<Void, Never>?
    private var prepared: PrepareResult?
    private var display: KyberDisplay?
    private var displays: [KyberDisplay] = []
    private var streaming: UInt32?
    private var settling: Task<Void, Never>?
    /// Fin de l'attente d'une liste d'écrans stable (une seule attente par démarrage).
    private var settleUntil: Date?
    private var restoring: Task<Void, Never>?
    private var lastRestore = Date.distantPast
    private var requested: (width: Int, height: Int, refresh: Int, channels: Int)?
    private var streamMode = (width: 0, height: 0, refresh: 0)
    private var fallback = 0
    private static let fallbackBitrate: UInt32 = 150_000_000
    private var hasImage = false
    private var imageWatch: Task<Void, Never>?
    private var streamingFallback = 0
    private var lastStallRestart = Date.distantPast
    private var lastReconnect: Date?
    private var retriedAfterReconnect = false
    private var routeObserver: NSObjectProtocol?
    private static let revealDelay: TimeInterval = 0.9

    init(host: Host, store: HostStore, onFinish: @escaping (SessionController) -> Void) {
        self.host = host
        self.store = store
        self.onFinish = onFinish
        remoteMouse = host.settings.profile == .bureau || host.settings.mouse == .desktop
        padMouse = host.settings.profile == .bureau
    }

    private var streamOptions: StreamOptions {
        var o = host.settings.options(width: streamMode.width, height: streamMode.height, refresh: streamMode.refresh)
        // L'Apple TV ne lit pas l'AV1 en matériel : HEVC (ou H.264).
        if o.codec == .av1 { o.codec = DecoderCapabilities.current.hevc ? .hevc : .h264 }
        o.clipboard = false
        o.vsync = true
        if fallback >= 1 { o.bitrate = min(o.bitrate, Self.fallbackBitrate) }
        if fallback >= 2 { o.yuv444 = false }
        return o
    }

    // MARK: - Déroulé

    func start() {
        guard let password = store.password(for: host) else {
            fail("mot de passe introuvable dans le Trousseau")
            return
        }
        let settings = host.settings
        let mode = TVDisplay.mode(for: settings)
        streamMode = mode
        modeLabel = "\(mode.width) × \(mode.height) à \(mode.refresh) Hz"
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
        let console = ConsoleClient(host: host.address, port: host.consolePort, username: host.username, password: password, fingerprint: host.consoleFingerprint)
        self.console = console
        Task {
            do {
                phase = .preparing
                // Son : celui de la sortie (ampli 5.1 ou 7.1, sinon stéréo), ou resté sur l'ordinateur.
                let channels = settings.audio == .off ? 0 : settings.audio.channels
                requested = (mode.width, mode.height, mode.refresh, channels)
                let result = try await console.prepare(width: mode.width, height: mode.height, refresh: mode.refresh, audioChannels: channels, virtualDisplay: settings.display != .physical, profile: settings.profileLabel, modePolicy: settings.modePolicy)
                prepared = result
                if let w = result.width, let h = result.height, let hz = result.refresh {
                    streamMode = (w, h, hz)
                    modeLabel = "\(w) × \(h) à \(hz) Hz"
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

    private func connect(password: String) throws {
        guard let port = prepared?.kyberPort ?? host.streamPort else {
            throw ConsoleError.http(0, "port du flux inconnu")
        }
        phase = .connecting
        let kyber = try KyberClient(host: host.address, port: port, username: host.username, password: password, autoReconnect: host.settings.autoReconnect)
        kyber.onEvent = { [weak self] in self?.handle($0) }
        kyber.onVideoStats = { [weak self] stats in
            guard let self else { return }
            self.stats = stats
            if stats.fps > 0, !self.hasImage {
                self.hasImage = true
                self.imageWatch?.cancel()
                if self.firstImageDelay == nil, let t = self.streamStartedAt {
                    // Les mesures arrivent chaque seconde : on retire la moitié d'une fenêtre.
                    self.firstImageDelay = max(0.1, Date().timeIntervalSince(t) - 0.5)
                }
            }
        }
        kyber.onVideoStalled = { [weak self] in
            guard let self else { return }
            if !self.hasImage, self.degrade() { return }
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
               let now = list.first(where: { $0.id == shown.id }), now.width > 0, now.height > 0,
               now.width != shown.width || now.height != shown.height {
                display = now
                kyber?.displayResized(width: now.width, height: now.height)
            }
            if case .reconnecting = phase {
                if let streaming, !list.contains(where: { $0.id == streaming }) { display = bestDisplay() }
            } else if let streaming, !list.contains(where: { $0.id == streaming }) {
                restartStreaming()
            } else if settling != nil, usableDisplays.count == 1 {
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
                withAnimation(.timingCurve(0.2, 0.9, 0.1, 1, duration: 1.5)) {
                    self.revealedAt = Date()
                    self.phase = .streaming
                }
                self.startedAt = Date()
                withAnimation(Theme.easeOut.delay(0.7)) { self.entering = true }
                DispatchQueue.main.asyncAfter(deadline: .now() + 5.2) { [weak self] in
                    withAnimation(.easeIn(duration: 0.35)) { self?.entering = false }
                }
                self.host.lastSession = Date()
                self.store.update(self.host)
            }
        case let .streamingFailed(reason), let .pipelineError(reason):
            if restoring != nil {
                KyberLog.app.notice("image refusée pendant que l'écran virtuel revient : on attend")
            } else if let reconnect = lastReconnect, Date().timeIntervalSince(reconnect) < 15, !retriedAfterReconnect {
                retriedAfterReconnect = true
                restoreScreen()
            } else {
                fail(reason)
            }
        case let .connectFailed(reason):
            fail(reason)
        case let .reconnecting(attempt, _):
            phase = .reconnecting(attempt)
        case let .reconnected(restart):
            phase = .streaming
            lastReconnect = Date()
            retriedAfterReconnect = false
            if restart {
                restoreScreen()
            } else if streaming == nil {
                chooseDisplay()
            } else if streamingFallback != fallback {
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

    private func restoreScreen() {
        guard restoring == nil else { return }
        streaming = nil
        lastRestore = Date()
        restoring = Task { [weak self] in
            guard let self else { return }
            defer { self.restoring = nil }
            if self.prepared?.virtualDisplay == true, let console = self.console, let r = self.requested {
                _ = try? await console.prepare(width: r.width, height: r.height, refresh: r.refresh, audioChannels: r.channels, virtualDisplay: true, profile: self.host.settings.profileLabel, modePolicy: self.host.settings.modePolicy)
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

    private var virtualDisplayAnnounced: Bool {
        let size = preparedSize ?? (streamMode.width, streamMode.height)
        return displays.count == 1 && displays[0].width == size.width && displays[0].height == size.height
    }

    /// Mode obtenu de la console au début de la session.
    private var preparedSize: (width: Int, height: Int)? {
        guard let w = prepared?.width, let h = prepared?.height else { return nil }
        return (w, h)
    }

    // MARK: - Mode de l'écran de l'ordinateur

    /// L'écran affiché a changé de taille (repère des entrées déjà recalé par
    /// KyberClient) : l'étiquette et le débit suivent, la capsule le dit.
    private func hostSizeChanged(width: Int, height: Int) {
        streamMode = (width, height, streamMode.refresh)
        modeLabel = "\(width) × \(height) à \(streamMode.refresh) Hz"
        if let shown = display, shown.width != width || shown.height != height {
            display = shown.resized(width: width, height: height)
        }
        refreshModeOffer()
        let back = preparedSize.map { $0 == (width, height) } ?? false
        var text = back ? "L'ordinateur est revenu en \(width) × \(height)" : "L'ordinateur est passé en \(width) × \(height)"
        if let offer = modeRestoreOffer { text += " · « \(offer) » dans le menu" }
        show(notice: text)
    }

    /// Capsule en haut de l'écran pendant quelques secondes.
    private func show(notice text: String, for seconds: TimeInterval = 5) {
        withAnimation(.easeOut(duration: 0.35)) { notice = text }
        DispatchQueue.main.asyncAfter(deadline: .now() + seconds) { [weak self] in
            guard self?.notice == text else { return }
            withAnimation(.easeIn(duration: 0.35)) { self?.notice = nil }
        }
    }

    /// Profil Bureau avec l'écran virtuel : remettre la taille du téléviseur.
    private func refreshModeOffer() {
        guard host.settings.profile == .bureau, prepared?.virtualDisplay == true, let size = preparedSize,
              size != (streamMode.width, streamMode.height) else {
            modeRestoreOffer = nil
            return
        }
        modeRestoreOffer = "Remettre \(size.width) × \(size.height)"
    }

    /// « Remettre » : la console remet le mode demandé au début de la session.
    func restoreMode() {
        guard let console, let r = requested else { return }
        modeRestoreOffer = nil
        KyberLog.app.notice("mode de l'ordinateur : retour à \(r.width)×\(r.height) demandé")
        Task {
            do {
                _ = try await console.prepare(width: r.width, height: r.height, refresh: r.refresh, audioChannels: r.channels, virtualDisplay: true, profile: host.settings.profileLabel, modePolicy: "restore")
            } catch {
                KyberLog.app.error("mode de l'ordinateur pas remis : \(error.localizedDescription, privacy: .public)")
                refreshModeOffer()
            }
        }
    }

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
        if displays.count == 1 { return displays[0] }
        let sameMode = displays.filter { $0.width == prepared?.width && $0.height == prepared?.height }
        return sameMode.first { $0.x == 0 && $0.y == 0 } ?? sameMode.first ?? displays.first
    }

    private func startStreaming() {
        guard let kyber, let display else { return }
        if case .reconnecting = phase { return }
        if revealedAt == nil { phase = .starting }
        do {
            let options = streamOptions
            try kyber.startStreaming(display: display, view: streamView, options: options)
            if streamStartedAt == nil { streamStartedAt = Date() }
            streaming = display.id
            streamingFallback = fallback
            KyberLog.app.notice("flux : \(options.yuv444 ? "4:4:4" : "4:2:0", privacy: .public), \(options.bitrate / 1_000_000) Mb/s")
            watchForImage()
        } catch {
            fail(error.localizedDescription)
        }
    }

    private func watchForImage() {
        hasImage = false
        imageWatch?.cancel()
        imageWatch = Task { [weak self] in
            try? await Task.sleep(for: .seconds(4))
            guard !Task.isCancelled, let self, !self.hasImage, self.phase == .streaming || self.phase == .starting else { return }
            self.degrade()
        }
    }

    @discardableResult
    private func degrade() -> Bool {
        let current = streamOptions
        var next = fallback + 1
        if next == 1, current.bitrate <= Self.fallbackBitrate { next = 2 }
        if next == 2, !current.yuv444 { next = 3 }
        guard next <= 2 else { return false }
        fallback = next
        imageWatch?.cancel()
        fallbackNote = next == 1 ? "L'image ne passait pas : débit ramené à \(Self.fallbackBitrate / 1_000_000) Mbit/s pour cette session." : "L'image ne passait pas : 4:2:0 pour cette session."
        DispatchQueue.main.asyncAfter(deadline: .now() + 6) { [weak self] in self?.fallbackNote = nil }
        lastStallRestart = Date()
        restartStreaming()
        return true
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
                guard let beat = await self.console?.heartbeat(latencyMs: self.stats?.latencyMs, profile: self.host.settings.profileLabel) else { continue }
                if beat.end {
                    self.end()
                    return
                }
                if beat.displayActive == false, self.prepared?.virtualDisplay == true, self.phase == .streaming,
                   self.restoring == nil, Date().timeIntervalSince(self.lastRestore) > 30 {
                    self.restoreScreen()
                }
            }
        }
    }

    /// Comme le mode Auto du Mac : un jeu en plein écran sur l'ordinateur
    /// (pas un navigateur ni un lecteur vidéo) fait passer la souris Bluetooth en brut.
    private func startForegroundPoll() {
        foregroundPoll?.cancel()
        foregroundPoll = Task { [weak self] in
            while !Task.isCancelled {
                guard let self, let console = self.console else { return }
                let game = await console.foreground()?.game == true
                if game != self.streamView.pcGameFullscreen {
                    KyberLog.app.notice("ordinateur : \(game ? "jeu en plein écran" : "pas de jeu en plein écran", privacy: .public)")
                    self.streamView.pcGameFullscreen = game
                }
                try? await Task.sleep(for: .seconds(1))
            }
        }
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

    /// Profil changé dans le menu : débit en direct, 4:4:4 ou format en relançant l'image.
    func select(_ profile: Profile) {
        guard profile != host.settings.profile else { return }
        let before = streamOptions
        host.settings.apply(profile)
        store.update(host)
        refreshModeOffer()
        fallback = 0
        let after = streamOptions
        if before.yuv444 != after.yuv444 || before.codec != after.codec {
            restartStreaming()
        } else {
            kyber?.setBitrate(after.bitrate)
        }
    }

    var sessionMinutes: Int { Int(Date().timeIntervalSince(startedAt) / 60) }

    #if DEBUG
    /// Démonstration (simulateur) : le passage, puis le menu, sans ordinateur.
    func demo(menu: Bool) {
        phase = .starting
        streamView.startTestPattern()
        DispatchQueue.main.asyncAfter(deadline: .now() + 1.6) { [weak self] in
            guard let self else { return }
            withAnimation(.timingCurve(0.2, 0.9, 0.1, 1, duration: 1.5)) {
                self.revealedAt = Date()
                self.phase = .streaming
            }
            self.startedAt = Date().addingTimeInterval(-42 * 60)
            self.modeLabel = "3840 × 2160 à 60 Hz"
            var stats = VideoStats()
            stats.latencyMs = 9.1
            stats.fps = 60
            stats.mbps = 78
            stats.format = "4:2:0 8 bits"
            self.stats = stats
            self.firstImageDelay = 0.6
            if menu {
                DispatchQueue.main.asyncAfter(deadline: .now() + 0.5) { self.openMenu() }
            } else {
                withAnimation(Theme.easeOut.delay(0.7)) { self.entering = true }
            }
        }
    }
    #endif

    // MARK: - Fin

    func end() {
        guard phase != .ending && phase != .ended else { return }
        GamepadBridge.shared.onMenuRequest = nil
        streamView.padMouse = false
        let sessionLatency = kyber?.sessionLatencyMs
        if let last = kyber?.snapshot(maxWidth: 1920) { store.imagery.saveLast(last, for: host) }
        if let revealedAt { host.lastDuration = Date().timeIntervalSince(revealedAt) }
        if let latency = kyber?.sessionLatencyMs {
            host.lastLatencyMs = latency
            host.lastLatencyP95Ms = kyber?.sessionLatencyP95Ms
        }
        store.update(host)
        withAnimation(.easeInOut(duration: 0.4)) {
            phase = .ending
            menuOpen = false
        }
        heartbeat?.cancel()
        foregroundPoll?.cancel()
        imageWatch?.cancel()
        streamView.detach()
        kyber?.stop()
        kyber = nil
        matchFrameRate(nil)
        if let routeObserver { NotificationCenter.default.removeObserver(routeObserver) }
        let console = self.console
        Task {
            _ = try? await console?.end(latencyMs: sessionLatency)
        }
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
        heartbeat?.cancel()
        foregroundPoll?.cancel()
        imageWatch?.cancel()
        streamView.detach()
        kyber?.stop()
        kyber = nil
        matchFrameRate(nil)
        withAnimation(Theme.easeOut) {
            menuOpen = false
            phase = .failed(message)
        }
        Task { try? await console?.end() }
    }
}
