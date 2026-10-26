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
            let wide = [kCVPixelFormatType_444YpCbCr8BiPlanarVideoRange, kCVPixelFormatType_444YpCbCr10BiPlanarVideoRange].contains(format)
            decoder?.forcePixelFormat(wide ? kCVPixelFormatType_444YpCbCr8BiPlanarVideoRange : kCVPixelFormatType_420YpCbCr8BiPlanarVideoRange)
        }
    }

    func stop() {
        decoder.invalidate()
    }
}
