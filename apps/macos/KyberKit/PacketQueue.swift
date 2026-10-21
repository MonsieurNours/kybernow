// File d'envoi vers kynput, et file de secours des paquets qui ne se
// rattrapent pas. Partagé par toutes les entrées produites par l'app (Mac et
// Apple TV) ; les manettes lues par SDL sur Mac n'y passent pas (kynput les
// envoie lui-même).
//
// La file d'envoi de kynput tient 64 paquets et jette sans retour (le
// protocole est en deltas : un relâchement jeté = touche collée jusqu'à la
// fin de la session). kynput_consumer_consume rend -1 quand elle est pleine,
// ou tant que le réseau n'est pas relié : on compte les refus, et les paquets
// qui ne se rattrapent pas sont réessayés depuis une petite file de secours,
// toutes les 3 ms, dans l'ordre, jusqu'à succès ou 200 ms.
//
// `consume` est l'envoi à kynput (kynput_consumer_consume sur le puits du
// pipeline) ; le test hors ligne apps/macos/tests/inputs lui en substitue un
// qui refuse à volonté.

import Foundation
@_implementationOnly import CKyber

final class PacketQueue {
    /// Envoi d'un paquet ; 0 : parti, sinon refusé (kynput garde le paquet dans
    /// les deux cas).
    typealias Consume = (OpaquePointer) -> Int32

    static let retryWindow: TimeInterval = 0.2
    static let retryStep: DispatchTimeInterval = .milliseconds(3)

    /// Paquets refusés par kynput, jamais arrivés (journal : premier refus,
    /// puis tous les 100, puis le total à la fin de la session).
    private(set) var droppedPackets = 0
    /// Journal de chaque premier envoi par sorte (`"touche"`, `"clic"`…).
    var onFirst: ((_ kind: String, _ rc: Int32) -> Void)?
    /// Journal d'un refus (`final` : perdu après les réessais), avec le total.
    var onDropped: ((_ kind: String, _ final: Bool, _ total: Int) -> Void)?

    private let consume: Consume
    private let queue: DispatchQueue
    private let lock = NSLock()
    private var logged = Set<String>()
    /// Paquet qui ne se rattrape pas, à réessayer : comment le refaire (kynput
    /// garde le paquet même refusé), et jusqu'à quand.
    private struct Pending {
        let kind: String
        let make: () -> OpaquePointer?
        let deadline: TimeInterval
    }
    private var backlog: [Pending] = []
    private var retryScheduled = false
    private var stopped = false

    /// `queue` : file série des réessais.
    init(queue: DispatchQueue, consume: @escaping Consume) {
        self.queue = queue
        self.consume = consume
    }

    /// Plus rien ne part ; la file de secours est vidée. Rend le total des
    /// paquets perdus pendant la session.
    func stop() -> Int {
        lock.lock()
        defer { lock.unlock() }
        stopped = true
        backlog.removeAll()
        return droppedPackets
    }

    /// Des paquets attendent encore un réessai.
    var hasBacklog: Bool {
        lock.lock()
        defer { lock.unlock() }
        return !backlog.isEmpty
    }

    /// Paquet qui se rattrape tout seul au suivant (position, déplacement,
    /// molette, axe en mouvement) : envoyé une fois, perdu s'il est refusé.
    /// Derrière une file de secours non vide, il n'est pas envoyé non plus :
    /// il doublerait ce qui attend (le retour au repos d'un axe, puis sa
    /// nouvelle valeur). `false` : perdu. Appelable de tout fil.
    @discardableResult
    func send(_ packet: OpaquePointer?, _ kind: String) -> Bool {
        guard let packet else { return false }
        lock.lock()
        defer { lock.unlock() }
        if stopped || !backlog.isEmpty {
            kynput_packet_unref(packet)
            if !stopped { dropped(kind) }
            return false
        }
        let rc = consume(packet)
        logFirst(kind, rc)
        if rc != 0 { dropped(kind) }
        return rc == 0
    }

    /// Paquet qui ne se rattrape pas (touche, clic, bouton, branchement, retour
    /// au repos d'un axe, FlushKeys) : réessayé toutes les 3 ms, dans l'ordre,
    /// jusqu'à succès ou 200 ms. Tant que la file de secours n'est pas vide,
    /// tout y passe : un relâchement ne doit pas doubler son appui.
    func sendReliably(_ kind: String, _ make: @escaping () -> OpaquePointer?) {
        lock.lock()
        defer { lock.unlock() }
        guard !stopped else { return }
        if backlog.isEmpty {
            guard let packet = make() else { return }
            let rc = consume(packet)
            logFirst(kind, rc)
            if rc == 0 { return }
        }
        backlog.append(Pending(kind: kind, make: make, deadline: ProcessInfo.processInfo.systemUptime + Self.retryWindow))
        scheduleRetry()
    }

    // Sous le verrou.
    private func scheduleRetry() {
        guard !retryScheduled else { return }
        retryScheduled = true
        queue.asyncAfter(deadline: .now() + Self.retryStep) { [weak self] in self?.retry() }
    }

    private func retry() {
        lock.lock()
        defer { lock.unlock() }
        retryScheduled = false
        guard !stopped else {
            backlog.removeAll()
            return
        }
        while let next = backlog.first {
            guard let packet = next.make() else {
                backlog.removeFirst()
                continue
            }
            if consume(packet) == 0 {
                backlog.removeFirst()
                continue
            }
            if ProcessInfo.processInfo.systemUptime > next.deadline {
                backlog.removeFirst()
                dropped(next.kind, final: true)
                continue
            }
            scheduleRetry()
            return
        }
    }

    // Sous le verrou.
    private func logFirst(_ kind: String, _ rc: Int32) {
        if logged.insert(kind).inserted { onFirst?(kind, rc) }
    }

    // Sous le verrou.
    private func dropped(_ kind: String, final: Bool = false) {
        droppedPackets += 1
        if final || droppedPackets == 1 || droppedPackets % 100 == 0 {
            onDropped?(kind, final, droppedPackets)
        }
    }
}
