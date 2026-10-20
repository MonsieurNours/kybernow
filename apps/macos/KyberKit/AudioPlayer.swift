// Son sans VLC : paquets Opus de Kyber → libopus → Core Audio.
//
// Latence : un petit tampon entre la réception (un paquet de 20 ms à la
// fois, plus ou moins régulier) et la carte son (qui réclame quelques
// millisecondes à la fois). Sa marge se règle toute seule : elle grandit
// après un trou, puis redescend ; ce qui dépasse durablement est jeté, pour
// que le retard ne s'accumule pas (horloges du PC et du Mac légèrement
// différentes). VLC gardait 50 ms de tampon fixe, plus le sien.
//
// Indépendant d'AppKit : sert tel quel sur iOS et tvOS (RemoteIO).

import AudioToolbox
@_implementationOnly import CKyber
@_implementationOnly import COpus
import Foundation
import os
#if os(iOS) || os(tvOS)
import AVFoundation
#endif

/// Mesures du son, une fois par seconde.
public struct AudioStats: Sendable {
    public var channels = 0
    /// Tampon moyen avant la carte son, en millisecondes.
    public var bufferMs: Double = 0
    /// Latence de la sortie (carte son, Bluetooth…), en millisecondes.
    public var outputMs: Double = 0
    /// Trous comblés (pertes réseau ou tampon vide) dans la seconde.
    public var gaps = 0

    public init(channels: Int = 0, bufferMs: Double = 0, outputMs: Double = 0, gaps: Int = 0) {
        (self.channels, self.bufferMs, self.outputMs, self.gaps) = (channels, bufferMs, outputMs, gaps)
    }
}

final class AudioPlayer {
    static let sampleRate = 48_000
    /// Plus grand paquet Opus : 120 ms.
    private static let maxFrames = 5760

    var onStats: ((AudioStats) -> Void)?
    var volume: Float = 1 { didSet { state.withLock { $0.volume = volume } } }

    private var decoder: OpaquePointer?
    private var channels = 0
    private var pcm: [Float] = []
    private var unit: AudioUnit?
    private var outputMs: Double = 0
    /// La sortie démarre hors du fil réseau de Kyber : démarrer Core Audio
    /// prend jusqu'à 100 ms, et ce fil reçoit aussi l'image (paquets perdus
    /// sinon, au moment où le son commence).
    private let outputQueue = DispatchQueue(label: "fr.leslignesclaires.kybernow.audio-output", qos: .userInteractive)
    /// Kyber livre depuis son fil réseau, l'app arrête depuis le fil principal.
    private let io = NSLock()

    /// Partagé entre le fil réseau (écriture) et le fil temps réel de la carte son (lecture).
    private struct State {
        var ring: [Float] = []
        var read = 0
        var count = 0 // échantillons (toutes voies) en attente
        var channels = 2
        var volume: Float = 1
        /// Marge visée au plus bas du tampon, en images audio.
        var margin = AudioPlayer.sampleRate * 8 / 1000
        var playing = false
        /// Taille du dernier paquet, en images audio (10 ou 20 ms selon le serveur).
        var packetFrames = AudioPlayer.sampleRate / 50
        var lowest = Int.max
        var windowStart = 0.0
        var fillSum = 0.0
        var fillCount = 0
        var gaps = 0
        var calmSince = 0.0
    }
    private let state = OSAllocatedUnfairLock(initialState: State())

    deinit { stop() }

    // MARK: - Paquets (fil réseau)

    /// Rappel C donné à Kyber (`userdata` : ce lecteur, non retenu).
    static let callback: kyclient_MediaPacketCb = { packet, userdata in
        guard let packet, let userdata else { return }
        let me = Unmanaged<AudioPlayer>.fromOpaque(userdata).takeUnretainedValue()
        let p = packet.pointee
        switch p.kind {
        case KYCLIENT_MEDIA_PACKET_KIND_MEDIA:
            guard let data = p.data, p.len > 0 else { return }
            let buffer = UnsafeBufferPointer(start: data, count: Int(p.len))
            if p.is_config { me.configure(opusHead: buffer) } else { me.decode(buffer) }
        case KYCLIENT_MEDIA_PACKET_KIND_HOLE:
            me.conceal(samples: Int(p.missing_samples))
        default:
            break
        }
    }

    func configure(opusHead: UnsafeBufferPointer<UInt8>) {
        io.lock()
        defer { io.unlock() }
        // OpusHead : « OpusHead », version, voies, pré-saut, fréquence, gain, famille [, flux, couplés, table].
        guard opusHead.count >= 19, let channels = Optional(Int(opusHead[9])), channels > 0 else { return }
        let family = opusHead[18]
        var streams: Int32 = 1
        var coupled: Int32 = channels > 1 ? 1 : 0
        var mapping: [UInt8] = channels > 1 ? [0, 1] : [0]
        if family != 0, opusHead.count >= 21 + channels {
            streams = Int32(opusHead[19])
            coupled = Int32(opusHead[20])
            mapping = Array(opusHead[21..<(21 + channels)])
        }
        guard channels != self.channels || decoder == nil else { return }
        teardown()
        var error: Int32 = 0
        decoder = opus_multistream_decoder_create(Int32(Self.sampleRate), Int32(channels), streams, coupled, mapping, &error)
        guard error == OPUS_OK, decoder != nil else {
            KyberLog.audio.error("décodeur Opus : erreur \(error)")
            return
        }
        self.channels = channels
        pcm = [Float](repeating: 0, count: Self.maxFrames * channels)
        state.withLock {
            $0 = State()
            $0.channels = channels
            $0.volume = volume
            $0.ring = [Float](repeating: 0, count: Self.sampleRate * channels) // une seconde
        }
        outputQueue.async { [self] in
            startOutput(channels: channels)
            KyberLog.audio.notice("son : Opus, \(channels) voies, \(streams) flux, latence de sortie \(String(format: "%.1f", self.outputMs), privacy: .public) ms")
        }
    }

    func decode(_ data: UnsafeBufferPointer<UInt8>) {
        io.lock()
        defer { io.unlock() }
        guard let decoder, let base = data.baseAddress else { return }
        let frames = pcm.withUnsafeMutableBufferPointer {
            opus_multistream_decode_float(decoder, base, Int32(data.count), $0.baseAddress!, Int32(Self.maxFrames), 0)
        }
        if frames > 0 { push(Int(frames)) }
    }

    /// Données perdues : Opus invente un raccord plausible au lieu d'un silence sec.
    func conceal(samples: Int) {
        io.lock()
        defer { io.unlock() }
        guard let decoder, samples > 0 else { return }
        // Par tranches de 20 ms, multiples de 2,5 ms comme l'exige Opus.
        var left = min(samples, Self.sampleRate / 5)
        while left >= 120 {
            let chunk = min(left - left % 120, 960)
            let frames = pcm.withUnsafeMutableBufferPointer {
                opus_multistream_decode_float(decoder, nil, 0, $0.baseAddress!, Int32(chunk), 0)
            }
            guard frames > 0 else { break }
            push(Int(frames))
            left -= Int(frames)
        }
        state.withLock { $0.gaps += 1 }
    }

    private func push(_ frames: Int) {
        let n = frames * channels
        pcm.withUnsafeBufferPointer { src in
            state.withLock { s in
                let capacity = s.ring.count
                // Tampon plein (sortie arrêtée ?) : on garde le plus récent.
                if s.count + n > capacity {
                    let drop = s.count + n - capacity
                    s.read = (s.read + drop) % capacity
                    s.count -= drop
                }
                var write = (s.read + s.count) % capacity
                var i = 0
                while i < n {
                    let run = min(n - i, capacity - write)
                    s.ring.withUnsafeMutableBufferPointer { ring in
                        (ring.baseAddress! + write).update(from: src.baseAddress! + i, count: run)
                    }
                    i += run
                    write = (write + run) % capacity
                }
                s.count += n
                s.packetFrames = frames
            }
        }
    }

    // MARK: - Sortie (Core Audio)

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

        #if os(iOS) || os(tvOS)
        let session = AVAudioSession.sharedInstance()
        try? session.setCategory(.playback, mode: .default)
        try? session.setPreferredIOBufferDuration(0.005)
        try? session.setActive(true)
        #endif

        var format = AudioStreamBasicDescription(
            mSampleRate: Float64(Self.sampleRate), mFormatID: kAudioFormatLinearPCM,
            mFormatFlags: kAudioFormatFlagIsFloat | kAudioFormatFlagIsPacked,
            mBytesPerPacket: UInt32(4 * channels), mFramesPerPacket: 1, mBytesPerFrame: UInt32(4 * channels),
            mChannelsPerFrame: UInt32(channels), mBitsPerChannel: 32, mReserved: 0
        )
        AudioUnitSetProperty(unit, kAudioUnitProperty_StreamFormat, kAudioUnitScope_Input, 0, &format, UInt32(MemoryLayout.size(ofValue: format)))
        Self.setLayout(unit, channels: channels)

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
        outputMs = Self.outputLatency(unit)
    }

    func stop() {
        io.lock()
        defer { io.unlock() }
        teardown()
    }

    private func teardown() {
        outputQueue.sync {
            if let unit {
                AudioOutputUnitStop(unit)
                AudioUnitUninitialize(unit)
                AudioComponentInstanceDispose(unit)
            }
            unit = nil
        }
        if let decoder { opus_multistream_decoder_destroy(decoder) }
        decoder = nil
        channels = 0
    }

    /// Fil temps réel de la carte son : copie le tampon, règle la marge.
    private func render(frames: Int, into buffers: UnsafeMutableAudioBufferListPointer) {
        guard let out = buffers.first?.mData?.assumingMemoryBound(to: Float.self) else { return }
        var report: AudioStats?
        state.withLock { s in
            let ch = s.channels
            let wanted = frames * ch
            let now = CACurrentMediaTimeCompat()
            if !s.playing {
                // Démarrage ou reprise : on attend d'avoir la marge plus un paquet.
                if s.count >= (s.margin + s.packetFrames) * ch { s.playing = true }
            }
            var copied = 0
            if s.playing {
                copied = min(wanted, s.count)
                let capacity = s.ring.count
                var i = 0
                while i < copied {
                    let run = min(copied - i, capacity - s.read)
                    s.ring.withUnsafeBufferPointer { ring in
                        for k in 0..<run { out[i + k] = ring[s.read + k] * s.volume }
                    }
                    i += run
                    s.read = (s.read + run) % capacity
                }
                s.count -= copied
                if copied < wanted {
                    // Tampon vide : silence, puis marge plus large.
                    s.playing = false
                    s.gaps += 1
                    s.margin = min(s.margin + Self.sampleRate * 5 / 1000, Self.sampleRate * 60 / 1000)
                    s.calmSince = now
                }
            }
            if copied < wanted { (out + copied).update(repeating: 0, count: wanted - copied) }

            // Plus bas niveau du tampon sur la fenêtre : ce qui dépasse la marge est du retard inutile.
            let level = s.count / ch
            s.lowest = min(s.lowest, level)
            s.fillSum += Double(level)
            s.fillCount += 1
            if s.windowStart == 0 { s.windowStart = now; s.calmSince = now }
            if now - s.windowStart >= 1 {
                if s.playing, s.lowest != Int.max, s.lowest > s.margin + Self.sampleRate * 4 / 1000 {
                    let excess = min((s.lowest - s.margin) * ch, s.count)
                    s.read = (s.read + excess) % s.ring.count
                    s.count -= excess
                }
                // Dix secondes sans trou : la marge redescend d'une milliseconde.
                if now - s.calmSince >= 10 {
                    s.margin = max(s.margin - Self.sampleRate / 1000, Self.sampleRate * 4 / 1000)
                    s.calmSince = now
                }
                report = AudioStats(
                    channels: ch,
                    bufferMs: s.fillCount > 0 ? s.fillSum / Double(s.fillCount) * 1000 / Double(Self.sampleRate) : 0,
                    outputMs: 0,
                    gaps: s.gaps
                )
                s.windowStart = now
                s.lowest = Int.max
                s.fillSum = 0
                s.fillCount = 0
                s.gaps = 0
            }
        }
        if var report, let onStats {
            report.outputMs = outputMs
            DispatchQueue.main.async { onStats(report) }
        }
    }

    // MARK: - Réglages de la sortie

    private static var outputSubType: OSType {
        #if os(macOS)
        return kAudioUnitSubType_DefaultOutput
        #else
        return kAudioUnitSubType_RemoteIO
        #endif
    }

    /// Ordre des voies d'Opus (celui de Vorbis) : G, C, D, arrière G, arrière D, LFE pour le 5.1.
    private static func setLayout(_ unit: AudioUnit, channels: Int) {
        let labels: [AudioChannelLabel]
        switch channels {
        case 6: labels = [kAudioChannelLabel_Left, kAudioChannelLabel_Center, kAudioChannelLabel_Right,
                          kAudioChannelLabel_LeftSurround, kAudioChannelLabel_RightSurround, kAudioChannelLabel_LFEScreen]
        case 8: labels = [kAudioChannelLabel_Left, kAudioChannelLabel_Center, kAudioChannelLabel_Right,
                          kAudioChannelLabel_LeftSurroundDirect, kAudioChannelLabel_RightSurroundDirect,
                          kAudioChannelLabel_LeftSurround, kAudioChannelLabel_RightSurround, kAudioChannelLabel_LFEScreen]
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

    /// Latence de la sortie : tampon de la carte son plus ce qu'elle annonce (Bluetooth compris).
    private static func outputLatency(_ unit: AudioUnit) -> Double {
        #if os(macOS)
        var device = AudioDeviceID(0)
        var size = UInt32(MemoryLayout<AudioDeviceID>.size)
        guard AudioUnitGetProperty(unit, kAudioOutputUnitProperty_CurrentDevice, kAudioUnitScope_Global, 0, &device, &size) == noErr else { return 0 }
        func value(_ selector: AudioObjectPropertySelector) -> UInt32 {
            var address = AudioObjectPropertyAddress(mSelector: selector, mScope: kAudioObjectPropertyScopeOutput, mElement: kAudioObjectPropertyElementMain)
            var v: UInt32 = 0
            var s = UInt32(MemoryLayout<UInt32>.size)
            AudioObjectGetPropertyData(device, &address, 0, nil, &s, &v)
            return v
        }
        var rate = Float64(sampleRate)
        var address = AudioObjectPropertyAddress(mSelector: kAudioDevicePropertyNominalSampleRate, mScope: kAudioObjectPropertyScopeGlobal, mElement: kAudioObjectPropertyElementMain)
        var s = UInt32(MemoryLayout<Float64>.size)
        AudioObjectGetPropertyData(device, &address, 0, nil, &s, &rate)
        let frames = value(kAudioDevicePropertyLatency) + value(kAudioDevicePropertySafetyOffset) + value(kAudioDevicePropertyBufferFrameSize)
        return rate > 0 ? Double(frames) / rate * 1000 : 0
        #else
        return AVAudioSession.sharedInstance().outputLatency * 1000 + AVAudioSession.sharedInstance().ioBufferDuration * 1000
        #endif
    }
}

/// Horloge monotone en secondes, sans QuartzCore (fil temps réel).
@inline(__always)
private func CACurrentMediaTimeCompat() -> Double {
    Double(clock_gettime_nsec_np(CLOCK_UPTIME_RAW)) / 1e9
}
