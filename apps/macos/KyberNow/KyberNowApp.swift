// KyberNow Link pour macOS.

import Sparkle
import KyberKit
import SwiftUI

enum Route: Equatable {
    case home
    case settings(UUID)
}

@MainActor
final class AppModel: ObservableObject {
    let store: HostStore
    @Published var route: Route = .home
    /// Ordinateur mis en avant sur l'accueil (le plus récent par défaut).
    @Published var selectedID: UUID?
    /// Rubrique ouverte dans les réglages d'un ordinateur.
    @Published var section: ComputerSection = .picture
    @Published private(set) var session: SessionController?
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

    /// Ordinateurs de démonstration (`-demo YES`, en Debug) : voir `HostStore.loadDemo`.
    static let demo: Bool = {
        #if DEBUG
        UserDefaults.standard.bool(forKey: "demo")
        #else
        false
        #endif
    }()

    init() {
        // Le cœur Rust : son journal dans le journal unifié, la langue de l'app.
        Core.install()
        store = HostStore(demo: Self.demo)
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

    /// Réglages d'un ordinateur, à la rubrique ouverte la dernière fois.
    func openSettings(_ host: Host, section: ComputerSection? = nil) {
        if let section { self.section = section }
        route = .settings(host.id)
    }

    /// kybernow://adresse:port?name=…&fp=… (QR code ou lien de la console).
    func open(_ url: URL) {
        guard session == nil, let candidate = PairingLink.parse(url.absoluteString) else { return }
        if known(candidate) {
            show(String(localized: "\(candidate.name) est déjà dans vos ordinateurs"))
            return
        }
        adding = false
        DispatchQueue.main.async { self.add(link: candidate) }
    }

    private func known(_ c: Candidate) -> Bool {
        store.hosts.contains { ($0.consoleFingerprint != nil && $0.consoleFingerprint == c.expected) || ($0.address == c.address && $0.consolePort == c.port) }
    }

    func connect(_ host: Host) {
        guard session == nil else { return }
        // La session se joue dans la fenêtre de l'accueil.
        let window = NSApp.keyWindow ?? NSApp.mainWindow ?? NSApp.windows.first { $0.isVisible }
        let session = SessionController(host: host, store: store, window: window) { [weak self] _ in
            self?.session = nil
        }
        // La console confirme la remise en état après le retour à l'accueil.
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
        self.session = session
        session.start()
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
        guard Self.demo, session == nil else { return }
        let screen = UserDefaults.standard.string(forKey: "screen")
        switch screen {
        case "connecting", "restarting", "failed", "session", "bar", "stats":
            guard let host = store.ordered.first(where: { $0.id == selectedID }) ?? store.ordered.first else { return }
            DispatchQueue.main.asyncAfter(deadline: .now() + 0.8) { [weak self] in
                guard let self else { return }
                let window = NSApp.keyWindow ?? NSApp.mainWindow ?? NSApp.windows.first { $0.isVisible }
                let session = SessionController(host: host, store: self.store, window: window) { [weak self] _ in self?.session = nil }
                self.session = session
                session.demo(screen ?? "connecting")
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

    @StateObject private var app = AppModel()
    /// Mises à jour (Sparkle) : vérification quotidienne, et à la demande.
    /// Le délégué est gardé par `UpdateChannels.shared` (Sparkle n'en garde
    /// qu'une référence faible).
    private let updater = SPUStandardUpdaterController(startingUpdater: !AppModel.demo, updaterDelegate: UpdateChannels.shared, userDriverDelegate: nil)

    init() {
        Theme.registerFonts()
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
            .onAppear {
                app.checkForUpdates = { [updater] in updater.checkForUpdates(nil) }
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
            CommandMenu("Session") {
                Button("Se déconnecter") { app.session?.end() }
                    .keyboardShortcut("d", modifiers: [.control, .option, .command])
                    .disabled(app.session == nil)
                Button("Plein écran") { app.session?.toggleFullScreen() }
                    .keyboardShortcut("f", modifiers: [.control, .option, .command])
                    .disabled(app.session == nil)
                Button("Souris des jeux ou du bureau") { app.session?.toggleMouseCapture() }
                    .keyboardShortcut("m", modifiers: [.control, .option, .command])
                    .disabled(app.session == nil)
                Button("Statistiques") { app.session?.toggleStatsPanel() }
                    .keyboardShortcut("s", modifiers: [.control, .option, .command])
                    .disabled(app.session == nil)
            }
        }
        // Réglages de l'app (⌘,) ; ceux de connexion vont par ordinateur.
        Settings {
            AppSettingsView(cloud: app.cloud)
        }
    }
}
