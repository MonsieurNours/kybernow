// « Envoyer à un appareil » et « Recevoir d'un autre appareil » dans l'onglet
// « PlayStation » de la fenêtre d'ajout (docs/link-ui.md, « Consoles
// PlayStation », « Plusieurs appareils » ; protocole : docs/playstation.md).
// Le paquet : PSShared (PlayStationSync.swift), chiffré par le cœur ; le
// modèle et les textes, communs aux apps Apple : PlayStationHandoffModel.swift.

import KyberKit
import SwiftUI

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
