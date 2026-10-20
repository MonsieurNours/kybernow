// Dialogue avec la console KyberNow de l'ordinateur (kyber-admin) :
// préparer la session (écran virtuel, son), signal de vie, fin.
//
// La console a un certificat auto-signé : on retient son empreinte SHA-256
// au premier contact, puis on refuse tout autre certificat.

import CryptoKit
import Foundation
import Security

public struct PrepareResult: Sendable {
    public let virtualDisplay: Bool
    public let kyberPort: UInt16?
    public let width: Int?
    public let height: Int?
    public let refresh: Int?
    public let heartbeatSeconds: Int
    public let audioChannels: Int?
}

/// Ce que la console dit de l'ordinateur.
public struct HostInfo: Sendable, Equatable {
    public let hostname: String?
    public let gpu: String?
    public let av1: Bool
    public let virtualDisplay: Bool
    public let speakers: Bool
    /// Système de l'ordinateur (« Windows 11 Pro 24H2 »), si la console le dit.
    public let os: String?
}

/// Aperçu de l'écran de l'ordinateur, capturé par la console.
public struct ScreenPreview: Sendable {
    public let jpeg: Data
    public let capturedAt: Date
}

/// Ce que la console confirme en fin de session.
public struct EndReport: Sendable {
    public let displayRestored: Bool
    public let audioRestored: Bool
    public let error: String?
}

public enum ConsoleError: LocalizedError {
    case http(Int, String)
    case certificateChanged

    public var errorDescription: String? {
        switch self {
        case let .http(code, message): return message.isEmpty ? "la console a répondu \(code)" : message
        case .certificateChanged: return "le certificat de la console a changé : ce n'est peut-être pas votre ordinateur"
        }
    }
}

public final class ConsoleClient: NSObject, URLSessionDelegate, @unchecked Sendable {
    private let base: URL
    private let authorization: String
    private let pinned: String?
    /// Empreinte vue au premier contact (à enregistrer avec l'ordinateur).
    public private(set) var fingerprint: String?
    private var changed = false
    private lazy var session = URLSession(configuration: .ephemeral, delegate: self, delegateQueue: nil)

    public init(host: String, port: UInt16, username: String, password: String, fingerprint: String?) {
        base = URL(string: "https://\(host.contains(":") ? "[\(host)]" : host):\(port)")!
        authorization = "Basic " + Data("\(username):\(password)".utf8).base64EncodedString()
        pinned = fingerprint
        self.fingerprint = fingerprint
    }

    private func call(_ method: String, _ path: String, body: [String: Any]? = nil, timeout: TimeInterval = 20) async throws -> [String: Any] {
        var request = URLRequest(url: base.appendingPathComponent(path))
        request.httpMethod = method
        request.timeoutInterval = timeout
        request.setValue(authorization, forHTTPHeaderField: "Authorization")
        request.setValue("1", forHTTPHeaderField: "x-kyber-admin")
        if let body {
            request.setValue("application/json", forHTTPHeaderField: "Content-Type")
            request.httpBody = try JSONSerialization.data(withJSONObject: body)
        }
        let data: Data
        let response: URLResponse
        do {
            (data, response) = try await session.data(for: request)
        } catch {
            if changed { throw ConsoleError.certificateChanged }
            throw error
        }
        let json = (try? JSONSerialization.jsonObject(with: data)) as? [String: Any] ?? [:]
        let code = (response as? HTTPURLResponse)?.statusCode ?? 0
        guard (200..<300).contains(code) else {
            throw ConsoleError.http(code, json["error"] as? String ?? "")
        }
        return json
    }

    public func info() async throws -> HostInfo {
        let json = try await call("GET", "api/stream/info", timeout: 4)
        let vd = json["virtual_display"] as? [String: Any]
        let audio = json["audio"] as? [String: Any]
        return HostInfo(
            hostname: json["hostname"] as? String,
            gpu: json["gpu"] as? String,
            av1: json["av1"] as? Bool ?? false,
            virtualDisplay: vd?["available"] as? Bool ?? false,
            speakers: audio?["speakers"] as? Bool ?? false,
            os: (json["os"] as? String).flatMap { $0.isEmpty ? nil : $0 }
        )
    }

    /// Aperçu de l'écran en ce moment (`GET /api/stream/preview`). Nil si la
    /// console ne sait pas le faire (version plus ancienne) ou ne peut pas
    /// capturer l'écran (personne de connecté, écran verrouillé).
    public func preview(width: Int = 960) async -> ScreenPreview? {
        var components = URLComponents(url: base.appendingPathComponent("api/stream/preview"), resolvingAgainstBaseURL: false)!
        components.queryItems = [URLQueryItem(name: "width", value: String(width))]
        var request = URLRequest(url: components.url!)
        request.timeoutInterval = 8
        request.setValue(authorization, forHTTPHeaderField: "Authorization")
        request.setValue("1", forHTTPHeaderField: "x-kyber-admin")
        guard let (data, response) = try? await session.data(for: request),
              let http = response as? HTTPURLResponse, http.statusCode == 200,
              (http.value(forHTTPHeaderField: "Content-Type") ?? "").contains("image/jpeg"), !data.isEmpty
        else { return nil }
        let at = http.value(forHTTPHeaderField: "X-Captured-At").flatMap { ISO8601DateFormatter().date(from: $0) } ?? Date()
        return ScreenPreview(jpeg: data, capturedAt: at)
    }

    /// Ce que l'app dit d'elle-même à la console (ses appareils, son fil
    /// d'événements) : un identifiant tiré à la première ouverture, le nom
    /// de l'appareil, son système et la version de l'app.
    public static var thisDevice: [String: Any] {
        let key = "kybernow.deviceID"
        let id = UserDefaults.standard.string(forKey: key) ?? {
            let id = UUID().uuidString
            UserDefaults.standard.set(id, forKey: key)
            return id
        }()
        let v = ProcessInfo.processInfo.operatingSystemVersion
        let version = v.patchVersion > 0 ? "\(v.majorVersion).\(v.minorVersion).\(v.patchVersion)" : "\(v.majorVersion).\(v.minorVersion)"
        let app = (Bundle.main.infoDictionary?["CFBundleShortVersionString"] as? String).map { "KyberNow \($0)" }
        #if os(macOS)
        let name = Host.current().localizedName ?? "Mac"
        return ["id": id, "name": name, "kind": "mac", "os": "macOS \(version)", "app": app as Any]
        #else
        return ["id": id, "name": "Apple TV", "kind": "tv", "os": "tvOS \(version)", "app": app as Any]
        #endif
    }

    /// Annonce l'écran de ce Mac et le son voulu, avant de se connecter à Kyber.
    public func prepare(width: Int, height: Int, refresh: Int, audioChannels: Int, virtualDisplay: Bool = true, profile: String? = nil) async throws -> PrepareResult {
        var body: [String: Any] = [
            "width": width, "height": height, "refresh": refresh, "audio_channels": audioChannels,
            "virtual_display": virtualDisplay, "device": Self.thisDevice,
        ]
        if let profile { body["profile"] = profile }
        // La console peut d'abord attendre qu'un autre appareil rende la main
        // (7 s au plus), puis créer l'écran virtuel (jusqu'à 15 s).
        let json = try await call("POST", "api/stream/prepare", body: body, timeout: 40)
        let applied = json["applied"] as? [String: Any]
        let audio = json["audio"] as? [String: Any]
        return PrepareResult(
            virtualDisplay: json["virtual_display"] as? Bool ?? false,
            kyberPort: (json["kyber_port"] as? NSNumber).map { UInt16(truncating: $0) },
            width: applied?["width"] as? Int,
            height: applied?["height"] as? Int,
            refresh: applied?["refresh"] as? Int,
            heartbeatSeconds: json["heartbeat_seconds"] as? Int ?? 5,
            audioChannels: (audio?["routed"] as? Bool ?? false) ? audio?["channels"] as? Int : nil
        )
    }

    /// Empreinte du certificat présenté par la console, sans compte : la
    /// poignée de main TLS suffit (la console répond ensuite « non autorisé »).
    public func probeFingerprint() async -> String? {
        _ = try? await call("GET", "api/stream/info", timeout: 6)
        return fingerprint
    }

    /// Ce qui occupe l'écran de l'ordinateur (veille de la console).
    public struct Foreground: Sendable {
        public let fullscreen: Bool
        public let game: Bool
        public let exe: String?
    }

    /// nil : console d'une version précédente, ou veille pas encore prête.
    public func foreground() async -> Foreground? {
        guard let json = try? await call("GET", "api/stream/foreground", timeout: 2),
              json["known"] as? Bool == true else { return nil }
        return Foreground(
            fullscreen: json["fullscreen"] as? Bool ?? false,
            game: json["game"] as? Bool ?? false,
            exe: json["exe"] as? String
        )
    }

    /// Réponse au signal de vie.
    public struct Beat: Sendable {
        /// La console demande la fin de la session : bouton « Mettre fin à la
        /// session », ou un autre appareil qui prend la main.
        public let end: Bool
        /// L'écran virtuel en place est celui de cet appareil (nil : console injoignable).
        public let displayActive: Bool?
    }

    /// Identifiant stable de cet appareil, le même que dans `prepare` : la
    /// console distingue ainsi deux appareils sur le même compte.
    private static var deviceID: String { thisDevice["id"] as? String ?? "" }

    /// Signal de vie, avec la latence du moment.
    public func heartbeat(latencyMs: Double? = nil, profile: String? = nil) async -> Beat {
        var body: [String: Any] = ["device_id": Self.deviceID]
        if let latencyMs { body["latency_ms"] = (latencyMs * 10).rounded() / 10 }
        if let profile { body["profile"] = profile }
        let json = try? await call("POST", "api/stream/heartbeat", body: body)
        return Beat(end: json?["end"] as? Bool ?? false, displayActive: json?["active"] as? Bool)
    }

    /// Fin de session : la console retire l'écran virtuel et rétablit le son.
    @discardableResult
    public func end(latencyMs: Double? = nil) async throws -> EndReport {
        var body: [String: Any] = ["device_id": Self.deviceID]
        if let latencyMs { body["latency_ms"] = (latencyMs * 10).rounded() / 10 }
        let json = try await call("POST", "api/stream/end", body: body)
        return EndReport(
            displayRestored: json["display_restored"] as? Bool ?? true,
            audioRestored: json["audio_restored"] as? Bool ?? true,
            error: json["error"] as? String
        )
    }

    // MARK: - Appairage par code (Apple TV)

    /// Réponse à une demande d'appairage par code.
    public enum PairState: Sendable, Equatable {
        case pending
        case accepted(username: String, password: String)
        case refused
        case expired
    }

    /// Demande d'accès sans compte : la console affiche l'appareil et son code
    /// (Appareils), à accepter ou refuser. Rend l'identifiant de la demande.
    /// `ConsoleError.http(404, _)` : console d'une version qui ne le connaît pas.
    public func pairRequest(code: String) async throws -> String {
        let json = try await call("POST", "api/pair/request", body: ["code": code, "device": Self.thisDevice], timeout: 6)
        guard let id = json["id"] as? String else { throw ConsoleError.http(0, "réponse inattendue de la console") }
        return id
    }

    public func pairStatus(_ id: String) async throws -> PairState {
        let json = try await call("GET", "api/pair/request/\(id)", timeout: 6)
        switch json["status"] as? String {
        case "accepted":
            guard let user = json["username"] as? String, let password = json["password"] as? String else { return .refused }
            return .accepted(username: user, password: password)
        case "refused": return .refused
        case "expired": return .expired
        default: return .pending
        }
    }

    // MARK: - Certificat

    public func urlSession(
        _ session: URLSession,
        didReceive challenge: URLAuthenticationChallenge,
        completionHandler: @escaping (URLSession.AuthChallengeDisposition, URLCredential?) -> Void
    ) {
        guard challenge.protectionSpace.authenticationMethod == NSURLAuthenticationMethodServerTrust,
              let trust = challenge.protectionSpace.serverTrust,
              let chain = SecTrustCopyCertificateChain(trust) as? [SecCertificate],
              let leaf = chain.first
        else {
            completionHandler(.performDefaultHandling, nil)
            return
        }
        let digest = SHA256.hash(data: SecCertificateCopyData(leaf) as Data)
        let seen = "sha256:" + digest.map { String(format: "%02x", $0) }.joined(separator: ":")
        if let pinned, pinned != seen {
            changed = true
            completionHandler(.cancelAuthenticationChallenge, nil)
            return
        }
        fingerprint = seen
        completionHandler(.useCredential, URLCredential(trust: trust))
    }
}
