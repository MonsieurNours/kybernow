// État de l'app : ordinateurs, découverte sur le réseau, iCloud, écran en
// cours, session. L'accueil, et par-dessus l'ajout, les réglages ou la
// session (plein écran, toujours en paysage sur iPhone).

import Combine
import KyberKit
import SwiftUI

@MainActor
final class AppModel: ObservableObject {
    enum Sheet: Identifiable, Equatable {
        case addHost(Candidate?)
        case settings(UUID)
        case appSettings

        var id: String {
            switch self {
            case .addHost: return "add"
            case let .settings(id): return "settings-\(id)"
            case .appSettings: return "app"
            }
        }
    }

    /// Fiche par-dessus l'accueil.
    @Published var sheet: Sheet?
    /// Ordinateur mis en avant sur l'accueil.
    @Published var selected: UUID?
    @Published private(set) var session: SessionController?
    /// Le lancement animé (une fois par démarrage de l'app).
    @Published var launched = false
    @Published var toast: String?

    let store = HostStore()
    let discovery = Discovery()
    let cloud = CloudSync()
    let prefs = Preferences.shared
    private var bag: Set<AnyCancellable> = []

    init() {
        discovery.start()
        selected = store.ordered.first?.id
        // iCloud : les ordinateurs du Mac arrivent tout seuls si la synchro est active.
        cloud.$hosts
            .sink { [weak self] list in self?.importCloud(list) }
            .store(in: &bag)
        #if DEBUG && targetEnvironment(simulator)
        // Ordinateurs de démonstration (simulateur) : `-demo YES`.
        if UserDefaults.standard.bool(forKey: "demo"), store.hosts.isEmpty {
            for (name, address) in [("Tour du bureau", "192.168.1.42"), ("Atelier", "10.8.0.12"), ("Mac mini du studio", "192.168.1.20")] {
                var h = Host(name: name, address: address, consolePort: 31877, username: "demo")
                h.lastSession = Date().addingTimeInterval(-Double.random(in: 3600...200_000))
                h.lastLatencyMs = 8.3
                store.add(h, password: "demo")
            }
            selected = store.ordered.first?.id
        }
        if UserDefaults.standard.bool(forKey: "skipLaunch") { launched = true }
        switch UserDefaults.standard.string(forKey: "screen") {
        case "add": sheet = .addHost(nil)
        case "settings": if let id = store.ordered.first?.id { sheet = .settings(id) }
        case "session", "bar":
            if let host = store.ordered.first {
                launched = true
                let controller = SessionController(host: host, store: store) { [weak self] _ in self?.session = nil }
                session = controller
                controller.demo(bar: UserDefaults.standard.string(forKey: "screen") == "bar")
            }
        default: break
        }
        #endif
    }

    var selectedHost: Host? {
        selected.flatMap { store.host($0) } ?? store.ordered.first
    }

    // MARK: - Réseau

    /// Vu sur le réseau local (annonce Bonjour) : sinon, c'est par VPN.
    func isLocal(_ host: Host) -> Bool {
        discovery.found.contains { found in
            found.name == host.name || (host.consoleFingerprint != nil && found.fingerprint == host.consoleFingerprint)
        }
    }

    /// Annoncés sur le réseau et pas encore ajoutés.
    var newOnNetwork: [Discovery.Found] {
        discovery.found.filter { found in
            !store.hosts.contains { $0.name == found.name || ($0.consoleFingerprint != nil && $0.consoleFingerprint == found.fingerprint) }
        }
    }

    // MARK: - iCloud

    private func importCloud(_ list: [CloudHost]) {
        guard prefs.icloudSync else { return }
        for c in list where store.host(c.id) == nil {
            if let password = cloud.password(for: c.id) {
                store.add(cloud.host(c), password: password)
            }
        }
    }

    func addFromCloud(_ c: CloudHost) -> Bool {
        guard store.host(c.id) == nil, let password = cloud.password(for: c.id) else { return false }
        store.add(cloud.host(c), password: password)
        return true
    }

    // MARK: - Session

    func connect(_ id: UUID) {
        guard session == nil, let host = store.host(id) else { return }
        selected = id
        sheet = nil
        let controller = SessionController(host: host, store: store) { [weak self] _ in
            withAnimation(Theme.join) { self?.session = nil }
        }
        session = controller
        controller.start()
    }

    /// kybernow://adresse:port?name=…&fp=… (QR code ou lien de la console,
    /// AirDrop, Messages) : l'ajout s'ouvre, pré-rempli.
    func open(_ url: URL) {
        guard session == nil, let candidate = PairingLink.parse(url.absoluteString) else { return }
        if store.hosts.contains(where: { ($0.consoleFingerprint != nil && $0.consoleFingerprint == candidate.expected) || ($0.address == candidate.address && $0.consolePort == candidate.port) }) {
            show(String(localized: "\(candidate.name) est déjà dans vos ordinateurs"))
            return
        }
        sheet = .addHost(candidate)
    }

    func show(_ text: String) {
        withAnimation(Theme.easeOut) { toast = text }
        DispatchQueue.main.asyncAfter(deadline: .now() + 4) { [weak self] in
            guard self?.toast == text else { return }
            withAnimation(.easeIn(duration: 0.3)) { self?.toast = nil }
        }
    }
}
