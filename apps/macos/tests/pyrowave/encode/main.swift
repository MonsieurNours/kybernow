// Encode une vidéo brute (ffmpeg -f rawvideo, yuv420p ou yuv444p 8 bits)
// en PyroWave avec l'encodeur Metal du port (apps/macos/PyroWave), dans le
// format de l'essai : « KNPW » de 16 octets (le paquet de configuration du
// contrat), puis pour chaque image sa taille (u32 LE) et l'image (paquets
// PyroWave bout à bout, comme un paquet média de la Station).
//
//   encode <entrée.yuv> <sortie.pyrowave> <largeur> <hauteur> <420|444> <Mbit/s> [hdr]
//
// `hdr` n'étiquette que le paquet de configuration (sortie 10 bits du
// décodeur, BT.2020 PQ) : la source reste 8 bits, l'essai mesure le décodage.
import Foundation
import Metal
@_implementationOnly import CPyroWave

let a = CommandLine.arguments
guard a.count >= 7, let width = Int(a[3]), let height = Int(a[4]), let mbps = Double(a[6]) else {
    print("usage : encode <entrée.yuv> <sortie.pyrowave> <largeur> <hauteur> <420|444> <Mbit/s> [hdr]")
    exit(2)
}
let chroma444 = a[5] == "444"
let hdr = a.count > 7 && a[7] == "hdr"
let fps = 60.0
let input = try! Data(contentsOf: URL(fileURLWithPath: a[1]), options: .alwaysMapped)
let cw = chroma444 ? width : width / 2, ch = chroma444 ? height : height / 2
let frameSize = width * height + 2 * cw * ch
let frames = input.count / frameSize

var device: pyrowave_device?
guard pyrowave_create_default_device(&device) == PYROWAVE_SUCCESS, let device else { fatalError("device PyroWave") }
var info = pyrowave_encoder_create_info()
info.device = device
info.width = Int32(width)
info.height = Int32(height)
info.chroma = chroma444 ? PYROWAVE_CHROMA_SUBSAMPLING_444 : PYROWAVE_CHROMA_SUBSAMPLING_420
var encoder: pyrowave_encoder?
guard pyrowave_encoder_create(&info, &encoder) == PYROWAVE_SUCCESS, let encoder else { fatalError("encodeur PyroWave") }

var out = Data(PyroWaveConfig(chroma444: chroma444, hdr: hdr, fullRange: false, width: width, height: height).bytes)
let budget = Int(mbps * 1_000_000 / 8 / fps)
var rate = pyrowave_rate_control(maximum_bitstream_size: budget)
var total = 0
var scratch = [UInt8](repeating: 0, count: budget + 65536)
var packets = [pyrowave_packet](repeating: pyrowave_packet(), count: 1)
input.withUnsafeBytes { raw in
    let base = raw.baseAddress!
    for i in 0..<frames {
        let f = base + i * frameSize
        var buf = pyrowave_cpu_buffer()
        buf.data = (UnsafeMutableRawPointer(mutating: f), UnsafeMutableRawPointer(mutating: f + width * height),
                    UnsafeMutableRawPointer(mutating: f + width * height + cw * ch))
        buf.row_stride_in_bytes = (width, cw, cw)
        buf.plane_size_in_bytes = (width * height, cw * ch, cw * ch)
        buf.width = Int32(width)
        buf.height = Int32(height)
        buf.format = chroma444 ? PYROWAVE_CPU_BUFFER_FORMAT_YUV444P : PYROWAVE_CPU_BUFFER_FORMAT_YUV420P
        guard pyrowave_encoder_encode_cpu(encoder, &buf, &rate) == PYROWAVE_SUCCESS else { fatalError("image \(i)") }
        // Paquets de 64 Kio au plus, mis bout à bout : une image = un paquet média.
        let boundary = 65536
        var count = 0
        pyrowave_encoder_compute_num_packets(encoder, boundary, &count)
        if packets.count < count { packets = [pyrowave_packet](repeating: pyrowave_packet(), count: count) }
        var written = 0
        let r = scratch.withUnsafeMutableBytes { s in
            pyrowave_encoder_packetize(encoder, &packets, boundary, &written, s.baseAddress, s.count)
        }
        guard r == PYROWAVE_SUCCESS else { fatalError("paquets de l'image \(i)") }
        var frame = Data()
        for p in packets.prefix(written) { frame.append(contentsOf: scratch[p.offset..<(p.offset + p.size)]) }
        var size = UInt32(frame.count).littleEndian
        withUnsafeBytes(of: &size) { out.append(contentsOf: $0) }
        out.append(frame)
        total += frame.count
    }
}
pyrowave_encoder_destroy(encoder)
pyrowave_device_destroy(device)
try! out.write(to: URL(fileURLWithPath: a[2]))
let avg = Double(total) / Double(max(frames, 1))
print(String(format: "%@ : %d images %dx%d %@, %.0f Kio par image en moyenne (%.0f Mbit/s à 60 i/s)",
             (a[2] as NSString).lastPathComponent, frames, width, height, chroma444 ? "4:4:4" : "4:2:0",
             avg / 1024, avg * 8 * fps / 1e6))
