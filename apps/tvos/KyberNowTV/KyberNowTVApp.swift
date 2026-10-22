// KyberNow Link pour l'Apple TV : vos ordinateurs sur le téléviseur, à la
// télécommande ou à la manette. Maquette : https://claude.ai/artifact/RfGbfguTyAP1jjwFASVRyK

import KyberKit
import SwiftUI

@main
struct KyberNowTVApp: App {
    @StateObject private var app = AppModel()

    init() {
        Theme.registerFonts()
        // Le cœur Rust : son journal dans le journal unifié, la langue de l'app.
        Core.install()
        _ = GamepadBridge.shared
        // Décodeurs sondés et pipeline Metal compilé hors du fil principal, avant le premier flux.
        DispatchQueue.global(qos: .userInitiated).async { DecoderCapabilities.prepare() }
    }

    var body: some Scene {
        WindowGroup {
            RootView()
                .environmentObject(app)
                .environmentObject(app.store)
                .environmentObject(app.store.imagery)
                .environmentObject(app.cloud)
                .environmentObject(app.discovery)
                .environmentObject(GamepadBridge.shared)
                .preferredColorScheme(.dark)
                .onOpenURL { app.open($0) }
        }
    }
}

struct RootView: View {
    @EnvironmentObject private var app: AppModel

    var body: some View {
        ZStack {
            Theme.background.ignoresSafeArea()
            switch app.screen {
            case .home:
                HomeView()
                    .transition(.opacity)
            case .addHost:
                AddHostView()
                    .transition(.opacity.combined(with: .scale(scale: 1.02)))
            case let .settings(id):
                SettingsView(hostID: id)
                    .transition(.opacity.combined(with: .scale(scale: 1.02)))
            case .gamepads:
                GamepadsView()
                    .transition(.opacity.combined(with: .scale(scale: 1.02)))
            case .session:
                if let session = app.session {
                    SessionView(session: session)
                        .transition(.identity)
                }
            }
        }
        .ignoresSafeArea()
        .animation(Theme.join, value: app.screen)
    }
}
