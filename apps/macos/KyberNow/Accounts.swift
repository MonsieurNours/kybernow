// Comptes KyberNow dans l'app (docs/comptes.md, lot 3 ; textes :
// docs/link-ui.md, « Comptes KyberNow ») : les comptes connectés sur cet
// appareil (plusieurs relais possibles), leurs ordinateurs à l'accueil avec les
// mêmes cartes que les autres, un ordinateur une fois, la pastille d'origine,
// la console et la session d'un ordinateur du compte. Commun au Mac, à
// l'iPhone, à l'iPad et à l'Apple TV ; chaque app dessine ses écrans (accueil
// sans ordinateur, réglages « Comptes », code de l'Apple TV).
//
// - Chaque compte est gardé entier (jeton compris) dans le Trousseau de cet
//   appareil, jamais dans iCloud : c'est la session de l'appareil
//   (`kSecAttrSynchronizable` faux). La dernière liste d'ordinateurs de chaque
//   compte, rien de secret, est dans les préférences (avec les empreintes
//   d'Admin déjà vues : le relais ne les donne que pour une Station connectée).
// - La liste est relue au lancement, au retour au premier plan, après la
//   connexion, et une fois par minute au plus pendant le sondage de l'accueil.
// - Un ordinateur du compte est un `Host` (`account`), sans mot de passe : ses
//   réglages et ses statistiques sont gardés comme ceux des autres ; il ne
//   part ni dans iCloud ni vers l'Apple TV (HostStore.save).
// - La création, le mot de passe et la suppression du compte se font sur la
//   page du compte (App Store, règle 5.1.1(v)) : l'app n'en crée pas.

import Combine
import Foundation
import KyberKit
import Security
#if os(macOS)
import AppKit
#else
import UIKit
#endif
#if os(iOS) || os(macOS)
import AuthenticationServices
#endif

/// Un ordinateur d'un compte KyberNow, gardé avec lui (`Host.account`) : le
/// compte qui le donne, son relais et sa Station chez ce relais.
struct AccountLink: Codable, Hashable {
    /// `KyberAccount.key`.
    var key: String
    var relay: String
    var station: String
}

// MARK: - Textes

/// Les textes des comptes (docs/link-ui.md, « Comptes KyberNow ») ; les refus
/// et les pastilles viennent du cœur.
enum AccountText {
    /// Accueil sans ordinateur.
    static var findTitle: String { String(localized: "Retrouvez vos ordinateurs") }
    static var findText: String {
        String(localized: "Avec un compte KyberNow, vos ordinateurs et ceux qu'on vous a ouverts arrivent ici, où que vous soyez.")
    }
    static var signIn: String { String(localized: "Se connecter à un compte KyberNow") }
    /// Aide sous le bouton : la fenêtre du système (iPhone, iPad, Mac).
    static var signInHelpWindow: String {
        String(localized: "La page du relais s'ouvre : mot de passe, passkey ou double authentification s'y font, jamais dans l'app. Pas encore de compte : « Créer un compte » sur cette page.")
    }
    /// Aide sous le bouton : le code (Apple TV).
    static var signInHelpCode: String { String(localized: "Un code s'affiche : vous l'approuvez depuis votre téléphone.") }
    /// Le séparateur, puis « Ajouter un ordinateur » et « Rejoindre avec un code ».
    static var withoutAccount: String { String(localized: "ou sans compte") }
    /// Pendant l'échange du retour de la fenêtre.
    static var signingIn: String { String(localized: "Connexion au compte…") }
    /// L'avis bref ({compte} : le nom, sinon l'e-mail).
    static func signedIn(_ account: String) -> String { String(localized: "Connecté à \(account).") }

    // Le code (Apple TV)
    static var stepScan: String { String(localized: "Scannez le QR code avec votre téléphone,") }
    static func stepOpen(_ address: String) -> String { String(localized: "ou ouvrez \(address) et entrez le code,") }
    static var stepConnect: String { String(localized: "puis touchez « Connecter ».") }
    static var qrLabel: String { String(localized: "QR code de connexion") }
    static func waiting(_ minutes: Int) -> String { String(localized: "En attente de votre téléphone… Le code change dans \(minutes) min.") }
    static func relay(_ relay: String) -> String { String(localized: "Relais : \(relay)") }
    static var newCode: String { String(localized: "Nouveau code") }

    // Un autre relais
    static var otherRelay: String { String(localized: "Un autre relais…") }
    static var otherRelayTitle: String { String(localized: "Un autre relais") }
    static var relayField: String { String(localized: "Relais") }
    /// L'exemple grisé du champ (pas traduit).
    static var relayPlaceholder: String { "relais.exemple.org:9443" }
    static var relayHelp: String { String(localized: "L'adresse du relais de votre compte, si ce n'est pas share.kybernow.app.") }
    static var continueLabel: String { String(localized: "Continuer") }

    // Réglages de l'app
    static var section: String { String(localized: "Comptes") }
    static var sectionHelp: String { String(localized: "Créer un compte, changer son mot de passe ou le supprimer : sur la page du compte.") }
    static var manage: String { String(localized: "Gérer le compte") }
    /// « Se déconnecter » d'un compte (« Sign out ») : une autre clé que celui d'une session (« Disconnect »).
    static var signOut: String { String(localized: "Se déconnecter du compte", defaultValue: "Se déconnecter") }
    static func signOutQuestion(_ account: String) -> String {
        String(localized: "Se déconnecter de \(account) ? Ses ordinateurs quittent cet appareil ; ceux que vous avez appairés restent.")
    }
    static var add: String { String(localized: "Ajouter un compte") }
    static var none: String { String(localized: "Aucun compte.") }

    /// Le texte d'un refus : celui du cœur, en phrase.
    static func text(_ error: Error) -> String {
        let text = (error as? AccountError)?.message ?? error.localizedDescription
        return ShareText.sentence(text)
    }
}

// MARK: - Trousseau

/// Les comptes dans le Trousseau de cet appareil : jamais synchronisés par
/// iCloud (la session d'un appareil ne vaut que pour lui).
enum AccountKeychain {
    private static let service = "fr.leslignesclaires.kybernow.accounts"

    private static func query(_ key: String) -> [String: Any] {
        [kSecClass as String: kSecClassGenericPassword, kSecAttrService as String: service, kSecAttrAccount as String: key,
         kSecAttrSynchronizable as String: false]
    }

    static func set(_ secret: String, for key: String) {
        SecItemDelete(query(key) as CFDictionary)
        var item = query(key)
        item[kSecValueData as String] = Data(secret.utf8)
        item[kSecAttrLabel as String] = "KyberNow"
        #if !os(macOS)
        // Ni sauvegarde vers un autre appareil : la session reste à celui-ci.
        item[kSecAttrAccessible as String] = kSecAttrAccessibleAfterFirstUnlockThisDeviceOnly
        #endif
        let status = SecItemAdd(item as CFDictionary, nil)
        if status != errSecSuccess { KyberLog.app.error("comptes : Trousseau, \(status, privacy: .public)") }
    }

    static func get(_ key: String) -> String? {
        var q = query(key)
        q[kSecReturnData as String] = true
        q[kSecMatchLimit as String] = kSecMatchLimitOne
        var out: CFTypeRef?
        guard SecItemCopyMatching(q as CFDictionary, &out) == errSecSuccess, let data = out as? Data else { return nil }
        return String(data: data, encoding: .utf8)
    }

    static func delete(_ key: String) {
        SecItemDelete(query(key) as CFDictionary)
    }
}

// MARK: - Les comptes

@MainActor
final class AccountStore: ObservableObject {
    /// Les comptes connectés, dans l'ordre de leur connexion.
    @Published private(set) var accounts: [KyberAccount] = []
    /// La dernière liste d'ordinateurs de chaque compte (`KyberAccount.key`).
    @Published private(set) var computers: [String: [AccountComputer]] = [:]
    /// La pastille d'origine de chaque ordinateur de l'accueil, quand un
    /// compte est connecté (« Compte », « Appairé »…) ; vide sans compte.
    @Published private(set) var origins: [UUID: String] = [:]
    /// L'échange du retour de la fenêtre : « Connexion au compte… ».
    @Published private(set) var signingIn = false
    /// Un avis bref (« Connecté à {compte}. », un refus du cœur) : l'app le montre.
    var onNotice: ((String) -> Void)?

    private unowned let hosts: HostStore
    private let defaults = UserDefaults.standard
    /// Les appairés qu'un compte donne aussi : leur pastille est celle du compte.
    private var linked: [UUID: AccountComputer] = [:]
    /// Les ordinateurs du compte vus sur le réseau (Bonjour, même empreinte d'Admin) : leur adresse.
    private var seen: [UUID: Candidate] = [:]
    private var resolving: Set<String> = []
    private var lastRefresh = Date.distantPast
    private var refreshing = false
    /// L'info d'un ordinateur du compte hors du réseau, demandée par le relais (2 min au plus).
    private var infoAsked: [UUID: Date] = [:]
    private var bag: Set<AnyCancellable> = []
    #if os(iOS) || os(macOS)
    private var web: AccountWebSignIn?
    #endif

    private static let keysKey = "accounts"
    private static let computersKey = "accountComputers"

    init(hosts: HostStore) {
        self.hosts = hosts
        hosts.accounts = self
        // Démonstration : ni Trousseau ni relais.
        guard !hosts.demo else { return }
        load()
        sync()
        // Un ordinateur appairé, ajouté, retiré ou dont l'empreinte change : l'accueil est refait.
        hosts.$hosts
            .map { $0.filter { $0.account == nil }.map { "\($0.id)|\($0.consoleFingerprint ?? "")|\($0.relay?.id ?? "")" } }
            .removeDuplicates()
            .receive(on: DispatchQueue.main)
            .sink { [weak self] _ in self?.sync() }
            .store(in: &bag)
        // Au retour au premier plan : la liste de chaque compte relue.
        #if os(macOS)
        let active = NSApplication.didBecomeActiveNotification
        #else
        let active = UIApplication.didBecomeActiveNotification
        #endif
        NotificationCenter.default.publisher(for: active)
            .sink { [weak self] _ in Task { await self?.refreshAll(force: true) } }
            .store(in: &bag)
        Task { await refreshAll(force: true) }
    }

    /// Des comptes sur plusieurs relais : « Compte · {relais} ».
    var severalRelays: Bool { Set(accounts.map(\.relay)).count > 1 }

    func account(_ key: String) -> KyberAccount? {
        accounts.first { $0.key == key }
    }

    /// L'ordinateur du compte de cet ordinateur de l'accueil (le sien, ou
    /// celui d'un appairé qu'un compte donne aussi).
    func computer(for host: Host) -> AccountComputer? {
        if let link = host.account {
            return accounts.lazy.flatMap { self.computers[$0.key] ?? [] }.first { $0.relay == link.relay && $0.station == link.station }
        }
        return linked[host.id]
    }

    // MARK: Garder

    private func load() {
        let keys = defaults.stringArray(forKey: Self.keysKey) ?? []
        accounts = keys.compactMap { AccountKeychain.get($0).flatMap(KyberAccount.init(json:)) }
        if let data = defaults.data(forKey: Self.computersKey), let list = try? JSONDecoder().decode([AccountComputer].self, from: data) {
            computers = Dictionary(grouping: list.filter { c in accounts.contains { $0.key == c.account } }, by: \.account)
        }
    }

    private func saveKeys() {
        defaults.set(accounts.map(\.key), forKey: Self.keysKey)
    }

    private func saveComputers() {
        let list = accounts.flatMap { computers[$0.key] ?? [] }
        if let data = try? JSONEncoder().encode(list) { defaults.set(data, forKey: Self.computersKey) }
    }

    /// Un compte connecté (fenêtre du système, code de l'Apple TV) : gardé
    /// dans le Trousseau, ses ordinateurs demandés tout de suite.
    func add(_ account: KyberAccount) {
        AccountKeychain.set(account.secret, for: account.key)
        if let i = accounts.firstIndex(where: { $0.key == account.key }) {
            accounts[i] = account
        } else {
            accounts.append(account)
        }
        saveKeys()
        KyberLog.app.notice("comptes : connecté sur \(account.relayLabel, privacy: .public)")
        onNotice?(AccountText.signedIn(account.displayName))
        Task { await refresh(account) }
    }

    /// Le compte oublié sur cet appareil (Trousseau compris) ; ses ordinateurs
    /// quittent l'accueil, les appairés restent.
    func forget(_ account: KyberAccount) {
        AccountKeychain.delete(account.key)
        accounts.removeAll { $0.key == account.key }
        computers[account.key] = nil
        saveKeys()
        saveComputers()
        sync()
    }

    /// « Se déconnecter » (confirmé) : la session de l'appareil est fermée
    /// chez le relais, et le compte oublié même si le relais ne répond pas.
    func signOut(_ account: KyberAccount) {
        forget(account)
        Task {
            do {
                try await Accounts.signOut(account)
            } catch {
                KyberLog.app.notice("comptes : déconnexion sans réponse du relais (\(error.localizedDescription, privacy: .public))")
            }
        }
    }

    // MARK: Les ordinateurs du compte

    /// La liste de chaque compte, relue (une fois par minute au plus, tout de suite avec `force`).
    func refreshAll(force: Bool = false) async {
        guard !hosts.demo, !refreshing, !accounts.isEmpty, force || Date().timeIntervalSince(lastRefresh) > 60 else { return }
        refreshing = true
        lastRefresh = Date()
        for account in accounts { await refresh(account) }
        refreshing = false
    }

    func refresh(_ account: KyberAccount) async {
        do {
            let fresh = try await Accounts.computers(account)
            // Déconnecté entre-temps : rien à garder.
            guard self.account(account.key) != nil else { return }
            computers[account.key] = Accounts.rememberFingerprints(fresh: fresh, known: computers[account.key] ?? [])
            saveComputers()
            sync()
        } catch let e as AccountError where e.failure == .signedOut {
            // La session de l'appareil est fermée (page du compte, nouveau mot de passe).
            guard self.account(account.key) != nil else { return }
            forget(account)
            onNotice?(AccountText.text(e))
        } catch {
            // Relais muet : la dernière liste reste.
            KyberLog.app.notice("comptes : liste des ordinateurs non relue (\(error.localizedDescription, privacy: .public))")
        }
    }

    /// L'accueil refait : un ordinateur une fois (les appairés restent, avec la
    /// pastille du compte qui les a aussi), les ordinateurs des comptes
    /// ajoutés, mis à jour ou retirés, et les pastilles.
    func sync() {
        guard !hosts.demo else { return }
        let all = accounts.flatMap { computers[$0.key] ?? [] }
        let paired = hosts.hosts.filter { $0.account == nil }
        let merged = Accounts.merge(paired: paired.map { AccountPaired(id: $0.id.uuidString, fingerprint: $0.consoleFingerprint, station: $0.relay?.id) },
                                    computers: all)
        linked = Dictionary(merged.linked.compactMap { id, c in UUID(uuidString: id).map { ($0, c) } }, uniquingKeysWith: { a, _ in a })
        var list: [Host] = []
        for c in merged.added {
            var h = hosts.hosts.first { $0.account.map { $0.relay == c.relay && $0.station == c.station } ?? false }
                ?? Host(name: c.name, address: "", consolePort: 9443, username: "")
            if !c.name.isEmpty { h.name = c.name }
            h.account = AccountLink(key: c.account, relay: c.relay, station: c.station)
            if let fp = c.adminFingerprint { h.consoleFingerprint = fp }
            if let candidate = seen[h.id] {
                h.address = candidate.address
                h.consolePort = candidate.port
            }
            list.append(h)
        }
        hosts.setAccountHosts(list)
        // Les pastilles : seulement quand un compte est connecté.
        var next: [UUID: String] = [:]
        if !accounts.isEmpty {
            let several = severalRelays
            let pairedBadge = Accounts.originPaired
            for h in hosts.hosts {
                if let c = computer(for: h) {
                    next[h.id] = Accounts.origin(c, severalRelays: several)
                } else {
                    next[h.id] = pairedBadge
                }
            }
        }
        if next != origins { origins = next }
    }

    // MARK: Sur le réseau ou par le relais

    /// Vu sur le réseau (Bonjour, même empreinte d'Admin) : la console et la
    /// session y passent d'abord.
    func isOnNetwork(_ host: Host) -> Bool { seen[host.id] != nil }

    /// Les annonces Bonjour du moment (la découverte de l'app) : l'adresse
    /// des ordinateurs du compte qui y sont, avec la même empreinte d'Admin.
    func noteNetwork(_ found: [Discovery.Found]) {
        guard !hosts.demo else { return }
        let mine = hosts.hosts.filter { $0.account != nil }
        var still: Set<UUID> = []
        for h in mine {
            guard let fp = h.consoleFingerprint,
                  let f = found.first(where: { $0.fingerprint.map { HostStore.sameFingerprint($0, fp) } ?? false }) else { continue }
            still.insert(h.id)
            guard seen[h.id] == nil, !resolving.contains(f.id) else { continue }
            resolving.insert(f.id)
            Task {
                let candidate = await Discovery.resolve(f)
                resolving.remove(f.id)
                guard let candidate else { return }
                seen[h.id] = candidate
                sync()
                if let host = hosts.host(h.id) { await hosts.refresh(host) }
            }
        }
        let gone = Set(seen.keys).subtracting(still)
        for id in gone { seen[id] = nil }
    }

    /// La console d'un ordinateur du compte : sur le réseau s'il y est vu,
    /// sinon (ou `viaRelay`) par le relais ; nil sans empreinte d'Admin connue
    /// hors du réseau (jamais de premier contact par le relais).
    func console(for host: Host, viaRelay: Bool) -> ConsoleClient? {
        guard let c = computer(for: host), let account = account(c.account),
              let access = Accounts.access(account, station: c.station) else { return nil }
        let fp = c.adminFingerprint ?? host.consoleFingerprint
        if !viaRelay, let candidate = seen[host.id] {
            return ConsoleClient(access: access, host: candidate.address, port: candidate.port, fingerprint: fp, relay: nil)
        }
        guard Relay.mode != .off, let route = Accounts.route(c) else { return nil }
        return ConsoleClient(access: access, host: "", port: 0, fingerprint: fp, relay: route)
    }

    /// L'état d'un ordinateur du compte hors du réseau : « Hors de chez vous »
    /// s'il est connecté au relais (liste du compte) et que l'app connaît son
    /// empreinte d'Admin, sinon « Hors ligne ». Son info (système, écrans) est
    /// demandée par le relais quand l'app ne l'a pas encore.
    func awayStatus(_ host: Host, previous: HostStatus?) async -> HostStatus {
        Task { await refreshAll() }
        var info = previous?.info
        guard let c = computer(for: host), c.online, Relay.mode != .off, Accounts.route(c) != nil else {
            return HostStatus(online: false, info: info)
        }
        if info == nil, Date().timeIntervalSince(infoAsked[host.id] ?? .distantPast) > 120, let console = console(for: host, viaRelay: true) {
            infoAsked[host.id] = Date()
            do {
                info = try await console.info()
            } catch let ConsoleError.account(failure, message) {
                refused(host, failure: failure, message: message)
                return HostStatus(online: false, info: nil)
            } catch {
                KyberLog.app.notice("comptes : info de \(host.name, privacy: .public) par le relais (\(error.localizedDescription, privacy: .public))")
            }
        }
        return HostStatus(online: true, info: info, away: true)
    }

    /// Un refus du cœur pour un ordinateur du compte : la session de
    /// l'appareil fermée (le compte est oublié), ou l'ordinateur qui n'est
    /// plus au compte (il quitte l'accueil) ; le reste se dit à l'ouverture.
    func refused(_ host: Host, failure: AccountFailure, message: String) {
        guard let c = computer(for: host), let account = account(c.account) else { return }
        switch failure {
        case .signedOut:
            forget(account)
            onNotice?(ShareText.sentence(message))
        case .unknownComputer:
            computers[c.account]?.removeAll { $0.same(c) }
            saveComputers()
            sync()
            onNotice?(ShareText.sentence(message))
        default:
            break
        }
    }

    /// La session d'un ordinateur du compte : le compte et la Station, le
    /// réseau d'abord s'il y est vu, sinon le relais.
    func session(for host: Host) -> SessionEngine.AccountSession? {
        guard host.account != nil, let c = computer(for: host), let account = account(c.account),
              let access = Accounts.access(account, station: c.station) else { return nil }
        return SessionEngine.AccountSession(access: access, relay: Relay.mode == .off ? nil : Accounts.route(c), onNetwork: isOnNetwork(host))
    }

    // MARK: Se connecter (iPhone, iPad, Mac)

    /// Le nom de cet appareil pour le relais (« iPad de Léa ») : montré sur
    /// la page de connexion et dans « Appareils connectés ».
    static var deviceName: String {
        ConsoleClient.thisDevice["name"] as? String ?? "KyberNow Link"
    }

    #if os(iOS) || os(macOS)
    /// « Se connecter à un compte KyberNow » : la page du relais dans la
    /// fenêtre du système (passkeys et session du navigateur gardées) ; son
    /// retour échangé contre le compte (« Connexion au compte… »). « Annuler »
    /// dans la page, fenêtre fermée : rien. `relay` vide : share.kybernow.app.
    func signIn(relay: String = "") {
        guard !hosts.demo, !signingIn, web == nil else { return }
        let request: AccountSignIn
        do {
            request = try Accounts.signIn(relay: relay, device: Self.deviceName)
        } catch {
            onNotice?(AccountText.text(error))
            return
        }
        let web = AccountWebSignIn()
        self.web = web
        web.start(request) { [weak self] callback in
            guard let self else { return }
            self.web = nil
            guard let callback else { return }
            self.finish(request, callback: callback)
        }
    }

    private func finish(_ request: AccountSignIn, callback: URL) {
        signingIn = true
        Task {
            do {
                let account = try await Accounts.finishSignIn(request, callback: callback.absoluteString)
                signingIn = false
                add(account)
            } catch let e as AccountError where e.failure == .cancelled {
                signingIn = false
            } catch {
                signingIn = false
                onNotice?(AccountText.text(error))
            }
        }
    }
    #endif
}

extension HostStore {
    /// La session d'un ordinateur d'un compte (`SessionEngine.Params.account`) ; nil pour les autres.
    func sessionAccount(for host: Host) -> SessionEngine.AccountSession? {
        host.account == nil ? nil : accounts?.session(for: host)
    }
}

// MARK: - Fenêtre du système (iPhone, iPad, Mac)

#if os(iOS) || os(macOS)
/// La page de connexion du relais dans `ASWebAuthenticationSession` : schéma
/// de retour `kybernow`, session du navigateur gardée (passkeys, « rester
/// connecté »). Le retour arrive ici, pas par `onOpenURL`.
final class AccountWebSignIn: NSObject, ASWebAuthenticationPresentationContextProviding {
    private var session: ASWebAuthenticationSession?

    /// `done` reçoit le retour, ou nil (« Annuler », fenêtre fermée, échec).
    @MainActor
    func start(_ request: AccountSignIn, done: @escaping @MainActor (URL?) -> Void) {
        let session = ASWebAuthenticationSession(url: request.url, callbackURLScheme: request.callbackScheme) { url, error in
            Task { @MainActor in done(error == nil ? url : nil) }
        }
        session.prefersEphemeralWebBrowserSession = false
        session.presentationContextProvider = self
        self.session = session
        if !session.start() { done(nil) }
    }

    func presentationAnchor(for session: ASWebAuthenticationSession) -> ASPresentationAnchor {
        MainActor.assumeIsolated { Self.anchor() }
    }

    @MainActor
    private static func anchor() -> ASPresentationAnchor {
        #if os(macOS)
        return NSApp.keyWindow ?? NSApp.windows.first { $0.isVisible } ?? ASPresentationAnchor()
        #else
        let scenes = UIApplication.shared.connectedScenes.compactMap { $0 as? UIWindowScene }
        return scenes.flatMap(\.windows).first { $0.isKeyWindow } ?? scenes.first?.windows.first ?? ASPresentationAnchor()
        #endif
    }
}
#endif

// MARK: - Un autre relais

/// « Ajouter un compte » : le relais d'abord. `share.kybernow.app` proposé ;
/// « Un autre relais… » ouvre un champ, lu par le cœur avant de continuer
/// (« Adresse du relais illisible. »).
@MainActor
final class AccountRelayModel: ObservableObject {
    @Published var text = ""
    /// Le refus, sous le champ.
    @Published private(set) var failure: String?

    var canContinue: Bool { !text.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty }

    func changed() {
        failure = nil
    }

    /// La forme normale du relais tapé, ou nil (le refus est sous le champ).
    func validate() -> String? {
        do {
            return try Accounts.normalizeRelay(text.trimmingCharacters(in: .whitespacesAndNewlines))
        } catch {
            failure = AccountText.text(error)
            return nil
        }
    }
}

// MARK: - Le code (Apple TV)

/// L'écran du code : un code et son QR code, une demande toutes les
/// `interval` secondes jusqu'à l'approbation sur le téléphone ; échu, un
/// nouveau code s'affiche de lui-même ; refusé, « Nouveau code ». `run()` vit
/// le temps de l'écran (`.task`) : quitter l'écran l'annule.
@MainActor
final class AccountCodeModel: ObservableObject {
    enum State: Equatable {
        case loading
        case showing(AccountTVCode)
        /// « Connexion refusée sur le téléphone. » (texte du cœur), et « Nouveau code ».
        case denied(String)
        /// Le relais refuse (sans comptes, muet, adresse illisible) : son texte.
        case failed(String)
    }

    @Published private(set) var state: State = .loading
    @Published private(set) var minutesLeft = 10
    /// Change à chaque « Nouveau code » : l'écran relance `run()`.
    @Published private(set) var attempt = 0
    /// Le relais du code ; vide : share.kybernow.app.
    @Published private(set) var relay: String
    let accounts: AccountStore
    /// Appelé une fois, le compte connecté.
    var onDone: (() -> Void)?

    init(accounts: AccountStore, relay: String = "") {
        self.accounts = accounts
        self.relay = relay
    }

    /// Le relais à montrer : « Relais : {relais} ».
    var relayLabel: String {
        if case let .showing(code) = state, !code.urlLabel.isEmpty {
            return code.urlLabel.split(separator: "/").first.map(String.init) ?? code.urlLabel
        }
        return relay.isEmpty ? Accounts.defaultRelay : relay.split(separator: "#").first.map(String.init) ?? relay
    }

    /// « Nouveau code », ou un autre relais choisi.
    func restart(relay: String? = nil) {
        if let relay { self.relay = relay }
        state = .loading
        attempt += 1
    }

    func run() async {
        while !Task.isCancelled {
            let code: AccountTVCode
            do {
                code = try await Accounts.tvCode(relay: relay, device: AccountStore.deviceName)
            } catch {
                guard !Task.isCancelled else { return }
                state = .failed(AccountText.text(error))
                return
            }
            guard !Task.isCancelled else { return }
            state = .showing(code)
            minutesLeft = max(1, code.minutesLeft)
            // Les demandes, jusqu'à l'échéance ; échu : un nouveau code, de lui-même.
            guard await poll(code) == true else { return }
        }
    }

    /// Demande toutes les `interval` secondes : nil (fini : connecté, refusé,
    /// écran quitté), `true` pour un nouveau code (échu).
    private func poll(_ code: AccountTVCode) async -> Bool? {
        var wait = code.interval
        while !Task.isCancelled {
            try? await Task.sleep(for: .seconds(wait))
            guard !Task.isCancelled else { return nil }
            wait = code.interval
            minutesLeft = code.minutesLeft
            if minutesLeft == 0 { return true }
            do {
                if let account = try await Accounts.tvPoll(code) {
                    guard !Task.isCancelled else { return nil }
                    accounts.add(account)
                    onDone?()
                    return nil
                }
            } catch let e as AccountError {
                guard !Task.isCancelled else { return nil }
                switch e.failure {
                case .expired: return true
                case .denied:
                    state = .denied(AccountText.text(e))
                    return nil
                case .tooMany: wait = max(30, code.interval)
                case .unreachable: break
                default:
                    state = .failed(AccountText.text(e))
                    return nil
                }
            } catch {
                return nil
            }
        }
        return nil
    }
}
