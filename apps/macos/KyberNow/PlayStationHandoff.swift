// « Envoyer à un appareil » et « Recevoir d'un autre appareil » dans l'onglet
// « PlayStation » de la fenêtre d'ajout (docs/link-ui.md, « Consoles
// PlayStation », « Plusieurs appareils » ; protocole : docs/playstation.md).
// Le paquet : PSShared (PlayStationSync.swift), chiffré par le cœur.

import KyberKit
import SwiftUI
import SystemConfiguration

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

    /// Le nom de ce Mac, montré par l'appareil qui envoie (« MacBook Pro de Léa »).
    static var deviceName: String { (SCDynamicStoreCopyComputerName(nil, nil) as String?) ?? "Mac" }

    // MARK: Envoi

    /// Cherche les appareils en attente toutes les 3 s environ.
    func startDiscovery() {
        guard discovering == nil else { return }
        discovering = Task { [weak self] in
            while !Task.isCancelled {
                let list = await Task.detached(priority: .utility) { PSHandoff.discover(wait: 1500) }.value
                guard let self, !Task.isCancelled else { return }
                // Ce Mac lui-même, s'il attend aussi un envoi.
                let me = Self.deviceName
                self.waiting = list.filter { !(self.code != nil && $0.name == me && $0.platform == "macos") }
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
        guard let r = PSHandoff.Receiver(name: Self.deviceName, platform: "macos") else {
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
                    guard let bundle = PSShared.decode(Data(text.utf8)) else {
                        KyberLog.app.error("PlayStation : paquet reçu illisible")
                        continue
                    }
                    let n = store.receive(bundle)
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

/// « Envoyer à un appareil » : la liste des appareils qui attendent, puis le code.
struct PSSendSection: View {
    @ObservedObject var model: PSHandoffModel
    @ObservedObject var store: PlayStationStore
    @State private var code = ""
    @FocusState private var focused: Bool

    private var digits: String { code.filter(\.isNumber) }

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            Text(PSText.sendTitle).font(Theme.display(17, .semibold)).foregroundStyle(Theme.text).padding(.top, 14)
            if model.waiting.isEmpty {
                Text(PSText.noDeviceWaiting).font(Theme.sans(13)).foregroundStyle(Theme.faint)
                    .fixedSize(horizontal: false, vertical: true)
            }
            ForEach(model.waiting) { w in
                Button {
                    model.choose(w)
                    code = ""
                    focused = true
                } label: {
                    PSListRow(title: "\(w.name) · \(PSPlatform.label(w.platform))", lit: model.chosen == w) {
                        Icon("arrow-right", size: 14, bold: true).foregroundStyle(Theme.muted)
                    }
                    .contentShape(Rectangle())
                }
                .buttonStyle(.plain)
            }
            if let to = model.chosen {
                Text(PSText.enterCodeOn(to.name)).font(Theme.sans(14)).foregroundStyle(Theme.text2)
                    .fixedSize(horizontal: false, vertical: true)
                HStack(spacing: 12) {
                    TextField(text: $code, prompt: Text(verbatim: "123 456").foregroundColor(Theme.ghost)) { EmptyView() }
                        .textFieldStyle(.plain)
                        .font(Theme.mono(26, .medium))
                        .tracking(3)
                        .foregroundStyle(Theme.accentLight)
                        .multilineTextAlignment(.center)
                        .focused($focused)
                        .onSubmit(send)
                        .padding(.horizontal, 14)
                        .frame(height: 58)
                        .background(Theme.bar, in: RoundedRectangle(cornerRadius: 14))
                        .overlay(RoundedRectangle(cornerRadius: 14).stroke(focused ? Theme.accentLine : Theme.line2))
                        .disabled(model.sending)
                        .accessibilityLabel(PSText.sixDigitCode)
                        .help(PSText.sixDigitCode)
                    Button(action: send) {
                        HStack(spacing: 10) {
                            if model.sending { ProgressView().controlSize(.small) }
                            Text(PSText.send)
                        }
                    }
                    .buttonStyle(PrimaryButton(height: 58, lit: model.sending))
                    .disabled(digits.count != 6 || model.sending)
                }
                if let r = model.sendResult { Notice(kind: r.ok ? .info : .warn, text: r.text) }
            }
        }
    }

    private func send() {
        guard digits.count == 6 else { return }
        model.send(code: digits, store: store)
    }
}

/// « Recevoir d'un autre appareil » : un code en grand, le temps de l'envoi.
struct PSReceiveSection: View {
    @ObservedObject var model: PSHandoffModel
    @ObservedObject var store: PlayStationStore

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            Text(PSText.receiveTitle).font(Theme.display(17, .semibold)).foregroundStyle(Theme.text).padding(.top, 14)
            if let code = model.code {
                Text(verbatim: PSHandoffModel.spaced(code))
                    .font(Theme.mono(44, .medium))
                    .tracking(6)
                    .foregroundStyle(Theme.accentLight)
                    .frame(maxWidth: .infinity)
                    .padding(.vertical, 14)
                    .background(Theme.bar, in: RoundedRectangle(cornerRadius: 14))
                    .overlay(RoundedRectangle(cornerRadius: 14).stroke(Theme.line2))
                    .textSelection(.enabled)
                    .accessibilityLabel(code.map(String.init).joined(separator: " "))
                Text(PSText.receiveHelp).font(Theme.sans(13)).foregroundStyle(Theme.text2)
                    .fixedSize(horizontal: false, vertical: true)
            } else if model.expired {
                Notice(kind: .warn, text: PSText.codeExpired)
                Button(PSText.newCode) { model.startReceiving(store: store) }
                    .buttonStyle(GhostButton(height: 40))
            } else {
                Button { model.startReceiving(store: store) } label: {
                    HStack(spacing: 8) { Icon("television-simple", size: 15, bold: true); Text(PSText.receiveTitle) }
                }
                .buttonStyle(GhostButton(height: 40))
            }
            if let r = model.received { Notice(kind: .info, text: r) }
        }
    }
}
