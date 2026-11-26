// La caméra de l'appareil vers l'ordinateur (docs/webcam.md, règles dans
// docs/link-ui.md, « Caméra de l'appareil »), commune au Mac, à l'iPhone et
// l'iPad, et à l'Apple TV (Caméra de continuité) : capture par AVFoundation,
// encodage matériel H.264 par VideoToolbox (temps réel, sans images B, une
// image clé toutes les 2 s et à la demande), puis chaque image en Annex B
// au cœur (`kn_camera_frame`), qui l'envoie à l'ordinateur ou la lâche.

import AVFoundation
import CKyberNow
import CoreMedia
import VideoToolbox

/// Une caméra de l'appareil, telle que la liste du menu la montre.
public struct CameraChoice: Identifiable, Hashable, Sendable {
    public let id: String
    /// « Caméra avant », « Caméra arrière » (iPhone, iPad) ; sinon le nom du système.
    public let name: String
}

/// L'état de la caméra dans la session (puce, tuile, voyant).
public enum CameraState: Equatable, Sendable {
    case off
    /// « Caméra… » : la caméra s'ouvre, l'ordinateur n'a pas encore répondu.
    case starting
    /// Le voyant : l'ordinateur la reçoit.
    case sending
    /// App en arrière-plan : l'ordinateur garde la dernière image.
    case paused
}

/// Pourquoi la caméra ne part pas (avis de docs/link-ui.md).
public enum CameraProblem: Error, Equatable, Sendable {
    case denied
    case inUse
    case unplugged(String)
    case noEncoder

    /// Le texte de l'avis, dans la langue de l'app.
    public var notice: String {
        let (which, name): (Int32, String) = switch self {
        case .denied: (Int32(KN_CAMERA_DENIED), "")
        case .inUse: (Int32(KN_CAMERA_IN_USE), "")
        case let .unplugged(camera): (Int32(KN_CAMERA_UNPLUGGED), camera)
        case .noEncoder: (Int32(KN_CAMERA_NO_ENCODER), "")
        }
        return Core.take(kn_camera_device_notice(which, name)) ?? ""
    }
}

public final class CameraCapture: NSObject, AVCaptureVideoDataOutputSampleBufferDelegate, @unchecked Sendable {
    /// Une image encodée : données Annex B, pts en µs, image clé. Rend
    /// `KN_CAMERA_SENT`, `KN_CAMERA_WANT_KEYFRAME` ou `KN_CAMERA_OFF`.
    var send: ((UnsafeRawBufferPointer, UInt64, Bool) -> Int32)?
    /// Le débit visé par le cœur (bit/s), relu chaque seconde.
    var targetBitrate: (() -> UInt32)?
    /// Un problème en cours de route (caméra débranchée, prise par une autre app).
    var onProblem: ((CameraProblem) -> Void)?
    /// L'app est passée en arrière-plan (iPhone, iPad) : la capture s'interrompt.
    var onInterrupted: ((Bool) -> Void)?

    private let queue = DispatchQueue(label: "fr.leslignesclaires.kybernow.camera")
    private var session: AVCaptureSession?
    private var device: AVCaptureDevice?
    private var compression: VTCompressionSession?
    private var encodedSize = (0, 0)
    private var bitrate: UInt32 = 0
    private var lastBitrateCheck: CFAbsoluteTime = 0
    private var wantKeyframe = true
    private var firstPTS: CMTime?
    private var observers: [NSObjectProtocol] = []
    #if os(iOS)
    private var rotation: AVCaptureDevice.RotationCoordinator?
    private var rotationObservation: NSKeyValueObservation?
    #endif

    // MARK: - Les caméras de l'appareil

    private static var deviceTypes: [AVCaptureDevice.DeviceType] {
        #if os(tvOS)
        return [.continuityCamera]
        #elseif os(iOS)
        return [.builtInWideAngleCamera]
        #else
        return [.builtInWideAngleCamera, .external, .continuityCamera]
        #endif
    }

    private static func devices() -> [AVCaptureDevice] {
        AVCaptureDevice.DiscoverySession(deviceTypes: deviceTypes, mediaType: .video, position: .unspecified).devices
    }

    /// Les caméras, dans l'ordre du système (iPhone et iPad : avant, puis arrière).
    public static func cameras() -> [CameraChoice] {
        var list = devices().map { d -> CameraChoice in
            #if os(iOS)
            switch d.position {
            case .front: return CameraChoice(id: d.uniqueID, name: String(localized: "Caméra avant", bundle: .kyberKit))
            case .back: return CameraChoice(id: d.uniqueID, name: String(localized: "Caméra arrière", bundle: .kyberKit))
            default: return CameraChoice(id: d.uniqueID, name: d.localizedName)
            }
            #else
            return CameraChoice(id: d.uniqueID, name: d.localizedName)
            #endif
        }
        #if os(iOS)
        let front = String(localized: "Caméra avant", bundle: .kyberKit)
        list.sort { a, b in a.name == front && b.name != front }
        #endif
        return list
    }

    /// L'appareil sait envoyer sa caméra : au moins une caméra (l'Apple TV
    /// propose d'en choisir une, par la Caméra de continuité).
    public static var available: Bool {
        #if os(tvOS)
        return true
        #else
        return !devices().isEmpty
        #endif
    }

    /// Par défaut : la caméra avant (téléphone, tablette) ou celle intégrée.
    public static func defaultCamera() -> String? {
        #if os(iOS)
        return AVCaptureDevice.default(.builtInWideAngleCamera, for: .video, position: .front)?.uniqueID
        #elseif os(macOS)
        return (AVCaptureDevice.default(.builtInWideAngleCamera, for: .video, position: .unspecified) ?? devices().first)?.uniqueID
        #else
        return devices().first?.uniqueID
        #endif
    }

    // MARK: - Départ et arrêt

    /// Ouvre la caméra `id` (nil : celle par défaut) à la définition de
    /// l'ordinateur ; rend la taille réellement capturée, ou le problème.
    func open(id: String?, resolution1080: Bool) async -> Result<(Int, Int), CameraProblem> {
        switch AVCaptureDevice.authorizationStatus(for: .video) {
        case .authorized: break
        case .notDetermined:
            guard await AVCaptureDevice.requestAccess(for: .video) else { return .failure(.denied) }
        default:
            return .failure(.denied)
        }
        let all = Self.devices()
        guard let device = all.first(where: { $0.uniqueID == id }) ?? all.first(where: { $0.uniqueID == Self.defaultCamera() }) ?? all.first else {
            return .failure(.unplugged(""))
        }
        return await withCheckedContinuation { done in
            queue.async {
                done.resume(returning: self.configure(device, resolution1080: resolution1080))
            }
        }
    }

    private func configure(_ device: AVCaptureDevice, resolution1080: Bool) -> Result<(Int, Int), CameraProblem> {
        stopOnQueue()
        let session = AVCaptureSession()
        session.beginConfiguration()
        let wanted: AVCaptureSession.Preset = resolution1080 ? .hd1920x1080 : .hd1280x720
        if session.canSetSessionPreset(wanted) {
            session.sessionPreset = wanted
        } else if session.canSetSessionPreset(.hd1280x720) {
            session.sessionPreset = .hd1280x720
        }
        guard let input = try? AVCaptureDeviceInput(device: device), session.canAddInput(input) else {
            session.commitConfiguration()
            return .failure(.inUse)
        }
        session.addInput(input)
        let output = AVCaptureVideoDataOutput()
        output.alwaysDiscardsLateVideoFrames = true
        output.videoSettings = [kCVPixelBufferPixelFormatTypeKey as String: kCVPixelFormatType_420YpCbCr8BiPlanarVideoRange]
        output.setSampleBufferDelegate(self, queue: queue)
        guard session.canAddOutput(output) else {
            session.commitConfiguration()
            return .failure(.inUse)
        }
        session.addOutput(output)
        if let connection = output.connection(with: .video) {
            // Comme une vraie webcam : pas en miroir (les apps de visio
            // retournent elles-mêmes leur aperçu).
            if connection.isVideoMirroringSupported {
                connection.automaticallyAdjustsVideoMirroring = false
                connection.isVideoMirrored = false
            }
            #if os(iOS)
            // L'image droite quelle que soit la tenue de l'appareil (portrait :
            // l'ordinateur la borde).
            let coordinator = AVCaptureDevice.RotationCoordinator(device: device, previewLayer: nil)
            rotation = coordinator
            let apply = { [weak connection] in
                guard let connection else { return }
                let angle = coordinator.videoRotationAngleForHorizonLevelCapture
                if connection.isVideoRotationAngleSupported(angle) { connection.videoRotationAngle = angle }
            }
            apply()
            rotationObservation = coordinator.observe(\.videoRotationAngleForHorizonLevelCapture) { [weak self] _, _ in
                self?.queue.async { apply() }
            }
            #endif
        }
        // 30 images par seconde quand la caméra les sait.
        if (try? device.lockForConfiguration()) != nil {
            let frame = CMTime(value: 1, timescale: 30)
            if device.activeFormat.videoSupportedFrameRateRanges.contains(where: { $0.minFrameDuration <= frame && frame <= $0.maxFrameDuration }) {
                device.activeVideoMinFrameDuration = frame
                device.activeVideoMaxFrameDuration = frame
            }
            device.unlockForConfiguration()
        }
        session.commitConfiguration()
        watch(session, device)
        self.session = session
        self.device = device
        wantKeyframe = true
        firstPTS = nil
        session.startRunning()
        let dims = CMVideoFormatDescriptionGetDimensions(device.activeFormat.formatDescription)
        let w = resolution1080 ? min(1920, Int(dims.width)) : min(1280, Int(dims.width))
        let h = resolution1080 ? min(1080, Int(dims.height)) : min(720, Int(dims.height))
        return .success((w, h))
    }

    /// Arrête la capture et l'encodeur.
    func close() {
        queue.async { self.stopOnQueue() }
    }

    /// La prochaine image sera une image clé.
    func forceKeyframe() {
        queue.async { self.wantKeyframe = true }
    }

    private func stopOnQueue() {
        for o in observers { NotificationCenter.default.removeObserver(o) }
        observers = []
        #if os(iOS)
        rotationObservation = nil
        rotation = nil
        #endif
        session?.stopRunning()
        session = nil
        device = nil
        if let c = compression {
            VTCompressionSessionCompleteFrames(c, untilPresentationTimeStamp: .invalid)
            VTCompressionSessionInvalidate(c)
        }
        compression = nil
        encodedSize = (0, 0)
    }

    private func watch(_ session: AVCaptureSession, _ device: AVCaptureDevice) {
        let center = NotificationCenter.default
        let name = device.localizedName
        observers.append(center.addObserver(forName: AVCaptureDevice.wasDisconnectedNotification, object: device, queue: nil) { [weak self] _ in
            self?.onProblem?(.unplugged(name))
        })
        #if os(iOS) || os(tvOS)
        observers.append(center.addObserver(forName: AVCaptureSession.wasInterruptedNotification, object: session, queue: nil) { [weak self] note in
            let reason = (note.userInfo?[AVCaptureSessionInterruptionReasonKey] as? NSNumber).flatMap { AVCaptureSession.InterruptionReason(rawValue: $0.intValue) }
            if reason == .videoDeviceInUseByAnotherClient {
                self?.onProblem?(.inUse)
            } else {
                self?.onInterrupted?(true)
            }
        })
        observers.append(center.addObserver(forName: AVCaptureSession.interruptionEndedNotification, object: session, queue: nil) { [weak self] _ in
            self?.forceKeyframe()
            self?.onInterrupted?(false)
        })
        #endif
        observers.append(center.addObserver(forName: AVCaptureSession.runtimeErrorNotification, object: session, queue: nil) { [weak self] _ in
            self?.onProblem?(.inUse)
        })
    }

    // MARK: - Encodage

    private func makeEncoder(width: Int, height: Int) -> Bool {
        if let c = compression {
            VTCompressionSessionInvalidate(c)
            compression = nil
        }
        var spec: [CFString: Any] = [:]
        #if os(macOS)
        spec[kVTVideoEncoderSpecification_EnableHardwareAcceleratedVideoEncoder] = true
        #endif
        var session: VTCompressionSession?
        let status = VTCompressionSessionCreate(allocator: nil, width: Int32(width), height: Int32(height),
                                                codecType: kCMVideoCodecType_H264,
                                                encoderSpecification: spec.isEmpty ? nil : spec as CFDictionary,
                                                imageBufferAttributes: nil, compressedDataAllocator: nil,
                                                outputCallback: nil, refcon: nil, compressionSessionOut: &session)
        guard status == noErr, let session else {
            KyberLog.app.error("caméra : encodeur H.264 indisponible (\(status))")
            return false
        }
        let bitrate = max(300_000, targetBitrate?() ?? 1_500_000)
        VTSessionSetProperty(session, key: kVTCompressionPropertyKey_RealTime, value: kCFBooleanTrue)
        VTSessionSetProperty(session, key: kVTCompressionPropertyKey_ProfileLevel, value: kVTProfileLevel_H264_Main_AutoLevel)
        VTSessionSetProperty(session, key: kVTCompressionPropertyKey_AllowFrameReordering, value: kCFBooleanFalse)
        VTSessionSetProperty(session, key: kVTCompressionPropertyKey_ExpectedFrameRate, value: 30 as CFNumber)
        VTSessionSetProperty(session, key: kVTCompressionPropertyKey_MaxKeyFrameInterval, value: 60 as CFNumber)
        VTSessionSetProperty(session, key: kVTCompressionPropertyKey_MaxKeyFrameIntervalDuration, value: 2 as CFNumber)
        VTSessionSetProperty(session, key: kVTCompressionPropertyKey_AverageBitRate, value: bitrate as CFNumber)
        // Pas plus de 1,5 fois le débit visé sur une seconde.
        let limits = [Int(bitrate) * 3 / 2 / 8, 1] as CFArray
        VTSessionSetProperty(session, key: kVTCompressionPropertyKey_DataRateLimits, value: limits)
        VTCompressionSessionPrepareToEncodeFrames(session)
        compression = session
        encodedSize = (width, height)
        self.bitrate = bitrate
        return true
    }

    private func updateBitrate() {
        let now = CFAbsoluteTimeGetCurrent()
        guard now - lastBitrateCheck >= 1, let compression, let target = targetBitrate?(), target > 0, target != bitrate else { return }
        lastBitrateCheck = now
        bitrate = target
        VTSessionSetProperty(compression, key: kVTCompressionPropertyKey_AverageBitRate, value: target as CFNumber)
        VTSessionSetProperty(compression, key: kVTCompressionPropertyKey_DataRateLimits, value: [Int(target) * 3 / 2 / 8, 1] as CFArray)
    }

    public func captureOutput(_ output: AVCaptureOutput, didOutput sampleBuffer: CMSampleBuffer, from connection: AVCaptureConnection) {
        guard let pixels = CMSampleBufferGetImageBuffer(sampleBuffer) else { return }
        let w = CVPixelBufferGetWidth(pixels), h = CVPixelBufferGetHeight(pixels)
        if compression == nil || encodedSize != (w, h) {
            guard makeEncoder(width: w, height: h) else {
                onProblem?(.noEncoder)
                return
            }
            wantKeyframe = true
        }
        updateBitrate()
        guard let compression else { return }
        let pts = CMSampleBufferGetPresentationTimeStamp(sampleBuffer)
        if firstPTS == nil { firstPTS = pts }
        let options: CFDictionary? = wantKeyframe ? [kVTEncodeFrameOptionKey_ForceKeyFrame: kCFBooleanTrue] as CFDictionary : nil
        wantKeyframe = false
        VTCompressionSessionEncodeFrame(compression, imageBuffer: pixels, presentationTimeStamp: pts, duration: .invalid,
                                        frameProperties: options, infoFlagsOut: nil) { [weak self] status, _, encoded in
            guard status == noErr, let encoded, let self else { return }
            self.deliver(encoded)
        }
    }

    /// AVCC (longueur sur 4 octets) vers Annex B (code de départ), avec SPS et
    /// PPS devant chaque image clé.
    private func deliver(_ sample: CMSampleBuffer) {
        guard let block = CMSampleBufferGetDataBuffer(sample) else { return }
        let attachments = CMSampleBufferGetSampleAttachmentsArray(sample, createIfNecessary: false) as? [[CFString: Any]]
        let key = !(attachments?.first?[kCMSampleAttachmentKey_NotSync] as? Bool ?? false)
        var out = Data()
        let start: [UInt8] = [0, 0, 0, 1]
        if key, let format = CMSampleBufferGetFormatDescription(sample) {
            var count = 0
            CMVideoFormatDescriptionGetH264ParameterSetAtIndex(format, parameterSetIndex: 0, parameterSetPointerOut: nil,
                                                               parameterSetSizeOut: nil, parameterSetCountOut: &count, nalUnitHeaderLengthOut: nil)
            for i in 0..<count {
                var pointer: UnsafePointer<UInt8>?
                var size = 0
                if CMVideoFormatDescriptionGetH264ParameterSetAtIndex(format, parameterSetIndex: i, parameterSetPointerOut: &pointer,
                                                                      parameterSetSizeOut: &size, parameterSetCountOut: nil,
                                                                      nalUnitHeaderLengthOut: nil) == noErr, let pointer {
                    out.append(contentsOf: start)
                    out.append(pointer, count: size)
                }
            }
        }
        var length = 0
        var pointer: UnsafeMutablePointer<CChar>?
        guard CMBlockBufferGetDataPointer(block, atOffset: 0, lengthAtOffsetOut: nil, totalLengthOut: &length, dataPointerOut: &pointer) == noErr,
              let pointer else { return }
        var offset = 0
        pointer.withMemoryRebound(to: UInt8.self, capacity: length) { bytes in
            while offset + 4 <= length {
                let n = Int(bytes[offset]) << 24 | Int(bytes[offset + 1]) << 16 | Int(bytes[offset + 2]) << 8 | Int(bytes[offset + 3])
                offset += 4
                guard n > 0, offset + n <= length else { break }
                out.append(contentsOf: start)
                out.append(bytes + offset, count: n)
                offset += n
            }
        }
        let pts = CMSampleBufferGetPresentationTimeStamp(sample)
        let since = firstPTS.map { CMTimeSubtract(pts, $0) } ?? .zero
        let us = UInt64(max(0, CMTimeGetSeconds(since)) * 1_000_000)
        let result = out.withUnsafeBytes { send?($0, us, key) ?? Int32(KN_CAMERA_OFF) }
        if result == Int32(KN_CAMERA_WANT_KEYFRAME) {
            forceKeyframe()
        }
    }
}
