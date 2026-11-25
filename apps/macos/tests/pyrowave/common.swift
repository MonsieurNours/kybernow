// Commun aux outils PyroWave : journal factice, lecture du fichier d'essai.
import Foundation
import os

enum KyberLog { static let video = Logger(subsystem: "test", category: "video") }

/// Fichier d'essai (tools encode) : paquet de configuration de 16 octets,
/// puis chaque image précédée de sa taille (u32 LE).
func readPyroWaveFile(_ path: String) -> (config: [UInt8], frames: [[UInt8]]) {
    let data = [UInt8](try! Data(contentsOf: URL(fileURLWithPath: path)))
    let config = Array(data[0..<16])
    var frames: [[UInt8]] = []
    var i = 16
    while i + 4 <= data.count {
        let n = Int(data[i]) | Int(data[i + 1]) << 8 | Int(data[i + 2]) << 16 | Int(data[i + 3]) << 24
        i += 4
        frames.append(Array(data[i..<(i + n)]))
        i += n
    }
    return (config, frames)
}

func ms(_ ticks: UInt64) -> Double {
    var info = mach_timebase_info_data_t()
    mach_timebase_info(&info)
    return Double(ticks) * Double(info.numer) / Double(info.denom) / 1e6
}
