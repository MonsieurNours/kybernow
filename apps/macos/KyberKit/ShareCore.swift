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

/// La « Qualité de l'image » d'un invité (docs/link-ui.md, « Partage ») : celle
/// qu'il demande à « Rejoindre » et dans sa barre. Les libellés viennent du
/// cœur (« Auto », « Comme l'hôte », « 1080p · 60 Hz », « 720p · 30 Hz »).
public enum ShareQuality: Int, CaseIterable, Sendable, Equatable {
    /// D'après l'écran de l'appareil et ce que son décodeur suit (proposé) ;
    /// c'est la Station qui tranche.
    case auto = 0
    /// Celle de l'hôte, partagée.
    case host = 1
    case p1080_60 = 2
    case p720_30 = 3

    /// Le libellé, dans la langue de l'app.
    public var label: String {
        Core.take(kn_share_quality_label(Int32(rawValue))) ?? ""
    }

    /// Le nom de la qualité dans l'échange avec Admin (`auto`, `host`, `1080p60`, `720p30`).
    var wire: String {
        switch self {
        case .auto: return "auto"
        case .host: return "host"
        case .p1080_60: return "1080p60"
        case .p720_30: return "720p30"
        }
    }

    init?(wire: String?) {
        switch wire {
        case "auto": self = .auto
        case "host": self = .host
        case "1080p60": self = .p1080_60
        case "720p30": self = .p720_30
        default: return nil
        }
    }
}

/// L'image que reçoit un invité (`on_share_image`, `guests[].image` chez
/// l'hôte). Tant que Kyber ne sait rien, la taille et le format manquent : la
/// qualité reçue (`label`) ne se montre que si le cœur la sait dire.
public struct ShareImage: Sendable, Equatable {
    /// La qualité demandée.
    public let quality: ShareQuality
    /// La qualité reçue (« 1080p · 60 Hz »), nil tant que la taille n'est pas sue.
    public let label: String?
    /// Elle est celle de l'hôte (partagée) ; nil tant qu'on ne sait pas.
    public let followsHost: Bool?
    /// Le format de l'image reçue, s'il est su.
    public let codec: VideoCodec?

    /// Une image à part, d'une qualité qui n'est pas celle de l'hôte : c'est
    /// elle que le panneau de l'hôte montre sous l'invité.
    public var own: Bool { followsHost == false }

    public init(quality: ShareQuality, label: String?, followsHost: Bool? = nil, codec: VideoCodec? = nil) {
        self.quality = quality
        self.label = label
        self.followsHost = followsHost
        self.codec = codec
    }

    /// Lu dans le JSON du cœur (`on_share_image`, `guests[].image`). Nil si ce
    /// n'en est pas une.
    public init?(json text: String?) {
        guard let text, let data = text.data(using: .utf8),
              let j = (try? JSONSerialization.jsonObject(with: data)) as? [String: Any] else { return nil }
        self.init(json: j, text: text)
    }

    /// Lu dans `guests[].image` de la vue de l'hôte (déjà un dictionnaire).
    init?(json j: [String: Any]) {
        guard let data = try? JSONSerialization.data(withJSONObject: j), let text = String(data: data, encoding: .utf8) else { return nil }
        self.init(json: j, text: text)
    }

    private init(json j: [String: Any], text: String) {
        quality = ShareQuality(wire: j["quality"] as? String) ?? .auto
        label = Core.take(kn_share_image_label(text))
        followsHost = j["follows_host"] as? Bool
        codec = (j["codec"] as? String).flatMap(VideoCodec.init(rawValue:))
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
    /// L'image qu'il reçoit, si la Station la dit.
    public let image: ShareImage?

    public init(id: String, name: String, app: String, role: ShareRole, since: Date? = nil, image: ShareImage? = nil) {
        self.id = id
        self.name = name
        self.app = app
        self.role = role
        self.since = since
        self.image = image
    }

    /// Le même invité avec un autre rôle.
    public func with(role: ShareRole) -> ShareGuest {
        ShareGuest(id: id, name: name, app: app, role: role, since: since, image: image)
    }

    /// Sa qualité sous son nom (« 1080p · 60 Hz ») : seulement si elle n'est
    /// pas celle de l'hôte, et si le cœur sait la dire (docs/link-ui.md).
    public var imageLabel: String? {
        guard let image, image.own else { return nil }
        return image.label
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
            // `image` : ce que l'invité reçoit (absent, ou rien de su encore, pour une Station sans qualité d'image).
            let image = (g["image"] as? [String: Any]).flatMap { ShareImage(json: $0) }
            return ShareGuest(id: id, name: g["name"] as? String ?? "", app: g["app"] as? String ?? "",
                              role: ShareRole(wire: g["role"] as? String) ?? .spectator, since: since, image: image)
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
    /// L'appareil ne lit aucun format possible (rare : voir `ShareJoinError.unreadable`).
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
    /// Le format de l'image de l'hôte (absent si elle n'avait pas démarré), pas
    /// celui de ses réglages : un appareil qui ne le lit pas reçoit une image à
    /// part dans un format qu'il lit (`ShareImage.codec` le dit).
    public let codec: VideoCodec?
    /// La « Qualité de l'image » demandée à « Rejoindre » ; l'image reçue vient
    /// ensuite de `SessionEngine.onShareImage`.
    public internal(set) var quality: ShareQuality = .auto

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
    public init(demoComputer: String, host: String, role: ShareRole, quality: ShareQuality = .auto) {
        json = "{}"
        self.quality = quality
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
    /// L'appareil ne lit aucun format que l'ordinateur puisse lui donner (jamais
    /// avec H.264, que tout appareil d'Apple décode) : un format de l'hôte qu'il
    /// ne lit pas ne refuse plus personne.
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

/// L'écran de l'appareil, pour « Auto » : sa définition native en pixels et sa
/// fréquence (celle de l'écran de la fenêtre sur Mac, iPhone et iPad ; la sortie
/// du téléviseur sur Apple TV).
public struct ShareScreen: Sendable, Equatable {
    public let pixelWidth: Int
    public let pixelHeight: Int
    public let refresh: Int

    /// Mis en paysage ; une définition ou une fréquence nulle est ramenée à 1080p, 60 Hz.
    public init(pixelWidth: Int, pixelHeight: Int, refresh: Int) {
        let long = max(pixelWidth, pixelHeight), short = min(pixelWidth, pixelHeight)
        let valid = long > 0 && short > 0
        self.pixelWidth = valid ? long : 1920
        self.pixelHeight = valid ? short : 1080
        self.refresh = refresh > 0 ? refresh : 60
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
    /// l'accepte, avec la « Qualité de l'image » demandée ; pour « Auto », ce
    /// que l'appareil sait afficher (`screen`) et décoder, la Station tranchant.
    /// Un format de l'hôte que l'appareil ne lit pas n'empêche pas d'entrer
    /// (il reçoit une image à part dans un format qu'il lit). Bloquant jusqu'à
    /// 6 s : hors du fil principal.
    public static func join(targets: [ShareTarget], code: String, name: String,
                            quality: ShareQuality = .auto, screen: ShareScreen? = nil,
                            capabilities: DecoderCapabilities = .current) async throws -> ShareFound {
        let targetsJSON = json(targets.map(\.json))
        var fields: [String: Any] = ["code": code, "name": name, "device": ConsoleClient.thisDeviceID, "app": app, "quality": quality.wire]
        // `auto` part quel que soit le choix : la Station l'ignore hors « Auto », et le cœur le garde pour un passage
        // à « Auto » en cours de session (sans lui, « Auto » donnerait l'image de l'hôte).
        if let screen, let auto = autoQuality(screen: screen, capabilities: capabilities) { fields["auto"] = auto }
        let request = json(fields)
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
                    if let result, var found = ShareFound(json: result) {
                        found.quality = quality
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

    /// « Auto » : ce que l'appareil sait afficher (la définition et la fréquence
    /// de son écran, en paysage) et décoder (les formats, le 4:4:4) ; le cœur
    /// en tire `auto` de `join`, que la Station borne à l'image de l'hôte. Le
    /// décodeur matériel d'Apple ne donne pas de limite de taille ni de
    /// fréquence au-delà de l'écran : rien n'est ajouté. Nil si le cœur ne le lit pas.
    static func autoQuality(screen: ShareScreen, capabilities: DecoderCapabilities) -> [String: Any]? {
        let screenJSON = json(["x": 0, "y": 0, "width": screen.pixelWidth, "height": screen.pixelHeight,
                               "pixel_width": screen.pixelWidth, "pixel_height": screen.pixelHeight,
                               "refresh": screen.refresh, "primary": true] as [String: Any])
        let decoderJSON = json(["hevc": capabilities.hevc, "hevc444": capabilities.hevc444, "av1": capabilities.av1])
        guard let text = Core.take(kn_share_auto_quality(screenJSON, decoderJSON, nil)), let data = text.data(using: .utf8) else { return nil }
        return (try? JSONSerialization.jsonObject(with: data)) as? [String: Any]
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
