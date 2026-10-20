// Décodage matériel de l'image (VideoToolbox), sans VLC.
//
// Kyber remet les paquets tels qu'ils sortent de l'encodeur : flux Annex B
// (codes de début 00 00 01), un paquet « config » avec les jeux de
// paramètres (VPS/SPS/PPS) puis les images. On les passe à VideoToolbox en
// longueurs sur 4 octets, une image par appel, en mode temps réel : pas de
// file d'attente, pas de réordonnancement (l'encodeur n'utilise pas d'images B).
//
// Le décodage est asynchrone : `decode` rend la main au fil réseau de Kyber
// dès l'image remise à VideoToolbox (il lit aussi QUIC et livre le son),
// l'image décodée sort sur un fil de VideoToolbox. Une image plus ancienne
// que la dernière sortie est ignorée (garde de monotonie des `pts`). Une
// session perdue (veille, changement de carte graphique) est recréée tout
// de suite, sans attendre la relance du flux.
//
// Indépendant d'AppKit : sert tel quel sur iOS et tvOS.

import CoreMedia
import Foundation
import VideoToolbox

/// Une image décodée, avec ses instants (horloge `mach_absolute_time`).
public struct DecodedFrame {
    public let buffer: CVPixelBuffer
    public let pts: UInt64
    public let received: UInt64
    public let decoded: UInt64
}

final class VideoDecoder {
    enum Codec: UInt32 {
        case h264 = 0x6832_3634 // 'h264'
        case hevc = 0x6832_3635 // 'h265'
        case av1 = 0x0061_7631 // 'av1'
    }

    /// Image prête (fil du décodeur).
    var onFrame: ((DecodedFrame) -> Void)?
    /// Une image de référence manque (paquets perdus, image refusée) : il
    /// faut une image clé pour repartir (fil du décodeur).
    var onLostReference: (() -> Void)?
    /// L'image est repartie sur une image clé (émis une fois l'image clé décodée).
    var onRecovered: (() -> Void)?

    private(set) var codec: Codec = .hevc
    private var parameterSets: [UInt8: Data] = [:]
    /// AV1 : dernier en-tête de séquence reçu.
    private var av1Sequence: AV1.OBU?
    private var format: CMVideoFormatDescription?
    private var session: VTDecompressionSession?
    /// Format de sortie imposé, si celui du décodeur ne convient pas au rendu.
    private var forcedPixelFormat: OSType?
    private var waitingKey = true
    /// Une référence a été perdue en cours de route (pas au démarrage).
    private var lostReference = false
    /// Numéro de la session en cours : une erreur d'une session déjà remplacée ne la recrée pas.
    private var generation = 0
    /// Kyber livre depuis son fil réseau, le rendu peut demander un autre format, l'app arrête.
    private let lock = NSLock()
    /// Sortie du décodeur (fil de VideoToolbox) : jamais `lock`, qui peut
    /// attendre la fin des images en cours.
    private let out = NSLock()
    private var failures = 0
    /// Dernier `pts` sorti : une image plus ancienne est ignorée.
    private var lastDelivered: UInt64?
    /// Une image clé a été soumise après une perte : `onRecovered` à sa sortie.
    private var pendingRecovery = false
    /// Reprise après une erreur de sortie (prend `lock`), hors du fil de VideoToolbox.
    private let recovery = DispatchQueue(label: "fr.leslignesclaires.kybernow.decoder-recovery", qos: .userInteractive)

    deinit { reset() }

    // MARK: - Paquets

    func setCodec(_ fourcc: UInt32) {
        lock.lock()
        defer { lock.unlock() }
        guard let c = Codec(rawValue: fourcc), c != codec || session == nil else { return }
        codec = c
        parameterSets.removeAll()
        av1Sequence = nil
        reset()
        KyberLog.video.notice("codec \(String(describing: c), privacy: .public)")
    }

    func decode(_ data: UnsafeBufferPointer<UInt8>, isConfig: Bool, isKey: Bool, pts: UInt64) {
        let received = mach_absolute_time()
        lock.lock()
        defer { lock.unlock() }
        var changed = false
        // L'échantillon à décoder, construit une fois la session prête.
        let makeSample: (CMVideoFormatDescription) -> CMSampleBuffer?
        if codec == .av1 {
            // OBU : délimiteurs et bourrage retirés, en-tête de séquence gardé
            // (permis dans un échantillon, utile aux images clés).
            var frame = Data()
            var hasPicture = false
            for obu in AV1.obus(data) {
                switch obu.type {
                case AV1.obuTemporalDelimiter, AV1.obuPadding:
                    continue
                case AV1.obuSequenceHeader:
                    if av1Sequence?.bytes != obu.bytes {
                        av1Sequence = obu
                        changed = true
                    }
                default:
                    hasPicture = true
                }
                frame.append(obu.bytes)
            }
            if changed || (session == nil && isKey) { rebuild() }
            guard hasPicture else { return }
            makeSample = { Self.sample(raw: frame, format: $0) }
        } else {
            var frame: [UnsafeBufferPointer<UInt8>] = []
            for nal in Self.nalUnits(data) {
                guard let type = nalType(nal) else { continue }
                if isParameterSet(type) {
                    let d = Data(nal)
                    if parameterSets[type] != d {
                        parameterSets[type] = d
                        changed = true
                    }
                } else {
                    frame.append(nal)
                }
            }
            if changed || (session == nil && isKey) { rebuild() }
            guard !frame.isEmpty else { return }
            makeSample = { Self.sample(frame, format: $0) }
        }
        guard let session, let format else {
            // Des images sans les paramètres du flux : l'image clé du début
            // s'est perdue en route. Sans demande, l'ordinateur n'en renverrait pas.
            markLost()
            return
        }
        if waitingKey {
            // Sans image clé, une image qui s'appuie sur une référence perdue
            // est refusée : inutile de la donner au décodeur. On en demande
            // une, même sans trou signalé (image clé perdue entière, au début
            // d'un flux lourd en 4:4:4 par exemple).
            guard isKey else {
                markLost()
                return
            }
            waitingKey = false
            if lostReference {
                lostReference = false
                out.lock()
                pendingRecovery = true
                out.unlock()
            }
        }
        guard let sample = makeSample(format) else { return }
        let generation = self.generation
        let status = VTDecompressionSessionDecodeFrame(session, sampleBuffer: sample, flags: [._EnableAsynchronousDecompression], infoFlagsOut: nil) { [weak self] status, _, image, _, _ in
            guard let self else { return }
            guard status == noErr, let image else {
                self.failed(status, generation: generation)
                return
            }
            self.deliver(DecodedFrame(buffer: image, pts: pts, received: received, decoded: mach_absolute_time()))
        }
        if status == kVTInvalidSessionErr {
            // Session perdue (veille, changement de carte graphique) : on la refait.
            KyberLog.video.notice("session de décodage perdue, recréée")
            rebuild()
            markLost()
        } else if status != noErr {
            out.lock()
            failures += 1
            out.unlock()
            markLost()
        }
    }

    /// Image décodée (fil de VideoToolbox) : remise au rendu si elle est plus
    /// récente que la dernière sortie.
    private func deliver(_ frame: DecodedFrame) {
        out.lock()
        if let last = lastDelivered, frame.pts <= last {
            out.unlock()
            return
        }
        lastDelivered = frame.pts
        let recovered = pendingRecovery
        pendingRecovery = false
        out.unlock()
        onFrame?(frame)
        if recovered {
            KyberLog.video.notice("image repartie sur une image clé")
            onRecovered?()
        }
    }

    /// Erreur de sortie (fil de VideoToolbox) : comptée ici, traitée hors de ce fil.
    private func failed(_ status: OSStatus, generation: Int) {
        out.lock()
        failures += 1
        let count = failures
        out.unlock()
        if count == 1 || count % 120 == 0 {
            KyberLog.video.error("décodage : erreur \(status) (\(count) au total)")
        }
        recovery.async { [weak self] in self?.recover(after: status, generation: generation) }
    }

    /// Session morte (−12785 après la veille, −12911 après un changement de
    /// carte graphique) : recréée tout de suite (50 ms) au lieu d'attendre la
    /// relance du flux (2 à 3 s). Dans tous les cas, une image clé est demandée.
    private func recover(after status: OSStatus, generation: Int) {
        lock.lock()
        defer { lock.unlock() }
        guard session != nil else { return }
        if status == kVTInvalidSessionErr || status == kVTVideoDecoderMalfunctionErr, generation == self.generation {
            KyberLog.video.notice("session de décodage en panne (\(status)), recréée")
            rebuild()
        }
        markLost()
    }

    /// Attend les images en cours de décodage (tests).
    func flush() {
        lock.lock()
        if let session { VTDecompressionSessionWaitForAsynchronousFrames(session) }
        lock.unlock()
        recovery.sync {}
    }

    /// Données perdues (trou dans le flux) : on attend une image clé.
    func dataLost() {
        lock.lock()
        defer { lock.unlock() }
        guard session != nil else { return }
        markLost()
    }

    /// À appeler verrou pris.
    private func markLost() {
        waitingKey = true
        out.lock()
        pendingRecovery = false
        out.unlock()
        guard !lostReference else { return }
        lostReference = true
        KyberLog.video.notice("référence perdue : demande d'une image clé")
        onLostReference?()
    }

    /// Le rendu ne sait pas afficher ce format : on en demande un autre.
    func forcePixelFormat(_ format: OSType) {
        lock.lock()
        defer { lock.unlock() }
        guard forcedPixelFormat != format else { return }
        forcedPixelFormat = format
        rebuild()
    }

    func invalidate() {
        lock.lock()
        defer { lock.unlock() }
        reset()
    }

    private func reset() {
        if let session {
            VTDecompressionSessionWaitForAsynchronousFrames(session)
            VTDecompressionSessionInvalidate(session)
        }
        session = nil
        format = nil
        waitingKey = true
    }

    // MARK: - Session

    private func rebuild() {
        reset()
        generation += 1
        guard let format = makeFormat() else { return }
        let dims = CMVideoFormatDescriptionGetDimensions(format)
        guard let session = Self.makeSession(format: format, pixelFormat: forcedPixelFormat, requireHardware: true)
            ?? Self.makeSession(format: format, pixelFormat: forcedPixelFormat, requireHardware: false)
        else {
            KyberLog.video.error("aucun décodeur pour \(String(describing: self.codec), privacy: .public) \(dims.width)×\(dims.height)")
            return
        }
        self.format = format
        self.session = session
        var hardware: CFBoolean?
        #if os(macOS)
        VTSessionCopyProperty(session, key: kVTDecompressionPropertyKey_UsingHardwareAcceleratedVideoDecoder, allocator: nil, valueOut: &hardware)
        #endif
        KyberLog.video.notice("décodeur \(String(describing: self.codec), privacy: .public) \(dims.width)×\(dims.height), matériel : \(hardware.map { CFBooleanGetValue($0) } ?? true)")
    }

    static func makeSession(format: CMVideoFormatDescription, pixelFormat: OSType?, requireHardware: Bool) -> VTDecompressionSession? {
        var spec: [CFString: Any] = [:]
        #if os(macOS)
        spec[kVTVideoDecoderSpecification_EnableHardwareAcceleratedVideoDecoder] = true
        spec[kVTVideoDecoderSpecification_RequireHardwareAcceleratedVideoDecoder] = requireHardware
        #endif
        var attributes: [CFString: Any] = [
            kCVPixelBufferMetalCompatibilityKey: true,
            kCVPixelBufferIOSurfacePropertiesKey: [CFString: Any](),
        ]
        if let pixelFormat { attributes[kCVPixelBufferPixelFormatTypeKey] = pixelFormat }
        var session: VTDecompressionSession?
        let status = VTDecompressionSessionCreate(
            allocator: kCFAllocatorDefault, formatDescription: format,
            decoderSpecification: spec as CFDictionary, imageBufferAttributes: attributes as CFDictionary,
            outputCallback: nil, decompressionSessionOut: &session
        )
        guard status == noErr, let session else { return nil }
        // Temps réel : chaque image est décodée tout de suite, sans tampon.
        VTSessionSetProperty(session, key: kVTDecompressionPropertyKey_RealTime, value: kCFBooleanTrue)
        return session
    }

    private func makeFormat() -> CMVideoFormatDescription? {
        if codec == .av1 {
            guard let obu = av1Sequence, let header = AV1.parseSequenceHeader(obu.payload) else { return nil }
            return AV1.format(header, sequenceHeaderOBU: obu.bytes)
        }
        let order: [UInt8] = codec == .hevc ? [32, 33, 34] : [7, 8]
        let sets = order.compactMap { parameterSets[$0] }
        guard sets.count == order.count else { return nil }
        return Self.format(codec: codec, parameterSets: sets)
    }

    static func format(codec: Codec, parameterSets sets: [Data]) -> CMVideoFormatDescription? {
        var format: CMVideoFormatDescription?
        let status: OSStatus = withPointers(sets) { pointers, sizes in
            switch codec {
            case .hevc:
                return CMVideoFormatDescriptionCreateFromHEVCParameterSets(
                    allocator: kCFAllocatorDefault, parameterSetCount: sets.count, parameterSetPointers: pointers,
                    parameterSetSizes: sizes, nalUnitHeaderLength: 4, extensions: nil, formatDescriptionOut: &format
                )
            default:
                return CMVideoFormatDescriptionCreateFromH264ParameterSets(
                    allocator: kCFAllocatorDefault, parameterSetCount: sets.count, parameterSetPointers: pointers,
                    parameterSetSizes: sizes, nalUnitHeaderLength: 4, formatDescriptionOut: &format
                )
            }
        }
        if status != noErr { KyberLog.video.error("jeux de paramètres refusés : \(status)") }
        return format
    }

    private static func withPointers<R>(_ sets: [Data], _ body: (UnsafePointer<UnsafePointer<UInt8>>, UnsafePointer<Int>) -> R) -> R {
        let copies = sets.map { d -> UnsafeMutablePointer<UInt8> in
            let p = UnsafeMutablePointer<UInt8>.allocate(capacity: d.count)
            d.copyBytes(to: p, count: d.count)
            return p
        }
        defer { copies.forEach { $0.deallocate() } }
        let pointers = copies.map { UnsafePointer($0) }
        let sizes = sets.map(\.count)
        return pointers.withUnsafeBufferPointer { p in
            sizes.withUnsafeBufferPointer { s in body(p.baseAddress!, s.baseAddress!) }
        }
    }

    /// NAL en Annex B → un échantillon en longueurs sur 4 octets.
    /// Échantillon tel quel (AV1 : OBU avec leur taille).
    static func sample(raw: Data, format: CMVideoFormatDescription) -> CMSampleBuffer? {
        var block: CMBlockBuffer?
        guard CMBlockBufferCreateWithMemoryBlock(
            allocator: kCFAllocatorDefault, memoryBlock: nil, blockLength: raw.count, blockAllocator: kCFAllocatorDefault,
            customBlockSource: nil, offsetToData: 0, dataLength: raw.count, flags: kCMBlockBufferAssureMemoryNowFlag, blockBufferOut: &block
        ) == noErr, let block else { return nil }
        let copied = raw.withUnsafeBytes { CMBlockBufferReplaceDataBytes(with: $0.baseAddress!, blockBuffer: block, offsetIntoDestination: 0, dataLength: raw.count) }
        guard copied == noErr else { return nil }
        var sample: CMSampleBuffer?
        var size = raw.count
        guard CMSampleBufferCreateReady(
            allocator: kCFAllocatorDefault, dataBuffer: block, formatDescription: format, sampleCount: 1,
            sampleTimingEntryCount: 0, sampleTimingArray: nil, sampleSizeEntryCount: 1, sampleSizeArray: &size, sampleBufferOut: &sample
        ) == noErr else { return nil }
        return sample
    }

    static func sample(_ nals: [UnsafeBufferPointer<UInt8>], format: CMVideoFormatDescription) -> CMSampleBuffer? {
        let total = nals.reduce(0) { $0 + 4 + $1.count }
        var block: CMBlockBuffer?
        guard CMBlockBufferCreateWithMemoryBlock(
            allocator: kCFAllocatorDefault, memoryBlock: nil, blockLength: total, blockAllocator: kCFAllocatorDefault,
            customBlockSource: nil, offsetToData: 0, dataLength: total, flags: kCMBlockBufferAssureMemoryNowFlag, blockBufferOut: &block
        ) == noErr, let block else { return nil }
        var pointer: UnsafeMutablePointer<CChar>?
        guard CMBlockBufferGetDataPointer(block, atOffset: 0, lengthAtOffsetOut: nil, totalLengthOut: nil, dataPointerOut: &pointer) == noErr,
              let base = pointer.map({ UnsafeMutableRawPointer($0) })
        else { return nil }
        var offset = 0
        for nal in nals {
            var length = UInt32(nal.count).bigEndian
            withUnsafeBytes(of: &length) { base.advanced(by: offset).copyMemory(from: $0.baseAddress!, byteCount: 4) }
            base.advanced(by: offset + 4).copyMemory(from: nal.baseAddress!, byteCount: nal.count)
            offset += 4 + nal.count
        }
        var sample: CMSampleBuffer?
        var size = total
        guard CMSampleBufferCreateReady(
            allocator: kCFAllocatorDefault, dataBuffer: block, formatDescription: format, sampleCount: 1,
            sampleTimingEntryCount: 0, sampleTimingArray: nil, sampleSizeEntryCount: 1, sampleSizeArray: &size, sampleBufferOut: &sample
        ) == noErr else { return nil }
        return sample
    }

    // MARK: - Annex B

    /// Découpe un flux Annex B en NAL (sans les codes de début).
    static func nalUnits(_ data: UnsafeBufferPointer<UInt8>) -> [UnsafeBufferPointer<UInt8>] {
        var units: [UnsafeBufferPointer<UInt8>] = []
        let n = data.count
        var i = 0
        var start = -1
        while i + 2 < n {
            if data[i] == 0, data[i + 1] == 0, data[i + 2] == 1 {
                if start >= 0 {
                    var end = i
                    // Le zéro de tête d'un code sur 4 octets n'appartient pas au NAL précédent.
                    while end > start, data[end - 1] == 0 { end -= 1 }
                    units.append(UnsafeBufferPointer(rebasing: data[start..<end]))
                }
                i += 3
                start = i
            } else {
                i += 1
            }
        }
        if start >= 0, start < n { units.append(UnsafeBufferPointer(rebasing: data[start..<n])) }
        return units
    }

    private func nalType(_ nal: UnsafeBufferPointer<UInt8>) -> UInt8? {
        guard let first = nal.first else { return nil }
        return codec == .hevc ? (first >> 1) & 0x3F : first & 0x1F
    }

    private func isParameterSet(_ type: UInt8) -> Bool {
        codec == .hevc ? (32...34).contains(type) : type == 7 || type == 8
    }
}
