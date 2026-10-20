// Commun aux tests : journal factice et lecture d'un flux par unités d'accès.
import Foundation
import CoreVideo
import os

enum KyberLog { static let video = Logger(subsystem: "test", category: "video") }

func run(_ path: String, codec: UInt32) {
    let data = try! Data(contentsOf: URL(fileURLWithPath: path))
    let dec = VideoDecoder()
    dec.setCodec(codec)
    var frames = 0, total = 0.0, maxMs = 0.0
    var fmt = ""
    var info = mach_timebase_info_data_t(); mach_timebase_info(&info)
    dec.onFrame = { f in
        frames += 1
        let ms = Double(f.decoded - f.received) * Double(info.numer) / Double(info.denom) / 1e6
        total += ms; maxMs = max(maxMs, ms)
        let p = CVPixelBufferGetPixelFormatType(f.buffer)
        fmt = String(bytes: [24,16,8,0].map { UInt8((p >> $0) & 0xFF) }, encoding: .ascii)! + " planes=\(CVPixelBufferGetPlaneCount(f.buffer)) \(CVPixelBufferGetWidth(f.buffer))x\(CVPixelBufferGetHeight(f.buffer)) layout=\(VideoRenderer.layout(p)?.label ?? "NON GÉRÉ")"
    }
    data.withUnsafeBytes { raw in
        let all = raw.bindMemory(to: UInt8.self)
        let nals = VideoDecoder.nalUnits(all)
        // Regroupe par unité d'accès (AUD en tête), rendu en Annex B.
        var aus: [[UInt8]] = []
        for n in nals {
            let t = codec == 0x68323635 ? (n[0] >> 1) & 0x3F : n[0] & 0x1F
            let isAUD = codec == 0x68323635 ? t == 35 : t == 9
            if isAUD || aus.isEmpty { aus.append([]) }
            aus[aus.count-1] += [0,0,0,1] + Array(n)
        }
        for (i, au) in aus.enumerated() {
            au.withUnsafeBufferPointer { dec.decode($0, isConfig: false, isKey: i % 60 == 0, pts: UInt64(i)) }
        }
        print("\(path): \(aus.count) unités, \(frames) images, décodage moyen \(String(format: "%.2f", total/Double(max(frames,1)))) ms, max \(String(format: "%.2f", maxMs)) ms, \(fmt)")
    }
}

/// Unités d'accès (AUD en tête) d'un flux Annex B, chacune en Annex B.
func accessUnits(_ path: String, codec: UInt32) -> [[UInt8]] {
    let data = try! Data(contentsOf: URL(fileURLWithPath: path))
    var aus: [[UInt8]] = []
    data.withUnsafeBytes { raw in
        for n in VideoDecoder.nalUnits(raw.bindMemory(to: UInt8.self)) {
            let t = codec == 0x68323635 ? (n[0] >> 1) & 0x3F : n[0] & 0x1F
            let isAUD = codec == 0x68323635 ? t == 35 : t == 9
            if isAUD || aus.isEmpty { aus.append([]) }
            aus[aus.count - 1] += [0, 0, 0, 1] + Array(n)
        }
    }
    return aus
}
