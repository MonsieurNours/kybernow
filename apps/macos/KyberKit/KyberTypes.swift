// Types communs aux apps Apple : l'écran de l'ordinateur, le format, les
// réglages d'un flux, et la rétention des objets donnés à Kyber. Compilé par
// le Mac (cœur Rust, SessionEngine) et par l'Apple TV (KyberClient).

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

/// Objets passés à Kyber comme `userdata` des sorties de paquets (pipeline
/// vidéo, lecteur audio). Ils sont **retenus** le temps que Kyber puisse les
/// appeler : la tâche qui livre les paquets ne s'arrête qu'avec le message
/// d'arrêt de la session, derrière d'autres messages, et une libération à
/// délai fixe pouvait tomber trop tôt (plantage sur un pointeur libéré).
/// Kyber (correctif KyberNow) signale son dernier appel par `CLOSED` : c'est
/// là que l'objet est rendu. Au cas où la bibliothèque n'enverrait pas ce
/// signal, `releaseLater` rend l'objet 30 s après l'arrêt ; le premier des
/// deux gagne, l'autre ne fait rien.
enum PacketSinkLease {
    private static let lock = NSLock()
    private static var live = Set<UnsafeMutableRawPointer>()

    /// Retient `object` et rend le pointeur à donner à Kyber.
    static func lease<T: AnyObject>(_ object: T) -> UnsafeMutableRawPointer {
        let pointer = Unmanaged.passRetained(object).toOpaque()
        lock.withLock { _ = live.insert(pointer) }
        return pointer
    }

    /// Rend l'objet si c'est le premier à le demander.
    static func release<T: AnyObject>(_: T.Type, _ pointer: UnsafeMutableRawPointer) {
        let leased = lock.withLock { live.remove(pointer) != nil }
        if leased { Unmanaged<T>.fromOpaque(pointer).release() }
    }

    static func releaseLater<T: AnyObject>(_ type: T.Type, _ object: T, after seconds: TimeInterval = 30) {
        let pointer = Unmanaged.passUnretained(object).toOpaque()
        DispatchQueue.main.asyncAfter(deadline: .now() + seconds) { release(type, pointer) }
    }
}

