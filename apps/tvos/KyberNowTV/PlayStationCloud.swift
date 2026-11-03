// Consoles PlayStation par iCloud sur l'iPhone, l'iPad et l'Apple TV
// (docs/playstation.md, « Plusieurs appareils ») : dans les deux sens, comme
// le Mac (CloudPublisher.swift). La liste (sans secret) dans le magasin
// clé-valeur, clé « playstation » ; l'identifiant de chaque compte et chaque
// association dans le Trousseau iCloud (éléments synchronisables, service
// « fr.leslignesclaires.kybernow.cloud.playstation », comptes
// `account.<UUID>` et `<HOSTID>.<UUID>`). Format et règles communs à tous les
// appareils Apple : PlayStationSync.swift (`PSShared`, `PSCloudPlan`).
//
// Seulement avec « Synchroniser avec iCloud » allumé (`enabled`) ; une
// synchro à chaque changement des consoles d'ici, à chaque changement venu
// d'iCloud et au retour de l'app au premier plan (le Trousseau iCloud arrive
// parfois après la liste ; des éléments en attente : un nouvel essai une
// minute après). Un changement de compte iCloud, ou la synchro coupée,
// efface le souvenir de la dernière synchro sans rien retirer.

import Foundation
import KyberKit
import Security
import UIKit

@MainActor
final class PlayStationCloud {
    private static let key = "playstation"
    /// `known` de la synchro (PSCloudPlan), propre à cet appareil.
    private static let knownKey = "icloudPlayStationKnown"
    private let kv = NSUbiquitousKeyValueStore.default
    private weak var playstation: PlayStationStore?
    private var observers: [NSObjectProtocol] = []
    private var scheduled = false
    private var retry: Task<Void, Never>?

    /// « Synchroniser avec iCloud » : allumée, une synchro tout de suite ;
    /// coupée, le souvenir de la dernière est oublié (rien n'est retiré).
    var enabled: Bool {
        didSet {
            guard enabled != oldValue else { return }
            if enabled { sync() } else { known = [] }
        }
    }

    private var known: Set<String> {
        get { Set(UserDefaults.standard.stringArray(forKey: Self.knownKey) ?? []) }
        set { UserDefaults.standard.set(newValue.sorted(), forKey: Self.knownKey) }
    }

    init(enabled: Bool) {
        self.enabled = enabled
    }

    func attach(_ store: PlayStationStore) {
        playstation = store
        store.onChange = { [weak self] in self?.schedule() }
        let center = NotificationCenter.default
        observers.append(center.addObserver(forName: NSUbiquitousKeyValueStore.didChangeExternallyNotification, object: kv, queue: .main) { [weak self] note in
            let reason = note.userInfo?[NSUbiquitousKeyValueStoreChangeReasonKey] as? Int
            Task { @MainActor in
                // Un autre compte iCloud : rien de ce qu'il n'a pas n'est « retiré ailleurs ».
                if reason == NSUbiquitousKeyValueStoreAccountChange { self?.known = [] }
                self?.schedule()
            }
        })
        observers.append(center.addObserver(forName: .NSUbiquityIdentityDidChange, object: nil, queue: .main) { [weak self] _ in
            Task { @MainActor in
                self?.known = []
                self?.schedule()
            }
        })
        observers.append(center.addObserver(forName: UIApplication.didBecomeActiveNotification, object: nil, queue: .main) { [weak self] _ in
            Task { @MainActor in self?.schedule() }
        })
        kv.synchronize()
        sync()
    }

    /// Plusieurs changements de suite : une seule synchro.
    private func schedule() {
        guard !scheduled else { return }
        scheduled = true
        Task { @MainActor in
            try? await Task.sleep(for: .milliseconds(500))
            scheduled = false
            sync()
        }
    }

    func sync() {
        guard enabled, let ps = playstation else { return }
        let cloud = kv.data(forKey: Self.key).flatMap(PSShared.decode) ?? PSShared()
        let plan = PSCloudPlan.make(local: ps.cloudLocal, cloud: cloud, known: known) { Secrets.get($0) }
        ps.apply(plan)
        for k in plan.deleteSecrets { Secrets.delete(k) }
        for (k, v) in plan.writeSecrets {
            let status = Secrets.set(v, account: k)
            // Réessayé à la prochaine synchro ; l'élément manquant n'efface rien ailleurs.
            if status != errSecSuccess { KyberLog.app.error("Trousseau iCloud : \(status, privacy: .public) pour une console PlayStation") }
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

    /// Le Trousseau iCloud des consoles (éléments synchronisables).
    private enum Secrets {
        static let service = "fr.leslignesclaires.kybernow.cloud.playstation"

        private static func query(_ account: String) -> [String: Any] {
            [kSecClass as String: kSecClassGenericPassword,
             kSecAttrService as String: service,
             kSecAttrAccount as String: account,
             kSecAttrSynchronizable as String: true]
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
        static func set(_ value: String, account: String) -> OSStatus {
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
}
