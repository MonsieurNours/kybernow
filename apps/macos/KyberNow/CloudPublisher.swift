// Ordinateurs publiés dans iCloud : l'Apple TV, l'iPhone et l'iPad connectés
// au même compte les récupèrent avec leurs accès, sans rien taper (CloudSync
// de l'app Apple TV, repris par l'app iPhone et iPad).
//
// La liste va dans le magasin clé-valeur d'iCloud (clé « hosts », tableau JSON
// de CloudHost), les mots de passe dans le Trousseau iCloud (éléments
// synchronisables, service « fr.leslignesclaires.kybernow.cloud », compte =
// identifiant de l'ordinateur). L'élément local du Trousseau reste la
// référence sur ce Mac ; la copie iCloud le suit.
//
// Il faut les droits iCloud (magasin clé-valeur, groupe du Trousseau), donc un
// profil de provisionnement : une app signée sans eux ne publie rien et le dit
// dans ses réglages.

import Foundation
import KyberKit
import Security
import SystemConfiguration

/// Format partagé avec l'app Apple TV (apps/tvos/KyberNowTV/CloudSync.swift).
struct CloudHost: Codable, Equatable {
    var id: UUID
    var name: String
    var address: String
    var consolePort: UInt16
    var username: String
    var streamPort: UInt16?
    var consoleFingerprint: String?
    /// Appareil qui l'a publié (« Mac de Léa »).
    var from: String?

    init(_ h: Host, from: String) {
        id = h.id
        name = h.name
        address = h.address
        consolePort = h.consolePort
        username = h.username
        streamPort = h.streamPort
        consoleFingerprint = h.consoleFingerprint
        self.from = from
    }
}

@MainActor
final class CloudPublisher: ObservableObject {
    enum State: Equatable {
        case off
        /// Version signée sans les droits iCloud.
        case unavailable
        case published(Int)
        case failed(String)
    }

    @Published private(set) var state: State = .off
    /// « Synchroniser avec iCloud », actif par défaut.
    @Published var enabled: Bool {
        didSet {
            guard enabled != oldValue else { return }
            UserDefaults.standard.set(enabled, forKey: Self.enabledKey)
            enabled ? publish(hosts) : withdraw()
        }
    }
    /// Aucun compte iCloud sur ce Mac : la liste attend localement.
    @Published private(set) var signedOut = FileManager.default.ubiquityIdentityToken == nil

    private static let enabledKey = "icloudSync"
    /// Ordinateurs que ce Mac a publiés : les autres, venus d'ailleurs, restent.
    private static let publishedKey = "icloudPublished"
    private static let listKey = "hosts"
    private let store = NSUbiquitousKeyValueStore.default
    private var hosts: [Host] = []
    /// Mots de passe déjà recopiés dans le Trousseau iCloud depuis le lancement.
    private var copied: Set<UUID> = []
    private var observers: [NSObjectProtocol] = []

    /// Les deux droits sont là (profil de provisionnement avec iCloud).
    static let entitled: Bool = {
        guard let task = SecTaskCreateFromSelf(nil) else { return false }
        return ["com.apple.developer.ubiquity-kvstore-identifier", "keychain-access-groups"].allSatisfy {
            SecTaskCopyValueForEntitlement(task, $0 as CFString, nil) != nil
        }
    }()

    init() {
        UserDefaults.standard.register(defaults: [Self.enabledKey: true])
        enabled = UserDefaults.standard.bool(forKey: Self.enabledKey)
        guard Self.entitled else { return }
        let center = NotificationCenter.default
        // Liste réécrite ailleurs (un autre Mac, un changement de compte) : on
        // y remet ce Mac si besoin.
        observers.append(center.addObserver(forName: NSUbiquitousKeyValueStore.didChangeExternallyNotification, object: store, queue: .main) { [weak self] note in
            let reason = note.userInfo?[NSUbiquitousKeyValueStoreChangeReasonKey] as? Int
            Task { @MainActor in
                guard let self else { return }
                if reason == NSUbiquitousKeyValueStoreAccountChange { self.copied = [] }
                if self.enabled { self.publish(self.hosts) }
            }
        })
        observers.append(center.addObserver(forName: .NSUbiquityIdentityDidChange, object: nil, queue: .main) { [weak self] _ in
            Task { @MainActor in
                guard let self else { return }
                self.signedOut = FileManager.default.ubiquityIdentityToken == nil
                self.copied = []
                if self.enabled { self.publish(self.hosts) }
            }
        })
        store.synchronize()
    }

    /// Nom de ce Mac, montré sur l'Apple TV (« Depuis MacBook Pro de Léa »).
    private var deviceName: String {
        (SCDynamicStoreCopyComputerName(nil, nil) as String?) ?? "Mac"
    }

    /// Après chaque changement des ordinateurs (HostStore.didSave), et au lancement.
    func publish(_ hosts: [Host]) {
        self.hosts = hosts
        guard enabled else { state = .off; return }
        guard Self.entitled else { state = .unavailable; return }

        let mine = Set(hosts.map(\.id))
        let before = published
        // Retirés de ce Mac : plus dans la liste ni dans le Trousseau iCloud.
        for id in before.subtracting(mine) {
            CloudKeychain.delete(id)
            copied.remove(id)
        }
        // Un mot de passe refusé n'empêche pas la liste de partir : réessayé au
        // prochain changement.
        var failure: OSStatus?
        for host in hosts where !copied.contains(host.id) {
            guard let password = Keychain.get(host.id) else { continue }
            let status = CloudKeychain.set(password, for: host.id)
            if status == errSecSuccess {
                copied.insert(host.id)
            } else {
                KyberLog.app.error("Trousseau iCloud : \(status, privacy: .public) pour \(host.name, privacy: .public)")
                failure = status
            }
        }

        let from = deviceName
        let others = cloudList.filter { !mine.contains($0.id) && !before.contains($0.id) }
        write(others + hosts.map { CloudHost($0, from: from) })
        published = mine
        state = failure.map { .failed(CloudKeychain.message($0)) } ?? .published(hosts.count)
    }

    /// Synchro coupée : ce Mac retire ses ordinateurs et leurs mots de passe
    /// d'iCloud (l'Apple TV garde ceux qu'elle a déjà ajoutés).
    private func withdraw() {
        state = .off
        guard Self.entitled else { return }
        let mine = published
        for id in mine { CloudKeychain.delete(id) }
        write(cloudList.filter { !mine.contains($0.id) })
        published = []
        copied = []
    }

    private var cloudList: [CloudHost] {
        guard let data = store.data(forKey: Self.listKey) else { return [] }
        return (try? JSONDecoder().decode([CloudHost].self, from: data)) ?? []
    }

    private func write(_ list: [CloudHost]) {
        guard list != cloudList, let data = try? JSONEncoder().encode(list) else { return }
        store.set(data, forKey: Self.listKey)
        store.synchronize()
    }

    private var published: Set<UUID> {
        get { Set((UserDefaults.standard.stringArray(forKey: Self.publishedKey) ?? []).compactMap(UUID.init)) }
        set { UserDefaults.standard.set(newValue.map(\.uuidString), forKey: Self.publishedKey) }
    }
}

/// Mots de passe dans le Trousseau iCloud (Trousseau à protection des données,
/// éléments synchronisables), lus par l'Apple TV.
enum CloudKeychain {
    private static let service = "fr.leslignesclaires.kybernow.cloud"

    private static func query(_ id: UUID) -> [String: Any] {
        [kSecClass as String: kSecClassGenericPassword,
         kSecAttrService as String: service,
         kSecAttrAccount as String: id.uuidString,
         kSecAttrSynchronizable as String: true,
         kSecUseDataProtectionKeychain as String: true]
    }

    static func get(_ id: UUID) -> String? {
        var q = query(id)
        q[kSecReturnData as String] = true
        q[kSecMatchLimit as String] = kSecMatchLimitOne
        var out: CFTypeRef?
        guard SecItemCopyMatching(q as CFDictionary, &out) == errSecSuccess, let data = out as? Data else { return nil }
        return String(data: data, encoding: .utf8)
    }

    /// Écrit seulement si le mot de passe a changé.
    static func set(_ password: String, for id: UUID) -> OSStatus {
        if let current = get(id) {
            if current == password { return errSecSuccess }
            return SecItemUpdate(query(id) as CFDictionary, [kSecValueData as String: Data(password.utf8)] as CFDictionary)
        }
        var item = query(id)
        item[kSecValueData as String] = Data(password.utf8)
        item[kSecAttrLabel as String] = "KyberNow (iCloud)"
        // Synchronisable : pas d'accès réservé à cet appareil.
        item[kSecAttrAccessible as String] = kSecAttrAccessibleAfterFirstUnlock
        return SecItemAdd(item as CFDictionary, nil)
    }

    static func delete(_ id: UUID) {
        SecItemDelete(query(id) as CFDictionary)
    }

    static func message(_ status: OSStatus) -> String {
        switch status {
        case errSecMissingEntitlement: return String(localized: "Trousseau iCloud refusé : droit keychain-access-groups absent du profil.")
        case errSecInteractionNotAllowed: return String(localized: "Trousseau verrouillé : les mots de passe partiront à son ouverture.")
        default: return String(localized: "Trousseau iCloud : erreur \(status).")
        }
    }
}
