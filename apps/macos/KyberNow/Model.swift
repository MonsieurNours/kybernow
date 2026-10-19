// Ordinateurs enregistrés, profils de connexion et Trousseau.

import Foundation
import KyberKit
import Security

enum Profile: String, Codable, CaseIterable, Identifiable {
    case jeu, bureau, max

    var id: String { rawValue }

    var label: String {
        switch self {
        case .jeu: return "Jeu"
        case .bureau: return "Bureau"
        case .max: return "Max"
        }
    }

    var hint: String {
        switch self {
        case .jeu: return "Latence minimale, image à la fréquence de cet écran."
        case .bureau: return "Texte net (4:4:4), débit plus élevé."
        case .max: return "Sans compromis : latence minimale, 4:4:4 et gros débit."
        }
    }

    var options: StreamOptions {
        var o = StreamOptions()
        switch self {
        case .jeu: o.codec = .hevc; o.yuv444 = false; o.bitrate = 60_000_000
        case .bureau: o.codec = .hevc; o.yuv444 = true; o.bitrate = 90_000_000
        case .max: o.codec = .hevc; o.yuv444 = true; o.bitrate = 300_000_000
        }
        return o
    }
}

struct Host: Codable, Identifiable, Hashable {
    var id = UUID()
    var name: String
    var address: String
    var consolePort: UInt16
    var username: String
    /// Port du flux Kyber, appris auprès de la console.
    var streamPort: UInt16?
    /// Empreinte du certificat de la console, retenue au premier contact.
    var consoleFingerprint: String?
    var profile: Profile = .jeu
    var lastSession: Date?
}

@MainActor
final class HostStore: ObservableObject {
    @Published private(set) var hosts: [Host] = []
    private let key = "hosts"

    init() {
        if let data = UserDefaults.standard.data(forKey: key),
           let saved = try? JSONDecoder().decode([Host].self, from: data) {
            hosts = saved
        }
    }

    private func save() {
        if let data = try? JSONEncoder().encode(hosts) {
            UserDefaults.standard.set(data, forKey: key)
        }
    }

    func add(_ host: Host, password: String) {
        hosts.append(host)
        Keychain.set(password, for: host.id)
        save()
    }

    func update(_ host: Host) {
        guard let i = hosts.firstIndex(where: { $0.id == host.id }) else { return }
        hosts[i] = host
        save()
    }

    func remove(_ host: Host) {
        hosts.removeAll { $0.id == host.id }
        Keychain.delete(host.id)
        save()
    }

    func password(for host: Host) -> String? {
        Keychain.get(host.id)
    }
}

enum Keychain {
    private static let service = "fr.leslignesclaires.kybernow"

    private static func query(_ id: UUID) -> [String: Any] {
        [kSecClass as String: kSecClassGenericPassword, kSecAttrService as String: service, kSecAttrAccount as String: id.uuidString]
    }

    static func set(_ password: String, for id: UUID) {
        SecItemDelete(query(id) as CFDictionary)
        var item = query(id)
        item[kSecValueData as String] = Data(password.utf8)
        item[kSecAttrLabel as String] = "KyberNow"
        SecItemAdd(item as CFDictionary, nil)
    }

    static func get(_ id: UUID) -> String? {
        var q = query(id)
        q[kSecReturnData as String] = true
        q[kSecMatchLimit as String] = kSecMatchLimitOne
        var out: CFTypeRef?
        guard SecItemCopyMatching(q as CFDictionary, &out) == errSecSuccess, let data = out as? Data else { return nil }
        return String(data: data, encoding: .utf8)
    }

    static func delete(_ id: UUID) {
        SecItemDelete(query(id) as CFDictionary)
    }
}
