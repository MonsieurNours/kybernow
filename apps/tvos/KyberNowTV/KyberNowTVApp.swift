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
        // Décodeurs sondés (PyroWave compris : une image de test décodée par le GPU)
        // et pipeline Metal compilé hors du fil principal, avant le premier flux.
        DispatchQueue.global(qos: .userInitiated).async { DecoderCapabilities.prepare() }
        // Câble Ethernet ou non (« Auto » et PyroWave, docs/pyrowave-client-apple.md).
        NetworkLink.start()
    }

    var body: some Scene {
        WindowGroup {
            RootView()
                .environmentObject(app)
                .environmentObject(app.store)
                .environmentObject(app.accounts)
                .environmentObject(app.borrowed)
                .environmentObject(app.playstation)
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
            case .hdrCalibration:
                HDRCalibrationView()
                    .transition(.opacity)
            case .session:
                if let session = app.session {
                    SessionView(session: session)
                        .transition(.identity)
                }
            case .console:
                if let session = app.consoleSession {
                    ConsoleSessionView(session: session)
                        .transition(.opacity)
                }
            case let .consoleSettings(id):
                ConsoleSettingsView(consoleID: id)
                    .transition(.opacity.combined(with: .scale(scale: 1.02)))
            case let .consolePair(request):
                ConsolePairView(request: request)
                    .transition(.opacity.combined(with: .scale(scale: 1.02)))
            case .accounts:
                AccountsView()
                    .transition(.opacity.combined(with: .scale(scale: 1.02)))
            case let .accountCode(relay):
                // Un autre relais : un autre écran (son propre modèle, son propre code).
                AccountCodeView(accounts: app.accounts, relay: relay)
                    .id(relay)
                    .transition(.opacity.combined(with: .scale(scale: 1.02)))
            case .accountRelay:
                AccountRelayView()
                    .transition(.opacity.combined(with: .scale(scale: 1.02)))
            }
        }
        // Le panneau du test du profil Auto (plein écran), de l'accueil ou des réglages.
        .modifier(AutoTestPresenter(model: app.autoTest))
        // Après une mise à jour de l'app : les Stations à mettre à jour aussi
        // (LinkUI.swift), en plein écran, depuis l'accueil seulement.
        .modifier(StationNoticePresenter(store: app.store, free: app.screen == .home))
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
        .overlay {
            // `-demoHDR YES` : la mire PQ par le vrai rendu, par-dessus tout.
            if let demo = app.hdrDemo {
                HDRDemoView(demo: demo).ignoresSafeArea()
            }
        }
        .ignoresSafeArea()
        .animation(Theme.join, value: app.screen)
        .animation(Theme.join, value: app.notice)
    }
}
