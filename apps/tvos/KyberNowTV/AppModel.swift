// État de l'app : ordinateurs, découverte sur le réseau, iCloud, écran en
// cours, session. Un seul écran à la fois, comme une app de TV : l'accueil,
// et par-dessus l'ajout, les réglages, les manettes ou la session.

import Combine
import KyberKit
import SwiftUI
import TVServices

@MainActor
final class AppModel: ObservableObject {
    enum Screen: Equatable {
        case home
        case addHost
        case settings(UUID)
        case gamepads
        case session(UUID)
    }

    @Published var screen: Screen = .home
    /// Ordinateur mis en avant sur l'accueil.
    @Published var selected: UUID?
    @Published private(set) var session: SessionController?

    let store = HostStore()
    let discovery = Discovery()
    let cloud = CloudSync()
    let prefs = Preferences.shared
    private var bag: Set<AnyCancellable> = []

    init() {
        discovery.start()
        selected = store.ordered.first?.id
        // Le Top Shelf suit les ordinateurs, leur état et leurs images.
        store.objectWillChange.merge(with: store.imagery.objectWillChange)
            .debounce(for: .seconds(1), scheduler: RunLoop.main)
            .sink { [weak self] in self?.publishTopShelf() }
            .store(in: &bag)
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
        #endif
        #if DEBUG
        // Vérification des écrans sans télécommande : `-screen add|gamepads|settings|menu|passage`.
        switch UserDefaults.standard.string(forKey: "screen") {
        case "add": screen = .addHost
        case "gamepads": screen = .gamepads
        case "settings": if let id = store.ordered.first?.id { screen = .settings(id) }
        case "menu", "passage":
            if let host = store.ordered.first {
                let controller = SessionController(host: host, store: store) { [weak self] _ in
                    self?.session = nil
                    self?.screen = .home
                }
                session = controller
                screen = .session(host.id)
                controller.demo(menu: UserDefaults.standard.string(forKey: "screen") == "menu")
            }
        default: break
        }
        #endif
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
        let controller = SessionController(host: host, store: store) { [weak self] _ in
            withAnimation(Theme.join) {
                self?.session = nil
                self?.screen = .home
            }
        }
        session = controller
        screen = .session(id)
        controller.start()
    }

    /// Liens du Top Shelf : `kybernow-tv://connect/<id>` ou `show/<id>`.
    func open(_ url: URL) {
        guard url.scheme == "kybernow-tv", let id = UUID(uuidString: url.lastPathComponent), store.host(id) != nil else { return }
        if url.host == "connect", store.status[id]?.online != false {
            connect(id)
        } else if session == nil {
            selected = id
            screen = .home
        }
    }

    private func publishTopShelf() {
        let entries = store.ordered.map { h in
            TopShelfEntry(id: h.id, name: h.name, online: store.status[h.id]?.online ?? false,
                          reach: store.status[h.id]?.online == true ? (isLocal(h) ? "réseau local" : "par VPN") : "hors ligne",
                          lastSession: h.lastSession, imageFile: store.imagery.file(h.id))
        }
        TopShelfStore.write(entries)
        TVTopShelfContentProvider.topShelfContentDidChange()
    }
}
