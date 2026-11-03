// « Envoyer à un appareil » et « Recevoir d'un autre appareil » (docs/link-ui.md,
// « Consoles PlayStation », « Plusieurs appareils » ; protocole :
// docs/playstation.md) : les textes et le modèle, communs au Mac, à l'Apple TV,
// à l'iPhone et à l'iPad (ni AppKit ni UIKit, sauf le nom de l'appareil). Les
// vues sont à chaque app (Mac : PlayStationHandoffViews.swift). Le paquet :
// PSShared (PlayStationSync.swift), chiffré par le cœur (KyberKit, `PSHandoff`).

import Combine
import Foundation
import KyberKit
#if os(macOS)
import SystemConfiguration
#else
import UIKit
#endif

extension PSText {
    static var sendTitle: String { String(localized: "Envoyer à un appareil") }
    static var noDeviceWaiting: String { String(localized: "Aucun appareil en attente. Sur la télé ou l'autre appareil : Ajouter une console › Recevoir d'un autre appareil.") }
    static func enterCodeOn(_ name: String) -> String { String(localized: "Saisissez le code affiché sur \(name).") }
    static var sixDigitCode: String { String(localized: "Code à 6 chiffres") }
    static var send: String { String(localized: "Envoyer") }
    static func sent(to name: String) -> String { String(localized: "Comptes et consoles envoyés à \(name).") }
    static func sendRefused(_ name: String) -> String { String(localized: "Code refusé : vérifiez-le sur \(name).") }
    static func sendUnreachable(_ name: String) -> String { String(localized: "\(name) ne répond plus.") }
    static var receiveTitle: String { String(localized: "Recevoir d'un autre appareil") }
    static var receiveHelp: String { String(localized: "Sur votre Mac, votre PC ou votre téléphone : Ajouter un ordinateur ou une console › PlayStation › Envoyer à un appareil, puis saisissez ce code.") }
    static func received(accounts: Int, consoles: Int) -> String { String(localized: "\(accounts) compte(s) et \(consoles) console(s) reçus.") }
    static var codeExpired: String { String(localized: "Code expiré.") }
    static var newCode: String { String(localized: "Nouveau code") }
}

/// L'envoi et la réception, le temps que l'onglet est à l'écran.
@MainActor
final class PSHandoffModel: ObservableObject {
    /// Appareils du réseau qui attendent un envoi.
    @Published private(set) var waiting: [PSHandoff.Waiting] = []
    /// L'appareil choisi, à qui envoyer.
    @Published var chosen: PSHandoff.Waiting?
    @Published private(set) var sending = false
    /// Le résultat du dernier envoi (texte, réussi).
    @Published private(set) var sendResult: (text: String, ok: Bool)?
    /// Le code affiché (nil : pas en attente d'un envoi).
    @Published private(set) var code: String?
    @Published private(set) var expired = false
    @Published private(set) var received: String?

    private var discovering: Task<Void, Never>?
    private var listening: Task<Void, Never>?
    private var receiver: PSHandoff.Receiver?

    /// Le nom de cet appareil, montré par l'appareil qui envoie (« MacBook Pro de Léa »).
    static var deviceName: String {
        #if os(macOS)
        (SCDynamicStoreCopyComputerName(nil, nil) as String?) ?? "Mac"
        #else
        UIDevice.current.name
        #endif
    }

    /// La plateforme annoncée aux recherches (docs/playstation.md, « Le paquet »).
    static var platform: String {
        #if os(macOS)
        "macos"
        #elseif os(tvOS)
        "tvos"
        #else
        UIDevice.current.userInterfaceIdiom == .pad ? "ipados" : "ios"
        #endif
    }

    // MARK: Envoi

    /// Cherche les appareils en attente toutes les 3 s environ.
    func startDiscovery() {
        guard discovering == nil else { return }
        discovering = Task { [weak self] in
            while !Task.isCancelled {
                let list = await Task.detached(priority: .utility) { PSHandoff.discover(wait: 1500) }.value
                guard let self, !Task.isCancelled else { return }
                // Ce Mac lui-même, s'il attend aussi un envoi.
                let me = Self.deviceName, platform = Self.platform
                self.waiting = list.filter { !(self.code != nil && $0.name == me && $0.platform == platform) }
                    .sorted { $0.name.localizedStandardCompare($1.name) == .orderedAscending }
                try? await Task.sleep(for: .seconds(2))
            }
        }
    }

    func choose(_ w: PSHandoff.Waiting) {
        guard !sending else { return }
        chosen = w
        sendResult = nil
    }

    /// Envoie tout d'un coup (comptes, associations, consoles) à l'appareil choisi.
    func send(code: String, store: PlayStationStore) {
        guard let to = chosen, !sending, let bundle = store.shared(secrets: true, from: Self.deviceName).text else { return }
        sending = true
        sendResult = nil
        let digits = code.filter(\.isNumber)
        Task {
            let result = await Task.detached { PSHandoff.send(to: to.address, code: digits, bundle: bundle) }.value
            sending = false
            switch result {
            case .success:
                KyberLog.app.notice("PlayStation : comptes et consoles envoyés (\(to.platform, privacy: .public))")
                sendResult = (PSText.sent(to: to.name), true)
            case .failure(.refused):
                sendResult = (PSText.sendRefused(to.name), false)
            case let .failure(.unreachable(detail)):
                KyberLog.app.notice("PlayStation : envoi impossible (\(detail, privacy: .public))")
                sendResult = (PSText.sendUnreachable(to.name), false)
            }
        }
    }

    // MARK: Réception

    /// Écoute (5 minutes au plus) et affiche le code ; chaque paquet reçu est
    /// fusionné tout de suite.
    func startReceiving(store: PlayStationStore) {
        stopReceiving()
        expired = false
        received = nil
        guard let r = PSHandoff.Receiver(name: Self.deviceName, platform: Self.platform) else {
            KyberLog.app.error("PlayStation : attente d'un envoi impossible")
            return
        }
        receiver = r
        code = r.code
        listening = Task { [weak self] in
            while !Task.isCancelled {
                let event = await Task.detached { r.next(timeout: 400) }.value
                guard let self, !Task.isCancelled else { return }
                switch event {
                case let .received(text):
                    guard let n = store.receive(json: text) else { continue }
                    self.received = PSText.received(accounts: n.accounts, consoles: n.consoles)
                case let .newCode(c):
                    self.code = c
                case .expired:
                    self.expired = true
                    self.receiver?.stop()
                    self.receiver = nil
                    return
                case nil:
                    continue
                }
            }
        }
    }

    func stopReceiving() {
        listening?.cancel()
        listening = nil
        receiver?.stop()
        receiver = nil
        code = nil
    }

    func stop() {
        discovering?.cancel()
        discovering = nil
        stopReceiving()
        expired = false
    }

    /// « 482 913 ».
    static func spaced(_ code: String) -> String {
        code.count == 6 ? "\(code.prefix(3)) \(code.suffix(3))" : code
    }
}
