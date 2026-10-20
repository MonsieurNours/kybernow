// Son sans VLC : paquets Opus de Kyber → libopus → Core Audio.
//
// Latence : un petit tampon entre la réception (un paquet de 10 ou 20 ms à
// la fois, plus ou moins régulier) et la carte son (qui réclame quelques
// millisecondes à la fois). Sa marge se règle toute seule : elle grandit
// après un trou, puis redescend d'une milliseconde toutes les deux secondes
// sans trou. Un tampon vide est comblé par une trame de masquage d'Opus
// (PLC, raccord plausible) plutôt que par du silence, et la lecture reprend
// dès que la marge est de nouveau là. La dérive entre les horloges du PC et
// du Mac est rattrapée en douceur : un échantillon sauté ou doublé sur 256
// (0,4 %, inaudible) jusqu'à ce que le tampon soit revenu à sa marge, au lieu
// d'une coupe sèche. VLC gardait 50 ms de tampon fixe, plus le sien.
//
// Sortie : la carte son choisie par le système (un changement en cours de
// session, AirPods branchés, est suivi : latence et taille de tampon relues,
// unité recréée) ; si elle a moins de voies que le flux (5.1 ou 7.1 vers un
// casque), le son est mixé vers le bas (G + 0,7 C + 0,7 surrounds, idem à
// droite) au lieu d'être coupé par index.
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
    /// Voies envoyées à la sortie (moins que le flux : mixage descendant).
    public var outputChannels = 0

    public init(channels: Int = 0, bufferMs: Double = 0, outputMs: Double = 0, gaps: Int = 0, outputChannels: Int = 0) {
        (self.channels, self.bufferMs, self.outputMs, self.gaps, self.outputChannels) = (channels, bufferMs, outputMs, gaps, outputChannels)
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
    /// Trame de masquage (PLC) en cours de lecture : écrite sous `io` par le
    /// fil de la carte son, lue par lui seul.
    private var plc: [Float] = []
    private var plcRead = 0
    private var plcCount = 0
    #if os(macOS)
    private var deviceListener: ((UInt32, UnsafePointer<AudioObjectPropertyAddress>) -> Void)?
    #endif
    /// Un échantillon sauté ou doublé toutes les `stretchPeriod` images audio
    /// (0,4 %, inaudible) ; deux fois plus souvent quand l'écart dépasse 6 ms.
    private static let stretchPeriod = 256
    private static let stretchPeriodFast = 128
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
        /// Voies de la sortie (moins que `channels` : mixage descendant).
        var outChannels = 2
        var volume: Float = 1
        /// Marge visée au plus bas du tampon, en images audio.
        var margin = AudioPlayer.sampleRate * 8 / 1000
        var playing = false
        /// Jamais lu : le premier départ attend la marge plus un paquet ;
        /// une reprise après un trou n'attend que la marge.
        var started = false
        /// Taille du dernier paquet, en images audio (10 ou 20 ms selon le serveur).
        var packetFrames = AudioPlayer.sampleRate / 50
        var lowest = Int.max
        var windowStart = 0.0
        var fillSum = 0.0
        var fillCount = 0
        var gaps = 0
        /// Tampons vides dans la fenêtre (pas de rattrapage de dérive dans ce cas).
        var underruns = 0
        var calmSince = 0.0
        /// Dérive à rattraper, en images audio : > 0 à sauter, < 0 à doubler.
        var stretch = 0
        var stretchCounter = 0
        var stretchPeriod = AudioPlayer.stretchPeriod
    }
    private let state = OSAllocatedUnfairLock(initialState: State())

    deinit { stop() }

    // MARK: - Paquets (fil réseau)

    /// Rappel C donné à Kyber (`userdata` : ce lecteur, retenu par
    /// `PacketSinkLease` jusqu'au dernier appel, `CLOSED`).
    static let callback: kyclient_MediaPacketCb = { packet, userdata in
        guard let packet, let userdata else { return }
        let me = Unmanaged<AudioPlayer>.fromOpaque(userdata).takeUnretainedValue()
        let p = packet.pointee
        if p.kind == KYCLIENT_MEDIA_PACKET_KIND_CLOSED {
            // Plus aucun paquet ne viendra : le lecteur peut être libéré.
            me.stop()
            PacketSinkLease.release(AudioPlayer.self, userdata)
            return
        }
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
        plc = [Float](repeating: 0, count: Self.maxFrames * channels)
        plcRead = 0
        plcCount = 0
        state.withLock {
            $0 = State()
            $0.channels = channels
            $0.outChannels = channels
            $0.volume = volume
            $0.ring = [Float](repeating: 0, count: Self.sampleRate * channels) // une seconde
        }
        outputQueue.async { [self] in
            startOutput(channels: channels)
            KyberLog.audio.notice("son : Opus, \(channels) voies, \(streams) flux, latence de sortie \(String(format: "%.1f", self.outputMs), privacy: .public) ms")
            watchOutputDevice()
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
        // Moins de voies à la sortie (casque, AirPods) que dans le flux : mixage descendant.
        let deviceChannels = Self.outputDeviceChannels(unit)
        let outChannels = deviceChannels > 0 ? min(channels, deviceChannels) : channels
        state.withLock { $0.outChannels = outChannels }

        #if os(iOS) || os(tvOS)
        let session = AVAudioSession.sharedInstance()
        try? session.setCategory(.playback, mode: .default)
        try? session.setPreferredIOBufferDuration(0.005)
        // Son multicanal : spatialisé dans les AirPods, envoyé tel quel à un
        // ampli HDMI (LPCM) qui accepte ce nombre de voies.
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
        // Bluetooth et AirPlay demandent jusqu'à 4 096 images d'un coup (512 par défaut : TooManyFramesToProcess, silence).
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
        outputMs = Self.outputLatency(unit)
        if outChannels != channels {
            KyberLog.audio.notice("son : sortie à \(outChannels) voies pour un flux à \(channels) : mixage descendant")
        }
    }

    #if os(macOS)
    /// La sortie par défaut change (AirPods branchés, écran HDMI) : l'unité
    /// est recréée sur la nouvelle (voies, taille de tampon), et sa latence relue.
    private func watchOutputDevice() {
        guard deviceListener == nil else { return }
        var address = AudioObjectPropertyAddress(mSelector: kAudioHardwarePropertyDefaultOutputDevice, mScope: kAudioObjectPropertyScopeGlobal, mElement: kAudioObjectPropertyElementMain)
        let listener: (UInt32, UnsafePointer<AudioObjectPropertyAddress>) -> Void = { [weak self] _, _ in
            guard let self, let unit = self.unit else { return }
            AudioOutputUnitStop(unit)
            AudioUnitUninitialize(unit)
            AudioComponentInstanceDispose(unit)
            self.unit = nil
            let channels = self.state.withLock { $0.channels }
            self.startOutput(channels: channels)
            if let unit = self.unit {
                let buffer = Self.outputBufferFrames(unit)
                KyberLog.audio.notice("son : sortie changée, latence \(String(format: "%.1f", self.outputMs), privacy: .public) ms, tampon de la carte \(buffer) images (\(String(format: "%.1f", Double(buffer) * 1000 / Double(Self.sampleRate)), privacy: .public) ms)")
            }
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

    func stop() {
        io.lock()
        defer { io.unlock() }
        teardown()
    }

    private func teardown() {
        outputQueue.sync {
            unwatchOutputDevice()
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

    /// Fil temps réel de la carte son : copie le tampon (mixé vers le bas si
    /// la sortie a moins de voies), rattrape la dérive, règle la marge, et
    /// comble un tampon vide avec une trame de masquage d'Opus.
    private func render(frames: Int, into buffers: UnsafeMutableAudioBufferListPointer) {
        guard let out = buffers.first?.mData?.assumingMemoryBound(to: Float.self) else { return }
        let (copied, ch, outCh, packetFrames, volume, report) = state.withLock { s -> (Int, Int, Int, Int, Float, AudioStats?) in
            var report: AudioStats?
            var copied = 0
            let ch = s.channels, outCh = s.outChannels, packetFrames = s.packetFrames, volume = s.volume
            let capacity = s.ring.count
            let now = CACurrentMediaTimeCompat()
            if !s.playing {
                // Premier départ : la marge plus un paquet. Reprise après un
                // trou : dès que la marge est là (le trou est comblé par Opus).
                let wanted = s.started ? s.margin : s.margin + s.packetFrames
                if s.count >= wanted * ch {
                    s.playing = true
                    s.started = true
                }
            }
            if s.playing {
                let n = min(frames, s.count / ch)
                s.ring.withUnsafeBufferPointer { ring in
                    while copied < n {
                        Self.mix(ring.baseAddress! + s.read, channels: ch, into: out + copied * outCh, outputChannels: outCh, volume: volume)
                        copied += 1
                        s.stretchCounter += 1
                        if s.stretch != 0, s.stretchCounter >= s.stretchPeriod {
                            s.stretchCounter = 0
                            if s.stretch > 0, s.count >= 3 * ch {
                                // En avance : une image audio sautée (la lecture avance de deux).
                                s.read = (s.read + 2 * ch) % capacity
                                s.count -= 2 * ch
                                s.stretch -= 1
                                continue
                            } else if s.stretch < 0 {
                                // En retard : une image audio doublée (la lecture n'avance pas).
                                s.stretch += 1
                                continue
                            }
                        }
                        s.read = (s.read + ch) % capacity
                        s.count -= ch
                    }
                }
                if copied < frames {
                    // Tampon vide : masquage d'Opus le temps que la marge revienne, marge plus large.
                    s.playing = false
                    s.gaps += 1
                    s.underruns += 1
                    s.margin = min(s.margin + Self.sampleRate * 5 / 1000, Self.sampleRate * 60 / 1000)
                    s.calmSince = now
                    s.stretch = 0
                }
            }

            // Plus bas niveau du tampon sur la fenêtre : ce qui dépasse la marge est du retard inutile.
            let level = s.count / ch
            s.lowest = min(s.lowest, level)
            s.fillSum += Double(level)
            s.fillCount += 1
            if s.windowStart == 0 { s.windowStart = now; s.calmSince = now }
            if now - s.windowStart >= 1 {
                if s.playing, s.underruns == 0, s.lowest != Int.max {
                    let excess = s.lowest - s.margin
                    if excess > Self.sampleRate * 10 / 1000 {
                        // Loin devant (démarrage, sortie restée arrêtée) : coupe sèche, une fois.
                        // À 1 sur 256, rattraper 20 ms prendrait 5 s.
                        let drop = min(excess * ch, s.count)
                        s.read = (s.read + drop) % capacity
                        s.count -= drop
                        s.stretch = 0
                    } else if excess > Self.sampleRate * 2 / 1000 {
                        s.stretch = excess // échantillons à sauter (dérive : 3 ms par minute au plus)
                        s.stretchPeriod = excess > Self.sampleRate * 6 / 1000 ? Self.stretchPeriodFast : Self.stretchPeriod
                    } else if excess < -(Self.sampleRate * 2 / 1000) {
                        s.stretch = excess // échantillons à doubler
                        s.stretchPeriod = Self.stretchPeriod
                    }
                }
                // Deux secondes sans trou : la marge redescend d'une milliseconde.
                if now - s.calmSince >= 2 {
                    s.margin = max(s.margin - Self.sampleRate / 1000, Self.sampleRate * 4 / 1000)
                    s.calmSince = now
                }
                report = AudioStats(
                    channels: ch,
                    bufferMs: s.fillCount > 0 ? s.fillSum / Double(s.fillCount) * 1000 / Double(Self.sampleRate) : 0,
                    outputMs: 0,
                    gaps: s.gaps,
                    outputChannels: outCh
                )
                s.windowStart = now
                s.lowest = Int.max
                s.fillSum = 0
                s.fillCount = 0
                s.gaps = 0
                s.underruns = 0
            }
            return (copied, ch, outCh, packetFrames, volume, report)
        }
        if copied < frames { conceal(into: out + copied * outCh, frames: frames - copied, channels: ch, outputChannels: outCh, packetFrames: packetFrames, volume: volume) }
        if var report, let onStats {
            report.outputMs = outputMs
            DispatchQueue.main.async { onStats(report) }
        }
    }

    /// Tampon vide (fil de la carte son) : trame de masquage d'Opus (PLC),
    /// par paquets, le reste gardé pour la demande suivante. Si le décodeur
    /// est occupé par le fil réseau (un paquet arrive justement), silence.
    private func conceal(into out: UnsafeMutablePointer<Float>, frames: Int, channels ch: Int, outputChannels outCh: Int, packetFrames: Int, volume: Float) {
        var done = 0
        while done < frames {
            if plcCount == 0 {
                guard io.try() else { break }
                var produced: Int32 = 0
                if let decoder, !plc.isEmpty {
                    let size = Int32(max(120, min(packetFrames, Self.maxFrames)))
                    produced = plc.withUnsafeMutableBufferPointer {
                        opus_multistream_decode_float(decoder, nil, 0, $0.baseAddress!, size, 0)
                    }
                }
                io.unlock()
                guard produced > 0 else { break }
                plcCount = Int(produced)
                plcRead = 0
            }
            let n = min(frames - done, plcCount)
            plc.withUnsafeBufferPointer { src in
                for i in 0..<n {
                    Self.mix(src.baseAddress! + (plcRead + i) * ch, channels: ch, into: out + (done + i) * outCh, outputChannels: outCh, volume: volume)
                }
            }
            plcRead += n
            plcCount -= n
            done += n
        }
        if done < frames { (out + done * outCh).update(repeating: 0, count: (frames - done) * outCh) }
    }

    /// Une image audio du flux (`channels` voies, ordre Vorbis) vers la sortie
    /// (`outputChannels` voies). Moins de voies : G + 0,7 C + 0,7 surrounds,
    /// D + 0,7 C + 0,7 surrounds, LFE ignoré ; mono = moyenne ; plus de voies :
    /// les voies du flux, le reste à zéro.
    @inline(__always)
    private static func mix(_ src: UnsafePointer<Float>, channels ch: Int, into dst: UnsafeMutablePointer<Float>, outputChannels outCh: Int, volume: Float) {
        if ch == outCh {
            for c in 0..<ch { dst[c] = src[c] * volume }
            return
        }
        @inline(__always) func clamp(_ v: Float) -> Float { min(max(v * volume, -1), 1) }
        let half: Float = 0.7071
        switch (ch, outCh) {
        case (6, 2):
            dst[0] = clamp(src[0] + half * src[1] + half * src[3])
            dst[1] = clamp(src[2] + half * src[1] + half * src[4])
        case (8, 2):
            dst[0] = clamp(src[0] + half * src[1] + half * (src[3] + src[5]))
            dst[1] = clamp(src[2] + half * src[1] + half * (src[4] + src[6]))
        case (8, 6):
            dst[0] = src[0] * volume
            dst[1] = src[1] * volume
            dst[2] = src[2] * volume
            dst[3] = clamp(src[3] + half * src[5])
            dst[4] = clamp(src[4] + half * src[6])
            dst[5] = src[7] * volume
        case (_, 1):
            var left = src[0], right = src[ch > 1 ? (ch >= 6 ? 2 : 1) : 0]
            if ch >= 6 {
                left += half * src[1] + half * src[3]
                right += half * src[1] + half * src[4]
            }
            dst[0] = clamp((left + right) / 2)
        default:
            for c in 0..<outCh { dst[c] = c < ch ? src[c] * volume : 0 }
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

    /// Ordre des voies d'Opus (celui de Vorbis) : G, C, D, surround G, surround D,
    /// LFE pour le 5.1 ; G, C, D, latéral G, latéral D, arrière G, arrière D,
    /// LFE pour le 7.1. Dans la disposition Core Audio du 7.1 (MPEG 7.1 C),
    /// les latérales sont `LeftSurround`/`RightSurround` et les arrières
    /// `RearSurroundLeft`/`RearSurroundRight`.
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

    /// Taille de tampon effective de la carte son, en images audio.
    private static func outputBufferFrames(_ unit: AudioUnit) -> Int {
        #if os(macOS)
        guard let device = outputDevice(unit) else { return 0 }
        return Int(deviceValue(device, kAudioDevicePropertyBufferFrameSize))
        #else
        return Int(AVAudioSession.sharedInstance().ioBufferDuration * Double(sampleRate))
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

/// Horloge monotone en secondes, sans QuartzCore (fil temps réel).
@inline(__always)
private func CACurrentMediaTimeCompat() -> Double {
    Double(clock_gettime_nsec_np(CLOCK_UPTIME_RAW)) / 1e9
}
