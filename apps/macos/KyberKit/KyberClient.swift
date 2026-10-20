// Façade Swift du SDK client de Kyber (libkyclient).
//
// Une instance = une connexion à un ordinateur. Les événements du SDK
// arrivent sur des fils quelconques ; ils sont dépaquetés tout de suite
// (le pointeur d'événement ne vit que le temps du rappel) puis livrés sur le
// fil principal.

import AppKit
@_implementationOnly import CKyber

public struct KyberDisplay: Sendable, Hashable {
    public let id: UInt32
    public let width: Int
    public let height: Int
    public let x: Int
    public let y: Int
}

public enum VideoCodec: String, Sendable, CaseIterable, Codable {
    case h264, hevc, av1
}

public struct StreamOptions: Sendable {
    public var codec: VideoCodec = .hevc
    public var yuv444 = false
    /// Débit en bits par seconde.
    public var bitrate: UInt32 = 80_000_000
    public var audio = true
    /// `true` : chaque image arrive, quitte à attendre ; `false` : une image
    /// perdue est réparée (correction d'erreurs) ou sautée, jamais attendue.
    public var reliable = false
    /// Synchro verticale : sans elle, chaque image part dès qu'elle est
    /// décodée (latence la plus basse, déchirure possible).
    public var vsync = true
    /// Décodage par l'app (VideoToolbox + Metal) plutôt que par VLC. L'AV1
    /// passe toujours par VLC.
    public var nativeVideo = true
    /// Son décodé par l'app (Opus + Core Audio) plutôt que par VLC.
    public var nativeAudio = true
    /// Presse-papiers partagé (texte), dans les deux sens.
    public var clipboard = true

    public init() {}
}

public enum KyberEvent: Sendable {
    case connected([KyberDisplay])
    case connectFailed(String)
    case streamingStarted
    case streamingFailed(String)
    case streamingStopped
    case reconnecting(attempt: Int, of: Int)
    case reconnected(restartStreaming: Bool)
    case reconnectionFailed
    case stopped(String)
    case displaysChanged([KyberDisplay])
    case pipelineError(String)
}

public enum KyberError: LocalizedError {
    case sdk(String, Int32)

    public var errorDescription: String? {
        switch self {
        case let .sdk(what, code): return "\(what) (code \(code))"
        }
    }
}

@inline(__always)
private func check(_ code: Int32, _ what: String) throws {
    if code != 0 { throw KyberError.sdk(what, code) }
}

public final class KyberClient {
    public var onEvent: ((KyberEvent) -> Void)?
    /// L1 + R1 + START + SELECT : la manette passe de l'ordinateur à l'app, ou revient.
    public var onGamepadCombo: (() -> Void)?
    /// Navigation à la manette pendant que l'app la tient.
    public var onGamepadNavigate: ((PadKey) -> Void)?
    /// Mesures de l'image, une fois par seconde (fil principal).
    public var onVideoStats: ((VideoStats) -> Void)?
    /// L'image reste figée malgré les demandes d'image clé (ordinateur sans
    /// le correctif KyberNow) : relancer le flux la fait repartir.
    public var onVideoStalled: (() -> Void)?
    /// Mesures du son, une fois par seconde (fil principal).
    public var onAudioStats: ((AudioStats) -> Void)?

    private var client: OpaquePointer?
    private var input: InputPipeline?
    private var video: VideoPipeline?
    private var audio: AudioPlayer?
    /// Latence de bout en bout, à partir des mesures de Kyber.
    private let latency = LatencyTracker()

    /// Médiane de la latence sur la session en cours, en millisecondes.
    public var sessionLatencyMs: Double? { latency.sessionMedian }
    private let host: String

    private static let setup: Void = {
        kyclient_init_crypto()
        KyberLog.install()
    }()

    public init(host: String, port: UInt16, username: String, password: String, autoReconnect: Bool = true) throws {
        _ = Self.setup
        self.host = host
        let creds = kyclient_auth_credentials_new_basic(username, password)
        var config: OpaquePointer?
        try check(kyclient_connect_config_default(host, creds, &config), "configuration de la connexion")
        defer { kyclient_connect_config_unref(config) }
        kyclient_connect_config_set_streamer_port(config, port)
        kyclient_connect_config_set_auto_reconnection(config, autoReconnect)
        // Premier contact : on retient le certificat (TOFU). La console a déjà
        // été vérifiée par empreinte ; une invite dédiée viendra ensuite.
        try check(kyclient_connect_config_enable_tofu_auto_accept(config), "vérification du certificat")
        // Mesures actives : horodatages des images côté ordinateur et recalage d'horloge.
        guard let c = kyclient_client_new(config, true) else {
            throw KyberError.sdk("création du client", -1)
        }
        client = c
        kyclient_client_set_metrics_callback(c, LatencyTracker.callback, Unmanaged.passUnretained(latency).toOpaque())
        let handler = kyclient_EventHandler(
            on_event: { event, userdata in
                guard let event, let userdata else { return }
                let me = Unmanaged<KyberClient>.fromOpaque(userdata).takeUnretainedValue()
                if let e = KyberClient.unpack(event) {
                    DispatchQueue.main.async { me.onEvent?(e) }
                }
            },
            userdata: Unmanaged.passUnretained(self).toOpaque()
        )
        try check(kyclient_client_set_event_handler(c, handler), "abonnement aux événements")
    }

    deinit {
        if let client {
            kyclient_client_disconnect_and_stop(client, true)
            kyclient_client_unref(client)
        }
    }

    public func connect() throws {
        try check(kyclient_client_connect(client), "connexion à \(host)")
    }

    /// Démarre l'image de `display` dans `view`, avec le son et les entrées.
    public func startStreaming(display: KyberDisplay, view: StreamView, options: StreamOptions) throws {
        // Les gestionnaires d'entrées ne redémarrent pas : un pipeline neuf par session.
        let input = InputPipeline(display: display, view: view, clipboard: options.clipboard)
        input.gamepad.onCombo = { [weak self] in self?.onGamepadCombo?() }
        input.gamepad.onNavigate = { [weak self] key in self?.onGamepadNavigate?(key) }
        self.input = input
        view.attach(input: input, hostSize: CGSize(width: display.width, height: display.height))

        let protocolRef = kyclient_streaming_protocol_kymux(
            options.reliable ? KYCLIENT_KYMUX_VIDEO_PROTOCOL_RELIABLE : KYCLIENT_KYMUX_VIDEO_PROTOCOL_UNRELIABLE_FEC,
            KYCLIENT_KYMUX_AUDIO_PROTOCOL_RELIABLE
        )
        defer { kyclient_streaming_protocol_unref(protocolRef) }
        var session: OpaquePointer?
        try check(kyclient_streaming_session_config_default(protocolRef, &session), "configuration de la session")
        defer { kyclient_streaming_session_config_unref(session) }
        kyclient_streaming_session_config_set_bitrate(session, options.bitrate)

        let handle = kyclient_windowshandle_from_ns_view(UInt64(UInt(bitPattern: Unmanaged.passUnretained(view.videoView).toOpaque())))
        var player: OpaquePointer?
        try check(kyclient_video_player_config_default(handle, &player), "lecteur vidéo")
        defer { kyclient_video_player_config_unref(player) }
        // La session vidéo recopie les réglages du lecteur à sa création : la
        // sortie vers l'app doit être posée avant (sinon l'image part à VLC).
        if options.nativeVideo, options.codec != .av1 || DecoderCapabilities.current.av1, let pipeline = VideoPipeline() {
            pipeline.renderer.vsync = options.vsync
            latency.reset()
            pipeline.renderer.latency = latency
            pipeline.renderer.onStats = { [weak self] stats in
                DispatchQueue.main.async { self?.onVideoStats?(stats) }
            }
            pipeline.decoder.onLostReference = { [weak self] in DispatchQueue.main.async { self?.keyframeNeeded() } }
            pipeline.decoder.onRecovered = { [weak self] in DispatchQueue.main.async { self?.keyframeArrived() } }
            kyclient_video_player_config_set_packet_sink(player, VideoPipeline.callback, Unmanaged.passUnretained(pipeline).toOpaque())
            retire(self.video)
            self.video = pipeline
            view.surface.show(pipeline.renderer)
            KyberLog.video.notice("image décodée par l'app (VideoToolbox, Metal)")
        } else {
            view.surface.show(nil)
            KyberLog.video.notice("image décodée par VLC (\(options.codec.rawValue, privacy: .public))")
        }
        var video: OpaquePointer?
        try check(kyclient_video_session_config_default(display.id, player, &video), "configuration vidéo")
        kyclient_video_session_config_set_codec(video, options.codec.sdk)
        kyclient_video_session_config_set_yuv444(video, options.yuv444)
        kyclient_streaming_session_config_add_video(session, video)

        if options.audio {
            var audio: OpaquePointer?
            try check(kyclient_audio_session_config_default(&audio), "configuration du son")
            if options.nativeAudio {
                let player = AudioPlayer()
                player.onStats = { [weak self] in self?.onAudioStats?($0) }
                kyclient_audio_session_config_set_packet_sink(audio, AudioPlayer.callback, Unmanaged.passUnretained(player).toOpaque())
                retire(self.audio)
                self.audio = player
            }
            kyclient_streaming_session_config_set_audio(session, audio)
        }

        var inputConfig: OpaquePointer?
        try check(kyclient_input_session_config_default(kynput_pipeline_ref(input.pipeline), &inputConfig), "configuration des entrées")
        kyclient_input_session_config_clipboard(inputConfig, options.clipboard)
        kyclient_streaming_session_config_set_input(session, inputConfig)

        try check(kyclient_client_start_streaming(client, session), "démarrage de l'image")
        input.startClipboard()
    }

    public func stopStreaming() {
        stopKeyframeWait()
        input?.stop()
        input = nil
        kyclient_client_stop_streaming(client)
        retire(video)
        video = nil
        retire(audio)
        audio = nil
    }

    // MARK: - Image clé après une perte

    private var keyframeTimer: Timer?
    private var keyframeAsked: Date?

    /// Paquets perdus au-delà de la correction d'erreurs : avec le
    /// rafraîchissement progressif de l'encodeur, aucune image clé n'arrive
    /// d'elle-même. On en demande une (kyber-patches serveur 0005), de nouveau
    /// toutes les 300 ms ; au bout de 1,5 s, on relance le flux.
    private func keyframeNeeded() {
        guard keyframeAsked == nil, let client else { return }
        keyframeAsked = Date()
        kyclient_client_request_keyframe(client)
        keyframeTimer = Timer.scheduledTimer(withTimeInterval: 0.3, repeats: true) { [weak self] _ in
            guard let self, let asked = self.keyframeAsked, let client = self.client else { return }
            if Date().timeIntervalSince(asked) > 1.5 {
                KyberLog.video.error("pas d'image clé en 1,5 s : relance du flux")
                self.stopKeyframeWait()
                self.onVideoStalled?()
            } else {
                kyclient_client_request_keyframe(client)
            }
        }
    }

    private func keyframeArrived() {
        if let asked = keyframeAsked {
            KyberLog.video.notice("image clé reçue en \(Int(Date().timeIntervalSince(asked) * 1000)) ms")
        }
        stopKeyframeWait()
    }

    private func stopKeyframeWait() {
        keyframeTimer?.invalidate()
        keyframeTimer = nil
        keyframeAsked = nil
    }

    /// Synchro verticale à chaud.
    public func setVSync(_ on: Bool) {
        video?.renderer.vsync = on
    }

    /// Kyber peut finir de livrer un paquet après l'arrêt : le pipeline vit
    /// encore un peu avant d'être libéré.
    private func retire(_ pipeline: VideoPipeline?) {
        guard let pipeline else { return }
        DispatchQueue.main.asyncAfter(deadline: .now() + 2) { pipeline.stop() }
    }

    private func retire(_ player: AudioPlayer?) {
        guard let player else { return }
        // La sortie se tait tout de suite ; le lecteur vit encore un peu (Kyber peut finir un paquet).
        player.volume = 0
        DispatchQueue.main.asyncAfter(deadline: .now() + 2) { player.stop() }
    }

    /// L'app rend la manette à l'ordinateur (barre fermée) ou la garde.
    public func setGamepadCaptured(_ captured: Bool) {
        input?.gamepad.setCaptured(captured)
    }

    public func setBitrate(_ bitsPerSecond: UInt32) {
        kyclient_client_set_bitrate(client, bitsPerSecond)
    }

    public func setVolume(_ volume: Float) {
        audio?.volume = volume
        kyclient_client_audio_set_volume(client, volume)
    }

    public func stop() {
        input?.stop()
        input = nil
        if let client {
            kyclient_client_disconnect_and_stop(client, true)
        }
        retire(video)
        video = nil
        retire(audio)
        audio = nil
    }

    // MARK: - Événements

    private static func displays(_ ptr: UnsafeMutablePointer<kyclient_Display>?, _ len: UInt32) -> [KyberDisplay] {
        guard let ptr else { return [] }
        defer { kyclient_event_free_displays(ptr, len) }
        return (0..<Int(len)).map {
            let d = ptr[$0]
            return KyberDisplay(id: d.id, width: Int(d.width), height: Int(d.height), x: Int(d.x), y: Int(d.y))
        }
    }

    private static func unpack(_ event: OpaquePointer) -> KyberEvent? {
        switch kyclient_event_get_type(event) {
        case KYCLIENT_EVENT_TYPE_CONTROL_PLANE_CONNECTED:
            var list: UnsafeMutablePointer<kyclient_Display>?
            var len: UInt32 = 0
            guard kyclient_event_unpack_control_plane_connected(event, &list, &len) == 0 else { return nil }
            return .connected(displays(list, len))
        case KYCLIENT_EVENT_TYPE_DISPLAY_LIST_UPDATED:
            var list: UnsafeMutablePointer<kyclient_Display>?
            var len: UInt32 = 0
            guard kyclient_event_unpack_display_list_updated(event, &list, &len) == 0 else { return nil }
            return .displaysChanged(displays(list, len))
        case KYCLIENT_EVENT_TYPE_CONTROL_PLANE_CONNECT_FAILED, KYCLIENT_EVENT_TYPE_DATA_PLANE_CONNECT_FAILED:
            var reason = KYCLIENT_CONNECT_FAILED_REASON_NETWORK_ERROR
            _ = kyclient_event_unpack_connect_failed(event, &reason)
            return .connectFailed(reason.text)
        case KYCLIENT_EVENT_TYPE_STREAMING_STARTED:
            return .streamingStarted
        case KYCLIENT_EVENT_TYPE_STREAMING_START_FAILED:
            return .streamingFailed("l'ordinateur n'a pas pu démarrer l'image")
        case KYCLIENT_EVENT_TYPE_STREAMING_STOPPED:
            return .streamingStopped
        case KYCLIENT_EVENT_TYPE_RECONNECTING:
            var attempt: UInt32 = 0
            var max: UInt32 = 0
            _ = kyclient_event_unpack_reconnecting(event, &attempt, &max)
            return .reconnecting(attempt: Int(attempt), of: Int(max))
        case KYCLIENT_EVENT_TYPE_RECONNECTED:
            var restart = false
            _ = kyclient_event_unpack_reconnected(event, &restart)
            return .reconnected(restartStreaming: restart)
        case KYCLIENT_EVENT_TYPE_RECONNECTION_FAILED:
            return .reconnectionFailed
        case KYCLIENT_EVENT_TYPE_STOPPED:
            var reason = KYCLIENT_STOP_REASON_REQUESTED
            _ = kyclient_event_unpack_stopped(event, &reason)
            return .stopped(reason.text)
        case KYCLIENT_EVENT_TYPE_STREAMER_PIPELINE_EVENT:
            var type = KYCLIENT_STREAMER_PIPELINE_TYPE_VIDEO
            var stage = KYCLIENT_STREAMER_PIPELINE_STAGE_PROCESS
            var kind = KYCLIENT_STREAMER_PIPELINE_EVENT_TYPE_ERROR
            guard kyclient_event_unpack_streamer_pipeline_event(event, &type, &stage, &kind) == 0,
                  kind == KYCLIENT_STREAMER_PIPELINE_EVENT_TYPE_ERROR else { return nil }
            let what = type == KYCLIENT_STREAMER_PIPELINE_TYPE_AUDIO ? "son" : type == KYCLIENT_STREAMER_PIPELINE_TYPE_VIDEO ? "image" : "écrans"
            return .pipelineError("erreur côté ordinateur (\(what))")
        default:
            return nil
        }
    }
}

private extension VideoCodec {
    var sdk: kyclient_VideoCodec {
        switch self {
        case .h264: return KYCLIENT_VIDEO_CODEC_H264
        case .hevc: return KYCLIENT_VIDEO_CODEC_HEVC
        case .av1: return KYCLIENT_VIDEO_CODEC_AV1
        }
    }
}

private extension kyclient_ConnectFailedReason {
    var text: String {
        switch self {
        case KYCLIENT_CONNECT_FAILED_REASON_UNAUTHORIZED: return "identifiant ou mot de passe refusé"
        case KYCLIENT_CONNECT_FAILED_REASON_TIMED_OUT: return "l'ordinateur ne répond pas"
        case KYCLIENT_CONNECT_FAILED_REASON_TLS_VERIFICATION_FAILED: return "le certificat de l'ordinateur a changé"
        case KYCLIENT_CONNECT_FAILED_REASON_SERVER_ERROR: return "erreur du serveur de streaming"
        default: return "connexion impossible"
        }
    }
}

private extension kyclient_StopReason {
    var text: String {
        switch self {
        case KYCLIENT_STOP_REASON_REQUESTED: return "session terminée"
        case KYCLIENT_STOP_REASON_CONTROL_PLANE_TIMEOUT: return "l'ordinateur ne répond plus"
        case KYCLIENT_STOP_REASON_DECODE_ERROR: return "image illisible"
        default: return "connexion perdue"
        }
    }
}
