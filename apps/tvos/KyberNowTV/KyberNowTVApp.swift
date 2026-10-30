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
                .environmentObject(app.borrowed)
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
            case let .join(link):
                JoinView(link: link)
                    .transition(.opacity.combined(with: .scale(scale: 1.02)))
            case let .invite(invite):
                FriendAccountView(invite: invite)
                    .transition(.opacity.combined(with: .scale(scale: 1.02)))
            case let .borrowPassword(id):
                if let computer = app.borrowed.computer(id) {
                    FriendAccountView(computer: computer)
                        .transition(.opacity.combined(with: .scale(scale: 1.02)))
                }
            case let .settings(id):
                SettingsView(hostID: id)
                    .transition(.opacity.combined(with: .scale(scale: 1.02)))
            case .appSettings:
                AppSettingsView()
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
        .overlay(alignment: .top) {
            if let notice = app.notice {
                HStack(spacing: 14) {
                    Icon("warning", size: 28).foregroundStyle(Theme.warn)
                    Text(notice).font(Theme.sans(24)).foregroundStyle(Theme.text)
                }
                .padding(.horizontal, 30).frame(height: 76)
                .glass(38)
                .padding(.top, 60)
                .transition(.opacity.combined(with: .offset(y: -14)))
            }
        }
        .ignoresSafeArea()
        .animation(Theme.join, value: app.screen)
        .animation(Theme.join, value: app.notice)
    }
}
