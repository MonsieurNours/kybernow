// Une partie avec une console PlayStation, dans sa fenêtre (textes :
// docs/link-ui.md, « Consoles PlayStation »). L'image passe par la même vue
// et le même rendu qu'une session d'ordinateur (StreamView, VideoToolbox,
// Metal, HDR sur un écran HDR) ; le protocole est dans le cœur
// (`PlayStationPlay`). La fenêtre s'ouvre aux deux tiers de l'écran, en
// 16:9 ; le plein écran est celui de macOS. Les manettes ne sont pas encore
// transmises.

import AppKit
import KyberKit
import SwiftUI

@MainActor
final class PlayStationSession: NSObject, ObservableObject, NSWindowDelegate {
    enum Phase: Equatable {
        case connecting, streaming
        /// Fin : le texte à montrer (nil : fermée par ce Mac).
        case ended(String?)
    }

    @Published private(set) var phase: Phase = .connecting
    /// Contenu protégé à l'écran de la console.
    @Published private(set) var cantDisplay = false

    let console: PlayStationConsole
    private let address: String
    private let registered: String
    private let hdr: Bool
    private let onEnd: (PlayStationSession, PlayStationPlay.End?) -> Void
    private let streamView = StreamView(frame: .zero)
    private var play: PlayStationPlay?
    private var window: NSWindow?
    private var overlay: NSHostingView<PlayStationOverlay>?
    private var end: PlayStationPlay.End?
    private var finished = false
    /// Mise en veille demandée ici : sa fin ne se dit pas.
    private var resting = false

    init(console: PlayStationConsole, address: String, registered: String, hdr: Bool, onEnd: @escaping (PlayStationSession, PlayStationPlay.End?) -> Void) {
        self.console = console
        self.address = address
        self.registered = registered
        self.hdr = hdr
        self.onEnd = onEnd
    }

    func open() {
        let screen = NSApp.keyWindow?.screen ?? NSScreen.main ?? NSScreen.screens[0]
        let area = screen.visibleFrame
        var w = area.width * 2 / 3, h = w * 9 / 16
        if h > area.height * 0.85 { h = area.height * 0.85; w = h * 16 / 9 }
        let content = NSRect(x: area.midX - w / 2, y: area.midY - h / 2, width: w, height: h)
        let window = NSWindow(contentRect: content, styleMask: [.titled, .closable, .miniaturizable, .resizable], backing: .buffered, defer: false)
        window.title = console.name
        window.appearance = NSAppearance(named: .darkAqua)
        window.backgroundColor = .black
        window.contentAspectRatio = NSSize(width: 16, height: 9)
        window.collectionBehavior = [.fullScreenPrimary, .managed]
        window.tabbingMode = .disallowed
        window.isRestorable = false
        window.isReleasedWhenClosed = false
        let container = NSView(frame: NSRect(origin: .zero, size: content.size))
        container.autoresizingMask = [.width, .height]
        streamView.frame = container.bounds
        streamView.autoresizingMask = [.width, .height]
        container.addSubview(streamView)
        let overlay = NSHostingView(rootView: PlayStationOverlay(session: self))
        overlay.frame = container.bounds
        overlay.autoresizingMask = [.width, .height]
        container.addSubview(overlay)
        self.overlay = overlay
        window.contentView = container
        // « Mettre la console en veille », dans la barre de titre.
        let rest = NSButton(title: PSText.rest, target: self, action: #selector(goToRest))
        rest.bezelStyle = .accessoryBarAction
        rest.controlSize = .small
        let accessory = NSTitlebarAccessoryViewController()
        let holder = NSView(frame: NSRect(x: 0, y: 0, width: rest.fittingSize.width + 16, height: 28))
        rest.frame.origin = NSPoint(x: 8, y: (28 - rest.fittingSize.height) / 2)
        rest.frame.size = rest.fittingSize
        holder.addSubview(rest)
        accessory.view = holder
        accessory.layoutAttribute = .trailing
        window.addTitlebarAccessoryViewController(accessory)
        window.delegate = self
        self.window = window
        window.makeKeyAndOrderFront(nil)

        let play = PlayStationPlay(view: streamView)
        play.onState = { [weak self] in self?.stateChanged($0) }
        self.play = play
        if !play.start(address: address, registered: registered, hdr: hdr) {
            stateChanged(.ended(.notPaired))
        }
    }

    func bringToFront() {
        NSApp.activate()
        window?.makeKeyAndOrderFront(nil)
    }

    @objc func goToRest() {
        guard case .streaming = phase else { return }
        resting = true
        play?.gotoBed()
    }

    private func stateChanged(_ s: PlayStationPlay.State) {
        switch s {
        case .connecting:
            phase = .connecting
        case let .streaming(w, h, hdr):
            KyberLog.app.notice("PlayStation : image \(w)x\(h) \(hdr ? "HDR" : "SDR", privacy: .public)")
            phase = .streaming
        case let .cantDisplay(on):
            cantDisplay = on
        case let .ended(e):
            guard !finished else { return }
            end = e
            cantDisplay = false
            // Fin voulue ici (fenêtre fermée) ou mise en veille : la fenêtre se ferme.
            let text = resting ? nil : PSText.end(e)
            phase = .ended(text)
            stopPlay()
            if text == nil { close() }
        }
        overlay?.isHidden = phase == .streaming && !cantDisplay
    }

    /// Arrêt de la partie hors du fil principal (`kn_ps_stop` attend sa fin).
    private func stopPlay() {
        guard let play else { return }
        self.play = nil
        DispatchQueue.global(qos: .userInitiated).async { play.stop() }
    }

    func close() {
        guard !finished else { return }
        finished = true
        stopPlay()
        streamView.detach()
        window?.delegate = nil
        window?.orderOut(nil)
        window?.close()
        window = nil
        onEnd(self, end)
    }

    // MARK: NSWindowDelegate

    nonisolated func windowShouldClose(_ sender: NSWindow) -> Bool {
        MainActor.assumeIsolated { close() }
        return false
    }
}

/// Ce qui se dit par-dessus l'image : la connexion, le contenu protégé, la fin.
struct PlayStationOverlay: View {
    @ObservedObject var session: PlayStationSession

    var body: some View {
        ZStack {
            switch session.phase {
            case .connecting:
                ZStack {
                    Theme.background
                    HStack(spacing: 12) {
                        ProgressView().controlSize(.small)
                        Text(PSText.connecting).font(Theme.sans(16)).foregroundStyle(Theme.text2)
                    }
                }
            case .streaming:
                if session.cantDisplay {
                    Text(PSText.cantDisplay)
                        .font(Theme.sans(16)).foregroundStyle(Theme.text)
                        .padding(.horizontal, 20).padding(.vertical, 14)
                        .glass(12, tint: 0.6)
                }
            case let .ended(text):
                ZStack {
                    Theme.background
                    VStack(spacing: 18) {
                        if let text {
                            Text(text).font(Theme.sans(17)).foregroundStyle(Theme.text)
                                .multilineTextAlignment(.center)
                                .frame(maxWidth: 520)
                        }
                        Button(String(localized: "Fermer")) { session.close() }
                            .buttonStyle(GhostButton(height: 40))
                            .keyboardShortcut(.defaultAction)
                    }
                }
            }
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .preferredColorScheme(.dark)
    }
}
