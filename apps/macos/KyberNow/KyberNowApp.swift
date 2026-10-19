// KyberNow pour macOS.

import SwiftUI

@MainActor
final class AppModel: ObservableObject {
    let store = HostStore()
    @Published private(set) var session: SessionController?
    @Published var toast: String?

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

    private func show(_ text: String) {
        withAnimation { toast = text }
        DispatchQueue.main.asyncAfter(deadline: .now() + 4) { [weak self] in
            withAnimation { self?.toast = nil }
        }
    }
}

@main
struct KyberNowApp: App {
    @StateObject private var app = AppModel()

    var body: some Scene {
        WindowGroup("KyberNow") {
            HomeView()
                .environmentObject(app)
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
            }
        }
    }
}
