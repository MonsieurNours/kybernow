// Vue d'une session : l'image de l'ordinateur et la capture du clavier et
// de la souris. L'image est décodée par l'app (VideoToolbox) et dessinée
// avec Metal dans `surface` ; pour l'AV1, VLC la dessine dans `videoView`.
//
// Curseur : en usage bureau, le Mac affiche toujours un curseur, à la forme
// de celui de l'ordinateur. Un masquage bref (frappe, changement de forme)
// ne le fait jamais disparaître : le serveur ne l'annonce qu'après 250 ms
// (kyber-patches serveur 0002). Un masquage durable (jeu en vue
// subjective) passe la souris en relative, en mode Auto (RelativeMouse).

import AppKit
@_implementationOnly import CKyber

public final class StreamView: NSView {
    /// Vue confiée au lecteur vidéo de Kyber (VLC).
    public let videoView = NSView()
    /// Image décodée par l'app, dessinée avec Metal.
    let surface = MetalSurface()

    /// « ⌘ envoyée comme Ctrl » (raccourcis Windows sous les doigts du Mac).
    public var commandAsControl = true
    /// Raccourcis gardés par le Mac (menus de l'app) : ⌃⌥⌘ + lettre.
    public static let reservedModifiers: NSEvent.ModifierFlags = [.control, .option, .command]
    /// Distance du pointeur au bord supérieur, à chaque mouvement (barre au survol).
    public var onPointerMoved: ((CGFloat) -> Void)?
    /// Bande du haut réservée à l'app (barre ouverte) : rien n'y part vers l'ordinateur.
    public var reservedTopBand: CGFloat = 0

    public enum MouseMode { case auto, absolute }
    /// Auto : souris relative quand l'ordinateur masque son pointeur (jeux).
    public var mouseMode: MouseMode = .auto { didSet { evaluateRelative() } }
    /// Barre de l'app ouverte : la souris redevient absolue pour s'en servir.
    public var relativeSuspended = false { didSet { evaluateRelative() } }
    /// La souris est-elle relative en ce moment (pour la barre et les menus).
    public var onRelativeChanged: ((Bool) -> Void)?
    private let relative = RelativeMouse()
    private var remoteHidden = false
    private var remotePosition: CGPoint?
    /// Libérée par l'utilisateur (⌃⌥⌘M) : reprise au prochain clic dans l'image.
    private var releasedByUser = false
    private var windowObservers: [NSObjectProtocol] = []

    private var input: InputPipeline?
    private var layout: OpaquePointer?
    private var hostSize = CGSize(width: 1920, height: 1080)
    private var pressedModifiers = Set<UInt16>()
    private var cursor: NSCursor = .arrow
    private var tracking: NSTrackingArea?
    private var keyUpMonitor: Any?

    public override init(frame: NSRect) {
        super.init(frame: frame)
        wantsLayer = true
        layer?.backgroundColor = NSColor.black.cgColor
        videoView.wantsLayer = true
        videoView.frame = bounds
        videoView.autoresizingMask = [.width, .height]
        addSubview(videoView)
        surface.frame = bounds
        surface.autoresizingMask = [.width, .height]
        surface.isHidden = true
        addSubview(surface)
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) { fatalError() }

    deinit {
        if let layout { kynput_video_layout_unref(layout) }
    }

    func attach(input: InputPipeline, hostSize: CGSize) {
        self.input = input
        self.hostSize = hostSize
        if let layout { kynput_video_layout_unref(layout) }
        var window = kynput_RectSize(width: bounds.width, height: bounds.height)
        var host = kynput_RectSize(width: hostSize.width, height: hostSize.height)
        layout = kynput_video_layout_new(&window, &host)
        // AppKit ne transmet pas aux vues le relâchement d'une touche tapée
        // avec ⌘ : on le récupère avant qu'il ne se perde.
        relative.onMove = { [weak input] dx, dy in input?.mouseMove(dx: dx, dy: dy) }
        if let keyUpMonitor { NSEvent.removeMonitor(keyUpMonitor) }
        keyUpMonitor = NSEvent.addLocalMonitorForEvents(matching: .keyUp) { [weak self] event in
            guard let self, event.modifierFlags.contains(.command), self.window?.firstResponder === self else { return event }
            self.keyUp(with: event)
            return nil
        }
    }

    public func detach() {
        endRelative(warp: false)
        remoteHidden = false
        if let keyUpMonitor { NSEvent.removeMonitor(keyUpMonitor) }
        keyUpMonitor = nil
        input?.flushKeys()
        input = nil
        pressedModifiers.removeAll()
    }

    // MARK: - Mise en page

    public override func setFrameSize(_ newSize: NSSize) {
        super.setFrameSize(newSize)
        if let layout {
            var window = kynput_RectSize(width: newSize.width, height: newSize.height)
            kynput_video_layout_set_window_size(layout, &window)
        }
    }

    // Les sous-vues de VLC ne doivent pas capter les événements.
    public override func hitTest(_ point: NSPoint) -> NSView? {
        frame.contains(point) ? self : nil
    }

    public override var acceptsFirstResponder: Bool { true }
    public override func acceptsFirstMouse(for event: NSEvent?) -> Bool { true }

    public override func updateTrackingAreas() {
        super.updateTrackingAreas()
        if let tracking { removeTrackingArea(tracking) }
        let area = NSTrackingArea(rect: bounds, options: [.mouseMoved, .mouseEnteredAndExited, .activeInKeyWindow, .inVisibleRect, .cursorUpdate], owner: self)
        addTrackingArea(area)
        tracking = area
    }

    // MARK: - Curseur

    func remoteCursorChanged(_ remote: RemoteCursor) {
        remotePosition = remote.position
        if remote.visible == remoteHidden {
            remoteHidden = !remote.visible
            evaluateRelative()
        }
        // Forme nouvelle : on l'adopte. Masquage : on garde la dernière forme.
        guard let image = remote.image else { return }
        let scale = window?.backingScaleFactor ?? 2
        image.size = NSSize(width: image.size.width / scale, height: image.size.height / scale)
        cursor = NSCursor(image: image, hotSpot: NSPoint(x: remote.hotSpot.x / scale, y: remote.hotSpot.y / scale))
        if let window, window.isKeyWindow, bounds.contains(convert(window.mouseLocationOutsideOfEventStream, from: nil)) {
            cursor.set()
        }
    }

    public override func cursorUpdate(with event: NSEvent) {
        cursor.set()
    }

    public override func resetCursorRects() {
        addCursorRect(bounds, cursor: cursor)
    }

    // MARK: - Souris relative

    /// ⌃⌥⌘M : rend la souris au Mac (ou la reprend).
    public func toggleMouseCapture() {
        if relative.active {
            releasedByUser = true
        } else {
            releasedByUser = false
        }
        evaluateRelative()
    }

    public override func viewDidMoveToWindow() {
        super.viewDidMoveToWindow()
        windowObservers.forEach(NotificationCenter.default.removeObserver)
        windowObservers = []
        guard let window else { return }
        for name in [NSWindow.didBecomeKeyNotification, NSWindow.didResignKeyNotification] {
            windowObservers.append(NotificationCenter.default.addObserver(forName: name, object: window, queue: .main) { [weak self] _ in
                self?.evaluateRelative()
            })
        }
    }

    private func evaluateRelative() {
        let want = mouseMode == .auto && remoteHidden && !relativeSuspended && !releasedByUser
            && input != nil && window?.isKeyWindow == true
        if want, !relative.active {
            // On n'entre qu'avec le pointeur sur l'image (pas pendant un tour dans les menus).
            guard let window, bounds.contains(convert(window.mouseLocationOutsideOfEventStream, from: nil)) else { return }
            input?.setRelative(true)
            relative.start()
            onRelativeChanged?(true)
        } else if !want, relative.active {
            endRelative(warp: true)
        }
    }

    /// Le pointeur du Mac réapparaît là où est celui de l'ordinateur.
    private func endRelative(warp: Bool) {
        guard relative.active else { return }
        relative.stop()
        input?.setRelative(false)
        onRelativeChanged?(false)
        guard warp, let layout, let host = remotePosition, let window, let screen = window.screen ?? NSScreen.screens.first else { return }
        var hostPos = kynput_Position(x: host.x, y: host.y)
        var local = kynput_Position(x: 0, y: 0)
        kynput_video_layout_host_to_local(layout, &hostPos, &local)
        let inView = NSPoint(x: local.x, y: bounds.height - local.y)
        let onScreen = window.convertPoint(toScreen: convert(inView, to: nil))
        // CGWarp compte en points depuis le coin haut gauche de l'écran principal.
        let top = NSScreen.screens.first?.frame.maxY ?? screen.frame.maxY
        CGWarpMouseCursorPosition(CGPoint(x: onScreen.x, y: top - onScreen.y))
        CGAssociateMouseAndMouseCursorPosition(1)
    }

    // MARK: - Souris

    private func inReservedBand(_ event: NSEvent) -> Bool {
        let p = convert(event.locationInWindow, from: nil)
        return bounds.height - p.y < reservedTopBand
    }

    private func sendPosition(_ event: NSEvent) {
        if relative.active {
            relative.appKitMoved(dx: event.deltaX, dy: event.deltaY)
            return
        }
        let p = convert(event.locationInWindow, from: nil)
        onPointerMoved?(bounds.height - p.y)
        guard let input, let layout, !inReservedBand(event) else { return }
        var local = kynput_Position(x: p.x, y: bounds.height - p.y)
        var host = kynput_Position(x: 0, y: 0)
        guard kynput_video_layout_local_to_host(layout, &local, false, &host) else { return }
        input.mouse(x: Int16(clamping: Int(host.x.rounded())), y: Int16(clamping: Int(host.y.rounded())))
    }

    public override func mouseMoved(with event: NSEvent) { sendPosition(event) }
    public override func mouseDragged(with event: NSEvent) { sendPosition(event) }
    public override func rightMouseDragged(with event: NSEvent) { sendPosition(event) }
    public override func otherMouseDragged(with event: NSEvent) { sendPosition(event) }

    public override func mouseDown(with event: NSEvent) {
        window?.makeFirstResponder(self)
        if releasedByUser {
            // Clic dans l'image après ⌃⌥⌘M : le jeu reprend la souris.
            releasedByUser = false
            evaluateRelative()
        }
        if !relative.active, inReservedBand(event) { return }
        sendPosition(event)
        input?.mouseButton(.left, pressed: true)
    }

    public override func mouseUp(with event: NSEvent) { input?.mouseButton(.left, pressed: false) }
    public override func rightMouseDown(with event: NSEvent) { sendPosition(event); input?.mouseButton(.right, pressed: true) }
    public override func rightMouseUp(with event: NSEvent) { input?.mouseButton(.right, pressed: false) }

    private func otherButton(_ event: NSEvent) -> InputPipeline.MouseButton? {
        switch event.buttonNumber {
        case 2: return .middle
        case 3: return .side
        case 4: return .extra
        default: return nil
        }
    }

    public override func otherMouseDown(with event: NSEvent) {
        if let b = otherButton(event) { input?.mouseButton(b, pressed: true) }
    }

    public override func otherMouseUp(with event: NSEvent) {
        if let b = otherButton(event) { input?.mouseButton(b, pressed: false) }
    }

    public override func scrollWheel(with event: NSEvent) {
        // Une encoche de molette = 1.0 ; un trackpad donne des pixels (même
        // réglage que le client Kyber : 16 pixels par encoche).
        let dy = event.hasPreciseScrollingDeltas ? event.scrollingDeltaY / 16 : event.scrollingDeltaY
        guard dy != 0 else { return }
        input?.wheel(dx: 0, dy: Float(dy))
    }

    // MARK: - Clavier

    private func send(keyCode: UInt16, pressed: Bool) {
        guard let usage = Keymap.usage(keyCode: keyCode, commandAsControl: commandAsControl),
              let scancode = Keymap.scancode(usage: usage) else { return }
        input?.key(scancode: scancode, pressed: pressed)
    }

    public override func keyDown(with event: NSEvent) {
        if event.isARepeat { return } // l'ordinateur répète lui-même
        send(keyCode: event.keyCode, pressed: true)
    }

    public override func keyUp(with event: NSEvent) {
        send(keyCode: event.keyCode, pressed: false)
    }

    public override func flagsChanged(with event: NSEvent) {
        let code = event.keyCode
        guard let flag = Keymap.modifiers[code] else { return }
        if flag == .capsLock {
            send(keyCode: code, pressed: true)
            send(keyCode: code, pressed: false)
            return
        }
        let down = event.modifierFlags.contains(flag) && !pressedModifiers.contains(code)
        if down { pressedModifiers.insert(code) } else { pressedModifiers.remove(code) }
        send(keyCode: code, pressed: down)
    }

    // ⌘ + touche : pour l'ordinateur, sauf les raccourcis ⌃⌥⌘ de l'app.
    public override func performKeyEquivalent(with event: NSEvent) -> Bool {
        guard window?.firstResponder === self else { return false }
        if event.modifierFlags.intersection(.deviceIndependentFlagsMask).isSuperset(of: Self.reservedModifiers) {
            return false
        }
        if event.type == .keyDown {
            keyDown(with: event)
            return true
        }
        return false
    }

    public override func resignFirstResponder() -> Bool {
        input?.flushKeys()
        pressedModifiers.removeAll()
        return super.resignFirstResponder()
    }
}

/// Vue qui porte la couche Metal du rendu (vue « hébergeant » une couche).
final class MetalSurface: NSView {
    private var renderer: VideoRenderer?

    /// Branche le rendu (nil : VLC dessine, la surface se cache).
    func show(_ renderer: VideoRenderer?) {
        self.renderer = renderer
        isHidden = renderer == nil
        guard let renderer else { return }
        layer = renderer.layer
        wantsLayer = true
        updateDrawableSize()
    }

    override func setFrameSize(_ newSize: NSSize) {
        super.setFrameSize(newSize)
        updateDrawableSize()
    }

    override func viewDidChangeBackingProperties() {
        super.viewDidChangeBackingProperties()
        updateDrawableSize()
    }

    override func hitTest(_ point: NSPoint) -> NSView? { nil }

    private func updateDrawableSize() {
        guard let renderer else { return }
        let scale = window?.backingScaleFactor ?? NSScreen.main?.backingScaleFactor ?? 2
        renderer.layer.contentsScale = scale
        renderer.setDrawableSize(CGSize(width: bounds.width * scale, height: bounds.height * scale))
    }
}
