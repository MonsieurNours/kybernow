// Clavier et souris d'une partie sur console PlayStation, traduits en
// manette par le cœur (`apps/core/src/playstation/keymouse.rs`). Les
// touches partent par leur place (usage HID) : la disposition de la console
// suit ainsi celle du clavier du Mac (ZQSD en AZERTY). Un clic dans l'image
// capture la souris (pointeur figé et caché, déplacements bruts, comme la
// souris des jeux d'une session d'ordinateur) ; ⌃⌥⌘M la rend, comme une
// fenêtre qui perd le clavier. Les raccourcis ⌃⌥⌘ restent à l'app. Une
// manette branchée l'emporte : le cœur ignore alors clavier et souris.

import AppKit

public final class PlayStationInputView: NSView {
    /// La partie qui reçoit les entrées.
    public weak var play: PlayStationPlay?
    /// La souris est capturée (fil principal).
    public private(set) var captured = false {
        didSet { if captured != oldValue { onCapture?(captured) } }
    }
    /// Annonce les changements de capture (textes d'aide de la fenêtre).
    public var onCapture: ((Bool) -> Void)?

    private let mouse = RelativeMouse()
    /// Touches de modification tenues (kVK).
    private var heldModifiers = Set<UInt16>()

    public override init(frame: NSRect) {
        super.init(frame: frame)
        mouse.onMove = { [weak self] dx, dy in self?.play?.mouseMove(dx: Float(dx), dy: Float(dy)) }
    }

    required init?(coder: NSCoder) { nil }

    public override var acceptsFirstResponder: Bool { true }
    public override func acceptsFirstMouse(for event: NSEvent?) -> Bool { true }

    // MARK: Capture

    /// Rend la souris et relâche tout (fenêtre qui perd le clavier, fin).
    public func releaseAll() {
        if captured {
            mouse.stop()
            captured = false
        }
        heldModifiers.removeAll()
        play?.releaseAll()
    }

    private func capture() {
        guard !captured, window?.isKeyWindow == true else { return }
        mouse.start()
        captured = true
    }

    // MARK: Clavier

    public override func performKeyEquivalent(with event: NSEvent) -> Bool {
        let flags = event.modifierFlags.intersection(.deviceIndependentFlagsMask)
        // ⌃⌥⌘M : rendre la souris (ou la reprendre).
        if flags.contains(StreamView.reservedModifiers), event.charactersIgnoringModifiers?.lowercased() == "m" {
            if captured { releaseAll() } else { capture() }
            return true
        }
        return super.performKeyEquivalent(with: event)
    }

    public override func keyDown(with event: NSEvent) {
        if reserved(event) { return super.keyDown(with: event) }
        guard !event.isARepeat, let usage = Keymap.hid(keyCode: event.keyCode) else { return }
        play?.key(usage: usage, pressed: true)
    }

    public override func keyUp(with event: NSEvent) {
        guard let usage = Keymap.hid(keyCode: event.keyCode) else { return }
        play?.key(usage: usage, pressed: false)
    }

    public override func flagsChanged(with event: NSEvent) {
        let code = event.keyCode
        guard let flag = Keymap.modifierFlags[code], let usage = Keymap.hid(keyCode: code) else { return }
        // Le drapeau seul ne dit pas quel côté : la touche est tenue si son
        // drapeau est levé et qu'elle ne l'était pas.
        let down = event.modifierFlags.contains(flag) && !heldModifiers.contains(code)
        if down { heldModifiers.insert(code) } else { heldModifiers.remove(code) }
        play?.key(usage: usage, pressed: down)
    }

    private func reserved(_ event: NSEvent) -> Bool {
        event.modifierFlags.intersection(.deviceIndependentFlagsMask).contains(StreamView.reservedModifiers)
    }

    // MARK: Souris

    public override func mouseDown(with event: NSEvent) {
        window?.makeFirstResponder(self)
        guard captured else { return capture() }
        play?.mouseButton(0, pressed: true)
    }

    public override func mouseUp(with event: NSEvent) {
        if captured { play?.mouseButton(0, pressed: false) }
    }

    public override func rightMouseDown(with event: NSEvent) {
        if captured { play?.mouseButton(1, pressed: true) }
    }

    public override func rightMouseUp(with event: NSEvent) {
        if captured { play?.mouseButton(1, pressed: false) }
    }

    public override func otherMouseDown(with event: NSEvent) {
        if captured { play?.mouseButton(UInt8(min(event.buttonNumber, 4)), pressed: true) }
    }

    public override func otherMouseUp(with event: NSEvent) {
        if captured { play?.mouseButton(UInt8(min(event.buttonNumber, 4)), pressed: false) }
    }

    public override func mouseMoved(with event: NSEvent) {
        if captured { mouse.appKitMoved(dx: event.deltaX, dy: event.deltaY) }
    }

    public override func mouseDragged(with event: NSEvent) { mouseMoved(with: event) }
    public override func rightMouseDragged(with event: NSEvent) { mouseMoved(with: event) }
    public override func otherMouseDragged(with event: NSEvent) { mouseMoved(with: event) }

    public override func viewDidMoveToWindow() {
        super.viewDidMoveToWindow()
        window?.acceptsMouseMovedEvents = true
        if window == nil { releaseAll() }
    }
}
