// Ce que ce Mac annonce à l'ordinateur : taille et fréquence de son écran,
// nombre de canaux de sa sortie audio.

import AppKit
import CoreAudio

enum DisplayInfo {
    /// Mode de l'écran en pixels réels, ramené à la 4K au plus (au-delà,
    /// l'encodeur n'apporte rien de visible et coûte en latence).
    static func mode(for screen: NSScreen) -> (width: Int, height: Int, refresh: Int) {
        let scale = screen.backingScaleFactor
        var w = screen.frame.width * scale
        var h = screen.frame.height * scale
        let fit = min(1, 3840 / w, 2160 / h)
        w = (w * fit / 2).rounded() * 2
        h = (h * fit / 2).rounded() * 2
        let hz = max(30, screen.maximumFramesPerSecond)
        return (Int(w), Int(h), hz)
    }
}

enum AudioOutput {
    /// Canaux à demander à l'ordinateur : la configuration d'enceintes de la
    /// sortie par défaut (Configuration audio et MIDI › Configurer les
    /// enceintes), pas son nombre de sorties physiques (une carte son à
    /// 8 sorties reste souvent branchée sur une paire d'enceintes).
    static func channels() -> Int {
        var device = AudioDeviceID(0)
        var size = UInt32(MemoryLayout<AudioDeviceID>.size)
        var address = AudioObjectPropertyAddress(
            mSelector: kAudioHardwarePropertyDefaultOutputDevice,
            mScope: kAudioObjectPropertyScopeGlobal,
            mElement: kAudioObjectPropertyElementMain
        )
        guard AudioObjectGetPropertyData(AudioObjectID(kAudioObjectSystemObject), &address, 0, nil, &size, &device) == noErr else { return 2 }

        address.mSelector = kAudioDevicePropertyPreferredChannelLayout
        address.mScope = kAudioDevicePropertyScopeOutput
        guard AudioObjectGetPropertyDataSize(device, &address, 0, nil, &size) == noErr, size >= MemoryLayout<AudioChannelLayout>.size else { return 2 }
        let raw = UnsafeMutableRawPointer.allocate(byteCount: Int(size), alignment: MemoryLayout<AudioChannelLayout>.alignment)
        defer { raw.deallocate() }
        guard AudioObjectGetPropertyData(device, &address, 0, nil, &size, raw) == noErr else { return 2 }
        let layout = raw.assumingMemoryBound(to: AudioChannelLayout.self)
        // Seules les positions d'enceintes comptent : une carte son déclare
        // souvent ses sorties en canaux « discrets » sans configuration surround.
        let count = Int(layout.pointee.mNumberChannelDescriptions)
        let offset = MemoryLayout<AudioChannelLayout>.offset(of: \.mChannelDescriptions)!
        let descriptions = UnsafeBufferPointer(start: (raw + offset).assumingMemoryBound(to: AudioChannelDescription.self), count: count)
        let labels = Set(descriptions.map(\.mChannelLabel))
        let surround: Set<AudioChannelLabel> = [kAudioChannelLabel_LeftSurround, kAudioChannelLabel_RightSurround]
        let rear: Set<AudioChannelLabel> = [kAudioChannelLabel_RearSurroundLeft, kAudioChannelLabel_RearSurroundRight,
                                            kAudioChannelLabel_LeftSurroundDirect, kAudioChannelLabel_RightSurroundDirect]
        guard labels.contains(kAudioChannelLabel_Center), labels.isSuperset(of: surround) else { return 2 }
        return labels.intersection(rear).count >= 2 ? 8 : 6
    }
}
