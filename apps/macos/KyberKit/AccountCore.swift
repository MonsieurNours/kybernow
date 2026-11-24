// Les comptes KyberNow sur le cœur Rust (docs/comptes.md, lot 3 ; textes :
// docs/link-ui.md, « Comptes KyberNow ») : la connexion de l'appareil (page
// du relais dans la fenêtre du système sur iPhone, iPad et Mac, code et QR
// code sur l'Apple TV), les ordinateurs du compte, un ordinateur une fois à
// l'accueil, la pastille d'origine, la console et la session d'un ordinateur
// du compte (jeton d'accès demandé et renouvelé par le cœur). Un seul
// composant pour le Mac, l'Apple TV, l'iPhone et l'iPad : les appels sont
// ceux du pont C du cœur (`kn_account_*`, `kn_console_new_account`,
// apps/apple/src/account.rs) ; les refus et les pastilles viennent du cœur,
// dans la langue de l'app.
//
// Le compte (`{relay, token, account, device}`) se garde entier dans le
// Trousseau de l'appareil, jamais dans iCloud : c'est la session de cet
// appareil (le modèle de l'app, apps/macos/KyberNow/Accounts.swift).

import Foundation
@_implementationOnly import CKyberNow

/// Pourquoi un appel au relais n'aboutit pas : le code du cœur
/// (`KN_ACCOUNT_*`, 70 et suivants), jamais reconnu au texte.
public enum AccountFailure: Int32, Sendable, Equatable {
    /// La session de l'appareil est fermée (page du compte, nouveau mot de
    /// passe) : l'app oublie le compte et le dit.
    case signedOut = 70
    /// L'ordinateur n'est plus au compte : il quitte l'accueil.
    case unknownComputer = 71
    /// Code de l'Apple TV refusé sur le téléphone.
    case denied = 72
    /// Code de l'Apple TV échu : un autre s'affiche de lui-même.
    case expired = 73
    /// Retour de la fenêtre refusé.
    case badCode = 74
    /// « Annuler » dans la page du relais : rien à dire.
    case cancelled = 75
    case tooMany = 76
    /// Le relais n'a pas de comptes KyberNow.
    case noAccounts = 77
    case badRelay = 78
    case unreachable = 79
    case other = 80
}

/// Un refus, avec le texte du cœur à montrer tel quel.
public struct AccountError: LocalizedError, Sendable, Equatable {
    public let failure: AccountFailure
    public let message: String

    public init(_ failure: AccountFailure, _ message: String) {
        self.failure = failure
        self.message = message
    }

    init(status: Int32, message: String) {
        self.init(AccountFailure(rawValue: status) ?? .other, message)
    }

    public var errorDescription: String? { message }
}

/// Un compte connecté sur cet appareil. `json` est le compte entier, jeton
/// compris : à garder dans le Trousseau, jamais ailleurs, jamais au journal.
public struct KyberAccount: Sendable, Equatable, Identifiable {
    /// Le relais et l'identifiant du compte (`relay|acc_…`) : deux relais ont
    /// leurs propres comptes.
    public let key: String
    /// Le relais, sous sa forme normale (empreinte d'un relais chez soi gardée).
    public let relay: String
    /// Le relais à montrer : `share.kybernow.app`, `relais.exemple.org:9443`.
    public let relayLabel: String
    public let name: String
    public let email: String
    /// La page du compte (« Gérer le compte ») : `https://{relais}/compte`.
    public let console: String
    /// Le compte entier, en JSON (le cœur le relit à chaque appel).
    let json: String

    public var id: String { key }

    /// « Connecté à {compte}. » : le nom, sinon l'e-mail.
    public var displayName: String { name.isEmpty ? email : name }

    /// Lu dans le JSON du cœur (`kn_account_finish_sign_in`,
    /// `kn_account_tv_poll`, ou le Trousseau) ; nil s'il ne se lit pas.
    public init?(json text: String) {
        guard let summary = Core.take(kn_account_summary(text)), let j = AccountJSON.object(summary),
              let key = j["key"] as? String, !key.isEmpty else { return nil }
        let account = j["account"] as? [String: Any]
        self.key = key
        relay = j["relay"] as? String ?? ""
        relayLabel = j["relay_label"] as? String ?? ""
        name = (account?["name"] as? String ?? "").trimmingCharacters(in: .whitespacesAndNewlines)
        email = account?["email"] as? String ?? ""
        console = j["console"] as? String ?? ""
        json = text
    }

    /// Le JSON à garder dans le Trousseau.
    public var secret: String { json }

    public static func == (a: KyberAccount, b: KyberAccount) -> Bool { a.json == b.json }
}

/// Un ordinateur du compte (`GET /v1/computers` du relais, lu par le cœur).
/// Rien de secret : l'app garde la dernière liste de chaque compte (l'empreinte
/// d'Admin n'arrive que d'une Station connectée).
public struct AccountComputer: Codable, Sendable, Equatable, Hashable {
    public struct Owner: Codable, Sendable, Equatable, Hashable {
        /// `person` ou `organization`.
        public var kind: String
        public var id: String
        public var name: String

        public init(from decoder: Decoder) throws {
            let c = try decoder.container(keyedBy: CodingKeys.self)
            kind = (try? c.decodeIfPresent(String.self, forKey: .kind)) ?? ""
            id = (try? c.decodeIfPresent(String.self, forKey: .id)) ?? ""
            name = (try? c.decodeIfPresent(String.self, forKey: .name)) ?? ""
        }
    }

    /// Le compte qui le donne (`KyberAccount.key`).
    public var account: String
    /// Le relais du compte, celui de la Station.
    public var relay: String
    /// L'identifiant de la Station chez le relais.
    public var station: String
    public var name: String
    /// `session`, `lend`, `share`, `admin`.
    public var rights: [String]
    public var owner: Owner?
    /// Rattaché à ce compte (et non ouvert par quelqu'un d'autre).
    public var mine: Bool
    /// Connecté au relais quand la liste est arrivée.
    public var online: Bool
    /// L'empreinte d'Admin, vérifiée par le cœur ; nil tant que la Station n'a
    /// pas été vue connectée.
    public var adminFingerprint: String?
    public var attachedAt: Int64

    private enum CodingKeys: String, CodingKey {
        case account, relay, station, name, rights, owner, mine, online
        case adminFingerprint = "admin_fp"
        case attachedAt = "attached_at"
    }

    // Lecture tolérante : un champ absent garde sa valeur par défaut.
    public init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        account = (try? c.decodeIfPresent(String.self, forKey: .account)) ?? ""
        relay = (try? c.decodeIfPresent(String.self, forKey: .relay)) ?? ""
        station = try c.decode(String.self, forKey: .station)
        name = (try? c.decodeIfPresent(String.self, forKey: .name)) ?? ""
        rights = (try? c.decodeIfPresent([String].self, forKey: .rights)) ?? []
        owner = try? c.decodeIfPresent(Owner.self, forKey: .owner)
        mine = (try? c.decodeIfPresent(Bool.self, forKey: .mine)) ?? false
        online = (try? c.decodeIfPresent(Bool.self, forKey: .online)) ?? false
        adminFingerprint = (try? c.decodeIfPresent(String.self, forKey: .adminFingerprint)).flatMap { $0.isEmpty ? nil : $0 }
        attachedAt = (try? c.decodeIfPresent(Int64.self, forKey: .attachedAt)) ?? 0
    }

    /// Le même ordinateur chez le même relais.
    public func same(_ other: AccountComputer) -> Bool {
        relay == other.relay && station == other.station
    }

    /// Le JSON du cœur.
    var json: String { AccountJSON.encode(self) }
}

/// Un code de l'Apple TV (`kn_account_tv_code`) : à afficher avec le QR code
/// de `urlComplete`, et « ou ouvrez {urlLabel} et entrez le code ».
public struct AccountTVCode: Sendable, Equatable {
    /// `KQ7M-4XPA`.
    public let code: String
    /// Le relais, sous sa forme normale.
    public let relay: String
    /// Une demande toutes les `interval` secondes.
    public let interval: Int
    /// La page avec le code : le QR code.
    public let urlComplete: String
    /// `share.kybernow.app/tv`, à écrire à l'écran.
    public let urlLabel: String
    /// Le JSON du cœur (il porte le jeton de la demande : secret).
    let json: String

    init?(json text: String?) {
        guard let text, let j = AccountJSON.object(text), let code = j["code"] as? String, !code.isEmpty else { return nil }
        self.code = code
        relay = j["relay"] as? String ?? ""
        interval = max(1, (j["interval"] as? NSNumber)?.intValue ?? 5)
        urlComplete = j["url_complete"] as? String ?? ""
        urlLabel = j["url_label"] as? String ?? ""
        json = text
    }

    /// « Le code change dans {n} min. » ; 0 : échu.
    public var minutesLeft: Int { Int(kn_account_tv_minutes_left(json)) }
}

/// La connexion par la fenêtre du système (iPhone, iPad, Mac) : la page à
/// ouvrir dans `ASWebAuthenticationSession`, avec son schéma de retour.
public struct AccountSignIn: Sendable, Equatable {
    public let url: URL
    /// `kybernow`.
    public let callbackScheme: String
    /// Le JSON du cœur, à rendre avec le retour (`state` le lie à la demande).
    let json: String
}

/// Un ordinateur appairé, pour un ordinateur une fois à l'accueil : son
/// identifiant chez l'app, l'empreinte d'Admin épinglée, et son identifiant
/// de Station chez son relais (`info.relay.id`) s'il est connu.
public struct AccountPaired: Sendable, Equatable {
    public let id: String
    public let fingerprint: String?
    public let station: String?

    public init(id: String, fingerprint: String?, station: String?) {
        self.id = id
        self.fingerprint = fingerprint
        self.station = station
    }
}

public enum Accounts {
    /// Le relais public, proposé d'abord (`share.kybernow.app`).
    public static var defaultRelay: String { Relay.defaultServer }

    /// La forme normale d'un relais tapé (« Un autre relais… ») ; vide :
    /// le relais public. Refus : `badRelay` (« Adresse du relais illisible. »).
    public static func normalizeRelay(_ relay: String) throws -> String {
        var out: UnsafeMutablePointer<CChar>?
        var error: UnsafeMutablePointer<CChar>?
        let status = kn_account_normalize_relay(relay, &out, &error)
        let message = Core.take(error) ?? ""
        let text = Core.take(out)
        guard status == KN_OK, let text else { throw AccountError(status: status, message: message) }
        return text
    }

    /// Vrai pour un retour de la fenêtre du système (`kybernow://compte…`) :
    /// ce n'est ni un ordinateur, ni un partage, ni une invitation.
    public static func isCallback(_ url: String) -> Bool {
        kn_account_is_callback(url)
    }

    // MARK: Fenêtre du système

    /// « Se connecter à un compte KyberNow » : la page de connexion du relais
    /// pour cet appareil (`device` : « iPad de Léa », montré sur la page et
    /// dans « Appareils connectés »). Pas d'appel réseau.
    public static func signIn(relay: String, device: String) throws -> AccountSignIn {
        var out: UnsafeMutablePointer<CChar>?
        var error: UnsafeMutablePointer<CChar>?
        let status = kn_account_sign_in(relay, device, &out, &error)
        let message = Core.take(error) ?? ""
        let text = Core.take(out)
        guard status == KN_OK, let text, let j = AccountJSON.object(text),
              let url = (j["url"] as? String).flatMap(URL.init(string:)) else {
            throw AccountError(status: status == KN_OK ? AccountFailure.other.rawValue : status, message: message)
        }
        return AccountSignIn(url: url, callbackScheme: j["callback_scheme"] as? String ?? "kybernow", json: text)
    }

    /// Le retour de la fenêtre (`kybernow://compte?…`) échangé contre le
    /// compte. `cancelled` : « Annuler » dans la page, rien à montrer.
    /// Bloquant 8 s au plus : hors du fil principal.
    public static func finishSignIn(_ request: AccountSignIn, callback: String) async throws -> KyberAccount {
        let json = request.json
        return try await blocking { out, error in kn_account_finish_sign_in(json, callback, out, error) } read: { KyberAccount(json: $0) }
    }

    // MARK: Code (Apple TV)

    /// Un code pour cet appareil (« Apple TV du salon »). Bloquant : hors du fil principal.
    public static func tvCode(relay: String, device: String) async throws -> AccountTVCode {
        try await blocking { out, error in kn_account_tv_code(relay, device, out, error) } read: { AccountTVCode(json: $0) }
    }

    /// Une demande, toutes les `interval` secondes : nil tant que le
    /// téléphone n'a pas approuvé, puis le compte. `expired` : un nouveau
    /// code ; `denied` : refusé sur le téléphone. Bloquant.
    public static func tvPoll(_ code: AccountTVCode) async throws -> KyberAccount? {
        let json = code.json
        let answer: [String: Any] = try await blocking { out, error in kn_account_tv_poll(json, out, error) } read: { AccountJSON.object($0) }
        guard answer["status"] as? String == "approved" else { return nil }
        guard let account = (answer["account"] as? [String: Any]).flatMap(AccountJSON.string(from:)).flatMap(KyberAccount.init(json:)) else {
            throw AccountError(.other, "")
        }
        return account
    }

    // MARK: Le compte

    /// « Se déconnecter » : la session de l'appareil est fermée chez le
    /// relais. L'app oublie le compte de toute façon. Bloquant.
    public static func signOut(_ account: KyberAccount) async throws {
        let json = account.json
        let _: Bool = try await blocking { _, error in kn_account_sign_out(json, error) } read: { _ in true }
    }

    /// Les ordinateurs du compte. `signedOut` : la session de l'appareil est
    /// fermée, l'app oublie le compte. Bloquant.
    public static func computers(_ account: KyberAccount) async throws -> [AccountComputer] {
        let json = account.json
        return try await blocking { out, error in kn_account_computers(json, out, error) } read: { AccountJSON.decode([AccountComputer].self, $0) }
    }

    /// La liste qui arrive, avec les empreintes d'Admin déjà vues (le relais
    /// ne les donne que pour une Station connectée).
    public static func rememberFingerprints(fresh: [AccountComputer], known: [AccountComputer]) -> [AccountComputer] {
        guard !known.isEmpty else { return fresh }
        let text = Core.take(kn_account_remember_fingerprints(AccountJSON.encode(fresh), AccountJSON.encode(known)))
        return text.flatMap { AccountJSON.decode([AccountComputer].self, $0) } ?? fresh
    }

    /// L'accueil, un ordinateur une fois : `linked`, les appairés qu'un
    /// compte donne aussi (identifiant de l'appairé, ordinateur du compte :
    /// la pastille), `added`, les ordinateurs des comptes qui ne sont pas
    /// déjà là, sans doublon d'un compte à l'autre.
    public static func merge(paired: [AccountPaired], computers: [AccountComputer]) -> (linked: [String: AccountComputer], added: [AccountComputer]) {
        let list: [[String: Any]] = paired.map { p in
            var d: [String: Any] = ["id": p.id]
            if let fp = p.fingerprint, !fp.isEmpty { d["fingerprint"] = fp }
            if let s = p.station, !s.isEmpty { d["station"] = s }
            return d
        }
        let pairedJSON = AccountJSON.string(from: list) ?? "[]"
        guard let text = Core.take(kn_account_merge(pairedJSON, AccountJSON.encode(computers))), let j = AccountJSON.object(text) else {
            return ([:], computers)
        }
        var linked: [String: AccountComputer] = [:]
        for case let pair as [Any] in j["linked"] as? [Any] ?? [] {
            guard pair.count == 2, let id = pair[0] as? String,
                  let c = AccountJSON.string(from: pair[1]).flatMap({ AccountJSON.decode(AccountComputer.self, $0) }) else { continue }
            linked[id] = c
        }
        let added = (j["added"] as? [Any]).flatMap(AccountJSON.string(from:)).flatMap { AccountJSON.decode([AccountComputer].self, $0) } ?? []
        return (linked, added)
    }

    /// La pastille d'origine : « Compte », « Compte · {relais} » avec des
    /// comptes sur plusieurs relais, l'organisation, « Ouvert par {prénom} ».
    public static func origin(_ computer: AccountComputer, severalRelays: Bool) -> String {
        Core.take(kn_account_origin(computer.json, severalRelays)) ?? ""
    }

    /// La pastille d'un ordinateur appairé (« Appairé »), quand un compte est connecté.
    public static var originPaired: String {
        Core.take(kn_account_origin_paired()) ?? ""
    }

    /// La route par le relais d'un ordinateur du compte ; nil sans empreinte
    /// d'Admin connue (jamais de premier contact par le relais).
    public static func route(_ computer: AccountComputer) -> RelayRoute? {
        Core.take(kn_account_route(computer.json)).flatMap(RelayRoute.init(json:))
    }

    /// `{account, station}` : ce qu'il faut pour ouvrir cet ordinateur du
    /// compte (console et session), jeton du compte compris.
    public static func access(_ account: KyberAccount, station: String) -> AccountAccess? {
        guard let object = AccountJSON.object(account.json),
              let text = AccountJSON.string(from: ["account": object, "station": station] as [String: Any]) else { return nil }
        return AccountAccess(json: text)
    }

    // MARK: Appels bloquants

    /// Un appel bloquant du pont, sur une file à part : `out` lu par `read`,
    /// un refus en `AccountError` (texte du cœur).
    private static func blocking<T>(_ call: @escaping @Sendable (UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>, UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>) -> Int32,
                                    read: @escaping @Sendable (String) -> T?) async throws -> T {
        let result: (status: Int32, out: String?, message: String) = await withCheckedContinuation { continuation in
            DispatchQueue.global(qos: .userInitiated).async {
                var out: UnsafeMutablePointer<CChar>?
                var error: UnsafeMutablePointer<CChar>?
                let status = call(&out, &error)
                let message = Core.take(error) ?? ""
                continuation.resume(returning: (status, Core.take(out), message))
            }
        }
        guard result.status == KN_OK else { throw AccountError(status: result.status, message: result.message) }
        guard let value = read(result.out ?? "") else { throw AccountError(.other, result.message) }
        return value
    }
}

/// `{account, station}`, en JSON : la valeur `account` de la session
/// (`SessionEngine.Params.account`) et de `kn_console_new_account`. Secret
/// (le jeton du compte y est).
public struct AccountAccess: Sendable, Equatable {
    let json: String
}

extension ConsoleClient {
    /// La console d'un ordinateur du compte : Admin sur le réseau (`host`,
    /// `port`, quand il y est vu avec la même empreinte), sinon par le relais
    /// (`relay` : `Accounts.route`). Le cœur demande le jeton d'accès au
    /// relais et le renouvelle. Nil pour un compte illisible.
    public convenience init?(access: AccountAccess, host: String, port: UInt16, fingerprint: String?, relay: RelayRoute?) {
        guard let console = kn_console_new_account(relay == nil ? host : "", relay == nil ? port : 0, fingerprint, access.json, relay?.json) else { return nil }
        self.init(raw: console, pinned: fingerprint)
    }
}

/// Lecture et écriture des JSON du cœur.
enum AccountJSON {
    static func object(_ text: String) -> [String: Any]? {
        text.data(using: .utf8).flatMap { try? JSONSerialization.jsonObject(with: $0) } as? [String: Any]
    }

    static func string(from value: Any) -> String? {
        guard JSONSerialization.isValidJSONObject(value), let data = try? JSONSerialization.data(withJSONObject: value) else { return nil }
        return String(data: data, encoding: .utf8)
    }

    static func decode<T: Decodable>(_ type: T.Type, _ text: String) -> T? {
        text.data(using: .utf8).flatMap { try? JSONDecoder().decode(type, from: $0) }
    }

    static func encode<T: Encodable>(_ value: T) -> String {
        (try? JSONEncoder().encode(value)).flatMap { String(data: $0, encoding: .utf8) } ?? "null"
    }
}
