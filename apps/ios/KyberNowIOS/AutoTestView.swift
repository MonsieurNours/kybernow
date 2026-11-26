// Le panneau du test du profil Auto sur iPhone et iPad (docs/link-ui.md,
// « Profil Auto et débit adaptatif ») : une fiche, par-dessus l'accueil ou
// par-dessus les réglages de l'ordinateur. Le modèle et les textes sont
// communs aux apps Apple (apps/macos/KyberNow/AutoProfile.swift).

import KyberKit
import SwiftUI

/// Présente le panneau du test quand il s'ouvre depuis `origin`.
struct AutoTestPresenter: ViewModifier {
    @ObservedObject var model: AutoTestModel
    let origin: AutoTestModel.Origin

    func body(content: Content) -> some View {
        content.sheet(item: Binding(get: { model.panel?.origin == origin ? model.panel : nil },
                                    set: { if $0 == nil, model.panel?.origin == origin { model.cancel() } })) { _ in
            AutoTestView(model: model)
        }
    }
}

struct AutoTestView: View {
    @ObservedObject var model: AutoTestModel

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 22) {
                HStack(alignment: .top, spacing: 14) {
                    Icon("sparkle", size: 22)
                        .foregroundStyle(Theme.accent)
                        .frame(width: 44, height: 44)
                        .background(Theme.accentFill.opacity(0.16), in: RoundedRectangle(cornerRadius: 12, style: .continuous))
                    VStack(alignment: .leading, spacing: 6) {
                        Text(AutoText.title).font(Theme.display(28)).foregroundStyle(Theme.text)
                            .accessibilityAddTraits(.isHeader)
                        Text(AutoText.subtitle).font(Theme.sans(15)).foregroundStyle(Theme.muted)
                            .fixedSize(horizontal: false, vertical: true)
                    }
                }
                switch model.phase {
                case let .running(steps):
                    VStack(alignment: .leading, spacing: 16) {
                        ForEach(steps) { AutoStepRow(step: $0) }
                    }
                    Button(AutoText.cancel) { model.cancel() }
                        .buttonStyle(GlassButton(height: 50, fontSize: 16))
                        .frame(maxWidth: .infinity)
                case let .done(_, choices):
                    Theme.eyebrow(AutoText.summary)
                    VStack(alignment: .leading, spacing: 0) {
                        ForEach(Array(choices.enumerated()), id: \.element.id) { i, c in
                            if i > 0 { Rectangle().fill(Theme.line).frame(height: 1) }
                            AutoChoiceRow(choice: c)
                        }
                    }
                    .padding(.horizontal, 16)
                    .glass(16)
                    Button(AutoText.apply) { model.apply() }
                        .buttonStyle(PrimaryButton(height: 54, fontSize: 17))
                    HStack {
                        Button(AutoText.again) { model.start() }
                            .font(Theme.sans(15, .semibold)).foregroundStyle(Theme.accentLight)
                            .frame(minHeight: 44)
                        Spacer()
                        Button(AutoText.cancel) { model.cancel() }
                            .buttonStyle(GlassButton(height: 44, fontSize: 15))
                    }
                case let .failed(text):
                    HStack(alignment: .top, spacing: 10) {
                        Icon("warning", size: 18).foregroundStyle(Theme.danger)
                        Text(text).font(Theme.sans(15)).foregroundStyle(Theme.dangerText)
                            .fixedSize(horizontal: false, vertical: true)
                    }
                    Button(AutoText.retry) { model.start() }
                        .buttonStyle(PrimaryButton(height: 54, fontSize: 17))
                    Button(AutoText.cancel) { model.cancel() }
                        .buttonStyle(GlassButton(height: 50, fontSize: 16))
                        .frame(maxWidth: .infinity)
                }
            }
            .padding(24)
            .frame(maxWidth: 560, alignment: .leading)
            .frame(maxWidth: .infinity)
            .animation(Theme.easeOut, value: model.phase)
        }
        .background(Theme.background)
        .presentationDetents([.large])
        .interactiveDismissDisabled(isRunning)
    }

    private var isRunning: Bool {
        if case .running = model.phase { return true }
        return false
    }
}

/// Une étape : en attente, en cours (une roue), cochée avec sa mesure, ou passée avec sa raison.
private struct AutoStepRow: View {
    let step: AutoTestStep

    var body: some View {
        HStack(alignment: .top, spacing: 14) {
            ZStack {
                switch step.state {
                case .waiting:
                    Circle().strokeBorder(Theme.line2, lineWidth: 1.5).frame(width: 20, height: 20)
                case .running:
                    ProgressView().tint(Theme.accentLight)
                case .done:
                    Icon("check-circle", size: 22).foregroundStyle(Theme.ok)
                case .skipped:
                    Icon("arrow-right", size: 18).foregroundStyle(Theme.faint)
                }
            }
            .frame(width: 24, height: 24)
            VStack(alignment: .leading, spacing: 3) {
                Text(step.title).font(Theme.sans(16, .semibold))
                    .foregroundStyle(step.state == .waiting ? Theme.faint : Theme.text)
                if let detail = step.detail {
                    Text(detail).font(Theme.sans(14)).foregroundStyle(Theme.muted)
                        .fixedSize(horizontal: false, vertical: true)
                }
            }
            Spacer(minLength: 0)
        }
        .accessibilityElement(children: .combine)
    }
}

/// Une ligne du résumé : le réglage, sa valeur, sa raison.
private struct AutoChoiceRow: View {
    let choice: AutoTestChoice

    var body: some View {
        VStack(alignment: .leading, spacing: 3) {
            HStack(alignment: .firstTextBaseline) {
                Text(choice.title).font(Theme.sans(14)).foregroundStyle(Theme.muted)
                Spacer(minLength: 12)
                Text(choice.value).font(Theme.sans(15, .semibold)).foregroundStyle(Theme.text)
                    .multilineTextAlignment(.trailing)
            }
            Text(choice.reason).font(Theme.sans(13)).foregroundStyle(Theme.faint)
                .fixedSize(horizontal: false, vertical: true)
        }
        .padding(.vertical, 12)
        .accessibilityElement(children: .combine)
    }
}
