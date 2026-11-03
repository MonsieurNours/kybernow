// « Envoyer à un appareil » et « Recevoir d'un autre appareil », en bas de la
// voie « PlayStation » de l'ajout (docs/link-ui.md, « Consoles PlayStation »,
// « Plusieurs appareils »). Le modèle et les textes sont communs aux apps
// Apple (PlayStationHandoffModel.swift) ; le paquet, PSShared
// (PlayStationSync.swift), chiffré par le cœur.
//
// Recherche et réception passent par la diffusion UDP du réseau local : sur
// un vrai iPhone, sans le droit « multicast » d'Apple, iOS peut la refuser
// (README, « Essais et limites »).

import KyberKit
import SwiftUI

struct PSHandoffSection: View {
    @ObservedObject var store: PlayStationStore
    @StateObject private var model = PSHandoffModel()

    var body: some View {
        VStack(alignment: .leading, spacing: 24) {
            // Envoyer : seulement depuis un appareil qui a des comptes.
            if !store.accounts.isEmpty { PSSendPane(model: model, store: store) }
            PSReceivePane(model: model, store: store)
        }
        .onAppear { if !store.accounts.isEmpty { model.startDiscovery() } }
        .onChange(of: store.accounts.isEmpty) { _, empty in if !empty { model.startDiscovery() } }
        .onDisappear { model.stop() }
    }
}

private struct PSHandoffTitle: View {
    let text: String
    @Environment(\.addHostWide) private var wide

    var body: some View {
        Text(text).font(Theme.display(wide ? 22 : 20, 640)).foregroundStyle(Theme.text)
            .fixedSize(horizontal: false, vertical: true)
            .accessibilityAddTraits(.isHeader)
    }
}

/// La liste des appareils qui attendent, puis le code affiché sur celui choisi.
private struct PSSendPane: View {
    @ObservedObject var model: PSHandoffModel
    @ObservedObject var store: PlayStationStore
    @State private var code = ""
    @FocusState private var focused: Bool

    private var digits: String { code.filter(\.isNumber) }

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            PSHandoffTitle(text: PSText.sendTitle)
            if model.waiting.isEmpty {
                Text(PSText.noDeviceWaiting).font(Theme.sans(14)).foregroundStyle(Theme.muted)
                    .fixedSize(horizontal: false, vertical: true)
            }
            ForEach(model.waiting) { w in
                Button {
                    model.choose(w)
                    code = ""
                    focused = true
                } label: {
                    PSRow(title: "\(w.name) · \(PSPlatform.label(w.platform))", lit: model.chosen == w) {
                        Icon("arrow-right", size: 14, bold: true).foregroundStyle(Theme.muted)
                    }
                    .contentShape(Rectangle())
                }
                .buttonStyle(.plain)
            }
            if let to = model.chosen {
                Text(PSText.enterCodeOn(to.name)).font(Theme.sans(15)).foregroundStyle(Theme.text2)
                    .fixedSize(horizontal: false, vertical: true)
                TextField(text: $code, prompt: Text(verbatim: "123 456").foregroundColor(Theme.faint)) { EmptyView() }
                    .keyboardType(.numberPad)
                    .textContentType(.oneTimeCode)
                    .font(Theme.mono(28, .medium))
                    .tracking(3)
                    .foregroundStyle(Theme.accentLight)
                    .multilineTextAlignment(.center)
                    .focused($focused)
                    .onChange(of: code) { _, v in if v.filter(\.isNumber).count > 6 { code = String(v.filter(\.isNumber).prefix(6)) } }
                    .padding(.horizontal, 14)
                    .frame(height: 60)
                    .background(RoundedRectangle(cornerRadius: 14, style: .continuous).fill(Color.white.opacity(0.05)))
                    .overlay(RoundedRectangle(cornerRadius: 14, style: .continuous).strokeBorder(focused ? Theme.accent.opacity(0.6) : Color.white.opacity(0.1)))
                    .disabled(model.sending)
                    .accessibilityLabel(PSText.sixDigitCode)
                Button(action: send) {
                    HStack(spacing: 10) {
                        if model.sending { ProgressView().tint(.white) }
                        Text(PSText.send)
                    }
                }
                .buttonStyle(PrimaryButton())
                .disabled(digits.count != 6 || model.sending)
                .opacity(digits.count != 6 ? 0.5 : 1)
                if let r = model.sendResult {
                    if r.ok { PSInfoNotice(text: r.text) } else { AddHostNotice(kind: .warn, text: r.text) }
                }
            }
        }
    }

    private func send() {
        guard digits.count == 6 else { return }
        focused = false
        model.send(code: digits, store: store)
    }
}

/// Un code en grand, le temps de l'envoi (5 min au plus).
private struct PSReceivePane: View {
    @ObservedObject var model: PSHandoffModel
    @ObservedObject var store: PlayStationStore

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            PSHandoffTitle(text: PSText.receiveTitle)
            if let code = model.code {
                Text(verbatim: PSHandoffModel.spaced(code))
                    .font(Theme.mono(44, .medium))
                    .tracking(6)
                    .foregroundStyle(Theme.accentLight)
                    .frame(maxWidth: .infinity)
                    .padding(.vertical, 14)
                    .background(RoundedRectangle(cornerRadius: 14, style: .continuous).fill(Color.white.opacity(0.05)))
                    .overlay(RoundedRectangle(cornerRadius: 14, style: .continuous).strokeBorder(Color.white.opacity(0.1)))
                    .textSelection(.enabled)
                    .accessibilityLabel(code.map(String.init).joined(separator: " "))
                Text(PSText.receiveHelp).font(Theme.sans(14)).foregroundStyle(Theme.text2)
                    .fixedSize(horizontal: false, vertical: true)
            } else if model.expired {
                AddHostNotice(kind: .warn, text: PSText.codeExpired)
                Button(PSText.newCode) { model.startReceiving(store: store) }
                    .buttonStyle(GlassButton())
            } else {
                Button { model.startReceiving(store: store) } label: {
                    HStack(spacing: 8) { Icon("television-simple", size: 16, bold: true); Text(PSText.receiveTitle) }
                }
                .buttonStyle(GlassButton())
            }
            if let r = model.received { PSInfoNotice(text: r) }
        }
    }
}
