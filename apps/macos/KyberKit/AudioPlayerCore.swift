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
import Foundation
#if os(iOS) || os(tvOS)
import AVFoundation
#endif
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
        try? session.setCategory(.playback, mode: .default)
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
    private func watchOutputDevice() {}
    private func unwatchOutputDevice() {}
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
