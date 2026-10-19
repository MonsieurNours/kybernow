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
    /// Écrans annoncés par Kyber (la liste se met à jour quelques secondes
    /// après la création de l'écran virtuel : les écrans éteints en sortent).
    private var displays: [KyberDisplay] = []
    private var streaming: UInt32?
    private var settling: Task<Void, Never>?

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
        window.acceptsMouseMovedEvents = true

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
        KyberLog.app.notice("événement : \(String(describing: event), privacy: .public)")
        switch event {
        case let .connected(list):
            displays = list
            chooseDisplay()
        case let .displaysChanged(list):
            displays = list
            if let streaming, !list.contains(where: { $0.id == streaming }) {
                // L'écran suivi a disparu (écran physique éteint) : on passe au bon.
                restartStreaming()
            } else if settling != nil, list.count == 1 {
                settling?.cancel()
                settling = nil
                chooseDisplay()
            }
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
            if restart {
                streaming = nil
                startStreaming()
            }
        case .reconnectionFailed:
            fail("connexion perdue")
        case let .stopped(reason):
            if phase != .ending && phase != .ended { fail(reason) }
        case .streamingStopped:
            break
        }
    }

    /// Avec l'écran virtuel, Kyber doit finir par n'annoncer qu'un écran :
    /// on attend que sa liste se stabilise (6 s au plus) avant de démarrer.
    private func chooseDisplay() {
        if prepared?.virtualDisplay == true, displays.count > 1, settling == nil, streaming == nil {
            phase = .starting
            settling = Task { [weak self] in
                try? await Task.sleep(for: .seconds(6))
                guard !Task.isCancelled, let self else { return }
                self.settling = nil
                self.chooseDisplay()
            }
            return
        }
        guard let best = bestDisplay() else {
            fail("l'ordinateur n'a annoncé aucun écran")
            return
        }
        if streaming == best.id { return }
        display = best
        startStreaming()
    }

    private func bestDisplay() -> KyberDisplay? {
        if displays.count == 1 { return displays[0] }
        let sameMode = displays.filter { $0.width == prepared?.width && $0.height == prepared?.height }
        // L'écran virtuel est placé en haut à gauche du bureau.
        return sameMode.first { $0.x == 0 && $0.y == 0 } ?? sameMode.first ?? displays.first
    }

    private func startStreaming() {
        guard let kyber, let display else { return }
        phase = .starting
        do {
            KyberLog.app.notice("image : écran \(display.id) \(display.width)×\(display.height) en (\(display.x), \(display.y))")
            try kyber.startStreaming(display: display, view: streamView, options: host.profile.options)
            streaming = display.id
        } catch {
            fail(error.localizedDescription)
        }
    }

    private func restartStreaming() {
        guard let kyber, let best = bestDisplay() else { return }
        kyber.stopStreaming()
        streaming = nil
        display = best
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.3) { [weak self] in self?.startStreaming() }
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
