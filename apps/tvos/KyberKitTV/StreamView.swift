// Vue de session de l'Apple TV : l'image (Metal), le pointeur de
// l'ordinateur quand une souris sert (télécommande, souris Bluetooth), et le
// branchement des entrées de GameController sur la chaîne de la session.
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

    /// Modes de souris de l'app Mac (réglages partagés) ; sur l'Apple TV, la
    /// souris se déplace toujours en relatif.
    public enum MouseMode { case auto, desktop, game }

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

    private let keyboardMouse = KeyboardMouse()
    /// Une souris Bluetooth a bougé pendant cette session.
    private var mouseSeen = false
    private var input: InputPipeline?
    private var layout: OpaquePointer?
    private var hostSize = CGSize(width: 1920, height: 1080)
    private var remoteHidden = false
    private var remoteShape: (image: CGImage, hotSpot: CGPoint)?
    private var remotePosition: CGPoint?

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

    func attach(input: InputPipeline, hostSize: CGSize) {
        self.input = input
        self.hostSize = hostSize
        rebuildLayout()
        GamepadBridge.shared.attach(input)
        keyboardMouse.attach(input)
        updateRelative()
    }

    public func detach() {
        GamepadBridge.shared.detach()
        keyboardMouse.detach()
        input?.flushKeys()
        input = nil
        mouseSeen = false
        pointerLayer.isHidden = true
    }

    /// Texte tapé au clavier de l'écran ou de l'iPhone, envoyé tel quel.
    public func type(_ text: String) {
        input?.type(text)
    }

    /// Sur l'Apple TV, toute souris (télécommande comprise) se déplace en relatif.
    private func updateRelative() {
        input?.setRelative(true)
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
        if let image = remote.image?.cgImage {
            remoteShape = (image, remote.hotSpot)
        }
        updatePointerLayer()
    }

    /// Dessiné seulement quand une souris sert : à la manette, le jeu a son propre curseur.
    private func updatePointerLayer() {
        guard remoteMouse || padMouse || mouseSeen, !remoteHidden, let shape = remoteShape, let host = remotePosition, let layout else {
            pointerLayer.isHidden = true
            return
        }
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
}
