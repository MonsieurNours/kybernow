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

}

public enum VideoCodec: String, Sendable, CaseIterable, Codable {
    /// `pyrowave` : décodé par le GPU (`PyroWaveDecoder`), seulement si la
    /// Station le propose (docs/pyrowave.md, docs/link-ui.md « PyroWave »).
    case h264, hevc, av1, pyrowave
}
