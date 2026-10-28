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
/// cœur (« Auto », « Comme l'hôte », « 4K · 60 Hz », « 1080p · 60 Hz »,
/// « 720p · 30 Hz »). L'ordre des cas est celui des menus (`allCases`) ; les
/// valeurs sont les codes du cœur (`KN_SHARE_QUALITY_*`), qui ne le suivent pas
/// (la 4K est venue après).
public enum ShareQuality: Int, CaseIterable, Sendable, Equatable {
    /// D'après l'écran de l'appareil et ce que son décodeur suit (proposé) ;
    /// c'est la Station qui tranche.
    case auto = 0
    /// Celle de l'hôte, partagée.
    case host = 1
    /// 3840 × 2160 à 60 Hz au plus (jamais au-dessus de l'image de l'hôte).
    case p2160_60 = 4
    case p1080_60 = 2
    case p720_30 = 3

    /// Le libellé, dans la langue de l'app.
    public var label: String {
        Core.take(kn_share_quality_label(Int32(rawValue))) ?? ""
    }

    /// Le nom de la qualité dans l'échange avec Admin (`auto`, `host`, `2160p60`, `1080p60`, `720p30`).
    var wire: String {
        switch self {
        case .auto: return "auto"
        case .host: return "host"
        case .p2160_60: return "2160p60"
        case .p1080_60: return "1080p60"
        case .p720_30: return "720p30"
        }
    }

    init?(wire: String?) {
        switch wire {
        case "auto": self = .auto
        case "host": self = .host
        case "2160p60": self = .p2160_60
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

/// Un invité, dans la liste de l'hôte, ou une demande venue par le relais qui
/// attend son accord (`pending`).
public struct ShareGuest: Identifiable, Sendable, Equatable {
    public let id: String
    public let name: String
    /// `mac`, `windows`, `linux`, `ios`, `tvos`, `android`, `webos`, `web`.
    public let app: String
    public let role: ShareRole
    /// Arrivée dans le partage (« depuis 3 min »), si la Station la donne ; pour
    /// une demande, l'heure de la demande.
    public let since: Date?
    /// L'image qu'il reçoit, si la Station la dit.
    public let image: ShareImage?
    /// Une demande venue par le relais, pas encore acceptée : « {prénom} demande
    /// à rejoindre », « Accepter » et « Refuser ». Elle est spectatrice, ne compte
    /// pas dans les invités et ne prend pas de place tant qu'elle n'est pas acceptée.
    public let pending: Bool
    /// Il est venu par le relais, hors de chez l'hôte : « hors de chez vous ».
    public let viaRelay: Bool

    public init(id: String, name: String, app: String, role: ShareRole, since: Date? = nil, image: ShareImage? = nil,
                pending: Bool = false, viaRelay: Bool = false) {
        self.id = id
        self.name = name
        self.app = app
        self.role = role
        self.since = since
        self.image = image
        self.pending = pending
        self.viaRelay = viaRelay
    }

    /// Le même invité avec un autre rôle.
    public func with(role: ShareRole) -> ShareGuest {
        ShareGuest(id: id, name: name, app: app, role: role, since: since, image: image, pending: pending, viaRelay: viaRelay)
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
    /// L'adresse du client web sur le réseau, même quand `web` est celle du relais.
    public var webLocal: String
    /// Le relais du partage : nil sans relais réglé ; sinon il tient le code
    /// (« Copier le lien » copie le lien du relais), ou n'est pas encore joignable.
    public var relay: Relay?
    /// Les invités acceptés et les demandes en attente, dans l'ordre d'arrivée.
    public var guests: [ShareGuest]

    /// L'état du relais pour ce partage (`relay.state` de la vue du partage).
    public enum Relay: Sendable, Equatable {
        /// Le relais tient le code : le lien et le client web passent par lui.
        case joined
        /// « Pas encore joignable hors de chez vous. »
        case unreachable
    }

    public init(code: String, link: String, web: String = "", display: String? = nil, webLocal: String = "",
                relay: Relay? = nil, guests: [ShareGuest] = []) {
        self.code = code
        self.link = link
        self.web = web
        self.display = display
        self.webLocal = webLocal
        self.relay = relay
        self.guests = guests
    }

    /// Les demandes qui attendent l'accord de l'hôte (« Demandes », pastille « 1 demande »).
    public var requests: [ShareGuest] { guests.filter(\.pending) }
    /// Les invités acceptés (« Invités », pastille « 2 invités ») ; une demande n'y compte pas.
    public var accepted: [ShareGuest] { guests.filter { !$0.pending } }

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
                              role: ShareRole(wire: g["role"] as? String) ?? .spectator, since: since, image: image,
                              pending: g["pending"] as? Bool ?? false, viaRelay: g["via"] as? String == "relay")
        }
        // `relay` : nul sans relais réglé, sinon `{"state": "joined"|"unreachable"}`.
        let relay = (j["relay"] as? [String: Any]).map { ($0["state"] as? String) == "joined" ? Relay.joined : Relay.unreachable }
        self.init(code: code, link: j["link"] as? String ?? "", web: j["web"] as? String ?? "",
                  display: (j["display"] as? [String: Any])?["label"] as? String, webLocal: j["web_local"] as? String ?? "",
                  relay: relay, guests: guests)
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
    /// Le relais de la Station et son identifiant chez lui, quand le lien a été
    /// fait relais joint (`relay=`, `sid=`) : hors de chez l'hôte, le lien passe par lui.
    public let relay: String?
    public let sid: String?
    /// Le JSON du cœur, repassé tel quel à `kn_share_join_link`.
    let json: String

    public var target: ShareTarget { ShareTarget(host: host, port: port, fingerprint: fingerprint) }

    /// Le lien porte le relais de la Station : il marche aussi hors de chez soi.
    public var viaRelay: Bool { relay != nil && sid != nil }
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
    /// Entré par le relais, hors de chez l'hôte : la session y reste (puce « Par le relais »).
    public internal(set) var viaRelay = false

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
        viaRelay = joined["route"] is [String: Any]
    }

    #if DEBUG
    /// Démonstration (Debug, sans Station) : un partage rejoint pour de faux,
    /// pour dessiner la session d'un invité.
    public init(demoComputer: String, host: String, role: ShareRole, quality: ShareQuality = .auto, viaRelay: Bool = false) {
        json = "{}"
        self.quality = quality
        self.viaRelay = viaRelay
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
    /// Par le relais : l'hôte a refusé (« L'hôte a refusé. »).
    case refused(String)
    /// Par le relais : l'hôte n'a pas répondu en 120 s (« L'hôte n'a pas répondu. »).
    case expired(String)
    /// Par le relais : trois demandes attendent déjà l'accord de l'hôte.
    case pendingFull(String)
    /// Par le relais : le relais ne répond pas (« Le relais ne répond pas. »), ou
    /// l'ordinateur n'y est pas, éteint ou en veille (texte du cœur).
    case relay(String)
    /// Le reste : Station injoignable, réponse illisible (message du cœur).
    case other(String)

    public var errorDescription: String? {
        switch self {
        case let .noShare(m), let .full(m), let .tooManyTries(m), let .unreadable(m), let .other(m),
             let .refused(m), let .expired(m), let .pendingFull(m), let .relay(m): return m
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

/// Une demande par le relais qui attend l'accord de l'hôte (« En attente de
/// l'accord de l'hôte… ») : à donner à `Share.wait` jusqu'à sa réponse, ou à
/// `Share.cancel` (« Annuler »). L'hôte n'est connu qu'après son accord : ni son
/// prénom ni le nom de l'ordinateur.
public struct SharePending: Sendable {
    /// `pending` du cœur (la route, l'identifiant de l'invité, son secret), repassé tel quel.
    let json: String
    /// Sans réponse de l'hôte en ce temps (secondes, 120), la demande expire.
    public let expires: Int
}

/// L'issue de « Rejoindre » : l'invité entre (sur le réseau, ou accepté
/// aussitôt), ou attend l'accord de l'hôte (venu par le relais).
public enum ShareJoining: Sendable {
    case joined(ShareFound)
    case pending(SharePending)
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
        return ShareJoinLink(host: host, port: port, fingerprint: j["fingerprint"] as? String, code: code,
                             relay: j["relay"] as? String, sid: j["sid"] as? String, json: text)
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

    // MARK: Rejoindre, hors de chez soi (docs/relais.md)

    /// « Rejoindre » sur le réseau et hors de chez soi : le code part aux Stations
    /// du réseau (`targets`) et, en même temps, aux relais (`relays` : celui des
    /// ordinateurs que l'appareil connaît, et le relais public). Une Station du
    /// réseau qui l'accepte en 1,5 s est prise ; sinon la Station que donne un
    /// relais, de bout en bout par le tunnel : l'invité entre, ou attend l'accord
    /// de l'hôte (`.pending`). Bloquant jusqu'à 8 s : hors du fil principal.
    public static func joinAnywhere(targets: [ShareTarget], relays: [String], code: String, name: String,
                                    quality: ShareQuality = .auto, screen: ShareScreen? = nil,
                                    capabilities: DecoderCapabilities = .current) async throws -> ShareJoining {
        let request = joinRequest(code: code, name: name, quality: quality, screen: screen, capabilities: capabilities)
        let targetsJSON = json(targets.map(\.json)), relaysJSON = json(relays)
        let caps = capabilitiesJSON(capabilities)
        // Hors de chez soi sur iPhone ou iPad : la radio cellulaire d'abord.
        await Relay.wakeRadio(server: relays.first)
        return try await joining(quality: quality) { out, error in
            kn_share_join_anywhere(targetsJSON, relaysJSON, request, caps, &out, &error)
        }
    }

    /// Un lien ou un QR code de partage (`kybernow://join…`) : la Station du
    /// réseau d'abord (1,5 s), puis, si le lien porte `relay` et `sid`, par le
    /// relais de bout en bout (Admin épinglé sur l'empreinte du lien : le relais
    /// ne peut pas s'interposer). Même issue que `joinAnywhere`.
    public static func joinLink(_ link: ShareJoinLink, code: String, name: String,
                                quality: ShareQuality = .auto, screen: ShareScreen? = nil,
                                capabilities: DecoderCapabilities = .current) async throws -> ShareJoining {
        let request = joinRequest(code: code, name: name, quality: quality, screen: screen, capabilities: capabilities)
        let linkJSON = link.json, caps = capabilitiesJSON(capabilities)
        if link.viaRelay { await Relay.wakeRadio(server: link.relay) }
        return try await joining(quality: quality) { out, error in
            kn_share_join_link(linkJSON, request, caps, &out, &error)
        }
    }

    /// L'hôte a-t-il répondu à la demande ? Un appel tient 25 s au plus : nil,
    /// toujours en attente (rappeler) ; sinon l'invité est accepté, et sa session
    /// s'ouvrira par le relais. Refus et délai : « L'hôte a refusé. », « L'hôte
    /// n'a pas répondu. » ; « partage arrêté » : `.noShare`. Hors du fil principal.
    public static func wait(_ pending: SharePending, quality: ShareQuality = .auto,
                            capabilities: DecoderCapabilities = .current) async throws -> ShareFound? {
        let text = pending.json, caps = capabilitiesJSON(capabilities)
        let result = await blocking { out, error in kn_share_wait(text, caps, &out, &error) }
        switch result.status {
        case KN_OK:
            guard let out = result.out, var found = ShareFound(json: out) else {
                throw ShareJoinError.other(String(localized: "réponse inattendue de la Station", bundle: .kyberKit))
            }
            found.quality = quality
            return found
        case KN_SHARE_WAITING: return nil
        default: throw joinError(result.status, result.message)
        }
    }

    /// « Annuler » la demande en attente : l'hôte ne la voit plus.
    public static func cancel(_ pending: SharePending) {
        let text = pending.json
        DispatchQueue.global(qos: .utility).async { _ = kn_share_cancel(text) }
    }

    private static func joinRequest(code: String, name: String, quality: ShareQuality, screen: ShareScreen?, capabilities: DecoderCapabilities) -> String {
        var fields: [String: Any] = ["code": code, "name": name, "device": ConsoleClient.thisDeviceID, "app": app, "quality": quality.wire]
        // `auto` part quel que soit le choix (voir `join`).
        if let screen, let auto = autoQuality(screen: screen, capabilities: capabilities) { fields["auto"] = auto }
        return json(fields)
    }

    private static func capabilitiesJSON(_ capabilities: DecoderCapabilities) -> String {
        json(["hevc": capabilities.hevc, "hevc444": capabilities.hevc444, "av1": capabilities.av1])
    }

    /// Un appel bloquant du cœur, sur une file à part : le code de retour, la
    /// réponse et le message.
    private static func blocking(_ call: @escaping @Sendable (inout UnsafeMutablePointer<CChar>?, inout UnsafeMutablePointer<CChar>?) -> Int32)
        async -> (status: Int32, out: String?, message: String) {
        await withCheckedContinuation { continuation in
            DispatchQueue.global(qos: .userInitiated).async {
                var out: UnsafeMutablePointer<CChar>?
                var error: UnsafeMutablePointer<CChar>?
                let status = call(&out, &error)
                let message = Core.take(error) ?? ""
                continuation.resume(returning: (status, Core.take(out), message))
            }
        }
    }

    /// `{"joined": {target, joined}}` ou `{"pending": {route, guest, secret, expires}}`.
    private static func joining(quality: ShareQuality,
                                _ call: @escaping @Sendable (inout UnsafeMutablePointer<CChar>?, inout UnsafeMutablePointer<CChar>?) -> Int32) async throws -> ShareJoining {
        let result = await blocking(call)
        guard result.status == KN_OK else { throw joinError(result.status, result.message) }
        guard let out = result.out, let data = out.data(using: .utf8), let j = (try? JSONSerialization.jsonObject(with: data)) as? [String: Any] else {
            throw ShareJoinError.other(String(localized: "réponse inattendue de la Station", bundle: .kyberKit))
        }
        if let joined = j["joined"], var found = ShareFound(json: json(joined)) {
            found.quality = quality
            return .joined(found)
        }
        if let pending = j["pending"] as? [String: Any] {
            return .pending(SharePending(json: json(pending), expires: (pending["expires"] as? NSNumber)?.intValue ?? 120))
        }
        throw ShareJoinError.other(String(localized: "réponse inattendue de la Station", bundle: .kyberKit))
    }

    /// Le refus du cœur (code de retour et texte, dans la langue de l'app).
    private static func joinError(_ status: Int32, _ message: String) -> ShareJoinError {
        switch status {
        case KN_SHARE_NO_SHARE: return .noShare(message)
        case KN_SHARE_FULL: return .full(message)
        case KN_SHARE_TOO_MANY: return .tooManyTries(message)
        case KN_SHARE_UNREADABLE: return .unreadable(message)
        case KN_SHARE_REFUSED: return .refused(message)
        case KN_SHARE_EXPIRED: return .expired(message)
        case KN_SHARE_PENDING_FULL: return .pendingFull(message)
        case KN_RELAY_UNREACHABLE, KN_RELAY_STATION_OFFLINE: return .relay(message)
        case KN_CERTIFICATE_CHANGED: return .certificateChanged
        default: return .other(message)
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

    /// « Accepter » la demande d'un invité venu par le relais : il entre dans le
    /// partage. Rend le partage à jour (quatre invités au plus : Admin refuse la
    /// cinquième acceptation).
    public func shareAccept(_ guest: String) async throws -> ShareState? {
        let text = try await call { console -> String? in
            var out: UnsafeMutablePointer<CChar>?
            var error: UnsafeMutablePointer<CChar>?
            try Self.check(kn_console_share_accept(console, guest, &out, &error), error)
            return Core.take(out)
        }
        return ShareState(json: text)
    }

    /// « Refuser » la demande d'un invité venu par le relais (son appareil est
    /// refusé 10 minutes). Rend le partage à jour.
    public func shareRefuse(_ guest: String) async throws -> ShareState? {
        let text = try await call { console -> String? in
            var out: UnsafeMutablePointer<CChar>?
            var error: UnsafeMutablePointer<CChar>?
            try Self.check(kn_console_share_refuse(console, guest, &out, &error), error)
            return Core.take(out)
        }
        return ShareState(json: text)
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
