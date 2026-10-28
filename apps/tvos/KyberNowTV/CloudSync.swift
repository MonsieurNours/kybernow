// Ordinateurs suivis par iCloud : ceux ajoutés dans KyberNow Link sur le Mac
// arrivent sur l'Apple TV (et l'iPhone, l'iPad : fichier repris par apps/ios)
// avec leurs accès, sans rien taper. Lecture seule : l'app Mac les publie
// (apps/macos/KyberNow/CloudPublisher.swift).
//
// La liste vit dans le magasin clé-valeur d'iCloud (clé « hosts », partagé
// par les apps : même identifiant, même équipe), les mots de passe dans le
// Trousseau iCloud (éléments synchronisables, service
// « fr.leslignesclaires.kybernow.cloud »). Une app signée par une autre
// équipe (scripts/sideload.sh) a son propre magasin : elle ne voit pas ce
// que publie l'app Mac de KyberNow.

import Combine
import Foundation
import KyberKit
import Security

struct CloudHost: Codable, Identifiable, Equatable {
    var id: UUID
    var name: String
    var address: String
    var consolePort: UInt16
    var username: String
    var streamPort: UInt16?
    var consoleFingerprint: String?
    /// Cartes réseau pour le réveil par le réseau, apprises par le Mac
    /// (absentes d'une liste publiée par une version précédente).
    var wake: [WakeTarget]?
    /// Le relais de l'ordinateur (`info.relay`, docs/relais.md), appris par le
    /// Mac (absent d'une liste publiée par une version précédente).
    var relay: RelayInfo?
    /// Appareil qui l'a publié (« Mac de Léa »).
    var from: String?
}

@MainActor
final class CloudSync: ObservableObject {
    @Published private(set) var hosts: [CloudHost] = []
    private let store = NSUbiquitousKeyValueStore.default
    private static let key = "hosts"

    init() {
        NotificationCenter.default.addObserver(forName: NSUbiquitousKeyValueStore.didChangeExternallyNotification, object: store, queue: .main) { [weak self] _ in
            Task { @MainActor in self?.load() }
        }
        store.synchronize()
        load()
    }

    func load() {
        guard let data = store.data(forKey: Self.key),
              let list = try? JSONDecoder().decode([CloudHost].self, from: data) else {
            hosts = []
            return
        }
        hosts = list
    }

    /// Mot de passe partagé par le Trousseau iCloud.
    func password(for id: UUID) -> String? {
        let q: [String: Any] = [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: "fr.leslignesclaires.kybernow.cloud",
            kSecAttrAccount as String: id.uuidString,
            kSecAttrSynchronizable as String: true,
            kSecReturnData as String: true,
            kSecMatchLimit as String: kSecMatchLimitOne,
        ]
        var out: CFTypeRef?
        guard SecItemCopyMatching(q as CFDictionary, &out) == errSecSuccess, let data = out as? Data else { return nil }
        return String(data: data, encoding: .utf8)
    }

    /// Ordinateur d'iCloud → ordinateur de cette Apple TV (même identifiant).
    func host(_ c: CloudHost) -> Host {
        var h = Host(name: c.name, address: c.address, consolePort: c.consolePort, username: c.username)
        h.id = c.id
        h.streamPort = c.streamPort
        h.consoleFingerprint = c.consoleFingerprint
        h.wake = c.wake
        h.relay = c.relay
        return h
    }
}
