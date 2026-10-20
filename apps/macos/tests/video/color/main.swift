import Foundation
import CoreVideo
import Metal

func checkColor(_ path: String, expect: String) {
    let data = try! Data(contentsOf: URL(fileURLWithPath: path))
    let dec = VideoDecoder()
    dec.setCodec(0x68323635)
    var last: CVPixelBuffer?
    dec.onFrame = { last = $0.buffer }
    data.withUnsafeBytes { raw in
        let all = raw.bindMemory(to: UInt8.self)
        var au: [UInt8] = []
        var aus: [[UInt8]] = []
        for n in VideoDecoder.nalUnits(all) {
            if (n[0] >> 1) & 0x3F == 35, !au.isEmpty { aus.append(au); au = [] }
            au += [0,0,0,1] + Array(n)
        }
        aus.append(au)
        for (i, a) in aus.enumerated() { a.withUnsafeBufferPointer { dec.decode($0, isConfig: false, isKey: i == 0, pts: UInt64(i)) } }
        dec.flush()
    }
    guard let buf = last, let r = VideoRenderer() else { print("\(path): pas d'image"); return }
    let pf = CVPixelBufferGetPixelFormatType(buf)
    guard let layout = VideoRenderer.layout(pf) else { print("\(path): format \(VideoRenderer.fourcc(pf)) non géré"); return }
    let dev = r.commandQueue.device
    let d = MTLTextureDescriptor.texture2DDescriptor(pixelFormat: .bgra8Unorm, width: 128, height: 128, mipmapped: false)
    d.usage = [.renderTarget, .shaderRead]; d.storageMode = .shared
    let tex = dev.makeTexture(descriptor: d)!
    let cb = r.commandQueue.makeCommandBuffer()!
    let ok = r.draw(buf, layout: layout, into: tex, commands: cb)
    cb.commit(); cb.waitUntilCompleted()
    var px = [UInt8](repeating: 0, count: 4)
    tex.getBytes(&px, bytesPerRow: 128*4, from: MTLRegionMake2D(64, 64, 1, 1), mipmapLevel: 0)
    let att = CVBufferCopyAttachment(buf, kCVImageBufferYCbCrMatrixKey, nil) as? String ?? "—"
    print("\(path) [\(VideoRenderer.fourcc(pf)), \(att)] dessin=\(ok) → #\(String(format: "%02X%02X%02X", px[2], px[1], px[0])) attendu #\(expect)")
}
for c in ["3366CC", "FF0000", "FFFFFF", "202020"] { for f in ["yuv420p", "yuv444p"] { checkColor("c_\(c)_\(f).hevc", expect: c) } }
