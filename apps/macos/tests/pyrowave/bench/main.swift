// Banc de décodage PyroWave : un fichier d'essai (tools encode) passe par
// VideoDecoder (format « pwv1 », paquet de configuration puis une image par
// paquet), comme dans une session, vers le décodeur Metal de KyberKit.
//
//   bench <fichier.pyrowave> [source.yuv]
//
// - cadencé : une image toutes les 16,7 ms (60 i/s) ; temps de décodage =
//   de la remise du paquet à la fin du GPU (ce que montrent les mesures de
//   la session) : moyenne, médiane, 95e centile, maximum ;
// - à la chaîne : une image dès la précédente sortie ; images par seconde ;
// - fidélité : PSNR de la première image décodée contre la source (Y, Cb, Cr) ;
// - perte : une image tronquée est sautée sans casser la suivante ;
// - en-tête KNPW version 2 (KYB-29) : lu, et ses métadonnées HDR10 sur les
//   images décodées en HDR.
import CoreVideo
import Foundation

let args = CommandLine.arguments
let (configBytes, frames) = readPyroWaveFile(args[1])
let config = configBytes.withUnsafeBufferPointer { PyroWaveConfig($0) }!
let source: Data? = args.count > 2 ? try? Data(contentsOf: URL(fileURLWithPath: args[2]), options: .alwaysMapped) : nil

// Sonde du lancement (DecoderCapabilities) : device PyroWave (pipelines
// compilés) puis une image de test, une fois par processus.
let probeStart = CFAbsoluteTimeGetCurrent()
let probed = PyroWaveDecoder.probe()
let probeMs = (CFAbsoluteTimeGetCurrent() - probeStart) * 1000

let decoder = VideoDecoder()
decoder.setCodec(0x7077_7631)
configBytes.withUnsafeBufferPointer { decoder.decode($0, isConfig: true, isKey: true, pts: 0) }

let lock = NSLock()
var latencies: [Double] = []
var delivered = 0
var psnr = ""
var format = ""
let done = DispatchSemaphore(value: 0)

/// PSNR (8 bits) d'un plan décodé contre la source, composante `c` d'un plan entrelacé de `stride` composantes.
func planePSNR(_ buffer: CVPixelBuffer, plane: Int, component: Int, of comps: Int, source: UnsafePointer<UInt8>, deep: Bool) -> Double {
    let w = CVPixelBufferGetWidthOfPlane(buffer, plane), h = CVPixelBufferGetHeightOfPlane(buffer, plane)
    let row = CVPixelBufferGetBytesPerRowOfPlane(buffer, plane)
    let base = CVPixelBufferGetBaseAddressOfPlane(buffer, plane)!
    var se = 0.0
    for y in 0..<h {
        for x in 0..<w {
            let v: Double
            if deep {
                v = Double(base.load(fromByteOffset: y * row + (x * comps + component) * 2, as: UInt16.self)) / 65535 * 255
            } else {
                v = Double(base.load(fromByteOffset: y * row + x * comps + component, as: UInt8.self))
            }
            let d = v - Double(source[y * w + x])
            se += d * d
        }
    }
    let mse = se / Double(w * h)
    return mse == 0 ? 99 : 10 * log10(255 * 255 / mse)
}

decoder.onFrame = { frame in
    let latency = ms(frame.decoded - frame.received)
    lock.lock()
    latencies.append(latency)
    delivered += 1
    lock.unlock()
    if frame.pts == 1, let source {
        let b = frame.buffer
        let p = CVPixelBufferGetPixelFormatType(b)
        format = String(bytes: [24, 16, 8, 0].map { UInt8((p >> $0) & 0xFF) }, encoding: .ascii)!
        CVPixelBufferLockBaseAddress(b, .readOnly)
        source.withUnsafeBytes { raw in
            let s = raw.bindMemory(to: UInt8.self).baseAddress!
            let luma = config.width * config.height
            let chroma = config.chromaWidth * config.chromaHeight
            let y = planePSNR(b, plane: 0, component: 0, of: 1, source: s, deep: config.hdr)
            let cb = planePSNR(b, plane: 1, component: 0, of: 2, source: s + luma, deep: config.hdr)
            let cr = planePSNR(b, plane: 1, component: 1, of: 2, source: s + luma + chroma, deep: config.hdr)
            psnr = String(format: "PSNR Y %.1f dB, Cb %.1f dB, Cr %.1f dB", y, cb, cr)
        }
        CVPixelBufferUnlockBaseAddress(b, .readOnly)
    }
}

func stats(_ v: [Double]) -> String {
    let s = v.sorted()
    guard !s.isEmpty else { return "aucune image" }
    let mean = s.reduce(0, +) / Double(s.count)
    return String(format: "moyenne %.2f ms, médiane %.2f, 95e centile %.2f, max %.2f", mean, s[s.count / 2], s[min(s.count - 1, s.count * 95 / 100)], s.last!)
}

// Échauffement (pipelines, pool de tampons) : 10 images, non comptées.
for i in 0..<min(10, frames.count) {
    frames[i].withUnsafeBufferPointer { decoder.decode($0, isConfig: false, isKey: true, pts: UInt64(100_000 + i)) }
    decoder.flush()
}
lock.lock(); latencies.removeAll(); delivered = 0; lock.unlock()

// Cadencé, 60 i/s. Les pts repartent de 1 (garde de monotonie : on remet le décodeur).
decoder.setCodec(0x6832_3635)
decoder.setCodec(0x7077_7631)
configBytes.withUnsafeBufferPointer { decoder.decode($0, isConfig: true, isKey: true, pts: 0) }
let period = 1.0 / 60
var next = CFAbsoluteTimeGetCurrent()
for (i, f) in frames.enumerated() {
    f.withUnsafeBufferPointer { decoder.decode($0, isConfig: false, isKey: true, pts: UInt64(i + 1)) }
    next += period
    let wait = next - CFAbsoluteTimeGetCurrent()
    if wait > 0 { usleep(UInt32(wait * 1e6)) }
}
decoder.flush()
lock.lock()
let paced = latencies
let pacedDelivered = delivered
latencies.removeAll(); delivered = 0
lock.unlock()

// À la chaîne.
let start = CFAbsoluteTimeGetCurrent()
for (i, f) in frames.enumerated() {
    f.withUnsafeBufferPointer { decoder.decode($0, isConfig: false, isKey: true, pts: UInt64(10_000 + i)) }
    decoder.flush()
}
let elapsed = CFAbsoluteTimeGetCurrent() - start
lock.lock()
let chained = delivered
lock.unlock()

// Perte : une image tronquée (moitié de ses octets) est sautée, la suivante sort.
lock.lock(); delivered = 0; lock.unlock()
let cut = Array(frames[0].prefix(frames[0].count / 2))
cut.withUnsafeBufferPointer { decoder.decode($0, isConfig: false, isKey: true, pts: 20_000) }
frames[1].withUnsafeBufferPointer { decoder.decode($0, isConfig: false, isKey: true, pts: 20_001) }
decoder.flush()
lock.lock()
let afterLoss = delivered
lock.unlock()

// En-tête KNPW version 2 (KYB-29) : le même paquet, 48 octets, avec les
// métadonnées HDR10 (primaires de Display P3, 1 000 nits, MaxCLL 1 000,
// MaxFALL 400) ; en HDR, les images décodées les portent au format de
// CoreVideo, vert d'abord. Le paquet v1 reste lu (plus haut).
var v2 = configBytes
v2[4] = 2
v2 += [3, 0, 0, 0]
func le16(_ v: UInt16) -> [UInt8] { [UInt8(v & 0xFF), UInt8(v >> 8)] }
for v: UInt16 in [34000, 16000, 13250, 34500, 7500, 3000, 15635, 16450] { v2 += le16(v) }
v2 += [0x80, 0x96, 0x98, 0x00] // 10 000 000 (1 000 cd/m²)
v2 += [1, 0, 0, 0]
v2 += le16(1000) + le16(400)
var metadata = "—"
let v2Config = v2.withUnsafeBufferPointer { PyroWaveConfig($0) }
lock.lock(); delivered = 0; lock.unlock()
let mastering = NSLock()
var seen: Data?
decoder.onFrame = { frame in
    mastering.lock()
    seen = CVBufferCopyAttachment(frame.buffer, kCVImageBufferMasteringDisplayColorVolumeKey, nil) as? Data
    mastering.unlock()
}
v2.withUnsafeBufferPointer { decoder.decode($0, isConfig: true, isKey: true, pts: 0) }
frames[2].withUnsafeBufferPointer { decoder.decode($0, isConfig: false, isKey: true, pts: 30_000) }
decoder.flush()
mastering.lock()
let attached = seen
mastering.unlock()
let expected = Data([0x33, 0xC2, 0x86, 0xC4, 0x1D, 0x4C, 0x0B, 0xB8, 0x84, 0xD0, 0x3E, 0x80, 0x3D, 0x13, 0x40, 0x42,
                     0x00, 0x98, 0x96, 0x80, 0x00, 0x00, 0x00, 0x01])
let v2Ok = v2Config != nil && (config.hdr ? attached == expected : attached == nil)
metadata = v2Config == nil ? "ILLISIBLE" : config.hdr ? (attached == expected ? "écran de mastering sur l'image" : "ATTENDU : écran de mastering sur l'image") : (attached == nil ? "lu, rien en SDR" : "ATTENDU : rien en SDR")

print("\((args[1] as NSString).lastPathComponent) — \(config.label), sortie \(format.isEmpty ? "?" : format)")
print(String(format: "  sonde du lancement : %@ en %.0f ms", probed ? "réussie" : "ÉCHOUÉE", probeMs))
print("  cadencé 60 i/s : \(pacedDelivered)/\(frames.count) images, \(stats(paced))")
print(String(format: "  à la chaîne : %d images en %.2f s, %.0f i/s", chained, elapsed, Double(chained) / elapsed))
if !psnr.isEmpty { print("  \(psnr)") }
print("  image tronquée puis entière : \(afterLoss) sortie(s) \(afterLoss == 1 ? "(tronquée sautée, suivante décodée)" : "(ATTENDU : 1)")")
print("  en-tête KNPW version 2 : \(metadata)")
if !probed || pacedDelivered != frames.count || afterLoss != 1 || !v2Ok { exit(1) }
