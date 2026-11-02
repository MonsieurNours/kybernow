// Apple TV : l'image HDR passe par une couche vidéo du système
// (`AVSampleBufferDisplayLayer`), pas par Metal (docs/hdr.md, « Apple TV »).
//
// tvOS n'a ni contenu EDR ni `edrMetadata` sur une `CAMetalLayer` : des
// valeurs PQ écrites dans une couche Metal y sont montrées comme du SDR,
// image délavée, grise et bleutée, sans noirs (essai du 2026-11-02, Apple
// TV 4K de 3ᵉ génération sur un téléviseur LG). La couche vidéo, elle,
// reçoit l'image décodée telle quelle (HEVC Main 10, x420, étiquetée PQ
// BT.2020, avec les métadonnées HDR10 du flux) : tvOS la sort en HDR10 si
// le téléviseur est passé en HDR (`AVDisplayManager`), et la ramène
// lui-même en SDR sinon.
//
// Chaque image part pour être montrée tout de suite (`DisplayImmediately`) :
// aucune file, aucune horloge. tvOS ne dit pas quand elle est montrée :
// l'instant est estimé (balayage suivant).

#if os(tvOS)
import AVFoundation
import CoreMedia
import CoreVideo
import UIKit

final class HDRVideoLayer {
    let layer = AVSampleBufferDisplayLayer()
    private var format: CMVideoFormatDescription?
    private var failures = 0
    private var described = false

    init() {
        layer.videoGravity = .resizeAspect
        layer.backgroundColor = CGColor(gray: 0, alpha: 1)
        layer.isOpaque = true
    }

    /// Montre `buffer` dès que possible (n'importe quel fil). Une image 10
    /// bits sans étiquette de transfert est étiquetée PQ BT.2020 (le cœur a
    /// dit le flux HDR).
    @discardableResult
    func show(_ buffer: CVPixelBuffer) -> Bool {
        if CVBufferCopyAttachment(buffer, kCVImageBufferTransferFunctionKey, nil) == nil {
            Self.tagPQ(buffer)
        }
        if format == nil || !CMVideoFormatDescriptionMatchesImageBuffer(format!, imageBuffer: buffer) {
            var f: CMVideoFormatDescription?
            CMVideoFormatDescriptionCreateForImageBuffer(allocator: nil, imageBuffer: buffer, formatDescriptionOut: &f)
            format = f
            if let f { describe(f) }
        }
        guard let format else { return false }
        var timing = CMSampleTimingInfo(duration: .invalid, presentationTimeStamp: CMClockGetTime(CMClockGetHostTimeClock()), decodeTimeStamp: .invalid)
        var sample: CMSampleBuffer?
        CMSampleBufferCreateReadyWithImageBuffer(allocator: nil, imageBuffer: buffer, formatDescription: format, sampleTiming: &timing, sampleBufferOut: &sample)
        guard let sample else { return false }
        if let attachments = CMSampleBufferGetSampleAttachmentsArray(sample, createIfNecessary: true), CFArrayGetCount(attachments) > 0 {
            let dict = unsafeBitCast(CFArrayGetValueAtIndex(attachments, 0), to: CFMutableDictionary.self)
            CFDictionarySetValue(dict, Unmanaged.passUnretained(kCMSampleAttachmentKey_DisplayImmediately).toOpaque(),
                                 Unmanaged.passUnretained(kCFBooleanTrue).toOpaque())
        }
        let renderer = layer.sampleBufferRenderer
        if renderer.status == .failed {
            if failures < 3 {
                failures += 1
                KyberLog.video.error("couche vidéo HDR en échec : \(renderer.error?.localizedDescription ?? "?", privacy: .public) ; vidée")
            }
            renderer.flush()
        }
        renderer.enqueue(sample)
        return true
    }

    /// Retire l'image montrée (retour en SDR, fin de session).
    func clear() {
        layer.sampleBufferRenderer.flush(removingDisplayedImage: true, completionHandler: nil)
        format = nil
    }

    /// Étiquette PQ BT.2020 posée sur une image qui n'en a pas.
    static func tagPQ(_ buffer: CVPixelBuffer) {
        CVBufferSetAttachment(buffer, kCVImageBufferColorPrimariesKey, kCVImageBufferColorPrimaries_ITU_R_2020, .shouldPropagate)
        CVBufferSetAttachment(buffer, kCVImageBufferTransferFunctionKey, kCVImageBufferTransferFunction_SMPTE_ST_2084_PQ, .shouldPropagate)
        CVBufferSetAttachment(buffer, kCVImageBufferYCbCrMatrixKey, kCVImageBufferYCbCrMatrix_ITU_R_2020, .shouldPropagate)
    }

    /// Le format de la couche, une fois par format (journal).
    private func describe(_ f: CMVideoFormatDescription) {
        let d = CMVideoFormatDescriptionGetDimensions(f)
        let ext = CMFormatDescriptionGetExtensions(f) as? [CFString: Any] ?? [:]
        func s(_ k: CFString) -> String { (ext[k] as? String) ?? "—" }
        let sub = CMFormatDescriptionGetMediaSubType(f)
        let code = String(bytes: [24, 16, 8, 0].map { UInt8((sub >> $0) & 0xff) }, encoding: .ascii) ?? "?"
        let mastering = ext[kCMFormatDescriptionExtension_MasteringDisplayColorVolume] != nil
        let cll = ext[kCMFormatDescriptionExtension_ContentLightLevelInfo] != nil
        KyberLog.video.notice("couche vidéo HDR : \(code, privacy: .public) \(d.width)×\(d.height), primaires \(s(kCMFormatDescriptionExtension_ColorPrimaries), privacy: .public), transfert \(s(kCMFormatDescriptionExtension_TransferFunction), privacy: .public), matrice \(s(kCMFormatDescriptionExtension_YCbCrMatrix), privacy: .public), métadonnées HDR10 \(mastering ? "écran" : "—", privacy: .public)/\(cll ? "MaxCLL" : "—", privacy: .public)")
    }
}

public extension HDRDisplay {
    /// L'état du téléviseur et de tvOS pour le HDR, en une ligne (journal) :
    /// HDR10 accepté, marges EDR données par tvOS, « Adapter la plage
    /// dynamique », critère posé ou non, changement de mode en cours.
    @MainActor
    static func describeTV(_ window: UIWindow?) -> String {
        let screen = window?.screen ?? UIScreen.main
        var parts = ["HDR10 accepté \(AVPlayer.eligibleForHDRPlayback ? "oui" : "non")",
                     "marge EDR \(String(format: "%.2f", screen.currentEDRHeadroom)) (possible \(String(format: "%.2f", screen.potentialEDRHeadroom)))"]
        if let manager = window?.avDisplayManager {
            parts.append("adapter la plage dynamique \(manager.isDisplayCriteriaMatchingEnabled ? "oui" : "non")")
            parts.append("changement de mode \(manager.isDisplayModeSwitchInProgress ? "en cours" : "non")")
            // AVDisplayCriteria ne se relit pas : seulement s'il y en a un.
            parts.append(manager.preferredDisplayCriteria != nil ? "critère demandé" : "aucun critère demandé")
        }
        return parts.joined(separator: ", ")
    }

    /// Écrit l'état du téléviseur au journal maintenant, puis à chaque début
    /// et fin de changement de mode (`AVDisplayManager`).
    @MainActor
    static func logTV(_ window: UIWindow?, _ why: String) -> [NSObjectProtocol] {
        KyberLog.video.notice("téléviseur (\(why, privacy: .public)) : \(describeTV(window), privacy: .public)")
        guard let manager = window?.avDisplayManager else { return [] }
        let c = NotificationCenter.default
        return [(Notification.Name.AVDisplayManagerModeSwitchStart, "début du changement de mode"),
                (Notification.Name.AVDisplayManagerModeSwitchEnd, "fin du changement de mode")].map { name, label in
            c.addObserver(forName: name, object: manager, queue: .main) { [weak window] _ in
                MainActor.assumeIsolated {
                    KyberLog.video.notice("téléviseur (\(label, privacy: .public)) : \(describeTV(window), privacy: .public)")
                }
            }
        }
    }
}
#endif
