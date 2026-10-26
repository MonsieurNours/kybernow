// Une session sur le cœur Rust (docs/coeur-apple.md) : le cœur prépare,
// connecte, choisit l'écran, relance l'image, reconnecte, finit ; Swift
// fournit ce qui dépend de l'appareil : la vue, le décodeur et le rendu
// Metal, le son, la chaîne d'entrées kynput, et reçoit états, mesures et
// avis. Mac, Apple TV, iPhone et iPad : la vue (StreamView) est celle de
// chaque plateforme.
//
// Les rappels du cœur arrivent sur ses fils (fil de session, fil réseau de
// Kyber pour les paquets) : ce qui touche l'interface remonte sur le fil
// principal, les paquets vont droit au décodeur.

#if os(macOS)
import AppKit
#else
import UIKit
#endif
import Foundation
@_implementationOnly import CKyber
@_implementationOnly import CKyberNow

public final class SessionEngine: @unchecked Sendable {
    /// Mode d'un écran : définition réelle et fréquence.
    public struct Mode: Equatable, Sendable {
        public var width: Int
        public var height: Int
        public var refresh: Int
        public init(width: Int, height: Int, refresh: Int) {
            self.width = width
            self.height = height
            self.refresh = refresh
        }
    }

    /// Réglages du flux, ceux d'un profil ou d'un réglage à la main.
    public struct Settings: Equatable, Sendable {
        /// `jeu`, `bureau`, `max` ; nil : personnalisé (le débit reste tel quel).
        public var profile: String?
        public var codec: VideoCodec
        public var yuv444: Bool
        /// Débit de base en Mbit/s (calé sur la 4K à 60 Hz pour un profil).
        public var mbps: Int
        public var reliable: Bool
        public init(profile: String?, codec: VideoCodec, yuv444: Bool, mbps: Int, reliable: Bool) {
            self.profile = profile
            self.codec = codec
            self.yuv444 = yuv444
            self.mbps = mbps
            self.reliable = reliable
        }

        var json: [String: Any] {
            var j: [String: Any] = ["codec": codec.rawValue, "yuv444": yuv444, "mbps": mbps, "reliable": reliable]
            if let profile { j["profile"] = profile }
            return j
        }
    }

    /// Multi-écrans (docs/multi-ecrans.md) : les écrans de cet appareil et
    /// ce qui décide de leur nombre ; le cœur en tire le nombre d'écrans de
    /// la session, au départ et quand le profil change.
    public struct DeviceScreens: Equatable, Sendable {
        public struct Screen: Equatable, Sendable {
            /// Cadre dans le repère logique de l'appareil (points), y vers le bas.
            public var x: Double
            public var y: Double
            public var width: Double
            public var height: Double
            /// Définition native en pixels (avant « Au plus 4K »).
            public var pixelWidth: Int
            public var pixelHeight: Int
            public var refresh: Int
            public var primary: Bool
            public init(x: Double, y: Double, width: Double, height: Double, pixelWidth: Int, pixelHeight: Int, refresh: Int, primary: Bool) {
                (self.x, self.y, self.width, self.height) = (x, y, width, height)
                (self.pixelWidth, self.pixelHeight, self.refresh, self.primary) = (pixelWidth, pixelHeight, refresh, primary)
            }
        }

        public var list: [Screen]
        /// « Cet écran » : l'écran de la fenêtre de l'app (index dans `list`).
        public var current: Int
        public var limit4K: Bool
        /// Définition sur mesure (ou session fenêtrée) : un seul écran.
        public var customResolution: Bool
        /// Réglage « Écrans » : `auto`, `this`, `all`.
        public var setting: String
        /// Écrans que la Station sait créer (1 : Station antérieure).
        public var maxScreens: Int

        public init(list: [Screen], current: Int, limit4K: Bool, customResolution: Bool, setting: String, maxScreens: Int) {
            self.list = list
            self.current = current
            self.limit4K = limit4K
            self.customResolution = customResolution
            self.setting = setting
            self.maxScreens = maxScreens
        }

        var json: [String: Any] {
            [
                "list": list.map { s in
                    ["x": s.x, "y": s.y, "width": s.width, "height": s.height, "pixel_width": s.pixelWidth,
                     "pixel_height": s.pixelHeight, "refresh": s.refresh, "primary": s.primary] as [String: Any]
                },
                "current": current, "limit_4k": limit4K, "custom_resolution": customResolution,
                "setting": setting, "max_screens": maxScreens,
            ]
        }

        var text: String {
            (try? JSONSerialization.data(withJSONObject: json)).flatMap { String(data: $0, encoding: .utf8) } ?? "{}"
        }
    }

    /// Un écran virtuel que la session demandera (`sessionPlan`).
    public struct PlannedScreen: Equatable, Sendable {
        /// Index de l'écran de cet appareil.
        public let device: Int
        public let width: Int
        public let height: Int
        public let refresh: Int
        public let primary: Bool
    }

    /// Un écran de l'ordinateur pris par la session, et l'écran de cet
    /// appareil qui le montre.
    public struct StreamScreen: Equatable, Sendable {
        /// Index de l'écran de cet appareil.
        public let index: Int
        /// L'écran de l'ordinateur (taille en pixels : repère des entrées).
        public let display: KyberDisplay
        public let refresh: Int
        public let primary: Bool
    }

    /// Les écrans que prendra la session, ou nil pour un écran : la ligne de
    /// l'accueil et les fenêtres à ouvrir, avec le même calcul que la session.
    public static func sessionPlan(_ screens: DeviceScreens, profile: String?, virtualDisplay: Bool, codec: VideoCodec) -> [PlannedScreen]? {
        guard let out = kn_screens_session_plan(screens.text, profile ?? "", virtualDisplay, codec.rawValue),
              let text = Core.take(out), let data = text.data(using: .utf8),
              let list = (try? JSONSerialization.jsonObject(with: data)) as? [[String: Any]] else { return nil }
        let plan = list.map { e in
            PlannedScreen(device: e["device"] as? Int ?? 0, width: e["width"] as? Int ?? 0, height: e["height"] as? Int ?? 0,
                          refresh: e["refresh"] as? Int ?? 0, primary: e["primary"] as? Bool ?? false)
        }
        return plan.count >= 2 ? plan : nil
    }

    /// Le réglage « Écrans » se change ; sinon inactif, sur « Cet écran ».
    public static func screensSettingEnabled(customResolution: Bool, virtualDisplay: Bool) -> Bool {
        kn_screens_setting_enabled(customResolution, virtualDisplay)
    }

    public struct Params: Sendable {
        public var host: String
        public var consolePort: UInt16
        public var username: String
        public var password: String
        public var consoleFingerprint: String?
        public var mode: Mode
        /// 0 : son laissé sur l'ordinateur.
        public var audioChannels: Int
        public var virtualDisplay: Bool
        public var settings: Settings
        public var autoReconnect = true
        public var gamepadAsXbox360 = true
        public var clipboard = true
        /// Sans image en 4 s : débit ramené à ce plafond, puis 4:2:0 (cette session seulement).
        public var fallbackBitrate: UInt32? = 250_000_000
        /// Synchro verticale du rendu.
        public var vsync = true
        /// Multi-écrans : les écrans de cet appareil (Mac) ; nil : une session
        /// à un écran, comme avant.
        public var screens: DeviceScreens?

        public init(host: String, consolePort: UInt16, username: String, password: String, consoleFingerprint: String?, mode: Mode, audioChannels: Int, virtualDisplay: Bool, settings: Settings) {
            self.host = host
            self.consolePort = consolePort
            self.username = username
            self.password = password
            self.consoleFingerprint = consoleFingerprint
            self.mode = mode
            self.audioChannels = audioChannels
            self.virtualDisplay = virtualDisplay
            self.settings = settings
        }
    }

    public enum Phase: Equatable, Sendable {
        case preparing, connecting, starting, streaming
        case reconnecting(attempt: Int, of: Int)
        case ending
        case ended(error: String?, report: EndReport?)
    }

    /// Ce que la session montre dans une capsule ou dans la barre.
    public enum Notice: Equatable, Sendable {
        /// L'image a été relancée.
        case relaunched
        /// L'écran affiché a changé de taille sur l'ordinateur ; `back` : revenu au mode du départ.
        case modeChanged(width: Int, height: Int, back: Bool)
        /// Flux allégé pour cette session : 1 = débit ramené à `mbps`, 2 = 4:2:0.
        case fallback(level: Int, mbps: Int)
        case adapting(width: Int, height: Int)
        case adapted(width: Int, height: Int)
        case adaptFailed(String)
        /// « Remettre W × H » à proposer dans la barre, ou plus rien (nil).
        case restoreOffer(Mode?)
        /// Dernier recours de la connexion : l'image ne venait pas, la Station
        /// redémarre (une fois) ; l'écran de connexion dit « La Station
        /// redémarre… » jusqu'à la phase suivante (préparation, ou fin).
        case stationRestarting
    }

    // Rappels, tous sur le fil principal.
    public var onPhase: ((Phase) -> Void)?
    public var onNotice: ((Notice) -> Void)?
    /// Les écrans annoncés par Kyber (choix dans la barre, sans écran virtuel).
    public var onDisplays: (([KyberDisplay]) -> Void)?
    /// L'écran affiché (aussi quand sa taille change).
    public var onShownDisplay: ((KyberDisplay) -> Void)?
    /// Mode de l'écran de la session, et si c'est un écran virtuel.
    public var onMode: ((Mode, Bool) -> Void)?
    /// Mesures de l'image, une fois par seconde.
    public var onVideoStats: ((VideoStats) -> Void)?
    public var onAudioStats: ((AudioStats) -> Void)?
    /// L1 + R1 + START + SELECT : la manette passe de l'ordinateur à l'app, ou revient.
    public var onGamepadCombo: (() -> Void)?
    public var onGamepadNavigate: ((PadKey) -> Void)?
    /// Un jeu est en plein écran sur l'ordinateur (les vues en sont déjà prévenues).
    public var onGameFullscreen: ((Bool) -> Void)?
    /// Multi-écrans : les écrans pris par la session (principal en premier)
    /// et ceux que l'ordinateur n'a pas pu créer, à chaque démarrage de
    /// l'image ; avant que leurs images arrivent. Les vues de ces écrans
    /// doivent être enregistrées (`register`) dans ce rappel.
    public var onScreens: (([StreamScreen], [Int]) -> Void)?
    /// Multi-écrans : la première image d'un écran est décodée.
    public var onScreenImage: ((Int) -> Void)?

    public let view: StreamView
    private let lock = NSLock()
    private var session: OpaquePointer?
    private var platform = KnPlatform()
    private var params: Params?
    /// Un décodeur et un lecteur par flux (le cœur numérote les flux ; un
    /// ancien flux peut finir de livrer après le démarrage du suivant).
    private var videos: [UInt64: VideoPipeline] = [:]
    private var audios: [UInt64: AudioPlayer] = [:]
    private var shownStream: UInt64?
    /// Multi-écrans : la vue de chaque écran de cet appareil, l'écran de
    /// chaque flux, l'écran principal (nil : session sans `screens`), les
    /// écrans pris par la session.
    private var screenViews: [Int: StreamView] = [:]
    private var streamScreens: [UInt64: Int] = [:]
    private var primaryScreen: Int?
    private var takenScreens: [StreamScreen] = []
    /// Multi-écrans : écrans dont la fenêtre est occultée (leur rendu attend).
    private var occludedScreens: Set<Int> = []
    private var input: InputPipeline?
    private var lastDisplay: KyberDisplay?
    private var lastSecond: LatencySecond?
    private var padCaptured = false
    private var occluded = false
    private var vsync = true
    private var topMarker = false
    private var volume: Float = 1
    private var ended = false

    public init(view: StreamView) {
        Core.install()
        self.view = view
    }

    // MARK: - Déroulé

    /// Ouvre la session : `onPhase` suit, jusqu'à `.ended`.
    public func start(_ p: Params) throws {
        lock.lock()
        params = p
        vsync = p.vsync
        lock.unlock()
        var json: [String: Any] = [
            "console_host": p.host, "console_port": Int(p.consolePort), "user": p.username, "password": p.password,
            "device": ConsoleClient.thisDevice,
            "mode": ["width": p.mode.width, "height": p.mode.height, "refresh": p.mode.refresh],
            "audio_channels": p.audioChannels, "virtual_display": p.virtualDisplay,
            "settings": p.settings.json,
            "auto_reconnect": p.autoReconnect, "gamepad_xbox360": p.gamepadAsXbox360, "clipboard": p.clipboard,
        ]
        if let fp = p.consoleFingerprint { json["console_fingerprint"] = fp }
        if let screens = p.screens { json["screens"] = screens.json }
        var recovery: [String: Any] = ["watch_image": true]
        if let cap = p.fallbackBitrate { recovery["fallback_bitrate"] = Int(cap) }
        json["recovery"] = recovery
        let data = try JSONSerialization.data(withJSONObject: json)
        let text = String(data: data, encoding: .utf8) ?? "{}"

        platform = KnPlatform()
        platform.userdata = Unmanaged.passRetained(self).toOpaque()
        platform.window_handle = UInt64(UInt(bitPattern: Unmanaged.passUnretained(view.videoView).toOpaque()))
        platform.has_audio = p.audioChannels > 0
        platform.input_pipeline = { ud in SessionEngine.from(ud).makeInputPipeline() }
        platform.video_packet = { ud, packet in
            guard let packet else { return }
            SessionEngine.from(ud).videoPacket(packet.pointee)
        }
        platform.audio_packet = { ud, packet in
            guard let packet else { return }
            SessionEngine.from(ud).audioPacket(packet.pointee)
        }
        platform.on_status = { ud, status in
            guard let status else { return }
            SessionEngine.from(ud).status(status.pointee)
        }
        platform.on_display = { ud, d in SessionEngine.from(ud).display(d) }
        platform.on_displays = { ud, list, count in
            let displays = (0..<Int(count)).map { KyberDisplay(list![$0]) }
            let me = SessionEngine.from(ud)
            DispatchQueue.main.async { me.onDisplays?(displays) }
        }
        platform.on_mode = { ud, mode, virtualDisplay in
            let me = SessionEngine.from(ud)
            let m = Mode(width: Int(mode.width), height: Int(mode.height), refresh: Int(mode.refresh))
            DispatchQueue.main.async { me.onMode?(m, virtualDisplay) }
        }
        platform.on_stream_started = { ud in SessionEngine.from(ud).streamStarted() }
        platform.on_stream_stopped = { ud in SessionEngine.from(ud).streamStopped() }
        platform.on_input_connected = { ud in
            let me = SessionEngine.from(ud)
            DispatchQueue.main.async {
                KyberLog.app.notice("entrées reliées à l'ordinateur")
                me.currentInput?.networkConnected()
                me.allViews.forEach { $0.inputConnected() }
            }
        }
        platform.on_second = { ud, second in
            guard let second else { return }
            SessionEngine.from(ud).second(second.pointee)
        }
        platform.on_game_fullscreen = { ud, game in
            let me = SessionEngine.from(ud)
            DispatchQueue.main.async {
                me.allViews.forEach { $0.pcGameFullscreen = game }
                me.onGameFullscreen?(game)
            }
        }
        platform.on_notice = { ud, notice in
            guard let notice else { return }
            SessionEngine.from(ud).notice(notice.pointee)
        }
        platform.on_screens = { ud, list, count, missing, missingCount in
            let screens = (0..<Int(count)).map { i -> StreamScreen in
                let s = list![i]
                return StreamScreen(index: Int(s.screen), display: KyberDisplay(s.display), refresh: Int(s.refresh), primary: s.primary)
            }
            let notCreated = (0..<Int(missingCount)).map { Int(missing![$0]) }
            SessionEngine.from(ud).screensTaken(screens, notCreated)
        }

        var error: UnsafeMutablePointer<CChar>?
        let s = withUnsafePointer(to: &platform) { kn_session_start(text, $0, &error) }
        guard let s else {
            Unmanaged<SessionEngine>.fromOpaque(platform.userdata!).release()
            throw ConsoleError.network(Core.take(error) ?? String(localized: "session impossible", bundle: .kyberKit))
        }
        lock.lock()
        session = s
        let hidden = occluded
        lock.unlock()
        // Fenêtre déjà cachée au départ : la veille d'image du cœur attend.
        if hidden { kn_session_set_hidden(s, true) }
    }

    private static func from(_ ud: UnsafeMutableRawPointer?) -> SessionEngine {
        Unmanaged<SessionEngine>.fromOpaque(ud!).takeUnretainedValue()
    }

    private var live: OpaquePointer? {
        lock.lock()
        defer { lock.unlock() }
        return session
    }

    private var currentInput: InputPipeline? {
        lock.lock()
        defer { lock.unlock() }
        return input
    }

    /// Demande la fin de la session : `.ending` puis `.ended` suivent.
    public func stop() {
        if let s = live { kn_session_stop(s) }
    }

    /// Change de profil ou de réglages en cours de session : le débit change
    /// en direct, le format, le 4:4:4 ou le transport relancent l'image.
    public func setSettings(_ settings: Settings) {
        guard let s = live, let data = try? JSONSerialization.data(withJSONObject: settings.json),
              let text = String(data: data, encoding: .utf8) else { return }
        kn_session_set_settings(s, text)
    }

    /// Plafond de débit (fenêtre occultée) ; nil le retire.
    public func setBitrateCap(_ cap: UInt32?) {
        if let s = live { kn_session_set_bitrate_cap(s, cap ?? 0) }
    }

    /// Multi-écrans : la vue qui montre l'écran `screen` de cet appareil
    /// (fil principal). Celle donnée à `init` sert aux écrans sans vue.
    public func register(_ view: StreamView, screen: Int) {
        lock.lock()
        screenViews[screen] = view
        lock.unlock()
    }

    /// Multi-écrans : toutes les vues d'un coup, par écran de cet appareil
    /// (fil principal) ; les vues qui n'y sont plus sont détachées.
    public func setScreenViews(_ views: [Int: StreamView]) {
        lock.lock()
        let old = Array(screenViews.values)
        screenViews = views
        lock.unlock()
        for v in old where v !== view && !views.values.contains(where: { $0 === v }) {
            v.detach()
            v.surface.show(nil)
        }
    }

    /// Multi-écrans : plus aucune vue pour cet écran (fenêtre fermée).
    public func unregister(screen: Int) {
        lock.lock()
        let old = screenViews.removeValue(forKey: screen)
        lock.unlock()
        if let old, old !== view {
            old.detach()
            old.surface.show(nil)
        }
    }

    /// Multi-écrans : les écrans de cet appareil ont changé ; la session se
    /// refait avec eux si le nombre ou la disposition change.
    public func setScreens(_ screens: DeviceScreens) {
        if let s = live { kn_session_set_screens(s, screens.text) }
    }

    /// La vue de l'écran `screen` (celle de `init` sans multi-écrans).
    private func view(for screen: Int?) -> StreamView {
        lock.lock()
        defer { lock.unlock() }
        return screen.flatMap { screenViews[$0] } ?? view
    }

    /// Toutes les vues de la session.
    private var allViews: [StreamView] {
        lock.lock()
        defer { lock.unlock() }
        var views = [view]
        for v in screenViews.values where !views.contains(where: { $0 === v }) { views.append(v) }
        return views
    }

    /// Sans écran virtuel : afficher un autre écran de l'ordinateur.
    public func pickDisplay(_ id: UInt32) {
        if let s = live { kn_session_pick_display(s, id) }
    }

    /// L'écran de ce Mac a changé de mode : la console refait l'écran virtuel.
    public func setMode(_ mode: Mode) {
        if let s = live { kn_session_set_mode(s, KnMode(width: UInt32(mode.width), height: UInt32(mode.height), refresh: UInt32(mode.refresh))) }
    }

    /// « Remettre » : le mode du début de session redemandé à la console.
    public func restoreMode() {
        if let s = live { kn_session_restore_mode(s) }
    }

    /// L'app rend la manette à l'ordinateur (barre fermée) ou la garde.
    public func setGamepadCaptured(_ captured: Bool) {
        lock.lock()
        padCaptured = captured
        let input = self.input
        lock.unlock()
        input?.gamepad.setCaptured(captured)
    }

    /// Synchro verticale à chaud.
    public func setVSync(_ on: Bool) {
        lock.lock()
        vsync = on
        let renderers = videos.values.map(\.renderer)
        lock.unlock()
        renderers.forEach { $0.vsync = on }
    }

    /// Trait de repère de la barre, tracé dans l'image (Mac) : allumé quand le
    /// geste qui ouvre la barre est possible. Repris par chaque flux d'image.
    public func setTopMarker(_ on: Bool) {
        lock.lock()
        topMarker = on
        let renderers = videos.values.map(\.renderer)
        lock.unlock()
        renderers.forEach { $0.topMarker = on }
    }

    /// Fenêtre occultée ou réduite : plus de rendu (le décodeur continue).
    public func setOccluded(_ occluded: Bool) {
        lock.lock()
        self.occluded = occluded
        let renderers = videos.map { ($0.value.renderer, streamScreens[$0.key]) }
        let screens = occludedScreens
        let s = session
        lock.unlock()
        for (renderer, screen) in renderers { renderer.paused = occluded || screen.map(screens.contains) == true }
        // Image figée exprès : pas une panne pour le cœur.
        if let s { kn_session_set_hidden(s, occluded) }
    }

    /// Multi-écrans : la fenêtre d'un écran est occultée (autre espace,
    /// recouverte, passage en plein écran) ou visible : seul son rendu
    /// attend ; la dernière image arrivée est rendue à la reprise.
    public func setScreenOccluded(_ occluded: Bool, screen: Int) {
        lock.lock()
        if occluded { occludedScreens.insert(screen) } else { occludedScreens.remove(screen) }
        let all = self.occluded
        let renderers = videos.filter { streamScreens[$0.key] == screen }.map(\.value.renderer)
        lock.unlock()
        renderers.forEach { $0.paused = all || occluded }
    }

    /// Multi-écrans : la fenêtre d'un écran vient de devenir visible ou de
    /// finir son passage en plein écran : son image est redessinée en place,
    /// et une image clé est demandée (le cœur attend les premières données de
    /// chaque flux avant de la demander).
    public func refreshScreen(_ screen: Int) {
        lock.lock()
        let renderers = videos.filter { streamScreens[$0.key] == screen }.map(\.value.renderer)
        let s = session
        lock.unlock()
        renderers.forEach { $0.redraw() }
        if let s, !renderers.isEmpty { kn_session_request_keyframe(s) }
    }

    public func setVolume(_ volume: Float) {
        lock.lock()
        self.volume = volume
        let players = Array(audios.values)
        lock.unlock()
        players.forEach { $0.volume = volume }
    }

    /// Latence de la session : médiane des secondes et 95e centile, en ms.
    public var sessionLatency: (median: Double, p95: Double)? {
        guard let s = live else { return nil }
        var median = 0.0, p95 = 0.0
        guard kn_session_latency(s, &median, &p95) else { return nil }
        return (median, p95)
    }

    /// Photographie de l'image affichée en ce moment, au plus `maxWidth` pixels de large.
    public func snapshot(maxWidth: Int = 1600) -> CGImage? {
        lock.lock()
        let pipeline = shownStream.flatMap { videos[$0] }
        lock.unlock()
        return pipeline?.renderer.snapshot(maxWidth: maxWidth)
    }

    // MARK: - Rappels du cœur

    private func status(_ s: KnStatus) {
        let phase: Phase
        // Les constantes de l'en-tête C (`#define`) arrivent en Int32 dans Swift.
        switch Int32(s.kind) {
        case KN_STATUS_PREPARING: phase = .preparing
        case KN_STATUS_CONNECTING: phase = .connecting
        case KN_STATUS_FIRST_IMAGES: phase = .starting
        case KN_STATUS_STREAMING: phase = .streaming
        case KN_STATUS_RECONNECTING: phase = .reconnecting(attempt: Int(s.attempt), of: Int(s.max))
        case KN_STATUS_ENDING: phase = .ending
        default:
            let error = s.error.map { String(cString: $0) }
            let report = s.has_report ? EndReport(displayRestored: s.display_restored, audioRestored: s.audio_restored, error: s.report_error.map { String(cString: $0) }) : nil
            phase = .ended(error: error, report: report)
        }
        DispatchQueue.main.async { [self] in
            if case .ended = phase { finish() }
            onPhase?(phase)
        }
    }

    /// Fin : la vue est détachée, les décodeurs arrêtés ; la session du cœur
    /// et cet objet vivent encore 30 s, le temps que Kyber finisse de livrer
    /// (et de fermer) ses flux.
    private func finish() {
        lock.lock()
        guard !ended else {
            lock.unlock()
            return
        }
        ended = true
        let s = session
        session = nil
        let videos = Array(self.videos.values)
        let audios = Array(self.audios.values)
        let input = self.input
        self.input = nil
        let ud = platform.userdata
        var views = [view]
        for v in screenViews.values where !views.contains(where: { $0 === v }) { views.append(v) }
        lock.unlock()
        for v in views {
            v.detach()
            v.surface.show(nil)
        }
        input?.stop()
        input?.shutdown()
        DispatchQueue.main.asyncAfter(deadline: .now() + 2) {
            videos.forEach { $0.stop() }
            audios.forEach { $0.stop() }
        }
        DispatchQueue.main.asyncAfter(deadline: .now() + 30) {
            if let s { kn_session_free(s) }
            if let ud { Unmanaged<SessionEngine>.fromOpaque(ud).release() }
        }
    }

    private func display(_ d: KnDisplay) {
        let display = KyberDisplay(d)
        lock.lock()
        lastDisplay = display
        let attached = input != nil
        // Multi-écrans : les tailles passent par `on_screens`, qui suit.
        let multi = primaryScreen != nil
        lock.unlock()
        DispatchQueue.main.async { [self] in
            if attached, !multi { view.setHostSize(CGSize(width: display.width, height: display.height)) }
            onShownDisplay?(display)
        }
    }

    /// Multi-écrans : les écrans pris par la session (fil de session du
    /// cœur, avant `input_pipeline` et les premières images).
    private func screensTaken(_ screens: [StreamScreen], _ notCreated: [Int]) {
        lock.lock()
        takenScreens = screens
        primaryScreen = screens.first(where: \.primary)?.index ?? screens.first?.index
        let attached = input
        lock.unlock()
        KyberLog.app.notice("écrans de la session : \(screens.map { "\($0.index) → \($0.display.id) \($0.display.width)×\($0.display.height)" }.joined(separator: ", "), privacy: .public)\(notCreated.isEmpty ? "" : " ; non créés : \(notCreated)", privacy: .public)")
        DispatchQueue.main.async { [self] in
            onScreens?(screens, notCreated)
            // Taille d'un écran changée en cours de session.
            if let attached {
                for s in screens { view(for: s.index).setHostSize(CGSize(width: s.display.width, height: s.display.height)) }
                attached.setScreens(inputScreens(screens))
            }
        }
    }

    /// Les écrans pour la chaîne d'entrées, avec la couche de pointeur de leur vue.
    private func inputScreens(_ screens: [StreamScreen]) -> [InputPipeline.Screen] {
        screens.map { s in
            InputPipeline.Screen(index: s.index, displayID: s.display.id, hostSize: CGSize(width: s.display.width, height: s.display.height),
                                 pointer: view(for: s.index).pointer)
        }
    }

    /// La chaîne d'entrées de l'image qui démarre (fil de session du cœur).
    private func makeInputPipeline() -> UnsafeMutableRawPointer? {
        lock.lock()
        let display = lastDisplay ?? KyberDisplay(id: 0, width: 0, height: 0, x: 0, y: 0)
        let clipboard = params?.clipboard ?? true
        let captured = padCaptured
        let old = input
        let screens = primaryScreen != nil ? takenScreens : []
        let primaryView = primaryScreen.flatMap { screenViews[$0] } ?? view
        lock.unlock()
        let input = InputPipeline(displayID: display.id, hostSize: CGSize(width: display.width, height: display.height), pointer: primaryView.pointer, clipboard: clipboard)
        if !screens.isEmpty { input.setScreens(inputScreens(screens)) }
        input.gamepad.onCombo = { [weak self] in DispatchQueue.main.async { self?.onGamepadCombo?() } }
        input.gamepad.onNavigate = { [weak self] key in DispatchQueue.main.async { self?.onGamepadNavigate?(key) } }
        // Une seule image à la fois : deux chaînes d'entrées se disputeraient
        // les manettes (SDL n'en sert qu'une).
        old?.stop()
        old?.shutdown()
        input.gamepad.setCaptured(captured)
        lock.lock()
        self.input = input
        lock.unlock()
        DispatchQueue.main.async { [self] in
            guard !screens.isEmpty else {
                view.attach(input: input, hostSize: CGSize(width: display.width, height: display.height))
                return
            }
            // Multi-écrans : chaque vue sur son écran de l'ordinateur ; les
            // fenêtres ouvertes par `onScreens` ont maintenant leur vue.
            for s in screens {
                let v = view(for: s.index)
                #if os(macOS)
                v.screenIndex = s.index
                #endif
                v.attach(input: input, hostSize: CGSize(width: s.display.width, height: s.display.height))
            }
            input.setScreens(inputScreens(screens))
        }
        return input.kynputRef()
    }

    private func streamStarted() {
        currentInput?.startClipboard()
    }

    private func streamStopped() {
        lock.lock()
        let old = input
        input = nil
        lock.unlock()
        old?.stop()
        // Kyber arrête lui-même la chaîne d'entrées de la session, sauf si la
        // connexion était déjà perdue : on s'en assure (manettes libérées).
        old?.shutdown()
    }

    private func second(_ s: KnSecond) {
        var second = LatencySecond()
        second.latencyMs = s.has_latency ? s.latency_ms : nil
        if s.has_chain {
            var c = ChainTimes()
            (c.capture, c.encode, c.send, c.network, c.decode, c.display) = (s.chain.0, s.chain.1, s.chain.2, s.chain.3, s.chain.4, s.chain.5)
            second.chain = c
        }
        second.displayed = Int(s.displayed)
        second.measured = Int(s.measured)
        second.rejected = Int(s.rejected)
        second.lostFrames = Int(s.lost_frames)
        second.holes = Int(s.holes)
        second.keyframeRequests = Int(s.keyframe_requests)
        second.lostPackets = Int(s.lost_packets)
        second.rttMs = s.has_rtt ? s.rtt_ms : nil
        second.bytes = Int(s.bytes)
        lock.lock()
        lastSecond = second
        lock.unlock()
    }

    private func notice(_ n: KnNotice) {
        let notice: Notice
        switch Int32(n.kind) {
        case KN_NOTICE_RELAUNCHED: notice = .relaunched
        case KN_NOTICE_MODE_CHANGED: notice = .modeChanged(width: Int(n.width), height: Int(n.height), back: n.back)
        case KN_NOTICE_FALLBACK: notice = .fallback(level: Int(n.level), mbps: Int(n.bitrate_mbps))
        case KN_NOTICE_ADAPTING: notice = .adapting(width: Int(n.width), height: Int(n.height))
        case KN_NOTICE_ADAPTED: notice = .adapted(width: Int(n.width), height: Int(n.height))
        case KN_NOTICE_ADAPT_FAILED: notice = .adaptFailed(n.error.map { String(cString: $0) } ?? "")
        case KN_NOTICE_RESTORE_OFFER: notice = .restoreOffer(n.has_mode ? Mode(width: Int(n.width), height: Int(n.height), refresh: 0) : nil)
        case KN_NOTICE_STATION_RESTARTING: notice = .stationRestarting
        default: return
        }
        DispatchQueue.main.async { [self] in onNotice?(notice) }
    }

    // MARK: - Paquets (fil réseau de Kyber)

    private func videoPipeline(for stream: UInt64, screen: Int) -> VideoPipeline? {
        lock.lock()
        if let p = videos[stream] {
            lock.unlock()
            return p
        }
        // Multi-écrans : latence, taille décodée et mesures viennent de l'écran principal.
        let primary = primaryScreen.map { $0 == screen } ?? true
        guard let pipeline = VideoPipeline() else {
            lock.unlock()
            KyberLog.video.error("rendu Metal indisponible : image ignorée")
            return nil
        }
        pipeline.renderer.vsync = vsync
        pipeline.renderer.paused = occluded || (primaryScreen != nil && occludedScreens.contains(screen))
        if primaryScreen != nil { pipeline.renderer.label = "écran \(screen)" }
        pipeline.renderer.topMarker = topMarker
        if primary {
            pipeline.renderer.latency = self
            pipeline.renderer.onStats = { [weak self] stats in self?.videoStats(stats) }
        }
        if primaryScreen != nil {
            pipeline.onFirstFrame = { [weak self] in DispatchQueue.main.async { self?.onScreenImage?(screen) } }
        }
        #if !os(macOS)
        pipeline.renderer.onStuck = { [weak self] in
            DispatchQueue.main.async { self?.view.surface.heal() }
        }
        #endif
        pipeline.decoder.onLostReference = { [weak self] in
            if let s = self?.live { kn_session_keyframe_lost(s) }
        }
        pipeline.decoder.onRecovered = { [weak self] in
            if let s = self?.live { kn_session_keyframe_recovered(s) }
        }
        videos[stream] = pipeline
        streamScreens[stream] = screen
        if primary { shownStream = stream }
        let multi = primaryScreen != nil
        lock.unlock()
        KyberLog.video.notice("image décodée par l'app (VideoToolbox, Metal), flux \(stream)\(multi ? ", écran \(screen)" : "", privacy: .public)")
        DispatchQueue.main.async { [self] in
            guard multi else {
                view.surface.show(pipeline.renderer)
                return
            }
            // Un écran sans vue (fenêtre fermée entre-temps) : l'image n'est
            // montrée nulle part, jamais à la place d'un autre écran.
            lock.lock()
            let target = screenViews[screen] ?? (screen == primaryScreen ? view : nil)
            lock.unlock()
            if let target { target.surface.show(pipeline.renderer) } else { KyberLog.video.notice("écran \(screen) sans fenêtre : image ignorée") }
        }
        return pipeline
    }

    private func videoPacket(_ p: KnPacket) {
        switch Int32(p.kind) {
        case KN_PACKET_CLOSED:
            // Plus aucun paquet ne viendra : le pipeline peut être arrêté.
            lock.lock()
            let pipeline = videos.removeValue(forKey: p.stream)
            streamScreens.removeValue(forKey: p.stream)
            lock.unlock()
            pipeline?.stop()
        case KN_PACKET_CODEC:
            videoPipeline(for: p.stream, screen: Int(p.screen))?.decoder.setCodec(p.codec)
        case KN_PACKET_MEDIA:
            guard let data = p.data, p.len > 0, let pipeline = videoPipeline(for: p.stream, screen: Int(p.screen)) else { return }
            if let s = live { kn_session_bytes_received(s, p.len) }
            pipeline.decoder.decode(UnsafeBufferPointer(start: data, count: Int(p.len)), isConfig: p.is_config, isKey: p.is_key, pts: p.pts)
        case KN_PACKET_HOLE:
            // Données perdues : l'image suivante n'a plus sa référence.
            if let s = live { kn_session_hole(s) }
            videoPipeline(for: p.stream, screen: Int(p.screen))?.decoder.dataLost()
        default:
            break
        }
    }

    private func videoStats(_ stats: VideoStats) {
        if stats.width > 0, stats.height > 0, let s = live {
            kn_session_decoded_size(s, UInt32(stats.width), UInt32(stats.height))
        }
        DispatchQueue.main.async { [self] in onVideoStats?(stats) }
    }

    private func audioPlayer(for stream: UInt64) -> AudioPlayer {
        lock.lock()
        defer { lock.unlock() }
        if let p = audios[stream] { return p }
        let player = AudioPlayer()
        player.volume = volume
        player.onStats = { [weak self] stats in DispatchQueue.main.async { self?.onAudioStats?(stats) } }
        audios[stream] = player
        return player
    }

    private func audioPacket(_ p: KnPacket) {
        switch Int32(p.kind) {
        case KN_PACKET_CLOSED:
            lock.lock()
            let player = audios.removeValue(forKey: p.stream)
            lock.unlock()
            player?.stop()
        case KN_PACKET_MEDIA:
            guard let data = p.data, p.len > 0 else { return }
            let buffer = UnsafeBufferPointer(start: data, count: Int(p.len))
            let player = audioPlayer(for: p.stream)
            if p.is_config { player.configure(opusHead: buffer) } else { player.decode(buffer) }
        case KN_PACKET_HOLE:
            audioPlayer(for: p.stream).conceal(samples: Int(p.missing_samples))
        default:
            break
        }
    }
}

// MARK: - Latence : le rendu parle au cœur

extension SessionEngine: LatencyReporting {
    func received(bytes: Int) {
        if let s = live { kn_session_bytes_received(s, UInt(bytes)) }
    }

    func hole() {
        if let s = live { kn_session_hole(s) }
    }

    func keyframeRequested() {}

    func skipped(pts: UInt64) {
        if let s = live { kn_session_frame_skipped(s, pts) }
    }

    func displayed(pts: UInt64, received: Int64, decoded: Int64, at micros: Int64) {
        if let s = live { kn_session_frame_displayed(s, pts, received, decoded, micros) }
    }

    func drain() -> LatencySecond {
        lock.lock()
        defer { lock.unlock() }
        return lastSecond.map { s in
            lastSecond = nil
            return s
        } ?? LatencySecond()
    }
}

private extension KyberDisplay {
    init(_ d: KnDisplay) {
        self.init(id: d.id, width: Int(d.width), height: Int(d.height), x: Int(d.x), y: Int(d.y))
    }
}
