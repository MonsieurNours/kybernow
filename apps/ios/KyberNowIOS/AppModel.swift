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
    /// Ordinateur à ouvrir une fois l'accord donné : l'appareil est en
    /// données mobiles et « Sessions en données mobiles » est coupé.
    @Published var mobileDataAsk: UUID?

    let store: HostStore
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
        guard demo else { return }
        store.loadDemo()
        let list = store.ordered
        let index = UserDefaults.standard.integer(forKey: "hostIndex")
        selected = list.indices.contains(index) ? list[index].id : list.first?.id
        if let host = selectedHost { demoPad(host) }
        if UserDefaults.standard.bool(forKey: "skipLaunch") { launched = true }
        // `-demoWaking YES` : le réveil de l'ordinateur choisi, déjà lancé.
        if UserDefaults.standard.bool(forKey: "demoWaking"), let host = selectedHost { store.wake(host) }
        switch UserDefaults.standard.string(forKey: "screen") {
        case "add": sheet = .addHost(nil)
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

    /// kybernow://adresse:port?name=…&fp=… (QR code ou lien d'Admin,
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
