// Ce que la machine sait décoder en matériel, mesuré une fois au lancement.
//
// VideoToolbox dit si un codec est décodé en matériel, mais pas pour le
// 4:4:4 : on décode pour de bon une petite image HEVC 4:4:4 (64 × 64), en
// exigeant le décodeur matériel. Ainsi l'app ne propose que ce que la
// machine affiche sans peine, sur n'importe quel Mac (et plus tard iPhone,
// iPad, Apple TV).

import CoreMedia
import Foundation
import VideoToolbox

public struct DecoderCapabilities: Sendable {
    public let h264: Bool
    public let hevc: Bool
    /// HEVC 4:4:4 décodé en matériel (texte net, le profil Bureau et Max).
    public let hevc444: Bool
    /// AV1 en matériel (puces M3 et suivantes). Sans lui, l'app ne propose
    /// pas l'AV1.
    public let av1: Bool

    public static let current = DecoderCapabilities.probe()

    /// À appeler au lancement, hors du fil principal : sonde les décodeurs
    /// (20 à 80 ms, dont le décodage 4:4:4) et compile le pipeline Metal du
    /// rendu (30 à 150 ms), pour que ni l'accueil ni le premier flux
    /// n'attendent.
    public static func prepare() {
        _ = current
        VideoRenderer.warmUp()
    }

    /// Résumé lisible pour les réglages.
    public var summary: String {
        var parts: [String] = []
        if hevc { parts.append(hevc444 ? String(localized: "HEVC (4:4:4 compris)", bundle: .kyberKit) : "HEVC") }
        if h264 { parts.append("H.264") }
        if av1 { parts.append("AV1") }
        return parts.isEmpty ? String(localized: "aucun décodeur matériel", bundle: .kyberKit) : parts.joined(separator: ", ")
    }

    private static func probe() -> DecoderCapabilities {
        let caps = DecoderCapabilities(
            h264: VTIsHardwareDecodeSupported(kCMVideoCodecType_H264),
            hevc: VTIsHardwareDecodeSupported(kCMVideoCodecType_HEVC),
            hevc444: VTIsHardwareDecodeSupported(kCMVideoCodecType_HEVC) && decodes444(),
            av1: VTIsHardwareDecodeSupported(kCMVideoCodecType_AV1)
        )
        KyberLog.video.notice("décodeurs matériels : \(caps.summary, privacy: .public)")
        return caps
    }

    /// Image grise HEVC Main 4:4:4 (Annex B : VPS, SPS, PPS, image IDR).
    private static let sample444 = Data(base64Encoded: "AAAAAUABDAH//wQIAAADAJ4oAAADAAAeugJAAAAAAUIBAQQIAAADAJ4oAAADAAAekAQQILLdSSZXgLQEAAADAAQAAAMABCAAAAABRAHBcoYMAiQAAAEoAa8FmP1cmYIz")!

    private static func decodes444() -> Bool {
        sample444.withUnsafeBytes { raw -> Bool in
            let all = raw.bindMemory(to: UInt8.self)
            let nals = VideoDecoder.nalUnits(all)
            let sets = nals.filter { (32...34).contains(($0[0] >> 1) & 0x3F) }.map { Data($0) }
            let picture = nals.filter { !(32...34).contains(($0[0] >> 1) & 0x3F) }
            guard sets.count == 3, !picture.isEmpty,
                  let format = VideoDecoder.format(codec: .hevc, parameterSets: sets),
                  let session = VideoDecoder.makeSession(format: format, pixelFormat: nil, requireHardware: true),
                  let sample = VideoDecoder.sample(picture, format: format)
            else { return false }
            defer { VTDecompressionSessionInvalidate(session) }
            var ok = false
            let status = VTDecompressionSessionDecodeFrame(session, sampleBuffer: sample, flags: [], infoFlagsOut: nil) { status, _, image, _, _ in
                ok = status == noErr && image != nil
            }
            VTDecompressionSessionWaitForAsynchronousFrames(session)
            return status == noErr && ok
        }
    }
}
