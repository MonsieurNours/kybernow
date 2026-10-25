// Vue de session de l'iPhone et de l'iPad : l'image (Metal), le pointeur de
// l'ordinateur, et le toucher, le clavier de l'écran, le clavier et le
// trackpad (ou la souris) de l'iPad, les manettes, branchés sur la chaîne
// d'entrées du cœur (InputCore.swift). Même rôle que StreamView sur Mac et
// sur Apple TV ; SessionEngine l'utilise de la même façon.
//
// Toucher (maquettes iOS, planche 4) :
// - Pavé tactile (par défaut) : tout l'écran est le pavé. Un doigt déplace le
//   pointeur de l'ordinateur en relatif (la position est tenue par l'app,
//   comme sur l'Apple TV, et envoyée en absolu) ; un toucher clique ; deux
//   doigts touchent = clic droit ; deux doigts glissent = molette ; un doigt
//   maintenu puis déplacé = glisser.
// - Toucher direct : le doigt est le pointeur, là où il touche (positions
//   absolues, comme le client web) ; toucher = clic, maintenir = glisser.
// - Jeu en plein écran sur l'ordinateur (mode Auto) ou mode Jeu : le pavé
//   envoie des déplacements bruts, sans pointeur dessiné.
//
// Clavier de l'écran : la vue est un `UIKeyInput` ; le texte part en Unicode
// (accents, emoji), Retour, Effacer et les touches de la rangée au-dessus du
// clavier (vue accessoire, fournie par l'app) partent en codes. Un toucher sur
// l'image le replie, sans cliquer sur l'ordinateur (l'iPhone n'a pas de
// touche pour le faire).
//
// Manette à l'écran (dessinée par l'app par-dessus l'image) : tant qu'elle
// est affichée (`touchAsMouse` à false), le toucher ne sert plus de souris ;
// le glissement depuis le bord haut ouvre toujours la barre, un toucher
// replie le clavier ou ferme la barre. Le trackpad et la souris de l'iPad
// gardent leur effet.
//
// Écran externe (USB-C, AirPlay ; iPhone et iPad) : l'image et le pointeur
// de l'ordinateur passent dans une `ImageHostView` posée sur l'écran externe
// (`showImage(in:)`) ; la vue reste sur l'appareil, qui devient le pupitre :
// tout l'écran est le pavé tactile (le toucher direct n'a plus de sens), et
// le trackpad de l'iPad déplace le pointeur en relatif.

@_implementationOnly import CKyber
import GameController
import QuartzCore
import UIKit

/// Là où l'image se montre quand elle n'est pas dans la vue de session
/// (l'écran externe).
public final class ImageHostView: UIView {
    var onLayout: (() -> Void)?

    public override init(frame: CGRect) {
        super.init(frame: frame)
        backgroundColor = .black
        isUserInteractionEnabled = false
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) { fatalError() }

    public override func layoutSubviews() {
        super.layoutSubviews()
        onLayout?()
    }
}

public final class StreamView: UIView, UIKeyInput, UIGestureRecognizerDelegate {
    /// Vue confiée au cœur comme fenêtre (jamais dessinée : l'image passe par Metal).
    public let videoView = UIView()
    let surface = MetalSurface()

    /// Auto (jeu en plein écran : déplacements bruts), Bureau, Jeu.
    public typealias MouseMode = KyberKit.MouseMode
    public var mouseMode: MouseMode = .auto { didSet { updateRelative() } }
    private static let rules = MouseModeRules(needsHiddenPointer: false)

    /// Un jeu est en plein écran sur l'ordinateur (d'après Admin).
    public var pcGameFullscreen = false {
        didSet {
            guard pcGameFullscreen != oldValue else { return }
            updateRelative()
            updatePointerLayer()
        }
    }

    /// Toucher direct (le doigt est le pointeur) plutôt que pavé tactile.
    public var directTouch = false { didSet { updatePointerLayer() } }
    /// Vitesse du pointeur au pavé tactile.
    public var pointerSpeed: Double = 1.4
    /// Clavier Apple ISO (Réglages) : touches `@#` et `<>` remises à leur place.
    public var appleISOKeyboard = false {
        didSet { keyboardMouse.appleISO = appleISOKeyboard }
    }
    /// « ⌘ envoyée comme Ctrl » (réglage de l'ordinateur) : clavier externe.
    public var commandAsControl = true {
        didSet { keyboardMouse.commandAsControl = commandAsControl }
    }
    /// Le toucher sert l'app (barre ouverte, feuille des mesures) : rien ne part.
    public var touchesSuspended = false
    /// Le doigt sert de souris (pavé tactile ou toucher direct) ; false pendant
    /// que la manette à l'écran est affichée.
    public var touchAsMouse = true {
        didSet {
            // Un glisser en cours ne reste pas enfoncé sur l'ordinateur.
            guard !touchAsMouse, dragging else { return }
            input?.mouseButton(.left, pressed: false)
            dragging = false
        }
    }
    /// Un toucher simple sur l'image (barre ouverte : la refermer).
    public var onTap: (() -> Void)?
    /// Glissement depuis le bord haut (ouvrir la barre).
    public var onTopEdgeSwipe: (() -> Void)?
    /// La souris est-elle en mode brut (jeu) en ce moment.
    public private(set) var isRelative = false
    /// L'image est sur un autre écran : l'appareil n'est plus que le pupitre.
    public private(set) var imageHost: ImageHostView?
    /// Dernière position du trackpad (image sur l'écran externe : en relatif).
    private var lastHover: CGPoint?

    private let keyboardMouse = KeyboardMouse()
    private var input: InputPipeline?
    private var layout: OpaquePointer?
    private var hostSize = CGSize(width: 1920, height: 1080)
    /// Pointeur de l'ordinateur et position tenue par l'app, dans le cœur.
    let pointer = PointerLayer(queue: InputQueue.device, holdsPosition: true)
    private var pointerState = PointerLayer.State()
    /// Une souris ou un trackpad a servi pendant cette session.
    private var mouseSeen = false
    /// Glisser en cours (bouton gauche tenu par un maintien).
    private var dragging = false
    private var wheelRest: CGFloat = 0
    /// Le toucher simple et le glissement depuis le bord haut : les seuls
    /// gestes du doigt qui restent quand la manette à l'écran est affichée.
    private var tapGesture: UITapGestureRecognizer?

    private let pointerLayer: CALayer = {
        let layer = CALayer()
        layer.isHidden = true
        layer.zPosition = 10
        return layer
    }()

    public override init(frame: CGRect) {
        super.init(frame: frame)
        backgroundColor = .black
        isMultipleTouchEnabled = true
        videoView.isHidden = true
        addSubview(videoView)
        addSubview(surface)
        layer.addSublayer(pointerLayer)
        keyboardMouse.onMouseActivity = { [weak self] in
            guard let self, !self.mouseSeen else { return }
            self.mouseSeen = true
            self.updatePointerLayer()
        }
        pointer.onChanged = { [weak self] state, _ in
            DispatchQueue.main.async {
                guard let self else { return }
                self.pointerState = state
                self.updatePointerLayer()
            }
        }
        installGestures()
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) { fatalError() }

    deinit {
        if let layout { kynput_video_layout_unref(layout) }
    }

    public override func layoutSubviews() {
        super.layoutSubviews()
        if imageHost == nil { surface.frame = bounds }
        videoView.frame = bounds
        rebuildLayout()
        updatePointerLayer()
    }

    /// Les bornes de l'image : la vue elle-même, ou l'écran externe.
    private var imageBounds: CGRect { imageHost?.bounds ?? bounds }

    /// Montre l'image (et le pointeur de l'ordinateur) dans `host`, sur un
    /// autre écran, ou de nouveau ici (`nil`). La surface Metal et son rendu
    /// sont déplacés tels quels : le flux continue sans relance.
    public func showImage(in host: ImageHostView?) {
        guard host !== imageHost else { return }
        imageHost?.onLayout = nil
        imageHost = host
        CATransaction.begin()
        CATransaction.setDisableActions(true)
        if let host {
            host.addSubview(surface)
            surface.frame = host.bounds
            host.layer.addSublayer(pointerLayer)
            host.onLayout = { [weak self, weak host] in
                guard let self, let host else { return }
                self.surface.frame = host.bounds
                self.rebuildLayout()
                self.updatePointerLayer()
            }
        } else {
            insertSubview(surface, aboveSubview: videoView)
            surface.frame = bounds
            layer.addSublayer(pointerLayer)
        }
        CATransaction.commit()
        lastHover = nil
        rebuildLayout()
        updatePointerLayer()
    }

    /// Le toucher direct n'a de sens que si l'image est sous le doigt.
    private var direct: Bool { directTouch && imageHost == nil }

    // MARK: - Session

    /// Rappelé à chaque redémarrage de l'image sans `detach` entre les deux
    /// (le cœur arrête la chaîne précédente, SessionEngine en crée une neuve).
    func attach(input: InputPipeline, hostSize: CGSize) {
        if self.input != nil { keyboardMouse.detach() }
        self.input = input
        self.hostSize = hostSize
        rebuildLayout()
        GamepadBridge.shared.setPointer(pointer)
        GamepadBridge.shared.attach(input)
        keyboardMouse.attach(input, pointer: pointer)
        updateRelative()
    }

    /// Entrées reliées à l'ordinateur (service d'entrées neuf après une relance).
    func inputConnected() {
        guard input != nil else { return }
        updateRelative()
        keyboardMouse.inputConnected()
        pointer.resendHeld()
    }

    /// L'écran de l'ordinateur a changé de taille en cours de session.
    func setHostSize(_ size: CGSize) {
        hostSize = size
        pointer.setHostSize(size)
        rebuildLayout()
        updatePointerLayer()
    }

    public func detach() {
        GamepadBridge.shared.setPointer(nil)
        GamepadBridge.shared.detach()
        keyboardMouse.detach()
        pointer.reset()
        if dragging { input?.mouseButton(.left, pressed: false) }
        dragging = false
        input?.flushKeys()
        input = nil
        mouseSeen = false
        pointerLayer.isHidden = true
        if isFirstResponder { resignFirstResponder() }
    }

    /// Arrière-plan, perte du premier plan (Stage Manager) : rien ne reste
    /// enfoncé sur l'ordinateur.
    public func releaseAll() {
        if dragging { input?.mouseButton(.left, pressed: false) }
        dragging = false
        input?.releaseAll()
    }

    private func updateRelative() {
        let relative = Self.rules.wantsRelative(mode: mouseMode, gameFullscreen: pcGameFullscreen, remoteHidden: false, visibleFor: 0, relativeActive: false)
        isRelative = relative
        input?.setRelative(relative)
        updatePointerLayer()
    }

    private func rebuildLayout() {
        let bounds = imageBounds
        guard bounds.width > 0, bounds.height > 0 else { return }
        if let layout { kynput_video_layout_unref(layout) }
        var window = kynput_RectSize(width: bounds.width, height: bounds.height)
        var host = kynput_RectSize(width: hostSize.width, height: hostSize.height)
        layout = kynput_video_layout_new(&window, &host)
    }

    /// Point de la vue → pixels de l'écran de l'ordinateur (nil : hors de l'image).
    private func hostPoint(_ p: CGPoint) -> CGPoint? {
        guard let layout else { return nil }
        var local = kynput_Position(x: p.x, y: p.y)
        var host = kynput_Position(x: 0, y: 0)
        guard kynput_video_layout_local_to_host(layout, &local, true, &host) else { return nil }
        return CGPoint(x: host.x, y: host.y)
    }

    // MARK: - Toucher

    private func installGestures() {
        let pan = UIPanGestureRecognizer(target: self, action: #selector(panned(_:)))
        pan.maximumNumberOfTouches = 1
        pan.allowedTouchTypes = [UITouch.TouchType.direct.rawValue as NSNumber]
        pan.delegate = self
        addGestureRecognizer(pan)

        let scroll = UIPanGestureRecognizer(target: self, action: #selector(scrolled(_:)))
        scroll.minimumNumberOfTouches = 2
        scroll.maximumNumberOfTouches = 2
        scroll.allowedScrollTypesMask = .all
        scroll.delegate = self
        addGestureRecognizer(scroll)

        let tap = UITapGestureRecognizer(target: self, action: #selector(tapped(_:)))
        tap.allowedTouchTypes = [UITouch.TouchType.direct.rawValue as NSNumber]
        tap.delegate = self
        addGestureRecognizer(tap)
        tapGesture = tap

        let twoTap = UITapGestureRecognizer(target: self, action: #selector(twoFingerTapped(_:)))
        twoTap.numberOfTouchesRequired = 2
        twoTap.delegate = self
        addGestureRecognizer(twoTap)

        let hold = UILongPressGestureRecognizer(target: self, action: #selector(held(_:)))
        hold.minimumPressDuration = 0.35
        hold.allowedTouchTypes = [UITouch.TouchType.direct.rawValue as NSNumber]
        hold.delegate = self
        addGestureRecognizer(hold)

        let edge = UIScreenEdgePanGestureRecognizer(target: self, action: #selector(edgeSwiped(_:)))
        edge.edges = .top
        edge.delegate = self
        addGestureRecognizer(edge)

        // Trackpad ou souris de l'iPad : le pointeur iPadOS survole l'image,
        // la position part en absolu (le pointeur de l'ordinateur y est dessiné).
        let hover = UIHoverGestureRecognizer(target: self, action: #selector(hovered(_:)))
        addGestureRecognizer(hover)
        addInteraction(UIPointerInteraction(delegate: self))
    }

    /// Manette à l'écran affichée : le doigt ne déplace ni ne clique plus.
    public func gestureRecognizer(_ g: UIGestureRecognizer, shouldReceive touch: UITouch) -> Bool {
        guard !touchAsMouse, touch.type == .direct else { return true }
        return g === tapGesture || g is UIScreenEdgePanGestureRecognizer
    }

    public func gestureRecognizer(_ g: UIGestureRecognizer, shouldRecognizeSimultaneouslyWith other: UIGestureRecognizer) -> Bool {
        // Le maintien et le déplacement d'un doigt vont ensemble (glisser).
        (g is UILongPressGestureRecognizer && other is UIPanGestureRecognizer) || (g is UIPanGestureRecognizer && other is UILongPressGestureRecognizer)
    }

    @objc private func panned(_ g: UIPanGestureRecognizer) {
        guard !touchesSuspended, let input else { return }
        let t = g.translation(in: self)
        g.setTranslation(.zero, in: self)
        if isRelative {
            // Jeu : déplacements bruts, en pixels de l'ordinateur.
            let k = pointerSpeed * Double(hostSize.width / max(bounds.width, 1))
            input.mouseMove(dx: Int16(clamping: Int((Double(t.x) * k).rounded())), dy: Int16(clamping: Int((Double(t.y) * k).rounded())))
            return
        }
        if direct {
            if let p = hostPoint(g.location(in: self)) {
                input.mouse(x: Int16(clamping: Int(p.x.rounded())), y: Int16(clamping: Int(p.y.rounded())))
            }
            return
        }
        // Pavé tactile : la position tenue avance à l'échelle de l'écran de l'ordinateur.
        let k = pointerSpeed * Double(hostSize.width / max(bounds.width, 1))
        pointer.move(dx: Double(t.x) * k, dy: Double(t.y) * k)
    }

    /// Dernier toucher en toucher direct : un second toucher rapide et proche
    /// reprend sa position. Piège : Windows ne compte un double clic que si le
    /// second clic tombe à 4 pixels du premier au plus, et deux touchers d'un
    /// doigt ne tombent jamais au même endroit (10 à 30 pixels d'écart sur
    /// l'écran de l'ordinateur) : sans cela, le double clic ne marche pas.
    private var lastTap: (view: CGPoint, host: CGPoint, at: CFTimeInterval)?

    @objc private func tapped(_ g: UITapGestureRecognizer) {
        switch TouchRules.tap(keyboardShown: isFirstResponder, barOpen: touchesSuspended, touchAsMouse: touchAsMouse) {
        case .foldKeyboard:
            hideKeyboard()
            return
        case .closeBar:
            onTap?()
            return
        case .nothing:
            return
        case .click:
            break
        }
        guard let input else { return }
        let at = g.location(in: self)
        if direct, !isRelative, var p = hostPoint(at) {
            let now = CACurrentMediaTime()
            if let last = lastTap, now - last.at < 0.5, hypot(at.x - last.view.x, at.y - last.view.y) < 24 {
                p = last.host
            }
            lastTap = (at, p, now)
            input.mouse(x: Int16(clamping: Int(p.x.rounded())), y: Int16(clamping: Int(p.y.rounded())))
        }
        input.mouseButton(.left, pressed: true)
        input.mouseButton(.left, pressed: false)
    }

    @objc private func twoFingerTapped(_ g: UITapGestureRecognizer) {
        guard !touchesSuspended, let input else { return }
        input.mouseButton(.right, pressed: true)
        input.mouseButton(.right, pressed: false)
    }

    @objc private func held(_ g: UILongPressGestureRecognizer) {
        guard !touchesSuspended, let input else { return }
        switch g.state {
        case .began:
            if direct, !isRelative, let p = hostPoint(g.location(in: self)) {
                input.mouse(x: Int16(clamping: Int(p.x.rounded())), y: Int16(clamping: Int(p.y.rounded())))
            }
            dragging = true
            input.mouseButton(.left, pressed: true)
            UIImpactFeedbackGenerator(style: .light).impactOccurred()
        case .ended, .cancelled, .failed:
            if dragging { input.mouseButton(.left, pressed: false) }
            dragging = false
        default:
            break
        }
    }

    @objc private func scrolled(_ g: UIPanGestureRecognizer) {
        guard !touchesSuspended, let input else { return }
        let t = g.translation(in: self)
        g.setTranslation(.zero, in: self)
        // Une encoche de molette pour 16 points, comme le trackpad du Mac.
        wheelRest += t.y / 16
        let steps = wheelRest.rounded(.towardZero)
        guard steps != 0 else { return }
        wheelRest -= steps
        input.wheel(dx: 0, dy: Float(steps))
    }

    @objc private func edgeSwiped(_ g: UIScreenEdgePanGestureRecognizer) {
        if g.state == .began { onTopEdgeSwipe?() }
    }

    @objc private func hovered(_ g: UIHoverGestureRecognizer) {
        guard !touchesSuspended, let input, !isRelative else { return }
        if !mouseSeen {
            mouseSeen = true
            updatePointerLayer()
        }
        if imageHost != nil {
            // L'image est ailleurs : le trackpad pousse le pointeur, en relatif.
            let at = g.location(in: self)
            defer { lastHover = g.state == .ended || g.state == .cancelled ? nil : at }
            guard let last = lastHover else { return }
            let k = pointerSpeed * Double(hostSize.width / max(bounds.width, 1))
            pointer.move(dx: Double(at.x - last.x) * k, dy: Double(at.y - last.y) * k)
            return
        }
        if let p = hostPoint(g.location(in: self)) {
            input.mouse(x: Int16(clamping: Int(p.x.rounded())), y: Int16(clamping: Int(p.y.rounded())))
        }
    }

    // MARK: - Clavier de l'écran

    public override var canBecomeFirstResponder: Bool { true }

    /// Rangée de touches au-dessus du clavier (fournie par l'app).
    public var keyboardAccessory: UIView?
    public override var inputAccessoryView: UIView? { keyboardAccessory }

    public var hasText: Bool { true }

    public func insertText(_ text: String) {
        guard let input else { return }
        if text == "\n" {
            tap(usage: 0x28)
            return
        }
        if text == "\t" {
            tap(usage: 0x2B)
            return
        }
        input.type(text)
        releaseStickyModifiers()
    }

    public func deleteBackward() {
        tap(usage: 0x2A)
    }

    public var autocorrectionType: UITextAutocorrectionType { get { .no } set {} }
    public var autocapitalizationType: UITextAutocapitalizationType { get { .none } set {} }
    public var smartQuotesType: UITextSmartQuotesType { get { .no } set {} }
    public var smartDashesType: UITextSmartDashesType { get { .no } set {} }
    public var spellCheckingType: UITextSpellCheckingType { get { .no } set {} }
    public var keyboardAppearance: UIKeyboardAppearance { get { .dark } set {} }

    public func showKeyboard() { becomeFirstResponder() }
    public func hideKeyboard() { resignFirstResponder() }

    // MARK: - Touches spéciales (rangée, menu Touches)

    /// Ctrl, Alt et ⊞ collants : allumés tant qu'ils sont tenus, relâchés
    /// après la frappe suivante.
    public private(set) var sticky: Set<UInt16> = []
    public var onStickyChanged: ((Set<UInt16>) -> Void)?

    /// Appui ou relâchement d'une touche (usage HID).
    public func press(usage: UInt16, pressed: Bool) {
        guard let input, let scancode = Keymap.scancode(usage: usage) else { return }
        input.key(scancode: scancode, pressed: pressed)
    }

    /// Appui puis relâchement ; les touches collantes partent après.
    public func tap(usage: UInt16) {
        press(usage: usage, pressed: true)
        press(usage: usage, pressed: false)
        releaseStickyModifiers()
    }

    /// Raccourci : les modificateurs tenus, la touche, puis tout relâché.
    public func chord(_ modifiers: [UInt16], _ usage: UInt16) {
        for m in modifiers { press(usage: m, pressed: true) }
        press(usage: usage, pressed: true)
        press(usage: usage, pressed: false)
        for m in modifiers.reversed() { press(usage: m, pressed: false) }
    }

    /// Ctrl, Alt ou ⊞ basculé (rangée de touches).
    public func toggleSticky(_ usage: UInt16) {
        if sticky.contains(usage) {
            sticky.remove(usage)
            press(usage: usage, pressed: false)
        } else {
            sticky.insert(usage)
            press(usage: usage, pressed: true)
        }
        onStickyChanged?(sticky)
    }

    private func releaseStickyModifiers() {
        guard !sticky.isEmpty else { return }
        for usage in sticky { press(usage: usage, pressed: false) }
        sticky = []
        onStickyChanged?(sticky)
    }

    // MARK: - Pointeur de l'ordinateur

    /// Dessiné au pavé tactile, à la souris ou au trackpad (en toucher direct,
    /// le doigt suffit ; en jeu, le jeu a son propre curseur).
    private func updatePointerLayer() {
        let visible = !isRelative && (!direct || mouseSeen)
        guard visible, let host = pointerState.remotePosition, let layout else {
            pointerLayer.isHidden = true
            return
        }
        let shape = pointerState.shape ?? Self.arrow
        let factor = PointerLayer.scale(pixels: shape.pixels, box: 28, maxFactor: 1.5)
        var hostPos = kynput_Position(x: host.x, y: host.y)
        var local = kynput_Position(x: 0, y: 0)
        kynput_video_layout_host_to_local(layout, &hostPos, &local)
        CATransaction.begin()
        CATransaction.setDisableActions(true)
        pointerLayer.contents = shape.image
        pointerLayer.contentsScale = (imageHost?.window ?? window)?.screen.scale ?? 3
        pointerLayer.frame = PointerLayer.frame(shape: shape, local: CGPoint(x: local.x, y: local.y), factor: factor)
        pointerLayer.isHidden = false
        CATransaction.commit()
    }

    /// Flèche de secours, tant que l'ordinateur n'a pas envoyé la forme du sien.
    private static let arrow: PointerLayer.Shape = {
        let size = CGSize(width: 24, height: 32)
        let image = UIGraphicsImageRenderer(size: size, format: { let f = UIGraphicsImageRendererFormat(); f.scale = 1; return f }()).image { _ in
            let path = UIBezierPath()
            path.move(to: CGPoint(x: 1, y: 1))
            path.addLine(to: CGPoint(x: 1, y: 26))
            path.addLine(to: CGPoint(x: 7.5, y: 20))
            path.addLine(to: CGPoint(x: 12, y: 30))
            path.addLine(to: CGPoint(x: 16, y: 28))
            path.addLine(to: CGPoint(x: 11.5, y: 18.5))
            path.addLine(to: CGPoint(x: 20, y: 18.5))
            path.close()
            UIColor.white.setFill()
            path.fill()
            UIColor.black.setStroke()
            path.lineWidth = 1.5
            path.stroke()
        }
        return PointerLayer.Shape(image: image.cgImage!, hotSpot: CGPoint(x: 1, y: 1))
    }()

    #if DEBUG
    private var testRenderer: VideoRenderer?
    private var testLink: CADisplayLink?
    private var testFrame = 0

    /// Mire animée (simulateur, session de démonstration).
    public func startTestPattern() {
        guard testRenderer == nil, let renderer = VideoRenderer() else { return }
        testRenderer = renderer
        surface.show(renderer)
        let link = CADisplayLink(target: self, selector: #selector(testTick))
        link.add(to: .main, forMode: .common)
        testLink = link
    }

    @objc private func testTick() {
        guard let renderer = testRenderer else { return }
        let w = 640, h = 296
        var buffer: CVPixelBuffer?
        let attrs: [CFString: Any] = [kCVPixelBufferIOSurfacePropertiesKey: [CFString: Any](), kCVPixelBufferMetalCompatibilityKey: true]
        guard CVPixelBufferCreate(nil, w, h, kCVPixelFormatType_420YpCbCr8BiPlanarVideoRange, attrs as CFDictionary, &buffer) == kCVReturnSuccess, let buffer else { return }
        CVPixelBufferLockBaseAddress(buffer, [])
        let band = (testFrame * 6) % w
        if let y = CVPixelBufferGetBaseAddressOfPlane(buffer, 0) {
            let stride = CVPixelBufferGetBytesPerRowOfPlane(buffer, 0)
            for row in 0..<h {
                let line = y.advanced(by: row * stride).assumingMemoryBound(to: UInt8.self)
                for x in 0..<w { line[x] = abs(x - band) < 24 ? 235 : 40 }
            }
        }
        if let uv = CVPixelBufferGetBaseAddressOfPlane(buffer, 1) {
            memset(uv, 128, CVPixelBufferGetBytesPerRowOfPlane(buffer, 1) * h / 2)
        }
        CVPixelBufferUnlockBaseAddress(buffer, [])
        testFrame += 1
        let now = mach_absolute_time()
        renderer.submit(DecodedFrame(buffer: buffer, pts: UInt64(testFrame), received: now, decoded: now))
    }
    #endif
}

// Le pointeur iPadOS est caché sur l'image : celui de l'ordinateur le remplace.
extension StreamView: UIPointerInteractionDelegate {
    public func pointerInteraction(_ interaction: UIPointerInteraction, styleFor region: UIPointerRegion) -> UIPointerStyle? {
        .hidden()
    }
}
