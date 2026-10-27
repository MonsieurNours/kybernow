// KyberNow Share sur le cœur Rust (docs/kybernow-share.md ; textes :
// docs/link-ui.md, « Partage ») : les types communs à l'hôte et à l'invité
// (rôle, invités, partage ouvert, fins), « Rejoindre » (code envoyé aux
// ordinateurs trouvés sur le réseau, lien, format vérifié) et le QR code du
// lien. Un seul composant pour le Mac, l'Apple TV, l'iPhone et l'iPad : les
// appels sont ceux du pont C du cœur (`kn_share_*`, `kn_console_share_*`,
// docs/coeur-apple.md) ; les textes de refus et de fin viennent du cœur, dans
// la langue de l'app.

import CoreImage
import Foundation
@_implementationOnly import CKyberNow

/// Ce qu'un invité a le droit de faire sur l'ordinateur de l'hôte : deux
/// droits indépendants, les manettes et le clavier et la souris, que l'hôte
/// donne ou reprend chacun sans toucher à l'autre (docs/kybernow-share.md).
public enum ShareRole: Int, Sendable, Equatable {
    /// Voit et entend, n'envoie rien.
    case spectator = 0
    /// Ses manettes deviennent la manette suivante de Windows.
    case player = 1
    /// Ses manettes, et le clavier et la souris que l'hôte lui a donnés.
    case playerKbm = 2
    /// Son clavier et sa souris seulement, sans manettes.
    case kbm = 3

    /// Les manettes de l'invité partent vers l'ordinateur.
    public var pads: Bool { self == .player || self == .playerKbm }
    /// Le clavier et la souris de l'invité partent vers l'ordinateur.
    public var keyboardAndMouse: Bool { self == .kbm || self == .playerKbm }
    /// Un droit au moins : « Joueur » dans la barre (sinon « Spectateur »).
    public var plays: Bool { self != .spectator }

    /// Le rôle que demande « Donner » ou « Reprendre les manettes » : le clavier
    /// et la souris restent tels quels (le calcul est celui du cœur).
    public func with(pads on: Bool) -> ShareRole {
        ShareRole(code: kn_share_role_with_pads(Int32(rawValue), on)) ?? self
    }

    /// Le rôle que demande « Donner » ou « Reprendre le clavier et la souris » :
    /// les manettes restent telles quelles.
    public func with(keyboardAndMouse on: Bool) -> ShareRole {
        ShareRole(code: kn_share_role_with_kbm(Int32(rawValue), on)) ?? self
    }

    /// Le nom du rôle dans l'échange avec Admin (`spectator`, `player`, `kbm`, `player_kbm`).
    init?(wire: String?) {
        switch wire {
        case "spectator": self = .spectator
        case "player": self = .player
        case "kbm": self = .kbm
        case "player_kbm": self = .playerKbm
        default: return nil
        }
    }

    init?(code: Int32) {
        self.init(rawValue: Int(code))
    }
}

/// Un invité, dans la liste de l'hôte.
public struct ShareGuest: Identifiable, Sendable, Equatable {
    public let id: String
    public let name: String
    /// `mac`, `windows`, `linux`, `ios`, `tvos`, `android`, `webos`, `web`.
    public let app: String
    public let role: ShareRole
    /// Arrivée dans le partage (« depuis 3 min »), si la Station la donne.
    public let since: Date?

    public init(id: String, name: String, app: String, role: ShareRole, since: Date? = nil) {
        self.id = id
        self.name = name
        self.app = app
        self.role = role
        self.since = since
    }

    /// Le même invité avec un autre rôle.
    public func with(role: ShareRole) -> ShareGuest {
        ShareGuest(id: id, name: name, app: app, role: role, since: since)
    }
}

/// Le partage ouvert par l'hôte : son code, son lien, ses invités.
public struct ShareState: Sendable, Equatable {
    /// Six chiffres, sans espace.
    public var code: String
    /// `kybernow://join?host=…&port=…&fp=…&code=…` : le QR code et « Copier le lien ».
    public var link: String
    /// Le client web en invité.
    public var web: String
    /// L'écran partagé (« Écran 1 »).
    public var display: String?
    public var guests: [ShareGuest]

    public init(code: String, link: String, web: String = "", display: String? = nil, guests: [ShareGuest] = []) {
        self.code = code
        self.link = link
        self.web = web
        self.display = display
        self.guests = guests
    }

    /// Lu dans le JSON du cœur (`kn_console_share_*`, `on_guests`). Nil si le
    /// partage n'est pas ouvert ou le JSON illisible.
    init?(json text: String?) {
        guard let text, let data = text.data(using: .utf8),
              let j = (try? JSONSerialization.jsonObject(with: data)) as? [String: Any] else { return nil }
        if let open = j["open"] as? Bool, !open { return nil }
        let code = j["code"] as? String ?? ""
        guard !code.isEmpty else { return nil }
        let guests = (j["guests"] as? [[String: Any]] ?? []).compactMap { g -> ShareGuest? in
            guard let id = g["id"] as? String, !id.isEmpty else { return nil }
            // `since` : secondes Unix de l'horloge de la Station (docs/kybernow-share.md).
            let since = (g["since"] as? NSNumber).map { Date(timeIntervalSince1970: $0.doubleValue) }
            return ShareGuest(id: id, name: g["name"] as? String ?? "", app: g["app"] as? String ?? "",
                              role: ShareRole(wire: g["role"] as? String) ?? .spectator, since: since)
        }
        self.init(code: code, link: j["link"] as? String ?? "", web: j["web"] as? String ?? "",
                  display: (j["display"] as? [String: Any])?["label"] as? String, guests: guests)
    }
}

/// Comment une session d'invité s'est finie par le partage (`on_share_end`).
public enum ShareEnd: Sendable, Equatable {
    /// L'hôte a arrêté le partage.
    case stopped
    /// L'hôte a retiré cet invité.
    case removed
    /// L'appareil ne lit pas le format de l'image.
    case unreadable

    init(code: Int32) {
        switch code {
        case KN_SHARE_END_STOPPED: self = .stopped
        case KN_SHARE_END_REMOVED: self = .removed
        default: self = .unreadable
        }
    }
}

/// « Partager » avec cet ordinateur : disponible, ou inactif avec la raison
/// (Station trop ancienne, partage coupé sur l'ordinateur).
public enum ShareAvailability: Sendable, Equatable {
    case available
    case unavailable(String)

    public var isAvailable: Bool { self == .available }

    /// La raison quand « Partager » est inactif (le texte du cœur).
    public var reason: String? {
        if case let .unavailable(text) = self, !text.isEmpty { return text }
        return nil
    }
}

/// Une Station à qui envoyer le code : l'adresse et le port d'Admin, et son
/// empreinte quand on la connaît.
public struct ShareTarget: Sendable, Equatable {
    public var host: String
    public var port: UInt16
    public var fingerprint: String?

    public init(host: String, port: UInt16, fingerprint: String? = nil) {
        self.host = host
        self.port = port
        self.fingerprint = fingerprint
    }

    var json: [String: Any] {
        var j: [String: Any] = ["host": host, "port": Int(port)]
        if let fingerprint { j["fingerprint"] = fingerprint }
        return j
    }
}

/// Un lien de partage lu (`kybernow://join?…` ou celui du client web).
public struct ShareJoinLink: Sendable, Equatable {
    public let host: String
    public let port: UInt16
    public let fingerprint: String?
    /// Six chiffres.
    public let code: String

    public var target: ShareTarget { ShareTarget(host: host, port: port, fingerprint: fingerprint) }
}

/// La Station qui a accepté le code, et la réponse à garder telle quelle pour
/// la session de l'invité (`SessionEngine.Params.guest`).
public struct ShareFound: Sendable {
    /// `{"target": …, "joined": …}` du cœur.
    let json: String
    public let target: ShareTarget
    /// Le nom de l'ordinateur (« Invité sur {nom} »).
    public let computer: String
    /// {hôte} des textes de fin : le prénom de l'hôte, ou le nom de l'ordinateur.
    public let host: String
    /// Le rôle de départ.
    public let role: ShareRole
    /// Le format de l'image de l'hôte (absent si elle n'avait pas démarré) :
    /// c'est celui que reçoit l'invité, pas celui de ses réglages.
    public let codec: VideoCodec?

    init?(json text: String) {
        guard let data = text.data(using: .utf8), let j = (try? JSONSerialization.jsonObject(with: data)) as? [String: Any],
              let target = j["target"] as? [String: Any], let host = target["host"] as? String,
              let port = (target["port"] as? NSNumber).flatMap({ UInt16(exactly: $0.intValue) }),
              let joined = j["joined"] as? [String: Any] else { return nil }
        json = text
        self.target = ShareTarget(host: host, port: port, fingerprint: target["fingerprint"] as? String)
        let computer = joined["computer"] as? String ?? ""
        self.computer = computer.isEmpty ? host : computer
        let name = joined["host"] as? String ?? ""
        self.host = name.isEmpty ? self.computer : name
        role = ShareRole(wire: joined["role"] as? String) ?? .spectator
        codec = (joined["codec"] as? String).flatMap(VideoCodec.init(rawValue:))
    }

    #if DEBUG
    /// Démonstration (Debug, sans Station) : un partage rejoint pour de faux,
    /// pour dessiner la session d'un invité.
    public init(demoComputer: String, host: String, role: ShareRole) {
        json = "{}"
        target = ShareTarget(host: "192.0.2.1", port: 1)
        computer = demoComputer
        self.host = host
        self.role = role
        codec = .hevc
    }
    #endif

    /// `joined`, pour `guest` dans le JSON de `kn_session_start`.
    var joined: Any {
        guard let data = json.data(using: .utf8), let j = (try? JSONSerialization.jsonObject(with: data)) as? [String: Any] else { return [:] }
        return j["joined"] ?? [:]
    }
}

/// Un refus de « Rejoindre », avec le texte du cœur à montrer tel quel.
public enum ShareJoinError: LocalizedError, Sendable, Equatable {
    /// Aucun partage avec ce code sur le réseau.
    case noShare(String)
    /// Quatre invités au plus.
    case full(String)
    /// Dix essais de code par minute au plus.
    case tooManyTries(String)
    /// L'appareil ne lit pas le format de l'image de l'ordinateur.
    case unreadable(String)
    /// Le certificat de la Station a changé depuis le lien ou l'annonce.
    case certificateChanged
    /// Le reste : Station injoignable, réponse illisible (message du cœur).
    case other(String)

    public var errorDescription: String? {
        switch self {
        case let .noShare(m), let .full(m), let .tooManyTries(m), let .unreadable(m), let .other(m): return m
        case .certificateChanged: return String(localized: "le certificat de la Station a changé : ce n'est peut-être pas votre ordinateur", bundle: .kyberKit)
        }
    }
}

public enum Share {
    /// `mac`, `ios` (iPhone et iPad) ou `tvos` : l'application dans l'échange avec Admin.
    public static var app: String {
        #if os(macOS)
        return "mac"
        #elseif os(tvOS)
        return "tvos"
        #else
        return "ios"
        #endif
    }

    // MARK: Code et lien

    /// Les six chiffres d'un code tapé (« 123 456 »), nil si ce n'en est pas un.
    public static func normalizeCode(_ text: String) -> String? {
        Core.take(kn_share_normalize_code(text))
    }

    /// Le code à afficher : « 123 456 ».
    public static func formatCode(_ code: String) -> String {
        Core.take(kn_share_format_code(code)) ?? code
    }

    /// Lit un lien de partage (`kybernow://join?…`, ou le lien du client web).
    public static func parseLink(_ url: String) -> ShareJoinLink? {
        guard let text = Core.take(kn_share_parse_link(url)), let data = text.data(using: .utf8),
              let j = (try? JSONSerialization.jsonObject(with: data)) as? [String: Any],
              let host = j["host"] as? String, let code = j["code"] as? String,
              let port = (j["port"] as? NSNumber).flatMap({ UInt16(exactly: $0.intValue) }) else { return nil }
        return ShareJoinLink(host: host, port: port, fingerprint: j["fingerprint"] as? String, code: code)
    }

    // MARK: Rejoindre

    /// Envoie le code à ces Stations à la fois et rend la première qui
    /// l'accepte ; le format de l'image de l'hôte est vérifié avec ce que
    /// l'appareil décode. Bloquant jusqu'à 6 s : hors du fil principal.
    public static func join(targets: [ShareTarget], code: String, name: String,
                            capabilities: DecoderCapabilities = .current) async throws -> ShareFound {
        let targetsJSON = json(targets.map(\.json))
        let request = json(["code": code, "name": name, "device": ConsoleClient.thisDeviceID, "app": app])
        let caps = json(["hevc": capabilities.hevc, "hevc444": capabilities.hevc444, "av1": capabilities.av1])
        return try await withCheckedThrowingContinuation { continuation in
            DispatchQueue.global(qos: .userInitiated).async {
                var out: UnsafeMutablePointer<CChar>?
                var error: UnsafeMutablePointer<CChar>?
                let status = kn_share_join(targetsJSON, request, caps, &out, &error)
                let message = Core.take(error) ?? ""
                let result = Core.take(out)
                switch status {
                case KN_OK:
                    if let result, let found = ShareFound(json: result) {
                        continuation.resume(returning: found)
                    } else {
                        continuation.resume(throwing: ShareJoinError.other(String(localized: "réponse inattendue de la Station", bundle: .kyberKit)))
                    }
                case KN_SHARE_NO_SHARE: continuation.resume(throwing: ShareJoinError.noShare(message))
                case KN_SHARE_FULL: continuation.resume(throwing: ShareJoinError.full(message))
                case KN_SHARE_TOO_MANY: continuation.resume(throwing: ShareJoinError.tooManyTries(message))
                case KN_SHARE_UNREADABLE: continuation.resume(throwing: ShareJoinError.unreadable(message))
                case KN_CERTIFICATE_CHANGED: continuation.resume(throwing: ShareJoinError.certificateChanged)
                default: continuation.resume(throwing: ShareJoinError.other(message))
                }
            }
        }
    }

    /// L'invité renonce avant d'avoir ouvert sa session (une session d'invité, elle, part seule).
    public static func leave(_ found: ShareFound) {
        let text = found.json
        DispatchQueue.global(qos: .utility).async { _ = kn_share_leave(text) }
    }

    // MARK: Hôte

    /// « Partager » avec cet ordinateur, d'après ce que la Station dit d'elle-même.
    static func availability(infoJSON: String) -> ShareAvailability {
        let code = kn_share_availability(infoJSON)
        guard code != KN_SHARE_AVAILABLE else { return .available }
        return .unavailable(Core.take(kn_share_availability_text(code)) ?? "")
    }

    // MARK: QR code

    /// Le QR code d'un texte, pixels nets (à agrandir sans lissage), noir sur
    /// blanc avec une marge de quatre modules : celui que les caméras lisent.
    public static func qrImage(for text: String, scale: Int = 8) -> CGImage? {
        guard let filter = CIFilter(name: "CIQRCodeGenerator") else { return nil }
        filter.setValue(Data(text.utf8), forKey: "inputMessage")
        filter.setValue("M", forKey: "inputCorrectionLevel")
        guard let code = filter.outputImage else { return nil }
        let quiet: CGFloat = 4
        let size = code.extent.integral
        let framed = code
            .transformed(by: CGAffineTransform(translationX: quiet - size.minX, y: quiet - size.minY))
            .composited(over: CIImage(color: .white).cropped(to: CGRect(x: 0, y: 0, width: size.width + 2 * quiet, height: size.height + 2 * quiet)))
        let scaled = framed.transformed(by: CGAffineTransform(scaleX: CGFloat(scale), y: CGFloat(scale)))
        let context = CIContext(options: [.useSoftwareRenderer: true])
        return context.createCGImage(scaled, from: scaled.extent.integral)
    }

    private static func json(_ value: Any) -> String {
        (try? JSONSerialization.data(withJSONObject: value)).flatMap { String(data: $0, encoding: .utf8) } ?? "{}"
    }
}

// MARK: - Hôte : les appels d'Admin

extension ConsoleClient {
    /// Ouvre le partage de l'écran principal de la session de cet appareil.
    public func shareOpen() async throws -> ShareState {
        let text = try await call { console -> String? in
            var out: UnsafeMutablePointer<CChar>?
            var error: UnsafeMutablePointer<CChar>?
            try Self.check(kn_console_share_open(console, &out, &error), error)
            return Core.take(out)
        }
        return try Self.shareState(from: text)
    }

    /// « Nouveau code » : les invités déjà là restent.
    public func shareNewCode() async throws -> ShareState {
        let text = try await call { console -> String? in
            var out: UnsafeMutablePointer<CChar>?
            var error: UnsafeMutablePointer<CChar>?
            try Self.check(kn_console_share_code(console, &out, &error), error)
            return Core.take(out)
        }
        return try Self.shareState(from: text)
    }

    /// Le partage en cours, nil s'il n'y en a pas.
    public func currentShare() async throws -> ShareState? {
        let text = try await call { console -> String? in
            var out: UnsafeMutablePointer<CChar>?
            var error: UnsafeMutablePointer<CChar>?
            try Self.check(kn_console_share_state(console, &out, &error), error)
            return Core.take(out)
        }
        return ShareState(json: text)
    }

    /// « Donner » ou « Reprendre les manettes », « Donner » ou « Reprendre le
    /// clavier et la souris » : le rôle entier que demande l'un des deux boutons.
    public func shareSetRole(_ role: ShareRole, of guest: String) async throws {
        try await call { console in
            var error: UnsafeMutablePointer<CChar>?
            try Self.check(kn_console_share_guest(console, guest, Int32(role.rawValue), &error), error)
        }
    }

    /// « Retirer » un invité.
    public func shareKick(_ guest: String) async throws {
        try await call { console in
            var error: UnsafeMutablePointer<CChar>?
            try Self.check(kn_console_share_kick(console, guest, &error), error)
        }
    }

    /// « Arrêter le partage » : tout le monde sort.
    public func shareClose() async throws {
        try await call { console in
            var error: UnsafeMutablePointer<CChar>?
            try Self.check(kn_console_share_close(console, &error), error)
        }
    }

    private static func shareState(from text: String?) throws -> ShareState {
        guard let state = ShareState(json: text) else {
            throw ConsoleError.network(String(localized: "réponse inattendue de la Station", bundle: .kyberKit))
        }
        return state
    }
}
