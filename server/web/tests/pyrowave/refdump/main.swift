// Décode une image d'un fichier d'essai PyroWave avec le décodeur Metal du
// port (apps/macos/PyroWave, la référence) et écrit ses plans Y, Cb, Cr en
// float32 (valeurs PyroWave, DC déjà ajouté : 0 à 1), à la suite.
//
//   refdump <entrée.pyrowave> <sortie.f32> [image]
import Foundation
import Metal
@_implementationOnly import CPyroWave

let a = CommandLine.arguments
guard a.count >= 3 else { print("usage : refdump <entrée.pyrowave> <sortie.f32> [image]"); exit(2) }
let index = a.count > 3 ? Int(a[3])! : 0
let (config, frames) = readPyroWaveFile(a[1])
func u32(_ o: Int) -> Int { Int(config[o]) | Int(config[o + 1]) << 8 | Int(config[o + 2]) << 16 | Int(config[o + 3]) << 24 }
let chroma444 = config[5] == 1
let width = u32(8), height = u32(12)
let cw = chroma444 ? width : width / 2, ch = chroma444 ? height : height / 2

var device: pyrowave_device?
guard pyrowave_create_default_device(&device) == PYROWAVE_SUCCESS, let device else { fatalError("device PyroWave") }
var info = pyrowave_decoder_create_info()
info.device = device
info.width = Int32(width)
info.height = Int32(height)
info.chroma = chroma444 ? PYROWAVE_CHROMA_SUBSAMPLING_444 : PYROWAVE_CHROMA_SUBSAMPLING_420
var decoder: pyrowave_decoder?
guard pyrowave_decoder_create(&info, &decoder) == PYROWAVE_SUCCESS, let decoder else { fatalError("décodeur") }

let mtl = MTLCreateSystemDefaultDevice()!
func plane(_ w: Int, _ h: Int) -> MTLTexture {
    let d = MTLTextureDescriptor.texture2DDescriptor(pixelFormat: .r32Float, width: w, height: h, mipmapped: false)
    d.usage = [.shaderRead, .shaderWrite]
    d.storageMode = .shared
    return mtl.makeTexture(descriptor: d)!
}
let planes = [plane(width, height), plane(cw, ch), plane(cw, ch)]
let queue = mtl.makeCommandQueue()!
pyrowave_decoder_clear(decoder)
let frame = frames[index]
guard frame.withUnsafeBytes({ pyrowave_decoder_push_packet(decoder, $0.baseAddress, $0.count) }) == PYROWAVE_SUCCESS else { fatalError("paquet") }
guard pyrowave_decoder_decode_is_ready(decoder, true) else { fatalError("image incomplète") }
let cmd = queue.makeCommandBuffer()!
var buffers = pyrowave_gpu_buffers()
buffers.planes = (Unmanaged.passUnretained(planes[0]).toOpaque(), Unmanaged.passUnretained(planes[1]).toOpaque(), Unmanaged.passUnretained(planes[2]).toOpaque())
guard pyrowave_decoder_decode_gpu_buffer(decoder, Unmanaged.passUnretained(cmd).toOpaque(), &buffers) == PYROWAVE_SUCCESS else { fatalError("décodage") }
cmd.commit()
cmd.waitUntilCompleted()
var out = Data()
for (i, t) in planes.enumerated() {
    let w = i == 0 ? width : cw, h = i == 0 ? height : ch
    var buf = [Float](repeating: 0, count: w * h)
    buf.withUnsafeMutableBytes { t.getBytes($0.baseAddress!, bytesPerRow: w * 4, from: MTLRegionMake2D(0, 0, w, h), mipmapLevel: 0) }
    buf.withUnsafeBytes { out.append(contentsOf: $0) }
}
try! out.write(to: URL(fileURLWithPath: a[2]))
print("\(width)x\(height) \(chroma444 ? "4:4:4" : "4:2:0"), image \(index) de \(frames.count), \(frame.count) octets → \(a[2])")
