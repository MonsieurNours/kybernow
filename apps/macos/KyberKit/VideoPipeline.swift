// Chemin de l'image sans VLC : paquets de Kyber → VideoToolbox → Metal.
//
// Kyber appelle `handle` depuis son fil réseau, un paquet à la fois ; on
// décode dans la foulée (quelques millisecondes en matériel) et l'image
// part au rendu sans attendre.

import CoreVideo
import Foundation
@_implementationOnly import CKyber

final class VideoPipeline {
    let decoder = VideoDecoder()
    let renderer: VideoRenderer

    init?() {
        guard let renderer = VideoRenderer() else { return nil }
        self.renderer = renderer
        decoder.onFrame = { [renderer] frame in renderer.submit(frame) }
        renderer.onUnsupportedFormat = { [weak decoder] format in
            // Format exotique : on redemande du 4:2:0 ou du 4:4:4 8 bits, toujours lisibles.
            let wide = [kCVPixelFormatType_444YpCbCr8BiPlanarVideoRange, kCVPixelFormatType_444YpCbCr10BiPlanarVideoRange].contains(format)
            decoder?.forcePixelFormat(wide ? kCVPixelFormatType_444YpCbCr8BiPlanarVideoRange : kCVPixelFormatType_420YpCbCr8BiPlanarVideoRange)
        }
    }

    /// Rappel C donné à Kyber (`userdata` : ce pipeline, non retenu).
    static let callback: kyclient_VideoPacketCb = { packet, userdata in
        guard let packet, let userdata else { return }
        let me = Unmanaged<VideoPipeline>.fromOpaque(userdata).takeUnretainedValue()
        me.handle(packet.pointee)
    }

    private func handle(_ p: kyclient_VideoPacket) {
        switch p.kind {
        case KYCLIENT_VIDEO_PACKET_KIND_CODEC:
            decoder.setCodec(p.codec)
        case KYCLIENT_VIDEO_PACKET_KIND_MEDIA:
            guard let data = p.data, p.len > 0 else { return }
            decoder.decode(UnsafeBufferPointer(start: data, count: Int(p.len)), isConfig: p.is_config, isKey: p.is_key, pts: p.pts)
        default:
            // Données perdues : le décodeur continue, l'image se répare à la prochaine image clé.
            break
        }
    }

    func stop() {
        decoder.invalidate()
    }
}
