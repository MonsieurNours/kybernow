// Prêter l'ordinateur (KyberNow Share) sur le cœur Rust (docs/pret.md ;
// textes : docs/link-ui.md, « Prêt de l'ordinateur ») : les types communs à
// l'ami et à l'hôte, l'emprunt (statut de l'ordinateur d'un ami, emprunt par un
// code ou un compte, attente de l'accord de l'hôte, annulation, départ, lien
// d'invitation) et les appels de l'hôte (vue du prêt, code de séance, réponse
// aux demandes, reprise). Un seul composant pour le Mac, l'Apple TV, l'iPhone
// et l'iPad : les appels sont ceux du pont C du cœur (`kn_lend_*`,
// `kn_console_lend_*`, docs/coeur-apple.md) ; les refus et les textes de fin
// viennent du cœur, dans la langue de l'app.

import Foundation
@_implementationOnly import CKyberNow

// MARK: - L'ami

/// L'état de la carte « Ordinateur de {hôte} » (`POST /api/lend/status`).
/// « Hors ligne » reste à l'app (la Station ne répond pas).
public enum LendAvailability: String, Sendable, Equatable {
    case free, busy, lent, locked, off

    /// « Disponible », « Occupé », « Prêté à quelqu'un d'autre », « Verrouillé »,
    /// « Prêt coupé » : le texte du cœur, dans la langue de l'app.
    public var label: String {
        Core.take(kn_lend_availability_label(rawValue)) ?? rawValue
    }
}

/// Une plage d'un créneau, à l'heure de la Station.
public struct LendPeriod: Sendable, Equatable {
    public let start: Date
    public let end: Date

    public init(start: Date, end: Date) {
        self.start = start
        self.end = end
    }
}

/// Le créneau de l'ami sur ce compte : en cours (`now`), et le prochain.
public struct LendSlot: Sendable, Equatable {
    public let now: Bool
    public let next: LendPeriod?

    public init(now: Bool, next: LendPeriod?) {
        self.now = now
        self.next = next
    }
}

/// Ce que dit la Station de l'ordinateur d'un ami (la carte de l'accueil).
public struct LendStatus: Sendable, Equatable {
    public let state: LendAvailability
    /// Le prénom de l'hôte (« Ordinateur de {hôte} »).
    public let host: String
    /// Le nom de l'ordinateur.
    public let computer: String
    /// nil : pas de créneau sur ce compte.
    public let slot: LendSlot?
    /// Le prêt en cours est celui de cet ami (`lent`) : il peut y revenir
    /// (« Revenir ») au lieu de lire « Prêté à quelqu'un d'autre ».
    public let mine: Bool

    public init(state: LendAvailability, host: String, computer: String, slot: LendSlot? = nil, mine: Bool = false) {
        self.state = state
        self.host = host
        self.computer = computer
        self.slot = slot
        self.mine = mine
    }

    init?(json text: String?) {
        guard let j = LendJSON.object(text) else { return nil }
        let slot = (j["slot"] as? [String: Any]).map { s -> LendSlot in
            let next = (s["next"] as? [String: Any]).flatMap { n -> LendPeriod? in
                guard let start = (n["start"] as? NSNumber)?.doubleValue, let end = (n["end"] as? NSNumber)?.doubleValue else { return nil }
                return LendPeriod(start: Date(timeIntervalSince1970: start), end: Date(timeIntervalSince1970: end))
            }
            return LendSlot(now: s["now"] as? Bool ?? false, next: next)
        }
        self.init(state: LendAvailability(rawValue: j["state"] as? String ?? "") ?? .off,
                  host: j["host"] as? String ?? "", computer: j["computer"] as? String ?? "", slot: slot,
                  mine: j["mine"] as? Bool ?? false)
    }
}

/// Une Station à qui emprunter : son adresse (`{host, port, fingerprint}`) et,
/// hors de chez soi, sa route par le relais. Repassées telles quelles au cœur.
public struct LendTarget: Sendable, Equatable {
    /// `target_json` du cœur.
    let targetJSON: String
    /// `route_json` du cœur, ou nil (sur le réseau seulement).
    let routeJSON: String?
    public let host: String
    public let port: UInt16
    public let fingerprint: String?

    /// Une route par le relais est connue : l'emprunt marche aussi hors de chez soi.
    public var hasRelay: Bool { routeJSON != nil }

    /// Une Station du réseau (« Ajouter un ordinateur » › « Avec un compte d'ami »).
    public init(host: String, port: UInt16, fingerprint: String?) {
        var j: [String: Any] = ["host": host, "port": Int(port)]
        if let fingerprint { j["fingerprint"] = fingerprint }
        self.init(target: j, route: nil)!
    }

    init?(target: Any?, route: Any?) {
        guard let t = target as? [String: Any], let host = t["host"] as? String,
              let port = (t["port"] as? NSNumber).flatMap({ UInt16(exactly: $0.intValue) }) else { return nil }
        self.host = host
        self.port = port
        fingerprint = t["fingerprint"] as? String
        var plain: [String: Any] = ["host": host, "port": Int(port)]
        if let fingerprint { plain["fingerprint"] = fingerprint }
        targetJSON = LendJSON.text(plain)
        routeJSON = (route as? [String: Any]).map(LendJSON.text)
    }
}

/// Un lien d'invitation lu (`kybernow://borrow?…`, ou le lien du client web
/// `https://…/link/?borrow=…`) : la Station et l'identifiant de l'ami. L'app
/// garde l'ordinateur à part de ses ordinateurs appairés.
public struct LendInvite: Sendable, Equatable, Codable, Identifiable {
    public let host: String
    public let port: UInt16
    public let fingerprint: String?
    public let login: String
    public let relay: String?
    public let sid: String?
    /// Le JSON du cœur (`{host, port, fingerprint, login, relay, sid, route}`),
    /// gardé tel quel : il donne la Station et la route.
    public let json: String

    public var id: String { json }

    public init?(json text: String) {
        guard let j = LendJSON.object(text), let host = j["host"] as? String, let login = j["login"] as? String,
              let port = (j["port"] as? NSNumber).flatMap({ UInt16(exactly: $0.intValue) }) else { return nil }
        self.host = host
        self.port = port
        self.login = login
        fingerprint = j["fingerprint"] as? String
        relay = j["relay"] as? String
        sid = j["sid"] as? String
        json = text
    }

    /// Un compte d'ami sur une Station du réseau, sans lien d'invitation.
    public init(host: String, port: UInt16, fingerprint: String?, login: String) {
        var j: [String: Any] = ["host": host, "port": Int(port), "login": login]
        if let fingerprint { j["fingerprint"] = fingerprint }
        self.init(json: LendJSON.text(j))!
    }

    /// Le même ordinateur pour le même ami (pour ne pas l'ajouter deux fois).
    public func same(as other: LendInvite) -> Bool {
        host.caseInsensitiveCompare(other.host) == .orderedSame && port == other.port && login == other.login
    }

    /// La Station de l'invitation et, si le lien en porte un, son relais.
    public var target: LendTarget? {
        let j = LendJSON.object(json)
        return LendTarget(target: j, route: j?["route"])
    }
}

/// Ce qui ouvre le prêt : un code pour une séance, ou un compte d'ami.
public enum LendCredentials: Sendable, Equatable {
    case code(String)
    case account(login: String, password: String)

    var fields: [String: Any] {
        switch self {
        case let .code(code): return ["code": code]
        case let .account(login, password): return ["login": login, "password": password]
        }
    }
}

/// L'écran de l'appareil, que la Station reproduit en écran virtuel.
public struct LendScreen: Sendable, Equatable {
    public let width: Int
    public let height: Int
    public let refresh: Int

    public init(width: Int, height: Int, refresh: Int) {
        self.width = max(640, width) / 2 * 2
        self.height = max(360, height) / 2 * 2
        self.refresh = refresh > 0 ? refresh : 60
    }
}

/// Une demande d'emprunt : à qui, avec quoi, pour quel écran. Gardée par la
/// session d'emprunt pour « Revenir » (le même appareil, le même code ou le
/// même compte, pendant 15 minutes).
public struct BorrowRequest: Sendable, Equatable {
    public let target: LendTarget
    public let credentials: LendCredentials
    public let name: String
    public let screen: LendScreen

    public init(target: LendTarget, credentials: LendCredentials, name: String, screen: LendScreen) {
        self.target = target
        self.credentials = credentials
        self.name = name
        self.screen = screen
    }

    /// `request_json` du cœur.
    func json(capabilities: DecoderCapabilities) -> String {
        var codecs: [String] = []
        if capabilities.hevc { codecs.append("hevc") }
        codecs.append("h264")
        if capabilities.av1 { codecs.append("av1") }
        var fields = credentials.fields
        fields["name"] = name
        fields["device"] = ConsoleClient.thisDeviceID
        fields["app"] = Share.app
        fields["screen"] = ["width": screen.width, "height": screen.height, "refresh": screen.refresh]
        fields["codecs"] = codecs
        return LendJSON.text(fields)
    }
}

/// Le prêt obtenu (`{target, borrowed}` du cœur) : à donner à la session
/// d'emprunt (`SessionEngine.Params(borrower:)`).
public struct LendLoan: Sendable {
    /// Repassé tel quel au cœur (`kn_lend_leave`, `borrower.borrowed`).
    let json: String
    public let target: ShareTarget
    /// {hôte} des textes du prêt : le prénom de l'hôte, sinon l'ordinateur.
    public let host: String
    /// Le nom de l'ordinateur.
    public let computer: String
    /// La fin prévue ; nil : « Jusqu'à ce que {hôte} reprenne ».
    public let until: Date?
    /// L'écran virtuel fait pour l'appareil.
    public let display: (width: Int, height: Int, refresh: Int)?
    /// Venu par le relais : la session y passe.
    public let viaRelay: Bool

    init?(json text: String) {
        guard let j = LendJSON.object(text), let target = j["target"] as? [String: Any], let host = target["host"] as? String,
              let port = (target["port"] as? NSNumber).flatMap({ UInt16(exactly: $0.intValue) }),
              let borrowed = j["borrowed"] as? [String: Any] else { return nil }
        json = text
        self.target = ShareTarget(host: host, port: port, fingerprint: target["fingerprint"] as? String)
        let computer = (borrowed["computer"] as? String ?? "").trimmingCharacters(in: .whitespaces)
        self.computer = computer.isEmpty ? host : computer
        let name = (borrowed["host"] as? String ?? "").trimmingCharacters(in: .whitespaces)
        self.host = name.isEmpty ? self.computer : name
        until = (borrowed["until"] as? NSNumber).map { Date(timeIntervalSince1970: $0.doubleValue) }
        if let d = borrowed["display"] as? [String: Any], let w = d["width"] as? Int, let h = d["height"] as? Int, w > 0, h > 0 {
            display = (w, h, d["refresh"] as? Int ?? 60)
        } else {
            display = nil
        }
        viaRelay = borrowed["route"] is [String: Any]
    }

    #if DEBUG
    /// Démonstration (Debug, sans Station) : un prêt pour de faux, pour
    /// dessiner la session d'emprunt.
    public init(demoComputer: String, host: String, until: Date?) {
        json = "{}"
        target = ShareTarget(host: "192.0.2.1", port: 1)
        computer = demoComputer
        self.host = host
        self.until = until
        display = nil
        viaRelay = false
    }
    #endif

    /// `borrowed`, pour `borrower` dans le JSON de `kn_session_start`.
    var borrowed: Any {
        LendJSON.object(json)?["borrowed"] ?? [:]
    }
}

/// Une demande qui attend l'accord de l'hôte (« En attente de {hôte}… ») : à
/// donner à `Lend.wait` jusqu'à sa réponse, ou à `Lend.cancel` (« Annuler »).
public struct LendPending: Sendable {
    let json: String
    /// Le prénom de l'hôte (« En attente de {hôte}… »), sinon l'ordinateur.
    public let host: String
    public let computer: String

    init?(json value: Any) {
        guard let j = value as? [String: Any] else { return nil }
        json = LendJSON.text(j)
        let computer = (j["computer"] as? String ?? "").trimmingCharacters(in: .whitespaces)
        let target = (j["target"] as? [String: Any])?["host"] as? String ?? ""
        self.computer = computer.isEmpty ? target : computer
        let name = (j["host"] as? String ?? "").trimmingCharacters(in: .whitespaces)
        host = name.isEmpty ? self.computer : name
    }

    #if DEBUG
    /// Démonstration : « En attente de {hôte}… », sans Station.
    public init(demoHost: String) {
        json = "{}"
        host = demoHost
        computer = demoHost
    }
    #endif
}

/// L'issue de « Emprunter » : le prêt démarre, ou l'hôte doit accepter.
public enum LendBorrowing: Sendable {
    case ready(LendLoan)
    case pending(LendPending)
}

/// « Rejoindre avec un code » est tombé sur un code de prêt (`KN_SHARE_LEND`) :
/// la Station qui l'a, et le chemin pris. L'app emprunte avec ce code.
public struct LendCodeFound: Sendable, Equatable {
    public let code: String
    public let target: LendTarget

    init?(json text: String?) {
        guard let j = LendJSON.object(text), let code = j["code"] as? String,
              let target = LendTarget(target: j["target"], route: j["route"]) else { return nil }
        self.code = code
        self.target = target
    }
}

/// Un refus de l'emprunt, avec le texte du cœur à montrer tel quel.
public enum LendError: LocalizedError, Sendable, Equatable {
    /// « Identifiant ou mot de passe refusé. »
    case badCredentials(String)
    /// « Ce code ne sert plus. »
    case noCode(String)
    /// « L'ordinateur est déjà prêté. »
    case lent(String)
    /// « L'ordinateur est verrouillé. »
    case locked(String)
    /// « Le prêt est coupé sur cet ordinateur. »
    case off(String)
    /// Trop d'essais.
    case tooMany(String)
    /// « {hôte} a refusé. »
    case refused(String)
    /// « {hôte} n'a pas répondu. »
    case expired(String)
    /// « Le prêt demande KyberNow Station 0.9.0 ou plus récente. »
    case unsupported(String)
    /// Le certificat de la Station a changé depuis l'invitation.
    case certificateChanged
    /// Le reste : Station injoignable, relais, réponse illisible (message du cœur).
    case other(String)

    public var errorDescription: String? {
        switch self {
        case let .badCredentials(m), let .noCode(m), let .lent(m), let .locked(m), let .off(m), let .tooMany(m),
             let .refused(m), let .expired(m), let .unsupported(m), let .other(m): return m
        case .certificateChanged: return String(localized: "le certificat de la Station a changé : ce n'est peut-être pas votre ordinateur", bundle: .kyberKit)
        }
    }

    /// La Station ne répond pas (ni sur le réseau ni par le relais) : la carte dit « Hors ligne ».
    public var unreachable: Bool {
        if case .other = self { return true }
        return false
    }

    init(status: Int32, message: String) {
        switch status {
        case KN_LEND_BAD_CREDENTIALS: self = .badCredentials(message)
        case KN_LEND_NO_CODE: self = .noCode(message)
        case KN_LEND_LENT: self = .lent(message)
        case KN_LEND_LOCKED: self = .locked(message)
        case KN_LEND_OFF: self = .off(message)
        case KN_LEND_TOO_MANY: self = .tooMany(message)
        case KN_LEND_REFUSED: self = .refused(message)
        case KN_LEND_EXPIRED: self = .expired(message)
        case KN_LEND_UNSUPPORTED: self = .unsupported(message)
        case KN_CERTIFICATE_CHANGED: self = .certificateChanged
        default: self = .other(message)
        }
    }
}

// MARK: - Pendant le prêt

/// Le préavis de 2 minutes : sa raison, et l'heure de la fin.
public struct LendNotice: Sendable, Equatable {
    public enum Reason: String, Sendable, Equatable {
        /// L'hôte reprend l'ordinateur.
        case takenBack = "taken_back"
        /// La durée du code est passée.
        case timeUp = "time_up"
        /// Le créneau de l'ami finit.
        case slotEnd = "slot_end"
        /// Chez l'hôte : le créneau d'un ami commence.
        case slotStart = "slot_start"
    }

    public let reason: Reason
    public let at: Date

    public init(reason: Reason, at: Date) {
        self.reason = reason
        self.at = at
    }

    init?(json value: Any?) {
        guard let j = value as? [String: Any], let reason = Reason(rawValue: j["reason"] as? String ?? "") else { return nil }
        self.init(reason: reason, at: Date(timeIntervalSince1970: (j["at"] as? NSNumber)?.doubleValue ?? 0))
    }

    init?(json text: String?) {
        self.init(json: LendJSON.object(text) as Any?)
    }

    /// Le texte de link-ui : chez l'ami, `name` est le prénom de l'hôte ; pour
    /// `slotStart`, chez l'hôte, celui de l'ami.
    public func text(name: String) -> String {
        let json = LendJSON.text(["reason": reason.rawValue, "at": Int(at.timeIntervalSince1970)])
        return Core.take(kn_lend_notice_text(json, name)) ?? ""
    }
}

/// La fin d'un emprunt (`on_lend_end`).
public enum LendEnd: Sendable, Equatable {
    /// « {hôte} a repris l'ordinateur. »
    case takenBack
    /// « Le prêt est terminé. » (durée du code passée)
    case timeUp
    /// « Votre créneau est terminé. »
    case slotEnd
    /// L'ami a rendu l'ordinateur : une fin voulue (« Vous avez rendu l'ordinateur. »).
    case left
    /// « Le prêt s'est arrêté. » (la Station ne le connaît plus).
    case stopped

    init(code: Int32) {
        switch code {
        case KN_LEND_END_TAKEN_BACK: self = .takenBack
        case KN_LEND_END_TIME_UP: self = .timeUp
        case KN_LEND_END_SLOT_END: self = .slotEnd
        case KN_LEND_END_LEFT: self = .left
        default: self = .stopped
        }
    }
}

// MARK: - L'hôte

/// Le compte Steam du prêt : « Son compte Steam » (proposé), « Votre compte Steam ».
public enum LendSteam: Int, CaseIterable, Sendable, Equatable {
    case friend = 0
    case host = 1

    init(wire: String?) {
        self = wire == "host" ? .host : .friend
    }
}

/// L'état du prêt chez l'hôte.
public enum LendHostState: String, Sendable, Equatable {
    /// Rien de prêté.
    case free
    /// Le mode console se met en place.
    case preparing
    /// « Prêté à {prénom} ».
    case lent
    /// Les apps du prêt se ferment, la session se verrouille.
    case ending
}

/// Comment l'ami est venu.
public enum LendKind: String, Sendable, Equatable {
    case code, friend, slot
}

/// L'ami qui a l'ordinateur.
public struct LendBorrower: Sendable, Equatable {
    public let name: String
    public let kind: LendKind
    public let app: String
    public let viaRelay: Bool
    public let since: Date?
    /// La fin prévue ; nil : jusqu'à ce que l'hôte reprenne.
    public let until: Date?
    public let steam: LendSteam
    /// Sa session est là (sinon : il peut revenir pendant 15 min).
    public let connected: Bool

    public init(name: String, kind: LendKind = .code, app: String = "ios", viaRelay: Bool = false, since: Date? = nil,
                until: Date? = nil, steam: LendSteam = .friend, connected: Bool = true) {
        self.name = name
        self.kind = kind
        self.app = app
        self.viaRelay = viaRelay
        self.since = since
        self.until = until
        self.steam = steam
        self.connected = connected
    }
}

/// Une demande d'emprunt, ordinateur utilisé : « {prénom} veut emprunter
/// l'ordinateur. », « Accepter », « Refuser ».
public struct LendRequest: Identifiable, Sendable, Equatable {
    public let id: String
    public let name: String
    public let kind: LendKind
    public let app: String
    public let viaRelay: Bool
    public let since: Date?
    /// Le compte Steam proposé (celui du compte d'ami, ou du code) : celui d'« Accepter ».
    public let steam: LendSteam
    /// La demande vient d'un créneau de l'ami : l'ordinateur lui est prêté
    /// dans 2 min sans réponse (« Créneau de {prénom} : … »).
    public let slot: Bool

    public init(id: String, name: String, kind: LendKind = .code, app: String = "ios", viaRelay: Bool = false, since: Date? = nil,
                steam: LendSteam = .friend, slot: Bool = false) {
        self.id = id
        self.name = name
        self.kind = kind
        self.app = app
        self.viaRelay = viaRelay
        self.since = since
        self.steam = steam
        self.slot = slot
    }
}

/// Le code de séance en cours (« Prêter l'ordinateur… »).
public struct LendCodeView: Sendable, Equatable {
    /// Six chiffres, sans espace.
    public let code: String
    /// `kybernow://join?…` : le QR code.
    public let link: String
    /// Le client web : « Copier le lien ».
    public let web: String
    /// En secondes ; nil : « Jusqu'à ce que je reprenne ».
    public let duration: Int?
    public let steam: LendSteam
    /// Le code ne sert plus après cette heure (24 h).
    public let expires: Date?

    public init(code: String, link: String, web: String, duration: Int?, steam: LendSteam, expires: Date? = nil) {
        self.code = code
        self.link = link
        self.web = web
        self.duration = duration
        self.steam = steam
        self.expires = expires
    }
}

/// La vue du prêt chez l'hôte : la réponse de chaque `ConsoleClient.lend*`, et
/// ce que dit le signe de vie de l'hôte (`SessionEngine.onLend`).
public struct LendView: Sendable, Equatable {
    /// Le prêt est activé dans Admin (page « Prêt »).
    public var enabled: Bool
    /// Le nom de l'ordinateur et le prénom de l'hôte, quand la Station les donne.
    public var computer: String = ""
    public var host: String = ""
    public var state: LendHostState
    public var borrower: LendBorrower?
    public var notice: LendNotice?
    public var requests: [LendRequest]
    public var code: LendCodeView?

    public init(enabled: Bool = true, state: LendHostState = .free, borrower: LendBorrower? = nil, notice: LendNotice? = nil,
                requests: [LendRequest] = [], code: LendCodeView? = nil) {
        self.enabled = enabled
        self.state = state
        self.borrower = borrower
        self.notice = notice
        self.requests = requests
        self.code = code
    }

    /// L'ordinateur est prêté (ou se prépare, ou se rend) : « Prêté à {prénom} ».
    public var isLent: Bool { borrower != nil && state != .free }

    init?(json text: String?) {
        guard let j = LendJSON.object(text) else { return nil }
        func date(_ v: Any?) -> Date? { (v as? NSNumber).map { Date(timeIntervalSince1970: $0.doubleValue) }.flatMap { $0.timeIntervalSince1970 > 0 ? $0 : nil } }
        let borrower = (j["borrower"] as? [String: Any]).map { b in
            LendBorrower(name: b["name"] as? String ?? "", kind: LendKind(rawValue: b["kind"] as? String ?? "") ?? .code,
                         app: b["app"] as? String ?? "", viaRelay: b["via"] as? String == "relay", since: date(b["since"]),
                         until: date(b["until"]), steam: LendSteam(wire: b["steam"] as? String), connected: b["connected"] as? Bool ?? false)
        }
        let requests = (j["requests"] as? [[String: Any]] ?? []).compactMap { r -> LendRequest? in
            guard let id = r["id"] as? String, !id.isEmpty else { return nil }
            return LendRequest(id: id, name: r["name"] as? String ?? "", kind: LendKind(rawValue: r["kind"] as? String ?? "") ?? .code,
                               app: r["app"] as? String ?? "", viaRelay: r["via"] as? String == "relay", since: date(r["since"]),
                               steam: LendSteam(wire: r["steam"] as? String), slot: r["slot"] as? Bool ?? false)
        }
        let code = (j["code"] as? [String: Any]).flatMap { c -> LendCodeView? in
            guard let code = c["code"] as? String, !code.isEmpty else { return nil }
            return LendCodeView(code: code, link: c["link"] as? String ?? "", web: c["web"] as? String ?? "",
                                duration: (c["duration"] as? NSNumber)?.intValue, steam: LendSteam(wire: c["steam"] as? String),
                                expires: date(c["expires"]))
        }
        self.init(enabled: j["enabled"] as? Bool ?? false, state: LendHostState(rawValue: j["state"] as? String ?? "") ?? .free,
                  borrower: borrower, notice: LendNotice(json: j["notice"]), requests: requests, code: code)
        computer = j["computer"] as? String ?? ""
        host = j["host"] as? String ?? ""
    }
}

/// `info.lend` (Station 0.9.0 et suivantes) : le prêt existe sur cet
/// ordinateur, et il est activé dans Admin. Absent : Station trop ancienne.
public struct LendInfo: Sendable, Equatable {
    public let available: Bool
    public let enabled: Bool

    public init(available: Bool, enabled: Bool) {
        self.available = available
        self.enabled = enabled
    }

    init?(json value: Any?) {
        guard let j = value as? [String: Any] else { return nil }
        self.init(available: j["available"] as? Bool ?? false, enabled: j["enabled"] as? Bool ?? false)
    }
}

// MARK: - Appels

public enum Lend {
    /// Lit un lien d'invitation (`kybernow://borrow?…` ou celui du client web).
    public static func parseInvite(_ url: String) -> LendInvite? {
        Core.take(kn_lend_parse_invite(url)).flatMap(LendInvite.init(json:))
    }

    /// L'état de l'ordinateur d'un ami (la carte de l'accueil). Bloquant
    /// quelques secondes au plus : hors du fil principal.
    public static func status(_ target: LendTarget, login: String, password: String) async throws -> LendStatus {
        let t = target.targetJSON, r = target.routeJSON
        let result = await blocking { out, error in kn_lend_status(t, r, login, password, &out, &error) }
        guard result.status == KN_OK else { throw LendError(status: result.status, message: result.message) }
        guard let status = LendStatus(json: result.out) else { throw unexpected }
        return status
    }

    /// « Emprunter » : le prêt démarre (30 s au plus, le temps du mode
    /// console), ou il faut l'accord de l'hôte (`pending`).
    public static func borrow(_ request: BorrowRequest, capabilities: DecoderCapabilities = .current) async throws -> LendBorrowing {
        let t = request.target.targetJSON, r = request.target.routeJSON
        let body = request.json(capabilities: capabilities)
        // Hors de chez soi sur iPhone ou iPad : la radio cellulaire d'abord.
        if r != nil { await Relay.wakeRadio() }
        let result = await blocking { out, error in kn_lend_borrow(t, r, body, &out, &error) }
        guard result.status == KN_OK else { throw LendError(status: result.status, message: result.message) }
        guard let j = LendJSON.object(result.out) else { throw unexpected }
        if let ready = j["ready"], let loan = LendLoan(json: LendJSON.text(ready)) { return .ready(loan) }
        if let pending = LendPending(json: j["pending"] as Any) { return .pending(pending) }
        throw unexpected
    }

    /// L'hôte a-t-il répondu ? Un appel tient 30 s : nil, toujours en attente
    /// (rappeler) ; sinon le prêt. Refus et délai : « {hôte} a refusé. »,
    /// « {hôte} n'a pas répondu. ». Hors du fil principal.
    public static func wait(_ pending: LendPending) async throws -> LendLoan? {
        let text = pending.json
        let result = await blocking { out, error in kn_lend_wait(text, &out, &error) }
        switch result.status {
        case KN_OK:
            guard let out = result.out, let loan = LendLoan(json: out) else { throw unexpected }
            return loan
        case KN_LEND_WAITING: return nil
        default: throw LendError(status: result.status, message: result.message)
        }
    }

    /// « Annuler » la demande en attente : l'hôte ne la voit plus.
    public static func cancel(_ pending: LendPending) {
        let text = pending.json
        guard text != "{}" else { return }
        DispatchQueue.global(qos: .utility).async { _ = kn_lend_cancel(text) }
    }

    /// Rendre l'ordinateur hors d'une session (prêt obtenu, session jamais
    /// ouverte) ; en session : `SessionEngine.giveBack()`.
    public static func leave(_ loan: LendLoan) {
        let text = loan.json
        guard text != "{}" else { return }
        DispatchQueue.global(qos: .utility).async {
            var error: UnsafeMutablePointer<CChar>?
            _ = kn_lend_leave(text, &error)
            _ = Core.take(error)
        }
    }

    private static var unexpected: LendError {
        .other(String(localized: "réponse inattendue de la Station", bundle: .kyberKit))
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
}

// MARK: - Hôte : les appels d'Admin

extension ConsoleClient {
    /// La vue du prêt de cet ordinateur.
    public func lendState() async throws -> LendView {
        try Self.lendView(try await call { console -> String? in
            var out: UnsafeMutablePointer<CChar>?
            var error: UnsafeMutablePointer<CChar>?
            try Self.check(kn_console_lend_state(console, &out, &error), error)
            return Core.take(out)
        })
    }

    /// « Créer un code » : `duration` en secondes, nil pour « Jusqu'à ce que je reprenne ».
    public func lendCode(duration: Int?, steam: LendSteam) async throws -> LendView {
        let seconds = Int64(duration ?? 0)
        let steamCode = Int32(steam.rawValue)
        return try Self.lendView(try await call { console -> String? in
            var out: UnsafeMutablePointer<CChar>?
            var error: UnsafeMutablePointer<CChar>?
            try Self.check(kn_console_lend_code(console, seconds, steamCode, &out, &error), error)
            return Core.take(out)
        })
    }

    /// « Annuler le code ».
    public func lendCancelCode() async throws -> LendView {
        try Self.lendView(try await call { console -> String? in
            var out: UnsafeMutablePointer<CChar>?
            var error: UnsafeMutablePointer<CChar>?
            try Self.check(kn_console_lend_cancel_code(console, &out, &error), error)
            return Core.take(out)
        })
    }

    /// « Accepter » (avec le compte Steam) ou « Refuser » une demande.
    public func lendAnswer(_ request: String, accept: Bool, steam: LendSteam = .friend) async throws -> LendView {
        let steamCode = Int32(steam.rawValue)
        return try Self.lendView(try await call { console -> String? in
            var out: UnsafeMutablePointer<CChar>?
            var error: UnsafeMutablePointer<CChar>?
            try Self.check(kn_console_lend_answer(console, request, accept, steamCode, &out, &error), error)
            return Core.take(out)
        })
    }

    /// « Reprendre l'ordinateur » (préavis de 2 min) ou, `now`, « Reprendre tout de suite ».
    public func lendTakeBack(now: Bool) async throws -> LendView {
        try Self.lendView(try await call { console -> String? in
            var out: UnsafeMutablePointer<CChar>?
            var error: UnsafeMutablePointer<CChar>?
            try Self.check(kn_console_lend_take_back(console, now, &out, &error), error)
            return Core.take(out)
        })
    }

    private static func lendView(_ text: String?) throws -> LendView {
        guard let view = LendView(json: text) else {
            throw ConsoleError.network(String(localized: "réponse inattendue de la Station", bundle: .kyberKit))
        }
        return view
    }
}

// MARK: - JSON

enum LendJSON {
    static func object(_ text: String?) -> [String: Any]? {
        guard let text, let data = text.data(using: .utf8) else { return nil }
        return (try? JSONSerialization.jsonObject(with: data)) as? [String: Any]
    }

    static func text(_ value: Any) -> String {
        (try? JSONSerialization.data(withJSONObject: value)).flatMap { String(data: $0, encoding: .utf8) } ?? "{}"
    }
}
