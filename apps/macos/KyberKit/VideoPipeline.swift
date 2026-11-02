// Chemin de l'image : paquets de Kyber → VideoToolbox → Metal.
// (Le son suit le même principe : AudioPlayer, AudioPlayerCore.swift.)
//
// Kyber appelle `handle` depuis son fil réseau, un paquet à la fois ; on
// remet l'image à VideoToolbox et on rend la main tout de suite (décodage
// asynchrone, quelques millisecondes en matériel) ; l'image décodée part
// au rendu depuis le fil de VideoToolbox, sans attendre.

import CoreVideo
import Foundation
@_implementationOnly import CKyber

final class VideoPipeline {
    let decoder = VideoDecoder()
    let renderer: VideoRenderer
    /// Première image décodée de ce flux (multi-écrans : l'écran passe à son
    /// image) ; appelé une fois, sur le fil de VideoToolbox.
    var onFirstFrame: (() -> Void)?
    private var firstFrame = true

    init?() {
        guard let renderer = VideoRenderer() else { return nil }
        self.renderer = renderer
        decoder.onFrame = { [renderer, weak self] frame in
            renderer.submit(frame)
            if let self, self.firstFrame {
                self.firstFrame = false
                self.onFirstFrame?()
            }
        }
        renderer.onUnsupportedFormat = { [weak decoder] format in
            // Format exotique : on redemande du 4:2:0 ou du 4:4:4 8 bits, toujours lisibles.
            // Une image 10 bits (HEVC Main 10, AV1 10 bits : le HDR, dont le
            // décodeur des puces Apple sort un format compressé « &xv0 ») garde
            // ses 10 bits : 4:2:0 10 bits en deux plans, que le rendu lit.
            let wide = [kCVPixelFormatType_444YpCbCr8BiPlanarVideoRange, kCVPixelFormatType_444YpCbCr10BiPlanarVideoRange].contains(format)
            let forced: OSType
            if Self.bitsPerComponent(format) > 8 {
                forced = wide ? kCVPixelFormatType_444YpCbCr10BiPlanarVideoRange : kCVPixelFormatType_420YpCbCr10BiPlanarVideoRange
            } else {
                forced = wide ? kCVPixelFormatType_444YpCbCr8BiPlanarVideoRange : kCVPixelFormatType_420YpCbCr8BiPlanarVideoRange
            }
            decoder?.forcePixelFormat(forced)
        }
    }

    func stop() {
        decoder.invalidate()
    }

    /// Bits par composante d'un format de CoreVideo (8 si inconnu).
    static func bitsPerComponent(_ format: OSType) -> Int {
        // `kCVPixelFormatBitsPerComponent` (macOS 15, iOS 18), écrite en toutes lettres pour macOS 14.
        let key = "BitsPerComponent"
        guard let desc = CVPixelFormatDescriptionCreateWithPixelFormatType(kCFAllocatorDefault, format) as? [String: Any] else { return 8 }
        if let bits = desc[key] as? Int { return bits }
        // Formats à plusieurs plans : la profondeur est donnée plan par plan.
        if let planes = desc[kCVPixelFormatPlanes as String] as? [[String: Any]],
           let bits = planes.compactMap({ $0[key] as? Int }).max() { return bits }
        return 8
    }
}
