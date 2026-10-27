// « Rejoindre avec un code » (KyberNow Share ; textes : docs/link-ui.md,
// « Partage ») : l'écran du code à six chiffres et du prénom (celui de la
// dernière fois, sinon le nom de ce Mac). Le code part vers les ordinateurs
// trouvés sur le réseau, celui qui partage répond ; la session d'invité
// s'ouvre alors, sans que l'ordinateur soit ajouté à cette liste.

import KyberKit
import SwiftUI

struct JoinSheet: View {
    @EnvironmentObject var app: AppModel
    @Environment(\.dismiss) private var dismiss
    @StateObject private var model: ShareJoinModel
    @FocusState private var codeFocused: Bool

    init(initialLink: ShareJoinLink?) {
        // « Auto » : l'écran de la fenêtre, où s'ouvre la session d'invité.
        _model = StateObject(wrappedValue: ShareJoinModel(link: initialLink) { DisplayInfo.shareScreen(NSApp.keyWindow?.screen) })
    }

    /// Les cinq qualités, « Auto » proposé ; le choix est retenu pour la fois suivante.
    private var qualityChoice: some View {
        VStack(alignment: .leading, spacing: 8) {
            Text(ShareText.imageQuality).font(Theme.sans(13, .semibold)).foregroundStyle(Theme.text2)
            Segmented(options: ShareQuality.allCases.map { ($0, $0.label) },
                      selection: Binding(get: { model.quality }, set: { model.quality = $0 ?? model.quality }),
                      height: 38, fill: true)
                .disabled(model.looking)
        }
        .accessibilityElement(children: .contain)
        .accessibilityLabel(ShareText.imageQuality)
    }

    /// Quatre ordinateurs au plus, invités compris.
    private var full: Bool { app.sessions.isFull }

    var body: some View {
        VStack(alignment: .leading, spacing: 22) {
            HStack(alignment: .top) {
                VStack(alignment: .leading, spacing: 6) {
                    Text(ShareText.joinTitle).font(Theme.display(30)).foregroundStyle(Theme.text)
                    Text(ShareText.joinHelp).font(Theme.sans(14)).foregroundStyle(Theme.muted)
                }
                Spacer()
                Button { close() } label: { Icon("x", size: 15, bold: true) }
                    .buttonStyle(GhostButton(height: 34))
                    .keyboardShortcut(.cancelAction)
                    .accessibilityLabel(StationText.cancel)
            }
            codeField
            LabeledField(label: ShareText.yourName, text: $model.name)
                .onSubmit(join)
            qualityChoice
            status
            HStack {
                Spacer()
                if model.looking {
                    Button(StationText.cancel) { model.cancel() }
                        .buttonStyle(GhostButton(height: 44))
                }
                Button(ShareText.joinButton, action: join)
                    .buttonStyle(PrimaryButton(lit: model.looking))
                    .disabled(!model.canJoin || full)
                    .keyboardShortcut(.defaultAction)
            }
        }
        .padding(.horizontal, 36).padding(.vertical, 32)
        .frame(width: 640)
        .background {
            ZStack {
                Theme.background
                RadialGradient(colors: [Theme.accentFill.opacity(0.22), .clear], center: .topTrailing, startRadius: 0, endRadius: 520)
            }
        }
        .preferredColorScheme(.dark)
        .onAppear {
            // Le rappel ne retient que l'app : en capturant la vue, qui tient
            // `model` (StateObject), le modèle se retiendrait lui-même.
            model.onJoined = { [app] found in app.openGuestSession(found) }
            model.start()
            codeFocused = true
        }
        .onDisappear {
            model.cancel()
            model.stop()
            model.onJoined = nil
        }
        .onChange(of: model.code) { _, _ in model.codeChanged() }
    }

    /// Les six chiffres, en grand, groupés par trois.
    private var codeField: some View {
        TextField(text: $model.code, prompt: Text(verbatim: "123 456").foregroundColor(Theme.ghost)) { EmptyView() }
            .textFieldStyle(.plain)
            .font(Theme.mono(34, .medium))
            .tracking(4)
            .foregroundStyle(Theme.accentLight)
            .multilineTextAlignment(.center)
            .focused($codeFocused)
            .onSubmit(join)
            .padding(.horizontal, 14)
            .frame(height: 78)
            .background(Theme.bar, in: RoundedRectangle(cornerRadius: 14))
            .overlay(RoundedRectangle(cornerRadius: 14).stroke(codeFocused ? Theme.accentLine : Theme.line2))
            .disabled(model.looking)
            .accessibilityLabel(ShareText.code)
    }

    @ViewBuilder private var status: some View {
        if full {
            Notice(kind: .warn, text: LinkText.computersLimit)
        } else {
            switch model.state {
            case .idle:
                EmptyView()
            case .looking:
                HStack(spacing: 10) {
                    ProgressView().controlSize(.small)
                    Text(ShareText.looking).font(Theme.sans(13)).foregroundStyle(Theme.muted)
                }
            case let .failed(text):
                Notice(kind: .warn, text: text)
            }
        }
    }

    private func join() {
        guard model.canJoin, !full else { return }
        model.join(hosts: app.store.hosts)
    }

    private func close() {
        model.cancel()
        dismiss()
    }
}
