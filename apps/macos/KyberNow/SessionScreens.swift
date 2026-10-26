// Multi-écrans (docs/multi-ecrans.md, docs/link-ui.md « Plusieurs écrans ») :
// chaque écran de ce Mac pris par la session a sa surface, une vue de session
// (son image, son rendu Metal, son trait de repère) dans une fenêtre plein
// écran. La surface de l'écran de la fenêtre de l'accueil est celle de la
// session à un écran ; les autres ont leur propre fenêtre, ouverte et fermée
// par la session. Le contrôleur (SessionController) tient la barre, unique,
// et la pose sur la surface où le geste a lieu.

import AppKit
import KyberKit
import SwiftUI

/// Un écran de ce Mac dans la session.
@MainActor
final class ScreenSurface {
    let streamView: StreamView
    /// Ce qui porte la vue de session, le fond de la connexion et, quand
    /// elle est ici, la couche SwiftUI de la barre.
    let container: NSView
    private(set) weak var window: NSWindow?
    /// Fond de la connexion, sans texte, ou avec la ligne d'un écran que
    /// l'ordinateur n'a pas pu créer ; retiré dès la première image de l'écran.
    let backdrop: ScreenBackdrop
    /// Fenêtre ouverte par la session (un autre écran que celui de l'accueil) :
    /// fermée à la fin.
    let owned: Bool
    /// Écran de cette surface (fenêtre ouverte par la session).
    let screenID: CGDirectDisplayID?
    private let delegate: SessionWindowDelegate?
    private var occlusion: NSObjectProtocol?

    /// La surface de la fenêtre de l'accueil.
    init(main view: StreamView, container: NSView, window: NSWindow?) {
        streamView = view
        self.container = container
        self.window = window
        backdrop = ScreenBackdrop(frame: container.bounds)
        owned = false
        screenID = nil
        delegate = nil
    }

    /// Une fenêtre de plus, plein écran sur `screen`, au même titre que celle de la session.
    init(screen: NSScreen, title: String, onClose: @escaping () -> Void) {
        streamView = StreamView(frame: .zero)
        owned = true
        screenID = DisplayInfo.displayID(screen)
        let separateSpaces = NSScreen.screensHaveSeparateSpaces
        // Écrans à espaces séparés (réglage par défaut de macOS) : plein écran
        // natif sur chaque écran, barre des menus et Dock masqués. Sinon, une
        // fenêtre sans bordure à la taille de l'écran.
        let style: NSWindow.StyleMask = separateSpaces ? [.titled, .closable, .resizable, .fullSizeContentView] : [.borderless]
        let frame = separateSpaces ? screen.visibleFrame.insetBy(dx: screen.visibleFrame.width * 0.1, dy: screen.visibleFrame.height * 0.1) : screen.frame
        let window = NSWindow(contentRect: frame, styleMask: style, backing: .buffered, defer: false, screen: screen)
        window.setFrame(frame, display: false)
        window.title = title
        window.titleVisibility = .hidden
        window.titlebarAppearsTransparent = true
        window.isReleasedWhenClosed = false
        window.backgroundColor = .black
        window.collectionBehavior = [.fullScreenPrimary, .managed]
        window.acceptsMouseMovedEvents = true
        let container = NSView(frame: NSRect(origin: .zero, size: frame.size))
        container.autoresizingMask = [.width, .height]
        streamView.frame = container.bounds
        streamView.autoresizingMask = [.width, .height]
        container.addSubview(streamView)
        backdrop = ScreenBackdrop(frame: container.bounds)
        container.addSubview(backdrop)
        window.contentView = container
        self.container = container
        let delegate = SessionWindowDelegate()
        delegate.onClose = onClose
        window.delegate = delegate
        self.delegate = delegate
        self.window = window
        window.orderFront(nil)
        if separateSpaces { window.toggleFullScreen(nil) }
    }

    /// Le fond s'insère sous ce qui est déjà posé par-dessus la vue (passage, barre).
    func installBackdrop() {
        guard backdrop.superview == nil else { return }
        backdrop.frame = container.bounds
        container.addSubview(backdrop, positioned: .above, relativeTo: streamView)
    }

    /// Fenêtre occultée (autre espace, recouverte, réduite).
    func observeOcclusion(_ changed: @escaping () -> Void) {
        guard let window, occlusion == nil else { return }
        occlusion = NotificationCenter.default.addObserver(forName: NSWindow.didChangeOcclusionStateNotification, object: window, queue: .main) { _ in
            MainActor.assumeIsolated { changed() }
        }
    }

    var occluded: Bool {
        guard let window else { return true }
        return !window.occlusionState.contains(.visible)
    }

    /// Fin de session : la fenêtre ouverte par la session se ferme.
    func close() {
        if let occlusion { NotificationCenter.default.removeObserver(occlusion) }
        occlusion = nil
        backdrop.removeFromSuperview()
        guard owned, let window else { return }
        delegate?.onClose = nil
        window.delegate = nil
        window.orderOut(nil)
        window.close()
    }
}

/// Le fond de la connexion sur un écran de plus : rien à lire, sauf pour un
/// écran que l'ordinateur n'a pas pu créer (une seule ligne).
@MainActor
final class ScreenBackdrop: NSView {
    private let model = ScreenBackdropModel()

    override init(frame: NSRect) {
        super.init(frame: frame)
        autoresizingMask = [.width, .height]
        let host = NSHostingView(rootView: ScreenBackdropView(model: model))
        host.frame = bounds
        host.autoresizingMask = [.width, .height]
        addSubview(host)
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) { fatalError() }

    /// Ne capte jamais la souris : elle va à la vue de session.
    override func hitTest(_ point: NSPoint) -> NSView? { nil }

    /// La ligne affichée (nil : le fond seul).
    var message: String? {
        get { model.message }
        set { model.message = newValue }
    }
}

final class ScreenBackdropModel: ObservableObject {
    @Published var message: String?
}

struct ScreenBackdropView: View {
    @ObservedObject var model: ScreenBackdropModel

    var body: some View {
        ZStack {
            // Le fond du passage, sans l'accueil photographié.
            Color(red: 0x05 / 255, green: 0x07 / 255, blue: 0x0A / 255)
            if let message = model.message {
                Text(message)
                    .font(Theme.sans(15))
                    .foregroundStyle(Theme.muted)
                    .multilineTextAlignment(.center)
                    .padding(32)
            }
        }
        .ignoresSafeArea()
        .preferredColorScheme(.dark)
    }
}
