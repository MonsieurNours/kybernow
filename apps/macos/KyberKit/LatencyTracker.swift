// Latence de bout en bout et temps de chaque étape de la chaîne : capture,
// encodage et envoi sur l'ordinateur, réseau, décodage et affichage sur le Mac.
//
// Le serveur horodate chaque image à chaque étape (« acquired », « encoding »,
// « encoded », « sent », sur son horloge) et l'envoie avec les mesures de
// Kyber ; les « pings » de Kyber donnent le décalage entre son horloge et
// celle du client (offset = serveur − client, en µs, horloge CLOCK_MONOTONIC
// côté Mac). Le rendu signale l'instant où chaque image est reçue, décodée et
// montrée ; on en tire la médiane de chaque seconde.
//
// Kyber donne aussi, toutes les quelques secondes, l'aller-retour réseau et
// les paquets perdus (cumulés depuis le début du flux). Une image envoyée par
// l'ordinateur qui n'est ni montrée ni remplacée par une plus récente est
// perdue (paquets manquants, ou image refusée en attendant une image clé).

import Foundation
@_implementationOnly import CKyber

/// Temps de chaque étape d'une image, en millisecondes (médianes de la seconde).
public struct ChainTimes: Sendable, Equatable {
    /// Capture → début de l'encodage (attente sur l'ordinateur).
    public var capture: Double = 0
    public var encode: Double = 0
    /// Fin de l'encodage → envoi (mise en paquets, cadence).
    public var send: Double = 0
    /// Envoi → image complète reçue par le Mac.
    public var network: Double = 0
    public var decode: Double = 0
    /// Décodée → montrée à l'écran (rendu, attente de la synchro).
    public var display: Double = 0

    public init() {}

    public var all: [Double] { [capture, encode, send, network, decode, display] }
    public var sum: Double { all.reduce(0, +) }
}

final class LatencyTracker {
    private struct Sent {
        var acquired: Int64?
        var encoding: Int64?
        var encoded: Int64?
        var sent: Int64?
        /// Arrivée de la première mesure, sur l'horloge du Mac (µs).
        let seen: Int64
    }

    /// Mesures de la seconde écoulée.
    struct Second {
        var latencyMs: Double?
        var chain: ChainTimes?
        /// Images envoyées par l'ordinateur jamais montrées.
        var lostFrames = 0
        /// Trous dans le flux (paquets perdus au-delà de la correction d'erreurs).
        var holes = 0
        var keyframeRequests = 0
        /// Paquets perdus d'après Kyber, depuis la mesure précédente.
        var lostPackets = 0
        /// Aller-retour réseau (dernière mesure), en millisecondes.
        var rttMs: Double?
        var bytes = 0
    }

    private let lock = NSLock()
    /// pts → étapes sur l'ordinateur (µs, son horloge).
    private var frames: [UInt64: Sent] = [:]
    private var order: [UInt64] = []
    /// Images déjà montrées ou remplacées : une mesure en retard ne les rouvre pas.
    private var done: Set<UInt64> = []
    private var doneOrder: [UInt64] = []
    private var offsets: [Int64] = []
    private var samples: [Double] = []
    private var stages: [[Double]] = Array(repeating: [], count: 6)
    private var second = Second()
    private var rttMs: Double?
    /// Derniers cumuls de paquets perdus (local, distant).
    private var lostTotals: [Int64] = [-1, -1]
    /// Toute la session, pour la médiane gardée sur l'accueil.
    private var session: [Double] = []
    /// Mesures reçues de l'ordinateur par étape (journal, une fois par flux).
    private var kinds = [0, 0, 0, 0]
    private var kindsLogged = false

    /// Rappel C donné à Kyber (`userdata` : ce suivi, non retenu).
    static let callback: kyclient_MetricsCb = { metric, userdata in
        guard let metric, let userdata else { return }
        let me = Unmanaged<LatencyTracker>.fromOpaque(userdata).takeUnretainedValue()
        let m = metric.pointee
        switch m.type_ {
        case KYCLIENT_METRIC_TYPE_VIDEO:
            me.server(m.data.video.data, pts: m.data.video.pts, at: m.data.video.ts)
        case KYCLIENT_METRIC_TYPE_NETWORK_PING:
            me.ping(offset: m.data.network_ping.offset_micros)
        case KYCLIENT_METRIC_TYPE_NETWORK_LOCAL:
            me.network(m.data.network_local, slot: 0)
        case KYCLIENT_METRIC_TYPE_NETWORK_REMOTE:
            me.network(m.data.network_remote, slot: 1)
        default:
            break
        }
    }

    func reset() {
        lock.lock()
        frames.removeAll()
        order.removeAll()
        done.removeAll()
        doneOrder.removeAll()
        samples.removeAll()
        stages = Array(repeating: [], count: 6)
        second = Second()
        rttMs = nil
        lostTotals = [-1, -1]
        session.removeAll()
        kinds = [0, 0, 0, 0]
        kindsLogged = false
        lock.unlock()
    }

    private func server(_ kind: kyclient_VideoMetricData, pts: UInt64, at ts: Int64) {
        lock.lock()
        defer { lock.unlock() }
        let slot: Int
        switch kind {
        case KYCLIENT_VIDEO_METRIC_DATA_FRAME_ACQUIRED: slot = 0
        case KYCLIENT_VIDEO_METRIC_DATA_FRAME_ENCODING: slot = 1
        case KYCLIENT_VIDEO_METRIC_DATA_FRAME_ENCODED: slot = 2
        case KYCLIENT_VIDEO_METRIC_DATA_FRAME_SENT: slot = 3
        default: return // étapes du client : l'app les mesure elle-même
        }
        kinds[slot] += 1
        guard !done.contains(pts) else { return }
        var f = frames[pts] ?? {
            order.append(pts)
            return Sent(seen: Self.nowMicros())
        }()
        switch slot {
        case 0: f.acquired = ts
        case 1: f.encoding = ts
        case 2: f.encoded = ts
        default: f.sent = ts
        }
        frames[pts] = f
        // Deux secondes d'images à 240 Hz au plus.
        while order.count > 480 { expire(order.removeFirst()) }
    }

    /// Image jamais montrée : perdue si l'ordinateur l'avait envoyée.
    private func expire(_ pts: UInt64) {
        if let f = frames.removeValue(forKey: pts), f.sent != nil { second.lostFrames += 1 }
    }

    private func finish(_ pts: UInt64) {
        if frames.removeValue(forKey: pts) != nil, let i = order.firstIndex(of: pts) { order.remove(at: i) }
        if done.insert(pts).inserted {
            doneOrder.append(pts)
            if doneOrder.count > 480 { done.remove(doneOrder.removeFirst()) }
        }
    }

    private func ping(offset: Int64) {
        lock.lock()
        defer { lock.unlock() }
        offsets.append(offset)
        if offsets.count > 15 { offsets.removeFirst() }
    }

    private func network(_ stats: kyclient_NetworkStats, slot: Int) {
        lock.lock()
        defer { lock.unlock() }
        if slot == 0, stats.rtt_micros >= 0 { rttMs = Double(stats.rtt_micros) / 1000 }
        if slot == 1, rttMs == nil, stats.rtt_micros >= 0 { rttMs = Double(stats.rtt_micros) / 1000 }
        guard stats.packets_lost >= 0 else { return }
        let last = lostTotals[slot]
        // Cumul : on garde l'écart. S'il recule, le flux a redémarré.
        if last >= 0 { second.lostPackets += Int(stats.packets_lost >= last ? stats.packets_lost - last : stats.packets_lost) }
        lostTotals[slot] = stats.packets_lost
    }

    /// Paquet d'image reçu de Kyber (fil réseau).
    func received(bytes: Int) {
        lock.lock()
        second.bytes += bytes
        lock.unlock()
    }

    /// Trou dans le flux.
    func hole() {
        lock.lock()
        second.holes += 1
        lock.unlock()
    }

    func keyframeRequested() {
        lock.lock()
        second.keyframeRequests += 1
        lock.unlock()
    }

    /// Image remplacée par une plus récente avant d'être montrée.
    func skipped(pts: UInt64) {
        lock.lock()
        finish(pts)
        lock.unlock()
    }

    /// Image `pts` reçue, décodée puis montrée (CLOCK_MONOTONIC, µs).
    func displayed(pts: UInt64, received: Int64, decoded: Int64, at micros: Int64) {
        lock.lock()
        defer { lock.unlock() }
        let f = frames[pts]
        finish(pts)
        guard let f, let captured = f.acquired, !offsets.isEmpty else { return }
        let offset = offsets.sorted()[offsets.count / 2]
        let ms = Double(micros - (captured - offset)) / 1000
        // Écarte les valeurs absurdes (horloge pas encore recalée).
        guard ms > 0, ms < 2000 else { return }
        samples.append(ms)

        guard let encoding = f.encoding, let encoded = f.encoded, let sent = f.sent else { return }
        let steps = [
            encoding - captured, encoded - encoding, sent - encoded,
            received - (sent - offset), decoded - received, micros - decoded,
        ].map { Double($0) / 1000 }
        // L'horloge recalée peut faire passer le réseau un rien sous zéro sur un réseau local.
        for (i, v) in steps.enumerated() { stages[i].append(max(0, v)) }
    }

    /// Mesures depuis le dernier appel (une fois par seconde).
    func drain() -> Second {
        lock.lock()
        defer { lock.unlock() }
        // Images vues il y a plus d'une seconde et toujours pas montrées.
        let limit = Self.nowMicros() - 1_000_000
        while let first = order.first, let f = frames[first], f.seen < limit {
            order.removeFirst()
            expire(first)
        }
        if !kindsLogged, kinds[0] > 0 {
            kindsLogged = true
            KyberLog.video.notice("mesures de l'ordinateur : capture \(self.kinds[0]), encodage \(self.kinds[1]), encodée \(self.kinds[2]), envoi \(self.kinds[3]), décalage d'horloge \(self.offsets.isEmpty ? "absent" : "présent", privacy: .public)")
        }
        var out = second
        out.rttMs = rttMs
        if !samples.isEmpty {
            let median = Self.median(samples)
            out.latencyMs = median
            session.append(median)
        }
        if !stages[0].isEmpty {
            let m = stages.map(Self.median)
            var chain = ChainTimes()
            (chain.capture, chain.encode, chain.send, chain.network, chain.decode, chain.display) = (m[0], m[1], m[2], m[3], m[4], m[5])
            out.chain = chain
        }
        samples.removeAll(keepingCapacity: true)
        for i in stages.indices { stages[i].removeAll(keepingCapacity: true) }
        second = Second()
        return out
    }

    private static func median(_ values: [Double]) -> Double {
        values.isEmpty ? 0 : values.sorted()[values.count / 2]
    }

    /// Médiane de la session (des médianes de chaque seconde).
    var sessionMedian: Double? {
        lock.lock()
        defer { lock.unlock() }
        guard !session.isEmpty else { return nil }
        return session.sorted()[session.count / 2]
    }

    /// 95e centile de la session (des médianes de chaque seconde) : la latence
    /// des mauvais moments, montrée avec la médiane sur l'accueil.
    var sessionP95: Double? {
        lock.lock()
        defer { lock.unlock() }
        guard !session.isEmpty else { return nil }
        let sorted = session.sorted()
        return sorted[min(sorted.count - 1, Int(Double(sorted.count) * 0.95))]
    }

    /// Maintenant, sur l'horloge des mesures de Kyber (µs).
    static func nowMicros() -> Int64 {
        Int64(clock_gettime_nsec_np(CLOCK_MONOTONIC) / 1000)
    }
}
