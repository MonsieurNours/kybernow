// Consoles PlayStation (docs/playstation.md) vues de Swift : recherche,
// identifiant du compte PSN, association, réveil, et la partie elle-même.
// Le protocole est dans le cœur Rust (apps/core/src/playstation, pont
// apps/apple/src/playstation.rs) ; l'image et le son arrivent en `KnPacket`
// comme ceux d'une session Kyber et passent par le même chemin
// (VideoPipeline : VideoToolbox et Metal, HDR compris ; AudioPlayer).
//
// Tout ce qui parle au réseau bloque : hors du fil de l'interface.

import Foundation
@_implementationOnly import CKyberNow

public enum PlayStation {
    /// Une console trouvée sur le réseau.
    public struct Found: Equatable, Sendable {
        public var address: String
        /// Identifiant de la console (le `mac` de son association).
        public var hostID: String
        public var name: String
        /// « PS5 » ou « PS4 ».
        public var model: String
        /// `ready`, `standby` ou `unknown`.
        public var state: String
        /// Pour `regist`.
        public var target: String
        public var runningApp: String?

        public init(address: String, hostID: String, name: String, model: String, state: String, target: String, runningApp: String?) {
            (self.address, self.hostID, self.name, self.model, self.state, self.target, self.runningApp) = (address, hostID, name, model, state, target, runningApp)
        }

        public var ready: Bool { state == "ready" }
        public var standby: Bool { state == "standby" }
    }

    /// Cherche les consoles pendant `wait` ms : en diffusion, et en direct
    /// vers les adresses déjà connues. Bloque.
    public static func search(direct: [String], wait: UInt32 = 2000) -> [Found] {
        Core.install()
        let json = (try? JSONSerialization.data(withJSONObject: direct)).flatMap { String(data: $0, encoding: .utf8) } ?? "[]"
        guard let text = Core.take(kn_ps_search(json, wait)),
              let list = try? JSONSerialization.jsonObject(with: Data(text.utf8)) as? [[String: Any]] else { return [] }
        return list.compactMap { o in
            guard let address = o["address"] as? String, let host = o["host_id"] as? String else { return nil }
            let app = (o["running_app"] as? String).flatMap { $0.isEmpty ? nil : $0 }
            return Found(address: address, hostID: host, name: o["name"] as? String ?? "PlayStation", model: o["model"] as? String ?? "PS5",
                         state: o["state"] as? String ?? "unknown", target: o["target"] as? String ?? "", runningApp: app)
        }
    }

    /// Page de connexion au compte Sony.
    public static var loginURL: URL? {
        Core.install()
        return Core.take(kn_ps_login_url()).flatMap(URL.init(string:))
    }

    /// L'adresse de retour de Sony : la fenêtre web s'arrête là.
    public static func isRedirect(_ url: URL) -> Bool {
        kn_ps_is_redirect(url.absoluteString)
    }

    /// Identifiant du compte (base64) d'après l'adresse de retour. Bloque.
    public static func account(redirect: URL) -> Result<String, AccountError> {
        guard let text = Core.take(kn_ps_account(redirect.absoluteString)),
              let o = try? JSONSerialization.jsonObject(with: Data(text.utf8)) as? [String: Any] else { return .failure(AccountError(detail: "")) }
        if let id = o["account"] as? String { return .success(id) }
        return .failure(AccountError(detail: o["error"] as? String ?? ""))
    }

    public struct AccountError: Error, Sendable {
        /// Pour le journal.
        public var detail: String
    }

    /// Refus d'une association.
    public enum RegistError: String, Error, Sendable {
        /// Écran « Associer un appareil » fermé.
        case noAnswer = "no_answer"
        case wrongCode = "wrong_code"
        case account
        case network
        case other
    }

    /// Associe la console : rend le JSON de l'association (clés comprises :
    /// au Trousseau). Bloque quelques secondes.
    public static func regist(address: String, target: String, account: String, pin: UInt32) -> Result<String, RegistError> {
        guard let text = Core.take(kn_ps_regist(address, target, account, pin)),
              let o = try? JSONSerialization.jsonObject(with: Data(text.utf8)) as? [String: Any] else { return .failure(.other) }
        if let r = o["registered"], let data = try? JSONSerialization.data(withJSONObject: r), let json = String(data: data, encoding: .utf8) {
            return .success(json)
        }
        let error = RegistError(rawValue: o["error"] as? String ?? "") ?? .other
        KyberLog.app.notice("PlayStation : association refusée (\(error.rawValue, privacy: .public))")
        return .failure(error)
    }

    /// Réveille une console associée. Vrai si le paquet est parti.
    public static func wake(address: String, registered: String) -> Bool {
        kn_ps_wake(address, registered)
    }

    /// Réglages par défaut d'une console (JSON du cœur : Auto, 60 i/s, plein écran).
    public static var defaultSettings: String {
        Core.install()
        return Core.take(kn_ps_default_settings()) ?? "{}"
    }

    /// Les réglages s'écartent d'Auto : « Personnalisé » à l'accueil.
    public static func settingsCustom(_ json: String) -> Bool {
        kn_ps_settings_custom(json)
    }

    /// Disposition par défaut du clavier et de la souris (JSON du cœur).
    public static var defaultLayout: String {
        Core.install()
        return Core.take(kn_ps_default_layout()) ?? "{}"
    }

    /// Identifiant de la console dans une association (`mac`, en majuscules).
    public static func hostID(ofRegistered json: String) -> String? {
        guard let o = try? JSONSerialization.jsonObject(with: Data(json.utf8)) as? [String: Any] else { return nil }
        return (o["mac"] as? String)?.uppercased()
    }
}

/// Une partie avec une console : l'image dans `view`, le son sur la sortie
/// du système. `onState` arrive sur le fil principal.
public final class PlayStationPlay: @unchecked Sendable {
    public enum State: Equatable, Sendable {
        case connecting
        case streaming(width: Int, height: Int, hdr: Bool)
        /// Contenu protégé : la console coupe l'image tant qu'il est affiché.
        case cantDisplay(Bool)
        case ended(End)
    }

    public enum End: Equatable, Sendable {
        case stopped, inUse, notPaired, unreachable, remote
    }

    public var onState: ((State) -> Void)?
    /// Options + Create + L1 + R1 tenus 250 ms : ouvrir ou fermer la barre
    /// (fil principal).
    public var onMenu: (() -> Void)?
    /// Barre ouverte (`setCaptured`) : touche de navigation de la manette
    /// (fil principal).
    public var onNavigate: ((PadKey) -> Void)?
    public let view: StreamView
    private let lock = NSLock()
    private var play: OpaquePointer?
    private var videos: [UInt64: VideoPipeline] = [:]
    private var audios: [UInt64: AudioPlayer] = [:]
    private var hdr = false
    private var stopped = false
    /// Son coupé depuis la barre (repris par chaque flux de son).
    private var silenced = false
    /// Trait de repère de la barre (repris par chaque flux d'image).
    private var marker = false
    /// Agrandissement MetalFX (repris par chaque flux d'image).
    private var upscale: UpscaleSettings?
    /// Dernier état de l'agrandissement (statistiques).
    public private(set) var upscaleReport: UpscaleReport? {
        get { lock.lock(); defer { lock.unlock() }; return report }
        set { lock.lock(); report = newValue; lock.unlock() }
    }
    private var report: UpscaleReport?

    public init(view: StreamView) {
        Core.install()
        self.view = view
    }

    /// Lance la partie (la console doit être allumée) avec les réglages de la
    /// console (`settings` : JSON, HDR compris) et sa disposition du clavier
    /// et de la souris. `hevcDecoder` : ce Mac décode le H.265. Faux si le
    /// cœur refuse (adresse ou association illisible).
    public func start(address: String, registered: String, settings: String, hevcDecoder: Bool, layout: String) -> Bool {
        var cb = KnPsCallbacks()
        cb.userdata = Unmanaged.passUnretained(self).toOpaque()
        cb.video_packet = { ud, p in
            guard let ud, let p else { return }
            Unmanaged<PlayStationPlay>.fromOpaque(ud).takeUnretainedValue().videoPacket(p.pointee)
        }
        cb.audio_packet = { ud, p in
            guard let ud, let p else { return }
            Unmanaged<PlayStationPlay>.fromOpaque(ud).takeUnretainedValue().audioPacket(p.pointee)
        }
        cb.on_state = { ud, state, w, h, hdr, detail in
            guard let ud else { return }
            let d = detail.map { String(cString: $0) } ?? ""
            Unmanaged<PlayStationPlay>.fromOpaque(ud).takeUnretainedValue().stateChanged(state, Int(w), Int(h), hdr, d)
        }
        cb.on_menu = { ud in
            guard let ud else { return }
            let me = Unmanaged<PlayStationPlay>.fromOpaque(ud).takeUnretainedValue()
            KyberLog.app.notice("PlayStation : combinaison Options + Create + L1 + R1")
            DispatchQueue.main.async { [weak me] in me?.onMenu?() }
        }
        cb.on_navigate = { ud, key in
            guard let ud else { return }
            let me = Unmanaged<PlayStationPlay>.fromOpaque(ud).takeUnretainedValue()
            let k: PadKey
            switch Int32(key) {
            case KN_PAD_KEY_LEFT: k = .left
            case KN_PAD_KEY_RIGHT: k = .right
            case KN_PAD_KEY_UP: k = .up
            case KN_PAD_KEY_DOWN: k = .down
            case KN_PAD_KEY_ACCEPT: k = .accept
            default: k = .back
            }
            DispatchQueue.main.async { [weak me] in me?.onNavigate?(k) }
        }
        KyberLog.app.notice("PlayStation : partie demandée (\(settings, privacy: .public), H.265 \(hevcDecoder ? "lu" : "pas lu", privacy: .public))")
        let p = withUnsafePointer(to: &cb) { kn_ps_start(address, registered, settings, hevcDecoder, $0) }
        // La disposition de la console avant toute entrée.
        if let p, !kn_ps_set_layout(p, layout) {
            KyberLog.app.notice("PlayStation : disposition illisible, celle par défaut est gardée")
        }
        lock.lock()
        play = p
        lock.unlock()
        return p != nil
    }

    // MARK: Clavier et souris (traduits en manette par le cœur)

    /// Fait `body` avec la partie si elle tourne (verrou tenu : `stop` la
    /// libère après avoir remis `play` à nil sous le même verrou).
    private func withPlay(_ body: (OpaquePointer) -> Void) {
        lock.lock()
        defer { lock.unlock() }
        if let p = play { body(p) }
    }

    func key(usage: UInt16, pressed: Bool) { withPlay { kn_ps_key($0, usage, pressed) } }
    func mouseButton(_ button: UInt8, pressed: Bool) { withPlay { kn_ps_mouse_button($0, button, pressed) } }
    func mouseMove(dx: Float, dy: Float) { withPlay { kn_ps_mouse_move($0, dx, dy) } }
    func releaseAll() { withPlay { kn_ps_release_all($0) } }

    /// Disposition du clavier et de la souris de la console (JSON du cœur).
    @discardableResult
    public func setLayout(_ json: String) -> Bool {
        var ok = false
        withPlay { ok = kn_ps_set_layout($0, json) }
        return ok
    }

    /// La barre s'ouvre (`true`) ou se ferme : tant qu'elle est ouverte, la
    /// console reçoit une manette au repos, le clavier et la souris ne vont
    /// plus à la console et la manette navigue dans la barre (`onNavigate`).
    public func setCaptured(_ on: Bool) { withPlay { kn_ps_set_captured($0, on) } }

    /// Mesures de la partie, une fois par seconde.
    public struct Stats: Equatable, Sendable {
        public var width = 0, height = 0
        public var hevc = false, hdr = false
        /// Débit demandé à la console, kbit/s.
        public var targetKbps = 0
        /// Reçu depuis la lecture précédente.
        public var fps = 0.0, mbps = 0.0
        /// Images perdues et réparées depuis la lecture précédente.
        public var lost = 0, recovered = 0
    }

    /// Mesures depuis l'appel précédent ; nil sans partie.
    public func stats() -> Stats? {
        var text: String?
        withPlay { text = Core.take(kn_ps_stats($0)) }
        guard let text, let o = try? JSONSerialization.jsonObject(with: Data(text.utf8)) as? [String: Any] else { return nil }
        let int = { (k: String) in (o[k] as? NSNumber)?.intValue ?? 0 }
        let num = { (k: String) in (o[k] as? NSNumber)?.doubleValue ?? 0 }
        return Stats(width: int("width"), height: int("height"), hevc: o["hevc"] as? Bool ?? false, hdr: o["hdr"] as? Bool ?? false,
                     targetKbps: int("target_kbps"), fps: num("fps"), mbps: num("mbps"), lost: int("lost"), recovered: int("recovered"))
    }

    /// Son coupé (« Couper le son ») : la partie continue, en silence.
    public var muted: Bool {
        get { lock.withLock { silenced } }
        set {
            lock.lock()
            silenced = newValue
            let players = Array(audios.values)
            lock.unlock()
            players.forEach { $0.volume = newValue ? 0 : 1 }
        }
    }

    /// Trait de repère de la barre, tracé dans l'image : allumé quand le
    /// geste qui ouvre la barre est possible.
    public func setTopMarker(_ on: Bool) {
        lock.lock()
        marker = on
        let renderers = videos.values.map(\.renderer)
        lock.unlock()
        renderers.forEach { $0.topMarker = on }
    }

    /// Agrandissement de l'image (nil : filtre bilinéaire), tout de suite.
    public func setUpscale(_ u: UpscaleSettings?) {
        lock.lock()
        upscale = u
        let renderers = videos.values.map(\.renderer)
        lock.unlock()
        renderers.forEach { $0.upscale = u }
    }

    /// Met la console en mode repos (la partie se termine ensuite).
    public func gotoBed() {
        lock.lock()
        let p = play
        lock.unlock()
        if let p { kn_ps_goto_bed(p) }
    }

    /// Arrête la partie et attend sa fin : hors des rappels du cœur, et pas
    /// sur le fil principal (l'attente peut durer).
    public func stop() {
        lock.lock()
        let p = play
        play = nil
        stopped = true
        let videos = Array(self.videos.values), audios = Array(self.audios.values)
        self.videos = [:]
        self.audios = [:]
        lock.unlock()
        if let p { kn_ps_stop(p) }
        videos.forEach { $0.stop() }
        audios.forEach { $0.stop() }
    }

    // MARK: - Rappels (fils du cœur)

    private func stateChanged(_ code: Int32, _ w: Int, _ h: Int, _ hdr: Bool, _ detail: String) {
        let state: State
        switch code {
        case KN_PS_CONNECTING: state = .connecting
        case KN_PS_STREAMING:
            state = .streaming(width: w, height: h, hdr: hdr)
            lock.lock()
            self.hdr = hdr
            let renderers = videos.values.map(\.renderer)
            lock.unlock()
            renderers.forEach { $0.streamHDR = hdr }
        case KN_PS_CANT_DISPLAY: state = .cantDisplay(true)
        case KN_PS_CAN_DISPLAY: state = .cantDisplay(false)
        case KN_PS_END_IN_USE: state = .ended(.inUse)
        case KN_PS_END_NOT_PAIRED: state = .ended(.notPaired)
        case KN_PS_END_UNREACHABLE: state = .ended(.unreachable)
        case KN_PS_END_REMOTE: state = .ended(.remote)
        default: state = .ended(.stopped)
        }
        if case .ended = state {
            KyberLog.app.notice("PlayStation : fin de la partie (\(code)) \(detail, privacy: .public)")
        }
        DispatchQueue.main.async { [weak self] in self?.onState?(state) }
    }

    private func pipeline(for stream: UInt64) -> VideoPipeline? {
        lock.lock()
        if let p = videos[stream] {
            lock.unlock()
            return p
        }
        guard !stopped, let pipeline = VideoPipeline() else {
            lock.unlock()
            return nil
        }
        pipeline.renderer.streamHDR = hdr
        pipeline.renderer.topMarker = marker
        pipeline.renderer.upscale = upscale
        pipeline.renderer.onUpscale = { [weak self] r in
            self?.upscaleReport = r
            KyberLog.video.notice("PlayStation : agrandissement \(String(describing: r.state), privacy: .public)")
        }
        // Référence perdue : le cœur redemande une image clé à la console
        // jusqu'à ce qu'elle arrive (verrou tenu : `stop` libère la partie
        // après avoir remis `play` à nil sous le même verrou).
        pipeline.decoder.onLostReference = { [weak self] in
            guard let self else { return }
            self.lock.lock()
            if let p = self.play { kn_ps_keyframe_lost(p) }
            self.lock.unlock()
        }
        pipeline.decoder.onRecovered = { [weak self] in
            guard let self else { return }
            self.lock.lock()
            if let p = self.play { kn_ps_keyframe_recovered(p) }
            self.lock.unlock()
        }
        videos[stream] = pipeline
        lock.unlock()
        KyberLog.video.notice("PlayStation : image décodée par l'app (VideoToolbox, Metal), flux \(stream)")
        DispatchQueue.main.async { [view] in view.surface.show(pipeline.renderer) }
        return pipeline
    }

    private func videoPacket(_ p: KnPacket) {
        switch Int32(p.kind) {
        case KN_PACKET_CLOSED:
            lock.lock()
            let pipeline = videos.removeValue(forKey: p.stream)
            lock.unlock()
            pipeline?.stop()
        case KN_PACKET_CODEC:
            pipeline(for: p.stream)?.decoder.setCodec(p.codec)
        case KN_PACKET_MEDIA:
            guard let data = p.data, p.len > 0, let pipeline = pipeline(for: p.stream) else { return }
            pipeline.decoder.decode(UnsafeBufferPointer(start: data, count: Int(p.len)), isConfig: p.is_config, isKey: p.is_key, pts: p.pts)
        case KN_PACKET_HOLE:
            pipeline(for: p.stream)?.decoder.dataLost()
        default:
            break
        }
    }

    private func player(for stream: UInt64) -> AudioPlayer? {
        lock.lock()
        defer { lock.unlock() }
        if let p = audios[stream] { return p }
        guard !stopped else { return nil }
        let player = AudioPlayer()
        if silenced { player.volume = 0 }
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
            guard let data = p.data, p.len > 0, let player = player(for: p.stream) else { return }
            let buffer = UnsafeBufferPointer(start: data, count: Int(p.len))
            if p.is_config { player.configure(opusHead: buffer) } else { player.decode(buffer) }
        case KN_PACKET_HOLE:
            player(for: p.stream)?.conceal(samples: Int(p.missing_samples))
        default:
            break
        }
    }
}
