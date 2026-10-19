// Une session avec un ordinateur : préparation par la console (écran
// virtuel à la taille de ce Mac, son), connexion à Kyber, image en plein
// écran, signal de vie, puis fin propre.

import AppKit
import KyberKit
import SwiftUI

@MainActor
final class SessionController: NSObject, ObservableObject, NSWindowDelegate {
    enum Phase: Equatable {
        case preparing, connecting, starting, streaming
        case reconnecting(Int)
        case ending
        case ended
        case failed(String)
    }

    @Published private(set) var phase: Phase = .preparing
    private(set) var host: Host
    private let store: HostStore
    private let onFinish: (SessionController) -> Void

    private let streamView = StreamView(frame: .zero)
    private var window: NSWindow?
    private var overlay: NSHostingView<SessionOverlay>?
    private var console: ConsoleClient?
    private var kyber: KyberClient?
    private var heartbeat: Task<Void, Never>?
    private var prepared: PrepareResult?
    private var display: KyberDisplay?

    init(host: Host, store: HostStore, onFinish: @escaping (SessionController) -> Void) {
        self.host = host
        self.store = store
        self.onFinish = onFinish
    }

    // MARK: - Fenêtre

    private func openWindow() {
        let screen = NSScreen.main ?? NSScreen.screens[0]
        let window = NSWindow(contentRect: screen.frame, styleMask: [.titled, .closable, .miniaturizable, .resizable, .fullSizeContentView], backing: .buffered, defer: false)
        window.title = host.name
        window.titlebarAppearsTransparent = true
        window.backgroundColor = .black
        window.collectionBehavior = [.fullScreenPrimary]
        window.delegate = self
        window.isReleasedWhenClosed = false

        let container = NSView(frame: window.contentRect(forFrameRect: window.frame))
        container.autoresizingMask = [.width, .height]
        streamView.frame = container.bounds
        streamView.autoresizingMask = [.width, .height]
        container.addSubview(streamView)
        let overlay = NSHostingView(rootView: SessionOverlay(session: self))
        overlay.frame = container.bounds
        overlay.autoresizingMask = [.width, .height]
        container.addSubview(overlay)
        self.overlay = overlay
        window.contentView = container
        window.makeKeyAndOrderFront(nil)
        window.makeFirstResponder(streamView)
        window.toggleFullScreen(nil)
        self.window = window
    }

    func toggleFullScreen() {
        window?.toggleFullScreen(nil)
    }

    func windowWillClose(_ notification: Notification) {
        if phase != .ended { end() }
    }

    // MARK: - Déroulé

    func start() {
        openWindow()
        guard let password = store.password(for: host) else {
            fail("mot de passe introuvable dans le Trousseau")
            return
        }
        let screen = window?.screen ?? NSScreen.main ?? NSScreen.screens[0]
        let mode = DisplayInfo.mode(for: screen)
        let console = ConsoleClient(host: host.address, port: host.consolePort, username: host.username, password: password, fingerprint: host.consoleFingerprint)
        self.console = console

        Task {
            do {
                phase = .preparing
                let result = try await console.prepare(width: mode.width, height: mode.height, refresh: mode.refresh, audioChannels: AudioOutput.channels())
                prepared = result
                if host.consoleFingerprint == nil || host.streamPort != result.kyberPort {
                    host.consoleFingerprint = console.fingerprint
                    host.streamPort = result.kyberPort ?? host.streamPort
                    store.update(host)
                }
                startHeartbeat(every: result.heartbeatSeconds)
                try connect(password: password)
            } catch {
                fail(error.localizedDescription)
            }
        }
    }

    private func connect(password: String) throws {
        guard let port = prepared?.kyberPort ?? host.streamPort else {
            throw ConsoleError.http(0, "port du flux inconnu")
        }
        phase = .connecting
        let kyber = try KyberClient(host: host.address, port: port, username: host.username, password: password)
        kyber.onEvent = { [weak self] in self?.handle($0) }
        self.kyber = kyber
        try kyber.connect()
    }

    private func handle(_ event: KyberEvent) {
        switch event {
        case let .connected(displays):
            // L'écran virtuel est le seul actif ; sinon, celui au mode demandé, ou le premier.
            let wanted = displays.first { $0.width == prepared?.width && $0.height == prepared?.height }
            guard let chosen = wanted ?? displays.first else {
                fail("l'ordinateur n'a annoncé aucun écran")
                return
            }
            display = chosen
            startStreaming()
        case .streamingStarted:
            phase = .streaming
            host.lastSession = Date()
            store.update(host)
            DispatchQueue.main.asyncAfter(deadline: .now() + 1.2) { [weak self] in
                self?.overlay?.removeFromSuperview()
                self?.overlay = nil
                if let view = self?.streamView { self?.window?.makeFirstResponder(view) }
            }
        case let .streamingFailed(reason), let .pipelineError(reason):
            fail(reason)
        case let .connectFailed(reason):
            fail(reason)
        case let .reconnecting(attempt, _):
            phase = .reconnecting(attempt)
        case let .reconnected(restart):
            phase = .streaming
            if restart { startStreaming() }
        case .reconnectionFailed:
            fail("connexion perdue")
        case let .stopped(reason):
            if phase != .ending && phase != .ended { fail(reason) }
        case .streamingStopped, .displaysChanged:
            break
        }
    }

    private func startStreaming() {
        guard let kyber, let display else { return }
        phase = .starting
        do {
            try kyber.startStreaming(display: display, view: streamView, options: host.profile.options)
        } catch {
            fail(error.localizedDescription)
        }
    }

    private func startHeartbeat(every seconds: Int) {
        heartbeat?.cancel()
        heartbeat = Task { [weak self] in
            while !Task.isCancelled {
                try? await Task.sleep(for: .seconds(max(1, seconds)))
                await self?.console?.heartbeat()
            }
        }
    }

    /// Fin de session : l'image se coupe, la console rallume les écrans du PC.
    func end() {
        guard phase != .ending && phase != .ended else { return }
        phase = .ending
        heartbeat?.cancel()
        streamView.detach()
        kyber?.stop()
        kyber = nil
        let console = self.console
        Task {
            try? await console?.end()
            phase = .ended
            window?.delegate = nil
            if window?.styleMask.contains(.fullScreen) == true { window?.toggleFullScreen(nil) }
            window?.close()
            onFinish(self)
        }
    }

    private func fail(_ message: String) {
        KyberLog.app.error("session : \(message, privacy: .public)")
        heartbeat?.cancel()
        kyber?.stop()
        kyber = nil
        phase = .failed(message)
        Task { try? await console?.end() }
    }

    func dismissFailure() {
        window?.delegate = nil
        window?.close()
        phase = .ended
        onFinish(self)
    }
}

/// Ce qui s'affiche par-dessus l'image tant qu'elle n'est pas là.
struct SessionOverlay: View {
    @ObservedObject var session: SessionController

    var body: some View {
        ZStack {
            Theme.background.opacity(session.phase == .streaming ? 0 : 1)
                .animation(.easeOut(duration: 1), value: session.phase)
            VStack(spacing: 18) {
                Mark(size: 96)
                    .scaleEffect(session.phase == .streaming ? 1.4 : 1)
                    .opacity(session.phase == .streaming ? 0 : 1)
                    .animation(.easeOut(duration: 0.6), value: session.phase)
                Text(session.host.name)
                    .font(Theme.display(30))
                    .foregroundStyle(Theme.text)
                    .opacity(session.phase == .streaming ? 0 : 1)
                Group {
                    switch session.phase {
                    case .preparing: Text("On prépare votre écran…")
                    case .connecting: Text("Connexion…")
                    case .starting: Text("Presque prêt")
                    case let .reconnecting(n): Text("Reconnexion (essai \(n))…")
                    case .ending: Text("Fin de la session…")
                    case let .failed(message): Text(message.prefix(1).uppercased() + message.dropFirst()).foregroundStyle(Theme.warn)
                    case .streaming, .ended: Text(" ")
                    }
                }
                .font(.system(size: 16))
                .foregroundStyle(Theme.muted)
                if case .failed = session.phase {
                    Button("Revenir à l'accueil") { session.dismissFailure() }
                        .buttonStyle(PrimaryButton())
                        .padding(.top, 8)
                }
            }
        }
    }
}
