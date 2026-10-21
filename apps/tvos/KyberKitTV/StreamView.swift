// Vue de session de l'Apple TV : l'image (Metal), le pointeur de
// l'ordinateur quand une souris sert (télécommande, manette, souris
// Bluetooth), et le branchement des entrées de GameController sur la chaîne
// de la session.
//
// Souris, comme l'app Mac en mode bureau : l'Apple TV tient la position du
// pointeur et envoie des positions absolues. Windows masque son pointeur sans
// souris branchée (ou à la moindre frappe) et ne le réaffiche que sur une
// position absolue : avec des déplacements relatifs, il restait masqué et
// l'app le perdait. Le pointeur est dessiné par l'app à cette position,
// toujours visible, avec la forme de celui de l'ordinateur. Seule une souris
// Bluetooth dans un jeu en plein écran envoie des déplacements bruts.
//
// L'état du pointeur (position tenue, forme, position de l'ordinateur) est
// dans PointerLayer, partagé avec le Mac, tenu sur la file des entrées de
// GameController (`InputQueue.device`) : manettes, télécommande et souris
// Bluetooth le déplacent depuis cette file, sans passer par le fil principal ;
// seul le dessin y remonte. La vue ne fait plus que dessiner.
//
// Même rôle que StreamView sur Mac (apps/macos/KyberKit) ; KyberClient
// l'utilise de la même façon.

@_implementationOnly import CKyber
import QuartzCore
import UIKit

public final class StreamView: UIView {
    /// Vue confiée à Kyber comme fenêtre (jamais dessinée : l'image passe par Metal).
    public let videoView = UIView()
    let surface = MetalSurface()

    /// Modes de souris de l'app Mac (réglages partagés). L'Apple TV applique
    /// Auto sans réglage : le plein écran d'un jeu suffit (`MouseModeRules`).
    public typealias MouseMode = KyberKit.MouseMode
    private static let rules = MouseModeRules(needsHiddenPointer: false)

    /// Un jeu est en plein écran sur l'ordinateur (d'après la console) : la
    /// souris Bluetooth envoie des déplacements bruts, sans pointeur dessiné.
    public var pcGameFullscreen = false {
        didSet {
            guard pcGameFullscreen != oldValue else { return }
            updateRelative()
            updatePointerLayer()
        }
    }

    /// La télécommande sert de souris (menu de session, profil Bureau).
    public var remoteMouse = false {
        didSet {
            GamepadBridge.shared.remoteMouse = remoteMouse
            updateRelative()
            updatePointerLayer()
        }
    }

    /// La manette sert de souris et de clavier (menu de session, profil Bureau).
    public var padMouse = false {
        didSet {
            GamepadBridge.shared.padMouse = padMouse
            updatePointerLayer()
        }
    }

    /// Clavier Apple ISO (Réglages) : touches `@#` et `<>` remises à leur place.
    public var appleISOKeyboard = false {
        didSet { keyboardMouse.appleISO = appleISOKeyboard }
    }

    private let keyboardMouse = KeyboardMouse()
    /// Une souris Bluetooth a bougé pendant cette session.
    private var mouseSeen = false
    private var input: InputPipeline?
    private var layout: OpaquePointer?
    private var hostSize = CGSize(width: 1920, height: 1080)
    /// Pointeur de l'ordinateur et position tenue par l'app, sur la file des entrées.
    let pointer = PointerLayer(queue: InputQueue.device, holdsPosition: true)
    /// Dernier état de la couche de pointeur, recopié sur le fil principal pour le dessin.
    private var pointerState = PointerLayer.State()

    private let pointerLayer: CALayer = {
        let layer = CALayer()
        layer.isHidden = true
        layer.zPosition = 10
        layer.magnificationFilter = .nearest
        return layer
    }()

    public override init(frame: CGRect) {
        super.init(frame: frame)
        backgroundColor = .black
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
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) { fatalError() }

    deinit {
        if let layout { kynput_video_layout_unref(layout) }
    }

    public override func layoutSubviews() {
        super.layoutSubviews()
        surface.frame = bounds
        videoView.frame = bounds
        rebuildLayout()
        updatePointerLayer()
    }

    // MARK: - Session

    /// Rappelé à chaque redémarrage de l'image sans `detach` entre les deux
    /// (KyberClient arrête la chaîne précédente puis en crée une neuve) : ce
    /// qui tenait à l'ancienne chaîne est défait d'abord, le pointeur et le
    /// menu restent en place.
    func attach(input: InputPipeline, hostSize: CGSize) {
        if self.input != nil {
            keyboardMouse.detach()
        }
        self.input = input
        self.hostSize = hostSize
        rebuildLayout()
        // La couche de pointeur envoie ses positions par la chaîne neuve ; la
        // position tenue reste.
        pointer.queue.sync {
            pointer.attach(hostSize: hostSize) { [weak input] x, y in input?.mouse(x: x, y: y) }
        }
        GamepadBridge.shared.setPointer(pointer)
        GamepadBridge.shared.attach(input)
        keyboardMouse.attach(input, pointer: pointer)
        updateRelative()
    }

    /// Entrées reliées à l'ordinateur. Après une relance de l'image, le service
    /// d'entrées de Kyber est neuf : la souris repart en brut ou en positions
    /// selon le plein écran, les modificateurs tenus sur le clavier Bluetooth
    /// repartent, et le pointeur tenu par l'app (télécommande, manette ou
    /// souris) est renvoyé à sa place. La manette en souris, elle, continue
    /// d'elle-même (GamepadBridge relance son minuteur à chaque chaîne neuve).
    func inputConnected() {
        guard input != nil else { return }
        updateRelative()
        keyboardMouse.inputConnected()
        if remoteMouse || padMouse || mouseSeen {
            pointer.queue.async { [pointer] in pointer.resendHeld() }
        }
    }

    /// L'écran de l'ordinateur a changé de taille en cours de session : le
    /// pointeur tenu par l'app reste dans l'écran, les positions suivent.
    func setHostSize(_ size: CGSize) {
        hostSize = size
        // Le pointeur garde sa place relative (le centre reste au centre), et
        // l'ordinateur l'apprend tout de suite (PointerLayer).
        pointer.queue.sync { pointer.setHostSize(size) }
        rebuildLayout()
        updatePointerLayer()
    }

    public func detach() {
        GamepadBridge.shared.setPointer(nil)
        GamepadBridge.shared.detach()
        keyboardMouse.detach()
        pointer.queue.sync { pointer.reset() }
        input?.flushKeys()
        input = nil
        mouseSeen = false
        pointerLayer.isHidden = true
    }

    /// Texte tapé au clavier de l'écran ou de l'iPhone, envoyé tel quel.
    public func type(_ text: String) {
        input?.type(text)
    }

    /// Déplacements bruts seulement pour la souris Bluetooth dans un jeu en
    /// plein écran ; sinon, les positions absolues suffisent.
    private func updateRelative() {
        let relative = Self.rules.wantsRelative(mode: .auto, gameFullscreen: pcGameFullscreen, remoteHidden: false, visibleFor: 0, relativeActive: false)
        input?.setRelative(relative)
    }

    private func rebuildLayout() {
        guard bounds.width > 0, bounds.height > 0 else { return }
        if let layout { kynput_video_layout_unref(layout) }
        var window = kynput_RectSize(width: bounds.width, height: bounds.height)
        var host = kynput_RectSize(width: hostSize.width, height: hostSize.height)
        layout = kynput_video_layout_new(&window, &host)
    }

    #if DEBUG
    private var testRenderer: VideoRenderer?
    private var testLink: CADisplayLink?
    private var testFrame = 0

    /// Mire animée (simulateur, session de démonstration) : une bande qui
    /// défile, dessinée par le même rendu Metal que le flux.
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
        let w = 640, h = 360
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

    // MARK: - Pointeur de l'ordinateur

    /// Dessiné quand une souris sert (à la manette, le jeu a son propre
    /// curseur), toujours visible : Windows masque le sien sans raison pour nous.
    /// Lisible à 3 mètres : case de 48 points, jusqu'à deux fois la forme.
    private func updatePointerLayer() {
        let mouseOn = remoteMouse || padMouse || (mouseSeen && !pcGameFullscreen)
        guard mouseOn, let host = pointerState.position, let layout else {
            pointerLayer.isHidden = true
            return
        }
        let shape = pointerState.shape ?? Self.arrow
        let factor = PointerLayer.scale(pixels: shape.pixels, box: 48, maxFactor: 2)
        var hostPos = kynput_Position(x: host.x, y: host.y)
        var local = kynput_Position(x: 0, y: 0)
        kynput_video_layout_host_to_local(layout, &hostPos, &local)
        CATransaction.begin()
        CATransaction.setDisableActions(true)
        pointerLayer.contents = shape.image
        pointerLayer.frame = PointerLayer.frame(shape: shape, local: CGPoint(x: local.x, y: local.y), factor: factor)
        pointerLayer.isHidden = false
        CATransaction.commit()
    }

    /// Flèche de secours, tant que l'ordinateur n'a pas envoyé la forme du sien.
    private static let arrow: PointerLayer.Shape = {
        let size = CGSize(width: 24, height: 32)
        let image = UIGraphicsImageRenderer(size: size, format: { let f = UIGraphicsImageRendererFormat(); f.scale = 1; return f }()).image { ctx in
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
            _ = ctx
        }
        return PointerLayer.Shape(image: image.cgImage!, hotSpot: CGPoint(x: 1, y: 1))
    }()
}
