// Dialogue avec KyberNow Admin (kyber-admin) sur l'ordinateur, hors session : info,
// aperçu de l'écran, empreinte, appairage par code. Le cœur Rust fait les
// appels (certificat épinglé, `Accept-Language`) ; ici, leur façade en
// Swift asynchrone. La préparation, le signal de vie et la fin de session
// sont dans le cœur, avec la session (SessionEngine).

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
    /// Version d'Admin (celle de la Station), si elle la dit.
    public let adminVersion: String?
    /// La Station (0.5.0 et suivantes) ; nil avec une Station plus ancienne,
    /// qui ne sait ni redémarrer ni se mettre à jour depuis un appareil.
    public let station: StationInfo?
    /// Les cartes réseau de l'ordinateur pour le réveiller (Ethernet
    /// d'abord) ; vide avec une Station d'avant la 0.5.0.
    public let wake: [WakeTarget]
    /// Le compte de cet appareil a été créé par l'appairage par code.
    public let ownDevice: Bool
    /// Écrans que la Station sait créer en une session (multi-écrans) ; 1
    /// avec une Station antérieure.
    public let maxScreens: Int
    /// « Partager » (KyberNow Share) : disponible, ou la raison pour laquelle
    /// il ne l'est pas (Station trop ancienne, partage coupé sur l'ordinateur).
    public let share: ShareAvailability

    public init(hostname: String? = nil, gpu: String? = nil, av1: Bool = false, virtualDisplay: Bool = true, speakers: Bool = true,
                os: String? = nil, adminVersion: String? = nil, station: StationInfo? = nil, wake: [WakeTarget] = [], ownDevice: Bool = false, maxScreens: Int = 1,
                share: ShareAvailability = .available) {
        self.hostname = hostname
        self.gpu = gpu
        self.av1 = av1
        self.virtualDisplay = virtualDisplay
        self.speakers = speakers
        self.os = os
        self.adminVersion = adminVersion
        self.station = station
        self.wake = wake
        self.ownDevice = ownDevice
        self.maxScreens = max(1, maxScreens)
        self.share = share
    }

    /// Version de la Station : celle de `station`, sinon celle d'Admin.
    public var stationVersion: String? {
        station.map(\.version).flatMap { $0.isEmpty ? nil : $0 } ?? adminVersion
    }
}

/// La Station, pour la rubrique « Station » des réglages d'un ordinateur
/// (`station` de `GET /api/stream/info`, lu par Admin sans appel réseau).
public struct StationInfo: Sendable, Equatable {
    /// Version de KyberNow Station.
    public var version: String
    /// Une version plus récente, signée par KyberNow, est publiée.
    public var updateAvailable: Bool
    public var updateVersion: String?
    /// Elle s'installe depuis un appareil (`stationUpdate`).
    public var installable: Bool
    /// Elle s'installe avec l'installateur, sur l'ordinateur.
    public var requiresSetup: Bool
    /// Une installation est en cours.
    public var installing: Bool
    /// Un redémarrage est en cours : Admin va s'arrêter puis revenir.
    public var restarting: Bool

    public init(version: String, updateAvailable: Bool = false, updateVersion: String? = nil, installable: Bool = false,
                requiresSetup: Bool = false, installing: Bool = false, restarting: Bool = false) {
        self.version = version
        self.updateAvailable = updateAvailable
        self.updateVersion = updateVersion
        self.installable = installable
        self.requiresSetup = requiresSetup
        self.installing = installing
        self.restarting = restarting
    }
}

/// Une carte réseau de l'ordinateur, pour le réveiller par le réseau
/// (`wake` de `GET /api/stream/info`, gardée par l'app pour chaque ordinateur).
public struct WakeTarget: Codable, Sendable, Hashable {
    /// Adresse matérielle, `AA:BB:CC:DD:EE:FF`.
    public var mac: String
    /// Dernière adresse IPv4 connue (envoi direct).
    public var ipv4: String?
    /// Adresse de diffusion de son sous-réseau.
    public var broadcast: String?
    /// `ethernet` ou `wifi` (vide : inconnu).
    public var kind: String
    /// Windows laisse cette carte réveiller l'ordinateur ; nil : inconnu.
    public var armed: Bool?
    /// Nom de la carte, vide s'il est inconnu.
    public var name: String

    public init(mac: String, ipv4: String? = nil, broadcast: String? = nil, kind: String = "", armed: Bool? = nil, name: String = "") {
        self.mac = mac
        self.ipv4 = ipv4
        self.broadcast = broadcast
        self.kind = kind
        self.armed = armed
        self.name = name
    }

    // Lecture tolérante : un champ absent garde sa valeur par défaut.
    private enum Keys: String, CodingKey { case mac, ipv4, broadcast, kind, armed, name }

    public init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: Keys.self)
        mac = try c.decode(String.self, forKey: .mac)
        ipv4 = try? c.decodeIfPresent(String.self, forKey: .ipv4)
        broadcast = try? c.decodeIfPresent(String.self, forKey: .broadcast)
        kind = (try? c.decodeIfPresent(String.self, forKey: .kind)) ?? ""
        armed = try? c.decodeIfPresent(Bool.self, forKey: .armed)
        name = (try? c.decodeIfPresent(String.self, forKey: .name)) ?? ""
    }
}

/// Réveil par le réseau : le cœur envoie le paquet magique vers la diffusion
/// du sous-réseau de chaque carte, 255.255.255.255 et sa dernière adresse,
/// en direct. Sur iPhone, iPad et Apple TV, la diffusion peut être refusée
/// (autorisation multicast d'Apple) : l'envoi direct part quand même.
public enum Wake {
    public enum Result: Sendable { case sent, noTarget, failed }

    public static func send(_ targets: [WakeTarget]) async -> Result {
        let json = (try? JSONEncoder().encode(targets)).flatMap { String(data: $0, encoding: .utf8) } ?? "[]"
        return await withCheckedContinuation { continuation in
            DispatchQueue.global(qos: .userInitiated).async {
                switch kn_wake(json) {
                case KN_OK: continuation.resume(returning: .sent)
                case KN_WAKE_NO_TARGET: continuation.resume(returning: .noTarget)
                default: continuation.resume(returning: .failed)
                }
            }
        }
    }
}

/// Refus d'un redémarrage ou d'une mise à jour de la Station (codes
/// `KN_STATION_*` du cœur, docs/coeur-apple.md).
public enum StationError: LocalizedError, Equatable, Sendable {
    /// Un appareil est en session (son nom, si Admin le donne) : refaire
    /// l'appel avec `force` coupe sa session.
    case inSession(device: String?)
    /// Un redémarrage par minute au plus, ou trop d'essais ratés ; l'attente en secondes.
    case tooSoon(seconds: Int?)
    /// Rien à installer depuis un appareil (message d'Admin).
    case nothingToInstall(String)
    /// La version publiée s'installe avec l'installateur, sur l'ordinateur (message d'Admin).
    case requiresSetup(String)
    /// Une installation ou un redémarrage est déjà en cours (message d'Admin).
    case busy(String)
    /// Station 0.4.9 ou plus ancienne : elle ne connaît pas l'appel.
    case unsupported
    /// Le reste : Admin injoignable, certificat changé, compte refusé (message du cœur).
    case failed(String)

    public var errorDescription: String? {
        switch self {
        case let .nothingToInstall(m), let .requiresSetup(m), let .busy(m), let .failed(m): return m
        case .inSession, .tooSoon, .unsupported: return nil
        }
    }
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
        case let .http(code, message): return message.isEmpty ? String(localized: "la Station a répondu \(code)", bundle: .kyberKit) : message
        case .certificateChanged: return String(localized: "le certificat de la Station a changé : ce n'est peut-être pas votre ordinateur", bundle: .kyberKit)
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
    func call<T: Sendable>(_ body: @escaping @Sendable (OpaquePointer) throws -> T) async throws -> T {
        let console = self.console
        return try await withCheckedThrowingContinuation { continuation in
            queue.async {
                continuation.resume(with: Result { try body(console) })
            }
        }
    }

    /// Traduit un code de retour du cœur en erreur Swift.
    static func check(_ code: Int32, _ error: UnsafeMutablePointer<CChar>?) throws {
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

    private static func hostInfo(_ json: [String: Any], share: ShareAvailability) -> HostInfo {
        let vd = json["virtual_display"] as? [String: Any]
        let audio = json["audio"] as? [String: Any]
        return HostInfo(
            hostname: (json["hostname"] as? String).flatMap { $0.isEmpty ? nil : $0 },
            gpu: json["gpu"] as? String,
            av1: json["av1"] as? Bool ?? false,
            virtualDisplay: vd?["available"] as? Bool ?? false,
            speakers: audio?["speakers"] as? Bool ?? false,
            os: (json["os"] as? String).flatMap { $0.isEmpty ? nil : $0 },
            adminVersion: (json["admin_version"] as? String).flatMap { $0.isEmpty ? nil : $0 },
            station: (json["station"] as? [String: Any]).map(stationInfo),
            wake: wakeTargets(json["wake"]),
            ownDevice: (json["account"] as? [String: Any])?["own_device"] as? Bool ?? false,
            maxScreens: vd?["max_screens"] as? Int ?? 1,
            share: share
        )
    }

    private static func wakeTargets(_ value: Any?) -> [WakeTarget] {
        guard let list = value as? [Any], let data = try? JSONSerialization.data(withJSONObject: list) else { return [] }
        return (try? JSONDecoder().decode([WakeTarget].self, from: data)) ?? []
    }

    private static func stationInfo(_ json: [String: Any]) -> StationInfo {
        let update = json["update"] as? [String: Any]
        return StationInfo(
            version: json["version"] as? String ?? "",
            updateAvailable: update?["available"] as? Bool ?? false,
            updateVersion: (update?["version"] as? String).flatMap { $0.isEmpty ? nil : $0 },
            installable: update?["installable"] as? Bool ?? false,
            requiresSetup: update?["requires_setup"] as? Bool ?? false,
            installing: update?["installing"] as? Bool ?? false,
            restarting: json["restarting"] as? Bool ?? false
        )
    }

    public func info() async throws -> HostInfo {
        try await call { console in
            var out: UnsafeMutablePointer<CChar>?
            var error: UnsafeMutablePointer<CChar>?
            try Self.check(kn_console_info(console, &out, &error), error)
            let text = Core.take(out)
            return Self.hostInfo(Self.json(text), share: Share.availability(infoJSON: text ?? "{}"))
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

    // MARK: - Station

    /// Adresse d'Admin à ouvrir dans le navigateur (`https://adresse:port/`).
    public var adminURL: URL? {
        Core.take(kn_console_admin_url(console)).flatMap(URL.init(string:))
    }

    /// Redémarre la Station entière (le serveur Kyber et Admin, environ une
    /// minute). `force` : même si un appareil est en session (elle est coupée).
    public func stationRestart(force: Bool) async throws {
        let device = Self.thisDeviceID
        _ = try await call { console in
            var out: UnsafeMutablePointer<CChar>?
            var error: UnsafeMutablePointer<CChar>?
            let code = kn_console_station_restart(console, device, force, &out, &error)
            try Self.checkStation(code, out, error)
        }
    }

    /// Lance la mise à jour de la Station vers la version annoncée par
    /// `HostInfo.station` ; rend cette version. L'appel attend qu'Admin ait
    /// téléchargé et vérifié le paquet (quelques minutes au plus).
    public func stationUpdate(force: Bool) async throws -> String {
        let device = Self.thisDeviceID
        return try await call { console in
            var out: UnsafeMutablePointer<CChar>?
            var error: UnsafeMutablePointer<CChar>?
            let code = kn_console_station_update(console, device, force, &out, &error)
            let detail = try Self.checkStation(code, out, error)
            return detail["version"] as? String ?? ""
        }
    }

    /// Code d'un appel de la Station : le détail en JSON, ou le refus.
    @discardableResult
    private static func checkStation(_ code: Int32, _ out: UnsafeMutablePointer<CChar>?, _ error: UnsafeMutablePointer<CChar>?) throws -> [String: Any] {
        let detail = json(Core.take(out))
        let message = Core.take(error) ?? detail["message"] as? String ?? ""
        switch code {
        case KN_OK: return detail
        case KN_STATION_IN_SESSION: throw StationError.inSession(device: (detail["device"] as? String).flatMap { $0.isEmpty ? nil : $0 })
        case KN_STATION_TOO_SOON: throw StationError.tooSoon(seconds: (detail["seconds"] as? NSNumber)?.intValue)
        case KN_STATION_NOTHING_TO_INSTALL: throw StationError.nothingToInstall(message)
        case KN_STATION_REQUIRES_SETUP: throw StationError.requiresSetup(message)
        case KN_STATION_BUSY: throw StationError.busy(message)
        case KN_STATION_UNSUPPORTED: throw StationError.unsupported
        case KN_CERTIFICATE_CHANGED: throw StationError.failed(ConsoleError.certificateChanged.localizedDescription)
        default:
            if code < 0, message.isEmpty { throw StationError.failed(ConsoleError.http(Int(-code), "").localizedDescription) }
            throw StationError.failed(message)
        }
    }

    /// Identifiant de cet appareil auprès des consoles (tiré à la première ouverture).
    public static var thisDeviceID: String {
        thisDevice["id"] as? String ?? ""
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
        let app = (Bundle.main.infoDictionary?["CFBundleShortVersionString"] as? String).map { "KyberNow Link \($0)" } ?? "KyberNow Link"
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
    /// (Appareils Link), à accepter ou refuser. Rend l'identifiant de la demande.
    /// `ConsoleError.http(404, _)` : console d'une version qui ne le connaît pas.
    public func pairRequest(code: String) async throws -> String {
        let device = Self.thisDeviceJSON
        return try await call { console in
            var out: UnsafeMutablePointer<CChar>?
            var error: UnsafeMutablePointer<CChar>?
            try Self.check(kn_console_pair_request(console, code, device, &out, &error), error)
            guard let id = Core.take(out), !id.isEmpty else {
                throw ConsoleError.network(String(localized: "réponse inattendue de la Station", bundle: .kyberKit))
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
