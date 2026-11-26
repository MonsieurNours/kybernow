// Le panneau du test du profil Auto sur Mac (docs/link-ui.md, « Profil Auto
// et débit adaptatif ») : une feuille sur la fenêtre de l'accueil, ouverte
// par Auto choisi sans test gardé ou par « Refaire le test ». Le modèle et
// les textes sont communs aux apps Apple (AutoProfile.swift).

import KyberKit
import SwiftUI

/// Présente le panneau du test par-dessus l'accueil et les réglages.
struct AutoTestPresenter: ViewModifier {
    @ObservedObject var model: AutoTestModel

    func body(content: Content) -> some View {
        content.sheet(item: $model.panel) { _ in AutoTestSheet(model: model) }
    }
}

struct AutoTestSheet: View {
    @ObservedObject var model: AutoTestModel

    var body: some View {
        VStack(alignment: .leading, spacing: 22) {
            HStack(alignment: .top, spacing: 14) {
                Icon("sparkle", size: 20)
                    .foregroundStyle(Theme.accent)
                    .frame(width: 40, height: 40)
                    .background(Theme.accentSoft, in: RoundedRectangle(cornerRadius: 11, style: .continuous))
                VStack(alignment: .leading, spacing: 6) {
                    Text(AutoText.title).font(Theme.display(26)).foregroundStyle(Theme.text)
                        .accessibilityAddTraits(.isHeader)
                    Text(AutoText.subtitle).font(Theme.sans(14)).foregroundStyle(Theme.muted)
                        .fixedSize(horizontal: false, vertical: true)
                }
            }
            switch model.phase {
            case let .running(steps):
                VStack(alignment: .leading, spacing: 14) {
                    ForEach(steps) { AutoStepRow(step: $0) }
                }
                HStack {
                    Spacer()
                    Button(AutoText.cancel) { model.cancel() }
                        .buttonStyle(GhostButton(height: 44))
                        .keyboardShortcut(.cancelAction)
                }
            case let .done(_, choices):
                VStack(alignment: .leading, spacing: 12) {
                    Theme.eyebrow(AutoText.summary, color: Theme.muted)
                    VStack(alignment: .leading, spacing: 0) {
                        ForEach(Array(choices.enumerated()), id: \.element.id) { i, c in
                            if i > 0 { Divider().overlay(Theme.line) }
                            AutoChoiceRow(choice: c)
                        }
                    }
                    .padding(.horizontal, 16)
                    .glass(Theme.radiusCard, tint: 0.55)
                }
                HStack(spacing: 12) {
                    Button(AutoText.again) { model.start() }
                        .buttonStyle(.plain)
                        .font(Theme.sans(13, .semibold)).foregroundStyle(Theme.accent)
                    Spacer()
                    Button(AutoText.cancel) { model.cancel() }
                        .buttonStyle(GhostButton(height: 44))
                        .keyboardShortcut(.cancelAction)
                    Button(AutoText.apply) { model.apply() }
                        .buttonStyle(PrimaryButton())
                        .keyboardShortcut(.defaultAction)
                }
            case let .failed(text):
                Notice(kind: .danger, text: text)
                HStack {
                    Spacer()
                    Button(AutoText.cancel) { model.cancel() }
                        .buttonStyle(GhostButton(height: 44))
                        .keyboardShortcut(.cancelAction)
                    Button(AutoText.retry) { model.start() }
                        .buttonStyle(PrimaryButton())
                        .keyboardShortcut(.defaultAction)
                }
            }
        }
        .padding(32)
        .frame(width: 560)
        .background(Theme.background)
        .preferredColorScheme(.dark)
        .animation(Theme.easeOut, value: model.phase)
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
                    Circle().strokeBorder(Theme.line3, lineWidth: 1.5).frame(width: 18, height: 18)
                case .running:
                    ProgressView().controlSize(.small)
                case .done:
                    Icon("check-circle", size: 20).foregroundStyle(Theme.ok)
                case .skipped:
                    Icon("arrow-right", size: 16).foregroundStyle(Theme.faint)
                }
            }
            .frame(width: 22, height: 22)
            VStack(alignment: .leading, spacing: 3) {
                Text(step.title).font(Theme.sans(15, .semibold))
                    .foregroundStyle(step.state == .waiting ? Theme.faint : Theme.text)
                if let detail = step.detail {
                    Text(detail).font(Theme.sans(13)).foregroundStyle(Theme.muted)
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
        HStack(alignment: .firstTextBaseline, spacing: 16) {
            Text(choice.title).font(Theme.sans(13)).foregroundStyle(Theme.muted)
                .frame(width: 150, alignment: .leading)
            VStack(alignment: .leading, spacing: 2) {
                Text(choice.value).font(Theme.sans(14, .semibold)).foregroundStyle(Theme.text)
                Text(choice.reason).font(Theme.sans(12)).foregroundStyle(Theme.faint)
                    .fixedSize(horizontal: false, vertical: true)
            }
            Spacer(minLength: 0)
        }
        .padding(.vertical, 11)
        .accessibilityElement(children: .combine)
    }
}
