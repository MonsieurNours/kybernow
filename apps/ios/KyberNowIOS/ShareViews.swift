// KyberNow Share sur iPhone et iPad (docs/link-ui.md, « Partage »).
// L'invité : « Rejoindre avec un code », l'écran du code à six chiffres et du
// prénom ; la session d'invité s'ouvre ensuite, sans que l'ordinateur soit
// ajouté. L'hôte : le panneau de « Partager » dans la barre de session (code,
// QR code, « Copier le lien », invités et ce qu'on peut faire de chacun,
// « Nouveau code », « Arrêter le partage »). Les appels et les règles sont
// dans Share.swift, commun aux apps d'Apple.

import KyberKit
import SwiftUI
import UIKit

// MARK: - Rejoindre

struct JoinView: View {
    @EnvironmentObject private var app: AppModel
    @EnvironmentObject private var store: HostStore
    @Environment(\.dismiss) private var dismiss
    @StateObject private var model: ShareJoinModel
    @FocusState private var codeFocused: Bool

    init(link: ShareJoinLink?) {
        _model = StateObject(wrappedValue: ShareJoinModel(link: link))
    }

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 20) {
                HStack(alignment: .top) {
                    Text(ShareText.joinTitle)
                        .font(Theme.display(34, 660)).tracking(-34 * 0.03)
                        .foregroundStyle(Theme.text)
                        .fixedSize(horizontal: false, vertical: true)
                        .accessibilityAddTraits(.isHeader)
                    Spacer(minLength: 12)
                    Button { close() } label: {
                        Icon("x", size: 16).foregroundStyle(Theme.muted).frame(width: 44, height: 44)
                    }
                    .accessibilityLabel(Text(StationText.cancel))
                }
                Text(ShareText.joinHelp).font(Theme.sans(15)).foregroundStyle(Theme.muted)
                    .fixedSize(horizontal: false, vertical: true)
                codeField
                AddHostField(label: ShareText.yourName, text: $model.name, submit: .go) { join() }
                status
                Button(ShareText.joinButton) { join() }
                    .buttonStyle(PrimaryButton(height: 54, fontSize: 17))
                    .disabled(!model.canJoin)
                    .opacity(model.canJoin ? 1 : 0.45)
                    .saturation(model.canJoin ? 1 : 0.6)
                if model.looking {
                    Button(StationText.cancel) { model.cancel() }
                        .buttonStyle(GlassButton(height: 48, fontSize: 16))
                        .frame(maxWidth: .infinity)
                }
            }
            .padding(24)
            .frame(maxWidth: 560, alignment: .leading)
            .frame(maxWidth: .infinity)
        }
        .scrollDismissesKeyboard(.interactively)
        .background(Theme.background)
        .environment(\.addHostWide, DeviceDisplay.isPad)
        .onAppear {
            model.onJoined = { found in
                dismiss()
                // La feuille se ferme, puis la session s'ouvre à sa place.
                DispatchQueue.main.asyncAfter(deadline: .now() + 0.35) { app.openGuestSession(found) }
            }
            model.start()
            codeFocused = true
        }
        .onDisappear {
            model.cancel()
            model.stop()
        }
        .onChange(of: model.code) { _, _ in model.codeChanged() }
    }

    /// Les six chiffres, en grand, groupés par trois.
    private var codeField: some View {
        TextField(text: $model.code, prompt: Text(verbatim: "123 456").foregroundStyle(Theme.ghost)) { Text(ShareText.code) }
            .keyboardType(.numberPad)
            .textContentType(.oneTimeCode)
            .autocorrectionDisabled()
            .focused($codeFocused)
            .font(Theme.mono(40, .medium))
            .tracking(4)
            .foregroundStyle(Theme.neonCore)
            .tint(Theme.accent)
            .multilineTextAlignment(.center)
            .padding(.horizontal, 16)
            .frame(height: 76)
            .background(RoundedRectangle(cornerRadius: 14, style: .continuous).fill(Theme.surface))
            .overlay(RoundedRectangle(cornerRadius: 14, style: .continuous).strokeBorder(codeFocused ? Theme.accent : Theme.line2))
            .disabled(model.looking)
            .accessibilityLabel(Text(ShareText.code))
    }

    @ViewBuilder private var status: some View {
        switch model.state {
        case .idle:
            EmptyView()
        case .looking:
            HStack(spacing: 10) {
                HomeSpinner(size: 13).foregroundStyle(Theme.accentLight).frame(width: 16, height: 16)
                Text(ShareText.looking).font(Theme.sans(14)).foregroundStyle(Theme.muted)
            }
            .accessibilityElement(children: .combine)
        case let .failed(text):
            AddHostNotice(kind: .warn, text: text)
        }
    }

    private func join() {
        guard model.canJoin else { return }
        codeFocused = false
        model.join(hosts: store.hosts)
    }

    private func close() {
        model.cancel()
        dismiss()
    }
}

// MARK: - Partager (hôte)

/// Le panneau de l'hôte : code, QR code, « Copier le lien », invités, actions.
struct SharePanelView: View {
    @ObservedObject var session: SessionController
    @ObservedObject var share: ShareHostModel
    @Environment(\.dismiss) private var dismiss
    @State private var copyResult: String?
    @State private var copied = false
    @State private var qr: (link: String, image: CGImage)?

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 18) {
                HStack(alignment: .top) {
                    Text(ShareText.title(session.host.name))
                        .font(Theme.sans(20, .semibold)).foregroundStyle(Theme.text)
                        .fixedSize(horizontal: false, vertical: true)
                        .accessibilityAddTraits(.isHeader)
                    Spacer(minLength: 12)
                    Button { dismiss() } label: {
                        Icon("x", size: 16).foregroundStyle(Theme.muted).frame(width: 44, height: 44)
                    }
                    .accessibilityLabel(Text("Fermer"))
                }
                if let reason = session.shareAvailability.reason {
                    AddHostNotice(kind: .warn, text: reason)
                } else if let state = share.state {
                    shared(state)
                } else if let error = share.error {
                    AddHostNotice(kind: .danger, text: error)
                    Button(ShareText.share) { Task { await share.openIfNeeded() } }
                        .buttonStyle(PrimaryButton(height: 48, fontSize: 16))
                        .disabled(share.busy)
                } else {
                    HomeSpinner(size: 16).foregroundStyle(Theme.accentLight)
                }
            }
            .padding(20)
        }
        .background(Theme.bar)
        .environment(\.colorScheme, .dark)
    }

    @ViewBuilder private func shared(_ state: ShareState) -> some View {
        ViewThatFits(in: .horizontal) {
            HStack(alignment: .top, spacing: 20) {
                qrImage(for: state.link)
                codeAndCopy(state)
            }
            VStack(alignment: .leading, spacing: 16) {
                qrImage(for: state.link)
                codeAndCopy(state)
            }
        }
        Text(ShareText.help).font(Theme.sans(13.5)).foregroundStyle(Theme.muted).fixedSize(horizontal: false, vertical: true)
        Rectangle().fill(Theme.line).frame(height: 1)
        VStack(alignment: .leading, spacing: 12) {
            Theme.eyebrow(ShareText.guests, color: Theme.faint)
            if state.guests.isEmpty {
                Text(ShareText.nobody).font(Theme.sans(15)).foregroundStyle(Theme.muted)
            } else {
                ForEach(state.guests) { guest in GuestRow(share: share, guest: guest) }
            }
            if share.full {
                Text(ShareText.fourGuests).font(Theme.sans(13)).foregroundStyle(Theme.faint)
            }
        }
        if let error = share.error { AddHostNotice(kind: .danger, text: error) }
        HStack(spacing: 10) {
            Button(ShareText.newCode) { Task { await share.newCode() } }
                .buttonStyle(GlassButton(height: 48, fontSize: 15))
            Button(ShareText.stop) { Task { await share.close() } }
                .buttonStyle(AddHostDangerButton())
        }
        .disabled(share.busy)
    }

    private func codeAndCopy(_ state: ShareState) -> some View {
        VStack(alignment: .leading, spacing: 10) {
            Theme.eyebrow(ShareText.code, color: Theme.faint)
            Text(verbatim: Share.formatCode(state.code))
                .font(Theme.mono(36, .medium)).tracking(2)
                .foregroundStyle(Theme.neonCore)
                .neon(.lit)
                .fixedSize()
                .accessibilityLabel(Text(verbatim: "\(ShareText.code) \(state.code.map(String.init).joined(separator: " "))"))
            Button { copyLink() } label: {
                HStack(spacing: 8) {
                    Icon(copied ? "check" : "copy", size: 16, bold: true)
                    Text(ShareText.copyLink)
                }
            }
            .buttonStyle(GlassButton(height: 44, fontSize: 15))
            if let copyResult {
                Text(copyResult).font(Theme.sans(13)).foregroundStyle(copied ? Theme.ok : Theme.warnText)
                    .fixedSize(horizontal: false, vertical: true)
            }
        }
    }

    /// Le QR code du lien des apps (`kybernow://join…`), net, sur fond blanc.
    @ViewBuilder private func qrImage(for link: String) -> some View {
        let image = qr?.link == link ? qr?.image : Share.qrImage(for: link)
        Group {
            if let image {
                Image(decorative: image, scale: 1)
                    .interpolation(.none)
                    .resizable()
                    .aspectRatio(1, contentMode: .fit)
                    .frame(width: 150, height: 150)
                    .clipShape(RoundedRectangle(cornerRadius: 10, style: .continuous))
                    .accessibilityElement()
                    .accessibilityLabel(ShareText.qrLabel)
                    .onAppear { if qr?.link != link { qr = (link, image) } }
            } else {
                RoundedRectangle(cornerRadius: 10, style: .continuous).fill(Theme.surface).frame(width: 150, height: 150)
            }
        }
    }

    private func copyLink() {
        guard let text = share.linkToCopy else { return }
        UIPasteboard.general.string = text
        let ok = UIPasteboard.general.hasStrings
        copied = ok
        copyResult = ok ? ShareText.linkCopied : ShareText.copyFailed
        Task {
            try? await Task.sleep(for: .seconds(3))
            copyResult = nil
            copied = false
        }
    }
}

/// Un invité : son prénom et son rôle, son app et depuis quand, et ce que
/// l'hôte peut en faire (rôle, « Retirer »).
private struct GuestRow: View {
    @ObservedObject var share: ShareHostModel
    let guest: ShareGuest

    var body: some View {
        HStack(alignment: .center, spacing: 12) {
            Icon(guest.role.plays ? "game-controller" : "eye", size: 20)
                .foregroundStyle(guest.role.plays ? Theme.accent : Theme.faint)
                .frame(width: 24)
            VStack(alignment: .leading, spacing: 3) {
                Text(ShareText.guest(guest.name, guest.role)).font(Theme.sans(15, .medium)).foregroundStyle(Theme.text)
                    .fixedSize(horizontal: false, vertical: true)
                TimelineView(.periodic(from: .now, by: 20)) { _ in
                    Text(detail).font(Theme.sans(12.5)).foregroundStyle(Theme.faint)
                }
            }
            Spacer(minLength: 8)
            Menu {
                ForEach(Array(ShareHostModel.roleActions(for: guest.role).enumerated()), id: \.offset) { _, action in
                    Button(action.title) { Task { await share.setRole(action.role, of: guest) } }
                }
                Divider()
                Button(ShareText.remove, role: .destructive) { Task { await share.kick(guest) } }
            } label: {
                Icon("sliders-horizontal", size: 20).foregroundStyle(Theme.text2).frame(width: 44, height: 44)
                    .glass(12)
            }
            .disabled(share.busy)
            .accessibilityLabel(Text(guest.name))
        }
    }

    /// « iPhone ou iPad · depuis 3 min ».
    private var detail: String {
        [ShareText.app(guest.app), guest.since.map(ShareText.since)].compactMap { $0 }.joined(separator: " · ")
    }
}
