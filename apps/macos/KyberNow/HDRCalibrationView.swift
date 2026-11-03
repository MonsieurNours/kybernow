// L'écran « Réglage du HDR » (docs/hdr.md, « Réglage du HDR » ; textes de
// docs/link-ui.md, section HDR) : une fenêtre sans bord qui couvre l'écran
// concerné, la mire de l'étape en HDR (`HDRPatternView`) en fond, et par-dessus
// un panneau discret : titre, consigne, valeur, boutons. Flèches gauche et
// droite (⌥ : un cran fin), molette ou trackpad font varier la valeur ;
// Entrée = Suivant ou Terminer, Échap = Annuler. Les valeurs sont gardées
// pour cet écran du Mac et valent pour la session suivante.

import AppKit
import KyberKit
import SwiftUI

/// La fenêtre du réglage : une à la fois, sur l'écran à régler.
@MainActor
final class HDRCalibrationWindow {
    /// Le réglage ouvert, s'il y en a un.
    private static var current: HDRCalibrationWindow?
    /// Le réglage d'un écran a été gardé ou remis en automatique.
    static let changed = Notification.Name("fr.leslignesclaires.kybernow.hdrCalibrationChanged")

    private let window: NSWindow
    private let model: HDRCalibrationModel
    private var monitor: Any?
    private var scroll = 0.0

    /// Ouvre le réglage de l'écran `screen` (celui de la fenêtre d'où il est
    /// demandé). `step` : étape de départ (démonstration).
    static func open(on screen: NSScreen?, step: Int = 0) {
        guard let screen = screen ?? NSScreen.main, let ref = HDRCalibrationStore.ScreenRef.current(screen) else { return }
        current?.close()
        let model = HDRCalibrationModel(screen: ref)
        for _ in 0..<max(0, min(step, HDRCalibrationModel.Step.allCases.count - 1)) { model.next() }
        current = HDRCalibrationWindow(screen: screen, model: model)
    }

    private init(screen: NSScreen, model: HDRCalibrationModel) {
        self.model = model
        let window = CalibrationNSWindow(contentRect: screen.frame, styleMask: [.borderless], backing: .buffered, defer: false)
        window.title = HDRText.calibration
        window.isReleasedWhenClosed = false
        window.backgroundColor = .black
        // Par-dessus la barre des menus : la luminosité sur tout l'écran se règle sur tout l'écran.
        window.level = .mainMenu + 1
        window.collectionBehavior = [.fullScreenAuxiliary, .moveToActiveSpace]
        window.setFrame(screen.frame, display: false)
        self.window = window
        let inSession = !AppModel.shared.sessions.isEmpty
        window.contentView = NSHostingView(rootView: HDRCalibrationView(model: model, inSession: inSession) { [weak self] in self?.close() })
        window.makeKeyAndOrderFront(nil)
        NSApp.activate(ignoringOtherApps: true)
        monitor = NSEvent.addLocalMonitorForEvents(matching: [.keyDown, .scrollWheel]) { [weak self] event in
            guard let self, event.window === self.window else { return event }
            return self.handle(event) ? nil : event
        }
        KyberLog.app.notice("réglage du HDR ouvert sur l'écran \(screen.localizedName, privacy: .public)")
    }

    /// Flèches, Entrée, Échap, molette (fil principal). Vrai si l'événement est pris.
    private func handle(_ event: NSEvent) -> Bool {
        if event.type == .scrollWheel {
            // Un cran tous les 6 points de trackpad, chaque cran de molette.
            let delta = event.hasPreciseScrollingDeltas ? event.scrollingDeltaY / 6 : event.scrollingDeltaY
            scroll += delta
            while abs(scroll) >= 1 {
                let up = scroll > 0
                model.adjust(up ? 1 : -1, fine: event.modifierFlags.contains(.option))
                scroll += up ? -1 : 1
            }
            return true
        }
        let fine = event.modifierFlags.contains(.option)
        switch event.keyCode {
        case 123, 125: model.adjust(-1, fine: fine) // ← ↓
        case 124, 126: model.adjust(1, fine: fine) // → ↑
        case 36, 76: // Entrée
            if model.isLast { finish() } else { model.next() }
        case 53: close() // Échap
        default: return false
        }
        return true
    }

    /// « Terminer » : les valeurs sont gardées pour cet écran.
    private func finish() {
        model.done()
        close()
    }

    func close() {
        if let monitor { NSEvent.removeMonitor(monitor) }
        monitor = nil
        window.orderOut(nil)
        window.close()
        NotificationCenter.default.post(name: Self.changed, object: nil)
        if Self.current === self { Self.current = nil }
    }
}

/// Fenêtre sans bord qui prend le clavier.
private final class CalibrationNSWindow: NSWindow {
    override var canBecomeKey: Bool { true }
    override var canBecomeMain: Bool { false }
}

/// La mire en fond, le panneau par-dessus, en bas.
struct HDRCalibrationView: View {
    @ObservedObject var model: HDRCalibrationModel
    /// Une session est en cours : elle garde ses valeurs.
    let inSession: Bool
    let close: () -> Void

    var body: some View {
        ZStack(alignment: .bottom) {
            HDRPatternView(model: model)
                .ignoresSafeArea()
            panel
                .padding(.bottom, 48)
        }
        .preferredColorScheme(.dark)
    }

    private var black: Bool { model.step == .black }

    private var panel: some View {
        VStack(alignment: .leading, spacing: 14) {
            HStack(alignment: .firstTextBaseline) {
                Text(HDRText.title(model.step))
                    .font(Theme.display(22, .bold))
                    .foregroundStyle(Theme.text)
                Spacer(minLength: 16)
                Text(verbatim: "\(model.step.rawValue + 1) / \(HDRCalibrationModel.Step.allCases.count)")
                    .font(Theme.mono(12)).foregroundStyle(Theme.faint)
            }
            if model.isFirst {
                Text(HDRText.calibrationIntro).font(Theme.sans(13)).foregroundStyle(Theme.muted)
                    .fixedSize(horizontal: false, vertical: true)
            }
            Text(HDRText.instruction(model.step)).font(Theme.sans(14)).foregroundStyle(Theme.text2)
                .fixedSize(horizontal: false, vertical: true)
            valueRow
            if inSession {
                Text(HDRText.nextSession).font(Theme.sans(12)).foregroundStyle(Theme.muted)
            }
            HStack(spacing: 10) {
                Button { model.resetToAutomatic() } label: {
                    Text(HDRText.backToAutomatic).lineLimit(1).fixedSize()
                }
                .buttonStyle(GhostButton(height: 36, tint: Theme.text2))
                Spacer(minLength: 16)
                Button(HDRText.cancel, action: close)
                    .buttonStyle(GhostButton(height: 36))
                if !model.isFirst {
                    Button(HDRText.back) { model.back() }
                        .buttonStyle(GhostButton(height: 36))
                }
                Button(model.isLast ? HDRText.done : HDRText.next) {
                    if model.isLast {
                        model.done()
                        close()
                    } else {
                        model.next()
                    }
                }
                .buttonStyle(PrimaryButton(height: 40))
            }
        }
        .padding(22)
        .frame(width: 640)
        .glass(16, tint: 0.7)
    }

    /// La valeur et ses deux crans ; ajustable au clavier par VoiceOver.
    private var valueRow: some View {
        HStack(spacing: 14) {
            step(-1, icon: "arrow-left")
            Text(verbatim: HDRText.nits(model.value, black: black))
                .font(Theme.mono(20)).foregroundStyle(Theme.text)
                .frame(minWidth: 150)
                .contentTransition(.numericText())
            step(1, icon: "arrow-right")
        }
        .frame(maxWidth: .infinity)
        .accessibilityElement(children: .ignore)
        .accessibilityLabel(HDRText.title(model.step))
        .accessibilityValue(HDRText.nits(model.value, black: black))
        .accessibilityAdjustableAction { direction in
            switch direction {
            case .increment: model.adjust(1)
            case .decrement: model.adjust(-1)
            @unknown default: break
            }
        }
    }

    private func step(_ direction: Int, icon: String) -> some View {
        Button { model.adjust(direction, fine: NSEvent.modifierFlags.contains(.option)) } label: {
            Icon(icon, size: 16, bold: true).frame(width: 36, height: 32)
        }
        .buttonStyle(GhostButton(height: 32))
    }
}
