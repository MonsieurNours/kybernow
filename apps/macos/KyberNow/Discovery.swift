// Trouver un ordinateur : lien ou QR code de sa console (`kybernow://`),
// annonce Bonjour sur le réseau local. Commun au Mac, à l'Apple TV, à
// l'iPhone et à l'iPad.

import Combine
import Foundation
import KyberKit
import Network

/// Un ordinateur trouvé, pas encore ajouté.
struct Candidate: Equatable {
    var name: String
    var address: String
    var port: UInt16
    /// Empreinte annoncée (lien, QR code, Bonjour) : à comparer à celle vue.
    var expected: String?
}

/// Lien `kybernow://adresse:port?name=…&fp=sha256:…` (QR code de la console).
enum PairingLink {
    static func parse(_ text: String) -> Candidate? {
        guard let url = URL(string: text.trimmingCharacters(in: .whitespacesAndNewlines)),
              url.scheme == "kybernow", let host = url.host(percentEncoded: false), let port = url.port,
              let port16 = UInt16(exactly: port)
        else { return nil }
        let items = URLComponents(url: url, resolvingAgainstBaseURL: false)?.queryItems ?? []
        let name = items.first { $0.name == "name" }?.value ?? host
        let fp = items.first { $0.name == "fp" }?.value
        return Candidate(name: name, address: host, port: port16, expected: fp)
    }
}

// MARK: - Découverte (Bonjour)

@MainActor
final class Discovery: ObservableObject {
    struct Found: Identifiable, Equatable {
        let id: String
        let name: String
        let fingerprint: String?
        let endpoint: NWEndpoint
    }

    @Published private(set) var found: [Found] = []
    private var browser: NWBrowser?

    func start() {
        guard browser == nil else { return }
        let browser = NWBrowser(for: .bonjourWithTXTRecord(type: "_kybernow._tcp", domain: nil), using: .tcp)
        browser.browseResultsChangedHandler = { [weak self] results, _ in
            let list = results.compactMap { r -> Found? in
                guard case let .service(name, _, _, _) = r.endpoint else { return nil }
                var txt: [String: String] = [:]
                if case let .bonjour(record) = r.metadata { txt = record.dictionary }
                // `kyber=0` : console sans serveur Kyber derrière (développement,
                // installation incomplète), aucune session n'y démarrerait. Sans
                // la clé (console d'une version précédente) : proposée.
                if txt["kyber"] == "0" { return nil }
                return Found(id: name, name: txt["name"] ?? name, fingerprint: txt["fp"], endpoint: r.endpoint)
            }
            Task { @MainActor in self?.found = list.sorted { $0.name < $1.name } }
        }
        // Échec (accès au réseau local refusé puis accordé, mDNS relancé) :
        // relancé 3 s plus tard au lieu de rester muet jusqu'au prochain
        // lancement (MAC-21).
        browser.stateUpdateHandler = { [weak self, weak browser] state in
            guard case let .failed(error) = state else { return }
            Task { @MainActor in
                guard let self, let browser, self.browser === browser else { return }
                KyberLog.app.error("découverte : \(String(describing: error), privacy: .public), relancée dans 3 s")
                browser.cancel()
                self.browser = nil
                self.restart?.cancel()
                let again = DispatchWorkItem { [weak self] in
                    guard let self, self.wanted else { return }
                    self.start()
                }
                self.restart = again
                DispatchQueue.main.asyncAfter(deadline: .now() + 3, execute: again)
            }
        }
        browser.start(queue: .main)
        self.browser = browser
        wanted = true
    }

    func stop() {
        wanted = false
        restart?.cancel()
        restart = nil
        browser?.cancel()
        browser = nil
    }

    /// Démarrée par l'app (et pas arrêtée depuis) : une relance après échec a lieu.
    private var wanted = false
    private var restart: DispatchWorkItem?

    /// Adresse IPv4 et port d'un service annoncé.
    static func resolve(_ found: Found) async -> Candidate? {
        let params = NWParameters.tcp
        (params.defaultProtocolStack.internetProtocol as? NWProtocolIP.Options)?.version = .v4
        let connection = NWConnection(to: found.endpoint, using: params)
        let once = Once()
        return await withCheckedContinuation { continuation in
            connection.stateUpdateHandler = { state in
                switch state {
                case .ready:
                    guard once.claim() else { return }
                    var result: Candidate?
                    if case let .hostPort(host, port) = connection.currentPath?.remoteEndpoint {
                        var address = "\(host)"
                        if let percent = address.firstIndex(of: "%") { address = String(address[..<percent]) }
                        result = Candidate(name: found.name, address: address, port: port.rawValue, expected: found.fingerprint)
                    }
                    connection.cancel()
                    continuation.resume(returning: result)
                case .failed, .cancelled:
                    guard once.claim() else { return }
                    continuation.resume(returning: nil)
                default:
                    break
                }
            }
            connection.start(queue: .global())
            // Sans réponse en 5 s : l'annulation passe par .cancelled.
            DispatchQueue.global().asyncAfter(deadline: .now() + 5) { connection.cancel() }
        }
    }
}

/// Vrai une seule fois, quel que soit le fil.
final class Once: @unchecked Sendable {
    private let lock = NSLock()
    private var done = false

    func claim() -> Bool {
        lock.lock()
        defer { lock.unlock() }
        if done { return false }
        done = true
        return true
    }
}
