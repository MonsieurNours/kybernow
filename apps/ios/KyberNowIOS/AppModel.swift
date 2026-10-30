// État de l'app : ordinateurs, découverte sur le réseau, iCloud, écran en
// cours, session. L'accueil, et par-dessus l'ajout, les réglages ou la
// session (plein écran, toujours en paysage sur iPhone).

import Combine
import KyberKit
import Network
import SwiftUI

@MainActor
final class AppModel: ObservableObject {
    enum Sheet: Identifiable, Equatable {
        case addHost(Candidate?)
        /// KyberNow Share : « Rejoindre avec un code » (le lien `kybernow://join` qui l'ouvre, s'il y en a un).
        case join(ShareJoinLink?)
        case settings(UUID)
        case appSettings
        /// Prêter l'ordinateur : le lien d'invitation d'un ami (`kybernow://borrow`).
        case invite(LendInvite)
        /// Le mot de passe d'un ordinateur prêté, demandé à « Emprunter ».
        case borrowPassword(UUID)

        var id: String {
            switch self {
            case .addHost: return "add"
            case .join: return "join"
            case let .settings(id): return "settings-\(id)"
            case .appSettings: return "app"
            case let .invite(invite): return "invite-\(invite.id)"
            case let .borrowPassword(id): return "borrow-\(id)"
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
    /// Ordinateur à ouvrir une fois l'accord donné : l'appareil est en
    /// données mobiles et « Sessions en données mobiles » est coupé.
    @Published var mobileDataAsk: UUID?

    let store: HostStore
    /// Les ordinateurs que des amis prêtent à cet appareil (docs/pret.md), à part des siens.
    let borrowed: BorrowedStore
    let discovery = Discovery()
    let cloud = CloudSync()
    let prefs = Preferences.shared
    private var bag: Set<AnyCancellable> = []
    /// Chemin réseau de l'appareil, lu au moment de se connecter.
    private let path = NWPathMonitor()

    init() {
        #if DEBUG && targetEnvironment(simulator)
        let demo = UserDefaults.standard.bool(forKey: "demo")
        #else
        let demo = false
        #endif
        store = HostStore(demo: demo)
        borrowed = BorrowedStore(demo: demo)
        borrowed.objectWillChange.sink { [weak self] _ in self?.objectWillChange.send() }.store(in: &bag)
        discovery.start()
        path.start(queue: DispatchQueue(label: "fr.leslignesclaires.kybernow.path"))
        selected = store.ordered.first?.id
        // iCloud : les ordinateurs du Mac arrivent tout seuls si la synchro est
        // active (pas en démonstration : ses ordinateurs restent les siens).
        if !demo {
            cloud.$hosts
                .sink { [weak self] list in self?.importCloud(list) }
                .store(in: &bag)
        }
        #if DEBUG && targetEnvironment(simulator)
        // Vérification des écrans sans Station (simulateur) : `-demo YES`, les
        // ordinateurs de démonstration de LinkUI.swift (`-demoState`, `-demoStation`,
        // `-demoProblem`…), `-hostIndex 0|1|2` (l'ordinateur choisi, dans l'ordre de
        // l'accueil), `-demoWaking YES` (son réveil en cours), `-screen
        // settings|app|add|connecting|restarting|session|bar`, `-section
        // picture|sound|controls|network|statistics|station` (rubrique des réglages).
        // Manette à l'écran : `-demoPad full|simple` (affichée), `-demoPadSize
        // small|medium|large`, `-demoPadOpacity 0.3`, `-demoPadHaptics
        // off|light|medium|strong`, `-demoPadPress a,lt,dpad,leftStick`
        // (commandes montrées appuyées) ; `-demoKeyboard YES` : clavier sorti.
        // Prêter l'ordinateur : `-demoBorrowed free|busy|lent|locked|off|offline|refused|slot|slot-now`
        // (« Ordinateur de Matthieu » sur l'accueil, en scène avec `-demoBorrowedSelected YES`) ;
        // `-screen borrow` : la session d'emprunt (`-demoBorrowWaiting YES` : « En attente de Matthieu… »,
        // `-demoBorrowOpen YES` : sans fin prévue, `-demoBorrowThen notice|taken_back|lost`) ;
        // `-screen invite` : un lien d'invitation ouvert ; `-screen bar -demoLend YES` (`-demoLendCode YES`,
        // `-demoLendRequest Léa`) : le panneau « Prêter l'ordinateur… » ; `-demoLent YES` : le premier
        // ordinateur prêté à Léa (`-demoLent notice` : pendant le préavis de reprise).
        guard demo else { return }
        store.loadDemo()
        let list = store.ordered
        let index = UserDefaults.standard.integer(forKey: "hostIndex")
        selected = list.indices.contains(index) ? list[index].id : list.first?.id
        if let host = selectedHost { demoPad(host) }
        if let state = UserDefaults.standard.string(forKey: "demoBorrowed") {
            borrowed.loadDemo(state)
            if UserDefaults.standard.bool(forKey: "demoBorrowedSelected") { selected = borrowed.computers.first?.id }
        }
        // Une Station qui sait prêter (0.9.0), pour « Prêter l'ordinateur… » et l'accueil d'un ordinateur prêté.
        let lendDemo = UserDefaults.standard.string(forKey: "demoLent") != nil || UserDefaults.standard.bool(forKey: "demoLend")
            || UserDefaults.standard.string(forKey: "demoLendRequest") != nil
        if lendDemo, let host = selectedHost {
            let i = store.status[host.id]?.info
            store.setDemoStatus(HostStatus(online: true, info: HostInfo(hostname: i?.hostname ?? host.name, gpu: i?.gpu, av1: i?.av1 ?? false, os: i?.os,
                                                                        adminVersion: "0.9.0", station: i?.station, maxScreens: i?.maxScreens ?? 1,
                                                                        lend: LendInfo(available: true, enabled: true))), for: host.id)
        }
        if let lent = UserDefaults.standard.string(forKey: "demoLent"), let host = selectedHost {
            store.setDemoLend(LendView(state: .lent, borrower: LendBorrower(name: "Léa", until: Date().addingTimeInterval(2 * 3600)),
                                       notice: lent == "notice" ? LendNotice(reason: .takenBack, at: Date().addingTimeInterval(102)) : nil), for: host.id)
        }
        if UserDefaults.standard.bool(forKey: "skipLaunch") { launched = true }
        // `-demoWaking YES` : le réveil de l'ordinateur choisi, déjà lancé.
        if UserDefaults.standard.bool(forKey: "demoWaking"), let host = selectedHost { store.wake(host) }
        switch UserDefaults.standard.string(forKey: "screen") {
        case "add": sheet = .addHost(nil)
        case "join": sheet = .join(nil)
        case "guest":
            // `-screen guest -demoRole spectator|player|kbm|player_kbm` : la session d'un invité, sans Station.
            launched = true
            let role: ShareRole = switch UserDefaults.standard.string(forKey: "demoRole") { case "player": .player; case "kbm": .kbm; case "player_kbm": .playerKbm; default: .spectator }
            openGuestSession(ShareFound(demoComputer: "PC-DE-MATTHIEU", host: "Matthieu", role: role))
        case "borrow":
            launched = true
            let target = LendTarget(host: "192.168.1.30", port: 37462, fingerprint: nil)
            openBorrowSession(BorrowSource(target: target, credentials: .code("482915"), name: "Camille", hostHint: "Matthieu"))
        case "invite":
            if let invite = Lend.parseInvite("kybernow://borrow?host=192.168.1.30&port=37462&fp=sha256:ab12&login=camille") { sheet = .invite(invite) }
        case "app": sheet = .appSettings
        case "settings": if let id = selected { sheet = .settings(id) }
        case "connecting", "restarting", "session", "bar":
            if let host = selectedHost {
                launched = true
                let controller = SessionController(host: host, store: store) { [weak self] _ in self?.session = nil }
                session = controller
                switch UserDefaults.standard.string(forKey: "screen") {
                case "connecting": controller.demo(.connecting)
                case "restarting": controller.demo(.restarting)
                case "bar": controller.demo(.bar)
                default: controller.demo(.image)
                }
            }
        default: break
        }
        #endif
    }

    #if DEBUG && targetEnvironment(simulator)
    /// Réglages de la manette à l'écran de démonstration (`-demoPad…`).
    private func demoPad(_ host: Host) {
        var h = host
        let d = UserDefaults.standard
        if let layout = d.string(forKey: "demoPad").flatMap(ScreenPadLayout.init(rawValue:)) {
            h.settings.screenPad = true
            h.settings.screenPadLayout = layout
        }
        if let size = d.string(forKey: "demoPadSize").flatMap(ScreenPadSize.init(rawValue:)) { h.settings.screenPadSize = size }
        if d.object(forKey: "demoPadOpacity") != nil { h.settings.screenPadOpacity = min(1, max(0.2, d.double(forKey: "demoPadOpacity"))) }
        if let haptics = d.string(forKey: "demoPadHaptics").flatMap(ScreenPadHaptics.init(rawValue:)) { h.settings.screenPadHaptics = haptics }
        store.update(h)
    }
    #endif

    #if DEBUG
    /// Rubrique ouverte d'emblée dans les réglages d'un ordinateur (`-section`).
    static var demoSection: ComputerSection? {
        UserDefaults.standard.string(forKey: "section").flatMap(ComputerSection.init(rawValue:))
    }
    #endif

    var selectedHost: Host? {
        guard selectedBorrowed == nil else { return nil }
        return selected.flatMap { store.host($0) } ?? store.ordered.first
    }

    /// L'ordinateur d'un ami en scène (choisi, ou le seul quand il n'y a pas d'ordinateur à soi).
    var selectedBorrowed: BorrowedComputer? {
        borrowed.computers.first { $0.id == selected } ?? (store.hosts.isEmpty ? borrowed.computers.first : nil)
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

    /// `mobileDataOK` : l'accord est donné (alerte de l'accueil), la session
    /// part même en données mobiles.
    func connect(_ id: UUID, mobileDataOK: Bool = false) {
        guard session == nil, let host = store.host(id) else { return }
        if !mobileDataOK, !prefs.allowCellular, onMobileData {
            mobileDataAsk = id
            return
        }
        selected = id
        sheet = nil
        let controller = SessionController(host: host, store: store) { [weak self] _ in
            withAnimation(Theme.join) { self?.session = nil }
        }
        session = controller
        controller.start()
    }

    /// Données mobiles, ou partage de connexion d'un iPhone (Wi-Fi « coûteux »
    /// pour le système) : c'est le forfait qui paie la session. Ethernet et
    /// Wi-Fi ordinaire ne demandent rien.
    private var onMobileData: Bool {
        let p = path.currentPath
        return p.status == .satisfied && (p.usesInterfaceType(.cellular) || p.isExpensive)
    }

    /// KyberNow Share : la session d'un invité (une Station qui a accepté un
    /// code). Une session à la fois : comme « Se connecter », sans ordinateur
    /// ajouté à cet appareil ; rien n'est gardé après le partage.
    func openGuestSession(_ found: ShareFound) {
        // Une session s'est ouverte entre-temps : l'invité repart tout de suite.
        guard session == nil else {
            Share.leave(found)
            return
        }
        sheet = nil
        let controller = SessionController(guest: found, store: store) { [weak self] _ in
            withAnimation(Theme.join) { self?.session = nil }
        }
        session = controller
        #if DEBUG && targetEnvironment(simulator)
        if store.demo {
            launched = true
            controller.demoGuest(role: found.role)
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
        guard let password = borrowed.password(for: computer) else {
            sheet = .borrowPassword(computer.id)
            return
        }
        selected = computer.id
        openBorrowSession(BorrowSource(target: target, credentials: .account(login: computer.login, password: password),
                                       name: GuestName.proposed, hostHint: computer.hostName, computer: computer.id))
    }

    /// La session d'un ami qui emprunte : elle emprunte d'abord (« Préparation
    /// de l'ordinateur », « En attente de {hôte}… »), puis se connecte. Une
    /// session à la fois, comme « Se connecter ».
    func openBorrowSession(_ source: BorrowSource) {
        guard session == nil else { return }
        sheet = nil
        let controller = SessionController(borrow: source, store: store) { [weak self] _ in
            withAnimation(Theme.join) { self?.session = nil }
            // La carte de l'accueil dit de nouveau l'état de l'ordinateur.
            if let id = source.computer, let self, let c = self.borrowed.computer(id) { Task { await self.borrowed.refresh(c) } }
        }
        // « Revenir » : le même emprunt, une fois la session fermée.
        controller.onComeBack = { [weak self] source in
            DispatchQueue.main.asyncAfter(deadline: .now() + 0.5) { self?.openBorrowSession(source) }
        }
        session = controller
        #if DEBUG && targetEnvironment(simulator)
        if store.demo {
            launched = true
            let d = UserDefaults.standard
            controller.demoBorrow(waiting: d.bool(forKey: "demoBorrowWaiting"),
                                  until: d.bool(forKey: "demoBorrowOpen") ? nil : Date().addingTimeInterval(80 * 60))
            return
        }
        #endif
        controller.start()
    }

    /// Un QR code ou un lien d'invitation lu dans la fenêtre d'ajout : l'ajout de l'ordinateur prêté prend sa place.
    func invite(fromAddSheet invite: LendInvite) {
        sheet = nil
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.4) { self.sheet = .invite(invite) }
    }

    /// « Retirer » un ordinateur prêté de cet appareil (après la confirmation de l'accueil).
    func removeBorrowed(_ computer: BorrowedComputer) {
        borrowed.remove(computer)
        if selected == computer.id { selected = store.ordered.first?.id ?? borrowed.computers.first?.id }
    }

    /// Un QR code ou un lien de partage lu dans la fenêtre d'ajout : l'écran « Rejoindre » prend sa place.
    func join(fromAddSheet link: ShareJoinLink) {
        sheet = nil
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.4) { self.sheet = .join(link) }
    }

    /// kybernow://adresse:port?name=…&fp=… (QR code ou lien d'Admin,
    /// AirDrop, Messages) : l'ajout s'ouvre, pré-rempli. kybernow://join?… :
    /// l'écran « Rejoindre », le code déjà rempli.
    func open(_ url: URL) {
        guard session == nil, let kind = LinkKind.parse(url.absoluteString) else { return }
        guard case let .computer(candidate) = kind else {
            if case let .join(link) = kind { sheet = .join(link) }
            // kybernow://borrow?… : un ami prête son ordinateur.
            if case let .borrow(invite) = kind { sheet = .invite(invite) }
            return
        }
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
