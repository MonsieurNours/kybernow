// Affichage de l'image décodée avec Metal.
//
// Latence : on affiche toujours l'image la plus récente. Chaque image part
// à l'écran dès qu'elle est décodée ; celles qui arrivent pendant qu'un
// rendu attend son tampon remplacent la précédente au lieu de faire la
// queue (coalescence `scheduled`), et Core Animation montre la plus récente
// au balayage, en écartant les autres (`presentedTime == 0`). L'ancienne
// garde « une image en vol » (N+1 attend que l'écran ait montré N, soit un
// rafraîchissement complet) reste disponible avec KN_INFLIGHT_GUARD=1. Sur
// un écran à fréquence variable (ProMotion, Adaptive-Sync), l'écran se cale
// sur l'arrivée des images. Sans synchro, chaque image part tout de suite
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
    /// Images avec une latence mesurée sur les images montrées (0 à 1) :
    /// la médiane n'est fiable que si ce rapport est proche de 1.
    public var measuredRatio: Double = 0
    /// Images montrées sans mesure du serveur (arrivée trop tard, jamais) ou valeur absurde.
    public var rejectedSamples = 0

    public init() {}
}

/// Mesures de présentation d'une seconde (`KN_PRESENT_STATS=1` : une ligne
/// par seconde dans le journal `video`).
public struct PresentStats: Sendable {
    /// Images rendues (envoyées à l'écran) dans la seconde.
    public var rendered = 0
    /// Images que l'écran confirme avoir montrées.
    public var presented = 0
    /// Images écartées par Core Animation au profit d'une plus récente (`presentedTime == 0`).
    public var discarded = 0
    /// Images dont l'instant de présentation est estimé (l'écran ne l'a pas
    /// donné : simulateur, ou Apple TV sans rappel de présentation).
    public var estimated = 0
    /// `presentedTime − instant du commit` : médiane et 95e centile, en millisecondes.
    public var p50Ms: Double = 0
    public var p95Ms: Double = 0
    public init() {}
}

/// Un instant de présentation ne sert qu'une fois : le premier des deux
/// rappels (présentation, ou fin du GPU plus un délai) le consomme.
private final class PresentOnce: @unchecked Sendable {
    private var done = false
    private let lock = NSLock()

    func take() -> Bool {
        lock.lock()
        defer { lock.unlock() }
        if done { return false }
        done = true
        return true
    }
}

final class VideoRenderer {
    /// Couche dessinée : celle du rendu, ou celle de la surface de session
    /// (`attach(to:)`, couche de la vue elle-même sur Mac : présentation
    /// directe possible en plein écran).
    private(set) var layer: CAMetalLayer
    /// Format que le rendu ne sait pas lire : le décodeur doit en produire un autre.
    var onUnsupportedFormat: ((OSType) -> Void)?
    var onStats: ((VideoStats) -> Void)?
    /// Latence de bout en bout (mesures de Kyber), si elles sont actives.
    var latency: (any LatencyReporting)?

    /// Synchro verticale (macOS). Sans elle, l'image part dès qu'elle est prête.
    var vsync = true {
        didSet {
            #if os(macOS)
            renderQueue.async { [self] in layer.displaySyncEnabled = vsync }
            #endif
        }
    }

    /// Garde « une image en vol » : l'image N+1 attend que l'écran ait montré N.
    /// `KN_INFLIGHT_GUARD=1` la remet (comparaison) ; sans elle, chaque image
    /// part dès qu'elle est décodée et Core Animation montre la plus récente
    /// au balayage.
    var inFlightGuard = ProcessInfo.processInfo.environment["KN_INFLIGHT_GUARD"] == "1"
    /// `KN_PRESENT_STATS=1` : mesures de présentation dans le journal chaque seconde.
    var logPresentStats = ProcessInfo.processInfo.environment["KN_PRESENT_STATS"] == "1"
    /// Mesures de présentation, une fois par seconde (tests).
    var onPresentStats: ((PresentStats) -> Void)?

    /// Vue (fenêtre occultée ou réduite) : rien n'est rendu, les images sont
    /// comptées comme remplacées ; le décodeur continue (références intactes).
    var paused = false

    private let device: MTLDevice
    private let queue: MTLCommandQueue
    private let pipeline: MTLRenderPipelineState
    private var cache: CVMetalTextureCache?
    /// Matrice de couleur du format en cours, relue au plus une fois par seconde
    /// (fil de rendu) : `CVBufferCopyAttachment` à chaque image coûtait pour rien.
    private var colorCache: (format: OSType, width: Int, height: Int, read: CFTimeInterval, value: ColorConversion)?
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
    /// Présentation (fil de rendu) : `presentedTime − commit` des images montrées, rendues, écartées, estimées.
    private var present = (lags: [Double](), rendered: 0, discarded: 0, estimated: 0)

    /// GPU, file de commandes et pipeline, compilés une fois par processus
    /// (le shader était compilé depuis la source à chaque flux, 30 à 150 ms
    /// sur le fil principal).
    private struct Core {
        let device: MTLDevice
        let queue: MTLCommandQueue
        let pipeline: MTLRenderPipelineState
    }

    private static let core: Core? = {
        guard let device = MTLCreateSystemDefaultDevice(), let queue = device.makeCommandQueue() else { return nil }
        do {
            let library = try device.makeLibrary(source: shader, options: nil)
            let desc = MTLRenderPipelineDescriptor()
            desc.vertexFunction = library.makeFunction(name: "kn_vertex")
            desc.fragmentFunction = library.makeFunction(name: "kn_fragment")
            desc.colorAttachments[0].pixelFormat = .bgra8Unorm
            return Core(device: device, queue: queue, pipeline: try device.makeRenderPipelineState(descriptor: desc))
        } catch {
            KyberLog.video.error("rendu Metal : \(error.localizedDescription, privacy: .public)")
            return nil
        }
    }()

    /// Compile le pipeline (au lancement, hors du fil principal).
    static func warmUp() {
        _ = core
    }

    init?() {
        guard let core = Self.core else { return nil }
        device = core.device
        queue = core.queue
        pipeline = core.pipeline
        CVMetalTextureCacheCreate(kCFAllocatorDefault, nil, device, nil, &cache)
        layer = CAMetalLayer()
        configure(layer)
    }

    /// Dessine désormais dans `layer` (nil : dans une couche à soi, hors écran).
    func attach(to layer: CAMetalLayer?) {
        renderQueue.sync {
            self.layer = layer ?? CAMetalLayer()
            configure(self.layer)
        }
    }

    private func configure(_ layer: CAMetalLayer) {
        layer.device = device
        layer.pixelFormat = .bgra8Unorm
        layer.framebufferOnly = true
        layer.isOpaque = true
        layer.colorspace = CGColorSpace(name: CGColorSpace.sRGB)
        layer.backgroundColor = CGColor(gray: 0, alpha: 1)
        layer.allowsNextDrawableTimeout = true
        #if os(macOS)
        layer.displaySyncEnabled = vsync
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
        if paused {
            lock.lock()
            skipped += 1
            lock.unlock()
            latency?.skipped(pts: frame.pts)
            return
        }
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
    /// `presentedTime` à 0 : écartée par l'écran. `estimated` : l'instant
    /// n'a pas été donné par l'écran, c'est une estimation.
    private func presented(at presentedTime: CFTimeInterval, committed: CFTimeInterval, received: UInt64, decoded: UInt64, pts: UInt64, estimated: Bool = false) {
        if presentedTime > 0, let latency {
            // presentedTime et les instants mach sont sur l'horloge de
            // CACurrentMediaTime ; Kyber compte en CLOCK_MONOTONIC.
            let now = CACurrentMediaTime(), micros = monotonicMicros()
            func monotonic(_ t: Double) -> Int64 { micros - Int64((now - t) * 1_000_000) }
            latency.displayed(pts: pts, received: monotonic(Self.seconds(received)), decoded: monotonic(Self.seconds(decoded)),
                              at: monotonic(presentedTime))
        }
        lock.lock()
        if presentedTime == 0 {
            // Écartée par l'écran (une plus récente l'a devancée) : remplacée, pas perdue.
            skipped += 1
            latency?.skipped(pts: pts)
        }
        inFlight = false
        let go = pending != nil && !scheduled
        if go { scheduled = true }
        lock.unlock()
        renderQueue.async { [weak self] in
            guard let self else { return }
            if presentedTime > 0 {
                self.window.display += (presentedTime - Self.seconds(received)) * 1000
                self.window.displayed += 1
                self.present.lags.append((presentedTime - committed) * 1000)
                if estimated { self.present.estimated += 1 }
            } else {
                self.present.discarded += 1
            }
        }
        if go { renderQueue.async { [weak self] in self?.render() } }
    }

    // MARK: - Rendu (fil de rendu)

    private func render() {
        // Le tampon d'abord (il peut faire attendre un balayage quand tous
        // sont pris) : l'image prise ensuite est la plus récente arrivée.
        let drawable = layer.nextDrawable()
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
        guard let drawable, let commands = queue.makeCommandBuffer(),
              draw(buffer, layout: layout, into: drawable.texture, commands: commands, color: conversion(buffer, layout: layout))
        else {
            latency?.skipped(pts: frame.pts)
            return
        }

        let received = frame.received, decoded = frame.decoded, pts = frame.pts
        let committed = CACurrentMediaTime()
        commands.addCompletedHandler { [weak self] cb in
            guard let self, let error = cb.error, self.gpuErrors < 3 else { return }
            self.gpuErrors += 1
            KyberLog.video.error("commande Metal en erreur : \(error.localizedDescription, privacy: .public)")
        }
        #if targetEnvironment(simulator)
        // Le simulateur ne dit pas quand l'image est montrée : elle l'est au
        // rafraîchissement qui suit la fin du rendu.
        commands.addCompletedHandler { [weak self] _ in
            self?.presented(at: CACurrentMediaTime() + 1.0 / 120, committed: committed, received: received, decoded: decoded, pts: pts, estimated: true)
        }
        #elseif os(tvOS)
        // Apple TV : l'instant de présentation quand l'écran le donne (c'est
        // lui qui mesure vraiment l'attente du balayage à 60 Hz). Sans
        // rappel 100 ms après la fin du GPU, ou avec un instant nul, on
        // estime comme avant : une image n'est jamais tenue pour écartée
        // ici, un 0 sur tvOS peut vouloir dire « inconnu », et la couche
        // n'est pas refaite pour cela.
        let once = PresentOnce()
        drawable.addPresentedHandler { [weak self] d in
            guard once.take() else { return }
            let t = d.presentedTime
            self?.presented(at: t > 0 ? t : CACurrentMediaTime(), committed: committed, received: received, decoded: decoded, pts: pts, estimated: t <= 0)
        }
        commands.addCompletedHandler { [weak self] _ in
            let finished = CACurrentMediaTime()
            self?.renderQueue.asyncAfter(deadline: .now() + 0.1) { [weak self] in
                guard once.take() else { return }
                self?.presented(at: finished + 1.0 / 120, committed: committed, received: received, decoded: decoded, pts: pts, estimated: true)
            }
        }
        #else
        drawable.addPresentedHandler { [weak self] d in
            self?.presented(at: d.presentedTime, committed: committed, received: received, decoded: decoded, pts: pts)
        }
        #endif
        lock.lock()
        inFlight = vsync && inFlightGuard
        inFlightSince = committed
        lock.unlock()
        commands.present(drawable)
        commands.commit()
        present.rendered += 1
        lock.lock()
        shown = (buffer, layout)
        lock.unlock()

        account(frame, width: CVPixelBufferGetWidth(buffer), height: CVPixelBufferGetHeight(buffer), layout: layout)
    }

    /// Matrice de couleur de l'image, lue à chaque changement de format ou de
    /// taille et au plus une fois par seconde (fil de rendu).
    private func conversion(_ buffer: CVPixelBuffer, layout: Layout) -> ColorConversion {
        let format = CVPixelBufferGetPixelFormatType(buffer)
        let width = CVPixelBufferGetWidth(buffer), height = CVPixelBufferGetHeight(buffer)
        let now = CACurrentMediaTime()
        if let c = colorCache, c.format == format, c.width == width, c.height == height, now - c.read < 1 { return c.value }
        if colorCache?.format != format || colorCache?.width != width || colorCache?.height != height {
            KyberLog.video.notice("images \(Self.fourcc(format), privacy: .public) \(width)×\(height), \(layout.label, privacy: .public)\(CVPixelBufferGetIOSurface(buffer) == nil ? ", sans IOSurface" : "", privacy: .public)")
        }
        let value = Self.color(buffer, fullRange: layout.full, deep: layout.deep)
        colorCache = (format, width, height, now, value)
        return value
    }

    /// Dessine l'image dans `target`, centrée et entière. Faux si les plans
    /// ne se lisent pas en texture. `color` : conversion déjà connue (sinon lue sur l'image).
    func draw(_ buffer: CVPixelBuffer, layout: Layout, into target: MTLTexture, commands: MTLCommandBuffer, color known: ColorConversion? = nil) -> Bool {
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
        var color = known ?? Self.color(buffer, fullRange: layout.full, deep: layout.deep)
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
        #if os(macOS)
        desc.storageMode = .managed
        #else
        desc.storageMode = .shared
        #endif
        guard let target = device.makeTexture(descriptor: desc), let commands = queue.makeCommandBuffer(),
              draw(last.buffer, layout: last.layout, into: target, commands: commands)
        else { return nil }
        #if os(macOS)
        guard let blit = commands.makeBlitCommandEncoder() else { return nil }
        blit.synchronize(resource: target)
        blit.endEncoding()
        #endif
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
        // Textures des images déjà rendues : le cache les garde sinon jusqu'à saturation.
        if let cache { CVMetalTextureCacheFlush(cache, 0) }
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
        let second = latency?.drain() ?? LatencySecond()
        current.latencyMs = second.latencyMs
        current.chain = second.chain
        current.lostFrames = second.lostFrames
        current.holes = second.holes
        current.keyframeRequests = second.keyframeRequests
        current.lostPackets = second.lostPackets
        current.rttMs = second.rttMs
        current.mbps = Double(second.bytes) * 8 / elapsed / 1_000_000
        current.measuredRatio = second.displayed > 0 ? Double(second.measured) / Double(second.displayed) : 0
        current.rejectedSamples = second.rejected
        onStats?(current)
        window = (now, 0, 0, 0, 0)
        reportPresentation()
    }

    /// Mesures de présentation de la seconde : journal (`KN_PRESENT_STATS=1`) et rappel.
    private func reportPresentation() {
        var stats = PresentStats()
        stats.rendered = present.rendered
        stats.presented = present.lags.count
        stats.discarded = present.discarded
        stats.estimated = present.estimated
        if !present.lags.isEmpty {
            let sorted = present.lags.sorted()
            stats.p50Ms = sorted[sorted.count / 2]
            stats.p95Ms = sorted[min(sorted.count - 1, Int(Double(sorted.count) * 0.95))]
        }
        present = ([], 0, 0, 0)
        if logPresentStats {
            // Metal ne dit pas publiquement si la présentation est directe ou composée : voir MTL_HUD_ENABLED=1.
            KyberLog.video.notice("présentation : \(stats.rendered) rendues, \(stats.presented) montrées (\(stats.estimated) estimées), \(stats.discarded) écartées, commit → écran p50 \(String(format: "%.1f", stats.p50Ms), privacy: .public) ms, p95 \(String(format: "%.1f", stats.p95Ms), privacy: .public) ms, synchro verticale \(self.vsync ? "oui" : "non", privacy: .public), garde une image en vol \(self.inFlightGuard ? "oui" : "non", privacy: .public), \(Int(self.layer.drawableSize.width))×\(Int(self.layer.drawableSize.height))")
        }
        onPresentStats?(stats)
        guard stats.rendered > 0 else { return }
        if stats.presented > 0 {
            stuckSeconds = 0
            return
        }
        if logPresentStats, unseenReports < 3 {
            unseenReports += 1
            describeUnseenLayer()
        }
        stuckSeconds += 1
        if stuckSeconds >= 2, heals < 3 {
            stuckSeconds = 0
            heals += 1
            KyberLog.video.error("rien de montré en 2 s : \(stats.rendered) images rendues, toutes écartées par l'écran")
            onStuck?()
        }
    }

    /// Erreurs GPU déjà écrites au journal (trois au plus).
    private var gpuErrors = 0

    /// L'écran écarte toutes les images rendues depuis deux secondes : la
    /// couche Metal est à refaire (fil de rendu). Voir MetalSurface (UIKit).
    var onStuck: (() -> Void)?
    private var stuckSeconds = 0
    private var heals = 0

    /// Rapports « rien de montré » déjà écrits (trois au plus par rendu).
    private var unseenReports = 0

    /// Rien de montré dans la seconde alors que des images ont été rendues :
    /// où est la couche Metal (cadre, chaîne de couches jusqu'à la racine) ?
    private func describeUnseenLayer() {
        DispatchQueue.main.async { [layer] in
            var chain: [String] = []
            var hidden = false
            var root: CALayer = layer
            var l: CALayer? = layer
            while let c = l {
                if c.isHidden || c.opacity == 0 { hidden = true }
                chain.append("\(type(of: c.delegate.map { $0 as AnyObject } ?? c))\(c.isHidden ? " cachée" : "")\(c.opacity < 1 ? " opacité \(c.opacity)" : "") \(Int(c.bounds.width))×\(Int(c.bounds.height))")
                root = c
                l = c.superlayer
            }
            let onRoot = layer.convert(layer.bounds, to: root)
            KyberLog.video.notice("rien de montré : couche \(Int(onRoot.minX)),\(Int(onRoot.minY)) \(Int(onRoot.width))×\(Int(onRoot.height)) dans la racine \(Int(root.bounds.width))×\(Int(root.bounds.height)), tampon \(Int(layer.drawableSize.width))×\(Int(layer.drawableSize.height)), \(hidden ? "cachée" : "visible", privacy: .public) ; \(chain.joined(separator: " ← "), privacy: .public)")
        }
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
