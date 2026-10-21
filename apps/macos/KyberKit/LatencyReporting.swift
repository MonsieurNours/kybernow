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

    public init() {}
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

    public var all: [Double] { [capture, encode, send, network, decode, display] }
    public var sum: Double { all.reduce(0, +) }
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
