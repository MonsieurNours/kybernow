// Les mires du réglage du HDR (docs/hdr.md, « Réglage du HDR »), dessinées
// en HDR par l'app, sans ordinateur : couche Metal en `rgba16Float`, espace
// `itur_2100_PQ`, contenu EDR, et **sans** `edrMetadata` : le système ne
// fait alors aucun tone mapping, une valeur au-delà de ce que l'écran
// affiche est écrêtée, et c'est cet écrêtage qu'on cherche à l'œil.
//
// 1. Luminosité maximale : une petite zone à 10 000 nits (écrêtée au maximum
//    de l'écran), un symbole à la valeur réglée, à peine plus sombre ; il
//    disparaît quand la valeur atteint le maximum de l'écran.
// 2. Luminosité sur tout l'écran : la même mire, en plein écran.
// 3. Noir : un symbole à la valeur réglée (0 à 1 nit) sur du noir ; il
//    disparaît quand l'écran ne sait plus le distinguer du noir.
// 4. Blanc du bureau : une fenêtre blanche à la valeur réglée, son texte et
//    son bureau, comme sur l'ordinateur.
//
// Apple TV : le téléviseur est prié de passer en HDR10 tant que la mire est
// à l'écran (`AVDisplayManager`), puis retrouve les critères d'avant.

#if os(tvOS)
import AVKit
#endif
import Metal
import QuartzCore
import SwiftUI
#if os(macOS)
import AppKit
#else
import UIKit
#endif

/// La vue qui dessine la mire d'une étape.
final class HDRPatternSurface: PlatformPatternView {
    private let metal = CAMetalLayer()
    private var step = HDRCalibrationModel.Step.peak
    private var value = 1000.0
    private static let device = MTLCreateSystemDefaultDevice()
    private static let queue = device?.makeCommandQueue()
    private static let pipeline: MTLRenderPipelineState? = {
        guard let device else { return nil }
        do {
            let library = try device.makeLibrary(source: shader, options: nil)
            let desc = MTLRenderPipelineDescriptor()
            desc.vertexFunction = library.makeFunction(name: "kn_pattern_vertex")
            desc.fragmentFunction = library.makeFunction(name: "kn_pattern_fragment")
            desc.colorAttachments[0].pixelFormat = .rgba16Float
            return try device.makeRenderPipelineState(descriptor: desc)
        } catch {
            KyberLog.video.error("mires du réglage du HDR : \(error.localizedDescription, privacy: .public)")
            return nil
        }
    }()

    override init(frame: CGRect) {
        super.init(frame: frame)
        metal.device = Self.device
        metal.pixelFormat = .rgba16Float
        metal.colorspace = CGColorSpace(name: CGColorSpace.itur_2100_PQ)
        metal.isOpaque = true
        metal.framebufferOnly = true
        metal.backgroundColor = CGColor(gray: 0, alpha: 1)
        #if !os(tvOS)
        metal.wantsExtendedDynamicRangeContent = true
        // Pas d'`edrMetadata` : aucune courbe du système, l'écran écrête.
        #endif
        #if os(macOS)
        layer = metal
        wantsLayer = true
        #else
        layer.addSublayer(metal)
        isUserInteractionEnabled = false
        #endif
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) { fatalError() }

    func show(step: HDRCalibrationModel.Step, value: Double) {
        self.step = step
        self.value = value
        draw()
    }

    #if os(macOS)
    override func hitTest(_ point: NSPoint) -> NSView? { nil }

    override func setFrameSize(_ newSize: NSSize) {
        super.setFrameSize(newSize)
        draw()
    }

    override func viewDidChangeBackingProperties() {
        super.viewDidChangeBackingProperties()
        draw()
    }

    override func viewDidMoveToWindow() {
        super.viewDidMoveToWindow()
        draw()
    }

    private var scale: CGFloat { window?.backingScaleFactor ?? 2 }
    #else
    override func layoutSubviews() {
        super.layoutSubviews()
        CATransaction.begin()
        CATransaction.setDisableActions(true)
        metal.frame = bounds
        CATransaction.commit()
        draw()
    }

    override func didMoveToWindow() {
        super.didMoveToWindow()
        #if os(tvOS)
        // Le téléviseur passe en HDR10 le temps du réglage (« Adapter la plage
        // dynamique »), puis retrouve ce qu'on lui demandait avant (une session en cours).
        if let window, tvWindow == nil {
            previousCriteria = window.avDisplayManager.preferredDisplayCriteria
            window.avDisplayManager.preferredDisplayCriteria = HDRDisplay.criteria(refresh: 60, width: 3840, height: 2160, hdr: true)
            tvWindow = window
        } else if window == nil, let old = tvWindow {
            old.avDisplayManager.preferredDisplayCriteria = previousCriteria
            tvWindow = nil
            previousCriteria = nil
        }
        #endif
        draw()
    }

    #if os(tvOS)
    private weak var tvWindow: UIWindow?
    private var previousCriteria: AVDisplayCriteria?
    #endif

    private var scale: CGFloat { window?.screen.nativeScale ?? UIScreen.main.nativeScale }
    #endif

    private struct Uniforms {
        var size: SIMD2<Float>
        var step: Int32
        var value: Float
    }

    private func draw() {
        let w = bounds.width * scale, h = bounds.height * scale
        guard w >= 1, h >= 1, window != nil, let pipeline = Self.pipeline, let queue = Self.queue else { return }
        metal.contentsScale = scale
        metal.drawableSize = CGSize(width: w, height: h)
        guard let drawable = metal.nextDrawable(), let commands = queue.makeCommandBuffer() else { return }
        let pass = MTLRenderPassDescriptor()
        pass.colorAttachments[0].texture = drawable.texture
        pass.colorAttachments[0].loadAction = .dontCare
        pass.colorAttachments[0].storeAction = .store
        guard let encoder = commands.makeRenderCommandEncoder(descriptor: pass) else { return }
        var u = Uniforms(size: SIMD2(Float(w), Float(h)), step: Int32(step.rawValue), value: Float(value))
        encoder.setRenderPipelineState(pipeline)
        encoder.setFragmentBytes(&u, length: MemoryLayout<Uniforms>.stride, index: 0)
        encoder.drawPrimitives(type: .triangle, vertexStart: 0, vertexCount: 3)
        encoder.endEncoding()
        commands.present(drawable)
        commands.commit()
    }

    private static let shader = """
    #include <metal_stdlib>
    using namespace metal;

    struct U { float2 size; int step; float value; };
    struct VOut { float4 position [[position]]; };

    vertex VOut kn_pattern_vertex(uint id [[vertex_id]]) {
        float2 p = float2((id << 1) & 2, id & 2);
        VOut out;
        out.position = float4(p * 2.0 - 1.0, 0.0, 1.0);
        return out;
    }

    // Lumière en nits → valeur PQ (SMPTE ST 2084).
    float pq(float nits) {
        const float m1 = 0.1593017578125, m2 = 78.84375;
        const float c1 = 0.8359375, c2 = 18.8515625, c3 = 18.6875;
        float y = pow(clamp(nits / 10000.0, 0.0, 1.0), m1);
        return pow((c1 + c2 * y) / (1.0 + c3 * y), m2);
    }

    // Symbole : un anneau et un point, dans un carré de côté `side`, centré en `c`.
    float symbol(float2 p, float2 c, float side) {
        float2 d = (p - c) / side;
        float r = length(d);
        float ring = step(abs(r - 0.32), 0.05);
        float dot_ = step(r, 0.1);
        return max(ring, dot_);
    }

    // Rectangle [a, b] en pixels.
    float inside(float2 p, float2 a, float2 b) {
        return step(a.x, p.x) * step(p.x, b.x) * step(a.y, p.y) * step(p.y, b.y);
    }

    fragment float4 kn_pattern_fragment(VOut in [[stage_in]], constant U &u [[buffer(0)]]) {
        float2 p = in.position.xy;
        float2 c = u.size * 0.5;
        float m = min(u.size.x, u.size.y);
        float nits = 0.0;
        if (u.step == 0 || u.step == 1) {
            // Zone à 10 000 nits (écrêtée par l'écran), symbole à la valeur réglée.
            float half_ = u.step == 0 ? m * 0.16 : 1e6;
            float zone = inside(p, c - half_, c + half_);
            float s = symbol(p, c, m * 0.24);
            nits = zone * mix(10000.0, u.value, s);
        } else if (u.step == 2) {
            // Noir, symbole à la valeur réglée (0 à 1 nit).
            nits = symbol(p, c, m * 0.4) * u.value;
        } else {
            // Blanc du bureau : bureau sombre, fenêtre blanche, barre de titre, lignes de texte.
            float2 a = c - float2(m * 0.62, m * 0.36), b = c + float2(m * 0.62, m * 0.36);
            float win = inside(p, a, b);
            float title = inside(p, a, float2(b.x, a.y + m * 0.07));
            float2 q = (p - a) / m;
            float row = fmod(q.y - 0.11, 0.055);
            float text = step(0.11, q.y) * step(q.y, 0.62) * step(row, 0.022) * step(0.05, q.x)
                       * step(q.x, 0.05 + 1.1 * (0.55 + 0.45 * fract(sin(floor((q.y - 0.11) / 0.055) * 12.9898) * 43758.5453)));
            nits = mix(u.value * 0.12, u.value, win);
            nits = mix(nits, u.value * 0.88, title * win);
            nits = mix(nits, u.value * 0.08, text * win * (1.0 - title));
        }
        float e = pq(nits);
        return float4(e, e, e, 1.0);
    }
    """
}

#if os(macOS)
typealias PlatformPatternView = NSView

/// La mire de l'étape en cours du réglage du HDR.
public struct HDRPatternView: NSViewRepresentable {
    @ObservedObject var model: HDRCalibrationModel
    public init(model: HDRCalibrationModel) { self.model = model }
    public func makeNSView(context: Context) -> NSView { HDRPatternSurface(frame: .zero) }
    public func updateNSView(_ view: NSView, context: Context) {
        (view as? HDRPatternSurface)?.show(step: model.step, value: model.value)
    }
}
#else
typealias PlatformPatternView = UIView

/// La mire de l'étape en cours du réglage du HDR.
public struct HDRPatternView: UIViewRepresentable {
    @ObservedObject var model: HDRCalibrationModel
    public init(model: HDRCalibrationModel) { self.model = model }
    public func makeUIView(context: Context) -> UIView { HDRPatternSurface(frame: .zero) }
    public func updateUIView(_ view: UIView, context: Context) {
        (view as? HDRPatternSurface)?.show(step: model.step, value: model.value)
    }
}
#endif
