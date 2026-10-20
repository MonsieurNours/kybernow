// Affichage de l'image décodée avec Metal.
//
// Latence : on affiche toujours l'image la plus récente. Avec la synchro
// verticale, une seule image attend l'écran à la fois ; celles qui arrivent
// entre-temps remplacent la suivante au lieu de faire la queue. Sur un écran
// à fréquence variable (ProMotion, Adaptive-Sync), l'écran se cale sur
// l'arrivée des images. Sans synchro, chaque image part tout de suite
// (déchirure possible).
//
// Les plans Y et CbCr sortent de VideoToolbox en IOSurface : Metal les lit
// sans copie (CVMetalTextureCache) et convertit en RGB dans le shader.
// Indépendant d'AppKit : sert tel quel sur iOS et tvOS.

import CoreGraphics
import CoreVideo
import Metal
import QuartzCore
import simd

/// Mesures de l'image, une fois par seconde.
public struct VideoStats: Sendable {
    /// Images affichées par seconde.
    public var fps: Double = 0
    /// Décodage moyen, en millisecondes.
    public var decodeMs: Double = 0
    /// De la réception du paquet à l'affichage, en millisecondes (moyenne).
    public var displayMs: Double = 0
    /// Images remplacées par une plus récente avant d'être affichées.
    public var skipped = 0
    /// De la capture sur l'ordinateur à l'affichage sur le Mac (médiane de la seconde), en millisecondes.
    public var latencyMs: Double?
    public var width = 0
    public var height = 0
    /// « 4:2:0 8 bits », « 4:4:4 10 bits »…
    public var format = ""
    /// Temps de chaque étape, de la capture à l'affichage (si l'ordinateur les envoie).
    public var chain: ChainTimes?
    /// Images envoyées par l'ordinateur et jamais montrées, dans la seconde.
    public var lostFrames = 0
    /// Trous dans le flux (paquets perdus au-delà de la correction d'erreurs).
    public var holes = 0
    /// Images clés demandées après une perte.
    public var keyframeRequests = 0
    /// Paquets perdus d'après Kyber (mesure toutes les quelques secondes).
    public var lostPackets = 0
    /// Aller-retour réseau, en millisecondes.
    public var rttMs: Double?
    /// Débit reçu pour l'image, en mégabits par seconde.
    public var mbps: Double = 0

    public init() {}
}

final class VideoRenderer {
    let layer = CAMetalLayer()
    /// Format que le rendu ne sait pas lire : le décodeur doit en produire un autre.
    var onUnsupportedFormat: ((OSType) -> Void)?
    var onStats: ((VideoStats) -> Void)?
    /// Latence de bout en bout (mesures de Kyber), si elles sont actives.
    var latency: LatencyTracker?

    /// Synchro verticale (macOS). Sans elle, l'image part dès qu'elle est prête.
    var vsync = true {
        didSet {
            #if os(macOS)
            layer.displaySyncEnabled = vsync
            #endif
        }
    }

    private let device: MTLDevice
    private let queue: MTLCommandQueue
    private let pipeline: MTLRenderPipelineState
    private var cache: CVMetalTextureCache?
    private let renderQueue = DispatchQueue(label: "fr.leslignesclaires.kybernow.render", qos: .userInteractive)

    private let lock = NSLock()
    /// Dernière image affichée, pour la photographier (fin de session).
    private var shown: (buffer: CVPixelBuffer, layout: Layout)?
    private var pending: DecodedFrame?
    private var scheduled = false
    private var inFlight = false
    private var inFlightSince = 0.0
    private var skipped = 0

    // Mesures (fil de rendu).
    private var window = (start: CACurrentMediaTime(), frames: 0, decode: 0.0, display: 0.0, displayed: 0)
    private var current = VideoStats()

    init?() {
        guard let device = MTLCreateSystemDefaultDevice(), let queue = device.makeCommandQueue() else { return nil }
        self.device = device
        self.queue = queue
        do {
            let library = try device.makeLibrary(source: Self.shader, options: nil)
            let desc = MTLRenderPipelineDescriptor()
            desc.vertexFunction = library.makeFunction(name: "kn_vertex")
            desc.fragmentFunction = library.makeFunction(name: "kn_fragment")
            desc.colorAttachments[0].pixelFormat = .bgra8Unorm
            pipeline = try device.makeRenderPipelineState(descriptor: desc)
        } catch {
            KyberLog.video.error("rendu Metal : \(error.localizedDescription, privacy: .public)")
            return nil
        }
        CVMetalTextureCacheCreate(kCFAllocatorDefault, nil, device, nil, &cache)
        layer.device = device
        layer.pixelFormat = .bgra8Unorm
        layer.framebufferOnly = true
        layer.isOpaque = true
        layer.colorspace = CGColorSpace(name: CGColorSpace.sRGB)
        layer.backgroundColor = CGColor(gray: 0, alpha: 1)
        layer.allowsNextDrawableTimeout = true
        #if os(macOS)
        layer.displaySyncEnabled = true
        #endif
    }

    /// Taille de l'image à l'écran, en pixels réels.
    func setDrawableSize(_ size: CGSize) {
        guard size.width >= 1, size.height >= 1 else { return }
        renderQueue.async { [layer] in
            if layer.drawableSize != size { layer.drawableSize = size }
        }
    }

    /// Nouvelle image (fil du décodeur) : elle remplace celle qui attendait.
    func submit(_ frame: DecodedFrame) {
        lock.lock()
        if let replaced = pending {
            skipped += 1
            latency?.skipped(pts: replaced.pts)
        }
        pending = frame
        // Garde-fou : un affichage jamais confirmé ne bloque pas l'image.
        if inFlight, CACurrentMediaTime() - inFlightSince > 0.1 { inFlight = false }
        let go = !scheduled && (!inFlight || !vsync)
        if go { scheduled = true }
        lock.unlock()
        if go { renderQueue.async { [weak self] in self?.render() } }
    }

    /// L'image affichée a quitté l'écran : la suivante peut partir.
    private func presented(_ drawable: MTLDrawable, received: UInt64, decoded: UInt64, pts: UInt64) {
        if drawable.presentedTime > 0, let latency {
            // presentedTime et les instants mach sont sur l'horloge de
            // CACurrentMediaTime ; Kyber compte en CLOCK_MONOTONIC.
            let now = CACurrentMediaTime(), micros = LatencyTracker.nowMicros()
            func monotonic(_ t: Double) -> Int64 { micros - Int64((now - t) * 1_000_000) }
            latency.displayed(pts: pts, received: monotonic(Self.seconds(received)), decoded: monotonic(Self.seconds(decoded)),
                              at: monotonic(drawable.presentedTime))
        }
        lock.lock()
        if drawable.presentedTime == 0 {
            // Écartée par l'écran (une plus récente l'a devancée) : remplacée, pas perdue.
            skipped += 1
            latency?.skipped(pts: pts)
        }
        inFlight = false
        let go = pending != nil && !scheduled
        if go { scheduled = true }
        lock.unlock()
        if drawable.presentedTime > 0 {
            renderQueue.async { [weak self] in
                guard let self else { return }
                self.window.display += (drawable.presentedTime - Self.seconds(received)) * 1000
                self.window.displayed += 1
            }
        }
        if go { renderQueue.async { [weak self] in self?.render() } }
    }

    // MARK: - Rendu (fil de rendu)

    private func render() {
        lock.lock()
        let frame = pending
        pending = nil
        scheduled = false
        lock.unlock()
        guard let frame else { return }

        let buffer = frame.buffer
        let pixelFormat = CVPixelBufferGetPixelFormatType(buffer)
        guard let layout = Self.layout(pixelFormat), CVPixelBufferGetPlaneCount(buffer) == 2 else {
            KyberLog.video.error("format d'image non géré : \(Self.fourcc(pixelFormat), privacy: .public)")
            onUnsupportedFormat?(pixelFormat)
            return
        }
        guard let drawable = layer.nextDrawable(), let commands = queue.makeCommandBuffer(),
              draw(buffer, layout: layout, into: drawable.texture, commands: commands)
        else {
            latency?.skipped(pts: frame.pts)
            return
        }

        let received = frame.received, decoded = frame.decoded, pts = frame.pts
        drawable.addPresentedHandler { [weak self] d in self?.presented(d, received: received, decoded: decoded, pts: pts) }
        lock.lock()
        inFlight = vsync
        inFlightSince = CACurrentMediaTime()
        lock.unlock()
        commands.present(drawable)
        commands.commit()
        lock.lock()
        shown = (buffer, layout)
        lock.unlock()

        account(frame, width: CVPixelBufferGetWidth(buffer), height: CVPixelBufferGetHeight(buffer), layout: layout)
    }

    /// Dessine l'image dans `target`, centrée et entière. Faux si les plans
    /// ne se lisent pas en texture.
    func draw(_ buffer: CVPixelBuffer, layout: Layout, into target: MTLTexture, commands: MTLCommandBuffer) -> Bool {
        guard let cache,
              let luma = texture(buffer, plane: 0, format: layout.deep ? .r16Unorm : .r8Unorm, cache: cache),
              let chroma = texture(buffer, plane: 1, format: layout.deep ? .rg16Unorm : .rg8Unorm, cache: cache)
        else { return false }

        let pass = MTLRenderPassDescriptor()
        pass.colorAttachments[0].texture = target
        pass.colorAttachments[0].loadAction = .clear
        pass.colorAttachments[0].clearColor = MTLClearColor(red: 0, green: 0, blue: 0, alpha: 1)
        pass.colorAttachments[0].storeAction = .store
        guard let encoder = commands.makeRenderCommandEncoder(descriptor: pass) else { return false }

        // Image entière, centrée, sans déformation (comme le calcul des entrées kynput).
        let width = Double(CVPixelBufferGetWidth(buffer)), height = Double(CVPixelBufferGetHeight(buffer))
        let targetW = Double(target.width), targetH = Double(target.height)
        let scale = min(targetW / width, targetH / height)
        let w = (width * scale).rounded(), h = (height * scale).rounded()
        encoder.setViewport(MTLViewport(originX: ((targetW - w) / 2).rounded(), originY: ((targetH - h) / 2).rounded(), width: w, height: h, znear: 0, zfar: 1))
        encoder.setRenderPipelineState(pipeline)
        encoder.setFragmentTexture(CVMetalTextureGetTexture(luma), index: 0)
        encoder.setFragmentTexture(CVMetalTextureGetTexture(chroma), index: 1)
        var color = Self.color(buffer, fullRange: layout.full, deep: layout.deep)
        encoder.setFragmentBytes(&color, length: MemoryLayout<ColorConversion>.stride, index: 0)
        encoder.drawPrimitives(type: .triangle, vertexStart: 0, vertexCount: 3)
        encoder.endEncoding()
        commands.addCompletedHandler { _ in _ = (luma, chroma) } // garde les textures jusqu'à la fin du GPU
        return true
    }

    /// Photographie de la dernière image affichée, réduite à `maxWidth` pixels
    /// de large (la dernière image d'un ordinateur, sur l'accueil).
    func snapshot(maxWidth: Int) -> CGImage? {
        lock.lock()
        let last = shown
        lock.unlock()
        guard let last else { return nil }
        let w = CVPixelBufferGetWidth(last.buffer), h = CVPixelBufferGetHeight(last.buffer)
        guard w > 0, h > 0 else { return nil }
        let scale = min(1, Double(maxWidth) / Double(w))
        let tw = max(2, Int(Double(w) * scale)), th = max(2, Int(Double(h) * scale))
        let desc = MTLTextureDescriptor.texture2DDescriptor(pixelFormat: .bgra8Unorm, width: tw, height: th, mipmapped: false)
        desc.usage = [.renderTarget, .shaderRead]
        desc.storageMode = .managed
        guard let target = device.makeTexture(descriptor: desc), let commands = queue.makeCommandBuffer(),
              draw(last.buffer, layout: last.layout, into: target, commands: commands),
              let blit = commands.makeBlitCommandEncoder()
        else { return nil }
        blit.synchronize(resource: target)
        blit.endEncoding()
        commands.commit()
        commands.waitUntilCompleted()
        let rowBytes = tw * 4
        var bytes = [UInt8](repeating: 0, count: rowBytes * th)
        target.getBytes(&bytes, bytesPerRow: rowBytes, from: MTLRegionMake2D(0, 0, tw, th), mipmapLevel: 0)
        guard let provider = CGDataProvider(data: Data(bytes) as CFData) else { return nil }
        return CGImage(width: tw, height: th, bitsPerComponent: 8, bitsPerPixel: 32, bytesPerRow: rowBytes,
                       space: CGColorSpace(name: CGColorSpace.sRGB)!, bitmapInfo: CGBitmapInfo(rawValue: CGBitmapInfo.byteOrder32Little.rawValue | CGImageAlphaInfo.noneSkipFirst.rawValue),
                       provider: provider, decode: nil, shouldInterpolate: true, intent: .defaultIntent)
    }

    /// File de commandes du rendu (tests hors écran).
    var commandQueue: MTLCommandQueue { queue }

    private func texture(_ buffer: CVPixelBuffer, plane: Int, format: MTLPixelFormat, cache: CVMetalTextureCache) -> CVMetalTexture? {
        var texture: CVMetalTexture?
        CVMetalTextureCacheCreateTextureFromImage(
            kCFAllocatorDefault, cache, buffer, nil, format,
            CVPixelBufferGetWidthOfPlane(buffer, plane), CVPixelBufferGetHeightOfPlane(buffer, plane), plane, &texture
        )
        return texture
    }

    private func account(_ frame: DecodedFrame, width: Int, height: Int, layout: Layout) {
        window.frames += 1
        window.decode += Self.seconds(frame.decoded &- frame.received) * 1000
        let now = CACurrentMediaTime()
        let elapsed = now - window.start
        guard elapsed >= 1 else { return }
        lock.lock()
        let skipped = self.skipped
        self.skipped = 0
        lock.unlock()
        current.fps = Double(window.frames) / elapsed
        current.decodeMs = window.decode / Double(max(window.frames, 1))
        current.displayMs = window.displayed > 0 ? window.display / Double(window.displayed) : 0
        current.skipped = skipped
        current.width = width
        current.height = height
        current.format = layout.label
        let second = latency?.drain() ?? LatencyTracker.Second()
        current.latencyMs = second.latencyMs
        current.chain = second.chain
        current.lostFrames = second.lostFrames
        current.holes = second.holes
        current.keyframeRequests = second.keyframeRequests
        current.lostPackets = second.lostPackets
        current.rttMs = second.rttMs
        current.mbps = Double(second.bytes) * 8 / elapsed / 1_000_000
        onStats?(current)
        window = (now, 0, 0, 0, 0)
    }

    // MARK: - Formats et couleurs

    struct Layout {
        let deep: Bool
        let full: Bool
        let label: String
    }

    static func layout(_ format: OSType) -> Layout? {
        switch format {
        // Formats compressés sans perte, natifs du décodeur des puces Apple :
        // le GPU les lit tels quels (moins de bande passante mémoire).
        case kCVPixelFormatType_Lossless_420YpCbCr8BiPlanarVideoRange: return Layout(deep: false, full: false, label: "4:2:0 8 bits")
        case kCVPixelFormatType_Lossless_420YpCbCr8BiPlanarFullRange: return Layout(deep: false, full: true, label: "4:2:0 8 bits")
        case Self.lossless444Video: return Layout(deep: false, full: false, label: "4:4:4 8 bits")
        case Self.lossless444Full: return Layout(deep: false, full: true, label: "4:4:4 8 bits")
        case kCVPixelFormatType_420YpCbCr8BiPlanarVideoRange: return Layout(deep: false, full: false, label: "4:2:0 8 bits")
        case kCVPixelFormatType_420YpCbCr8BiPlanarFullRange: return Layout(deep: false, full: true, label: "4:2:0 8 bits")
        case kCVPixelFormatType_422YpCbCr8BiPlanarVideoRange: return Layout(deep: false, full: false, label: "4:2:2 8 bits")
        case kCVPixelFormatType_422YpCbCr8BiPlanarFullRange: return Layout(deep: false, full: true, label: "4:2:2 8 bits")
        case kCVPixelFormatType_444YpCbCr8BiPlanarVideoRange: return Layout(deep: false, full: false, label: "4:4:4 8 bits")
        case kCVPixelFormatType_444YpCbCr8BiPlanarFullRange: return Layout(deep: false, full: true, label: "4:4:4 8 bits")
        case kCVPixelFormatType_420YpCbCr10BiPlanarVideoRange: return Layout(deep: true, full: false, label: "4:2:0 10 bits")
        case kCVPixelFormatType_420YpCbCr10BiPlanarFullRange: return Layout(deep: true, full: true, label: "4:2:0 10 bits")
        case kCVPixelFormatType_422YpCbCr10BiPlanarVideoRange: return Layout(deep: true, full: false, label: "4:2:2 10 bits")
        case kCVPixelFormatType_422YpCbCr10BiPlanarFullRange: return Layout(deep: true, full: true, label: "4:2:2 10 bits")
        case kCVPixelFormatType_444YpCbCr10BiPlanarVideoRange: return Layout(deep: true, full: false, label: "4:4:4 10 bits")
        case kCVPixelFormatType_444YpCbCr10BiPlanarFullRange: return Layout(deep: true, full: true, label: "4:4:4 10 bits")
        default: return nil
        }
    }

    /// Pendants 4:4:4 de '&8v0' / '&8f0', produits par VideoToolbox mais absents des en-têtes.
    static let lossless444Video: OSType = 0x2638_7634 // '&8v4'
    static let lossless444Full: OSType = 0x2638_6634 // '&8f4'

    struct ColorConversion {
        var matrix: simd_float3x3
        var offset: simd_float3
    }

    /// YCbCr → RGB selon la matrice annoncée par le flux (709 par défaut) et la plage.
    static func color(_ buffer: CVPixelBuffer, fullRange: Bool, deep: Bool) -> ColorConversion {
        let matrixName = CVBufferCopyAttachment(buffer, kCVImageBufferYCbCrMatrixKey, nil) as? String
        let (kr, kb): (Float, Float)
        switch matrixName {
        case String(kCVImageBufferYCbCrMatrix_ITU_R_601_4): (kr, kb) = (0.299, 0.114)
        case String(kCVImageBufferYCbCrMatrix_ITU_R_2020): (kr, kb) = (0.2627, 0.0593)
        default: (kr, kb) = (0.2126, 0.0722)
        }
        let kg = 1 - kr - kb
        let rgb = simd_float3x3(columns: (
            simd_float3(1, 1, 1),
            simd_float3(0, -2 * (1 - kb) * kb / kg, 2 * (1 - kb)),
            simd_float3(2 * (1 - kr), -2 * (1 - kr) * kr / kg, 0)
        ))
        // Plage « vidéo » : Y de 16 à 235, CbCr de 16 à 240 (sur 8 bits),
        // valeurs calées en haut des 16 bits pour le 10 bits.
        let max: Float = deep ? 65535 : 255
        let unit: Float = deep ? 64 : 1 // 10 bits calés en haut de 16 : un pas vaut 64
        let bits: Float = deep ? 4 : 1 // échelle 10 bits / 8 bits
        let yOffset: Float = fullRange ? 0 : 16 * bits * unit / max
        let cOffset: Float = (deep ? 512 * unit : 128) / max
        let yScale: Float = fullRange ? 1 : max / (219 * bits * unit)
        let cScale: Float = fullRange ? 1 : max / (224 * bits * unit)
        let range = simd_float3x3(diagonal: simd_float3(yScale, cScale, cScale))
        return ColorConversion(matrix: rgb * range, offset: simd_float3(yOffset, cOffset, cOffset))
    }

    static func seconds(_ machTicks: UInt64) -> Double {
        var info = mach_timebase_info_data_t()
        mach_timebase_info(&info)
        return Double(machTicks) * Double(info.numer) / Double(info.denom) / 1e9
    }

    static func fourcc(_ code: OSType) -> String {
        let bytes = [24, 16, 8, 0].map { UInt8((code >> $0) & 0xFF) }
        return String(bytes: bytes, encoding: .ascii) ?? "\(code)"
    }

    private static let shader = """
    #include <metal_stdlib>
    using namespace metal;

    struct VOut { float4 position [[position]]; float2 uv; };
    struct Conversion { float3x3 matrix; float3 offset; };

    // Un triangle qui couvre la zone d'affichage.
    vertex VOut kn_vertex(uint id [[vertex_id]]) {
        float2 p = float2((id << 1) & 2, id & 2);
        VOut out;
        out.position = float4(p * 2.0 - 1.0, 0.0, 1.0);
        out.uv = float2(p.x, 1.0 - p.y);
        return out;
    }

    fragment float4 kn_fragment(VOut in [[stage_in]],
                                texture2d<float> luma [[texture(0)]],
                                texture2d<float> chroma [[texture(1)]],
                                constant Conversion &c [[buffer(0)]]) {
        constexpr sampler s(filter::linear, address::clamp_to_edge);
        float3 ycbcr = float3(luma.sample(s, in.uv).r, chroma.sample(s, in.uv).rg);
        return float4(saturate(c.matrix * (ycbcr - c.offset)), 1.0);
    }
    """
}
