// Affichage de l'image décodée avec Metal.
//
// Latence : on affiche toujours l'image la plus récente. Chaque image part
// à l'écran dès qu'elle est décodée ; celles qui arrivent pendant qu'un
// rendu attend son tampon remplacent la précédente au lieu de faire la
// queue (coalescence `scheduled`), et Core Animation montre la plus récente
// au balayage, en écartant les autres (`presentedTime == 0`). La garde
// « une image en vol » (N+1 attend que l'écran ait montré N, soit un
// rafraîchissement complet) se remet avec KN_INFLIGHT_GUARD=1, pour comparer. Sur
// un écran à fréquence variable (ProMotion, Adaptive-Sync), l'écran se cale
// sur l'arrivée des images. Sans synchro, chaque image part tout de suite
// (déchirure possible).
//
// Les plans Y et CbCr sortent de VideoToolbox en IOSurface : Metal les lit
// sans copie (CVMetalTextureCache) et convertit en RGB dans le shader.
// Option « trait de repère » (Mac, barre de session) : une capsule blanche
// à 35 %, 220 × 3 points, centrée en haut de la vue, tracée dans la même passe
// que l'image. Elle doit se voir sans composition (présentation directe en
// plein écran) ; allumée ou éteinte, l'image affichée est redessinée en place.
//
// HDR (docs/hdr.md) : une image PQ (BT.2020, SMPTE ST 2084, étiquetée par
// le flux) sur un écran HDR (`hdrDisplay`) passe par la chaîne HDR : couche
// en `rgba16Float`, espace `itur_2100_PQ`, contenu EDR et `edrMetadata`
// HDR10 (Mac, iPhone, iPad). Le shader écrit les valeurs PQ telles
// quelles. L'Apple TV n'a ni contenu EDR ni `edrMetadata` sur une couche
// Metal (des valeurs PQ y sont montrées comme du SDR, délavées) : l'image
// PQ y part telle que décodée dans la couche vidéo du système
// (`HDRVideoLayer`, posée par la surface), et le téléviseur passe en HDR
// par `AVDisplayManager`. Sur un écran SDR (fenêtre glissée sur un autre
// écran, le temps que l'image reparte en SDR), une image PQ est ramenée en
// SDR dans le shader (blanc à 203 nits, genou à 75 %, BT.709) ; la
// photographie de la dernière image aussi. Sinon : la chaîne SDR (BGRA 8 bits, sRGB).
//
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
    /// Images redessinées en place (fenêtre redevenue visible, taille
    /// changée, trait de repère) que l'écran confirme avoir montrées.
    public var redrawn = 0
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

    /// Vue (fenêtre occultée ou réduite) : rien n'est rendu ; le décodeur
    /// continue (références intactes) et la dernière image arrivée est gardée,
    /// puis rendue à la reprise : un bureau immobile n'en envoie peut-être pas
    /// d'autre (une fenêtre qui passe en plein écran est vue occultée un instant).
    var paused: Bool {
        get {
            lock.lock()
            defer { lock.unlock() }
            return isPaused
        }
        set {
            lock.lock()
            let resumed = isPaused && !newValue
            isPaused = newValue
            lock.unlock()
            if resumed { renderQueue.async { [weak self] in self?.resume() } }
        }
    }
    private var isPaused = false
    /// Dernière image arrivée pendant la pause.
    private var held: DecodedFrame?

    /// Nom du rendu dans le journal (multi-écrans : son écran).
    var label = ""

    /// L'écran qui montre ce rendu est HDR (surface, à chaque changement
    /// d'écran) : une image PQ y passe par la chaîne HDR. Un changement
    /// redessine l'image affichée dans la bonne chaîne.
    var hdrDisplay: Bool {
        get {
            lock.lock()
            defer { lock.unlock() }
            return hdrScreen
        }
        set {
            lock.lock()
            let changed = hdrScreen != newValue
            hdrScreen = newValue
            lock.unlock()
            if changed { renderQueue.async { [weak self] in self?.redrawInPlace() } }
        }
    }
    private var hdrScreen = false

    /// Marge EDR du moment de l'écran (couche `linear` : la courbe du shader
    /// s'y cale ; la surface la relit), et nits du blanc du bureau de
    /// l'écran (réglage du HDR), qui vaut 1,0.
    var edrHeadroom: Double {
        get { lock.lock(); defer { lock.unlock() }; return headroomNow }
        set { lock.lock(); headroomNow = newValue; lock.unlock() }
    }
    var sdrWhite: Double {
        get { lock.lock(); defer { lock.unlock() }; return sdrWhiteNits }
        set { lock.lock(); sdrWhiteNits = newValue; lock.unlock() }
    }
    private var headroomNow = 1.0
    private var sdrWhiteNits = 203.0

    /// Le cœur dit ce flux en HDR (`on_hdr`) : une image 10 bits sans
    /// étiquette de transfert est lue comme du PQ BT.2020.
    var streamHDR: Bool {
        get {
            lock.lock()
            defer { lock.unlock() }
            return hdrStream
        }
        set {
            lock.lock()
            hdrStream = newValue
            lock.unlock()
        }
    }
    private var hdrStream = false
    /// Chaîne de la couche en ce moment (fil de rendu).
    private var chain = Chain.sdr
    /// La chaîne de la couche change (fil de rendu) : HDR ou non.
    var onChain: ((Bool) -> Void)?

    #if os(tvOS)
    /// Apple TV : la couche vidéo du système où part l'image en chaîne HDR
    /// (nil : la couche Metal, comme ailleurs ; fil de rendu).
    private var video: HDRVideoLayer?
    /// La couche vidéo est montrée, la couche Metal cachée (fil principal) ;
    /// la surface les échange.
    var onVideoLayer: ((Bool) -> Void)?

    /// Branche la couche vidéo de la surface (nil : tout passe par Metal).
    func attachVideo(_ layer: HDRVideoLayer?) {
        renderQueue.sync {
            if video !== layer { video?.clear() }
            video = layer
            apply(chain, edr: appliedEDR, to: self.layer)
            let on = chain == .hdr && layer != nil
            DispatchQueue.main.async { [weak self] in self?.onVideoLayer?(on) }
        }
    }

    /// L'image part dans la couche vidéo (chaîne HDR sur l'Apple TV).
    private var throughVideo: Bool { chain == .hdr && video != nil }
    #endif

    /// Chaîne de la couche : SDR (BGRA 8 bits, sRGB) ou HDR (FP16, PQ, EDR).
    enum Chain: Equatable {
        case sdr
        case hdr
    }

    /// Trait de repère de la barre de session, par-dessus l'image. Coupé par
    /// défaut : l'Apple TV, l'iPhone et l'iPad ne l'ont pas. Un changement
    /// redessine l'image affichée (sans attendre la suivante, qui peut ne
    /// jamais venir sur un bureau immobile).
    var topMarker: Bool {
        get {
            lock.lock()
            defer { lock.unlock() }
            return markerOn
        }
        set {
            lock.lock()
            let changed = markerOn != newValue
            markerOn = newValue
            lock.unlock()
            if changed { renderQueue.async { [weak self] in self?.redrawInPlace() } }
        }
    }

    /// Pixels par point de la vue (`backingScaleFactor`) : le trait garde sa taille en points.
    var markerScale: CGFloat {
        get {
            lock.lock()
            defer { lock.unlock() }
            return markerPixelsPerPoint
        }
        set {
            lock.lock()
            markerPixelsPerPoint = newValue
            lock.unlock()
        }
    }
    private var markerOn = false
    private var markerPixelsPerPoint: CGFloat = 1

    private let device: MTLDevice
    private let queue: MTLCommandQueue
    private let pipeline: MTLRenderPipelineState
    /// Même shader, cible FP16 (chaîne HDR).
    private let hdrPipeline: MTLRenderPipelineState
    /// Trait de repère : absent si son shader n'a pas compilé (la session continue sans).
    private let markerPipeline: MTLRenderPipelineState?
    private let hdrMarkerPipeline: MTLRenderPipelineState?
    private var cache: CVMetalTextureCache?
    /// Matrice de couleur du format en cours, relue au plus une fois par seconde
    /// (fil de rendu) : `CVBufferCopyAttachment` à chaque image coûtait pour rien.
    private var colorCache: (format: OSType, width: Int, height: Int, read: CFTimeInterval, value: ColorConversion, edr: EDRInfo)?
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
    private var redrawnShown = 0

    /// GPU, file de commandes et pipeline, compilés une fois par processus
    /// (compilé depuis la source à chaque flux, le shader coûterait 30 à
    /// 150 ms sur le fil principal).
    private struct Core {
        let device: MTLDevice
        let queue: MTLCommandQueue
        let pipeline: MTLRenderPipelineState
        let hdrPipeline: MTLRenderPipelineState
    }

    /// Format de la couche dans chaque chaîne.
    static let sdrPixelFormat = MTLPixelFormat.bgra8Unorm
    static let hdrPixelFormat: MTLPixelFormat = hdrLayer == .pq10 ? .bgr10a2Unorm : .rgba16Float

    /// Couche de la chaîne HDR (Mac, iPhone, iPad), choisie une fois par
    /// processus : `KN_HDR_LAYER=edr|pq10|linear` pour comparer.
    /// - `edr` : FP16 en `itur_2100_PQ`, `edrMetadata` HDR10 (le compositeur
    ///   fait la courbe).
    /// - `pq10` : `bgr10a2Unorm` en `itur_2100_PQ`, sans `edrMetadata`.
    /// - `linear` : FP16 en `extendedLinearDisplayP3`, sans `edrMetadata` ; la
    ///   courbe est faite dans le shader d'après la marge EDR du moment
    ///   (`edrHeadroom`), le blanc du bureau à 1,0.
    enum HDRLayer: String {
        case edr, pq10, linear

        var label: String {
            switch self {
            case .edr: return "FP16, PQ, EDR, métadonnées HDR10"
            case .pq10: return "10 bits, PQ, EDR, sans métadonnées"
            case .linear: return "FP16, linéaire Display P3 étendu, courbe du shader"
            }
        }
    }
    static let hdrLayer: HDRLayer = {
        #if os(tvOS)
        return .edr
        #else
        return ProcessInfo.processInfo.environment["KN_HDR_LAYER"].flatMap(HDRLayer.init(rawValue:)) ?? defaultHDRLayer
        #endif
    }()
    static let defaultHDRLayer = HDRLayer.edr

    private static let core: Core? = {
        guard let device = MTLCreateSystemDefaultDevice(), let queue = device.makeCommandQueue() else { return nil }
        do {
            let library = try device.makeLibrary(source: shader, options: nil)
            let desc = MTLRenderPipelineDescriptor()
            desc.vertexFunction = library.makeFunction(name: "kn_vertex")
            desc.fragmentFunction = library.makeFunction(name: "kn_fragment")
            desc.colorAttachments[0].pixelFormat = sdrPixelFormat
            let sdr = try device.makeRenderPipelineState(descriptor: desc)
            desc.colorAttachments[0].pixelFormat = hdrPixelFormat
            let hdr = try device.makeRenderPipelineState(descriptor: desc)
            return Core(device: device, queue: queue, pipeline: sdr, hdrPipeline: hdr)
        } catch {
            KyberLog.video.error("rendu Metal : \(error.localizedDescription, privacy: .public)")
            return nil
        }
    }()

    /// Pipeline du trait de repère, compilé à part : un échec est au journal et
    /// n'empêche ni l'image ni la session.
    private static let markerCore: (sdr: MTLRenderPipelineState, hdr: MTLRenderPipelineState)? = {
        guard let device = core?.device else { return nil }
        do {
            let library = try device.makeLibrary(source: markerShader, options: nil)
            let desc = MTLRenderPipelineDescriptor()
            desc.vertexFunction = library.makeFunction(name: "kn_marker_vertex")
            desc.fragmentFunction = library.makeFunction(name: "kn_marker_fragment")
            let color = desc.colorAttachments[0]!
            color.pixelFormat = sdrPixelFormat
            color.isBlendingEnabled = true
            color.rgbBlendOperation = .add
            color.alphaBlendOperation = .add
            color.sourceRGBBlendFactor = .sourceAlpha
            color.sourceAlphaBlendFactor = .sourceAlpha
            color.destinationRGBBlendFactor = .oneMinusSourceAlpha
            color.destinationAlphaBlendFactor = .oneMinusSourceAlpha
            let sdr = try device.makeRenderPipelineState(descriptor: desc)
            color.pixelFormat = hdrPixelFormat
            return (sdr, try device.makeRenderPipelineState(descriptor: desc))
        } catch {
            KyberLog.video.error("trait de repère : \(error.localizedDescription, privacy: .public) ; la session continue sans lui")
            return nil
        }
    }()

    /// Compile le pipeline (au lancement, hors du fil principal) ; le trait de
    /// repère seulement sur Mac, les autres plateformes ne l'allument pas.
    static func warmUp() {
        _ = core
        #if os(macOS)
        _ = markerCore
        #endif
    }

    init?() {
        guard let core = Self.core else { return nil }
        device = core.device
        queue = core.queue
        pipeline = core.pipeline
        hdrPipeline = core.hdrPipeline
        markerPipeline = Self.markerCore?.sdr
        hdrMarkerPipeline = Self.markerCore?.hdr
        CVMetalTextureCacheCreate(kCFAllocatorDefault, nil, device, nil, &cache)
        layer = CAMetalLayer()
        configure(layer)
    }

    /// Dessine dans `layer` à partir de cet appel (nil : dans une couche à soi, hors écran).
    func attach(to layer: CAMetalLayer?) {
        renderQueue.sync {
            self.layer = layer ?? CAMetalLayer()
            configure(self.layer)
        }
    }

    private func configure(_ layer: CAMetalLayer) {
        layer.device = device
        layer.framebufferOnly = true
        layer.isOpaque = true
        layer.backgroundColor = CGColor(gray: 0, alpha: 1)
        layer.allowsNextDrawableTimeout = true
        #if os(macOS)
        layer.displaySyncEnabled = vsync
        #endif
        apply(chain, edr: nil, to: layer)
    }

    /// Règle la couche pour une chaîne (fil de rendu). HDR : FP16, PQ BT.2100,
    /// contenu EDR et métadonnées HDR10 (celles du flux, sinon 1000 nits).
    private func apply(_ chain: Chain, edr: EDRInfo?, to layer: CAMetalLayer) {
        #if os(tvOS)
        // L'image HDR part dans la couche vidéo : la couche Metal reste en SDR.
        let chain = video != nil ? Chain.sdr : chain
        #endif
        switch chain {
        case .sdr:
            layer.pixelFormat = Self.sdrPixelFormat
            layer.colorspace = CGColorSpace(name: CGColorSpace.sRGB)
            #if !os(tvOS)
            layer.wantsExtendedDynamicRangeContent = false
            layer.edrMetadata = nil
            #endif
        case .hdr:
            layer.pixelFormat = Self.hdrPixelFormat
            layer.colorspace = CGColorSpace(name: Self.hdrLayer == .linear ? CGColorSpace.extendedLinearDisplayP3 : CGColorSpace.itur_2100_PQ)
            #if !os(tvOS)
            layer.wantsExtendedDynamicRangeContent = true
            guard Self.hdrLayer == .edr else {
                // Sans métadonnées : pas de courbe du compositeur.
                layer.edrMetadata = nil
                return
            }
            // Valeurs PQ absolues : 100 nits de la scène valent 1,0 de l'écran (référence SDR).
            if let edr, edr.display != nil || edr.content != nil {
                layer.edrMetadata = CAEDRMetadata.hdr10(displayInfo: edr.display, contentInfo: edr.content, opticalOutputScale: 100)
            } else {
                layer.edrMetadata = CAEDRMetadata.hdr10(minLuminance: 0.005, maxLuminance: 1000, opticalOutputScale: 100)
            }
            #endif
        }
    }

    /// Passe la couche dans la chaîne voulue pour cette image (fil de rendu).
    private func select(_ wanted: Chain, edr: EDRInfo?) {
        guard wanted != chain || (wanted == .hdr && edr != appliedEDR) else { return }
        let from = chain
        chain = wanted
        appliedEDR = wanted == .hdr ? edr : nil
        apply(wanted, edr: edr, to: layer)
        if from != wanted {
            #if os(tvOS)
            let hdrLabel = video != nil ? "HDR (couche vidéo du système, image décodée telle quelle)" : "HDR (couche Metal FP16, PQ)"
            if let video {
                if wanted == .sdr { video.clear() }
                let on = wanted == .hdr
                DispatchQueue.main.async { [weak self] in self?.onVideoLayer?(on) }
            }
            #else
            let hdrLabel = "HDR (\(Self.hdrLayer.label))"
            #endif
            KyberLog.video.notice("chaîne \(wanted == .hdr ? hdrLabel : "SDR", privacy: .public)\(self.label.isEmpty ? "" : " (\(self.label))", privacy: .public)")
            onChain?(wanted == .hdr)
        }
    }
    /// Métadonnées HDR10 posées sur la couche (fil de rendu).
    private var appliedEDR: EDRInfo?

    /// Chaîne voulue pour une image : HDR si l'image est PQ et l'écran HDR ;
    /// la conversion dit alors au shader d'écrire le PQ tel quel, ou de le
    /// ramener en SDR (écran SDR).
    private func route(_ color: ColorConversion) -> (Chain, ColorConversion) {
        guard color.mode != ColorConversion.sdr else { return (.sdr, color) }
        var c = color
        let hdr = hdrDisplay
        c.mode = hdr ? Self.hdrMode : ColorConversion.pqToSDR
        return (hdr ? .hdr : .sdr, c)
    }

    /// Taille de l'image à l'écran, en pixels réels.
    func setDrawableSize(_ size: CGSize) {
        guard size.width >= 1, size.height >= 1 else { return }
        renderQueue.async { [weak self] in
            guard let self, self.layer.drawableSize != size else { return }
            self.layer.drawableSize = size
            // L'image affichée l'était à l'ancienne taille (fenêtre qui passe
            // en plein écran) : redessinée tout de suite, sans attendre la suivante.
            self.redrawInPlace()
        }
    }

    /// Redessine l'image affichée (fenêtre redevenue visible, passage en
    /// plein écran fini) : une image présentée pendant l'animation a pu être
    /// écartée, et un bureau immobile n'en envoie pas d'autre.
    func redraw() {
        renderQueue.async { [weak self] in self?.redrawInPlace() }
    }

    /// Fin de la pause (fil de rendu) : la dernière image arrivée pendant la
    /// pause est rendue, sinon l'image affichée est redessinée.
    private func resume() {
        lock.lock()
        let frame = held
        held = nil
        var go = false
        if let frame, pending == nil {
            pending = frame
            if !scheduled {
                scheduled = true
                go = true
            }
        }
        lock.unlock()
        if go { render() } else if frame == nil { redrawInPlace() }
    }

    /// Nouvelle image (fil du décodeur) : elle remplace celle qui attendait.
    func submit(_ frame: DecodedFrame) {
        lock.lock()
        if isPaused {
            if let replaced = held {
                skipped += 1
                latency?.skipped(pts: replaced.pts)
            }
            held = frame
            lock.unlock()
            return
        }
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
        // La chaîne d'abord (le tampon doit être pris dans le bon format) :
        // celle de l'image en attente, la même que la suivante presque toujours.
        lock.lock()
        let peek = pending
        lock.unlock()
        if let peek, let layout = Self.layout(CVPixelBufferGetPixelFormatType(peek.buffer)) {
            let color = conversion(peek.buffer, layout: layout)
            select(route(color).0, edr: colorCache?.edr)
        }
        #if os(tvOS)
        if throughVideo, let video {
            renderThroughVideo(video)
            return
        }
        #endif
        // Le tampon ensuite (il peut faire attendre un balayage quand tous
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
              draw(buffer, layout: layout, into: drawable.texture, commands: commands, color: routed(conversion(buffer, layout: layout), for: drawable.texture), marker: currentMarkerScale())
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
        // estime (fin du GPU plus un rafraîchissement à 120 Hz) : une image n'est jamais tenue pour écartée
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

    #if os(tvOS)
    /// Chaîne HDR de l'Apple TV : l'image en attente part telle quelle dans
    /// la couche vidéo du système (fil de rendu). tvOS ne dit pas quand elle
    /// est montrée : l'instant est estimé (balayage suivant, à 60 Hz).
    private func renderThroughVideo(_ video: HDRVideoLayer) {
        lock.lock()
        let frame = pending
        pending = nil
        scheduled = false
        lock.unlock()
        guard let frame else { return }
        let buffer = frame.buffer
        guard let layout = Self.layout(CVPixelBufferGetPixelFormatType(buffer)), video.show(buffer) else {
            latency?.skipped(pts: frame.pts)
            return
        }
        let committed = CACurrentMediaTime()
        present.rendered += 1
        lock.lock()
        shown = (buffer, layout)
        lock.unlock()
        presented(at: committed + 1.0 / 60, committed: committed, received: frame.received, decoded: frame.decoded, pts: frame.pts, estimated: true)
        account(frame, width: CVPixelBufferGetWidth(buffer), height: CVPixelBufferGetHeight(buffer), layout: layout)
    }
    #endif

    /// Échelle du trait de repère s'il doit être tracé, sinon nil.
    private func currentMarkerScale() -> Double? {
        lock.lock()
        defer { lock.unlock() }
        return markerOn ? Double(markerPixelsPerPoint) : nil
    }

    /// Le trait de repère change : l'image affichée est redessinée telle
    /// quelle, avec ou sans le trait (fil de rendu). Une image déjà en attente
    /// le portera de toute façon ; fenêtre occultée, rien n'est rendu.
    private func redrawInPlace() {
        lock.lock()
        let last = shown
        let waiting = pending != nil || scheduled
        lock.unlock()
        guard !paused, !waiting, let last, layer.drawableSize.width >= 1 else { return }
        // Écran changé (HDR ou non) : la chaîne suit, l'image est redessinée dedans.
        select(route(conversion(last.buffer, layout: last.layout)).0, edr: colorCache?.edr)
        #if os(tvOS)
        if throughVideo, let video {
            video.show(last.buffer)
            return
        }
        #endif
        guard let drawable = layer.nextDrawable(), let commands = queue.makeCommandBuffer(),
              draw(last.buffer, layout: last.layout, into: drawable.texture, commands: commands,
                   color: routed(conversion(last.buffer, layout: last.layout), for: drawable.texture), marker: currentMarkerScale())
        else { return }
        #if !targetEnvironment(simulator)
        // Le simulateur ne dit pas quand l'image est montrée.
        drawable.addPresentedHandler { [weak self] d in
            guard d.presentedTime > 0 else { return }
            self?.renderQueue.async { self?.redrawnShown += 1 }
        }
        #endif
        commands.present(drawable)
        commands.commit()
    }

    /// Matrice de couleur de l'image, lue à chaque changement de format ou de
    /// taille et au plus une fois par seconde (fil de rendu).
    private func conversion(_ buffer: CVPixelBuffer, layout: Layout) -> ColorConversion {
        let format = CVPixelBufferGetPixelFormatType(buffer)
        let width = CVPixelBufferGetWidth(buffer), height = CVPixelBufferGetHeight(buffer)
        let now = CACurrentMediaTime()
        if let c = colorCache, c.format == format, c.width == width, c.height == height, now - c.read < 1 { return c.value }
        var value = Self.color(buffer, fullRange: layout.full, deep: layout.deep)
        if value.mode == ColorConversion.sdr, layout.deep, streamHDR,
           CVBufferCopyAttachment(buffer, kCVImageBufferTransferFunctionKey, nil) == nil {
            value = Self.color(buffer, fullRange: layout.full, deep: true, matrix: String(kCVImageBufferYCbCrMatrix_ITU_R_2020))
            value.mode = ColorConversion.pqToSDR
        }
        if colorCache?.format != format || colorCache?.width != width || colorCache?.height != height || (colorCache?.value.mode == ColorConversion.sdr) != (value.mode == ColorConversion.sdr) {
            KyberLog.video.notice("images \(Self.fourcc(format), privacy: .public) \(width)×\(height), \(layout.label, privacy: .public)\(value.mode == ColorConversion.sdr ? "" : ", PQ BT.2020", privacy: .public)\(CVPixelBufferGetIOSurface(buffer) == nil ? ", sans IOSurface" : "", privacy: .public)")
        }
        // Métadonnées HDR10 : le flux ne les porte qu'avec les images clés ;
        // une image qui n'en a pas garde les dernières lues (sinon la couche
        // retomberait sur 1 000 nits par défaut à chaque relecture).
        var edr = EDRInfo()
        if value.mode != ColorConversion.sdr {
            edr = EDRInfo(buffer)
            if edr.isEmpty, let last = colorCache?.edr, !last.isEmpty { edr = last }
        }
        colorCache = (format, width, height, now, value, edr)
        return value
    }

    /// La conversion d'une image PQ selon la cible : PQ tel quel dans une
    /// cible FP16 (chaîne HDR), ramené en SDR dans une cible 8 bits.
    private func routed(_ color: ColorConversion, for target: MTLTexture) -> ColorConversion {
        guard color.mode != ColorConversion.sdr else { return color }
        var c = color
        c.mode = target.pixelFormat == Self.hdrPixelFormat ? Self.hdrMode : ColorConversion.pqToSDR
        if c.mode == ColorConversion.pqToEDR {
            lock.lock()
            c.headroom = Float(max(headroomNow, 1))
            c.white = Float(sdrWhiteNits)
            lock.unlock()
        }
        return c
    }

    /// Dessine l'image dans `target`, centrée et entière. Faux si les plans
    /// ne se lisent pas en texture. `color` : conversion déjà connue (sinon lue
    /// sur l'image). `marker` : pixels par point du trait de repère, à tracer
    /// par-dessus (nil : pas de trait, comme pour les photographies).
    func draw(_ buffer: CVPixelBuffer, layout: Layout, into target: MTLTexture, commands: MTLCommandBuffer,
              color known: ColorConversion? = nil, marker: Double? = nil) -> Bool {
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
        let hdrTarget = target.pixelFormat == Self.hdrPixelFormat
        encoder.setRenderPipelineState(hdrTarget ? hdrPipeline : pipeline)
        encoder.setFragmentTexture(CVMetalTextureGetTexture(luma), index: 0)
        encoder.setFragmentTexture(CVMetalTextureGetTexture(chroma), index: 1)
        var color = routed(known ?? Self.color(buffer, fullRange: layout.full, deep: layout.deep), for: target)
        encoder.setFragmentBytes(&color, length: MemoryLayout<ColorConversion>.stride, index: 0)
        encoder.drawPrimitives(type: .triangle, vertexStart: 0, vertexCount: 3)
        if let marker, let markerPipeline = hdrTarget ? hdrMarkerPipeline : markerPipeline {
            // Capsule de 220 × 3 points contre le bord haut de la vue (et non de
            // l'image : l'image peut être cernée de noir), centrée, mélange alpha.
            let markerW = (220 * marker).rounded(), markerH = (3 * marker).rounded()
            let x = ((targetW - markerW) / 2).rounded()
            // Blanc du trait : 1 en SDR ; en PQ, le blanc de référence (203 nits).
            var uniforms = MarkerUniforms(rect: SIMD4(Float(x), 0, Float(x + markerW), Float(markerH)),
                                          target: SIMD2(Float(targetW), Float(targetH)), opacity: 0.35, white: hdrTarget && Self.hdrLayer != .linear ? 0.58 : 1)
            encoder.setViewport(MTLViewport(originX: 0, originY: 0, width: targetW, height: targetH, znear: 0, zfar: 1))
            encoder.setRenderPipelineState(markerPipeline)
            encoder.setVertexBytes(&uniforms, length: MemoryLayout<MarkerUniforms>.stride, index: 0)
            encoder.setFragmentBytes(&uniforms, length: MemoryLayout<MarkerUniforms>.stride, index: 0)
            encoder.drawPrimitives(type: .triangleStrip, vertexStart: 0, vertexCount: 4)
        }
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

    /// Relève tout de suite les mesures de présentation (tests : elles ne
    /// partent sinon qu'avec l'image qui suit chaque seconde).
    func reportPresentationNow() {
        renderQueue.sync { reportPresentation() }
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
        stats.redrawn = redrawnShown
        redrawnShown = 0
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
            KyberLog.video.error("rien de montré en 2 s\(self.label.isEmpty ? "" : " (\(self.label))", privacy: .public) : \(stats.rendered) images rendues, toutes écartées par l'écran, \(Int(self.layer.drawableSize.width))×\(Int(self.layer.drawableSize.height))")
            // Une image écartée n'est pas reprise : la dernière est redessinée.
            redrawInPlace()
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

    /// Même disposition que `Conversion` du shader (80 octets).
    struct ColorConversion {
        var matrix: simd_float3x3
        var offset: simd_float3
        /// `sdr` : R'G'B' tels quels ; `pq` : valeurs PQ telles quelles (cible
        /// FP16, espace PQ) ; `pqToSDR` : image PQ ramenée en SDR BT.709.
        var mode: Int32 = 0

        /// Marge EDR de l'écran (`pqToEDR`) et nits du blanc du bureau, posé à 1,0.
        var headroom: Float = 1
        var white: Float = 203

        static let sdr: Int32 = 0
        static let pq: Int32 = 1
        static let pqToSDR: Int32 = 2
        /// Image PQ en lumière linéaire Display P3 étendue (couche `linear`).
        static let pqToEDR: Int32 = 3
    }

    /// Mode du shader dans la chaîne HDR, selon la couche.
    static var hdrMode: Int32 { hdrLayer == .linear ? ColorConversion.pqToEDR : ColorConversion.pq }

    /// Métadonnées HDR10 du flux (SEI mastering display et MaxCLL), telles
    /// que VideoToolbox les attache à l'image.
    struct EDRInfo: Equatable {
        var display: Data?
        var content: Data?

        init() {}

        init(_ buffer: CVPixelBuffer) {
            display = CVBufferCopyAttachment(buffer, kCVImageBufferMasteringDisplayColorVolumeKey, nil) as? Data
            content = CVBufferCopyAttachment(buffer, kCVImageBufferContentLightLevelInfoKey, nil) as? Data
        }

        var isEmpty: Bool { display == nil && content == nil }
    }

    /// L'image est en PQ (SMPTE ST 2084) : étiquetée ainsi par le flux.
    static func isPQ(_ buffer: CVPixelBuffer) -> Bool {
        let transfer = CVBufferCopyAttachment(buffer, kCVImageBufferTransferFunctionKey, nil) as? String
        return transfer == String(kCVImageBufferTransferFunction_SMPTE_ST_2084_PQ)
    }

    /// Paramètres du trait de repère (même disposition que `MarkerU` du shader) :
    /// rectangle en pixels (x0, y0, x1, y1, origine en haut à gauche), taille
    /// de la cible, opacité.
    struct MarkerUniforms {
        var rect: SIMD4<Float>
        var target: SIMD2<Float>
        var opacity: Float
        var white: Float
    }

    /// YCbCr → RGB selon la matrice annoncée par le flux (709 par défaut) et la plage.
    static func color(_ buffer: CVPixelBuffer, fullRange: Bool, deep: Bool, matrix: String? = nil) -> ColorConversion {
        let matrixName = matrix ?? CVBufferCopyAttachment(buffer, kCVImageBufferYCbCrMatrixKey, nil) as? String
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
        // Une image PQ se ramène en SDR par défaut (photographie, cible 8 bits) ;
        // le rendu la laisse en PQ dans la chaîne HDR.
        return ColorConversion(matrix: rgb * range, offset: simd_float3(yOffset, cOffset, cOffset),
                               mode: isPQ(buffer) ? ColorConversion.pqToSDR : ColorConversion.sdr)
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

    /// Capsule (rectangle aux bouts ronds) en distance signée, bords adoucis
    /// sur un pixel ; blanche, de l'opacité demandée, mélangée à l'image.
    private static let markerShader = """
    #include <metal_stdlib>
    using namespace metal;

    struct MarkerU { float4 rect; float2 target; float opacity; float white; };
    struct MarkerOut { float4 position [[position]]; float2 local; };

    // Un rectangle agrandi d'un pixel, en bande de deux triangles.
    vertex MarkerOut kn_marker_vertex(uint id [[vertex_id]], constant MarkerU &u [[buffer(0)]]) {
        float2 corner = float2(id & 1, id >> 1);
        float2 px = mix(u.rect.xy - 1.0, u.rect.zw + 1.0, corner);
        MarkerOut out;
        out.position = float4(px.x / u.target.x * 2.0 - 1.0, 1.0 - px.y / u.target.y * 2.0, 0.0, 1.0);
        out.local = px - (u.rect.xy + u.rect.zw) * 0.5;
        return out;
    }

    fragment float4 kn_marker_fragment(MarkerOut in [[stage_in]], constant MarkerU &u [[buffer(0)]]) {
        float2 halfSize = (u.rect.zw - u.rect.xy) * 0.5;
        float r = halfSize.y;
        float2 q = abs(in.local) - float2(halfSize.x - r, 0.0);
        float d = length(max(q, 0.0)) - r;
        return float4(float3(u.white), u.opacity * saturate(0.5 - d));
    }
    """

    private static let shader = """
    #include <metal_stdlib>
    using namespace metal;

    struct VOut { float4 position [[position]]; float2 uv; };
    struct Conversion { float3x3 matrix; float3 offset; int mode; float headroom; float white; };

    // PQ (SMPTE ST 2084) → lumière linéaire, 1 = 10 000 nits.
    float3 kn_pq_eotf(float3 e) {
        const float m1 = 0.1593017578125, m2 = 78.84375;
        const float c1 = 0.8359375, c2 = 18.8515625, c3 = 18.6875;
        float3 p = pow(max(e, 0.0), 1.0 / m2);
        return pow(max(p - c1, 0.0) / (c2 - c3 * p), 1.0 / m1);
    }

    // Image PQ BT.2020 ramenée en SDR BT.709 : blanc SDR à 203 nits, genou à
    // 75 % du blanc, compression douce jusqu'à 4 fois le blanc (comme l'image
    // à part de la Station), puis la courbe sRGB.
    float3 kn_pq_to_sdr(float3 pq) {
        float3 l = kn_pq_eotf(pq) * (10000.0 / 203.0);
        float3 c = float3(dot(float3( 1.6605, -0.5876, -0.0728), l),
                          dot(float3(-0.1246,  1.1329, -0.0083), l),
                          dot(float3(-0.0182, -0.1006,  1.1187), l));
        c = max(c, 0.0);
        float m = max(max(c.r, c.g), c.b);
        const float k = 0.75, b = (4.0 - k) / (1.0 - k);
        if (m > k) {
            float a = (m - k) / (1.0 - k);
            float y = k + (1.0 - k) * a * (1.0 + a / (b * b)) / (1.0 + a);
            c *= y / m;
        }
        c = saturate(c);
        return select(1.055 * pow(c, 1.0 / 2.4) - 0.055, 12.92 * c, c <= 0.0031308);
    }

    // Image PQ BT.2020 en lumière linéaire Display P3 étendue (couche
    // `linear`) : le blanc du bureau vaut 1,0 ; au-dessus de 80 % de la marge
    // EDR du moment, compression douce jusqu'à la marge (10 000 nits au bout).
    float3 kn_pq_to_edr(float3 pq, float white, float headroom) {
        float3 l = kn_pq_eotf(pq) * (10000.0 / white);
        float3 c = float3(dot(float3( 1.3436, -0.2822, -0.0614), l),
                          dot(float3(-0.0653,  1.0758, -0.0105), l),
                          dot(float3( 0.0028, -0.0196,  1.0168), l));
        c = max(c, 0.0);
        float h = max(headroom, 1.0);
        float k = h > 1.01 ? 0.8 * h : 0.75;
        float top = 10000.0 / white;
        float m = max(max(c.r, c.g), c.b);
        if (m > k && top > h) {
            float a = (m - k) / (h - k);
            float b = (top - k) / (h - k);
            float y = k + (h - k) * a * (1.0 + a / (b * b)) / (1.0 + a);
            c *= y / m;
        }
        return c;
    }

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
        float3 rgb = saturate(c.matrix * (ycbcr - c.offset));
        if (c.mode == 2) { rgb = kn_pq_to_sdr(rgb); }
        else if (c.mode == 3) { rgb = kn_pq_to_edr(rgb, c.white, c.headroom); }
        return float4(rgb, 1.0);
    }
    """
}
