// Types communs aux apps Apple : l'écran de l'ordinateur, le format, les
// réglages d'un flux.

import CoreGraphics
import Foundation

public struct KyberDisplay: Sendable, Hashable {
    public let id: UInt32
    public let width: Int
    public let height: Int
    public let x: Int
    public let y: Int

    /// Le même écran à une autre taille (mode changé sur l'ordinateur).
    public func resized(width: Int, height: Int) -> KyberDisplay {
        KyberDisplay(id: id, width: width, height: height, x: x, y: y)
    }
}

public enum VideoCodec: String, Sendable, CaseIterable, Codable {
    case h264, hevc, av1
}

public struct StreamOptions: Sendable {
    public var codec: VideoCodec = .hevc
    public var yuv444 = false
    /// Débit en bits par seconde.
    public var bitrate: UInt32 = 80_000_000
    public var audio = true
    /// `true` : chaque image arrive, quitte à attendre ; `false` : une image
    /// perdue est réparée (correction d'erreurs) ou sautée, jamais attendue.
    public var reliable = false
    /// Synchro verticale : sans elle, chaque image part dès qu'elle est
    /// décodée (latence la plus basse, déchirure possible).
    public var vsync = true
    /// Décodage par l'app (VideoToolbox + Metal) plutôt que par VLC. L'AV1
    /// passe toujours par VLC.
    public var nativeVideo = true
    /// Son décodé par l'app (Opus + Core Audio) plutôt que par VLC.
    public var nativeAudio = true
    /// Presse-papiers partagé (texte), dans les deux sens.
    public var clipboard = true
    /// Toutes les manettes présentées en Xbox 360 à l'ordinateur ; sinon,
    /// fidèles au modèle (DualShock 4 pour les manettes Sony).
    public var gamepadAsXbox360 = true

    public init() {}
}
