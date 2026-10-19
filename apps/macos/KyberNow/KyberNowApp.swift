// KyberNow pour macOS.

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

    func add(link: Candidate? = nil) {
        pendingLink = link
        route = .home
        adding = true
    }

    /// kybernow://adresse:port?name=…&fp=… (QR code ou lien de la console).
    func open(_ url: URL) {
        guard session == nil, let candidate = PairingLink.parse(url.absoluteString) else { return }
        if known(candidate) {
            show("\(candidate.name) est déjà dans vos ordinateurs")
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
        let session = SessionController(host: host, store: store) { [weak self] finished in
            guard let self else { return }
            if finished.closing.displayRestored == true {
                self.show("Session terminée · \(finished.host.name) a retrouvé ses écrans")
            }
            self.session = nil
        }
        self.session = session
        session.start()
    }

    func show(_ text: String) {
        withAnimation { toast = text }
        DispatchQueue.main.asyncAfter(deadline: .now() + 4) { [weak self] in
            withAnimation { self?.toast = nil }
        }
    }
}

@main
struct KyberNowApp: App {
    @StateObject private var app = AppModel()

    init() {
        Theme.registerFonts()
    }

    var body: some Scene {
        WindowGroup("KyberNow") {
            Group {
                switch app.route {
                case .home: HomeView()
                case let .settings(id): SettingsView(hostID: id)
                }
            }
            .animation(.easeInOut(duration: 0.25), value: app.route)
            .environmentObject(app)
            .onOpenURL { app.open($0) }
            .onReceive(app.store.objectWillChange) { app.objectWillChange.send() }
        }
        .windowStyle(.hiddenTitleBar)
        .commands {
            CommandMenu("Session") {
                Button("Se déconnecter") { app.session?.end() }
                    .keyboardShortcut("d", modifiers: [.control, .option, .command])
                    .disabled(app.session == nil)
                Button("Plein écran") { app.session?.toggleFullScreen() }
                    .keyboardShortcut("f", modifiers: [.control, .option, .command])
                    .disabled(app.session == nil)
                Button("Reprendre ou rendre la souris") { app.session?.toggleMouseCapture() }
                    .keyboardShortcut("m", modifiers: [.control, .option, .command])
                    .disabled(app.session == nil)
            }
        }
    }
}
