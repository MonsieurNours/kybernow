// Vibrations HD de la DualSense branchée en USB (docs/playstation.md,
// « Vibrations HD ») : la manette (ou une DualSense Edge) est aussi une
// carte son à quatre voies dont les voies 3 et 4 tiennent ses moteurs. Le
// cœur rééchantillonne à 48 kHz le flux de vibrations de la console et le
// garde dans un anneau (`apps/core/src/playstation/dualsense.rs`) ; ici,
// seulement la sortie : une unité Core Audio sur la carte son de la manette,
// dont le fil temps réel tire les échantillons (`kn_ps_haptics_fill`).
//
// Une seule pour l'app (le cœur n'a qu'un anneau) : prise par chaque partie
// (`retain`), rendue à sa fin (`release`). La liste des cartes son est
// suivie : manette branchée pendant la partie (sa carte son apparaît une
// seconde après elle), débranchée ou passée en Bluetooth (le cœur revient
// aux vibrations simples). Le son du jeu n'est pas touché : il reste sur la
// sortie du système, la manette n'est qu'une sortie de plus.

#if os(macOS)
import AudioToolbox
import CoreAudio
import Foundation
@_implementationOnly import CKyberNow

final class DualSenseHaptics: @unchecked Sendable {
    static let shared = DualSenseHaptics()

    private static let sampleRate = 48_000.0
    /// DualSense et DualSense Edge (Sony).
    private static let vendor = 0x054C
    private static let products: Set<Int> = [0x0CE6, 0x0DF2]

    /// Tout se passe sur cette file (listes de cartes son comprises).
    private let queue = DispatchQueue(label: "fr.leslignesclaires.kybernow.dualsense-haptics", qos: .userInteractive)
    private var users = 0
    private var unit: AudioUnit?
    private var device = AudioDeviceID(0)
    private var listener: AudioObjectPropertyListenerBlock?

    private init() {}

    /// Une partie commence : la sortie s'ouvre si une DualSense est branchée en USB.
    func retain() {
        queue.async { [self] in
            users += 1
            guard users == 1 else { return }
            watch()
            connect()
        }
    }

    /// La partie se termine : la dernière ferme la sortie.
    func release() {
        queue.async { [self] in
            users = max(0, users - 1)
            guard users == 0 else { return }
            unwatch()
            disconnect()
        }
    }

    // MARK: - Carte son de la manette

    /// Une carte son de DualSense branchée en USB, à quatre voies au moins.
    private struct Found {
        var id: AudioDeviceID
        var name: String
        var channels: Int
    }

    private static func find() -> Found? {
        var address = AudioObjectPropertyAddress(mSelector: kAudioHardwarePropertyDevices, mScope: kAudioObjectPropertyScopeGlobal, mElement: kAudioObjectPropertyElementMain)
        var size: UInt32 = 0
        guard AudioObjectGetPropertyDataSize(AudioObjectID(kAudioObjectSystemObject), &address, 0, nil, &size) == noErr, size > 0 else { return nil }
        var ids = [AudioDeviceID](repeating: 0, count: Int(size) / MemoryLayout<AudioDeviceID>.size)
        guard AudioObjectGetPropertyData(AudioObjectID(kAudioObjectSystemObject), &address, 0, nil, &size, &ids) == noErr else { return nil }
        for id in ids {
            let name = string(id, kAudioObjectPropertyName) ?? ""
            let model = string(id, kAudioDevicePropertyModelUID) ?? ""
            guard uint32(id, kAudioDevicePropertyTransportType) == kAudioDeviceTransportTypeUSB,
                  isDualSense(name: name, modelUID: model) else { continue }
            let n = outputChannels(id)
            guard n >= 4 else {
                KyberLog.audio.notice("vibrations HD : « \(name, privacy: .public) » n'a que \(n) voies de sortie")
                continue
            }
            return Found(id: id, name: name, channels: n)
        }
        return nil
    }

    /// Par les identifiants USB de l'identifiant du modèle (« nom:054C:0CE6 »
    /// pour une carte son USB), sinon par le nom.
    static func isDualSense(name: String, modelUID: String) -> Bool {
        let parts = modelUID.split(separator: ":")
        if parts.count >= 2, let v = Int(parts[parts.count - 2], radix: 16), let p = Int(parts[parts.count - 1], radix: 16) {
            return v == vendor && products.contains(p)
        }
        return name.localizedCaseInsensitiveContains("DualSense") || name.localizedCaseInsensitiveContains("Wireless Controller")
    }

    private func connect() {
        guard unit == nil, users > 0 else { return }
        guard let found = Self.find() else {
            KyberLog.audio.notice("vibrations HD : pas de DualSense en USB, vibrations simples")
            return
        }
        guard let unit = Self.open(found, owner: self) else {
            KyberLog.audio.error("vibrations HD : « \(found.name, privacy: .public) » ne s'ouvre pas, vibrations simples")
            return
        }
        self.unit = unit.unit
        device = found.id
        kn_ps_haptics_set_output(true)
        KyberLog.audio.notice("vibrations HD : « \(found.name, privacy: .public) » (\(found.channels) voies), voies 3 et 4 à 48 kHz\(unit.mapped ? "" : " (sans carte des voies)", privacy: .public)")
    }

    private func disconnect() {
        guard let unit else { return }
        kn_ps_haptics_set_output(false)
        AudioOutputUnitStop(unit)
        AudioUnitUninitialize(unit)
        AudioComponentInstanceDispose(unit)
        self.unit = nil
        device = 0
    }

    private struct Opened {
        var unit: AudioUnit
        /// Carte des voies posée : l'unité reçoit deux voies, envoyées aux voies 3 et 4.
        var mapped: Bool
    }

    /// L'unité de sortie sur la carte son de la manette : deux voies (les
    /// vibrations) envoyées aux voies 3 et 4 par la carte des voies ; si la
    /// carte des voies est refusée, toutes les voies de la manette, du
    /// silence sur les deux premières.
    private static func open(_ found: Found, owner: DualSenseHaptics) -> Opened? {
        var desc = AudioComponentDescription(componentType: kAudioUnitType_Output, componentSubType: kAudioUnitSubType_HALOutput,
                                             componentManufacturer: kAudioUnitManufacturer_Apple, componentFlags: 0, componentFlagsMask: 0)
        guard let component = AudioComponentFindNext(nil, &desc) else { return nil }
        var instance: AudioUnit?
        guard AudioComponentInstanceNew(component, &instance) == noErr, let unit = instance else { return nil }
        var ok = false
        defer { if !ok { AudioComponentInstanceDispose(unit) } }
        var device = found.id
        guard AudioUnitSetProperty(unit, kAudioOutputUnitProperty_CurrentDevice, kAudioUnitScope_Global, 0, &device, UInt32(MemoryLayout<AudioDeviceID>.size)) == noErr else { return nil }

        // Voies 3 et 4 de la manette ← voies 1 et 2 de l'unité ; les autres muettes.
        var map = [Int32](repeating: -1, count: found.channels)
        map[2] = 0
        map[3] = 1
        let mapped = AudioUnitSetProperty(unit, kAudioOutputUnitProperty_ChannelMap, kAudioUnitScope_Output, 0, &map, UInt32(map.count * MemoryLayout<Int32>.size)) == noErr
        let channels = mapped ? 2 : found.channels
        var format = AudioStreamBasicDescription(
            mSampleRate: sampleRate, mFormatID: kAudioFormatLinearPCM,
            mFormatFlags: kAudioFormatFlagIsFloat | kAudioFormatFlagIsPacked,
            mBytesPerPacket: UInt32(4 * channels), mFramesPerPacket: 1, mBytesPerFrame: UInt32(4 * channels),
            mChannelsPerFrame: UInt32(channels), mBitsPerChannel: 32, mReserved: 0
        )
        guard AudioUnitSetProperty(unit, kAudioUnitProperty_StreamFormat, kAudioUnitScope_Input, 0, &format, UInt32(MemoryLayout.size(ofValue: format))) == noErr else { return nil }

        var callback = AURenderCallbackStruct(inputProc: { refCon, _, _, _, frames, buffers in
            guard let buffers else { return noErr }
            _ = refCon
            let list = UnsafeMutableAudioBufferListPointer(buffers)
            guard let buffer = list.first, let out = buffer.mData?.assumingMemoryBound(to: Float.self) else { return noErr }
            // Deux voies : la carte des voies les envoie aux voies 3 et 4 ;
            // toutes les voies de la manette : les vibrations sur les voies 3 et 4.
            let channels = max(1, Int(buffer.mNumberChannels))
            kn_ps_haptics_fill(out, UInt(frames), UInt(channels), UInt(channels >= 4 ? 2 : 0))
            return noErr
        }, inputProcRefCon: Unmanaged.passUnretained(owner).toOpaque())
        guard AudioUnitSetProperty(unit, kAudioUnitProperty_SetRenderCallback, kAudioUnitScope_Input, 0, &callback, UInt32(MemoryLayout<AURenderCallbackStruct>.size)) == noErr else { return nil }

        // Petites demandes de la carte son de la manette (5 ms) : les
        // vibrations suivent l'image de près. Elle n'a pas d'autre usage.
        var frames: UInt32 = 240
        AudioUnitSetProperty(unit, kAudioDevicePropertyBufferFrameSize, kAudioUnitScope_Global, 0, &frames, UInt32(MemoryLayout<UInt32>.size))

        guard AudioUnitInitialize(unit) == noErr else { return nil }
        guard AudioOutputUnitStart(unit) == noErr else {
            AudioUnitUninitialize(unit)
            return nil
        }
        ok = true
        return Opened(unit: unit, mapped: mapped)
    }

    // MARK: - Cartes son branchées, débranchées

    private func watch() {
        guard listener == nil else { return }
        var address = AudioObjectPropertyAddress(mSelector: kAudioHardwarePropertyDevices, mScope: kAudioObjectPropertyScopeGlobal, mElement: kAudioObjectPropertyElementMain)
        let block: AudioObjectPropertyListenerBlock = { [weak self] _, _ in self?.devicesChanged() }
        if AudioObjectAddPropertyListenerBlock(AudioObjectID(kAudioObjectSystemObject), &address, queue, block) == noErr {
            listener = block
        }
    }

    private func unwatch() {
        guard let listener else { return }
        var address = AudioObjectPropertyAddress(mSelector: kAudioHardwarePropertyDevices, mScope: kAudioObjectPropertyScopeGlobal, mElement: kAudioObjectPropertyElementMain)
        AudioObjectRemovePropertyListenerBlock(AudioObjectID(kAudioObjectSystemObject), &address, queue, listener)
        self.listener = nil
    }

    /// Sur la file : la manette ouverte a disparu (débranchée), ou une
    /// DualSense vient d'apparaître.
    private func devicesChanged() {
        guard users > 0 else { return }
        if unit != nil {
            if Self.uint32(device, kAudioDevicePropertyDeviceIsAlive) == 0 || Self.find()?.id != device {
                KyberLog.audio.notice("vibrations HD : DualSense débranchée, vibrations simples")
                disconnect()
                connect()
            }
        } else {
            connect()
        }
    }

    // MARK: - Propriétés

    private static func string(_ id: AudioObjectID, _ selector: AudioObjectPropertySelector) -> String? {
        var address = AudioObjectPropertyAddress(mSelector: selector, mScope: kAudioObjectPropertyScopeGlobal, mElement: kAudioObjectPropertyElementMain)
        var value: Unmanaged<CFString>?
        var size = UInt32(MemoryLayout<Unmanaged<CFString>?>.size)
        guard AudioObjectGetPropertyData(id, &address, 0, nil, &size, &value) == noErr, let value else { return nil }
        return value.takeRetainedValue() as String
    }

    private static func uint32(_ id: AudioObjectID, _ selector: AudioObjectPropertySelector) -> UInt32 {
        var address = AudioObjectPropertyAddress(mSelector: selector, mScope: kAudioObjectPropertyScopeGlobal, mElement: kAudioObjectPropertyElementMain)
        var value: UInt32 = 0
        var size = UInt32(MemoryLayout<UInt32>.size)
        AudioObjectGetPropertyData(id, &address, 0, nil, &size, &value)
        return value
    }

    private static func outputChannels(_ id: AudioDeviceID) -> Int {
        var address = AudioObjectPropertyAddress(mSelector: kAudioDevicePropertyStreamConfiguration, mScope: kAudioObjectPropertyScopeOutput, mElement: kAudioObjectPropertyElementMain)
        var size: UInt32 = 0
        guard AudioObjectGetPropertyDataSize(id, &address, 0, nil, &size) == noErr, size > 0 else { return 0 }
        let raw = UnsafeMutableRawPointer.allocate(byteCount: Int(size), alignment: MemoryLayout<AudioBufferList>.alignment)
        defer { raw.deallocate() }
        guard AudioObjectGetPropertyData(id, &address, 0, nil, &size, raw) == noErr else { return 0 }
        let list = UnsafeMutableAudioBufferListPointer(raw.assumingMemoryBound(to: AudioBufferList.self))
        return list.reduce(0) { $0 + Int($1.mNumberChannels) }
    }
}
#endif
