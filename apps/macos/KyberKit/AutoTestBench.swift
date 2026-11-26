// Profil « Auto » (docs/profil-auto.md) : le décodage des extraits du test
// par le décodeur de la session, commun au Mac, à l'Apple TV, à l'iPhone et
// à l'iPad. À part d'AutoTestCore.swift (sans le cœur) pour l'essai hors
// ligne apps/macos/tests/autotest.

import Foundation

/// Le décodage des extraits du test (`SampleDecoder` du cœur) : les paquets
/// donnés au décodeur de la session (`VideoDecoder`) aussi vite qu'il les
/// prend, en boucle depuis l'image clé, pendant 1 s au plus ; les images
/// par seconde, comptées entre la première et la dernière image sortie.
enum AutoTestBench {
    /// Images au plus chez le décodeur à la fois (PyroWave : sa limite).
    private static let inFlight = 4
    private static let budget: TimeInterval = 1

    static func run(codec: VideoDecoder.Codec, packets: [(data: Data, key: Bool)], pyrowave: PyroWaveConfig?) -> Double? {
        let decoder = VideoDecoder()
        // Créé à 0 puis rempli : la mesure s'arrête avec des images encore
        // chez le décodeur (compte sous sa valeur de départ), et libdispatch
        // arrête l'app quand un sémaphore est rendu sous sa valeur de départ
        // (« Semaphore object deallocated while in use », 0.11.2).
        let slots = DispatchSemaphore(value: 0)
        for _ in 0..<(codec == .pyrowave ? PyroWaveDecoder.maxInFlight : inFlight) { slots.signal() }
        let lock = NSLock()
        var frames = 0
        var first: UInt64?
        var last: UInt64 = 0
        var lost = false
        decoder.onFrame = { _ in
            let now = DispatchTime.now().uptimeNanoseconds
            lock.lock()
            frames += 1
            if first == nil { first = now }
            last = now
            lock.unlock()
            slots.signal()
        }
        decoder.onLostReference = {
            lock.lock()
            lost = true
            lock.unlock()
        }
        decoder.setCodec(codec.rawValue)
        if let pyrowave {
            pyrowave.bytes.withUnsafeBufferPointer { decoder.decode($0, isConfig: true, isKey: true, pts: 0) }
        }
        let start = Date()
        var pts: UInt64 = 1
        var index = 0
        while Date().timeIntervalSince(start) < budget {
            // Le décodeur n'a rien rendu depuis 300 ms : refusé, ou trop lent.
            guard slots.wait(timeout: .now() + .milliseconds(300)) == .success else { break }
            lock.lock()
            let stop = lost
            lock.unlock()
            if stop { break }
            let p = packets[index]
            p.data.withUnsafeBytes { decoder.decode($0.bindMemory(to: UInt8.self), isConfig: false, isKey: p.key, pts: pts) }
            pts += 1
            index = (index + 1) % packets.count
        }
        decoder.flush()
        decoder.invalidate()
        lock.lock()
        defer { lock.unlock() }
        guard frames >= 2, let first, last > first else {
            KyberLog.video.notice("profil Auto : \(String(describing: codec), privacy: .public) pas décodé (\(frames) image(s))")
            return nil
        }
        let fps = Double(frames - 1) / (Double(last - first) / 1e9)
        KyberLog.video.notice("profil Auto : \(String(describing: codec), privacy: .public)\(pyrowave?.chroma444 == true ? " 4:4:4" : "", privacy: .public) \(Int(fps)) i/s (\(frames) images)")
        return fps
    }
}
