// AV1 décodé par VideoToolbox (puces M3 et suivantes) : flux SVT-AV1 en
// faible délai (sans réordonnancement, comme NVENC en temps réel).

import CoreVideo
import Foundation
import Metal

guard DecoderCapabilities.current.av1 else {
    print("ce Mac ne décode pas l'AV1 en matériel : test sauté")
    exit(0)
}
var info = mach_timebase_info_data_t()
mach_timebase_info(&info)

func decodeAll(_ path: String) -> (frames: Int, units: Int, ms: Double, last: CVPixelBuffer?) {
    let units = temporalUnits(path)
    let dec = VideoDecoder()
    dec.setCodec(0x0061_7631)
    var frames = 0, total = 0.0
    var last: CVPixelBuffer?
    dec.onFrame = { f in
        frames += 1
        total += Double(f.decoded - f.received) * Double(info.numer) / Double(info.denom) / 1e6
        last = f.buffer
    }
    for (i, u) in units.enumerated() {
        u.withUnsafeBufferPointer { dec.decode($0, isConfig: false, isKey: i == 0, pts: UInt64(i)) }
    }
    return (frames, units.count, total / Double(max(frames, 1)), last)
}

let r = decodeAll(CommandLine.arguments[1])
let fmt = r.last.map { VideoRenderer.fourcc(CVPixelBufferGetPixelFormatType($0)) + " \(CVPixelBufferGetWidth($0))x\(CVPixelBufferGetHeight($0)) " + (VideoRenderer.layout(CVPixelBufferGetPixelFormatType($0))?.label ?? "NON GÉRÉ") } ?? "-"
print("av1 : \(r.units) unités, \(r.frames) images, décodage moyen \(String(format: "%.2f", r.ms)) ms, \(fmt)")
if r.frames != r.units { print("ÉCHEC des images manquent"); exit(1) }

for (path, expect) in CommandLine.arguments.dropFirst(2).map({ ($0, String($0.split(separator: "_").last!.prefix(6))) }) {
    guard let buf = decodeAll(path).last, let renderer = VideoRenderer(), let layout = VideoRenderer.layout(CVPixelBufferGetPixelFormatType(buf)) else {
        print("ÉCHEC \(path) : pas d'image"); exit(1)
    }
    let d = MTLTextureDescriptor.texture2DDescriptor(pixelFormat: .bgra8Unorm, width: 128, height: 128, mipmapped: false)
    d.usage = [.renderTarget, .shaderRead]; d.storageMode = .shared
    let tex = renderer.commandQueue.device.makeTexture(descriptor: d)!
    let cb = renderer.commandQueue.makeCommandBuffer()!
    _ = renderer.draw(buf, layout: layout, into: tex, commands: cb)
    cb.commit(); cb.waitUntilCompleted()
    var px = [UInt8](repeating: 0, count: 4)
    tex.getBytes(&px, bytesPerRow: 128 * 4, from: MTLRegionMake2D(64, 64, 1, 1), mipmapLevel: 0)
    print("av1 \(expect) → #\(String(format: "%02X%02X%02X", px[2], px[1], px[0]))")
}
