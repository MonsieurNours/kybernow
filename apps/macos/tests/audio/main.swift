// Test hors ligne du son de KyberKit : un la (440 Hz) encodé en Opus par
// paquets de 20 ms (puis 10 ms), envoyé au lecteur au rythme réel avec des à-coups
// (± 8 ms) et quelques pertes, en stéréo puis en 5.1. Volume à zéro : rien
// ne s'entend. On relève le tampon moyen, la latence de sortie et les trous.

import COpus
import Foundation
import os

enum KyberLog {
    static let audio = Logger(subsystem: "test", category: "audio")
}

func run(channels: Int32, seconds: Double, frame: Int = 960) {
    var err: Int32 = 0
    var streams: Int32 = 0, coupled: Int32 = 0
    var mapping = [UInt8](repeating: 0, count: Int(channels))
    let family: Int32 = channels > 2 ? 1 : 0
    let enc = opus_multistream_surround_encoder_create(48000, channels, family, &streams, &coupled, &mapping, OPUS_APPLICATION_RESTRICTED_LOWDELAY, &err)!
    var head: [UInt8] = Array("OpusHead".utf8) + [1, UInt8(channels), 0, 0] + withUnsafeBytes(of: UInt32(48000).littleEndian, Array.init) + [0, 0, UInt8(family)]
    if family != 0 { head += [UInt8(streams), UInt8(coupled)] + mapping }

    let player = AudioPlayer()
    player.volume = 0
    var reports: [AudioStats] = []
    player.onStats = { reports.append($0) }
    head.withUnsafeBufferPointer { player.configure(opusHead: $0) }

    var pcm = [Float](repeating: 0, count: frame * Int(channels))
    var packet = [UInt8](repeating: 0, count: 4000)
    var phase = 0.0
    let start = Date()
    var n = 0
    while Date().timeIntervalSince(start) < seconds {
        for i in 0..<frame {
            let v = Float(sin(phase) * 0.2)
            phase += 2 * .pi * 440 / 48000
            for c in 0..<Int(channels) { pcm[i * Int(channels) + c] = v }
        }
        let len = opus_multistream_encode_float(enc, pcm, Int32(frame), &packet, 4000)
        if n % 97 == 50 {
            player.conceal(samples: frame) // paquet perdu
        } else {
            packet.withUnsafeBufferPointer { player.decode(UnsafeBufferPointer(rebasing: $0[0..<Int(len)])) }
        }
        n += 1
        // Paquets attendus toutes les 20 ms, arrivant avec ± 8 ms d'à-coups.
        let due = start.addingTimeInterval(Double(n * frame) / 48000 + Double.random(in: -0.008...0.008))
        let wait = due.timeIntervalSinceNow
        RunLoop.main.run(until: Date().addingTimeInterval(max(0, wait)))
    }
    RunLoop.main.run(until: Date().addingTimeInterval(1.1))
    player.stop()
    let settled = reports.dropFirst(2)
    let buffer = settled.map(\.bufferMs).reduce(0, +) / Double(max(settled.count, 1))
    let gaps = settled.map(\.gaps).reduce(0, +)
    print("\(channels) voies, paquets de \(frame / 48) ms : \(reports.count) relevés, tampon moyen \(String(format: "%.1f", buffer)) ms, sortie \(String(format: "%.1f", reports.last?.outputMs ?? 0)) ms, trous \(gaps) (dont \(n / 97) pertes simulées)")
    opus_multistream_encoder_destroy(enc)
}

run(channels: 2, seconds: 8)
run(channels: 6, seconds: 5)
run(channels: 2, seconds: 6, frame: 480) // serveur avec kyber-patches/0004
