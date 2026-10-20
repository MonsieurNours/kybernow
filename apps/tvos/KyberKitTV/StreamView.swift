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
// Même rôle que StreamView sur Mac (apps/macos/KyberKit) ; KyberClient
// l'utilise de la même façon.

@_implementationOnly import CKyber
import QuartzCore
import UIKit

public final class StreamView: UIView {
    /// Vue confiée à Kyber comme fenêtre (jamais dessinée : l'image passe par Metal).
    public let videoView = UIView()
    let surface = MetalSurface()

    /// Modes de souris de l'app Mac (réglages partagés).
    public enum MouseMode { case auto, desktop, game }

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
    private var remoteHidden = false
    private var remoteShape: (image: CGImage, hotSpot: CGPoint)?
    private var remotePosition: CGPoint?
    /// Position du pointeur tenue par l'app, en pixels de l'écran de l'ordinateur.
    private var pointer: CGPoint?
    private var lastLocalMove = 0.0

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
            GamepadBridge.shared.pointerMove = nil
        }
        self.input = input
        self.hostSize = hostSize
        rebuildLayout()
        GamepadBridge.shared.pointerMove = { [weak self] dx, dy in self?.movePointer(dx: dx, dy: dy) }
        GamepadBridge.shared.attach(input)
        keyboardMouse.onMove = { [weak self] dx, dy in
            guard let self else { return false }
            // Jeu en plein écran : déplacements bruts, comme une souris de jeu.
            if self.pcGameFullscreen { return false }
            self.movePointer(dx: dx, dy: dy)
            return true
        }
        keyboardMouse.attach(input)
        updateRelative()
    }

    /// Entrées reliées à l'ordinateur. Après une relance de l'image, le service
    /// d'entrées de Kyber est neuf : la souris repart en brut ou en positions
    /// selon le plein écran, les modificateurs tenus sur le clavier Bluetooth
    /// repartent, et le pointeur tenu par l'app (télécommande, manette ou
    /// souris) est renvoyé à sa place. La manette en souris, elle, continue
    /// d'elle-même (GamepadBridge relance son minuteur à chaque chaîne neuve).
    func inputConnected() {
        guard let input else { return }
        updateRelative()
        keyboardMouse.resendHeldModifiers()
        if remoteMouse || padMouse || mouseSeen, let p = pointer {
            input.mouse(x: Int16(clamping: Int(p.x.rounded())), y: Int16(clamping: Int(p.y.rounded())))
        }
    }

    /// L'écran de l'ordinateur a changé de taille en cours de session : le
    /// pointeur tenu par l'app reste dans l'écran, les positions suivent.
    func setHostSize(_ size: CGSize) {
        let old = hostSize
        hostSize = size
        // Le pointeur garde sa place relative (le centre reste au centre), et
        // l'ordinateur l'apprend tout de suite.
        if let p = pointer, old.width > 0, old.height > 0 {
            let q = CGPoint(x: min(max(0, p.x * size.width / old.width), size.width - 1),
                            y: min(max(0, p.y * size.height / old.height), size.height - 1))
            pointer = q
            input?.mouse(x: Int16(clamping: Int(q.x.rounded())), y: Int16(clamping: Int(q.y.rounded())))
        }
        rebuildLayout()
        updatePointerLayer()
    }

    public func detach() {
        GamepadBridge.shared.pointerMove = nil
        GamepadBridge.shared.detach()
        keyboardMouse.onMove = nil
        keyboardMouse.detach()
        pointer = nil
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
        input?.setRelative(pcGameFullscreen)
    }

    /// Déplace le pointeur tenu par l'app (en pixels de l'écran de l'ordinateur)
    /// et envoie sa position.
    func movePointer(dx: Double, dy: Double) {
        guard let input, hostSize.width > 0, hostSize.height > 0 else { return }
        var p = pointer ?? remotePosition ?? CGPoint(x: hostSize.width / 2, y: hostSize.height / 2)
        p.x = min(max(0, p.x + dx), hostSize.width - 1)
        p.y = min(max(0, p.y + dy), hostSize.height - 1)
        pointer = p
        lastLocalMove = CACurrentMediaTime()
        input.mouse(x: Int16(clamping: Int(p.x.rounded())), y: Int16(clamping: Int(p.y.rounded())))
        updatePointerLayer()
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

    func remoteCursorChanged(_ remote: RemoteCursor) {
        remotePosition = remote.position
        remoteHidden = !remote.visible
        // L'ordinateur a déplacé son pointeur lui-même (au repos de nos
        // entrées) : on repart de sa position.
        if CACurrentMediaTime() - lastLocalMove > 0.4 { pointer = remote.position }
        if let image = remote.image?.cgImage {
            remoteShape = (image, remote.hotSpot)
        }
        updatePointerLayer()
    }

    /// Dessiné quand une souris sert (à la manette, le jeu a son propre
    /// curseur), toujours visible : Windows masque le sien sans raison pour nous.
    private func updatePointerLayer() {
        let mouseOn = remoteMouse || padMouse || (mouseSeen && !pcGameFullscreen)
        guard mouseOn, let host = pointer ?? remotePosition, let layout else {
            pointerLayer.isHidden = true
            return
        }
        let shape = remoteShape ?? Self.arrow
        // Lisible à 3 mètres : deux fois la taille d'un pointeur de bureau.
        let pixels = CGSize(width: shape.image.width, height: shape.image.height)
        let factor = min(2, 48 / max(pixels.width, pixels.height, 1))
        var hostPos = kynput_Position(x: host.x, y: host.y)
        var local = kynput_Position(x: 0, y: 0)
        kynput_video_layout_host_to_local(layout, &hostPos, &local)
        CATransaction.begin()
        CATransaction.setDisableActions(true)
        pointerLayer.contents = shape.image
        pointerLayer.frame = CGRect(x: local.x - shape.hotSpot.x * factor, y: local.y - shape.hotSpot.y * factor,
                                    width: pixels.width * factor, height: pixels.height * factor)
        pointerLayer.isHidden = false
        CATransaction.commit()
    }

    /// Flèche de secours, tant que l'ordinateur n'a pas envoyé la forme du sien.
    private static let arrow: (image: CGImage, hotSpot: CGPoint) = {
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
        return (image.cgImage!, CGPoint(x: 1, y: 1))
    }()
}
