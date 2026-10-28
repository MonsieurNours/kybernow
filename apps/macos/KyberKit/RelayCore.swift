// Le relais de KyberNow sur le cœur Rust (docs/relais.md ; textes :
// docs/link-ui.md, « Hors de chez vous ») : la route d'un ordinateur par son
// relais (`info.relay` gardé pour chaque ordinateur), sa présence chez le
// relais (accueil : « Hors de chez vous »), le chemin d'une session (puce « Par
// le relais », ligne « Chemin » des statistiques) et, sur iPhone et iPad, le
// réveil de la radio cellulaire avant d'ouvrir un tunnel. Un seul composant
// pour le Mac, l'Apple TV, l'iPhone et l'iPad : les appels sont ceux du pont C
// du cœur (`kn_relay_*`, docs/coeur-apple.md) ; les textes de refus viennent du
// cœur, dans la langue de l'app.

import Foundation
#if os(iOS)
import Network
#endif
@_implementationOnly import CKyberNow

/// Ce qu'un ordinateur dit de son relais (`relay` de `GET /api/stream/info`,
/// Station 0.8.0 et suivantes quand un relais est réglé) : l'app le garde pour
/// chaque ordinateur, pour passer par lui hors de chez soi.
public struct RelayInfo: Sendable, Equatable, Hashable, Codable {
    /// Le relais de la Station (`share.kybernow.app:8443`).
    public var server: String
    /// L'identifiant de la Station chez ce relais.
    public var id: String
    /// « Accès hors de chez vous » est actif dans Admin : les appareils
    /// appairés ouvrent l'ordinateur par le relais.
    public var remoteAccess: Bool

    public init(server: String, id: String, remoteAccess: Bool) {
        self.server = server
        self.id = id
        self.remoteAccess = remoteAccess
    }

    private enum Keys: String, CodingKey { case server, id, remoteAccess = "remote_access" }

    // Lecture tolérante : un champ absent garde sa valeur par défaut.
    public init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: Keys.self)
        server = (try? c.decodeIfPresent(String.self, forKey: .server)) ?? ""
        id = (try? c.decodeIfPresent(String.self, forKey: .id)) ?? ""
        remoteAccess = (try? c.decodeIfPresent(Bool.self, forKey: .remoteAccess)) ?? false
    }

    public func encode(to encoder: Encoder) throws {
        var c = encoder.container(keyedBy: Keys.self)
        try c.encode(server, forKey: .server)
        try c.encode(id, forKey: .id)
        try c.encode(remoteAccess, forKey: .remoteAccess)
    }

    /// Lu dans `relay` de l'info de la Station ; nil sans relais (absent, vide).
    init?(json j: [String: Any]?) {
        guard let j, let server = j["server"] as? String, !server.isEmpty, let id = j["id"] as? String, !id.isEmpty else { return nil }
        self.init(server: server, id: id, remoteAccess: j["remote_access"] as? Bool ?? false)
    }
}

/// Une Station par son relais : le relais, son identifiant chez lui, et
/// l'empreinte d'Admin que l'appareil connaît (jamais de premier contact par le
/// relais). À garder pour l'ordinateur et à donner à la session
/// (`SessionEngine.Params.relay`) et à la console (`ConsoleClient`).
public struct RelayRoute: Sendable, Equatable, Hashable {
    public let server: String
    public let station: String
    public let adminFingerprint: String

    /// Le JSON du cœur (`{server, station, admin_fp}`), repassé tel quel.
    var dictionary: [String: Any] {
        ["server": server, "station": station, "admin_fp": adminFingerprint]
    }

    var json: String {
        (try? JSONSerialization.data(withJSONObject: dictionary)).flatMap { String(data: $0, encoding: .utf8) } ?? "{}"
    }

    init?(json text: String) {
        guard let data = text.data(using: .utf8), let j = (try? JSONSerialization.jsonObject(with: data)) as? [String: Any],
              let server = j["server"] as? String, let station = j["station"] as? String, let fp = j["admin_fp"] as? String else { return nil }
        self.init(server: server, station: station, adminFingerprint: fp)
    }

    init(server: String, station: String, adminFingerprint: String) {
        self.server = server
        self.station = station
        self.adminFingerprint = adminFingerprint
    }
}

/// Par où passe une session (`on_route` du cœur) : la puce « Par le relais »
/// de la barre et la ligne « Chemin » des statistiques.
public struct RouteInfo: Sendable, Equatable {
    /// Les trois chemins, dans l'ordre des codes du cœur (`kn_relay_path_label`).
    public enum Transport: Int, Sendable {
        case direct = 0
        /// Par le relais, en QUIC.
        case relay = 1
        /// Par le relais, en TCP (repli quand l'UDP ne passe pas).
        case relayTcp = 2
    }

    public let transport: Transport
    /// Le relais (`share.kybernow.app:8443`), par le relais.
    public let relay: String?
    /// Aller-retour appareil ↔ relais, en millisecondes.
    public let rttClient: Double?
    /// Aller-retour relais ↔ ordinateur, en millisecondes.
    public let rttStation: Double?

    public static let direct = RouteInfo(transport: .direct)

    public init(transport: Transport, relay: String? = nil, rttClient: Double? = nil, rttStation: Double? = nil) {
        self.transport = transport
        self.relay = relay
        self.rttClient = rttClient
        self.rttStation = rttStation
    }

    /// La session passe par le relais : puce « Par le relais ».
    public var viaRelay: Bool { transport != .direct }

    /// La ligne « Chemin » : « Direct », « Par le relais », « Par le relais (TCP) »,
    /// dans la langue de l'app (le cœur la donne).
    public var label: String { RouteInfo.label(transport) }

    public static func label(_ transport: Transport) -> String {
        Core.take(kn_relay_path_label(Int32(transport.rawValue))) ?? ""
    }

    /// Lu dans le JSON de `on_route` (`transport` : `direct`, `relay`, `relay_tcp`).
    init?(json text: String?) {
        guard let text, let data = text.data(using: .utf8), let j = (try? JSONSerialization.jsonObject(with: data)) as? [String: Any] else { return nil }
        let transport: Transport
        switch j["transport"] as? String {
        case "relay": transport = .relay
        case "relay_tcp": transport = .relayTcp
        default: transport = .direct
        }
        self.init(transport: transport, relay: j["relay"] as? String,
                  rttClient: (j["rtt_client"] as? NSNumber)?.doubleValue, rttStation: (j["rtt_station"] as? NSNumber)?.doubleValue)
    }
}

public enum Relay {
    /// Le relais public (`share.kybernow.app:8443`), celui qu'une Station utilise sans réglage.
    public static var defaultServer: String {
        Core.take(kn_relay_default()) ?? "share.kybernow.app:8443"
    }

    /// `KYBERNOW_RELAY` (essais) : `force` fait passer toute session par le
    /// relais, même à la maison ; `off` coupe le relais.
    public enum Mode: Int, Sendable {
        case normal = 0
        case force = 1
        case off = 2
    }

    public static var mode: Mode { Mode(rawValue: Int(kn_relay_mode())) ?? .normal }

    /// La route de cet ordinateur par son relais, d'après ce que sa Station dit
    /// (`info.relay`) et l'empreinte d'Admin que l'appareil retient ; nil sans
    /// empreinte (jamais de premier contact par le relais), ou si la Station ne
    /// donne pas un relais lisible.
    public static func route(for relay: RelayInfo, adminFingerprint: String?) -> RelayRoute? {
        guard let adminFingerprint, !adminFingerprint.isEmpty else { return nil }
        let info: [String: Any] = ["relay": ["server": relay.server, "id": relay.id, "remote_access": relay.remoteAccess]]
        guard let data = try? JSONSerialization.data(withJSONObject: info), let text = String(data: data, encoding: .utf8),
              let route = Core.take(kn_relay_route_from_info(text, adminFingerprint)) else { return nil }
        return RelayRoute(json: route)
    }

    /// Quels ordinateurs (leur identifiant chez ce relais) sont joignables par
    /// lui : l'accueil dit « Hors de chez vous » de ceux qui ne répondent pas
    /// sur le réseau mais sont là (16 au plus par appel ; une connexion brève à
    /// chaque appel : à espacer, une fois toutes les 30 s). Le résultat suit
    /// l'ordre de `stations`. Bloquant côté cœur : l'appel part sur une file à part.
    public static func presence(server: String, stations: [String]) async throws -> [Bool] {
        guard !stations.isEmpty else { return [] }
        let list = (try? JSONSerialization.data(withJSONObject: Array(stations.prefix(16)))).flatMap { String(data: $0, encoding: .utf8) } ?? "[]"
        await wakeRadio()
        return try await withCheckedThrowingContinuation { continuation in
            DispatchQueue.global(qos: .utility).async {
                var out: UnsafeMutablePointer<CChar>?
                var error: UnsafeMutablePointer<CChar>?
                let status = kn_relay_presence(server, list, &out, &error)
                let message = Core.take(error) ?? ""
                let text = Core.take(out)
                guard status == KN_OK, let text, let data = text.data(using: .utf8),
                      let present = (try? JSONSerialization.jsonObject(with: data)) as? [Bool] else {
                    continuation.resume(throwing: ConsoleError.relay(message.isEmpty ? String(localized: "réponse inattendue de la Station", bundle: .kyberKit) : message))
                    return
                }
                continuation.resume(returning: present)
            }
        }
    }

    // MARK: Radio cellulaire (iPhone et iPad)

    /// iPhone et iPad : une requête de l'app (`URLSession`) vers `/v1/health` du
    /// relais allume la radio cellulaire avant que le cœur n'ouvre son tunnel
    /// (ses sockets, seuls, ne la réveillent pas). Rien sur Mac ni Apple TV, et
    /// rien quand l'appareil est sur le Wi-Fi ou en Ethernet. La réponse
    /// n'importe pas : un relais qui ne répond pas se verra à l'ouverture du tunnel.
    public static func wakeRadio(server: String? = nil) async {
        #if os(iOS)
        guard await onCellularOnly() else { return }
        let authority = (server ?? defaultServer).split(separator: "#").first.map(String.init) ?? defaultServer
        guard let url = URL(string: "https://\(authority)/v1/health") else { return }
        var request = URLRequest(url: url)
        request.timeoutInterval = 4
        request.cachePolicy = .reloadIgnoringLocalCacheData
        _ = try? await URLSession.shared.data(for: request)
        #endif
    }

    #if os(iOS)
    /// Seul le réseau mobile est là (ni Wi-Fi ni Ethernet), ou l'appareil ne sait
    /// pas encore : la radio est à réveiller.
    private static func onCellularOnly() async -> Bool {
        await withCheckedContinuation { continuation in
            let monitor = NWPathMonitor()
            let queue = DispatchQueue(label: "fr.leslignesclaires.kybernow.path")
            let gate = NSLock()
            var answered = false
            func answer(_ value: Bool) {
                gate.lock()
                defer { gate.unlock() }
                guard !answered else { return }
                answered = true
                monitor.cancel()
                continuation.resume(returning: value)
            }
            monitor.pathUpdateHandler = { path in
                let local = path.usesInterfaceType(.wifi) || path.usesInterfaceType(.wiredEthernet)
                answer(!local)
            }
            monitor.start(queue: queue)
            // Pas de réponse en 0,5 s : on réveille la radio, sans risque.
            queue.asyncAfter(deadline: .now() + 0.5) { answer(true) }
        }
    }
    #endif
}
