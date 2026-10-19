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
    /// Canaux de la sortie audio par défaut du Mac (2, 6 ou 8).
    static func channels() -> Int {
        var device = AudioDeviceID(0)
        var size = UInt32(MemoryLayout<AudioDeviceID>.size)
        var address = AudioObjectPropertyAddress(
            mSelector: kAudioHardwarePropertyDefaultOutputDevice,
            mScope: kAudioObjectPropertyScopeGlobal,
            mElement: kAudioObjectPropertyElementMain
        )
        guard AudioObjectGetPropertyData(AudioObjectID(kAudioObjectSystemObject), &address, 0, nil, &size, &device) == noErr else { return 2 }

        address.mSelector = kAudioDevicePropertyStreamConfiguration
        address.mScope = kAudioDevicePropertyScopeOutput
        guard AudioObjectGetPropertyDataSize(device, &address, 0, nil, &size) == noErr, size > 0 else { return 2 }
        let raw = UnsafeMutableRawPointer.allocate(byteCount: Int(size), alignment: MemoryLayout<AudioBufferList>.alignment)
        defer { raw.deallocate() }
        guard AudioObjectGetPropertyData(device, &address, 0, nil, &size, raw) == noErr else { return 2 }
        let list = UnsafeMutableAudioBufferListPointer(raw.assumingMemoryBound(to: AudioBufferList.self))
        let total = list.reduce(0) { $0 + Int($1.mNumberChannels) }
        return total >= 8 ? 8 : total >= 6 ? 6 : 2
    }
}
