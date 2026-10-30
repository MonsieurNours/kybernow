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
        /// Prêter l'ordinateur : le lien d'invitation d'un ami (`kybernow-tv://borrow?…`).
        case invite(LendInvite)
        /// Le mot de passe d'un ordinateur prêté, demandé à « Emprunter ».
        case borrowPassword(UUID)
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
    /// Les ordinateurs que des amis prêtent à cette Apple TV (docs/pret.md), à part des siens.
    let borrowed: BorrowedStore
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
        borrowed = BorrowedStore(demo: demo)
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
            // `-demoBorrowed free|busy|lent|locked|off|offline|slot|slot-now|refused` : « Ordinateur de Matthieu »
            // sur l'accueil, mis en avant ; `-demoLent YES` : la Tour est prêtée à Léa (`-demoLentNotice YES` : préavis).
            if let state = UserDefaults.standard.string(forKey: "demoBorrowed") {
                borrowed.loadDemo(state)
                selected = borrowed.computers.first?.id
            }
            // `-demoLend code|request` (menu de session) ou `-demoLendInfo on|off` : la Station de la Tour sait prêter
            // (prêt activé, ou coupé dans Admin) ; sans eux, une Station d'avant la 0.9.0 (pas de tuile du prêt).
            let lendInfo = UserDefaults.standard.string(forKey: "demoLendInfo") ?? (UserDefaults.standard.string(forKey: "demoLend") != nil ? "on" : nil)
            if let lendInfo, let first = ordered.first, let i = store.status[first.id]?.info {
                let info = HostInfo(hostname: i.hostname, gpu: i.gpu, av1: i.av1, virtualDisplay: i.virtualDisplay, speakers: i.speakers, os: i.os,
                                    adminVersion: i.adminVersion, station: i.station, wake: i.wake, ownDevice: i.ownDevice, maxScreens: i.maxScreens,
                                    share: i.share, relay: i.relay, lend: LendInfo(available: true, enabled: lendInfo == "on"))
                store.setDemoStatus(HostStatus(online: true, info: info), for: first.id)
            }
            if UserDefaults.standard.bool(forKey: "demoLent"), let first = ordered.first {
                let notice = UserDefaults.standard.bool(forKey: "demoLentNotice") ? LendNotice(reason: .takenBack, at: Date().addingTimeInterval(102)) : nil
                store.setDemoLend(LendView(state: .lent, borrower: LendBorrower(name: "Léa", until: Date().addingTimeInterval(3 * 3600)), notice: notice), for: first.id)
            }
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
        case "borrow", "borrow-waiting", "borrow-menu":
            // `-screen borrow|borrow-waiting|borrow-menu [-demoBorrowThen notice|taken_back|lost] [-demoBorrowOpen YES]` :
            // la session d'emprunt, sans Station (« En attente de {hôte}… », en image, menu ouvert).
            let target = LendTarget(host: "192.168.1.30", port: 37462, fingerprint: nil)
            openBorrowSession(BorrowSource(target: target, credentials: .code("482915"), name: "Apple TV", hostHint: "Matthieu"))
        case "invite":
            if let invite = Lend.parseInvite("kybernow://borrow?host=192.168.1.30&port=37462&fp=sha256:ab12&login=camille") { screen = .invite(invite) }
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

    // MARK: - Prêter l'ordinateur (l'ami)

    /// « Emprunter » (« Demander à {hôte} », « Revenir ») : la session
    /// d'emprunt, avec le compte d'ami ; sans mot de passe retenu, il est demandé.
    func borrow(_ computer: BorrowedComputer) {
        guard session == nil, let target = computer.target else { return }
        selected = computer.id
        guard let password = borrowed.password(for: computer) else {
            screen = .borrowPassword(computer.id)
            return
        }
        openBorrowSession(BorrowSource(target: target, credentials: .account(login: computer.login, password: password),
                                       name: GuestName.proposed, hostHint: computer.hostName, computer: computer.id))
    }

    /// « Retirer » un ordinateur prêté de cette Apple TV.
    func removeBorrowed(_ computer: BorrowedComputer) {
        borrowed.remove(computer)
        if selected == computer.id { selected = store.ordered.first?.id ?? borrowed.computers.first?.id }
    }

    /// La session d'un ami qui emprunte : elle emprunte d'abord
    /// (« Préparation de l'ordinateur », « En attente de {hôte}… »), puis se
    /// connecte ; « Revenir » en ouvre une nouvelle avec la même demande.
    func openBorrowSession(_ source: BorrowSource) {
        guard session == nil else { return }
        let controller = SessionController(borrow: source, store: store) { [weak self] _ in
            withAnimation(Theme.join) {
                self?.session = nil
                self?.screen = .home
            }
            // La carte de l'accueil dit de nouveau l'état de l'ordinateur.
            if let self, let id = source.computer, let c = self.borrowed.computer(id) { Task { await self.borrowed.refresh(c) } }
        }
        controller.onComeBack = { [weak self] source in
            DispatchQueue.main.asyncAfter(deadline: .now() + 0.5) { self?.openBorrowSession(source) }
        }
        session = controller
        screen = .session(controller.host.id)
        #if DEBUG
        if store.demo {
            let args = UserDefaults.standard
            controller.demoBorrow(waiting: args.string(forKey: "screen") == "borrow-waiting", menu: args.string(forKey: "screen") == "borrow-menu",
                                  until: args.bool(forKey: "demoBorrowOpen") ? nil : Date().addingTimeInterval(80 * 60),
                                  then: args.string(forKey: "demoBorrowThen"))
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
        // `kybernow-tv://borrow?…` : l'invitation d'un ami qui prête son ordinateur (le lien `kybernow://borrow`).
        if url.scheme == "kybernow-tv", url.host == "borrow" {
            let text = url.absoluteString.replacingOccurrences(of: "kybernow-tv://", with: "kybernow://")
            if session == nil, let invite = Lend.parseInvite(text) { screen = .invite(invite) }
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
