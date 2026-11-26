// Profil « Auto » (docs/profil-auto.md) vu de Swift, commun au Mac, à
// l'Apple TV, à l'iPhone et à l'iPad : le test des conditions mené par le
// cœur (`kn_autotest_start`, sur son fil), avec la console de l'ordinateur et
// le vrai décodeur de l'app (`VideoDecoder` : VideoToolbox, PyroWave sur
// Metal), et les aides de son résultat (résumé, réglages, réseau).

import Foundation
import Network
#if os(macOS)
import CoreWLAN
#endif
@_implementationOnly import CKyberNow

/// Ce que l'app dit de l'appareil pour le test.
public struct AutoTestDevice: Sendable {
    /// L'écran de l'appareil, en pixels.
    public var width: Int
    public var height: Int
    public var refresh: Int
    /// Le choix « HDR » de l'app pour cet ordinateur.
    public var hdr: Bool
    /// Le réseau du moment (`CurrentNetwork.name`) ; nil : inconnu.
    public var network: String?
    /// Relié par câble (Ethernet).
    public var wired: Bool
    /// Le nom de l'ordinateur (texte d'un échec).
    public var name: String

    public init(width: Int, height: Int, refresh: Int, hdr: Bool, network: String?, wired: Bool, name: String) {
        self.width = width
        self.height = height
        self.refresh = refresh
        self.hdr = hdr
        self.network = network
        self.wired = wired
        self.name = name
    }

    var json: String {
        var j: [String: Any] = [
            "screen": ["width": width, "height": height, "refresh": refresh],
            "caps": SessionEngine.capabilitiesJSON(.current),
            "hdr": hdr, "wired": wired, "name": name,
        ]
        if let network { j["network"] = network }
        let data = (try? JSONSerialization.data(withJSONObject: j)) ?? Data()
        return String(decoding: data, as: UTF8.self)
    }
}

/// Une étape du test, telle que le panneau la montre.
public struct AutoTestStep: Sendable, Equatable, Identifiable {
    public enum State: Sendable, Equatable {
        case waiting
        case running
        /// Cochée, avec sa mesure (« 2,1 ms · gigue 0,4 ms »).
        case done(String)
        /// Passée, avec sa raison.
        case skipped(String)
    }

    /// `link`, `latency`, `bandwidth`, `decode`.
    public let key: String
    public let title: String
    public let state: State
    public var id: String { key }

    public init(key: String, title: String, state: State) {
        self.key = key
        self.title = title
        self.state = state
    }

    static func parse(_ json: String) -> [AutoTestStep] {
        guard let data = json.data(using: .utf8),
              let root = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
              let steps = root["steps"] as? [[String: Any]] else { return [] }
        return steps.map { s in
            let text = s["text"] as? String ?? ""
            let state: State
            switch s["state"] as? String {
            case "running": state = .running
            case "done": state = .done(text)
            case "skipped": state = .skipped(text)
            default: state = .waiting
            }
            return AutoTestStep(key: s["step"] as? String ?? "", title: s["title"] as? String ?? "", state: state)
        }
    }
}

/// Une ligne du résumé : réglage, valeur, raison.
public struct AutoTestChoice: Sendable, Equatable, Identifiable {
    /// `codec`, `mode`, `yuv444`, `bitrate`, `transport`, `adaptive`.
    public let key: String
    public let title: String
    public let value: String
    public let reason: String
    public var id: String { key }
}

/// Le résultat du test (`AutoResult` du cœur), gardé en JSON par l'app pour
/// chaque ordinateur ; ses champs utiles lus ici.
public struct AutoTestResult: Sendable, Equatable {
    public let json: String
    /// Date du test.
    public let at: Date
    /// Le réseau du test (`relay` par le relais), nil : inconnu.
    public let network: String?
    /// Le format choisi (`pyrowave` : PyroWave par câble, débit de HEVC sinon).
    public let codec: VideoCodec
    public let yuv444: Bool
    public let width: Int
    public let height: Int
    public let refresh: Int
    /// Débit de base, calé sur la 4K à 60 Hz comme celui des profils.
    public let mbps: Int
    public let reliable: Bool
    /// Le test a gardé la définition et la fréquence de l'écran (« comme cet écran »).
    public let matchesDisplay: Bool

    public init?(json: String) {
        guard let data = json.data(using: .utf8),
              let j = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
              let at = j["at"] as? NSNumber,
              let codec = (j["codec"] as? String).flatMap(VideoCodec.init(rawValue:)),
              let width = j["width"] as? Int, let height = j["height"] as? Int, let refresh = j["refresh"] as? Int,
              let mbps = j["mbps"] as? Int else { return nil }
        self.json = json
        self.at = Date(timeIntervalSince1970: at.doubleValue)
        network = j["network"] as? String
        self.codec = codec
        yuv444 = j["yuv444"] as? Bool ?? false
        self.width = width
        self.height = height
        self.refresh = refresh
        self.mbps = mbps
        reliable = j["reliable"] as? Bool ?? false
        matchesDisplay = (j["reasons"] as? [String: Any])?["mode"] as? String == "matches_display"
    }

    /// Le résumé, une ligne par réglage, dans la langue de l'app.
    public var choices: [AutoTestChoice] {
        guard let text = Core.take(kn_autotest_choices(json, LanguageChoice.effective)),
              let data = text.data(using: .utf8),
              let list = try? JSONSerialization.jsonObject(with: data) as? [[String: Any]] else { return [] }
        return list.map {
            AutoTestChoice(key: $0["key"] as? String ?? "", title: $0["title"] as? String ?? "",
                           value: $0["value"] as? String ?? "", reason: $0["reason"] as? String ?? "")
        }
    }

    /// Les réglages de session d'Auto (`AutoResult::settings` du cœur) :
    /// profil `auto`, le format, le 4:4:4, le débit et le transport choisis.
    /// `hdr` faux : le 4:4:4 choisi est gardé tel quel (le cœur l'ôte en
    /// session quand le HDR est demandé).
    public func settings(hdr: Bool = false, wired: Bool) -> SessionEngine.Settings? {
        guard let text = Core.take(kn_autotest_settings(json, hdr, wired)),
              let data = text.data(using: .utf8),
              let j = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
              let codec = (j["codec"] as? String).flatMap(VideoCodec.init(rawValue:)) else { return nil }
        return SessionEngine.Settings(profile: j["profile"] as? String, codec: codec, yuv444: j["yuv444"] as? Bool ?? false,
                                      mbps: j["mbps"] as? Int ?? mbps, reliable: j["reliable"] as? Bool ?? false,
                                      hdr: j["hdr"] as? Bool ?? hdr, wired: j["wired"] as? Bool ?? wired,
                                      adaptive: j["adaptive"] as? Bool ?? true, encoderMax: j["encoder_max"] as? Int)
    }

    /// Le plafond de l'encodeur à garder pour cet ordinateur quand ce
    /// résultat est appliqué (`AutoResult::next_encoder_max` du cœur), d'après
    /// celui qu'il avait : la limite trouvée par le test ; sinon un plafond
    /// gardé plus bas que le débit accepté du premier coup est retiré.
    public func nextEncoderMax(previous: Int?) -> Int? {
        let n = kn_autotest_next_encoder_max(json, UInt32(max(0, previous ?? 0)))
        return n > 0 ? Int(n) : nil
    }

    /// Le test a-t-il été fait sur ce réseau ? (`CurrentNetwork.name`, `relay`
    /// par le relais.)
    public func sameNetwork(_ network: String?) -> Bool {
        kn_autotest_same_network(json, network)
    }
}

/// Comment le test s'est terminé.
public enum AutoTestOutcome: Sendable {
    case result(AutoTestResult)
    /// Le texte de l'échec (« Le test n'a pas pu joindre {nom}. »).
    case failed(String)
    case cancelled
}

/// Un test en cours. `cancel()` l'arrête ; `onDone` est toujours appelé, une
/// fois, sur le fil principal.
public final class AutoTestRun: @unchecked Sendable {
    private final class Box {
        let onProgress: @MainActor ([AutoTestStep]) -> Void
        let onDone: @MainActor (AutoTestOutcome) -> Void
        init(onProgress: @escaping @MainActor ([AutoTestStep]) -> Void, onDone: @escaping @MainActor (AutoTestOutcome) -> Void) {
            self.onProgress = onProgress
            self.onDone = onDone
        }
    }

    private let handle: OpaquePointer

    /// Lance le test avec la console de l'ordinateur (réseau ou relais) ; nil
    /// si le cœur l'a refusé (et `onDone` n'est pas appelé).
    public init?(console: ConsoleClient, device: AutoTestDevice,
                 onProgress: @escaping @MainActor ([AutoTestStep]) -> Void,
                 onDone: @escaping @MainActor (AutoTestOutcome) -> Void) {
        let box = Unmanaged.passRetained(Box(onProgress: onProgress, onDone: onDone)).toOpaque()
        var cb = KnAutoCallbacks()
        cb.userdata = box
        cb.decode = { _, sample in
            AutoTestBench.fps(sample) ?? -1
        }
        cb.on_progress = { userdata, json in
            guard let userdata, let json else { return }
            let box = Unmanaged<Box>.fromOpaque(userdata).takeUnretainedValue()
            let steps = AutoTestStep.parse(String(cString: json))
            DispatchQueue.main.async { MainActor.assumeIsolated { box.onProgress(steps) } }
        }
        cb.on_done = { userdata, result, error, cancelled in
            guard let userdata else { return }
            let box = Unmanaged<Box>.fromOpaque(userdata).takeRetainedValue()
            let outcome: AutoTestOutcome
            if let result, let r = AutoTestResult(json: String(cString: result)) {
                outcome = .result(r)
            } else if cancelled {
                outcome = .cancelled
            } else {
                outcome = .failed(error.map { String(cString: $0) } ?? "")
            }
            DispatchQueue.main.async { MainActor.assumeIsolated { box.onDone(outcome) } }
        }
        guard let handle = console.withRaw({ kn_autotest_start($0, device.json, &cb) }) else {
            Unmanaged<Box>.fromOpaque(box).release()
            return nil
        }
        self.handle = handle
    }

    public func cancel() {
        kn_autotest_cancel(handle)
    }

    deinit {
        kn_autotest_free(handle)
    }
}

// MARK: - Décodage des extraits

extension AutoTestBench {
    /// L'extrait du cœur (`kn_auto_sample_*`, valable pendant le rappel),
    /// copié puis décodé par `run`.
    static func fps(_ sample: OpaquePointer?) -> Double? {
        guard let sample else { return nil }
        let fourcc = kn_auto_sample_fourcc(sample)
        guard let codec = VideoDecoder.Codec(rawValue: fourcc) else { return nil }
        var packets: [(data: Data, key: Bool)] = []
        for i in 0..<kn_auto_sample_packet_count(sample) {
            var len = 0
            var key = false
            guard let p = kn_auto_sample_packet(sample, i, &len, &key) else { return nil }
            packets.append((Data(bytes: p, count: len), key))
        }
        guard !packets.isEmpty else { return nil }
        let config = codec == .pyrowave
            ? PyroWaveConfig(chroma444: kn_auto_sample_yuv444(sample), hdr: false, fullRange: false,
                             width: Int(kn_auto_sample_width(sample)), height: Int(kn_auto_sample_height(sample)))
            : nil
        return run(codec: codec, packets: packets, pyrowave: config)
    }
}

// MARK: - Réseau du moment

/// Le nom du réseau de cet appareil, gardé avec le résultat du test et
/// comparé en début de session (« Autre réseau que celui du test ») :
/// « Ethernet » par câble ; le nom du Wi-Fi quand le système le donne (Mac
/// autorisé à la localisation), sinon « Wi-Fi » ; `cellular` en données mobiles.
public enum CurrentNetwork {
    public static var name: String? {
        switch NetworkLink.shared.kind {
        case .ethernet: return "Ethernet"
        case .wifi:
            #if os(macOS)
            if let ssid = CWWiFiClient.shared().interface()?.ssid(), !ssid.isEmpty { return ssid }
            #endif
            return "Wi-Fi"
        case .cellular: return "cellular"
        case .other: return nil
        }
    }
}
