// « Réglage du HDR » (docs/hdr.md, docs/link-ui.md, section HDR) : quatre
// étapes, chacune une mire dessinée en HDR par l'app (`HDRPatternView`, qui
// fait aussi passer le téléviseur en HDR10 le temps de l'écran). Par-dessus,
// en bandeau bas et lisible de loin : le titre de l'étape, la consigne, la valeur et
// les boutons. Gauche et droite, à la télécommande ou à la manette, font
// varier la valeur quand elle a le focus (elle l'a à l'arrivée) ; bas mène
// aux boutons. Menu ou Retour : « Annuler ».

import KyberKit
import SwiftUI

struct HDRCalibrationView: View {
    @EnvironmentObject private var app: AppModel
    @StateObject private var model: HDRCalibrationModel
    @FocusState private var focus: Target?

    enum Target: Hashable { case value, back, next, cancel, automatic }

    init() {
        let screen = HDRCalibrationStore.ScreenRef.current() ?? HDRCalibrationStore.ScreenRef()
        let model = HDRCalibrationModel(screen: screen)
        #if DEBUG
        // `-calibrationStep 0…3` : l'écran ouvert sur cette étape (démonstration).
        for _ in 0..<min(3, max(0, UserDefaults.standard.integer(forKey: "calibrationStep"))) { model.next() }
        #endif
        _model = StateObject(wrappedValue: model)
    }

    var body: some View {
        ZStack(alignment: .bottom) {
            HDRPatternView(model: model).ignoresSafeArea()
            panel
                .padding(.horizontal, 72).padding(.bottom, 48)
        }
        .frame(width: 1920, height: 1080)
        .ignoresSafeArea()
        .defaultFocus($focus, .value)
        .onExitCommand { close() }
        .onChange(of: model.step) { _, _ in focus = .value }
    }

    /// Un bandeau bas, sous la mire (dont le symbole est au centre) : les
    /// textes à gauche, la valeur et les boutons à droite.
    private var panel: some View {
        HStack(alignment: .bottom, spacing: 40) {
            VStack(alignment: .leading, spacing: 12) {
                Theme.eyebrow(HDRText.calibration + " · \(model.step.rawValue + 1)/\(HDRCalibrationModel.Step.allCases.count)")
                    .font(Theme.mono(18))
                Text(HDRText.title(model.step))
                    .font(Theme.display(46, 660)).tracking(-46 * 0.035)
                    .foregroundStyle(Theme.text)
                    .lineLimit(1).minimumScaleFactor(0.7)
                if model.isFirst {
                    Text(HDRText.calibrationIntro).font(Theme.sans(21)).foregroundStyle(Theme.text2)
                }
                Text(HDRText.instruction(model.step)).font(Theme.sans(23)).foregroundStyle(Theme.text2)
                    .fixedSize(horizontal: false, vertical: true)
            }
            .frame(maxWidth: .infinity, alignment: .leading)
            VStack(alignment: .trailing, spacing: 18) {
                valueControl
                HStack(spacing: 14) {
                    if !model.isFirst {
                        button(.back, HDRText.back) { model.back() }
                    }
                    if model.isLast {
                        Button(HDRText.done) { finish() }
                            .buttonStyle(PrimaryButton(height: 60, fontSize: 22))
                            .focused($focus, equals: .next)
                    } else {
                        Button(HDRText.next) { model.next() }
                            .buttonStyle(PrimaryButton(height: 60, fontSize: 22))
                            .focused($focus, equals: .next)
                    }
                    button(.cancel, HDRText.cancel) { close() }
                    button(.automatic, HDRText.backToAutomatic) {
                        model.resetToAutomatic()
                        close()
                    }
                }
                .focusSection()
            }
            .fixedSize()
        }
        .padding(.horizontal, 36).padding(.vertical, 28)
        .frame(maxWidth: .infinity)
        .glass(28, tint: 0.7)
    }

    /// La valeur, focalisable : gauche et droite la font varier sans quitter
    /// le focus ; haut et bas le donnent aux boutons. Un appui : « Suivant » (ou « Terminer »).
    private var valueControl: some View {
        Button { model.isLast ? finish() : model.next() } label: {
            HStack(spacing: 28) {
                Icon("arrow-left", size: 30, bold: true).foregroundStyle(Theme.accentLight)
                Text(HDRText.nits(model.value, black: model.step == .black))
                    .font(Theme.mono(40, .medium)).foregroundStyle(Theme.text)
                    .monospacedDigit()
                    .frame(minWidth: 300)
                Icon("arrow-right", size: 30, bold: true).foregroundStyle(Theme.accentLight)
            }
            .padding(.horizontal, 30)
            .frame(height: 76)
        }
        .buttonStyle(GlassButton(radius: 22, scale: 1.04))
        .focused($focus, equals: .value)
        .onMoveCommand { direction in
            switch direction {
            case .left: model.adjust(-1)
            case .right: model.adjust(1)
            case .up: break
            case .down: focus = .next
            @unknown default: break
            }
        }
        .accessibilityLabel(HDRText.title(model.step))
        .accessibilityValue(HDRText.nits(model.value, black: model.step == .black))
        .accessibilityAdjustableAction { direction in
            switch direction {
            case .increment: model.adjust(1)
            case .decrement: model.adjust(-1)
            @unknown default: break
            }
        }
    }

    private func button(_ target: Target, _ title: String, action: @escaping () -> Void) -> some View {
        Button(action: action) {
            Text(title).font(Theme.sans(22, .medium))
                .padding(.horizontal, 24)
                .frame(height: 60)
        }
        .buttonStyle(GlassButton(radius: 18, scale: 1.05))
        .focused($focus, equals: target)
    }

    /// « Terminer » : gardé pour ce téléviseur ; en session, il vaudra à la suivante.
    private func finish() {
        model.done()
        let inSession = app.session != nil
        close()
        if inSession { app.show(notice: HDRText.nextSession) }
    }

    private func close() {
        app.screen = app.calibrationReturn
    }
}
