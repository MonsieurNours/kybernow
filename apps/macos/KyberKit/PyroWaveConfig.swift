// Le paquet de configuration de PyroWave (`KNPW`), lu par le cœur
// (`pyrowave::Config`, pont `kn_pyrowave_config`) : l'app n'en garde plus de
// copie (phase 2b de l'audit). Version 1 (16 octets) ou 2 (48 octets, avec
// les métadonnées HDR10 de la capture, KYB-29 ; docs/kyber-changes.md,
// « En-tête KNPW version 2 ») ; les octets au-delà de ce que l'app connaît
// sont ignorés. Sans paquet (perdu), la définition se lit dans l'en-tête de
// séquence d'une image. Fichier à part du décodeur : les essais hors ligne
// (tests/pyrowave, tests/video, tests/autotest) le compilent avec la
// bibliothèque du cœur.

import CoreVideo
import Foundation
@_implementationOnly import CKyberNow

/// Ce que décrit un paquet de configuration PyroWave, et ce qu'en tire le
/// décodeur (format de sortie, plans de chrominance, métadonnées HDR10).
public struct PyroWaveConfig: Equatable, Sendable {
    public var chroma444: Bool
    public var hdr: Bool
    public var fullRange: Bool
    public var width: Int
    public var height: Int
    /// Métadonnées HDR10 de l'en-tête version 2 au format de CoreVideo (celui
    /// des SEI HEVC) : écran de mastering
    /// (`kCVImageBufferMasteringDisplayColorVolumeKey`) et niveaux de lumière
    /// (`kCVImageBufferContentLightLevelInfoKey`) ; nil sans elles (version 1,
    /// SDR, drapeau absent).
    public var masteringDisplay: Data?
    public var contentLightLevel: Data?

    public init(chroma444: Bool, hdr: Bool, fullRange: Bool, width: Int, height: Int,
                masteringDisplay: Data? = nil, contentLightLevel: Data? = nil) {
        self.chroma444 = chroma444
        self.hdr = hdr
        self.fullRange = fullRange
        self.width = width
        self.height = height
        self.masteringDisplay = masteringDisplay
        self.contentLightLevel = contentLightLevel
    }

    /// Lit un paquet de configuration (versions 1 et 2) ; nil s'il n'en est
    /// pas un (définition impossible, version 2 tronquée).
    public init?(_ data: UnsafeBufferPointer<UInt8>) {
        guard let base = data.baseAddress else { return nil }
        self.init(json: Self.take(kn_pyrowave_config(base, UInt(data.count))))
    }

    /// Un paquet de configuration plutôt qu'une image (lu à chaque paquet).
    static func isConfig(_ data: UnsafeBufferPointer<UInt8>) -> Bool {
        guard let base = data.baseAddress else { return false }
        return kn_pyrowave_is_config(base, UInt(data.count))
    }

    /// Repli si le paquet de configuration s'est perdu : définition et
    /// chroma lues dans l'en-tête de séquence du flux ; SDR, plage pleine
    /// (l'encodeur Metal n'y écrit ni la couleur ni la plage).
    init?(sequenceHeader data: UnsafeBufferPointer<UInt8>) {
        guard let base = data.baseAddress else { return nil }
        self.init(json: Self.take(kn_pyrowave_config_from_sequence_header(base, UInt(data.count))))
    }

    private init?(json: String?) {
        guard let json, let v = try? JSONSerialization.jsonObject(with: Data(json.utf8)) as? [String: Any],
              let width = (v["width"] as? NSNumber)?.intValue, let height = (v["height"] as? NSNumber)?.intValue else { return nil }
        self.init(chroma444: v["chroma444"] as? Bool ?? false, hdr: v["hdr"] as? Bool ?? false,
                  fullRange: v["full_range"] as? Bool ?? true, width: width, height: height,
                  masteringDisplay: (v["mastering_sei"] as? String).flatMap { Data(base64Encoded: $0) },
                  contentLightLevel: (v["light_sei"] as? String).flatMap { Data(base64Encoded: $0) })
    }

    /// Le paquet version 1 de ces valeurs (outils, essais).
    public var bytes: [UInt8] {
        var out = [UInt8](repeating: 0, count: 16)
        out.withUnsafeMutableBufferPointer { b in
            kn_pyrowave_config_bytes(chroma444, hdr, fullRange, UInt32(clamping: width), UInt32(clamping: height), b.baseAddress)
        }
        return out
    }

    /// Même image (définition, chroma, couleur, plage) : le décodeur reste,
    /// seules les métadonnées changent.
    func sameImage(_ other: PyroWaveConfig) -> Bool {
        (chroma444, hdr, fullRange, width, height) == (other.chroma444, other.hdr, other.fullRange, other.width, other.height)
    }

    /// Format CoreVideo de sortie, l'un de ceux que lit le rendu.
    var pixelFormat: OSType {
        switch (chroma444, hdr, fullRange) {
        case (false, false, false): return kCVPixelFormatType_420YpCbCr8BiPlanarVideoRange
        case (false, false, true): return kCVPixelFormatType_420YpCbCr8BiPlanarFullRange
        case (true, false, false): return kCVPixelFormatType_444YpCbCr8BiPlanarVideoRange
        case (true, false, true): return kCVPixelFormatType_444YpCbCr8BiPlanarFullRange
        case (false, true, false): return kCVPixelFormatType_420YpCbCr10BiPlanarVideoRange
        case (false, true, true): return kCVPixelFormatType_420YpCbCr10BiPlanarFullRange
        case (true, true, false): return kCVPixelFormatType_444YpCbCr10BiPlanarVideoRange
        case (true, true, true): return kCVPixelFormatType_444YpCbCr10BiPlanarFullRange
        }
    }

    var chromaWidth: Int { chroma444 ? width : width / 2 }
    var chromaHeight: Int { chroma444 ? height : height / 2 }

    /// Les attachements des images décodées : matrice, primaires, transfert,
    /// et, en HDR, les métadonnées HDR10 du paquet version 2 (le rendu en
    /// tire la crête du flux, comme celles d'un flux HEVC).
    var attachments: [CFString: Any] {
        guard hdr else {
            return [
                kCVImageBufferYCbCrMatrixKey: kCVImageBufferYCbCrMatrix_ITU_R_709_2,
                kCVImageBufferColorPrimariesKey: kCVImageBufferColorPrimaries_ITU_R_709_2,
                kCVImageBufferTransferFunctionKey: kCVImageBufferTransferFunction_ITU_R_709_2,
            ]
        }
        var a: [CFString: Any] = [
            kCVImageBufferYCbCrMatrixKey: kCVImageBufferYCbCrMatrix_ITU_R_2020,
            kCVImageBufferColorPrimariesKey: kCVImageBufferColorPrimaries_ITU_R_2020,
            kCVImageBufferTransferFunctionKey: kCVImageBufferTransferFunction_SMPTE_ST_2084_PQ,
        ]
        if let masteringDisplay { a[kCVImageBufferMasteringDisplayColorVolumeKey] = masteringDisplay as CFData }
        if let contentLightLevel { a[kCVImageBufferContentLightLevelInfoKey] = contentLightLevel as CFData }
        return a
    }

    /// Pour le journal : « 3840×2160 4:2:0 HDR10 pleine » (`Config::label`
    /// du cœur), et « métadonnées HDR10 » quand l'en-tête en porte.
    public var label: String {
        let bytes = self.bytes
        let json = bytes.withUnsafeBufferPointer { b in Self.take(kn_pyrowave_config(b.baseAddress, UInt(b.count))) }
        let base = json.flatMap { try? JSONSerialization.jsonObject(with: Data($0.utf8)) as? [String: Any] }?["label"] as? String ?? "\(width)×\(height)"
        return masteringDisplay != nil || contentLightLevel != nil ? base + ", métadonnées HDR10" : base
    }

    private static func take(_ p: UnsafeMutablePointer<CChar>?) -> String? {
        guard let p else { return nil }
        defer { kn_string_free(p) }
        return String(cString: p)
    }
}
