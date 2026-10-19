// Ordinateurs enregistrés, réglages de connexion et Trousseau.

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

    /// Phrase courte, sous le sélecteur de l'accueil.
    var hint: String {
        switch self {
        case .jeu: return "Latence minimale, image à la fréquence de cet écran, manette prête."
        case .bureau: return "Texte net (4:4:4), aucune image perdue, débit plus élevé."
        case .max: return "Sans compromis : latence minimale, 4:4:4 et gros débit. Idéal en réseau local."
        }
    }

    /// Phrase de la page Réglages.
    var summary: String {
        switch self {
        case .jeu: return "Jeu : latence minimale, image fluide."
        case .bureau: return "Bureau : texte net, aucune image perdue."
        case .max: return "Max : sans compromis. Latence minimale, 4:4:4, gros débit."
        }
    }
}

enum DisplayChoice: String, Codable, CaseIterable {
    /// Écran virtuel à la taille et à la fréquence de l'écran de ce Mac.
    case match
    /// Écran virtuel à la taille de la fenêtre KyberNow (session fenêtrée).
    case window
    /// Écran virtuel au mode choisi.
    case custom
    /// Pas d'écran virtuel : les écrans de l'ordinateur tels quels.
    case physical
}

enum Transport: String, Codable, CaseIterable {
    case fast, reliable

    var label: String { self == .fast ? "Rapide" : "Fiable" }

    var hint: String {
        self == .fast
            ? "Une image perdue est réparée ou sautée, jamais attendue : la latence reste basse."
            : "Chaque image arrive, quitte à attendre un peu sur un réseau chargé."
    }
}

enum AudioChoice: String, Codable, CaseIterable {
    case auto, stereo, surround51, surround71, off

    var label: String {
        switch self {
        case .auto: return "Auto"
        case .stereo: return "Stéréo"
        case .surround51: return "5.1"
        case .surround71: return "7.1"
        case .off: return "Aucun"
        }
    }

    var hint: String {
        switch self {
        case .auto: return "Auto : stéréo pour l'instant, quelle que soit la sortie de ce Mac."
        case .stereo: return "Deux canaux : idéal au casque ou sur les haut-parleurs du Mac."
        case .surround51: return "Six canaux. Il faut une sortie 5.1 sur ce Mac (ampli HDMI, barre de son)."
        case .surround71: return "Huit canaux. Il faut une sortie 7.1 sur ce Mac."
        case .off: return "Le son reste sur l'ordinateur."
        }
    }

    var channels: Int {
        switch self {
        case .auto: return AudioOutput.channels()
        case .stereo: return 2
        case .surround51: return 6
        case .surround71: return 8
        case .off: return 0
        }
    }
}

/// Réglages de connexion d'un ordinateur. Un profil remplit l'image et le
/// réseau ; toucher à l'un de ces réglages rend le profil « Personnalisé ».
struct ConnectionSettings: Codable, Hashable {
    /// `nil` : Personnalisé.
    var profile: Profile? = .jeu
    var display: DisplayChoice = .match
    var customWidth = 2560
    var customHeight = 1440
    var customRefresh = 60
    var codec: VideoCodec = .hevc
    var yuv444 = false
    var bitrateMbps = 60
    var transport: Transport = .fast
    var autoReconnect = true
    var commandAsControl = true
    var audio: AudioChoice = .auto

    static let minBitrate = 5
    /// Le plus que Kyber accepte (débit en bits/s sur 32 bits).
    static let maxBitrate = 4294

    mutating func apply(_ profile: Profile) {
        self.profile = profile
        switch profile {
        case .jeu: codec = .hevc; yuv444 = false; bitrateMbps = 60; transport = .fast
        case .bureau: codec = .hevc; yuv444 = true; bitrateMbps = 90; transport = .reliable
        case .max: codec = .hevc; yuv444 = true; bitrateMbps = 300; transport = .fast
        }
    }

    var options: StreamOptions {
        var o = StreamOptions()
        o.codec = codec
        o.yuv444 = yuv444
        o.bitrate = UInt32(min(max(bitrateMbps, Self.minBitrate), Self.maxBitrate)) * 1_000_000
        o.reliable = transport == .reliable
        o.audio = audio != .off
        return o
    }

    var profileLabel: String { profile?.label ?? "Personnalisé" }
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
    var settings = ConnectionSettings()
    var lastSession: Date?
    /// Latence médiane de la dernière session, en millisecondes.
    var lastLatencyMs: Double?

    init(name: String, address: String, consolePort: UInt16, username: String) {
        self.name = name
        self.address = address
        self.consolePort = consolePort
        self.username = username
    }

    // Lecture tolérante : les ordinateurs enregistrés par les versions
    // précédentes n'avaient qu'un profil.
    private enum CodingKeys: String, CodingKey {
        case id, name, address, consolePort, username, streamPort, consoleFingerprint, settings, lastSession, lastLatencyMs, profile
    }

    init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        id = try c.decode(UUID.self, forKey: .id)
        name = try c.decode(String.self, forKey: .name)
        address = try c.decode(String.self, forKey: .address)
        consolePort = try c.decode(UInt16.self, forKey: .consolePort)
        username = try c.decode(String.self, forKey: .username)
        streamPort = try c.decodeIfPresent(UInt16.self, forKey: .streamPort)
        consoleFingerprint = try c.decodeIfPresent(String.self, forKey: .consoleFingerprint)
        lastSession = try c.decodeIfPresent(Date.self, forKey: .lastSession)
        lastLatencyMs = try c.decodeIfPresent(Double.self, forKey: .lastLatencyMs)
        if let s = try c.decodeIfPresent(ConnectionSettings.self, forKey: .settings) {
            settings = s
        } else if let p = try c.decodeIfPresent(Profile.self, forKey: .profile) {
            settings.apply(p)
        }
    }

    func encode(to encoder: Encoder) throws {
        var c = encoder.container(keyedBy: CodingKeys.self)
        try c.encode(id, forKey: .id)
        try c.encode(name, forKey: .name)
        try c.encode(address, forKey: .address)
        try c.encode(consolePort, forKey: .consolePort)
        try c.encode(username, forKey: .username)
        try c.encodeIfPresent(streamPort, forKey: .streamPort)
        try c.encodeIfPresent(consoleFingerprint, forKey: .consoleFingerprint)
        try c.encode(settings, forKey: .settings)
        try c.encodeIfPresent(lastSession, forKey: .lastSession)
        try c.encodeIfPresent(lastLatencyMs, forKey: .lastLatencyMs)
    }
}

/// État d'un ordinateur vu depuis ce Mac, rafraîchi en continu.
struct HostStatus: Equatable {
    var online: Bool
    var info: HostInfo?
}

@MainActor
final class HostStore: ObservableObject {
    @Published private(set) var hosts: [Host] = []
    @Published private(set) var status: [UUID: HostStatus] = [:]
    private let key = "hosts"
    private var polling: Task<Void, Never>?

    init() {
        if let data = UserDefaults.standard.data(forKey: key),
           let saved = try? JSONDecoder().decode([Host].self, from: data) {
            hosts = saved
        }
        startPolling()
    }

    /// Le plus récemment utilisé d'abord.
    var ordered: [Host] {
        hosts.sorted { ($0.lastSession ?? .distantPast) > ($1.lastSession ?? .distantPast) }
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
        Task { await refresh(host) }
    }

    func update(_ host: Host) {
        guard let i = hosts.firstIndex(where: { $0.id == host.id }) else { return }
        hosts[i] = host
        save()
    }

    func host(_ id: UUID) -> Host? {
        hosts.first { $0.id == id }
    }

    func remove(_ host: Host) {
        hosts.removeAll { $0.id == host.id }
        status[host.id] = nil
        Keychain.delete(host.id)
        save()
    }

    func password(for host: Host) -> String? {
        Keychain.get(host.id)
    }

    // MARK: - En ligne ou pas

    private func startPolling() {
        polling = Task { [weak self] in
            while !Task.isCancelled {
                if let self {
                    await withTaskGroup(of: Void.self) { group in
                        for host in await self.hosts {
                            group.addTask { await self.refresh(host) }
                        }
                    }
                }
                try? await Task.sleep(for: .seconds(8))
            }
        }
    }

    func refresh(_ host: Host) async {
        guard let password = password(for: host) else { return }
        let console = ConsoleClient(host: host.address, port: host.consolePort, username: host.username, password: password, fingerprint: host.consoleFingerprint)
        do {
            let info = try await console.info()
            status[host.id] = HostStatus(online: true, info: info)
            if host.consoleFingerprint == nil, let fp = console.fingerprint, var h = self.host(host.id) {
                h.consoleFingerprint = fp
                update(h)
            }
        } catch ConsoleError.http(404, _) {
            // Console d'une version précédente, sans /api/stream/info : elle répond, donc en ligne.
            status[host.id] = HostStatus(online: true, info: nil)
        } catch {
            status[host.id] = HostStatus(online: false, info: status[host.id]?.info)
        }
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
