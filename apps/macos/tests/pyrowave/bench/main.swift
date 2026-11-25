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
// - perte : une image tronquée est sautée sans casser la suivante.
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

print("\((args[1] as NSString).lastPathComponent) — \(config.label), sortie \(format.isEmpty ? "?" : format)")
print(String(format: "  sonde du lancement : %@ en %.0f ms", probed ? "réussie" : "ÉCHOUÉE", probeMs))
print("  cadencé 60 i/s : \(pacedDelivered)/\(frames.count) images, \(stats(paced))")
print(String(format: "  à la chaîne : %d images en %.2f s, %.0f i/s", chained, elapsed, Double(chained) / elapsed))
if !psnr.isEmpty { print("  \(psnr)") }
print("  image tronquée puis entière : \(afterLoss) sortie(s) \(afterLoss == 1 ? "(tronquée sautée, suivante décodée)" : "(ATTENDU : 1)")")
if !probed || pacedDelivered != frames.count || afterLoss != 1 { exit(1) }
