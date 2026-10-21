// Une session sur le cœur Rust (docs/coeur-apple.md) : le cœur prépare,
// connecte, choisit l'écran, relance l'image, reconnecte, finit ; Swift
// fournit ce qui dépend de l'appareil : la vue, le décodeur et le rendu
// Metal, le son, la chaîne d'entrées kynput, et reçoit états, mesures et
// avis. Remplace `KyberClient` sur Mac ; l'Apple TV compile encore
// KyberClient.swift jusqu'à son propre passage sur le cœur.
//
// Les rappels du cœur arrivent sur ses fils (fil de session, fil réseau de
// Kyber pour les paquets) : ce qui touche l'interface remonte sur le fil
// principal, les paquets vont droit au décodeur.

import AppKit
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
    /// Un jeu est en plein écran sur l'ordinateur (la vue en est déjà prévenue).
    public var onGameFullscreen: ((Bool) -> Void)?

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
    private var input: InputPipeline?
    private var lastDisplay: KyberDisplay?
    private var lastSecond: LatencySecond?
    private var padCaptured = false
    private var occluded = false
    private var vsync = true
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
                me.view.inputConnected()
            }
        }
        platform.on_second = { ud, second in
            guard let second else { return }
            SessionEngine.from(ud).second(second.pointee)
        }
        platform.on_game_fullscreen = { ud, game in
            let me = SessionEngine.from(ud)
            DispatchQueue.main.async {
                me.view.pcGameFullscreen = game
                me.onGameFullscreen?(game)
            }
        }
        platform.on_notice = { ud, notice in
            guard let notice else { return }
            SessionEngine.from(ud).notice(notice.pointee)
        }

        var error: UnsafeMutablePointer<CChar>?
        let s = withUnsafePointer(to: &platform) { kn_session_start(text, $0, &error) }
        guard let s else {
            Unmanaged<SessionEngine>.fromOpaque(platform.userdata!).release()
            throw ConsoleError.network(Core.take(error) ?? String(localized: "session impossible", bundle: .kyberKit))
        }
        lock.lock()
        session = s
        lock.unlock()
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

    /// Fenêtre occultée ou réduite : plus de rendu (le décodeur continue).
    public func setOccluded(_ occluded: Bool) {
        lock.lock()
        self.occluded = occluded
        let renderers = videos.values.map(\.renderer)
        lock.unlock()
        renderers.forEach { $0.paused = occluded }
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
        lock.unlock()
        view.detach()
        view.surface.show(nil)
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
        lock.unlock()
        DispatchQueue.main.async { [self] in
            if attached { view.setHostSize(CGSize(width: display.width, height: display.height)) }
            onShownDisplay?(display)
        }
    }

    /// La chaîne d'entrées de l'image qui démarre (fil de session du cœur).
    private func makeInputPipeline() -> UnsafeMutableRawPointer? {
        lock.lock()
        let display = lastDisplay ?? KyberDisplay(id: 0, width: 0, height: 0, x: 0, y: 0)
        let clipboard = params?.clipboard ?? true
        let captured = padCaptured
        let old = input
        lock.unlock()
        let input = InputPipeline(displayID: display.id, hostSize: CGSize(width: display.width, height: display.height), pointer: view.pointer, clipboard: clipboard)
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
        DispatchQueue.main.async { [self] in view.attach(input: input, hostSize: CGSize(width: display.width, height: display.height)) }
        return UnsafeMutableRawPointer(kynput_pipeline_ref(input.pipeline))
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
        default: return
        }
        DispatchQueue.main.async { [self] in onNotice?(notice) }
    }

    // MARK: - Paquets (fil réseau de Kyber)

    private func videoPipeline(for stream: UInt64) -> VideoPipeline? {
        lock.lock()
        if let p = videos[stream] {
            lock.unlock()
            return p
        }
        guard let pipeline = VideoPipeline() else {
            lock.unlock()
            KyberLog.video.error("rendu Metal indisponible : image ignorée")
            return nil
        }
        pipeline.renderer.vsync = vsync
        pipeline.renderer.paused = occluded
        pipeline.renderer.latency = self
        pipeline.renderer.onStats = { [weak self] stats in self?.videoStats(stats) }
        pipeline.decoder.onLostReference = { [weak self] in
            if let s = self?.live { kn_session_keyframe_lost(s) }
        }
        pipeline.decoder.onRecovered = { [weak self] in
            if let s = self?.live { kn_session_keyframe_recovered(s) }
        }
        videos[stream] = pipeline
        shownStream = stream
        lock.unlock()
        KyberLog.video.notice("image décodée par l'app (VideoToolbox, Metal), flux \(stream)")
        DispatchQueue.main.async { [self] in view.surface.show(pipeline.renderer) }
        return pipeline
    }

    private func videoPacket(_ p: KnPacket) {
        switch Int32(p.kind) {
        case KN_PACKET_CLOSED:
            // Plus aucun paquet ne viendra : le pipeline peut être arrêté.
            lock.lock()
            let pipeline = videos.removeValue(forKey: p.stream)
            lock.unlock()
            pipeline?.stop()
        case KN_PACKET_CODEC:
            videoPipeline(for: p.stream)?.decoder.setCodec(p.codec)
        case KN_PACKET_MEDIA:
            guard let data = p.data, p.len > 0, let pipeline = videoPipeline(for: p.stream) else { return }
            if let s = live { kn_session_bytes_received(s, p.len) }
            pipeline.decoder.decode(UnsafeBufferPointer(start: data, count: Int(p.len)), isConfig: p.is_config, isKey: p.is_key, pts: p.pts)
        case KN_PACKET_HOLE:
            // Données perdues : l'image suivante n'a plus sa référence.
            if let s = live { kn_session_hole(s) }
            videoPipeline(for: p.stream)?.decoder.dataLost()
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
