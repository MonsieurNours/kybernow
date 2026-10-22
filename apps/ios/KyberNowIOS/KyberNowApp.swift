// KyberNow Link pour l'iPhone et l'iPad.

import KyberKit
import SwiftUI
import UIKit

@main
struct KyberNowApp: App {
    @UIApplicationDelegateAdaptor(AppDelegate.self) private var delegate
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

/// Orientation : la session est toujours en paysage sur iPhone ; ailleurs, libre.
final class AppDelegate: NSObject, UIApplicationDelegate {
    static var orientationLock: UIInterfaceOrientationMask = .all

    func application(_ application: UIApplication, supportedInterfaceOrientationsFor window: UIWindow?) -> UIInterfaceOrientationMask {
        Self.orientationLock
    }

    /// L'écran externe a sa scène à nous (ExternalDisplay) ; la fenêtre de
    /// l'app reste celle de SwiftUI.
    func application(_ application: UIApplication, configurationForConnecting session: UISceneSession, options: UIScene.ConnectionOptions) -> UISceneConfiguration {
        if session.role == .windowExternalDisplayNonInteractive {
            let config = UISceneConfiguration(name: "Écran externe", sessionRole: session.role)
            config.delegateClass = ExternalSceneDelegate.self
            return config
        }
        return UISceneConfiguration(name: nil, sessionRole: session.role)
    }
}

enum Orientation {
    /// iPhone : paysage pendant la session, portrait ou paysage ailleurs.
    @MainActor
    static func lockLandscape(_ landscape: Bool) {
        guard !DeviceDisplay.isPad else { return }
        AppDelegate.orientationLock = landscape ? .landscape : .allButUpsideDown
        for case let scene as UIWindowScene in UIApplication.shared.connectedScenes {
            scene.requestGeometryUpdate(.iOS(interfaceOrientations: AppDelegate.orientationLock)) { _ in }
            scene.keyWindow?.rootViewController?.setNeedsUpdateOfSupportedInterfaceOrientations()
        }
    }
}

struct RootView: View {
    @EnvironmentObject private var app: AppModel

    var body: some View {
        ZStack {
            Theme.background.ignoresSafeArea()
            HomeView()
                .opacity(app.launched ? 1 : 0)
            if !app.launched {
                LaunchView { withAnimation(Theme.join) { app.launched = true } }
                    .transition(.opacity)
            }
            if let session = app.session {
                SessionView(session: session)
                    .transition(.opacity)
                    .zIndex(2)
            }
            if let toast = app.toast {
                VStack {
                    Spacer()
                    Text(toast)
                        .font(Theme.sans(14, .medium))
                        .foregroundStyle(Theme.text)
                        .padding(.horizontal, 18)
                        .padding(.vertical, 12)
                        .glass(16)
                        .padding(.bottom, 24)
                }
                .transition(.move(edge: .bottom).combined(with: .opacity))
                .zIndex(3)
            }
        }
        .sheet(item: $app.sheet) { sheet in
            switch sheet {
            case let .addHost(candidate): AddHostView(link: candidate)
            case let .settings(id): SettingsView(hostID: id)
            case .appSettings: AppSettingsView()
            }
        }
        .onChange(of: app.session != nil, initial: true) { _, inSession in
            Orientation.lockLandscape(inSession)
        }
    }
}
