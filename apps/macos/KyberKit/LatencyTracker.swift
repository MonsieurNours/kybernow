// Latence de bout en bout : de la capture de l'image sur l'ordinateur à son
// affichage sur l'écran du Mac.
//
// Le serveur horodate chaque image (« acquired », sur son horloge QPC) et
// l'envoie avec les mesures de Kyber ; les « pings » de Kyber donnent le
// décalage entre son horloge et celle du client (offset = serveur − client,
// en µs, horloge CLOCK_MONOTONIC côté Mac). Le rendu signale l'instant où
// chaque image est montrée ; on en tire la médiane de chaque seconde.

import Foundation
@_implementationOnly import CKyber

final class LatencyTracker {
    private let lock = NSLock()
    /// pts → instant de capture sur l'ordinateur (µs, son horloge).
    private var acquired: [UInt64: Int64] = [:]
    private var order: [UInt64] = []
    private var offsets: [Int64] = []
    private var samples: [Double] = []
    /// Toute la session, pour la médiane gardée sur l'accueil.
    private var session: [Double] = []

    /// Rappel C donné à Kyber (`userdata` : ce suivi, non retenu).
    static let callback: kyclient_MetricsCb = { metric, userdata in
        guard let metric, let userdata else { return }
        let me = Unmanaged<LatencyTracker>.fromOpaque(userdata).takeUnretainedValue()
        let m = metric.pointee
        switch m.type_ {
        case KYCLIENT_METRIC_TYPE_VIDEO where m.data.video.data == KYCLIENT_VIDEO_METRIC_DATA_FRAME_ACQUIRED:
            me.serverAcquired(pts: m.data.video.pts, at: m.data.video.ts)
        case KYCLIENT_METRIC_TYPE_NETWORK_PING:
            me.ping(offset: m.data.network_ping.offset_micros)
        default:
            break
        }
    }

    func reset() {
        lock.lock()
        acquired.removeAll()
        order.removeAll()
        samples.removeAll()
        session.removeAll()
        lock.unlock()
    }

    private func serverAcquired(pts: UInt64, at ts: Int64) {
        lock.lock()
        defer { lock.unlock() }
        if acquired.updateValue(ts, forKey: pts) == nil { order.append(pts) }
        // Deux secondes d'images à 240 Hz au plus.
        if order.count > 480 {
            let old = order.removeFirst()
            acquired[old] = nil
        }
    }

    private func ping(offset: Int64) {
        lock.lock()
        defer { lock.unlock() }
        offsets.append(offset)
        if offsets.count > 15 { offsets.removeFirst() }
    }

    /// Image `pts` montrée à `micros` (CLOCK_MONOTONIC).
    func displayed(pts: UInt64, at micros: Int64) {
        lock.lock()
        defer { lock.unlock() }
        guard let captured = acquired.removeValue(forKey: pts), !offsets.isEmpty else { return }
        let offset = offsets.sorted()[offsets.count / 2]
        let ms = Double(micros - (captured - offset)) / 1000
        // Écarte les valeurs absurdes (horloge pas encore recalée).
        guard ms > 0, ms < 2000 else { return }
        samples.append(ms)
    }

    /// Médiane depuis le dernier appel (une fois par seconde), nil sans mesure.
    func drainMedian() -> Double? {
        lock.lock()
        defer { lock.unlock() }
        guard !samples.isEmpty else { return nil }
        let median = samples.sorted()[samples.count / 2]
        samples.removeAll(keepingCapacity: true)
        session.append(median)
        return median
    }

    /// Médiane de la session (des médianes de chaque seconde).
    var sessionMedian: Double? {
        lock.lock()
        defer { lock.unlock() }
        guard !session.isEmpty else { return nil }
        return session.sorted()[session.count / 2]
    }

    /// Maintenant, sur l'horloge des mesures de Kyber (µs).
    static func nowMicros() -> Int64 {
        Int64(clock_gettime_nsec_np(CLOCK_MONOTONIC) / 1000)
    }
}
