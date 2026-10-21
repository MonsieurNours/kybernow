// Écran externe (maquettes iOS, planche 5) : un écran branché en USB-C ou
// choisi en AirPlay, sur iPhone comme sur iPad. iOS donne à l'app une scène
// « écran externe, non interactive » ; on y pose une fenêtre à nous plutôt
// que de laisser l'écran recopier celui de l'appareil.
//
// Hors session, l'écran externe montre le symbole et une phrase d'attente.
// En session, l'image de l'ordinateur y passe (StreamView.showImage(in:)),
// à la définition de cet écran (« comme cet écran » vise alors l'externe) ;
// l'appareil devient le pupitre. Débrancher l'écran ramène l'image sur
// l'appareil sans couper la session.

import KyberKit
import SwiftUI
import UIKit

@MainActor
final class ExternalDisplay: ObservableObject {
    static let shared = ExternalDisplay()

    /// La scène de l'écran externe, tant qu'il est branché.
    @Published private(set) var scene: UIWindowScene?
    /// Là où l'image de la session se montre sur l'écran externe.
    let imageHost = ImageHostView()
    private var window: UIWindow?
    private var idle: UIHostingController<ExternalIdleView>?

    var screen: UIScreen? { scene?.screen }

    /// Mode « comme cet écran » de l'écran externe (sa définition et sa fréquence).
    func mode(for settings: ConnectionSettings) -> (width: Int, height: Int, refresh: Int)? {
        screen.map { DeviceDisplay.mode(for: settings, screen: $0) }
    }

    func connect(_ scene: UIWindowScene) {
        let window = UIWindow(windowScene: scene)
        let root = UIViewController()
        root.view.backgroundColor = .black
        let idle = UIHostingController(rootView: ExternalIdleView())
        idle.view.backgroundColor = .clear
        idle.view.frame = root.view.bounds
        idle.view.autoresizingMask = [.flexibleWidth, .flexibleHeight]
        root.addChild(idle)
        root.view.addSubview(idle.view)
        idle.didMove(toParent: root)
        imageHost.frame = root.view.bounds
        imageHost.autoresizingMask = [.flexibleWidth, .flexibleHeight]
        imageHost.isHidden = true
        root.view.addSubview(imageHost)
        window.rootViewController = root
        window.isHidden = false
        self.window = window
        self.idle = idle
        self.scene = scene
        let px = scene.screen.nativeBounds.size
        KyberLog.app.notice("écran externe branché : \(Int(px.width))×\(Int(px.height)) à \(scene.screen.maximumFramesPerSecond) Hz")
    }

    func disconnect(_ scene: UIScene) {
        guard scene === self.scene else { return }
        imageHost.removeFromSuperview()
        window?.isHidden = true
        window = nil
        idle = nil
        self.scene = nil
        KyberLog.app.notice("écran externe débranché")
    }

    /// L'image de la session est (ou n'est plus) sur l'écran externe.
    func showingImage(_ on: Bool) {
        imageHost.isHidden = !on
    }
}

/// La scène de l'écran externe : sa fenêtre est celle d'ExternalDisplay.
final class ExternalSceneDelegate: NSObject, UIWindowSceneDelegate {
    func scene(_ scene: UIScene, willConnectTo session: UISceneSession, options: UIScene.ConnectionOptions) {
        guard let scene = scene as? UIWindowScene else { return }
        MainActor.assumeIsolated { ExternalDisplay.shared.connect(scene) }
    }

    func sceneDidDisconnect(_ scene: UIScene) {
        MainActor.assumeIsolated { ExternalDisplay.shared.disconnect(scene) }
    }
}

/// L'écran externe hors session : le symbole, éteint, et ce qui va s'y passer.
struct ExternalIdleView: View {
    var body: some View {
        ZStack {
            Theme.background.ignoresSafeArea()
            VStack(spacing: 22) {
                Mark(size: 96, level: .off)
                Wordmark(size: 30)
                Text("L'image de votre ordinateur s'affichera ici. L'appareil servira de pavé tactile.")
                    .font(Theme.sans(20))
                    .foregroundStyle(Theme.muted)
                    .multilineTextAlignment(.center)
                    .frame(maxWidth: 640)
            }
        }
    }
}
