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
            speakers: audio?["speakers"] as? Bool ?? false
        )
    }

    /// Annonce l'écran de ce Mac et le son voulu, avant de se connecter à Kyber.
    public func prepare(width: Int, height: Int, refresh: Int, audioChannels: Int, virtualDisplay: Bool = true) async throws -> PrepareResult {
        let json = try await call("POST", "api/stream/prepare", body: [
            "width": width, "height": height, "refresh": refresh, "audio_channels": audioChannels,
            "virtual_display": virtualDisplay,
        ])
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

    public func heartbeat() async {
        _ = try? await call("POST", "api/stream/heartbeat")
    }

    /// Fin de session : la console retire l'écran virtuel et rétablit le son.
    @discardableResult
    public func end() async throws -> EndReport {
        let json = try await call("POST", "api/stream/end")
        return EndReport(
            displayRestored: json["display_restored"] as? Bool ?? true,
            audioRestored: json["audio_restored"] as? Bool ?? true,
            error: json["error"] as? String
        )
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
