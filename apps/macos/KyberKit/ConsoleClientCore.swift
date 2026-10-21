// Dialogue avec la console KyberNow de l'ordinateur, hors session : info,
// aperçu de l'écran, empreinte, appairage par code. Le cœur Rust fait les
// appels (certificat épinglé, `Accept-Language`) ; ici la même façade
// qu'avant le cœur, en Swift asynchrone. La préparation, le signal de vie et la fin de session sont
// dans le cœur, avec la session (SessionEngine).

import Foundation
#if os(iOS)
import UIKit
#endif
@_implementationOnly import CKyberNow

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
public struct EndReport: Sendable, Equatable {
    public let displayRestored: Bool
    public let audioRestored: Bool
    public let error: String?
}

public enum ConsoleError: LocalizedError {
    case http(Int, String)
    case certificateChanged
    /// Console injoignable ou réponse illisible (message du cœur).
    case network(String)

    public var errorDescription: String? {
        switch self {
        case let .http(code, message): return message.isEmpty ? String(localized: "la console a répondu \(code)", bundle: .kyberKit) : message
        case .certificateChanged: return String(localized: "le certificat de la console a changé : ce n'est peut-être pas votre ordinateur", bundle: .kyberKit)
        case let .network(message): return message
        }
    }
}

public final class ConsoleClient: @unchecked Sendable {
    private let console: OpaquePointer
    /// Les appels au cœur bloquent (jusqu'à 40 s pour une préparation) : une
    /// file à part, un appel à la fois par console.
    private let queue = DispatchQueue(label: "fr.leslignesclaires.kybernow.console", qos: .userInitiated)
    private let pinned: String?

    public init(host: String, port: UInt16, username: String, password: String, fingerprint: String?) {
        pinned = fingerprint
        console = kn_console_new(host, port, username, password, fingerprint)
    }

    deinit {
        kn_console_free(console)
    }

    /// Empreinte vue au premier contact (à enregistrer avec l'ordinateur).
    public var fingerprint: String? {
        Core.take(kn_console_fingerprint_seen(console)) ?? pinned
    }

    /// Un appel au cœur, sur la file de la console.
    private func call<T: Sendable>(_ body: @escaping @Sendable (OpaquePointer) throws -> T) async throws -> T {
        let console = self.console
        return try await withCheckedThrowingContinuation { continuation in
            queue.async {
                continuation.resume(with: Result { try body(console) })
            }
        }
    }

    /// Traduit un code de retour du cœur en erreur Swift.
    private static func check(_ code: Int32, _ error: UnsafeMutablePointer<CChar>?) throws {
        let message = Core.take(error) ?? ""
        switch code {
        case KN_OK: return
        case KN_CERTIFICATE_CHANGED: throw ConsoleError.certificateChanged
        case KN_ERROR: throw ConsoleError.network(message)
        default:
            if code < 0 { throw ConsoleError.http(Int(-code), message) }
            throw ConsoleError.network(message)
        }
    }

    private static func json(_ text: String?) -> [String: Any] {
        guard let text, let data = text.data(using: .utf8) else { return [:] }
        return (try? JSONSerialization.jsonObject(with: data)) as? [String: Any] ?? [:]
    }

    private static func hostInfo(_ json: [String: Any]) -> HostInfo {
        let vd = json["virtual_display"] as? [String: Any]
        let audio = json["audio"] as? [String: Any]
        return HostInfo(
            hostname: (json["hostname"] as? String).flatMap { $0.isEmpty ? nil : $0 },
            gpu: json["gpu"] as? String,
            av1: json["av1"] as? Bool ?? false,
            virtualDisplay: vd?["available"] as? Bool ?? false,
            speakers: audio?["speakers"] as? Bool ?? false,
            os: (json["os"] as? String).flatMap { $0.isEmpty ? nil : $0 }
        )
    }

    public func info() async throws -> HostInfo {
        try await call { console in
            var out: UnsafeMutablePointer<CChar>?
            var error: UnsafeMutablePointer<CChar>?
            try Self.check(kn_console_info(console, &out, &error), error)
            return Self.hostInfo(Self.json(Core.take(out)))
        }
    }

    /// Aperçu de l'écran en ce moment. Nil si la console ne sait pas le faire
    /// (version plus ancienne) ou ne peut pas capturer l'écran (personne de
    /// connecté, écran verrouillé, session en cours).
    public func preview(width: Int = 960) async -> ScreenPreview? {
        try? await call { console in
            var data: UnsafeMutablePointer<UInt8>?
            var len: UInt = 0
            var error: UnsafeMutablePointer<CChar>?
            try Self.check(kn_console_preview(console, UInt32(width), &data, &len, &error), error)
            guard let data, len > 0 else { return nil }
            defer { kn_bytes_free(data, len) }
            return ScreenPreview(jpeg: Data(bytes: data, count: Int(len)), capturedAt: Date())
        }
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
        let app = (Bundle.main.infoDictionary?["CFBundleShortVersionString"] as? String).map { "KyberNow \($0)" } ?? "KyberNow"
        #if os(macOS)
        let name = Host.current().localizedName ?? "Mac"
        return ["id": id, "name": name, "kind": "mac", "os": "macOS \(version)", "app": app]
        #elseif os(tvOS)
        return ["id": id, "name": "Apple TV", "kind": "tv", "os": "tvOS \(version)", "app": app]
        #else
        let kind = UIDevice.current.userInterfaceIdiom == .pad ? "ipad" : "iphone"
        return ["id": id, "name": UIDevice.current.name, "kind": kind, "os": "iOS \(version)", "app": app]
        #endif
    }

    /// L'appareil, en JSON pour le cœur.
    static var thisDeviceJSON: String {
        (try? JSONSerialization.data(withJSONObject: thisDevice)).flatMap { String(data: $0, encoding: .utf8) } ?? "{}"
    }

    /// Empreinte du certificat présenté par la console, sans compte : la
    /// poignée de main TLS suffit (la console répond ensuite « non autorisé »).
    public func probeFingerprint() async -> String? {
        _ = try? await call { console in
            var out: UnsafeMutablePointer<CChar>?
            var error: UnsafeMutablePointer<CChar>?
            let code = kn_console_probe(console, &out, &error)
            kn_string_free(out)
            try Self.check(code, error)
        }
        return fingerprint
    }

    // MARK: - Appairage par code

    /// Réponse à une demande d'appairage par code.
    public enum PairState: Sendable, Equatable {
        case pending
        case accepted(username: String, password: String)
        case refused
        case expired
    }

    /// Demande d'accès sans compte : la console affiche l'appareil et son code
    /// (Accès › Appareils), à accepter ou refuser. Rend l'identifiant de la demande.
    /// `ConsoleError.http(404, _)` : console d'une version qui ne le connaît pas.
    public func pairRequest(code: String) async throws -> String {
        let device = Self.thisDeviceJSON
        return try await call { console in
            var out: UnsafeMutablePointer<CChar>?
            var error: UnsafeMutablePointer<CChar>?
            try Self.check(kn_console_pair_request(console, code, device, &out, &error), error)
            guard let id = Core.take(out), !id.isEmpty else {
                throw ConsoleError.network(String(localized: "réponse inattendue de la console", bundle: .kyberKit))
            }
            return id
        }
    }

    public func pairStatus(_ id: String) async throws -> PairState {
        try await call { console in
            var out: UnsafeMutablePointer<CChar>?
            var error: UnsafeMutablePointer<CChar>?
            try Self.check(kn_console_pair_status(console, id, &out, &error), error)
            let json = Self.json(Core.take(out))
            switch json["status"] as? String {
            case "accepted":
                guard let user = json["username"] as? String, let password = json["password"] as? String else { return .refused }
                return .accepted(username: user, password: password)
            case "refused": return .refused
            case "expired": return .expired
            default: return .pending
            }
        }
    }
}
