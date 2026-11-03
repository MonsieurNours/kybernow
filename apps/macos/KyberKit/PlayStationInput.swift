// Clavier et souris d'une partie sur console PlayStation, traduits en
// manette par le cœur (`apps/core/src/playstation/keymouse.rs`). Les
// touches partent par leur place (usage HID) : la disposition de la console
// suit ainsi celle du clavier du Mac (ZQSD en AZERTY). Un clic dans l'image
// capture la souris (pointeur figé et caché, déplacements bruts, comme la
// souris des jeux d'une session d'ordinateur) ; ⌃⌥⌘M la rend, comme une
// fenêtre qui perd le clavier. Les raccourcis ⌃⌥⌘ restent à l'app. Une
// manette branchée l'emporte : le cœur ignore alors clavier et souris.
// Barre de la partie ouverte (`suspended`) : rien ne part à la console, et le
// pointeur libre est suivi pour l'ouvrir (`onPointerMoved`).

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
    /// Pointeur libre déplacé : sa place sur l'image, depuis le coin haut
    /// gauche (nil hors de l'image) ; la barre s'ouvre contre le bord haut.
    public var onPointerMoved: ((CGPoint?) -> Void)?
    /// Barre ouverte : touches et clics ne vont plus à la console (un clic
    /// dans l'image capture encore la souris, ce qui ferme la barre).
    /// Les autres raccourcis ⌃⌥⌘ (D, F, S), pour la partie de cette
    /// fenêtre plutôt qu'une session d'ordinateur : vrai s'il est pris.
    public var onShortcut: ((String) -> Bool)?
    public var suspended = false {
        didSet { if suspended, !oldValue { heldModifiers.removeAll(); play?.releaseAll() } }
    }

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

    /// Prend la souris (« Prendre la souris », clic dans l'image).
    public func capture() {
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
        if flags.contains(StreamView.reservedModifiers), let key = event.charactersIgnoringModifiers?.lowercased(),
           window?.isKeyWindow == true, onShortcut?(key) == true {
            return true
        }
        return super.performKeyEquivalent(with: event)
    }

    public override func keyDown(with event: NSEvent) {
        if reserved(event) { return super.keyDown(with: event) }
        guard !suspended, !event.isARepeat, let usage = Keymap.hid(keyCode: event.keyCode) else { return }
        play?.key(usage: usage, pressed: true)
    }

    public override func keyUp(with event: NSEvent) {
        guard !suspended, let usage = Keymap.hid(keyCode: event.keyCode) else { return }
        play?.key(usage: usage, pressed: false)
    }

    public override func flagsChanged(with event: NSEvent) {
        let code = event.keyCode
        guard !suspended, let flag = Keymap.modifierFlags[code], let usage = Keymap.hid(keyCode: code) else { return }
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
        if !suspended { play?.mouseButton(0, pressed: true) }
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
        if captured {
            mouse.appKitMoved(dx: event.deltaX, dy: event.deltaY)
        } else {
            onPointerMoved?(spot(inWindow: event.locationInWindow))
        }
    }

    /// Le pointeur sur l'image, depuis son coin haut gauche ; nil ailleurs
    /// (la fenêtre clé reçoit les mouvements même hors d'elle).
    public var pointerSpot: CGPoint? {
        guard let window else { return nil }
        return spot(inWindow: window.mouseLocationOutsideOfEventStream)
    }

    private func spot(inWindow location: NSPoint) -> CGPoint? {
        let p = convert(location, from: nil)
        guard p.x >= 0, p.x <= bounds.width, p.y >= 0, p.y <= bounds.height else { return nil }
        return CGPoint(x: p.x, y: isFlipped ? p.y : bounds.height - p.y)
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
