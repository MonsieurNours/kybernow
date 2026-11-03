// Agrandissement de l'image d'une console (docs/playstation.md,
// « Agrandissement ») : la PS5 n'envoie pas plus de 1080p ; l'image est
// agrandie aux pixels de l'écran (ou de la fenêtre) par MetalFX, puis
// rendue plus nette par un filtre de contours réglable.
//
// Trois passes à chaque image, sur la puce graphique :
// 1. YCbCr → RGB à la taille de l'image reçue, dans la chaîne de la couche
//    (sRGB 8 bits en SDR, PQ ou lumière linéaire en FP16 en HDR) ;
// 2. MetalFX « spatial » vers la taille affichée (FSR 1 d'AMD, image par
//    image : pas de vecteurs de mouvement dans une vidéo) ;
// 3. netteté (RCAS d'AMD FidelityFX, licence MIT) en recopiant dans le
//    tampon de l'écran, cerné de noir comme sans agrandissement.
//
// Pourquoi MetalFX seul : la super-résolution de VideoToolbox « à faible
// latence » refuse plus de 1280 pixels de côté, et l'autre ne fait que ×4
// et prend 413 ms par image en 1080p (M4 Max, 2026-11-03). MetalFX :
// 0,4 ms vers 4K, 0,7 ms vers 5K, 1 ms vers 6K, en SDR comme en HDR.
//
// Coût : mesuré sur les 120 premières images (passes 1 et 2, à part de
// l'image). Au-delà de 2 ms en médiane, l'agrandissement se coupe pour la
// partie (filtre bilinéaire, comme avant) et le dit.
//
// Plateformes : Mac (macOS 13+), iPhone et iPad (iOS 16+), Apple TV
// (tvOS 27.1+, MetalFX n'y est pas avant ; sur l'Apple TV, seulement en SDR :
// en HDR, l'image passe par la couche vidéo du système, qui l'agrandit
// elle-même). Ailleurs, `Upscaler` n'existe pas et le rendu garde le filtre
// bilinéaire.

import Metal
#if canImport(MetalFX)
import MetalFX
#endif
import QuartzCore

/// Réglages de l'agrandissement (ceux de la console).
public struct UpscaleSettings: Equatable, Sendable {
    /// Netteté, de 0 (coupée) à 1.
    public var sharpness: Float

    public init(sharpness: Float) {
        self.sharpness = max(0, min(1, sharpness))
    }
}

/// Ce que l'agrandissement fait en ce moment (statistiques de la partie).
public struct UpscaleReport: Equatable, Sendable {
    public enum State: Equatable, Sendable {
        /// Image agrandie (taille affichée, en pixels).
        case active(width: Int, height: Int)
        /// L'image n'est pas plus petite que l'écran : rien à agrandir.
        case notNeeded
        /// Coupé : trop lent sur cet appareil (médiane mesurée, en ms).
        case tooSlow(ms: Double)
        /// Coupé : MetalFX n'est pas là ou refuse ce format.
        case unavailable
    }

    public var state: State
    /// Coût médian des passes 1 et 2 (nil : pas encore mesuré).
    public var gpuMs: Double?

    public init(state: State, gpuMs: Double?) {
        self.state = state
        self.gpuMs = gpuMs
    }
}

/// L'agrandissement est possible sur cet appareil (MetalFX là, et la puce
/// graphique le prend) : sinon, les réglages de la console ne le proposent pas.
public enum Upscaling {
    public static let available: Bool = {
        #if canImport(MetalFX)
        if #available(macOS 13.0, iOS 16.0, tvOS 27.1, *), let device = MTLCreateSystemDefaultDevice() {
            return MTLFXSpatialScalerDescriptor.supportsDevice(device)
        }
        #endif
        return false
    }()
}

/// Réglages de la passe de netteté (passe 3), lus par le nuanceur.
struct UpscalerSharpenUniforms {
    /// 0 : coupée ; 1 : la plus forte (RCAS à 0 stop).
    var sharpness: Float
    /// Valeurs au-delà de 1 (lumière linéaire) : crête locale.
    var hdr: Int32
    /// Coin haut gauche de la zone d'affichage, en pixels du tampon.
    var origin: SIMD2<Int32>
}

#if canImport(MetalFX)
@available(macOS 13.0, iOS 16.0, tvOS 27.1, *)
final class Upscaler {
    /// Au-delà, l'agrandissement se coupe pour la partie.
    static let budgetMs = 2.0
    /// Images mesurées avant de décider (les 10 premières sont écartées).
    static let measured = 120

    private let device: MTLDevice
    private let library: MTLLibrary?
    private var sharpenPipelines: [MTLPixelFormat: MTLRenderPipelineState] = [:]
    private var scaler: MTLFXSpatialScaler?
    private var key: Key?
    private var input: MTLTexture?
    private var output: MTLTexture?
    /// Formats que MetalFX refuse (un échec n'est pas retenté à chaque image).
    private var refused = Set<MTLPixelFormat>()

    private let lock = NSLock()
    private var samples: [Double] = []
    private var count = 0
    /// Trop lent : la médiane mesurée (lue par le fil de rendu).
    var tooSlow: Double? {
        lock.lock()
        defer { lock.unlock() }
        return slow
    }
    private var slow: Double?

    /// Dernier état dit (le rendu ne le redit qu'au changement).
    var lastReport: UpscaleReport?

    private struct Key: Equatable {
        var format: MTLPixelFormat
        var inW: Int, inH: Int, outW: Int, outH: Int
        var hdr: Bool
    }

    struct Textures {
        let input: MTLTexture
        let output: MTLTexture
        let scaler: MTLFXSpatialScaler
        let sharpen: MTLRenderPipelineState
    }

    init?(device: MTLDevice) {
        guard MTLFXSpatialScalerDescriptor.supportsDevice(device) else { return nil }
        self.device = device
        do {
            library = try device.makeLibrary(source: Self.shader, options: nil)
        } catch {
            KyberLog.video.error("netteté de l'agrandissement : \(error.localizedDescription, privacy: .public)")
            library = nil
        }
    }

    /// Médiane mesurée sur les `measured` premières images (nil : pas encore).
    var medianMs: Double? {
        lock.lock()
        defer { lock.unlock() }
        return decided
    }
    private var decided: Double?

    /// Les textures et le scaler pour cette taille (refaits si elle change).
    /// `hdr` : valeurs au-delà de 1 (couche en lumière linéaire).
    func prepare(format: MTLPixelFormat, input inW: Int, _ inH: Int, output outW: Int, _ outH: Int, hdr: Bool) -> Textures? {
        guard tooSlow == nil, !refused.contains(format), let sharpen = sharpenPipeline(format) else { return nil }
        let wanted = Key(format: format, inW: inW, inH: inH, outW: outW, outH: outH, hdr: hdr)
        if key != wanted || scaler == nil {
            scaler = nil
            let d = MTLFXSpatialScalerDescriptor()
            d.colorTextureFormat = format
            d.outputTextureFormat = format
            d.inputWidth = inW
            d.inputHeight = inH
            d.outputWidth = outW
            d.outputHeight = outH
            d.colorProcessingMode = hdr ? .hdr : .perceptual
            guard let s = d.makeSpatialScaler(device: device) else {
                KyberLog.video.error("MetalFX refuse \(inW)×\(inH) → \(outW)×\(outH) en format \(format.rawValue) : filtre bilinéaire")
                refused.insert(format)
                return nil
            }
            let i = MTLTextureDescriptor.texture2DDescriptor(pixelFormat: format, width: inW, height: inH, mipmapped: false)
            i.usage = s.colorTextureUsage.union(.renderTarget)
            i.storageMode = .private
            let o = MTLTextureDescriptor.texture2DDescriptor(pixelFormat: format, width: outW, height: outH, mipmapped: false)
            o.usage = s.outputTextureUsage.union(.shaderRead)
            o.storageMode = .private
            guard let it = device.makeTexture(descriptor: i), let ot = device.makeTexture(descriptor: o) else { return nil }
            s.colorTexture = it
            s.outputTexture = ot
            s.inputContentWidth = inW
            s.inputContentHeight = inH
            scaler = s
            input = it
            output = ot
            if key?.inW != inW || key?.inH != inH || key?.outW != outW || key?.outH != outH {
                KyberLog.video.notice("agrandissement MetalFX : \(inW)×\(inH) → \(outW)×\(outH)\(hdr ? ", HDR" : "", privacy: .public)")
            }
            key = wanted
        }
        guard let scaler, let input, let output else { return nil }
        return Textures(input: input, output: output, scaler: scaler, sharpen: sharpen)
    }

    /// Coût des passes 1 et 2 d'une image (fin du GPU). Décide après
    /// `measured` images : trop lent, l'agrandissement se coupe.
    func record(_ commands: MTLCommandBuffer) {
        let ms = (commands.gpuEndTime - commands.gpuStartTime) * 1000
        guard ms > 0 else { return }
        lock.lock()
        defer { lock.unlock() }
        count += 1
        guard count > 10, decided == nil else { return }
        samples.append(ms)
        if samples.count == Self.measured {
            let median = samples.sorted()[samples.count / 2]
            decided = median
            if median > Self.budgetMs {
                slow = median
                KyberLog.video.notice("agrandissement coupé : \(String(format: "%.2f", median), privacy: .public) ms par image (plus de \(Self.budgetMs, privacy: .public) ms)")
            } else {
                KyberLog.video.notice("agrandissement : \(String(format: "%.2f", median), privacy: .public) ms par image")
            }
        }
    }

    private func sharpenPipeline(_ format: MTLPixelFormat) -> MTLRenderPipelineState? {
        if let p = sharpenPipelines[format] { return p }
        guard let library else { return nil }
        let desc = MTLRenderPipelineDescriptor()
        desc.vertexFunction = library.makeFunction(name: "kn_up_vertex")
        desc.fragmentFunction = library.makeFunction(name: "kn_up_sharpen")
        desc.colorAttachments[0].pixelFormat = format
        do {
            let p = try device.makeRenderPipelineState(descriptor: desc)
            sharpenPipelines[format] = p
            return p
        } catch {
            KyberLog.video.error("netteté de l'agrandissement : \(error.localizedDescription, privacy: .public)")
            return nil
        }
    }

    typealias SharpenUniforms = UpscalerSharpenUniforms

    /// RCAS (FidelityFX, AMD, licence MIT), sur l'image agrandie lue pixel à
    /// pixel (la zone d'affichage a sa taille exacte). En lumière linéaire,
    /// la crête prise est la plus forte des voisines (au moins 1).
    private static let shader = """
    #include <metal_stdlib>
    using namespace metal;

    struct UpOut { float4 position [[position]]; };
    struct Sharpen { float sharpness; int hdr; int2 origin; };

    vertex UpOut kn_up_vertex(uint id [[vertex_id]]) {
        float2 p = float2((id << 1) & 2, id & 2);
        UpOut out;
        out.position = float4(p * 2.0 - 1.0, 0.0, 1.0);
        return out;
    }

    fragment float4 kn_up_sharpen(UpOut in [[stage_in]],
                                  texture2d<float, access::read> img [[texture(0)]],
                                  constant Sharpen &u [[buffer(0)]]) {
        int2 size = int2(img.get_width(), img.get_height());
        int2 p = clamp(int2(in.position.xy) - u.origin, int2(0), size - 1);
        float3 e = img.read(uint2(p)).rgb;
        if (u.sharpness <= 0.0) { return float4(e, 1.0); }
        float3 b = img.read(uint2(clamp(p + int2(0, -1), int2(0), size - 1))).rgb;
        float3 d = img.read(uint2(clamp(p + int2(-1, 0), int2(0), size - 1))).rgb;
        float3 f = img.read(uint2(clamp(p + int2(1, 0), int2(0), size - 1))).rgb;
        float3 h = img.read(uint2(clamp(p + int2(0, 1), int2(0), size - 1))).rgb;
        float3 mn4 = min(min(b, d), min(f, h));
        float3 mx4 = max(max(b, d), max(f, h));
        float peak = u.hdr != 0 ? max(1.0, max(max(mx4.r, mx4.g), mx4.b)) : 1.0;
        float3 hitMin = mn4 / (4.0 * mx4 + 1e-5);
        float3 hitMax = (peak - mx4) / (4.0 * mn4 - 4.0 * peak - 1e-5);
        float3 lobeRGB = max(-hitMin, hitMax);
        const float limit = 0.25 - 1.0 / 16.0;
        float lobe = max(-limit, min(max(max(lobeRGB.r, lobeRGB.g), lobeRGB.b), 0.0)) * u.sharpness;
        float3 c = (lobe * (b + d + f + h) + e) / (4.0 * lobe + 1.0);
        return float4(max(c, 0.0), 1.0);
    }
    """
}
#endif
