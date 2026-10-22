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
    let store = HostStore()
    @Published var route: Route = .home
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

    init() {
        // Le cœur Rust : son journal dans le journal unifié, la langue de l'app.
        Core.install()
        store.didSave = { [cloud] in cloud.publish($0) }
        cloud.publish(store.hosts)
    }

    func add(link: Candidate? = nil, tab: AddHostSheet.Tab = .network) {
        pendingLink = link
        addTab = tab
        route = .home
        adding = true
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
            if closing.displayRestored == true, closing.audioRestored == true, closing.error == nil {
                self?.show(String(localized: "\(host.name) a retrouvé ses écrans et son son"))
            } else if closing.displayRestored == false {
                self?.show(String(localized: "Les écrans de \(host.name) ne répondent pas encore : ils reviendront au prochain démarrage de KyberNow Station"), warning: true)
            } else {
                self?.show(String(localized: "Le son de \(host.name) n'est pas encore rétabli : il reviendra au prochain démarrage de KyberNow Station"), warning: true)
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
}

/// Canaux des mises à jour : le flux du site liste toutes les versions, les
/// bêtas marquées `sparkle:channel` beta. Elles ne sont proposées que si
/// « Recevoir les versions bêta » est coché ; une stable plus récente l'est
/// toujours (sans canal, elle est permise à tous).
final class UpdateChannels: NSObject, SPUUpdaterDelegate {
    static let shared = UpdateChannels()
    /// Réglage de l'app (KyberNow Link › Réglages…), dans `UserDefaults`.
    static let betaKey = "receiveBetaUpdates"

    func allowedChannels(for updater: SPUUpdater) -> Set<String> {
        UserDefaults.standard.bool(forKey: Self.betaKey) ? ["beta"] : []
    }
}

@main
struct KyberNowApp: App {
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
    private let updater = SPUStandardUpdaterController(startingUpdater: true, updaterDelegate: UpdateChannels.shared, userDriverDelegate: nil)

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
                Button("Souris : mode jeu ou bureau") { app.session?.toggleMouseCapture() }
                    .keyboardShortcut("m", modifiers: [.control, .option, .command])
                    .disabled(app.session == nil)
                Button("Mesures détaillées") { app.session?.toggleStatsPanel() }
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
