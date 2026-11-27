// Ce que le rendu dit au suivi de latence, et ce qu'il en reçoit chaque
// seconde : le cœur Rust, par `SessionEngine` (docs/coeur-apple.md) ; les
// tests du rendu s'en passent.

import Foundation

/// Mesures de la seconde écoulée.
public struct LatencySecond: Sendable {
    public var latencyMs: Double?
    public var chain: ChainTimes?
    /// Images montrées, images avec une latence mesurée, images montrées
    /// sans mesure (du serveur) ou avec une valeur absurde.
    public var displayed = 0
    public var measured = 0
    public var rejected = 0
    /// Images envoyées par l'ordinateur jamais montrées.
    public var lostFrames = 0
    /// Trous dans le flux (paquets perdus au-delà de la correction d'erreurs).
    public var holes = 0
    public var keyframeRequests = 0
    /// Paquets perdus d'après Kyber, depuis la mesure précédente.
    public var lostPackets = 0
    /// Aller-retour réseau (dernière mesure), en millisecondes.
    public var rttMs: Double?
    public var bytes = 0
    /// PyroWave : la définition ramenée en place (`KnSecond.lowered`), nil sinon.
    public var lowered: LoweredMode?
    /// Débit visé de la seconde (`KnSecond.has_target`), nil sans.
    public var target: TargetBitrate?

    public init() {}
}

/// Débit visé (Statistiques, ligne « Débit visé » sous « Débit » et trait
/// sur la courbe du débit ; docs/link-ui.md) : ce que la session demande à
/// l'ordinateur, le débit du profil (ou réglé à la main), en bit/s, et la
/// raison (`KN_TARGET_*`). Le texte vient du cœur (`text`, `label`).
public struct TargetBitrate: Sendable, Equatable {
    public var bitrate: UInt32
    public var profile: UInt32
    public var reason: UInt32

    public init(bitrate: UInt32, profile: UInt32, reason: UInt32) {
        (self.bitrate, self.profile, self.reason) = (bitrate, profile, reason)
    }

    /// Le débit visé en Mbit/s (le trait de la courbe).
    public var mbps: Double { Double(bitrate) / 1_000_000 }
}

/// PyroWave (docs/pyrowave.md, « Définition ramenée ») : la définition que le
/// cœur a demandée à l'ordinateur pour tenir dans `mbps`.
public struct LoweredMode: Sendable, Equatable {
    public var width: Int
    public var height: Int
    public var refresh: Int
    public var mbps: Int

    public init(width: Int, height: Int, refresh: Int, mbps: Int) {
        (self.width, self.height, self.refresh, self.mbps) = (width, height, refresh, mbps)
    }
}

/// Temps de chaque étape d'une image, en millisecondes (médianes de la seconde).
public struct ChainTimes: Sendable, Equatable {
    /// Capture → début de l'encodage (attente sur l'ordinateur).
    public var capture: Double = 0
    public var encode: Double = 0
    /// Fin de l'encodage → envoi (mise en paquets, cadence).
    public var send: Double = 0
    /// Envoi → image complète reçue par l'appareil.
    public var network: Double = 0
    public var decode: Double = 0
    /// Décodée → montrée à l'écran (rendu, attente de la synchro).
    public var display: Double = 0

    public init() {}

}

protocol LatencyReporting: AnyObject {
    /// Paquet d'image reçu de Kyber (fil réseau).
    func received(bytes: Int)
    /// Trou dans le flux.
    func hole()
    func keyframeRequested()
    /// Image remplacée par une plus récente avant d'être montrée.
    func skipped(pts: UInt64)
    /// Image `pts` reçue, décodée puis montrée (µs, `monotonicMicros`).
    func displayed(pts: UInt64, received: Int64, decoded: Int64, at micros: Int64)
    /// Mesures depuis le dernier appel (une fois par seconde).
    func drain() -> LatencySecond
}

/// Maintenant, sur l'horloge des mesures de Kyber (µs, CLOCK_MONOTONIC).
func monotonicMicros() -> Int64 {
    Int64(clock_gettime_nsec_np(CLOCK_MONOTONIC) / 1000)
}
