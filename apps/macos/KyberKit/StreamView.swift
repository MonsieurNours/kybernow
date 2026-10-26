// Vue d'une session : l'image de l'ordinateur et la capture du clavier et
// de la souris. L'image, quel que soit le codec (H.264, HEVC, AV1), est
// décodée par l'app (VideoToolbox) et dessinée avec Metal dans `surface`.
//
// Curseur : en usage bureau, le Mac affiche toujours un curseur, à la forme
// de celui de l'ordinateur. Un masquage bref (frappe, changement de forme)
// ne le fait jamais disparaître : le serveur ne l'annonce qu'après 250 ms
// (kyber-patches serveur 0002). Un masquage durable (jeu en vue
// subjective) passe la souris en relative, en mode Auto (RelativeMouse).
//
// L'état du pointeur de l'ordinateur (position, forme, visibilité) et les
// règles Auto / Bureau / Jeu sont dans PointerLayer (le cœur), partagé avec
// l'Apple TV, l'iPhone et l'iPad ;
// ici, le dessin (NSCursor, couche en mode jeu), la souris relative et les
// événements AppKit. La couche de pointeur est tenue sur le fil principal,
// où AppKit livre la souris et où vit l'état de la fenêtre.

import AppKit
@_implementationOnly import CKyber

public final class StreamView: NSView {
    /// Vue confiée à Kyber comme fenêtre (sa configuration l'exige) ; l'image
    /// n'y est pas dessinée : elle passe par Metal, dans `surface`.
    public let videoView = NSView()
    /// Image décodée par l'app, dessinée avec Metal.
    let surface = MetalSurface()

    /// « ⌘ envoyée comme Ctrl » (raccourcis Windows sous les doigts du Mac).
    public var commandAsControl = true
    /// Raccourcis gardés par le Mac (menus de l'app) : ⌃⌥⌘ + lettre.
    public static let reservedModifiers: NSEvent.ModifierFlags = [.control, .option, .command]
    /// Où est le pointeur du Mac sur la vue (points depuis son coin haut gauche),
    /// à chaque mouvement ; nil dès qu'il n'y est plus (sorti de la vue, sur un
    /// autre écran ou une autre app, fenêtre ou app qui perd le premier plan,
    /// souris des jeux). La barre au pointeur en vit.
    public var onPointerMoved: ((CGPoint?) -> Void)?
    /// Bande du haut réservée à l'app (barre ouverte) : rien n'y part vers l'ordinateur.
    public var reservedTopBand: CGFloat = 0

    /// Bureau : souris absolue, pointeur du Mac toujours là. Jeu : souris
    /// relative (déplacements bruts). Auto : jeu quand un jeu est en plein
    /// écran sur l'ordinateur et cache son pointeur (règles dans PointerLayer).
    public typealias MouseMode = KyberKit.MouseMode
    public var mouseMode: MouseMode = .auto { didSet { evaluateRelative() } }
    private static let rules = MouseModeRules(needsHiddenPointer: true)
    /// Un jeu est en plein écran sur l'ordinateur (veille de la console).
    public var pcGameFullscreen = false {
        didSet { if pcGameFullscreen != oldValue { evaluateRelative() } }
    }
    /// La souris est-elle en mode jeu en ce moment.
    public var isRelative: Bool { relative.active }
    /// Barre de l'app ouverte : la souris redevient absolue pour s'en servir.
    public var relativeSuspended = false { didSet { evaluateRelative() } }
    /// La souris est-elle relative en ce moment (pour la barre et les menus).
    public var onRelativeChanged: ((Bool) -> Void)?
    private let relative = RelativeMouse()
    /// Pointeur de l'ordinateur (position, forme, visibilité), partagé ; sur le
    /// fil principal ici. Le Mac ne tient pas de position à lui.
    let pointer = PointerLayer(queue: .main, holdsPosition: false)
    /// En mode jeu, le pointeur du Mac est caché : celui de l'ordinateur
    /// (menus, inventaires) est dessiné ici, là où l'ordinateur le place.
    /// Ses déplacements arrivent du serveur (kyber-patches 0006).
    private let pointerLayer: CALayer = {
        let layer = CALayer()
        layer.zPosition = 100
        layer.isHidden = true
        layer.actions = ["position": NSNull(), "bounds": NSNull(), "contents": NSNull(), "hidden": NSNull()]
        return layer
    }()
    private var windowObservers: [NSObjectProtocol] = []

    private var input: InputPipeline?
    private var layout: OpaquePointer?
    private var hostSize = CGSize(width: 1920, height: 1080)
    // Touches, modificateurs (codes kVK) et boutons de souris tenus sur
    // l'ordinateur : tenus par la chaîne d'entrées du cœur (« tout relâcher »).
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
        layer?.addSublayer(pointerLayer)
        pointer.onChanged = { [weak self] state, change in self?.pointerChanged(state, change) }
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
        applyCursorShape()
        // Image relancée en mode jeu : la chaîne neuve part en souris absolue,
        // elle doit repasser en relatif tout de suite.
        if relative.active { input.setRelative(true) }
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

    /// Entrées reliées à l'ordinateur. Après une relance de l'image, le service
    /// d'entrées de Kyber est neuf : il ne tient plus aucune touche. Le mode
    /// de la souris est réaffirmé, et les modificateurs encore tenus sur le
    /// Mac (⌘ ou ⇧ gardés pendant la relance) repartent ; les autres touches
    /// et les boutons ne sont pas renvoyés (un jeu y verrait un nouvel appui).
    func inputConnected() {
        guard let input else { return }
        input.setRelative(relative.active)
        input.forgetModifiers()
        resyncModifiers()
    }

    /// L'écran de l'ordinateur a changé de taille en cours de session : les
    /// positions envoyées suivent, sans refaire la chaîne d'entrées.
    func setHostSize(_ size: CGSize) {
        hostSize = size
        if let layout {
            var host = kynput_RectSize(width: size.width, height: size.height)
            kynput_video_layout_set_host_size(layout, &host)
        }
        pointer.setHostSize(size)
    }

    public func detach() {
        endRelative(warp: false)
        pointer.reset()
        if let keyUpMonitor { NSEvent.removeMonitor(keyUpMonitor) }
        keyUpMonitor = nil
        input?.flushKeys()
        input = nil
    }

    // MARK: - Mise en page

    public override func setFrameSize(_ newSize: NSSize) {
        super.setFrameSize(newSize)
        if let layout {
            var window = kynput_RectSize(width: newSize.width, height: newSize.height)
            kynput_video_layout_set_window_size(layout, &window)
        }
        applyCursorShape()
    }

    // Aucune sous-vue (`videoView`, `surface`) ne capte les événements : ils
    // arrivent tous ici.
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

    /// Case de 32 points, jamais agrandi : la taille d'un pointeur du Mac.
    private static let pointerBox: CGFloat = 32

    /// La couche de pointeur a changé (fil principal).
    private func pointerChanged(_ state: PointerLayer.State, _ change: PointerLayer.Change) {
        if change.visibility {
            evaluateRelative()
            if !state.remoteHidden {
                // Un pointeur remontré un instant (menus, clignotement de
                // Windows) ne fait pas quitter le mode jeu : on revérifie après.
                DispatchQueue.main.asyncAfter(deadline: .now() + MouseModeRules.recheckDelay) { [weak self] in self?.evaluateRelative() }
            }
        }
        if change.shape { applyCursorShape() }
        updatePointerLayer()
    }

    /// Pointeur de l'ordinateur dessiné par l'app, en mode jeu seulement.
    private func updatePointerLayer() {
        let state = pointer.state
        guard relative.active, !state.remoteHidden, let shape = state.shape, let host = state.remotePosition, let layout else {
            pointerLayer.isHidden = true
            return
        }
        // Même taille que le pointeur du Mac (voir applyCursorShape).
        let factor = PointerLayer.scale(pixels: shape.pixels, box: Self.pointerBox, maxFactor: 1)
        var hostPos = kynput_Position(x: host.x, y: host.y)
        var local = kynput_Position(x: 0, y: 0)
        kynput_video_layout_host_to_local(layout, &hostPos, &local)
        // La position annoncée est la pointe (point actif) ; vue non retournée : y vers le haut.
        pointerLayer.contents = shape.image
        pointerLayer.contentsScale = window?.backingScaleFactor ?? 2
        pointerLayer.frame = PointerLayer.frame(shape: shape, local: CGPoint(x: local.x, y: local.y), factor: factor, flippedHeight: bounds.height)
        pointerLayer.isHidden = false
    }

    /// Le pointeur du Mac prend la forme de celui de l'ordinateur, à la
    /// taille d'un pointeur du Mac ; pointe et image suivent la même échelle
    /// (voir `PointerLayer.scale`).
    private func applyCursorShape() {
        guard let shape = pointer.state.shape else { return }
        let factor = PointerLayer.scale(pixels: shape.pixels, box: Self.pointerBox, maxFactor: 1)
        let image = NSImage(cgImage: shape.image, size: NSSize(width: max(1, shape.pixels.width * factor), height: max(1, shape.pixels.height * factor)))
        cursor = NSCursor(image: image, hotSpot: NSPoint(x: shape.hotSpot.x * factor, y: shape.hotSpot.y * factor))
        window?.invalidateCursorRects(for: self)
        if let window, window.isKeyWindow, !relative.active, bounds.contains(convert(window.mouseLocationOutsideOfEventStream, from: nil)) {
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

    public override func viewDidMoveToWindow() {
        super.viewDidMoveToWindow()
        windowObservers.forEach(NotificationCenter.default.removeObserver)
        windowObservers = []
        guard let window else { return }
        let center = NotificationCenter.default
        windowObservers.append(center.addObserver(forName: NSWindow.didBecomeKeyNotification, object: window, queue: .main) { [weak self] _ in
            self?.keyboardRegained()
        })
        windowObservers.append(center.addObserver(forName: NSWindow.didResignKeyNotification, object: window, queue: .main) { [weak self] _ in
            self?.keyboardLost()
        })
        // Spotlight et certains raccourcis système ne retirent pas toujours le
        // clavier à la fenêtre ; l'app, elle, cesse d'être active.
        windowObservers.append(center.addObserver(forName: NSApplication.didResignActiveNotification, object: nil, queue: .main) { [weak self] _ in
            self?.keyboardLost()
        })
    }

    // MARK: - Tout relâcher

    /// La fenêtre perd le clavier (⌘Tab, ⌘Espace, Spotlight, autre app) : les
    /// relâchements qui suivent ne nous parviendront pas, rien ne doit rester
    /// enfoncé sur l'ordinateur, ni dans notre état.
    private func keyboardLost() {
        releaseEverything()
        evaluateRelative()
        onPointerMoved?(nil)
    }

    /// Relâche sur l'ordinateur ce que cette vue y tient : touches,
    /// modificateurs, boutons de souris (ReleaseAll). Pas de FlushKeys : il
    /// relâcherait aussi les manettes, qui, lues par SDL, continuent de parler
    /// à l'ordinateur sans la fenêtre et ne renverraient pas leur état.
    private func releaseEverything() {
        input?.releaseAll()
    }

    /// La fenêtre retrouve le clavier : les modificateurs tenus à cet instant
    /// (⌘ encore enfoncée après un ⌘Tab) repartent vers l'ordinateur, du côté
    /// réellement tenu quand le système le dit, sinon du côté gauche.
    private func keyboardRegained() {
        evaluateRelative()
        resyncModifiers()
    }

    private func resyncModifiers() {
        guard input != nil, window?.isKeyWindow == true, window?.firstResponder === self else { return }
        guard let input else { return }
        for code in input.resyncModifiers(held: Keymap.heldModifierCodes(flags: NSEvent.modifierFlags)) {
            send(keyCode: code, pressed: true)
        }
    }

    private func evaluateRelative() {
        let state = pointer.state
        let wanted = Self.rules.wantsRelative(mode: mouseMode, gameFullscreen: pcGameFullscreen, remoteHidden: state.remoteHidden,
                                              visibleFor: state.visibleFor, relativeActive: relative.active)
        let want = wanted && !relativeSuspended && input != nil && window?.isKeyWindow == true
        if want, !relative.active {
            // On n'entre qu'avec le pointeur sur l'image (pas pendant un tour dans les menus).
            guard let window, bounds.contains(convert(window.mouseLocationOutsideOfEventStream, from: nil)) else { return }
            input?.setRelative(true)
            relative.start()
            onPointerMoved?(nil)
            updatePointerLayer()
            KyberLog.app.notice("souris en mode jeu (\(self.mouseMode == .game ? "choisi" : "jeu en plein écran", privacy: .public))")
            onRelativeChanged?(true)
        } else if !want, relative.active {
            endRelative(warp: true)
            KyberLog.app.notice("souris en mode bureau")
        }
    }

    /// Le pointeur du Mac réapparaît là où est celui de l'ordinateur.
    private func endRelative(warp: Bool) {
        guard relative.active else { return }
        relative.stop()
        pointerLayer.isHidden = true
        input?.setRelative(false)
        onRelativeChanged?(false)
        guard warp, let layout, let host = pointer.state.remotePosition, let window, let screen = window.screen ?? NSScreen.screens.first else { return }
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

    /// Pointeur du Mac sur la vue, en points depuis son coin haut gauche ; nil
    /// hors de la vue. La fenêtre clé reçoit les mouvements même quand le
    /// pointeur est ailleurs (autre écran, barre de titre) : la position doit
    /// donc être vérifiée, jamais supposée.
    public var pointerSpot: CGPoint? {
        guard let window else { return nil }
        return spot(inWindow: window.mouseLocationOutsideOfEventStream)
    }

    private func spot(inWindow location: NSPoint) -> CGPoint? {
        let p = convert(location, from: nil)
        guard p.x >= 0, p.x <= bounds.width, p.y >= 0, p.y <= bounds.height else { return nil }
        return CGPoint(x: p.x, y: bounds.height - p.y)
    }

    private func sendPosition(_ event: NSEvent) {
        if relative.active {
            relative.appKitMoved(dx: event.deltaX, dy: event.deltaY)
            return
        }
        let p = convert(event.locationInWindow, from: nil)
        onPointerMoved?(spot(inWindow: event.locationInWindow))
        guard let input, let layout, !inReservedBand(event) else { return }
        var local = kynput_Position(x: p.x, y: bounds.height - p.y)
        var host = kynput_Position(x: 0, y: 0)
        guard kynput_video_layout_local_to_host(layout, &local, false, &host) else { return }
        input.mouse(x: Int16(clamping: Int(host.x.rounded())), y: Int16(clamping: Int(host.y.rounded())))
    }

    public override func mouseMoved(with event: NSEvent) { sendPosition(event) }
    public override func mouseExited(with event: NSEvent) { onPointerMoved?(nil) }
    public override func mouseDragged(with event: NSEvent) { sendPosition(event) }
    public override func rightMouseDragged(with event: NSEvent) { sendPosition(event) }
    public override func otherMouseDragged(with event: NSEvent) { sendPosition(event) }

    private func mouseButton(_ button: InputPipeline.MouseButton, pressed: Bool) {
        input?.mouseButton(button, pressed: pressed)
    }

    public override func mouseDown(with event: NSEvent) {
        window?.makeFirstResponder(self)
        if !relative.active, inReservedBand(event) { return }
        sendPosition(event)
        mouseButton(.left, pressed: true)
    }

    public override func mouseUp(with event: NSEvent) { mouseButton(.left, pressed: false) }
    public override func rightMouseDown(with event: NSEvent) { sendPosition(event); mouseButton(.right, pressed: true) }
    public override func rightMouseUp(with event: NSEvent) { mouseButton(.right, pressed: false) }

    private func otherButton(_ event: NSEvent) -> InputPipeline.MouseButton? {
        switch event.buttonNumber {
        case 2: return .middle
        case 3: return .side
        case 4: return .extra
        default: return nil
        }
    }

    public override func otherMouseDown(with event: NSEvent) {
        if let b = otherButton(event) { mouseButton(b, pressed: true) }
    }

    public override func otherMouseUp(with event: NSEvent) {
        if let b = otherButton(event) { mouseButton(b, pressed: false) }
    }

    public override func scrollWheel(with event: NSEvent) {
        // Une encoche de molette = 1.0 ; un trackpad donne des pixels (même
        // réglage que le client Kyber : 16 pixels par encoche).
        let dy = event.hasPreciseScrollingDeltas ? event.scrollingDeltaY / 16 : event.scrollingDeltaY
        guard dy != 0 else { return }
        input?.wheel(dx: 0, dy: Float(dy))
    }

    // MARK: - Clavier

    private func send(keyCode: UInt16, pressed: Bool, keyboardType: Int64? = nil) {
        guard let scancode = Keymap.scancode(keyCode: keyCode, commandAsControl: commandAsControl, keyboardType: keyboardType) else { return }
        input?.key(scancode: scancode, pressed: pressed)
    }

    /// Type du clavier qui a produit l'événement (ANSI, ISO, JIS : échange
    /// des touches `@#` et `<>` des claviers Apple ISO).
    private func keyboardType(_ event: NSEvent) -> Int64? {
        event.cgEvent?.getIntegerValueField(.keyboardEventKeyboardType)
    }

    public override func keyDown(with event: NSEvent) {
        if event.isARepeat { return } // l'ordinateur répète lui-même
        send(keyCode: event.keyCode, pressed: true, keyboardType: keyboardType(event))
    }

    public override func keyUp(with event: NSEvent) {
        send(keyCode: event.keyCode, pressed: false, keyboardType: keyboardType(event))
    }

    public override func flagsChanged(with event: NSEvent) {
        let code = event.keyCode
        guard let flag = Keymap.modifierFlags[code] else { return }
        if let hid = Keymap.hid(keyCode: code), Keymap.isLock(hid) {
            // Verr. maj. : une bascule, pas une touche tenue (voir Keymap) :
            // appui + relâchement, comme une frappe.
            send(keyCode: code, pressed: true)
            send(keyCode: code, pressed: false)
            return
        }
        // Le bit du côté dit si cette touche-ci est enfoncée (les deux ⇧ tenues
        // et une relâchée : le drapeau reste levé). Sans aucun bit de côté
        // (événements synthétiques : partage d'écran, clavier d'accessibilité),
        // le drapeau, en bascule.
        let flags = event.modifierFlags
        let raw = flags.rawValue
        let down: Bool
        if let side = Keymap.sideBit(code), raw & Keymap.sideBits(of: flag) != 0 {
            down = raw & side != 0
        } else {
            down = flags.contains(flag) && !(input?.modifierHeld(code) ?? false)
        }
        if down {
            guard input?.modifier(code, pressed: true) == true else { return }
            send(keyCode: code, pressed: true)
            return
        }
        // Relâchement : cette touche, et, si le drapeau est retombé, tout ce
        // qui le portait encore chez nous (côté resynchronisé au retour du
        // clavier, qui n'est pas forcément celui-ci).
        var codes: Set<UInt16> = [code]
        if !flags.contains(flag) {
            codes.formUnion((input?.heldModifiers() ?? []).filter { Keymap.modifierFlags[$0] == flag })
        }
        for c in codes {
            input?.modifier(c, pressed: false)
            send(keyCode: c, pressed: false)
        }
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

    public override func becomeFirstResponder() -> Bool {
        let ok = super.becomeFirstResponder()
        // La fenêtre ne nous a pas encore inscrits : au prochain tour.
        if ok { DispatchQueue.main.async { [weak self] in self?.resyncModifiers() } }
        return ok
    }

    public override func resignFirstResponder() -> Bool {
        releaseEverything()
        return super.resignFirstResponder()
    }
}
