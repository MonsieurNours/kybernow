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

/// Multi-écrans : les vues de session d'une même session, une par écran de
/// ce Mac. La règle Auto de la souris (celle du cœur) les regarde ensemble,
/// et un changement du pointeur sur l'une les fait toutes réévaluer.
public final class StreamViewGroup {
    private var members: [WeakView] = []
    private struct WeakView { weak var view: StreamView? }

    public init() {}

    public func add(_ view: StreamView) {
        members.removeAll { $0.view == nil || $0.view === view }
        members.append(WeakView(view: view))
        view.group = self
    }

    public func remove(_ view: StreamView) {
        members.removeAll { $0.view == nil || $0.view === view }
        if view.group === self { view.group = nil }
    }

    var views: [StreamView] { members.compactMap(\.view) }
}

public final class StreamView: NSView {
    /// Vue confiée à Kyber comme fenêtre (sa configuration l'exige) ; l'image
    /// n'y est pas dessinée : elle passe par Metal, dans `surface`.
    public let videoView = NSView()
    /// Image décodée par l'app, dessinée avec Metal.
    let surface = MetalSurface()

    /// « ⌘ envoyée comme Ctrl » (raccourcis Windows sous les doigts du Mac).
    public var commandAsControl = true
    /// Mode immersif (docs/link-ui.md) : les touches arrivent par le crochet
    /// de l'app (`deliverImmersive`), et ⌘ part comme la touche ⊞ : « ⌘
    /// envoyée comme Ctrl » ne s'y applique pas. Posé par le crochet à chaque
    /// touche ; le passage d'un état à l'autre relâche ce qui est tenu.
    public var immersive = false
    /// L'état dans lequel les touches tenues sont parties.
    private var sentImmersive = false
    /// Raccourcis gardés par le Mac (menus de l'app) : ⌃⌥⌘ + lettre.
    public static let reservedModifiers: NSEvent.ModifierFlags = [.control, .option, .command]
    /// Où est le pointeur du Mac sur la vue (points depuis son coin haut gauche),
    /// à chaque mouvement ; nil dès qu'il n'y est plus (sorti de la vue, sur un
    /// autre écran ou une autre app, fenêtre ou app qui perd le premier plan,
    /// souris des jeux). La barre au pointeur en vit.
    public var onPointerMoved: ((CGPoint?) -> Void)?
    /// Bande du haut réservée à l'app (barre ouverte) : rien n'y part vers l'ordinateur.
    public var reservedTopBand: CGFloat = 0
    /// Multi-écrans : l'écran de ce Mac que montre cette vue (index de la
    /// liste donnée au cœur) ; ses positions partent vers l'écran de
    /// l'ordinateur qui lui correspond. Nil : session à un écran.
    public internal(set) var screenIndex: Int?
    /// Multi-écrans : les mouvements arrivent aussi quand la fenêtre de cette
    /// vue n'est pas la fenêtre clé (le pointeur passe d'un écran à l'autre).
    public var tracksInActiveApp = false { didSet { updateTrackingAreas() } }
    /// Les vues de la même session (multi-écrans).
    public internal(set) weak var group: StreamViewGroup?

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
    /// Veille, écrans éteints, changement d'utilisateur (centre de NSWorkspace).
    private var workspaceObservers: [NSObjectProtocol] = []

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
        workspaceObservers.forEach(NSWorkspace.shared.notificationCenter.removeObserver)
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
            guard let self, event.modifierFlags.contains(.command), self.window?.isKeyWindow == true, self.window?.firstResponder === self else { return event }
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
        // Plusieurs vues sur la même chaîne : seule celle qui a la souris (ou
        // la fenêtre clé) dit son mode.
        if group == nil || relative.active || window?.isKeyWindow == true { input.setRelative(relative.active) }
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
        let activation: NSTrackingArea.Options = tracksInActiveApp ? .activeInActiveApp : .activeInKeyWindow
        let area = NSTrackingArea(rect: bounds, options: [.mouseMoved, .mouseEnteredAndExited, activation, .inVisibleRect, .cursorUpdate], owner: self)
        addTrackingArea(area)
        tracking = area
    }

    // MARK: - Curseur

    /// Case de 32 points, jamais agrandi : la taille d'un pointeur du Mac.
    private static let pointerBox: CGFloat = 32

    /// La couche de pointeur a changé (fil principal).
    private func pointerChanged(_ state: PointerLayer.State, _ change: PointerLayer.Change) {
        if change.visibility {
            // Plusieurs écrans : la règle regarde toutes les vues de la session.
            let views = group?.views ?? [self]
            views.forEach { $0.evaluateRelative() }
            if !state.remoteHidden {
                // Un pointeur remontré un instant (menus, clignotement de
                // Windows) ne fait pas quitter le mode jeu : on revérifie après.
                DispatchQueue.main.asyncAfter(deadline: .now() + MouseModeRules.recheckDelay) {
                    views.forEach { $0.evaluateRelative() }
                }
            }
        }
        if change.shape { applyCursorShape() }
        // L'état reçu, sans relire le cœur à chaque mouvement (MAC-30).
        updatePointerLayer(state)
    }

    /// Pointeur de l'ordinateur dessiné par l'app, en mode jeu seulement.
    private func updatePointerLayer(_ known: PointerLayer.State? = nil) {
        let state = known ?? pointer.state
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
        // L'image seulement quand la forme change, pas à chaque mouvement.
        if (pointerLayer.contents as AnyObject?) !== (shape.image as AnyObject) { pointerLayer.contents = shape.image }
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
        // Veille, écrans éteints, verrouillage ou autre utilisateur : l'app
        // peut garder le focus jusque-là, rien ne doit rester tenu (MAC-10).
        let workspace = NSWorkspace.shared.notificationCenter
        workspaceObservers.forEach(workspace.removeObserver)
        workspaceObservers = [NSWorkspace.willSleepNotification, NSWorkspace.screensDidSleepNotification,
                              NSWorkspace.sessionDidResignActiveNotification].map { name in
            workspace.addObserver(forName: name, object: nil, queue: .main) { [weak self] _ in self?.keyboardLost() }
        }
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

    /// Le pointeur de l'ordinateur pour la règle Auto : celui de cette vue,
    /// ou, avec plusieurs écrans, la règle du cœur sur toutes les vues de la
    /// session (`kn_pointer_summary` : un écran jamais visité ne compte pas).
    private var ruleState: (hidden: Bool, visibleFor: TimeInterval) {
        guard let group else {
            let state = pointer.state
            return (state.remoteHidden, state.visibleFor)
        }
        return PointerLayer.summary(group.views.map(\.pointer))
    }

    private func evaluateRelative() {
        let state = ruleState
        let wanted = Self.rules.wantsRelative(mode: mouseMode, gameFullscreen: pcGameFullscreen, remoteHidden: state.hidden,
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

    // MARK: - Stylet (docs/link-ui.md, « Stylet »)

    /// La tablette dit quel bout approche (pointe ou gomme) ; le stylet est
    /// près de la surface jusqu'à sa sortie de proximité.
    private var penEraser = false
    private var penNear = false
    /// Le stylet est parti en stylet à l'ordinateur : ses clics ne repartent
    /// pas en souris.
    private var penSent = false
    private var penContact = false
    private var penBarrel = false

    /// Un stylet de tablette graphique approche (la ligne « Stylet » de la
    /// barre a alors sa place, si l'ordinateur le reçoit).
    public var onPenNear: (() -> Void)?

    public override func tabletProximity(with event: NSEvent) {
        penEraser = event.pointingDeviceType == .eraser
        penNear = event.isEnteringProximity
        if penNear { onPenNear?() }
        if !penNear, penSent {
            _ = sendPen(event, near: false)
            penSent = false
        }
    }

    /// Un événement de souris qui vient du stylet.
    private func fromPen(_ event: NSEvent) -> Bool {
        switch event.type {
        case .mouseMoved, .leftMouseDown, .leftMouseUp, .leftMouseDragged,
             .rightMouseDown, .rightMouseUp, .rightMouseDragged, .otherMouseDown, .otherMouseUp, .otherMouseDragged:
            return event.subtype == .tabletPoint
        case .tabletPoint:
            return true
        default:
            return false
        }
    }

    /// Envoie l'état du stylet ; `false` : il part en souris.
    private func sendPen(_ event: NSEvent, near: Bool = true) -> Bool {
        guard let input, let layout, !relative.active else { return false }
        let p = convert(event.locationInWindow, from: nil)
        var local = kynput_Position(x: p.x, y: bounds.height - p.y)
        var host = kynput_Position(x: 0, y: 0)
        guard kynput_video_layout_local_to_host(layout, &local, false, &host) else { return false }
        let x = Int16(clamping: Int(host.x.rounded())), y = Int16(clamping: Int(host.y.rounded()))
        // Sur la bande du haut (la barre), le stylet est à l'app : l'ordinateur le voit partir.
        let inRange = near && !inReservedBand(event)
        switch event.type {
        case .leftMouseDown, .leftMouseDragged: penContact = true
        case .leftMouseUp, .mouseMoved: penContact = false
        case .rightMouseDown, .rightMouseDragged: penBarrel = true
        case .rightMouseUp: penBarrel = false
        default: break
        }
        if !near { penContact = false; penBarrel = false }
        let tablet = event.type == .tabletProximity ? nil : event
        var pen = InputPipeline.Pen()
        pen.inRange = inRange
        pen.contact = inRange && penContact
        pen.pressure = tablet.map { $0.pressure } ?? 0
        if let tablet {
            // NSEvent : -1 à 1 de chaque côté, Y positif vers soi ; 64° au plus (Wacom).
            if tablet.tilt != .zero { pen.tilt = (Float(tablet.tilt.x) * 64, Float(tablet.tilt.y) * 64) }
            if tablet.rotation != 0 { pen.rotation = tablet.rotation }
        }
        pen.eraser = penEraser
        // Le bouton du stylet : le clic droit que la tablette en fait.
        pen.barrel = inRange && penBarrel
        return input.pen(on: screenIndex, x: x, y: y, pen)
    }

    /// Le stylet passe par `sendPen` ; vrai s'il est parti en stylet.
    private func handledAsPen(_ event: NSEvent) -> Bool {
        guard fromPen(event) else { return false }
        onPointerMoved?(spot(inWindow: event.locationInWindow))
        penSent = sendPen(event)
        return penSent
    }

    public override func tabletPoint(with event: NSEvent) { _ = handledAsPen(event) }

    private func sendPosition(_ event: NSEvent) {
        if handledAsPen(event) { return }
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
        let x = Int16(clamping: Int(host.x.rounded())), y = Int16(clamping: Int(host.y.rounded()))
        if let screenIndex { input.mouse(on: screenIndex, x: x, y: y) } else { input.mouse(x: x, y: y) }
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
        if handledAsPen(event) { return }
        sendPosition(event)
        mouseButton(.left, pressed: true)
    }

    public override func mouseUp(with event: NSEvent) {
        if handledAsPen(event) { return }
        mouseButton(.left, pressed: false)
    }

    public override func rightMouseDown(with event: NSEvent) {
        // La bande réservée à la barre ne clique jamais sur l'ordinateur
        // (MAC-09) ; les relâchements passent toujours.
        if !relative.active, inReservedBand(event) { return }
        if handledAsPen(event) { return }
        sendPosition(event); mouseButton(.right, pressed: true)
    }

    public override func rightMouseUp(with event: NSEvent) {
        if handledAsPen(event) { return }
        mouseButton(.right, pressed: false)
    }

    private func otherButton(_ event: NSEvent) -> InputPipeline.MouseButton? {
        switch event.buttonNumber {
        case 2: return .middle
        case 3: return .side
        case 4: return .extra
        default: return nil
        }
    }

    public override func otherMouseDown(with event: NSEvent) {
        if !relative.active, inReservedBand(event) { return }
        if let b = otherButton(event) { mouseButton(b, pressed: true) }
    }

    public override func otherMouseUp(with event: NSEvent) {
        if let b = otherButton(event) { mouseButton(b, pressed: false) }
    }

    public override func scrollWheel(with event: NSEvent) {
        if !relative.active, inReservedBand(event) { return }
        // Une encoche de molette = 1.0 ; un trackpad donne des pixels (même
        // réglage que le client Kyber : 16 pixels par encoche).
        let dy = event.hasPreciseScrollingDeltas ? event.scrollingDeltaY / 16 : event.scrollingDeltaY
        guard dy != 0 else { return }
        input?.wheel(dx: 0, dy: Float(dy))
    }

    // MARK: - Clavier

    private func send(keyCode: UInt16, pressed: Bool, keyboardType: Int64? = nil) {
        if immersive != sentImmersive {
            // ⌘ change de touche (Ctrl ↔ ⊞) : ce qui est tenu se relâche tel
            // qu'il est parti, puis les modificateurs encore tenus repartent.
            sentImmersive = immersive
            if let input {
                input.releaseAll()
                input.forgetModifiers()
                if pressed { resyncModifiers() }
            }
        }
        guard let scancode = Keymap.scancode(keyCode: keyCode, commandAsControl: commandAsControl && !immersive, keyboardType: keyboardType) else { return }
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

    /// Une touche prise par le crochet du mode immersif, avant le système
    /// (⌘ ⇥, ⌘ Espace, Mission Control) : elle va à l'ordinateur, comme une
    /// touche tapée dans la vue.
    public func deliverImmersive(_ event: NSEvent) {
        switch event.type {
        case .keyDown: keyDown(with: event)
        case .keyUp: keyUp(with: event)
        case .flagsChanged: flagsChanged(with: event)
        default: break
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
