// Réglages d'une console PlayStation sur l'iPhone et l'iPad (docs/link-ui.md,
// « Consoles PlayStation », « Réglages de la console ») : présentés comme
// ceux d'un ordinateur (SettingsView : retour, titre, rubriques en onglets
// sur iPhone, en colonne sur iPad, lignes de SettingsRows). Deux rubriques :
// Image et Commandes. Chaque changement est gardé tout de suite et vaut pour
// la partie suivante ; l'agrandissement, les intensités de la manette et la
// disposition du clavier et de la souris changent aussi tout de suite
// pendant une partie (`PlayStationStore`).
//
// Propre à l'iPhone et à l'iPad : pas de « plein écran » (la partie l'est
// toujours) ; pas de vibrations HD (iOS ne donne pas la carte son de la
// DualSense) ; clavier et souris seulement sur iPad, touches prises par
// GameController (usage HID) et boutons de la souris.

import GameController
import KyberKit
import SwiftUI

/// Les rubriques des réglages d'une console, dans cet ordre.
enum ConsoleSettingsSection: String, CaseIterable, Identifiable {
    case picture, controls

    var id: String { rawValue }

    var title: String {
        switch self {
        case .picture: return String(localized: "Image")
        case .controls: return String(localized: "Commandes")
        }
    }
}

extension PSLayout {
    /// La disposition par défaut du cœur.
    static let standard: PSLayout = (try? JSONDecoder().decode(PSLayout.self, from: Data(PlayStation.defaultLayout.utf8))) ?? PSLayout()
}

extension PSText {
    /// Aide des intensités sur iPhone et iPad (pas de vibrations HD).
    static var controllerHelpTouch: String { String(localized: "Pour la DualSense.") }
}

struct ConsoleSettingsView: View {
    let console: PlayStationConsole
    @ObservedObject var store: PlayStationStore

    var body: some View {
        ZStack {
            AmbientBackground()
            GeometryReader { g in
                ConsoleSettingsPane(console: store.console(console.id) ?? console, store: store, size: g.size)
            }
        }
        .preferredColorScheme(.dark)
        .presentationSizing(SettingsSheetSizing())
    }
}

private struct ConsoleSettingsPane: View {
    let console: PlayStationConsole
    @ObservedObject var store: PlayStationStore
    let size: CGSize
    @EnvironmentObject private var app: AppModel
    @StateObject private var catcher = ConsoleKeyCatcher()
    @State private var section: ConsoleSettingsSection
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    init(console: PlayStationConsole, store: PlayStationStore, size: CGSize) {
        self.console = console
        self.store = store
        self.size = size
        #if DEBUG
        _section = State(initialValue: UserDefaults.standard.string(forKey: "consoleSection").flatMap(ConsoleSettingsSection.init(rawValue:)) ?? .picture)
        #else
        _section = State(initialValue: .picture)
        #endif
    }

    private var s: PSSettings { store.settings(console) }
    private var l: PSLayout { store.layout(console) }
    /// La PS4 n'envoie que du H.264.
    private var ps4: Bool { !console.hasHDR }
    /// Clavier et souris : seulement sur iPad.
    private var keyboardAndMouse: Bool { DeviceDisplay.isPad }

    private var twoColumns: Bool { size.width >= 760 }
    private var sideWidth: CGFloat { min(380, max(280, size.width * 0.3)) }
    private var wideRows: Bool { twoColumns && size.width - sideWidth - 2 * gutter - 28 - 72 >= 600 }
    private var gutter: CGFloat { size.width >= 1100 ? 72 : 40 }
    private var rows: SettingsRows { SettingsRows(wide: wideRows) }

    private func update(_ change: (inout PSSettings) -> Void) {
        var v = store.settings(console)
        change(&v)
        store.setSettings(v, for: console)
    }

    private func updateLayout(_ change: (inout PSLayout) -> Void) {
        var v = store.layout(console)
        change(&v)
        store.setLayout(v, for: console)
    }

    var body: some View {
        Group {
            if twoColumns { wideLayout } else { compactLayout }
        }
        .onDisappear { catcher.stop() }
    }

    // MARK: - Mises en page

    private var title: some View {
        Text(LinkText.settingsTitle(console.name))
            .font(Theme.display(twoColumns ? 40 : 30))
            .tracking(twoColumns ? -40 * 0.035 : -30 * 0.03)
            .foregroundStyle(Theme.text)
            .lineLimit(2)
            .minimumScaleFactor(0.7)
            .fixedSize(horizontal: false, vertical: true)
            .accessibilityAddTraits(.isHeader)
    }

    /// Le modèle, et « Auto » ou « Personnalisé » (la puce de l'accueil).
    private var summary: some View {
        HStack(spacing: 10) {
            Theme.eyebrow(console.model)
            HStack(spacing: 8) {
                Icon("sliders-horizontal", size: 15, bold: true)
                Text(store.isCustom(console) ? PSText.custom : PSText.auto)
            }
            .font(Theme.sans(13.5, .semibold))
            .foregroundStyle(Theme.text2)
            .padding(.horizontal, 12)
            .frame(height: 32)
            .glass(10)
        }
    }

    private var backButton: some View {
        Button { close() } label: {
            HStack(spacing: 4) {
                Icon("arrow-left", size: 20, bold: true)
                Text("Retour").font(Theme.sans(16))
            }
            .foregroundStyle(Theme.accentLight)
            .padding(.leading, 6).padding(.trailing, 12)
            .frame(minHeight: 44)
            .contentShape(Rectangle())
        }
        .buttonStyle(SettingsPressStyle())
    }

    private func close() {
        catcher.stop()
        app.sheet = nil
    }

    /// iPhone : retour, titre, onglets des rubriques, détail.
    private var compactLayout: some View {
        VStack(alignment: .leading, spacing: 0) {
            backButton
                .padding(.leading, 12)
                .padding(.top, 8)
            VStack(alignment: .leading, spacing: 12) {
                title
                summary
            }
            .padding(.horizontal, 24)
            .padding(.top, 4)
            ScrollView(.horizontal, showsIndicators: false) {
                HStack(spacing: 6) {
                    ForEach(ConsoleSettingsSection.allCases) { sec in
                        let on = sec == section
                        Button { select(sec) } label: {
                            Text(sec.title)
                                .font(Theme.sans(14, .semibold))
                                .foregroundStyle(on ? Theme.background : Theme.text2)
                                .padding(.horizontal, 14)
                                .frame(minHeight: 44)
                                .background(Capsule().fill(on ? Theme.text : Color.white.opacity(0.07)))
                                .contentShape(Capsule())
                        }
                        .buttonStyle(SettingsPressStyle())
                        .accessibilityAddTraits(on ? [.isSelected, .isButton] : .isButton)
                    }
                }
                .padding(.horizontal, 24)
            }
            .padding(.top, 16)
            ScrollView {
                detail
                    .padding(.horizontal, 24)
                    .padding(.top, 14)
                    .padding(.bottom, 34)
            }
            .scrollBounceBehavior(.basedOnSize)
        }
    }

    /// iPad : retour en haut, rubriques à gauche, détail à droite.
    private var wideLayout: some View {
        VStack(spacing: 0) {
            HStack(spacing: 12) {
                Button { close() } label: {
                    HStack(spacing: 8) {
                        Icon("arrow-left", size: 18, bold: true)
                        Text("Retour")
                    }
                }
                .buttonStyle(GlassButton(height: 44, fontSize: 15))
                Spacer(minLength: 16)
                summary
            }
            .padding(.horizontal, gutter)
            .padding(.top, 28)

            HStack(alignment: .top, spacing: 28) {
                ScrollView {
                    VStack(alignment: .leading, spacing: 8) {
                        title.padding(.bottom, 16)
                        ForEach(ConsoleSettingsSection.allCases) { sec in
                            sectionButton(sec)
                        }
                    }
                    .padding(.bottom, 32)
                }
                .scrollBounceBehavior(.basedOnSize)
                .frame(width: sideWidth)

                ScrollView {
                    detail
                        .padding(.horizontal, 36)
                        .padding(.vertical, 32)
                }
                .scrollBounceBehavior(.basedOnSize)
                .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .top)
                .glass(26, tint: 0.58)
                .clipShape(RoundedRectangle(cornerRadius: 26, style: .continuous))
            }
            .padding(.horizontal, gutter)
            .padding(.top, 34)
            .padding(.bottom, 40)
        }
    }

    private func sectionButton(_ sec: ConsoleSettingsSection) -> some View {
        let on = sec == section
        return Button { select(sec) } label: {
            HStack(spacing: 12) {
                VStack(alignment: .leading, spacing: 3) {
                    Text(sec.title).font(Theme.sans(17, .semibold)).foregroundStyle(Theme.text)
                    Text(verbatim: subtitle(sec)).font(Theme.sans(13)).foregroundStyle(Theme.muted).lineLimit(1)
                }
                Spacer(minLength: 8)
                Icon("arrow-right", size: 18, bold: true).foregroundStyle(Theme.faint)
            }
            .padding(.horizontal, 18)
            .frame(maxWidth: .infinity, minHeight: 72)
            .background(RoundedRectangle(cornerRadius: 16, style: .continuous).fill(on ? Color.white.opacity(0.1) : Color.clear))
            .overlay(RoundedRectangle(cornerRadius: 16, style: .continuous).strokeBorder(on ? Color.white.opacity(0.16) : Color.clear))
            .contentShape(Rectangle())
        }
        .buttonStyle(SettingsPressStyle())
        .hoverEffect(.highlight)
        .accessibilityAddTraits(on ? [.isSelected, .isButton] : .isButton)
    }

    private func select(_ sec: ConsoleSettingsSection) {
        catcher.stop()
        if reduceMotion { section = sec } else { withAnimation(Theme.easeOut) { section = sec } }
    }

    /// Ce que contient chaque rubrique, en une ligne (iPad).
    private func subtitle(_ sec: ConsoleSettingsSection) -> String {
        switch sec {
        case .picture: return "\(PSSettings.resolutionLabel(s.resolution)) · \(PSText.fps(s.fps))"
        case .controls:
            guard keyboardAndMouse else { return PSText.controller }
            return l == .standard ? PSText.defaultLayout : PSText.custom
        }
    }

    @ViewBuilder private var detail: some View {
        VStack(alignment: .leading, spacing: 10) {
            switch section {
            case .picture: picture
            case .controls: controls
            }
        }
        .id(section)
        .transition(.opacity)
    }

    private func heading(_ text: String) -> some View {
        Theme.eyebrow(text)
            .padding(.top, 14)
            .padding(.bottom, 2)
            .accessibilityAddTraits(.isHeader)
    }

    // MARK: - Image

    @ViewBuilder private var picture: some View {
        rows.choice(PSText.resolution, s.resolution == "auto" ? PSText.resolutionAutoHelp : "",
                    PSSettings.resolutions.map { SettingsOption(key: $0, label: PSSettings.resolutionLabel($0)) }, s.resolution) { r in
            update { $0.resolution = r }
        }
        rows.choice(PSText.frameRate, "", [SettingsOption(key: "60", label: PSText.fps(60)), SettingsOption(key: "30", label: PSText.fps(30))],
                    s.fps == 30 ? "30" : "60") { f in
            update { $0.fps = Int(f) ?? 60 }
        }
        bitrate
        rows.choice(PSText.format, formatHelp,
                    PSSettings.codecs.map { SettingsOption(key: $0, label: $0 == "h264" ? "H.264" : $0 == "h265" ? "H.265" : PSText.auto) },
                    ps4 ? "h264" : s.codec, enabled: !ps4) { c in
            update { $0.codec = c }
        }
        rows.toggle(PSText.upscale, PSText.upscaleHelpDevice, s.upscale) {
            update { $0.upscale.toggle() }
        }
        rows.slider(PSText.sharpness, "", value: Double(s.sharpness), in: 0...100, step: 5,
                    display: { PSText.percent(Int($0.rounded())) }) { v in
            update { $0.sharpness = min(max(Int((v / 5).rounded()) * 5, 0), 100) }
        }
        .disabled(!s.upscale)
        .opacity(s.upscale ? 1 : 0.45)
    }

    private var formatHelp: String {
        if ps4 { return PSText.formatPS4Help }
        switch s.codec {
        case "auto": return PSText.formatAutoHelp
        case "h264": return PSText.formatH264Help
        default: return ""
        }
    }

    /// Débit : « Auto » (suit la définition) ou au choix, de 2 à 100 Mbit/s.
    @ViewBuilder private var bitrate: some View {
        let auto = s.bitrateKbps == nil
        let range = PSSettings.bitrateRange
        let mbps = (s.bitrateKbps ?? PSSettings.autoBitrate(s.resolution)) / 1000
        rows.choice(PSText.bitrate, auto ? PSText.bitrateAutoHelp : "",
                    [SettingsOption(key: "auto", label: PSText.auto), SettingsOption(key: "manual", label: PSText.mbps(mbps))],
                    auto ? "auto" : "manual") { k in
            update { $0.bitrateKbps = k == "auto" ? nil : ($0.bitrateKbps ?? PSSettings.autoBitrate($0.resolution)) }
        }
        if !auto {
            rows.slider(PSText.bitrate, "", value: Double(mbps), in: Double(range.lowerBound)...Double(range.upperBound), step: 1,
                        display: { PSText.mbps(Int($0.rounded())) }) { v in
                update { $0.bitrateKbps = min(max(Int(v.rounded()), range.lowerBound), range.upperBound) * 1000 }
            }
        }
    }

    // MARK: - Commandes

    @ViewBuilder private var controls: some View {
        heading(PSText.controller)
        rows.slider(PSText.rumble, "", value: Double(s.rumble), in: 0...100, step: 10,
                    display: { PSText.percent(Int($0.rounded())) }) { v in
            update { $0.rumble = min(max(Int((v / 10).rounded()) * 10, 0), 100) }
        }
        rows.slider(PSText.adaptiveTriggers, "", value: Double(s.triggers), in: 0...100, step: 10,
                    display: { PSText.percent(Int($0.rounded())) }) { v in
            update { $0.triggers = min(max(Int((v / 10).rounded()) * 10, 0), 100) }
        }
        Text(PSText.controllerHelpTouch)
            .font(Theme.sans(12)).foregroundStyle(Theme.muted)
            .fixedSize(horizontal: false, vertical: true)
            .padding(.horizontal, 4)
        if keyboardAndMouse {
            keys
            mouse
        }
    }

    /// Les actions de la manette et leurs touches (iPad).
    @ViewBuilder private var keys: some View {
        heading(String(localized: "Clavier"))
        Text(PSText.controlsHelp)
            .font(Theme.sans(12)).foregroundStyle(Theme.muted)
            .fixedSize(horizontal: false, vertical: true)
            .padding(.horizontal, 4)
        rows.container {
            VStack(spacing: 0) {
                ForEach(PSLayout.actions, id: \.self) { action in
                    actionRow(action)
                    if action != PSLayout.actions.last {
                        Rectangle().fill(Theme.line).frame(height: 1)
                    }
                }
            }
        }
        Button { catcher.stop(); store.resetLayout(for: console) } label: {
            HStack(spacing: 8) {
                Image(systemName: "arrow.counterclockwise").font(.system(size: 14, weight: .bold))
                Text(PSText.defaultLayout)
            }
        }
        .buttonStyle(GlassButton(height: 44, fontSize: 15))
        .disabled(l == .standard)
        .opacity(l == .standard ? 0.45 : 1)
        .padding(.top, 4)
    }

    private func actionRow(_ action: String) -> some View {
        let inputs = l.inputs(action)
        let adding = catcher.waiting == ConsoleKeyCatcher.Wait(action: action, replacing: nil)
        return HStack(spacing: 8) {
            Text(PSText.action(action))
                .font(Theme.sans(14)).foregroundStyle(Theme.text2)
                .lineLimit(1).minimumScaleFactor(0.8)
            Spacer(minLength: 8)
            if inputs.isEmpty, !adding {
                Text(PSText.none).font(Theme.sans(13)).foregroundStyle(Theme.faint)
            }
            ForEach(inputs, id: \.self) { input in
                if catcher.waiting == ConsoleKeyCatcher.Wait(action: action, replacing: input) {
                    waitingChip
                } else {
                    Button { listen(action, replacing: input) } label: { chip(PSText.input(input)) }
                        .buttonStyle(SettingsPressStyle())
                }
            }
            if adding {
                waitingChip
            } else {
                Button { listen(action, replacing: nil) } label: {
                    Icon("plus", size: 14, bold: true)
                        .foregroundStyle(Theme.muted)
                        .frame(width: 36, height: 36)
                        .background(Color.white.opacity(0.06), in: RoundedRectangle(cornerRadius: 8, style: .continuous))
                        .contentShape(Rectangle())
                }
                .buttonStyle(SettingsPressStyle())
                .accessibilityLabel(Text(verbatim: "+ \(PSText.action(action))"))
            }
        }
        .frame(minHeight: 44)
        .padding(.vertical, 2)
    }

    private func chip(_ text: String) -> some View {
        // ↩, ⌫… en signes de texte, pas en emoji.
        Text(verbatim: text.count == 1 ? text + "\u{FE0E}" : text)
            .font(Theme.sans(13, .semibold))
            .foregroundStyle(Theme.text)
            .lineLimit(1)
            .fixedSize()
            .padding(.horizontal, 12)
            .frame(minWidth: 36, minHeight: 36)
            .background(Color.white.opacity(0.1), in: RoundedRectangle(cornerRadius: 8, style: .continuous))
            .contentShape(Rectangle())
    }

    private var waitingChip: some View {
        Text(PSText.pressKey)
            .font(Theme.sans(13, .semibold))
            .foregroundStyle(Theme.accentLight)
            .lineLimit(1)
            .fixedSize()
            .padding(.horizontal, 12)
            .frame(minHeight: 36)
            .background(Theme.accentFill.opacity(0.18), in: RoundedRectangle(cornerRadius: 8, style: .continuous))
            .overlay(RoundedRectangle(cornerRadius: 8, style: .continuous).strokeBorder(Theme.accent.opacity(0.6)))
    }

    /// Attend la touche ou le bouton de la souris de cette action.
    private func listen(_ action: String, replacing: PSInput?) {
        let store = store, console = console
        catcher.start(ConsoleKeyCatcher.Wait(action: action, replacing: replacing)) { wait, outcome in
            var v = store.layout(console)
            switch outcome {
            case let .input(input): v.bind(input, to: wait.action, replacing: wait.replacing)
            case .remove:
                guard let r = wait.replacing else { return }
                v.unbind(r, from: wait.action)
            case .cancel: return
            }
            store.setLayout(v, for: console)
        }
    }

    /// La souris (iPad) : stick droit, sensibilité, courbe, inclinaison minimale, axe vertical.
    @ViewBuilder private var mouse: some View {
        let curve = PSLayout.curves.enumerated().min { abs($0.element - l.curve) < abs($1.element - l.curve) }?.offset ?? 2
        heading(String(localized: "Souris"))
        rows.toggle(PSText.mouseStick, "", l.mouseStick) { updateLayout { $0.mouseStick.toggle() } }
        Group {
            rows.slider(PSText.sensitivity, "", value: l.sensitivity, in: 0.2...5, step: 0.1,
                        display: { $0.formatted(.number.precision(.fractionLength(1))) }) { v in
                updateLayout { $0.sensitivity = (v * 10).rounded() / 10 }
            }
            rows.choice(PSText.startingResponse, "", PSText.curves.enumerated().map { SettingsOption(key: String($0.offset), label: $0.element) },
                        String(curve)) { k in
                if let i = Int(k), PSLayout.curves.indices.contains(i) { updateLayout { $0.curve = PSLayout.curves[i] } }
            }
            rows.slider(PSText.minimumTilt, PSText.minimumTiltHelp, value: l.deadzone, in: 0...0.5, step: 0.01,
                        display: { $0.formatted(.percent.precision(.fractionLength(0))) }) { v in
                updateLayout { $0.deadzone = (v * 100).rounded() / 100 }
            }
            rows.toggle(PSText.invertY, "", l.invertY) { updateLayout { $0.invertY.toggle() } }
        }
        .disabled(!l.mouseStick)
        .opacity(l.mouseStick ? 1 : 0.45)
    }
}

/// Attend la touche (clavier branché, usage HID par GameController) ou le
/// bouton de la souris d'une action : Échap annule, Effacement retire la
/// touche qu'on changeait. Les gestionnaires d'avant (une partie en cours
/// sous les réglages) sont rendus à la fin.
@MainActor
final class ConsoleKeyCatcher: ObservableObject {
    struct Wait: Equatable {
        var action: String
        /// La touche qu'on change (nil : une de plus).
        var replacing: PSInput?
    }

    enum Outcome {
        case input(PSInput)
        case remove
        case cancel
    }

    @Published private(set) var waiting: Wait?
    private var done: ((Wait, Outcome) -> Void)?
    private var savedKeys: GCKeyboardValueChangedHandler??
    private var savedButtons: [(GCControllerButtonInput, GCControllerButtonValueChangedHandler?)] = []

    func start(_ wait: Wait, done: @escaping (Wait, Outcome) -> Void) {
        stop()
        waiting = wait
        self.done = done
        if let input = GCKeyboard.coalesced?.keyboardInput {
            savedKeys = .some(input.keyChangedHandler)
            input.keyChangedHandler = { [weak self] _, _, code, pressed in
                guard pressed else { return }
                let usage = UInt16(truncatingIfNeeded: code.rawValue)
                DispatchQueue.main.async { self?.key(usage) }
            }
        }
        for mouse in GCMouse.mice() {
            guard let m = mouse.mouseInput else { continue }
            let list: [(GCControllerButtonInput?, UInt8)] = [(m.leftButton, 0), (m.rightButton, 1), (m.middleButton, 2)]
                + (m.auxiliaryButtons ?? []).prefix(2).enumerated().map { ($0.element, UInt8(3 + $0.offset)) }
            for case let (button?, index) in list {
                savedButtons.append((button, button.pressedChangedHandler))
                button.pressedChangedHandler = { [weak self] _, _, pressed in
                    guard pressed else { return }
                    DispatchQueue.main.async { self?.finish(.input(.mouse(index))) }
                }
            }
        }
    }

    func stop() {
        if let saved = savedKeys { GCKeyboard.coalesced?.keyboardInput?.keyChangedHandler = saved }
        savedKeys = nil
        for (button, handler) in savedButtons { button.pressedChangedHandler = handler }
        savedButtons = []
        waiting = nil
        done = nil
    }

    private func key(_ usage: UInt16) {
        switch usage {
        case 0x29: finish(.cancel)  // Échap
        case 0x2A: finish(.remove)  // Effacement
        default: finish(.input(.key(usage)))
        }
    }

    private func finish(_ outcome: Outcome) {
        guard let wait = waiting else { return }
        let callback = done
        stop()
        callback?(wait, outcome)
    }
}
