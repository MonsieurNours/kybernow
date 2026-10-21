// Façade Swift du SDK client de Kyber (libkyclient).
//
// Une instance = une connexion à un ordinateur. Les événements du SDK
// arrivent sur des fils quelconques ; ils sont dépaquetés tout de suite
// (le pointeur d'événement ne vit que le temps du rappel) puis livrés sur le
// fil principal.
//
// Commun au Mac et à l'Apple TV (apps/tvos) : la vue de session (StreamView)
// est propre à chaque plateforme.

import CoreGraphics
import Foundation
@_implementationOnly import CKyber

public struct KyberDisplay: Sendable, Hashable {
    public let id: UInt32
    public let width: Int
    public let height: Int
    public let x: Int
    public let y: Int

    /// Le même écran à une autre taille (mode changé sur l'ordinateur).
    public func resized(width: Int, height: Int) -> KyberDisplay {
        KyberDisplay(id: id, width: width, height: height, x: x, y: y)
    }
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
    /// Toutes les manettes présentées en Xbox 360 à l'ordinateur ; sinon,
    /// fidèles au modèle (DualShock 4 pour les manettes Sony).
    public var gamepadAsXbox360 = true

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
        case let .sdk(what, code): return String(localized: "\(what) (code \(code))", bundle: .kyberKit)
        }
    }
}

@inline(__always)
private func check(_ code: Int32, _ what: String) throws {
    if code != 0 { throw KyberError.sdk(what, code) }
}

/// Objets passés à Kyber comme `userdata` des sorties de paquets (pipeline
/// vidéo, lecteur audio). Ils sont **retenus** le temps que Kyber puisse les
/// appeler : la tâche qui livre les paquets ne s'arrête qu'avec le message
/// d'arrêt de la session, derrière d'autres messages, et une libération à
/// délai fixe pouvait tomber trop tôt (plantage sur un pointeur libéré).
/// Kyber (correctif KyberNow) signale son dernier appel par `CLOSED` : c'est
/// là que l'objet est rendu. Au cas où la bibliothèque n'enverrait pas ce
/// signal, `releaseLater` rend l'objet 30 s après l'arrêt ; le premier des
/// deux gagne, l'autre ne fait rien.
enum PacketSinkLease {
    private static let lock = NSLock()
    private static var live = Set<UnsafeMutableRawPointer>()

    /// Retient `object` et rend le pointeur à donner à Kyber.
    static func lease<T: AnyObject>(_ object: T) -> UnsafeMutableRawPointer {
        let pointer = Unmanaged.passRetained(object).toOpaque()
        lock.withLock { _ = live.insert(pointer) }
        return pointer
    }

    /// Rend l'objet si c'est le premier à le demander.
    static func release<T: AnyObject>(_: T.Type, _ pointer: UnsafeMutableRawPointer) {
        let leased = lock.withLock { live.remove(pointer) != nil }
        if leased { Unmanaged<T>.fromOpaque(pointer).release() }
    }

    static func releaseLater<T: AnyObject>(_ type: T.Type, _ object: T, after seconds: TimeInterval = 30) {
        let pointer = Unmanaged.passUnretained(object).toOpaque()
        DispatchQueue.main.asyncAfter(deadline: .now() + seconds) { release(type, pointer) }
    }
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
    /// L'écran affiché a changé de taille sur l'ordinateur (un jeu qui passe
    /// en 1080p) : le repère des entrées a déjà été recalé (fil principal).
    public var onHostSizeChanged: ((_ width: Int, _ height: Int) -> Void)?

    private var client: OpaquePointer?
    private var input: InputPipeline?
    private var video: VideoPipeline?
    private var audio: AudioPlayer?
    /// Vue de la session en cours, et taille de l'écran de l'ordinateur dans
    /// le repère de ses entrées.
    private weak var view: StreamView?
    private var hostSize = (width: 0, height: 0)
    /// Taille des dernières images décodées par l'app, et celle que la liste
    /// de Kyber donne à l'écran affiché.
    private var frameSize: (width: Int, height: Int)?
    private var listSize = (width: 0, height: 0)
    /// Latence de bout en bout, à partir des mesures de Kyber.
    private let latency = LatencyTracker()

    /// Médiane de la latence sur la session en cours, en millisecondes.
    public var sessionLatencyMs: Double? { latency.sessionMedian }
    public var sessionLatencyP95Ms: Double? { latency.sessionP95 }

    /// Photographie de l'image affichée en ce moment (décodage par l'app), au
    /// plus `maxWidth` pixels de large.
    public func snapshot(maxWidth: Int = 1600) -> CGImage? {
        video?.renderer.snapshot(maxWidth: maxWidth)
    }
    private let host: String

    private static let setup: Void = {
        kyclient_init_crypto()
        KyberLog.install()
    }()

    /// `fingerprint` : celle du certificat de Kyber donnée par la console
    /// (`PrepareResult.kyberFingerprint`) ; sans elle, le certificat est
    /// retenu au premier contact.
    public init(host: String, port: UInt16, username: String, password: String, autoReconnect: Bool = true, fingerprint: String? = nil) throws {
        _ = Self.setup
        self.host = host
        let creds = kyclient_auth_credentials_new_basic(username, password)
        var config: OpaquePointer?
        try check(kyclient_connect_config_default(host, creds, &config), String(localized: "configuration de la connexion", bundle: .kyberKit))
        defer { kyclient_connect_config_unref(config) }
        kyclient_connect_config_set_streamer_port(config, port)
        kyclient_connect_config_set_auto_reconnection(config, autoReconnect)
        if let fingerprint {
            // La console, vérifiée par empreinte, dit celle du certificat de
            // Kyber : on l'exige, sans mémoire du premier contact. Réinstaller
            // l'ordinateur change ce certificat ; retenu, il bloquait l'app
            // (« connexion impossible ») jusqu'à effacer `known_hosts` à la main.
            try check(kyclient_connect_config_set_tls_fingerprint(config, fingerprint), String(localized: "vérification du certificat", bundle: .kyberKit))
        } else {
            // Console d'avant 0.2.8 : on retient le certificat au premier contact (TOFU).
            try check(kyclient_connect_config_enable_tofu_auto_accept(config), String(localized: "vérification du certificat", bundle: .kyberKit))
        }
        // Mesures actives : horodatages des images côté ordinateur et recalage d'horloge.
        guard let c = kyclient_client_new(config, true) else {
            throw KyberError.sdk(String(localized: "création du client", bundle: .kyberKit), -1)
        }
        client = c
        kyclient_client_set_metrics_callback(c, LatencyTracker.callback, Unmanaged.passUnretained(latency).toOpaque())
        let handler = kyclient_EventHandler(
            on_event: { event, userdata in
                guard let event, let userdata else { return }
                let me = Unmanaged<KyberClient>.fromOpaque(userdata).takeUnretainedValue()
                // Entrées reliées à l'ordinateur (début de session, ou relance :
                // Kyber relance alors son service d'entrées, qui ne tient plus
                // rien) : ce qui a été envoyé avant s'est perdu. La chaîne
                // renvoie les manettes branchées, la vue l'état qu'elle tient
                // (mode de la souris, modificateurs, pointeur).
                if kyclient_event_get_type(event) == KYCLIENT_EVENT_TYPE_INPUT_SERVICE_CONNECTED {
                    DispatchQueue.main.async {
                        me.input?.networkConnected()
                        me.view?.inputConnected()
                    }
                }
                if let e = KyberClient.unpack(event) {
                    // Nouveau flux (démarrage, reconnexion) : les pts repartent de zéro.
                    if case .streamingStarted = e { me.latency.reset() }
                    DispatchQueue.main.async { me.onEvent?(e) }
                }
            },
            userdata: Unmanaged.passUnretained(self).toOpaque()
        )
        try check(kyclient_client_set_event_handler(c, handler), String(localized: "abonnement aux événements", bundle: .kyberKit))
    }

    deinit {
        if let client {
            kyclient_client_disconnect_and_stop(client, true)
            kyclient_client_unref(client)
        }
    }

    public func connect() throws {
        try check(kyclient_client_connect(client), String(localized: "connexion à \(host)", bundle: .kyberKit))
    }

    /// Démarre l'image de `display` dans `view`, avec le son et les entrées.
    public func startStreaming(display: KyberDisplay, view: StreamView, options: StreamOptions) throws {
        // Une seule image à la fois : deux chaînes d'entrées se disputeraient
        // les manettes (SDL n'en sert qu'une), et l'ancienne, débranchée du
        // réseau, rejetterait tout.
        if input != nil {
            KyberLog.app.notice("image déjà démarrée : arrêt de la précédente")
            stopStreaming()
        }
        // Les gestionnaires d'entrées ne redémarrent pas : un pipeline neuf par session.
        let input = InputPipeline(displayID: display.id, hostSize: CGSize(width: display.width, height: display.height), pointer: view.pointer, clipboard: options.clipboard)
        input.gamepad.onCombo = { [weak self] in self?.onGamepadCombo?() }
        input.gamepad.onNavigate = { [weak self] key in self?.onGamepadNavigate?(key) }
        self.input = input
        self.view = view
        hostSize = (display.width, display.height)
        listSize = hostSize
        frameSize = nil
        view.attach(input: input, hostSize: CGSize(width: display.width, height: display.height))

        let protocolRef = kyclient_streaming_protocol_kymux(
            options.reliable ? KYCLIENT_KYMUX_VIDEO_PROTOCOL_RELIABLE : KYCLIENT_KYMUX_VIDEO_PROTOCOL_UNRELIABLE_FEC,
            // Son non fiable avec correction d'erreurs : un paquet perdu est
            // masqué par Opus au lieu d'être retransmis (plus de retard qui
            // s'accumule sur Wi-Fi). L'ordinateur doit annoncer la taille de
            // ses paquets (correctif serveur 0008).
            KYCLIENT_KYMUX_AUDIO_PROTOCOL_UNRELIABLE_FEC
        )
        defer { kyclient_streaming_protocol_unref(protocolRef) }
        var session: OpaquePointer?
        try check(kyclient_streaming_session_config_default(protocolRef, &session), String(localized: "configuration de la session", bundle: .kyberKit))
        defer { kyclient_streaming_session_config_unref(session) }
        kyclient_streaming_session_config_set_bitrate(session, options.bitrate)

        let viewPointer = UInt64(UInt(bitPattern: Unmanaged.passUnretained(view.videoView).toOpaque()))
        #if os(macOS)
        let handle = kyclient_windowshandle_from_ns_view(viewPointer)
        #else
        let handle = kyclient_windowshandle_from_ui_view(viewPointer)
        #endif
        var player: OpaquePointer?
        try check(kyclient_video_player_config_default(handle, &player), String(localized: "lecteur vidéo", bundle: .kyberKit))
        defer { kyclient_video_player_config_unref(player) }
        // La session vidéo recopie les réglages du lecteur à sa création : la
        // sortie vers l'app doit être posée avant (sinon l'image part à VLC).
        if options.nativeVideo, options.codec != .av1 || DecoderCapabilities.current.av1, let pipeline = VideoPipeline() {
            pipeline.renderer.vsync = options.vsync
            pipeline.renderer.paused = occluded
            latency.reset()
            pipeline.renderer.latency = latency
            pipeline.renderer.onStats = { [weak self, weak pipeline] stats in
                DispatchQueue.main.async {
                    guard let self else { return }
                    // Mesures d'une image arrêtée entre-temps : sa taille ne compte plus.
                    if let pipeline, self.video === pipeline { self.decoded(width: stats.width, height: stats.height) }
                    self.onVideoStats?(stats)
                }
            }
            pipeline.decoder.onLostReference = { [weak self] in DispatchQueue.main.async { self?.keyframeNeeded() } }
            pipeline.decoder.onRecovered = { [weak self] in DispatchQueue.main.async { self?.keyframeArrived() } }
            kyclient_video_player_config_set_packet_sink(player, VideoPipeline.callback, PacketSinkLease.lease(pipeline))
            retire(self.video)
            self.video = pipeline
            view.surface.show(pipeline.renderer)
            KyberLog.video.notice("image décodée par l'app (VideoToolbox, Metal)")
        } else {
            view.surface.show(nil)
            KyberLog.video.notice("image décodée par VLC (\(options.codec.rawValue, privacy: .public))")
        }
        var video: OpaquePointer?
        try check(kyclient_video_session_config_default(display.id, player, &video), String(localized: "configuration vidéo", bundle: .kyberKit))
        kyclient_video_session_config_set_codec(video, options.codec.sdk)
        kyclient_video_session_config_set_yuv444(video, options.yuv444)
        kyclient_streaming_session_config_add_video(session, video)

        if options.audio {
            var audio: OpaquePointer?
            try check(kyclient_audio_session_config_default(&audio), String(localized: "configuration du son", bundle: .kyberKit))
            if options.nativeAudio {
                let player = AudioPlayer()
                player.onStats = { [weak self] in self?.onAudioStats?($0) }
                kyclient_audio_session_config_set_packet_sink(audio, AudioPlayer.callback, PacketSinkLease.lease(player))
                retire(self.audio)
                self.audio = player
            }
            kyclient_streaming_session_config_set_audio(session, audio)
        }

        var inputConfig: OpaquePointer?
        try check(kyclient_input_session_config_default(kynput_pipeline_ref(input.pipeline), &inputConfig), String(localized: "configuration des entrées", bundle: .kyberKit))
        kyclient_input_session_config_clipboard(inputConfig, options.clipboard)
        // Manette vue par l'ordinateur (réglage par ordinateur) : Xbox 360
        // pour toutes, Sony comprises, ou fidèle au modèle.
        kyclient_input_session_config_gamepad_force_xbox360(inputConfig, options.gamepadAsXbox360)
        kyclient_streaming_session_config_set_input(session, inputConfig)

        try check(kyclient_client_start_streaming(client, session), String(localized: "démarrage de l'image", bundle: .kyberKit))
        input.startClipboard()
    }

    public func stopStreaming() {
        stopKeyframeWait()
        let old = input
        old?.stop()
        input = nil
        kyclient_client_stop_streaming(client)
        // Kyber arrête lui-même la chaîne d'entrées de la session, sauf si la
        // connexion était déjà perdue : on s'en assure (manettes libérées).
        old?.shutdown()
        retire(video)
        video = nil
        retire(audio)
        audio = nil
    }

    // MARK: - Taille de l'écran de l'ordinateur

    /// Images d'une autre taille (txproto recrée l'encodeur quand le mode de
    /// l'écran change, le décodeur suit les nouveaux SPS/PPS) : les positions
    /// de souris se calent sur elles, avant même que Kyber annonce la liste.
    /// Seul un *changement* de taille compte : la première image peut être
    /// à l'échelle (encodeur limité, libavconv), la liste reste alors la
    /// référence. Une image qui change de taille en cours de flux annonce un
    /// changement de mode, que la liste confirmera ou corrigera.
    private func decoded(width: Int, height: Int) {
        guard width > 0, height > 0 else { return }
        guard let previous = frameSize else {
            frameSize = (width, height)
            return
        }
        guard previous != (width, height) else { return }
        frameSize = (width, height)
        // Quelques pixels d'écart : l'alignement de l'encodeur (2234 → 2240
        // lignes), pas un changement de mode.
        if abs(width - hostSize.width) <= 16, abs(height - hostSize.height) <= 16 { return }
        setHostSize(width: width, height: height, from: "image décodée")
    }

    /// Taille que la liste de Kyber donne à l'écran affiché (à chaque liste).
    /// Quand elle change, elle fait foi : avec libavconv, l'image garde sa
    /// taille de départ (mise à l'échelle) alors que les positions suivent
    /// l'écran, et avec VLC l'app ne voit pas les images.
    public func displayResized(width: Int, height: Int) {
        guard width > 0, height > 0, listSize != (width, height) else { return }
        listSize = (width, height)
        setHostSize(width: width, height: height, from: "liste des écrans")
    }

    private func setHostSize(width: Int, height: Int, from source: String) {
        guard input != nil, hostSize != (width, height) else { return }
        KyberLog.app.notice("écran de l'ordinateur : \(self.hostSize.width)×\(self.hostSize.height) → \(width)×\(height) (\(source, privacy: .public))")
        hostSize = (width, height)
        view?.setHostSize(CGSize(width: width, height: height))
        onHostSizeChanged?(width, height)
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
        latency.keyframeRequested()
        kyclient_client_request_keyframe(client)
        let timer = Timer(timeInterval: 0.3, repeats: true) { [weak self] _ in
            guard let self, let asked = self.keyframeAsked, let client = self.client else { return }
            if Date().timeIntervalSince(asked) > 1.5 {
                KyberLog.video.error("pas d'image clé en 1,5 s : relance du flux")
                self.stopKeyframeWait()
                self.onVideoStalled?()
            } else {
                self.latency.keyframeRequested()
                kyclient_client_request_keyframe(client)
            }
        }
        // Mode « common » : la relance tire aussi pendant un menu ouvert ou un
        // redimensionnement, où la boucle principale quitte le mode par défaut.
        RunLoop.main.add(timer, forMode: .common)
        keyframeTimer = timer
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

    /// Fenêtre occultée ou réduite : plus de rendu (le décodeur continue, les
    /// références restent bonnes) ; le débit est à baisser à côté (`setBitrate`).
    public func setOccluded(_ occluded: Bool) {
        self.occluded = occluded
        video?.renderer.paused = occluded
    }

    private var occluded = false

    /// Kyber peut finir de livrer un paquet après l'arrêt : le pipeline vit
    /// encore un peu, et sa mémoire n'est rendue qu'au dernier appel de Kyber
    /// (`CLOSED`, voir `PacketSinkLease`), ou 30 s plus tard si une
    /// bibliothèque d'avant ce signal ne l'envoie pas.
    private func retire(_ pipeline: VideoPipeline?) {
        guard let pipeline else { return }
        DispatchQueue.main.asyncAfter(deadline: .now() + 2) { pipeline.stop() }
        PacketSinkLease.releaseLater(VideoPipeline.self, pipeline)
    }

    private func retire(_ player: AudioPlayer?) {
        guard let player else { return }
        // La sortie se tait tout de suite ; le lecteur vit encore un peu (Kyber peut finir un paquet).
        player.volume = 0
        DispatchQueue.main.asyncAfter(deadline: .now() + 2) { player.stop() }
        PacketSinkLease.releaseLater(AudioPlayer.self, player)
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
            return .streamingFailed(String(localized: "l'ordinateur n'a pas pu démarrer l'image", bundle: .kyberKit))
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
            switch type {
            case KYCLIENT_STREAMER_PIPELINE_TYPE_AUDIO: return .pipelineError(String(localized: "erreur côté ordinateur (son)", bundle: .kyberKit))
            case KYCLIENT_STREAMER_PIPELINE_TYPE_VIDEO: return .pipelineError(String(localized: "erreur côté ordinateur (image)", bundle: .kyberKit))
            default: return .pipelineError(String(localized: "erreur côté ordinateur (écrans)", bundle: .kyberKit))
            }
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
        case KYCLIENT_CONNECT_FAILED_REASON_UNAUTHORIZED: return String(localized: "identifiant ou mot de passe refusé", bundle: .kyberKit)
        case KYCLIENT_CONNECT_FAILED_REASON_TIMED_OUT: return String(localized: "l'ordinateur ne répond pas", bundle: .kyberKit)
        case KYCLIENT_CONNECT_FAILED_REASON_TLS_VERIFICATION_FAILED: return String(localized: "le certificat de l'ordinateur a changé", bundle: .kyberKit)
        case KYCLIENT_CONNECT_FAILED_REASON_SERVER_ERROR: return String(localized: "erreur du serveur de streaming", bundle: .kyberKit)
        default: return String(localized: "connexion impossible", bundle: .kyberKit)
        }
    }
}

private extension kyclient_StopReason {
    var text: String {
        switch self {
        case KYCLIENT_STOP_REASON_REQUESTED: return String(localized: "session terminée", bundle: .kyberKit)
        case KYCLIENT_STOP_REASON_CONTROL_PLANE_TIMEOUT: return String(localized: "l'ordinateur ne répond plus", bundle: .kyberKit)
        case KYCLIENT_STOP_REASON_DECODE_ERROR: return String(localized: "image illisible", bundle: .kyberKit)
        default: return String(localized: "connexion perdue", bundle: .kyberKit)
        }
    }
}
