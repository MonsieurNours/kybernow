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
        /// KyberNow Share : « Rejoindre avec un code » (le lien qui l'ouvre, s'il y en a un).
        case join(ShareJoinLink?)
        case settings(UUID)
        case appSettings
        case gamepads
        case session(UUID)
    }

    @Published var screen: Screen = .home
    /// Ordinateur mis en avant sur l'accueil.
    @Published var selected: UUID?
    @Published private(set) var session: SessionController?
    /// Écran où revient le test des manettes (accueil ou réglages de l'app).
    var gamepadsReturn: Screen = .home
    /// Rubrique ouverte à l'arrivée dans les réglages d'un ordinateur.
    var settingsSection: ComputerSection = .picture

    let store: HostStore
    let discovery = Discovery()
    let cloud = CloudSync()
    let prefs = Preferences.shared
    private var bag: Set<AnyCancellable> = []

    init() {
        #if DEBUG
        // Ordinateurs de démonstration : `-demo YES` (LinkUI.swift, `loadDemo`).
        let demo = UserDefaults.standard.bool(forKey: "demo")
        #else
        let demo = false
        #endif
        store = HostStore(demo: demo)
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
        #if DEBUG
        if demo {
            store.loadDemo()
            let ordered = store.ordered
            let i = UserDefaults.standard.integer(forKey: "hostIndex")
            selected = ordered.indices.contains(i) ? ordered[i].id : ordered.first?.id
        }
        // Vérification des écrans sans télécommande :
        // `-screen add|app|gamepads|settings|menu|passage|connecting|restarting`,
        // `-section picture|sound|controls|network|statistics|station`,
        // `-stationAction restart|update`.
        if let section = UserDefaults.standard.string(forKey: "section").flatMap(ComputerSection.init(rawValue:)) {
            settingsSection = section
        }
        switch UserDefaults.standard.string(forKey: "screen") {
        case "add": screen = .addHost
        case "join": screen = .join(nil)
        case "guest", "guest-menu":
            // `-screen guest|guest-menu -demoRole spectator|player|kbm|player_kbm` : la session d'un invité, sans Station.
            let role: ShareRole = switch UserDefaults.standard.string(forKey: "demoRole") { case "player": .player; case "kbm": .kbm; case "player_kbm": .playerKbm; default: .spectator }
            openGuestSession(ShareFound(demoComputer: "PC-DE-MATTHIEU", host: "Matthieu", role: role))
        case "app": screen = .appSettings
        case "gamepads": screen = .gamepads
        case "settings": if let id = selected { screen = .settings(id) }
        case "menu", "passage", "connecting", "restarting":
            if let host = selected.flatMap(store.host) {
                let controller = SessionController(host: host, store: store) { [weak self] _ in
                    self?.session = nil
                    self?.screen = .home
                }
                session = controller
                screen = .session(host.id)
                switch UserDefaults.standard.string(forKey: "screen") {
                case "menu": controller.demo(.menu)
                case "connecting": controller.demo(.connecting)
                case "restarting": controller.demo(.restarting)
                default: controller.demo(.passage)
                }
            }
        default: break
        }
        // `-wakeNow YES` : « Réveiller l'ordinateur » choisi, appuyé à l'arrivée.
        if UserDefaults.standard.bool(forKey: "wakeNow"), let host = selected.flatMap(store.host) {
            DispatchQueue.main.asyncAfter(deadline: .now() + 1) { [store] in store.wake(host) }
        }
        // `-stationAction restart|update` : l'action de la rubrique Station, lancée à l'arrivée.
        if let action = UserDefaults.standard.string(forKey: "stationAction"), let host = selected.flatMap(store.host) {
            DispatchQueue.main.asyncAfter(deadline: .now() + 1.5) { [store] in
                if action == "update" { store.updateStation(host) } else { store.restartStation(host) }
            }
        }
        #endif
    }

    /// Le test des manettes, depuis l'accueil ou les réglages de l'app.
    func showGamepads(from: Screen) {
        gamepadsReturn = from
        screen = .gamepads
    }

    /// Réglages d'un ordinateur, sur une rubrique.
    func showSettings(_ id: UUID, section: ComputerSection = .picture) {
        settingsSection = section
        screen = .settings(id)
    }

    // MARK: - Réseau

    /// Annoncés sur le réseau et pas encore ajoutés.
    var newOnNetwork: [Discovery.Found] {
        discovery.found.filter { found in
            !store.hosts.contains { $0.name == found.name || ($0.consoleFingerprint != nil && $0.consoleFingerprint == found.fingerprint) }
        }
    }

    // MARK: - iCloud

    private func importCloud(_ list: [CloudHost]) {
        guard prefs.icloudSync, !store.demo else { return }
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

    /// KyberNow Share : la session d'un invité (une Station qui a accepté un
    /// code). Comme « Se connecter », sans ordinateur ajouté à cet appareil ;
    /// rien n'est gardé après le partage.
    func openGuestSession(_ found: ShareFound) {
        // Une session s'est ouverte entre-temps : l'invité repart tout de suite.
        guard session == nil else {
            Share.leave(found)
            return
        }
        let controller = SessionController(guest: found, store: store) { [weak self] _ in
            withAnimation(Theme.join) {
                self?.session = nil
                self?.screen = .home
            }
        }
        session = controller
        screen = .session(controller.host.id)
        #if DEBUG
        if store.demo {
            controller.demoGuest(role: found.role, menu: UserDefaults.standard.string(forKey: "screen") == "guest-menu")
            return
        }
        #endif
        controller.start()
    }

    /// Liens du Top Shelf : `kybernow-tv://connect/<id>` ou `show/<id>` ;
    /// `kybernow-tv://join?host=…&port=…&fp=…&code=…` : un partage à rejoindre
    /// (le lien `kybernow://join` de l'hôte, sous le schéma de l'Apple TV).
    func open(_ url: URL) {
        if url.scheme == "kybernow-tv", url.host == "join" {
            let text = url.absoluteString.replacingOccurrences(of: "kybernow-tv://", with: "kybernow://")
            if session == nil, let link = Share.parseLink(text) { screen = .join(link) }
            return
        }
        guard url.scheme == "kybernow-tv", let id = UUID(uuidString: url.lastPathComponent), store.host(id) != nil else { return }
        if url.host == "connect", store.status[id]?.online != false {
            connect(id)
        } else if session == nil {
            selected = id
            screen = .home
        }
    }

    private func publishTopShelf() {
        guard !store.demo else { return }
        let entries = store.ordered.map { h in
            TopShelfEntry(id: h.id, name: h.name, online: store.status[h.id]?.online ?? false,
                          reach: store.status[h.id]?.online == true ? "en ligne" : "hors ligne",
                          lastSession: h.lastSession, imageFile: store.imagery.file(h.id))
        }
        TopShelfStore.write(entries)
        TVTopShelfContentProvider.topShelfContentDidChange()
    }
}
