// Décodage PyroWave (docs/pyrowave.md) : codec par ondelettes, intra seul,
// décodé sur le GPU par le port Metal de PyroWave (apps/macos/PyroWave,
// compilé dans KyberKit, module CPyroWave).
//
// Contrat avec la Station : un paquet de configuration `KNPW` (version 1,
// 16 octets, ou 2, 48 octets avec les métadonnées HDR10 ; `PyroWaveConfig`,
// lu par le cœur, renvoyé à chaque changement de définition) puis des
// paquets média, chacun une image complète (paquets PyroWave mis bout à
// bout), toujours image clé. Une image perdue ou incomplète est sautée,
// jamais demandée de nouveau : la suivante se décode seule.
//
// Sortie : un `CVPixelBuffer` bi-plan (IOSurface) du même format que ceux de
// VideoToolbox, étiqueté BT.709 ou BT.2020 / PQ et de la bonne plage, que le
// rendu Metal (`VideoRenderer`) affiche sans rien savoir de PyroWave. Le plan
// Y est écrit directement par PyroWave dans l'IOSurface ; Cb et Cr sortent
// dans deux textures à part, qu'une petite passe entrelace dans le plan CbCr.
// SDR : 8 bits (`420v`/`420f`, 4:4:4 `444v`/`444f`) ; HDR10 : 10 bits calés
// en haut de 16 bits (`x420`/`xf20`, `x444`/`xf44`), lus en r16Unorm — la
// valeur normalisée de PyroWave y tombe telle quelle (convention P010).
//
// Fil : `decode` (fil réseau de Kyber) pousse l'image dans PyroWave, encode
// le décodage dans un tampon de commandes et rend la main ; l'image part au
// rendu depuis le fil de fin du tampon (GPU terminé), comme VideoToolbox.
// Au plus `maxInFlight` images en cours sur le GPU : au-delà, l'image est
// sautée plutôt que mise en file (latence).
//
// Indépendant d'AppKit : sert tel quel sur iOS et tvOS.

import CoreVideo
import Foundation
import Metal
@_implementationOnly import CPyroWave

/// Le GPU et les objets PyroWave partagés par tous les décodeurs : la
/// création compile les pipelines de PyroWave (une fois par processus).
final class PyroWaveGPU: @unchecked Sendable {
    let device: MTLDevice
    let queue: MTLCommandQueue
    let interleave: MTLComputePipelineState
    fileprivate let handle: pyrowave_device

    /// Nil si ce Mac (ou cet appareil) ne peut pas décoder PyroWave : GPU
    /// d'avant Apple7 (A14, M1), Intel ou AMD, ou pipelines refusés.
    static let shared: PyroWaveGPU? = PyroWaveGPU()

    private init?() {
        guard let device = MTLCreateSystemDefaultDevice(),
              pyrowave_device_is_supported(Unmanaged.passUnretained(device).toOpaque()),
              let queue = device.makeCommandQueue()
        else {
            KyberLog.video.notice("PyroWave : GPU non pris en charge")
            return nil
        }
        queue.label = "PyroWave"
        var info = pyrowave_device_create_info()
        info.mtl_device = Unmanaged.passUnretained(device).toOpaque()
        info.message_callback = { _, message in
            guard let message else { return }
            KyberLog.video.error("PyroWave : \(String(cString: message), privacy: .public)")
        }
        var handle: pyrowave_device?
        let start = CFAbsoluteTimeGetCurrent()
        let result = pyrowave_device_create(&info, &handle)
        guard result == PYROWAVE_SUCCESS, let handle else {
            KyberLog.video.error("PyroWave : device refusé (\(String(cString: pyrowave_result_to_string(result)), privacy: .public))")
            return nil
        }
        let interleave: MTLComputePipelineState
        do {
            let library = try device.makeLibrary(source: Self.interleaveSource, options: nil)
            guard let function = library.makeFunction(name: "pyrowave_interleave_cbcr") else { throw CocoaError(.featureUnsupported) }
            interleave = try device.makeComputePipelineState(function: function)
        } catch {
            KyberLog.video.error("PyroWave : passe CbCr refusée (\(error.localizedDescription, privacy: .public))")
            pyrowave_device_destroy(handle)
            return nil
        }
        self.device = device
        self.queue = queue
        self.handle = handle
        self.interleave = interleave
        KyberLog.video.notice("PyroWave prêt en \(Int((CFAbsoluteTimeGetCurrent() - start) * 1000)) ms (\(device.name, privacy: .public))")
    }

    /// Cb et Cr (deux textures d'une composante) → plan CbCr entrelacé.
    private static let interleaveSource = """
    #include <metal_stdlib>
    using namespace metal;
    kernel void pyrowave_interleave_cbcr(texture2d<float, access::read> cb [[texture(0)]],
                                         texture2d<float, access::read> cr [[texture(1)]],
                                         texture2d<float, access::write> cbcr [[texture(2)]],
                                         uint2 gid [[thread_position_in_grid]]) {
        if (gid.x >= cbcr.get_width() || gid.y >= cbcr.get_height()) return;
        cbcr.write(float4(cb.read(gid).r, cr.read(gid).r, 0.0, 1.0), gid);
    }
    """
}

/// Décodeur PyroWave d'un flux. Même rôle que la session VideoToolbox de
/// `VideoDecoder`, qui lui passe les paquets quand le flux est en PyroWave.
final class PyroWaveDecoder {
    /// Image prête (fil de fin du tampon de commandes).
    var onFrame: ((DecodedFrame) -> Void)?
    /// PyroWave ne s'ouvre pas sur cet appareil (pas de GPU qui le décode,
    /// décodeur ou mémoire refusés) : dit une fois, pour que le cœur relance
    /// l'image dans le format de repli (`kn_session_pyrowave_failed`).
    var onUnavailable: (() -> Void)?
    private var unavailableTold = false

    /// Images au plus sur le GPU à la fois ; au-delà, l'image est sautée.
    static let maxInFlight = 3

    private(set) var config: PyroWaveConfig?
    private let gpu: PyroWaveGPU?
    private var decoder: pyrowave_decoder?
    private var pool: CVPixelBufferPool?
    /// Cb et Cr tels que PyroWave les sort (privées au GPU, 16 bits).
    private var chroma: (MTLTexture, MTLTexture)?
    /// Vues Metal des plans de chaque IOSurface du pool (réutilisées).
    private var planes: [IOSurfaceID: (y: MTLTexture, cbcr: MTLTexture)] = [:]
    private var attachments: [CFString: Any] = [:]

    private let lock = NSLock()
    /// Images en cours sur le GPU (protégé par `out`).
    private var inFlight = 0
    private let out = NSLock()
    /// Compteurs (journal, tests ; protégés par `out`) : sautées faute de
    /// place, incomplètes, erreurs.
    fileprivate(set) var skipped = 0
    fileprivate(set) var incomplete = 0
    fileprivate(set) var failures = 0

    init(gpu: PyroWaveGPU? = PyroWaveGPU.shared) {
        self.gpu = gpu
    }

    deinit { teardown() }

    // MARK: - Paquets

    /// Paquet de configuration du contrat : (re)crée le décodeur s'il change.
    /// Faux si le paquet ne se lit pas.
    @discardableResult
    func configure(_ data: UnsafeBufferPointer<UInt8>) -> Bool {
        guard let config = PyroWaveConfig(data) else {
            KyberLog.video.error("PyroWave : paquet de configuration illisible (\(data.count) octets)")
            return false
        }
        lock.lock()
        defer { lock.unlock() }
        apply(config)
        return true
    }

    func configure(_ config: PyroWaveConfig) {
        lock.lock()
        defer { lock.unlock() }
        apply(config)
    }

    /// Une image complète (paquets PyroWave bout à bout). Sautée si elle est
    /// incomplète, illisible, ou si le GPU a déjà `maxInFlight` images.
    func decode(_ data: UnsafeBufferPointer<UInt8>, pts: UInt64, received: UInt64 = mach_absolute_time()) {
        lock.lock()
        defer { lock.unlock() }
        if config == nil, let fallback = PyroWaveConfig(sequenceHeader: data) {
            KyberLog.video.notice("PyroWave : pas de paquet de configuration, définition lue dans le flux (\(fallback.label, privacy: .public))")
            apply(fallback)
        }
        guard let gpu, let decoder, let pool, let chroma, let config, let base = data.baseAddress else { return }
        out.lock()
        let busy = inFlight >= Self.maxInFlight
        out.unlock()
        if busy {
            note(\.skipped, "GPU en retard, image sautée")
            return
        }

        // Chaque paquet média est une image entière : rien à garder de la précédente.
        pyrowave_decoder_clear(decoder)
        let pushed = pyrowave_decoder_push_packet(decoder, base, data.count)
        guard pushed == PYROWAVE_SUCCESS else {
            note(\.failures, "image illisible (\(String(cString: pyrowave_result_to_string(pushed))))")
            return
        }
        // Image entière seulement (MAC-24) : une image incomplète est sautée,
        // comme le disent docs/pyrowave.md et l'essai (la suivante, toujours
        // image clé, répare tout de suite) ; l'accepter montrait des trous.
        guard pyrowave_decoder_decode_is_ready(decoder, false) else {
            note(\.incomplete, "image incomplète, sautée")
            return
        }

        var buffer: CVPixelBuffer?
        let aux = [kCVPixelBufferPoolAllocationThresholdKey: Self.maxInFlight + 4] as CFDictionary
        guard CVPixelBufferPoolCreatePixelBufferWithAuxAttributes(kCFAllocatorDefault, pool, aux, &buffer) == kCVReturnSuccess,
              let buffer, let target = planes(of: buffer, config: config, device: gpu.device),
              let commands = gpu.queue.makeCommandBuffer()
        else {
            note(\.skipped, "pas de tampon de sortie libre, image sautée")
            return
        }
        CVBufferSetAttachments(buffer, attachments as CFDictionary, .shouldPropagate)

        var outputs = pyrowave_gpu_buffers()
        outputs.planes = (Unmanaged.passUnretained(target.y).toOpaque(),
                          Unmanaged.passUnretained(chroma.0).toOpaque(),
                          Unmanaged.passUnretained(chroma.1).toOpaque())
        let decoded = pyrowave_decoder_decode_gpu_buffer(decoder, Unmanaged.passUnretained(commands).toOpaque(), &outputs)
        guard decoded == PYROWAVE_SUCCESS else {
            note(\.failures, "décodage refusé (\(String(cString: pyrowave_result_to_string(decoded))))")
            return
        }
        // Décodage accepté : l'emplacement d'envoi attend désormais ce tampon
        // (`slot->consumer`). Il est validé quoi qu'il arrive, sinon le
        // quatrième envoi suivant attendrait pour toujours (MAC-02).
        guard let pass = commands.makeComputeCommandEncoder() else {
            commands.commit()
            note(\.failures, "passe d'entrelacement refusée, image sautée")
            return
        }
        pass.setComputePipelineState(gpu.interleave)
        pass.setTexture(chroma.0, index: 0)
        pass.setTexture(chroma.1, index: 1)
        pass.setTexture(target.cbcr, index: 2)
        let w = gpu.interleave.threadExecutionWidth
        let h = max(1, gpu.interleave.maxTotalThreadsPerThreadgroup / w)
        pass.dispatchThreads(MTLSize(width: config.chromaWidth, height: config.chromaHeight, depth: 1),
                             threadsPerThreadgroup: MTLSize(width: w, height: h, depth: 1))
        pass.endEncoding()

        out.lock()
        inFlight += 1
        out.unlock()
        commands.addCompletedHandler { [weak self] done in
            let now = mach_absolute_time()
            guard let self else { return }
            self.out.lock()
            self.inFlight -= 1
            self.out.unlock()
            guard done.status == .completed else {
                self.note(\.failures, "GPU en erreur (\(done.error?.localizedDescription ?? "?"))")
                return
            }
            self.onFrame?(DecodedFrame(buffer: buffer, pts: pts, received: received, decoded: now))
        }
        commands.commit()
    }

    /// Attend les images en cours sur le GPU (tests, sonde).
    func flush() {
        guard let gpu else { return }
        gpu.queue.makeCommandBuffer().map { $0.commit(); $0.waitUntilCompleted() }
        // Les rappels de fin passent juste après la fin du tampon.
        while true {
            out.lock()
            let busy = inFlight > 0
            out.unlock()
            if !busy { return }
            usleep(200)
        }
    }

    func invalidate() {
        lock.lock()
        defer { lock.unlock() }
        teardown()
        config = nil
    }

    // MARK: - Sonde

    /// Sonde du lancement (`DecoderCapabilities`) : crée un décodeur, décode
    /// pour de bon une image de test et vérifie ce qui sort. Faux si le GPU ne
    /// prend pas PyroWave en charge ou si l'image décodée n'est pas la bonne.
    static func probe() -> Bool {
        guard PyroWaveGPU.shared != nil else { return false }
        let decoder = PyroWaveDecoder()
        decoder.configure(PyroWaveConfig(chroma444: false, hdr: false, fullRange: false, width: 128, height: 128))
        var output: CVPixelBuffer?
        let lock = NSLock()
        decoder.onFrame = { frame in
            lock.lock()
            output = frame.buffer
            lock.unlock()
        }
        sample.withUnsafeBytes { decoder.decode($0.bindMemory(to: UInt8.self), pts: 1) }
        decoder.flush()
        lock.lock()
        let buffer = output
        lock.unlock()
        guard let buffer else { return false }
        // Rampe horizontale de Y (16 à gauche, 235 à droite, plage réduite),
        // Cb et Cr à 128 : quelques points, à 4 près.
        CVPixelBufferLockBaseAddress(buffer, .readOnly)
        defer { CVPixelBufferUnlockBaseAddress(buffer, .readOnly) }
        guard let y = CVPixelBufferGetBaseAddressOfPlane(buffer, 0), let c = CVPixelBufferGetBaseAddressOfPlane(buffer, 1) else { return false }
        let yRow = CVPixelBufferGetBytesPerRowOfPlane(buffer, 0), cRow = CVPixelBufferGetBytesPerRowOfPlane(buffer, 1)
        let near = { (value: UInt8, expected: Double) in abs(Double(value) - expected) <= 4 }
        for x in [0, 32, 64, 96, 127] {
            guard near(y.load(fromByteOffset: 64 * yRow + x, as: UInt8.self), 16 + Double(x) * 219 / 127) else { return false }
        }
        for x in [0, 63, 127] {
            guard near(c.load(fromByteOffset: 32 * cRow + x, as: UInt8.self), 128) else { return false }
        }
        return true
    }

    /// Image de test : 128 × 128 (le plus petit que PyroWave accepte), 4:2:0,
    /// rampe de Y de 16 à 235, Cb et Cr à 128 ; encodée par l'encodeur Metal
    /// du port (tests/pyrowave, outil `encode`), 1 812 octets.
    private static let sample = Data(base64Encoded: "f8AfkAsAAAABABUQgAAAAA8AWAPPz8zM/PwD8A8wA8/PzMz8/APwDzAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA/PwABABEQgAEAAA8AVsDA8wDwDPPwA8DA8wDwDPPwAwAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAADAwAAEABRCABAAADwBw////////AAAAAQAFEIAIAAAPAHD///////8AAAABABMQgAwAAFX/VQMAAAwDMwMAAAwDMwMAAAwDMwMAAAwDM8DA8ADwADD8wMDwAPAAMPzAwPAA8AAw/MDA8ADwADD8MAzDDMMwAAAAMwAjEHgVAAD/VQD//1UA/1BTUFMDA8MDA8MDA8MDA8MzMzMzAAwDAAwDAAwDAAwDwMDwwAAAwMDwwAAAwMDwwAAAwMDwwAAAAwPDAwPDAwPDAwPDMzMzMwAMAwAMAwAMAwAMA8DA8MAAAMDA8MAAAMDA8MAAAMDA8MAAAAAAzMwzMzMzAADMzDMzMzP//1cQcB4AAKpVVVVVVQD/qlVVVVVVAP+qVVVVVVUA/6pVVVVVVQD/UHBwYVBwcGFQcHBhUHBwYc8zzzPPM88z//////Pz8/P8/Pz8PDw8PM/Pz8//////wMDAA8DAwAPAwMADwMDAA88zzzPPM88z//////Pz8/P8/Pz8PDw8PM/Pz8//////wMDAA8DAwAPAwMADwMDAA88zzzPPM88z//////Pz8/P8/Pz8PDw8PM/Pz8//////wMDAA8DAwAPAwMADwMDAA88zzzPPM88z//////Pz8/P8/Pz8PDw8PM/Pz8//////wMDAA8DAwAPAwMADwMDAAwAAAAD8/Pz8wzAMMAzD//8AAAD/////MzMAAAAA/Pz8/MMwDDAMw///AAAA/////zMzAAAAAPz8/PzDMAwwDMP//wAAAP////8zMwAAAAD8/Pz8wzAMMAzD//8AAAD/////MzP//z8QaCcAAFVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVQEBAQEBAQEBAQEBAQEBAQMzMzMzMzMzMPDw8PDMzMzPz8/PzzMzMzMzMzMw8PDw8zMzMzMzMzMw8PDw8MzMzM/Pz8/PMzMzMzMzMzDw8PDzMzMzMzMzMzDw8PDwzMzMz8/Pz88zMzMzMzMzMPDw8PMzMzMzMzMzMPDw8PDMzMzPz8/PzzMzMzMzMzMw8PDw8AAD//zMzAADP8zwAAP//MzMAAP//MzMAAM/zPAAA//8zMwAA//8zMwAAz/M8AAD//zMzAAD//zMzAADP8zwAAP//MzP//0QQaCgAAFVVVVVVVVWqVVVVVVVVVapVVVVVVVVVqlVVVVVVVVWqUFBAYFBQQGBQUEBgUFBAYDMzMzPz8/PzzMzMzMzMzMwzMzMzMzMzM/Pz8/PAzMDMwMzAzDMzMzPz8/PzzMzMzMzMzMwzMzMzMzMzM/Pz8/PAzMDMwMzAzDMzMzPz8/PzzMzMzMzMzMwzMzMzMzMzM/Pz8/PAzMDMwMzAzDMzMzPz8/PzzMzMzMzMzMwzMzMzMzMzM/Pz8/PAzMDMwMzAzMzMz/M8AAD//wAAzMzP8zwAAMzMz/M8AAD//wAAzMzP8zwAAMzMz/M8AAD//wAAzMzP8zwAAMzMz/M8AAD//wAAzMzP8zwAAP//PxBoKQAAVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVAQEBAQEBAQEBAQEBAQEBAzMzMzMzMzMw8PDw8MzMzM/Pz8/PMzMzMzMzMzDw8PDzMzMzMzMzMzDw8PDwzMzMz8/Pz88zMzMzMzMzMPDw8PMzMzMzMzMzMPDw8PDMzMzPz8/PzzMzMzMzMzMw8PDw8zMzMzMzMzMw8PDw8MzMzM/Pz8/PMzMzMzMzMzDw8PDwAAP//MzMAAM/zPAAA//8zMwAA//8zMwAAz/M8AAD//zMzAAD//zMzAADP8zwAAP//MzMAAP//MzMAAM/zPAAA//8zM///RBBoKgAAVVVVVVVVVapVVVVVVVVVqlVVVVVVVVWqVVVVVVVVVapQUEBgUFBAYFBQQGBQUEBgMzMzM/Pz8/PMzMzMzMzMzDMzMzMzMzMz8/Pz88DMwMzAzMDMMzMzM/Pz8/PMzMzMzMzMzDMzMzMzMzMz8/Pz88DMwMzAzMDMMzMzM/Pz8/PMzMzMzMzMzDMzMzMzMzMz8/Pz88DMwMzAzMDMMzMzM/Pz8/PMzMzMzMzMzDMzMzMzMzMz8/Pz88DMwMzAzMDMzMzP8zwAAP//AADMzM/zPAAAzMzP8zwAAP//AADMzM/zPAAAzMzP8zwAAP//AADMzM/zPAAAzMzP8zwAAP//AADMzM/zPAAA")!

    // MARK: - Décodeur

    /// À appeler verrou pris.
    private func apply(_ new: PyroWaveConfig) {
        // Même image, d'autres métadonnées HDR10 : le décodeur reste.
        if let config, decoder != nil, config.sameImage(new) {
            if new != config {
                self.config = new
                attachments = new.attachments
            }
            return
        }
        teardown()
        config = new
        guard let gpu else {
            unavailable("aucun GPU qui décode PyroWave")
            return
        }
        var info = pyrowave_decoder_create_info()
        info.device = gpu.handle
        info.width = Int32(new.width)
        info.height = Int32(new.height)
        info.chroma = new.chroma444 ? PYROWAVE_CHROMA_SUBSAMPLING_444 : PYROWAVE_CHROMA_SUBSAMPLING_420
        var handle: pyrowave_decoder?
        let result = pyrowave_decoder_create(&info, &handle)
        guard result == PYROWAVE_SUCCESS, let handle else {
            KyberLog.video.error("PyroWave : aucun décodeur pour \(new.label, privacy: .public) (\(String(cString: pyrowave_result_to_string(result)), privacy: .public))")
            unavailable("décodeur refusé")
            return
        }
        let desc = MTLTextureDescriptor.texture2DDescriptor(pixelFormat: .r16Unorm, width: new.chromaWidth, height: new.chromaHeight, mipmapped: false)
        desc.usage = [.shaderRead, .shaderWrite]
        desc.storageMode = .private
        let attributes: [CFString: Any] = [
            kCVPixelBufferPixelFormatTypeKey: new.pixelFormat,
            kCVPixelBufferWidthKey: new.width,
            kCVPixelBufferHeightKey: new.height,
            kCVPixelBufferMetalCompatibilityKey: true,
            kCVPixelBufferIOSurfacePropertiesKey: [CFString: Any](),
        ]
        var pool: CVPixelBufferPool?
        guard let cb = gpu.device.makeTexture(descriptor: desc), let cr = gpu.device.makeTexture(descriptor: desc),
              CVPixelBufferPoolCreate(kCFAllocatorDefault, nil, attributes as CFDictionary, &pool) == kCVReturnSuccess, let pool
        else {
            KyberLog.video.error("PyroWave : mémoire refusée pour \(new.label, privacy: .public)")
            pyrowave_decoder_destroy(handle)
            unavailable("mémoire refusée")
            return
        }
        decoder = handle
        chroma = (cb, cr)
        self.pool = pool
        attachments = new.attachments
        KyberLog.video.notice("décodeur PyroWave \(new.label, privacy: .public)")
    }

    /// À appeler verrou pris : PyroWave ne s'ouvre pas ici, dit une fois.
    private func unavailable(_ why: String) {
        guard !unavailableTold else { return }
        unavailableTold = true
        KyberLog.video.error("PyroWave indisponible sur cet appareil : \(why, privacy: .public)")
        onUnavailable?()
    }

    /// À appeler verrou pris : attend le GPU, libère le décodeur.
    private func teardown() {
        if decoder != nil { flush() }
        if let decoder { pyrowave_decoder_destroy(decoder) }
        decoder = nil
        pool = nil
        chroma = nil
        planes.removeAll()
    }

    /// Plans Y et CbCr de l'IOSurface d'une image du pool, en textures où le
    /// GPU écrit (créées une fois par IOSurface).
    private func planes(of buffer: CVPixelBuffer, config: PyroWaveConfig, device: MTLDevice) -> (y: MTLTexture, cbcr: MTLTexture)? {
        guard let surface = CVPixelBufferGetIOSurface(buffer)?.takeUnretainedValue() else { return nil }
        let id = IOSurfaceGetID(surface)
        if let known = planes[id] { return known }
        let (luma, pair): (MTLPixelFormat, MTLPixelFormat) = config.hdr ? (.r16Unorm, .rg16Unorm) : (.r8Unorm, .rg8Unorm)
        let yd = MTLTextureDescriptor.texture2DDescriptor(pixelFormat: luma, width: config.width, height: config.height, mipmapped: false)
        let cd = MTLTextureDescriptor.texture2DDescriptor(pixelFormat: pair, width: config.chromaWidth, height: config.chromaHeight, mipmapped: false)
        for d in [yd, cd] {
            d.usage = [.shaderRead, .shaderWrite]
            d.storageMode = .shared
        }
        guard let y = device.makeTexture(descriptor: yd, iosurface: surface, plane: 0),
              let cbcr = device.makeTexture(descriptor: cd, iosurface: surface, plane: 1)
        else { return nil }
        // Le pool garde ses IOSurface : quelques entrées au plus.
        if planes.count > 16 { planes.removeAll() }
        planes[id] = (y, cbcr)
        return (y, cbcr)
    }

    /// Compte un incident (sous `out`) ; journal à la première et toutes les 120.
    private func note(_ counter: ReferenceWritableKeyPath<PyroWaveDecoder, Int>, _ what: String) {
        out.lock()
        self[keyPath: counter] += 1
        let n = self[keyPath: counter]
        out.unlock()
        if n == 1 || n % 120 == 0 {
            KyberLog.video.notice("PyroWave : \(what, privacy: .public) (\(n) au total)")
        }
    }
}
