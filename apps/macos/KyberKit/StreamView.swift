// Vue d'une session : l'image de l'ordinateur (dessinée par VLC dans
// `videoView`) et la capture du clavier et de la souris.
//
// Curseur : en usage bureau, le Mac affiche toujours un curseur, à la forme
// de celui du PC. Un masquage annoncé par Windows (frappe, changement de
// forme) ne le fait jamais disparaître : on garde la dernière forme connue.

import AppKit
@_implementationOnly import CKyber

public final class StreamView: NSView {
    /// Vue confiée au lecteur vidéo de Kyber.
    public let videoView = NSView()

    /// « ⌘ envoyée comme Ctrl » (raccourcis Windows sous les doigts du Mac).
    public var commandAsControl = true
    /// Raccourcis gardés par le Mac (menus de l'app) : ⌃⌥⌘ + lettre.
    public static let reservedModifiers: NSEvent.ModifierFlags = [.control, .option, .command]

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
        if let keyUpMonitor { NSEvent.removeMonitor(keyUpMonitor) }
        keyUpMonitor = NSEvent.addLocalMonitorForEvents(matching: .keyUp) { [weak self] event in
            guard let self, event.modifierFlags.contains(.command), self.window?.firstResponder === self else { return event }
            self.keyUp(with: event)
            return nil
        }
    }

    public func detach() {
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

    // MARK: - Souris

    private func sendPosition(_ event: NSEvent) {
        guard let input, let layout else { return }
        let p = convert(event.locationInWindow, from: nil)
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
