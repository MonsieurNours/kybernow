// KyberNow Link pour macOS.

import Combine
import Sparkle
import KyberKit
import SwiftUI

enum Route: Equatable {
    case home
    case settings(UUID)
}

@MainActor
final class AppModel: ObservableObject {
    /// L'app n'a qu'un modèle : la scène SwiftUI, le délégué de l'app (icône
    /// de la barre des menus, notifications) et les fenêtres le partagent.
    static let shared = AppModel()

    let store: HostStore
    /// Les ordinateurs que des amis prêtent à ce Mac (docs/pret.md), à part des siens.
    let borrowed: BorrowedStore
    @Published var route: Route = .home
    /// Ordinateur mis en avant sur l'accueil (le plus récent par défaut).
    @Published var selectedID: UUID?
    /// Rubrique ouverte dans les réglages d'un ordinateur.
    @Published var section: ComputerSection = .picture
    /// Les sessions, une fenêtre chacune (plusieurs ordinateurs à la fois, Sessions.swift).
    let sessions = SessionManager()
    @Published var toast: String?
    /// Feuille « Ajouter un ordinateur », avec le lien kybernow:// qui l'a ouverte.
    @Published var adding = false
    var pendingLink: Candidate?
    var addTab: AddHostSheet.Tab = .network
    /// Ordinateurs annoncés sur le réseau local (Bonjour), montrés sur l'accueil.
    let discovery = Discovery()
    /// Ordinateurs et accès publiés dans iCloud pour l'Apple TV.
    let cloud = CloudPublisher()
    /// « Version {v} disponible » : la vérification de Sparkle, à la demande.
    var checkForUpdates: () -> Void = {}
    /// La fenêtre de l'accueil, disponible aussi pendant les sessions (elles
    /// ont chacune la leur). Fermée, elle n'est que cachée : l'app continue
    /// dans la barre des menus (AppLife.swift).
    weak var homeWindow: NSWindow?
    /// Préférences de l'app (celles de la démonstration à part).
    let defaults: UserDefaults
    /// Notifications du système (Notifications.swift).
    let notifier = Notifier()
    /// Délégué de la fenêtre de l'accueil (fermer la cache), gardé ici : la fenêtre n'en garde qu'une référence faible.
    let homeDelegate = HomeWindowDelegate()
    /// Lancée à l'ouverture de session : la fenêtre reste cachée au démarrage.
    var startHidden = false
    /// Fin de l'app en attente de la fin des sessions : chaque fin dans le cœur l'appelle.
    var afterSessionEnded: ((SessionController) -> Void)?
    var watchers = Set<AnyCancellable>()

    /// Ordinateurs de démonstration (`-demo YES`, en Debug) : voir `HostStore.loadDemo`.
    static let demo: Bool = {
        #if DEBUG
        UserDefaults.standard.bool(forKey: "demo")
        #else
        false
        #endif
    }()

    private init() {
        // Le cœur Rust : son journal dans le journal unifié, la langue de l'app.
        Core.install()
        store = HostStore(demo: Self.demo)
        borrowed = BorrowedStore(demo: Self.demo)
        defaults = Self.demo ? (UserDefaults(suiteName: "fr.leslignesclaires.kybernow.demo") ?? .standard) : .standard
        // Les sessions (ouvertes, fermées, active) redessinent l'accueil et les menus.
        sessions.objectWillChange.sink { [weak self] _ in self?.objectWillChange.send() }.store(in: &watchers)
        borrowed.objectWillChange.sink { [weak self] _ in self?.objectWillChange.send() }.store(in: &watchers)
        #if DEBUG
        if Self.demo {
            // Rien vers iCloud : les ordinateurs de démonstration restent sur ce Mac.
            store.loadDemo()
            applyDemoArguments()
            return
        }
        #endif
        store.didSave = { [cloud] in cloud.publish($0) }
        cloud.publish(store.hosts)
    }

    func add(link: Candidate? = nil, tab: AddHostSheet.Tab = .network) {
        pendingLink = link
        addTab = tab
        route = .home
        adding = true
    }

    /// Feuille « Rejoindre un partage » (KyberNow Share), avec le lien
    /// `kybernow://join?…` qui l'a ouverte, s'il y en a un.
    @Published var joining = false
    var pendingJoin: ShareJoinLink?

    /// « Rejoindre avec un code » : l'écran du code et du prénom, par-dessus
    /// l'accueil (aussi pendant les sessions).
    func join(link: ShareJoinLink? = nil) {
        pendingJoin = link
        route = .home
        showWindow()
        joining = true
    }

    /// Un QR code ou un lien de partage lu dans la feuille d'ajout : elle se
    /// ferme, puis l'écran « Rejoindre » s'ouvre à sa place.
    func join(fromAddSheet link: ShareJoinLink) {
        adding = false
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.4) { self.join(link: link) }
    }

    /// Réglages d'un ordinateur, à la rubrique ouverte la dernière fois.
    func openSettings(_ host: Host, section: ComputerSection? = nil) {
        if let section { self.section = section }
        route = .settings(host.id)
    }

    /// kybernow://adresse:port?name=…&fp=… (QR code ou lien de la console).
    func open(_ url: URL) {
        guard let kind = LinkKind.parse(url.absoluteString) else { return }
        // kybernow://join?… : un partage à rejoindre (le code est déjà dans le lien).
        guard case let .computer(candidate) = kind else {
            if case let .join(link) = kind {
                adding = false
                joining = false
                DispatchQueue.main.asyncAfter(deadline: .now() + 0.2) { self.join(link: link) }
            }
            // kybernow://borrow?… : un ami prête son ordinateur.
            if case let .borrow(invite) = kind { openInvite(invite) }
            return
        }
        // Fenêtre cachée (l'app vivait dans la barre des menus) : elle revient pour l'ajout.
        showWindow()
        if known(candidate) {
            show(String(localized: "\(candidate.name) est déjà dans vos ordinateurs"))
            return
        }
        adding = false
        DispatchQueue.main.async { self.add(link: candidate) }
    }

    // MARK: Prêter l'ordinateur (l'ami)

    /// Le lien d'invitation d'un ami qui prête son ordinateur, à son compte d'ami.
    @Published var inviting: LendInvite?
    /// Le mot de passe d'un ordinateur prêté, demandé à « Emprunter ».
    @Published var borrowPassword: BorrowedComputer?

    /// « {hôte} vous prête son ordinateur. » : l'ajout, identifiant rempli.
    func openInvite(_ invite: LendInvite) {
        showWindow()
        route = .home
        adding = false
        joining = false
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.3) { self.inviting = invite }
    }

    /// Un QR code ou un lien d'invitation lu dans la feuille d'ajout : elle se
    /// ferme, l'ajout de l'ordinateur prêté s'ouvre à sa place.
    func openInvite(fromAddSheet invite: LendInvite) {
        adding = false
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.4) { self.inviting = invite }
    }

    /// « Emprunter » (« Demander à {hôte} », « Revenir ») : la session
    /// d'emprunt, avec le compte d'ami ; sans mot de passe retenu, il est demandé.
    func borrow(_ computer: BorrowedComputer) {
        if let session = sessions.all.first(where: { $0.borrowSource?.computer == computer.id }) {
            session.bringToFront()
            return
        }
        guard let target = computer.target else { return }
        guard let password = borrowed.password(for: computer) else {
            borrowPassword = computer
            return
        }
        openBorrowSession(BorrowSource(target: target, credentials: .account(login: computer.login, password: password),
                                       name: GuestName.proposed, hostHint: computer.hostName, computer: computer.id))
    }

    /// « Retirer » un ordinateur prêté de cet appareil.
    func removeBorrowed(_ computer: BorrowedComputer) {
        let alert = NSAlert()
        alert.messageText = StationText.removeQuestion(computer.title)
        alert.addButton(withTitle: ShareText.remove)
        alert.addButton(withTitle: StationText.cancel)
        guard alert.runModal() == .alertFirstButtonReturn else { return }
        borrowed.remove(computer)
        if selectedID == computer.id { selectedID = nil }
    }

    /// La session d'un ami qui emprunte, dans sa fenêtre comme toute session
    /// (comptée parmi les quatre) : elle emprunte d'abord (« Préparation de
    /// l'ordinateur », « En attente de {hôte}… »), puis se connecte.
    func openBorrowSession(_ source: BorrowSource) {
        joining = false
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.4) { [self] in
            guard !sessions.isFull else { return }
            let first = sessions.isEmpty
            let opening: SessionController.Opening = first ? .first(home: homeWindow) : .next(anchor: sessions.anchorWindow)
            let session = SessionController(borrow: source, store: store, opening: opening) { [weak self] in self?.sessionFinished($0) }
            session.onWindowClosed = { [weak self] in self?.sessionWindowClosed($0) }
            session.onEnded = { [weak self] in
                self?.sessionEnded($0)
                // La carte de l'accueil dit de nouveau l'état de l'ordinateur.
                if let id = source.computer, let c = self?.borrowed.computer(id) { Task { await self?.borrowed.refresh(c) } }
            }
            if !first { Dock.comeForward() }
            #if DEBUG
            if Self.demo, !UserDefaults.standard.bool(forKey: "demoCore") {
                let args = UserDefaults.standard
                session.demoBorrow(waiting: args.bool(forKey: "demoBorrowWaiting"),
                                   until: args.bool(forKey: "demoBorrowOpen") ? nil : Date().addingTimeInterval(80 * 60))
                sessions.add(session)
                sessions.refresh()
                hideHomeForFirstSession(first)
                return
            }
            #endif
            session.start()
            sessions.add(session)
            sessions.refresh()
            hideHomeForFirstSession(first)
        }
    }

    private func known(_ c: Candidate) -> Bool {
        store.hosts.contains { ($0.consoleFingerprint != nil && $0.consoleFingerprint == c.expected) || ($0.address == c.address && $0.consolePort == c.port) }
    }

    /// « Se connecter » : la session de cet ordinateur dans sa fenêtre. Déjà
    /// en session, sa fenêtre passe devant ; quatre sessions, rien. La première
    /// prend la place de l'accueil (plein écran, ou tous les écrans), les
    /// suivantes s'ouvrent dans une fenêtre par-dessus (« Cet écran »).
    func connect(_ host: Host) {
        openSession(host)
    }

    private func openSession(_ host: Host, demo: String? = nil) {
        if let existing = sessions.session(for: host.id) {
            existing.bringToFront()
            return
        }
        guard !sessions.isFull else { return }
        let first = sessions.isEmpty
        let opening: SessionController.Opening = first ? .first(home: homeWindow) : .next(anchor: sessions.anchorWindow)
        let session = SessionController(host: host, store: store, opening: opening) { [weak self] in self?.sessionFinished($0) }
        session.onWindowClosed = { [weak self] in self?.sessionWindowClosed($0) }
        session.onEnded = { [weak self] in self?.sessionEnded($0) }
        // La console confirme la remise en état après la fin de la session.
        session.onReport = { [weak self] host, closing in
            let elide = French.elides(host.name)
            if closing.displayRestored == true, closing.audioRestored == true, closing.error == nil {
                self?.show(String(localized: "\(host.name) a retrouvé ses écrans et son son"))
            } else if closing.displayRestored == false {
                self?.show(elide
                    ? String(localized: "Les écrans d'\(host.name) ne répondent pas encore : ils reviendront au prochain démarrage de KyberNow Station")
                    : String(localized: "Les écrans de \(host.name) ne répondent pas encore : ils reviendront au prochain démarrage de KyberNow Station"), warning: true)
            } else {
                self?.show(elide
                    ? String(localized: "Le son d'\(host.name) n'est pas encore rétabli : il reviendra au prochain démarrage de KyberNow Station")
                    : String(localized: "Le son de \(host.name) n'est pas encore rétabli : il reviendra au prochain démarrage de KyberNow Station"), warning: true)
            }
        }
        if !first { Dock.comeForward() }
        // Sa fenêtre s'ouvre (l'accueil photographié pour le passage tel qu'il
        // était, avant « Afficher la session »), puis elle rejoint les sessions.
        #if DEBUG
        // Démonstration : la session sans ordinateur derrière, rien sur le réseau
        // (`-demoCore YES` : une vraie session du cœur vers une adresse que rien
        // ne route, pour essayer le registre des sessions).
        if Self.demo, !UserDefaults.standard.bool(forKey: "demoCore") {
            session.demo(demo ?? "session")
            sessions.add(session)
            sessions.refresh()
            hideHomeForFirstSession(first)
            return
        }
        #endif
        // Inscrite au registre des sessions du cœur à son départ : elle devient active.
        session.start()
        sessions.add(session)
        sessions.refresh()
        hideHomeForFirstSession(first)
    }

    /// KyberNow Share : la session d'un invité, dans sa fenêtre comme toute
    /// session (la première prend la place de l'accueil, les suivantes
    /// s'ouvrent par-dessus), comptée parmi les quatre. Aucun ordinateur
    /// n'est ajouté à la liste de cet appareil. La feuille « Rejoindre » se
    /// ferme d'abord : l'accueil est photographié sans elle pour le passage.
    func openGuestSession(_ found: ShareFound) {
        joining = false
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.4) { [self] in
            // Quatre ordinateurs au plus, invités compris : une session ouverte
            // entre-temps a pris la dernière place, l'invité repart tout de suite.
            guard !sessions.isFull else {
                Share.leave(found)
                return
            }
            let first = sessions.isEmpty
            let opening: SessionController.Opening = first ? .first(home: homeWindow) : .next(anchor: sessions.anchorWindow)
            let session = SessionController(guest: found, store: store, opening: opening) { [weak self] in self?.sessionFinished($0) }
            session.onWindowClosed = { [weak self] in self?.sessionWindowClosed($0) }
            session.onEnded = { [weak self] in self?.sessionEnded($0) }
            if !first { Dock.comeForward() }
            #if DEBUG
            if Self.demo, !UserDefaults.standard.bool(forKey: "demoCore") {
                session.demoGuest(role: found.role)
                sessions.add(session)
                sessions.refresh()
                hideHomeForFirstSession(first)
                return
            }
            #endif
            session.start()
            sessions.add(session)
            sessions.refresh()
            hideHomeForFirstSession(first)
        }
    }

    /// La première session a pris la place de l'accueil : il se cache
    /// derrière elle, la dernière session le ramènera.
    private func hideHomeForFirstSession(_ first: Bool) {
        guard first, let home = homeWindow, home.isVisible, !home.styleMask.contains(.fullScreen) else { return }
        home.orderOut(nil)
        sessions.homeHidden = true
        windowsChanged()
    }

    @Published var toastWarning = false

    func show(_ text: String, warning: Bool = false) {
        toastWarning = warning
        withAnimation { toast = text }
        DispatchQueue.main.asyncAfter(deadline: .now() + (warning ? 8 : 4)) { [weak self] in
            withAnimation { self?.toast = nil }
        }
    }

    #if DEBUG
    /// Démonstration : `-hostIndex 0|1|2` (ordinateur choisi, dans l'ordre
    /// de l'accueil), `-screen settings|app|connecting|failed|session`,
    /// `-section picture|sound|controls|network|statistics|station`,
    /// `-demoUpdate 0.5.1` (mise à jour de l'app proposée).
    private func applyDemoArguments() {
        let args = UserDefaults.standard
        let ordered = store.ordered
        let index = args.integer(forKey: "hostIndex")
        if ordered.indices.contains(index) { selectedID = ordered[index].id }
        // Prêter l'ordinateur : `-demoBorrowed free|busy|lent|mine|locked|off|offline|refused|slot|slot-now`
        // (« Ordinateur de Matthieu » sur l'accueil, mis en avant avec `-demoBorrowedSelected YES`) ;
        // `-demoLent YES|notice` : la Tour prêtée à Léa (« Prêté à Léa », « Reprendre l'ordinateur »).
        if let state = args.string(forKey: "demoBorrowed") {
            borrowed.loadDemo(state)
            if args.bool(forKey: "demoBorrowedSelected") { selectedID = borrowed.computers.first?.id }
        }
        if let lent = args.string(forKey: "demoLent"), let tour = ordered.first {
            store.setDemoLend(LendDemo.lentView(notice: lent == "notice"), for: tour.id)
        }
        if let raw = args.string(forKey: "section"), let s = ComputerSection(rawValue: raw) { section = s }
        if args.string(forKey: "screen") == "settings", let id = selectedID ?? ordered.first?.id { route = .settings(id) }
        if let v = args.string(forKey: "demoUpdate") { UpdateChannels.shared.available = v }
        // `-demoWake YES` : le réveil de l'ordinateur choisi lancé d'emblée.
        if args.bool(forKey: "demoWake"), let host = store.ordered.first(where: { $0.id == selectedID }) ?? ordered.first {
            DispatchQueue.main.asyncAfter(deadline: .now() + 0.5) { [store] in store.wake(host) }
        }
        // `-stationAction restart|update` : l'action lancée d'emblée (confirmation, état en cours).
        if let action = args.string(forKey: "stationAction"), let host = store.ordered.first(where: { $0.id == selectedID }) ?? ordered.first {
            DispatchQueue.main.asyncAfter(deadline: .now() + 1.2) { [store] in
                if action == "update" { store.updateStation(host) } else { store.restartStation(host) }
            }
        }
    }

    /// Démonstration : `-demoWindow 1100x760`, taille du contenu de la fenêtre ;
    /// `-demoSnapshot <fichier.png> -demoDelay 4` : la fenêtre (ou celle des
    /// réglages de l'app, `-screen app`) dessinée dans un PNG, puis l'app se
    /// ferme (vérification des écrans même quand l'écran du Mac est verrouillé).
    func demoWindow() {
        guard Self.demo else { return }
        let args = UserDefaults.standard
        if let size = args.string(forKey: "demoWindow")?.split(separator: "x").compactMap({ Double($0) }), size.count == 2 {
            DispatchQueue.main.asyncAfter(deadline: .now() + 0.3) {
                guard let window = NSApp.windows.first(where: { $0.canBecomeMain }) else { return }
                window.setContentSize(NSSize(width: size[0], height: size[1]))
                window.center()
            }
        }
        if let path = args.string(forKey: "demoSnapshot") {
            let delay = args.double(forKey: "demoDelay") > 0 ? args.double(forKey: "demoDelay") : 4
            DispatchQueue.main.asyncAfter(deadline: .now() + delay) {
                let settings = args.string(forKey: "screen") == "app"
                let windows = NSApp.windows.filter { $0.contentView != nil && $0.frame.width > 300 }
                let window = settings ? windows.last(where: { !$0.canBecomeMain || $0.title != "KyberNow Link" }) ?? windows.last : windows.first(where: { $0.canBecomeMain })
                if let view = window?.contentView?.superview ?? window?.contentView,
                   let rep = view.bitmapImageRepForCachingDisplay(in: view.bounds) {
                    view.cacheDisplay(in: view.bounds, to: rep)
                    try? rep.representation(using: .png, properties: [:])?.write(to: URL(fileURLWithPath: path))
                }
                NSApp.terminate(nil)
            }
        }
    }

    /// Démonstration, une fois l'accueil affiché : la connexion (sans
    /// ordinateur derrière), son échec, la session, les réglages de l'app.
    func demoAppeared() {
        guard Self.demo, sessions.isEmpty else { return }
        let screen = UserDefaults.standard.string(forKey: "screen")
        switch screen {
        case "connecting", "restarting", "failed", "session", "bar", "stats":
            guard let host = store.ordered.first(where: { $0.id == selectedID }) ?? store.ordered.first else { return }
            DispatchQueue.main.asyncAfter(deadline: .now() + 0.8) { [weak self] in
                self?.openSession(host, demo: screen)
            }
        default: break
        }
    }
    #endif
}

/// Canaux des mises à jour : le flux du site liste toutes les versions, les
/// bêtas marquées `sparkle:channel` beta. Elles ne sont proposées que si
/// « Recevoir les versions bêta » est coché ; une stable plus récente l'est
/// toujours (sans canal, elle est permise à tous).
final class UpdateChannels: NSObject, SPUUpdaterDelegate, ObservableObject {
    static let shared = UpdateChannels()
    /// Réglage de l'app (KyberNow Link › Réglages…), dans `UserDefaults`.
    static let betaKey = "receiveBetaUpdates"
    /// Version trouvée par la dernière vérification (« Version {v} disponible »
    /// en haut de l'accueil), nil si l'app est à jour.
    @Published var available: String?

    func allowedChannels(for updater: SPUUpdater) -> Set<String> {
        UserDefaults.standard.bool(forKey: Self.betaKey) ? ["beta"] : []
    }

    func updater(_ updater: SPUUpdater, didFindValidUpdate item: SUAppcastItem) {
        let version = item.displayVersionString
        DispatchQueue.main.async { self.available = version }
    }

    func updaterDidNotFindUpdate(_ updater: SPUUpdater) {
        DispatchQueue.main.async { self.available = nil }
    }
}

/// Les Mac installés avant la 0.4.3 ont l'app sous « KyberNow.app » : Sparkle
/// met à jour le paquet en place, sans le renommer. Au lancement, avant que
/// Sparkle démarre, l'app prend son nom « KyberNow Link.app » et se relance
/// depuis ce chemin. Seulement dans un dossier Applications (pas un build, une
/// image disque ou un dossier translocalisé), si le dossier est modifiable et
/// le nom libre ; en cas d'échec, l'app reprend son nom et continue.
enum BundleRename {
    static let ran: Void = run()

    private static func run() {
        let fm = FileManager.default
        let old = Bundle.main.bundleURL
        let dir = old.deletingLastPathComponent()
        let new = dir.appendingPathComponent("KyberNow Link.app")
        guard old.lastPathComponent == "KyberNow.app", dir.lastPathComponent == "Applications",
              !fm.fileExists(atPath: new.path), fm.isWritableFile(atPath: dir.path) else { return }
        do { try fm.moveItem(at: old, to: new) } catch { return }
        let open = Process()
        open.executableURL = URL(fileURLWithPath: "/usr/bin/open")
        open.arguments = ["-n", new.path]
        if (try? open.run()) != nil {
            open.waitUntilExit()
            if open.terminationStatus == 0 { exit(0) }
        }
        try? fm.moveItem(at: new, to: old)
    }
}

@main
struct KyberNowApp: App {
    /// Avant toute autre propriété (Sparkle compris) : voir `BundleRename`.
    private let renamed: Void = BundleRename.ran

    @ViewBuilder private var route: some View {
        switch app.route {
        case .home: HomeView()
        case let .settings(id): SettingsView(hostID: id)
        }
    }

    /// Icône de la barre des menus, Dock, démarrage, fin de l'app (AppLife.swift).
    @NSApplicationDelegateAdaptor(AppDelegate.self) private var delegate
    @StateObject private var app = AppModel.shared
    /// Mises à jour (Sparkle) : vérification quotidienne, et à la demande.
    /// Le délégué est gardé par `UpdateChannels.shared` (Sparkle n'en garde
    /// qu'une référence faible).
    private let updater = SPUStandardUpdaterController(startingUpdater: !AppModel.demo, updaterDelegate: UpdateChannels.shared, userDriverDelegate: nil)

    init() {
        Theme.registerFonts()
        AppModel.shared.checkForUpdates = { [updater] in updater.checkForUpdates(nil) }
        // Décodeurs sondés et pipeline Metal compilé hors du fil principal, avant le premier flux.
        DispatchQueue.global(qos: .userInitiated).async { DecoderCapabilities.prepare() }
    }

    var body: some Scene {
        WindowGroup("KyberNow Link") {
            Group {
                #if DEBUG
                if ProcessInfo.processInfo.environment["KN_STATS_PREVIEW"] != nil {
                    StatsPanelPreview()
                } else {
                    route
                }
                #else
                route
                #endif
            }
            .animation(.easeInOut(duration: 0.25), value: app.route)
            .environmentObject(app)
            .onOpenURL { app.open($0) }
            .onReceive(app.store.objectWillChange) { app.objectWillChange.send() }
            // La fenêtre de l'accueil : fermée, elle se cache (AppLife.swift) ; un
            // lien kybernow:// la reprend au lieu d'en ouvrir une autre.
            .background(WindowReader { app.attach(window: $0) })
            .handlesExternalEvents(preferring: ["*"], allowing: ["*"])
            .onAppear {
                #if DEBUG
                app.demoWindow()
                #endif
            }
        }
        .windowStyle(.hiddenTitleBar)
        .commands {
            CommandGroup(after: .appInfo) {
                Button("Rechercher des mises à jour…") { updater.checkForUpdates(nil) }
            }
            // Une seule fenêtre d'accueil : « Ouvrir KyberNow Link » (barre des menus) la montre.
            CommandGroup(replacing: .newItem) {}
            // Les commandes vont à la session de la fenêtre clé (même en échec),
            // sinon à la session active du registre ; relue au moment du clic.
            CommandMenu("Session") {
                let target = app.sessions.commandTarget
                // Un invité quitte le partage (docs/link-ui.md, « Partage »).
                Button(target?.isGuest == true ? ShareText.leave : String(localized: "Se déconnecter")) {
                    guard let s = app.sessions.commandTarget else { return }
                    if s.isLive { s.end() } else { s.dismissFailure() }
                }
                .keyboardShortcut("d", modifiers: [.control, .option, .command])
                .disabled(target == nil)
                // Plusieurs écrans : pas de « Plein écran » (docs/link-ui.md).
                if target?.canToggleFullScreen ?? true {
                    Button("Plein écran") { app.sessions.commandTarget?.toggleFullScreen() }
                        .keyboardShortcut("f", modifiers: [.control, .option, .command])
                        .disabled(target?.isLive != true)
                }
                // La souris seulement avec le clavier et la souris (invité : s'ils lui sont donnés).
                Button("Souris des jeux ou du bureau") { app.sessions.commandTarget?.toggleMouseCapture() }
                    .keyboardShortcut("m", modifiers: [.control, .option, .command])
                    .disabled(target?.isLive != true || target?.hasKeyboardAndMouse != true)
                Button("Statistiques") { app.sessions.commandTarget?.toggleStatsPanel() }
                    .keyboardShortcut("s", modifiers: [.control, .option, .command])
                    .disabled(target?.isLive != true)
                // Plusieurs ordinateurs : la fenêtre de l'ordinateur suivant, dans l'ordre d'ouverture.
                Button("Ordinateur suivant") { app.sessions.showNext() }
                    .keyboardShortcut("o", modifiers: [.control, .option, .command])
                    .disabled(app.sessions.count < 2)
            }
        }
        // Réglages de l'app (⌘,) ; ceux de connexion vont par ordinateur.
        Settings {
            AppSettingsView(cloud: app.cloud)
        }
    }
}
