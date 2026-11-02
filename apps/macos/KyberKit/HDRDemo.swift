// Démonstration du rendu HDR, sans Station (Debug : `-demoHDR YES`).
//
// Une mire PQ synthétique (BT.2020, SMPTE ST 2084, Y'CbCr 4:2:0 10 bits
// en plage vidéo, étiquetée comme une image du décodeur) passe par le vrai
// rendu (`VideoRenderer`) : chaîne HDR sur un écran HDR, ramenée en SDR
// ailleurs. Rampe PQ de 0 à 10 000 nits, blancs de 100 à 4 000 nits,
// couleurs saturées BT.2020. Le journal (`video`) dit chaque seconde la
// chaîne et la marge EDR en cours de l'écran : au-dessus de 1, l'écran
// montre bien de l'EDR.

import CoreText
import CoreVideo
import Foundation
import QuartzCore
import SwiftUI
#if os(macOS)
import AppKit
#else
import UIKit
#endif

public final class HDRDemo {
    /// `-demoHDR YES` (Debug seulement).
    public static var requested: Bool {
        #if DEBUG
        return UserDefaults.standard.bool(forKey: "demoHDR")
        #else
        return false
        #endif
    }

    let surface = MetalSurface()
    private let renderer: VideoRenderer
    private let frame: CVPixelBuffer
    private var timer: Timer?
    private var pts: UInt64 = 0
    private var lastReport = 0.0
    private var hdrChain = false
    #if os(tvOS)
    /// Le téléviseur a été prié de passer en HDR (une fois la vue dans sa fenêtre).
    private var askedTV = false
    #endif

    public init?(width: Int = 1920, height: Int = 1080) {
        guard let renderer = VideoRenderer(), let frame = Self.pattern(width: width, height: height) else { return nil }
        self.renderer = renderer
        self.frame = frame
        renderer.label = "mire HDR"
        renderer.onChain = { [weak self] hdr in self?.hdrChain = hdr }
        surface.show(renderer)
        KyberLog.video.notice("mire HDR : \(width)×\(height), PQ BT.2020, 4:2:0 10 bits")
    }

    /// Une image toutes les 33 ms, comme un bureau qui bouge.
    public func start() {
        timer?.invalidate()
        timer = Timer.scheduledTimer(withTimeInterval: 1.0 / 30, repeats: true) { [weak self] _ in self?.tick() }
    }

    public func stop() {
        timer?.invalidate()
        timer = nil
        #if os(tvOS)
        surface.window?.avDisplayManager.preferredDisplayCriteria = nil
        #endif
        surface.show(nil)
    }

    private func tick() {
        #if os(tvOS)
        if !askedTV, let window = surface.window {
            askedTV = true
            window.avDisplayManager.preferredDisplayCriteria = HDRDisplay.criteria(refresh: 60, width: CVPixelBufferGetWidth(frame), height: CVPixelBufferGetHeight(frame), hdr: true)
            KyberLog.video.notice("mire HDR : téléviseur prié de passer en HDR10 (adapter la plage dynamique : \(window.avDisplayManager.isDisplayCriteriaMatchingEnabled ? "oui" : "non", privacy: .public))")
        }
        #endif
        pts += 33_333
        let now = mach_absolute_time()
        renderer.submit(DecodedFrame(buffer: frame, pts: pts, received: now, decoded: now))
        let t = CACurrentMediaTime()
        guard t - lastReport >= 1 else { return }
        lastReport = t
        #if os(tvOS)
        // tvOS ne donne pas de marge EDR : seul le HDR10 du téléviseur se dit.
        KyberLog.video.notice("mire HDR : chaîne \(self.hdrChain ? "HDR" : "SDR", privacy: .public), téléviseur \(self.surface.screenHDR ? "HDR" : "SDR", privacy: .public)")
        #else
        KyberLog.video.notice("mire HDR : chaîne \(self.hdrChain ? "HDR" : "SDR", privacy: .public), écran \(self.surface.screenHDR ? "HDR" : "SDR", privacy: .public), marge EDR en cours \(String(format: "%.2f", self.currentHeadroom), privacy: .public)")
        #endif
    }

    /// Marge EDR en cours de l'écran (au-dessus de 1 : de l'EDR est montré ; 1 sur tvOS, qui ne la donne pas).
    public var currentHeadroom: Double {
        #if os(macOS)
        return Double(surface.window?.screen?.maximumExtendedDynamicRangeColorComponentValue ?? 1)
        #elseif os(iOS)
        return Double((surface.window?.screen ?? UIScreen.main).currentEDRHeadroom)
        #else
        return 1
        #endif
    }

    // MARK: - Mire

    /// PQ (SMPTE ST 2084) d'une luminance en nits.
    static func pq(_ nits: Double) -> Double {
        let m1 = 0.1593017578125, m2 = 78.84375, c1 = 0.8359375, c2 = 18.8515625, c3 = 18.6875
        let y = pow(max(0, min(nits, 10000)) / 10000, m1)
        return pow((c1 + c2 * y) / (1 + c3 * y), m2)
    }

    /// Y'CbCr 10 bits en plage vidéo (BT.2020 non constant) d'une couleur
    /// BT.2020 linéaire, en nits par composante.
    static func ycbcr(_ r: Double, _ g: Double, _ b: Double) -> (y: UInt16, cb: UInt16, cr: UInt16) {
        let (rp, gp, bp) = (pq(r), pq(g), pq(b))
        let y = 0.2627 * rp + 0.6780 * gp + 0.0593 * bp
        let cb = (bp - y) / 1.8814, cr = (rp - y) / 1.4746
        func code(_ v: Double, _ lo: Double, _ span: Double) -> UInt16 { UInt16(max(0, min(1023, (lo + span * v).rounded()))) }
        return (code(y, 64, 876), code(cb, 512, 896), code(cr, 512, 896))
    }

    private struct Band {
        let top: Double
        let bottom: Double
        /// Couleur (nits BT.2020) à la colonne x (0 à 1), et le libellé de chaque case.
        let color: (Double) -> (Double, Double, Double)
        let labels: [(x: Double, text: String)]
    }

    static func pattern(width w: Int, height h: Int) -> CVPixelBuffer? {
        let attrs: [CFString: Any] = [kCVPixelBufferIOSurfacePropertiesKey: [CFString: Any](), kCVPixelBufferMetalCompatibilityKey: true]
        var out: CVPixelBuffer?
        guard CVPixelBufferCreate(kCFAllocatorDefault, w, h, kCVPixelFormatType_420YpCbCr10BiPlanarVideoRange, attrs as CFDictionary, &out) == kCVReturnSuccess,
              let buffer = out else { return nil }
        CVBufferSetAttachment(buffer, kCVImageBufferColorPrimariesKey, kCVImageBufferColorPrimaries_ITU_R_2020, .shouldPropagate)
        CVBufferSetAttachment(buffer, kCVImageBufferTransferFunctionKey, kCVImageBufferTransferFunction_SMPTE_ST_2084_PQ, .shouldPropagate)
        CVBufferSetAttachment(buffer, kCVImageBufferYCbCrMatrixKey, kCVImageBufferYCbCrMatrix_ITU_R_2020, .shouldPropagate)

        // Six cases par bande, de gauche à droite.
        func patches(_ list: [(Double, Double, Double)]) -> (Double) -> (Double, Double, Double) {
            { x in list[min(list.count - 1, Int(x * Double(list.count)))] }
        }
        func centers(_ texts: [String]) -> [(x: Double, text: String)] {
            texts.enumerated().map { (x: (Double($0.offset) + 0.5) / Double(texts.count), text: $0.element) }
        }
        let whites: [Double] = [100, 203, 400, 1000, 2000, 4000]
        let bands = [
            // Rampe PQ, linéaire en code : de 0 à 10 000 nits.
            Band(top: 0.06, bottom: 0.30, color: { x in
                let n = 10000 * pow(max(0, (pow(x, 1 / 78.84375) - 0.8359375)) / (18.8515625 - 18.6875 * pow(x, 1 / 78.84375)), 1 / 0.1593017578125)
                return (n, n, n)
            }, labels: [0, 100, 203, 1000, 4000, 10000].map { (x: pq(Double($0)), text: "\($0)") }),
            Band(top: 0.34, bottom: 0.62, color: patches(whites.map { ($0, $0, $0) }), labels: centers(whites.map { "\(Int($0)) nits" })),
            // Primaires et secondaires BT.2020 saturées, à 600 nits.
            Band(top: 0.66, bottom: 0.90, color: patches([(600, 0, 0), (0, 600, 0), (0, 0, 600), (600, 600, 0), (0, 600, 600), (600, 0, 600)]),
                 labels: centers(["R 2020", "G 2020", "B 2020", "Y", "C", "M"])),
        ]

        // Libellés : un masque en niveaux de gris dessiné par Core Text.
        let mask = labelMask(width: w, height: h, bands: bands,
                             footer: "KyberNow · mire PQ BT.2020 · 4:2:0 10 bits · nits")
        let text203 = ycbcr(203, 203, 203), textBlack = ycbcr(0, 0, 0), black = ycbcr(0, 0, 0)

        CVPixelBufferLockBaseAddress(buffer, [])
        defer { CVPixelBufferUnlockBaseAddress(buffer, []) }
        guard let yBase = CVPixelBufferGetBaseAddressOfPlane(buffer, 0), let cBase = CVPixelBufferGetBaseAddressOfPlane(buffer, 1) else { return nil }
        let yStride = CVPixelBufferGetBytesPerRowOfPlane(buffer, 0), cStride = CVPixelBufferGetBytesPerRowOfPlane(buffer, 1)

        // Une rangée par bande (toutes les rangées d'une bande sont pareilles), puis le texte.
        var rows: [Int: [(y: UInt16, cb: UInt16, cr: UInt16, nits: Double)]] = [:]
        func row(_ band: Int) -> [(y: UInt16, cb: UInt16, cr: UInt16, nits: Double)] {
            if let r = rows[band] { return r }
            let r = (0..<w).map { x -> (y: UInt16, cb: UInt16, cr: UInt16, nits: Double) in
                let c = bands[band].color(Double(x) / Double(w - 1))
                let v = ycbcr(c.0, c.1, c.2)
                return (v.y, v.cb, v.cr, 0.2627 * c.0 + 0.6780 * c.1 + 0.0593 * c.2)
            }
            rows[band] = r
            return r
        }
        for y in 0..<h {
            let fy = Double(y) / Double(h)
            let band = bands.firstIndex { fy >= $0.top && fy < $0.bottom }
            let line = band.map(row)
            let yRow = (yBase + y * yStride).assumingMemoryBound(to: UInt16.self)
            let cRow = (cBase + (y / 2) * cStride).assumingMemoryBound(to: UInt16.self)
            for x in 0..<w {
                var v = line.map { ($0[x].y, $0[x].cb, $0[x].cr) } ?? (black.y, black.cb, black.cr)
                if mask[y * w + x] > 127 {
                    let bright = (line?[x].nits ?? 0) > 150
                    let t = bright ? textBlack : text203
                    v = (t.y, t.cb, t.cr)
                }
                // 10 bits calés en haut des 16 bits (« x420 »).
                yRow[x] = v.0 << 6
                if y % 2 == 0, x % 2 == 0 {
                    cRow[x] = v.1 << 6
                    cRow[x + 1] = v.2 << 6
                }
            }
        }
        return buffer
    }

    /// Masque des libellés (255 : texte), origine en haut à gauche.
    private static func labelMask(width w: Int, height h: Int, bands: [Band], footer: String) -> [UInt8] {
        var bytes = [UInt8](repeating: 0, count: w * h)
        bytes.withUnsafeMutableBytes { raw in
            guard let ctx = CGContext(data: raw.baseAddress, width: w, height: h, bitsPerComponent: 8, bytesPerRow: w,
                                      space: CGColorSpaceCreateDeviceGray(), bitmapInfo: CGImageAlphaInfo.none.rawValue) else { return }
            ctx.setFillColor(gray: 1, alpha: 1)
            let size = CGFloat(h) / 36
            let font = CTFontCreateWithName("Helvetica-Bold" as CFString, size, nil)
            func draw(_ text: String, x: CGFloat, top: CGFloat, center: Bool) {
                let attributed = NSAttributedString(string: text, attributes: [
                    NSAttributedString.Key(kCTFontAttributeName as String): font,
                    NSAttributedString.Key(kCTForegroundColorFromContextAttributeName as String): true,
                ])
                let line = CTLineCreateWithAttributedString(attributed)
                let width = CTLineGetTypographicBounds(line, nil, nil, nil)
                // Le contexte a son origine en bas : `top` est compté depuis le haut.
                ctx.textPosition = CGPoint(x: center ? x - CGFloat(width) / 2 : x, y: CGFloat(h) - top - size)
                CTLineDraw(line, ctx)
            }
            for band in bands {
                for label in band.labels {
                    let x = min(max(CGFloat(label.x) * CGFloat(w), size * 2), CGFloat(w) - size * 2)
                    draw(label.text, x: x, top: CGFloat(band.bottom) * CGFloat(h) - size * 1.6, center: true)
                }
            }
            draw(footer, x: size, top: CGFloat(h) * 0.93, center: false)
        }
        return bytes
    }
}

#if os(macOS)
/// La mire dans une vue SwiftUI (fenêtre de démonstration).
public struct HDRDemoView: NSViewRepresentable {
    let demo: HDRDemo
    public init(demo: HDRDemo) { self.demo = demo }
    public func makeNSView(context: Context) -> NSView {
        demo.start()
        return demo.surface
    }
    public func updateNSView(_ view: NSView, context: Context) {}
}
#else
public struct HDRDemoView: UIViewRepresentable {
    let demo: HDRDemo
    public init(demo: HDRDemo) { self.demo = demo }
    public func makeUIView(context: Context) -> UIView {
        demo.start()
        return demo.surface
    }
    public func updateUIView(_ view: UIView, context: Context) {}
}
#endif
