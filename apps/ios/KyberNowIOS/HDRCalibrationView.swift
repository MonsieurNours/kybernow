// L'écran « Réglage du HDR » (docs/hdr.md, « Réglage du HDR » ; textes de
// docs/link-ui.md, section HDR) : quatre étapes, chacune une mire dessinée
// en HDR par l'app (`HDRPatternView`, sans ordinateur), plein écran. Par-
// dessus, discret : l'étape, sa consigne, la valeur et les boutons. Glisser
// horizontalement, les boutons − et +, ou les flèches d'un clavier font
// varier la valeur (Maj : un cran fin). Les valeurs sont gardées pour cet
// écran de l'appareil (`HDRCalibrationStore`) et valent pour la session
// suivante.
//
// Seulement sur un écran HDR : Réglages de l'app (« Réglage du HDR ») et
// Réglages › Image (« Régler le HDR de cet écran… »).

import KyberKit
import SwiftUI
import UIKit

/// L'écran, avec son modèle, créé à l'ouverture pour l'écran de l'appareil.
struct HDRCalibrationCover: View {
    @EnvironmentObject private var app: AppModel
    @StateObject private var model: HDRCalibrationModel
    let close: () -> Void

    init(close: @escaping () -> Void) {
        // L'écran qui montre l'app (l'image d'une session sur un écran externe ne se règle pas ici).
        _model = StateObject(wrappedValue: HDRCalibrationModel(screen: HDRCalibrationStore.ScreenRef(UIScreen.main)))
        self.close = close
    }

    var body: some View {
        HDRCalibrationView(model: model, close: close) {
            // En session, la session en cours garde ses valeurs.
            app.session?.show(notice: HDRText.nextSession, for: 4)
        }
        .onAppear {
            #if DEBUG
            // `-calibrationStep 0…3` : l'étape montrée d'emblée (captures).
            for _ in 0..<min(3, max(0, UserDefaults.standard.integer(forKey: "calibrationStep"))) { model.next() }
            #endif
        }
    }
}

extension View {
    /// L'écran « Réglage du HDR » en plein écran.
    func hdrCalibrationCover(isPresented: Binding<Bool>) -> some View {
        fullScreenCover(isPresented: isPresented) {
            HDRCalibrationCover { isPresented.wrappedValue = false }
        }
    }
}

struct HDRCalibrationView: View {
    @ObservedObject var model: HDRCalibrationModel
    let close: () -> Void
    /// « Terminer » : après l'enregistrement, avant la fermeture.
    var finished: () -> Void = {}

    /// Glisser : crans déjà appliqués dans ce geste.
    @State private var dragSteps = 0
    @FocusState private var focused: Bool

    private var isPad: Bool { DeviceDisplay.isPad }
    /// Un cran tous les 14 points glissés.
    private static let stepWidth: CGFloat = 14

    var body: some View {
        ZStack(alignment: .bottom) {
            HDRPatternView(model: model)
                .ignoresSafeArea()
                .contentShape(Rectangle())
                .gesture(drag)
                .accessibilityHidden(true)
            panel
                .padding(.horizontal, isPad ? 0 : 12)
                .padding(.bottom, 12)
                .frame(maxWidth: 560)
        }
        .background(Color.black)
        .statusBarHidden()
        .persistentSystemOverlays(.hidden)
        .focusable()
        .focused($focused)
        .focusEffectDisabled()
        .onKeyPress(keys: [.leftArrow, .rightArrow, .upArrow, .downArrow]) { press in
            let up = press.key == .rightArrow || press.key == .upArrow
            model.adjust(up ? 1 : -1, fine: press.modifiers.contains(.shift))
            return .handled
        }
        .onKeyPress(.escape) {
            close()
            return .handled
        }
        .onAppear { focused = true }
        .preferredColorScheme(.dark)
    }

    /// Glisser vers la droite monte, vers la gauche baisse.
    private var drag: some Gesture {
        DragGesture(minimumDistance: 4)
            .onChanged { g in
                let steps = Int((g.translation.width / Self.stepWidth).rounded(.towardZero))
                let delta = steps - dragSteps
                guard delta != 0 else { return }
                for _ in 0..<abs(delta) { model.adjust(delta > 0 ? 1 : -1) }
                dragSteps = steps
            }
            .onEnded { _ in dragSteps = 0 }
    }

    private var panel: some View {
        VStack(alignment: .leading, spacing: 12) {
            HStack(alignment: .firstTextBaseline) {
                Theme.eyebrow("\(HDRText.calibration) · \(model.step.rawValue + 1) / \(HDRCalibrationModel.Step.allCases.count)")
                Spacer(minLength: 8)
                Button(HDRText.cancel, action: close)
                    .font(Theme.sans(14, .medium))
                    .foregroundStyle(Theme.text2)
                    .frame(minHeight: 44)
            }
            VStack(alignment: .leading, spacing: 4) {
                Text(HDRText.title(model.step))
                    .font(Theme.sans(isPad ? 22 : 19, .semibold))
                    .foregroundStyle(Theme.text)
                    .accessibilityAddTraits(.isHeader)
                if model.isFirst {
                    Text(HDRText.calibrationIntro)
                        .font(Theme.sans(13))
                        .foregroundStyle(Theme.muted)
                        .fixedSize(horizontal: false, vertical: true)
                }
                Text(HDRText.instruction(model.step))
                    .font(Theme.sans(14))
                    .foregroundStyle(Theme.text2)
                    .fixedSize(horizontal: false, vertical: true)
            }
            valueRow
            HStack(spacing: 10) {
                if model.wasCalibrated || HDRCalibrationStore.luminance(for: model.screen).calibrated {
                    Button(HDRText.backToAutomatic) { model.resetToAutomatic() }
                        .font(Theme.sans(13, .medium))
                        .foregroundStyle(Theme.accentLight)
                        .frame(minHeight: 44)
                        .lineLimit(2)
                }
                Spacer(minLength: 0)
                if !model.isFirst {
                    Button(HDRText.back) { model.back() }
                        .buttonStyle(GlassButton(height: 44, fontSize: 15))
                }
                Button(model.isLast ? HDRText.done : HDRText.next) {
                    if model.isLast {
                        model.done()
                        finished()
                        close()
                    } else {
                        model.next()
                    }
                }
                .buttonStyle(PrimaryButton(height: 44, fontSize: 15))
                .frame(width: isPad ? 150 : 128)
                .keyboardShortcut(.defaultAction)
            }
        }
        .padding(16)
        .glass(18, tint: 0.7)
        .animation(.easeOut(duration: 0.2), value: model.step)
    }

    /// − la valeur +, en chasse fixe ; la valeur suit chaque cran.
    private var valueRow: some View {
        HStack(spacing: 12) {
            Button { model.adjust(-1) } label: { Text(verbatim: "−").font(Theme.sans(22)) }
                .buttonStyle(HomeCircleStyle())
                .accessibilityLabel(Text(verbatim: "−"))
            Text(HDRText.nits(model.value, black: model.step == .black))
                .font(Theme.mono(isPad ? 26 : 22, .medium))
                .foregroundStyle(Theme.text)
                .monospacedDigit()
                .frame(maxWidth: .infinity)
                .contentTransition(.numericText())
            Button { model.adjust(1) } label: { Text(verbatim: "+").font(Theme.sans(22)) }
                .buttonStyle(HomeCircleStyle())
                .accessibilityLabel(Text(verbatim: "+"))
        }
        .accessibilityElement(children: .contain)
        .accessibilityValue(Text(HDRText.nits(model.value, black: model.step == .black)))
        .accessibilityAdjustableAction { direction in
            model.adjust(direction == .increment ? 1 : -1)
        }
    }
}
