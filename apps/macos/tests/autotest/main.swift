// Essai hors ligne du décodage des extraits du profil Auto : les six extraits
// embarqués par le cœur (apps/core/samples/*.kns, lus comme parse_sample)
// passés à AutoTestBench.run, le chemin du test dans l'app ; puis le
// décodeur rendu, comme à la fin de chaque format.
import Foundation
import os
import VideoToolbox

enum KyberLog { static let video = Logger(subsystem: "test", category: "video") }

struct Sample { var fourcc: UInt32; var yuv444: Bool; var width: Int; var height: Int; var packets: [(data: Data, key: Bool)] }

func parse(_ d: Data) -> Sample? {
    var i = d.startIndex
    func take(_ n: Int) -> Data? { guard d.endIndex - i >= n else { return nil }; defer { i += n }; return d[i..<i + n] }
    func le<T: FixedWidthInteger>(_ t: T.Type) -> T? { take(MemoryLayout<T>.size).map { $0.reversed().reduce(T(0)) { $0 << 8 | T($1) } } }
    guard take(4) == Data("KNS1".utf8), let fourcc = le(UInt32.self), let flags = take(1)?.first,
          let w = le(UInt16.self), let h = le(UInt16.self), let count = le(UInt16.self) else { return nil }
    var packets: [(Data, Bool)] = []
    for _ in 0..<count {
        guard let f = take(1)?.first, let len = le(UInt32.self), let p = take(Int(len)) else { return nil }
        packets.append((Data(p), f & 1 == 1))
    }
    return Sample(fourcc: fourcc, yuv444: flags & 1 == 1, width: Int(w), height: Int(h), packets: packets)
}

let av1 = VTIsHardwareDecodeSupported(kCMVideoCodecType_AV1)
var status: Int32 = 0
for path in CommandLine.arguments.dropFirst() {
    let name = URL(fileURLWithPath: path).lastPathComponent
    guard let s = parse(try! Data(contentsOf: URL(fileURLWithPath: path))),
          let codec = VideoDecoder.Codec(rawValue: s.fourcc) else {
        print("\(name) : illisible"); status = 1; continue
    }
    let config = codec == .pyrowave
        ? PyroWaveConfig(chroma444: s.yuv444, hdr: false, fullRange: false, width: s.width, height: s.height)
        : nil
    let fps = AutoTestBench.run(codec: codec, packets: s.packets, pyrowave: config)
    // AV1 sans décodeur matériel (puces M1, M2) : pas de mesure, sans plantage.
    let expected = codec != .av1 || av1
    print("\(name) : \(s.packets.count) paquets, " + (fps.map { "\(Int($0)) i/s" } ?? "pas décodé") + (expected ? "" : " (pas de décodeur AV1 ici)"))
    if fps == nil && expected { status = 1 }
}
exit(status)
