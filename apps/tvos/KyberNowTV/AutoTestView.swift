// Le panneau du test du profil Auto sur l'Apple TV (docs/link-ui.md,
// « Profil Auto et débit adaptatif ») : plein écran, par-dessus l'accueil ou
// les réglages de l'ordinateur ; la télécommande va aux boutons. Le modèle et
// les textes sont communs aux apps Apple (apps/macos/KyberNow/AutoProfile.swift).

import KyberKit
import SwiftUI

/// Présente le panneau du test, d'où qu'il s'ouvre.
struct AutoTestPresenter: ViewModifier {
    @ObservedObject var model: AutoTestModel

    func body(content: Content) -> some View {
        content.fullScreenCover(item: Binding(get: { model.panel }, set: { if $0 == nil, model.panel != nil { model.cancel() } })) { _ in
            AutoTestView(model: model)
        }
    }
}

struct AutoTestView: View {
    @ObservedObject var model: AutoTestModel
    @FocusState private var focus: Target?

    enum Target: Hashable { case cancel, apply, again, retry }

    var body: some View {
        ZStack {
            Theme.background.ignoresSafeArea()
            VStack(alignment: .leading, spacing: 40) {
                HStack(alignment: .top, spacing: 24) {
                    Icon("sparkle", size: 40)
                        .foregroundStyle(Theme.accent)
                        .frame(width: 80, height: 80)
                        .background(Theme.accentFill.opacity(0.16), in: RoundedRectangle(cornerRadius: 20, style: .continuous))
                    VStack(alignment: .leading, spacing: 10) {
                        Text(AutoText.title).font(Theme.display(56)).foregroundStyle(Theme.text)
                        Text(AutoText.subtitle).font(Theme.sans(26)).foregroundStyle(Theme.muted)
                    }
                }
                content
                Spacer(minLength: 0)
            }
            .padding(.horizontal, 160).padding(.top, 110).padding(.bottom, 80)
            .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
        }
        .onExitCommand { model.cancel() }
        .animation(Theme.easeOut, value: model.phase)
        .onChange(of: model.phase, initial: true) { _, phase in
            switch phase {
            case .running: focus = .cancel
            case .done: focus = .apply
            case .failed: focus = .retry
            }
        }
    }

    @ViewBuilder private var content: some View {
        switch model.phase {
        case let .running(steps):
            VStack(alignment: .leading, spacing: 28) {
                ForEach(steps) { AutoStepRow(step: $0) }
            }
            Button(AutoText.cancel) { model.cancel() }
                .buttonStyle(ChoiceButton(selected: false, height: 72))
                .focused($focus, equals: .cancel)
        case let .done(_, choices):
            Theme.eyebrow(AutoText.summary)
            VStack(alignment: .leading, spacing: 0) {
                ForEach(Array(choices.enumerated()), id: \.element.id) { i, c in
                    if i > 0 { Rectangle().fill(Theme.line).frame(height: 1) }
                    AutoChoiceRow(choice: c)
                }
            }
            .padding(.horizontal, 32)
            .glass(28)
            HStack(spacing: 20) {
                Button(AutoText.apply) { model.apply() }
                    .buttonStyle(PrimaryButton(height: 80, fontSize: 28))
                    .frame(width: 420)
                    .focused($focus, equals: .apply)
                Button(AutoText.cancel) { model.cancel() }
                    .buttonStyle(ChoiceButton(selected: false, height: 80))
                    .focused($focus, equals: .cancel)
                Button(AutoText.again) { model.start() }
                    .buttonStyle(ChoiceButton(selected: false, height: 80))
                    .focused($focus, equals: .again)
            }
            .focusSection()
        case let .failed(text):
            HStack(alignment: .top, spacing: 18) {
                Icon("warning", size: 34).foregroundStyle(Theme.danger)
                Text(text).font(Theme.sans(28)).foregroundStyle(Theme.dangerText)
            }
            HStack(spacing: 20) {
                Button(AutoText.retry) { model.start() }
                    .buttonStyle(PrimaryButton(height: 80, fontSize: 28))
                    .frame(width: 420)
                    .focused($focus, equals: .retry)
                Button(AutoText.cancel) { model.cancel() }
                    .buttonStyle(ChoiceButton(selected: false, height: 80))
                    .focused($focus, equals: .cancel)
            }
            .focusSection()
        }
    }
}

/// Une étape : en attente, en cours (une roue), cochée avec sa mesure, ou passée avec sa raison.
private struct AutoStepRow: View {
    let step: AutoTestStep

    var body: some View {
        HStack(alignment: .top, spacing: 24) {
            ZStack {
                switch step.state {
                case .waiting:
                    Circle().strokeBorder(Theme.line2, lineWidth: 2.5).frame(width: 34, height: 34)
                case .running:
                    ProgressView()
                case .done:
                    Icon("check-circle", size: 40).foregroundStyle(Theme.ok)
                case .skipped:
                    Icon("arrow-right", size: 32).foregroundStyle(Theme.faint)
                }
            }
            .frame(width: 44, height: 44)
            VStack(alignment: .leading, spacing: 6) {
                Text(step.title).font(Theme.sans(30, .semibold))
                    .foregroundStyle(step.state == .waiting ? Theme.faint : Theme.text)
                if let detail = step.detail {
                    Text(detail).font(Theme.sans(24)).foregroundStyle(Theme.muted)
                }
            }
        }
        .accessibilityElement(children: .combine)
    }
}

/// Une ligne du résumé : le réglage, sa valeur, sa raison.
private struct AutoChoiceRow: View {
    let choice: AutoTestChoice

    var body: some View {
        HStack(alignment: .firstTextBaseline, spacing: 28) {
            Text(choice.title).font(Theme.sans(24)).foregroundStyle(Theme.muted)
                .frame(width: 360, alignment: .leading)
            Text(choice.value).font(Theme.sans(26, .semibold)).foregroundStyle(Theme.text)
                .frame(width: 420, alignment: .leading)
            Text(choice.reason).font(Theme.sans(22)).foregroundStyle(Theme.faint)
            Spacer(minLength: 0)
        }
        .padding(.vertical, 16)
        .accessibilityElement(children: .combine)
    }
}
