// Son des apps Apple sur le cœur Rust (docs/coeur-apple.md) : Opus, le
// tampon adaptatif (marge qui grandit après un trou et redescend sans trou),
// le masquage des pertes, le rattrapage de la dérive et le mixage
// descendant sont dans `kybernow_core::audio`, par l'API C de apps/apple.
// Ici, la sortie seulement : l'unité Core Audio de la carte choisie par le
// système, dont le fil temps réel tire les échantillons du moteur
// (`kn_audio_fill`), suivie quand elle change (AirPods branchés : voies,
// tampon et latence relus, unité recréée).
//
// Mac (DefaultOutput), Apple TV, iPhone et iPad (RemoteIO).

import AudioToolbox
import AVFoundation
import Foundation
@_implementationOnly import CKyberNow

/// Mesures du son, une fois par seconde.
public struct AudioStats: Sendable {
    public var channels = 0
    /// Tampon moyen avant la carte son, en millisecondes.
    public var bufferMs: Double = 0
    /// Latence de la sortie (carte son, Bluetooth…), en millisecondes.
    public var outputMs: Double = 0
    /// Trous comblés (pertes réseau ou tampon vide) dans la seconde.
    public var gaps = 0
    /// Voies envoyées à la sortie (moins que le flux : mixage descendant).
    public var outputChannels = 0

    public init(channels: Int = 0, bufferMs: Double = 0, outputMs: Double = 0, gaps: Int = 0, outputChannels: Int = 0) {
        (self.channels, self.bufferMs, self.outputMs, self.gaps, self.outputChannels) = (channels, bufferMs, outputMs, gaps, outputChannels)
    }
}

final class AudioPlayer: @unchecked Sendable {
    static let sampleRate = 48_000

    var onStats: ((AudioStats) -> Void)?
    var volume: Float = 1 { didSet { kn_audio_set_volume(engine, volume) } }

    /// Le moteur du cœur ; la sortie déclare ses voies dans l'ordre d'Opus
    /// (étiquettes de `setLayout`), le moteur n'a rien à réordonner.
    private let engine: OpaquePointer
    /// Le moteur, pour que la session le suive (son coupé : `kn_session_attach_audio`).
    var core: OpaquePointer { engine }
    private var unit: AudioUnit?
    /// Voies du flux pour lesquelles la sortie est ouverte.
    private var channels = 0
    #if os(macOS)
    private var deviceListener: ((UInt32, UnsafePointer<AudioObjectPropertyAddress>) -> Void)?
    #else
    /// iPhone, iPad, Apple TV : interruption, route, services audio (`outputQueue`).
    private var sessionObservers: [NSObjectProtocol] = []
    #endif
    /// La sortie démarre hors du fil réseau de Kyber : démarrer Core Audio
    /// prend jusqu'à 100 ms, et ce fil reçoit aussi l'image.
    private let outputQueue = DispatchQueue(label: "fr.leslignesclaires.kybernow.audio-output", qos: .userInteractive)
    private let lock = NSLock()
    private var stopped = false

    init() {
        engine = kn_audio_new(UInt32(KN_AUDIO_ORDER_OPUS))
    }

    deinit {
        stop()
        kn_audio_free(engine)
    }

    // MARK: - Paquets (fil réseau)

    func configure(opusHead: UnsafeBufferPointer<UInt8>) {
        kn_audio_configure(engine, opusHead.baseAddress, UInt(opusHead.count))
        let channels = Int(kn_audio_channels(engine))
        lock.lock()
        let changed = channels > 0 && channels != self.channels && !stopped
        if changed { self.channels = channels }
        lock.unlock()
        guard changed else { return }
        outputQueue.async { [self] in
            stopOutput()
            startOutput(channels: channels)
            KyberLog.audio.notice("son : Opus (cœur), \(channels) voies")
            watchOutputDevice()
        }
    }

    func decode(_ data: UnsafeBufferPointer<UInt8>) {
        kn_audio_decode(engine, data.baseAddress, UInt(data.count))
    }

    /// Données perdues : Opus invente un raccord plausible au lieu d'un silence sec.
    func conceal(samples: Int) {
        kn_audio_conceal(engine, UInt32(clamping: samples))
    }

    func stop() {
        lock.lock()
        stopped = true
        channels = 0
        lock.unlock()
        kn_audio_stop(engine)
        outputQueue.sync {
            unwatchOutputDevice()
            stopOutput()
        }
    }

    // MARK: - Sortie (Core Audio)

    private func stopOutput() {
        guard let unit else { return }
        AudioOutputUnitStop(unit)
        AudioUnitUninitialize(unit)
        AudioComponentInstanceDispose(unit)
        self.unit = nil
    }

    private func startOutput(channels: Int) {
        var desc = AudioComponentDescription(
            componentType: kAudioUnitType_Output,
            componentSubType: Self.outputSubType,
            componentManufacturer: kAudioUnitManufacturer_Apple,
            componentFlags: 0, componentFlagsMask: 0
        )
        guard let component = AudioComponentFindNext(nil, &desc) else { return }
        var unit: AudioUnit?
        guard AudioComponentInstanceNew(component, &unit) == noErr, let unit else { return }
        // Moins de voies à la sortie (casque, AirPods) que dans le flux : le moteur mixe vers le bas.
        let deviceChannels = Self.outputDeviceChannels(unit)
        let outChannels = deviceChannels > 0 ? min(channels, deviceChannels) : channels

        #if os(iOS) || os(tvOS)
        let session = AVAudioSession.sharedInstance()
        #if os(iOS)
        // Micro allumé (docs/micro.md) : la session reste en `playAndRecord`
        // jusqu'à la coupure, qui remet la catégorie d'avant.
        if !MicrophoneCapture.holdsAudioSession { try? session.setCategory(.playback, mode: .default) }
        #else
        try? session.setCategory(.playback, mode: .default)
        #endif
        try? session.setPreferredIOBufferDuration(0.005)
        try? session.setSupportsMultichannelContent(channels > 2)
        try? session.setActive(true)
        if channels > 2 {
            try? session.setPreferredOutputNumberOfChannels(min(channels, session.maximumOutputNumberOfChannels))
        }
        #endif

        var format = AudioStreamBasicDescription(
            mSampleRate: Float64(Self.sampleRate), mFormatID: kAudioFormatLinearPCM,
            mFormatFlags: kAudioFormatFlagIsFloat | kAudioFormatFlagIsPacked,
            mBytesPerPacket: UInt32(4 * outChannels), mFramesPerPacket: 1, mBytesPerFrame: UInt32(4 * outChannels),
            mChannelsPerFrame: UInt32(outChannels), mBitsPerChannel: 32, mReserved: 0
        )
        AudioUnitSetProperty(unit, kAudioUnitProperty_StreamFormat, kAudioUnitScope_Input, 0, &format, UInt32(MemoryLayout.size(ofValue: format)))
        Self.setLayout(unit, channels: outChannels)
        // Bluetooth et AirPlay demandent jusqu'à 4 096 images d'un coup.
        var maxFrames: UInt32 = 4096
        AudioUnitSetProperty(unit, kAudioUnitProperty_MaximumFramesPerSlice, kAudioUnitScope_Global, 0, &maxFrames, UInt32(MemoryLayout<UInt32>.size))

        var callback = AURenderCallbackStruct(inputProc: { refCon, _, _, _, frames, buffers in
            guard let buffers else { return noErr }
            let me = Unmanaged<AudioPlayer>.fromOpaque(refCon).takeUnretainedValue()
            me.render(frames: Int(frames), into: UnsafeMutableAudioBufferListPointer(buffers))
            return noErr
        }, inputProcRefCon: Unmanaged.passUnretained(self).toOpaque())
        AudioUnitSetProperty(unit, kAudioUnitProperty_SetRenderCallback, kAudioUnitScope_Input, 0, &callback, UInt32(MemoryLayout<AURenderCallbackStruct>.size))

        #if os(macOS)
        // Petites demandes de la carte son : 5 ms au lieu de 10 à 20 par défaut.
        var frames: UInt32 = 240
        AudioUnitSetProperty(unit, kAudioDevicePropertyBufferFrameSize, kAudioUnitScope_Global, 0, &frames, UInt32(MemoryLayout<UInt32>.size))
        #endif

        guard AudioUnitInitialize(unit) == noErr, AudioOutputUnitStart(unit) == noErr else {
            AudioComponentInstanceDispose(unit)
            KyberLog.audio.error("sortie audio impossible à démarrer")
            return
        }
        self.unit = unit
        let latency = Self.outputLatency(unit)
        kn_audio_set_output_latency_ms(engine, latency)
        KyberLog.audio.notice("son : sortie à \(outChannels) voies, latence \(String(format: "%.1f", latency), privacy: .public) ms\(outChannels != channels ? " (mixage descendant)" : "", privacy: .public)")
    }

    /// Fil temps réel de la carte son : le moteur remplit la sortie.
    private func render(frames: Int, into buffers: UnsafeMutableAudioBufferListPointer) {
        guard let buffer = buffers.first, let out = buffer.mData?.assumingMemoryBound(to: Float.self) else { return }
        let outChannels = max(1, Int(buffer.mNumberChannels))
        kn_audio_fill(engine, out, UInt(frames), UInt(outChannels))
        var s = KnAudioStats()
        if kn_audio_take_stats(engine, &s), let onStats {
            let stats = AudioStats(channels: Int(s.channels), bufferMs: s.buffer_ms, outputMs: s.output_ms, gaps: Int(s.gaps), outputChannels: Int(s.output_channels))
            DispatchQueue.main.async { onStats(stats) }
        }
    }

    #if os(macOS)
    /// La sortie par défaut change (AirPods branchés, écran HDMI) : l'unité
    /// est recréée sur la nouvelle (voies, taille de tampon), et sa latence relue.
    private func watchOutputDevice() {
        guard deviceListener == nil else { return }
        var address = AudioObjectPropertyAddress(mSelector: kAudioHardwarePropertyDefaultOutputDevice, mScope: kAudioObjectPropertyScopeGlobal, mElement: kAudioObjectPropertyElementMain)
        let listener: (UInt32, UnsafePointer<AudioObjectPropertyAddress>) -> Void = { [weak self] _, _ in
            guard let self, self.unit != nil else { return }
            self.lock.lock()
            let channels = self.channels
            self.lock.unlock()
            guard channels > 0 else { return }
            self.stopOutput()
            self.startOutput(channels: channels)
            KyberLog.audio.notice("son : sortie changée")
        }
        if AudioObjectAddPropertyListenerBlock(AudioObjectID(kAudioObjectSystemObject), &address, outputQueue, listener) == noErr {
            deviceListener = listener
        }
    }

    private func unwatchOutputDevice() {
        guard let deviceListener else { return }
        var address = AudioObjectPropertyAddress(mSelector: kAudioHardwarePropertyDefaultOutputDevice, mScope: kAudioObjectPropertyScopeGlobal, mElement: kAudioObjectPropertyElementMain)
        AudioObjectRemovePropertyListenerBlock(AudioObjectID(kAudioObjectSystemObject), &address, outputQueue, deviceListener)
        self.deviceListener = nil
    }
    #else
    /// iPhone, iPad, Apple TV : la fin d'une interruption (appel, Siri,
    /// alarme), un changement de route (AirPods, AirPlay, HDMI) ou le
    /// redémarrage des services audio relancent l'unité de sortie et relisent
    /// sa latence ; sans cela, RemoteIO restait arrêtée jusqu'à la fin de la
    /// session (MAC-16, IOS-03).
    private func watchOutputDevice() {
        guard sessionObservers.isEmpty else { return }
        let center = NotificationCenter.default
        let restart: (String) -> Void = { [weak self] why in
            self?.outputQueue.async {
                guard let self, self.unit != nil else { return }
                self.lock.lock()
                let channels = self.channels
                self.lock.unlock()
                guard channels > 0 else { return }
                self.stopOutput()
                self.startOutput(channels: channels)
                KyberLog.audio.notice("son : sortie relancée (\(why, privacy: .public))")
            }
        }
        sessionObservers = [
            center.addObserver(forName: AVAudioSession.interruptionNotification, object: nil, queue: nil) { note in
                let raw = note.userInfo?[AVAudioSessionInterruptionTypeKey] as? UInt
                if raw == AVAudioSession.InterruptionType.ended.rawValue { restart("fin d'interruption") }
            },
            center.addObserver(forName: AVAudioSession.routeChangeNotification, object: nil, queue: nil) { note in
                let raw = note.userInfo?[AVAudioSessionRouteChangeReasonKey] as? UInt
                let reason = raw.flatMap(AVAudioSession.RouteChangeReason.init(rawValue:))
                // Nos propres changements de catégorie n'y comptent pas.
                if reason == .newDeviceAvailable || reason == .oldDeviceUnavailable || reason == .override { restart("route changée") }
            },
            center.addObserver(forName: AVAudioSession.mediaServicesWereResetNotification, object: nil, queue: nil) { _ in
                restart("services audio relancés")
            },
        ]
    }

    private func unwatchOutputDevice() {
        sessionObservers.forEach(NotificationCenter.default.removeObserver)
        sessionObservers = []
    }
    #endif

    // MARK: - Réglages de la sortie

    private static var outputSubType: OSType {
        #if os(macOS)
        return kAudioUnitSubType_DefaultOutput
        #else
        return kAudioUnitSubType_RemoteIO
        #endif
    }

    /// Voies déclarées dans l'ordre d'Opus (celui de Vorbis) : G, C, D,
    /// surround G, surround D, LFE pour le 5.1 ; G, C, D, latéral G, latéral
    /// D, arrière G, arrière D, LFE pour le 7.1.
    private static func setLayout(_ unit: AudioUnit, channels: Int) {
        let labels: [AudioChannelLabel]
        switch channels {
        case 6: labels = [kAudioChannelLabel_Left, kAudioChannelLabel_Center, kAudioChannelLabel_Right,
                          kAudioChannelLabel_LeftSurround, kAudioChannelLabel_RightSurround, kAudioChannelLabel_LFEScreen]
        case 8: labels = [kAudioChannelLabel_Left, kAudioChannelLabel_Center, kAudioChannelLabel_Right,
                          kAudioChannelLabel_LeftSurround, kAudioChannelLabel_RightSurround,
                          kAudioChannelLabel_RearSurroundLeft, kAudioChannelLabel_RearSurroundRight, kAudioChannelLabel_LFEScreen]
        default: return
        }
        let size = MemoryLayout<AudioChannelLayout>.size + (labels.count - 1) * MemoryLayout<AudioChannelDescription>.stride
        let raw = UnsafeMutableRawPointer.allocate(byteCount: size, alignment: MemoryLayout<AudioChannelLayout>.alignment)
        defer { raw.deallocate() }
        raw.initializeMemory(as: UInt8.self, repeating: 0, count: size)
        let layout = raw.assumingMemoryBound(to: AudioChannelLayout.self)
        layout.pointee.mChannelLayoutTag = kAudioChannelLayoutTag_UseChannelDescriptions
        layout.pointee.mNumberChannelDescriptions = UInt32(labels.count)
        let offset = MemoryLayout<AudioChannelLayout>.offset(of: \AudioChannelLayout.mChannelDescriptions)!
        let descriptions = (raw + offset).assumingMemoryBound(to: AudioChannelDescription.self)
        for (i, label) in labels.enumerated() { descriptions[i].mChannelLabel = label }
        AudioUnitSetProperty(unit, kAudioUnitProperty_AudioChannelLayout, kAudioUnitScope_Input, 0, layout, UInt32(size))
    }

    #if os(macOS)
    private static func outputDevice(_ unit: AudioUnit) -> AudioDeviceID? {
        var device = AudioDeviceID(0)
        var size = UInt32(MemoryLayout<AudioDeviceID>.size)
        guard AudioUnitGetProperty(unit, kAudioOutputUnitProperty_CurrentDevice, kAudioUnitScope_Global, 0, &device, &size) == noErr else { return nil }
        return device
    }

    private static func deviceValue(_ device: AudioDeviceID, _ selector: AudioObjectPropertySelector) -> UInt32 {
        var address = AudioObjectPropertyAddress(mSelector: selector, mScope: kAudioObjectPropertyScopeOutput, mElement: kAudioObjectPropertyElementMain)
        var v: UInt32 = 0
        var s = UInt32(MemoryLayout<UInt32>.size)
        AudioObjectGetPropertyData(device, &address, 0, nil, &s, &v)
        return v
    }
    #endif

    /// Voies de la sortie choisie par le système (0 : inconnu).
    private static func outputDeviceChannels(_ unit: AudioUnit) -> Int {
        #if os(macOS)
        guard let device = outputDevice(unit) else { return 0 }
        var address = AudioObjectPropertyAddress(mSelector: kAudioDevicePropertyStreamConfiguration, mScope: kAudioObjectPropertyScopeOutput, mElement: kAudioObjectPropertyElementMain)
        var size: UInt32 = 0
        guard AudioObjectGetPropertyDataSize(device, &address, 0, nil, &size) == noErr, size > 0 else { return 0 }
        let raw = UnsafeMutableRawPointer.allocate(byteCount: Int(size), alignment: MemoryLayout<AudioBufferList>.alignment)
        defer { raw.deallocate() }
        guard AudioObjectGetPropertyData(device, &address, 0, nil, &size, raw) == noErr else { return 0 }
        let list = UnsafeMutableAudioBufferListPointer(raw.assumingMemoryBound(to: AudioBufferList.self))
        return list.reduce(0) { $0 + Int($1.mNumberChannels) }
        #else
        return AVAudioSession.sharedInstance().outputNumberOfChannels
        #endif
    }

    /// Latence de la sortie : tampon de la carte son plus ce qu'elle annonce (Bluetooth compris).
    private static func outputLatency(_ unit: AudioUnit) -> Double {
        #if os(macOS)
        guard let device = outputDevice(unit) else { return 0 }
        var rate = Float64(sampleRate)
        var address = AudioObjectPropertyAddress(mSelector: kAudioDevicePropertyNominalSampleRate, mScope: kAudioObjectPropertyScopeGlobal, mElement: kAudioObjectPropertyElementMain)
        var s = UInt32(MemoryLayout<Float64>.size)
        AudioObjectGetPropertyData(device, &address, 0, nil, &s, &rate)
        let frames = deviceValue(device, kAudioDevicePropertyLatency) + deviceValue(device, kAudioDevicePropertySafetyOffset) + deviceValue(device, kAudioDevicePropertyBufferFrameSize)
        return rate > 0 ? Double(frames) / rate * 1000 : 0
        #else
        return AVAudioSession.sharedInstance().outputLatency * 1000 + AVAudioSession.sharedInstance().ioBufferDuration * 1000
        #endif
    }
}

// MARK: - Micro de l'appareil (docs/micro.md)

/// L'état du micro dit par le cœur (`on_microphone`) : ce que montrent la
/// puce et la tuile « Micro ».
public enum MicState: Equatable, Sendable {
    /// Coupé : rien n'est capté.
    case off
    /// Allumé, la réponse de l'ordinateur pas encore venue (« Envoi… ») : capté.
    case waiting
    /// Allumé et pris par l'ordinateur : capté et envoyé.
    case on
    /// L'ordinateur ne le prend pas : `reason` (`not_allowed`, `no_device`,
    /// `unsupported`, `old_station`) et le texte de link-ui (`text`).
    case refused(reason: String, text: String)

    /// Lu dans le JSON du cœur (`MicState::to_json`) ; illisible : coupé.
    public init(json: String?) {
        guard let json, let data = json.data(using: .utf8),
              let j = (try? JSONSerialization.jsonObject(with: data)) as? [String: Any] else {
            self = .off
            return
        }
        switch j["state"] as? String {
        case "waiting": self = .waiting
        case "on": self = .on
        case "refused": self = .refused(reason: j["reason"] as? String ?? "unsupported", text: j["text"] as? String ?? "")
        default: self = .off
        }
    }

    /// L'app capte le micro.
    public var capturing: Bool { self == .on || self == .waiting }

    /// La raison quand l'ordinateur ne le prend pas.
    public var refusal: String? {
        if case let .refused(_, text) = self { return text }
        return nil
    }

    /// Un invité que l'hôte ne laisse pas parler (sa puce n'est pas montrée).
    public var notAllowed: Bool {
        if case .refused("not_allowed", _) = self { return true }
        return false
    }
}

#if os(macOS) || os(iOS)
/// Réglages de l'app pour le micro (Réglages de l'app › Micro), pour tous les ordinateurs.
public enum MicrophonePrefs {
    private static let deviceKey = "micDevice"
    private static let voiceKey = "micVoiceProcessing"

    /// Le micro choisi (son identifiant : UID Core Audio sur Mac, `uid` du
    /// port sur iPhone et iPad) ; nil : « Celui du système ». Un micro
    /// débranché reste choisi : celui du système en attendant.
    public static var device: String? {
        get { UserDefaults.standard.string(forKey: deviceKey) }
        set { UserDefaults.standard.set(newValue, forKey: deviceKey) }
    }

    /// « Réduire l'écho et le bruit » : le traitement voix d'Apple, allumé par défaut.
    public static var voiceProcessing: Bool {
        get { UserDefaults.standard.object(forKey: voiceKey) as? Bool ?? true }
        set { UserDefaults.standard.set(newValue, forKey: voiceKey) }
    }
}

/// Un micro de l'appareil, pour la liste des réglages.
public struct MicDevice: Identifiable, Equatable, Sendable {
    public let id: String
    public let name: String
}

/// L'autorisation du système pour le micro, demandée la première fois qu'on
/// l'allume, jamais avant.
public enum MicrophoneAccess {
    public enum Status: Sendable { case undetermined, granted, denied }

    public static var status: Status {
        switch AVCaptureDevice.authorizationStatus(for: .audio) {
        case .authorized: return .granted
        case .notDetermined: return .undetermined
        default: return .denied
        }
    }

    /// Demande l'autorisation si elle n'a jamais été demandée ; `done` sur le
    /// fil principal, vrai si le micro est permis.
    public static func request(_ done: @escaping @MainActor (Bool) -> Void) {
        switch status {
        case .granted: Task { @MainActor in done(true) }
        case .denied: Task { @MainActor in done(false) }
        case .undetermined:
            AVCaptureDevice.requestAccess(for: .audio) { granted in Task { @MainActor in done(granted) } }
        }
    }
}

/// La capture du micro pendant une session : AVAudioEngine (nœud d'entrée,
/// traitement voix d'Apple selon « Réduire l'écho et le bruit »), un nœud
/// puits (`AVAudioSinkNode`) au rythme de la carte son, sans le tampon de
/// 100 ms d'un `installTap` ; les échantillons, mono, partent au cœur depuis
/// le fil temps réel (`kn_session_push_microphone` ne bloque pas).
/// iPhone et iPad : la session audio passe en `playAndRecord` (mode
/// `voiceChat` avec le traitement voix) tant que le micro est capté, puis
/// revient à sa catégorie d'avant.
final class MicrophoneCapture {
    typealias Push = (UnsafePointer<Float>, Int, UInt32) -> Void

    /// iPhone, iPad : la capture tient la session audio (le son de la
    /// session ne la remet pas en `playback`).
    nonisolated(unsafe) static var holdsAudioSession = false

    private let engine = AVAudioEngine()
    private let push: Push
    private var sink: AVAudioSinkNode?
    private var observer: NSObjectProtocol?
    private var running = false
    #if os(iOS)
    private var previous: (category: AVAudioSession.Category, mode: AVAudioSession.Mode, options: AVAudioSession.CategoryOptions)?
    #endif

    init(push: @escaping Push) {
        self.push = push
    }

    deinit { stop() }

    /// Démarre la capture, avec le micro et le traitement voix des réglages de l'app.
    func start() throws {
        guard !running else { return }
        let device = MicrophonePrefs.device
        let voice = MicrophonePrefs.voiceProcessing
        #if os(iOS)
        let session = AVAudioSession.sharedInstance()
        if previous == nil { previous = (session.category, session.mode, session.categoryOptions) }
        let mode: AVAudioSession.Mode = voice ? .voiceChat : .default
        // Le son de l'ordinateur reste en haut-parleur ou en A2DP (AirPods en stéréo).
        let base: AVAudioSession.CategoryOptions = [.defaultToSpeaker, .allowBluetoothA2DP, .allowAirPlay]
        try session.setCategory(.playAndRecord, mode: mode, options: base)
        Self.holdsAudioSession = true
        func find() -> AVAudioSessionPortDescription? { device.flatMap { id in session.availableInputs?.first { $0.uid == id } } }
        var chosen = find()
        if device != nil, chosen == nil {
            // Un micro Bluetooth choisi (AirPods) n'apparaît qu'en mains libres ;
            // absent même ainsi (débranché) : celui du système, sans mains libres.
            try? session.setCategory(.playAndRecord, mode: mode, options: base.union(.allowBluetoothHFP))
            chosen = find()
            if chosen == nil { try? session.setCategory(.playAndRecord, mode: mode, options: base) }
        }
        try session.setActive(true)
        // Le micro choisi, s'il est branché ; sinon celui du système.
        try? session.setPreferredInput(chosen)
        #endif
        let input = engine.inputNode
        if voice {
            // Le traitement voix tient l'entrée et la sortie du moteur ensemble.
            _ = engine.outputNode
            do {
                try input.setVoiceProcessingEnabled(true)
                // Le son de l'ordinateur ne doit pas baisser pendant qu'on parle.
                input.voiceProcessingOtherAudioDuckingConfiguration = .init(enableAdvancedDucking: false, duckingLevel: .min)
            } catch {
                KyberLog.app.error("micro : traitement voix impossible (\(error.localizedDescription, privacy: .public))")
            }
        }
        #if os(macOS)
        if let device, let id = Self.deviceID(uid: device), let unit = input.audioUnit {
            var value = id
            let status = AudioUnitSetProperty(unit, kAudioOutputUnitProperty_CurrentDevice, kAudioUnitScope_Global, 0, &value, UInt32(MemoryLayout<AudioDeviceID>.size))
            if status != noErr { KyberLog.app.error("micro : choix de \(device, privacy: .public) refusé (\(status))") }
        }
        #endif
        let inFormat = input.outputFormat(forBus: 0)
        guard inFormat.sampleRate > 0, inFormat.channelCount > 0,
              let mono = AVAudioFormat(commonFormat: .pcmFormatFloat32, sampleRate: inFormat.sampleRate, channels: 1, interleaved: false) else {
            throw NSError(domain: "KyberNow.Micro", code: 1, userInfo: [NSLocalizedDescriptionKey: "pas d'entrée"])
        }
        let rate = UInt32(mono.sampleRate)
        let push = self.push
        let sink = AVAudioSinkNode { _, frames, list in
            let buffers = UnsafeMutableAudioBufferListPointer(UnsafeMutablePointer(mutating: list))
            if let first = buffers.first, let data = first.mData {
                let count = min(Int(frames), Int(first.mDataByteSize) / MemoryLayout<Float>.size)
                if count > 0 { push(data.assumingMemoryBound(to: Float.self), count, rate) }
            }
            return noErr
        }
        engine.attach(sink)
        engine.connect(input, to: sink, format: mono)
        self.sink = sink
        engine.prepare()
        try engine.start()
        running = true
        // Micro branché ou débranché, sortie changée : le moteur s'arrête ; il reprend.
        observer = NotificationCenter.default.addObserver(forName: .AVAudioEngineConfigurationChange, object: engine, queue: .main) { [weak self] _ in
            guard let self, self.running else { return }
            KyberLog.app.notice("micro : configuration changée, reprise")
            self.teardown()
            try? self.start()
        }
        KyberLog.app.notice("micro capté : \(Int(mono.sampleRate)) Hz, traitement voix \(voice ? "oui" : "non", privacy: .public), \(device ?? "celui du système", privacy: .public)")
    }

    /// Arrête la capture ; iPhone et iPad : la session audio revient à sa catégorie d'avant.
    func stop() {
        guard running || sink != nil else { return }
        teardown()
        #if os(iOS)
        Self.holdsAudioSession = false
        // Toujours la lecture du son de l'ordinateur : la catégorie d'avant
        // pouvait être `.soloAmbient` (micro allumé avant le premier paquet de
        // son), soumise au bouton silencieux et coupée au verrouillage (IOS-10).
        let back = previous.map { $0.category == .soloAmbient || $0.category == .ambient ? (AVAudioSession.Category.playback, AVAudioSession.Mode.default, AVAudioSession.CategoryOptions()) : ($0.category, $0.mode, $0.options) }
            ?? (.playback, .default, [])
        try? AVAudioSession.sharedInstance().setCategory(back.0, mode: back.1, options: back.2)
        previous = nil
        #endif
        KyberLog.app.notice("micro arrêté")
    }

    private func teardown() {
        running = false
        if let observer { NotificationCenter.default.removeObserver(observer) }
        observer = nil
        engine.stop()
        if let sink {
            engine.disconnectNodeInput(sink)
            engine.detach(sink)
        }
        sink = nil
        if engine.inputNode.isVoiceProcessingEnabled { try? engine.inputNode.setVoiceProcessingEnabled(false) }
    }

    // MARK: Les micros de l'appareil

    /// Les micros de l'appareil, par leur nom (Mac : appareils Core Audio à
    /// entrée, sans le « Micro KyberNow » d'une Station ; iPhone, iPad : les
    /// entrées de la session audio).
    static func devices() -> [MicDevice] {
        #if os(macOS)
        var address = AudioObjectPropertyAddress(mSelector: kAudioHardwarePropertyDevices, mScope: kAudioObjectPropertyScopeGlobal, mElement: kAudioObjectPropertyElementMain)
        var size: UInt32 = 0
        guard AudioObjectGetPropertyDataSize(AudioObjectID(kAudioObjectSystemObject), &address, 0, nil, &size) == noErr else { return [] }
        var ids = [AudioDeviceID](repeating: 0, count: Int(size) / MemoryLayout<AudioDeviceID>.size)
        guard AudioObjectGetPropertyData(AudioObjectID(kAudioObjectSystemObject), &address, 0, nil, &size, &ids) == noErr else { return [] }
        return ids.compactMap { id -> MicDevice? in
            guard inputChannels(id) > 0, let uid = string(id, kAudioDevicePropertyDeviceUID), let name = string(id, kAudioObjectPropertyName) else { return nil }
            // Le micro virtuel de la Station Mac n'est pas un micro de cet appareil.
            if name.localizedCaseInsensitiveContains("KyberNow") { return nil }
            return MicDevice(id: uid, name: name)
        }
        #else
        return (AVAudioSession.sharedInstance().availableInputs ?? []).map { MicDevice(id: $0.uid, name: $0.portName) }
        #endif
    }

    #if os(macOS)
    private static func inputChannels(_ id: AudioDeviceID) -> Int {
        var address = AudioObjectPropertyAddress(mSelector: kAudioDevicePropertyStreamConfiguration, mScope: kAudioDevicePropertyScopeInput, mElement: kAudioObjectPropertyElementMain)
        var size: UInt32 = 0
        guard AudioObjectGetPropertyDataSize(id, &address, 0, nil, &size) == noErr, size > 0 else { return 0 }
        let raw = UnsafeMutableRawPointer.allocate(byteCount: Int(size), alignment: MemoryLayout<AudioBufferList>.alignment)
        defer { raw.deallocate() }
        guard AudioObjectGetPropertyData(id, &address, 0, nil, &size, raw) == noErr else { return 0 }
        let list = UnsafeMutableAudioBufferListPointer(raw.assumingMemoryBound(to: AudioBufferList.self))
        return list.reduce(0) { $0 + Int($1.mNumberChannels) }
    }

    private static func string(_ id: AudioDeviceID, _ selector: AudioObjectPropertySelector) -> String? {
        var address = AudioObjectPropertyAddress(mSelector: selector, mScope: kAudioObjectPropertyScopeGlobal, mElement: kAudioObjectPropertyElementMain)
        var value: Unmanaged<CFString>?
        var size = UInt32(MemoryLayout<Unmanaged<CFString>?>.size)
        guard AudioObjectGetPropertyData(id, &address, 0, nil, &size, &value) == noErr, let value else { return nil }
        return value.takeRetainedValue() as String
    }

    /// L'appareil Core Audio d'un UID, s'il est branché.
    static func deviceID(uid: String) -> AudioDeviceID? {
        var address = AudioObjectPropertyAddress(mSelector: kAudioHardwarePropertyTranslateUIDToDevice, mScope: kAudioObjectPropertyScopeGlobal, mElement: kAudioObjectPropertyElementMain)
        var cf = uid as CFString
        var id = AudioDeviceID(kAudioObjectUnknown)
        var size = UInt32(MemoryLayout<AudioDeviceID>.size)
        let status = withUnsafeMutablePointer(to: &cf) { ptr in
            AudioObjectGetPropertyData(AudioObjectID(kAudioObjectSystemObject), &address, UInt32(MemoryLayout<CFString>.size), ptr, &size, &id)
        }
        return status == noErr && id != kAudioObjectUnknown ? id : nil
    }
    #endif
}

/// Les micros de l'appareil, pour Réglages de l'app › Micro.
public enum MicrophoneDevices {
    public static func list() -> [MicDevice] { MicrophoneCapture.devices() }
}
#endif
