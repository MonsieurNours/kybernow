// Consoles PlayStation dans iCloud, dans les deux sens (docs/playstation.md,
// « Plusieurs appareils ») : commun au Mac, à l'Apple TV, à l'iPhone et à
// l'iPad (ni AppKit ni UIKit). Format et règles : PlayStationSync.swift.
//
// L'app crée un `PlayStationCloud` pour son `PlayStationStore` quand elle a
// les droits iCloud, lui dit si « Synchroniser avec iCloud » est allumé
// (`enabled`), appelle `schedule()` quand elle revient au premier plan et
// `forget()` quand la synchro est coupée ou que le compte iCloud change.
// Il se relance seul : à chaque changement des comptes ou des associations
// (`PlayStationStore.onChange`), à chaque changement de la clé venu
// d'iCloud, et une minute après une synchro qui attendait le Trousseau.

import Foundation
import KyberKit
import Security

@MainActor
final class PlayStationCloud {
    /// Clé du magasin clé-valeur (`Data`, JSON de `PSShared`, sans secret).
    static let key = "playstation"
    /// Service des éléments synchronisables du Trousseau iCloud
    /// (`account.<UUID>`, `<HOSTID>.<UUID>`).
    static let service = "fr.leslignesclaires.kybernow.cloud.playstation"
    /// `known` de `PSCloudPlan`, dans les préférences.
    private static let knownKey = "icloudPlayStationKnown"

    private let store: PlayStationStore
    private let kv = NSUbiquitousKeyValueStore.default
    /// « Synchroniser avec iCloud » allumé (et les droits iCloud là).
    private let enabled: () -> Bool
    /// Un élément du Trousseau iCloud refusé (réessayé à la synchro suivante).
    var onKeychainError: ((OSStatus) -> Void)?
    private var scheduled = false
    private var retry: Task<Void, Never>?
    private var observer: NSObjectProtocol?

    init(store: PlayStationStore, enabled: @escaping () -> Bool) {
        self.store = store
        self.enabled = enabled
        store.onChange = { [weak self] in self?.schedule() }
        observer = NotificationCenter.default.addObserver(forName: NSUbiquitousKeyValueStore.didChangeExternallyNotification, object: kv, queue: .main) { [weak self] note in
            let reason = note.userInfo?[NSUbiquitousKeyValueStoreChangeReasonKey] as? Int
            let keys = note.userInfo?[NSUbiquitousKeyValueStoreChangedKeysKey] as? [String] ?? []
            Task { @MainActor in
                guard let self else { return }
                // Un autre compte iCloud : rien de ce qu'il n'a pas n'est « retiré ailleurs ».
                if reason == NSUbiquitousKeyValueStoreAccountChange { self.forget() }
                if reason == NSUbiquitousKeyValueStoreAccountChange || keys.contains(Self.key) { self.schedule() }
            }
        }
        kv.synchronize()
    }

    /// Plusieurs changements de suite : une seule synchro, une demi-seconde après.
    func schedule() {
        guard !scheduled else { return }
        scheduled = true
        Task { @MainActor in
            try? await Task.sleep(for: .milliseconds(500))
            scheduled = false
            sync()
        }
    }

    /// Synchro coupée ou compte iCloud changé : la suivante repartira d'une
    /// union (rien n'est retiré, ni ici ni dans iCloud).
    func forget() {
        known = []
        retry?.cancel()
        retry = nil
    }

    func sync() {
        guard enabled() else { return }
        let cloud = kv.data(forKey: Self.key).flatMap(PSShared.decode) ?? PSShared()
        let plan = PSCloudPlan.make(local: store.cloudLocal, cloud: cloud, known: known) { Self.get($0) }
        store.apply(plan)
        for k in plan.deleteSecrets { Self.delete(k) }
        var failure: OSStatus?
        for (k, v) in plan.writeSecrets {
            let status = Self.set(v, for: k)
            if status != errSecSuccess { failure = status }
        }
        if let failure {
            // L'élément manquant n'efface rien ailleurs (règle 1) : réessayé à la synchro suivante.
            KyberLog.app.error("Trousseau iCloud : \(failure, privacy: .public) pour une console PlayStation")
            onKeychainError?(failure)
        }
        if plan.cloud != cloud, let data = plan.cloud.data {
            kv.set(data, forKey: Self.key)
            kv.synchronize()
        }
        known = plan.known
        retry?.cancel()
        retry = plan.pending ? Task { @MainActor [weak self] in
            try? await Task.sleep(for: .seconds(60))
            guard !Task.isCancelled else { return }
            self?.sync()
        } : nil
    }

    private var known: Set<String> {
        get { Set(UserDefaults.standard.stringArray(forKey: Self.knownKey) ?? []) }
        set { UserDefaults.standard.set(newValue.sorted(), forKey: Self.knownKey) }
    }

    // MARK: Trousseau iCloud

    private static func query(_ account: String) -> [String: Any] {
        [kSecClass as String: kSecClassGenericPassword,
         kSecAttrService as String: service,
         kSecAttrAccount as String: account,
         kSecAttrSynchronizable as String: true,
         kSecUseDataProtectionKeychain as String: true]
    }

    static func get(_ account: String) -> String? {
        var q = query(account)
        q[kSecReturnData as String] = true
        q[kSecMatchLimit as String] = kSecMatchLimitOne
        var out: CFTypeRef?
        guard SecItemCopyMatching(q as CFDictionary, &out) == errSecSuccess, let data = out as? Data else { return nil }
        return String(data: data, encoding: .utf8)
    }

    /// Écrit seulement si la valeur a changé.
    static func set(_ value: String, for account: String) -> OSStatus {
        if let current = get(account) {
            if current == value { return errSecSuccess }
            return SecItemUpdate(query(account) as CFDictionary, [kSecValueData as String: Data(value.utf8)] as CFDictionary)
        }
        var item = query(account)
        item[kSecValueData as String] = Data(value.utf8)
        item[kSecAttrLabel as String] = "KyberNow (iCloud)"
        // Synchronisable : pas d'accès réservé à cet appareil.
        item[kSecAttrAccessible as String] = kSecAttrAccessibleAfterFirstUnlock
        return SecItemAdd(item as CFDictionary, nil)
    }

    static func delete(_ account: String) {
        SecItemDelete(query(account) as CFDictionary)
    }
}
