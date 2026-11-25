// Banc PyroWave sur l'appareil, d'après l'essai du Mac
// (apps/macos/tests/pyrowave) : pas de ffmpeg ici, la mire est dessinée sur
// place (rampe qui défile, damier fin, barres de couleur, plage réduite
// BT.709), encodée par l'encodeur Metal du port à 1,2 bit par pixel à 60 i/s
// (une image = ses paquets bout à bout, comme un paquet média de la
// Station), puis décodée par VideoDecoder (« pwv1 ») → PyroWaveDecoder.
//
// Pour chaque cas : sonde du lancement, temps de décodage cadencé à 60 i/s
// (de la remise du paquet à la fin du GPU : moyenne, médiane, 95e centile,
// maximum), images par seconde à la chaîne, PSNR de la première image, image
// tronquée sautée sans casser la suivante. Définitions : argument
// `-sizes 1920x1080,3840x2160` (par défaut 1080p et 4K, et 1440p sur
// iPhone et iPad).
import CoreVideo
import Foundation
import Metal
@_implementationOnly import CPyroWave

enum Bench {
    static let frameCount = 60

    static func run(_ say: @escaping (String) -> Void) -> Bool {
        let gpu = MTLCreateSystemDefaultDevice()
        say("GPU : \(gpu?.name ?? "aucun") · Apple7 : \(gpu?.supportsFamily(.apple7) == true ? "oui" : "non")")
        let start = CFAbsoluteTimeGetCurrent()
        let probed = PyroWaveDecoder.probe()
        say(String(format: "Sonde : PyroWave %@ (%.0f ms)", probed ? "disponible" : "indisponible", (CFAbsoluteTimeGetCurrent() - start) * 1000))
        guard probed else { return false }

        var sizes: [(Int, Int)] = [(1920, 1080), (3840, 2160)]
        #if os(iOS)
        sizes.insert((2560, 1440), at: 1)
        #endif
        if let arg = UserDefaults.standard.string(forKey: "sizes") {
            sizes = arg.split(separator: ",").compactMap { s in
                let p = s.split(separator: "x").compactMap { Int($0) }
                return p.count == 2 ? (p[0], p[1]) : nil
            }
        }
        var ok = true
        for (w, h) in sizes {
            for chroma444 in [false, true] {
                guard let file = encode(width: w, height: h, chroma444: chroma444, say: say) else { ok = false; continue }
                for hdr in [false, true] {
                    ok = decode(file, hdr: hdr, say: say) && ok
                }
            }
        }
        return ok
    }

    // MARK: Mire et encodage

    struct Encoded {
        var width: Int, height: Int, chroma444: Bool
        var frames: [[UInt8]]
        /// La première image source (Y, Cb, Cr à la suite), pour le PSNR.
        var source: [UInt8]
    }

    /// Une image de la mire (plage réduite, 8 bits) : rampe de Y qui défile,
    /// damier d'un pixel dans le quart du bas, barres de couleur en chroma.
    static func draw(_ buf: inout [UInt8], frame: Int, width w: Int, height h: Int, cw: Int, ch: Int) {
        buf.withUnsafeMutableBufferPointer { p in
            for y in 0..<h {
                let row = y * w
                let checker = y > h * 3 / 4
                for x in 0..<w {
                    p[row + x] = checker
                        ? ((x + y) & 1 == 0 ? 40 : 200)
                        : UInt8(16 + ((x + frame * 8) % w) * 219 / w)
                }
            }
            let cb = w * h, cr = cb + cw * ch
            let bars: [(UInt8, UInt8)] = [(128, 128), (44, 136), (156, 44), (72, 58), (184, 198), (212, 120), (100, 212), (128, 128)]
            for y in 0..<ch {
                for x in 0..<cw {
                    let b = bars[x * bars.count / cw]
                    p[cb + y * cw + x] = b.0
                    p[cr + y * cw + x] = b.1
                }
            }
        }
    }

    static func encode(width w: Int, height h: Int, chroma444: Bool, say: (String) -> Void) -> Encoded? {
        let cw = chroma444 ? w : w / 2, ch = chroma444 ? h : h / 2
        var device: pyrowave_device?
        guard pyrowave_create_default_device(&device) == PYROWAVE_SUCCESS, let device else { say("Encodeur : pas de device PyroWave"); return nil }
        defer { pyrowave_device_destroy(device) }
        var info = pyrowave_encoder_create_info()
        info.device = device
        info.width = Int32(w)
        info.height = Int32(h)
        info.chroma = chroma444 ? PYROWAVE_CHROMA_SUBSAMPLING_444 : PYROWAVE_CHROMA_SUBSAMPLING_420
        var encoder: pyrowave_encoder?
        guard pyrowave_encoder_create(&info, &encoder) == PYROWAVE_SUCCESS, let encoder else { say("Encodeur refusé (\(w)x\(h))"); return nil }
        defer { pyrowave_encoder_destroy(encoder) }

        let budget = w * h * 12 / 10 / 8
        var rate = pyrowave_rate_control(maximum_bitstream_size: budget)
        var scratch = [UInt8](repeating: 0, count: budget + 65536)
        var packets = [pyrowave_packet](repeating: pyrowave_packet(), count: 1)
        var image = [UInt8](repeating: 0, count: w * h + 2 * cw * ch)
        var out = Encoded(width: w, height: h, chroma444: chroma444, frames: [], source: [])
        var total = 0
        let started = CFAbsoluteTimeGetCurrent()
        for i in 0..<frameCount {
            draw(&image, frame: i, width: w, height: h, cw: cw, ch: ch)
            if i == 0 { out.source = image }
            let r = image.withUnsafeMutableBytes { raw -> pyrowave_result in
                let base = raw.baseAddress!
                var buf = pyrowave_cpu_buffer()
                buf.data = (base, base + w * h, base + w * h + cw * ch)
                buf.row_stride_in_bytes = (w, cw, cw)
                buf.plane_size_in_bytes = (w * h, cw * ch, cw * ch)
                buf.width = Int32(w)
                buf.height = Int32(h)
                buf.format = chroma444 ? PYROWAVE_CPU_BUFFER_FORMAT_YUV444P : PYROWAVE_CPU_BUFFER_FORMAT_YUV420P
                return pyrowave_encoder_encode_cpu(encoder, &buf, &rate)
            }
            guard r == PYROWAVE_SUCCESS else { say("Encodage refusé à l'image \(i)"); return nil }
            let boundary = 65536
            var count = 0
            pyrowave_encoder_compute_num_packets(encoder, boundary, &count)
            if packets.count < count { packets = [pyrowave_packet](repeating: pyrowave_packet(), count: count) }
            var written = 0
            let pr = scratch.withUnsafeMutableBytes { s in
                pyrowave_encoder_packetize(encoder, &packets, boundary, &written, s.baseAddress, s.count)
            }
            guard pr == PYROWAVE_SUCCESS else { say("Paquets refusés à l'image \(i)"); return nil }
            var frame: [UInt8] = []
            for p in packets.prefix(written) { frame.append(contentsOf: scratch[p.offset..<(p.offset + p.size)]) }
            total += frame.count
            out.frames.append(frame)
        }
        let avg = Double(total) / Double(frameCount)
        say(String(format: "Encodé : %d images %dx%d %@, %.0f Kio par image (%.0f Mbit/s à 60 i/s), en %.1f s",
                   frameCount, w, h, chroma444 ? "4:4:4" : "4:2:0", avg / 1024, avg * 8 * 60 / 1e6, CFAbsoluteTimeGetCurrent() - started))
        return out
    }

    // MARK: Décodage

    static func ms(_ ticks: UInt64) -> Double {
        var info = mach_timebase_info_data_t()
        mach_timebase_info(&info)
        return Double(ticks) * Double(info.numer) / Double(info.denom) / 1e6
    }

    static func stats(_ v: [Double]) -> String {
        let s = v.sorted()
        guard !s.isEmpty else { return "aucune image" }
        let mean = s.reduce(0, +) / Double(s.count)
        return String(format: "moyenne %.2f ms, médiane %.2f, 95e centile %.2f, max %.2f", mean, s[s.count / 2], s[min(s.count - 1, s.count * 95 / 100)], s.last!)
    }

    /// PSNR (8 bits) d'un plan décodé contre la source, composante `component` de `comps`.
    static func planePSNR(_ buffer: CVPixelBuffer, plane: Int, component: Int, of comps: Int, source: UnsafePointer<UInt8>, deep: Bool) -> Double {
        let w = CVPixelBufferGetWidthOfPlane(buffer, plane), h = CVPixelBufferGetHeightOfPlane(buffer, plane)
        let row = CVPixelBufferGetBytesPerRowOfPlane(buffer, plane)
        let base = CVPixelBufferGetBaseAddressOfPlane(buffer, plane)!
        var se = 0.0
        for y in 0..<h {
            for x in 0..<w {
                let v: Double = deep
                    ? Double(base.load(fromByteOffset: y * row + (x * comps + component) * 2, as: UInt16.self)) / 65535 * 255
                    : Double(base.load(fromByteOffset: y * row + x * comps + component, as: UInt8.self))
                let d = v - Double(source[y * w + x])
                se += d * d
            }
        }
        let mse = se / Double(w * h)
        return mse == 0 ? 99 : 10 * log10(255 * 255 / mse)
    }

    static func decode(_ file: Encoded, hdr: Bool, say: (String) -> Void) -> Bool {
        let config = PyroWaveConfig(chroma444: file.chroma444, hdr: hdr, fullRange: false, width: file.width, height: file.height)
        let configBytes = config.bytes
        let frames = file.frames
        let decoder = VideoDecoder()
        decoder.setCodec(0x7077_7631)
        configBytes.withUnsafeBufferPointer { decoder.decode($0, isConfig: true, isKey: true, pts: 0) }

        let lock = NSLock()
        var latencies: [Double] = []
        var delivered = 0
        var psnr = ""
        var format = ""
        decoder.onFrame = { frame in
            let latency = ms(frame.decoded - frame.received)
            lock.lock()
            latencies.append(latency)
            delivered += 1
            lock.unlock()
            guard frame.pts == 1 else { return }
            let b = frame.buffer
            let p = CVPixelBufferGetPixelFormatType(b)
            let f = String(bytes: [24, 16, 8, 0].map { UInt8((p >> $0) & 0xFF) }, encoding: .ascii) ?? "?"
            CVPixelBufferLockBaseAddress(b, .readOnly)
            let text = file.source.withUnsafeBufferPointer { s -> String in
                let base = s.baseAddress!
                let luma = config.width * config.height
                let chroma = config.chromaWidth * config.chromaHeight
                let y = planePSNR(b, plane: 0, component: 0, of: 1, source: base, deep: hdr)
                let cb = planePSNR(b, plane: 1, component: 0, of: 2, source: base + luma, deep: hdr)
                let cr = planePSNR(b, plane: 1, component: 1, of: 2, source: base + luma + chroma, deep: hdr)
                return String(format: "PSNR Y %.1f dB, Cb %.1f dB, Cr %.1f dB", y, cb, cr)
            }
            CVPixelBufferUnlockBaseAddress(b, .readOnly)
            lock.lock()
            psnr = text
            format = f
            lock.unlock()
        }

        // Échauffement : 10 images, non comptées.
        for i in 0..<min(10, frames.count) {
            frames[i].withUnsafeBufferPointer { decoder.decode($0, isConfig: false, isKey: true, pts: UInt64(100_000 + i)) }
            decoder.flush()
        }
        lock.lock(); latencies.removeAll(); delivered = 0; lock.unlock()

        // Cadencé, 60 i/s (pts depuis 1 : on remet le décodeur).
        decoder.setCodec(0x6832_3635)
        decoder.setCodec(0x7077_7631)
        configBytes.withUnsafeBufferPointer { decoder.decode($0, isConfig: true, isKey: true, pts: 0) }
        var next = CFAbsoluteTimeGetCurrent()
        for (i, f) in frames.enumerated() {
            f.withUnsafeBufferPointer { decoder.decode($0, isConfig: false, isKey: true, pts: UInt64(i + 1)) }
            next += 1.0 / 60
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
        delivered = 0
        lock.unlock()

        // Perte : une image tronquée (moitié de ses octets) est sautée, la suivante sort.
        let cut = Array(frames[0].prefix(frames[0].count / 2))
        cut.withUnsafeBufferPointer { decoder.decode($0, isConfig: false, isKey: true, pts: 20_000) }
        frames[1].withUnsafeBufferPointer { decoder.decode($0, isConfig: false, isKey: true, pts: 20_001) }
        decoder.flush()
        lock.lock()
        let afterLoss = delivered
        let (psnrText, formatText) = (psnr, format)
        lock.unlock()
        decoder.invalidate()

        say("\(config.label)\(hdr ? " HDR10" : " SDR"), sortie \(formatText.isEmpty ? "?" : formatText)")
        say("  cadencé 60 i/s : \(pacedDelivered)/\(frames.count) images, \(stats(paced))")
        say(String(format: "  à la chaîne : %d images en %.2f s, %.0f i/s", chained, elapsed, Double(chained) / max(elapsed, 0.001)))
        if !psnrText.isEmpty { say("  \(psnrText)") }
        say("  image tronquée puis entière : \(afterLoss) sortie(s) \(afterLoss == 1 ? "(tronquée sautée, suivante décodée)" : "(ATTENDU : 1)")")
        return pacedDelivered == frames.count && afterLoss == 1
    }
}
