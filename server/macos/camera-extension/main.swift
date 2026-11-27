// Extension caméra de KyberNow Station (docs/webcam.md, « Station Mac ») :
// le périphérique « KyberNow Camera » que voient FaceTime, Zoom, Teams,
// Discord, OBS et les navigateurs.
//
// Deux flux : celui que lisent les apps (source, NV12 1280 × 720 ou
// 1920 × 1080 à 30 images par seconde) et une entrée (sink) où
// kybernow-camera (lancé par kycontroller pendant que l'appareil envoie)
// pose ses images. La dernière image reçue repart 30 fois par seconde,
// agrandie ou réduite au format choisi par l'app ; sans image depuis 2 s,
// du gris foncé.
//
// Écrit d'après la documentation de CMIOExtension (macOS 12.3 et après).

import CoreMediaIO
import CoreVideo
import Foundation
import Security
import VideoToolbox
import os.log

let log = Logger(subsystem: "fr.leslignesclaires.kybernow.station.camera", category: "camera")
let cameraName = "KyberNow Camera"
let fps: Int32 = 30
let sizes: [(Int32, Int32)] = [(1280, 720), (1920, 1080)]
let staleAfter: Double = 2

/// Le seul programme admis sur l'entrée de la caméra (STMAC-03).
let sinkSigningID = "fr.leslignesclaires.kybernow.station.kybernow-camera"
let sinkRequirement = "anchor apple generic and identifier \"\(sinkSigningID)\" and certificate leaf[subject.OU] = \"P6DB682S4K\""

/// Identifiant de signature attendu, puis la signature du processus vérifiée
/// (un identifiant seul se choisit librement en signant soi-même).
func sinkClientAllowed(signingID: String?, pid: pid_t) -> Bool {
    guard signingID == sinkSigningID, pid > 0 else { return false }
    var code: SecCode?
    let attributes = [kSecGuestAttributePid: NSNumber(value: pid)] as CFDictionary
    guard SecCodeCopyGuestWithAttributes(nil, attributes, [], &code) == errSecSuccess, let code else { return false }
    var requirement: SecRequirement?
    guard SecRequirementCreateWithString(sinkRequirement as CFString, [], &requirement) == errSecSuccess, let requirement else { return false }
    return SecCodeCheckValidity(code, [], requirement) == errSecSuccess
}

/// Une image NV12 gris foncé (Y 40, chrominance neutre).
func greyBuffer(width: Int, height: Int) -> CVPixelBuffer? {
    var pb: CVPixelBuffer?
    let attrs: [String: Any] = [kCVPixelBufferIOSurfacePropertiesKey as String: [:]]
    guard CVPixelBufferCreate(nil, width, height, kCVPixelFormatType_420YpCbCr8BiPlanarVideoRange,
                              attrs as CFDictionary, &pb) == kCVReturnSuccess, let pb else { return nil }
    CVPixelBufferLockBaseAddress(pb, [])
    for plane in 0..<2 {
        if let base = CVPixelBufferGetBaseAddressOfPlane(pb, plane) {
            let rows = CVPixelBufferGetHeightOfPlane(pb, plane)
            memset(base, plane == 0 ? 40 : 128, CVPixelBufferGetBytesPerRowOfPlane(pb, plane) * rows)
        }
    }
    CVPixelBufferUnlockBaseAddress(pb, [])
    return pb
}

final class ProviderSource: NSObject, CMIOExtensionProviderSource {
    private(set) var provider: CMIOExtensionProvider!
    private var deviceSource: DeviceSource!

    init(clientQueue: DispatchQueue?) {
        super.init()
        provider = CMIOExtensionProvider(source: self, clientQueue: clientQueue)
        deviceSource = DeviceSource()
        do {
            try provider.addDevice(deviceSource.device)
        } catch {
            log.error("device not added: \(error.localizedDescription, privacy: .public)")
        }
    }

    func connect(to client: CMIOExtensionClient) throws {}
    func disconnect(from client: CMIOExtensionClient) {}

    var availableProperties: Set<CMIOExtensionProperty> { [.providerManufacturer] }

    func providerProperties(forProperties properties: Set<CMIOExtensionProperty>) throws -> CMIOExtensionProviderProperties {
        let p = CMIOExtensionProviderProperties(dictionary: [:])
        if properties.contains(.providerManufacturer) { p.manufacturer = "KyberNow" }
        return p
    }

    func setProviderProperties(_ providerProperties: CMIOExtensionProviderProperties) throws {}
}

final class DeviceSource: NSObject, CMIOExtensionDeviceSource {
    private(set) var device: CMIOExtensionDevice!
    private var source: StreamSource!
    private var sink: SinkSource!
    private let queue = DispatchQueue(label: "fr.leslignesclaires.kybernow.station.camera.frames")
    private var timer: DispatchSourceTimer?
    private var transfer: VTPixelTransferSession?
    private var pool: CVPixelBufferPool?
    private var poolSize = (0, 0)
    private var formats: [CMVideoFormatDescription] = []

    // Ce que l'outil a posé en dernier (fil `queue`).
    private var latest: CVPixelBuffer?
    private var latestAt: Double = 0
    /// Image grise gardée pour sa taille : sans flux de l'appareil, elle
    /// repart 30 fois par seconde sans être refaite (STMAC-20).
    private var grey: CVPixelBuffer?

    override init() {
        super.init()
        device = CMIOExtensionDevice(localizedName: cameraName,
                                     deviceID: UUID(uuidString: "349E693B-7096-4DCC-85F4-1A09FBB34353")!,
                                     legacyDeviceID: nil, source: self)
        var streamFormats: [CMIOExtensionStreamFormat] = []
        for (w, h) in sizes {
            var fd: CMFormatDescription?
            CMVideoFormatDescriptionCreate(allocator: kCFAllocatorDefault,
                                           codecType: kCVPixelFormatType_420YpCbCr8BiPlanarVideoRange,
                                           width: w, height: h, extensions: nil, formatDescriptionOut: &fd)
            guard let fd else { continue }
            formats.append(fd)
            let frame = CMTime(value: 1, timescale: fps)
            streamFormats.append(CMIOExtensionStreamFormat(formatDescription: fd, maxFrameDuration: frame,
                                                           minFrameDuration: frame, validFrameDurations: nil))
        }
        source = StreamSource(device: self, formats: streamFormats,
                              streamID: UUID(uuidString: "C209E3AD-EBB7-4AC9-9B34-E2A4166D8431")!,
                              direction: .source, name: "\(cameraName).Video")
        sink = SinkSource(device: self, formats: streamFormats,
                          streamID: UUID(uuidString: "295199ED-E9EF-4115-B44B-3C38CB762B91")!,
                          direction: .sink, name: "\(cameraName).Sink")
        do {
            try device.addStream(source.stream)
            try device.addStream(sink.stream)
        } catch {
            log.error("streams not added: \(error.localizedDescription, privacy: .public)")
        }
        VTPixelTransferSessionCreate(allocator: nil, pixelTransferSessionOut: &transfer)
    }

    var availableProperties: Set<CMIOExtensionProperty> { [.deviceTransportType, .deviceModel] }

    func deviceProperties(forProperties properties: Set<CMIOExtensionProperty>) throws -> CMIOExtensionDeviceProperties {
        let p = CMIOExtensionDeviceProperties(dictionary: [:])
        if properties.contains(.deviceTransportType) { p.transportType = 0x7669_7274 /* « virt », kIOAudioDeviceTransportTypeVirtual */ }
        if properties.contains(.deviceModel) { p.model = cameraName }
        return p
    }

    func setDeviceProperties(_ deviceProperties: CMIOExtensionDeviceProperties) throws {}

    // Le flux des apps : 30 images par seconde tant qu'une app filme.
    func startSourceStreaming() {
        queue.async { [self] in
            guard self.timer == nil else { return }
            let t = DispatchSource.makeTimerSource(queue: self.queue)
            t.schedule(deadline: .now(), repeating: .nanoseconds(Int(1_000_000_000 / Int64(fps))), leeway: .milliseconds(2))
            t.setEventHandler { [weak self] in self?.tick() }
            self.timer = t
            t.resume()
        }
    }

    func stopSourceStreaming() {
        queue.async {
            self.timer?.cancel()
            self.timer = nil
        }
    }

    // L'entrée : une image de l'outil.
    func received(_ buffer: CMSampleBuffer) {
        guard let pb = CMSampleBufferGetImageBuffer(buffer) else { return }
        queue.async {
            self.latest = pb
            self.latestAt = CFAbsoluteTimeGetCurrent()
        }
    }

    private func outputBuffer(width: Int, height: Int) -> CVPixelBuffer? {
        if pool == nil || poolSize != (width, height) {
            let attrs: [String: Any] = [
                kCVPixelBufferPixelFormatTypeKey as String: kCVPixelFormatType_420YpCbCr8BiPlanarVideoRange,
                kCVPixelBufferWidthKey as String: width,
                kCVPixelBufferHeightKey as String: height,
                kCVPixelBufferIOSurfacePropertiesKey as String: [:],
            ]
            pool = nil
            CVPixelBufferPoolCreate(nil, nil, attrs as CFDictionary, &pool)
            poolSize = (width, height)
        }
        var pb: CVPixelBuffer?
        guard let pool, CVPixelBufferPoolCreatePixelBuffer(nil, pool, &pb) == kCVReturnSuccess else { return nil }
        return pb
    }

    private func tick() {
        let index = min(max(source.activeFormatIndex, 0), formats.count - 1)
        let dims = CMVideoFormatDescriptionGetDimensions(formats[index])
        let w = Int(dims.width), h = Int(dims.height)
        var frame: CVPixelBuffer?
        if let latest, CFAbsoluteTimeGetCurrent() - latestAt < staleAfter {
            if CVPixelBufferGetWidth(latest) == w && CVPixelBufferGetHeight(latest) == h {
                frame = latest
            } else if let out = outputBuffer(width: w, height: h), let transfer,
                      VTPixelTransferSessionTransferImage(transfer, from: latest, to: out) == noErr {
                frame = out
            }
        }
        if frame == nil {
            if grey.map({ CVPixelBufferGetWidth($0) != w || CVPixelBufferGetHeight($0) != h }) ?? true {
                grey = greyBuffer(width: w, height: h)
            }
            frame = grey
        }
        guard let frame else { return }

        var fd: CMFormatDescription?
        CMVideoFormatDescriptionCreateForImageBuffer(allocator: kCFAllocatorDefault, imageBuffer: frame, formatDescriptionOut: &fd)
        guard let fd else { return }
        let now = CMClockGetTime(CMClockGetHostTimeClock())
        var timing = CMSampleTimingInfo(duration: CMTime(value: 1, timescale: fps), presentationTimeStamp: now,
                                        decodeTimeStamp: .invalid)
        var sample: CMSampleBuffer?
        guard CMSampleBufferCreateReadyWithImageBuffer(allocator: kCFAllocatorDefault, imageBuffer: frame,
                                                       formatDescription: fd, sampleTiming: &timing,
                                                       sampleBufferOut: &sample) == noErr, let sample else { return }
        source.stream.send(sample, discontinuity: [], hostTimeInNanoseconds: UInt64(now.seconds * 1_000_000_000))
    }
}

final class StreamSource: NSObject, CMIOExtensionStreamSource {
    private(set) var stream: CMIOExtensionStream!
    private unowned let device: DeviceSource
    private let streamFormats: [CMIOExtensionStreamFormat]
    private let direction: CMIOExtensionStream.Direction
    var activeFormatIndex: Int = 0
    private var client: CMIOExtensionClient?
    private var consuming = false

    init(device: DeviceSource, formats: [CMIOExtensionStreamFormat], streamID: UUID,
         direction: CMIOExtensionStream.Direction, name: String) {
        self.device = device
        self.streamFormats = formats
        self.direction = direction
        super.init()
        stream = CMIOExtensionStream(localizedName: name, streamID: streamID, direction: direction,
                                     clockType: .hostTime, source: self)
    }

    var formats: [CMIOExtensionStreamFormat] { streamFormats }

    var availableProperties: Set<CMIOExtensionProperty> {
        var p: Set<CMIOExtensionProperty> = [.streamActiveFormatIndex, .streamFrameDuration]
        if direction == .sink {
            p.formUnion([.streamSinkBufferQueueSize, .streamSinkBuffersRequiredForStartup,
                         .streamSinkBufferUnderrunCount, .streamSinkEndOfData])
        }
        return p
    }

    func streamProperties(forProperties properties: Set<CMIOExtensionProperty>) throws -> CMIOExtensionStreamProperties {
        let p = CMIOExtensionStreamProperties(dictionary: [:])
        if properties.contains(.streamActiveFormatIndex) { p.activeFormatIndex = activeFormatIndex }
        if properties.contains(.streamFrameDuration) { p.frameDuration = CMTime(value: 1, timescale: fps) }
        if properties.contains(.streamSinkBufferQueueSize) { p.sinkBufferQueueSize = 4 }
        if properties.contains(.streamSinkBuffersRequiredForStartup) { p.sinkBuffersRequiredForStartup = 1 }
        return p
    }

    func setStreamProperties(_ streamProperties: CMIOExtensionStreamProperties) throws {
        if let index = streamProperties.activeFormatIndex, index >= 0, index < streamFormats.count {
            activeFormatIndex = index
        }
    }

    func authorizedToStartStream(for client: CMIOExtensionClient) -> Bool {
        // Les apps (FaceTime, Zoom…) lisent la source librement. L'entrée,
        // elle, n'accepte que kybernow-camera de la Station, signé par notre
        // équipe : sinon n'importe quelle app y pousserait ses images, que
        // les autres présenteraient comme la caméra de l'appareil (STMAC-03).
        guard direction == .sink else { return true }
        guard sinkClientAllowed(signingID: client.signingID, pid: client.pid) else {
            log.error("entrée refusée : \(client.signingID ?? "?", privacy: .public) (pid \(client.pid))")
            return false
        }
        self.client = client
        return true
    }

    func startStream() throws {
        if direction == .source {
            device.startSourceStreaming()
        } else {
            consuming = true
            consume()
        }
    }

    func stopStream() throws {
        if direction == .source {
            device.stopSourceStreaming()
        } else {
            consuming = false
        }
    }

    // L'entrée : chaque image de l'outil, l'une après l'autre. Sans image en
    // attente, nouvel essai 5 ms plus tard : CoreMediaIO ne prévient pas
    // l'extension qu'une image arrive sur l'entrée (pas de sémaphore possible,
    // STMAC-20) ; l'appel est léger et l'image attend au plus 5 ms.
    private func consume() {
        guard consuming, let client else { return }
        stream.consumeSampleBuffer(from: client) { [weak self] buffer, sequence, _, hasMore, error in
            guard let self else { return }
            if let buffer {
                self.device.received(buffer)
                let output = CMIOExtensionScheduledOutput(sequenceNumber: sequence,
                                                          hostTimeInNanoseconds: UInt64(CMClockGetTime(CMClockGetHostTimeClock()).seconds * 1_000_000_000))
                self.stream.notifyScheduledOutputChanged(output)
            }
            if error != nil || !hasMore {
                DispatchQueue.global().asyncAfter(deadline: .now() + .milliseconds(5)) { self.consume() }
            } else {
                self.consume()
            }
        }
    }
}

typealias SinkSource = StreamSource

let providerSource = ProviderSource(clientQueue: nil)
CMIOExtensionProvider.startService(provider: providerSource.provider)
CFRunLoopRun()
